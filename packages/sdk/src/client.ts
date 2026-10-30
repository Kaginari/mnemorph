/**
 * The client: one JSON-RPC 2.0 connection to the core, typed requests and events, the human
 * gate answered by a handler, and replay by `seq` so a client that reconnects loses nothing.
 * It imports nothing from Node: the board runs it in the browser.
 */
import type {
  Arrived,
  EventsResult,
  GateReply,
  GateRequest,
  Hello,
  PromptResult,
  Role,
} from '@mnemorph/protocol';
import { eventType, PROTOCOL } from '@mnemorph/protocol';

/** A line-oriented connection to the core. */
export interface Transport {
  /** Send one message, a single line of JSON. */
  send(line: string): void;
  /** Called with each line the core sends. */
  onLine(listener: (line: string) => void): void;
  /** Called once when the connection ends for good, with why. */
  onClose(listener: (why: string) => void): void;
  /** Close this end. */
  close(): void;
  /** A reconnecting transport: called when the connection dropped and will be tried again. */
  onDrop?(listener: (why: string) => void): void;
  /** A reconnecting transport: called when a dropped connection is open again. */
  onReopen?(listener: () => void): void;
  /** A reconnecting transport: whether a message sent now would go out. */
  ready?(): boolean;
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

/** A message the board's bridge adds beside the protocol: `board/<name>` and its parameters. */
export type Notice = { method: string; params: Record<string, unknown> };

type Message = {
  id?: unknown;
  method?: string;
  params?: Record<string, unknown>;
  result?: unknown;
  error?: { code: number; message: string };
};

/** One connection to the core. */
export class Client {
  private next = 0;
  private readonly pending = new Map<number, Pending>();
  private readonly listeners = new Set<(event: Arrived) => void>();
  private readonly closers = new Set<(why: string) => void>();
  private readonly roleListeners = new Set<(role: Role) => void>();
  private readonly noticeListeners = new Set<(notice: Notice) => void>();
  private readonly askListeners = new Set<(asks: ReadonlyMap<string, GateRequest>) => void>();
  private gate: GateHandler = () => ({ allow: false, reason: 'no one is there to approve it' });
  private closedWhy: string | undefined;
  /** The highest `seq` seen: replay asks from here. */
  lastSeq = 0;
  private session: string | undefined;
  /** This tab's role on a board chat; `undefined` on a connection with no roles (stdio). */
  role: Role | undefined;
  /** The asks waiting for the human, by id, as last sent. */
  readonly asks = new Map<string, GateRequest>();
  /** The asks being answered now: an ask sent again meanwhile is not asked twice. */
  private readonly answering = new Set<string>();
  /** While a replay runs, live events wait here, so every event arrives once and in order. */
  private held: Message[] | undefined;

