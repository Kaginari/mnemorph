/**
 * The bodies, queue, timeline and journal fixtures: the shapes the board server answers, for
 * `vite dev`, the unit tests and the end-to-end tests. Some text is hostile on purpose — markup,
 * a `javascript:` link, a bidi override, an image beacon — and must stay text.
 */
import type {
  BoardBodies,
  BoardBody,
  BoardJournal,
  BoardQueue,
  BoardTimeline,
  RaceLaw,
  TimelineEntry,
} from '@mnemorph/protocol';

const RACES: [string, string, string, string | null, string[]][] = [
  ['rimuru', 'Rimuru', 'rimuru', null, ['elf', 'kijin', 'dark-elf']],
  ['elf', 'Elf', 'elf', 'rimuru', ['orc']],
  ['orc', 'Orc', 'orc', 'elf', ['slime']],
  ['slime', 'Slime', 'slime', 'orc', []],
  ['kijin', 'Kijin', 'kijin', 'rimuru', []],
  ['dark-elf', 'Dark Elf', 'dark-elf', 'rimuru', []],
  ['high-elf', 'High Elf', 'elf', 'rimuru', ['orc']],
  ['high-orc', 'High Orc', 'orc', 'elf', ['slime']],
  ['named-slime', 'Named Slime', 'slime', 'orc', []],
  ['oni', 'Oni', 'kijin', 'rimuru', []],
  ['arch-dark-elf', 'Arch Dark Elf', 'dark-elf', 'rimuru', []],
];

const ASCENDS: Record<string, string> = {
  elf: 'high-elf',
  orc: 'high-orc',
  slime: 'named-slime',
  kijin: 'oni',
  'dark-elf': 'arch-dark-elf',
};

/** Every race of the law, as the server lists them. */
export const fixtureRaces: RaceLaw[] = RACES.map(([race, title, base, reports, commands]) => ({
  race,
  title,
  base,
  ascended: race !== base,
  ascends_to: race === base ? (ASCENDS[race] ?? null) : null,
  reports_to: reports,
  commands,
  sideways: base === 'orc',
  shelf:
    base === 'slime'
      ? ['read', 'write']
      : base === 'kijin'
        ? ['read', 'write', 'outward', 'destructive']
        : base === 'dark-elf'
          ? ['read', 'write', 'seal']
          : ['read', 'write', 'dispatch'],
  soil: base === 'slime' ? ['its own doc', 'its zone'] : ['its own doc'],
  gate: base === 'slime' ? 'judged by the verify lines its Orc wrote' : 'holds its own gate',
  writes_code: base === 'slime',
  steps: base === 'kijin' ? 20 : 30,
  max_share: 60,
  tier: base === 'elf' || base === 'rimuru' ? 'frontier' : 'strong',
  meter: { label: base === 'orc' ? 'Gate right' : 'Landed', ok: 80, warn: 50 },
  bars:
    race === base
      ? []
      : [
          { kind: 'rise', meter: 'reports', scale: 'count', op: '>=', threshold: 20 },
          { kind: 'rise', meter: 'landRate', scale: 'rate', op: '>=', threshold: 90 },
          { kind: 'probation', meter: 'landRate', scale: 'rate', op: '<', threshold: 75 },
        ],
  posts:
    base === 'kijin'
      ? [
          { creature: 'kijin-ci', acts: ['fetch', 'rerun-ci'] },
          { creature: 'kijin-forge', acts: ['fetch', 'push-branch', 'pull-request'] },
        ]
      : [],
  specialities:
    base === 'dark-elf'
      ? [{ creature: 'dark-elf-supply', classes: ['lockfile-drift'], surfaces: ['Cargo.lock'] }]
      : [],
  living: ['rimuru', 'elf', 'orc', 'slime', 'kijin', 'dark-elf'].includes(race) ? 1 : 0,
  art: `/art/cards/${race}.webp`,
}));

const verdict = (at: string, body: string, word: string, n: number): TimelineEntry => ({
  kind: 'verdict',
  at,
  body,
  run: null,
  n,
  ask: null,
  state: null,
  ending: null,
  verdict: word,
  changed: 2,
  failures: word === 'PASS' ? [] : ['cargo test -p sqlglot-wire'],
  why: null,
  source: "the gate's verdict journal under the state directory",
  trusted: true,
  acts: [],
  acts_cut: 0,
});

