/**
 * A chat's transcript as the board keeps it, folded from the protocol's events in the TUI's
 * grammar (crates/mnemorph-tui/src/transcript.rs): the human's lines, the body's answer in
 * paragraphs as they finish, tool calls as cards from their first byte, consecutive looks (read,
 * index, grep, glob) as one card, the end gate as one line, a turn that ended otherwise as one
 * line, and what a commissioned body did as an indented line under it. Beside it, the session's
 * vitals, the files its acts changed and the bodies it commissioned.
 *
 * Every event is folded once, in `seq` order (the SDK drops a repeat), and nothing here renders:
 * the text is kept as it came and drawn inert by the views.
 */
import type { Arrived } from '@mnemorph/protocol';

/** What a tool returned. */
export interface Outcome {
  content: string;
  isError: boolean;
  /** Bytes the core cut from it. */
  cut: number;
}

/** A tool call. */
export interface Call {
  id: string;
  name: string;
  /** Its input; `null` until the call is whole. */
  input: unknown;
  /** When it started (ms since the epoch), if it was seen live. */
  began?: number | undefined;
  /** How long it ran, once it answered, if it was seen live. */
  took?: number | undefined;
  outcome?: Outcome;
  /** The turn ended before it answered. */
  stopped: boolean;
}

export type Entry =
  | { kind: 'user'; key: string; text: string }
  | { kind: 'assistant'; key: string; text: string; head: boolean }
  | { kind: 'tool'; key: string; call: Call }
  | { kind: 'group'; key: string; calls: Call[] }
  | { kind: 'thought'; key: string; began: number; took?: number | undefined }
  | { kind: 'note'; key: string; text: string }
  | { kind: 'warning'; key: string; text: string }
  | {
      kind: 'verdict';
      key: string;
      verdict: string;
      changed: string[];
      failures: string[];
      unverified: string[];
    }
  | { kind: 'ending'; key: string; ending: string }
  | { kind: 'below'; key: string; body: string; text: string; alarm: boolean };

/** The tools a group gathers: they look, and change nothing. */
export const EXPLORE: readonly string[] = ['read', 'index', 'grep', 'glob'];

/** The spend, as the vitals read it. */
export interface Usage {
  /** Dollars, where turns were priced. */
  usd: number;
  /** Some turn had no price: the dollars are a floor, never the whole. */
  unpriced: boolean;
  /** Whether any turn's spend was seen. */
  seen: boolean;
  /** Tokens through the model: input, cache and output — the session body's own. */
  tokens: number;
  /** The run's weighted tokens so far. */
  spentWt: number;
  /** How full the context is, and the line it is measured against. */
  occupancy: number;
  stressLine: number;
}

/** A path the session's acts changed: added, modified or deleted. */
export type FileMark = 'A' | 'M' | 'D';

/** A body the session commissioned, as the tree shows it. */
export interface Body {
  name: string;
  state: 'working' | 'done' | 'failed';
  /** What it is on, or how it ended. */
  last: string;
}

/** The chat as the views read it: a snapshot, replaced whole on each change. */
export interface ChatView {
  entries: readonly Entry[];
  /** The answer streaming now, not yet a paragraph. */
  streaming: string;
  /** Lines waiting their turn. */
  queued: readonly string[];
  /** A turn runs. */
  busy: boolean;
  /** When the running turn began (ms), if seen live. */
  since?: number | undefined;
  /** The tokens the running turn spent so far. */
  turnTokens: number;
  /** The call running now, the newest. */
  current?: Call | undefined;
  /** The model's step is waiting for its first output since `waiting` (ms), if seen live. */
  waiting?: number | undefined;
  usage: Usage;
  files: readonly (readonly [string, FileMark])[];
  bodies: readonly Body[];
  /** The orders held on the shared work queue. */
  held: number;
}

const str = (v: unknown, k: string): string | undefined => {
  const x = (v as Record<string, unknown> | null)?.[k];
  return typeof x === 'string' ? x : undefined;
};

