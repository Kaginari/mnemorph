/**
 * The worlds, from `GET /api/worlds` (the board server's `BoardWorlds`, generated from Rust). The
 * answer is checked field by field before the page uses it: a row that does not hold together is
 * left out and counted, never guessed at.
 */
import type { BoardMissing, BoardWorld, WorldState } from '@mnemorph/protocol';
import { useCallback, useEffect, useState } from 'react';
import { type Change, usePush } from './push.ts';
import { establish, signedOut } from './session.ts';

export interface Worlds {
  /** The live worlds. */
  worlds: BoardWorld[];
  /** The worlds whose directory is gone, the longest missing first: shown apart. */
  missing: BoardMissing[];
  /** Rows the answer held that were not a world. */
  unreadable: number;
}

/** Every state a world can be in. */
export const WORLD_STATES: readonly WorldState[] = ['present', 'missing'];

const str = (v: unknown): v is string => typeof v === 'string';
const strOrNull = (v: unknown): v is string | null => v === null || typeof v === 'string';

function row(v: unknown): BoardWorld | undefined {
  if (typeof v !== 'object' || v === null) return undefined;
  const { id, name, path, remote, last, sessions, state } = v as Record<string, unknown>;
  if (!str(id) || id === '' || !str(name) || !str(path)) return undefined;
  if (!strOrNull(remote) || !strOrNull(last)) return undefined;
  if (last !== null && Number.isNaN(Date.parse(last))) return undefined;
  if (typeof sessions !== 'number' || !Number.isInteger(sessions) || sessions < 0) {
    return undefined;
  }
  if (!WORLD_STATES.includes(state as WorldState)) return undefined;
  return { id, name, path, remote, last, sessions, state: state as WorldState };
}

function gone(v: unknown): BoardMissing | undefined {
  if (typeof v !== 'object' || v === null) return undefined;
  const { id, name, path, since } = v as Record<string, unknown>;
  if (!str(id) || id === '' || !str(name) || !str(path) || !str(since)) return undefined;
  if (Number.isNaN(Date.parse(since))) return undefined;
  return { id, name, path, since };
}

/** When a world was last opened, for sorting: a world never opened sorts last. */
const when = (w: BoardWorld) => (w.last === null ? Number.NEGATIVE_INFINITY : Date.parse(w.last));

/** The worlds an answer holds, the latest session first, a world never opened last. */
export function parseWorlds(body: unknown): Worlds {
  const list =
    typeof body === 'object' && body !== null ? (body as { worlds?: unknown }).worlds : undefined;
  if (!Array.isArray(list)) throw new Error('the answer holds no list of worlds');
  const worlds: BoardWorld[] = [];
  let unreadable = 0;
  for (const v of list) {
    const w = row(v);
    if (w) worlds.push(w);
    else unreadable += 1;
  }
  worlds.sort((a, b) => when(b) - when(a) || a.name.localeCompare(b.name));
  const away = (body as { missing?: unknown }).missing;
  const missing: BoardMissing[] = [];
  for (const v of Array.isArray(away) ? away : []) {
    const m = gone(v);
    if (m) missing.push(m);
    else unreadable += 1;
  }
  return { worlds, missing, unreadable };
}

export type Load =
  | { state: 'loading' }
  | { state: 'ready'; data: Worlds }
  | { state: 'failed'; reason: string };

/**
 * The worlds, read on mount after the session is established, and again when the push channel
 * says the list moved. A reread that fails keeps the list shown.
 */
export function useWorlds(): Load {
  const [load, setLoad] = useState<Load>({ state: 'loading' });
  const [tick, setTick] = useState(0);
  const on = useCallback((c: Change) => {
    if (c.kind === 'worlds') setTick((t) => t + 1);
  }, []);
  usePush(on);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `tick` asks for a reread
  useEffect(() => {
    const ctl = new AbortController();
    establish()
      .then(() =>
        fetch('/api/worlds', {
          signal: ctl.signal,
          credentials: 'same-origin',
          headers: { accept: 'application/json' },
        }),
      )
      .then(async (res) => {
        if (res.status === 401) {
          signedOut();
          throw new Error(
            'this browser holds no board session: open the URL `mnemorphd board` prints',
          );
        }
        if (!res.ok) throw new Error(`the board server answered ${res.status}`);
        setLoad({ state: 'ready', data: parseWorlds(await res.json()) });
      })
      .catch((e: unknown) => {
        if (ctl.signal.aborted) return;
        const reason = e instanceof Error ? e.message : String(e);
        setLoad((was) => (was.state === 'ready' ? was : { state: 'failed', reason }));
      });
    return () => ctl.abort();
  }, [tick]);
  return load;
}