  constructor(private readonly transport: Transport) {
    transport.onLine((line) => this.receive(line));
    transport.onClose((why) => {
      this.closedWhy ??= why;
      for (const l of this.closers) l(this.closedWhy);
      this.fail(why);
    });
    transport.onDrop?.((why) => this.fail(why));
    transport.onReopen?.(() => void this.reopened());
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

  /** Steer the running turn: the body reads `text` after its next tool step. Refused when
   * nothing runs — then it is a message. */
  async send(text: string): Promise<void> {
    await this.request('session/send', { text });
  }

  /** Esc: stop the turn in flight, and any act waiting at the gate. */
  async interrupt(): Promise<void> {
    await this.request('session/interrupt', {});
  }

  /** On a board chat: take control from another tab of this browser. */
  async takeOver(): Promise<void> {
    await this.request('board/takeOver', {});
  }

  /** Every event after `lastSeq` the core still keeps, delivered to the listeners in order —
   * page after page while the answer says `more`. */
  async resync(): Promise<EventsResult> {
    for (;;) {
      const result = (await this.request('session/events', {
        since: this.lastSeq,
      })) as EventsResult;
      for (const raw of result.events) this.deliver(raw as Message);
      if (!result.more || result.events.length === 0) return result;
    }
  }

  /** Close the session; the core exits once its output is written. */
  async shutdown(): Promise<void> {
    await this.request('shutdown', {});
    this.closedWhy ??= 'the session was shut down';
    this.transport.close();
  }

  /** Close this end without asking: the core's input ends, and a core that does not leave is
   * stopped. */
  close(): void {
    this.closedWhy ??= 'the client closed the connection';
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

  /** Called with this tab's role whenever the board says it; returns the way to stop. */
  onRole(listener: (role: Role) => void): () => void {
    this.roleListeners.add(listener);
    return () => this.roleListeners.delete(listener);
  }

  /** Called with the asks waiting whenever they change — an observer sees them too; returns the
   * way to stop. */
  onAsks(listener: (asks: ReadonlyMap<string, GateRequest>) => void): () => void {
    this.askListeners.add(listener);
    return () => this.askListeners.delete(listener);
  }

  /** Called with each other message of the board's bridge (`board/refused`, `board/ended`,
   * `board/oversize`); returns the way to stop. */
  onNotice(listener: (notice: Notice) => void): () => void {
    this.noticeListeners.add(listener);
    return () => this.noticeListeners.delete(listener);
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
    if (this.transport.ready?.() === false) {
      return Promise.reject(new Error('not connected: reconnecting'));
    }
    this.next += 1;
    const id = this.next;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.transport.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }));
    });
  }

  /** What waits on an answer fails: the connection that would carry it is gone. */
  private fail(why: string): void {
    for (const p of this.pending.values()) p.reject(new Error(why));
    this.pending.clear();
  }

  /** Catch up: every event after `lastSeq` the core still keeps, then the live ones that came
   * meanwhile — each once, in order. A tab opening on a chat already running calls it at once,
   * beside `initialize`: live events wait from the call on, so none overtakes the replay. If the
   * replay fails, they keep waiting, for the next catch-up. */
  async catchUp(): Promise<EventsResult> {
    this.held ??= [];
    const result = await this.resync();
    const held = this.held ?? [];
    this.held = undefined;
    for (const m of held) this.deliver(m);
    return result;
  }

  /** The connection is back: control taken again if this tab had it, then the events missed,
   * then the live ones that came meanwhile. */
  private async reopened(): Promise<void> {
    this.held ??= [];
    try {
      if (this.role === 'controller') await this.takeOver().catch(() => {});
      await this.catchUp();
    } catch {
      // Dropped again: the next reopening replays from the same place.
    }
  }

  private receive(line: string): void {
    let msg: Message;
    try {
      msg = JSON.parse(line);
    } catch {
      return;
    }
    if (msg === null || typeof msg !== 'object') return;
    if (msg.method === 'gate/ask' && typeof msg.id === 'string') {
      this.asked(msg.id, msg.params as unknown as GateRequest);
      return;
    }
    if (msg.method?.startsWith('board/')) {
      this.notice(msg.method, msg.params ?? {});
      return;
    }
    if (msg.method && eventType(msg.method)) {
      if (this.held) this.held.push(msg);
      else this.deliver(msg);
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

  /** An event, once each, in order: one already seen is dropped; one with no number is not an
   * event of this protocol. */
  private deliver(msg: Message): void {
    const type = msg.method ? eventType(msg.method) : undefined;
    if (!type) return;
    const event = { ...(msg.params ?? {}), type } as Arrived;
    if (typeof event.seq !== 'number' || event.seq <= this.lastSeq) return;
    this.lastSeq = event.seq;
    for (const l of this.listeners) l(event);
  }

  private notice(method: string, params: Record<string, unknown>): void {
    if (method === 'board/role') {
      this.role = params.role as Role;
      for (const l of this.roleListeners) l(this.role);
    } else if (method === 'board/askGone') {
      if (this.asks.delete(String(params.id))) this.asksChanged();
    } else if (method === 'board/ended') {
      this.asks.clear();
      this.asksChanged();
    }
    for (const l of this.noticeListeners) l({ method, params });
  }

  private asksChanged(): void {
    for (const l of this.askListeners) l(this.asks);
  }

  private asked(id: string, request: GateRequest): void {
    this.asks.set(id, request);
    this.asksChanged();
    // An observer sees the ask and answers nothing: the board would refuse it.
    if (this.role === 'observer' || this.answering.has(id)) return;
    void this.answerGate(id, request);
  }

  private async answerGate(id: string, request: GateRequest): Promise<void> {
    this.answering.add(id);
    let reply: GateReply;
    try {
      reply = await this.gate(request);
    } catch (e) {
      reply = { allow: false, reason: `the client could not ask: ${(e as Error).message}` };
    }
    this.answering.delete(id);
    this.asks.delete(id);
    this.asksChanged();
    this.transport.send(JSON.stringify({ jsonrpc: '2.0', id, result: reply }));
  }
}
