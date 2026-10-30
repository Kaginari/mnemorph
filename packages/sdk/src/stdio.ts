/** The stdio transport: `mnemorphd` started as a child, spoken to on its stdin and stdout. Node
 * only. */
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import type { Transport } from './client.js';

/** Start `mnemorphd` and speak to it on its stdin and stdout. */
export function stdio(
  options: {
    command?: string | undefined;
    world?: string | undefined;
    /** Open the session on this stopped run — its id, or `last` — instead of a new one. */
    resume?: string | undefined;
    env?: NodeJS.ProcessEnv | undefined;
  } = {},
): Transport {
  const args = [
    ...(options.world ? ['--world', options.world] : []),
    ...(options.resume ? ['--resume', options.resume] : []),
  ];
  const child = spawn(options.command ?? 'mnemorphd', args, {
    stdio: ['pipe', 'pipe', 'pipe'],
    env: options.env ?? process.env,
  });
  const lines = createInterface({ input: child.stdout });
  let stderr = '';
  child.stderr.on('data', (d: Buffer) => {
    stderr += d.toString();
  });
  const closers: ((why: string) => void)[] = [];
  let closed = false;
  const close = (why: string) => {
    if (closed) return;
    closed = true;
    for (const c of closers) c(why);
  };
  child.on('error', (e: Error) => close(`mnemorphd could not start: ${e.message}`));
  // A write on a dead pipe is an error event, never a crash of the host.
  child.stdin.on('error', (e: Error) => close(`mnemorphd's input closed: ${e.message}`));
  // 'close', not 'exit': it fires once the core's output is drained, so its last lines arrive.
  child.on('close', (code: number | null, signal: NodeJS.Signals | null) =>
    close(
      stderr.trim() ||
        (signal ? `mnemorphd was stopped by ${signal}` : `mnemorphd exited with ${code ?? 0}`),
    ),
  );
  return {
    send: (line) => {
      if (!closed) child.stdin.write(`${line}\n`);
    },
    onLine: (listener) => lines.on('line', listener),
    onClose: (listener) => closers.push(listener),
    close: () => {
      if (!closed) child.stdin.end();
      close('the client closed the connection');
      // A core that does not leave when its input ends is stopped.
      setTimeout(() => {
        if (child.exitCode === null && child.signalCode === null) child.kill();
      }, 1000).unref();
    },
  };
}
