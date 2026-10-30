import { describe, expect, it } from 'vitest';
import { fixtureOverviews } from './fixtures.ts';
import { parseOverview } from './overview.ts';
import { parseChange } from './push.ts';

describe('the overview answer', () => {
  it('reads every fixture whole, unknowns and all', () => {
    for (const o of Object.values(fixtureOverviews)) {
      expect(parseOverview(JSON.parse(JSON.stringify(o)))).toEqual(o);
    }
  });

  it('reads a field that does not hold together as unknown, saying so', () => {
    const [first] = Object.values(fixtureOverviews);
    if (!first) throw new Error('fixtures');
    const bad = JSON.parse(JSON.stringify(first));
    bad.memory.value.shared = -1;
    bad.spend.value.usd = 'free';
    bad.status.value = 'thriving';
    bad.gate = 'nope';
    const o = parseOverview(bad);
    expect(o?.memory).toEqual({
      value: null,
      source: first.memory.source,
      trusted: false,
      why: 'the answer for this field did not read',
    });
    expect(o?.spend.value).toBeNull();
    expect(o?.status.value).toBeNull();
    expect(o?.gate).toEqual({ value: null, source: 'unknown', trusted: false, why: 'unknown' });
    // What still reads, reads.
    expect(o?.challenges).toEqual(first.challenges);
  });

  it('refuses what is no overview', () => {
    expect(parseOverview(null)).toBeUndefined();
    expect(parseOverview({ world: { id: 1 } })).toBeUndefined();
  });
});

describe('the push channel', () => {
  it('reads only its two notifications', () => {
    expect(parseChange('{"jsonrpc":"2.0","method":"world/changed","params":{"id":"a-1"}}')).toEqual(
      { kind: 'world', id: 'a-1' },
    );
    expect(parseChange('{"jsonrpc":"2.0","method":"worlds/changed"}')).toEqual({
      kind: 'worlds',
    });
    for (const no of [
      'nope',
      '{"jsonrpc":"2.0","method":"world/changed","params":{"id":""}}',
      '{"jsonrpc":"2.0","method":"world/changed"}',
      '{"jsonrpc":"1.0","method":"worlds/changed"}',
      '{"jsonrpc":"2.0","method":"gate/ask","params":{}}',
      42,
    ]) {
      expect(parseChange(no)).toBeUndefined();
    }
  });
});
