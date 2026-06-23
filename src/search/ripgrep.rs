use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};

#[derive(Debug, Clone)]
pub struct RgSearchQuery {
    pub query: String,
    pub dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RgMatch {
    pub path: PathBuf,
    pub line_number: usize,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum RgMessage {
    Match(RgMatch),
    Finished,
}

/// A background worker thread function that processes incoming ripgrep search queries.
/// It kills any active rg child process when a new query is received.
pub fn spawn_rg_worker(query_rx: Receiver<RgSearchQuery>, match_tx: Sender<RgMessage>) {
    let mut current_child: Option<std::process::Child> = None;

    while let Ok(search) = query_rx.recv() {
        // Kill previous active process if any
        if let Some(mut child) = current_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        if search.query.is_empty() {
            let _ = match_tx.send(RgMessage::Finished);
            continue;
        }

        // Spawn new rg command
        let child_res = Command::new("rg")
            .args([
                "--json",
                "-S", // Smart case matching
                "--line-number",
                "--column",
                "--no-heading",
                "--color=never",
                "--glob=!node_modules",
                "--glob=!target",
                "--glob=!.git",
                &search.query,
                &search.dir.to_string_lossy(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        match child_res {
            Ok(mut child) => {
                let stdout = child.stdout.take().expect("Failed to open stdout");
                let match_tx_clone = match_tx.clone();
                
                // Track child so we can cancel it if needed
                current_child = Some(child);

                // Run a helper reader loop on stdout
                let reader = BufReader::new(stdout);
                let mut current_file: Option<PathBuf> = None;

                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        if let Some(msg_type) = val.get("type").and_then(|t| t.as_str()) {
                            match msg_type {
                                "begin" => {
                                    if let Some(path_val) = val.get("data").and_then(|d| d.get("path")).and_then(|p| p.get("text")).and_then(|t| t.as_str()) {
                                        current_file = Some(PathBuf::from(path_val));
                                    }
                                }
                                "match" => {
                                    let line_number = val.get("data")
                                        .and_then(|d| d.get("line_number"))
                                        .and_then(|l| l.as_u64())
                                        .unwrap_or(0) as usize;

                                    let text = val.get("data")
                                        .and_then(|d| d.get("lines"))
                                        .and_then(|l| l.get("text"))
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    if let Some(ref path) = current_file {
                                        let m = RgMatch {
                                            path: path.clone(),
                                            line_number,
                                            text,
                                        };
                                        if match_tx_clone.send(RgMessage::Match(m)).is_err() {
                                            break;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Search is finished or child process terminated
                let _ = match_tx_clone.send(RgMessage::Finished);
            }
            Err(_) => {
                let _ = match_tx.send(RgMessage::Finished);
            }
        }
    }
}
