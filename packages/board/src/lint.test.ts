import { expect, it } from 'vitest';
// @ts-expect-error: a plain ES module, typed by its use here
import { offences } from '../../../tools/check-board.mjs';

const check = offences as (text: string) => string[];
// The samples are built from pieces, so this file passes the check it tests.
const hex = (h: string) => `#${h}`;
const fn = (f: string) => `${f}(`;

it('fails on a raw colour', () => {
  expect(check(`a { color: ${hex('fff')}; }`)).toHaveLength(1);
  expect(check(`const c = '${hex('00f2ff')}';`)).toHaveLength(1);
  expect(check(`b { background: ${fn('rgb')}0 0 0); }`)).toHaveLength(1);
  expect(check(`b { background: ${fn('oklch')}0.5 0.1 200); }`)).toHaveLength(1);
});

it('passes tokens, mixes of tokens, routes and ids', () => {
  expect(check('a { color: var(--mn-text); }')).toEqual([]);
  expect(check('a { background: color-mix(in srgb, var(--mn-holo) 10%, transparent); }')).toEqual(
    [],
  );
  expect(check("const r = '#/w/abc/s/def';")).toEqual([]);
  expect(check('#root { height: 100%; }')).toEqual([]);
  expect(check(`/* a comment may say ${hex('fff')} */`)).toEqual([]);
});

it('fails on HTML written as a string', () => {
  for (const s of [
    'dangerously' + 'SetInnerHTML',
    'el.inner' + 'HTML = x',
    'el.outer' + 'HTML',
    'insertAdjacent' + 'HTML',
    'document.' + 'write(x)',
  ]) {
    expect(check(s), s).toHaveLength(1);
  }
});
