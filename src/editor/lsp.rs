use std::process::{Command, Stdio};
use std::io::{Write, Read, BufReader};
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DiagnosticDetail {
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub severity: u32,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SemanticTokenDetail {
    pub line: usize,
    pub start_col: usize,
    pub length: usize,
    pub token_type: String,
}

#[derive(Debug, Clone)]
pub struct LspDiagnosticsUpdate {
    pub file_path: String,
    pub errors: usize,
    pub warnings: usize,
    pub diagnostics: Vec<DiagnosticDetail>,
    pub tokens: Vec<SemanticTokenDetail>,
    pub is_tokens_update: bool,
}

pub enum LspCommand {
    OpenFile { path: String, text: String },
    ChangeFile { path: String, text: String },
    SaveFile { path: String },
    SetActiveFile { path: String },
    RetrySpawn { lang_id: String },
    RequestActiveTokens { lang_id: String },
    RequestTokensForFile { path: String },
    RunFlycheck { lang_id: String },
    RegisterServer { lang_id: String, server: ServerInstance },
    SpawnFailed { lang_id: String },
}

#[derive(Clone)]
pub struct LspClient {
    cmd_tx: Sender<LspCommand>,
}

struct ServerInstance {
    stdin: Arc<Mutex<std::process::ChildStdin>>,
    cmd_name: String,
    token_requests: Arc<Mutex<HashMap<u64, String>>>,
    next_req_id: Arc<Mutex<u64>>,
    _token_types: Arc<Mutex<Vec<String>>>,
}

fn request_semantic_tokens(server: &ServerInstance, path: &str) {
    let req_id = {
        let mut id_lock = server.next_req_id.lock().unwrap();
        *id_lock += 1;
        *id_lock
    };
    {
        let mut req_map = server.token_requests.lock().unwrap();
        req_map.insert(req_id, path.to_string());
    }
    let tokens_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "method": "textDocument/semanticTokens/full",
        "params": {
            "textDocument": {
                "uri": format!("file://{}", path)
            }
        }
    });
    if let Ok(mut writer) = server.stdin.lock() {
        let _ = write_message(&mut *writer, &tokens_msg.to_string());
    }
}

fn trigger_rust_analyzer_flycheck(server: &ServerInstance) {
    if server.cmd_name == "rust-analyzer" {
        let cmd_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "rust-analyzer/runFlycheck",
            "params": {
                "textDocument": null
            }
        });
        if let Ok(mut writer) = server.stdin.lock() {
            let _ = write_message(&mut *writer, &cmd_msg.to_string());
        }
    }
}

fn open_all_documents_for_server(
    server: &ServerInstance,
    lang_id: &str,
    open_documents: &HashMap<String, String>,
    document_versions: &mut HashMap<String, usize>,
) {
    let mut opened_any = false;
    for (path, text) in open_documents {
        if detect_language_id(path) == lang_id {
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
            if let Ok(mut writer) = server.stdin.lock() {
                let _ = write_message(&mut *writer, &open_msg.to_string());
            }
            request_semantic_tokens(server, path);
            opened_any = true;
        }
    }
    if opened_any {
        trigger_rust_analyzer_flycheck(server);
    }
}
fn get_local_lsp_path(subpath: &str) -> String {
    let current_dir = std::env::current_dir().unwrap_or_default();
    current_dir.join(".lsp").join(subpath).to_string_lossy().to_string()
}

fn get_node_bin_paths(subpath: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let current_dir = std::env::current_dir().unwrap_or_default();
    
    // 1. Auto-installed .lsp folder
    paths.push(current_dir.join(".lsp").join("node_modules/.bin").join(subpath).to_string_lossy().to_string());
    
    // 2. Current workspace root node_modules
    paths.push(current_dir.join("node_modules/.bin").join(subpath).to_string_lossy().to_string());
    
    // 3. Parent workspace root node_modules (e.g. parent of sub-project)
    if let Some(parent) = current_dir.parent() {
        paths.push(parent.join("node_modules/.bin").join(subpath).to_string_lossy().to_string());
    }
    
    paths
}

fn validate_binary(path: &str) -> bool {
    if path.contains('/') && !std::path::Path::new(path).exists() {
        return false;
    }
    match Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            let _ = child.wait();
            true
        }
        Err(_) => false,
    }
}

