/**
 * The approval card's rules, apart from its drawing — the TUI's card (`mnemorph-tui`'s
 * `approval.rs`) on the web:
 *
 * - **Arming.** The card arms {@link ARM_MS} after it is first *visible* — the tab visible, the
 *   window focused, the card in the viewport — and after the board took its render
 *   acknowledgement. Input before that is ignored. Losing visibility disarms it; regaining it
 *   arms it again {@link ARM_MS} later.
 * - **Disarming.** Any key that is not a pick, Enter, Esc, a scroll or a focus move — and any
 *   paste — clears the choice and arms the card again {@link ARM_MS} later, so a word typed into
 *   it never ends in an answer. Enter decides only {@link CONFIRM_MS} after its pick, and never in
 *   the task that picked: a `y` and an Enter that come together are typing, and disarm it.
 * - **Reading.** Yes is refused until every row of the act has been on screen, in order: a row
 *   counts only once every row before it has.
 * - **Denials.** Esc denies at once, armed or not; so does the deadline.
 *
 * The board holds the same rules it can see — the digest, the acknowledgement, the 300 ms, one
 * answer, the controller only — whatever this page does.
 */
import type { GateRequest } from '@mnemorph/protocol';

/** How long the card ignores input once it is visible. */
export const ARM_MS = 300;
/** How long after its pick an Enter may confirm it. */
export const CONFIRM_MS = 150;
/** The most characters of the act one row holds. */
export const ROW_CHARS = 64;
/** The most bytes of an act, as shown, the web may approve (the board's `ACT_BYTES`). */
export const ACT_BYTES = 32 * 1024;
/** The most rows of an act the web may approve (the board's `ACT_ROWS`). */
export const ACT_ROWS = 400;

/** The answers, in their order on the card. */
export const ANSWERS = ['Yes, run it once', 'No', 'No, and tell the model why'] as const;

/** One of {@link ANSWERS}, by its index: 0 yes, 1 no, 2 no with a reason. */
export type Pick = 0 | 1 | 2;

/** An escape the board's `shown` wrote: `\u{…}`. A backslash written in the text is itself one
 * (`\u{5c}`), so every match is an escape. */
const ESCAPE = /\\u\{[0-9a-f]{1,6}\}/g;

/** The escape of a newline: a row ends after it. */
const NEWLINE = '\\u{a}';

/**
 * The act, as shown, in rows: a row ends after each escaped newline, and a longer run is cut
 * every {@link ROW_CHARS} characters — never inside an escape. Every character is in exactly one
 * row, in order.
 */
export function actRows(act: string, width = ROW_CHARS, max = Number.POSITIVE_INFINITY): string[] {
  const rows: string[] = [];
  let row = '';
  let n = 0;
  const end = () => {
    rows.push(row);
    row = '';
    n = 0;
  };
  let at = 0;
  while (at < act.length && rows.length <= max) {
    // The next atom: an escape `shown` wrote, or one character.
    let atom: string;
    if (act.startsWith('\\u{', at)) {
      const close = act.indexOf('}', at);
      atom = close > at && close - at <= 9 ? act.slice(at, close + 1) : '\\';
    } else {
      const cp = act.codePointAt(at) ?? 0;
      atom = String.fromCodePoint(cp);
    }
    if (n > 0 && n + atom.length > width) end();
    row += atom;
    n += atom.length;
    at += atom.length;
    if (atom === NEWLINE) end();
  }
  if ((row !== '' && rows.length <= max) || rows.length === 0) rows.push(row);
  return rows;
}

/** Whether ask `a` is too large to approve from the web, its act drawn in `rows` rows: the board
 * says so, or the act is over {@link ACT_BYTES} or {@link ACT_ROWS} as shown. Only No answers it
 * here; the terminal may approve it. */
export function tooLarge(a: GateRequest, rows: number): boolean {
  return (
    a.tooLarge === true || new TextEncoder().encode(a.act).length > ACT_BYTES || rows > ACT_ROWS
  );
}

/** The text in runs: plain, or an escape `shown` wrote. */
export function escapes(text: string): { text: string; escaped: boolean }[] {
  const out: { text: string; escaped: boolean }[] = [];
  let at = 0;
  for (const m of text.matchAll(ESCAPE)) {
    if (m.index > at) out.push({ text: text.slice(at, m.index), escaped: false });
    out.push({ text: m[0], escaped: true });
    at = m.index + m[0].length;
  }
  if (at < text.length) out.push({ text: text.slice(at), escaped: false });
  return out;
}

/**
 * The SHA-256 of ask `id` as shown, in lowercase hex — the board's `Shown::digest`: a version
 * line, then each field as its length in UTF-8 bytes, a colon, the text and a newline; each list
 * first its count; the plan `-` when absent, `+` before it when present. Computed over exactly
 * what the card renders, so an allow names what was on screen.
 */
