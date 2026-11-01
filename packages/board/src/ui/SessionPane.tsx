/**
 * The side pane beside a chat, as the TUI's sidebar keeps it: the vitals (how full the context is,
 * the tokens, the spend — never $0 for what was not priced), the world's memory counted, the
 * files the session's acts changed as a tree, and the bodies it commissioned.
 */
import { useEffect, useState } from 'react';
import type { FileMark } from '../data/chat.ts';
import { establish, signedOut } from '../data/session.ts';
import { Safe } from '../safe/Safe.tsx';
import type { Side } from '../views/Chat.tsx';
import { tokens } from '../views/Chat.tsx';
import { Mark } from './Mark.tsx';

function Block({
  title,
  count,
  children,
}: {
  title: string;
  count?: number;
  children: React.ReactNode;
}) {
  return (
    <section className="pane-block">
      <h3 className="pane-title">
        ▾ {title}
        {count !== undefined && <span className="pane-count"> · {count}</span>}
      </h3>
      {children}
    </section>
  );
}

function Row({ k, children }: { k: string; children: React.ReactNode }) {
  return (
    <div className="pane-row">
      <dt>{k}</dt>
      <dd>{children}</dd>
    </div>
  );
}

const MEMORY_KEYS = ['grounded', 'shared', 'proposed', 'retired', 'unproven'] as const;
type Counts = Record<(typeof MEMORY_KEYS)[number], number>;

/** The world's memory counted, as its overview (`GET /api/w/<id>`) reads it; `undefined` while
 * unknown, `null` when the board does not say. */
function useMemory(world: string): Counts | null | undefined {
  const [m, setM] = useState<Counts | null | undefined>(undefined);
  useEffect(() => {
    const ctl = new AbortController();
    setM(undefined);
    establish()
      .then(() =>
        fetch(`/api/w/${encodeURIComponent(world)}`, {
          signal: ctl.signal,
          credentials: 'same-origin',
          headers: { accept: 'application/json' },
        }),
      )
      .then(async (res) => {
        if (res.status === 401) signedOut();
        if (!res.ok) return setM(null);
        const v = ((await res.json()) as { memory?: { value?: unknown } })?.memory?.value;
        if (typeof v !== 'object' || v === null) return setM(null);
        const out = {} as Counts;
        for (const k of MEMORY_KEYS) {
          const n = (v as Record<string, unknown>)[k];
          if (typeof n !== 'number') return setM(null);
          out[k] = n;
        }
        setM(out);
      })
      .catch(() => {
        if (!ctl.signal.aborted) setM(null);
      });
    return () => ctl.abort();
  }, [world]);
  return m;
}

const MARK_WORD: Record<FileMark, string> = { A: 'added', M: 'modified', D: 'deleted' };

/** Paths as a tree of their directories, one level: each directory once, its files under it in
 * the order touched; files at the root first. */
export function tree(files: readonly (readonly [string, FileMark])[]) {
  const root: [string, FileMark][] = [];
  const dirs = new Map<string, [string, FileMark][]>();
  for (const [p, m] of files) {
    const cut = p.lastIndexOf('/');
    if (cut < 0) root.push([p, m]);
    else {
      const d = p.slice(0, cut);
      const list = dirs.get(d) ?? [];
      list.push([p.slice(cut + 1), m]);
      dirs.set(d, list);
    }
  }
  return { root, dirs: [...dirs] };
}

function File({ name, mark }: { name: string; mark: FileMark }) {
  return (
    <li className="file" data-mark={mark}>
      <span className="file-mark" title={MARK_WORD[mark]}>
        {mark}
      </span>{' '}
      <span className="mono">
        <Safe text={name} />
      </span>
      <span className="sr-only"> {MARK_WORD[mark]}</span>
    </li>
  );
}

