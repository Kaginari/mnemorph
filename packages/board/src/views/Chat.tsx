/**
 * A world's chat: a live one over the bridge — the transcript, the activity strip, the asks
 * waiting, the composer — or a past one, read-only from its run journal, with a resume when it
 * stopped. The session's vitals, files and bodies go to the side pane.
 */
import type { BoardWorld, Hello } from '@mnemorph/protocol';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Chat, type ChatView, type Entry } from '../data/chat.ts';
import { type Chats, startChat, useRun } from '../data/chats.ts';
import { type Live, useLive } from '../data/live.ts';
import { format } from '../router.ts';
import { Safe } from '../safe/Safe.tsx';
import { announce } from '../ui/Announcer.tsx';
import { Approval } from '../ui/Approval.tsx';
import { Empty } from '../ui/Empty.tsx';
import { Mark } from '../ui/Mark.tsx';
import { EntryView, elapsed, opensAnswer, Streaming, useOpen } from '../ui/Transcript.tsx';
import { Virtual } from '../ui/Virtual.tsx';

/** What a chat tells the side pane. */
export interface Side {
  world: string;
  view: ChatView | undefined;
  hello: Hello | undefined;
  /** A live chat, rather than a past one. */
  live: boolean;
}

const keyOf = (e: Entry) => e.key;

/** A clock that moves every `ms` while `on`. */
function useTicker(on: boolean, ms = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    const t = window.setInterval(() => setNow(Date.now()), ms);
    return () => window.clearInterval(t);
  }, [on, ms]);
  return now;
}

/** Tokens, short: `950`, `12.3k`, `1.2M`. */
export function tokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

/** The transcript, virtualized, the text streaming and the live line after it. */
function TranscriptView({
  view,
  me,
  tail,
}: {
  view: ChatView;
  me: string;
  tail?: React.ReactNode;
}) {
  const [open, toggle] = useOpen();
  const render = useCallback(
    (e: Entry) => <EntryView e={e} me={me} open={open.has(e.key)} onToggle={toggle} />,
    [me, open, toggle],
  );
  return (
    <Virtual
      items={view.entries}
      keyOf={keyOf}
      render={render}
      label="Transcript"
      tail={
        <>
          <Streaming text={view.streaming} me={me} head={opensAnswer(view.entries)} />
          {tail}
        </>
      }
    />
  );
}

const SPIN = ['⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷'];

/** What the session does now, in words: `Thinking… 12s · 61k tok · esc to stop`. */
export function phase(view: ChatView, asks: number): string {
  if (asks > 0) return 'Waiting for approval…';
  const c = view.current;
  if (c) return c.input === null ? `Calling ${c.name}…` : `Running ${c.name}…`;
  if (view.streaming.trim() !== '') return 'Writing…';
  const u = view.usage;
  if (view.waiting === undefined && u.stressLine > 0 && u.occupancy >= u.stressLine) {
    return 'Compacting…';
  }
  return 'Thinking…';
}

function ActivityStrip({
  view,
  asks,
  controller,
}: {
  view: ChatView;
  asks: number;
  controller: boolean;
}) {
  const now = useTicker(view.busy, 120);
  if (!view.busy) return null;
  // Reduced motion: the spinner holds still.
  const still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const frame = still ? SPIN[0] : SPIN[Math.floor(now / 120) % SPIN.length];
  const parts = [phase(view, asks)];
  if (view.since !== undefined) parts.push(elapsed(now - view.since));
  parts.push(`${tokens(view.turnTokens)} tok`);
  return (
    // Not a live region: it ticks. The chat's announcements say what changed, once.
    <output className="activity" aria-live="off">
      <span className="activity-spin" aria-hidden="true">
        {frame}
      </span>
      <span className="activity-verb">{parts[0]}</span>
      <span className="activity-rest">
        {parts.slice(1).map((p) => ` · ${p}`)}
        {controller ? ' · esc to stop' : ''}
      </span>
    </output>
  );
}

/**
 * What a screen reader is told of a live chat, once each: an answer when a turn ends, an ask when
 * one arrives at the gate. What was already there when the chat opened (a replay) is not said.
 */
