use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread;
use winit::event_loop::EventLoopProxy;

pub fn update_git_statuses(tx: Sender<HashMap<PathBuf, String>>, proxy: EventLoopProxy<()>) {
    thread::spawn(move || {
        let output = Command::new("git")
            .args(&["status", "--porcelain"])
            .output();
        
        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let mut map = HashMap::new();
                for line in stdout.lines() {
                    if line.len() > 3 {
                        let status = line[0..2].to_string();
                        let file_path = PathBuf::from(line[3..].trim().to_string());
                        map.insert(file_path, status);
                    }
                }
                let _ = tx.send(map);
                let _ = proxy.send_event(());
            }
        }
    });
}
