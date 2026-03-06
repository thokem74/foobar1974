export type RepeatMode = 'off' | 'all' | 'one';

export interface Track {
  id: number;
  title: string;
  artist: string;
  album: string;
  duration_ms?: number;
}

export interface QueueItem {
  queueId: string;
  track: Track;
}

export interface PlaybackState {
  status: 'playing' | 'paused' | 'stopped' | 'buffering';
  positionSec: number;
  lengthSec: number;
  volumePercent: number;
  shuffle: boolean;
  repeatMode: RepeatMode;
}
