/**
 * The command palette (⌘K, Ctrl+K): type, and jump to a world, a page, a chat or a past run. A
 * dialog over the dimmed page with the focus held inside it; Esc, a click beside it or a jump
 * closes it and the focus goes back where it was. Each command only changes the address — the
 * palette answers no ask and acts on nothing — and it never opens while an approval card is up,
 * and closes if one arrives.
 */
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { type Command, type CommandKind, marked, rank } from '../data/commands.ts';
import { Safe } from '../safe/Safe.tsx';
import { onCard, trapTab } from './modal.ts';

const GLYPH: Record<CommandKind, string> = {
  page: '▤',
  world: '◈',
  section: '›',
  session: '●',
  run: '○',
};

export function CommandPalette({ all, onClose }: { all: readonly Command[]; onClose: () => void }) {
  const [query, setQuery] = useState('');
  const [at, setAt] = useState(0);
  const box = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const ids = useId();
  const found = useMemo(() => rank(all, query), [all, query]);
  const shown = Math.min(at, found.length - 1);
  const current = found[shown];
  const optionId = (i: number) => `${ids}-o${i}`;

  useEffect(() => {
    const was = document.activeElement as HTMLElement | null;
    input.current?.focus();
    return () => was?.focus?.();
  }, []);
  useEffect(() => onCard(onClose), [onClose]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new query starts at the top
  useEffect(() => setAt(0), [query]);
  useEffect(() => {
    if (!current) return;
    document.getElementById(optionId(shown))?.scrollIntoView?.({ block: 'nearest' });
  });

  const go = (c: Command) => {
    onClose();
    window.location.hash = c.href;
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (trapTab(e, box.current)) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      setAt((i) => (found.length === 0 ? 0 : (Math.min(i, found.length - 1) + 1) % found.length));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setAt((i) =>
        found.length === 0 ? 0 : (Math.min(i, found.length - 1) - 1 + found.length) % found.length,
      );
    } else if (e.key === 'Home' && e.ctrlKey) {
      e.preventDefault();
      setAt(0);
    } else if (e.key === 'End' && e.ctrlKey) {
      e.preventDefault();
      setAt(Math.max(0, found.length - 1));
    } else if (e.key === 'Enter' && !e.nativeEvent.isComposing) {
      e.preventDefault();
      if (current) go(current.command);
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
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        onKeyDown={onKey}
      >
        <div className="palette-search">
          <span aria-hidden="true">⌕</span>
          <input
            ref={input}
            className="palette-input"
            type="text"
            role="combobox"
            aria-label="Jump to a world, a page, a chat or a run"
            aria-expanded="true"
            aria-controls={`${ids}-list`}
            aria-autocomplete="list"
            aria-activedescendant={current ? optionId(shown) : undefined}
            placeholder="Jump to…"
            autoComplete="off"
            spellCheck={false}
            maxLength={200}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <kbd>Esc</kbd>
        </div>
        <div className="palette-list" id={`${ids}-list`} role="listbox" aria-label="Matches">
          {found.map(({ command: c, at: hits }, i) => {
            const selected = i === shown;
            return (
              // The focus stays in the box; ↑ and ↓ move the active option, a click picks it.
              // biome-ignore lint/a11y/useKeyWithClickEvents: the box's keys pick it
              <div
                key={c.id}
                id={optionId(i)}
                className="palette-item"
                role="option"
                tabIndex={-1}
                aria-selected={selected}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => go(c)}
                onMouseMove={() => {
                  if (i !== shown) setAt(i);
                }}
              >
                <span className="palette-glyph" aria-hidden="true">
                  {GLYPH[c.kind]}
                </span>
                <span className="palette-label">
                  {marked(c.label, hits).map((r, i) =>
                    r.hit ? (
                      // biome-ignore lint/suspicious/noArrayIndexKey: runs never reorder
                      <mark key={i} className="palette-hit">
                        <Safe text={r.text} />
                      </mark>
                    ) : (
                      // biome-ignore lint/suspicious/noArrayIndexKey: runs never reorder
                      <Safe key={i} text={r.text} />
                    ),
                  )}
                </span>
                <span className="palette-detail">
                  <Safe text={c.detail} />
                </span>
              </div>
            );
          })}
        </div>
        {found.length === 0 && (
          <p className="palette-none" role="status">
            Nothing matches <Safe text={query.slice(0, 120)} />.
          </p>
        )}
        <p className="palette-foot" aria-hidden="true">
          <kbd>↑</kbd> <kbd>↓</kbd> to move · <kbd>Enter</kbd> to go · it only moves the page
        </p>
      </div>
    </div>
  );
}
