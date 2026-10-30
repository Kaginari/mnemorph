/** Home: every world on this machine, live first. */
import type { WorldSummary } from '@mnemorph/protocol';
import { useState } from 'react';
import type { Load } from '../data/worlds.ts';
import { format } from '../router.ts';
import { Safe } from '../safe/Safe.tsx';
import { ago } from '../time.ts';
import { Empty } from '../ui/Empty.tsx';
import { Mark, STATUS } from '../ui/Mark.tsx';

function Card({ w, now }: { w: WorldSummary; now: number }) {
  const s = STATUS[w.status];
  return (
    <li>
      <a className="world-card" href={format({ name: 'world', world: w.id })}>
        <div className="world-card-top">
          <Mark tone={s.tone} glyph={s.glyph}>
            <span className="world-card-name">
              <Safe text={w.name} />
            </span>
          </Mark>
          <span className="world-card-path mono">
            <Safe text={w.world} />
          </span>
          <span className="world-card-when">
            {s.label}
            {w.status !== 'missing' && ` · ${ago(w.last, now)}`}
          </span>
        </div>
        <div className="world-card-facts mono">
          <span>
            {w.sessions} {w.sessions === 1 ? 'session' : 'sessions'}
          </span>
          <span>since {w.first.slice(0, 10)}</span>
          {w.remote && (
            <span>
              <Safe text={w.remote} />
            </span>
          )}
        </div>
      </a>
    </li>
  );
}

export function WorldsView({ load, now }: { load: Load; now: number }) {
  const [adding, setAdding] = useState(false);
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">
          Worlds
          {load.state === 'ready' && <span className="page-count">{load.data.worlds.length}</span>}
        </h1>
        <button
          type="button"
          className="button"
          aria-expanded={adding}
          onClick={() => setAdding((a) => !a)}
        >
          + Add a world
        </button>
      </header>
      {adding && (
        <p className="hint">
          A world joins the board when a session opens in it: run{' '}
          <code className="mono">mnemorph</code> in its directory.
        </p>
      )}
      {load.state === 'loading' && (
        <Empty glyph="⣾" title="Reading worlds…">
          Asking the board server for the worlds on this machine.
        </Empty>
      )}
      {load.state === 'failed' && (
        <Empty glyph="✕" title="The worlds could not be read">
          <p>
            <Safe text={load.reason} />.
          </p>
          <p>Is the board server still running? Reload once it is.</p>
        </Empty>
      )}
      {load.state === 'ready' && load.data.worlds.length === 0 && (
        <Empty glyph="◈" title="No worlds yet">
          Open a session with <code className="mono">mnemorph</code> in a directory, and it appears
          here.
        </Empty>
      )}
      {load.state === 'ready' && load.data.worlds.length > 0 && (
        <ul className="world-list">
          {load.data.worlds.map((w) => (
            <Card key={w.id} w={w} now={now} />
          ))}
        </ul>
      )}
      {load.state === 'ready' && load.data.unreadable > 0 && (
        <p className="hint">
          <Mark tone="gold" glyph="⚠">
            {load.data.unreadable} {load.data.unreadable === 1 ? 'row' : 'rows'} of the registry
            could not be read and {load.data.unreadable === 1 ? 'is' : 'are'} not shown.
          </Mark>
        </p>
      )}
    </div>
  );
}
