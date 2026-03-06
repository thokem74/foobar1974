import { Virtuoso } from 'react-virtuoso';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../state/store';

export function QueuePane() {
  const queue = useAppStore((s) => s.queue);

  return (
    <section className="pane queue-pane">
      <div className="pane-header">
        <h2>Queue</h2>
        <button onClick={() => invoke('clear_queue')}>Clear</button>
      </div>
      <Virtuoso
        style={{ height: '100%' }}
        totalCount={queue.length}
        itemContent={(index) => {
          const item = queue[index];
          return (
            <div className="row" key={item.queueId}>
              <span>{item.track.title}</span>
              <span>{item.track.artist}</span>
              <button onClick={() => invoke('remove_from_queue', { index })}>Remove</button>
            </div>
          );
        }}
      />
    </section>
  );
}
