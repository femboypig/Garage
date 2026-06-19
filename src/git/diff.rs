use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread;
use winit::event_loop::EventLoopProxy;

#[derive(Clone, Debug, PartialEq)]
pub enum GitDiffHunk {
    Added { line: usize, count: usize },
    Modified { line: usize, count: usize },
    Deleted { line: usize },
}

pub fn update_git_diff(
    file_path: String,
    tx: Sender<(String, Vec<GitDiffHunk>)>,
    proxy: EventLoopProxy<()>,
) {
    thread::spawn(move || {
        let output = Command::new("git")
            .args(&["diff", "--no-ext-diff", "-U0", "--", &file_path])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut hunks = Vec::new();

            for line in stdout.lines() {
                if line.starts_with("@@ ") {
                    let parts: Vec<&str> = line.split("@@").collect();
                    if parts.len() >= 2 {
                        let header = parts[1].trim();
                        let specs: Vec<&str> = header.split_whitespace().collect();
                        if specs.len() >= 2 {
                            let new_spec = specs[1];
                            if new_spec.starts_with('+') {
                                let content = &new_spec[1..];
                                let subparts: Vec<&str> = content.split(',').collect();
                                if !subparts.is_empty() {
                                    let line_idx =
                                        subparts[0].parse::<usize>().unwrap_or(1).saturating_sub(1);
                                    let count = if subparts.len() >= 2 {
                                        subparts[1].parse::<usize>().unwrap_or(1)
                                    } else {
                                        1
                                    };

                                    let old_spec = specs[0];
                                    let old_count = if old_spec.starts_with('-') {
                                        let old_content = &old_spec[1..];
                                        let old_subparts: Vec<&str> =
                                            old_content.split(',').collect();
                                        if old_subparts.len() >= 2 {
                                            old_subparts[1].parse::<usize>().unwrap_or(1)
                                        } else {
                                            1
                                        }
                                    } else {
                                        1
                                    };

                                    if old_count == 0 {
                                        hunks.push(GitDiffHunk::Added {
                                            line: line_idx,
                                            count,
                                        });
                                    } else if count == 0 {
                                        hunks.push(GitDiffHunk::Deleted { line: line_idx });
                                    } else {
                                        hunks.push(GitDiffHunk::Modified {
                                            line: line_idx,
                                            count,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let _ = tx.send((file_path, hunks));
            let _ = proxy.send_event(());
        }
    });
}
