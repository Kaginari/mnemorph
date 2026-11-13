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
  summoner: '#3fa9f5',
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
  player: '#d4af37',
} as const;

export type RaceKey = keyof typeof races;

/**
 * A colour scheme: the board and the terminal app wear the same ones. `hud` is the palette above
 * (`dark`, `light`); the others are built on four-colour palettes, their values derived in the
 * terminal app's `theme.rs` — a test there keeps the two equal, and prints these on a change.
 */
export interface SchemePalette extends Palette {
  /** The human's colour: their lines, the ask, the gate. */
  player: string;
}

export const SCHEMES = ['neon', 'ember', 'lagoon', 'harbor', 'dune', 'spice', 'hud'] as const;
export type SchemeName = (typeof SCHEMES)[number];
/** The scheme worn unless another is asked for. */
export const DEFAULT_SCHEME: SchemeName = 'neon';

export const neonDark: SchemePalette = {
  bg: '#0d0a23',
  panel: '#161234',
  panelRaised: '#1e1b4a',
  line: '#2e256b',
  text: '#e2e5e1',
  textMuted: '#9796b6',
  holo: '#ff467a',
  alarm: '#fe4439',
  gold: '#ffd51e',
  player: '#ffd51e',
};

export const neonLight: SchemePalette = {
  bg: '#f4f3e4',
  panel: '#fffdf4',
  panelRaised: '#dddcf2',
  line: '#aba1da',
  text: '#14122f',
  textMuted: '#474866',
  holo: '#923051',
  alarm: '#ae2e2b',
  gold: '#695d1f',
  player: '#695d1f',
};

export const emberDark: SchemePalette = {
  bg: '#1d1410',
  panel: '#2a1e1b',
  panelRaised: '#372b2b',
  line: '#5d4131',
  text: '#e1e5e8',
  textMuted: '#aaa19f',
  holo: '#ff9a00',
  alarm: '#ed7566',
  gold: '#f7d794',
  player: '#f7d794',
};

export const emberLight: SchemePalette = {
  bg: '#f3f3f0',
  panel: '#fffdfa',
  panelRaised: '#e9e4e2',
  line: '#d2b8aa',
  text: '#241c1c',
  textMuted: '#575253',
  holo: '#87570f',
  alarm: '#ad362a',
  gold: '#665e4b',
  player: '#665e4b',
};

export const lagoonDark: SchemePalette = {
  bg: '#09161c',
  panel: '#11212a',
  panelRaised: '#172e3d',
  line: '#224854',
  text: '#e2e7e9',
  textMuted: '#92a4ad',
  holo: '#aeeed3',
  alarm: '#ff6f61',
  gold: '#fff8b0',
  player: '#fff8b0',
};

export const lagoonLight: SchemePalette = {
  bg: '#f4f6f3',
  panel: '#fffffb',
  panelRaised: '#dae6ec',
  line: '#a1bec7',
  text: '#101e28',
  textMuted: '#43545e',
  holo: '#486663',
  alarm: '#9d4c47',
  gold: '#606351',
  player: '#606351',
};

export const harborDark: SchemePalette = {
  bg: '#080f1a',
  panel: '#0f1827',
  panelRaised: '#15243a',
  line: '#1e344d',
  text: '#dce6ed',
  textMuted: '#919caa',
  holo: '#60c1e5',
  alarm: '#ff5c7a',
  gold: '#a0e0ef',
  player: '#a0e0ef',
};

export const harborLight: SchemePalette = {
  bg: '#ebf4f9',
  panel: '#fafdfe',
  panelRaised: '#d9e0ea',
  line: '#9eadc1',
  text: '#0e1725',
  textMuted: '#424d5c',
  holo: '#34657d',
  alarm: '#9d3f56',
  gold: '#45626e',
  player: '#45626e',
};

export const duneDark: SchemePalette = {
  bg: '#111a24',
  panel: '#1a2735',
  panelRaised: '#23354b',
  line: '#38556e',
  text: '#e2e4e4',
  textMuted: '#9ba9b8',
  holo: '#ff933e',
  alarm: '#e48689',
  gold: '#ffcb56',
  player: '#ffcb56',
};

export const duneLight: SchemePalette = {
  bg: '#f4f2ea',
  panel: '#fffcf7',
  panelRaised: '#e0e9f2',
  line: '#b4c9dc',
  text: '#172330',
  textMuted: '#4b5967',
  holo: '#8c5730',
  alarm: '#a4383f',
  gold: '#695a34',
  player: '#695a34',
};

export const spiceDark: SchemePalette = {
  bg: '#0a191f',
  panel: '#12252f',
  panelRaised: '#183343',
  line: '#24515f',
  text: '#e2e2df',
  textMuted: '#93a8b2',
  holo: '#3cb8b8',
  alarm: '#e57d77',
  gold: '#ff9f00',
  player: '#ff9f00',
};

export const spiceLight: SchemePalette = {
  bg: '#f4ede1',
  panel: '#fffaf2',
  panelRaised: '#dae8ee',
  line: '#a3c6d0',
  text: '#11212b',
  textMuted: '#445762',
  holo: '#276e73',
  alarm: '#a62321',
  gold: '#694913',
  player: '#694913',
};

/** Every scheme's palettes; `hud` is Mnemorph's own, with the races' player colour. */
export const schemes: Record<SchemeName, { dark: SchemePalette; light: SchemePalette }> = {
  neon: { dark: neonDark, light: neonLight },
  ember: { dark: emberDark, light: emberLight },
  lagoon: { dark: lagoonDark, light: lagoonLight },
  harbor: { dark: harborDark, light: harborLight },
  dune: { dark: duneDark, light: duneLight },
  spice: { dark: spiceDark, light: spiceLight },
  hud: { dark: { ...dark, player: races.player }, light: { ...light, player: races.player } },
};

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
