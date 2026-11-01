// @vitest-environment jsdom
import type { BoardWorld } from '@mnemorph/protocol';
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { parseBodies, parseJournal, parseQueue, parseTimeline } from '../data/bodies.ts';
import { fixtureBodies, fixtureQueue, journalOf, timelineOf } from '../data/bodiesFixtures.ts';
import { fixtureWorlds } from '../data/fixtures.ts';
import { artOf, BodiesView } from './Bodies.tsx';
import { JournalView } from './Journal.tsx';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const WORLD = fixtureWorlds.worlds[0] as BoardWorld;

let host: HTMLDivElement;
let mounted: ReturnType<typeof createRoot> | undefined;

beforeEach(() => {
  host = document.createElement('div');
  document.body.append(host);
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string) => {
      const u = new URL(url, 'http://board');
      const reply = (status: number, body: unknown) => ({
        ok: status < 400,
        status,
        json: async () => body,
        text: async () => (typeof body === 'string' ? body : JSON.stringify(body)),
      });
      if (u.pathname === '/api/session') return reply(200, { csrf: 'ab'.repeat(32) });
      if (u.pathname.endsWith('/bodies')) return reply(200, fixtureBodies);
      if (u.pathname.endsWith('/queue')) return reply(200, fixtureQueue);
      if (u.pathname.endsWith('/timeline')) {
        return reply(200, timelineOf(u.searchParams.get('kind'), u.searchParams.get('body')));
      }
      if (u.pathname.endsWith('/journal')) return reply(200, journalOf(u.searchParams.get('q')));
      return reply(404, 'not found');
    }),
  );
});

afterEach(async () => {
  await act(async () => mounted?.unmount());
  mounted = undefined;
  host.remove();
  vi.unstubAllGlobals();
});

async function mount(view: React.ReactNode) {
  const root = createRoot(host);
  mounted = root;
  await act(async () => root.render(view));
  await act(async () => {});
}

test('each answer is read row by row: a row that does not hold together is left out and counted', () => {
  const bad = { ...fixtureBodies, bodies: [...fixtureBodies.bodies, { name: 7 }] };
  expect(parseBodies(bad)?.unreadable).toBe(1);
  expect(parseBodies(fixtureBodies)?.bodies).toHaveLength(fixtureBodies.bodies.length);
  expect(parseBodies({ bodies: [] })).toBeUndefined();
  const q = { ...fixtureQueue, orders: [...fixtureQueue.orders, { id: 'x', state: 'gone' }] };
  expect(parseQueue(q)?.unreadable).toBe(1);
  const t = timelineOf(null, null);
  expect(parseTimeline({ ...t, entries: [...t.entries, { kind: 'act' }] })?.unreadable).toBe(1);
  const j = journalOf(null);
  expect(parseJournal({ ...j, toc: [...j.toc, { n: -1 }] })?.unreadable).toBe(1);
});

test('only the art the law names by a plain path loads', () => {
  const r = fixtureBodies.races[0];
  if (!r) throw new Error('a race');
  expect(artOf(r)).toBe('/art/cards/rimuru.webp');
  for (const art of [
    'http://evil.example/x.webp',
    '/art/cards/../x.webp',
    '//evil/x.webp',
    'javascript:alert(1)',
    '/art/cards/x.png',
  ]) {
    expect(artOf({ ...r, art })).toBeUndefined();
  }
});

