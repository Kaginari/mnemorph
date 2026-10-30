/**
 * Worlds for development and tests, one of each status. The clock they read against is
 * `FIXTURE_NOW`.
 */
import type { WorldsResponse } from '@mnemorph/protocol';

export const FIXTURE_NOW = Date.parse('2026-10-30T12:00:00Z');

export const fixtureWorlds: WorldsResponse = {
  worlds: [
    {
      id: 'sqlglot-9a1c33e07f2b4d10',
      name: 'sqlglot',
      world: '/home/dev/src/sqlglot',
      remote: 'github.com/tobymao/sqlglot',
      first: '2026-09-02T08:11:00Z',
      last: '2026-10-30T10:02:00Z',
      sessions: 7,
      status: 'idle',
    },
    {
      id: 'tempest-try-41f0aa2c9d3e8b77',
      name: 'tempest-try',
      world: '/home/dev/kaginari/tempest-try',
      remote: 'github.com/Kaginari/mnemorph',
      first: '2026-08-14T17:40:00Z',
      last: '2026-10-30T11:58:30Z',
      sessions: 41,
      status: 'live',
    },
    {
      id: 'notes-0b7e5512aa90c3f1',
      name: 'notes',
      world: '/home/dev/notes',
      first: '2026-10-01T09:00:00Z',
      last: '2026-10-29T21:15:00Z',
      sessions: 3,
      status: 'tui-live',
    },
    {
      id: 'old-spike-c3d2e1f0a9b8c7d6',
      name: 'old-spike',
      world: '/tmp/old-spike',
      first: '2026-06-10T10:00:00Z',
      last: '2026-07-01T16:20:00Z',
      sessions: 2,
      status: 'missing',
    },
  ],
};
