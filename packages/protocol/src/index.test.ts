import { expect, test } from 'vitest';
import { PROTOCOL_VERSION_NAME } from './index.js';

test('the package names itself', () => {
  expect(PROTOCOL_VERSION_NAME).toBe('@mnemorph/protocol');
});
