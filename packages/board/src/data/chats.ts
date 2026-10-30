/**
 * A world's chats: the live ones the board's bridge runs (`GET /api/sessions`) and the past ones
 * its run journals keep (`GET /api/w/<id>/runs`), each answer checked field by field; a new chat
 * or a resumed run started by `POST /api/w/<id>/sessions`; a past run's transcript read whole.
 */
import type {
  Arrived,
  BoardRun,
  BoardSession,
  BoardStarted,
  BoardTranscript,
  RunState,
} from '@mnemorph/protocol';
import { eventType } from '@mnemorph/protocol';
import { useCallback, useEffect, useState } from 'react';
import { establish, post } from './session.ts';

const str = (v: unknown): v is string => typeof v === 'string';
const nat = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v) && v >= 0;

export const RUN_STATES: readonly RunState[] = ['live', 'stopped', 'ended', 'unknown'];

function session(v: unknown): BoardSession | undefined {
  if (typeof v !== 'object' || v === null) return undefined;
  const { id, world, session, run, controlled, tabs, asks } = v as Record<string, unknown>;
  if (!str(id) || id === '' || !str(world) || !str(session) || !str(run)) return undefined;
  if (typeof controlled !== 'boolean' || !nat(tabs) || !nat(asks)) return undefined;
  return { id, world, session, run, controlled, tabs, asks };
}

function run(v: unknown): BoardRun | undefined {
  if (typeof v !== 'object' || v === null) return undefined;
  const { id, when, state, why, ask, turns } = v as Record<string, unknown>;
  if (!str(id) || !/^r\d+-\d+$/.test(id) || !str(when) || Number.isNaN(Date.parse(when))) {
    return undefined;
  }
  if (!RUN_STATES.includes(state as RunState) || !str(why) || !str(ask) || !nat(turns)) {
    return undefined;
  }
  return { id, when, state: state as RunState, why, ask, turns };
}

/** The live chats an answer holds. */
export function parseSessions(body: unknown): BoardSession[] {
  const list = (body as { sessions?: unknown } | null)?.sessions;
  if (!Array.isArray(list)) throw new Error('the answer holds no list of chats');
  return list.map(session).filter((s): s is BoardSession => s !== undefined);
}

/** The past runs an answer holds, and how many it did not name. */
export function parseRuns(body: unknown): { runs: BoardRun[]; more: number } {
  const b = body as { runs?: unknown; more?: unknown } | null;
  if (!Array.isArray(b?.runs)) throw new Error('the answer holds no list of runs');
  return {
    runs: b.runs.map(run).filter((r): r is BoardRun => r !== undefined),
    more: nat(b.more) ? b.more : 0,
  };
}

/** An event as a message carries it (`{method: "event/<type>", params}`), if it is one. */
export function arrived(message: unknown): Arrived | undefined {
  const m = message as { method?: unknown; params?: unknown } | null;
  if (!m || !str(m.method) || typeof m.params !== 'object' || m.params === null) return undefined;
  const type = eventType(m.method);
  if (!type) return undefined;
  const e = { ...(m.params as Record<string, unknown>), type } as Arrived;
  if (!nat(e.seq) || !str(e.body)) return undefined;
  return e;
}

/** A past run's transcript, its events checked. */
export function parseTranscript(body: unknown): BoardTranscript & { arrived: Arrived[] } {
  const b = body as Partial<BoardTranscript> | null;
  if (!b || !str(b.run) || !Array.isArray(b.events)) throw new Error('the answer is no transcript');
  const list = b.events.map(arrived).filter((e): e is Arrived => e !== undefined);
  return {
    run: b.run,
    state: RUN_STATES.includes(b.state as RunState) ? (b.state as RunState) : 'unknown',
    events: b.events,
    dropped: nat(b.dropped) ? b.dropped : 0,
    skipped: Array.isArray(b.skipped) ? b.skipped.filter(nat) : [],
    arrived: list,
  };
}

