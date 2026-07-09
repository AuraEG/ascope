// File    : preview.rs
// Project : AuraScope
// Layer   : TUI
// Purpose : Handles image rendering protocols and PDF previews.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{DynamicImage, GenericImageView};
use once_cell::sync::Lazy;
use ratatui::prelude::*;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalProtocol {
    Kitty,
    Iterm2,
    Sixel,
    HalfBlock,
}

pub static HAS_CHAFA: Lazy<bool> = Lazy::new(|| {
    if cfg!(test) {
        return false;
    }
    Command::new("chafa")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
});

/// Detect the best terminal graphics protocol supported by the current environment.
pub fn detect_protocol() -> TerminalProtocol {
    *DETECTED_PROTOCOL
}

pub static TMUX_NATIVE_SIXEL: Lazy<bool> = Lazy::new(|| {
    if std::env::var("TMUX").is_err() {
        return false;
    }
    Command::new("tmux")
        .args(["display", "-p", "#{client_termfeatures}"])
        .output()
        .map(|out| {
            let s = String::from_utf8_lossy(&out.stdout);
            s.contains("sixel")
        })
        .unwrap_or(false)
});

pub static DETECTED_PROTOCOL: Lazy<TerminalProtocol> = Lazy::new(|| {
    if cfg!(test) {
        return TerminalProtocol::HalfBlock;
    }

    let in_tmux = std::env::var("TMUX").is_ok();

    if in_tmux {
        // 1. Check if tmux natively supports Sixel
        if *TMUX_NATIVE_SIXEL {
            return TerminalProtocol::Sixel;
        }

        // 2. Check if tmux allow-passthrough is enabled (on or all)
        let passthrough_enabled = Command::new("tmux")
            .args(["show-options", "-gqv", "allow-passthrough"])
            .output()
            .map(|out| {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
                s == "on" || s == "all"
            })
            .unwrap_or(false);

        if !passthrough_enabled {
            // If tmux doesn't support Sixel natively and allow-passthrough is off,
            // we MUST fall back to HalfBlock because any graphics sequence will be blocked.
            return TerminalProtocol::HalfBlock;
        }
    }

    // Now detect the host terminal / protocol
    // 1. Check WezTerm
    let is_wezterm = std::env::var("WEZTERM_PANE").is_ok()
        || std::env::var("TERM_PROGRAM")
            .map(|s| s == "WezTerm")
            .unwrap_or(false);
    if is_wezterm {
        return TerminalProtocol::Iterm2;
    }

    // 2. Check Ghosty
    let is_ghosty = std::env::var("GHOSTY_RESOURCES_DIR").is_ok()
        || std::env::var("GHOSTY_SOCKET_DIR").is_ok()
        || std::env::var("TERM_PROGRAM")
            .map(|s| s == "Ghosty")
            .unwrap_or(false);
    if is_ghosty {
        return TerminalProtocol::Kitty;
    }

    // 3. Check Kitty
    let is_kitty = std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("TERM")
            .map(|s| s.contains("kitty"))
            .unwrap_or(false);
    if is_kitty {
        return TerminalProtocol::Kitty;
    }

    // 4. Check iTerm2
    let is_iterm2 = std::env::var("TERM_PROGRAM")
        .map(|s| s == "iTerm.app")
        .unwrap_or(false);
    if is_iterm2 {
        return TerminalProtocol::Iterm2;
    }

    // 5. Check Windows Terminal
    if std::env::var("WT_SESSION").is_ok() {
        return TerminalProtocol::Sixel;
    }

    // 6. Generic Sixel detection
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("foot") || term.contains("sixel") {
            return TerminalProtocol::Sixel;
        }
    }
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        if term_program == "foot" || term_program == "Zellij" || term_program == "vscode" {
            return TerminalProtocol::Sixel;
        }
    }

    TerminalProtocol::HalfBlock
});

