use std::process::{Command, Stdio};
use std::io::{Write, Read, BufReader};
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LspDiagnosticsUpdate {
    pub file_path: String,
    pub errors: usize,
    pub warnings: usize,
}

pub enum LspCommand {
    OpenFile { path: String, text: String },
    ChangeFile { path: String, text: String },
    SaveFile { path: String },
    SetActiveFile { path: String },
}

pub struct LspClient {
    cmd_tx: Sender<LspCommand>,
}

struct ServerInstance {
    stdin: std::process::ChildStdin,
    cmd_name: String,
}

fn spawn_server(lang_id: &str) -> Result<(std::process::Child, String), String> {
    let alternatives = match lang_id {
        "rust" => vec![("rust-analyzer", vec![])],
        "python" => vec![
            ("pyright-langserver", vec!["--stdio"]),
            ("pylsp", vec![]),
        ],
        "go" => vec![("gopls", vec![])],
        "c" | "cpp" => vec![("clangd", vec![])],
        "typescript" | "javascript" => vec![
            ("typescript-language-server", vec!["--stdio"]),
            ("deno", vec!["lsp"]),
        ],
        _ => return Err("No configured LSP server for this language".to_string()),
    };

    let mut last_err = String::new();
    for (cmd, args) in alternatives {
        log::info!("LSP: Trying to spawn {} {:?}", cmd, args);
        match Command::new(cmd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                return Ok((child, cmd.to_string()));
            }
            Err(e) => {
                last_err = format!("{} not found ({:?})", cmd, e.kind());
            }
        }
    }
    Err(last_err)
}

