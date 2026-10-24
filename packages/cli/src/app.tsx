/** The session in the terminal: Ink renders the state the reducers keep; it decides nothing. */
import type { Client, GateReply, GateRequest, Hello } from '@mnemorph/sdk';
import { dark, races } from '@mnemorph/theme';
import { Box, Static, Text, useApp, useInput, usePaste, useStdout, useWindowSize } from 'ink';
import { useEffect, useReducer, useRef, useState } from 'react';
import * as ed from './editor.js';
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

export const COMMANDS = '/help  the keys and commands · /status  the session · /quit  leave';

const RESULT_LINES = 4;

function ItemView({ item }: { item: Item }) {
  switch (item.kind) {
    case 'user':
      return (
        <Box marginTop={1}>
          <Text color={dark.textMuted}>{'❯ '}</Text>
          <Text bold>{item.text}</Text>
        </Box>
      );
    case 'assistant':
      return (
        <Box marginTop={1}>
          <Text color={races.rimuru}>{'● '}</Text>
          <Text>{item.text}</Text>
        </Box>
      );
    case 'tool': {
      const lines = (item.result?.content ?? '').replace(/\n$/, '').split('\n');
      const shown = lines.slice(0, RESULT_LINES);
      const more = lines.length - shown.length;
      const bad = item.result?.isError;
      return (
        <Box flexDirection="column" marginTop={1}>
          <Text>
            <Text color={bad ? dark.alarm : dark.holo}>{'⏺ '}</Text>
            <Text bold>{item.name}</Text>
            <Text color={dark.textMuted}> {summary(item.name, item.input)}</Text>
          </Text>
          {shown.map((line, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: lines of one finished result never move
            <Text key={i} color={bad ? dark.alarm : dark.textMuted}>
              {i === 0 ? '  ⎿ ' : '    '}
              {line}
            </Text>
          ))}
          {more > 0 || (item.result?.cut ?? 0) > 0 ? (
            <Text color={dark.textMuted}>
              {'    … '}
              {more > 0 ? `${more} more lines` : ''}
              {(item.result?.cut ?? 0) > 0 ? ` (${item.result?.cut} bytes cut)` : ''}
            </Text>
          ) : null}
        </Box>
      );
    }
    case 'note':
      return <Text color={dark.textMuted}>{`  ${item.text}`}</Text>;
    case 'warning':
      return <Text color={dark.gold}>{`  ! ${item.text}`}</Text>;
    case 'verdict':
      return item.passed ? (
        <Text color={races.elf}>{'  ✓ the end gate passed'}</Text>
      ) : (
        <Text color={dark.alarm}>{`  ✗ the end gate: ${item.failures.join('; ')}`}</Text>
      );
    case 'ending':
      return (
        <Box marginTop={1}>
          <Text color={item.ending === 'interrupted' ? dark.textMuted : dark.alarm}>
            {`— ${item.ending}`}
          </Text>
        </Box>
      );
    case 'info':
      return <Text color={dark.textMuted}>{item.text}</Text>;
  }
}

/** The input, its cursor drawn as an inverse cell. */
function Input({ editor, busy }: { editor: ed.Editor; busy: boolean }) {
  const before = editor.text.slice(0, editor.cursor);
  const at = editor.text[editor.cursor];
  const after = editor.text.slice(editor.cursor + 1);
  const cursor = at === undefined || at === '\n' ? ' ' : at;
  return (
    <Box borderStyle="round" borderColor={busy ? dark.line : dark.holo} paddingX={1}>
      <Text>
        <Text color={dark.textMuted}>{'❯ '}</Text>
        {before}
        <Text inverse>{cursor}</Text>
        {at === '\n' ? '\n' : ''}
        {after}
      </Text>
    </Box>
  );
}

