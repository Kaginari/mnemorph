/**
 * Home: every world on this machine, the latest session first — a row each with its status, path,
 * activity, memory standing, open challenges, gate and spend, and "waiting for you" when one of the
 * board's chats keeps a gate ask.
 */
import type { BoardWorld } from '@mnemorph/protocol';
import { useState } from 'react';
import { type OverviewLoad, useOverview } from '../data/overview.ts';
import type { Load } from '../data/worlds.ts';
import { format } from '../router.ts';
import { Safe } from '../safe/Safe.tsx';
import { ago } from '../time.ts';
import { Empty } from '../ui/Empty.tsx';
import { Mark, STATUS, WORLD_STATUS } from '../ui/Mark.tsx';
import { GatePill, money, Unknown, When } from './facts.tsx';

/** The overview's line under a world: standing, challenges, gate, cost — or why it is not here. */
function Facts({ load, now }: { load: OverviewLoad; now: number }) {
  if (load.state === 'loading') return <span className="world-card-note">reading…</span>;
  const o = load.data;
  if (!o) {
    return (
      <span className="world-card-note">
        overview unavailable: <Safe text={load.state === 'failed' ? load.reason : ''} />
      </span>
    );
  }
  const m = o.memory.value;
  const c = o.challenges.value;
  const held = c ? c.pending + c.blocked + c.verifying : 0;
  return (
    <>
      {m ? (
        <>
          <span>grounded {m.grounded}</span>
          <span>shared {m.shared}</span>
          {m.unproven > 0 && (
            <Mark tone="gold" glyph="⚠">
              {m.unproven} unproven
            </Mark>
          )}
        </>
      ) : (
        <span>
          memory <Unknown why={o.memory.why} />
        </span>
      )}
      {c && held > 0 && (
        <Mark tone={c.blocked > 0 ? 'alarm' : 'gold'} glyph="◇">
          {held} {held === 1 ? 'challenge' : 'challenges'} open
        </Mark>
      )}
      {o.gate.value ? (
        <GatePill g={o.gate.value} />
      ) : (
        <span>
          gate <Unknown why={o.gate.why} />
        </span>
      )}
      {o.spend.value && <span>{money(o.spend.value)}</span>}
      {o.activity.value && (
        <span className="world-card-activity">
          <Safe text={o.activity.value.what} /> <When at={o.activity.value.at} now={now} />
        </span>
      )}
      {load.state === 'failed' && (
        <Mark tone="gold" glyph="⚠">
          stale: <Safe text={load.reason} />
        </Mark>
      )}
    </>
  );
}

/** "Waiting for you", when the board's chats in a world keep a gate ask. */
function Waiting({ load }: { load: OverviewLoad }) {
  const n = load.state === 'ready' ? (load.data.waiting.value ?? 0) : 0;
  if (n === 0) return null;
  return (
    <div className="world-card-waiting">
      <Mark tone="gold" glyph="◆">
        waiting for you: {n} gate {n === 1 ? 'ask' : 'asks'}
      </Mark>
    </div>
  );
}

function Status({ w, load }: { w: BoardWorld; load: OverviewLoad }) {
  const v = load.state === 'ready' ? load.data.status.value : null;
  const s = v
    ? WORLD_STATUS[v]
    : w.state === 'missing'
      ? WORLD_STATUS.missing
      : {
          glyph: '◌',
          tone: 'muted' as const,
          label: load.state === 'loading' ? 'reading' : 'unknown',
        };
  return (
    <Mark tone={s.tone} glyph={s.glyph}>
      <span className="world-card-name">
        <Safe text={w.name} />
      </span>
      <span className="visually-hidden"> — {s.label}</span>
    </Mark>
  );
}

function Card({ w, now }: { w: BoardWorld; now: number }) {
  const load = useOverview(w.id);
  const v = load.state === 'ready' ? load.data.status.value : null;
  const label = v ? WORLD_STATUS[v].label : STATUS[w.state].label;
  return (
    <li>
      <a className="world-card" href={format({ name: 'world', world: w.id })}>
        <div className="world-card-top">
          <Status w={w} load={load} />
          <span className="world-card-path mono">
            <Safe text={w.path} />
          </span>
          <span className="world-card-when">
            {label}
            {w.state !== 'missing' && ` · ${w.last === null ? 'never opened' : ago(w.last, now)}`}
          </span>
        </div>
        <div className="world-card-facts mono">
          <span>
            {w.sessions} {w.sessions === 1 ? 'session' : 'sessions'}
          </span>
          {w.remote && (
            <span>
              <Safe text={w.remote} />
            </span>
          )}
          <Facts load={load} now={now} />
        </div>
        <Waiting load={load} />
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
          A world joins the board when a session opens in it. To add one by hand, run{' '}
          <code className="mono">mnemorphd board add &lt;dir&gt;</code> in a terminal.
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
