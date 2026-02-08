use clap::{Parser, Subcommand};
use signal_flow::engine::Engine;
use signal_flow::player::{play_playlist, PlaybackResult, Player, SilenceConfig};
use signal_flow::scheduler::{self, Priority, ScheduleMode};
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
    /// Play a sound on top of current audio (overlay)
    Overlay {
        /// Audio file to play as overlay
        file: PathBuf,
    },
    /// Stop current audio and play a file (hard break)
    Interrupt {
        /// Audio file to play after stopping current audio
        file: PathBuf,
    },
    /// Engine configuration
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Schedule management (timed events)
    Schedule {
        #[command(subcommand)]
        action: ScheduleCmd,
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
        /// Insert at position (1-based) instead of appending
        #[arg(long)]
        at: Option<usize>,
    },
    /// Show tracks in a playlist
    Show { name: String },
    /// Set a playlist as the active context
    Activate { name: String },
    /// Remove track(s) from a playlist by position (1-based)
    Remove {
        /// Playlist name
        name: String,
        /// Track numbers to remove (1-based)
        #[arg(required = true)]
        tracks: Vec<usize>,
    },
    /// Move a track from one position to another (1-based)
    Move {
        /// Playlist name
        name: String,
        /// Current position (1-based)
        from: usize,
        /// New position (1-based)
        to: usize,
    },
    /// Copy tracks from one playlist to another
    Copy {
        /// Source playlist name
        source: String,
        /// Destination playlist name
        dest: String,
        /// Track numbers to copy (1-based)
        #[arg(required = true)]
        tracks: Vec<usize>,
        /// Insert at position in destination (1-based)
        #[arg(long)]
        at: Option<usize>,
    },
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
    /// Configure auto-intro system
    Intros {
        #[command(subcommand)]
        action: IntrosCmd,
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

#[derive(Subcommand)]
enum IntrosCmd {
    /// Set the intros folder path
    Set {
        /// Path to folder containing artist intro files
        path: String,
    },
    /// Disable auto-intros
    Off,
}

#[derive(Subcommand)]
enum ScheduleCmd {
    /// Add a scheduled event
    Add {
        /// Time of day (HH:MM or HH:MM:SS)
        time: String,
        /// Mode: overlay, stop, or insert
        mode: String,
        /// Audio file path
        file: PathBuf,
        /// Priority level 1-9 (default: 5)
        #[arg(short, long, default_value = "5")]
        priority: u8,
        /// Optional label/description
        #[arg(short, long)]
        label: Option<String>,
        /// Days of week (0=Mon..6=Sun), comma-separated. Omit for daily.
        #[arg(short, long)]
        days: Option<String>,
    },
    /// List all scheduled events
    List,
    /// Remove a scheduled event by ID
    Remove {
        /// Event ID to remove
        id: u32,
    },
    /// Enable or disable a scheduled event
    Toggle {
        /// Event ID to toggle
        id: u32,
    },
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
            let intros_status = engine
                .intros_folder
                .as_deref()
                .unwrap_or("off");
            let sched_count = engine.schedule.len();
            println!(
                "Playlists: {} | Active: {} | Crossfade: {}s | Silence: {} | Intros: {} | Schedule: {} event(s)",
                engine.playlists.len(),
                engine
                    .active_playlist()
                    .map(|p| p.name.as_str())
                    .unwrap_or("none"),
                engine.crossfade_secs,
                silence_status,
                intros_status,
                sched_count
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

            let intros_path = engine.intros_folder.as_ref().map(std::path::PathBuf::from);

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
            if intros_path.is_some() {
                info_parts.push("auto-intros: on".to_string());
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
            let PlaybackResult { last_index, played_durations } =
                play_playlist(&player, &tracks, start, crossfade_secs, silence_cfg, intros_path.as_deref());

            // Update current_index and played durations after playback
            if let Some(pl_mut) = engine.active_playlist_mut() {
                pl_mut.current_index = Some(last_index);
                for (idx, played) in &played_durations {
                    if let Some(track) = pl_mut.tracks.get_mut(*idx) {
                        track.played_duration = Some(*played);
                    }
                }
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
        Commands::Overlay { file } => {
            if !file.exists() {
                eprintln!("Error: file '{}' not found", file.display());
                std::process::exit(1);
            }
            let player = match Player::new() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            println!("Overlay: {}", file.display());
            match player.play_overlay(&file) {
                Ok(()) => println!("Overlay finished."),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Interrupt { file } => {
            if !file.exists() {
                eprintln!("Error: file '{}' not found", file.display());
                std::process::exit(1);
            }
            let player = match Player::new() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            println!("Interrupt: stopping current audio, playing {}", file.display());
            match player.play_stop_mode(&file) {
                Ok(()) => println!("Interrupt finished."),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
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
            ConfigCmd::Intros { action } => match action {
                IntrosCmd::Set { path } => {
                    let p = std::path::Path::new(&path);
                    if !p.is_dir() {
                        eprintln!("Error: '{}' is not a valid directory", path);
                        std::process::exit(1);
                    }
                    engine.intros_folder = Some(path.clone());
                    engine.save().expect("Failed to save state");
                    println!("Intros folder set to: {}", path);
                }
                IntrosCmd::Off => {
                    engine.intros_folder = None;
                    engine.save().expect("Failed to save state");
                    println!("Auto-intros disabled.");
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
                match &engine.intros_folder {
                    Some(folder) => println!("Intros folder: {}", folder),
                    None => println!("Auto-intros: off"),
                }
            }
        },
        Commands::Schedule { action } => match action {
            ScheduleCmd::Add {
                time,
                mode,
                file,
                priority,
                label,
                days,
            } => {
                let parsed_time = match scheduler::parse_time(&time) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                let parsed_mode = match ScheduleMode::from_str_loose(&mode) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                if priority == 0 || priority > 9 {
                    eprintln!("Error: priority must be 1-9");
                    std::process::exit(1);
                }
                let parsed_days: Vec<u8> = match days {
                    Some(d) => {
                        let mut result = Vec::new();
                        for part in d.split(',') {
                            match part.trim().parse::<u8>() {
                                Ok(v) if v <= 6 => result.push(v),
                                _ => {
                                    eprintln!("Error: invalid day '{}'. Use 0=Mon..6=Sun", part.trim());
                                    std::process::exit(1);
                                }
                            }
                        }
                        result.sort_unstable();
                        result.dedup();
                        result
                    }
                    None => vec![],
                };
                let id = engine.schedule.add_event(
                    parsed_time,
                    parsed_mode,
                    file.clone(),
                    Priority(priority),
                    label.clone(),
                    parsed_days,
                );
                engine.save().expect("Failed to save state");
                println!(
                    "Added schedule event #{}: {} {} {} (priority {})",
                    id,
                    time,
                    parsed_mode,
                    file.display(),
                    priority
                );
                if let Some(lbl) = &label {
                    println!("  Label: {}", lbl);
                }
            }
            ScheduleCmd::List => {
                if engine.schedule.is_empty() {
                    println!("No scheduled events. Use 'schedule add' to create one.");
                    return;
                }
                println!(
                    "{:<4} {:<10} {:<8} {:<30} {:<4} {:<8} {}",
                    "ID", "Time", "Mode", "File", "Pri", "Status", "Days"
                );
                println!("{}", "-".repeat(80));
                for event in engine.schedule.events_by_time() {
                    let status = if event.enabled { "on" } else { "off" };
                    let label_suffix = event
                        .label
                        .as_ref()
                        .map(|l| format!(" ({})", l))
                        .unwrap_or_default();
                    println!(
                        "{:<4} {:<10} {:<8} {:<30} {:<4} {:<8} {}{}",
                        event.id,
                        event.time_display(),
                        event.mode,
                        truncate(&event.file.display().to_string(), 29),
                        event.priority,
                        status,
                        event.days_display(),
                        label_suffix
                    );
                }
            }
            ScheduleCmd::Remove { id } => {
                match engine.schedule.remove_event(id) {
                    Ok(event) => {
                        engine.save().expect("Failed to save state");
                        println!(
                            "Removed schedule event #{}: {} {} {}",
                            id,
                            event.time_display(),
                            event.mode,
                            event.file.display()
                        );
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            ScheduleCmd::Toggle { id } => {
                match engine.schedule.toggle_event(id) {
                    Ok(enabled) => {
                        engine.save().expect("Failed to save state");
                        let status = if enabled { "enabled" } else { "disabled" };
                        println!("Schedule event #{} {}", id, status);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
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
            PlaylistCmd::Add { playlist, files, at } => {
                // Convert 1-based --at to 0-based
                let insert_at = at.map(|n| {
                    if n == 0 {
                        eprintln!("Error: --at position must be >= 1");
                        std::process::exit(1);
                    }
                    n - 1
                });

                let pl = match engine.find_playlist_mut(&playlist) {
                    Some(p) => p,
                    None => {
                        eprintln!("Error: playlist '{}' not found", playlist);
                        std::process::exit(1);
                    }
                };

                // Parse all tracks first, then insert at position or append
                let mut parsed_tracks = Vec::new();
                for file in &files {
                    match signal_flow::track::Track::from_path(file) {
                        Ok(t) => {
                            println!(
                                "  Added: {} — {} [{}]",
                                t.artist, t.title, t.duration_display()
                            );
                            parsed_tracks.push(t);
                        }
                        Err(e) => eprintln!("  Skip: {} — {}", file.display(), e),
                    }
                }
                if !parsed_tracks.is_empty() {
                    if let Err(e) = pl.insert_tracks(parsed_tracks, insert_at) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
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
            PlaylistCmd::Remove { name, tracks } => {
                let pl = match engine.find_playlist_mut(&name) {
                    Some(p) => p,
                    None => {
                        eprintln!("Error: playlist '{}' not found", name);
                        std::process::exit(1);
                    }
                };
                // Sort indices descending so removals don't shift later indices
                let mut indices: Vec<usize> = tracks
                    .iter()
                    .map(|&n| {
                        if n == 0 {
                            eprintln!("Error: track numbers are 1-based");
                            std::process::exit(1);
                        }
                        n - 1
                    })
                    .collect();
                indices.sort_unstable();
                indices.dedup();
                indices.reverse();

                for idx in indices {
                    match pl.remove_track(idx) {
                        Ok(t) => println!("  Removed: {} — {}", t.artist, t.title),
                        Err(e) => eprintln!("  Error: {}", e),
                    }
                }
                engine.save().expect("Failed to save state");
            }
            PlaylistCmd::Move { name, from, to } => {
                if from == 0 || to == 0 {
                    eprintln!("Error: positions are 1-based");
                    std::process::exit(1);
                }
                let pl = match engine.find_playlist_mut(&name) {
                    Some(p) => p,
                    None => {
                        eprintln!("Error: playlist '{}' not found", name);
                        std::process::exit(1);
                    }
                };
                match pl.reorder(from - 1, to - 1) {
                    Ok(()) => {
                        println!("Moved track {} → {} in '{}'", from, to, name);
                        engine.save().expect("Failed to save state");
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            PlaylistCmd::Copy {
                source,
                dest,
                tracks,
                at,
            } => {
                let indices: Vec<usize> = tracks
                    .iter()
                    .map(|&n| {
                        if n == 0 {
                            eprintln!("Error: track numbers are 1-based");
                            std::process::exit(1);
                        }
                        n - 1
                    })
                    .collect();

                let copied = match engine.copy_tracks(&source, &indices) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };

                let count = copied.len();
                let insert_at = at.map(|n| {
                    if n == 0 {
                        eprintln!("Error: --at position must be >= 1");
                        std::process::exit(1);
                    }
                    n - 1
                });

                match engine.paste_tracks(&dest, copied, insert_at) {
                    Ok(()) => {
                        println!(
                            "Copied {} track(s) from '{}' to '{}'",
                            count, source, dest
                        );
                        engine.save().expect("Failed to save state");
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            PlaylistCmd::Show { name } => {
                let pl = match engine.find_playlist(&name) {
                    Some(p) => p,
                    None => {
                        eprintln!("Error: playlist '{}' not found", name);
                        std::process::exit(1);
                    }
                };
                let has_played = pl.tracks.iter().any(|t| t.played_duration.is_some());
                println!("Playlist: {} ({} tracks)", pl.name, pl.track_count());
                if has_played {
                    println!("{:<4} {:<20} {:<30} {:>8} {:>8}", "#", "Artist", "Title", "Dur", "Played");
                    println!("{}", "-".repeat(75));
                } else {
                    println!("{:<4} {:<20} {:<30} {:>8}", "#", "Artist", "Title", "Dur");
                    println!("{}", "-".repeat(66));
                }
                for (i, t) in pl.tracks.iter().enumerate() {
                    let marker = if pl.current_index == Some(i) {
                        ">"
                    } else {
                        " "
                    };
                    if has_played {
                        let played = t
                            .played_duration_display()
                            .unwrap_or_else(|| "-".to_string());
                        println!(
                            "{}{:<3} {:<20} {:<30} {:>8} {:>8}",
                            marker,
                            i + 1,
                            truncate(&t.artist, 19),
                            truncate(&t.title, 29),
                            t.duration_display(),
                            played
                        );
                    } else {
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
