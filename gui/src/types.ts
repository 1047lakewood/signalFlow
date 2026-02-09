export interface PlaylistInfo {
  id: number;
  name: string;
  track_count: number;
  is_active: boolean;
  current_index: number | null;
}

export interface TrackInfo {
  index: number;
  path: string;
  title: string;
  artist: string;
  duration_secs: number;
  duration_display: string;
  played_duration_secs: number | null;
  has_intro: boolean;
}

export interface StatusResponse {
  playlist_count: number;
  active_playlist: string | null;
  schedule_event_count: number;
  crossfade_secs: number;
  conflict_policy: string;
  silence_threshold: number;
  silence_duration_secs: number;
  intros_folder: string | null;
  now_playing_path: string | null;
}

export interface ScheduleEventInfo {
  id: number;
  time: string;
  mode: string;
  file: string;
  priority: number;
  enabled: boolean;
  label: string | null;
  days: string;
}

export interface ConfigResponse {
  crossfade_secs: number;
  silence_threshold: number;
  silence_duration_secs: number;
  intros_folder: string | null;
  recurring_intro_interval_secs: number;
  recurring_intro_duck_volume: number;
  conflict_policy: string;
  now_playing_path: string | null;
}

export interface AdInfo {
  index: number;
  name: string;
  enabled: boolean;
  mp3_file: string;
  scheduled: boolean;
  days: string[];
  hours: number[];
}

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
}

export interface AdStatsResponse {
  total_plays: number;
  per_ad: AdStatEntry[];
}

export interface AdStatEntry {
  name: string;
  play_count: number;
}

export interface AdDailyCount {
  date: string;
  count: number;
}

export interface AdFailure {
  timestamp: string;
  ads: string[];
  error: string;
}

export interface RdsMessageInfo {
  index: number;
  text: string;
  enabled: boolean;
  duration: number;
  scheduled: boolean;
  days: string[];
  hours: number[];
}

export interface RdsConfigResponse {
  ip: string;
  port: number;
  default_message: string;
  messages: RdsMessageInfo[];
}

export interface TransportState {
  is_playing: boolean;
  is_paused: boolean;
  elapsed_secs: number;
  duration_secs: number;
  track_index: number | null;
  track_artist: string | null;
  track_title: string | null;
  next_artist: string | null;
  next_title: string | null;
  track_path: string | null;
}