fn get_rustup_rust_analyzer() -> Option<String> {
    for file_name in &["rust-toolchain.toml", "rust-toolchain"] {
        if std::path::Path::new(file_name).exists() {
            if let Ok(output) = Command::new("rustup")
                .args(&["which", "rust-analyzer"])
                .output()
            {
                if output.status.success() {
                    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path_str.is_empty() {
                        return Some(path_str);
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
enum LibcType {
    Gnu,
    Musl,
}

#[cfg(target_os = "linux")]
fn determine_libc_type() -> LibcType {
    if let Ok(output) = Command::new("ldd").arg("--version").output() {
        let ldd_version = String::from_utf8_lossy(&output.stdout);
        if ldd_version.contains("GNU libc") || ldd_version.contains("GLIBC") {
            return LibcType::Gnu;
        } else if ldd_version.contains("musl") {
            return LibcType::Musl;
        }
    }

    if let Ok(entries) = std::fs::read_dir("/lib") {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.starts_with("ld-musl-") {
                return LibcType::Musl;
            } else if name_str.starts_with("ld-linux-") {
                return LibcType::Gnu;
            }
        }
    }

    LibcType::Gnu
}

fn spawn_server(lang_id: &str) -> Result<(std::process::Child, String), String> {
    let alternatives = match lang_id {
        "rust" => {
            let mut list = Vec::new();
            if let Some(path) = get_rustup_rust_analyzer() {
                if validate_binary(&path) {
                    list.push((path, vec![]));
                }
            }
            if validate_binary("rust-analyzer") {
                list.push(("rust-analyzer".to_string(), vec![]));
            }
            list.push((get_local_lsp_path("rust-analyzer"), vec![]));
            list
        }
        "python" => vec![
            ("pyright-langserver".to_string(), vec!["--stdio".to_string()]),
            (get_local_lsp_path("node_modules/.bin/pyright-langserver"), vec!["--stdio".to_string()]),
            ("pylsp".to_string(), vec![]),
            (get_local_lsp_path("venv/bin/pylsp"), vec![]),
        ],
        "go" => vec![
            ("gopls".to_string(), vec![]),
            (get_local_lsp_path("go/gopls"), vec![]),
        ],
        "c" | "cpp" => {
            let mut list = Vec::new();
            if validate_binary("clangd") {
                list.push(("clangd".to_string(), vec![]));
            }
            if let Ok(entries) = std::fs::read_dir(get_local_lsp_path("")) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if entry.file_name().to_string_lossy().starts_with("clangd") {
                        let bin = path.join("bin").join("clangd");
                        if bin.exists() {
                            list.push((bin.to_string_lossy().to_string(), vec![]));
                        }
                    }
                }
            }
            list
        }
        "typescript" | "javascript" => {
            let mut list = Vec::new();
            let current_dir = std::env::current_dir().unwrap_or_default();
            let is_deno_project = current_dir.join("deno.json").exists() || current_dir.join("deno.jsonc").exists();

            if is_deno_project && validate_binary("deno") {
                list.push(("deno".to_string(), vec!["lsp".to_string()]));
            }

            if validate_binary("typescript-language-server") {
                list.push(("typescript-language-server".to_string(), vec!["--stdio".to_string()]));
            }
            for path in get_node_bin_paths("typescript-language-server") {
                list.push((path, vec!["--stdio".to_string()]));
            }
            list
        }
        "json" => {
            let mut list = vec![("vscode-json-language-server".to_string(), vec!["--stdio".to_string()])];
            for path in get_node_bin_paths("vscode-json-language-server") {
                list.push((path, vec!["--stdio".to_string()]));
            }
            list
        }
        "yaml" => {
            let mut list = vec![("yaml-language-server".to_string(), vec!["--stdio".to_string()])];
            for path in get_node_bin_paths("yaml-language-server") {
                list.push((path, vec!["--stdio".to_string()]));
            }
            list
        }
        "html" => {
            let mut list = vec![("vscode-html-language-server".to_string(), vec!["--stdio".to_string()])];
            for path in get_node_bin_paths("vscode-html-language-server") {
                list.push((path, vec!["--stdio".to_string()]));
            }
            list
        }
        "css" | "scss" => {
            let mut list = vec![("vscode-css-language-server".to_string(), vec!["--stdio".to_string()])];
            for path in get_node_bin_paths("vscode-css-language-server") {
                list.push((path, vec!["--stdio".to_string()]));
            }
            list
        }
        "toml" => {
            let mut list = vec![
                ("taplo".to_string(), vec!["lsp".to_string(), "stdio".to_string()]),
                (get_local_lsp_path("taplo"), vec!["lsp".to_string(), "stdio".to_string()]),
            ];
            for path in get_node_bin_paths("taplo") {
                list.push((path, vec!["lsp".to_string(), "stdio".to_string()]));
            }
            list
        }
        "markdown" => vec![
            ("marksman".to_string(), vec!["server".to_string()]),
            (get_local_lsp_path("marksman"), vec!["server".to_string()]),
        ],
        _ => return Err("No configured LSP server for this language".to_string()),
    };

    let mut last_err = String::new();
    let lsp_dir = std::env::current_dir().unwrap_or_default().join(".lsp");
    let node_bin_dir = lsp_dir.join("node").join("bin");
    let new_path = if node_bin_dir.exists() {
        if let Ok(current_path) = std::env::var("PATH") {
            format!("{}:{}", node_bin_dir.to_string_lossy(), current_path)
        } else {
            node_bin_dir.to_string_lossy().to_string()
        }
    } else {
        std::env::var("PATH").unwrap_or_default()
    };

    for (cmd, args) in alternatives {
        if cmd.contains('/') && !std::path::Path::new(&cmd).exists() {
            continue;
        }
        let lsp_dir = std::env::current_dir().unwrap_or_default().join(".lsp");
        let _ = std::fs::create_dir_all(&lsp_dir);
        let stderr_file = if let Ok(f) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(lsp_dir.join(format!("{}.stderr.log", lang_id)))
        {
            Stdio::from(f)
        } else {
            Stdio::null()
        };

        log::info!("LSP: Trying to spawn {} {:?}", cmd, args);
        let mut command = Command::new(&cmd);
        command.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr_file);
        if !new_path.is_empty() {
            command.env("PATH", &new_path);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                command.pre_exec(|| {
                    unsafe extern "C" {
                        fn nice(inc: std::os::raw::c_int) -> std::os::raw::c_int;
                    }
                    nice(19);
                    Ok(())
                });
            }
        }
        match command.spawn() {
            Ok(child) => {
                let cmd_name = std::path::Path::new(&cmd)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                return Ok((child, cmd_name));
            }
            Err(e) => {
                last_err = format!("{} not found ({:?})", cmd, e.kind());
            }
        }
    }
    Err(last_err)
}

fn spawn_server_async(
    lang_id: String,
    cmd_tx: Sender<LspCommand>,
    diagnostics_tx: Sender<LspDiagnosticsUpdate>,
    proxy_init: winit::event_loop::EventLoopProxy<()>,
) {
    let cmd_tx_clone = cmd_tx.clone();
    std::thread::spawn(move || {
        log::info!("LSP: Spawning server asynchronously for {}", lang_id);
        match spawn_server(&lang_id) {
            Ok((mut child, cmd_name)) => {
                let mut stdin_raw = child.stdin.take().expect("Failed to open stdin");
                let stdout = child.stdout.take().expect("Failed to open stdout");
                let mut stdout_reader = BufReader::new(stdout);
                
                let token_requests = Arc::new(Mutex::new(HashMap::<u64, String>::new()));
                let next_req_id = Arc::new(Mutex::new(1000u64));
                
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
                                },
                                "semanticTokens": {
                                    "requests": {
                                        "full": true
                                    },
                                    "tokenTypes": [
                                        "namespace", "type", "class", "enum", "interface", "struct", "typeParameter",
                                        "parameter", "variable", "property", "enumMember", "event", "function",
                                        "method", "macro", "keyword", "modifier", "comment", "string", "number",
                                        "regexp", "operator"
                                    ],
                                    "tokenModifiers": [
                                        "declaration", "definition", "readonly", "static", "deprecated",
                                        "abstract", "async", "modification", "documentation", "defaultLibrary"
                                    ],
                                    "formats": ["relative"]
                                }
                            },
                            "window": {
                                "workDoneProgress": true
                            }
                        },
                        "initializationOptions": {
                            "check": {
                                "command": "check",
                                "extraArgs": ["--target-dir", "target/rust-analyzer"]
                            },
                            "checkOnSave": {
                                "enable": true,
                                "command": "check",
                                "extraArgs": ["--target-dir", "target/rust-analyzer"]
                            },
                            "cargo": {
                                "targetDir": "target/rust-analyzer"
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
                if write_message(&mut stdin_raw, &init_msg.to_string()).is_ok() {
                    if let Ok(resp_str) = read_message(&mut stdout_reader) {
                        log::info!("LSP: Initialize response from {} received", cmd_name);
                        let server_token_types = parse_legend_token_types(&resp_str);
                        let token_types = Arc::new(Mutex::new(server_token_types));

                        let initialized_msg = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "initialized",
                            "params": {}
                        });
                        let _ = write_message(&mut stdin_raw, &initialized_msg.to_string());
                        
                        let stdin = Arc::new(Mutex::new(stdin_raw));
                        start_reader_thread(stdout_reader, lang_id.clone(), cmd_name.clone(), diagnostics_tx.clone(), proxy_init.clone(), token_requests.clone(), stdin.clone(), token_types.clone(), cmd_tx_clone.clone());

                        let server_instance = ServerInstance { stdin, cmd_name, token_requests, next_req_id, _token_types: token_types };
                        
                        let _ = cmd_tx_clone.send(LspCommand::RegisterServer { lang_id, server: server_instance });
                    } else {
                        let _ = cmd_tx_clone.send(LspCommand::SpawnFailed { lang_id });
                    }
                } else {
                    let _ = cmd_tx_clone.send(LspCommand::SpawnFailed { lang_id });
                }
            }
            Err(e_msg) => {
                log::warn!("LSP: Async spawn failed for {}: {}", lang_id, e_msg);
                let _ = cmd_tx_clone.send(LspCommand::SpawnFailed { lang_id });
            }
        }
    });
}

