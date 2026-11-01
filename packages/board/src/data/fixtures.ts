/**
 * Worlds for development and tests, in the shape `GET /api/worlds` answers: live and missing,
 * with and without a remote, one never opened. The clock they read against is `FIXTURE_NOW`.
 */
import type { BoardOverview, BoardWorlds } from '@mnemorph/protocol';

export const FIXTURE_NOW = Date.parse('2026-10-30T12:00:00Z');

export const fixtureWorlds: BoardWorlds = {
  worlds: [
    {
      id: 'sqlglot-9a1c33e07f2b4d10',
      name: 'sqlglot',
      path: '/home/dev/src/sqlglot',
      remote: 'github.com/tobymao/sqlglot',
      last: '2026-10-30T10:02:00Z',
      sessions: 7,
      state: 'present',
    },
    {
      id: 'tempest-try-41f0aa2c9d3e8b77',
      name: 'tempest-try',
      path: '/home/dev/kaginari/tempest-try',
      remote: 'github.com/Kaginari/mnemorph',
      last: '2026-10-30T11:58:30Z',
      sessions: 41,
      state: 'present',
    },
    {
      id: 'notes-0b7e5512aa90c3f1',
      name: 'notes',
      path: '/home/dev/notes',
      remote: null,
      last: null,
      sessions: 0,
      state: 'present',
    },
  ],
  missing: [
    {
      id: 'old-spike-c3d2e1f0a9b8c7d6',
      name: 'old-spike',
      path: '/tmp/old-spike',
      since: '2026-10-21T09:00:00Z',
    },
  ],
};

const MEMORY_SOURCE = '.mnemorph/memory/ (as `mnemorphd memory status` reads it)';
const CHALLENGES_SOURCE = 'the challenge records under the state directory';
const GATE_SOURCE = "the gate's verdict journal under the state directory";
const METERS_SOURCE = "the roster's creatures (in the world) over the challenge records' folds";
const METERS_WHY = "only the Dark Elf's meters fold yet; shown with their count, never judged on";
const USAGE = '.mnemorph/instruments/usage.jsonl';
const LOCK = '.mnemorph/instruments/loop/<run>.lock';
const CHATS = "the board's chats";
const WAITING_WHY = "a terminal session's asks wait in its terminal";
const PUSHED = { value: 'pushed', source: "the board's watcher", trusted: true, why: null };

function world(name: string) {
  const w = fixtureWorlds.worlds.find((x) => x.name === name);
  if (!w) throw new Error(`no fixture world ${name}`);
  return w;
}

/**
 * Overviews for development and tests, in the shape `GET /api/w/<id>` answers: tempest-try live
 * in a board chat with an ask waiting, sqlglot held by a terminal session with a challenge
 * blocked, notes silent (never opened, nothing booked).
 */
