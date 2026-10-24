/**
 * The client for mnemorphd: one JSON-RPC 2.0 connection, typed requests and events, the human
 * gate answered by a handler, and replay by `seq` so a client that reconnects loses nothing.
 */
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import type {
  Arrived,
  EventsResult,
  GateReply,
  GateRequest,
  Hello,
  PromptResult,
} from '@mnemorph/protocol';
import { eventType, PROTOCOL } from '@mnemorph/protocol';

export type * from '@mnemorph/protocol';

/** A line-oriented connection to the core. */
export interface Transport {
  /** Send one message, a single line of JSON. */
  send(line: string): void;
  /** Called with each line the core sends. */
  onLine(listener: (line: string) => void): void;
  /** Called once when the core's end closes, with why. */
  onClose(listener: (why: string) => void): void;
  /** Close this end. */
  close(): void;
}

/** Start `mnemorphd` and speak to it on its stdin and stdout. */
export function stdio(
  options: {
    command?: string | undefined;
    world?: string | undefined;
    env?: NodeJS.ProcessEnv | undefined;
  } = {},
): Transport {
  const args = options.world ? ['--world', options.world] : [];
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
    },
  };
}

/** A JSON-RPC error the core answered with. */
export class RpcError extends Error {
  constructor(
    readonly code: number,
    message: string,
  ) {
    super(message);
    this.name = 'RpcError';
  }
}

type Pending = { resolve: (v: unknown) => void; reject: (e: Error) => void };

/** Asked when the core needs the human's yes; silence (no handler) denies. */
export type GateHandler = (request: GateRequest) => Promise<GateReply> | GateReply;

/** One connection to the core. */
export class Client {
  private next = 0;
  private readonly pending = new Map<number, Pending>();
  private readonly listeners = new Set<(event: Arrived) => void>();
  private readonly closers = new Set<(why: string) => void>();
  private gate: GateHandler = () => ({ allow: false, reason: 'no one is there to approve it' });
  private closedWhy: string | undefined;
  /** The highest `seq` seen: replay asks from here. */
  lastSeq = 0;
  private session: string | undefined;

  constructor(private readonly transport: Transport) {
    transport.onLine((line) => this.receive(line));
    transport.onClose((why) => {
      this.closedWhy ??= why;
      for (const l of this.closers) l(this.closedWhy);
      for (const p of this.pending.values()) p.reject(new Error(why));
      this.pending.clear();
    });
  }

  /** Open the session; refuses a core that speaks another protocol. */
  async initialize(clientInfo: { name: string; version?: string }): Promise<Hello> {
    const hello = (await this.request('initialize', { clientInfo })) as Hello;
    // A new session numbers its events from 1 again.
    if (hello.session !== this.session) {
      this.session = hello.session;
      this.lastSeq = 0;
    }
    if (hello.protocol !== PROTOCOL) {
      throw new Error(
        `mnemorphd speaks protocol ${hello.protocol}; this client speaks ${PROTOCOL}: update one of them`,
      );
    }
    return hello;
  }

  /** Send a line; it waits its turn when a turn runs. */
  prompt(text: string): Promise<PromptResult> {
    return this.request('session/prompt', { text }) as Promise<PromptResult>;
  }

  /** Esc: stop the turn in flight, and any act waiting at the gate. */
  async interrupt(): Promise<void> {
    await this.request('session/interrupt', {});
  }

  /** Every event after `lastSeq` the core still keeps, delivered to the listeners in order. */
  async resync(): Promise<EventsResult> {
    const result = (await this.request('session/events', { since: this.lastSeq })) as EventsResult;
    for (const raw of result.events) this.receive(JSON.stringify(raw));
    return result;
  }

  /** Close the session; the core exits once its output is written. */
  async shutdown(): Promise<void> {
    await this.request('shutdown', {});
    this.closedWhy ??= 'the session was shut down';
    this.transport.close();
  }

  /** Listen to every event, in order, once each; returns the way to stop. */
  on(listener: (event: Arrived) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Called once when the connection closes, with why; returns the way to stop. */
  onClosed(listener: (why: string) => void): () => void {
    this.closers.add(listener);
    return () => this.closers.delete(listener);
  }

  /** Answer the human gate with `handler`. */
  onGate(handler: GateHandler): void {
    this.gate = handler;
  }

  /** Why the connection closed, once it has. */
  get closed(): string | undefined {
    return this.closedWhy;
  }

  private request(method: string, params: unknown): Promise<unknown> {
    if (this.closedWhy !== undefined) return Promise.reject(new Error(this.closedWhy));
    this.next += 1;
    const id = this.next;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.transport.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }));
    });
  }

  private receive(line: string): void {
    let msg: {
      id?: unknown;
      method?: string;
      params?: Record<string, unknown>;
      result?: unknown;
      error?: { code: number; message: string };
    };
    try {
      msg = JSON.parse(line);
    } catch {
      return;
    }
    if (msg.method === 'gate/ask' && msg.id !== undefined) {
      void this.answerGate(msg.id, msg.params as unknown as GateRequest);
      return;
    }
    const type = msg.method ? eventType(msg.method) : undefined;
    if (type) {
      const event = { ...(msg.params ?? {}), type } as Arrived;
      // Once each, in order: a replayed event already seen is dropped; one with no number is
      // not an event of this protocol.
      if (typeof event.seq !== 'number' || event.seq <= this.lastSeq) return;
      this.lastSeq = event.seq;
      for (const l of this.listeners) l(event);
      return;
    }
    if (typeof msg.id === 'number') {
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      if (msg.error) p.reject(new RpcError(msg.error.code, msg.error.message));
      else p.resolve(msg.result);
    }
  }

  private async answerGate(id: unknown, request: GateRequest): Promise<void> {
    let reply: GateReply;
    try {
      reply = await this.gate(request);
    } catch (e) {
      reply = { allow: false, reason: `the client could not ask: ${(e as Error).message}` };
    }
    this.transport.send(JSON.stringify({ jsonrpc: '2.0', id, result: reply }));
  }
}
