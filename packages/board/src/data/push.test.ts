// @vitest-environment jsdom
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { expect, it, vi } from 'vitest';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('./session.ts', () => ({ csrf: async () => 'cd'.repeat(32) }));

const { start, usePush } = await import('./push.ts');

import type { Change } from './push.ts';

/** A component that listens with `on`. */
function Listener({ on }: { on: (c: Change) => void }) {
  usePush(on);
  return null;
}

/** Mount a listener. */
async function listen(on: (c: Change) => void) {
  const root = createRoot(document.createElement('div'));
  await act(async () => root.render(createElement(Listener, { on })));
}

class FakeSocket extends EventTarget {
  constructor(
    readonly url: string,
    readonly protocols: string[],
  ) {
    super();
  }
}

it('offers its subprotocol and the CSRF entry, tells changes, and after a drop says all moved', async () => {
  vi.useFakeTimers();
  const made: FakeSocket[] = [];
  start((url, protocols) => {
    const s = new FakeSocket(url, protocols);
    made.push(s);
    return s as unknown as WebSocket;
  });
  await vi.waitFor(() => expect(made).toHaveLength(1));
  const [first] = made;
  expect(first?.url).toBe(`ws://${window.location.host}/ws/board`);
  expect(first?.protocols).toEqual(['mnemorph.board.v1', `mnemorph.csrf.${'cd'.repeat(32)}`]);
  // A listener, through the module's own set: the hook adds the same way.
  const heard: Change[] = [];
  await listen((c) => heard.push(c));
  await act(async () => first?.dispatchEvent(new Event('open')));
  first?.dispatchEvent(
    new MessageEvent('message', {
      data: '{"jsonrpc":"2.0","method":"world/changed","params":{"id":"a-1"}}',
    }),
  );
  first?.dispatchEvent(new MessageEvent('message', { data: '<b>not json</b>' }));
  expect(heard).toEqual([{ kind: 'world', id: 'a-1' }]);
  first?.dispatchEvent(new Event('close'));
  await vi.advanceTimersByTimeAsync(1000);
  await vi.waitFor(() => expect(made).toHaveLength(2));
  await act(async () => made[1]?.dispatchEvent(new Event('open')));
  expect(heard).toEqual([{ kind: 'world', id: 'a-1' }, { kind: 'worlds' }]);
  vi.useRealTimers();
});
