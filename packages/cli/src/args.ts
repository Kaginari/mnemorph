/** The command line, read before anything else: help and the version never look for a world. */

export type Command =
  | { kind: 'help' }
  | { kind: 'version' }
  | { kind: 'chat'; world?: string | undefined; core?: string | undefined }
  | {
      kind: 'run';
      ask?: string | undefined;
      fromStdin: boolean;
      world?: string | undefined;
      core?: string | undefined;
      json: boolean;
    }
  | { kind: 'error'; message: string };

export const HELP = `mnemorph — talk to a Mnemorph body in your terminal

Usage:
  mnemorph [options]                 a session in this world
  mnemorph run "<ask>" [options]     ask once, print the answer, exit
  mnemorph run [-]                   ask once, the ask read from stdin

Options:
  --world <dir>    the world to work in (default: the current directory)
  --core <path>    the mnemorphd to start (default: MNEMORPH_CORE, then PATH)
  --json           with run: print every event as a JSON line
  -h, --help       this help
  -V, --version    the version

In a session:
  Enter            send; while a turn runs, the line waits its turn
  Alt+Enter        a new line in the message (Shift+Enter where the terminal reports it)
  Esc              stop the turn; at an approval, deny it
  Ctrl+C           stop the turn; when nothing runs, quit
  ↑ ↓              earlier messages
  /help /quit      the session's commands
`;

/** Read `argv` (without node and the script). */
export function parse(argv: string[]): Command {
  // Help and the version win wherever they appear: nothing else is read.
  if (argv.some((a) => a === '-h' || a === '--help')) return { kind: 'help' };
  if (argv.some((a) => a === '-V' || a === '--version')) return { kind: 'version' };
  let world: string | undefined;
  let core: string | undefined;
  let json = false;
  const rest: string[] = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i] as string;
    if (a === '--world' || a === '--core') {
      const value = argv[i + 1];
      if (value === undefined || value.startsWith('--')) {
        return { kind: 'error', message: `${a} needs a value` };
      }
      if (a === '--world') world = value;
      else core = value;
      i++;
    } else if (a === '--json') {
      json = true;
    } else if (a.startsWith('--')) {
      return { kind: 'error', message: `unknown option ${a}; see mnemorph --help` };
    } else {
      rest.push(a);
    }
  }
  if (rest.length === 0) {
    if (json) return { kind: 'error', message: '--json goes with run' };
    return { kind: 'chat', world, core };
  }
  if (rest[0] !== 'run') {
    return { kind: 'error', message: `unknown command ${rest[0]}; see mnemorph --help` };
  }
  const words = rest.slice(1);
  // Arguments win: stdin is read only for `run` with no ask, or `run -`.
  if (words.length === 0 || (words.length === 1 && words[0] === '-')) {
    return { kind: 'run', fromStdin: true, world, core, json };
  }
  return { kind: 'run', ask: words.join(' '), fromStdin: false, world, core, json };
}
