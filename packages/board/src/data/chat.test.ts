import type { Arrived } from '@mnemorph/protocol';
import { describe, expect, it } from 'vitest';
import { Chat, groupCounts, summary } from './chat.ts';

let seq = 0;
const ev = (type: string, fields: Record<string, unknown> = {}, body = 'rimuru') => {
  seq += 1;
  return { type, session: 's', run: 'r', body, seq, ...fields } as unknown as Arrived;
};

function turn(chat: Chat, events: Arrived[], now?: number) {
  for (const e of events) chat.fold(e, now);
  return chat.view();
}

describe('the transcript', () => {
  it('folds a turn in the TUI grammar: the line, the answer, the cards, the gate', () => {
    const chat = new Chat('rimuru');
    const v = turn(chat, [
      ev('turnStarted', { text: 'fix the collate parse' }),
      ev('textDelta', { text: "I'll look first.\n\n" }),
      ev('toolStart', { id: 't1', name: 'read' }),
      ev('assistant', { text: "I'll look first.\n\n", stop: 'tool_use' }),
      ev('toolCall', { id: 't1', name: 'read', input: { path: 'parser.py' } }),
      ev('toolResult', { id: 't1', content: '1\ta', isError: false, cut: 0 }),
      ev('toolCall', { id: 't2', name: 'grep', input: { pattern: 'collate' } }),
      ev('toolResult', { id: 't2', content: 'x', isError: false, cut: 0 }),
      ev('toolCall', {
        id: 't3',
        name: 'edit',
        input: { path: 'parser.py', old_string: 'a\n', new_string: 'b\n' },
      }),
      ev('toolResult', { id: 't3', content: 'ok', isError: false, cut: 0 }),
      ev('assistant', { text: 'Fixed.', stop: 'end_turn' }),
      ev('gateVerdict', {
        passed: true,
        verdict: 'PASS',
        changed: ['parser.py'],
        failures: [],
        unverified: [],
      }),
      ev('turnEnded', { ending: 'answered' }),
    ]);
    expect(v.entries.map((e) => e.kind)).toEqual([
      'user',
      'assistant',
      'group',
      'tool',
      'assistant',
      'verdict',
    ]);
    const group = v.entries[2];
    expect(group?.kind === 'group' && groupCounts(group.calls)).toBe(
      'read 1 file · grep 1 pattern',
    );
    expect(v.entries[1]).toMatchObject({ kind: 'assistant', head: true, text: "I'll look first." });
    expect(v.entries[4]).toMatchObject({ kind: 'assistant', head: false, text: 'Fixed.' });
    expect(v.files).toEqual([['parser.py', 'M']]);
    expect(v.busy).toBe(false);
    // Keys are stable and unique: the virtual list keeps rows by them.
    expect(new Set(v.entries.map((e) => e.key)).size).toBe(v.entries.length);
  });

  it('keeps a paragraph streaming inside an open fence', () => {
    const chat = new Chat('rimuru');
    const v = turn(chat, [
      ev('turnStarted', { text: 'go' }),
      ev('textDelta', { text: '```py\nx = 1\n\ny = 2\n' }),
    ]);
    expect(v.entries.map((e) => e.kind)).toEqual(['user']);
    expect(v.streaming).toContain('y = 2');
  });

  it('says what a turn that did not answer ended on, and stops its running calls', () => {
    const chat = new Chat('rimuru');
    const v = turn(chat, [
      ev('turnStarted', { text: 'go' }),
      ev('toolCall', { id: 't1', name: 'bash', input: { command: 'sleep 9' } }),
      ev('turnEnded', { ending: 'interrupted' }),
    ]);
    const tool = v.entries[1];
    expect(tool?.kind === 'tool' && tool.call.stopped).toBe(true);
    expect(v.entries[2]).toMatchObject({ kind: 'ending', ending: 'interrupted' });
  });

  it('shows a commissioned body as lines below, and in the tree', () => {
    const chat = new Chat('rimuru');
    const v = turn(chat, [
      ev('turnStarted', { text: 'go' }),
      ev('turnStarted', { text: 'judge the wire\nmore' }, 'orc-wire'),
      ev('toolCall', { id: 'b1', name: 'bash', input: { command: 'cargo test' } }, 'orc-wire'),
      ev('textDelta', { text: 'not shown' }, 'orc-wire'),
      ev('turnEnded', { ending: 'answered' }, 'orc-wire'),
    ]);
    expect(v.entries.slice(1).map((e) => (e.kind === 'below' ? e.text : e.kind))).toEqual([
      'is working on: judge the wire',
      'bash cargo test',
      'answered',
    ]);
    expect(v.bodies).toEqual([{ name: 'orc-wire', state: 'done', last: 'answered' }]);
  });

  it('counts the spend, and an unpriced turn is never $0', () => {
    const chat = new Chat('rimuru');
    const usage = (usd: number | null) =>
      ev('usage', {
        input: 100,
        output: 20,
        cacheRead: 0,
        cacheWrite: 0,
        usd,
        wt: 120,
        spentWt: 120,
        occupancy: 41_000,
        stressLine: 100_000,
      });
    const v = turn(chat, [ev('turnStarted', { text: 'go' }), usage(0.01), usage(null)]);
    expect(v.usage).toMatchObject({ unpriced: true, tokens: 240, occupancy: 41_000 });
    expect(v.usage.usd).toBeCloseTo(0.01);
    expect(v.turnTokens).toBe(240);
  });

  it('takes a queued line off the queue when its turn starts', () => {
    const chat = new Chat('rimuru');
    let v = turn(chat, [ev('turnStarted', { text: 'one' }), ev('queued', { text: 'two' })]);
    expect(v.queued).toEqual(['two']);
    v = turn(chat, [ev('turnEnded', { ending: 'answered' }), ev('turnStarted', { text: 'two' })]);
    expect(v.queued).toEqual([]);
  });

  it('times calls and a long wait only when seen live', () => {
    const chat = new Chat('rimuru');
    chat.fold(ev('turnStarted', { text: 'go' }), 1000);
    chat.fold(ev('toolCall', { id: 't', name: 'bash', input: { command: 'ls' } }), 4000);
    chat.fold(ev('toolResult', { id: 't', content: '', isError: false, cut: 0 }), 4500);
    const v = chat.view();
    expect(v.entries[1]).toMatchObject({ kind: 'thought', took: 3000 });
    const tool = v.entries[2];
    expect(tool?.kind === 'tool' && tool.call.took).toBe(500);
    const replayed = new Chat('rimuru');
    replayed.fold(ev('turnStarted', { text: 'go' }));
    replayed.fold(ev('toolCall', { id: 't', name: 'bash', input: { command: 'ls' } }));
    expect(replayed.view().entries.map((e) => e.kind)).toEqual(['user', 'tool']);
  });

  it('summarises a call as the TUI does', () => {
    expect(summary('bash', { command: 'pytest -q\nmore' })).toBe('pytest -q');
    expect(summary('read', { path: 'a.py', offset: 10, limit: 5 })).toBe('a.py:10-14');
    expect(summary('grep', { pattern: 'x' })).toBe('x');
    expect(summary('t', { n: 1 })).toBe('{"n":1}');
  });
});
