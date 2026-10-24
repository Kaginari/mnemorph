/**
 * The session as the interface shows it, folded from the core's events: a pure function, so the
 * view renders state and never reasons about the protocol.
 */
import type { Arrived, GateRequest } from '@mnemorph/sdk';

/** One finished thing in the transcript: printed once, never redrawn. */
export type Item =
  | { kind: 'user'; id: number; text: string }
  | { kind: 'assistant'; id: number; text: string; cont?: boolean }
  | {
      kind: 'tool';
      id: number;
      call: string;
      name: string;
      input: unknown;
      result?: { content: string; isError: boolean; cut: number };
    }
  | { kind: 'note'; id: number; text: string }
  | { kind: 'warning'; id: number; text: string }
  | { kind: 'verdict'; id: number; passed: boolean; failures: string[] }
  | { kind: 'ending'; id: number; ending: string }
  | { kind: 'info'; id: number; text: string };

export interface Usage {
  /** The session's spend so far, when every call was priced. */
  usd: number;
  /** Whether some call had no known price. */
  unpriced: boolean;
  /** The run's weighted tokens. */
  spentWt: number;
  /** How full the context is, and against what. */
  occupancy: number;
  stressLine: number;
  /** The last call's cache reads over its whole input. */
  cacheShare: number;
}

export interface State {
  /** Finished items, in order. */
  items: Item[];
  /** The model's text as it streams, not yet whole: its last, unfinished paragraph. */
  streaming: string;
  /** This model turn's text already moved to the transcript, paragraph by paragraph. */
  flushed: string;
  /** Tool calls started and not yet answered, by call id. */
  running: Map<string, Extract<Item, { kind: 'tool' }>>;
  /** Whether a turn runs. */
  busy: boolean;
  /** Lines waiting their turn. */
  queued: string[];
  usage: Usage;
  /** The act waiting for the human, if any. */
  gate?: { id: number; request: GateRequest } | undefined;
  next: number;
}

export function initial(): State {
  return {
    items: [],
    streaming: '',
    flushed: '',
    running: new Map(),
    busy: false,
    queued: [],
    usage: { usd: 0, unpriced: false, spentWt: 0, occupancy: 0, stressLine: 0, cacheShare: 0 },
    next: 1,
  };
}

/** An item before it is numbered. */
type Unnumbered = Item extends infer I ? (I extends Item ? Omit<I, 'id'> : never) : never;

function push(s: State, item: Unnumbered): State {
  return { ...s, items: [...s.items, { ...item, id: s.next } as Item], next: s.next + 1 };
}

/** Fold one event in. */
export function fold(s: State, e: Arrived): State {
  switch (e.type) {
    case 'turnStarted': {
      const queued = s.queued[0] === e.text ? s.queued.slice(1) : s.queued;
      return push({ ...s, busy: true, queued, streaming: '' }, { kind: 'user', text: e.text });
    }
    case 'queued':
      return { ...s, queued: [...s.queued, e.text] };
    case 'textDelta': {
      // A finished paragraph leaves the live region for the transcript at once, so the live
      // region never grows taller than the terminal.
      const streaming = s.streaming + e.text;
      const cut = streaming.lastIndexOf('\n\n');
      if (cut === -1) return { ...s, streaming };
      const done = streaming.slice(0, cut + 2);
      const moved = { ...s, streaming: streaming.slice(cut + 2), flushed: s.flushed + done };
      return done.trim()
        ? push(moved, { kind: 'assistant', text: done.trim(), cont: s.flushed !== '' })
        : moved;
    }
    case 'toolStart':
      return s;
    case 'assistant': {
      // The whole turn's text, less what already left paragraph by paragraph.
      const rest = s.flushed
        ? e.text.startsWith(s.flushed)
          ? e.text.slice(s.flushed.length)
          : s.streaming
        : e.text;
      const text = rest.trim();
      const cleared = { ...s, streaming: '', flushed: '' };
      return text ? push(cleared, { kind: 'assistant', text, cont: s.flushed !== '' }) : cleared;
    }
    case 'toolCall': {
      const running = new Map(s.running);
      running.set(e.id, { kind: 'tool', id: 0, call: e.id, name: e.name, input: e.input });
      return { ...s, running };
    }
    case 'toolResult': {
      const call = s.running.get(e.id);
      const running = new Map(s.running);
      running.delete(e.id);
      return push(
        { ...s, running },
        {
          kind: 'tool',
          call: e.id,
          name: call?.name ?? 'tool',
          input: call?.input ?? null,
          result: { content: e.content, isError: e.isError, cut: e.cut },
        },
      );
    }
    case 'usage': {
      const input = e.input + e.cacheRead + e.cacheWrite;
      return {
        ...s,
        usage: {
          usd: s.usage.usd + (e.usd ?? 0),
          unpriced: s.usage.unpriced || e.usd === null,
          spentWt: e.spentWt,
          occupancy: e.occupancy,
          stressLine: e.stressLine,
          cacheShare: input > 0 ? e.cacheRead / input : 0,
        },
      };
    }
    case 'note':
      return push(s, { kind: 'note', text: e.text });
    case 'warning':
      return push(s, { kind: 'warning', text: e.text });
    case 'gateVerdict':
      return push(s, { kind: 'verdict', passed: e.passed, failures: e.failures });
    case 'turnEnded': {
      // A turn that ends with text still streaming keeps what was said.
      const kept = s.streaming.trim()
        ? push(
            { ...s, streaming: '' },
            { kind: 'assistant', text: s.streaming.trim(), cont: s.flushed !== '' },
          )
        : s;
      // An approval still open when its turn ends is a question no one asks any more.
      const ended = {
        ...kept,
        flushed: '',
        busy: kept.queued.length > 0,
        running: new Map(),
        gate: undefined,
      };
      return e.ending === 'answered' ? ended : push(ended, { kind: 'ending', ending: e.ending });
    }
  }
}

/** A line the interface itself writes into the transcript. */
export function info(s: State, text: string): State {
  return push(s, { kind: 'info', text });
}

/** The one-line summary of a tool call's input. */
export function summary(_name: string, input: unknown): string {
  if (input && typeof input === 'object') {
    const o = input as Record<string, unknown>;
    for (const key of ['command', 'path', 'pattern', 'query', 'url']) {
      if (typeof o[key] === 'string') return o[key] as string;
    }
  }
  const text = JSON.stringify(input ?? '');
  return text.length > 80 ? `${text.slice(0, 77)}…` : text;
}

/** The status line's reading of the usage. */
export function status(u: Usage): string[] {
  const parts: string[] = [];
  parts.push(
    u.unpriced && u.usd === 0 ? 'unpriced' : `$${u.usd.toFixed(4)}${u.unpriced ? '+' : ''}`,
  );
  if (u.stressLine > 0) {
    parts.push(`ctx ${Math.round((100 * u.occupancy) / u.stressLine)}%`);
  }
  if (u.occupancy > 0) parts.push(`cache ${Math.round(100 * u.cacheShare)}%`);
  return parts;
}
