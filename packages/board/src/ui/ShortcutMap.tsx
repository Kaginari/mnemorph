/**
 * The shortcut map (`?`): every shortcut the board has, from `data/shortcuts.ts`. A dialog with
 * the focus held inside it, closed by Esc, its button or a click beside it. No shortcut on it
 * approves anything.
 */
import { useEffect, useRef } from 'react';
import { shortcuts } from '../data/shortcuts.ts';
import { onCard, trapTab } from './modal.ts';

export function ShortcutMap({ onClose }: { onClose: () => void }) {
  const box = useRef<HTMLDivElement>(null);
  const close = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const was = document.activeElement as HTMLElement | null;
    close.current?.focus();
    return () => was?.focus?.();
  }, []);
  useEffect(() => onCard(onClose), [onClose]);
  const onKey = (e: React.KeyboardEvent) => {
    if (trapTab(e, box.current)) return;
    if (e.key === 'Escape' || e.key === '?') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  };
  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: a click beside the dialog closes it
    <div
      className="modal-scrim"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={box}
        className="shortcut-map"
        role="dialog"
        aria-modal="true"
        aria-labelledby="shortcut-title"
        onKeyDown={onKey}
      >
        <header className="shortcut-head">
          <h2 id="shortcut-title" className="shortcut-title">
            Keyboard shortcuts
          </h2>
          <button ref={close} type="button" className="button" onClick={onClose}>
            Close
          </button>
        </header>
        <div
          className="shortcut-groups"
          // biome-ignore lint/a11y/noNoninteractiveTabindex: the list scrolls with the keys
          tabIndex={0}
        >
          {shortcuts().map((g) => (
            <section key={g.title} className="shortcut-group" aria-label={g.title}>
              <h3 className="shortcut-group-title">{g.title}</h3>
              <dl className="shortcut-list">
                {g.items.map((s) => (
                  <div key={`${s.keys.join('+')} ${s.does}`} className="shortcut-row">
                    <dt>
                      {s.keys.map((k, i) => (
                        <span key={k}>
                          {i > 0 && <span aria-hidden="true">+</span>}
                          <kbd>{k}</kbd>
                        </span>
                      ))}
                    </dt>
                    <dd>{s.does}</dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>
        <p className="shortcut-note">
          No shortcut approves anything. An approval card takes only its own answers, on the card,
          once it is armed — and while one is up, neither this map nor the palette opens.
        </p>
      </div>
    </div>
  );
}
