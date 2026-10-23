import { renderToString } from 'react-dom/server';
import { expect, test } from 'vitest';
import { App } from './App.tsx';

test('the board renders', () => {
  expect(renderToString(<App />)).toContain('Mnemorph');
});