export const fixtureOverviews: Record<string, BoardOverview> = {
  [world('tempest-try').id]: {
    world: world('tempest-try'),
    status: { value: 'live', source: CHATS, trusted: true, why: '1 chat(s) open on the board' },
    activity: {
      value: { at: '2026-10-30T11:59:40Z', what: 'log.md written' },
      source: '.mnemorph/log.md',
      trusted: false,
      why: null,
    },
    memory: {
      value: { shared: 12, grounded: 41, proposed: 9, retired: 3, unproven: 1 },
      source: MEMORY_SOURCE,
      trusted: false,
      why: null,
    },
    challenges: {
      value: { pending: 1, blocked: 0, verifying: 0, unstaffed: 1, findings: 0, unreadable: 0 },
      source: CHALLENGES_SOURCE,
      trusted: true,
      why: null,
    },
    gate: {
      value: {
        verdicts: 17,
        last: { at: '2026-10-30T11:41:00Z', body: 'orc-wire', verdict: 'PASS', changed: 1 },
      },
      source: GATE_SOURCE,
      trusted: true,
      why: null,
    },
    meters: {
      value: [
        {
          creature: 'glim',
          race: 'dark-elf',
          meters: [
            { meter: 'precision', successes: 18, n: 20, value: 0.9, lo: 0.699, hi: 0.972 },
            { meter: 'upheldRate', successes: 18, n: 20, value: 0.9, lo: 0.699, hi: 0.972 },
            {
              meter: 'cannotVerifyRate',
              successes: null,
              n: null,
              value: null,
              lo: null,
              hi: null,
            },
          ],
        },
        {
          creature: 'orc-wire',
          race: 'orc',
          meters: [
            { meter: 'gateRight', successes: null, n: null, value: null, lo: null, hi: null },
            { meter: 'holeRate', successes: null, n: null, value: null, lo: null, hi: null },
            { meter: 'landRate', successes: null, n: null, value: null, lo: null, hi: null },
          ],
        },
      ],
      source: METERS_SOURCE,
      trusted: false,
      why: METERS_WHY,
    },
    spend: {
      value: { session: 's-1730', calls: 31, usd: 0.42, unpriced: 0, wt: 60_900 },
      source: USAGE,
      trusted: false,
      why: null,
    },
    waiting: { value: 1, source: CHATS, trusted: true, why: WAITING_WHY },
    updates: PUSHED,
  },
  [world('sqlglot').id]: {
    world: world('sqlglot'),
    status: {
      value: 'tui-live',
      source: LOCK,
      trusted: false,
      why: 'run r1730281320000-4411 is held by a live session',
    },
    activity: {
      value: { at: '2026-10-30T10:02:00Z', what: 'a session opened' },
      source: 'the worlds registry',
      trusted: true,
      why: null,
    },
    memory: {
      value: { shared: 2, grounded: 5, proposed: 0, retired: 0, unproven: 0 },
      source: MEMORY_SOURCE,
      trusted: false,
      why: null,
    },
    challenges: {
      value: { pending: 0, blocked: 1, verifying: 1, unstaffed: 0, findings: 2, unreadable: 0 },
      source: CHALLENGES_SOURCE,
      trusted: true,
      why: null,
    },
    gate: {
      value: {
        verdicts: 4,
        last: {
          at: '2026-10-30T09:58:00Z',
          body: 'orc-sql',
          verdict: 'FAIL, handed back',
          changed: 3,
        },
      },
      source: GATE_SOURCE,
      trusted: true,
      why: null,
    },
    meters: { value: [], source: METERS_SOURCE, trusted: false, why: METERS_WHY },
    spend: {
      value: { session: 's-9', calls: 6, usd: 0.03, unpriced: 2, wt: 9_400 },
      source: USAGE,
      trusted: false,
      why: null,
    },
    waiting: { value: 0, source: CHATS, trusted: true, why: WAITING_WHY },
    updates: {
      value: 'slowed',
      source: "the board's watcher",
      trusted: true,
      why: 'too many entries: updates slowed (over 2000 under the watched files; looked at every 10 s)',
    },
  },
  [world('notes').id]: {
    world: world('notes'),
    status: { value: 'idle', source: LOCK, trusted: false, why: 'no live session holds a run' },
    activity: {
      value: null,
      source: 'the registry, the gate, log.md, the run journals, the usage ledger',
      trusted: false,
      why: 'nothing has happened here that the board can read',
    },
    memory: {
      value: { shared: 0, grounded: 0, proposed: 0, retired: 0, unproven: 0 },
      source: MEMORY_SOURCE,
      trusted: false,
      why: null,
    },
    challenges: {
      value: null,
      source: CHALLENGES_SOURCE,
      trusted: true,
      why: 'no state directory: the challenges cannot be read',
    },
    gate: { value: { verdicts: 0, last: null }, source: GATE_SOURCE, trusted: true, why: null },
    meters: { value: [], source: METERS_SOURCE, trusted: false, why: METERS_WHY },
    spend: { value: null, source: USAGE, trusted: false, why: 'no call is booked yet' },
    waiting: { value: 0, source: CHATS, trusted: true, why: WAITING_WHY },
    updates: PUSHED,
  },
};