function useAnnouncements(view: ChatView | undefined, asks: Live['asks']) {
  const heard = useRef<{ answers: number; asks: Set<string> } | undefined>(undefined);
  const busy = view?.busy ?? false;
  useEffect(() => {
    if (!view) return;
    const answers = view.entries.filter((e) => e.kind === 'assistant');
    if (heard.current === undefined) {
      heard.current = { answers: answers.length, asks: new Set(asks.map((a) => a.id)) };
      return;
    }
    if (!busy && answers.length > heard.current.answers) {
      const fresh = answers
        .slice(heard.current.answers)
        .map((e) => (e.kind === 'assistant' ? e.text : ''))
        .join(' ');
      announce(`Answered: ${fresh}`);
    }
    if (!busy) heard.current.answers = answers.length;
  }, [view, busy, asks]);
  useEffect(() => {
    const h = heard.current;
    if (!h) return;
    for (const a of asks) {
      if (h.asks.has(a.id)) continue;
      h.asks.add(a.id);
      announce(
        `An ask waits at the gate: ${a.ask.class} by ${a.ask.body}. The approval card is open.`,
        true,
      );
    }
  }, [asks]);
}

/** The end gate's verdicts as toasts, as they arrive live — a replayed one is only a line. */
function Toasts({ entries }: { entries: readonly Entry[] }) {
  const seen = useRef(new Set<string>());
  const [toasts, setToasts] = useState<Extract<Entry, { kind: 'verdict' }>[]>([]);
  useEffect(() => {
    const fresh = entries.filter(
      (e): e is Extract<Entry, { kind: 'verdict' }> =>
        e.kind === 'verdict' && e.at !== undefined && !seen.current.has(e.key),
    );
    for (const v of fresh) seen.current.add(v.key);
    if (fresh.length > 0) setToasts((t) => [...t, ...fresh].slice(-3));
  }, [entries]);
  useEffect(() => {
    if (toasts.length === 0) return;
    const t = window.setTimeout(() => setToasts((x) => x.slice(1)), 6000);
    return () => window.clearTimeout(t);
  }, [toasts]);
  if (toasts.length === 0) return null;
  return (
    <div className="toasts">
      {toasts.map((v) => (
        <output
          key={v.key}
          className="toast"
          data-verdict={
            v.verdict === 'PASS' ? 'ok' : v.verdict === 'NOT_VERIFIED' ? 'gold' : 'alarm'
          }
        >
          <span aria-hidden="true">◆ </span>end gate {v.verdict}
          {v.changed.length > 0 ? ` · ${v.changed.length} changed` : ''}
          {v.failures.length > 0 ? ` · ${v.failures.length} failed` : ''}
        </output>
      ))}
    </div>
  );
}

function Composer({ live }: { live: Live }) {
  const [text, setText] = useState('');
  const view = live.view;
  const busy = view?.busy ?? false;
  const closed = live.link === 'closed';
  const observer = live.role === 'observer';
  const ready = live.role === 'controller' && !closed;
  const submit = (steer: boolean) => {
    const t = text.trim();
    if (!t || !ready) return;
    setText('');
    const slash = /^\/send\s+([\s\S]+)$/.exec(t);
    if (slash?.[1] && busy) void live.steer(slash[1]);
    else if (steer && busy) void live.steer(t);
    else void live.prompt(slash?.[1] ?? t);
  };
  if (closed) {
    return (
      <div className="composer composer-closed" aria-disabled="true">
        <span className="composer-label">ended</span>
        <span className="composer-hint">
          <Safe text={live.closed ?? 'the chat ended'} />
        </span>
      </div>
    );
  }
  return (
    <div className="composer-wrap">
      {view && view.queued.length > 0 && (
        <ul className="queued" aria-label="Queued">
          {view.queued.map((q, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: the queue's order is its key
            <li key={i}>
              <span aria-hidden="true">⧗ </span>queued: <Safe text={q} />
            </li>
          ))}
        </ul>
      )}
      {observer || (live.role === undefined && live.hello) ? (
        <div className="observer">
          <span>
            {live.controlled
              ? 'This tab observes: another tab of this browser controls the chat.'
              : 'No tab controls this chat.'}
          </span>
          <button type="button" className="button" onClick={() => void live.takeOver()}>
            Take over
          </button>
        </div>
      ) : null}
      <form
        className="composer"
        onSubmit={(e) => {
          e.preventDefault();
          submit(false);
        }}
      >
        <label className="composer-label" htmlFor="composer-text">
          ask
        </label>
        <textarea
          id="composer-text"
          className="composer-text"
          rows={1}
          value={text}
          disabled={!ready}
          placeholder={ready ? (busy ? 'Queue a message, or steer…' : 'Ask…') : 'Observing'}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              submit(false);
            }
          }}
        />
        {busy && (
          <button
            type="button"
            className="button"
            disabled={!ready || !text.trim()}
            onClick={() => submit(true)}
          >
            Steer
          </button>
        )}
        <button type="submit" className="button button-primary" disabled={!ready || !text.trim()}>
          {busy ? 'Queue' : 'Send'}
        </button>
      </form>
      <p className="composer-hint">
        {busy
          ? 'enter queues · Steer (or /send …) reaches the turn after its next step · esc stops'
          : 'enter sends · shift+enter for a new line'}
      </p>
      {live.refused && (
        <p className="refused" role="alert">
          <Mark tone="alarm" glyph="✕">
            <Safe text={live.refused} />
          </Mark>
        </p>
      )}
    </div>
  );
}

