/**
 * How each entry of a transcript reads, in the TUI's grammar (crates/mnemorph-tui/src/blocks.rs):
 * the human's line on a gold spine, the answer on a holo spine under `✦ <body>`, a tool call as a
 * card coloured by its class — a look in Rimuru's blue, a write in the Dark Elf's violet, a
 * destructive act in the alarm — its class always named beside the colour, its output folded to
 * three lines and its diff to twelve, numbered; consecutive looks as one card; the wait before an
 * answer as one quiet line; the end gate as one line with its counts. Every word a model or a tool
 * wrote is a `<Safe>` text node.
 */
import { memo, useCallback, useState } from 'react';
import { type Call, classOf, type Entry, groupCounts, running, summary } from '../data/chat.ts';
import { type Change, changeOf, type DiffLine, stat } from '../data/diff.ts';
import { Markdown } from '../safe/Markdown.tsx';
import { Safe } from '../safe/Safe.tsx';

/** Output lines a done call shows before it folds. */
export const OUTPUT_LINES = 3;
/** Diff lines a card shows before it folds. */
export const DIFF_LINES = 12;

/** A duration as the TUI writes a call's: `12ms`, `1.2s`, `1m3s`. */
export function took(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60_000)}m${Math.floor(ms / 1000) % 60}s`;
}

/** Elapsed time in whole seconds: `0s`, `12s`, `1m3s`. */
export function elapsed(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m${s % 60}s`;
}

const lineCount = (s: string) => {
  const t = s.replace(/\n+$/, '');
  return t === '' ? 0 : t.split('\n').length;
};

/** Whether a call has something folded that expanding it would show. */
export function foldable(call: Call): boolean {
  const o = call.outcome;
  if (!o) return false;
  if (o.isError) return lineCount(o.content) > OUTPUT_LINES;
  const c = changeOf(call.name, call.input);
  if (c?.kind === 'diff') return c.lines.length > DIFF_LINES;
  if (c?.kind === 'file') return c.lines > c.head.length;
  if (call.name === 'read') return o.content.trim() !== '';
  return lineCount(o.content) > OUTPUT_LINES;
}

function Toggle({ open, more, onToggle }: { open: boolean; more: number; onToggle: () => void }) {
  return (
    <button type="button" className="fold" aria-expanded={open} onClick={onToggle}>
      {open ? 'fold' : more > 0 ? `… +${more} lines · expand` : 'expand'}
    </button>
  );
}

/** Output cut to `OUTPUT_LINES`, the rest counted; open, all of it. */
function Folded({
  content,
  open,
  alarm,
  onToggle,
}: {
  content: string;
  open: boolean;
  alarm?: boolean;
  onToggle: () => void;
}) {
  const text = content.replace(/\n+$/, '');
  if (text.trim() === '') return null;
  const all = text.split('\n');
  const shown = open ? all : all.slice(0, OUTPUT_LINES);
  const more = all.length - shown.length;
  return (
    <div className={alarm ? 'out out-alarm' : 'out'}>
      <pre className="out-text">
        <Safe text={shown.join('\n')} lines />
      </pre>
      {(more > 0 || open) && all.length > OUTPUT_LINES && (
        <Toggle open={open} more={more} onToggle={onToggle} />
      )}
    </div>
  );
}

function Row({ line }: { line: DiffLine }) {
  if (line.kind === 'hunk') {
    return (
      <div className="diff-row diff-hunk">
        <span className="diff-n" />
        <span className="diff-text">
          <Safe text={line.text} />
        </span>
      </div>
    );
  }
  const sign = line.kind === 'add' ? '+' : line.kind === 'del' ? '−' : ' ';
  return (
    <div className={`diff-row diff-${line.kind}`}>
      <span className="diff-n">{line.kind === 'del' ? '' : line.n}</span>
      {line.kind === 'same' ? (
        <span className="diff-sign" />
      ) : (
        <span
          className="diff-sign"
          role="img"
          aria-label={line.kind === 'add' ? 'added' : 'removed'}
        >
          {sign}
        </span>
      )}
      <span className="diff-text">
        <Safe text={line.text} />
      </span>
    </div>
  );
}

/** A change's lines with their numbers; long diffs fold. */
function Diff({
  change,
  input,
  open,
  onToggle,
}: {
  change: Change;
  input: unknown;
  open: boolean;
  onToggle: () => void;
}) {
  if (change.kind === 'file') {
    const content = (input as { content?: unknown } | null)?.content;
    const all = typeof content === 'string' ? content.replace(/\n$/, '').split('\n') : change.head;
    const shown = open ? all : change.head;
    const more = change.lines - shown.length;
    return (
      <div className="diff">
        {shown.map((t, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: lines never reorder
          <Row key={i} line={{ kind: 'add', text: t, n: i + 1 }} />
        ))}
        {(more > 0 || open) && change.lines > change.head.length && (
          <Toggle open={open} more={more} onToggle={onToggle} />
        )}
      </div>
    );
  }
  const shown = open ? change.lines : change.lines.slice(0, DIFF_LINES);
  const more = change.lines.length - shown.length;
  return (
    <div className="diff">
      {shown.map((l, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: lines never reorder
        <Row key={i} line={l} />
      ))}
      {(more > 0 || open) && change.lines.length > DIFF_LINES && (
        <Toggle open={open} more={more} onToggle={onToggle} />
      )}
    </div>
  );
}

