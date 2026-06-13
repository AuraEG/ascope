// ==========================================================================
// File    : main.rs
// Project : AuraScope
// Layer   : Entry
// Purpose : CLI entrypoint; dispatches to TUI or stats-only mode.
//
// Author  : Ahmed Ashour
// Created : 2026-06-13
// ==========================================================================

mod fs;

use clap::Parser;
use std::path::PathBuf;

// --------------------------------------------------------------------------
// [SECTION] CLI Arguments
// --------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(author, version, about = "Blazingly fast terminal workspace inspector")]
struct Args {
    /// Root path to inspect (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Print disk-usage statistics without launching the TUI
    #[arg(long)]
    stats: bool,

    /// Emit statistics as JSON (requires --stats)
    #[arg(long)]
    json: bool,
}

// --------------------------------------------------------------------------
// [SECTION] Entry
// --------------------------------------------------------------------------

fn main() {
    let args = Args::parse();

    if args.stats {
        match fs::walker::scan_path(&args.path) {
            Ok(stats) => {
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&stats).unwrap());
                } else {
                    println!("Scan Path : {:?}", args.path);
                    println!("Total Size: {} bytes", stats.total_size);
                    println!("File Count: {}", stats.file_count);
                }
            }
            Err(e) => eprintln!("[x] Scan error: {e}"),
        }
    }
}
