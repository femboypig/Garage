use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread;
use winit::event_loop::EventLoopProxy;

pub fn update_git_branch(tx: Sender<String>, proxy: EventLoopProxy<()>) {
    thread::spawn(move || {
        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output();

        if let Ok(out) = output
            && out.status.success()
        {
            let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !branch.is_empty() {
                let _ = tx.send(branch);
                // Wake the event loop so AboutToWait can drain the channel.
                // UserEvent no longer blindly redraws; it just schedules a drain.
                let _ = proxy.send_event(());
            }
        }
    });
}
