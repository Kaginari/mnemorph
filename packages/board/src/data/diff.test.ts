import { expect, it } from 'vitest';
import { changeOf, stat } from './diff.ts';

it('an edit is a numbered diff with its context', () => {
  const c = changeOf('edit', {
    path: 'p.py',
    old_string: 'a\nreturn this\nb\n',
    new_string: 'a\nreturn self._parse_collate(this)\nb\n',
  });
  expect(c && stat(c)).toBe('+1 −1');
  expect(c?.kind === 'diff' && c.lines).toEqual([
    { kind: 'same', text: 'a', n: 1 },
    { kind: 'del', text: 'return this' },
    { kind: 'add', text: 'return self._parse_collate(this)', n: 2 },
    { kind: 'same', text: 'b', n: 3 },
  ]);
});

it('a patch keeps its hunks and numbers from them', () => {
  const c = changeOf('patch', { patch: '--- a/x\n+++ b/x\n@@ -7 +7,2 @@\n-old\n+new\n ctx\n' });
  expect(c && stat(c)).toBe('+1 −1');
  expect(c?.kind === 'diff' && c.lines).toEqual([
    { kind: 'hunk', text: '@@ -7 +7,2 @@' },
    { kind: 'del', text: 'old' },
    { kind: 'add', text: 'new', n: 7 },
    { kind: 'same', text: 'ctx', n: 8 },
  ]);
});

it('a write is its head, and a read is no change', () => {
  const c = changeOf('write', { path: 'n.md', content: '1\n2\n3\n4\n5\n6\n' });
  expect(c).toEqual({ kind: 'file', path: 'n.md', lines: 6, head: ['1', '2', '3', '4'] });
  expect(changeOf('read', { path: 'x' })).toBeUndefined();
});

it('distant changes in one edit are two hunks', () => {
  const old = Array.from({ length: 20 }, (_, i) => `l${i}`).join('\n');
  const neu = old.replace('l2', 'L2').replace('l17', 'L17');
  const c = changeOf('edit', { old_string: old, new_string: neu });
  expect(c?.kind === 'diff' && c.lines.filter((l) => l.kind === 'hunk')).toHaveLength(1);
});
