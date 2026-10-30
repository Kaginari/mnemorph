/**
 * The worlds, from `GET /api/worlds` (the board server's `BoardWorlds`, generated from Rust). The
 * answer is checked field by field before the page uses it: a row that does not hold together is
 * left out and counted, never guessed at.
 */
import type { BoardWorld, WorldState } from '@mnemorph/protocol';
import { useEffect, useState } from 'react';
import { establish } from './session.ts';

export interface Worlds {
  worlds: BoardWorld[];
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
  return { worlds, unreadable };
}

export type Load =
  | { state: 'loading' }
  | { state: 'ready'; data: Worlds }
  | { state: 'failed'; reason: string };

/** The worlds, read once on mount, after the session is established. */
export function useWorlds(): Load {
  const [load, setLoad] = useState<Load>({ state: 'loading' });
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
          throw new Error(
            'this browser holds no board session: open the URL `mnemorphd board` prints',
          );
        }
        if (!res.ok) throw new Error(`the board server answered ${res.status}`);
        setLoad({ state: 'ready', data: parseWorlds(await res.json()) });
      })
      .catch((e: unknown) => {
        if (ctl.signal.aborted) return;
        setLoad({ state: 'failed', reason: e instanceof Error ? e.message : String(e) });
      });
    return () => ctl.abort();
  }, []);
  return load;
}
