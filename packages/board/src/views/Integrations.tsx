/**
 * Integrations and settings, read-only: the providers and where each key is read from (never the
 * key), the models each role rides and their prices, the configuration's holes, the MCP servers
 * the global file declares, the effective configuration with each key's origin, and the board's
 * port and theme. Nothing here edits: each section carries a "how to add" snippet for the global
 * file, to copy.
 */
import './metrics.css';
import type { BoardIntegrations, ConfigView } from '@mnemorph/protocol';
import { useState } from 'react';
import { keyWords, useIntegrations } from '../data/integrations.ts';
import { Safe } from '../safe/Safe.tsx';
import type { Choice } from '../theme.ts';
import { Empty } from '../ui/Empty.tsx';
import { Mark } from '../ui/Mark.tsx';
import { Unknown } from './facts.tsx';

/** A snippet for the global file, with a button that copies it. */
function HowTo({ title, text }: { title: string; text: string }) {
  const [copied, setCopied] = useState(false);
  const copy = () => {
    navigator.clipboard
      ?.writeText(text)
      .then(() => setCopied(true))
      .catch(() => setCopied(false));
  };
  return (
    <details className="howto">
      <summary>{title}</summary>
      <pre className="mono howto-text">{text}</pre>
      <button type="button" className="button" onClick={copy}>
        {copied ? 'Copied' : 'Copy'}
      </button>
    </details>
  );
}

const ADD_PROVIDER = `# ~/.config/mnemorph/config.toml
[providers.openrouter]
kind = "openai"
base_url = "https://openrouter.ai/api/v1"
key_env = "OPENROUTER_API_KEY"   # the variable's name, never the key

[models.tier]
fast = "openrouter:deepseek/deepseek-v4-flash"`;

const ADD_MCP = `# ~/.config/mnemorph/config.toml
[mcp.servers.search]
command = "web-search-mcp"
args = ["--stdio"]
env = ["SEARCH_API_KEY"]   # passed by name
races = ["kijin"]          # its scope: who may be offered its tools
read_only = true`;

const ADD_BENCH = `# ~/.config/mnemorph/config.toml
[board]
bench_results = "/home/you/kaginari/bench/results"`;

function hidden(v: string | null) {
  return v === null ? <span className="panel-note">hidden</span> : <Safe text={v} />;
}

function KeyMark({ k }: { k: Parameters<typeof keyWords>[0] }) {
  const tone = k.state === 'set' ? 'holo' : k.state === 'unset' ? 'gold' : 'muted';
  return (
    <Mark tone={tone} glyph={k.state === 'set' ? '●' : '○'}>
      <Safe text={keyWords(k)} />
    </Mark>
  );
}

