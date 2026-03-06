import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../state/store';

export function NowPlayingBar() {
  const now = useAppStore((s) => s.nowPlaying);
  const playback = useAppStore((s) => s.playback);
  const setRepeatMode = useAppStore((s) => s.setRepeatMode);
  const toggleShuffle = useAppStore((s) => s.toggleShuffle);

  return (
    <footer className="now-playing">
      <div className="meta">
        <div className="album-art-placeholder" />
        <div>
          <strong>{now?.title ?? 'Nothing playing'}</strong>
          <p>{[now?.artist, now?.album].filter(Boolean).join(' • ')}</p>
        </div>
      </div>
      <div className="controls">
        <button onClick={() => invoke('previous')}>Prev</button>
        <button onClick={() => invoke('play_pause')}>{playback.status === 'playing' ? 'Pause' : 'Play'}</button>
        <button onClick={() => invoke('next')}>Next</button>
        <button onClick={() => invoke('stop')}>Stop</button>
      </div>
      <div className="sliders">
        <input type="range" min={0} max={Math.max(playback.lengthSec, 1)} value={playback.positionSec} readOnly />
        <input type="range" min={0} max={100} value={playback.volumePercent} onChange={(e) => invoke('set_volume', { percent: Number(e.target.value) })} />
      </div>
      <div className="modes">
        <button onClick={() => { toggleShuffle(); invoke('set_shuffle', { enabled: !playback.shuffle }); }}>
          Shuffle {playback.shuffle ? 'On' : 'Off'}
        </button>
        <select
          value={playback.repeatMode}
          onChange={(e) => {
            const mode = e.target.value as 'off' | 'all' | 'one';
            setRepeatMode(mode);
            invoke('set_repeat_mode', { mode });
          }}
        >
          <option value="off">Repeat Off</option>
          <option value="all">Repeat All</option>
          <option value="one">Repeat One</option>
        </select>
      </div>
    </footer>
  );
}
