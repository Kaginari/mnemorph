/**
 * A live chat in this tab: the SDK's client over the board's WebSocket (`/ws/session/<id>`, the
 * CSRF token offered as a subprotocol entry, held in memory only), the transcript folded from its
 * events, this tab's role, the asks waiting at the gate, and the human's acts — send, queue, steer,
 * interrupt, take over, end.
 *
 * On opening, the tab replays what the chat already said (`catchUp`): live events that overtake
 * the replay wait for it, and the SDK drops any event already seen by its `seq`, so a reconnect
 * after a drop — which replays from the last `seq` — never shows a message twice.
 *
 * The chat answers no ask: approving is the approval card's, a component of its own (#278). An
 * ask stays waiting — shown, never approved here — until it is answered there, Esc denies it with
 * the turn, or its time runs out and the core denies it.
 */
import type { GateRequest, Hello, Role } from '@mnemorph/protocol';
import { type Arrived, Client, ws } from '@mnemorph/sdk/browser';
import { useEffect, useRef, useState } from 'react';
import { Chat, type ChatView } from './chat.ts';
import { csrf } from './session.ts';

export type Link = 'connecting' | 'open' | 'reconnecting' | 'closed';

/** An ask waiting, with when it was seen and how long it had left then. */
export interface Waiting {
  id: string;
  ask: GateRequest;
  /** When this tab saw it (ms). */
  seen: number;
}

export interface LiveState {
  view: ChatView | undefined;
  hello: Hello | undefined;
  role: Role | undefined;
  /** Whether any tab controls the chat. */
  controlled: boolean;
  asks: Waiting[];
  link: Link;
  /** Why the chat closed, once it has. */
  closed?: string | undefined;
  /** What the last act was refused for, if it was. */
  refused?: string | undefined;
  /** The opening replay is done: what follows is live. */
  replayed: boolean;
}

export interface Live extends LiveState {
  /** Send a line: it runs, or waits behind the turn that runs. */
  prompt(text: string): Promise<void>;
  /** Steer the running turn: the body reads it after its next tool step. */
  steer(text: string): Promise<void>;
  /** Esc: stop the turn and deny what waits at the gate. */
  interrupt(): Promise<void>;
  /** Take control from another tab of this browser. */
  takeOver(): Promise<void>;
  /** End the chat: its child leaves. */
  end(): Promise<void>;
}

/** The chat's socket, on this page's own host. */
export function socketUrl(id: string): string {
  const scheme = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${scheme}://${window.location.host}/ws/session/${encodeURIComponent(id)}`;
}

const reason = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** The live chat `id`, for as long as the view shows it. */
export function useLive(id: string): Live {
  const [state, setState] = useState<LiveState>({
    view: undefined,
    hello: undefined,
    role: undefined,
    controlled: false,
    asks: [],
    link: 'connecting',
    replayed: false,
  });
  const client = useRef<Client | undefined>(undefined);

  useEffect(() => {
    const transport = ws({ url: socketUrl(id), csrf });
    const c = new Client(transport);
    client.current = c;
    let gone = false;
    let chat: Chat | undefined;
    const early: Arrived[] = [];
    // What the opening replay brings says nothing of how long anything took.
    let replaying = true;
    let frame: number | undefined;
    const set = (f: (s: LiveState) => LiveState) => {
      if (!gone) setState(f);
    };
    // One redraw per frame, however many events a replay page brings.
    const draw = () => {
      if (frame !== undefined || gone) return;
      frame = window.requestAnimationFrame(() => {
        frame = undefined;
        if (chat) {
          const view = chat.view();
          set((s) => ({ ...s, view }));
        }
      });
    };
    // An ask is shown, never answered by the chat: the approval card (#278) answers it.
    c.onGate(() => new Promise(() => {}));
    c.on((e) => {
      if (!chat) early.push(e);
      else chat.fold(e, replaying ? undefined : Date.now());
      draw();
    });
    c.onRole((role) => set((s) => ({ ...s, role })));
    c.onAsks((asks) =>
      set((s) => {
        const was = new Map(s.asks.map((w) => [w.id, w.seen]));
        return {
          ...s,
          asks: [...asks].map(([aid, ask]) => ({ id: aid, ask, seen: was.get(aid) ?? Date.now() })),
        };
      }),
    );
    c.onNotice((n) => {
      if (n.method === 'board/role') {
        set((s) => ({ ...s, controlled: n.params.controlled === true }));
      } else if (n.method === 'board/refused') {
        const why = typeof n.params.why === 'string' ? n.params.why : 'refused';
        set((s) => ({ ...s, refused: why }));
      } else if (n.method === 'board/ended') {
        const why = typeof n.params.why === 'string' ? n.params.why : 'the chat ended';
        set((s) => ({ ...s, closed: why }));
      }
    });
    c.onClosed((why) => set((s) => ({ ...s, link: 'closed', closed: s.closed ?? why })));
    transport.onDrop?.(() => set((s) => ({ ...s, link: 'reconnecting' })));
    transport.onReopen?.(() => set((s) => ({ ...s, link: 'open' })));

    const hello = c.initialize({ name: 'mnemorph-board' });
    const caught = c.catchUp();
    hello
      .then((h) => {
        chat = new Chat(h.body);
        for (const e of early.splice(0)) chat.fold(e);
        set((s) => ({ ...s, hello: h, link: 'open' }));
        draw();
      })
      .catch((e: unknown) => set((s) => ({ ...s, link: 'closed', closed: s.closed ?? reason(e) })));
    caught
      .catch(() => {})
      .finally(() => {
        replaying = false;
        set((s) => ({ ...s, replayed: true }));
      });
    return () => {
      gone = true;
      if (frame !== undefined) window.cancelAnimationFrame(frame);
      c.close();
      client.current = undefined;
    };
  }, [id]);

  const act = async (f: (c: Client) => Promise<unknown>) => {
    const c = client.current;
    if (!c) return;
    setState((s) => ({ ...s, refused: undefined }));
    try {
      await f(c);
    } catch (e) {
      setState((s) => ({ ...s, refused: reason(e) }));
    }
  };

  return {
    ...state,
    prompt: (text) => act((c) => c.prompt(text)),
    steer: (text) => act((c) => c.send(text)),
    interrupt: () => act((c) => c.interrupt()),
    takeOver: () => act((c) => c.takeOver()),
    end: () => act((c) => c.shutdown()),
  };
}