fn is_path_ignored(path_str: &str, gitignore: &ignore::gitignore::Gitignore) -> bool {
    let path = std::path::Path::new(path_str);
    
    // Check if it is inside the workspace (current directory)
    let current_dir = std::env::current_dir().unwrap_or_default();
    if !path.starts_with(&current_dir) {
        return true;
    }
    
    // Check if hidden (starts with a dot component)
    let has_hidden = path.components().any(|comp| {
        if let std::path::Component::Normal(name) = comp {
            name.to_string_lossy().starts_with('.')
        } else {
            false
        }
    });
    if has_hidden {
        return true;
    }

    // Check gitignore
    let relative_path = path.strip_prefix(&current_dir).unwrap_or(path);
    gitignore.matched_path_or_any_parents(relative_path, false).is_ignore()
}


fn get_latest_github_tag(repo: &str) -> Option<String> {
    let output = Command::new("curl")
        .args(&["-sI", &format!("https://github.com/{}/releases/latest", repo)])
        .output()
        .ok()?;
    if output.status.success() {
        let headers = String::from_utf8_lossy(&output.stdout);
        for line in headers.lines() {
            if line.to_lowercase().starts_with("location:") {
                if let Some(tag) = line.split("/tag/").nth(1) {
                    return Some(tag.trim().to_string());
                }
            }
        }
    }
    None
}

fn ensure_node_runtime() -> Result<(String, String), String> {
    if validate_binary("node") && validate_binary("npm") {
        return Ok(("node".to_string(), "npm".to_string()));
    }

    let lsp_dir = std::env::current_dir().unwrap_or_default().join(".lsp");
    let node_dir = lsp_dir.join("node");
    let local_node = node_dir.join("bin").join("node");
    let local_npm = node_dir.join("bin").join("npm");

    if local_node.exists() && local_npm.exists() {
        return Ok((local_node.to_string_lossy().to_string(), local_npm.to_string_lossy().to_string()));
    }

    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    let os = if cfg!(target_os = "macos") { "darwin" } else { "linux" };
    let version = "v20.11.0";
    let ext = if cfg!(target_os = "macos") { "tar.gz" } else { "tar.xz" };
    let folder_name = format!("node-{}-{}-{}", version, os, arch);
    let url = format!("https://nodejs.org/dist/{}/{}.{}", version, folder_name, ext);
    let dest_archive = lsp_dir.join(format!("node.{}", ext));

    log::info!("LSP Auto-Install: Downloading Node.js from {}", url);
    let curl_status = std::process::Command::new("curl")
        .args(&["-L", "-o", dest_archive.to_str().unwrap(), &url])
        .status();

    if curl_status.map(|s| s.success()).unwrap_or(false) {
        let tar_status = std::process::Command::new("tar")
            .args(&["-xf", dest_archive.to_str().unwrap(), "-C", lsp_dir.to_str().unwrap()])
            .status();

        if tar_status.map(|s| s.success()).unwrap_or(false) {
            let _ = std::fs::remove_file(&dest_archive);
            let extracted_dir = lsp_dir.join(folder_name);
            if extracted_dir.exists() {
                let _ = std::fs::remove_dir_all(&node_dir);
                if std::fs::rename(&extracted_dir, &node_dir).is_ok() {
                    if local_node.exists() && local_npm.exists() {
                        return Ok((local_node.to_string_lossy().to_string(), local_npm.to_string_lossy().to_string()));
                    }
                }
            }
            Err("Failed to setup Node.js directory".to_string())
        } else {
            Err("Failed to extract Node.js archive".to_string())
        }
    } else {
        Err("Failed to download Node.js".to_string())
    }
}

