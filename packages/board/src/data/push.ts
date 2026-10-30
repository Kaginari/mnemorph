/**
 * The board's push channel, `WS /ws/board`: the server tells `world/changed {id}` when a world's
 * files moved and `worlds/changed` when the list did; the page refetches what it shows. One
 * socket per page, opened with the subprotocol `mnemorph.board.v1` and the CSRF entry, and opened
 * again after a drop — each reopening tells every listener that everything may have moved, since
 * what was told while it was down is lost.
 */
import { useEffect, useState } from 'react';
import { csrf } from './session.ts';

export const SUBPROTOCOL = 'mnemorph.board.v1';

export type Change = { kind: 'world'; id: string } | { kind: 'worlds' };

/** Whether pushes arrive: `open`, being opened, or down (the page then reads once per visit). */
export type PushState = 'connecting' | 'open' | 'down';

/** The change a message names, or nothing for anything else. */
export function parseChange(text: unknown): Change | undefined {
  if (typeof text !== 'string') return undefined;
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch {
    return undefined;
  }
  if (typeof v !== 'object' || v === null) return undefined;
  const { jsonrpc, method, params } = v as Record<string, unknown>;
  if (jsonrpc !== '2.0') return undefined;
  if (method === 'worlds/changed') return { kind: 'worlds' };
  if (method !== 'world/changed' || typeof params !== 'object' || params === null) {
    return undefined;
  }
  const id = (params as { id?: unknown }).id;
  return typeof id === 'string' && id !== '' ? { kind: 'world', id } : undefined;
}

type Make = (url: string, protocols: string[]) => WebSocket;

const listeners = new Set<(c: Change) => void>();
const watchers = new Set<(s: PushState) => void>();
let state: PushState = 'connecting';
let started = false;
let make: Make = (url, protocols) => new WebSocket(url, protocols);
let backoff = 1000;

function set(s: PushState) {
  state = s;
  for (const w of watchers) w(s);
}

function tell(c: Change) {
  for (const l of listeners) l(c);
}

function open() {
  set('connecting');
  csrf()
    .then((token) => {
      const url = `ws://${window.location.host}/ws/board`;
      const ws = make(url, [SUBPROTOCOL, `mnemorph.csrf.${token}`]);
      ws.addEventListener('open', () => {
        // A reopening: what was told while the socket was down is lost.
        const again = backoff > 1000;
        backoff = 1000;
        set('open');
        if (again) tell({ kind: 'worlds' });
      });
      ws.addEventListener('message', (e) => {
        const c = parseChange(e.data);
        if (c) tell(c);
      });
      ws.addEventListener('close', () => retry());
    })
    .catch(() => retry());
}

function retry() {
  set('down');
  const wait = backoff;
  backoff = Math.min(backoff * 2, 30_000);
  window.setTimeout(open, wait);
}

/** Open the socket once per page; `with` replaces the WebSocket constructor (tests). */
export function start(withMake?: Make) {
  if (withMake) make = withMake;
  if (started) return;
  started = true;
  open();
}

/** Follow the push channel: `on` hears each change; the channel's state is returned. */
export function usePush(on?: (c: Change) => void): PushState {
  const [s, setS] = useState(state);
  useEffect(() => {
    start();
    watchers.add(setS);
    setS(state);
    return () => {
      watchers.delete(setS);
    };
  }, []);
  useEffect(() => {
    if (!on) return;
    listeners.add(on);
    return () => {
      listeners.delete(on);
    };
  }, [on]);
  return s;
}