function Config({ c }: { c: ConfigView }) {
  return (
    <>
      <section className="panel panel-wide" aria-label="Providers">
        <h2 className="panel-title">Providers</h2>
        <div className="table-scroll">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">name</th>
                <th scope="col">kind</th>
                <th scope="col">base URL</th>
                <th scope="col">key source</th>
                <th scope="col">from</th>
              </tr>
            </thead>
            <tbody>
              {c.providers.map((p) => (
                <tr key={p.name}>
                  <th scope="row">
                    <Safe text={p.name} />
                  </th>
                  <td>
                    <Safe text={p.kind} />
                  </td>
                  <td className="mono">{hidden(p.baseUrl)}</td>
                  <td>
                    <KeyMark k={p.key} />
                  </td>
                  <td className="panel-note">
                    <Safe text={p.origin} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <HowTo title="How to add a provider" text={ADD_PROVIDER} />
      </section>
      <section className="panel panel-wide" aria-label="Models">
        <h2 className="panel-title">Models and prices</h2>
        <div className="table-scroll">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">key</th>
                <th scope="col">model</th>
                <th scope="col">provider</th>
                <th scope="col">$ / M tokens (in · out · cache read · write)</th>
              </tr>
            </thead>
            <tbody>
              {c.models.map((m) => (
                <tr key={m.key}>
                  <th scope="row" className="mono">
                    <Safe text={m.key} />
                  </th>
                  <td className="mono">{hidden(m.model)}</td>
                  <td>
                    {m.provider ? (
                      <Safe text={m.provider} />
                    ) : (
                      <Mark tone="alarm" glyph="✕">
                        <Safe text={m.problem ?? 'no provider'} />
                      </Mark>
                    )}
                  </td>
                  <td className="mono">
                    {m.price
                      ? `${m.price.input} · ${m.price.output} · ${m.price.cacheRead} · ${m.price.cacheWrite}`
                      : 'unpriced'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
      <section className="panel panel-wide" aria-label="MCP servers">
        <h2 className="panel-title">MCP servers</h2>
        {c.mcp.value === null ? (
          <p className="panel-unknown">
            <Unknown why={c.mcp.why} /> — <Safe text={c.mcp.why ?? ''} />
          </p>
        ) : c.mcp.value.servers.length === 0 ? (
          <p className="panel-note">No MCP server is declared in your global file.</p>
        ) : (
          <div className="table-scroll">
            <table className="table">
              <thead>
                <tr>
                  <th scope="col">server</th>
                  <th scope="col">command</th>
                  <th scope="col">scope</th>
                  <th scope="col">class</th>
                  <th scope="col">env</th>
                </tr>
              </thead>
              <tbody>
                {c.mcp.value.servers.map((s) => (
                  <tr key={s.name}>
                    <th scope="row">
                      <Safe text={s.name} />
                    </th>
                    <td className="mono">
                      {hidden(s.command)}{' '}
                      {s.args.map((a, i) => (
                        // biome-ignore lint/suspicious/noArrayIndexKey: arguments never reorder
                        <span key={i}>
                          {a === null ? (
                            <span className="panel-note">hidden</span>
                          ) : (
                            <Safe text={a} />
                          )}{' '}
                        </span>
                      ))}
                    </td>
                    <td>{s.races.length === 0 ? 'no race' : <Safe text={s.races.join(', ')} />}</td>
                    <td>
                      {s.readOnly ? 'read-only' : 'outward, asked each call'}
                      {s.network ? ' · network' : ''}
                    </td>
                    <td>
                      {s.env.map((k) => (
                        <KeyMark key={k.variable ?? 'hidden'} k={k} />
                      ))}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        <HowTo title="How to add an MCP server" text={ADD_MCP} />
      </section>
      <section className="panel panel-wide" aria-label="Holes">
        <h2 className="panel-title">Holes · {c.holes.length}</h2>
        {c.holes.length === 0 ? (
          <p className="panel-note">Every key was taken.</p>
        ) : (
          <ul className="hole-list">
            {c.holes.map((h, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: holes never reorder
              <li key={i}>
                <Mark tone="gold" glyph="⚠">
                  <span className="mono">{hidden(h.key)}</span>
                </Mark>{' '}
                <Safe text={h.why} />{' '}
                <span className="panel-note">
                  — <Safe text={h.origin} />
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
      <section className="panel panel-wide" aria-label="Effective configuration">
        <h2 className="panel-title">Effective configuration</h2>
        <div className="table-scroll">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">key</th>
                <th scope="col">value</th>
                <th scope="col">from</th>
                <th scope="col">set by</th>
              </tr>
            </thead>
            <tbody>
              {c.effective.map((s) => (
                <tr key={s.key}>
                  <th scope="row" className="mono">
                    <Safe text={s.key} />
                  </th>
                  <td className="mono">{hidden(s.value)}</td>
                  <td>
                    <Safe text={s.layer} />{' '}
                    <span className="panel-note">
                      <Safe text={s.file} />
                    </span>
                  </td>
                  <td>{s.globalOnly ? 'global file only' : 'any layer'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </>
  );
}

function Board({ i, theme }: { i: BoardIntegrations; theme: Choice }) {
  return (
    <section className="panel panel-wide" aria-label="The board">
      <h2 className="panel-title">The board</h2>
      <dl className="panel-rows">
        <div className="panel-row">
          <dt>port</dt>
          <dd className="mono">{i.board.port}</dd>
        </div>
        <div className="panel-row">
          <dt>bound to</dt>
          <dd className="mono">
            <Safe text={i.board.hosts.join(', ')} />
          </dd>
        </div>
        <div className="panel-row">
          <dt>theme</dt>
          <dd>{theme} (Settings changes it, in this browser)</dd>
        </div>
        <div className="panel-row">
          <dt>global file</dt>
          <dd className="mono">
            <Safe text={i.global} />
          </dd>
        </div>
      </dl>
      <HowTo title="How to show bench results" text={ADD_BENCH} />
    </section>
  );
}

export function IntegrationsView({ theme }: { theme: Choice }) {
  const load = useIntegrations(undefined);
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">Integrations</h1>
      </header>
      <p className="hint">
        Read-only. Keys are shown by where they are read from, never by value; to change anything,
        edit your global file.
      </p>
      {load.state === 'loading' && (
        <Empty glyph="⣾" title="Reading the configuration…">
          Asking the board server.
        </Empty>
      )}
      {load.state === 'failed' && (
        <Empty glyph="✕" title="The integrations could not be read">
          <p>
            <Safe text={load.reason} />.
          </p>
        </Empty>
      )}
      {load.state === 'ready' && (
        <div className="panels">
          {load.data.config.value ? (
            <Config c={load.data.config.value} />
          ) : (
            <section className="panel panel-wide" aria-label="Configuration">
              <h2 className="panel-title">Configuration</h2>
              <p className="panel-unknown">
                <Unknown why={load.data.config.why} /> — <Safe text={load.data.config.why ?? ''} />
              </p>
            </section>
          )}
          <Board i={load.data} theme={theme} />
        </div>
      )}
    </div>
  );
}
