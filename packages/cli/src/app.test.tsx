import { Client } from '@mnemorph/sdk';
import { renderToString } from 'ink';
import { render } from 'ink-testing-library';
import { expect, test } from 'vitest';
import { App, ItemView, tail } from './app.js';
import { FakeCore } from './fake.js';

const hello = {
  protocol: 1,
  version: 't',
  world: '/w',
  model: 'm',
  provider: 'p',
  session: 's',
  run: 'r',
  holes: [],
};
const tick = (ms = 20) => new Promise((r) => setTimeout(r, ms));

function start() {
  const core = new FakeCore();
  const client = new Client(core);
  const ui = render(<App client={client} hello={hello} />);
  return { core, ui };
}

async function type(ui: ReturnType<typeof render>, text: string) {
  for (const ch of text) {
    ui.stdin.write(ch);
    await tick(2);
  }
}

test('what is typed is sent, a capital C included, and the turn streams in', async () => {
  const { core, ui } = start();
  await tick();
  await type(ui, 'Cargo build');
  expect(ui.lastFrame()).toContain('Cargo build');
  ui.stdin.write('\r');
  await tick();
  expect(core.sent.find((m) => m.method === 'session/prompt')).toMatchObject({
    params: { text: 'Cargo build' },
  });
  core.event('turnStarted', { text: 'Cargo build' });
  core.event('toolCall', { id: 't1', name: 'bash', input: { command: 'cargo build' } });
  core.event('textDelta', { text: 'Building' });
  await tick();
  const frame = ui.lastFrame() ?? '';
  expect(frame).toContain('Building');
  expect(frame).toContain('cargo build …');
  expect(frame).toContain('esc to stop');
  ui.unmount();
});

test('Esc during a turn interrupts it', async () => {
  const { core, ui } = start();
  await tick();
  core.event('turnStarted', { text: 'long' });
  await tick();
  ui.stdin.write('\u001b');
  await tick(80);
  expect(core.methods()).toContain('session/interrupt');
  ui.unmount();
});

test('an approval needs a y; Enter alone approves nothing', async () => {
  const { core, ui } = start();
  await tick();
  core.say({
    jsonrpc: '2.0',
    id: 'gate-1',
    method: 'gate/ask',
    params: {
      class: 'destructive',
      act: 'rm -rf build',
      race: 'slime',
      why: ['deletes files'],
      outside: [],
    },
  });
  await tick();
  expect(ui.lastFrame()).toContain('Approve this destructive act?');
  ui.stdin.write('\r');
  await tick();
  expect(core.sent.find((m) => m.id === 'gate-1')).toBeUndefined();
  ui.stdin.write('y');
  await tick();
  expect(core.sent.find((m) => m.id === 'gate-1')).toMatchObject({ result: { allow: true } });
  expect(ui.lastFrame()).not.toContain('Approve this');
  ui.unmount();
});

test('n denies an approval, with a reason for the body', async () => {
  const { core, ui } = start();
  await tick();
  core.say({
    jsonrpc: '2.0',
    id: 'gate-2',
    method: 'gate/ask',
    params: { class: 'write', act: 'sed -i x f', race: 'slime', why: [], outside: [] },
  });
  await tick();
  ui.stdin.write('n');
  await tick();
  expect(core.sent.find((m) => m.id === 'gate-2')).toMatchObject({
    result: { allow: false, reason: 'the human said no' },
  });
  ui.unmount();
});

test('/help is answered here, and sends nothing', async () => {
  const { core, ui } = start();
  await tick();
  await type(ui, '/help');
  ui.stdin.write('\r');
  await tick();
  expect(ui.lastFrame()).toContain('/quit');
  expect(core.methods()).not.toContain('session/prompt');
  ui.unmount();
});

test('keys that arrive together are taken in order, Enter among them', async () => {
  const { core, ui } = start();
  await tick();
  ui.stdin.write('first\rsecond\r');
  await tick(60);
  const sent = core.sent
    .filter((m) => m.method === 'session/prompt')
    .map((m) => (m.params as { text: string }).text);
  expect(sent).toEqual(['first', 'second']);
  ui.unmount();
});

test('long lines wrap beside their prefix, never past the width', () => {
  const text =
    'please read every file under src and tell me which of them still use the old config loader so that I can move them';
  for (const columns of [50, 80, 120]) {
    for (const item of [
      { kind: 'user', id: 1, text },
      { kind: 'assistant', id: 2, text },
    ] as const) {
      const out = renderToString(<ItemView item={item} />, { columns });
      const lines = out.split('\n').filter((l) => l.trim());
      for (const line of lines) expect(line.length).toBeLessThanOrEqual(columns);
      expect(lines[0]?.startsWith(item.kind === 'user' ? '❯ ' : '● ')).toBe(true);
      for (const line of lines.slice(1)) expect(line.startsWith('  ')).toBe(true);
    }
  }
});

test('the live text keeps to the rows it has', () => {
  const text = Array.from({ length: 40 }, (_, i) => `line ${i}`).join('\n');
  expect(tail(text, 5).split('\n')).toEqual([
    'line 35',
    'line 36',
    'line 37',
    'line 38',
    'line 39',
  ]);
  expect(tail('short', 5)).toBe('short');
});

test('an approval the turn left behind is denied, and Ctrl+C at an approval denies and stops', async () => {
  const { core, ui } = start();
  await tick();
  core.event('turnStarted', { text: 'go' });
  core.say({
    jsonrpc: '2.0',
    id: 'gate-1',
    method: 'gate/ask',
    params: { class: 'write', act: 'sed -i x f', race: 'slime', why: [], outside: [] },
  });
  await tick();
  core.event('turnEnded', { ending: 'interrupted' });
  await tick();
  expect(core.sent.find((m) => m.id === 'gate-1')).toMatchObject({ result: { allow: false } });
  expect(ui.lastFrame()).not.toContain('Approve this');
  core.event('turnStarted', { text: 'again' });
  core.say({
    jsonrpc: '2.0',
    id: 'gate-2',
    method: 'gate/ask',
    params: { class: 'write', act: 'sed -i y f', race: 'slime', why: [], outside: [] },
  });
  await tick();
  ui.stdin.write('\u0003');
  await tick(60);
  expect(core.sent.find((m) => m.id === 'gate-2')).toMatchObject({ result: { allow: false } });
  expect(core.methods()).toContain('session/interrupt');
  ui.unmount();
});

test('a core that goes away by itself is told to the caller, with why', async () => {
  const core = new FakeCore();
  const client = new Client(core);
  let lost: string | undefined;
  const ui = render(
    <App
      client={client}
      hello={hello}
      onCoreClosed={(why) => {
        lost = why;
      }}
    />,
  );
  await tick();
  core.end('mnemorphd exited with 3');
  await tick();
  expect(lost).toBe('mnemorphd exited with 3');
  ui.unmount();
});
