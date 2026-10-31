/**
 * The board's live regions: what a screen reader is told as it happens, once — a chat's answer
 * arriving, an ask waiting at the gate. Two regions, polite and assertive, always on the page
 * (a region added with its text is not read). The text is a model's or a tool's: rendered inert.
 */
import { useSyncExternalStore } from 'react';
import { Safe } from '../safe/Safe.tsx';

type Said = { polite: string; assertive: string; n: number };

let said: Said = { polite: '', assertive: '', n: 0 };
const listeners = new Set<() => void>();

/** The most characters one announcement reads. */
export const ANNOUNCE_CAP = 300;

/** Tell the screen reader `text`; `urgent` interrupts what it is reading. */
export function announce(text: string, urgent = false) {
  const t = text.length > ANNOUNCE_CAP ? `${text.slice(0, ANNOUNCE_CAP)}…` : text;
  said = urgent ? { ...said, assertive: t, n: said.n + 1 } : { ...said, polite: t, n: said.n + 1 };
  for (const l of listeners) l();
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

export function Announcer() {
  const s = useSyncExternalStore(
    subscribe,
    () => said,
    () => said,
  );
  return (
    <>
      <div className="sr-only" aria-live="polite" aria-atomic="true" data-testid="announce-polite">
        {s.polite && <Safe key={s.n} text={s.polite} />}
      </div>
      <div
        className="sr-only"
        aria-live="assertive"
        aria-atomic="true"
        data-testid="announce-assertive"
      >
        {s.assertive && <Safe key={s.n} text={s.assertive} />}
      </div>
    </>
  );
}
