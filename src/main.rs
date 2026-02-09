use clap::{Parser, Subcommand};
use signal_flow::ad_inserter::AdInserterService;
use signal_flow::ad_logger::AdPlayLogger;
use signal_flow::ad_scheduler::AdConfig;
use signal_flow::engine::Engine;
use signal_flow::now_playing::NowPlaying;
use signal_flow::player::{play_playlist, PlaybackResult, Player, RecurringIntroConfig, SilenceConfig};
use signal_flow::scheduler::{self, ConflictPolicy, Priority, ScheduleMode};
use std::path::{Path, PathBuf};

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
    /// Insert a file as the next track in the active playlist
    Insert {
        /// Audio file to queue as next track
        file: PathBuf,
    },
    /// Write now-playing info to an XML file
    NowPlaying {
        /// Output XML file path (overrides config if provided)
        file: Option<PathBuf>,
    },
    /// Track operations (metadata editing)
    Track {
        #[command(subcommand)]
        action: TrackCmd,
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
    /// Generate waveform peaks for an audio file
    Waveform {
        /// Audio file path
        file: PathBuf,
        /// Number of peaks to generate (default 200)
        #[arg(short, long, default_value = "200")]
        peaks: usize,
    },
    /// Ad management (scheduler/inserter system)
    Ad {
        #[command(subcommand)]
        action: AdCmd,
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
enum TrackCmd {
    /// Edit track metadata (artist, title) and persist to file tags
    Edit {
        /// Playlist name
        playlist: String,
        /// Track number (1-based)
        track: usize,
        /// New artist name
        #[arg(long)]
        artist: Option<String>,
        /// New title
        #[arg(long)]
        title: Option<String>,
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
    /// Configure now-playing XML export path
    NowPlaying {
        #[command(subcommand)]
        action: NowPlayingConfigCmd,
    },
    /// Set conflict resolution policy (schedule-wins or manual-wins)
    Conflict {
        /// Policy: "schedule-wins" (default) or "manual-wins"
        policy: String,
    },
    /// Configure ad inserter settings
    AdInserter {
        #[command(subcommand)]
        action: AdInserterConfigCmd,
    },
    /// Configure lecture detector (blacklist/whitelist)
    Lecture {
        #[command(subcommand)]
        action: LectureCmd,
    },
    /// Show current configuration
    Show,
}

#[derive(Subcommand)]
enum AdInserterConfigCmd {
    /// Set the output MP3 path for concatenated ad rolls
    Output {
        /// File path for concatenated ad roll
        path: String,
    },
    /// Configure station ID
    StationId {
        #[command(subcommand)]
        action: StationIdCmd,
    },
}

#[derive(Subcommand)]
enum StationIdCmd {
    /// Enable station ID with a file
    Set {
        /// Path to station ID audio file
        file: PathBuf,
    },
    /// Disable station ID
    Off,
}

#[derive(Subcommand)]
enum LectureCmd {
    /// Add an artist to the blacklist (never a lecture)
    BlacklistAdd {
        /// Artist name
        artist: String,
    },
    /// Remove an artist from the blacklist
    BlacklistRemove {
        /// Artist name
        artist: String,
    },
    /// Add an artist to the whitelist (always a lecture)
    WhitelistAdd {
        /// Artist name
        artist: String,
    },
    /// Remove an artist from the whitelist
    WhitelistRemove {
        /// Artist name
        artist: String,
    },
    /// Show current lecture detector lists
    Show,
    /// Test if an artist is classified as a lecture
    Test {
        /// Artist name to test
        artist: String,
    },
}

#[derive(Subcommand)]
enum NowPlayingConfigCmd {
    /// Set the default XML output path
    Set {
        /// File path for XML output
        path: String,
    },
    /// Disable now-playing XML export
    Off,
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
    /// Configure recurring intro overlay
    Recurring {
        #[command(subcommand)]
        action: RecurringIntroCmd,
    },
}

#[derive(Subcommand)]
enum RecurringIntroCmd {
    /// Enable recurring intro overlay with interval
    Set {
        /// Interval in seconds between recurring intros (e.g., 900 for 15 min)
        interval: f32,
        /// Duck volume level (0.0–1.0, default 0.3)
        #[arg(long, default_value = "0.3")]
        duck: f32,
    },
    /// Disable recurring intro overlay
    Off,
}

#[derive(Subcommand)]
enum AdCmd {
    /// Add a new ad
    Add {
        /// Ad name
        name: String,
        /// MP3 file path
        file: PathBuf,
        /// Enable scheduling (day/hour restrictions)
        #[arg(long)]
        scheduled: bool,
        /// Days of the week (e.g., "Monday,Wednesday,Friday")
        #[arg(long)]
        days: Option<String>,
        /// Hours to play (e.g., "9,10,14,15")
        #[arg(long)]
        hours: Option<String>,
    },
    /// List all ads
    List,
    /// Remove an ad by number (1-based)
    Remove {
        /// Ad number (1-based)
        num: usize,
    },
    /// Toggle an ad's enabled state
    Toggle {
        /// Ad number (1-based)
        num: usize,
    },
    /// Show details of an ad
    Show {
        /// Ad number (1-based)
        num: usize,
    },
    /// Manually trigger instant ad insertion (stops playback, plays ads)
    InsertInstant,
    /// Manually trigger scheduled ad insertion (queues ads as next tracks)
    InsertScheduled,
    /// Show ad play statistics
    Stats {
        /// Start date filter (MM-DD-YY)
        #[arg(long)]
        from: Option<String>,
        /// End date filter (MM-DD-YY)
        #[arg(long)]
        to: Option<String>,
    },
    /// Show recent ad insertion failures
    Failures,
    /// Clear all ad play statistics and failure records
    ResetStats,
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
            let ad_count = engine.ads.len();
            let enabled_ads = engine.ads.iter().filter(|a| a.enabled).count();
            println!(
                "Playlists: {} | Active: {} | Crossfade: {}s | Silence: {} | Intros: {} | Schedule: {} event(s) | Ads: {}/{} | Conflict: {}",
                engine.playlists.len(),
                engine
                    .active_playlist()
                    .map(|p| p.name.as_str())
                    .unwrap_or("none"),
                engine.crossfade_secs,
                silence_status,
                intros_status,
                sched_count,
                enabled_ads,
                ad_count,
                engine.conflict_policy
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
            let recurring_intro_cfg = RecurringIntroConfig {
                interval_secs: engine.recurring_intro_interval_secs,
                duck_volume: engine.recurring_intro_duck_volume,
            };
            if recurring_intro_cfg.enabled() {
                info_parts.push(format!(
                    "recurring intros: every {}s (duck: {:.0}%)",
                    recurring_intro_cfg.interval_secs,
                    recurring_intro_cfg.duck_volume * 100.0
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

            let PlaybackResult { last_index, played_durations } =
                play_playlist(&player, &tracks, start, crossfade_secs, silence_cfg, intros_path.as_deref(), recurring_intro_cfg);

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
        Commands::Insert { file } => {
            if !file.exists() {
                eprintln!("Error: file '{}' not found", file.display());
                std::process::exit(1);
            }
            match engine.insert_next_track(&file) {
                Ok(pos) => {
                    let pl = engine.active_playlist().unwrap();
                    let track = &pl.tracks[pos];
                    println!(
                        "Inserted as next track [{}] in '{}': {} — {}",
                        pos + 1,
                        pl.name,
                        track.artist,
                        track.title
                    );
                    engine.save().expect("Failed to save state");
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::NowPlaying { file } => {
            let output_path = file
                .or_else(|| engine.now_playing_path.as_ref().map(PathBuf::from));
            match output_path {
                Some(path) => {
                    let np = NowPlaying::from_engine(&engine, None);
                    match np.write_xml(&path) {
                        Ok(()) => println!("Now-playing XML written to: {}", path.display()),
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    eprintln!("Error: no output path. Provide a file argument or set one with 'config nowplaying set <path>'");
                    std::process::exit(1);
                }
            }
        }
        Commands::Track { action } => match action {
            TrackCmd::Edit {
                playlist,
                track,
                artist,
                title,
            } => {
                if track == 0 {
                    eprintln!("Error: track number must be >= 1");
                    std::process::exit(1);
                }
                if artist.is_none() && title.is_none() {
                    eprintln!("Error: provide --artist and/or --title to edit");
                    std::process::exit(1);
                }
                match engine.edit_track_metadata(
                    &playlist,
                    track - 1,
                    artist.as_deref(),
                    title.as_deref(),
                ) {
                    Ok(()) => {
                        let pl = engine.find_playlist(&playlist).unwrap();
                        let t = &pl.tracks[track - 1];
                        println!(
                            "Updated track {} in '{}': {} — {}",
                            track, playlist, t.artist, t.title
                        );
                        engine.save().expect("Failed to save state");
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
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
                IntrosCmd::Recurring { action } => match action {
                    RecurringIntroCmd::Set { interval, duck } => {
                        if interval <= 0.0 {
                            eprintln!("Error: interval must be > 0");
                            std::process::exit(1);
                        }
                        if !(0.0..=1.0).contains(&duck) {
                            eprintln!("Error: duck volume must be between 0.0 and 1.0");
                            std::process::exit(1);
                        }
                        engine.recurring_intro_interval_secs = interval;
                        engine.recurring_intro_duck_volume = duck;
                        engine.save().expect("Failed to save state");
                        println!(
                            "Recurring intro overlay: every {}s (duck volume: {:.0}%)",
                            interval,
                            duck * 100.0
                        );
                    }
                    RecurringIntroCmd::Off => {
                        engine.recurring_intro_interval_secs = 0.0;
                        engine.save().expect("Failed to save state");
                        println!("Recurring intro overlay disabled.");
                    }
                },
            },
            ConfigCmd::NowPlaying { action } => match action {
                NowPlayingConfigCmd::Set { path } => {
                    engine.now_playing_path = Some(path.clone());
                    engine.save().expect("Failed to save state");
                    println!("Now-playing XML path set to: {}", path);
                }
                NowPlayingConfigCmd::Off => {
                    engine.now_playing_path = None;
                    engine.save().expect("Failed to save state");
                    println!("Now-playing XML export disabled.");
                }
            },
            ConfigCmd::Conflict { policy } => {
                match ConflictPolicy::from_str_loose(&policy) {
                    Ok(p) => {
                        engine.conflict_policy = p;
                        engine.save().expect("Failed to save state");
                        println!("Conflict policy set to: {}", p);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            ConfigCmd::AdInserter { action } => match action {
                AdInserterConfigCmd::Output { path } => {
                    engine.ad_inserter.output_mp3 = path.clone().into();
                    engine.save().expect("Failed to save state");
                    println!("Ad inserter output path set to: {}", path);
                }
                AdInserterConfigCmd::StationId { action } => match action {
                    StationIdCmd::Set { file } => {
                        if !file.exists() {
                            eprintln!("Error: file '{}' not found", file.display());
                            std::process::exit(1);
                        }
                        engine.ad_inserter.station_id_enabled = true;
                        engine.ad_inserter.station_id_file = Some(file.clone());
                        engine.save().expect("Failed to save state");
                        println!("Station ID enabled: {}", file.display());
                    }
                    StationIdCmd::Off => {
                        engine.ad_inserter.station_id_enabled = false;
                        engine.save().expect("Failed to save state");
                        println!("Station ID disabled.");
                    }
                },
            },
            ConfigCmd::Lecture { action } => match action {
                LectureCmd::BlacklistAdd { artist } => {
                    engine.lecture_detector.add_blacklist(&artist);
                    engine.save().expect("Failed to save state");
                    println!("Added '{}' to lecture blacklist (never a lecture)", artist);
                }
                LectureCmd::BlacklistRemove { artist } => {
                    if engine.lecture_detector.remove_blacklist(&artist) {
                        engine.save().expect("Failed to save state");
                        println!("Removed '{}' from lecture blacklist", artist);
                    } else {
                        eprintln!("'{}' not found in blacklist", artist);
                        std::process::exit(1);
                    }
                }
                LectureCmd::WhitelistAdd { artist } => {
                    engine.lecture_detector.add_whitelist(&artist);
                    engine.save().expect("Failed to save state");
                    println!("Added '{}' to lecture whitelist (always a lecture)", artist);
                }
                LectureCmd::WhitelistRemove { artist } => {
                    if engine.lecture_detector.remove_whitelist(&artist) {
                        engine.save().expect("Failed to save state");
                        println!("Removed '{}' from lecture whitelist", artist);
                    } else {
                        eprintln!("'{}' not found in whitelist", artist);
                        std::process::exit(1);
                    }
                }
                LectureCmd::Show => {
                    let ld = &engine.lecture_detector;
                    println!("Lecture Detector:");
                    if ld.blacklist.is_empty() {
                        println!("  Blacklist: (empty)");
                    } else {
                        let mut items: Vec<_> = ld.blacklist.iter().collect();
                        items.sort();
                        println!("  Blacklist: {}", items.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                    }
                    if ld.whitelist.is_empty() {
                        println!("  Whitelist: (empty)");
                    } else {
                        let mut items: Vec<_> = ld.whitelist.iter().collect();
                        items.sort();
                        println!("  Whitelist: {}", items.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                    }
                }
                LectureCmd::Test { artist } => {
                    let is_lecture = engine.lecture_detector.is_lecture(&artist);
                    if is_lecture {
                        println!("'{}' -> LECTURE", artist);
                    } else {
                        println!("'{}' -> MUSIC (not a lecture)", artist);
                    }
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
                if engine.recurring_intro_interval_secs > 0.0 {
                    println!(
                        "Recurring intro: every {}s (duck: {:.0}%)",
                        engine.recurring_intro_interval_secs,
                        engine.recurring_intro_duck_volume * 100.0
                    );
                } else {
                    println!("Recurring intro: off");
                }
                println!("Conflict policy: {}", engine.conflict_policy);
                match &engine.now_playing_path {
                    Some(path) => println!("Now-playing XML: {}", path),
                    None => println!("Now-playing XML: off"),
                }
                println!("Ads configured: {}", engine.ads.len());
                println!("Ad output: {}", engine.ad_inserter.output_mp3.display());
                if engine.ad_inserter.station_id_enabled {
                    println!(
                        "Station ID: {}",
                        engine
                            .ad_inserter
                            .station_id_file
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "(not set)".into())
                    );
                } else {
                    println!("Station ID: off");
                }
                let bl_count = engine.lecture_detector.blacklist.len();
                let wl_count = engine.lecture_detector.whitelist.len();
                println!(
                    "Lecture detector: blacklist={}, whitelist={}",
                    bl_count, wl_count
                );
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
        Commands::Waveform { file, peaks } => {
            match signal_flow::waveform::generate_peaks(&file, peaks) {
                Ok(data) => {
                    println!("Waveform ({} peaks) for '{}':", data.len(), file.display());
                    // Print a simple ASCII visualization
                    let max_bar = 40;
                    for (i, &val) in data.iter().enumerate() {
                        let bar_len = (val * max_bar as f32) as usize;
                        let bar: String = "#".repeat(bar_len);
                        if i % 10 == 0 {
                            println!("{:>4} |{:<40} {:.2}", i, bar, val);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Ad { action } => match action {
            AdCmd::Add {
                name,
                file,
                scheduled,
                days,
                hours,
            } => {
                if !file.exists() {
                    eprintln!("Error: file '{}' not found", file.display());
                    std::process::exit(1);
                }
                let parsed_days: Vec<String> = match days {
                    Some(d) => d.split(',').map(|s| s.trim().to_string()).collect(),
                    None => vec![],
                };
                let parsed_hours: Vec<u8> = match hours {
                    Some(h) => {
                        let mut result = Vec::new();
                        for part in h.split(',') {
                            match part.trim().parse::<u8>() {
                                Ok(v) if v <= 23 => result.push(v),
                                _ => {
                                    eprintln!("Error: invalid hour '{}'. Use 0-23", part.trim());
                                    std::process::exit(1);
                                }
                            }
                        }
                        result
                    }
                    None => vec![],
                };
                let ad = AdConfig {
                    name: name.clone(),
                    enabled: true,
                    mp3_file: file.clone(),
                    scheduled,
                    days: parsed_days,
                    hours: parsed_hours,
                };
                let idx = engine.add_ad(ad);
                engine.save().expect("Failed to save state");
                println!(
                    "Added ad #{}: '{}' -> {}",
                    idx + 1,
                    name,
                    file.display()
                );
            }
            AdCmd::List => {
                if engine.ads.is_empty() {
                    println!("No ads configured. Use 'ad add' to create one.");
                    return;
                }
                println!(
                    "{:<4} {:<8} {:<20} {:<30} {:<8} {:<10} {}",
                    "#", "Status", "Name", "File", "Sched", "Days", "Hours"
                );
                println!("{}", "-".repeat(90));
                for (i, ad) in engine.ads.iter().enumerate() {
                    let status = if ad.enabled { "on" } else { "off" };
                    let sched = if ad.scheduled { "yes" } else { "no" };
                    println!(
                        "{:<4} {:<8} {:<20} {:<30} {:<8} {:<10} {}",
                        i + 1,
                        status,
                        truncate(&ad.name, 19),
                        truncate(&ad.mp3_file.display().to_string(), 29),
                        sched,
                        truncate(&ad.days_display(), 9),
                        ad.hours_display()
                    );
                }
            }
            AdCmd::Remove { num } => {
                if num == 0 {
                    eprintln!("Error: ad number must be >= 1");
                    std::process::exit(1);
                }
                match engine.remove_ad(num - 1) {
                    Ok(ad) => {
                        engine.save().expect("Failed to save state");
                        println!("Removed ad #{}: '{}'", num, ad.name);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            AdCmd::Toggle { num } => {
                if num == 0 {
                    eprintln!("Error: ad number must be >= 1");
                    std::process::exit(1);
                }
                match engine.toggle_ad(num - 1) {
                    Ok(enabled) => {
                        engine.save().expect("Failed to save state");
                        let status = if enabled { "enabled" } else { "disabled" };
                        println!("Ad #{} {}", num, status);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            AdCmd::Show { num } => {
                if num == 0 {
                    eprintln!("Error: ad number must be >= 1");
                    std::process::exit(1);
                }
                let idx = num - 1;
                match engine.ads.get(idx) {
                    Some(ad) => {
                        println!("Ad #{}: {}", num, ad.name);
                        println!("  Enabled:   {}", ad.enabled);
                        println!("  File:      {}", ad.mp3_file.display());
                        println!("  Scheduled: {}", ad.scheduled);
                        println!("  Days:      {}", ad.days_display());
                        println!("  Hours:     {}", ad.hours_display());
                    }
                    None => {
                        eprintln!(
                            "Error: ad #{} not found ({} ads total)",
                            num,
                            engine.ads.len()
                        );
                        std::process::exit(1);
                    }
                }
            }
            AdCmd::InsertInstant => {
                let player = match Player::new() {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                let logger = AdPlayLogger::new(Path::new("."));
                println!("Running instant ad insertion...");
                match AdInserterService::insert_instant(&player, &engine, false) {
                    Ok(result) => {
                        if result.station_id_played {
                            println!("  Station ID: played");
                        }
                        for name in &result.ads_inserted {
                            logger.log_play(name);
                            println!("  Played: {}", name);
                        }
                        println!("Instant insertion complete: {} ad(s) played.", result.ad_count);
                    }
                    Err(e) => {
                        let ad_names: Vec<String> = engine.ads.iter()
                            .filter(|a| a.enabled)
                            .map(|a| a.name.clone())
                            .collect();
                        logger.log_failure(&ad_names, &format!("instant:{}", e));
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            AdCmd::InsertScheduled => {
                let logger = AdPlayLogger::new(Path::new("."));
                println!("Running scheduled ad insertion...");
                match AdInserterService::insert_scheduled(&mut engine, false) {
                    Ok(result) => {
                        if result.station_id_played {
                            println!("  Station ID: queued");
                        }
                        for name in &result.ads_inserted {
                            logger.log_play(name);
                            println!("  Queued: {}", name);
                        }
                        println!(
                            "Scheduled insertion complete: {} ad(s) queued as next tracks.",
                            result.ad_count
                        );
                        engine.save().expect("Failed to save state");
                    }
                    Err(e) => {
                        let ad_names: Vec<String> = engine.ads.iter()
                            .filter(|a| a.enabled)
                            .map(|a| a.name.clone())
                            .collect();
                        logger.log_failure(&ad_names, &format!("scheduled:{}", e));
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            AdCmd::Stats { from, to } => {
                let logger = AdPlayLogger::new(Path::new("."));
                let stats = match (&from, &to) {
                    (Some(start), Some(end)) => logger.get_ad_statistics_filtered(start, end),
                    _ => logger.get_ad_statistics(),
                };

                if let (Some(start), Some(end)) = (&from, &to) {
                    println!("Ad Play Statistics ({} to {}):", start, end);
                } else {
                    println!("Ad Play Statistics (all time):");
                }
                println!("Total plays: {}", stats.total_plays);
                println!();

                if stats.per_ad.is_empty() {
                    println!("No ad plays recorded.");
                } else {
                    println!("{:<30} {:>8}", "Ad Name", "Plays");
                    println!("{}", "-".repeat(40));
                    for entry in &stats.per_ad {
                        println!("{:<30} {:>8}", truncate(&entry.name, 29), entry.play_count);
                    }
                }
            }
            AdCmd::Failures => {
                let logger = AdPlayLogger::new(Path::new("."));
                let failures = logger.get_failures();

                if failures.is_empty() {
                    println!("No ad insertion failures recorded.");
                } else {
                    println!("Recent Ad Insertion Failures ({}):", failures.len());
                    println!("{:<16} {:<30} {}", "Time", "Ads", "Error");
                    println!("{}", "-".repeat(70));
                    for f in failures.iter().rev() {
                        let ads_str = f.ads.join(", ");
                        println!("{:<16} {:<30} {}", f.t, truncate(&ads_str, 29), f.err);
                    }
                }
            }
            AdCmd::ResetStats => {
                let logger = AdPlayLogger::new(Path::new("."));
                logger.reset_all();
                println!("Ad play statistics and failure records cleared.");
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
