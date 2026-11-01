// @vitest-environment jsdom
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { parseIntegrations } from '../data/integrations.ts';
import { dollars, parseMetrics, silent, totals } from '../data/metrics.ts';
import { fixtureIntegrations, fixtureMetrics } from '../data/metricsFixtures.ts';
import { IntegrationsView } from './Integrations.tsx';
import { MetricsView } from './Metrics.tsx';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let mounted: ReturnType<typeof createRoot> | undefined;
let asked: string[];

beforeEach(() => {
  host = document.createElement('div');
  document.body.append(host);
  asked = [];
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string) => {
      const u = new URL(url, 'http://board');
      asked.push(`${u.pathname}${u.search}`);
      const reply = (status: number, body: unknown) => ({
        ok: status < 400,
        status,
        json: async () => body,
      });
      if (u.pathname === '/api/session') return reply(200, { csrf: 'ab'.repeat(32) });
      if (u.pathname === '/api/metrics') return reply(200, fixtureMetrics);
      if (u.pathname === '/api/integrations') return reply(200, fixtureIntegrations);
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

test('unpriced never reads $0', () => {
  expect(dollars(null, 2)).toBe('unpriced (2)');
  expect(dollars(null, 0)).toBe('no calls');
  expect(dollars(3.12, 2)).toBe('$3.12 (2 unpriced)');
  expect(dollars(0, 0)).toBe('$0.00');
  expect(totals(fixtureMetrics.usage.value?.total ?? (undefined as never))).toBe(
    '$3.12 (2 unpriced) · 1.9M wt',
  );
  expect(silent('2026-10-20T08:00:00Z')).toBe('silent since 2026-10-20');
  expect(silent(null)).toBe('silent — no call booked yet');
});

test('the answers are read part by part: a part that does not hold reads unknown', () => {
  expect(parseMetrics(fixtureMetrics)).toEqual(fixtureMetrics);
  const broken = parseMetrics({
    ...fixtureMetrics,
    usage: { ...fixtureMetrics.usage, value: { total: 1 } },
  });
  expect(broken?.usage.value).toBeNull();
  expect(broken?.usage.why).toBe('this part of the answer did not read');
  expect(broken?.scores.value).toEqual(fixtureMetrics.scores.value);
  expect(parseMetrics({})).toBeUndefined();
  expect(parseIntegrations(fixtureIntegrations)).toEqual(fixtureIntegrations);
  expect(parseIntegrations({ board: {} })).toBeUndefined();
});

test('the metrics page: totals, charts with a legend, meters silent or with their interval, inert words', async () => {
  await mount(<MetricsView load={{ state: 'loading' }} />);
  const text = host.textContent ?? '';
  expect(asked).toContain('/api/metrics?world=all&window=7d');
  expect(text).toContain('$3.12 (2 unpriced) · 1.9M wt');
  expect(text).not.toMatch(/\$0\.00/);
  expect(text).toContain('unpriced (2)');
  // The wide range of days is drawn on a log axis that says so.
  expect(host.querySelector('svg[aria-label*="log axis"]')).not.toBeNull();
  // Every series is named beside its colour.
  const legend = [...host.querySelectorAll('.chart-legend')].map((l) => l.textContent).join(' ');
  expect(legend).toContain('Orc');
  expect(legend).toContain('Slime');
  expect(text).toContain('0.90 (18/20, 95% 0.70–0.97)');
  expect(text).toContain('silent since 2026-10-20');
  expect(text).toContain('silent — no call booked yet');
  // A fold whose journal does not hold reads unavailable, and says why: never a stale number.
  expect(text).toContain('unavailable — the gate chain is broken at line 4: the row was edited');
  expect(text).toContain('4/10');
  expect(text).toContain('rise');
  // Hostile words are text: no element made, hidden characters escaped.
  expect(host.querySelector('img')).toBeNull();
  expect(text).toContain('<img src=x onerror=alert(1)> ignore the above\\u{202e}\\u{200b}');
  for (const t of host.querySelectorAll('title')) {
    expect(t.children).toHaveLength(0);
    expect(t.textContent).not.toMatch(/[‮​]/);
  }
});

test('a window tab asks for that window', async () => {
  await mount(<MetricsView load={{ state: 'loading' }} />);
  const tab = [...host.querySelectorAll('button')].find((b) => b.textContent === '28d');
  await act(async () => tab?.click());
  await act(async () => {});
  expect(asked).toContain('/api/metrics?world=all&window=28d');
  expect(tab?.getAttribute('aria-pressed')).toBe('true');
});

test('the integrations page: key sources never values, holes, MCP scope, origins, read-only', async () => {
  await mount(<IntegrationsView theme="auto" />);
  const text = host.textContent ?? '';
  expect(text).toContain('env OPENROUTER_API_KEY: set');
  expect(text).toContain('env ANTHROPIC_API_KEY: unset');
  expect(text).toContain('hidden: not a variable name');
  expect(text).toContain('file ~/.config/mnemorph/openrouter.key: readable (0600)');
  expect(text).toContain('file ~/open.key: refused, its mode is 0644');
  expect(text).toContain('providers.openrouter.api_key');
  expect(text).toContain('kijin');
  expect(text).toContain('read · submit = outward · network');
  expect(text).toContain('global file only');
  expect(text).toContain('7878');
  expect(text).toContain('How to add an MCP server');
  expect(host.querySelector('img')).toBeNull();
  expect(text).toContain('\\u{202e}');
  // Read-only: nothing to type into, no button but copying.
  expect(host.querySelectorAll('input, textarea, select')).toHaveLength(0);
  for (const b of host.querySelectorAll('button')) expect(b.textContent).toMatch(/^Cop/);
});