impl LspClient {
    pub fn new(
        diagnostics_tx: Sender<LspDiagnosticsUpdate>,
        event_loop_proxy: winit::event_loop::EventLoopProxy<()>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<LspCommand>();
        
        let proxy_init = event_loop_proxy.clone();
        std::thread::spawn(move || {
            let mut document_versions = HashMap::<String, usize>::new();
            let mut servers = HashMap::<String, ServerInstance>::new();
            let pending_changes = Arc::new(Mutex::new(HashMap::<String, (String, usize, bool)>::new()));
            
            let pending_changes_clone = pending_changes.clone();
            loop {
                // 1. Flush any pending changes to active servers
                let mut changes_to_flush = Vec::new();
                if let Ok(mut pending) = pending_changes_clone.lock() {
                    for (path, (text, version, needs_send)) in pending.iter_mut() {
                        if *needs_send {
                            changes_to_flush.push((path.clone(), text.clone(), *version));
                            *needs_send = false;
                        }
                    }
                }

                for (path, text, version) in changes_to_flush {
                    let lang_id = detect_language_id(&path);
                    if let Some(server) = servers.get_mut(lang_id) {
                        let change_msg = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didChange",
                            "params": {
                                "textDocument": {
                                    "uri": format!("file://{}", path),
                                    "version": version
                                },
                                "contentChanges": [
                                    { "text": text }
                                ]
                            }
                        });
                        let _ = write_message(&mut server.stdin, &change_msg.to_string());
                    }
                }

                // 2. Receive commands with a timeout to allow flushing
                match cmd_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(LspCommand::OpenFile { path, text }) => {
                        let lang_id = detect_language_id(&path);
                        if lang_id != "plaintext" {
                            let mut server_ready = servers.contains_key(lang_id);
                            
                            if !server_ready {
                                match spawn_server(lang_id) {
                                    Ok((mut child, cmd_name)) => {
                                        let mut stdin = child.stdin.take().expect("Failed to open stdin");
                                        let stdout = child.stdout.take().expect("Failed to open stdout");
                                        let mut stdout_reader = BufReader::new(stdout);
                                        
                                        // Handshake
                                        let current_dir = std::env::current_dir().unwrap_or_default();
                                        let root_uri = format!("file://{}", current_dir.to_string_lossy());
                                        let init_msg = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": 1,
                                            "method": "initialize",
                                            "params": {
                                                "processId": std::process::id(),
                                                "rootUri": root_uri,
                                                "capabilities": {
                                                    "textDocument": {
                                                        "publishDiagnostics": {
                                                            "relatedInformation": true
                                                        },
                                                        "synchronization": {
                                                            "didOpen": true,
                                                            "didChange": true,
                                                            "didSave": true,
                                                            "willSave": false,
                                                            "willSaveWaitUntil": false
                                                        }
                                                    }
                                                },
                                                "workspaceFolders": [{
                                                    "uri": root_uri,
                                                    "name": current_dir.file_name()
                                                        .and_then(|n| n.to_str())
                                                        .unwrap_or("workspace")
                                                }]
                                            }
                                        });

                                        log::info!("LSP: Sending initialize to {}...", cmd_name);
                                        if write_message(&mut stdin, &init_msg.to_string()).is_ok() {
                                            if let Ok(_resp_str) = read_message(&mut stdout_reader) {
                                                log::info!("LSP: Initialize response from {} received", cmd_name);
                                                let initialized_msg = serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "method": "initialized",
                                                    "params": {}
                                                });
                                                let _ = write_message(&mut stdin, &initialized_msg.to_string());
                                                
                                                let diag_tx = diagnostics_tx.clone();
                                                let proxy_reader = proxy_init.clone();
                                                std::thread::spawn(move || {
                                                    loop {
                                                        match read_message(&mut stdout_reader) {
                                                            Ok(resp_str) => {
                                                                if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&resp_str) {
                                                                    if resp["method"] == "textDocument/publishDiagnostics" {
                                                                        if let Some(params) = resp.get("params") {
                                                                            let uri = params["uri"].as_str().unwrap_or("");
                                                                            let file_path = uri.trim_start_matches("file://").to_string();
                                                                            
                                                                            let mut errors = 0;
                                                                            let mut warnings = 0;
                                                                            if let Some(diags) = params["diagnostics"].as_array() {
                                                                                for d in diags {
                                                                                    let severity = d["severity"].as_i64().unwrap_or(1);
                                                                                    if severity == 1 {
                                                                                        errors += 1;
                                                                                    } else if severity == 2 {
                                                                                        warnings += 1;
                                                                                    }
                                                                                }
                                                                            }
                                                                            let _ = diag_tx.send(LspDiagnosticsUpdate {
                                                                                file_path,
                                                                                errors,
                                                                                warnings,
                                                                            });
                                                                            let _ = proxy_reader.send_event(());
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                log::warn!("LSP: Reader thread exit: {:?}", e);
                                                                break;
                                                            }
                                                        }
                                                    }
                                                });

                                                let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                                    file_path: format!("status:{}", cmd_name),
                                                    errors: 0,
                                                    warnings: 0,
                                                });
                                                let _ = proxy_init.send_event(());
                                                
                                                servers.insert(lang_id.to_string(), ServerInstance { stdin, cmd_name });
                                                server_ready = true;
                                            }
                                        }
                                    }
                                    Err(e_msg) => {
                                        log::warn!("LSP: Failed to start server for {}: {}", lang_id, e_msg);
                                        let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                            file_path: "status:offline".to_string(),
                                            errors: 9999,
                                            warnings: 0,
                                        });
                                        let _ = proxy_init.send_event(());
                                    }
                                }
                            }

                            if server_ready {
                                if let Some(server) = servers.get_mut(lang_id) {
                                    let version = document_versions.entry(path.clone()).or_insert(0);
                                    *version += 1;
                                    let open_msg = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "method": "textDocument/didOpen",
                                        "params": {
                                            "textDocument": {
                                                "uri": format!("file://{}", path),
                                                "languageId": lang_id,
                                                "version": *version,
                                                "text": text
                                            }
                                        }
                                    });
                                    let _ = write_message(&mut server.stdin, &open_msg.to_string());
                                }
                            }
                        }
                    }
                    Ok(LspCommand::ChangeFile { path, text }) => {
                        let lang_id = detect_language_id(&path);
                        if servers.contains_key(lang_id) {
                            let version = document_versions.entry(path.clone()).or_insert(0);
                            *version += 1;
                            if let Ok(mut pending) = pending_changes_clone.lock() {
                                pending.insert(path, (text, *version, true));
                            }
                        }
                    }
                    Ok(LspCommand::SaveFile { path }) => {
                        let lang_id = detect_language_id(&path);
                        if let Some(server) = servers.get_mut(lang_id) {
                            let save_msg = serde_json::json!({
                                "jsonrpc": "2.0",
                                "method": "textDocument/didSave",
                                "params": {
                                    "textDocument": {
                                        "uri": format!("file://{}", path)
                                    }
                                }
                            });
                            let _ = write_message(&mut server.stdin, &save_msg.to_string());
                        }
                    }
                    Ok(LspCommand::SetActiveFile { path }) => {
                        let lang_id = detect_language_id(&path);
                        if path.is_empty() || lang_id == "plaintext" {
                            let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                file_path: "status:none".to_string(),
                                errors: 0,
                                warnings: 0,
                            });
                        } else if let Some(server) = servers.get(lang_id) {
                            let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                file_path: format!("status:{}", server.cmd_name),
                                errors: 0,
                                warnings: 0,
                            });
                        } else {
                            let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                file_path: "status:offline".to_string(),
                                errors: 9999,
                                warnings: 0,
                            });
                        }
                        let _ = proxy_init.send_event(());
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        Self { cmd_tx }
    }

    pub fn notify_open(&self, path: &str, text: String) {
        let abs_path = get_absolute_path(path);
        let _ = self.cmd_tx.send(LspCommand::OpenFile { path: abs_path, text });
    }

    pub fn notify_change(&self, path: &str, text: String) {
        let abs_path = get_absolute_path(path);
        let _ = self.cmd_tx.send(LspCommand::ChangeFile { path: abs_path, text });
    }

    pub fn notify_save(&self, path: &str) {
        let abs_path = get_absolute_path(path);
        let _ = self.cmd_tx.send(LspCommand::SaveFile { path: abs_path });
    }

    pub fn notify_active_file(&self, path: &str) {
        let abs_path = if path.is_empty() {
            "".to_string()
        } else {
            get_absolute_path(path)
        };
        let _ = self.cmd_tx.send(LspCommand::SetActiveFile { path: abs_path });
    }
}