/** A call's dot: the spinner's place while it runs, then ok, failed or stopped — in words too. */
function Dot({ call }: { call: Call }) {
  if (running(call)) return <span className="dot dot-run" role="img" aria-label="running" />;
  if (call.outcome?.isError)
    return <span className="dot dot-fail" role="img" aria-label="failed" />;
  if (call.stopped) return <span className="dot dot-stop" role="img" aria-label="stopped" />;
  return <span className="dot dot-ok" role="img" aria-label="ok" />;
}

function Status({ call, change }: { call: Call; change: Change | undefined }) {
  const o = call.outcome;
  const parts: string[] = [];
  if (!o) {
    return (
      <div className="tool-status">
        <span className="elbow" aria-hidden="true">
          ⎿
        </span>{' '}
        {call.stopped ? 'stopped' : 'running…'}
      </div>
    );
  }
  if (call.took !== undefined) parts.push(took(call.took));
  if (change) parts.push(stat(change));
  else if (!o.isError) {
    const n = lineCount(o.content);
    if (n > 0) parts.push(`${n} line${n === 1 ? '' : 's'}`);
  }
  if (o.cut > 0) parts.push(`${o.cut} bytes cut`);
  return (
    <div className="tool-status">
      <span className="elbow" aria-hidden="true">
        ⎿
      </span>{' '}
      <span className={o.isError ? 'word-fail' : 'word-ok'}>{o.isError ? 'failed' : 'ok'}</span>
      {parts.map((p) => (
        <span key={p} className="tool-part">
          {' · '}
          {p}
        </span>
      ))}
    </div>
  );
}

function ToolCard({ call, open, onToggle }: { call: Call; open: boolean; onToggle: () => void }) {
  const ok = call.outcome !== undefined && !call.outcome.isError;
  const change = ok ? changeOf(call.name, call.input) : undefined;
  const cls = classOf(call.name);
  const o = call.outcome;
  return (
    <article className="card tool" data-class={cls ?? 'none'}>
      <header className="tool-head">
        <Dot call={call} />
        <span className="tool-name">
          <Safe text={call.name} />
        </span>
        <span className="tool-sum mono">
          <Safe text={summary(call.name, call.input)} />
        </span>
        {cls && <span className="tool-class">{cls}</span>}
      </header>
      <Status call={call} change={change} />
      {o?.isError ? (
        <Folded content={o.content} open={open} alarm onToggle={onToggle} />
      ) : o && change ? (
        <Diff change={change} input={call.input} open={open} onToggle={onToggle} />
      ) : o && (call.name !== 'read' || open) ? (
        <Folded content={o.content} open={open} onToggle={onToggle} />
      ) : o && call.name === 'read' && o.content.trim() !== '' ? (
        <Toggle open={false} more={lineCount(o.content)} onToggle={onToggle} />
      ) : null}
    </article>
  );
}

function GroupCard({
  calls,
  open,
  onToggle,
}: {
  calls: readonly Call[];
  open: boolean;
  onToggle: () => void;
}) {
  const live = calls.find(running);
  const failed = calls.filter((c) => c.outcome?.isError).length;
  const spent = calls.reduce((t, c) => t + (c.took ?? 0), 0);
  const timed = calls.some((c) => c.took !== undefined);
  return (
    <article className="card tool group" data-class="read">
      <header className="tool-head">
        <span
          className={live ? 'dot dot-run' : failed ? 'dot dot-fail' : 'dot dot-ok'}
          role="img"
          aria-label={live ? 'running' : failed ? 'failed' : 'ok'}
        />
        <span className="tool-name">explore</span>
        <span className="tool-sum">
          <Safe text={groupCounts(calls)} />
        </span>
        <span className="tool-class">read</span>
      </header>
      <div className="tool-status">
        <span className="elbow" aria-hidden="true">
          ⎿
        </span>{' '}
        {live ? (
          <span className="mono">
            <Safe text={`${live.name} ${summary(live.name, live.input)}`} />
          </span>
        ) : (
          <>
            <span className={failed ? 'word-fail' : 'word-ok'}>
              {failed ? `${failed} failed` : 'ok'}
            </span>
            <span className="tool-part">
              {' · '}
              {calls.length} calls{timed ? ` · ${took(spent)}` : ''}
            </span>
          </>
        )}
        <button type="button" className="fold" aria-expanded={open} onClick={onToggle}>
          {open ? 'fold' : 'list'}
        </button>
      </div>
      {open && (
        <ul className="group-list">
          {calls.map((c) => (
            <li key={c.id}>
              <Dot call={c} /> <span className="tool-name">{<Safe text={c.name} />}</span>{' '}
              <span className="mono">
                <Safe text={summary(c.name, c.input)} />
              </span>
            </li>
          ))}
        </ul>
      )}
    </article>
  );
}

