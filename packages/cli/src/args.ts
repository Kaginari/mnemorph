/** The command line, read before anything else: help and the version never look for a world. */

export type Command =
  | { kind: 'help' }
  | { kind: 'version' }
  | {
      kind: 'chat';
      world?: string | undefined;
      core?: string | undefined;
      theme?: string | undefined;
    }
  | {
      kind: 'run';
      ask?: string | undefined;
      fromStdin: boolean;
      world?: string | undefined;
      core?: string | undefined;
      json: boolean;
    }
  | { kind: 'status'; world?: string | undefined; core?: string | undefined }
  | {
      kind: 'onto';
      words: string[];
      world?: string | undefined;
      core?: string | undefined;
      /** The words are a whole core command (`q …`), not `onto …`'s. */
      raw?: boolean | undefined;
    }
  | { kind: 'error'; message: string };

export const HELP = `mnemorph — talk to a Mnemorph body in your terminal

Usage:
  mnemorph [options]                 a session in this world
  mnemorph run "<ask>" [options]     ask once, print the answer, exit
  mnemorph run [-]                   ask once, the ask read from stdin
  mnemorph status [options]          the build, the core, the key's origin, the sandbox
  mnemorph onto owner <path>...      who owns each path, by which rule and claim
  mnemorph q list|show|add|claim|touch|requeue|close …   the shared work queue

Options:
  --world <dir>    the world to work in (default: the current directory)
  --core <path>    the mnemorphd to start (default: MNEMORPH_CORE, then the one built in
                   this checkout, then PATH)
  --theme <name>   dark, light or auto (default: MNEMORPH_THEME, then the terminal's)
  --json           with run: print every event as a JSON line
  -h, --help       this help
  -V, --version    the version

In a session:
  Enter            send; while a turn runs, the line waits its turn
  Alt+Enter        a new line in the message (Shift+Enter where the terminal reports it)
  Esc              stop the turn; at an approval, deny it
  y n, then Enter  at an approval: choose, then confirm
  Ctrl+C           stop the turn; when nothing runs, quit
  ↑ ↓              earlier messages
  /help /status /theme /quit
                   the session's commands
`;

/** Read `argv` (without node and the script). */
export function parse(argv: string[]): Command {
  // Help and the version win wherever they appear: nothing else is read.
  if (argv.some((a) => a === '-h' || a === '--help')) return { kind: 'help' };
  if (argv.some((a) => a === '-V' || a === '--version')) return { kind: 'version' };
  // The queue's words are the core's: everything after `q` goes to it untouched, but `--world`
  // and `--core`, which are this CLI's.
  const at = argv.indexOf('q');
  if (
    at !== -1 &&
    argv.slice(0, at).every((a, i, all) => a.startsWith('--') || all[i - 1]?.startsWith('--'))
  ) {
    const words: string[] = [];
    let qWorld: string | undefined;
    let qCore: string | undefined;
    const all = [...argv.slice(0, at), ...argv.slice(at + 1)];
    for (let i = 0; i < all.length; i++) {
      const a = all[i] as string;
      if (a === '--world' || a === '--core') {
        const value = all[i + 1];
        if (value === undefined) return { kind: 'error', message: `${a} needs a value` };
        if (a === '--world') qWorld = value;
        else qCore = value;
        i++;
      } else {
        words.push(a);
      }
    }
    return { kind: 'onto', words: ['q', ...words], world: qWorld, core: qCore, raw: true };
  }
  let world: string | undefined;
  let core: string | undefined;
  let json = false;
  let theme: string | undefined;
  const rest: string[] = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i] as string;
    if (a === '--world' || a === '--core' || a === '--theme') {
      const value = argv[i + 1];
      if (value === undefined || value.startsWith('--')) {
        return { kind: 'error', message: `${a} needs a value` };
      }
      if (a === '--world') world = value;
      else if (a === '--core') core = value;
      else theme = value;
      i++;
    } else if (a === '--json') {
      json = true;
    } else if (a.startsWith('--')) {
      return { kind: 'error', message: `unknown option ${a}; see mnemorph --help` };
    } else {
      rest.push(a);
    }
  }
  if (theme !== undefined && !['dark', 'light', 'auto'].includes(theme)) {
    return { kind: 'error', message: `--theme is dark, light or auto, not ${theme}` };
  }
  if (rest.length === 0) {
    if (json) return { kind: 'error', message: '--json goes with run' };
    return { kind: 'chat', world, core, theme };
  }
  if (rest[0] === 'status') {
    if (json) return { kind: 'error', message: '--json goes with run' };
    if (rest.length > 1)
      return { kind: 'error', message: 'status takes no words; see mnemorph --help' };
    return { kind: 'status', world, core };
  }
  if (rest[0] === 'onto') {
    if (json) return { kind: 'error', message: '--json goes with run' };
    // The core reads the world and answers; the words go to it as they are.
    return { kind: 'onto', words: rest.slice(1), world, core };
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
