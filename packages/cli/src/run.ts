/** `mnemorph run`: one ask, the answer on stdout, the rest on stderr, an exit code that tells. */
import { Client, stdio, type Transport } from '@mnemorph/sdk';
import type { Command } from './args.js';
import { findCore } from './core.js';
import { summary } from './transcript.js';

export interface Io {
  stdin: NodeJS.ReadableStream;
  stdout: NodeJS.WritableStream;
  stderr: NodeJS.WritableStream;
}

async function readAll(stream: NodeJS.ReadableStream): Promise<string> {
  let text = '';
  for await (const chunk of stream) text += chunk.toString();
  return text;
}

/** Ask once. Answered is 0; anything else — a checkpoint, a failure, a refusal to start — is 1. */
export async function runOnce(
  cmd: Extract<Command, { kind: 'run' }>,
  io: Io,
  transport?: Transport,
): Promise<number> {
  // Arguments win: stdin is read only when the ask is not on the command line.
  const ask = cmd.fromStdin ? (await readAll(io.stdin)).trim() : (cmd.ask ?? '');
  if (!ask) {
    io.stderr.write('mnemorph run: nothing to ask\n');
    return 1;
  }
  const client = new Client(transport ?? stdio({ command: findCore(cmd.core), world: cmd.world }));
  client.onGate((req) => {
    io.stderr.write(`mnemorph: denied ${req.class} act \`${req.act}\` — run asks no one\n`);
    return {
      allow: false,
      reason: 'mnemorph run has no human to approve it; ask in a session instead',
    };
  });
  const ended = new Promise<string>((resolve) => {
    client.on((e) => {
      if (cmd.json) {
        io.stdout.write(`${JSON.stringify(e)}\n`);
      } else if (e.type === 'textDelta') {
        io.stdout.write(e.text);
      } else if (e.type === 'toolCall') {
        io.stderr.write(`⏺ ${e.name} ${summary(e.name, e.input)}\n`);
      } else if (e.type === 'toolResult' && e.isError) {
        io.stderr.write(`  ✗ ${e.content.split('\n')[0]}\n`);
      } else if (e.type === 'warning') {
        io.stderr.write(`! ${e.text}\n`);
      }
      if (e.type === 'turnEnded') resolve(e.ending);
    });
  });
  try {
    await client.initialize({ name: 'mnemorph run', version: '0.0.0' });
    await client.prompt(ask);
  } catch (e) {
    io.stderr.write(`mnemorph: ${(e as Error).message}\n`);
    return 1;
  }
  const ending = await Promise.race([
    ended,
    new Promise<string>((resolve) => client.onClosed((why) => resolve(`failed: ${why}`))),
  ]);
  if (!cmd.json) io.stdout.write('\n');
  if (ending !== 'answered') io.stderr.write(`mnemorph: ${ending}\n`);
  await client.shutdown().catch(() => {});
  return ending === 'answered' ? 0 : 1;
}
