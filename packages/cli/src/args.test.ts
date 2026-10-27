import { expect, test } from 'vitest';
import { parse } from './args.js';

test('help and the version answer before anything else is read', () => {
  expect(parse(['--bogus', '--help'])).toEqual({ kind: 'help' });
  expect(parse(['run', '-V'])).toEqual({ kind: 'version' });
});

test('a session, or one ask; arguments win over stdin', () => {
  expect(parse([])).toEqual({ kind: 'chat', world: undefined, core: undefined, theme: undefined });
  expect(parse(['--theme', 'light'])).toMatchObject({ kind: 'chat', theme: 'light' });
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
  expect(parse(['--theme', 'blue'])).toMatchObject({
    kind: 'error',
    message: expect.stringContaining('blue'),
  });
});

test('onto hands its words to the core', () => {
  expect(parse(['onto', 'owner', 'a.rs', '--world', 'w'])).toEqual({
    kind: 'onto',
    words: ['owner', 'a.rs'],
    world: 'w',
    core: undefined,
  });
  expect(parse(['onto', 'owner', '--json']).kind).toBe('error');
});

test('q hands its words to the core whole, --json with them', () => {
  expect(parse(['q', 'add', 'tidy', '--for', 'slime-s', '--json'])).toEqual({
    kind: 'onto',
    words: ['q', 'add', 'tidy', '--for', 'slime-s', '--json'],
    world: undefined,
    core: undefined,
    raw: true,
  });
});

test('q is a command only as the first word that is not a flag value', () => {
  expect(parse(['--world', 'q', 'status']).kind).not.toBe('onto');
  expect(parse(['--world', 'w', '--theme', 'dark', 'q', 'list'])).toEqual({
    kind: 'onto',
    words: ['q', 'list'],
    world: 'w',
    core: undefined,
    raw: true,
  });
  expect(parse(['q', 'add', 'x', '--world', 'elsewhere'])).toMatchObject({
    words: ['q', 'add', 'x', '--world', 'elsewhere'],
  });
});

test('housekeep hands its words to the core whole', () => {
  expect(parse(['housekeep', '--heal'])).toMatchObject({
    kind: 'onto',
    words: ['housekeep', '--heal'],
    raw: true,
  });
});

test('memory status hands its words to the core whole', () => {
  expect(parse(['--world', 'w', 'memory', 'status', '--json'])).toEqual({
    kind: 'onto',
    words: ['memory', 'status', '--json'],
    world: 'w',
    core: undefined,
    raw: true,
  });
});

test('resume opens a session on a stopped run; runs lists them through the core', () => {
  expect(parse(['resume', 'last', '--world', 'w'])).toEqual({
    kind: 'chat',
    world: 'w',
    core: undefined,
    theme: undefined,
    resume: 'last',
  });
  expect(parse(['resume', 'r1-2'])).toMatchObject({ kind: 'chat', resume: 'r1-2' });
  expect(parse(['resume'])).toMatchObject({
    kind: 'error',
    message: expect.stringContaining('mnemorph runs'),
  });
  expect(parse(['resume', 'a', 'b'])).toMatchObject({ kind: 'error' });
  expect(parse(['runs', '--json'])).toEqual({
    kind: 'onto',
    words: ['runs', '--json'],
    world: undefined,
    core: undefined,
    raw: true,
  });
});
