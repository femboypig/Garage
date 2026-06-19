use std::collections::HashMap;
use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::SystemTime;
use winit::event_loop::EventLoopProxy;

pub fn update_git_file_blame(
    file_path: String,
    tx: Sender<(String, HashMap<usize, String>)>,
    proxy: EventLoopProxy<()>,
) {
    thread::spawn(move || {
        let output = Command::new("git")
            .args(&["blame", "--porcelain", &file_path])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);

                struct CommitInfo {
                    author: String,
                    time: u64,
                    summary: String,
                }

                let mut commits = HashMap::<String, CommitInfo>::new();
                let mut line_commits = HashMap::<usize, String>::new();

                let mut lines = stdout.lines();
                while let Some(line) = lines.next() {
                    if line.starts_with('\t') {
                        continue;
                    }
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.is_empty() {
                        continue;
                    }
                    let first_part = parts[0];
                    if first_part.len() == 40 && parts.len() >= 3 {
                        let commit_hash = first_part.to_string();
                        if let Ok(result_line) = parts[2].parse::<usize>() {
                            line_commits.insert(result_line, commit_hash.clone());
                            if !commits.contains_key(&commit_hash) {
                                let mut author = None;
                                let mut author_time = None;
                                let mut summary = None;

                                while let Some(hdr_line) = lines.next() {
                                    if hdr_line.starts_with('\t') {
                                        break;
                                    }
                                    if hdr_line.starts_with("author ") {
                                        author =
                                            Some(hdr_line["author ".len()..].trim().to_string());
                                    } else if hdr_line.starts_with("author-time ") {
                                        author_time = hdr_line["author-time ".len()..]
                                            .trim()
                                            .parse::<u64>()
                                            .ok();
                                    } else if hdr_line.starts_with("summary ") {
                                        summary =
                                            Some(hdr_line["summary ".len()..].trim().to_string());
                                    }
                                }

                                if let (Some(auth), Some(time), Some(sum)) =
                                    (author, author_time, summary)
                                {
                                    commits.insert(
                                        commit_hash,
                                        CommitInfo {
                                            author: auth,
                                            time,
                                            summary: sum,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }

                let mut file_blame_map = HashMap::new();
                for (result_line, commit_hash) in line_commits {
                    if let Some(info) = commits.get(&commit_hash) {
                        let blame_str = if info.author == "Not Committed Yet" {
                            "Not Committed Yet".to_string()
                        } else {
                            let now = SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let diff = now.saturating_sub(info.time);
                            let time_str = if diff < 60 {
                                "just now".to_string()
                            } else if diff < 3600 {
                                format!("{}m ago", diff / 60)
                            } else if diff < 86400 {
                                format!("{}h ago", diff / 3600)
                            } else if diff < 2592000 {
                                let days = diff / 86400;
                                if days == 1 {
                                    "yesterday".to_string()
                                } else {
                                    format!("{} days ago", days)
                                }
                            } else if diff < 31536000 {
                                let months = diff / 2592000;
                                if months == 1 {
                                    "1 month ago".to_string()
                                } else {
                                    format!("{} months ago", months)
                                }
                            } else {
                                let years = diff / 31536000;
                                if years == 1 {
                                    "1 year ago".to_string()
                                } else {
                                    format!("{} years ago", years)
                                }
                            };
                            format!("{} • {} • {}", info.author, time_str, info.summary)
                        };
                        file_blame_map.insert(result_line - 1, blame_str);
                    }
                }

                let _ = tx.send((file_path, file_blame_map));
                let _ = proxy.send_event(());
            }
        }
    });
}