async function getJson(path: string, signal?: AbortSignal): Promise<unknown> {
  await establish();
  const res = await fetch(path, {
    signal: signal ?? null,
    credentials: 'same-origin',
    headers: { accept: 'application/json' },
  });
  if (res.status === 401) {
    throw new Error('this browser holds no board session: open the URL `mnemorphd board` prints');
  }
  if (!res.ok) {
    const why = (await res.text().catch(() => '')).slice(0, 300);
    throw new Error(why ? `${why} (${res.status})` : `the board server answered ${res.status}`);
  }
  return res.json();
}

export interface Chats {
  /** The world's live chats. */
  live: BoardSession[];
  /** Its past runs, newest first. */
  runs: BoardRun[];
  /** Runs past the listing's cap. */
  more: number;
  /** Why the last read failed, if it did. */
  failed?: string | undefined;
  /** Read once at least. */
  ready: boolean;
  /** Read again now. */
  refresh: () => void;
}

/** How often the chat list is read again while a world is open. */
export const CHATS_EVERY = 10_000;

/** The chats of `world`, read on opening, every `CHATS_EVERY` and on `refresh`. */
export function useChats(world: string | undefined): Chats {
  const [state, setState] = useState<Omit<Chats, 'refresh'>>({
    live: [],
    runs: [],
    more: 0,
    ready: false,
  });
  const [tick, setTick] = useState(0);
  const refresh = useCallback(() => setTick((t) => t + 1), []);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `tick` asks for a read again
  useEffect(() => {
    if (world === undefined) return;
    const ctl = new AbortController();
    const read = () =>
      Promise.all([
        getJson('/api/sessions', ctl.signal).then(parseSessions),
        getJson(`/api/w/${encodeURIComponent(world)}/runs`, ctl.signal).then(parseRuns),
      ])
        .then(([sessions, past]) =>
          setState({
            live: sessions.filter((s) => s.world === world),
            runs: past.runs,
            more: past.more,
            ready: true,
          }),
        )
        .catch((e: unknown) => {
          if (ctl.signal.aborted) return;
          setState((s) => ({
            ...s,
            ready: true,
            failed: e instanceof Error ? e.message : String(e),
          }));
        });
    void read();
    const t = window.setInterval(() => void read(), CHATS_EVERY);
    return () => {
      ctl.abort();
      window.clearInterval(t);
    };
  }, [world, tick]);
  useEffect(() => {
    if (world === undefined) setState({ live: [], runs: [], more: 0, ready: false });
  }, [world]);
  return { ...state, refresh };
}

/** Start a chat in `world`: new, or resuming `resume`. Answers the chat, or throws why not. */
export async function startChat(world: string, resume?: string): Promise<BoardStarted> {
  const res = await post(
    `/api/w/${encodeURIComponent(world)}/sessions`,
    resume === undefined ? {} : { resume },
  );
  if (!res.ok) {
    const why = (await res.text().catch(() => '')).slice(0, 300);
    throw new Error(why || `the board server answered ${res.status}`);
  }
  const body = (await res.json()) as BoardStarted;
  if (!session(body?.session)) throw new Error('the board answered no chat');
  return body;
}

export type RunLoad =
  | { state: 'loading' }
  | { state: 'ready'; data: ReturnType<typeof parseTranscript> }
  | { state: 'failed'; reason: string };

/** A past run's transcript. */
export function useRun(world: string, run: string): RunLoad {
  const [load, setLoad] = useState<RunLoad>({ state: 'loading' });
  useEffect(() => {
    const ctl = new AbortController();
    setLoad({ state: 'loading' });
    getJson(`/api/w/${encodeURIComponent(world)}/runs/${encodeURIComponent(run)}`, ctl.signal)
      .then((b) => setLoad({ state: 'ready', data: parseTranscript(b) }))
      .catch((e: unknown) => {
        if (ctl.signal.aborted) return;
        setLoad({ state: 'failed', reason: e instanceof Error ? e.message : String(e) });
      });
    return () => ctl.abort();
  }, [world, run]);
  return load;
}
