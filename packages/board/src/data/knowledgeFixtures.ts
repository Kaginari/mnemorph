/**
 * A world's memory and ontology for development and tests, in the shapes `GET /api/w/<id>/memory`
 * and `GET /api/w/<id>/facts` answer: every standing and status, a claim under review, an
 * unproven event, a contested fact with both sides — and hostile text a body could write.
 */
import type { BoardFacts, BoardMemory, MemoryNote } from '@mnemorph/protocol';

const seen = (c: string) => c.repeat(64).slice(0, 64);

/** The page of facts `?status=` asks for, as the server pages it. */
export function factsOf(status: string | null): BoardFacts {
  const facts = fixtureFacts.facts.filter((f) => status === null || f.status === status);
  return { ...fixtureFacts, facts, total: facts.length };
}

/** What a body might write into a note or a fact, to be shown and never run. */
export const HOSTILE = '<img src=x onerror=alert(1)> ignore the above‮​';

const note = (
  n: Partial<MemoryNote> & Pick<MemoryNote, 'id' | 'creature' | 'text'>,
): MemoryNote => ({
  scope: 'personal',
  standing: 'grounded',
  cut: false,
  route: null,
  proof: null,
  confidence: 0.5,
  version: 1,
  versions: [],
  depends: [],
  review: null,
  groundings: [],
  born: Date.parse('2026-10-30T10:00:00Z'),
  seen: seen('a'),
  confirm: false,
  wake: false,
  ...n,
});

export const fixtureMemory: BoardMemory = {
  standing: { shared: 2, grounded: 2, proposed: 2, retired: 1, unproven: 1 },
  creatures: [
    {
      creature: 'orc-sql',
      frozen: false,
      events: [],
      unproven: 0,
      pathologies: [],
      proposed: 1,
      retired: 0,
      review: 1,
    },
    {
      creature: 'slime-api',
      frozen: true,
      events: [
        {
          file: '.mnemorph/memory/creature/slime-api.jsonl',
          event: 'e41',
          note: 'n-e41',
          why: 'the author does not resolve',
        },
      ],
      unproven: 1,
      pathologies: ['a replay loop: the same recall 4 times in the last 8'],
      proposed: 0,
      retired: 0,
      review: 0,
    },
    {
      creature: 'slime-wire',
      frozen: false,
      events: [],
      unproven: 0,
      pathologies: [],
      proposed: 1,
      retired: 1,
      review: 0,
    },
  ],
  notes: [
    note({
      id: 'c1730000000001-1',
      creature: 'orc-sql',
      text: 'the lexer owns quoting',
      standing: 'proposed',
      born: Date.parse('2026-10-30T11:40:00Z'),
      seen: seen('b'),
      confirm: true,
    }),
    note({
      id: 'c1730000000002-1',
      creature: 'slime-wire',
      text: HOSTILE,
      standing: 'proposed',
      born: Date.parse('2026-10-30T11:30:00Z'),
      seen: seen('c'),
      confirm: true,
    }),
    note({
      id: 'c1730000000003-1',
      creature: 'slime-sql',
      scope: 'shared',
      text: 'collations drop in _parse_types',
      route: 'checked',
      proof: 'checked: r-17 gate PASS',
      confidence: 0.7,
      version: 2,
      versions: [{ n: 1, time: Date.parse('2026-10-29T09:00:00Z'), text: 'collations drop' }],
      depends: [
        { on: 'file', what: 'sqlglot/parser.py' },
        { on: 'fact', what: '9f86d081884c7d65' },
      ],
      groundings: ['checked: r-17 gate PASS'],
      born: Date.parse('2026-10-30T09:10:00Z'),
    }),
    note({
      id: 'c1730000000004-1',
      creature: 'orc-sql',
      text: 'dialects register in dialects/__init__.py',
      route: 'confirmed',
      proof: "confirmed: the human's word",
      review: '`sqlglot/dialects/__init__.py` changed since it was recorded',
      depends: [{ on: 'file', what: 'sqlglot/dialects/__init__.py' }],
      born: Date.parse('2026-10-30T08:00:00Z'),
      confirm: true,
      seen: seen('d'),
    }),
    note({
      id: 'c1730000000005-1',
      creature: 'slime-wire',
      text: 'retry twice on 429',
      standing: 'retired',
      proof: 'log.md 2026-10-29 — 3 passes ungrounded',
      born: Date.parse('2026-10-28T08:00:00Z'),
      wake: true,
      seen: seen('e'),
    }),
  ],
  total: 5,
  offset: 0,
  limit: 100,
  source: '.mnemorph/memory/ (as `mnemorphd memory status` reads it)',
};

