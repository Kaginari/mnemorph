import { expect, test } from 'vitest';
import { eventMethod, eventType, PROTOCOL } from './index.js';

test('speaks protocol 1', () => {
  expect(PROTOCOL).toBe(1);
});

test('an event type and its method name each other', () => {
  expect(eventMethod('textDelta')).toBe('event/textDelta');
  expect(eventType('event/turnEnded')).toBe('turnEnded');
  expect(eventType('gate/ask')).toBeUndefined();
});
