import { create } from 'zustand';
import type { PlaybackState, QueueItem, RepeatMode, Track } from '../types';

interface AppState {
  query: string;
  tracks: Track[];
  queue: QueueItem[];
  nowPlaying?: Track;
  playback: PlaybackState;
  setQuery: (query: string) => void;
  setTracks: (tracks: Track[]) => void;
  setQueue: (queue: QueueItem[]) => void;
  setNowPlaying: (track?: Track) => void;
  setRepeatMode: (mode: RepeatMode) => void;
  toggleShuffle: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  query: '',
  tracks: [],
  queue: [],
  playback: {
    status: 'stopped',
    positionSec: 0,
    lengthSec: 0,
    volumePercent: 100,
    shuffle: false,
    repeatMode: 'off'
  },
  setQuery: (query) => set({ query }),
  setTracks: (tracks) => set({ tracks }),
  setQueue: (queue) => set({ queue }),
  setNowPlaying: (track) => set({ nowPlaying: track }),
  setRepeatMode: (repeatMode) => set((s) => ({ playback: { ...s.playback, repeatMode } })),
  toggleShuffle: () => set((s) => ({ playback: { ...s.playback, shuffle: !s.playback.shuffle } }))
}));
