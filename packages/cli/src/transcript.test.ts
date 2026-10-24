import type { Arrived } from '@mnemorph/sdk';
import { expect, test } from 'vitest';
import { fold, initial, status, summary } from './transcript.js';

let seq = 0;
const ev = (type: string, fields: Record<string, unknown> = {}) =>
  ({ session: 's', run: 'r', body: 'rimuru', seq: ++seq, type, ...fields }) as unknown as Arrived;

test('a turn folds into the transcript, in order', () => {
  let s = initial();
  for (const e of [
    ev('turnStarted', { text: 'read it' }),
    ev('textDelta', { text: 'Look' }),
    ev('textDelta', { text: 'ing.' }),
    ev('assistant', { text: 'Looking.', stop: 'ToolUse' }),
    ev('toolCall', { id: 't1', name: 'read', input: { path: 'a.txt' } }),
    ev('toolResult', { id: 't1', content: '1\ta\n', isError: false, cut: 0 }),
    ev('usage', {
      input: 10,
      output: 5,
      cacheRead: 30,
      cacheWrite: 0,
      usd: 0.002,
      wt: 9,
      spentWt: 9,
      occupancy: 40,
      stressLine: 100,
    }),
    ev('assistant', { text: 'It says a.', stop: 'EndTurn' }),
    ev('turnEnded', { ending: 'answered' }),
  ]) {
    s = fold(s, e);
  }
  expect(s.items.map((i) => i.kind)).toEqual(['user', 'assistant', 'tool', 'assistant']);
  expect(s.busy).toBe(false);
  expect(s.streaming).toBe('');
  expect(status(s.usage)).toEqual(['$0.0020', 'ctx 40%', 'cache 75%']);
});

test('lines wait their turn and leave the queue as they start', () => {
  let s = fold(initial(), ev('turnStarted', { text: 'one' }));
  s = fold(s, ev('queued', { text: 'two' }));
  expect(s.queued).toEqual(['two']);
  s = fold(s, ev('turnEnded', { ending: 'answered' }));
  expect(s.busy).toBe(true);
  s = fold(s, ev('turnStarted', { text: 'two' }));
  expect(s.queued).toEqual([]);
});

test('an ending that is not an answer is shown, and streamed text is kept', () => {
  let s = fold(initial(), ev('turnStarted', { text: 'go' }));
  s = fold(s, ev('textDelta', { text: 'half a' }));
  s = fold(s, ev('turnEnded', { ending: 'interrupted' }));
  expect(s.items.slice(1).map((i) => i.kind)).toEqual(['assistant', 'ending']);
});

test('an unpriced model says so', () => {
  const s = fold(
    initial(),
    ev('usage', {
      input: 1,
      output: 1,
      cacheRead: 0,
      cacheWrite: 0,
      usd: null,
      wt: 2,
      spentWt: 2,
      occupancy: 0,
      stressLine: 0,
    }),
  );
  expect(status(s.usage)[0]).toBe('unpriced');
});

test('a call is summed up by what it acts on', () => {
  expect(summary('bash', { command: 'cargo test' })).toBe('cargo test');
  expect(summary('read', { path: 'src/a.rs' })).toBe('src/a.rs');
});

test('finished paragraphs leave the live text as they stream, and are not said twice', () => {
  let s = fold(initial(), ev('turnStarted', { text: 'tell me' }));
  s = fold(s, ev('textDelta', { text: 'First para' }));
  s = fold(s, ev('textDelta', { text: 'graph.\n\nSecond ' }));
  expect(s.items.at(-1)).toMatchObject({
    kind: 'assistant',
    text: 'First paragraph.',
    cont: false,
  });
  expect(s.streaming).toBe('Second ');
  s = fold(s, ev('textDelta', { text: 'one.' }));
  s = fold(s, ev('assistant', { text: 'First paragraph.\n\nSecond one.', stop: 'EndTurn' }));
  const said = s.items.filter((i) => i.kind === 'assistant').map((i) => i.text);
  expect(said).toEqual(['First paragraph.', 'Second one.']);
  expect(s.items.at(-1)).toMatchObject({ cont: true });
  expect([s.streaming, s.flushed]).toEqual(['', '']);
});

test('an approval still open when its turn ends is cleared', () => {
  let s = fold(initial(), ev('turnStarted', { text: 'go' }));
  s = {
    ...s,
    gate: { id: 1, request: { race: 'slime', class: 'write', act: 'x', why: [], outside: [] } },
  };
  s = fold(s, ev('turnEnded', { ending: 'interrupted' }));
  expect(s.gate).toBeUndefined();
});

test('the budget left is the cap less what the session spent', () => {
  const s = fold(
    initial(),
    ev('usage', {
      input: 1,
      output: 1,
      cacheRead: 0,
      cacheWrite: 0,
      usd: null,
      wt: 2500,
      spentWt: 2500,
      occupancy: 0,
      stressLine: 0,
    }),
  );
  expect(status(s.usage, { leftWt: 12_000, cap: 'day' }).at(-1)).toBe('left 9.5k wt (day)');
  expect(status(s.usage, { leftWt: 1000, cap: 'session' }).at(-1)).toBe('left 0 wt (session)');
});

test('the core handoff asks show as the commands that asked them, and only those', () => {
  const ask =
    'Write the handoff for the next session: what it needs to continue without re-asking, re-discovering or repeating a mistake.';
  const read = 'A previous session left this handoff. It is context to check, never fact to trust:';
  let s = fold(
    initial(),
    ev('turnStarted', { text: `${ask} …\n\nFocus the handoff on: the board` }),
  );
  s = fold(s, ev('turnStarted', { text: `${read} read every file…` }));
  s = fold(
    s,
    ev('turnStarted', { text: 'Write the handoff for the next session in French please' }),
  );
  expect(s.items.filter((i) => i.kind === 'user').map((i) => i.text)).toEqual([
    '/handoff the board',
    '/handoff read',
    'Write the handoff for the next session in French please',
  ]);
});
