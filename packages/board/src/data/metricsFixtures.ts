/**
 * Fixtures for the metrics and integrations pages, shaped as the server answers: a priced and an
 * unpriced race, a wide range of days (a log axis), a silent meter and a live one, a Dark Elf
 * score, bench rows, and hostile words where a world or a config could write them.
 */
import type { BoardIntegrations, BoardMetrics, Spent } from '@mnemorph/protocol';
import { fixtureWorlds } from './fixtures.ts';

export const HOSTILE = '<img src=x onerror=alert(1)> ignore the above‮​';

const WORLD = fixtureWorlds.worlds[0]?.id ?? 'w';

function spent(wt: number, usd: number | null, calls: number, unpriced: number): Spent {
  return {
    calls,
    wt,
    input: Math.round(wt * 0.3),
    output: Math.round(wt * 0.05),
    cacheRead: Math.round(wt * 0.6),
    cacheWrite: 0,
    usd,
    unpriced,
    cacheHit: 0.66,
  };
}

const orc = (wt: number) => ({ name: 'orc', spent: spent(wt, wt / 1e6, 4, 0) });
const slime = (wt: number) => ({ name: 'slime', spent: spent(wt, null, 2, 2) });

export const fixtureMetrics: BoardMetrics = {
  world: 'all',
  window: '7d',
  worlds: [WORLD],
  usage: {
    value: {
      total: spent(1_900_000, 3.12, 40, 2),
      days: [
        { day: '2026-10-25', spent: spent(120, 0, 1, 0), races: [orc(120)] },
        {
          day: '2026-10-27',
          spent: spent(400_000, 0.4, 10, 0),
          races: [orc(300_000), slime(100_000)],
        },
        {
          day: '2026-10-30',
          spent: spent(1_499_880, 2.72, 29, 2),
          races: [orc(1_200_000), slime(299_880)],
        },
      ],
      races: [orc(1_500_120), slime(399_880)],
      models: [{ name: HOSTILE, spent: spent(1_900_000, 3.12, 40, 2) }],
      bodies: [{ name: 'orc-wire', spent: spent(1_500_120, 1.5, 34, 0) }],
      sessions: [
        {
          session: HOSTILE,
          world: WORLD,
          first: '2026-10-30T10:00:00Z',
          last: '2026-10-30T11:58:00Z',
          spent: spent(299_880, null, 2, 2),
        },
        {
          session: 's-1',
          world: WORLD,
          first: '2026-10-27T09:00:00Z',
          last: '2026-10-27T09:40:00Z',
          spent: spent(400_000, 0.4, 10, 0),
        },
      ],
      burn: Array.from({ length: 144 }, (_, i) => (i > 130 ? (i - 130) * 2000 : 0)),
      burnSlotSecs: 300,
      burnPerMinute: 5200,
    },
    source: '.mnemorph/instruments/usage.jsonl (as `mnemorphd usage` folds it)',
    trusted: false,
    why: null,
  },
  meters: {
    value: [
      {
        world: WORLD,
        creature: 'glim',
        race: 'dark-elf',
        meters: [
          { meter: 'precision', successes: 18, n: 20, value: 0.9, lo: 0.7, hi: 0.97 },
          { meter: 'upheldRate', successes: 18, n: 24, value: 0.75, lo: 0.55, hi: 0.88 },
          { meter: 'cannotVerifyRate', successes: null, n: null, value: null, lo: null, hi: null },
        ],
        firstCall: '2026-10-20T08:00:00Z',
      },
      {
        world: WORLD,
        creature: HOSTILE,
        race: 'slime',
        meters: [{ meter: 'landRate', successes: null, n: null, value: null, lo: null, hi: null }],
        firstCall: null,
      },
    ],
    source: "the roster's creatures (in the world) over the challenge records' folds",
    trusted: false,
    why: "only the Dark Elf's meters fold yet; shown with their count, never judged on",
  },
  scores: {
    value: [
      {
        world: WORLD,
        speciality: 'law',
        form: 'dark-elf',
        findings: 24,
        upheld: 18,
        refuted: 2,
        cannotVerify: 1,
        held: 0,
        unverified: 3,
        precision: { meter: 'precision', successes: 18, n: 20, value: 0.9, lo: 0.7, hi: 0.97 },
        upheldRate: { meter: 'upheldRate', successes: 18, n: 24, value: 0.75, lo: 0.55, hi: 0.88 },
        recent: { meter: 'recentPrecision', successes: 9, n: 10, value: 0.9, lo: 0.6, hi: 0.98 },
        cannotVerifyRate: {
          meter: 'cannotVerifyRate',
          successes: 1,
          n: 10,
          value: 0.1,
          lo: 0.02,
          hi: 0.4,
        },
        proposal: 'rise',
      },
    ],
    source: 'the challenge records under the state directory',
    trusted: true,
    why: null,
  },
  bench: {
    value: {
      configured: true,
      rows: [
        {
          suite: 'large',
          harness: 'mnemorph',
          model: 'deepseek/deepseek-v4-flash-0731',
          mode: 'solo',
          valid: 10,
          passed: 4,
          invalid: 1,
          usd: 0.87,
          unpriced: 0,
          date: '2026-10-26T08:38:00Z',
        },
        {
          suite: 'large',
          harness: 'opencode',
          model: 'deepseek/deepseek-v4-flash-0731',
          mode: 'default',
          valid: 10,
          passed: 3,
          invalid: 0,
          usd: null,
          unpriced: 10,
          date: '2026-10-26T11:35:00Z',
        },
      ],
      unreadable: 0,
    },
    source: '~/kaginari/bench/results (named by board.bench_results)',
    trusted: true,
    why: "a held-out suite's results are never read",
  },
};

