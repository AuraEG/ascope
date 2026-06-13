// ==========================================================================
// File    : ui/event.rs
// Project : AuraScope
// Layer   : TUI
// Purpose : Threaded event handler that decouples raw input reading from the
//           regular tick/render loops to achieve zero-latency input response.
//
// Author  : Ahmed Ashour
// Created : 2026-06-14
// ==========================================================================

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, KeyEvent, MouseEvent};

/// Events dispatched to the main TUI event loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    /// Keyboard input event.
    Key(KeyEvent),
    /// Mouse interaction event.
    Mouse(MouseEvent),
    /// Terminal resize event containing (width, height).
    Resize(u16, u16),
    /// Regular clock tick to trigger UI updates and background polling.
    Tick,
}

/// Receives events from background worker threads and feeds them to the TUI event loop.
pub struct EventHandler {
    receiver: mpsc::Receiver<AppEvent>,
}

impl EventHandler {
    /// Spawn background input and tick threads, returning an event handler.
    pub fn new(tick_rate: Duration) -> Self {
        // Use a bounded channel to prevent event piling if rendering is delayed.
        let (sender, receiver) = mpsc::sync_channel(256);

        // 1. Spawns input thread that blocks on crossterm event reading.
        let s_input = sender.clone();
        thread::Builder::new()
            .name("ascope-input".to_string())
            .spawn(move || loop {
                match event::read() {
                    Ok(event::Event::Key(key)) => {
                        if s_input.send(AppEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                    Ok(event::Event::Mouse(mouse)) => {
                        if s_input.send(AppEvent::Mouse(mouse)).is_err() {
                            break;
                        }
                    }
                    Ok(event::Event::Resize(w, h)) => {
                        if s_input.send(AppEvent::Resize(w, h)).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        break;
                    }
                    _ => {}
                }
            })
            .expect("Failed to spawn input thread");

        // 2. Spawns tick thread to pace render updates.
        let s_tick = sender;
        thread::Builder::new()
            .name("ascope-tick".to_string())
            .spawn(move || loop {
                if s_tick.send(AppEvent::Tick).is_err() {
                    break;
                }
                thread::sleep(tick_rate);
            })
            .expect("Failed to spawn tick thread");

        Self { receiver }
    }

    /// Block and receive the next event from the queue.
    pub fn next(&self) -> Result<AppEvent, mpsc::RecvError> {
        self.receiver.recv()
    }
}

// --------------------------------------------------------------------------
// [SECTION] Tests
// --------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_handler_ticks() {
        let tick_rate = Duration::from_millis(10);
        let handler = EventHandler::new(tick_rate);

        // We should receive at least 3 tick events within a short window.
        let mut ticks = 0;
        let start = std::time::Instant::now();
        while ticks < 3 {
            if let Ok(AppEvent::Tick) = handler.next() {
                ticks += 1;
            }
            assert!(
                start.elapsed() < Duration::from_secs(1),
                "timed out waiting for ticks"
            );
        }
        assert_eq!(ticks, 3);
    }
}