fn attempt_auto_install(
    lang_id: &str,
    cmd_tx: Sender<LspCommand>,
    diagnostics_tx: Sender<LspDiagnosticsUpdate>,
    event_loop_proxy: winit::event_loop::EventLoopProxy<()>,
) {
    let tx = cmd_tx;
    let diag_tx = diagnostics_tx;
    let proxy = event_loop_proxy;
    let lang_id = lang_id.to_string();
    std::thread::spawn(move || {
        let lsp_dir = std::env::current_dir().unwrap_or_default().join(".lsp");
        let _ = std::fs::create_dir_all(&lsp_dir);
        let package_json_path = lsp_dir.join("package.json");
        if !package_json_path.exists() {
            let _ = std::fs::write(&package_json_path, "{}");
        }

        let install_result = match lang_id.as_str() {
            "rust" => {
                let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
                let os = if cfg!(target_os = "macos") {
                    "apple-darwin".to_string()
                } else if cfg!(target_os = "windows") {
                    "pc-windows-msvc".to_string()
                } else {
                    #[cfg(target_os = "linux")]
                    {
                        match determine_libc_type() {
                            LibcType::Musl => "unknown-linux-musl".to_string(),
                            LibcType::Gnu => "unknown-linux-gnu".to_string(),
                        }
                    }
                    #[cfg(not(target_os = "linux"))]
                    "unknown-linux-gnu".to_string()
                };
                let url = format!("https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-{}-{}.gz", arch, os);
                let dest_gz = lsp_dir.join("rust-analyzer.gz");
                let dest_bin = lsp_dir.join("rust-analyzer");

                log::info!("LSP Auto-Install: Downloading rust-analyzer from {}", url);
                let curl_status = std::process::Command::new("curl")
                    .args(&["-L", "-o", dest_gz.to_str().unwrap(), &url])
                    .status();
                
                if curl_status.map(|s| s.success()).unwrap_or(false) {
                    let gunzip_status = std::process::Command::new("gunzip")
                        .args(&["-f", dest_gz.to_str().unwrap()])
                        .status();
                    
                    if gunzip_status.map(|s| s.success()).unwrap_or(false) {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(metadata) = std::fs::metadata(&dest_bin) {
                                let mut perms = metadata.permissions();
                                perms.set_mode(0o755);
                                let _ = std::fs::set_permissions(&dest_bin, perms);
                            }
                        }
                        Ok(())
                    } else {
                        Err("Failed to decompress rust-analyzer".to_string())
                    }
                } else {
                    Err("Failed to download rust-analyzer".to_string())
                }
            }
            "python" => {
                log::info!("LSP Auto-Install: Installing pyright via npm");
                let (node_bin, npm_bin) = ensure_node_runtime().unwrap_or(("node".to_string(), "npm".to_string()));
                let node_dir = std::path::Path::new(&node_bin).parent().map(|p| p.parent().unwrap_or(p)).map(|p| p.to_path_buf()).unwrap_or_default();
                let npm_status = std::process::Command::new(&npm_bin)
                    .env("PATH", format!("{}:{}", node_dir.join("bin").to_str().unwrap(), std::env::var("PATH").unwrap_or_default()))
                    .args(&["install", "--prefix", lsp_dir.to_str().unwrap(), "pyright"])
                    .status();
                
                if npm_status.map(|s| s.success()).unwrap_or(false) {
                    Ok(())
                } else {
                    log::warn!("LSP Auto-Install: npm pyright install failed. Retrying with python-lsp-server via venv");
                    let venv_status = std::process::Command::new("python3")
                        .args(&["-m", "venv", lsp_dir.join("venv").to_str().unwrap()])
                        .status();
                    
                    if venv_status.map(|s| s.success()).unwrap_or(false) {
                        let pip_bin = lsp_dir.join("venv").join("bin").join("pip");
                        let pip_status = std::process::Command::new(pip_bin)
                            .args(&["install", "python-lsp-server"])
                            .status();
                        if pip_status.map(|s| s.success()).unwrap_or(false) {
                            Ok(())
                        } else {
                            Err("Failed to install python-lsp-server via pip".to_string())
                        }
                    } else {
                        Err("Failed to create venv or install pyright".to_string())
                    }
                }
            }
            "go" => {
                log::info!("LSP Auto-Install: Installing gopls via go install");
                let bin_dir = lsp_dir.join("go");
                let go_status = std::process::Command::new("go")
                    .env("GOBIN", bin_dir.to_str().unwrap())
                    .args(&["install", "golang.org/x/tools/gopls@latest"])
                    .status();
                if go_status.map(|s| s.success()).unwrap_or(false) {
                    Ok(())
                } else {
                    Err("Failed to install gopls via go install".to_string())
                }
            }
            "c" | "cpp" => {
                if let Some(tag) = get_latest_github_tag("clangd/clangd") {
                    let os_suffix = if cfg!(target_os = "macos") {
                        "mac"
                    } else if cfg!(target_os = "windows") {
                        "windows"
                    } else {
                        "linux"
                    };
                    let asset_name = format!("clangd-{}-{}.zip", os_suffix, tag);
                    let url = format!("https://github.com/clangd/clangd/releases/download/{}/{}", tag, asset_name);
                    let dest_zip = lsp_dir.join("clangd.zip");

                    log::info!("LSP Auto-Install: Downloading clangd from {}", url);
                    let curl_status = std::process::Command::new("curl")
                        .args(&["-L", "-o", dest_zip.to_str().unwrap(), &url])
                        .status();

                    if curl_status.map(|s| s.success()).unwrap_or(false) {
                        let unzip_status = std::process::Command::new("unzip")
                            .args(&["-o", dest_zip.to_str().unwrap(), "-d", lsp_dir.to_str().unwrap()])
                            .status();

                        if unzip_status.map(|s| s.success()).unwrap_or(false) {
                            let _ = std::fs::remove_file(&dest_zip);
                            let clangd_folder = lsp_dir.join(format!("clangd_{}", tag));
                            let dest_bin = clangd_folder.join("bin").join("clangd");
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Ok(metadata) = std::fs::metadata(&dest_bin) {
                                    let mut perms = metadata.permissions();
                                    perms.set_mode(0o755);
                                    let _ = std::fs::set_permissions(&dest_bin, perms);
                                }
                            }
                            Ok(())
                        } else {
                            Err("Failed to unzip clangd".to_string())
                        }
                    } else {
                        Err("Failed to download clangd".to_string())
                    }
                } else {
                    Err("Failed to resolve latest clangd version tag".to_string())
                }
            }
            "typescript" | "javascript" => {
                log::info!("LSP Auto-Install: Installing typescript-language-server and typescript via npm");
                let (node_bin, npm_bin) = ensure_node_runtime().unwrap_or(("node".to_string(), "npm".to_string()));
                let node_dir = std::path::Path::new(&node_bin).parent().map(|p| p.parent().unwrap_or(p)).map(|p| p.to_path_buf()).unwrap_or_default();
                let npm_status = std::process::Command::new(&npm_bin)
                    .env("PATH", format!("{}:{}", node_dir.join("bin").to_str().unwrap(), std::env::var("PATH").unwrap_or_default()))
                    .args(&["install", "--prefix", lsp_dir.to_str().unwrap(), "typescript-language-server", "typescript"])
                    .status();
                if npm_status.map(|s| s.success()).unwrap_or(false) {
                    Ok(())
                } else {
                    Err("Failed to install typescript-language-server via npm".to_string())
                }
            }
            "json" | "html" | "css" | "scss" => {
                log::info!("LSP Auto-Install: Installing vscode-langservers-extracted via npm");
                let (node_bin, npm_bin) = ensure_node_runtime().unwrap_or(("node".to_string(), "npm".to_string()));
                let node_dir = std::path::Path::new(&node_bin).parent().map(|p| p.parent().unwrap_or(p)).map(|p| p.to_path_buf()).unwrap_or_default();
                let npm_status = std::process::Command::new(&npm_bin)
                    .env("PATH", format!("{}:{}", node_dir.join("bin").to_str().unwrap(), std::env::var("PATH").unwrap_or_default()))
                    .args(&["install", "--prefix", lsp_dir.to_str().unwrap(), "vscode-langservers-extracted"])
                    .status();
                if npm_status.map(|s| s.success()).unwrap_or(false) {
                    Ok(())
                } else {
                    Err("Failed to install vscode-langservers-extracted".to_string())
                }
            }
            "yaml" => {
                log::info!("LSP Auto-Install: Installing yaml-language-server via npm");
                let (node_bin, npm_bin) = ensure_node_runtime().unwrap_or(("node".to_string(), "npm".to_string()));
                let node_dir = std::path::Path::new(&node_bin).parent().map(|p| p.parent().unwrap_or(p)).map(|p| p.to_path_buf()).unwrap_or_default();
                let npm_status = std::process::Command::new(&npm_bin)
                    .env("PATH", format!("{}:{}", node_dir.join("bin").to_str().unwrap(), std::env::var("PATH").unwrap_or_default()))
                    .args(&["install", "--prefix", lsp_dir.to_str().unwrap(), "yaml-language-server"])
                    .status();
                if npm_status.map(|s| s.success()).unwrap_or(false) {
                    Ok(())
                } else {
                    Err("Failed to install yaml-language-server".to_string())
                }
            }
            "toml" => {
                let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
                let os = if cfg!(target_os = "macos") {
                    "darwin"
                } else if cfg!(target_os = "windows") {
                    "windows"
                } else {
                    "linux"
                };
                let filename = format!("taplo-{}-{}.gz", os, arch);
                let url = format!("https://github.com/tamasfe/taplo/releases/latest/download/{}", filename);
                let dest_gz = lsp_dir.join(if cfg!(target_os = "windows") { "taplo.exe.gz" } else { "taplo.gz" });
                let dest_bin = lsp_dir.join(if cfg!(target_os = "windows") { "taplo.exe" } else { "taplo" });

                log::info!("LSP Auto-Install: Downloading taplo from {}", url);
                let curl_status = std::process::Command::new("curl")
                    .args(&["-L", "-o", dest_gz.to_str().unwrap(), &url])
                    .status();

                if curl_status.map(|s| s.success()).unwrap_or(false) {
                    let gunzip_status = std::process::Command::new("gunzip")
                        .args(&["-f", dest_gz.to_str().unwrap()])
                        .status();

                    if gunzip_status.map(|s| s.success()).unwrap_or(false) {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(metadata) = std::fs::metadata(&dest_bin) {
                                let mut perms = metadata.permissions();
                                perms.set_mode(0o755);
                                let _ = std::fs::set_permissions(&dest_bin, perms);
                            }
                        }
                        Ok(())
                    } else {
                        Err("Failed to decompress taplo".to_string())
                    }
                } else {
                    Err("Failed to download taplo".to_string())
                }
            }
            "markdown" => {
                let os = if cfg!(target_os = "macos") {
                    "macos"
                } else if cfg!(target_os = "windows") {
                    "win.exe"
                } else {
                    if cfg!(target_arch = "aarch64") {
                        "linux-arm64"
                    } else {
                        "linux-x64"
                    }
                };
                let filename = if os == "win.exe" { "marksman.exe".to_string() } else if os == "macos" { "marksman-macos".to_string() } else { format!("marksman-{}", os) };
                let url = format!("https://github.com/artempyanykh/marksman/releases/latest/download/{}", filename);
                let dest_bin = lsp_dir.join(if cfg!(target_os = "windows") { "marksman.exe" } else { "marksman" });

                log::info!("LSP Auto-Install: Downloading marksman from {}", url);
                let curl_status = std::process::Command::new("curl")
                    .args(&["-L", "-o", dest_bin.to_str().unwrap(), &url])
                    .status();
                
                if curl_status.map(|s| s.success()).unwrap_or(false) {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(metadata) = std::fs::metadata(&dest_bin) {
                            let mut perms = metadata.permissions();
                            perms.set_mode(0o755);
                            let _ = std::fs::set_permissions(&dest_bin, perms);
                        }
                    }
                    Ok(())
                } else {
                    Err("Failed to download marksman".to_string())
                }
            }
            _ => Err(format!("Unsupported language {}", lang_id)),
        };

        match install_result {
            Ok(_) => {
                log::info!("LSP Auto-Install: Success for {}", lang_id);
                let _ = tx.send(LspCommand::RetrySpawn { lang_id: lang_id.to_string() });
            }
            Err(e) => {
                log::warn!("LSP Auto-Install: Failed for {}: {}", lang_id, e);
                let display_name = match lang_id.as_str() {
                    "rust" => "rust-analyzer",
                    "python" => "pyright/pylsp",
                    "go" => "gopls",
                    "c" | "cpp" => "clangd",
                    "typescript" | "javascript" => "typescript-language-server",
                    "json" => "json-language-server",
                    "yaml" => "yaml-language-server",
                    "toml" => "taplo",
                    "html" => "html-language-server",
                    "css" | "scss" => "css-language-server",
                    "markdown" => "marksman",
                    _ => &lang_id,
                };
                let _ = diag_tx.send(LspDiagnosticsUpdate {
                    file_path: format!("status:offline (install failed for {})", display_name),
                    errors: 9999,
                    warnings: 0,
                    diagnostics: vec![],
                    tokens: vec![],
                    is_tokens_update: false,
                });
                let _ = proxy.send_event(());
            }
        }
    });
}