fn read_message<R: Read>(reader: &mut R) -> Result<String, Box<dyn std::error::Error>> {
    let mut header = String::new();
    let mut buf = [0u8; 1];
    
    loop {
        reader.read_exact(&mut buf)?;
        header.push(buf[0] as char);
        if header.ends_with("\r\n\r\n") {
            break;
        }
    }
    
    let mut content_len = 0;
    for line in header.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            if let Some(len_str) = line.split(':').nth(1) {
                content_len = len_str.trim().parse::<usize>()?;
            }
        }
    }
    
    if content_len == 0 {
        return Err("Content-Length not found or 0".into());
    }
    
    let mut body = vec![0u8; content_len];
    reader.read_exact(&mut body)?;
    
    Ok(String::from_utf8(body)?)
}

fn write_message<W: Write>(writer: &mut W, msg: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg);
    writer.write_all(content.as_bytes())?;
    writer.flush()?;
    Ok(())
}

fn detect_language_id(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "rs" => "rust",
        "toml" => "toml",
        "json" => "json",
        "md" => "markdown",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "jsx" => "javascriptreact",
        "html" => "html",
        "css" => "css",
        "scss" => "scss",
        "yaml" | "yml" => "yaml",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" | "hpp" => "cpp",
        "go" => "go",
        "sh" | "bash" => "shellscript",
        "lua" => "lua",
        "rb" => "ruby",
        "java" => "java",
        "xml" => "xml",
        "sql" => "sql",
        _ => "plaintext",
    }
}

fn get_absolute_path(path: &str) -> String {
    let path_buf = std::path::PathBuf::from(path);
    let abs_path = if path_buf.is_absolute() {
        path_buf
    } else {
        std::env::current_dir().unwrap_or_default().join(path_buf)
    };
    std::fs::canonicalize(&abs_path)
        .unwrap_or(abs_path)
        .to_string_lossy()
        .to_string()
}