function Verdict({ e }: { e: Extract<Entry, { kind: 'verdict' }> }) {
  const [mark, word, tone] =
    e.verdict === 'PASS'
      ? ['✓', 'pass', 'ok']
      : e.verdict === 'NOT_VERIFIED'
        ? ['◌', 'not verified', 'gold']
        : ['✗', 'fail', 'alarm'];
  const counts: string[] = [];
  if (e.changed.length > 0) counts.push(`${e.changed.length} changed`);
  if (e.failures.length > 0) counts.push(`${e.failures.length} failed`);
  if (e.unverified.length > 0) counts.push(`${e.unverified.length} unverified`);
  const first = e.failures[0] ?? e.unverified[0];
  return (
    <div className="verdict">
      <span className="chip" data-verdict={tone}>
        {mark} end gate · {word}
      </span>
      {counts.length > 0 && <span className="verdict-counts"> · {counts.join(' · ')}</span>}
      {first && (
        <span className="verdict-why">
          : <Safe text={first} />
        </span>
      )}
    </div>
  );
}

/** One entry. `me` is the session's body; `open` whether its folded part is shown. */
export const EntryView = memo(function EntryView({
  e,
  me,
  open,
  onToggle,
}: {
  e: Entry;
  me: string;
  open: boolean;
  onToggle: (key: string) => void;
}) {
  const toggle = () => onToggle(e.key);
  switch (e.kind) {
    case 'user':
      return (
        <div className="msg msg-user">
          <span className="msg-who">you</span>
          <div className="msg-text">
            <Safe text={e.text} lines />
          </div>
        </div>
      );
    case 'assistant':
      return (
        <div className={e.head ? 'msg msg-body' : 'msg msg-body msg-cont'}>
          {e.head && (
            <span className="msg-who">
              <span aria-hidden="true">✦ </span>
              <Safe text={me || 'the body'} />
            </span>
          )}
          <Markdown text={e.text} />
        </div>
      );
    case 'tool':
      return <ToolCard call={e.call} open={open} onToggle={toggle} />;
    case 'group':
      return <GroupCard calls={e.calls} open={open} onToggle={toggle} />;
    case 'thought':
      return (
        <div className="quiet" title="the wait before the model's first output">
          <span aria-hidden="true">✓ </span>
          {e.took !== undefined ? `thought for ${elapsed(e.took)}` : 'thinking'}
        </div>
      );
    case 'note':
      return (
        <div className="line line-note">
          <span aria-hidden="true">ℹ </span>
          <Safe text={e.text} lines />
        </div>
      );
    case 'warning':
      return (
        <div className="line line-warn">
          <span className="line-mark">⚠ warning</span> <Safe text={e.text} lines />
        </div>
      );
    case 'verdict':
      return <Verdict e={e} />;
    case 'ending':
      return (
        <div className="line line-end">
          <span className="line-mark">◇ turn ended</span> <Safe text={e.ending} />
        </div>
      );
    case 'below':
      return (
        <div className={e.alarm ? 'line line-below line-alarm' : 'line line-below'}>
          <span aria-hidden="true">↳ </span>
          <span className="below-who">
            <Safe text={e.body} />
          </span>{' '}
          <Safe text={e.text} />
        </div>
      );
  }
});

/** The text still streaming, as the answer's next paragraph. */
export function Streaming({ text, me, head }: { text: string; me: string; head: boolean }) {
  if (text.trim() === '') return null;
  return (
    <div className={head ? 'msg msg-body msg-live' : 'msg msg-body msg-cont msg-live'}>
      {head && (
        <span className="msg-who">
          <span aria-hidden="true">✦ </span>
          <Safe text={me || 'the body'} />
        </span>
      )}
      <div className="md">
        <p>
          <Safe text={text} lines />
        </p>
      </div>
    </div>
  );
}

/** Whether the next answer piece opens the turn's answer: no answer since the last human line. */
export function opensAnswer(entries: readonly Entry[]): boolean {
  for (let i = entries.length - 1; i >= 0; i--) {
    const k = entries[i]?.kind;
    if (k === 'assistant') return false;
    if (k === 'user') return true;
  }
  return true;
}

export function useOpen(): [ReadonlySet<string>, (key: string) => void] {
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const toggle = useCallback(
    (key: string) =>
      setOpen((s) => {
        const n = new Set(s);
        if (n.has(key)) n.delete(key);
        else n.add(key);
        return n;
      }),
    [],
  );
  return [open, toggle];
}
