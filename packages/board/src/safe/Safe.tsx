import { REL, safeHref } from './links.ts';
import { runs } from './shown.ts';

/**
 * Text a model or a tool wrote, rendered inert: React text nodes only, every hidden character
 * shown as its `\u{…}` escape, set apart so it reads as an escape. `lines` keeps newlines (prose).
 */
export function Safe({ text, lines = false }: { text: string; lines?: boolean }) {
  const parts = runs(text, lines);
  return (
    <span className={lines ? 'safe safe-lines' : 'safe'}>
      {parts.map((r, i) =>
        r.escaped ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: runs never reorder
          <span key={i} className="safe-esc" title="a hidden character, escaped">
            {r.text}
          </span>
        ) : (
          r.text
        ),
      )}
    </span>
  );
}

/**
 * A link from model or tool text: followed only to http, https or mailto, in a new tab with no
 * opener and no referrer. Any other target is shown as text, with the reason.
 */
export function SafeLink({ href, text }: { href: string; text?: string }) {
  const to = safeHref(href);
  if (!to) {
    return (
      <span className="safe-blocked">
        <Safe text={text ?? href} /> <span className="safe-note">(link not followed)</span>
      </span>
    );
  }
  return (
    <a href={to} rel={REL} target="_blank">
      <Safe text={text ?? href} />
    </a>
  );
}

/**
 * An image from model or tool text: never loaded (a load is a request the model chose — a beacon
 * or a leak). Its description and address are shown as text.
 */
export function SafeImage({ src, alt }: { src: string; alt?: string }) {
  return (
    <span className="safe-blocked">
      <span className="safe-note">image not loaded:</span> <Safe text={alt || src} />
    </span>
  );
}
