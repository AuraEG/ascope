use ascope::search::ripgrep::{spawn_rg_worker, RgMessage, RgSearchQuery};
use std::sync::mpsc;
use tempfile::tempdir;
use std::fs::File;
use std::io::Write;

#[test]
fn test_async_ripgrep_search() {
    let dir = tempdir().unwrap();
    let file1 = dir.path().join("test1.txt");
    let mut f1 = File::create(&file1).unwrap();
    f1.write_all(b"hello world\nthis is a test\nrust is awesome\n").unwrap();

    let (query_tx, query_rx) = mpsc::channel();
    let (match_tx, match_rx) = mpsc::channel();

    // Spawn the background worker
    let _handle = std::thread::spawn(move || {
        spawn_rg_worker(query_rx, match_tx);
    });

    // Send search query
    query_tx.send(RgSearchQuery {
        query: "awesome".to_string(),
        dir: dir.path().to_path_buf(),
    }).unwrap();

    // Wait and receive matches
    let mut matches = Vec::new();
    let start = std::time::Instant::now();
    loop {
        if let Ok(msg) = match_rx.recv_timeout(std::time::Duration::from_millis(1000)) {
            match msg {
                RgMessage::Match(m) => {
                    matches.push(m);
                }
                RgMessage::Finished => {
                    break;
                }
            }
        }
        if start.elapsed().as_secs() > 5 {
            panic!("ripgrep search timed out");
        }
    }

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].line_number, 3);
    assert!(matches[0].text.contains("rust is awesome"));
}
