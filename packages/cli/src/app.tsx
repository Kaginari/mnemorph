/** The session in the terminal: Ink renders the state the reducers keep; it decides nothing. */
import type { Client, GateReply, GateRequest, Hello } from '@mnemorph/sdk';
import { races } from '@mnemorph/theme';
import { Box, Static, Text, useApp, useInput, usePaste, useStdout, useWindowSize } from 'ink';
import type { ReactNode } from 'react';
import { useEffect, useReducer, useRef, useState } from 'react';
import * as ed from './editor.js';
import { palette, ThemeContext, type ThemeName, usePalette } from './theme.js';
import { fold, type Item, info, initial, type State, status, summary } from './transcript.js';

type Action =
  | { kind: 'event'; event: Parameters<typeof fold>[1] }
  | { kind: 'info'; text: string }
  | { kind: 'gate'; gate: State['gate'] }
  | { kind: 'local-busy' };

function reduce(s: State, a: Action): State {
  switch (a.kind) {
    case 'event':
      return fold(s, a.event);
    case 'info':
      return info(s, a.text);
    case 'gate':
      return { ...s, gate: a.gate };
    case 'local-busy':
      return { ...s, busy: true };
  }
}

export const COMMANDS =
  '/send <text>  steer the running turn · /status · /theme dark|light · /quit';

const RESULT_LINES = 4;

/** How long quitting waits for the core to say goodbye before it goes anyway. */
export const QUIT_WAIT_MS = 1000;

/** A prefix in its own column and text that wraps beside it, never under it. */
function Row({ prefix, color, children }: { prefix: string; color: string; children: ReactNode }) {
  return (
    <Box flexDirection="row">
      <Box width={prefix.length} flexShrink={0}>
        <Text color={color}>{prefix}</Text>
      </Box>
      <Box flexGrow={1} flexShrink={1}>
        {children}
      </Box>
    </Box>
  );
}

export function ItemView({ item }: { item: Item }) {
  const p = usePalette();
  switch (item.kind) {
    case 'user':
      return (
        <Box marginTop={1}>
          <Row prefix="❯ " color={p.textMuted}>
            <Text bold>{item.text}</Text>
          </Row>
        </Box>
      );
    case 'assistant':
      return (
        <Box marginTop={item.cont ? 0 : 1}>
          <Row prefix={item.cont ? '  ' : '● '} color={races.rimuru}>
            <Text>{item.text}</Text>
          </Row>
        </Box>
      );
    case 'tool': {
      const lines = (item.result?.content ?? '').replace(/\n$/, '').split('\n');
      const shown = lines.slice(0, RESULT_LINES);
      const more = lines.length - shown.length;
      const bad = item.result?.isError;
      const cut = item.result?.cut ?? 0;
      return (
        <Box flexDirection="column" marginTop={1}>
          <Row prefix="⏺ " color={bad ? p.alarm : p.holo}>
            <Text>
              <Text bold>{item.name}</Text>
              <Text color={p.textMuted}> {summary(item.name, item.input)}</Text>
            </Text>
          </Row>
          <Row prefix="  ⎿ " color={p.textMuted}>
            <Box flexDirection="column">
              {shown.map((line, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: lines of one finished result never move
                <Text key={i} color={bad ? p.alarm : p.textMuted}>
                  {line}
                </Text>
              ))}
              {more > 0 || cut > 0 ? (
                <Text color={p.textMuted}>
                  {'… '}
                  {more > 0 ? `${more} more lines` : ''}
                  {cut > 0 ? ` (${cut} bytes cut)` : ''}
                </Text>
              ) : null}
            </Box>
          </Row>
        </Box>
      );
    }
    case 'note':
      return (
        <Row prefix="  " color={p.textMuted}>
          <Text color={p.textMuted}>{item.text}</Text>
        </Row>
      );
    case 'warning':
      return (
        <Row prefix="  ! " color={p.gold}>
          <Text color={p.gold}>{item.text}</Text>
        </Row>
      );
    case 'verdict':
      return item.passed ? (
        <Text color={races.elf}>{'  ✓ the end gate passed'}</Text>
      ) : (
        <Row prefix="  ✗ " color={p.alarm}>
          <Text color={p.alarm}>{`the end gate: ${item.failures.join('; ')}`}</Text>
        </Row>
      );
    case 'ending':
      return (
        <Box marginTop={1}>
          <Text color={item.ending === 'interrupted' ? p.textMuted : p.alarm}>
            {`— ${item.ending}`}
          </Text>
        </Box>
      );
    case 'info':
      return <Text color={p.textMuted}>{item.text}</Text>;
  }
}

