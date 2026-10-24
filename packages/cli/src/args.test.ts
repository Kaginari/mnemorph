import { expect, test } from 'vitest';
import { parse } from './args.js';

test('help and the version answer before anything else is read', () => {
  expect(parse(['--bogus', '--help'])).toEqual({ kind: 'help' });
  expect(parse(['run', '-V'])).toEqual({ kind: 'version' });
});

test('a session, or one ask; arguments win over stdin', () => {
  expect(parse([])).toEqual({ kind: 'chat', world: undefined, core: undefined });
  expect(parse(['run', 'fix', 'the', 'test'])).toMatchObject({
    kind: 'run',
    ask: 'fix the test',
    fromStdin: false,
  });
  expect(parse(['run'])).toMatchObject({ kind: 'run', fromStdin: true });
  expect(parse(['run', '-'])).toMatchObject({ kind: 'run', fromStdin: true });
  expect(parse(['--world', '/w', 'run', 'x', '--json'])).toMatchObject({ world: '/w', json: true });
});

test('what is not understood is said, by name', () => {
  expect(parse(['--wat'])).toMatchObject({
    kind: 'error',
    message: expect.stringContaining('--wat'),
  });
  expect(parse(['go'])).toMatchObject({ kind: 'error', message: expect.stringContaining('go') });
  expect(parse(['--world'])).toMatchObject({ kind: 'error' });
});
