/**
 * The board's own hash router: every view has an address (`#/w/<world>/s/<chat>` a live chat,
 * `#/w/<world>/r/<run>` a past one), so a link,
 * a reload or the back button lands where it was. No library: a parse, a format and a hook.
 */
import { useEffect, useState } from 'react';

/** The sections of a world, beside its chats. */
export const WORLD_SECTIONS = ['bodies', 'memory', 'ontology', 'journal'] as const;
export type WorldSection = (typeof WORLD_SECTIONS)[number];

export type Route =
  | { name: 'worlds' }
  | { name: 'world'; world: string }
  | { name: 'section'; world: string; section: WorldSection }
  | { name: 'session'; world: string; session: string }
  | { name: 'run'; world: string; run: string }
  | { name: 'metrics' }
  | { name: 'integrations' }
  | { name: 'settings' }
  | { name: 'unknown'; path: string };

function decode(segment: string): string | undefined {
  try {
    const s = decodeURIComponent(segment);
    return s === '' ? undefined : s;
  } catch {
    return undefined;
  }
}

/** The route a location hash names; anything else is `unknown`, carrying the path as written. */
export function parse(hash: string): Route {
  const path = hash.replace(/^#/, '');
  const trimmed = path.replace(/^\/+/, '').replace(/\/+$/, '');
  if (trimmed === '') return { name: 'worlds' };
  const parts = trimmed.split('/');
  const unknown: Route = { name: 'unknown', path };
  const [head, a, b, c, ...rest] = parts;
  if (rest.length > 0) return unknown;
  if (head === 'metrics' || head === 'integrations' || head === 'settings') {
    return a === undefined ? { name: head } : unknown;
  }
  if (head !== 'w' || a === undefined) return unknown;
  const world = decode(a);
  if (world === undefined) return unknown;
  if (b === undefined) return { name: 'world', world };
  if (b === 's') {
    const session = c === undefined ? undefined : decode(c);
    return session === undefined ? unknown : { name: 'session', world, session };
  }
  if (b === 'r') {
    const run = c === undefined ? undefined : decode(c);
    return run === undefined ? unknown : { name: 'run', world, run };
  }
  if (c === undefined && (WORLD_SECTIONS as readonly string[]).includes(b)) {
    return { name: 'section', world, section: b as WorldSection };
  }
  return unknown;
}

/** The hash that names `route`; `parse(format(r))` gives `r` back. */
export function format(route: Route): string {
  const w = (id: string) => `#/w/${encodeURIComponent(id)}`;
  switch (route.name) {
    case 'worlds':
      return '#/';
    case 'world':
      return w(route.world);
    case 'section':
      return `${w(route.world)}/${route.section}`;
    case 'session':
      return `${w(route.world)}/s/${encodeURIComponent(route.session)}`;
    case 'run':
      return `${w(route.world)}/r/${encodeURIComponent(route.run)}`;
    case 'unknown':
      return `#${route.path}`;
    default:
      return `#/${route.name}`;
  }
}

/** The world a route is inside, if any. */
export function worldOf(route: Route): string | undefined {
  return 'world' in route ? route.world : undefined;
}

/** The current route, following the location hash. */
export function useRoute(): Route {
  const [route, setRoute] = useState(() => parse(window.location.hash));
  useEffect(() => {
    const on = () => setRoute(parse(window.location.hash));
    window.addEventListener('hashchange', on);
    return () => window.removeEventListener('hashchange', on);
  }, []);
  return route;
}
