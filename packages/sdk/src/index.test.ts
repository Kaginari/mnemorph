import { expect, test } from 'vitest';
import { SDK_NAME } from './index.js';

test('the package names itself', () => {
  expect(SDK_NAME).toBe('@tempest/sdk');
});
