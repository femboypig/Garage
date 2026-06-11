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
    RetrySpawn { lang_id: String },
}

pub struct LspClient {
    cmd_tx: Sender<LspCommand>,
}

struct ServerInstance {
    stdin: std::process::ChildStdin,
    cmd_name: String,
}

fn get_local_lsp_path(subpath: &str) -> String {
    let current_dir = std::env::current_dir().unwrap_or_default();
    current_dir.join(".lsp").join(subpath).to_string_lossy().to_string()
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
            list.push((get_local_lsp_path("node_modules/.bin/typescript-language-server"), vec!["--stdio".to_string()]));
            list
        }
        "json" => vec![
            ("vscode-json-language-server".to_string(), vec!["--stdio".to_string()]),
            (get_local_lsp_path("node_modules/.bin/vscode-json-language-server"), vec!["--stdio".to_string()]),
        ],
        "yaml" => vec![
            ("yaml-language-server".to_string(), vec!["--stdio".to_string()]),
            (get_local_lsp_path("node_modules/.bin/yaml-language-server"), vec!["--stdio".to_string()]),
        ],
        "html" => vec![
            ("vscode-html-language-server".to_string(), vec!["--stdio".to_string()]),
            (get_local_lsp_path("node_modules/.bin/vscode-html-language-server"), vec!["--stdio".to_string()]),
        ],
        "css" | "scss" => vec![
            ("vscode-css-language-server".to_string(), vec!["--stdio".to_string()]),
            (get_local_lsp_path("node_modules/.bin/vscode-css-language-server"), vec!["--stdio".to_string()]),
        ],
        "markdown" => vec![
            ("marksman".to_string(), vec!["server".to_string()]),
            (get_local_lsp_path("marksman"), vec!["server".to_string()]),
        ],
        _ => return Err("No configured LSP server for this language".to_string()),
    };

    let mut last_err = String::new();
    for (cmd, args) in alternatives {
        if cmd.contains('/') && !std::path::Path::new(&cmd).exists() {
            continue;
        }
        log::info!("LSP: Trying to spawn {} {:?}", cmd, args);
        match Command::new(&cmd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
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
    lang_id: &'static str,
    cmd_tx: Sender<LspCommand>,
    diagnostics_tx: Sender<LspDiagnosticsUpdate>,
    event_loop_proxy: winit::event_loop::EventLoopProxy<()>,
) {
    let tx = cmd_tx;
    let diag_tx = diagnostics_tx;
    let proxy = event_loop_proxy;
    std::thread::spawn(move || {
        let lsp_dir = std::env::current_dir().unwrap_or_default().join(".lsp");
        let _ = std::fs::create_dir_all(&lsp_dir);

        let install_result = match lang_id {
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
                let display_name = match lang_id {
                    "rust" => "rust-analyzer",
                    "python" => "pyright/pylsp",
                    "go" => "gopls",
                    "c" | "cpp" => "clangd",
                    "typescript" | "javascript" => "typescript-language-server",
                    "json" => "json-language-server",
                    "yaml" => "yaml-language-server",
                    "html" => "html-language-server",
                    "css" | "scss" => "css-language-server",
                    "markdown" => "marksman",
                    _ => lang_id,
                };
                let _ = diag_tx.send(LspDiagnosticsUpdate {
                    file_path: format!("status:offline (install failed for {})", display_name),
                    errors: 9999,
                    warnings: 0,
                });
                let _ = proxy.send_event(());
            }
        }
    });
}

