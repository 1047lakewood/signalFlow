//! signalFlow — Core library for the Radio Automation Engine.
//!
//! All audio, playlist, and scheduling logic lives here.
//! The CLI and future Tauri GUI consume this crate.

pub mod auto_intro;
pub mod engine;
pub mod now_playing;
pub mod player;
pub mod playlist;
pub mod scheduler;
pub mod silence;
pub mod track;
