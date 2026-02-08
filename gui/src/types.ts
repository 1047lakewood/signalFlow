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
  conflict_policy: string;
  now_playing_path: string | null;
}