/// Convert a PDF's first page into a temporary PNG image using standard CLI tools.
pub fn extract_pdf_first_page(pdf_path: &Path) -> Result<PathBuf, String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    pdf_path.hash(&mut hasher);
    let hash_val = hasher.finish();

    let temp_dir = std::env::temp_dir().join("ascope_previews");
    let _ = std::fs::create_dir_all(&temp_dir);
    let output_png = temp_dir.join(format!("pdf_{:x}.png", hash_val));

    // If cached PNG is newer than PDF, reuse it
    if output_png.exists() {
        if let (Ok(pdf_meta), Ok(png_meta)) = (pdf_path.metadata(), output_png.metadata()) {
            if let (Ok(pdf_time), Ok(png_time)) = (pdf_meta.modified(), png_meta.modified()) {
                if png_time > pdf_time {
                    return Ok(output_png);
                }
            }
        }
    }

    // Try pdftoppm (poppler-utils)
    let prefix = temp_dir.join(format!("pdf_{:x}_page", hash_val));
    let output = Command::new("pdftoppm")
        .args([
            "-png",
            "-f",
            "1",
            "-l",
            "1",
            "-r",
            "150",
            &pdf_path.to_string_lossy(),
            &prefix.to_string_lossy(),
        ])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let generated_png = temp_dir.join(format!("pdf_{:x}_page-1.png", hash_val));
            if generated_png.exists() {
                let _ = std::fs::rename(&generated_png, &output_png);
                return Ok(output_png);
            }
        }
    }

    // Try mutool draw (mupdf)
    let output = Command::new("mutool")
        .args([
            "draw",
            "-o",
            &output_png.to_string_lossy(),
            &pdf_path.to_string_lossy(),
            "1",
        ])
        .output();

    if let Ok(out) = output {
        if out.status.success() && output_png.exists() {
            return Ok(output_png);
        }
    }

    // Try gs (Ghostscript)
    let output = Command::new("gs")
        .args([
            "-dNOPAUSE",
            "-sDEVICE=png16m",
            "-r150",
            &format!("-sOutputFile={}", output_png.to_string_lossy()),
            "-dFirstPage=1",
            "-dLastPage=1",
            "-q",
            &pdf_path.to_string_lossy(),
            "-c",
            "quit",
        ])
        .output();

    if let Ok(out) = output {
        if out.status.success() && output_png.exists() {
            return Ok(output_png);
        }
    }

    Err("No PDF renderer tool found (install poppler-utils, mupdf, or ghostscript)".to_string())
}

/// Resizes the image and renders it using Unicode half-blocks with truecolor.
pub fn render_half_block(img: &DynamicImage, cols: u16, rows: u16) -> Vec<Line<'static>> {
    if cols == 0 || rows == 0 {
        return vec![];
    }
    let target_w = cols as u32;
    let target_h = (rows * 2) as u32;
    let resized = img.thumbnail(target_w, target_h);
    let (w, h) = resized.dimensions();

    let mut lines = Vec::new();
    for y in (0..h).step_by(2) {
        let mut spans = Vec::new();
        for x in 0..w {
            let top_pixel = resized.get_pixel(x, y);
            let bottom_pixel = if y + 1 < h {
                resized.get_pixel(x, y + 1)
            } else {
                image::Rgba([0, 0, 0, 0])
            };

            let style = Style::default()
                .fg(Color::Rgb(
                    bottom_pixel[0],
                    bottom_pixel[1],
                    bottom_pixel[2],
                ))
                .bg(Color::Rgb(top_pixel[0], top_pixel[1], top_pixel[2]));
            spans.push(Span::styled("▄", style));
        }
        lines.push(Line::from(spans));
    }
    lines
}

