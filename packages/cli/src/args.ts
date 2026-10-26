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
  mnemorph onto settle <fact-id>     settle a contested fact in the ontology
  mnemorph q list|show|add|claim|touch|requeue|close …   the shared work queue
  mnemorph housekeep [--heal]        what is broken in the roster; --heal fixes what is mechanical
  mnemorph relief <name> [--apply]   split a strained creature's ground in two young, or retire it
  mnemorph memory status [--json]    note events that do not resolve, retired notes, recall loops
  mnemorph memory wake <note>        restore a note the gate retired

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
  // The queue's words are the core's. `q` is a command only as the first word that is not a flag
  // or a flag's value; what comes after it goes to the core untouched, and what comes before it is
  // this CLI's (`--world`, `--core`; `--json` rides along to the core).
  const valued = new Set(['--world', '--core', '--theme']);
  let first = -1;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i] as string;
    if (valued.has(a)) i++;
    else if (!a.startsWith('--')) {
      first = i;
      break;
    }
  }
  if (first !== -1 && ['q', 'housekeep', 'relief', 'memory'].includes(argv[first] as string)) {
    let qWorld: string | undefined;
    let qCore: string | undefined;
    const before: string[] = [];
    for (let i = 0; i < first; i++) {
      const a = argv[i] as string;
      if (valued.has(a)) {
        const value = argv[i + 1] as string;
        if (a === '--world') qWorld = value;
        else if (a === '--core') qCore = value;
        i++;
      } else if (a === '--json') before.push(a);
      else return { kind: 'error', message: `${a} does not go with q` };
    }
    return {
      kind: 'onto',
      words: [argv[first] as string, ...argv.slice(first + 1), ...before],
      world: qWorld,
      core: qCore,
      raw: true,
    };
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
