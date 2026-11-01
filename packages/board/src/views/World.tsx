/** Inside a world: its overview, a section, or a session. */
import type { BoardMissing, BoardOverview, BoardWorld, Sourced } from '@mnemorph/protocol';
import { useOverview } from '../data/overview.ts';
import { usePush } from '../data/push.ts';
import type { WorldSection } from '../router.ts';
import { Safe } from '../safe/Safe.tsx';
import { shellWord, since } from '../time.ts';
import { Empty } from '../ui/Empty.tsx';
import { Mark, STATUS, WORLD_STATUS } from '../ui/Mark.tsx';
import { SECTIONS } from '../ui/Rail.tsx';
import { GatePill, money, rate, StatusMark, tokens, When } from './facts.tsx';

/** The world a route names, or why there is none. */
export function NoWorld({ id }: { id: string }) {
  return (
    <div className="page">
      <Empty glyph="✕" title="No such world">
        <p>
          No world with the id{' '}
          <code className="mono">
            <Safe text={id} />
          </code>{' '}
          is known to this board.
        </p>
        <p>
          <a href="#/">See every world</a>
        </p>
      </Empty>
    </div>
  );
}

/** A world the board knows whose directory is gone: where it was, since when, how to forget it. */
export function MissingWorld({ m, now }: { m: BoardMissing; now: number }) {
  return (
    <div className="page">
      <Empty glyph="✕" title={<Safe text={`${m.name} is missing`} />}>
        <p>
          Its directory{' '}
          <code className="mono">
            <Safe text={m.path} />
          </code>{' '}
          is gone, or holds no <code className="mono">.mnemorph/</code>: missing since{' '}
          {since(m.since, now)}. It is live again once it is back.
        </p>
        <p>
          To forget it:{' '}
          <code className="mono">
            <Safe text={`mnemorphd board forget ${shellWord(m.path)}`} />
          </code>
        </p>
        <p>
          <a href="#/">See every world</a>
        </p>
      </Empty>
    </div>
  );
}

/** Where a field was read, and whether that source is the harness's own or the world's. */
function Source<T>({ f }: { f: Sourced<T> }) {
  return (
    <p className="panel-source">
      <Mark tone={f.trusted ? 'holo' : 'muted'} glyph={f.trusted ? '◆' : '◇'}>
        {f.trusted ? 'harness' : 'in-world, shown only'}
      </Mark>{' '}
      <span className="mono">
        <Safe text={f.source} />
      </span>
      {f.value !== null && f.why && (
        <>
          {' '}
          · <Safe text={f.why} />
        </>
      )}
    </p>
  );
}

function Panel<T>({
  title,
  f,
  children,
}: {
  title: string;
  f: Sourced<T>;
  children: (v: T) => React.ReactNode;
}) {
  return (
    <section className="panel" aria-label={title}>
      <h2 className="panel-title">{title}</h2>
      <div className="panel-body">
        {f.value === null ? (
          <p className="panel-unknown">
            <span className="unknown">unknown</span> — <Safe text={f.why ?? 'not said'} />
          </p>
        ) : (
          children(f.value)
        )}
      </div>
      <Source f={f} />
    </section>
  );
}

function Rows({ rows }: { rows: [string, React.ReactNode][] }) {
  return (
    <dl className="panel-rows">
      {rows.map(([k, v]) => (
        <div key={k} className="panel-row">
          <dt>{k}</dt>
          <dd>{v}</dd>
        </div>
      ))}
    </dl>
  );
}