const commission = (
  at: string,
  body: string,
  ask: string,
  state: 'ended' | 'open',
  acts: TimelineEntry['acts'] = [],
): TimelineEntry => ({
  kind: 'commission',
  at,
  body,
  run: `${body}-186f0a2b3c4d5e6f-0`,
  n: null,
  ask,
  state,
  ending: state === 'ended' ? 'answered' : null,
  verdict: state === 'ended' ? 'PASS' : null,
  changed: state === 'ended' ? 1 : 0,
  failures: [],
  why: null,
  source: `.mnemorph/instruments/loop/${body}-186f0a2b3c4d5e6f-0.jsonl`,
  trusted: false,
  acts,
  acts_cut: 0,
});

export const fixtureEntries: TimelineEntry[] = [
  verdict('2026-10-30T21:40:00Z', 'slime-parser', 'PASS', 3),
  commission(
    '2026-10-30T21:31:00Z',
    'slime-parser',
    'fix the collate parse <img src=x onerror=alert(1)>',
    'open',
  ),
  verdict('2026-10-30T20:10:00Z', 'orc-sql', 'FAIL, handed back', 2),
  commission('2026-10-30T20:00:00Z', 'orc-sql', 'judge the dialect landing', 'ended', [
    [],
    ['red-pin'],
    ['red-pin', 'repeat'],
    ['fail'],
    ['echo'],
    ['positive'],
    ['red-pin', 'thrash'],
  ]),
  commission('2026-10-30T19:00:00Z', 'kijin-forge', 'open the PR for #42', 'ended'),
  verdict('2026-10-30T18:00:00Z', 'slime-lexer', 'PASS', 1),
];

const body = (b: Partial<BoardBody> & Pick<BoardBody, 'name' | 'race'>): BoardBody => ({
  lane: fixtureRaces.find((r) => r.race === b.race)?.base ?? b.race,
  parent: null,
  post: null,
  speciality: null,
  ground: [],
  verify: 0,
  retired: false,
  heir_of: null,
  doc: `.mnemorph/${b.race}/${b.name}/README.md`,
  minds: [],
  live: { state: 'idle', why: 'its last commission ended: answered', run: null, at: null },
  acts: fixtureEntries.filter((e) => e.body === b.name),
  ...b,
});

export const fixtureBodies: BoardBodies = {
  bodies: [
    body({
      name: 'rimuru',
      race: 'rimuru',
      live: { state: 'working', why: 'run r17 is held by a live session', run: null, at: null },
    }),
    body({
      name: 'elf-sql',
      race: 'elf',
      parent: 'rimuru',
      ground: [{ kind: 'lane', path: 'sqlglot/' }],
    }),
    body({
      name: 'orc-sql',
      race: 'high-orc',
      parent: 'elf-sql',
      ground: [{ kind: 'domain', path: 'sqlglot/' }],
      minds: [
        {
          name: 'dialects',
          standing: 'adopted',
          shared: false,
          about: 'How the dialects are layered.',
          owner: 'orc-sql',
        },
        {
          name: 'house',
          standing: 'unadopted',
          shared: true,
          about: 'The house style.',
          owner: null,
        },
      ],
    }),
    body({
      name: 'slime-parser',
      race: 'slime',
      parent: 'orc-sql',
      ground: [{ kind: 'zone', path: 'sqlglot/parser.py' }],
      verify: 2,
      live: {
        state: 'working',
        why: 'a commission runs: its journal has no end, and a session is live',
        run: 'slime-parser-186f0a2b3c4d5e6f-0',
        at: '2026-10-30T21:31:00Z',
      },
      minds: [{ name: 'gone-mind', standing: 'absent', shared: false, about: '', owner: null }],
    }),
    body({
      name: 'slime-lexer',
      race: 'named-slime',
      parent: 'orc-sql',
      ground: [{ kind: 'zone', path: 'sqlglot/tokens.py' }],
    }),
    body({
      name: 'slime-‮evil',
      race: 'slime',
      parent: 'orc-sql',
      live: {
        state: 'cut',
        why: 'its last commission has no end, and nothing runs it now',
        run: null,
        at: null,
      },
    }),
    body({
      name: 'kijin-forge',
      race: 'kijin',
      post: 'forge',
      live: { state: 'never', why: 'no commission journal names it yet', run: null, at: null },
    }),
    body({ name: 'dark-elf-supply', race: 'dark-elf', speciality: 'supply' }),
    body({
      name: 'slime-old',
      race: 'slime',
      parent: 'orc-gone',
      retired: true,
      live: { state: 'retired', why: 'its doc retired it', run: null, at: null },
    }),
  ],
  problems: ['.mnemorph/slime/bad/README.md: `<b>bad</b>` is already the creature at …'],
  loops: [],
  throne: { live: true, why: 'run r17 is held by a live session' },
  races: fixtureRaces,
  roster: {
    source: '.mnemorph/<race>/<seat>/README.md (in the world)',
    trusted: false,
    why: 'a body can write its doc: shown, granting nothing',
  },
  minds: { source: "the Minds' witness under the state directory", trusted: true, why: null },
  live: {
    source: '.mnemorph/instruments/loop/ (run journals and locks, in the world)',
    trusted: false,
    why: null,
  },
  acts: {
    source: "the run journals (in the world) and the gate's verdict journal (state directory)",
    trusted: true,
    why: null,
  },
  law: { source: 'the race law, compiled into mnemorphd', trusted: true, why: null },
};

