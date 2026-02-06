use clap::{Parser, Subcommand};
use signal_flow::engine::Engine;
use signal_flow::player::{play_playlist, Player, SilenceConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "signalflow", about = "Radio Automation Engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show engine status
    Status,
    /// Playlist management
    Playlist {
        #[command(subcommand)]
        action: PlaylistCmd,
    },
    /// Play the active playlist
    Play {
        /// Track number to start from (1-based)
        #[arg(short, long)]
        track: Option<usize>,
        /// Crossfade duration in seconds (overrides config)
        #[arg(short = 'x', long)]
        crossfade: Option<f32>,
        /// Silence detection threshold (RMS level, overrides config)
        #[arg(long)]
        silence_threshold: Option<f32>,
        /// Silence duration in seconds before auto-skip (overrides config)
        #[arg(long)]
        silence_duration: Option<f32>,
    },
    /// Stop playback and clear the current track
    Stop,
    /// Skip to the next track in the active playlist
    Skip,
    /// Engine configuration
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
}

#[derive(Subcommand)]
enum PlaylistCmd {
    /// Create a new playlist
    Create { name: String },
    /// List all playlists
    List,
    /// Add track(s) to a playlist
    Add {
        /// Playlist name
        playlist: String,
        /// Audio file path(s)
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Show tracks in a playlist
    Show { name: String },
    /// Set a playlist as the active context
    Activate { name: String },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Set crossfade duration in seconds (0 = disabled)
    Crossfade {
        /// Duration in seconds
        seconds: f32,
    },
    /// Configure silence detection (auto-skip on dead air)
    Silence {
        #[command(subcommand)]
        action: SilenceCmd,
    },
    /// Show current configuration
    Show,
}

#[derive(Subcommand)]
enum SilenceCmd {
    /// Enable silence detection with threshold and duration
    Set {
        /// RMS threshold (e.g., 0.01 for ~-40dB)
        threshold: f32,
        /// Seconds of silence before auto-skip
        duration: f32,
    },
    /// Disable silence detection
    Off,
}

fn main() {
    let cli = Cli::parse();
    let mut engine = Engine::load();

    match cli.command {
        Commands::Status => {
            println!("signalFlow engine v{}", env!("CARGO_PKG_VERSION"));
            let silence_status = if engine.silence_duration_secs > 0.0 {
                format!(
                    "thresh={}, dur={}s",
                    engine.silence_threshold, engine.silence_duration_secs
                )
            } else {
                "off".to_string()
            };
            println!(
                "Playlists: {} | Active: {} | Crossfade: {}s | Silence: {}",
                engine.playlists.len(),
                engine
                    .active_playlist()
                    .map(|p| p.name.as_str())
                    .unwrap_or("none"),
                engine.crossfade_secs,
                silence_status
            );
            if let Some(pl) = engine.active_playlist() {
                if let Some(idx) = pl.current_index {
                    if let Some(t) = pl.tracks.get(idx) {
                        println!(
                            "Current track: [{}/{}] {} — {}",
                            idx + 1,
                            pl.track_count(),
                            t.artist,
                            t.title
                        );
                    }
                }
            }
        }
        Commands::Play {
            track,
            crossfade,
            silence_threshold,
            silence_duration,
        } => {
            let pl = match engine.active_playlist() {
                Some(p) => p,
                None => {
                    eprintln!("Error: no active playlist. Use 'playlist activate <name>' first.");
                    std::process::exit(1);
                }
            };

            if pl.tracks.is_empty() {
                eprintln!("Error: active playlist '{}' has no tracks.", pl.name);
                std::process::exit(1);
            }

            // Determine start index (--track is 1-based, internal is 0-based)
            let start = match track {
                Some(n) if n >= 1 && n <= pl.tracks.len() => n - 1,
                Some(n) => {
                    eprintln!(
                        "Error: track {} out of range (playlist has {} tracks)",
                        n,
                        pl.tracks.len()
                    );
                    std::process::exit(1);
                }
                None => pl.current_index.unwrap_or(0),
            };

            // Use --crossfade override, or fall back to engine config
            let crossfade_secs = crossfade.unwrap_or(engine.crossfade_secs);

            let playlist_name = pl.name.clone();
            let tracks = pl.tracks.clone();

            // Update current_index before playback starts
            if let Some(pl_mut) = engine.active_playlist_mut() {
                pl_mut.current_index = Some(start);
            }
            engine.save().expect("Failed to save state");

            // Initialize audio player
            let player = match Player::new() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            let silence_cfg = SilenceConfig {
                threshold: silence_threshold.unwrap_or(engine.silence_threshold),
                duration_secs: silence_duration.unwrap_or(engine.silence_duration_secs),
            };

            let mut info_parts = Vec::new();
            if crossfade_secs > 0.0 {
                info_parts.push(format!("crossfade: {}s", crossfade_secs));
            }
            if silence_cfg.enabled() {
                info_parts.push(format!(
                    "silence detect: thresh={}, dur={}s",
                    silence_cfg.threshold, silence_cfg.duration_secs
                ));
            }
            if info_parts.is_empty() {
                println!(
                    "Playing playlist '{}' from track {}...",
                    playlist_name,
                    start + 1
                );
            } else {
                println!(
                    "Playing playlist '{}' from track {} ({})...",
                    playlist_name,
                    start + 1,
                    info_parts.join(", ")
                );
            }
            let last_index = play_playlist(&player, &tracks, start, crossfade_secs, silence_cfg);

            // Update current_index after playback
            if let Some(pl_mut) = engine.active_playlist_mut() {
                pl_mut.current_index = Some(last_index);
            }
            engine.save().expect("Failed to save state");
        }
        Commands::Stop => {
            let pl = match engine.active_playlist_mut() {
                Some(p) => p,
                None => {
                    eprintln!("Error: no active playlist.");
                    std::process::exit(1);
                }
            };
            let name = pl.name.clone();
            pl.current_index = None;
            engine.save().expect("Failed to save state");
            println!("Stopped. Playlist '{}' reset to beginning.", name);
        }
        Commands::Skip => {
            let (next, name, artist, title, total) = {
                let pl = match engine.active_playlist() {
                    Some(p) => p,
                    None => {
                        eprintln!("Error: no active playlist.");
                        std::process::exit(1);
                    }
                };

                if pl.tracks.is_empty() {
                    eprintln!("Error: playlist '{}' has no tracks.", pl.name);
                    std::process::exit(1);
                }

                let next = match pl.current_index {
                    Some(i) if i + 1 < pl.tracks.len() => i + 1,
                    Some(_) => {
                        println!("Already at the last track.");
                        return;
                    }
                    None => 0,
                };

                let track = &pl.tracks[next];
                (
                    next,
                    pl.name.clone(),
                    track.artist.clone(),
                    track.title.clone(),
                    pl.track_count(),
                )
            };

            if let Some(pl_mut) = engine.active_playlist_mut() {
                pl_mut.current_index = Some(next);
            }
            engine.save().expect("Failed to save state");
            println!(
                "Skipped to [{}/{}] in '{}': {} — {}",
                next + 1,
                total,
                name,
                artist,
                title
            );
        }
        Commands::Config { action } => match action {
            ConfigCmd::Crossfade { seconds } => {
                if seconds < 0.0 {
                    eprintln!("Error: crossfade duration must be >= 0");
                    std::process::exit(1);
                }
                engine.crossfade_secs = seconds;
                engine.save().expect("Failed to save state");
                if seconds == 0.0 {
                    println!("Crossfade disabled.");
                } else {
                    println!("Crossfade set to {}s.", seconds);
                }
            }
            ConfigCmd::Silence { action } => match action {
                SilenceCmd::Set {
                    threshold,
                    duration,
                } => {
                    if threshold <= 0.0 {
                        eprintln!("Error: threshold must be > 0");
                        std::process::exit(1);
                    }
                    if duration <= 0.0 {
                        eprintln!("Error: duration must be > 0");
                        std::process::exit(1);
                    }
                    engine.silence_threshold = threshold;
                    engine.silence_duration_secs = duration;
                    engine.save().expect("Failed to save state");
                    println!(
                        "Silence detection enabled: threshold={}, duration={}s",
                        threshold, duration
                    );
                }
                SilenceCmd::Off => {
                    engine.silence_duration_secs = 0.0;
                    engine.save().expect("Failed to save state");
                    println!("Silence detection disabled.");
                }
            },
            ConfigCmd::Show => {
                println!("Crossfade: {}s", engine.crossfade_secs);
                if engine.silence_duration_secs > 0.0 {
                    println!(
                        "Silence detection: threshold={}, duration={}s",
                        engine.silence_threshold, engine.silence_duration_secs
                    );
                } else {
                    println!("Silence detection: off");
                }
            }
        },
        Commands::Playlist { action } => match action {
            PlaylistCmd::Create { name } => {
                if engine.find_playlist(&name).is_some() {
                    eprintln!("Error: playlist '{}' already exists", name);
                    std::process::exit(1);
                }
                let id = engine.create_playlist(name.clone());
                engine.save().expect("Failed to save state");
                println!("Created playlist '{}' (id: {})", name, id);
            }
            PlaylistCmd::List => {
                if engine.playlists.is_empty() {
                    println!("No playlists. Use 'playlist create <name>' to add one.");
                    return;
                }
                let active_id = engine.active_playlist_id;
                for pl in &engine.playlists {
                    let marker = if Some(pl.id) == active_id { " *" } else { "" };
                    println!(
                        "[{}] {} — {} tracks{}",
                        pl.id,
                        pl.name,
                        pl.track_count(),
                        marker
                    );
                }
            }
            PlaylistCmd::Add { playlist, files } => {
                let pl = match engine.find_playlist_mut(&playlist) {
                    Some(p) => p,
                    None => {
                        eprintln!("Error: playlist '{}' not found", playlist);
                        std::process::exit(1);
                    }
                };
                for file in &files {
                    match pl.add_track(file) {
                        Ok(idx) => {
                            let t = &pl.tracks[idx];
                            println!(
                                "  Added: {} — {} [{}]",
                                t.artist,
                                t.title,
                                t.duration_display()
                            );
                        }
                        Err(e) => eprintln!("  Skip: {} — {}", file.display(), e),
                    }
                }
                engine.save().expect("Failed to save state");
            }
            PlaylistCmd::Activate { name } => match engine.set_active(&name) {
                Ok(id) => {
                    engine.save().expect("Failed to save state");
                    println!("Active playlist: '{}' (id: {})", name, id);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            },
            PlaylistCmd::Show { name } => {
                let pl = match engine.find_playlist(&name) {
                    Some(p) => p,
                    None => {
                        eprintln!("Error: playlist '{}' not found", name);
                        std::process::exit(1);
                    }
                };
                println!("Playlist: {} ({} tracks)", pl.name, pl.track_count());
                println!("{:<4} {:<20} {:<30} {:>8}", "#", "Artist", "Title", "Dur");
                println!("{}", "-".repeat(66));
                for (i, t) in pl.tracks.iter().enumerate() {
                    let marker = if pl.current_index == Some(i) {
                        ">"
                    } else {
                        " "
                    };
                    println!(
                        "{}{:<3} {:<20} {:<30} {:>8}",
                        marker,
                        i + 1,
                        truncate(&t.artist, 19),
                        truncate(&t.title, 29),
                        t.duration_display()
                    );
                }
            }
        },
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max - 1])
    } else {
        s.to_string()
    }
}
