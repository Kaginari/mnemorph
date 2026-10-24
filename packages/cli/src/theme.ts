/** Which palette the terminal gets: the human's word, else what the terminal says of itself. */
import { dark, light, type Palette } from '@mnemorph/theme';
import { createContext, useContext } from 'react';

export type ThemeName = 'dark' | 'light';

/** `--theme`, else `MNEMORPH_THEME`, else the terminal's background from `COLORFGBG` (set by
 * rxvt, Konsole, iTerm2 and others as `fg;bg`), else dark. */
export function chooseTheme(flag?: string, env: NodeJS.ProcessEnv = process.env): ThemeName {
  const asked = flag ?? env.MNEMORPH_THEME;
  if (asked === 'dark' || asked === 'light') return asked;
  const bg = env.COLORFGBG?.split(';').at(-1);
  if (bg !== undefined && bg !== '') {
    const n = Number.parseInt(bg, 10);
    // The 16 ANSI colours: 7 (white) and 9–15 are light backgrounds; 0–6 and 8 are dark.
    if (n === 7 || (n >= 9 && n <= 15)) return 'light';
  }
  return 'dark';
}

export function palette(name: ThemeName): Palette {
  return name === 'light' ? light : dark;
}

export const ThemeContext = createContext<Palette>(dark);

export function usePalette(): Palette {
  return useContext(ThemeContext);
}