impl LspClient {
    pub fn new(
        diagnostics_tx: Sender<LspDiagnosticsUpdate>,
        event_loop_proxy: winit::event_loop::EventLoopProxy<()>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<LspCommand>();
        
        let cmd_tx_clone = cmd_tx.clone();
        let proxy_init = event_loop_proxy.clone();
        std::thread::spawn(move || {
            let mut document_versions = HashMap::<String, usize>::new();
            let mut open_documents = HashMap::<String, String>::new();
            let mut servers = HashMap::<String, ServerInstance>::new();
            let mut installing_servers = Vec::<String>::new();
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
                        open_documents.insert(path.clone(), text.clone());
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
                                                    },
                                                    "window": {
                                                        "workDoneProgress": true
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
                                                
                                                start_reader_thread(stdout_reader, cmd_name.clone(), diagnostics_tx.clone(), proxy_init.clone());

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
                                         if !installing_servers.contains(&lang_id.to_string()) && (lang_id == "rust" || lang_id == "python" || lang_id == "go" || lang_id == "c" || lang_id == "cpp" || lang_id == "typescript" || lang_id == "javascript" || lang_id == "json" || lang_id == "yaml" || lang_id == "html" || lang_id == "css" || lang_id == "scss" || lang_id == "markdown") {
                                             installing_servers.push(lang_id.to_string());
                                             let display_name = match lang_id {
                                                 "rust" => "rust-analyzer",
                                                 "python" => "python-lsp-server",
                                                 "go" => "gopls",
                                                 "c" | "cpp" => "clangd",
                                                 "typescript" | "javascript" => "typescript-language-server",
                                                 "json" => "json-language-server",
                                                 "yaml" => "yaml-language-server",
                                                 "html" => "html-language-server",
                                                 "css" | "scss" => "css-language-server",
                                                 "markdown" => "marksman",
                                                 _ => lang_id,
                                             };
                                            let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                                file_path: format!("status:installing {}...", display_name),
                                                errors: 0,
                                                warnings: 0,
                                            });
                                            let _ = proxy_init.send_event(());
                                            attempt_auto_install(lang_id, cmd_tx_clone.clone(), diagnostics_tx.clone(), proxy_init.clone());
                                        } else {
                                            let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                                file_path: "status:offline".to_string(),
                                                errors: 9999,
                                                warnings: 0,
                                            });
                                            let _ = proxy_init.send_event(());
                                        }
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
                        open_documents.insert(path.clone(), text.clone());
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
                        } else if installing_servers.contains(&lang_id.to_string()) {
                            let display_name = match lang_id {
                                "rust" => "rust-analyzer",
                                "python" => "python-lsp-server",
                                "go" => "gopls",
                                "c" | "cpp" => "clangd",
                                "typescript" | "javascript" => "typescript-language-server",
                                "json" => "json-language-server",
                                "yaml" => "yaml-language-server",
                                "html" => "html-language-server",
                                "css" | "scss" => "css-language-server",
                                "markdown" => "marksman",
                                _ => lang_id,
                            };
                            let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                file_path: format!("status:installing {}...", display_name),
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
                    Ok(LspCommand::RetrySpawn { lang_id }) => {
                        installing_servers.retain(|x| x != &lang_id);
                        if !servers.contains_key(&lang_id) {
                            match spawn_server(&lang_id) {
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
                                                },
                                                "window": {
                                                    "workDoneProgress": true
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

                                    log::info!("LSP: Sending initialize to {} after auto-install...", cmd_name);
                                    if write_message(&mut stdin, &init_msg.to_string()).is_ok() {
                                        if let Ok(_resp_str) = read_message(&mut stdout_reader) {
                                            log::info!("LSP: Initialize response from {} received", cmd_name);
                                            let initialized_msg = serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "method": "initialized",
                                                "params": {}
                                            });
                                            let _ = write_message(&mut stdin, &initialized_msg.to_string());
                                            
                                            start_reader_thread(stdout_reader, cmd_name.clone(), diagnostics_tx.clone(), proxy_init.clone());

                                            let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                                file_path: format!("status:{}", cmd_name),
                                                errors: 0,
                                                warnings: 0,
                                            });
                                            let _ = proxy_init.send_event(());
                                            
                                            // Open all files that belong to this language
                                            for (path, text) in &open_documents {
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
                                                    let _ = write_message(&mut stdin, &open_msg.to_string());
                                                }
                                            }
                                            
                                            servers.insert(lang_id.to_string(), ServerInstance { stdin, cmd_name });
                                        }
                                    }
                                }
                                Err(e_msg) => {
                                    log::warn!("LSP: Spawn failed again for {} after auto-install: {}", lang_id, e_msg);
                                    let _ = diagnostics_tx.send(LspDiagnosticsUpdate {
                                        file_path: "status:offline".to_string(),
                                        errors: 9999,
                                        warnings: 0,
                                    });
                                    let _ = proxy_init.send_event(());
                                }
                            }
                        }
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

fn start_reader_thread(
    stdout_reader: BufReader<std::process::ChildStdout>,
    cmd_name: String,
    diag_tx: Sender<LspDiagnosticsUpdate>,
    proxy: winit::event_loop::EventLoopProxy<()>,
) {
    let cmd_name_for_reader = cmd_name;
    std::thread::spawn(move || {
        let mut stdout_reader = stdout_reader;
        let mut active_progress = HashMap::<String, String>::new();
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
                                            });
                                            let _ = proxy.send_event(());
                                        }
                                        _ => {}
                                    }
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
