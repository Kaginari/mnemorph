/**
 * A confirm dialog for a human act: it names exactly what the act is on — the note or fact, by
 * id, with its text — over the dimmed page. Focus starts on Cancel, never on the act, and stays
 * inside; Esc or Cancel closes it, and a stray click beside it does nothing. While the act runs,
 * nothing can be pressed twice; a refusal stays in the dialog with the server's words.
 */
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { Safe } from '../safe/Safe.tsx';
import { Mark } from './Mark.tsx';

export function Confirm({
  title,
  verb,
  children,
  ready = true,
  onAct,
  onClose,
}: {
  /** What the act is, as a heading: "Confirm this claim". */
  title: string;
  /** The act's button: "Confirm", "Wake", "Settle". */
  verb: string;
  /** What it is on, and anything the act needs (a proof). */
  children: ReactNode;
  /** Whether the act may be pressed yet: a settle waits for its proof. */
  ready?: boolean;
  /** Does the act; resolves with why it was refused, or nothing when done. */
  onAct: () => Promise<string | undefined>;
  onClose: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();

  useEffect(() => {
    const was = document.activeElement as HTMLElement | null;
    cancel.current?.focus();
    return () => was?.focus?.();
  }, []);

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      if (!busy) onClose();
      return;
    }
    if (e.key !== 'Tab') return;
    const all = [
      ...(box.current?.querySelectorAll<HTMLElement>(
        'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled])',
      ) ?? []),
    ];
    const first = all[0];
    const last = all[all.length - 1];
    if (!first || !last) return;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  const go = async () => {
    setBusy(true);
    setRefused(undefined);
    const no = await onAct();
    setBusy(false);
    if (no === undefined) onClose();
    else setRefused(no);
  };

  return (
    <div className="confirm-scrim">
      <div
        ref={box}
        className="confirm"
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        onKeyDown={onKey}
      >
        <h2 id="confirm-title" className="confirm-title">
          {title}
        </h2>
        <div className="confirm-body">{children}</div>
        {refused && (
          <p className="confirm-refused" role="alert">
            <Mark tone="alarm" glyph="✕">
              Refused: <Safe text={refused} />
            </Mark>
          </p>
        )}
        <div className="confirm-actions">
          <button ref={cancel} type="button" className="button" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button
            type="button"
            className="button button-act"
            onClick={go}
            disabled={busy || !ready}
          >
            {busy ? `${verb}…` : verb}
          </button>
        </div>
      </div>
    </div>
  );
}
