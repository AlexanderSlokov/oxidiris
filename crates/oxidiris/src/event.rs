//! Input thread and the main loop.
//!
//! Implements the event half of OXD-021. See spec §4.2.
//!
//! Input is read on its own thread and delivered through a channel, so the main loop can block on
//! `recv_timeout(until_next_token)`. That keeps keystrokes responsive to the millisecond while the
//! loop is otherwise asleep, without the complexity of an async runtime.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event as CtEvent};
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::keymap;
use crate::scheduler::Deadline;
use crate::ui;

/// How long the input thread waits between polls.
const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Idle timeout while paused. Long enough that a paused reader costs no measurable CPU.
const IDLE_TIMEOUT: Duration = Duration::from_millis(250);

/// Something the main loop reacts to.
#[derive(Debug)]
pub enum AppEvent {
    /// A key was pressed.
    Key(crossterm::event::KeyEvent),
    /// The terminal was resized.
    Resize,
    /// The input thread failed.
    Error(String),
}

/// Start the input thread and return the receiving end of its channel.
pub fn spawn_input() -> Receiver<AppEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        loop {
            match event::poll(POLL_INTERVAL) {
                Ok(false) => continue,
                Ok(true) => match event::read() {
                    Ok(CtEvent::Key(key)) => {
                        if tx.send(AppEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                    Ok(CtEvent::Resize(_, _)) => {
                        if tx.send(AppEvent::Resize).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        let _ = tx.send(AppEvent::Error(e.to_string()));
                        break;
                    }
                },
                Err(e) => {
                    let _ = tx.send(AppEvent::Error(e.to_string()));
                    break;
                }
            }
        }
    });
    rx
}

/// Run the reader until the reader quits.
pub fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let rx = spawn_input();
    let mut deadline = Deadline::start(Instant::now());
    let mut dirty = true;
    let mut was_playing = false;

    loop {
        if dirty {
            terminal.draw(|frame| ui::render(frame, app))?;
            dirty = false;
        }
        if app.should_quit {
            return Ok(());
        }

        // Starting playback resets the schedule, so the first word is not cut short by whatever
        // time was left on the previous deadline.
        let playing = app.player.is_playing();
        if playing && !was_playing {
            deadline.resync(Instant::now());
            deadline.advance(
                Instant::now(),
                Duration::from_millis(u64::from(app.player.current_duration_ms())),
            );
        }
        was_playing = playing;

        let now = Instant::now();
        let timeout = if playing { deadline.timeout(now) } else { IDLE_TIMEOUT };

        match rx.recv_timeout(timeout) {
            Ok(AppEvent::Key(key)) => {
                if let Some(action) = keymap::resolve(key) {
                    app.handle(action);
                    dirty = true;
                }
            }
            Ok(AppEvent::Resize) => dirty = true,
            Ok(AppEvent::Error(msg)) => {
                app.message = Some(format!("input error: {msg}"));
                dirty = true;
            }
            Err(RecvTimeoutError::Timeout) => {
                if playing && deadline.is_due(Instant::now()) {
                    let step = Duration::from_millis(u64::from(app.player.current_duration_ms()));
                    if app.player.advance().is_none() {
                        app.player.pause();
                    }
                    deadline.advance(Instant::now(), step);
                    dirty = true;
                }
            }
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_paused_reader_waits_a_long_time_between_wakeups() {
        // A busy loop while paused would burn a laptop battery for nothing.
        assert!(IDLE_TIMEOUT >= Duration::from_millis(100));
    }

    #[test]
    fn the_input_poll_interval_keeps_keys_responsive() {
        // The channel delivers immediately; this only bounds how long a shutdown takes.
        assert!(POLL_INTERVAL <= Duration::from_millis(100));
    }
}
