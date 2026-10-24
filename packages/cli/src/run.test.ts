import { PassThrough } from 'node:stream';
import { expect, test } from 'vitest';
import { parse } from './args.js';
import { FakeCore } from './fake.js';
import { runOnce } from './run.js';

function io(stdin = new PassThrough()) {
  const out = new PassThrough();
  const err = new PassThrough();
  let stdout = '';
  let stderr = '';
  out.on('data', (d) => {
    stdout += d;
  });
  err.on('data', (d) => {
    stderr += d;
  });
  return { stdin, stdout: out, stderr: err, text: () => ({ stdout, stderr }) };
}

/** The core answers the prompt with a short turn. */
function answering(core: FakeCore, ending = 'answered') {
  const send = core.send.bind(core);
  core.send = (line: string) => {
    send(line);
    if (JSON.parse(line).method === 'session/prompt') {
      setTimeout(() => {
        core.event('turnStarted', { text: 'x' });
        core.event('toolCall', { id: 't', name: 'bash', input: { command: 'ls' } });
        core.event('textDelta', { text: 'done.' });
        core.event('turnEnded', { ending });
      }, 5);
    }
  };
}

test('one ask prints its answer and exits 0', async () => {
  const core = new FakeCore();
  answering(core);
  const streams = io();
  const code = await runOnce(parse(['run', 'list it']) as never, streams, core);
  expect(code).toBe(0);
  expect(streams.text().stdout).toBe('done.\n');
  expect(streams.text().stderr).toContain('⏺ bash ls');
  expect(core.sent.find((m) => m.method === 'session/prompt')).toMatchObject({
    params: { text: 'list it' },
  });
});

test('an ask on the command line never waits on an open stdin', async () => {
  const core = new FakeCore();
  answering(core);
  // stdin is a pipe that never ends, as under CI, cron or another agent.
  const streams = io(new PassThrough());
  const code = await Promise.race([
    runOnce(parse(['run', 'go']) as never, streams, core),
    new Promise<string>((r) => setTimeout(() => r('hung'), 2000)),
  ]);
  expect(code).toBe(0);
});

test('`run -` reads the ask from stdin', async () => {
  const core = new FakeCore();
  answering(core);
  const stdin = new PassThrough();
  stdin.end('from the pipe\n');
  const code = await runOnce(parse(['run', '-']) as never, io(stdin), core);
  expect(code).toBe(0);
  expect(core.sent.find((m) => m.method === 'session/prompt')).toMatchObject({
    params: { text: 'from the pipe' },
  });
});

test('a turn that does not answer exits 1 and says how it ended', async () => {
  const core = new FakeCore();
  answering(core, 'checkpoint: steps: 40 taken');
  const streams = io();
  expect(await runOnce(parse(['run', 'go']) as never, streams, core)).toBe(1);
  expect(streams.text().stderr).toContain('checkpoint: steps: 40 taken');
});

test('a core that will not start says why, on stderr, and exits 1', async () => {
  const core = new FakeCore();
  core.answers.initialize = new Error('ANTHROPIC_API_KEY is not set');
  const streams = io();
  expect(await runOnce(parse(['run', 'go']) as never, streams, core)).toBe(1);
  expect(streams.text().stderr).toContain('ANTHROPIC_API_KEY is not set');
});

test('run asks no one: an act that needs approval is denied, and says so', async () => {
  const core = new FakeCore();
  answering(core);
  const streams = io();
  const done = runOnce(parse(['run', 'go']) as never, streams, core);
  await new Promise((r) => setTimeout(r, 1));
  core.say({
    jsonrpc: '2.0',
    id: 'gate-1',
    method: 'gate/ask',
    params: { class: 'destructive', act: 'rm -rf build', race: 'slime', why: [], outside: [] },
  });
  await done;
  expect(core.sent.find((m) => m.id === 'gate-1')).toMatchObject({ result: { allow: false } });
  expect(streams.text().stderr).toContain('denied destructive act `rm -rf build`');
});