/** The chat's state in words, for its head. */
function chip(live: Live): [string, 'holo' | 'muted' | 'gold' | 'alarm', string] {
  if (live.link === 'closed') return ['✕', 'alarm', 'ended'];
  if (live.link === 'reconnecting') return ['◌', 'gold', 'reconnecting…'];
  if (live.link === 'connecting') return ['◌', 'muted', 'connecting…'];
  if (live.asks.length > 0) return ['◆', 'gold', 'waiting for you'];
  const v = live.view;
  if (v?.busy) {
    const c = v.current;
    return ['●', 'holo', c ? `running ${c.name}` : 'running'];
  }
  return ['○', 'muted', 'idle'];
}

export function LiveChat({
  w,
  id,
  chats,
  onSide,
}: {
  w: BoardWorld;
  id: string;
  chats: Chats;
  onSide: (s: Side | undefined) => void;
}) {
  const live = useLive(id);
  // A chat that ends moves from the live list to the past runs.
  const refresh = chats.refresh;
  const closed = live.link === 'closed';
  useEffect(() => {
    if (closed) refresh();
  }, [closed, refresh]);
  const { view, hello } = live;
  useEffect(() => {
    onSide({ world: w.id, view, hello, live: true });
  }, [onSide, w.id, view, hello]);
  useEffect(() => () => onSide(undefined), [onSide]);
  const busy = view?.busy ?? false;
  const controller = live.role === 'controller' && live.link !== 'closed';
  const interrupt = live.interrupt;
  useEffect(() => {
    if (!busy || !controller) return;
    const on = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !e.defaultPrevented) void interrupt();
    };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, [busy, controller, interrupt]);
  useAnnouncements(view, live.asks);
  const [glyph, tone, word] = chip(live);
  const me = hello?.body ?? '';
  return (
    <div className="page page-chat">
      <header className="chat-head">
        <h1 className="chat-title">
          <Safe text={me || 'chat'} />
          {hello && (
            <span className="chat-model">
              {' · '}
              <Safe text={hello.model} />
            </span>
          )}
        </h1>
        <Mark tone={tone} glyph={glyph}>
          {word}
        </Mark>
        {controller && (
          <button type="button" className="button button-quiet" onClick={() => void live.end()}>
            End chat
          </button>
        )}
      </header>
      <div className="chat-body">
        {view ? (
          view.entries.length === 0 && !busy ? (
            <div className="chat-empty">
              <Empty glyph="✦" title="A new chat">
                Ask below: the body works in <Safe text={w.name} />.
              </Empty>
            </div>
          ) : (
            <TranscriptView
              view={view}
              me={me}
              tail={
                view.waiting !== undefined && view.busy ? (
                  <div className="quiet quiet-live">
                    <span aria-hidden="true">◌ </span>thinking
                  </div>
                ) : null
              }
            />
          )
        ) : (
          <div className="chat-empty">
            <Empty
              glyph="◌"
              title={live.link === 'closed' ? 'This chat is not live' : 'Connecting…'}
            >
              {live.closed ? <Safe text={live.closed} /> : 'Opening the chat over the board.'}
            </Empty>
          </div>
        )}
        <Toasts entries={view?.entries ?? []} />
      </div>
      <div className="chat-foot">
        <Approval live={live} />
        {view && <ActivityStrip view={view} asks={live.asks.length} controller={controller} />}
        <Composer live={live} />
      </div>
    </div>
  );
}