export async function askDigest(id: string, a: GateRequest): Promise<string> {
  const enc = new TextEncoder();
  const parts: Uint8Array[] = [enc.encode('mnemorph.ask.v1\n')];
  const field = (s: string) => {
    const b = enc.encode(s);
    parts.push(enc.encode(`${b.length}:`), b, enc.encode('\n'));
  };
  for (const s of [id, a.body, a.race, a.class, a.act]) field(s);
  for (const list of [a.why, a.outside]) {
    parts.push(enc.encode(`${list.length}\n`));
    for (const s of list) field(s);
  }
  if (a.plan === undefined || a.plan === null) parts.push(enc.encode('-\n'));
  else {
    parts.push(enc.encode('+'));
    field(a.plan);
  }
  const all = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    all.set(p, at);
    at += p.length;
  }
  const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', all));
  return Array.from(hash, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** What a key did to the card. */
export type Decision =
  | { kind: 'none' }
  | { kind: 'allow' }
  | { kind: 'deny'; reason?: string }
  /** The third answer confirmed: the reason is typed next. */
  | { kind: 'reason' };

/** The keys that move through the act or the card without answering. */
const MOVES = new Set([
  'ArrowUp',
  'ArrowDown',
  'PageUp',
  'PageDown',
  'Home',
  'End',
  ' ',
  'j',
  'k',
  'Tab',
  'Shift',
  'Control',
  'Alt',
  'Meta',
  'CapsLock',
]);

/** The keys that pick an answer. */
const PICKS: Record<string, Pick> = { '1': 0, y: 0, Y: 0, '2': 1, n: 1, N: 1, '3': 2 };

/**
 * One card's state: when it arms, what is picked, how much of the act has been read. Times are
 * in the page's milliseconds (`performance.now()`, an event's `timeStamp`), passed in so the
 * rules test without a clock.
 */
export class Card {
  /** When the card arms; `undefined` while it is not visible or not acknowledged. */
  armsAt: number | undefined;
  /** The answer picked and not yet confirmed. */
  choice: Pick | undefined;
  /** Yes was picked before the act was read to its end. */
  unread = false;
  /** The rows read in order, from the first. */
  seenTo = 0;
  private chosenAt = 0;
  /** The task that picked: an Enter in the same one is typing. */
  private chosenTask = -1;
  private visibleNow = false;
  private ackedNow = false;

  /** `approvable` false: the act is too large for the web, and Yes is never picked. */
  constructor(
    readonly rows: number,
    readonly approvable = true,
  ) {}

  /** Whether every row of the act has been on screen. */
  get viewed(): boolean {
    return this.seenTo >= this.rows;
  }

  /** Whether input at `at` counts. */
  armed(at: number): boolean {
    return this.armsAt !== undefined && at >= this.armsAt;
  }

  /** The card became visible (or not) at `at`. */
  visible(on: boolean, at: number): void {
    if (on === this.visibleNow) return;
    this.visibleNow = on;
    this.restart(at);
  }

  /** The board took the render acknowledgement (or control moved, and it must be sent again). */
  acked(on: boolean, at: number): void {
    if (on === this.ackedNow) return;
    this.ackedNow = on;
    this.restart(at);
  }

  private restart(at: number): void {
    this.choice = undefined;
    this.armsAt = this.visibleNow && this.ackedNow ? at + ARM_MS : undefined;
  }

  /** Disarm at `at`: the choice cleared, armed again {@link ARM_MS} later if still visible. */
  disarm(at: number): void {
    this.choice = undefined;
    if (this.armsAt !== undefined) this.armsAt = Math.max(this.armsAt, at + ARM_MS);
  }

  /** Text pasted at `at`: never an answer. */
  pasted(at: number): void {
    this.disarm(at);
  }

  /** Row `i` is on screen: it counts once every row before it has. */
  rowSeen(i: number): void {
    if (i <= this.seenTo) this.seenTo = Math.max(this.seenTo, i + 1);
  }

  /** Pick answer `p` at `at`, in task `task`: yes only once the act is read. */
  pick(p: Pick, at: number, task: number): void {
    if (!this.armed(at)) return;
    if (p === 0 && !this.approvable) return;
    if (p === 0 && !this.viewed) {
      this.unread = true;
      this.choice = undefined;
      return;
    }
    this.unread = false;
    this.choice = p;
    this.chosenAt = at;
    this.chosenTask = task;
  }

  /** The decision of the choice confirmed at `at`, in task `task`. */
  private confirm(at: number, task: number): Decision {
    if (this.choice === undefined) return { kind: 'none' };
    if (task === this.chosenTask || at - this.chosenAt < CONFIRM_MS) {
      this.disarm(at);
      return { kind: 'none' };
    }
    if (this.choice === 0) {
      return this.viewed && this.approvable ? { kind: 'allow' } : { kind: 'none' };
    }
    return this.choice === 1 ? { kind: 'deny' } : { kind: 'reason' };
  }

  /** A key pressed at `at`, in task `task`. */
  key(key: string, at: number, task: number): Decision {
    if (key === 'Escape') return { kind: 'deny' };
    if (!this.armed(at)) return { kind: 'none' };
    const p = PICKS[key];
    if (p !== undefined) {
      this.pick(p, at, task);
      return { kind: 'none' };
    }
    if (key === 'Enter') return this.confirm(at, task);
    if (!MOVES.has(key)) this.disarm(at);
    return { kind: 'none' };
  }

  /** A pointer's click on answer `p` at `at`: armed, and yes only once the act is read. */
  click(p: Pick, at: number): Decision {
    if (!this.armed(at)) return { kind: 'none' };
    if (p === 0) return this.viewed && this.approvable ? { kind: 'allow' } : { kind: 'none' };
    return p === 1 ? { kind: 'deny' } : { kind: 'reason' };
  }
}
