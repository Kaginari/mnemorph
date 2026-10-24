import { chmodSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, expect, test } from 'vitest';
import { type Arrived, Client, RpcError, stdio, type Transport } from './index.js';

/** A core in memory: it sees what the client sends and says what the test makes it say. */
class Fake implements Transport {
  sent: Record<string, unknown>[] = [];
  private line: (l: string) => void = () => {};
  private closer: (why: string) => void = () => {};
  send(line: string) {
    this.sent.push(JSON.parse(line));
  }
  onLine(listener: (line: string) => void) {
    this.line = listener;
  }
  onClose(listener: (why: string) => void) {
    this.closer = listener;
  }
  close() {}
  say(msg: unknown) {
    this.line(JSON.stringify(msg));
  }
  end(why: string) {
    this.closer(why);
  }
  /** Answer the last request with `result`. */
  answer(result: unknown) {
    const last = this.sent.at(-1) as { id: number };
    this.say({ jsonrpc: '2.0', id: last.id, result });
  }
}

const event = (type: string, seq: number, fields: Record<string, unknown> = {}) => ({
  jsonrpc: '2.0',
  method: `event/${type}`,
  params: { session: 's', run: 'r', body: 'rimuru', seq, ...fields },
});

test('requests are answered by id, errors as RpcError', async () => {
  const core = new Fake();
  const c = new Client(core);
  const hello = c.initialize({ name: 'test' });
  expect(core.sent[0]).toMatchObject({
    method: 'initialize',
    params: { clientInfo: { name: 'test' } },
  });
  core.answer({ protocol: 1, world: '/w', model: 'm' });
  expect((await hello).world).toBe('/w');
  const prompt = c.prompt('hi');
  core.say({ jsonrpc: '2.0', id: 2, error: { code: -32602, message: 'needs text' } });
  await expect(prompt).rejects.toBeInstanceOf(RpcError);
  // The session goes on after an error.
  const again = c.prompt('again');
  core.answer({ queued: false });
  expect((await again).queued).toBe(false);
});

test('a core that speaks another protocol is refused', async () => {
  const core = new Fake();
  const c = new Client(core);
  const hello = c.initialize({ name: 'test' });
  core.answer({ protocol: 2 });
  await expect(hello).rejects.toThrow(/protocol 2/);
});

test('events arrive in order, once each, even when replayed', async () => {
  const core = new Fake();
  const c = new Client(core);
  const seen: Arrived[] = [];
  c.on((e) => seen.push(e));
  core.say(event('turnStarted', 1, { text: 'go' }));
  core.say(event('textDelta', 2, { text: 'he' }));
  const replay = c.resync();
  expect(core.sent.at(-1)).toMatchObject({ method: 'session/events', params: { since: 2 } });
  core.answer({
    events: [event('textDelta', 2, { text: 'he' }), event('textDelta', 3, { text: 'llo' })],
    oldest: 1,
    complete: true,
  });
  await replay;
  expect(seen.map((e) => [e.type, e.seq])).toEqual([
    ['turnStarted', 1],
    ['textDelta', 2],
    ['textDelta', 3],
  ]);
  expect(c.lastSeq).toBe(3);
});

test('the gate is answered by the handler, and denied with no handler', async () => {
  const core = new Fake();
  const c = new Client(core);
  core.say({ jsonrpc: '2.0', id: 'gate-1', method: 'gate/ask', params: { act: 'rm -rf build' } });
  await new Promise((r) => setTimeout(r, 0));
  expect(core.sent.at(-1)).toMatchObject({ id: 'gate-1', result: { allow: false } });
  c.onGate((req) => ({ allow: req.act.startsWith('git'), reason: 'not that one' }));
  core.say({ jsonrpc: '2.0', id: 'gate-2', method: 'gate/ask', params: { act: 'git status' } });
  await new Promise((r) => setTimeout(r, 0));
  expect(core.sent.at(-1)).toMatchObject({ id: 'gate-2', result: { allow: true } });
});

test('a closed connection fails what waits and refuses what follows', async () => {
  const core = new Fake();
  const c = new Client(core);
  const waiting = c.prompt('hi');
  core.end('mnemorphd exited with 1');
  await expect(waiting).rejects.toThrow('exited with 1');
  await expect(c.prompt('again')).rejects.toThrow('exited with 1');
  expect(c.closed).toBe('mnemorphd exited with 1');
});

// The real core, when it is built: a world whose backend needs no key opens, and shuts down.
const daemon = resolve(__dirname, '../../../target/debug/mnemorphd');
let dir: string | undefined;
afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

test.runIf(existsSync(daemon))('speaks to the real mnemorphd over stdio', async () => {
  dir = mkdtempSync(join(tmpdir(), 'mnemorph-sdk-'));
  mkdirSync(join(dir, '.mnemorph'));
  writeFileSync(
    join(dir, '.mnemorph/config.local.toml'),
    '[providers.local]\nkind = "openai"\nbase_url = "http://127.0.0.1:9"\n[models]\nprovider = "local"\ndefault = "m"\n',
  );
  const c = new Client(
    stdio({
      command: daemon,
      world: dir,
      env: { PATH: process.env.PATH, HOME: dir, XDG_STATE_HOME: dir },
    }),
  );
  const hello = await c.initialize({ name: 'sdk-test' });
  expect(hello.protocol).toBe(1);
  expect(hello.model).toBe('m');
  expect(hello.provider).toBe('local');
  await c.shutdown();
  // After shutdown nothing is written: a request is refused, never a crash.
  await expect(c.prompt('after')).rejects.toThrow('shut down');
});

/** A stand-in core: a shell script given as the command. */
function core(name: string, script: string): string {
  dir ??= mkdtempSync(join(tmpdir(), 'mnemorph-sdk-'));
  const path = join(dir, name);
  writeFileSync(path, `#!/bin/sh\n${script}\n`);
  chmodSync(path, 0o755);
  return path;
}

test('the last line a core writes before it exits is never lost', async () => {
  const quick = core(
    'quick',
    `read line; echo '{"jsonrpc":"2.0","id":1,"result":{"protocol":1,"session":"s"}}'`,
  );
  for (let i = 0; i < 50; i++) {
    const c = new Client(stdio({ command: quick }));
    await expect(c.initialize({ name: 'quick' })).resolves.toMatchObject({ protocol: 1 });
  }
});

test('a core whose input is gone fails the request, never the host', async () => {
  const crashes: unknown[] = [];
  const count = (e: unknown) => crashes.push(e);
  process.on('uncaughtException', count);
  try {
    const deaf = core('deaf', 'exec 0<&-; sleep 1');
    const c = new Client(stdio({ command: deaf }));
    await new Promise((r) => setTimeout(r, 100));
    await expect(c.prompt('hello')).rejects.toThrow();
    await new Promise((r) => setTimeout(r, 100));
  } finally {
    process.off('uncaughtException', count);
  }
  expect(crashes).toEqual([]);
});