/** The first line of `s`, its whitespace folded. */
export function firstLine(s: string): string {
  return (s.split('\n').find((l) => l.trim() !== '') ?? '').trim().replace(/\s+/g, ' ');
}

/** The one-line summary of a call's input: its command, path (with the range read), pattern,
 * query or URL; else its JSON, cut. As the TUI's `blocks::summary`. */
export function summary(name: string, input: unknown): string {
  if (input === null || input === undefined) return '';
  const cmd = str(input, 'command');
  if (cmd !== undefined) return firstLine(cmd);
  const path = str(input, 'path');
  if (path !== undefined) {
    const num = (k: string) => {
      const x = (input as Record<string, unknown>)[k];
      return typeof x === 'number' && Number.isInteger(x) && x >= 0 ? x : undefined;
    };
    const off = num('offset');
    const end = num('end');
    if (name === 'read' && (off !== undefined || end !== undefined)) {
      const o = off ?? 1;
      const lim = num('limit') ?? 0;
      if (end !== undefined) return `${firstLine(path)}:${o}-${end}`;
      if (lim > 0) return `${firstLine(path)}:${o}-${o + lim - 1}`;
      return `${firstLine(path)}:${o}`;
    }
    return firstLine(path);
  }
  for (const k of ['pattern', 'query', 'url', 'body']) {
    const v = str(input, k);
    if (v !== undefined) return firstLine(v);
  }
  let json: string;
  try {
    json = JSON.stringify(input) ?? '';
  } catch {
    json = '';
  }
  json = firstLine(json);
  return json.length > 80 ? `${json.slice(0, 79)}…` : json;
}

/** A call's class: the tool's own, as the TUI reads it. A command's class is the classifier's,
 * which the protocol does not carry: it shows none. */
export function classOf(name: string): 'read' | 'write' | 'destructive' | undefined {
  if (EXPLORE.includes(name)) return 'read';
  if (['write', 'edit', 'multiedit', 'patch', 'move'].includes(name)) return 'write';
  if (name === 'delete') return 'destructive';
  return undefined;
}

/** What a group's calls looked at, counted: `read 4 files · grep 3 patterns`. */
export function groupCounts(calls: readonly Call[]): string {
  const parts: string[] = [];
  for (const [tool, one, many] of [
    ['read', 'file', 'files'],
    ['index', 'file', 'files'],
    ['grep', 'pattern', 'patterns'],
    ['glob', 'pattern', 'patterns'],
  ] as const) {
    const seen = new Set(
      calls
        .filter((c) => c.name === tool)
        .map((c) => {
          const s = summary(c.name, c.input);
          if (tool !== 'read') return s;
          const file = s.split(':')[0] ?? '';
          return file.split('#')[0] ?? file;
        }),
    );
    if (seen.size > 0) parts.push(`${tool} ${seen.size} ${seen.size === 1 ? one : many}`);
  }
  return parts.join(' · ');
}

