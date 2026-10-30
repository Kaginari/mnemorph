/**
 * Light, dark or auto. `@mnemorph/theme`'s tokens.css is dark by default and light under
 * `data-theme="light"`; auto follows the system and moves when it does. The choice is kept per
 * browser.
 */
import { useCallback, useEffect, useState } from 'react';

export type Choice = 'auto' | 'light' | 'dark';
export type Theme = 'light' | 'dark';
export const CHOICES: readonly Choice[] = ['auto', 'light', 'dark'];

const KEY = 'mnemorph.theme';

/** The theme a choice gives, the system preferring light or not. */
export function resolve(choice: Choice, systemLight: boolean): Theme {
  if (choice === 'auto') return systemLight ? 'light' : 'dark';
  return choice;
}

/** The stored choice; auto when none, or when storage cannot be read. */
export function stored(): Choice {
  try {
    const v = window.localStorage.getItem(KEY);
    return v === 'light' || v === 'dark' ? v : 'auto';
  } catch {
    return 'auto';
  }
}

function store(choice: Choice) {
  try {
    if (choice === 'auto') window.localStorage.removeItem(KEY);
    else window.localStorage.setItem(KEY, choice);
  } catch {
    // A private window: the choice lasts this page only.
  }
}

const query = () => window.matchMedia('(prefers-color-scheme: light)');

/** Sets the resolved theme on `<html>`; called once before the first render, then by the hook. */
export function apply(choice: Choice) {
  const root = document.documentElement;
  root.dataset.theme = resolve(choice, query().matches);
  root.dataset.themeChoice = choice;
}

/** The theme choice and its setter; auto follows the system while the page is open. */
export function useTheme(): [Choice, (c: Choice) => void] {
  const [choice, setChoice] = useState<Choice>(stored);
  useEffect(() => {
    apply(choice);
    if (choice !== 'auto') return;
    const mq = query();
    const on = () => apply('auto');
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  }, [choice]);
  const set = useCallback((c: Choice) => {
    store(c);
    setChoice(c);
  }, []);
  return [choice, set];
}
