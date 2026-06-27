use std::sync::{Arc, Mutex};
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct WindowInfo {
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub socket_path: String,
}

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::thread;

    fn ensure_private_dir(path: &Path) {
        let _ = fs::create_dir_all(path);
        if let Ok(metadata) = fs::symlink_metadata(path)
            && metadata.file_type().is_dir()
        {
            let mut perms = metadata.permissions();
            perms.set_mode(0o700);
            let _ = fs::set_permissions(path, perms);
        }
    }

    fn get_secure_runtime_dir() -> PathBuf {
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            let path = PathBuf::from(runtime_dir).join("garage");
            ensure_private_dir(&path);
            path
        } else {
            let user = std::env::var("USER").unwrap_or_else(|_| "default".to_string());
            let path = PathBuf::from("/tmp").join(format!("garage-runtime-{}", user));
            ensure_private_dir(&path);
            path
        }
    }

    fn normalize_ipc_path(data: &str) -> Option<String> {
        let file_path = data.trim();
        if file_path.is_empty() || file_path.chars().any(|c| c.is_control()) {
            return None;
        }

        let path = PathBuf::from(file_path).canonicalize().ok()?;
        Some(path.to_string_lossy().into_owned())
    }

    fn active_windows_file_path() -> PathBuf {
        get_secure_runtime_dir().join("active-windows.json")
    }

    fn is_process_running(pid: u32) -> bool {
        // kill(pid, 0) sends no signal but checks if process exists — works on Linux and macOS
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }

    /// Helper to load and filter active windows list
    fn load_active_windows() -> Vec<WindowInfo> {
        let path = active_windows_file_path();
        if !path.exists() {
            return Vec::new();
        }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let list: Vec<WindowInfo> = match serde_json::from_str(&content) {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        };
        // Clean up stale ones
        let mut active = Vec::new();
        let current_pid = std::process::id();
        for w in list {
            if w.pid == current_pid || is_process_running(w.pid) {
                active.push(w);
            } else {
                // Remove stale socket file if it exists
                let _ = fs::remove_file(&w.socket_path);
            }
        }
        active
    }

    /// Helper to save active windows list
    fn save_active_windows(list: &[WindowInfo]) {
        let content = match serde_json::to_string(list) {
            Ok(c) => c,
            Err(_) => return,
        };
        let _ = fs::write(active_windows_file_path(), content);
    }

    /// Start IPC Server thread for this window
    pub fn start_ipc_server(
        pending_open_files: Arc<Mutex<Vec<String>>>,
        proxy: EventLoopProxy<()>,
    ) -> String {
        let pid = std::process::id();
        let socket_path = get_secure_runtime_dir()
            .join(format!("garage-ipc-{}.sock", pid))
            .to_string_lossy()
            .into_owned();

        // Remove if socket file already exists
        let _ = fs::remove_file(&socket_path);

        let listener = match UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(_) => return socket_path,
        };
        if let Ok(metadata) = fs::symlink_metadata(&socket_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o600);
            let _ = fs::set_permissions(&socket_path, perms);
        }

        let socket_path_clone = socket_path.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut data = String::new();
                // Avoid DoS/OOM by limiting maximum path reading size to 4KB
                if stream.take(4096).read_to_string(&mut data).is_ok()
                    && let Some(file_path) = normalize_ipc_path(&data)
                {
                    if let Ok(mut pending) = pending_open_files.lock() {
                        pending.push(file_path);
                    }
                    let _ = proxy.send_event(());
                }
            }
            let _ = fs::remove_file(&socket_path_clone);
        });

        socket_path
    }

    /// Register this window at startup
    pub fn register_window(window: &Window, socket_path: &str) {
        let pid = std::process::id();
        let pos = window
            .inner_position()
            .unwrap_or(winit::dpi::PhysicalPosition::new(0, 0));
        let size = window.inner_size();

        let mut list = load_active_windows();
        // Remove existing if any
        list.retain(|w| w.pid != pid);

        list.push(WindowInfo {
            pid,
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
            socket_path: socket_path.to_string(),
        });

        save_active_windows(&list);
    }

    /// Update window geometry when moved or resized
    pub fn update_window_geometry(window: &Window) {
        let pid = std::process::id();
        let pos = match window.inner_position() {
            Ok(p) => p,
            Err(_) => return,
        };
        let size = window.inner_size();

        let mut list = load_active_windows();
        let mut found = false;
        for w in &mut list {
            if w.pid == pid {
                w.x = pos.x;
                w.y = pos.y;
                w.width = size.width;
                w.height = size.height;
                found = true;
                break;
            }
        }

        if found {
            save_active_windows(&list);
        }
    }

    /// Unregister window on exit
    pub fn unregister_window() {
        let pid = std::process::id();
        let mut list = load_active_windows();
        list.retain(|w| w.pid != pid);
        save_active_windows(&list);
    }

    /// Check if a global mouse coordinate is inside another open window, and send file path to it if so.
    /// Returns true if successfully dropped into another window.
    pub fn try_drop_to_other_window(global_x: i32, global_y: i32, file_path: &str) -> bool {
        let list = load_active_windows();
        let current_pid = std::process::id();
        let runtime_dir = get_secure_runtime_dir();

        for w in list {
            if w.pid == current_pid {
                continue;
            }

            let inside_x = global_x >= w.x && global_x < w.x + w.width as i32;
            let inside_y = global_y >= w.y && global_y < w.y + w.height as i32;

            if inside_x && inside_y {
                let socket_path = PathBuf::from(&w.socket_path);
                if !socket_path.starts_with(&runtime_dir) {
                    continue;
                }
                // Attempt to connect to other window's IPC socket and send the path
                if let Ok(mut stream) = UnixStream::connect(&socket_path)
                    && stream.write_all(file_path.as_bytes()).is_ok()
                {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(unix)]
pub use unix_impl::*;

#[cfg(not(unix))]
mod dummy_impl {
    use super::*;

    pub fn start_ipc_server(
        _pending_open_files: Arc<Mutex<Vec<String>>>,
        _proxy: EventLoopProxy<()>,
    ) -> String {
        String::new()
    }

    pub fn register_window(_window: &Window, _socket_path: &str) {}

    pub fn update_window_geometry(_window: &Window) {}

    pub fn unregister_window() {}

    pub fn try_drop_to_other_window(_global_x: i32, _global_y: i32, _file_path: &str) -> bool {
        false
    }
}

#[cfg(not(unix))]
pub use dummy_impl::*;
