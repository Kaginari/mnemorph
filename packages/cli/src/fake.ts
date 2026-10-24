/** A core in memory, for the tests: it records what the client sends and says what it is told. */
import type { Transport } from '@mnemorph/sdk';

export class FakeCore implements Transport {
  sent: Record<string, unknown>[] = [];
  private line: (l: string) => void = () => {};
  private closer: (why: string) => void = () => {};
  /** Answers given to requests by method, as they come. */
  answers: Record<string, unknown> = {
    initialize: {
      protocol: 1,
      world: '/w',
      model: 'm',
      provider: 'p',
      session: 's',
      run: 'r',
      version: '0.0.0 (core)',
      holes: [],
      budget: null,
      key: null,
      sandbox: 'none — the guard helper is missing (/x)',
    },
    'session/prompt': { queued: false },
    'session/interrupt': {},
    shutdown: {},
  };
  send(line: string) {
    const msg = JSON.parse(line) as { id?: unknown; method?: string };
    this.sent.push(msg);
    if (msg.method && msg.id !== undefined && msg.method in this.answers) {
      const result = this.answers[msg.method];
      queueMicrotask(() =>
        this.say(
          result instanceof Error
            ? { jsonrpc: '2.0', id: msg.id, error: { code: -32001, message: result.message } }
            : { jsonrpc: '2.0', id: msg.id, result },
        ),
      );
    }
  }
  onLine(listener: (line: string) => void) {
    this.line = listener;
  }
  onClose(listener: (why: string) => void) {
    this.closer = listener;
  }
  close() {}
  say(msg: unknown) {
    this.line(JSON.stringify(msg));
  }
  private seq = 0;
  /** Send an event, numbered. */
  event(type: string, fields: Record<string, unknown> = {}) {
    this.seq += 1;
    this.say({
      jsonrpc: '2.0',
      method: `event/${type}`,
      params: { session: 's', run: 'r', body: 'rimuru', seq: this.seq, ...fields },
    });
  }
  end(why: string) {
    this.closer(why);
  }
  methods(): string[] {
    return this.sent.map((m) => (m.method as string) ?? 'response');
  }
}