test('the colony net labels every body, and a click opens its card with its art, law, Minds and acts', async () => {
  await mount(<BodiesView w={WORLD} tab="colony" />);
  const nodes = [...host.querySelectorAll<SVGGElement>('.colony-node')];
  expect(nodes).toHaveLength(fixtureBodies.bodies.length);
  for (const n of nodes) {
    expect(n.getAttribute('aria-label')?.length).toBeGreaterThan(0);
    expect(n.querySelector('.mark-label')?.textContent?.trim()).not.toBe('');
  }
  // A hidden character in a name is shown escaped, never applied.
  const evil = nodes.find((n) => n.dataset.name?.includes('evil'));
  expect(evil?.querySelector('.svg-esc')?.textContent).toMatch(/\\u\{?202e/i);
  const orc = nodes.find((n) => n.dataset.name === 'orc-sql');
  await act(async () => orc?.dispatchEvent(new MouseEvent('click', { bubbles: true })));
  const card = host.querySelector('.body-card');
  expect(card?.querySelector('img')?.getAttribute('src')).toBe('/art/cards/high-orc.webp');
  expect(card?.textContent).toContain('dialects');
  expect(card?.textContent).toContain('adopted: loads');
  expect(card?.textContent).toContain('verdict #2');
  expect(card?.textContent).toContain('reports to');
});

test('a card says how it stands against the witness: trusted, mismatched, or a stranger', async () => {
  await mount(<BodiesView w={WORLD} tab="colony" />);
  const open = async (name: string) => {
    const node = [...host.querySelectorAll<SVGGElement>('.colony-node')].find(
      (n) => n.dataset.name === name,
    );
    await act(async () => node?.dispatchEvent(new MouseEvent('click', { bubbles: true })));
    return host.querySelector('.body-card')?.textContent ?? '';
  };
  expect(await open('orc-sql')).toContain('trusted');
  const lexer = await open('slime-lexer');
  expect(lexer).toContain('mismatched');
  expect(lexer).toContain('runs as slime');
  expect(lexer).toContain('differs from its row on Race');
  expect(lexer).toContain('mnemorphd roster adopt slime-lexer');
  const evil = await open('slime-‮evil');
  expect(evil).toContain('stranger');
  expect(evil).toContain('its own seat alone');
  expect(host.textContent).toContain("the creatures' witness under the state directory");
});

test('hostile text in an order and a commission stays text', async () => {
  await mount(<BodiesView w={WORLD} tab="queue" />);
  expect(host.querySelector('script')).toBeNull();
  expect(host.textContent).toContain('judge <script>alert(1)</script> the landing');
  expect(host.textContent).toContain('not witnessed');
  expect(host.querySelector('a[href^="javascript"]')).toBeNull();
});

test('the journal renders log.md inert, with its contents', async () => {
  await mount(<JournalView w={WORLD} />);
  expect(host.querySelector('script')).toBeNull();
  expect(host.querySelector('img')).toBeNull();
  expect(host.querySelector('a[href^="javascript"]')).toBeNull();
  expect(host.textContent).toContain('<script>alert(1)</script>');
  expect(host.querySelectorAll('.journal-toc li')).toHaveLength(3);
  expect(host.querySelectorAll('.journal-entry')).toHaveLength(3);
});

test('a commission shows a guts chip per class of each act, every chip with its word', async () => {
  await mount(<BodiesView w={WORLD} tab="timeline" />);
  const acts = [...host.querySelectorAll<HTMLLIElement>('.guts-act')];
  expect(acts).toHaveLength(7);
  const words = acts.map((a) =>
    [...a.querySelectorAll('.guts-chip .mark-label')].map((m) => m.textContent),
  );
  expect(words).toEqual([
    [],
    ['red-pin'],
    ['red-pin', 'repeat'],
    ['fail'],
    ['echo'],
    ['positive'],
    ['red-pin', 'thrash'],
  ]);
  expect(acts[0]?.dataset.plain).toBe('true');
  expect(acts[0]?.textContent).toContain('no class');
  for (const chip of host.querySelectorAll<HTMLElement>('.guts-chip')) {
    expect(chip.dataset.class).toBe(chip.querySelector('.mark-label')?.textContent);
  }
});

test('an act with a class the board does not know leaves its entry out, counted', () => {
  const t = timelineOf(null, null);
  const forged = t.entries.map((e) =>
    e.acts.length > 0 ? { ...e, acts: [['positive'], ['<img src=x onerror=alert(1)>']] } : e,
  );
  const page = parseTimeline({ ...t, entries: forged });
  expect(page?.unreadable).toBe(1);
  expect(page?.entries.every((e) => e.acts.length === 0)).toBe(true);
});