function Overview({ o, now }: { o: BoardOverview; now: number }) {
  return (
    <div className="panels">
      <Panel title="Status" f={o.status}>
        {(v) => (
          <>
            <p>
              <Mark tone={WORLD_STATUS[v].tone} glyph={WORLD_STATUS[v].glyph}>
                {WORLD_STATUS[v].label}
              </Mark>
            </p>
          </>
        )}
      </Panel>
      <Panel title="Last activity" f={o.activity}>
        {(v) => (
          <p>
            <Safe text={v.what} /> · <When at={v.at} now={now} />{' '}
            <span className="mono panel-note">{v.at}</span>
          </p>
        )}
      </Panel>
      <Panel title="Memory" f={o.memory}>
        {(m) => (
          <Rows
            rows={[
              ['grounded', m.grounded],
              ['shared', m.shared],
              ['proposed', m.proposed],
              ['retired', m.retired],
              [
                'unproven',
                m.unproven > 0 ? (
                  <Mark key="unproven" tone="gold" glyph="⚠">
                    {m.unproven} · writes frozen
                  </Mark>
                ) : (
                  0
                ),
              ],
            ]}
          />
        )}
      </Panel>
      <Panel title="Challenges" f={o.challenges}>
        {(c) => (
          <Rows
            rows={[
              ['pending', c.pending],
              [
                'blocked',
                c.blocked > 0 ? (
                  <Mark key="blocked" tone="alarm" glyph="◇">
                    {c.blocked}
                  </Mark>
                ) : (
                  0
                ),
              ],
              ['verifying', c.verifying],
              ['unstaffed', c.unstaffed],
              ['open findings', c.findings],
              ...(c.unreadable > 0
                ? ([
                    [
                      'unreadable',
                      <Mark key="u" tone="gold" glyph="⚠">
                        {c.unreadable} line(s)
                      </Mark>,
                    ],
                  ] as [string, React.ReactNode][])
                : []),
            ]}
          />
        )}
      </Panel>
      <Panel title="End gate" f={o.gate}>
        {(g) =>
          g.last === null ? (
            <p>No verdict yet.</p>
          ) : (
            <>
              <p>
                <GatePill g={g} /> on <Safe text={g.last.body} />
                's landing · <When at={g.last.at} now={now} />
              </p>
              <p className="panel-note">
                {g.last.changed} {g.last.changed === 1 ? 'path' : 'paths'} changed · {g.verdicts}{' '}
                {g.verdicts === 1 ? 'verdict' : 'verdicts'} in all
              </p>
            </>
          )
        }
      </Panel>
      <Panel title="Latest spend" f={o.spend}>
        {(s) => (
          <Rows
            rows={[
              ['session', <Safe key="s" text={s.session} />],
              ['cost', money(s)],
              ['calls', s.calls],
              ['weighted tokens', tokens(s.wt)],
            ]}
          />
        )}
      </Panel>
      <Panel title="Meters" f={o.meters}>
        {(all) =>
          all.length === 0 ? (
            <p>No living creature.</p>
          ) : (
            <ul className="meter-list">
              {all.map((c) => (
                <li key={c.creature}>
                  <span className="meter-who">
                    <Safe text={c.creature} />{' '}
                    <span className="panel-note">
                      (<Safe text={c.race} />)
                    </span>
                  </span>
                  <ul className="meter-readings mono">
                    {c.meters.map((m) => (
                      <li key={m.meter}>
                        <Safe text={m.meter} /> <Safe text={rate(m)} />
                      </li>
                    ))}
                  </ul>
                </li>
              ))}
            </ul>
          )
        }
      </Panel>
    </div>
  );
}

export function WorldView({ w, now }: { w: BoardWorld; now: number }) {
  const load = useOverview(w.id);
  const push = usePush();
  const o = load.state === 'loading' ? undefined : load.data;
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">
          <Safe text={w.name} />
        </h1>
        {o ? (
          <StatusMark o={o} />
        ) : (
          <Mark tone={STATUS[w.state].tone} glyph={STATUS[w.state].glyph}>
            {STATUS[w.state].label}
          </Mark>
        )}
      </header>
      <p className="mono page-sub">
        <Safe text={w.path} />
      </p>
      <p className="page-live" data-push={push}>
        {push === 'open'
          ? 'Updates as the world changes.'
          : push === 'connecting'
            ? 'Connecting for updates…'
            : 'Updates paused: the board’s socket is down, reconnecting. What is shown may be stale.'}
      </p>
      {w.state === 'missing' && (
        <p className="hint">
          <Mark tone="alarm" glyph="✕">
            This world's directory is gone.
          </Mark>{' '}
          It stays listed, never silently dropped, until{' '}
          <code className="mono">mnemorphd board forget &lt;dir&gt;</code>. What the harness kept
          for it is still shown.
        </p>
      )}
      {o?.updates.value === 'slowed' && (
        <p className="hint">
          <Mark tone="gold" glyph="⚠">
            <Safe text={o.updates.why ?? 'updates slowed'} />
          </Mark>
        </p>
      )}
      {o?.waiting.value ? (
        <p className="hint">
          <Mark tone="gold" glyph="◆">
            Waiting for you: {o.waiting.value} gate {o.waiting.value === 1 ? 'ask' : 'asks'}
          </Mark>
        </p>
      ) : null}
      {load.state === 'loading' && (
        <Empty glyph="⣾" title="Loading the overview…">
          Asking the board server what this world holds.
        </Empty>
      )}
      {load.state === 'failed' && (
        <p className="hint" role="status">
          <Mark tone="alarm" glyph="✕">
            The overview could not be read: <Safe text={load.reason} />.
          </Mark>
          {load.data && ' What was read before is shown below.'}
        </p>
      )}
      {o && <Overview o={o} now={now} />}
    </div>
  );
}

export function SectionView({ w, section }: { w: BoardWorld; section: WorldSection }) {
  const [, glyph, label] = SECTIONS.find(([s]) => s === section) ?? ['', '◇', section];
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">
          {label}
          <span className="page-crumb">
            <Safe text={w.name} />
          </span>
        </h1>
      </header>
      <Empty glyph={glyph} title={`${label} is not drawn yet`}>
        This page reads the world's {label.toLowerCase()} once the board server serves it.
      </Empty>
    </div>
  );
}