export function SessionPane({ side }: { side: Side | undefined }) {
  const memory = useMemory(side?.world ?? '');
  const v = side?.view;
  const u = v?.usage;
  const pct = u && u.stressLine > 0 ? Math.round((u.occupancy * 100) / u.stressLine) : undefined;
  const spend = !u?.seen
    ? '—'
    : u.unpriced
      ? u.usd > 0
        ? `$${u.usd.toFixed(2)} + unpriced`
        : 'unpriced'
      : `$${u.usd.toFixed(2)}`;
  const files = tree(v?.files ?? []);
  const me = side?.hello?.body;
  return (
    <>
      <h2 className="pane-head">{side?.live === false ? 'Past chat' : 'Session'}</h2>
      <Block title="vitals">
        <dl className="pane-rows">
          <Row k="ctx">
            {pct === undefined ? (
              '—'
            ) : (
              <span className="gauge-row">
                <meter
                  className="gauge"
                  min={0}
                  max={100}
                  low={70}
                  high={90}
                  optimum={0}
                  value={Math.min(100, pct)}
                  aria-label="context"
                />
                <span>
                  {pct}% / {tokens(u?.stressLine ?? 0)}
                </span>
              </span>
            )}
          </Row>
          <Row k="tokens">{u?.seen ? tokens(u.tokens) : '—'}</Row>
          <Row k="spend">{spend}</Row>
          <Row k="weighted">{u?.seen ? `${tokens(u.spentWt)} wt` : '—'}</Row>
          {side?.hello && (
            <Row k="sandbox">
              <Safe text={side.hello.sandbox} />
            </Row>
          )}
        </dl>
      </Block>
      <Block title="memory">
        {memory === undefined ? (
          <p className="pane-note">loading…</p>
        ) : memory === null ? (
          <p className="pane-note">not read: the world's overview does not say</p>
        ) : (
          <dl className="pane-rows">
            {MEMORY_KEYS.map((k) => (
              <Row key={k} k={k}>
                {k === 'unproven' && memory[k] > 0 ? (
                  <Mark tone="alarm" glyph="⚠">
                    {memory[k]}
                  </Mark>
                ) : (
                  memory[k]
                )}
              </Row>
            ))}
          </dl>
        )}
      </Block>
      <Block title="files" count={v?.files.length ?? 0}>
        {(v?.files.length ?? 0) === 0 ? (
          <p className="pane-note">— none changed</p>
        ) : (
          <ul className="files">
            {files.root.map(([n, m]) => (
              <File key={n} name={n} mark={m} />
            ))}
            {files.dirs.map(([d, list]) => (
              <li key={d} className="dir">
                <span className="mono">
                  <Safe text={`${d}/`} />
                </span>
                <ul className="files">
                  {list.map(([n, m]) => (
                    <File key={n} name={n} mark={m} />
                  ))}
                </ul>
              </li>
            ))}
          </ul>
        )}
      </Block>
      <Block title="bodies" count={(v?.bodies.length ?? 0) + (me ? 1 : 0)}>
        {me ? (
          <ul className="bodies">
            <li>
              <Mark tone={v?.busy ? 'holo' : 'muted'} glyph="◆">
                <Safe text={me} /> {v?.busy ? '· working' : '· idle'}
              </Mark>
              {(v?.bodies.length ?? 0) > 0 && (
                <ul className="bodies">
                  {v?.bodies.map((b) => (
                    <li key={b.name}>
                      <span aria-hidden="true">└ </span>
                      <Mark
                        tone={
                          b.state === 'failed' ? 'alarm' : b.state === 'working' ? 'holo' : 'muted'
                        }
                        glyph={b.state === 'failed' ? '✕' : b.state === 'working' ? '●' : '○'}
                      >
                        <Safe text={b.name} /> · {b.state}
                      </Mark>
                    </li>
                  ))}
                </ul>
              )}
            </li>
          </ul>
        ) : (v?.bodies.length ?? 0) > 0 ? (
          <ul className="bodies">
            {v?.bodies.map((b) => (
              <li key={b.name}>
                <Safe text={b.name} /> · {b.state}
              </li>
            ))}
          </ul>
        ) : (
          <p className="pane-note">— none commissioned</p>
        )}
      </Block>
    </>
  );
}
