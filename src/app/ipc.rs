use std::sync::{Arc, Mutex};
use std::thread;
use std::os::unix::net::{UnixListener, UnixStream};
use std::io::{Read, Write};
use std::fs;
use std::path::{Path, PathBuf};
use winit::window::Window;
use winit::event_loop::EventLoopProxy;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct WindowInfo {
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub socket_path: String,
}

const SHRED_STATE_FILE: &str = "/tmp/garage-active-windows.json";

fn is_process_running(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

/// Helper to load and filter active windows list
fn load_active_windows() -> Vec<WindowInfo> {
    let path = Path::new(SHRED_STATE_FILE);
    if !path.exists() {
        return Vec::new();
    }
    let content = match fs::read_to_string(path) {
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
    let _ = fs::write(SHRED_STATE_FILE, content);
}

/// Start IPC Server thread for this window
pub fn start_ipc_server(
    pending_open_files: Arc<Mutex<Vec<String>>>,
    proxy: EventLoopProxy<()>,
) -> String {
    let pid = std::process::id();
    let socket_path = format!("/tmp/garage-ipc-{}.sock", pid);
    
    // Remove if socket file already exists
    let _ = fs::remove_file(&socket_path);
    
    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(_) => return socket_path,
    };
    
    let socket_path_clone = socket_path.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut data = String::new();
                if stream.read_to_string(&mut data).is_ok() {
                    let file_path = data.trim().to_string();
                    if !file_path.is_empty() {
                        if let Ok(mut pending) = pending_open_files.lock() {
                            pending.push(file_path);
                        }
                        let _ = proxy.send_event(());
                    }
                }
            }
        }
        let _ = fs::remove_file(&socket_path_clone);
    });
    
    socket_path
}

/// Register this window at startup
pub fn register_window(window: &Window, socket_path: &str) {
    let pid = std::process::id();
    let pos = window.inner_position().unwrap_or(winit::dpi::PhysicalPosition::new(0, 0));
    let size = window.outer_size();
    
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
    let size = window.outer_size();
    
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
    
    for w in list {
        if w.pid == current_pid {
            continue;
        }
        
        let inside_x = global_x >= w.x && global_x < w.x + w.width as i32;
        let inside_y = global_y >= w.y && global_y < w.y + w.height as i32;
        
        if inside_x && inside_y {
            // Attempt to connect to other window's IPC socket and send the path
            if let Ok(mut stream) = UnixStream::connect(&w.socket_path) {
                if stream.write_all(file_path.as_bytes()).is_ok() {
                    return true;
                }
            }
        }
    }
    
    false
}