/** A past chat, from its run journal: read-only, with a resume when it stopped. */
export function PastChat({
  w,
  run,
  chats,
  onSide,
}: {
  w: BoardWorld;
  run: string;
  chats: Chats;
  onSide: (s: Side | undefined) => void;
}) {
  const load = useRun(w.id, run);
  const [view, setView] = useState<ChatView | undefined>(undefined);
  const [why, setWhy] = useState<string | undefined>(undefined);
  const [starting, setStarting] = useState(false);
  useEffect(() => {
    if (load.state !== 'ready') {
      setView(undefined);
      return;
    }
    const chat = new Chat('');
    for (const e of load.data.arrived) chat.fold(e);
    setView(chat.view());
  }, [load]);
  useEffect(() => {
    onSide({ world: w.id, view, hello: undefined, live: false });
  }, [onSide, w.id, view]);
  useEffect(() => () => onSide(undefined), [onSide]);
  const listed = chats.runs.find((r) => r.id === run);
  const state = load.state === 'ready' ? load.data.state : listed?.state;
  const liveChat = chats.live.find((s) => s.run === run);
  const resume = async () => {
    setStarting(true);
    setWhy(undefined);
    try {
      const started = await startChat(w.id, run);
      chats.refresh();
      window.location.hash = format({ name: 'session', world: w.id, session: started.session.id });
    } catch (e) {
      setWhy(e instanceof Error ? e.message : String(e));
    } finally {
      setStarting(false);
    }
  };
  return (
    <div className="page page-chat">
      <header className="chat-head">
        <h1 className="chat-title">
          <span className="mono">
            <Safe text={run} />
          </span>
        </h1>
        <Mark
          tone={state === 'stopped' ? 'gold' : state === 'live' ? 'holo' : 'muted'}
          glyph={state === 'live' ? '●' : '○'}
        >
          {state === 'live'
            ? 'live elsewhere'
            : state === 'stopped'
              ? `stopped · ${listed?.why ?? ''}`.replace(/ · $/, '')
              : state === 'ended'
                ? `ended · ${listed?.why ?? ''}`.replace(/ · $/, '')
                : 'past'}
        </Mark>
        {liveChat ? (
          <a
            className="button"
            href={format({ name: 'session', world: w.id, session: liveChat.id })}
          >
            Open the live chat
          </a>
        ) : (
          state === 'stopped' && (
            <button
              type="button"
              className="button button-primary"
              disabled={starting}
              onClick={() => void resume()}
            >
              {starting ? 'Resuming…' : 'Resume'}
            </button>
          )
        )}
      </header>
      <div className="chat-body">
        {load.state === 'loading' && (
          <div className="chat-empty">
            <Empty glyph="◌" title="Loading the run's journal…" />
          </div>
        )}
        {load.state === 'failed' && (
          <div className="chat-empty">
            <Empty glyph="✕" title="This run's journal could not be read">
              <Safe text={load.reason} />
            </Empty>
          </div>
        )}
        {view && <TranscriptView view={view} me="" />}
      </div>
      <div className="chat-foot">
        {load.state === 'ready' && (load.data.dropped > 0 || load.data.skipped.length > 0) && (
          <p className="hint">
            {load.data.dropped > 0 && `${load.data.dropped} older events not shown. `}
            {load.data.skipped.length > 0 &&
              `${load.data.skipped.length} journal line(s) did not read and are left out.`}
          </p>
        )}
        {why && (
          <p className="refused" role="alert">
            <Mark tone="alarm" glyph="✕">
              <Safe text={why} />
            </Mark>
          </p>
        )}
        <div className="composer composer-closed" aria-disabled="true">
          <span className="composer-label">read-only</span>
          <span className="composer-hint">
            A past chat, from its run journal{state === 'stopped' ? ': resume it to go on.' : '.'}
          </span>
        </div>
      </div>
    </div>
  );
}
