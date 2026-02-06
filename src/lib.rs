//! signalFlow — Core library for the Radio Automation Engine.
//!
//! All audio, playlist, and scheduling logic lives here.
//! The CLI and future Tauri GUI consume this crate.

pub mod engine;
pub mod player;
pub mod playlist;
pub mod track;