/** The input, its cursor drawn as an inverse cell over one whole character. */
function Input({ editor, busy }: { editor: ed.Editor; busy: boolean }) {
  const p = usePalette();
  const end = ed.after(editor.text, editor.cursor);
  const before = editor.text.slice(0, editor.cursor);
  const at = editor.text.slice(editor.cursor, end);
  const after = editor.text.slice(end);
  const cursor = at === '' || at === '\n' ? ' ' : at;
  return (
    <Box borderStyle="round" borderColor={busy ? p.line : p.holo} paddingX={1}>
      <Row prefix="❯ " color={p.textMuted}>
        <Text>
          {before}
          <Text inverse>{cursor}</Text>
          {at === '\n' ? '\n' : ''}
          {after}
        </Text>
      </Row>
    </Box>
  );
}

function Gate({ request, choice }: { request: GateRequest; choice: 'y' | 'n' | undefined }) {
  const p = usePalette();
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={p.gold} paddingX={1}>
      <Text color={p.gold} bold>
        {`Approve this ${request.class} act?`}
      </Text>
      <Text>{request.act}</Text>
      {request.why.map((w) => (
        <Text key={w} color={p.textMuted}>{`  · ${w}`}</Text>
      ))}
      {request.outside.length > 0 ? (
        <Text color={p.alarm}>{`  opens outside the world: ${request.outside.join(', ')}`}</Text>
      ) : null}
      <Text>
        <Text bold inverse={choice === 'y'}>
          {' y '}
        </Text>
        <Text color={p.textMuted}> run it once </Text>
        <Text bold inverse={choice === 'n'}>
          {' n '}
        </Text>
        <Text color={p.textMuted}>
          {choice
            ? ' deny · Enter confirms · Esc denies'
            : ' deny · choose, then Enter · Esc denies'}
        </Text>
      </Text>
    </Box>
  );
}

/** The last `rows` lines of `text`, so the live region never outgrows the terminal. */
export function tail(text: string, rows: number): string {
  const lines = text.split('\n');
  return lines.length <= rows ? text : lines.slice(-rows).join('\n');
}

export interface AppProps {
  client: Client;
  hello: Hello;
  /** Told why the core closed, when it closed by itself. */
  onCoreClosed?: (why: string) => void;
  /** The palette to start with. */
  theme?: ThemeName;
}