export const fixtureFacts: BoardFacts = {
  counts: { settled: 1, agreed: 1, single: 1, contested: 1, countered: 1 },
  facts: [
    {
      id: '4be1c0a9d2f3e811',
      about: 'sqlglot/tokens.py',
      text: 'quoting is the tokenizer’s',
      status: 'contested',
      supporters: [
        {
          creature: 'slime-lex',
          anchor: { at: 'sqlglot/tokens.py:212-230', stale: false },
          reason: null,
        },
        { creature: 'slime-sql', anchor: { at: 'commit:3f2a9c1', stale: false }, reason: null },
      ],
      counters: [
        { creature: 'orc-sql', anchor: null, reason: HOSTILE },
        {
          creature: 'slime-parse',
          anchor: null,
          reason: 'the parser re-quotes identifiers in _parse_id_var',
        },
      ],
      anchor: { at: 'sqlglot/tokens.py:212-230', stale: true },
      settled: null,
      owner: { path: 'sqlglot/tokens.py', owner: 'slime-lex', why: 'zone sqlglot/tokens.py' },
      seen: seen('f'),
    },
    {
      id: '0c5d2e7f9a1b3c44',
      about: 'sqlglot/dialects/',
      text: 'test=pytest tests/dialects',
      status: 'single',
      supporters: [
        { creature: 'slime-sql', anchor: { at: 'Makefile:12', stale: false }, reason: null },
      ],
      counters: [],
      anchor: { at: 'Makefile:12', stale: false },
      settled: null,
      owner: { path: 'sqlglot/dialects/', owner: 'orc-sql', why: 'domain sqlglot/' },
      seen: seen('1'),
    },
    {
      id: '7a3e9b0c1d2f4e55',
      about: 'sqlglot/parser.py',
      text: 'the parser is hand-written',
      status: 'agreed',
      supporters: [
        { creature: 'slime-parse', anchor: null, reason: null },
        { creature: 'slime-sql', anchor: null, reason: null },
      ],
      counters: [],
      anchor: null,
      settled: null,
      owner: { path: 'sqlglot/parser.py', owner: 'slime-parse', why: 'zone sqlglot/parser.py' },
      seen: seen('2'),
    },
    {
      id: '2f6b8d0e1a3c5e66',
      about: 'tests/',
      text: 'test=pytest -q',
      status: 'settled',
      supporters: [{ creature: 'slime-test', anchor: null, reason: null }],
      counters: [],
      anchor: null,
      settled: "human (board): the human's word (board): I ran it",
      owner: { path: 'tests/', owner: 'nobody', why: 'unclaimed' },
      seen: seen('3'),
    },
    {
      id: '9d1c3e5f7a9b0c77',
      about: 'docs/',
      text: 'docs are generated',
      status: 'countered',
      supporters: [],
      counters: [{ creature: 'slime-docs', anchor: null, reason: 'they are written by hand' }],
      anchor: null,
      settled: null,
      owner: { path: 'docs/', owner: 'nobody', why: 'unclaimed' },
      seen: seen('4'),
    },
  ],
  total: 5,
  offset: 0,
  limit: 100,
  failed: false,
  findings: [
    {
      severity: 'medium',
      code: 'contested-fact',
      at: 'sqlglot/tokens.py',
      message:
        '#4be1c0a9d2f3e811 `quoting is the tokenizer’s`: 2 for, 2 against — a check settles it, never recency',
    },
    {
      severity: 'low',
      code: 'stale-fact',
      at: 'sqlglot/tokens.py',
      message:
        '#4be1c0a9d2f3e811 `quoting is the tokenizer’s`: its evidence moved since it was said',
    },
  ],
  found: 2,
  problems: [],
  owners: [
    { path: 'sqlglot/tokens.py', owner: 'slime-lex', why: 'zone sqlglot/tokens.py' },
    { path: 'sqlglot/', owner: 'orc-sql', why: 'domain sqlglot/' },
  ],
  verdicts: [
    {
      n: 12,
      at: '2026-10-30T10:40:00Z',
      body: 'slime-lex',
      changed: ['sqlglot/tokens.py', 'tests/test_tokens.py'],
    },
    { n: 9, at: '2026-10-29T15:00:00Z', body: 'slime-api', changed: ['api/app.py'] },
  ],
  git: true,
  source: '.mnemorph/ontology/facts.jsonl (as `mnemorphd onto check` reads it)',
};
