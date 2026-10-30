/**
 * The approval card: the TUI's card, on the web. A modal over the dimmed chat, the rest of the
 * page inert behind it, showing the act whole as the board's `shown` wrote it — text nodes only,
 * every escape set apart — with its class, why it asks, what lies outside the world, the time
 * left and the asks waiting behind it. Its rules are `data/approval.ts`'s; the board holds the
 * ones it can see again, whatever this page does.
 *
 * Nothing here approves without the card: no "always allow", no approving from a notification,
 * no batch, no key that answers before the card is armed. Yes is never the default focus.
 */
import {
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';
import {
  ANSWERS,
  actRows,
  askDigest,
  Card,
  type Decision,
  escapes,
  type Pick,
} from '../data/approval.ts';
import type { Live, Waiting } from '../data/live.ts';
import { Safe } from '../safe/Safe.tsx';
import { Mark } from './Mark.tsx';

/** The core denies an ask after this long when the board says nothing of its time. */
const GATE_WAIT_MS = 10 * 60 * 1000;

/** The minutes and seconds an ask has left: `9:41`. */
export function left(ms: number): string {
  const s = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/** Text as the board's `shown` wrote it: each escape set apart, every run a text node. */
export function Shown({ text }: { text: string }) {
  return (
    <>
      {escapes(text).map((r, i) =>
        r.escaped ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: runs never reorder
          <span key={i} className="safe-esc" title="a hidden character, escaped">
            {r.text}
          </span>
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: runs never reorder
          <Safe key={i} text={r.text} />
        ),
      )}
    </>
  );
}

/** A counter that moves once per task: two events in one task read the same number. */
let task = 0;
let ticking = false;
const channel = typeof MessageChannel === 'undefined' ? undefined : new MessageChannel();
if (channel) {
  channel.port1.onmessage = () => {
    task += 1;
    ticking = false;
  };
}
function thisTask(): number {
  if (!ticking) {
    ticking = true;
    if (channel) channel.port2.postMessage(0);
    else
      setTimeout(() => {
        task += 1;
        ticking = false;
      }, 0);
  }
  return task;
}

/** Whether the page is where a human can see it: the tab shown and the window focused. */
const pageSeen = () => document.visibilityState === 'visible' && document.hasFocus();

function usePageSeen(): boolean {
  const read = pageSeen;
  const [seen, setSeen] = useState(read);
  useEffect(() => {
    const on = () => setSeen(read());
    document.addEventListener('visibilitychange', on);
    window.addEventListener('focus', on);
    window.addEventListener('blur', on);
    return () => {
      document.removeEventListener('visibilitychange', on);
      window.removeEventListener('focus', on);
      window.removeEventListener('blur', on);
    };
  }, []);
  return seen;
}

/** The asks waiting, the first on the card: the card for the chat. */
export function Approval({ live }: { live: Live }) {
  const first = live.asks[0];
  const gone = live.gone;
  const [note, setNote] = useState<string | undefined>(undefined);
  useEffect(() => {
    if (!gone || gone.here) return;
    setNote(
      gone.why === 'answered'
        ? 'The ask was answered elsewhere.'
        : `The ask is gone: ${gone.why.replace(/^withdrawn: |^denied: /, '')}.`,
    );
    const t = window.setTimeout(() => setNote(undefined), 6000);
    return () => window.clearTimeout(t);
  }, [gone]);
  if (!first) {
    return note ? (
      <output className="approval-note">
        <Mark tone="muted" glyph="◇">
          {note}
        </Mark>
      </output>
    ) : null;
  }
  return createPortal(
    <ApprovalCard key={first.id} live={live} waiting={first} more={live.asks.length - 1} />,
    document.body,
  );
}

function ApprovalCard({ live, waiting, more }: { live: Live; waiting: Waiting; more: number }) {
  const { id, ask: a } = waiting;
  const rows = useMemo(() => actRows(a.act), [a.act]);
  const card = useRef(new Card(rows.length)).current;
  const [, redraw] = useReducer((n: number) => n + 1, 0);
  const controller = live.role === 'controller' && live.link !== 'closed';
  const [digest, setDigest] = useState<'checking' | 'ok' | 'bad'>('checking');
  const [reason, setReason] = useState<string | undefined>(undefined);
  const [sent, setSent] = useState(false);
  const [now, setNow] = useState(() => Date.now());
  const dialog = useRef<HTMLDivElement>(null);
  const viewport = useRef<HTMLElement>(null);
  const reasonBox = useRef<HTMLInputElement>(null);
  const [inView, setInView] = useState(false);
  const pageSeen = usePageSeen();
  const visible = inView && pageSeen;

  // The act's digest, over exactly what is rendered: the board's must match it.
  useEffect(() => {
    let on = true;
    askDigest(id, a)
      .then((d) => on && setDigest(d === a.digest ? 'ok' : 'bad'))
      .catch(() => on && setDigest('bad'));
    return () => {
      on = false;
    };
  }, [id, a]);

  // The page behind the card is inert: no click, key or focus reaches it.
  useEffect(() => {
    const root = document.getElementById('root');
    if (!root) return;
    root.inert = true;
    return () => {
      root.inert = false;
    };
  }, []);

  // The card in the viewport.
  useEffect(() => {
    const el = dialog.current;
    if (!el) return;
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) setInView(e.isIntersecting && e.intersectionRatio >= 0.5);
      },
      { threshold: [0, 0.5, 1] },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  useEffect(() => {
    card.visible(visible, performance.now());
    redraw();
  }, [visible, card]);

  // The render acknowledgement: once the card is first visible to the controller, and again after
  // control comes back. Arming waits for the board to take it.
  const acking = useRef(false);
  useEffect(() => {
    if (!controller) {
      acking.current = false;
      card.acked(false, performance.now());
      redraw();
      return;
    }
    if (!visible || digest !== 'ok' || acking.current || !a.digest) return;
    acking.current = true;
    live
      .seen(id, a.digest)
      .then(() => {
        card.acked(true, performance.now());
        redraw();
      })
      .catch(() => {
        acking.current = false;
      });
  }, [controller, visible, digest, id, a.digest, live, card]);

  // Redraw when the card arms, and every second for the time left.
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, []);
  useEffect(() => {
    if (card.armsAt === undefined) return;
    const wait = card.armsAt - performance.now();
    if (wait <= 0) return;
    const t = window.setTimeout(redraw, wait + 5);
    return () => window.clearTimeout(t);
  });

  // Each act row, read in order: an observer per row, inside the act's viewport.
  useEffect(() => {
    const root = viewport.current;
    if (!root) return;
    const shown = new Set<number>();
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          const i = Number((e.target as HTMLElement).dataset.row);
          const whole =
            e.intersectionRatio >= 0.99 ||
            (e.rootBounds !== null && e.intersectionRect.height >= e.rootBounds.height - 2);
          if (e.isIntersecting && whole) shown.add(i);
          else shown.delete(i);
        }
        const before = card.seenTo;
        while (shown.has(card.seenTo)) card.rowSeen(card.seenTo);
        if (card.seenTo !== before) redraw();
      },
      { root, threshold: [0, 0.5, 0.99, 1] },
    );
    for (const el of root.querySelectorAll('[data-row]')) io.observe(el);
    return () => io.disconnect();
  }, [card]);

  // Focus starts on the act, never on Yes.
  useEffect(() => {
    viewport.current?.focus();
  }, []);
  useEffect(() => {
    if (reason !== undefined) reasonBox.current?.focus();
  }, [reason]);

  const remaining = (a.leftMs ?? GATE_WAIT_MS) - (now - waiting.seen);
  const send = useCallback(
    (allow: boolean, why?: string) => {
      if (!controller || sent) return;
      setSent(true);
      live.answer(
        id,
        allow && a.digest
          ? { allow: true, digest: a.digest }
          : { allow: false, reason: why ?? 'the human said no' },
      );
    },
    [controller, sent, live, id, a.digest],
  );

  // The deadline denies.
  useEffect(() => {
    if (remaining <= 0 && controller) send(false, 'the human did not answer in time');
  }, [remaining, controller, send]);

  // A refused answer: the ask still waits, and the card asks again.
  const refused = live.askRefused?.id === id ? live.askRefused.why : undefined;
  useEffect(() => {
    if (refused) {
      setSent(false);
      card.disarm(performance.now());
      redraw();
    }
  }, [refused, card]);

  const decide = (d: Decision) => {
    if (d.kind === 'allow') send(true);
    else if (d.kind === 'deny') send(false, d.reason);
    else if (d.kind === 'reason') setReason('');
    redraw();
  };

  const focusables = () =>
    Array.from(
      dialog.current?.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input, [tabindex="0"]',
      ) ?? [],
    );

  const onKeyDown = (e: React.KeyboardEvent) => {
    // No key on the card reaches the chat behind it.
    e.stopPropagation();
    if (e.key === 'Tab') {
      // The focus stays on the card.
      const all = focusables();
      const i = all.indexOf(document.activeElement as HTMLElement);
      const next = all[(i + (e.shiftKey ? -1 : 1) + all.length) % all.length];
      e.preventDefault();
      next?.focus();
      return;
    }
    if (reason !== undefined) {
      if (e.key === 'Escape') {
        e.preventDefault();
        send(false);
      } else if (e.key === 'Enter' && e.nativeEvent.isTrusted) {
        e.preventDefault();
        const why = reason.trim();
        send(false, why ? `the human said no: ${why}` : 'the human said no');
      }
      return;
    }
    if (e.key === 'Enter' || e.key === 'Escape' || e.key.length === 1) e.preventDefault();
    // A key the page's own script made is no human's.
    if (!e.nativeEvent.isTrusted) return;
    if (!controller && e.key !== 'Escape') return;
    decide(card.key(e.key, e.timeStamp, thisTask()));
  };

  const onPaste = (e: React.ClipboardEvent) => {
    if (reason !== undefined) return;
    e.preventDefault();
    card.pasted(performance.now());
    redraw();
  };

  // A pointer's click: never a key's (`detail` 0), never a script's.
  const clicked = (p: Pick) => (e: React.MouseEvent) => {
    if (!e.nativeEvent.isTrusted || e.detail === 0) return;
    decide(card.click(p, e.timeStamp));
  };

  const at = performance.now();
  const armed = controller && card.armed(at) && digest === 'ok' && !sent;
  const yesReady = armed && card.viewed;
  let status: ReactNode;
  if (!controller) status = 'Waiting in another tab: take over to answer here.';
  else if (digest === 'bad')
    status =
      'The act on this card does not match the one the board sent: it cannot be approved here.';
  else if (sent) status = 'Answer sent: waiting for the board…';
  else if (!visible) status = 'Disarmed: the card is not in view.';
  else if (!card.armed(at)) status = 'Arming…';
  else if (!card.viewed) status = 'Read the whole act to enable Yes.';
  else if (card.choice !== undefined) status = `${ANSWERS[card.choice]}: Enter to confirm.`;
  else status = 'Pick an answer: 1 (y), 2 (n) or 3, then Enter. Esc denies.';

  return (
    <div className="approval-backdrop">
      <div
        ref={dialog}
        className="approval"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="approval-title"
        aria-describedby="approval-status"
        data-armed={armed ? 'true' : 'false'}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
      >
        <h2 className="approval-title" id="approval-title">
          <Mark tone="gold" glyph="◆">
            <Shown text={a.body} /> (<Shown text={a.race} />) wants to run{' '}
            {a.class === 'outward' ? 'an' : 'a'} <Shown text={a.class} /> act
          </Mark>
          {more > 0 && <span className="approval-more">{more} more waiting</span>}
        </h2>
        <section
          ref={viewport}
          className="approval-act mono"
          // biome-ignore lint/a11y/noNoninteractiveTabindex: the act scrolls with the keys
          tabIndex={0}
          aria-label="The act, whole"
        >
          {rows.map((r, i) => (
            <div
              // biome-ignore lint/suspicious/noArrayIndexKey: the rows never reorder
              key={i}
              className="approval-row"
              data-row={i}
              data-seen={i < card.seenTo ? 'true' : 'false'}
            >
              <Shown text={r} />
            </div>
          ))}
        </section>
        <p className="approval-read">
          read {Math.min(card.seenTo, rows.length)} of {rows.length} row
          {rows.length === 1 ? '' : 's'}
        </p>
        {a.why.length > 0 && (
          <ul className="approval-why" aria-label="Why it asks">
            {a.why.map((w, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: the reasons never reorder
              <li key={i}>
                · <Shown text={w} />
              </li>
            ))}
          </ul>
        )}
        {a.outside.length > 0 && (
          <ul className="approval-outside" aria-label="Outside the world">
            {a.outside.map((p, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: the paths never reorder
              <li key={i}>
                <Mark tone="alarm" glyph="⚠">
                  approving lets it write <Shown text={p} />
                </Mark>
              </li>
            ))}
          </ul>
        )}
        {a.plan && (
          <p className="approval-plan mono">
            bound to <Shown text={a.plan} />
          </p>
        )}
        {reason !== undefined ? (
          <label className="approval-reason">
            <span>Tell the model why (Enter sends, Esc denies without it)</span>
            <input
              ref={reasonBox}
              className="composer-text"
              value={reason}
              maxLength={400}
              onChange={(e) => setReason(e.target.value)}
            />
          </label>
        ) : controller ? (
          <div className="approval-answers">
            {ANSWERS.map((label, i) => (
              <button
                key={label}
                type="button"
                className="button"
                aria-pressed={card.choice === i}
                disabled={i === 0 ? !yesReady : !armed}
                onClick={clicked(i as Pick)}
              >
                <span className="mono">{i + 1}</span> {label}
              </button>
            ))}
          </div>
        ) : (
          <div className="approval-answers">
            <button type="button" className="button" onClick={() => void live.takeOver()}>
              Take over
            </button>
          </div>
        )}
        <p className="approval-status" id="approval-status" aria-live="polite">
          {status}
          {card.unread &&
            card.choice === undefined &&
            controller &&
            ' Yes waits until every row has been read.'}
        </p>
        {refused && (
          <p className="refused" role="alert">
            <Mark tone="alarm" glyph="✕">
              The board refused the answer: <Safe text={refused} />
            </Mark>
          </p>
        )}
        <p className="approval-left">
          Denied in <span className="mono">{left(remaining)}</span> if left alone.
        </p>
      </div>
    </div>
  );
}
