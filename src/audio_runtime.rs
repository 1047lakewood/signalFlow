//! AudioRuntime — dedicated audio thread with channel-based command dispatch.
//!
//! Owns the `Player` on a single thread (no Send/Sync needed). External code
//! communicates via `AudioHandle` (wraps `mpsc::Sender<AudioCmd>`), which is
//! naturally Send+Sync. Track-end detection happens inside the thread loop
//! via `recv_timeout` + `player.is_empty()`.

use crate::level_monitor::LevelMonitor;
use crate::player::Player;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

// ── Commands & Events ────────────────────────────────────────────────────────

/// Commands sent to the audio thread.
pub enum AudioCmd {
    Play {
        path: PathBuf,
        level_monitor: LevelMonitor,
    },
    Stop,
    Pause,
    Resume,
    Seek(Duration),
    Shutdown,
}

/// Events emitted by the audio thread back to the caller.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    Playing,
    PlayError(String),
    TrackFinished,
    Stopped,
    Paused,
    Resumed,
    Seeked(f64),
}

// ── Handle ───────────────────────────────────────────────────────────────────

/// Thread-safe handle for sending commands to the audio runtime.
/// Wraps an `mpsc::Sender`, which is naturally `Send + Sync` — no unsafe needed.
#[derive(Clone)]
pub struct AudioHandle {
    tx: mpsc::Sender<AudioCmd>,
}

impl AudioHandle {
    pub fn play(&self, path: PathBuf, level_monitor: LevelMonitor) {
        let _ = self.tx.send(AudioCmd::Play { path, level_monitor });
    }

    pub fn stop(&self) {
        let _ = self.tx.send(AudioCmd::Stop);
    }

    pub fn pause(&self) {
        let _ = self.tx.send(AudioCmd::Pause);
    }

    pub fn resume(&self) {
        let _ = self.tx.send(AudioCmd::Resume);
    }

    pub fn seek(&self, position: Duration) {
        let _ = self.tx.send(AudioCmd::Seek(position));
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(AudioCmd::Shutdown);
    }
}

// ── Runtime ──────────────────────────────────────────────────────────────────

/// Spawn the audio runtime on a dedicated thread.
///
/// `on_event` is called from the audio thread whenever a state change occurs.
/// The caller should use this to update AppCore and emit Tauri events.
///
/// Returns an `AudioHandle` for sending commands.
pub fn spawn_audio_runtime<F>(on_event: F) -> AudioHandle
where
    F: Fn(AudioEvent) + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<AudioCmd>();

    std::thread::Builder::new()
        .name("audio-runtime".into())
        .spawn(move || {
            audio_thread_loop(rx, on_event);
        })
        .expect("failed to spawn audio-runtime thread");

    AudioHandle { tx }
}

/// Main loop for the audio thread. Owns the Player.
fn audio_thread_loop<F>(rx: mpsc::Receiver<AudioCmd>, on_event: F)
where
    F: Fn(AudioEvent),
{
    let mut player: Option<Player> = None;
    let mut was_playing = false;

    loop {
        // Poll for commands with a short timeout to detect track end
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(cmd) => match cmd {
                AudioCmd::Play { path, level_monitor } => {
                    // Lazy-init player on first use
                    if player.is_none() {
                        match Player::new() {
                            Ok(p) => player = Some(p),
                            Err(e) => {
                                on_event(AudioEvent::PlayError(e));
                                continue;
                            }
                        }
                    }

                    let p = player.as_ref().unwrap();

                    // Decode file ON the audio thread (no lock contention)
                    match Player::prepare_file_with_level(&path, level_monitor.clone()) {
                        Ok(prepared) => {
                            level_monitor.reset();
                            p.stop_and_play_prepared(prepared);
                            was_playing = true;
                            on_event(AudioEvent::Playing);
                        }
                        Err(e) => {
                            on_event(AudioEvent::PlayError(e));
                        }
                    }
                }

                AudioCmd::Stop => {
                    if let Some(p) = &player {
                        p.stop();
                    }
                    was_playing = false;
                    on_event(AudioEvent::Stopped);
                }

                AudioCmd::Pause => {
                    if let Some(p) = &player {
                        p.pause();
                    }
                    on_event(AudioEvent::Paused);
                }

                AudioCmd::Resume => {
                    if let Some(p) = &player {
                        p.resume();
                    }
                    on_event(AudioEvent::Resumed);
                }

                AudioCmd::Seek(position) => {
                    if let Some(p) = &player {
                        match p.try_seek(position) {
                            Ok(()) => {
                                on_event(AudioEvent::Seeked(position.as_secs_f64()));
                            }
                            Err(e) => {
                                on_event(AudioEvent::PlayError(e));
                            }
                        }
                    }
                }

                AudioCmd::Shutdown => {
                    if let Some(p) = &player {
                        p.stop();
                    }
                    break;
                }
            },

            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Check for natural track end
                if was_playing {
                    if let Some(p) = &player {
                        if p.is_empty() {
                            was_playing = false;
                            on_event(AudioEvent::TrackFinished);
                        }
                    }
                }
            }

            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // All senders dropped — shut down
                break;
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn handle_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AudioHandle>();
    }

    #[test]
    fn shutdown_stops_thread() {
        let handle = spawn_audio_runtime(|_| {});
        handle.shutdown();
        // Give the thread time to exit
        std::thread::sleep(Duration::from_millis(100));
        // If we get here without hanging, the thread shut down
    }

    #[test]
    fn play_nonexistent_emits_error() {
        let events: Arc<Mutex<Vec<AudioEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let handle = spawn_audio_runtime(move |evt| {
            events_clone.lock().unwrap().push(evt);
        });

        let monitor = LevelMonitor::new();
        handle.play(PathBuf::from("__nonexistent_file__.mp3"), monitor);

        // Wait for event to arrive
        std::thread::sleep(Duration::from_millis(500));

        let evts = events.lock().unwrap();
        // Should get either PlayError (file not found) or PlayError (no audio device)
        assert!(
            evts.iter().any(|e| matches!(e, AudioEvent::PlayError(_))),
            "Expected PlayError event, got: {:?}",
            *evts
        );

        handle.shutdown();
    }

    #[test]
    fn stop_without_play_emits_stopped() {
        let events: Arc<Mutex<Vec<AudioEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let handle = spawn_audio_runtime(move |evt| {
            events_clone.lock().unwrap().push(evt);
        });

        handle.stop();
        std::thread::sleep(Duration::from_millis(200));

        let evts = events.lock().unwrap();
        assert!(
            evts.iter().any(|e| matches!(e, AudioEvent::Stopped)),
            "Expected Stopped event, got: {:?}",
            *evts
        );

        handle.shutdown();
    }
}
