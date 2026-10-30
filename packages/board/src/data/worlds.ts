/**
 * The worlds, from `GET /api/worlds`. The answer is checked field by field before the page uses
 * it: a row that does not hold together is left out and counted, never guessed at.
 */
import { WORLD_STATUSES, type WorldStatus, type WorldSummary } from '@mnemorph/protocol';
import { useEffect, useState } from 'react';

export interface Worlds {
  worlds: WorldSummary[];
  /** Rows the answer held that were not a world. */
  unreadable: number;
}

const str = (v: unknown): v is string => typeof v === 'string';

function row(v: unknown): WorldSummary | undefined {
  if (typeof v !== 'object' || v === null) return undefined;
  const r = v as Record<string, unknown>;
  const { id, name, world, remote, first, last, sessions, status } = r;
  if (!str(id) || id === '' || !str(name) || !str(world) || !str(first) || !str(last)) {
    return undefined;
  }
  if (typeof sessions !== 'number' || !Number.isInteger(sessions) || sessions < 0) {
    return undefined;
  }
  if (!WORLD_STATUSES.includes(status as WorldStatus)) return undefined;
  if (remote !== undefined && !str(remote)) return undefined;
  const out: WorldSummary = {
    id,
    name,
    world,
    first,
    last,
    sessions,
    status: status as WorldStatus,
  };
  if (remote !== undefined) out.remote = remote;
  return out;
}

/** The worlds an answer holds, sorted live first, then by latest activity. */
export function parseWorlds(body: unknown): Worlds {
  const list =
    typeof body === 'object' && body !== null ? (body as { worlds?: unknown }).worlds : undefined;
  if (!Array.isArray(list)) throw new Error('the answer holds no list of worlds');
  const worlds: WorldSummary[] = [];
  let unreadable = 0;
  for (const v of list) {
    const w = row(v);
    if (w) worlds.push(w);
    else unreadable += 1;
  }
  worlds.sort(
    (a, b) =>
      WORLD_STATUSES.indexOf(a.status) - WORLD_STATUSES.indexOf(b.status) ||
      Date.parse(b.last) - Date.parse(a.last) ||
      a.name.localeCompare(b.name),
  );
  return { worlds, unreadable };
}

export type Load =
  | { state: 'loading' }
  | { state: 'ready'; data: Worlds }
  | { state: 'failed'; reason: string };

/** The worlds, read once on mount. */
export function useWorlds(): Load {
  const [load, setLoad] = useState<Load>({ state: 'loading' });
  useEffect(() => {
    const ctl = new AbortController();
    fetch('/api/worlds', { signal: ctl.signal, headers: { accept: 'application/json' } })
      .then(async (res) => {
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
