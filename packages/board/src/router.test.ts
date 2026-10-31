import { describe, expect, it } from 'vitest';
import { format, parse, type Route } from './router.ts';

describe('the router', () => {
  it.each<[string, Route]>([
    ['', { name: 'worlds' }],
    ['#', { name: 'worlds' }],
    ['#/', { name: 'worlds' }],
    ['#/metrics', { name: 'metrics' }],
    ['#/integrations/', { name: 'integrations' }],
    ['#/settings', { name: 'settings' }],
    ['#/w/sqlglot-9a1c', { name: 'world', world: 'sqlglot-9a1c' }],
    ['#/w/sqlglot-9a1c/memory', { name: 'section', world: 'sqlglot-9a1c', section: 'memory' }],
    ['#/w/a/s/r-17', { name: 'session', world: 'a', session: 'r-17' }],
    ['#/w/a%20b/s/x%2Fy', { name: 'session', world: 'a b', session: 'x/y' }],
    ['#/w/a/r/r1-2', { name: 'run', world: 'a', run: 'r1-2' }],
    ['#/w/a/bodies/queue', { name: 'section', world: 'a', section: 'bodies', tab: 'queue' }],
  ])('reads %j', (hash, route) => expect(parse(hash)).toEqual(route));

  it.each([
    '#/nope',
    '#/w',
    '#/w/',
    '#/w/a/s',
    '#/w/a/s/',
    '#/w/a/r',
    '#/w/a/nope',
    '#/w/a/memory/extra',
    '#/w/a/memory/queue',
    '#/w/a/bodies/nope',
    '#/metrics/extra',
    '#/w/%E0%A4%A/s/x',
    '#/w/a/s/b/c/d',
  ])('gives an unknown route for %j, carrying the path', (hash) => {
    expect(parse(hash)).toEqual({ name: 'unknown', path: hash.slice(1) });
  });

  it.each<Route>([
    { name: 'worlds' },
    { name: 'metrics' },
    { name: 'settings' },
    { name: 'world', world: 'x-1' },
    { name: 'section', world: 'x/1', section: 'ontology' },
    { name: 'session', world: 'w?#', session: 's p' },
    { name: 'run', world: 'w', run: 'r1-2' },
    { name: 'section', world: 'w', section: 'bodies', tab: 'timeline' },
  ])('formats %j and reads it back', (route) => expect(parse(format(route))).toEqual(route));
});
