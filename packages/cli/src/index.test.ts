import { expect, test } from 'vitest';
import { CLI_NAME } from './index.js';

test('the package names itself', () => {
  expect(CLI_NAME).toBe('mnemorph');
});
