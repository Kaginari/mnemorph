/**
 * Worlds for development and tests, in the shape `GET /api/worlds` answers: present and missing,
 * with and without a remote, one never opened. The clock they read against is `FIXTURE_NOW`.
 */
import type { BoardWorlds } from '@mnemorph/protocol';

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
    {
      id: 'old-spike-c3d2e1f0a9b8c7d6',
      name: 'old-spike',
      path: '/tmp/old-spike',
      remote: null,
      last: '2026-07-01T16:20:00Z',
      sessions: 2,
      state: 'missing',
    },
  ],
};
