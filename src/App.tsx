import { LibraryPane } from './components/LibraryPane';
import { QueuePane } from './components/QueuePane';
import { NowPlayingBar } from './components/NowPlayingBar';

export default function App() {
  return (
    <main className="app-shell">
      <div className="workspace">
        <LibraryPane />
        <QueuePane />
      </div>
      <NowPlayingBar />
    </main>
  );
}
