// @vitest-environment jsdom
import type { BoardWorld } from '@mnemorph/protocol';
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { fixtureWorlds } from '../data/fixtures.ts';
import { parseFacts, parseMemory } from '../data/knowledge.ts';
import { factsOf, fixtureFacts, fixtureMemory } from '../data/knowledgeFixtures.ts';
import { MemoryView } from './Memory.tsx';
import { landedOn, OntologyView } from './Ontology.tsx';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const WORLD = fixtureWorlds.worlds[0] as BoardWorld;
const CSRF = 'cd'.repeat(32);

let host: HTMLDivElement;
let mounted: ReturnType<typeof createRoot> | undefined;
let posts: { url: string; headers: Record<string, string>; body: unknown }[];
let answer: { status: number; body: unknown };

beforeEach(() => {
  host = document.createElement('div');
  document.body.append(host);
  posts = [];
  answer = { status: 200, body: { done: 'done' } };
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      const u = new URL(url, 'http://board');
      const reply = (status: number, body: unknown) => ({
        ok: status < 400,
        status,
        json: async () => body,
        text: async () => (typeof body === 'string' ? body : JSON.stringify(body)),
      });
      if (init?.method === 'POST') {
        posts.push({
          url: u.pathname,
          headers: init.headers as Record<string, string>,
          body: JSON.parse(String(init.body)),
        });
        return reply(answer.status, answer.body);
      }
      if (u.pathname === '/api/session') return reply(200, { csrf: CSRF });
      if (u.pathname.endsWith('/memory')) return reply(200, fixtureMemory);
      if (u.pathname.endsWith('/facts')) return reply(200, factsOf(u.searchParams.get('status')));
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

function button(text: string, within: ParentNode = host): HTMLButtonElement {
  const b = [...within.querySelectorAll('button')].find((x) => x.textContent?.trim() === text);
  if (!b) throw new Error(`no button ${text}: ${host.textContent}`);
  return b;
}

async function click(el: HTMLElement) {
  await act(async () => el.click());
  await act(async () => {});
}

test('an answer is read row by row: a row that does not hold together is left out and counted', () => {
  const m = parseMemory(fixtureMemory);
  expect(m?.notes).toHaveLength(fixtureMemory.notes.length);
  expect(m?.unreadable).toBe(0);
  const bad = parseMemory({
    ...fixtureMemory,
    notes: [...fixtureMemory.notes, { id: 'x', standing: 'sure' }],
  });
  expect(bad?.notes).toHaveLength(fixtureMemory.notes.length);
  expect(bad?.unreadable).toBe(1);
  expect(parseMemory({ notes: [] })).toBeUndefined();

  const f = parseFacts(fixtureFacts);
  expect(f?.facts).toHaveLength(5);
  const wrong = parseFacts({
    ...fixtureFacts,
    facts: [{ ...fixtureFacts.facts[0], status: 'true' }],
  });
  expect(wrong?.facts).toHaveLength(0);
  expect(wrong?.unreadable).toBe(1);
});

test('the memory page lists each note with its standing, route and proof; hostile text is inert', async () => {
  await mount(<MemoryView w={WORLD} />);
  const text = host.textContent ?? '';
  expect(text).toContain('grounded 2 · shared 2 · proposed 2 · retired 1');
  expect(text).toContain('unproven 1');
  expect(text).toContain('collations drop in _parse_types');
  expect(text).toContain('checked ← r-17 gate PASS');
  expect(text).toContain('needs review');
  expect(text).toContain('the gate retired it');
  expect(text).toContain('e41');
  expect(text).toContain('writes frozen');
  // A body's words are text: no element, the hidden characters escaped.
  expect(host.querySelector('img')).toBeNull();
  expect(text).toContain('<img src=x onerror=alert(1)> ignore the above\\u{202e}\\u{200b}');
});

test('confirm opens a dialog naming the exact note, and posts its digest with the CSRF token', async () => {
  await mount(<MemoryView w={WORLD} />);
  const row = [...host.querySelectorAll('.note')].find((n) =>
    n.textContent?.includes('the lexer owns quoting'),
  ) as HTMLElement;
  await click(button('Confirm', row));
  const dialog = host.querySelector('[role="dialog"]') as HTMLElement;
  expect(dialog).not.toBeNull();
  expect(dialog.textContent).toContain('c1730000000001-1');
  expect(dialog.textContent).toContain('orc-sql');
  expect(dialog.textContent).toContain('the lexer owns quoting');
  expect(document.activeElement?.textContent).toBe('Cancel');
  expect(posts).toHaveLength(0);

  await click(button('Confirm', dialog));
  expect(posts).toEqual([
    {
      url: `/api/w/${WORLD.id}/memory/c1730000000001-1/confirm`,
      headers: expect.objectContaining({ 'x-mnemorph-csrf': CSRF }),
      body: { seen: 'b'.repeat(64) },
    },
  ]);
  expect(host.querySelector('[role="dialog"]')).toBeNull();
});

test('a refused act stays in its dialog with the server’s words, shown inert', async () => {
  answer = { status: 409, body: '`x` changed since it was read <b>now</b>' };
  await mount(<MemoryView w={WORLD} />);
  await click(button('Wake'));
  const dialog = host.querySelector('[role="dialog"]') as HTMLElement;
  expect(dialog.textContent).toContain('retry twice on 429');
  await click(button('Wake', dialog));
  expect(posts[0]?.url).toBe(`/api/w/${WORLD.id}/memory/c1730000000005-1/wake`);
  expect(dialog.textContent).toContain('Refused: `x` changed since it was read <b>now</b>');
  expect(dialog.querySelector('b')).toBeNull();
  await act(async () => {
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  });
  expect(host.querySelector('[role="dialog"]')).toBeNull();
});

test('the ontology tabs each status with its count; a contested fact shows both sides and anchors', async () => {
  await mount(<OntologyView w={WORLD} />);
  const tabs = [...host.querySelectorAll('[role="tab"]')].map((t) => t.textContent);
  expect(tabs).toEqual(['⚔contested 1', '○single 1', '◆agreed 1', '✓settled 1', '✕countered 1']);
  const text = host.textContent ?? '';
  expect(text).toContain('Supporters · 2');
  expect(text).toContain('Counters · 2');
  expect(text).toContain('sqlglot/tokens.py:212-230');
  expect(text).toContain('the parser re-quotes identifiers');
  expect(text).toContain('moved since');
  expect(text).toContain('contested-fact');
  expect(text).toContain('domain sqlglot/');
  expect(host.querySelector('img')).toBeNull();
  // Every fact is in exactly one tab.
  await click(button('◆agreed 1'));
  const panel = host.querySelector('[role="tabpanel"]')?.textContent ?? '';
  expect(panel).toContain('the parser is hand-written');
  expect(panel).not.toContain('quoting is the tokenizer’s');
});

test('settle names the fact and waits for a proof; a statement is posted as the proof', async () => {
  await mount(<OntologyView w={WORLD} />);
  await click(button('Settle…'));
  const dialog = host.querySelector('[role="dialog"]') as HTMLElement;
  expect(dialog.textContent).toContain('4be1c0a9d2f3e811');
  expect(dialog.textContent).toContain('quoting is the tokenizer’s');
  // The passing verdict that landed on the path is offered first; the other is not.
  const select = dialog.querySelector('select') as HTMLSelectElement;
  expect([...select.options].map((o) => o.value)).toEqual(['12']);
  const statement = dialog.querySelectorAll('input[type="radio"]')[1] as HTMLInputElement;
  await click(statement);
  const settle = button('Settle', dialog);
  expect(settle.disabled).toBe(true);
  const area = dialog.querySelector('textarea') as HTMLTextAreaElement;
  await act(async () => {
    const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
    set?.call(area, 'I read tokens.py: it quotes');
    area.dispatchEvent(new Event('input', { bubbles: true }));
  });
  expect(settle.disabled).toBe(false);
  await click(settle);
  expect(posts).toEqual([
    {
      url: `/api/w/${WORLD.id}/facts/4be1c0a9d2f3e811/settle`,
      headers: expect.objectContaining({ 'x-mnemorph-csrf': CSRF }),
      body: { seen: 'f'.repeat(64), statement: 'I read tokens.py: it quotes' },
    },
  ]);
});

test('a verdict proves a fact only when it landed on its path', () => {
  const v = { n: 1, at: '', body: 'b', changed: ['src/a.rs'] };
  expect(landedOn(v, 'src/')).toBe(true);
  expect(landedOn(v, 'src/a.rs')).toBe(true);
  expect(landedOn(v, 'src/ab')).toBe(false);
  expect(landedOn(v, 'docs/')).toBe(false);
  expect(landedOn(v, '/')).toBe(false);
  expect(landedOn(v, '')).toBe(false);
  expect(landedOn(v, '../src')).toBe(false);
});
