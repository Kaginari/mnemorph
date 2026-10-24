#!/usr/bin/env node
/** `mnemorph`: the command line first, then a session in the terminal or one ask. */
import { Client, stdio } from '@mnemorph/sdk';
import { render } from 'ink';
import { App } from './app.js';
import { HELP, parse } from './args.js';
import { findCore } from './core.js';
import { VERSION } from './index.js';
import { runOnce } from './run.js';

async function main(): Promise<number> {
  const cmd = parse(process.argv.slice(2));
  switch (cmd.kind) {
    case 'help':
      process.stdout.write(HELP);
      return 0;
    case 'version':
      process.stdout.write(`mnemorph ${VERSION}\n`);
      return 0;
    case 'error':
      process.stderr.write(`mnemorph: ${cmd.message}\n`);
      return 2;
    case 'run':
      return runOnce(cmd, process);
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
        hello = await client.initialize({ name: 'mnemorph', version: VERSION });
      } catch (e) {
        // Before anything is drawn, so the reason reaches the terminal as written.
        process.stderr.write(`mnemorph: ${(e as Error).message}\n`);
        return 1;
      }
      const ink = render(<App client={client} hello={hello} />, { exitOnCtrlC: false });
      await ink.waitUntilExit();
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
