import { expect, test } from 'vitest';
import * as ed from './editor.js';

const type = (e: ed.Editor, s: string) => ed.insert(e, s);

test('typing, moving and deleting', () => {
  let e = type(ed.empty(), 'helo');
  e = ed.left(e);
  e = type(e, 'l');
  expect(e.text).toBe('hello');
  e = ed.end(e);
  e = ed.backspace(e);
  expect([e.text, e.cursor]).toEqual(['hell', 4]);
  e = ed.home(e);
  e = ed.del(e);
  expect(e.text).toBe('ell');
});

test('a capital letter is text, like any other', () => {
  expect(type(ed.empty(), 'Cargo C').text).toBe('Cargo C');
});

test('a paste keeps its lines, and the cursor moves between them', () => {
  let e = type(ed.empty(), 'one\r\ntwo\nthree');
  expect(e.text).toBe('one\ntwo\nthree');
  e = ed.up(e);
  expect(e.text.slice(0, e.cursor)).toBe('one\ntwo');
  e = ed.home(e);
  e = ed.up(e);
  expect(e.cursor).toBe(0);
  e = ed.down(e);
  expect(e.cursor).toBe(4);
});

test('words and lines are killed', () => {
  let e = type(ed.empty(), 'git commit -m');
  e = ed.killWord(e);
  expect(e.text).toBe('git commit ');
  e = ed.killBack(e);
  expect(e.text).toBe('');
});

test('the history walks back and returns to the draft', () => {
  const [line, first] = ed.submit(type(ed.empty(), 'first'));
  let e = first;
  expect(line).toBe('first');
  [, e] = ed.submit(type(e, 'second'));
  e = type(e, 'draft');
  e = ed.up(e);
  expect(e.text).toBe('second');
  e = ed.up(e);
  expect(e.text).toBe('first');
  e = ed.down(e);
  e = ed.down(e);
  expect(e.text).toBe('draft');
  expect(ed.submit(type(ed.empty(), '   '))[0]).toBe('');
});
