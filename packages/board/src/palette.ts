/**
 * The palette audit's arithmetic: colours as people with a colour-vision deficiency see them
 * (Machado, Oliveira and Fernandes 2009, full severity, in linear sRGB) and how far apart two
 * colours read (OKLab distance). The board's tones are checked with it in palette.test.ts.
 */

type Rgb = [number, number, number];

const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);

/** A `#rrggbb` token value as linear sRGB. */
export function linear(hex: string): Rgb {
  return [1, 3, 5].map((i) => toLinear(Number.parseInt(hex.slice(i, i + 2), 16) / 255)) as Rgb;
}

/** Machado et al. 2009, severity 1.0. */
export const DEFICIENCIES = {
  protanopia: [
    [0.152286, 1.052583, -0.204868],
    [0.114503, 0.786281, 0.099216],
    [-0.003882, -0.048116, 1.051998],
  ],
  deuteranopia: [
    [0.367322, 0.860646, -0.227968],
    [0.280085, 0.672501, 0.047413],
    [-0.01182, 0.04294, 0.968881],
  ],
  tritanopia: [
    [1.255528, -0.076749, -0.178779],
    [-0.078411, 0.930809, 0.147602],
    [0.004733, 0.691367, 0.3039],
  ],
} as const;

export type Vision = 'normal' | keyof typeof DEFICIENCIES;
export const VISIONS: readonly Vision[] = ['normal', 'protanopia', 'deuteranopia', 'tritanopia'];

/** `c` as seen with `vision`. */
export function seen(c: Rgb, vision: Vision): Rgb {
  if (vision === 'normal') return c;
  const m = DEFICIENCIES[vision];
  const clamp = (x: number) => Math.min(1, Math.max(0, x));
  return m.map((row) => clamp(row[0] * c[0] + row[1] * c[1] + row[2] * c[2])) as Rgb;
}

/** Linear sRGB to OKLab (Ottosson). */
export function toOklab([r, g, b]: Rgb): Rgb {
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817201489 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

/** How far apart two `#rrggbb` colours read with `vision`: OKLab distance, 0 for the same. */
export function apart(a: string, b: string, vision: Vision): number {
  const [x, y] = [toOklab(seen(linear(a), vision)), toOklab(seen(linear(b), vision))];
  return Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]);
}
