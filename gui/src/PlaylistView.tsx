import type { TrackInfo } from "./types";

interface PlaylistViewProps {
  tracks: TrackInfo[];
  currentIndex: number | null;
  playlistName: string;
}

function PlaylistView({ tracks, currentIndex, playlistName }: PlaylistViewProps) {
  if (tracks.length === 0) {
    return (
      <div className="playlist-empty">
        <p>Playlist "{playlistName}" is empty</p>
      </div>
    );
  }

  return (
    <div className="playlist-view">
      <table className="track-table">
        <thead>
          <tr>
            <th className="col-num">#</th>
            <th className="col-status"></th>
            <th className="col-artist">Artist</th>
            <th className="col-title">Title</th>
            <th className="col-duration">Duration</th>
          </tr>
        </thead>
        <tbody>
          {tracks.map((track) => {
            const isCurrent = track.index === currentIndex;
            return (
              <tr
                key={track.index}
                className={isCurrent ? "track-row current" : "track-row"}
              >
                <td className="col-num">{track.index + 1}</td>
                <td className="col-status">
                  {isCurrent && <span className="playing-indicator">{"\u25B6"}</span>}
                  {track.has_intro && <span className="intro-dot" title="Has intro">{"\u2022"}</span>}
                </td>
                <td className="col-artist">{track.artist}</td>
                <td className="col-title">{track.title}</td>
                <td className="col-duration">{track.duration_display}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export default PlaylistView;