export function App({ client, hello, onCoreClosed, theme: startTheme = 'dark' }: AppProps) {
  const [theme, setTheme] = useState<ThemeName>(startTheme);
  const p = palette(theme);
  const { exit } = useApp();
  const { write } = useStdout();
  const { columns, rows } = useWindowSize();
  const [state, dispatch] = useReducer(reduce, undefined, () => {
    let s = initial();
    s = info(s, `mnemorph · ${hello.model} via ${hello.provider} · ${hello.world}`);
    for (const h of hello.holes) s = info(s, `  config hole: ${h}`);
    if (hello.budget && hello.budget.leftWt === 0) {
      s = info(s, `  the ${hello.budget.cap} cap is spent: a turn stops at once until it resets`);
    }
    return s;
  });
  // The editor lives in a ref every key updates at once, so a burst of keys — a fast typist, a
  // terminal over SSH — is taken strictly in order, never against a stale render.
  const editorRef = useRef(ed.empty());
  const [, redraw] = useReducer((n: number) => n + 1, 0);
  const setEditor = (f: (e: ed.Editor) => ed.Editor) => {
    editorRef.current = f(editorRef.current);
    redraw();
  };
  const editor = editorRef.current;
  const answer = useRef<((r: GateReply) => void) | undefined>(undefined);
  const quitting = useRef(false);
  // An approval is chosen, then confirmed: a key typed as the box appears approves nothing.
  const [choice, setChoice] = useState<'y' | 'n' | undefined>(undefined);

  // Every width change reprints the transcript at the new width. Only the screen is cleared —
  // through Ink's own writer, so nothing races its frames — and the scrollback above is kept.
  const [epoch, setEpoch] = useState(0);
  const width = useRef(columns);
  useEffect(() => {
    if (columns > 0 && columns !== width.current) {
      width.current = columns;
      // After Ink has taken the new width in, so the reprint is laid out at it.
      const later = setTimeout(() => {
        write('\u001b[2J\u001b[H');
        setEpoch((n) => n + 1);
      }, 50);
      return () => clearTimeout(later);
    }
  }, [columns, write]);

  useEffect(() => {
    const off = client.on((event) => dispatch({ kind: 'event', event }));
    client.onGate(
      (request) =>
        new Promise<GateReply>((resolve) => {
          answer.current = resolve;
          dispatch({ kind: 'gate', gate: { id: Date.now(), request } });
        }),
    );
    const closed = client.onClosed((why) => {
      if (!quitting.current) onCoreClosed?.(why);
      exit();
    });
    return () => {
      off();
      closed();
    };
  }, [client, exit, onCoreClosed]);

  // An approval the turn left behind is denied, never left to answer a dead question.
  useEffect(() => {
    if (!state.gate && answer.current) {
      answer.current({ allow: false, reason: 'the turn ended before the human answered' });
      answer.current = undefined;
    }
  }, [state.gate]);

  const decide = (reply: GateReply) => {
    setChoice(undefined);
    answer.current?.(reply);
    answer.current = undefined;
    dispatch({ kind: 'gate', gate: undefined });
  };

  /** Leave: ask the core to stop, and go after a second whether it answered or not. A second
   * quit while waiting goes at once. */
  const quit = () => {
    if (quitting.current) return exit();
    quitting.current = true;
    const wait = new Promise((r) => setTimeout(r, QUIT_WAIT_MS));
    void Promise.race([client.shutdown().catch(() => {}), wait]).finally(() => {
      // Answered or not, the core is let go: one that never said goodbye is stopped.
      client.close();
      exit();
    });
  };

  const send = (line: string) => {
    if (line.startsWith('/')) {
      const [name] = line.slice(1).split(/\s+/);
      if (name === 'quit' || name === 'exit') return quit();
      if (name === 'help') return dispatch({ kind: 'info', text: COMMANDS });
      if (name === 'status') {
        return dispatch({
          kind: 'info',
          text: [
            `${hello.model} via ${hello.provider} · ${hello.world} · session ${hello.session}`,
            `core ${hello.version} · key ${hello.key ?? 'none'} · sandbox ${hello.sandbox}`,
            status(state.usage, hello.budget).join(' · '),
          ]
            .filter(Boolean)
            .join('\n'),
        });
      }
      const rest = line.slice(1 + (name?.length ?? 0)).trim();
      if (name === 'send') {
        if (!rest) return dispatch({ kind: 'info', text: '/send <text>: steer the running turn' });
        void client
          .send(rest)
          .then(() => dispatch({ kind: 'info', text: `  sent: ${rest}` }))
          .catch((e: Error) => dispatch({ kind: 'info', text: e.message }));
        return;
      }
      if (name === 'theme') {
        const [, word] = line.slice(1).split(/\s+/);
        if (word !== 'dark' && word !== 'light') {
          return dispatch({ kind: 'info', text: `/theme dark or /theme light (now ${theme})` });
        }
        setTheme(word);
        // The transcript is reprinted in the new colours.
        write('\u001b[2J\u001b[H');
        setEpoch((n) => n + 1);
        return;
      }
      return dispatch({ kind: 'info', text: `no command /${name}; ${COMMANDS}` });
    }
    dispatch({ kind: 'local-busy' });
    void client.prompt(line).catch((e: Error) => dispatch({ kind: 'info', text: e.message }));
  };

  usePaste((text) => setEditor((e) => ed.insert(e, text)), { isActive: !state.gate });

  const enter = () => {
    const [line, next] = ed.submit(editorRef.current);
    editorRef.current = next;
    redraw();
    if (line) send(line);
  };

  useInput((input, key) => {
    if (key.ctrl && input === 'c') {
      // Ctrl+C always does something: it denies an open approval and stops the turn, or quits.
      if (state.gate) decide({ allow: false, reason: 'the human stopped the turn' });
      if (state.busy || state.gate) void client.interrupt();
      else quit();
      return;
    }
    if (state.gate) {
      if (input === 'y' || input === 'n') setChoice(input);
      else if (key.escape || (key.return && choice === 'n')) {
        decide({ allow: false, reason: 'the human said no' });
      } else if (key.return && choice === 'y') decide({ allow: true });
      return;
    }
    if (key.ctrl && input === 'd') {
      if (!editorRef.current.text) quit();
      return;
    }
    if (key.escape) {
      if (state.busy) void client.interrupt();
      return;
    }
    if (key.return) {
      if (key.meta || key.shift) return setEditor((e) => ed.insert(e, '\n'));
      return enter();
    }
    if (key.backspace) return setEditor(ed.backspace);
    if (key.delete) return setEditor(ed.del);
    if (key.leftArrow) return setEditor(ed.left);
    if (key.rightArrow) return setEditor(ed.right);
    if (key.upArrow) return setEditor(ed.up);
    if (key.downArrow) return setEditor(ed.down);
    if (key.home || (key.ctrl && input === 'a')) return setEditor(ed.home);
    if (key.end || (key.ctrl && input === 'e')) return setEditor(ed.end);
    if (key.ctrl && input === 'u') return setEditor(ed.killBack);
    if (key.ctrl && input === 'w') return setEditor(ed.killWord);
    if (key.ctrl || key.meta || key.tab) return;
    // Keys that arrive together come as one input: a carriage return among them is Enter.
    const parts = input.split('\r');
    parts.forEach((part, i) => {
      if (part) setEditor((e) => ed.insert(e, part));
      if (i < parts.length - 1) enter();
    });
  });

  const running = [...state.running.values()];
  const parts = [hello.model, ...status(state.usage, hello.budget)];
  if (state.queued.length > 0) parts.push(`${state.queued.length} queued`);
  // What streams gets the rows the input, the status line and the calls leave, and no more.
  const room = Math.max(3, rows - 6 - running.length - state.queued.length - (state.gate ? 6 : 0));
  // The last column stays free: a line that fills it would wrap its trailing space onto a row
  // of its own.
  return (
    <ThemeContext.Provider value={p}>
      <Box flexDirection="column" width={Math.max(10, columns - 1)}>
        <Static key={epoch} items={state.items}>
          {(item) => (
            // Ink lays the scrollback out at full width: the last column is kept free here too.
            <Box key={item.id} width={Math.max(10, columns - 1)}>
              <ItemView item={item} />
            </Box>
          )}
        </Static>
        {state.streaming.trim() ? (
          <Box marginTop={state.flushed ? 0 : 1}>
            <Row prefix={state.flushed ? '  ' : '● '} color={races.rimuru}>
              <Text>{tail(state.streaming, room)}</Text>
            </Row>
          </Box>
        ) : null}
        {running.map((t) => (
          <Row key={t.call} prefix="⏺ " color={p.holo}>
            <Text>
              <Text bold>{t.name}</Text>
              <Text color={p.textMuted}> {summary(t.name, t.input)} …</Text>
            </Text>
          </Row>
        ))}
        {state.queued.map((q, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: the same line may wait twice
          <Text key={i} color={p.textMuted}>{`  waiting: ${q}`}</Text>
        ))}
        {state.gate ? <Gate request={state.gate.request} choice={choice} /> : null}
        <Box marginTop={1} flexDirection="column">
          <Input editor={editor} busy={state.busy || state.gate !== undefined} />
          <Text color={p.textMuted}>
            {`  ${parts.join(' · ')}`}
            {state.busy ? '  ·  esc to stop' : '  ·  /help'}
          </Text>
        </Box>
      </Box>
    </ThemeContext.Provider>
  );
}