impl LspClient {
    pub fn new(
        _diagnostics_tx: Sender<LspDiagnosticsUpdate>,
        _event_loop_proxy: winit::event_loop::EventLoopProxy<()>,
    ) -> Self {
        let (cmd_tx, _) = std::sync::mpsc::channel::<LspCommand>();
        Self { cmd_tx }
    }

    pub fn notify_open(&self, _path: &str, _text: String) {}

    pub fn notify_change(&self, _path: &str, _text: String) {}

    pub fn notify_save(&self, _path: &str) {}

    pub fn notify_active_file(&self, _path: &str) {}

    pub fn trigger_flycheck(&self, _lang_id: &str) {}
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
    log::debug!("LSP: Sending message: {}", msg);
    let content = format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg);
    writer.write_all(content.as_bytes())?;
    writer.flush()?;
    Ok(())
}

fn start_reader_thread(
    stdout_reader: BufReader<std::process::ChildStdout>,
    lang_id: String,
    cmd_name: String,
    diag_tx: Sender<LspDiagnosticsUpdate>,
    proxy: winit::event_loop::EventLoopProxy<()>,
    token_requests: Arc<Mutex<HashMap<u64, String>>>,
    stdin: Arc<Mutex<std::process::ChildStdin>>,
    token_types: Arc<Mutex<Vec<String>>>,
    cmd_tx: Sender<LspCommand>,
) {
    let cmd_name_for_reader = cmd_name;
    std::thread::spawn(move || {
        let mut stdout_reader = stdout_reader;
        let mut active_progress = HashMap::<String, String>::new();
        let mut builder = ignore::gitignore::GitignoreBuilder::new(".");
        builder.add(".gitignore");
        let gitignore = builder.build().unwrap_or_else(|_| ignore::gitignore::Gitignore::empty());
        loop {
            match read_message(&mut stdout_reader) {
                Ok(resp_str) => {
                    if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&resp_str) {
                        log::debug!("LSP: Received raw message: {}", resp_str);
                        log::debug!("LSP: Received message method/id: {:?}", resp.get("method").or_else(|| resp.get("id")));
                        
                        // Check if this is a request/notification from the server to the client
                        if let Some(method_val) = resp.get("method") {
                            let method = method_val.as_str().unwrap_or("");
                            if method == "workspace/semanticTokens/refresh" || method == "workspace/diagnostic/refresh" {
                                if let Some(id_val) = resp.get("id") {
                                    let response = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id_val,
                                        "result": serde_json::Value::Null
                                    });
                                    if let Ok(mut writer) = stdin.lock() {
                                        let _ = write_message(&mut *writer, &response.to_string());
                                    }
                                }
                                let _ = cmd_tx.send(LspCommand::RequestActiveTokens { lang_id: lang_id.clone() });
                                if method == "workspace/diagnostic/refresh" {
                                    let _ = cmd_tx.send(LspCommand::RunFlycheck { lang_id: lang_id.clone() });
                                }
                            }
                        }

                        if let Some(id_val) = resp.get("id") {
                            if let Some(method_val) = resp.get("method") {
                                let method = method_val.as_str().unwrap_or("");
                                if method == "window/workDoneProgress/create" {
                                    let response = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id_val,
                                        "result": serde_json::Value::Null
                                    });
                                    if let Ok(mut writer) = stdin.lock() {
                                        let _ = write_message(&mut *writer, &response.to_string());
                                    }
                                } else if method == "workspace/configuration" {
                                    let items = resp.get("params")
                                        .and_then(|p| p.get("items"))
                                        .and_then(|i| i.as_array());
                                    
                                    let mut result_array = Vec::new();
                                    if let Some(items_list) = items {
                                        for item in items_list {
                                            let section = item.get("section").and_then(|s| s.as_str()).unwrap_or("");
                                            if section == "rust-analyzer" {
                                                result_array.push(serde_json::json!({
                                                    "check": {
                                                        "command": "check",
                                                        "extraArgs": ["--target-dir", "target/rust-analyzer"]
                                                    },
                                                    "checkOnSave": {
                                                        "enable": true,
                                                        "command": "check",
                                                        "extraArgs": ["--target-dir", "target/rust-analyzer"]
                                                    },
                                                    "cargo": {
                                                        "targetDir": "target/rust-analyzer"
                                                    }
                                                }));
                                            } else if section == "rust-analyzer.check" {
                                                result_array.push(serde_json::json!({
                                                    "command": "check",
                                                    "extraArgs": ["--target-dir", "target/rust-analyzer"]
                                                }));
                                            } else if section == "rust-analyzer.checkOnSave" {
                                                result_array.push(serde_json::json!({
                                                    "enable": true,
                                                    "command": "check",
                                                    "extraArgs": ["--target-dir", "target/rust-analyzer"]
                                                }));
                                            } else if section == "rust-analyzer.check.extraArgs" || section == "rust-analyzer.checkOnSave.extraArgs" {
                                                result_array.push(serde_json::json!(["--target-dir", "target/rust-analyzer"]));
                                            } else if section == "rust-analyzer.check.command" || section == "rust-analyzer.checkOnSave.command" {
                                                result_array.push(serde_json::json!("check"));
                                            } else if section == "rust-analyzer.checkOnSave.enable" {
                                                result_array.push(serde_json::json!(true));
                                            } else if section == "rust-analyzer.cargo" {
                                                result_array.push(serde_json::json!({
                                                    "targetDir": "target/rust-analyzer"
                                                }));
                                            } else if section == "rust-analyzer.cargo.targetDir" {
                                                result_array.push(serde_json::json!("target/rust-analyzer"));
                                            } else {
                                                result_array.push(serde_json::Value::Null);
                                            }
                                        }
                                    } else {
                                        result_array.push(serde_json::Value::Null);
                                    }
                                    
                                    let response = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id_val,
                                        "result": result_array
                                    });
                                    if let Ok(mut writer) = stdin.lock() {
                                        let _ = write_message(&mut *writer, &response.to_string());
                                    }
                                } else if method == "workspace/semanticTokens/refresh" || method == "workspace/diagnostic/refresh" {
                                    // Already handled above, do nothing
                                } else {
                                    let response = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id_val,
                                        "result": serde_json::Value::Null
                                    });
                                    if let Ok(mut writer) = stdin.lock() {
                                        let _ = write_message(&mut *writer, &response.to_string());
                                    }
                                }
                            }
                        }

                        if resp["method"] == "textDocument/publishDiagnostics" {
                            log::debug!("LSP: publishDiagnostics: {:?}", resp);
                            if let Some(params) = resp.get("params") {
                                let uri = params["uri"].as_str().unwrap_or("");
                                let file_path = uri_to_file_path(uri);
                                if is_path_ignored(&file_path, &gitignore) {
                                    continue;
                                }
                                
                                let mut errors = 0;
                                let mut warnings = 0;
                                let mut diagnostics_list = Vec::new();
                                if let Some(diags) = params["diagnostics"].as_array() {
                                    for d in diags {
                                        let severity = d["severity"].as_i64().unwrap_or(1) as u32;
                                        if severity == 1 {
                                            errors += 1;
                                        } else if severity == 2 {
                                            warnings += 1;
                                        }
                                        
                                        let message = d["message"].as_str().unwrap_or("").to_string();
                                        if let Some(range) = d.get("range") {
                                            let start_line = range["start"]["line"].as_u64().unwrap_or(0) as usize;
                                            let start_col = range["start"]["character"].as_u64().unwrap_or(0) as usize;
                                            let end_line = range["end"]["line"].as_u64().unwrap_or(start_line as u64) as usize;
                                            let end_col = range["end"]["character"].as_u64().unwrap_or(start_col as u64) as usize;
                                            
                                            diagnostics_list.push(DiagnosticDetail {
                                                line: start_line,
                                                col: start_col,
                                                end_line,
                                                end_col,
                                                severity,
                                                message,
                                            });
                                        }
                                    }
                                }
                                let _ = diag_tx.send(LspDiagnosticsUpdate {
                                    file_path: file_path.clone(),
                                    errors,
                                    warnings,
                                    diagnostics: diagnostics_list,
                                    tokens: vec![],
                                    is_tokens_update: false,
                                });
                                let _ = cmd_tx.send(LspCommand::RequestTokensForFile { path: file_path });
                                let _ = proxy.send_event(());
                            }
                        } else if resp["method"] == "$/progress" {
                            if let Some(params) = resp.get("params") {
                                let token = params["token"].as_str()
                                    .map(|s| s.to_string())
                                    .or_else(|| params["token"].as_i64().map(|i| i.to_string()))
                                    .unwrap_or_default();
                                
                                if let Some(value) = params.get("value") {
                                    let kind = value["kind"].as_str().unwrap_or("");
                                    match kind {
                                        "begin" => {
                                            let title = value["title"].as_str().unwrap_or("");
                                            active_progress.insert(token.clone(), title.to_string());
                                            
                                            let message = value["message"].as_str().unwrap_or("");
                                            let status_str = if message.is_empty() {
                                                format!("status:{} ({})", cmd_name_for_reader, title)
                                            } else {
                                                format!("status:{} ({} - {})", cmd_name_for_reader, title, message)
                                            };
                                            let _ = diag_tx.send(LspDiagnosticsUpdate {
                                                file_path: status_str,
                                                errors: 0,
                                                warnings: 0,
                                                diagnostics: vec![],
                                                tokens: vec![],
                                                is_tokens_update: false,
                                            });
                                            let _ = proxy.send_event(());
                                        }
                                        "report" => {
                                            let title = active_progress.get(&token).map(|s| s.as_str()).unwrap_or("");
                                            let message = value["message"].as_str().unwrap_or("");
                                            let percentage = value["percentage"].as_i64();
                                            
                                            let detail = if message.is_empty() {
                                                if let Some(pct) = percentage {
                                                    format!("{}% Finished", pct)
                                                } else {
                                                    "".to_string()
                                                }
                                            } else {
                                                if let Some(pct) = percentage {
                                                    format!("{} - {}%", message, pct)
                                                } else {
                                                    message.to_string()
                                                }
                                            };
                                            
                                            let status_str = if title.is_empty() {
                                                if detail.is_empty() {
                                                    format!("status:{}", cmd_name_for_reader)
                                                } else {
                                                    format!("status:{} ({})", cmd_name_for_reader, detail)
                                                }
                                            } else {
                                                if detail.is_empty() {
                                                    format!("status:{} ({})", cmd_name_for_reader, title)
                                                } else {
                                                    format!("status:{} ({}: {})", cmd_name_for_reader, title, detail)
                                                }
                                            };
                                            
                                            let _ = diag_tx.send(LspDiagnosticsUpdate {
                                                file_path: status_str,
                                                errors: 0,
                                                warnings: 0,
                                                diagnostics: vec![],
                                                tokens: vec![],
                                                is_tokens_update: false,
                                            });
                                            let _ = proxy.send_event(());
                                        }
                                        "end" => {
                                            active_progress.remove(&token);
                                            
                                            let status_str = if active_progress.is_empty() {
                                                format!("status:{}", cmd_name_for_reader)
                                            } else {
                                                let next_title = active_progress.values().next().unwrap();
                                                format!("status:{} ({})", cmd_name_for_reader, next_title)
                                            };
                                            
                                            let _ = diag_tx.send(LspDiagnosticsUpdate {
                                                file_path: status_str,
                                                errors: 0,
                                                warnings: 0,
                                                diagnostics: vec![],
                                                tokens: vec![],
                                                is_tokens_update: false,
                                            });
                                            let _ = proxy.send_event(());

                                            // Request active tokens as indexing is now complete
                                            let _ = cmd_tx.send(LspCommand::RequestActiveTokens { lang_id: lang_id.clone() });
                                            if active_progress.is_empty() {
                                                let _ = cmd_tx.send(LspCommand::RunFlycheck { lang_id: lang_id.clone() });
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        // Check if this response is for a token request
                        let mut is_token_request = false;
                        let mut file_path_for_tokens = None;
                        if let Some(id_val) = resp.get("id") {
                            if let Some(id) = id_val.as_u64() {
                                let mut req_map = token_requests.lock().unwrap();
                                if req_map.contains_key(&id) {
                                    is_token_request = true;
                                    file_path_for_tokens = req_map.remove(&id);
                                }
                            }
                        }

                        if is_token_request {
                            if let Some(file_path) = file_path_for_tokens {
                                let mut parsed_success = false;
                                if let Some(result) = resp.get("result") {
                                    if let Some(data) = result["data"].as_array() {
                                        let mut tokens_list = Vec::new();
                                        let mut current_line = 0;
                                        let mut current_start = 0;

                                        let mut i = 0;
                                        let token_types_guard = token_types.lock().unwrap();
                                        while i + 4 < data.len() {
                                            let delta_line = data[i].as_u64().unwrap_or(0) as usize;
                                            let delta_start = data[i+1].as_u64().unwrap_or(0) as usize;
                                            let length = data[i+2].as_u64().unwrap_or(0) as usize;
                                            let token_type_idx = data[i+3].as_u64().unwrap_or(0) as usize;

                                            if delta_line > 0 {
                                                current_line += delta_line;
                                                current_start = delta_start;
                                            } else {
                                                current_start += delta_start;
                                            }

                                            let token_type = if token_type_idx < token_types_guard.len() {
                                                token_types_guard[token_type_idx].as_str()
                                            } else {
                                                "variable"
                                            }.to_string();

                                            tokens_list.push(SemanticTokenDetail {
                                                line: current_line,
                                                start_col: current_start,
                                                length,
                                                token_type,
                                            });

                                            i += 5;
                                        }

                                        log::debug!("LSP: Parsed {} semantic tokens for {}", tokens_list.len(), file_path);

                                        let _ = diag_tx.send(LspDiagnosticsUpdate {
                                            file_path: file_path.clone(),
                                            errors: 0,
                                            warnings: 0,
                                            diagnostics: vec![],
                                            tokens: tokens_list,
                                            is_tokens_update: true,
                                        });
                                        let _ = proxy.send_event(());
                                        parsed_success = true;
                                    }
                                }

                                if !parsed_success {
                                    // Retry requesting semantic tokens after a delay
                                    log::debug!("LSP: Semantic tokens request failed or null for {}, retrying...", file_path);
                                    let cmd_tx_thread = cmd_tx.clone();
                                    let path_clone = file_path.clone();
                                    std::thread::spawn(move || {
                                        std::thread::sleep(std::time::Duration::from_millis(400));
                                        let _ = cmd_tx_thread.send(LspCommand::RequestTokensForFile { path: path_clone });
                                    });
                                }
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
}

pub fn detect_language_id(path: &str) -> &'static str {
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

pub fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};

    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek() {
        let buf = PathBuf::from(c.as_os_str());
        components.next();
        buf
    } else {
        PathBuf::new()
    };

    let mut normalized = Vec::new();
    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(Component::RootDir.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(c) => {
                normalized.push(c);
            }
        }
    }
    for component in normalized {
        ret.push(component);
    }
    ret
}

pub fn get_absolute_path(path: &str) -> String {
    let path_buf = std::path::PathBuf::from(path);
    let abs_path = if path_buf.is_absolute() {
        path_buf
    } else {
        std::env::current_dir().unwrap_or_default().join(path_buf)
    };
    normalize_path(&abs_path)
        .to_string_lossy()
        .to_string()
}

pub fn uri_to_file_path(uri: &str) -> String {
    let mut raw_path = if uri.starts_with("file://") {
        let path_part = &uri["file://".len()..];
        if path_part.starts_with('/') {
            path_part.to_string()
        } else {
            if let Some(slash_idx) = path_part.find('/') {
                path_part[slash_idx..].to_string()
            } else {
                path_part.to_string()
            }
        }
    } else {
        uri.to_string()
    };

    #[cfg(target_os = "windows")]
    {
        if raw_path.starts_with('/') && raw_path.chars().nth(2) == Some(':') {
            raw_path = raw_path[1..].to_string();
        }
        raw_path = raw_path.replace('/', "\\");
    }

    if let Ok(decoded) = percent_decode(&raw_path) {
        raw_path = decoded;
    }

    get_absolute_path(&raw_path)
}

pub fn percent_decode(s: &str) -> Result<String, std::string::FromUtf8Error> {
    let mut bytes = Vec::new();
    let mut chars = s.as_bytes().iter().peekable();
    while let Some(&b) = chars.next() {
        if b == b'%' {
            if let (Some(&h), Some(&l)) = (chars.next(), chars.next()) {
                if let Some(hex) = hex_to_byte(h, l) {
                    bytes.push(hex);
                    continue;
                }
            }
        }
        bytes.push(b);
    }
    String::from_utf8(bytes)
}

fn hex_to_byte(h: u8, l: u8) -> Option<u8> {
    let h_val = (h as char).to_digit(16)?;
    let l_val = (l as char).to_digit(16)?;
    Some(((h_val << 4) | l_val) as u8)
}

fn parse_legend_token_types(resp_str: &str) -> Vec<String> {
    let mut token_types = Vec::new();
    if let Ok(resp_val) = serde_json::from_str::<serde_json::Value>(resp_str) {
        if let Some(types) = resp_val.get("result")
            .and_then(|r| r.get("capabilities"))
            .and_then(|c| c.get("semanticTokensProvider"))
            .and_then(|s| s.get("legend"))
            .and_then(|l| l.get("tokenTypes"))
            .and_then(|t| t.as_array()) 
        {
            for t in types {
                if let Some(s) = t.as_str() {
                    token_types.push(s.to_string());
                }
            }
        }
    }
    if token_types.is_empty() {
        token_types = vec![
            "namespace".to_string(), "type".to_string(), "class".to_string(), "enum".to_string(), "interface".to_string(), "struct".to_string(), "typeParameter".to_string(),
            "parameter".to_string(), "variable".to_string(), "property".to_string(), "enumMember".to_string(), "event".to_string(), "function".to_string(),
            "method".to_string(), "macro".to_string(), "keyword".to_string(), "modifier".to_string(), "comment".to_string(), "string".to_string(), "number".to_string(),
            "regexp".to_string(), "operator".to_string()
        ];
    }
    token_types
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_path_ignored() {
        let mut builder = ignore::gitignore::GitignoreBuilder::new(".");
        builder.add(".gitignore");
        let gitignore = builder.build().unwrap();

        // Let's test with absolute paths or relative paths since the language server might send both.
        let current_dir = std::env::current_dir().unwrap_or_default();
        let target_file = current_dir.join("target/debug/build/foo.rs");
        let zed_file = current_dir.join("zed_src/crates/editor/src/editor.rs");
        let main_file = current_dir.join("src/main.rs");
        let hidden_file = current_dir.join(".git/config");
        
        let external_cargo_file = std::path::PathBuf::from("/home/source/.cargo/registry/src/lib.rs");
        let std_lib_file = std::path::PathBuf::from("/rustc/8904744439b503d2bbb32/library/core/src/ops/function.rs");

        assert!(is_path_ignored(&target_file.to_string_lossy(), &gitignore), "target folder should be ignored");
        assert!(is_path_ignored(&zed_file.to_string_lossy(), &gitignore), "zed_src folder should be ignored");
        assert!(is_path_ignored(&hidden_file.to_string_lossy(), &gitignore), "hidden .git folder should be ignored");
        assert!(is_path_ignored(&external_cargo_file.to_string_lossy(), &gitignore), "external cargo registry files should be ignored");
        assert!(is_path_ignored(&std_lib_file.to_string_lossy(), &gitignore), "standard library path should be ignored");
        assert!(!is_path_ignored(&main_file.to_string_lossy(), &gitignore), "src/main.rs should NOT be ignored");
    }
}