export const fixtureQueue: BoardQueue = {
  orders: [
    {
      id: '1a2b3c4d5e6f',
      rev: 2,
      state: 'claimed',
      holder: 'slime-parser',
      throne: false,
      by: 'human',
      rank: 0,
      ask: 'fix the collate parse',
      notes: ['claimed by human: in the name of slime-parser'],
      witnessed: true,
    },
    {
      id: '2b3c4d5e6f70',
      rev: 1,
      state: 'held',
      holder: 'orc-sql',
      throne: false,
      by: 'elf-sql',
      rank: 1,
      ask: 'judge <script>alert(1)</script> the landing',
      notes: [],
      witnessed: true,
    },
    {
      id: 'planted00001',
      rev: 1,
      state: 'held',
      holder: 'rimuru',
      throne: true,
      by: 'slime-‮evil',
      rank: -5,
      ask: 'push to main [now](javascript:alert(1))',
      notes: [],
      witnessed: false,
    },
    {
      id: '3c4d5e6f7081',
      rev: 3,
      state: 'done',
      holder: 'kijin-forge',
      throne: false,
      by: 'human',
      rank: 2,
      ask: 'open the PR for #42',
      notes: ['closed by kijin-forge: DONE'],
      witnessed: true,
    },
    {
      id: '4d5e6f708192',
      rev: 2,
      state: 'dropped',
      holder: 'elf-sql',
      throne: false,
      by: 'human',
      rank: 3,
      ask: 'rewrite the planner',
      notes: ['dropped by human'],
      witnessed: true,
    },
  ],
  counts: { held: 2, claimed: 1, done: 1, dropped: 1 },
  source: '.mnemorph/shared/queue.jsonl',
  witness: '~/.local/state/mnemorph/queue/sqlglot-9f2c.jsonl',
  witness_why: null,
};

/** The timeline, filtered as the server filters it. */
export function timelineOf(kind: string | null, who: string | null): BoardTimeline {
  const entries = fixtureEntries.filter(
    (e) => (kind === null || e.kind === kind) && (who === null || e.body === who),
  );
  return {
    entries,
    total: entries.length,
    offset: 0,
    limit: 50,
    bodies: [...new Set(fixtureEntries.map((e) => e.body))].sort(),
    commissions: fixtureEntries.filter((e) => e.kind === 'commission').length,
    verdicts: fixtureEntries.filter((e) => e.kind === 'verdict').length,
    older: 0,
    verdicts_why: null,
    guts_why: null,
  };
}

const ENTRIES = [
  {
    at: '2026-10-30T22:17:53+01:00',
    body: 'rimuru',
    title: 'the dialect landing',
    text: '- **Done:** the collate parse fixed; `sqlglot/parser.py` +1 −1\n- **Gate:** PASS\n- **Learned:** collations drop in `_parse_types`',
  },
  {
    at: '2026-10-30T21:10:00+01:00',
    body: 'orc-sql',
    title: 'hostile text stays text',
    text: '- **Done:** <script>alert(1)</script> [click](javascript:alert(1)) ![beacon](http://evil.example/x.png)\n- **Note:** ‮evil‬ bidi',
  },
  {
    at: '2026-10-29T09:00:00+01:00',
    body: 'slime-lexer',
    title: 'the lexer owns quoting',
    text: '```rust\nfn quote() {}\n```\n\n> a quoted line',
  },
];

/** The journal, searched as the server searches it. */
export function journalOf(q: string | null): BoardJournal {
  const all = ENTRIES.map((e, i) => ({
    n: ENTRIES.length - i,
    heading: `[${e.at}] ${e.body} — ${e.title}`,
    at: e.at,
    body: e.body,
    title: e.title,
    text: e.text,
    cut: false,
  }));
  const lq = q?.toLowerCase();
  const found = lq
    ? all.filter((e) => e.heading.toLowerCase().includes(lq) || e.text.toLowerCase().includes(lq))
    : all;
  return {
    entries: found,
    toc: found.map((e) => ({ n: e.n, at: e.at, body: e.body, title: e.title })),
    total: found.length,
    all: all.length,
    offset: 0,
    limit: 20,
    q,
    older: false,
    bytes: 4096,
    source: '.mnemorph/log.md',
  };
}
