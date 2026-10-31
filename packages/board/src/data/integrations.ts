/**
 * Integrations and settings, read-only, from `GET /api/integrations[?world=<id>]` (the board
 * server's `BoardIntegrations`, generated from Rust): providers, models, prices, key sources (never
 * values), holes, MCP servers, the effective configuration with origins. The server redacts; the
 * page checks the shape and shows a hidden value as hidden.
 */
import type {
  BoardIntegrations,
  ConfigView,
  HoleView,
  KeySource,
  McpServerView,
  McpView,
  ModelView,
  PriceView,
  ProviderView,
  Setting,
} from '@mnemorph/protocol';
import { useEffect, useState } from 'react';
import { sourced } from './metrics.ts';
import { establish } from './session.ts';

type Obj = Record<string, unknown>;
const obj = (v: unknown): Obj | undefined =>
  typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Obj) : undefined;
const str = (v: unknown): v is string => typeof v === 'string';
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const bool = (v: unknown): v is boolean => typeof v === 'boolean';
const strOrNull = (v: unknown): v is string | null => v === null || str(v);

function list<T>(v: unknown, check: (v: unknown) => T | undefined): T[] | undefined {
  if (!Array.isArray(v)) return undefined;
  const out: T[] = [];
  for (const x of v) {
    const t = check(x);
    if (t === undefined) return undefined;
    out.push(t);
  }
  return out;
}

const KEY_STATES = ['set', 'unset', 'none', 'hidden'];

const key = (v: unknown): KeySource | undefined => {
  const o = obj(v);
  return o && strOrNull(o.variable) && str(o.state) && KEY_STATES.includes(o.state)
    ? { variable: o.variable, state: o.state }
    : undefined;
};

const price = (v: unknown): PriceView | undefined => {
  const o = obj(v);
  if (!o || !str(o.model)) return undefined;
  const { input, output, cacheRead, cacheWrite } = o;
  return [input, output, cacheRead, cacheWrite].every(num)
    ? {
        model: o.model,
        input: input as number,
        output: output as number,
        cacheRead: cacheRead as number,
        cacheWrite: cacheWrite as number,
      }
    : undefined;
};

const provider = (v: unknown): ProviderView | undefined => {
  const o = obj(v);
  const k = key(o?.key);
  return o && k && str(o.name) && str(o.kind) && strOrNull(o.baseUrl) && str(o.origin)
    ? { name: o.name, kind: o.kind, baseUrl: o.baseUrl, key: k, origin: o.origin }
    : undefined;
};

const model = (v: unknown): ModelView | undefined => {
  const o = obj(v);
  if (!o || !str(o.key) || !strOrNull(o.model) || !strOrNull(o.provider)) return undefined;
  if (!strOrNull(o.problem) || !str(o.origin)) return undefined;
  const p = o.price === null ? null : price(o.price);
  if (p === undefined) return undefined;
  return {
    key: o.key,
    model: o.model,
    provider: o.provider,
    price: p,
    problem: o.problem,
    origin: o.origin,
  };
};

const hole = (v: unknown): HoleView | undefined => {
  const o = obj(v);
  return o && strOrNull(o.key) && str(o.origin) && str(o.why)
    ? { key: o.key, origin: o.origin, why: o.why }
    : undefined;
};

const server = (v: unknown): McpServerView | undefined => {
  const o = obj(v);
  const args = list(o?.args, (a) => (strOrNull(a) ? a : undefined));
  const env = list(o?.env, key);
  const races = list(o?.races, (r) => (str(r) ? r : undefined));
  if (!o || !args || !env || !races || !str(o.name) || !strOrNull(o.command)) return undefined;
  const tools = list(o.tools, (t) => (str(t) ? t : undefined));
  if (!strOrNull(o.class) || !tools || !bool(o.network) || !num(o.timeoutSecs)) return undefined;
  return {
    name: o.name,
    command: o.command,
    args,
    env,
    races,
    class: o.class,
    tools,
    network: o.network,
    timeoutSecs: o.timeoutSecs,
  };
};

const mcp = (v: unknown): McpView | undefined => {
  const o = obj(v);
  const servers = list(o?.servers, server);
  return o && servers && num(o.topK) ? { servers, topK: o.topK } : undefined;
};

const setting = (v: unknown): Setting | undefined => {
  const o = obj(v);
  return o && str(o.key) && strOrNull(o.value) && str(o.layer) && str(o.file) && bool(o.globalOnly)
    ? { key: o.key, value: o.value, layer: o.layer, file: o.file, globalOnly: o.globalOnly }
    : undefined;
};

const config = (v: unknown): ConfigView | undefined => {
  const o = obj(v);
  const providers = list(o?.providers, provider);
  const models = list(o?.models, model);
  const holes = list(o?.holes, hole);
  const effective = list(o?.effective, setting);
  if (!o || !providers || !models || !holes || !effective) return undefined;
  return { providers, models, holes, mcp: sourced(o.mcp, mcp), effective };
};

/** The integrations an answer holds; `undefined` when it is not integrations at all. */
export function parseIntegrations(body: unknown): BoardIntegrations | undefined {
  const o = obj(body);
  const b = obj(o?.board);
  const prices = list(o?.prices, price);
  const hosts = list(b?.hosts, (h) => (str(h) ? h : undefined));
  if (!o || !b || !prices || !hosts || !str(o.global) || !strOrNull(o.world)) return undefined;
  if (!num(b.port)) return undefined;
  return {
    world: o.world,
    global: o.global,
    config: sourced(o.config, config),
    prices,
    board: { port: b.port, hosts },
  };
}

/** A key source in words: `env OPENROUTER_API_KEY: set`. */
export function keyWords(k: KeySource): string {
  if (k.state === 'none') return 'no key';
  if (k.state === 'hidden' || k.variable === null) return 'hidden: not a variable name';
  return `env ${k.variable}: ${k.state}`;
}

export type IntegrationsLoad =
  | { state: 'loading' }
  | { state: 'ready'; data: BoardIntegrations }
  | { state: 'failed'; reason: string };

/** The integrations, of `world`'s files over the global one, or of the global file alone. */
export function useIntegrations(world: string | undefined): IntegrationsLoad {
  const [load, setLoad] = useState<IntegrationsLoad>({ state: 'loading' });
  useEffect(() => {
    const ctl = new AbortController();
    const q = world === undefined ? '' : `?world=${encodeURIComponent(world)}`;
    establish()
      .then(() =>
        fetch(`/api/integrations${q}`, {
          signal: ctl.signal,
          credentials: 'same-origin',
          headers: { accept: 'application/json' },
        }),
      )
      .then(async (res) => {
        if (res.status === 401) throw new Error('this browser holds no board session');
        if (!res.ok) throw new Error(`the board server answered ${res.status}`);
        const i = parseIntegrations(await res.json());
        if (!i) throw new Error('the answer is not integrations');
        setLoad({ state: 'ready', data: i });
      })
      .catch((e: unknown) => {
        if (ctl.signal.aborted) return;
        setLoad({ state: 'failed', reason: e instanceof Error ? e.message : String(e) });
      });
    return () => ctl.abort();
  }, [world]);
  return load;
}