/** Whether `text` ends inside an open code fence. */
function openFence(text: string): boolean {
  let open: [string, number] | undefined;
  for (const line of text.split('\n')) {
    const indent = line.length - line.trimStart().length;
    if (indent > 3) continue;
    const m = /^(`{3,}|~{3,})/.exec(line.slice(indent));
    if (!m?.[1]) continue;
    const c = m[1][0] ?? '`';
    const run = m[1].length;
    if (!open) open = [c, run];
    else if (open[0] === c && run >= open[1] && line.slice(indent + run).trim() === '')
      open = undefined;
  }
  return open !== undefined;
}

/** The paths a unified diff writes, and how. */
function patchMarks(patch: string): [string, FileMark][] {
  const out: [string, FileMark][] = [];
  let from: string | undefined;
  for (const l of patch.split('\n')) {
    if (l.startsWith('--- ')) from = l.slice(4).trim();
    else if (l.startsWith('+++ ')) {
      const to = l.slice(4).trim();
      const strip = (p: string) => p.replace(/^[ab]\//, '');
      if (to === '/dev/null' && from) out.push([strip(from), 'D']);
      else if (from === '/dev/null') out.push([strip(to), 'A']);
      else out.push([strip(to), 'M']);
    }
  }
  return out;
}

/** The marks a call's acts would make, once it lands. */
function marksOf(name: string, input: unknown): [string, FileMark][] {
  const path = str(input, 'path');
  switch (name) {
    case 'edit':
    case 'multiedit':
    case 'write':
      return path ? [[path, 'M']] : [];
    case 'delete':
      return path ? [[path, 'D']] : [];
    case 'move': {
      const out: [string, FileMark][] = [];
      const from = str(input, 'from');
      const to = str(input, 'to');
      if (from) out.push([from, 'D']);
      if (to) out.push([to, 'A']);
      return out;
    }
    case 'patch': {
      const p = str(input, 'patch');
      return p ? patchMarks(p) : [];
    }
    default:
      return [];
  }
}

/** The step of the model in progress: when it began, and whether it has shown output. */
interface Step {
  began?: number | undefined;
  output: boolean;
  thought?: number | undefined;
}

/** A step that waited this long before its first output leaves a line saying so. */
export const THOUGHT_MS = 1000;

/**
 * The transcript, folded event by event. `fold` changes it; `view` is the snapshot the page
 * draws, made again only after a change.
 */
export class Chat {
  private entries: Entry[] = [];
  private next = 0;
  private streaming = '';
  private flushed = '';
  private headed = false;
  private readonly running = new Map<string, number>();
  private queued: string[] = [];
  private busy = false;
  private since: number | undefined;
  private turnBase = 0;
  private step: Step = { output: true };
  private usage: Usage = {
    usd: 0,
    unpriced: false,
    seen: false,
    tokens: 0,
    spentWt: 0,
    occupancy: 0,
    stressLine: 0,
  };
  private readonly files = new Map<string, FileMark>();
  private readonly pending = new Map<string, [string, FileMark][]>();
  private readonly bodies = new Map<string, Body>();
  private held = 0;
  private snapshot: ChatView | undefined;

  /** `self` is the session's own body (the hello's `body`); events under any other came from
   * the bodies it commissioned. */
  constructor(private self: string) {}

  /** The session's own body, once the hello names it. */
  setSelf(body: string): void {
    this.self = body;
  }

  /** Fold `event` in; `now` is when it arrived, or `undefined` for one replayed — a replayed
   * event says nothing of how long anything took. */
  fold(event: Arrived, now?: number): void {
    this.snapshot = undefined;
    if (event.body !== this.self && event.body !== '') {
      this.below(event.body, event);
      return;
    }
    switch (event.type) {
      case 'turnStarted': {
        const q = this.queued[0];
        if (q !== undefined && (q === event.text || event.text.endsWith(q))) this.queued.shift();
        this.streaming = '';
        this.flushed = '';
        this.headed = false;
        this.busy = true;
        this.since = now;
        this.turnBase = this.usage.tokens;
        this.push({ kind: 'user', key: '', text: event.text });
        this.beginStep(now);
        return;
      }
      case 'queued':
        this.queued.push(event.text);
        return;
      case 'textDelta':
        this.output(now);
        this.delta(event.text);
        return;
      case 'toolStart':
        this.output(now);
        this.startCall(event.id, event.name, null, now);
        return;
      case 'assistant': {
        this.output(now);
        let rest: string;
        if (this.flushed === '') rest = event.text;
        else if (event.text.startsWith(this.flushed)) rest = event.text.slice(this.flushed.length);
        else rest = this.streaming;
        this.streaming = '';
        this.flushed = '';
        this.say(rest);
        return;
      }
      case 'toolCall':
        this.output(now);
        this.startCall(event.id, event.name, event.input, now);
        return;
      case 'toolResult':
        this.result(event.id, event.content, event.isError, event.cut, now);
        return;
      case 'usage':
        this.spend(event, false);
        return;
      case 'note':
        this.push({ kind: 'note', key: '', text: event.text });
        return;
      case 'warning':
        this.push({ kind: 'warning', key: '', text: event.text });
        return;
      case 'gateVerdict':
        for (const p of event.changed) if (!this.files.has(p)) this.files.set(p, 'M');
        this.push({
          kind: 'verdict',
          key: '',
          verdict: event.verdict,
          changed: event.changed,
          failures: event.failures,
          unverified: event.unverified,
        });
        return;
      case 'turnEnded':
        this.endTurn(event.ending, now);
        return;
      case 'held':
        this.held = event.held;
        return;
    }
  }

  /** The snapshot to draw. */
  view(): ChatView {
    if (this.snapshot) return this.snapshot;
    const view = this.make();
    this.snapshot = view;
    return view;
  }

  private make(): ChatView {
    const newest = Math.max(-1, ...this.running.values());
    let current: Call | undefined;
    const e = this.entries[newest];
    if (e?.kind === 'tool') current = e.call;
    else if (e?.kind === 'group') current = [...e.calls].reverse().find((c) => running(c));
    return {
      entries: this.entries.slice(),
      streaming: this.streaming,
      queued: this.queued.slice(),
      busy: this.busy,
      since: this.since,
      turnTokens: this.usage.tokens - this.turnBase,
      current,
      waiting:
        this.busy && !this.step.output && this.running.size === 0 ? this.step.began : undefined,
      usage: { ...this.usage },
      files: [...this.files],
      bodies: [...this.bodies.values()],
      held: this.held,
    };
  }

  private push(entry: Entry): void {
    this.next += 1;
    this.entries.push({ ...entry, key: `e${this.next}` } as Entry);
  }

  private replace(i: number, entry: Entry): void {
    this.entries[i] = entry;
  }

  private beginStep(now: number | undefined): void {
    this.step = { began: now, output: false };
  }

  /** The step's first output: the wait before it, if long, is a line of its own. */
  private output(now: number | undefined): void {
    if (this.step.output) return;
    this.step.output = true;
    const began = this.step.began;
    if (began !== undefined && now !== undefined && now - began >= THOUGHT_MS) {
      this.settle();
      this.push({ kind: 'thought', key: '', began, took: now - began });
    }
  }

  /** Text as the body's answer: the first piece of a turn names the body. */
  private say(text: string): void {
    const t = text.trim();
    if (t === '') return;
    const head = !this.headed;
    this.headed = true;
    this.push({ kind: 'assistant', key: '', text: t, head });
  }

  /** Whatever streamed and is not yet an entry becomes one. */
  private settle(): void {
    const s = this.streaming;
    this.streaming = '';
    this.flushed += s;
    this.say(s);
  }

  /** A piece of streaming text: a finished paragraph, outside any open fence, becomes an entry
   * at once, so the live part stays short. */
  private delta(text: string): void {
    this.streaming += text;
    const cut = this.streaming.lastIndexOf('\n\n');
    if (cut < 0) return;
    const done = this.streaming.slice(0, cut + 2);
    if (openFence(done)) return;
    this.streaming = this.streaming.slice(cut + 2);
    this.flushed += done;
    this.say(done);
  }

  private startCall(id: string, name: string, input: unknown, now: number | undefined): void {
    const at = this.running.get(id);
    if (at !== undefined) {
      if (input !== null) this.update(at, id, (c) => ({ ...c, input }));
      this.pending.set(id, marksOf(name, input));
      return;
    }
    this.settle();
    const call: Call = { id, name, input, began: now, stopped: false };
    if (input !== null) this.pending.set(id, marksOf(name, input));
    const last = this.entries.length - 1;
    const prev = this.entries[last];
    const joins =
      EXPLORE.includes(name) &&
      (prev?.kind === 'group' || (prev?.kind === 'tool' && EXPLORE.includes(prev.call.name)));
    if (!joins || !prev) {
      this.push({ kind: 'tool', key: '', call });
      this.running.set(id, this.entries.length - 1);
      return;
    }
    if (prev.kind === 'group') this.replace(last, { ...prev, calls: [...prev.calls, call] });
    else if (prev.kind === 'tool')
      this.replace(last, { kind: 'group', key: prev.key, calls: [prev.call, call] });
    this.running.set(id, last);
  }

  /** Call `id` in entry `i`, changed by `f`. */
  private update(i: number, id: string, f: (c: Call) => Call): void {
    const e = this.entries[i];
    if (e?.kind === 'tool' && e.call.id === id) this.replace(i, { ...e, call: f(e.call) });
    else if (e?.kind === 'group')
      this.replace(i, { ...e, calls: e.calls.map((c) => (c.id === id ? f(c) : c)) });
  }

  private result(
    id: string,
    content: string,
    isError: boolean,
    cut: number,
    now: number | undefined,
  ): void {
    const outcome: Outcome = { content, isError, cut };
    const i = this.running.get(id);
    if (i !== undefined) {
      this.running.delete(id);
      this.update(i, id, (c) => ({
        ...c,
        outcome,
        took: c.began !== undefined && now !== undefined ? now - c.began : undefined,
      }));
      if (!isError) for (const [p, m] of this.pending.get(id) ?? []) this.mark(p, m);
      this.pending.delete(id);
      // Every call of the step answered: the model is asked again.
      if (this.running.size === 0 && this.busy) this.beginStep(now);
      return;
    }
    this.push({
      kind: 'tool',
      key: '',
      call: { id, name: 'tool', input: null, outcome, stopped: false },
    });
  }

  private mark(path: string, mark: FileMark): void {
    const was = this.files.get(path);
    // A file added then changed is still added; one deleted then written is modified.
    const next = was === 'A' && mark === 'M' ? 'A' : was === 'D' && mark === 'A' ? 'M' : mark;
    this.files.delete(path);
    this.files.set(path, next);
  }

  private spend(event: Extract<Arrived, { type: 'usage' }>, below: boolean): void {
    const u = this.usage;
    u.seen = true;
    u.usd += event.usd ?? 0;
    u.unpriced ||= event.usd === null;
    // A body below spends dollars of its own; its tokens are its own context's.
    if (!below) {
      u.tokens += event.input + event.output + event.cacheRead + event.cacheWrite;
      u.spentWt = event.spentWt;
      u.occupancy = event.occupancy;
      u.stressLine = event.stressLine;
    }
  }

  private endTurn(ending: string, now: number | undefined): void {
    this.output(now);
    this.settle();
    this.flushed = '';
    for (const [id, i] of this.running) this.update(i, id, (c) => ({ ...c, stopped: true }));
    this.running.clear();
    this.pending.clear();
    this.busy = false;
    this.since = undefined;
    this.step = { output: true };
    if (ending !== 'answered') this.push({ kind: 'ending', key: '', ending });
  }

  /** An event of a body the session commissioned: one line in the transcript, and its row in the
   * tree. */
  private below(who: string, event: Arrived): void {
    const line = (text: string, alarm: boolean) =>
      this.push({ kind: 'below', key: '', body: who, text, alarm });
    const body = (state: Body['state'], last: string) =>
      this.bodies.set(who, { name: who, state, last });
    switch (event.type) {
      case 'turnStarted':
        body('working', firstLine(event.text));
        line(`is working on: ${firstLine(event.text)}`, false);
        return;
      case 'toolCall':
        line(`${event.name} ${summary(event.name, event.input)}`.trimEnd(), false);
        return;
      case 'toolResult':
        if (event.isError) line(`✗ ${firstLine(event.content)}`, true);
        return;
      case 'warning':
        this.push({ kind: 'warning', key: '', text: `${who}: ${event.text}` });
        return;
      case 'gateVerdict':
        if (!event.passed) {
          const why = event.failures[0] ?? event.unverified[0] ?? 'failed';
          line(`gate ${event.verdict}: ${why}`, true);
        }
        return;
      case 'turnEnded': {
        const ok = event.ending === 'answered';
        body(ok ? 'done' : 'failed', event.ending);
        line(event.ending, !ok);
        return;
      }
      case 'usage':
        this.spend(event, true);
        return;
      default:
        return;
    }
  }
}

/** Whether a call runs still. */
export function running(c: Call): boolean {
  return c.outcome === undefined && !c.stopped;
}
