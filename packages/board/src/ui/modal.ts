/**
 * What the board's own dialogs share: whether an approval card (or another dialog) holds the page
 * — then no shortcut opens anything over it — and the focus trap.
 */

/** The approval card is on the page, or has made the page behind it inert. */
export function cardUp(): boolean {
  return (
    document.querySelector('.approval-backdrop') !== null ||
    document.getElementById('root')?.inert === true
  );
}

/** A dialog already holds the page: the approval card, a confirm, the palette or the map. */
export function dialogUp(): boolean {
  return cardUp() || document.querySelector('[aria-modal="true"]') !== null;
}

/** Whether a key went to a place that takes text: a shortcut there is the text's. */
export function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  if (tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if (tag !== 'INPUT') return false;
  const type = (target as HTMLInputElement).type;
  return !['button', 'checkbox', 'radio', 'submit', 'reset', 'range', 'color'].includes(type);
}

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Tab and Shift+Tab kept inside `box`. Returns whether the key was the trap's. */
export function trapTab(e: React.KeyboardEvent, box: HTMLElement | null): boolean {
  if (e.key !== 'Tab' || !box) return false;
  const all = [...box.querySelectorAll<HTMLElement>(FOCUSABLE)];
  const first = all[0];
  const last = all[all.length - 1];
  if (!first || !last) {
    e.preventDefault();
    return true;
  }
  const at = document.activeElement;
  if (e.shiftKey && (at === first || !box.contains(at))) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && (at === last || !box.contains(at))) {
    e.preventDefault();
    first.focus();
  }
  return true;
}

/**
 * Calls `close` when an approval card arrives while a dialog of the board's is open: the card
 * takes the page, and nothing of the board's stays over or under it.
 */
export function onCard(close: () => void): () => void {
  const mo = new MutationObserver(() => {
    if (cardUp()) close();
  });
  mo.observe(document.body, { childList: true });
  const root = document.getElementById('root');
  if (root) mo.observe(root, { attributes: true, attributeFilter: ['inert'] });
  return () => mo.disconnect();
}
