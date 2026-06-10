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
}

pub struct LspClient {
    cmd_tx: Sender<LspCommand>,
}

impl LspClient {
    pub fn new(diagnostics_tx: Sender<LspDiagnosticsUpdate>) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<LspCommand>();
        
        std::thread::spawn(move || {
            // Attempt to spawn rust-analyzer
            let mut child = match Command::new("rust-analyzer")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn() 
            {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("LSP: Failed to spawn rust-analyzer: {:?}", e);
                    return;
                }
            };

            let mut stdin = child.stdin.take().expect("Failed to open stdin");
            let stdout = child.stdout.take().expect("Failed to open stdout");
            let mut stdout_reader = BufReader::new(stdout);

            // Spawn Reader thread
            let diag_tx = diagnostics_tx.clone();
            std::thread::spawn(move || {
                loop {
                    match read_message(&mut stdout_reader) {
                        Ok(msg_str) => {
                            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&msg_str) {
                                if msg["method"] == "textDocument/publishDiagnostics" {
                                    if let Some(params) = msg["params"].as_object() {
                                        if let (Some(uri), Some(diagnostics)) = (params.get("uri"), params.get("diagnostics")) {
                                            if let Some(uri_str) = uri.as_str() {
                                                let file_path = if uri_str.starts_with("file://") {
                                                    uri_str["file://".len()..].to_string()
                                                } else {
                                                    uri_str.to_string()
                                                };
                                                
                                                let mut errors = 0;
                                                let mut warnings = 0;
                                                if let Some(diag_array) = diagnostics.as_array() {
                                                    for diag in diag_array {
                                                        let severity = diag["severity"].as_i64().unwrap_or(0);
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
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("LSP: Reader thread exit due to error: {:?}", e);
                            break;
                        }
                    }
                }
            });

            // Initialize rust-analyzer
            let current_dir = std::env::current_dir().unwrap_or_default();
            let root_uri = format!("file://{}", current_dir.to_string_lossy());
            
            let init_msg = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": std::process::id(),
                    "rootUri": root_uri,
                    "capabilities": {},
                    "workspaceFolders": null
                }
            });

            if let Err(e) = write_message(&mut stdin, &init_msg.to_string()) {
                log::warn!("LSP: Failed to write initialize message: {:?}", e);
                return;
            }

            // Send initialized notification
            let initialized_msg = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            });
            if let Err(e) = write_message(&mut stdin, &initialized_msg.to_string()) {
                log::warn!("LSP: Failed to write initialized notification: {:?}", e);
                return;
            }

            let mut document_versions = HashMap::<String, usize>::new();
            let pending_changes = Arc::new(Mutex::new(HashMap::<String, (String, usize, bool)>::new()));
            
            let pending_changes_clone = pending_changes.clone();
            loop {
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
                    let _ = write_message(&mut stdin, &change_msg.to_string());
                }

                match cmd_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(LspCommand::OpenFile { path, text }) => {
                        let version = document_versions.entry(path.clone()).or_insert(0);
                        *version += 1;
                        let open_msg = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didOpen",
                            "params": {
                                "textDocument": {
                                    "uri": format!("file://{}", path),
                                    "languageId": "rust",
                                    "version": *version,
                                    "text": text
                                }
                            }
                        });
                        let _ = write_message(&mut stdin, &open_msg.to_string());
                    }
                    Ok(LspCommand::ChangeFile { path, text }) => {
                        let version = document_versions.entry(path.clone()).or_insert(0);
                        *version += 1;
                        if let Ok(mut pending) = pending_changes_clone.lock() {
                            pending.insert(path, (text, *version, true));
                        }
                    }
                    Ok(LspCommand::SaveFile { path }) => {
                        let save_msg = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didSave",
                            "params": {
                                "textDocument": {
                                    "uri": format!("file://{}", path)
                                }
                            }
                        });
                        let _ = write_message(&mut stdin, &save_msg.to_string());
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
        let _ = self.cmd_tx.send(LspCommand::OpenFile { path: path.to_string(), text });
    }

    pub fn notify_change(&self, path: &str, text: String) {
        let _ = self.cmd_tx.send(LspCommand::ChangeFile { path: path.to_string(), text });
    }

    pub fn notify_save(&self, path: &str) {
        let _ = self.cmd_tx.send(LspCommand::SaveFile { path: path.to_string() });
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
