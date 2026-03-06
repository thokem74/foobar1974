import { useEffect } from 'react';
import { Virtuoso } from 'react-virtuoso';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../state/store';
import type { Track } from '../types';

const PAGE_SIZE = 250;

export function LibraryPane() {
  const { query, tracks, setQuery, setTracks } = useAppStore();

  useEffect(() => {
    const timeout = setTimeout(async () => {
      const result = await invoke<Track[]>('search_tracks', {
        query,
        sort: 'artist',
        dir: 'asc',
        offset: 0,
        limit: PAGE_SIZE
      }).catch(() => []);
      setTracks(result);
    }, 150);
    return () => clearTimeout(timeout);
  }, [query, setTracks]);

  const playNow = (trackId: number) => invoke('enqueue_and_play', { trackId });

  return (
    <section className="pane library-pane">
      <div className="pane-header">
        <h2>Library</h2>
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search title, artist, album"
          className="search"
        />
      </div>
      <Virtuoso
        style={{ height: '100%' }}
        totalCount={tracks.length}
        itemContent={(index) => {
          const track = tracks[index];
          return (
            <div
              className="row"
              key={track.id}
              onDoubleClick={() => playNow(track.id)}
              onKeyDown={(e) => e.key === 'Enter' && playNow(track.id)}
              tabIndex={0}
            >
              <span>{track.title || 'Unknown title'}</span>
              <span>{track.artist || 'Unknown artist'}</span>
              <span>{track.album || 'Unknown album'}</span>
            </div>
          );
        }}
      />
    </section>
  );
}