pub fn render_chafa_symbols(
    path: &Path,
    cols: u16,
    rows: u16,
) -> Result<Vec<Line<'static>>, String> {
    let output = Command::new("chafa")
        .args([
            "-f",
            "symbols",
            "-s",
            &format!("{}x{}", cols, rows),
            &path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        use ansi_to_tui::IntoText as _;
        let text = output.stdout.into_text().map_err(|e| e.to_string())?;
        Ok(text.lines)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn wrap_escape_sequence(seq: &str) -> String {
    // Save cursor (\x1b7) and restore cursor (\x1b8) to prevent outer terminal cursor movement
    let wrapped_seq = format!("\x1b7{}\x1b8", seq);
    if std::env::var("TMUX").is_ok() {
        let escaped = wrapped_seq.replace("\x1b", "\x1b\x1b");
        format!("\x1bPtmux;{}\x1b\\", escaped)
    } else {
        wrapped_seq
    }
}

fn wrap_escape_sequence_sixel(seq: &str) -> String {
    let wrapped_seq = format!("\x1b7{}\x1b8", seq);
    if std::env::var("TMUX").is_ok() && !*TMUX_NATIVE_SIXEL {
        // If inside TMUX but TMUX doesn't natively support Sixel, we must wrap it in TMUX passthrough
        let escaped = wrapped_seq.replace("\x1b", "\x1b\x1b");
        format!("\x1bPtmux;{}\x1b\\", escaped)
    } else {
        wrapped_seq
    }
}

/// Builds an iTerm2 Inline Image protocol escape sequence.
pub fn build_iterm2_sequence(img_bytes: &[u8], cols: u16, rows: u16) -> String {
    let b64 = STANDARD.encode(img_bytes);
    let seq = format!(
        "\x1b]1337;File=inline=1;width={};height={};preserveAspectRatio=1:{}\x07",
        cols, rows, b64
    );
    wrap_escape_sequence(&seq)
}

pub fn build_iterm2_sequence_via_chafa(
    path: &Path,
    cols: u16,
    rows: u16,
) -> Result<String, String> {
    let output = Command::new("chafa")
        .args([
            "-f",
            "iterm",
            "-s",
            &format!("{}x{}", cols, rows),
            &path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let seq = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(wrap_escape_sequence(&seq))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

pub fn build_sixel_sequence_via_chafa(path: &Path, cols: u16, rows: u16) -> Result<String, String> {
    let output = Command::new("chafa")
        .args([
            "-f",
            "sixels",
            "-s",
            &format!("{}x{}", cols, rows),
            &path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let seq = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(wrap_escape_sequence_sixel(&seq))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

pub fn build_kitty_sequence_via_chafa(path: &Path, cols: u16, rows: u16) -> Result<String, String> {
    let output = Command::new("chafa")
        .args([
            "-f",
            "kitty",
            "-s",
            &format!("{}x{}", cols, rows),
            &path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let seq = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(wrap_escape_sequence(&seq))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// Builds a Kitty Graphics protocol escape sequence with chunking support (fallback/legacy).
pub fn build_kitty_sequence(img_bytes: &[u8], cols: u16, rows: u16) -> String {
    let b64 = STANDARD.encode(img_bytes);
    let mut seq = String::new();
    let chunk_size = 4096;
    let chars: Vec<char> = b64.chars().collect();
    let total_chars = chars.len();

    let mut offset = 0;
    while offset < total_chars {
        let end = (offset + chunk_size).min(total_chars);
        let chunk: String = chars[offset..end].iter().collect();
        let is_last = end == total_chars;

        if offset == 0 {
            seq.push_str(&format!(
                "\x1b_Ga=T,f=100,c={},r={},m={};{}\x1b\\",
                cols,
                rows,
                if is_last { 0 } else { 1 },
                chunk
            ));
        } else {
            seq.push_str(&format!(
                "\x1b_Gm={};{}\x1b\\",
                if is_last { 0 } else { 1 },
                chunk
            ));
        }
        offset = end;
    }
    wrap_escape_sequence(&seq)
}
