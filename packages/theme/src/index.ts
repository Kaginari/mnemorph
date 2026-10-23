/**
 * The Mnemorph theme for TypeScript: the CLI (Ink takes hex colours) and anything that cannot
 * read CSS. The same values live in tokens.css; the test keeps them equal.
 */

export interface Palette {
  bg: string;
  panel: string;
  panelRaised: string;
  line: string;
  text: string;
  textMuted: string;
  holo: string;
  alarm: string;
  gold: string;
}

export const dark: Palette = {
  bg: '#060b12',
  panel: '#0d141f',
  panelRaised: '#142033',
  line: '#1f3346',
  text: '#e0e6ed',
  textMuted: '#a1aab5',
  holo: '#00f2ff',
  alarm: '#ff2a7d',
  gold: '#f7d002',
};

export const light: Palette = {
  bg: '#f3f6fa',
  panel: '#ffffff',
  panelRaised: '#e9eff6',
  line: '#c9d5e2',
  text: '#0d141f',
  textMuted: '#46505c',
  holo: '#00707a',
  alarm: '#c0004c',
  gold: '#7a5c00',
};

/** One colour per race and form: the cards' frames, the board's lanes, the CLI's prefixes. */
export const races = {
  rimuru: '#3fa9f5',
  elf: '#2fa36b',
  orc: '#c9822b',
  slime: '#19c3b0',
  kijin: '#d7263d',
  darkElf: '#7b4fd6',
  highElf: '#f5e6b8',
  highOrc: '#e8a83c',
  namedSlime: '#a0e8f0',
  oni: '#e8b04a',
  archDarkElf: '#c4cce4',
  veldora: '#d4af37',
} as const;

export type RaceKey = keyof typeof races;

/** WCAG relative luminance of a #rrggbb colour. */
export function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = Number.parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio between two #rrggbb colours, from 1 to 21. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}