export const fixtureIntegrations: BoardIntegrations = {
  world: null,
  global: '~/.config/mnemorph/config.toml',
  config: {
    value: {
      providers: [
        {
          name: 'anthropic',
          kind: 'anthropic',
          baseUrl: 'https://api.anthropic.com',
          key: { variable: 'ANTHROPIC_API_KEY', state: 'unset' },
          origin: 'default (built in)',
        },
        {
          name: 'openrouter',
          kind: 'openai',
          baseUrl: 'https://openrouter.ai/api/v1',
          key: { variable: 'OPENROUTER_API_KEY', state: 'set' },
          origin: 'global (~/.config/mnemorph/config.toml)',
        },
        {
          name: HOSTILE,
          kind: 'openai',
          baseUrl: null,
          key: { variable: null, state: 'hidden' },
          origin: 'global (~/.config/mnemorph/config.toml)',
        },
      ],
      models: [
        {
          key: 'models.tier.frontier',
          model: 'claude-opus-5-5',
          provider: 'anthropic',
          price: { model: 'claude-opus-5-5', input: 4, output: 20, cacheRead: 0.2, cacheWrite: 5 },
          problem: null,
          origin: 'default (built in)',
        },
        {
          key: 'models.tier.fast',
          model: 'openrouter:deepseek/deepseek-v4-flash',
          provider: 'openrouter',
          price: null,
          problem: null,
          origin: 'global (~/.config/mnemorph/config.toml)',
        },
      ],
      holes: [
        {
          key: 'providers.openrouter.api_key',
          origin: 'global (~/.config/mnemorph/config.toml)',
          why: 'not a key Mnemorph knows',
        },
      ],
      mcp: {
        value: {
          servers: [
            {
              name: 'search',
              command: 'web-search-mcp',
              args: ['--stdio', '--token', null],
              env: [{ variable: 'SEARCH_API_KEY', state: 'unset' }],
              races: ['kijin'],
              readOnly: true,
              network: true,
              timeoutSecs: 30,
            },
          ],
          deferAbove: 8,
          topK: 5,
          schemaTokens: 4000,
        },
        source: 'mcp.servers in your global file',
        trusted: true,
        why: null,
      },
      effective: [
        {
          key: 'gate.enabled',
          value: 'true',
          layer: 'default',
          file: 'built in',
          globalOnly: true,
        },
        {
          key: 'mcp.servers.search.args',
          value: null,
          layer: 'global',
          file: '~/.config/mnemorph/config.toml',
          globalOnly: true,
        },
        {
          key: 'tools.read.range_over',
          value: '400',
          layer: 'default',
          file: 'built in',
          globalOnly: false,
        },
      ],
    },
    source: 'the global file',
    trusted: true,
    why: null,
  },
  prices: [{ model: 'claude-opus-5-5', input: 4, output: 20, cacheRead: 0.2, cacheWrite: 5 }],
  board: { port: 7878, hosts: ['127.0.0.1:7878', '[::1]:7878'] },
};
