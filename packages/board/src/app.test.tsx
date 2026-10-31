// @vitest-environment jsdom
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { App } from './App.tsx';
import { fixtureWorlds } from './data/fixtures.ts';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let mounted: ReturnType<typeof createRoot> | undefined;

beforeEach(() => {
  window.matchMedia = vi.fn().mockReturnValue({
    matches: false,
    addEventListener: () => {},
    removeEventListener: () => {},
  });
  host = document.createElement('div');
  document.body.append(host);
});

afterEach(async () => {
  // Unmounted before the environment goes: no fetch or retry renders into a torn-down window.
  await act(async () => mounted?.unmount());
  mounted = undefined;
  host.remove();
  vi.unstubAllGlobals();
  window.location.hash = '';
});

async function mount(hash: string, body: unknown = fixtureWorlds, status = 200) {
  window.location.hash = hash;
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue({ ok: status === 200, status, json: async () => body }),
  );
  const root = createRoot(host);
  mounted = root;
  await act(async () => root.render(<App />));
  await act(async () => {});
  return root;
}

test('the home lists every world, each state in words', async () => {
  await mount('#/');
  const text = host.textContent ?? '';
  expect(text).toContain('MNEMORPH');
  for (const n of ['tempest-try', 'sqlglot', 'notes', 'old-spike']) expect(text).toContain(n);
  expect(text).toContain('never opened');
  expect(text).toContain('missing');
});

test('a world name is shown, never run', async () => {
  const evil = { ...fixtureWorlds.worlds[0], name: '<img src=x onerror=alert(1)>\u202e' };
  await mount('#/', { worlds: [evil] });
  expect(host.querySelector('img')).toBeNull();
  expect(host.textContent).toContain('<img src=x onerror=alert(1)>\\u{202e}');
});

test('an unknown address gives an honest empty state', async () => {
  await mount('#/nowhere');
  expect(host.textContent).toContain('Nothing at this address');
  expect(host.textContent).toContain('/nowhere');
});

test('an unknown world says so', async () => {
  await mount('#/w/ghost-1/memory');
  expect(host.textContent).toContain('No such world');
});

test('a server that fails is said to', async () => {
  await mount('#/', {}, 503);
  expect(host.textContent).toContain('The worlds could not be read');
  expect(host.textContent).toContain('503');
});

test('the tab title escapes a hidden character from the address or a world', async () => {
  await mount('#/w/%E2%80%AEevil');
  expect(document.title).not.toContain('\u202e');
  expect(document.title).toContain('\\u{202e}');
});

const key = (k: string, more: KeyboardEventInit = {}) =>
  act(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, ...more }));
  });

const palette = () => document.querySelector('[aria-label="Command palette"]');

test('Ctrl+K opens the palette and Esc closes it; ? opens the shortcut map', async () => {
  await mount('#/');
  await key('k', { ctrlKey: true });
  expect(palette()).not.toBeNull();
  expect(document.activeElement?.getAttribute('role')).toBe('combobox');
  await act(async () => {
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
    );
  });
  expect(palette()).toBeNull();
  await key('?');
  expect(document.body.textContent).toContain('Keyboard shortcuts');
  expect(document.body.textContent).toContain('No shortcut approves anything');
});

test('the palette never opens while an approval card is up, and closes when one arrives', async () => {
  await mount('#/');
  const card = document.createElement('div');
  card.className = 'approval-backdrop';
  document.body.append(card);
  await key('k', { ctrlKey: true });
  await key('?');
  expect(palette()).toBeNull();
  expect(document.body.textContent).not.toContain('Keyboard shortcuts');
  card.remove();

  await key('k', { metaKey: true });
  expect(palette()).not.toBeNull();
  await act(async () => {
    document.body.append(card);
    await new Promise((r) => setTimeout(r, 0));
  });
  expect(palette()).toBeNull();
  card.remove();
});

test('the palette lists only addresses: picking one moves the page', async () => {
  await mount('#/');
  await key('k', { ctrlKey: true });
  const input = document.querySelector<HTMLInputElement>('[role="combobox"]');
  if (!input) throw new Error('no input');
  await act(async () => {
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
    set?.call(input, 'metrics');
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
  const first = document.querySelector('[role="option"]');
  expect(first?.textContent).toContain('Metrics');
  expect(document.querySelectorAll('[role="option"] button, [role="option"] a').length).toBe(0);
  await act(async () => {
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  });
  expect(window.location.hash).toBe('#/metrics');
  expect(palette()).toBeNull();
});
