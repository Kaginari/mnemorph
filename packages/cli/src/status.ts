/** `mnemorph status`: what this build is, and what the core in force says about itself. */
import { Client, stdio, type Transport } from '@mnemorph/sdk';
import type { Command } from './args.js';
import { findCore } from './core.js';
import { RELEASE } from './index.js';
import type { Io } from './run.js';
import { wt } from './transcript.js';

type Hello = Awaited<ReturnType<Client['initialize']>>;

/** The lines `status` prints for a core that started. */
export function report(hello: Hello, core: string): string[] {
  const lines = [
    ['core', `${hello.version} — ${core}`],
    ['world', hello.world],
    ['model', `${hello.model} via ${hello.provider}`],
    // The key's variable and the file that names it; its value is never read back.
    ['key', hello.key ?? 'none — the provider takes no key'],
    ['sandbox', hello.sandbox],
    [
      'budget',
      hello.budget ? `left ${wt(hello.budget.leftWt)} wt (${hello.budget.cap})` : 'no caps set',
    ],
    ...hello.holes.map((h) => ['hole', h]),
  ];
  return lines.map(([k, v]) => `${(k ?? '').padEnd(9)}${v}`);
}

export async function statusOnce(
  cmd: Extract<Command, { kind: 'status' }>,
  io: Io,
  transport?: Transport,
): Promise<number> {
  const core = findCore(cmd.core);
  io.stdout.write(`${'mnemorph'.padEnd(9)}${RELEASE}\n`);
  const client = new Client(transport ?? stdio({ command: core, world: cmd.world }));
  try {
    const hello = await client.initialize({ name: 'mnemorph status', version: RELEASE });
    io.stdout.write(`${report(hello, core).join('\n')}\n`);
    await client.shutdown().catch(() => undefined);
    return 0;
  } catch (e) {
    io.stdout.write(`${'core'.padEnd(9)}did not start — ${(e as Error).message}\n`);
    return 1;
  } finally {
    client.close();
  }
}
