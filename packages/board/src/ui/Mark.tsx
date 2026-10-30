/**
 * A coloured mark always travels with its words: colour is never the only carrier of meaning
 * (colour-vision deficiency, a monochrome screenshot). Every coloured node on the board is one of
 * these, and the end-to-end tests check each has a label.
 */
import type { WorldState, WorldStatus } from '@mnemorph/protocol';
import type { ReactNode } from 'react';

export type Tone = 'holo' | 'muted' | 'alarm' | 'gold';

/** A glyph in a tone, followed by its label. */
export function Mark({
  tone,
  glyph,
  children,
}: {
  tone: Tone;
  glyph: string;
  children: ReactNode;
}) {
  return (
    <span className="mark" data-tone={tone}>
      <span className="mark-glyph" aria-hidden="true">
        {glyph}
      </span>
      <span className="mark-label">{children}</span>
    </span>
  );
}

/** How each world state is drawn: a glyph, a tone and its words. */
export const STATUS: Record<WorldState, { glyph: string; tone: Tone; label: string }> = {
  present: { glyph: '◆', tone: 'holo', label: 'present' },
  missing: { glyph: '✕', tone: 'alarm', label: 'missing' },
};

/** How each world status is drawn: a glyph, a tone and its words. */
export const WORLD_STATUS: Record<WorldStatus, { glyph: string; tone: Tone; label: string }> = {
  live: { glyph: '●', tone: 'holo', label: 'live' },
  'tui-live': { glyph: '●', tone: 'holo', label: 'live in a terminal' },
  idle: { glyph: '○', tone: 'muted', label: 'idle' },
  missing: { glyph: '✕', tone: 'alarm', label: 'missing' },
};

/** The races, in the order the law lists them, with their token names. */
export const RACES = [
  ['rimuru', 'Rimuru'],
  ['elf', 'Elf'],
  ['orc', 'Orc'],
  ['slime', 'Slime'],
  ['kijin', 'Kijin'],
  ['dark-elf', 'Dark Elf'],
  ['high-elf', 'High Elf'],
  ['high-orc', 'High Orc'],
  ['named-slime', 'Named Slime'],
  ['oni', 'Oni'],
  ['arch-dark-elf', 'Arch Dark Elf'],
  ['veldora', 'Veldora'],
] as const;

export type Race = (typeof RACES)[number][0];

/** A race's colour as a swatch, with its name beside it. */
export function RaceChip({ race, name }: { race: Race; name: string }) {
  return (
    <span className="mark race" data-race={race}>
      <span className="race-swatch" aria-hidden="true" />
      <span className="mark-label">{name}</span>
    </span>
  );
}
