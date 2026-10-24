import { Client } from '@mnemorph/sdk';
import { render } from 'ink-testing-library';
import { expect, test } from 'vitest';
import { App } from './app.js';
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
