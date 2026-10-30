/**
 * The checks every end-to-end page passes: no CSP violation, rendered text at WCAG AA contrast,
 * a label on every coloured node.
 */
import type { Page } from '@playwright/test';

/** The CSP violations the page has reported. */
export function violations(page: Page) {
  return page.evaluate(() => (window as { __csp?: string[] }).__csp ?? ['no CSP listener']);
}

/**
 * Every visible text node whose colour misses WCAG AA on what is behind it: 4.5:1, or 3:1 for
 * large text. Backgrounds are composited up the tree over the page's own.
 */
export function lowContrast(page: Page) {
  return page.evaluate(() => {
    type C = [number, number, number, number];
    const parse = (s: string): C | undefined => {
      const n = (s.match(/-?[\d.]+(?:e-?\d+)?/g) ?? []).map(Number);
      const [r = 0, g = 0, b = 0, a = 1] = n;
      if (s === 'transparent') return [0, 0, 0, 0];
      if (s.startsWith('color')) return [r * 255, g * 255, b * 255, a];
      if (s.startsWith('rgb') && n.length >= 3) return [r, g, b, a];
      return undefined;
    };
    const over = (top: C, under: C): C => {
      const a = top[3] + under[3] * (1 - top[3]);
      if (a === 0) return [0, 0, 0, 0];
      const m = (i: 0 | 1 | 2) => (top[i] * top[3] + under[i] * under[3] * (1 - top[3])) / a;
      return [m(0), m(1), m(2), a];
    };
    const lum = (c: C) => {
      const ch = (v: number) => {
        const x = v / 255;
        return x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
      };
      return 0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2]);
    };
    const backdrop = (el: Element | null): C => {
      const layers: C[] = [];
      for (let e = el; e; e = e.parentElement) {
        const c = parse(getComputedStyle(e).backgroundColor);
        if (c && c[3] > 0) layers.push(c);
        if (c && c[3] >= 1) break;
      }
      let acc: C = [255, 255, 255, 1];
      for (const l of layers.reverse()) acc = over(l, acc);
      return acc;
    };
    const out: string[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const el = n.parentElement;
      if (!el || !n.textContent?.trim()) continue;
      const r = el.getBoundingClientRect();
      const cs = getComputedStyle(el);
      if (r.width === 0 || r.height === 0 || cs.visibility === 'hidden') continue;
      if (el.closest('[hidden]')) continue;
      const fg = parse(cs.color);
      if (!fg) {
        out.push(`${n.textContent.trim()}: unreadable colour ${cs.color}`);
        continue;
      }
      if (fg[3] === 0) continue; // gradient text: the wordmark, a logotype
      const bg = backdrop(el);
      const shown = over(fg, bg);
      const [hi, lo] = [lum(shown), lum(bg)].sort((a, b) => b - a) as [number, number];
      const ratio = (hi + 0.05) / (lo + 0.05);
      const px = Number.parseFloat(cs.fontSize);
      const large = px >= 24 || (px >= 18.66 && Number(cs.fontWeight) >= 700);
      const disabled = el.closest(':disabled, [aria-disabled="true"]') !== null;
      if (!disabled && ratio < (large ? 3 : 4.5)) {
        out.push(`${n.textContent.trim().slice(0, 40)}: ${ratio.toFixed(2)}`);
      }
    }
    return out;
  });
}

/** Coloured nodes without words beside them. */
export function unlabelled(page: Page) {
  return page.$$eval('[data-tone], [data-race]', (els) =>
    els
      .filter((e) => !(e.querySelector('.mark-label')?.textContent ?? '').trim())
      .map((e) => `${e.tagName} ${e.className}`),
  );
}