function Gate({ request }: { request: GateRequest }) {
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={dark.gold} paddingX={1}>
      <Text color={dark.gold} bold>
        {`Approve this ${request.class} act?`}
      </Text>
      <Text>{request.act}</Text>
      {request.why.map((w) => (
        <Text key={w} color={dark.textMuted}>{`  · ${w}`}</Text>
      ))}
      {request.outside.length > 0 ? (
        <Text color={dark.alarm}>{`  opens outside the world: ${request.outside.join(', ')}`}</Text>
      ) : null}
      <Text>
        <Text bold>y</Text>
        <Text color={dark.textMuted}> run it once · </Text>
        <Text bold>n</Text>
        <Text color={dark.textMuted}> or Esc deny</Text>
      </Text>
    </Box>
  );
}

export interface AppProps {
  client: Client;
  hello: Hello;
}

export function App({ client, hello }: AppProps) {
  const { exit } = useApp();
  const { stdout } = useStdout();
  const { columns } = useWindowSize();
  const [state, dispatch] = useReducer(reduce, undefined, () => {
    let s = initial();
    s = info(s, `mnemorph · ${hello.model} via ${hello.provider} · ${hello.world}`);
    for (const h of hello.holes) s = info(s, `  config hole: ${h}`);
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
  // Every size change reprints the transcript at the new width, from a cleared screen.
  const [epoch, setEpoch] = useState(0);
  const width = useRef(columns);
  useEffect(() => {
    if (columns > 0 && columns !== width.current) {
      width.current = columns;
      stdout.write('\u001b[2J\u001b[3J\u001b[H');
      setEpoch((n) => n + 1);
    }
  }, [columns, stdout]);

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
      dispatch({ kind: 'info', text: `the core closed: ${why}` });
      exit();
    });
    return () => {
      off();
      closed();
    };
  }, [client, exit]);

  const decide = (reply: GateReply) => {
    answer.current?.(reply);
    answer.current = undefined;
    dispatch({ kind: 'gate', gate: undefined });
  };

  const quit = () => {
    void client.shutdown().finally(() => exit());
  };

  const send = (line: string) => {
    if (line.startsWith('/')) {
      const [name] = line.slice(1).split(/\s+/);
      if (name === 'quit' || name === 'exit') return quit();
      if (name === 'help') return dispatch({ kind: 'info', text: COMMANDS });
      if (name === 'status') {
        return dispatch({
          kind: 'info',
          text: `${hello.model} via ${hello.provider} · ${hello.world} · session ${hello.session} · ${status(state.usage).join(' · ')}`,
        });
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
    if (state.gate) {
      if (input === 'y') decide({ allow: true });
      else if (input === 'n' || key.escape) decide({ allow: false, reason: 'the human said no' });
      return;
    }
    if (key.ctrl && input === 'c') {
      if (state.busy) void client.interrupt();
      else quit();
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
  const parts = [hello.model, ...status(state.usage)];
  if (state.queued.length > 0) parts.push(`${state.queued.length} queued`);
  return (
    <Box flexDirection="column">
      <Static key={epoch} items={state.items}>
        {(item) => <ItemView key={item.id} item={item} />}
      </Static>
      {state.streaming ? (
        <Box marginTop={1}>
          <Text color={races.rimuru}>{'● '}</Text>
          <Text>{state.streaming}</Text>
        </Box>
      ) : null}
      {running.map((t) => (
        <Text key={t.call}>
          <Text color={dark.holo}>{'⏺ '}</Text>
          <Text bold>{t.name}</Text>
          <Text color={dark.textMuted}> {summary(t.name, t.input)} …</Text>
        </Text>
      ))}
      {state.queued.map((q, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: the same line may wait twice
        <Text key={i} color={dark.textMuted}>{`  waiting: ${q}`}</Text>
      ))}
      {state.gate ? <Gate request={state.gate.request} /> : null}
      <Box marginTop={1} flexDirection="column">
        <Input editor={editor} busy={state.busy} />
        <Text color={dark.textMuted}>
          {`  ${parts.join(' · ')}`}
          {state.busy ? '  ·  esc to stop' : '  ·  /help'}
        </Text>
      </Box>
    </Box>
  );
}
