#!/usr/bin/env node
/** `mnemorph`: the command line first, then a session in the terminal or one ask. */
import { spawnSync } from 'node:child_process';
import { Client, stdio } from '@mnemorph/sdk';
import { render } from 'ink';
import { App } from './app.js';
import { HELP, parse } from './args.js';
import { findCore } from './core.js';
import { RELEASE } from './index.js';
import { runOnce } from './run.js';
import { statusOnce } from './status.js';
import { chooseTheme } from './theme.js';

/** `onto …`: the core answers from the creature docs, with no session and no key. */
function onto(cmd: Extract<ReturnType<typeof parse>, { kind: 'onto' }>): number {
  const core = findCore(cmd.core);
  const head = cmd.raw ? [] : ['onto'];
  const args = [...head, ...cmd.words, ...(cmd.world ? ['--world', cmd.world] : [])];
  const ran = spawnSync(core, args, { stdio: 'inherit' });
  if (ran.error) {
    process.stderr.write(`mnemorph: the core ${core} did not start: ${ran.error.message}\n`);
    return 1;
  }
  return ran.status ?? 1;
}

async function main(): Promise<number> {
  const cmd = parse(process.argv.slice(2));
  switch (cmd.kind) {
    case 'help':
      process.stdout.write(HELP);
      return 0;
    case 'version':
      process.stdout.write(`mnemorph ${RELEASE}\n`);
      return 0;
    case 'error':
      process.stderr.write(`mnemorph: ${cmd.message}\n`);
      return 2;
    case 'run':
      return runOnce(cmd, process);
    case 'status':
      return statusOnce(cmd, process);
    case 'onto':
      return onto(cmd);
    case 'chat': {
      if (!process.stdin.isTTY || !process.stdout.isTTY) {
        process.stderr.write(
          'mnemorph: a session needs a terminal; to ask once from a script, use `mnemorph run "<ask>"`\n',
        );
        return 2;
      }
      const client = new Client(stdio({ command: findCore(cmd.core), world: cmd.world }));
      let hello: Awaited<ReturnType<Client['initialize']>>;
      try {
        hello = await client.initialize({ name: 'mnemorph', version: RELEASE });
      } catch (e) {
        // Before anything is drawn, so the reason reaches the terminal as written.
        process.stderr.write(`mnemorph: ${(e as Error).message}\n`);
        // The core waits to be asked again; nothing will ask it, so it is let go.
        client.close();
        return 1;
      }
      let lost: string | undefined;
      const ink = render(
        <App
          client={client}
          hello={hello}
          theme={chooseTheme(cmd.theme === 'auto' ? undefined : cmd.theme)}
          onCoreClosed={(why) => {
            lost = why;
          }}
        />,
        { exitOnCtrlC: false },
      );
      await ink.waitUntilExit();
      client.close();
      // A core that went away by itself is said on the terminal, after the last frame.
      if (lost !== undefined) {
        process.stderr.write(`mnemorph: the core closed: ${lost}\n`);
        return 1;
      }
      return 0;
    }
  }
}

main().then(
  (code) => {
    process.exitCode = code;
  },
  (e: Error) => {
    process.stderr.write(`mnemorph: ${e.message}\n`);
    process.exitCode = 1;
  },
);
