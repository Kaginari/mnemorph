/**
 * Metrics: what the usage ledgers spent over a window — the totals line, spend per day by race,
 * the cache hit rate, the burn — beside the race meters with their count and interval, the Dark
 * Elf's scores and the bench. Unpriced never reads $0; a silent meter reads "silent since …", an unavailable one says why;
 * every word a world wrote goes through `<Safe>`.
 */
import './metrics.css';
import type { BoardMetrics, MetricsCreature, Named, Sourced, Tally } from '@mnemorph/protocol';
import { useState } from 'react';
import {
  dollars,
  duration,
  pct,
  silent,
  tokens,
  totals,
  useMetrics,
  WINDOWS,
  type Window,
  wt,
} from '../data/metrics.ts';
import type { Load } from '../data/worlds.ts';
import { format } from '../router.ts';
import { Safe } from '../safe/Safe.tsx';
import { Legend, ShareLine, Sparkline, StackedBars, Whisker } from '../ui/Charts.tsx';
import { Empty } from '../ui/Empty.tsx';
import { Mark, type Tone } from '../ui/Mark.tsx';
import { rate, Unknown } from './facts.tsx';

function Source({ s }: { s: Sourced<unknown> }) {
  return (
    <p className="panel-source">
      <Safe text={s.source} />
      {s.trusted ? ' · the harness’s own' : ' · read from inside the world'}
      {s.value !== null && s.why && (
        <>
          {' '}
          — <Safe text={s.why} />
        </>
      )}
    </p>
  );
}

function Panel({
  title,
  s,
  children,
  wide = false,
}: {
  title: string;
  s: Sourced<unknown>;
  children: React.ReactNode;
  wide?: boolean;
}) {
  return (
    <section className={wide ? 'panel panel-wide' : 'panel'} aria-label={title}>
      <h2 className="panel-title">{title}</h2>
      <div className="panel-body">
        {s.value === null ? (
          <p className="panel-unknown">
            <Unknown why={s.why} /> — <Safe text={s.why ?? ''} />
          </p>
        ) : (
          children
        )}
      </div>
      <Source s={s} />
    </section>
  );
}

function Charts({ m }: { m: BoardMetrics }) {
  const u = m.usage.value;
  if (!u) {
    return (
      <Panel title="Spend" s={m.usage}>
        {null}
      </Panel>
    );
  }
  const days = u.days.map((d) => ({
    day: d.day,
    parts: d.races.map((r) => ({ series: r.name, value: r.spent.wt })),
  }));
  return (
    <div className="panels metrics-charts">
      <Panel title="Spend per day by race" s={m.usage} wide>
        {days.length === 0 ? (
          <p className="panel-note">No call in this window.</p>
        ) : (
          <StackedBars days={days} label="weighted tokens per day, stacked by race" value={wt} />
        )}
      </Panel>
      <Panel title="Cache hit" s={m.usage}>
        <p className="metric-big">{pct(u.total.cacheHit)}</p>
        {u.days.length > 0 && (
          <ShareLine
            values={u.days.map((d) => d.spent.cacheHit)}
            keys={u.days.map((d) => d.day)}
            label="the share of input read from the cache, per day"
          />
        )}
      </Panel>
      <Panel title="Burn · 5 min" s={m.usage}>
        <p className="metric-big">{wt(Math.round(u.burnPerMinute))}/min</p>
        <Sparkline
          values={u.burn}
          label="weighted tokens per five minutes, the last twelve hours"
        />
        <p className="panel-note">the last 12 hours, in {u.burnSlotSecs / 60}-minute slots</p>
      </Panel>
    </div>
  );
}

function Races({ m }: { m: BoardMetrics }) {
  const u = m.usage.value;
  if (!u) return null;
  return (
    <Panel title="Per race" s={m.usage} wide>
      {u.races.length === 0 ? (
        <p className="panel-note">No call in this window.</p>
      ) : (
        <div className="table-scroll">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">race</th>
                <th scope="col">spend</th>
                <th scope="col">weighted tokens</th>
                <th scope="col">calls</th>
                <th scope="col">cache hit</th>
              </tr>
            </thead>
            <tbody>
              {u.races.map((r) => (
                <tr key={r.name}>
                  <th scope="row">
                    <Legend series={[r.name]} />
                  </th>
                  <td>{dollars(r.spent.usd, r.spent.unpriced)}</td>
                  <td>{wt(r.spent.wt)}</td>
                  <td>{r.spent.calls}</td>
                  <td>{pct(r.spent.cacheHit)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}

function Sessions({ m, names }: { m: BoardMetrics; names: Map<string, string> }) {
  const u = m.usage.value;
  if (!u) return null;
  return (
    <Panel title="Sessions" s={m.usage} wide>
      {u.sessions.length === 0 ? (
        <p className="panel-note">No session in this window.</p>
      ) : (
        <div className="table-scroll">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">session</th>
                <th scope="col">world</th>
                <th scope="col">last call</th>
                <th scope="col">spend</th>
                <th scope="col">tokens</th>
                <th scope="col">calls</th>
              </tr>
            </thead>
            <tbody>
              {u.sessions.slice(0, 50).map((s) => (
                <tr key={`${s.world}/${s.session}`}>
                  <th scope="row" className="mono">
                    <Safe text={s.session} />
                  </th>
                  <td>
                    <Safe text={names.get(s.world) ?? s.world} />
                  </td>
                  <td>
                    <time dateTime={s.last}>{s.last.slice(0, 16).replace('T', ' ')}</time>
                  </td>
                  <td>{dollars(s.spent.usd, s.spent.unpriced)}</td>
                  <td>{wt(s.spent.wt)}</td>
                  <td>{s.spent.calls}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}

function CreatureMeters({ c, names }: { c: MetricsCreature; names: Map<string, string> }) {
  return (
    <li>
      <span className="meter-who">
        <Legend series={[c.race]} />
        <Safe text={c.creature} />
        {names.size > 1 && (
          <span className="panel-note">
            {' '}
            · <Safe text={names.get(c.world) ?? c.world} />
          </span>
        )}
      </span>
      <ul className="meter-readings">
        {c.meters.map((r) => (
          <li key={r.meter} className="meter-reading">
            <Safe text={r.meter} />{' '}
            {r.status === 'unavailable' ? (
              <Mark tone="alarm" glyph="✕">
                <Safe text={rate(r)} />
              </Mark>
            ) : r.n === null ? (
              <Mark tone="muted" glyph="◌">
                {silent(c.firstCall)}
              </Mark>
            ) : (
              <>
                <Whisker value={r.value} lo={r.lo} hi={r.hi} label={`${r.meter}: ${rate(r)}`} />{' '}
                <span className="mono">{rate(r)}</span>
              </>
            )}
          </li>
        ))}
      </ul>
    </li>
  );
}

function Meters({ m, names }: { m: BoardMetrics; names: Map<string, string> }) {
  return (
    <Panel title="Meters · n and 95% interval" s={m.meters} wide>
      {m.meters.value && m.meters.value.length === 0 ? (
        <p className="panel-note">No living creature.</p>
      ) : (
        <ul className="meter-list">
          {m.meters.value?.map((c) => (
            <CreatureMeters key={`${c.world}/${c.creature}`} c={c} names={names} />
          ))}
        </ul>
      )}
    </Panel>
  );
}

function Scores({ m }: { m: BoardMetrics }) {
  const s = m.scores.value;
  return (
    <Panel title="Dark Elf scores" s={m.scores} wide>
      {s && s.length === 0 ? (
        <p className="panel-note">No Dark Elf finding is on record.</p>
      ) : (
        <div className="table-scroll">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">speciality</th>
                <th scope="col">findings</th>
                <th scope="col">upheld · refuted · cannot verify · held</th>
                <th scope="col">precision</th>
                <th scope="col">last ten</th>
                <th scope="col">proposed</th>
              </tr>
            </thead>
            <tbody>
              {s?.map((x) => (
                <tr key={`${x.world}/${x.speciality}`}>
                  <th scope="row">
                    <Safe text={x.speciality} />
                    {x.form && (
                      <span className="panel-note">
                        {' '}
                        (<Safe text={x.form} />)
                      </span>
                    )}
                  </th>
                  <td>{x.findings}</td>
                  <td>
                    {x.upheld} · {x.refuted} · {x.cannotVerify} · {x.held}
                  </td>
                  <td className="mono">{rate(x.precision)}</td>
                  <td className="mono">{rate(x.recent)}</td>
                  <td>
                    {x.proposal ? (
                      <Mark tone={x.proposal === 'rise' ? 'holo' : 'gold'} glyph="◆">
                        <Safe text={x.proposal} />
                      </Mark>
                    ) : (
                      '—'
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}

function BenchTable({ m }: { m: BoardMetrics }) {
  const b = m.bench.value;
  return (
    <Panel title="Bench" s={m.bench} wide>
      {b && !b.configured ? (
        <p className="panel-note">
          Not configured. Name the bench results' directory in your global file:{' '}
          <code className="mono">[board] bench_results = "~/kaginari/bench/results"</code>
        </p>
      ) : b && b.rows.length === 0 ? (
        <p className="panel-note">No result row in the configured directory.</p>
      ) : (
        <div className="table-scroll">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">suite</th>
                <th scope="col">harness</th>
                <th scope="col">model</th>
                <th scope="col">mode</th>
                <th scope="col">score</th>
                <th scope="col">$</th>
                <th scope="col">date</th>
              </tr>
            </thead>
            <tbody>
              {b?.rows.map((r) => (
                <tr key={`${r.suite}/${r.harness}/${r.model}/${r.mode}`}>
                  <th scope="row">
                    <Safe text={r.suite} />
                  </th>
                  <td>
                    <Safe text={r.harness} />
                  </td>
                  <td className="mono">
                    <Safe text={r.model} />
                  </td>
                  <td>
                    <Safe text={r.mode} />
                  </td>
                  <td>
                    {r.passed}/{r.valid}
                    {r.invalid > 0 && (
                      <span className="panel-note"> ({r.invalid} not counted)</span>
                    )}
                  </td>
                  <td>{dollars(r.usd, r.unpriced)}</td>
                  <td>{r.date ? <time dateTime={r.date}>{r.date.slice(0, 10)}</time> : '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {b && b.unreadable > 0 && (
        <p className="panel-note">
          <Mark tone="gold" glyph="⚠">
            {b.unreadable} result {b.unreadable === 1 ? 'line or file' : 'lines or files'} did not
            read
          </Mark>
        </p>
      )}
    </Panel>
  );
}

/** A usage table by name: per model, per body. */
function SpentTable({
  title,
  of,
  rows,
  s,
}: {
  title: string;
  of: string;
  rows: Named[];
  s: Sourced<unknown>;
}) {
  return (
    <Panel title={title} s={s} wide>
      {rows.length === 0 ? (
        <p className="panel-note">No call in this window.</p>
      ) : (
        <div className="table-scroll">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">{of}</th>
                <th scope="col">spend</th>
                <th scope="col">weighted tokens</th>
                <th scope="col">calls</th>
                <th scope="col">input · output</th>
                <th scope="col">cache read · write</th>
                <th scope="col">cache hit</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.name}>
                  <th scope="row" className="mono">
                    <Safe text={r.name} />
                  </th>
                  <td>{dollars(r.spent.usd, r.spent.unpriced)}</td>
                  <td>{wt(r.spent.wt)}</td>
                  <td>{r.spent.calls}</td>
                  <td className="mono">
                    {tokens(r.spent.input)} · {tokens(r.spent.output)}
                  </td>
                  <td className="mono">
                    {tokens(r.spent.cacheRead)} · {tokens(r.spent.cacheWrite)}
                  </td>
                  <td>{pct(r.spent.cacheHit)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}

/** A gate verdict's tone and glyph. */
function verdictMark(v: string): { tone: Tone; glyph: string } {
  switch (v) {
    case 'PASS':
      return { tone: 'holo', glyph: '✓' };
    case 'FAIL':
      return { tone: 'alarm', glyph: '✕' };
    case 'NOT_VERIFIED':
    case 'UNCHECKED':
      return { tone: 'gold', glyph: '◌' };
    default:
      return { tone: 'muted', glyph: '·' };
  }
}

function TallyList({ label, rows, marked }: { label: string; rows: Tally[]; marked?: boolean }) {
  const all = rows.reduce((n, r) => n + r.count, 0);
  return (
    <div className="tally">
      <h3 className="tally-title">{label}</h3>
      <ul className="tally-list">
        {rows.map((r) => {
          const share = all === 0 ? 0 : Math.round((r.count / all) * 100);
          const m = marked ? verdictMark(r.name) : { tone: 'muted' as Tone, glyph: '·' };
          return (
            <li key={r.name} className="tally-row">
              <span className="tally-name">
                <Mark tone={m.tone} glyph={m.glyph}>
                  <Safe text={r.name.toLowerCase().replace('_', ' ')} />
                </Mark>
              </span>
              <span className="tally-bar" aria-hidden="true">
                <span className="tally-fill" data-fill={m.tone} style={{ width: `${share}%` }} />
              </span>
              <span className="tally-count mono">
                {r.count} · {share}%
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function RunsSummary({ m }: { m: BoardMetrics }) {
  const h = m.runs.value;
  return (
    <Panel title="Runs · verdicts and endings" s={m.runs} wide>
      {h && h.runs === 0 ? (
        <p className="panel-note">No session run in this window.</p>
      ) : (
        h && (
          <>
            <p className="panel-note">
              {h.runs} {h.runs === 1 ? 'run' : 'runs'}
              {h.wroteCode > 0 && (
                <>
                  {' '}
                  · a test ran after the last code write in {h.testedAfter} of {h.wroteCode}
                </>
              )}
              {h.leftOut > 0 && <> · {h.leftOut} left out (past the caps, or unreadable)</>}
            </p>
            <div className="tallies">
              <TallyList label="last gate verdict" rows={h.verdicts} marked />
              <TallyList label="how it ended" rows={h.endings} />
            </div>
          </>
        )
      )}
    </Panel>
  );
}

function RunHealthTable({ m, names }: { m: BoardMetrics; names: Map<string, string> }) {
  const h = m.runs.value;
  if (!h || h.rows.length === 0) return null;
  return (
    <Panel title="Run health · the newest runs" s={m.runs} wide>
      <div className="table-scroll">
        <table className="table">
          <thead>
            <tr>
              <th scope="col">run</th>
              <th scope="col">world</th>
              <th scope="col">when</th>
              <th scope="col">turns · calls</th>
              <th scope="col">model · tools</th>
              <th scope="col">re-read</th>
              <th scope="col">cut short</th>
              <th scope="col">lines</th>
              <th scope="col">tested after write</th>
              <th scope="col">gate</th>
              <th scope="col">ended</th>
            </tr>
          </thead>
          <tbody>
            {h.rows.map((r) => {
              const v = verdictMark(r.verdict);
              return (
                <tr key={`${r.world}/${r.run}`}>
                  <th scope="row" className="mono">
                    <a href={format({ name: 'run', world: r.world, run: r.run })}>
                      <Safe text={r.run} />
                    </a>
                  </th>
                  <td>
                    <Safe text={names.get(r.world) ?? r.world} />
                  </td>
                  <td>
                    <time dateTime={r.when}>{r.when.slice(0, 16).replace('T', ' ')}</time>
                  </td>
                  <td>
                    {r.turns} · {r.calls}
                  </td>
                  <td className="mono">
                    {duration(r.modelSecs)} · {duration(r.toolSecs)}
                  </td>
                  <td>{pct(r.reread)}</td>
                  <td>
                    {r.truncated > 0 ? (
                      <Mark tone="gold" glyph="✂">
                        {r.truncated}
                      </Mark>
                    ) : (
                      '0'
                    )}
                  </td>
                  <td className="mono">
                    +{r.linesAdded} −{r.linesRemoved}
                  </td>
                  <td>
                    {r.testedAfterWrite === null ? (
                      '—'
                    ) : r.testedAfterWrite ? (
                      <Mark tone="holo" glyph="✓">
                        yes
                      </Mark>
                    ) : (
                      <Mark tone="gold" glyph="⚠">
                        no
                      </Mark>
                    )}
                  </td>
                  <td>
                    {r.verdict ? (
                      <Mark tone={v.tone} glyph={v.glyph}>
                        <Safe text={r.verdict.toLowerCase().replace('_', ' ')} />
                      </Mark>
                    ) : (
                      '—'
                    )}
                  </td>
                  <td>
                    <Safe text={r.why} />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </Panel>
  );
}

export function MetricsView({ load }: { load: Load }) {
  const [world, setWorld] = useState('all');
  const [window, setWindow] = useState<Window>('7d');
  const m = useMetrics(world, window);
  const worlds = load.state === 'ready' ? load.data.worlds : [];
  const names = new Map(worlds.map((w) => [w.id, w.name]));
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">Metrics</h1>
        <div className="metrics-filters">
          <label className="select">
            <span className="visually-hidden">World</span>
            <select value={world} onChange={(e) => setWorld(e.target.value)}>
              <option value="all">every world</option>
              {worlds.map((w) => (
                <option key={w.id} value={w.id}>
                  {w.name}
                </option>
              ))}
            </select>
          </label>
          <fieldset className="tabs">
            <legend className="visually-hidden">Window</legend>
            {WINDOWS.map((w) => (
              <button
                key={w}
                type="button"
                className="tab"
                aria-pressed={window === w}
                onClick={() => setWindow(w)}
              >
                {w}
              </button>
            ))}
          </fieldset>
        </div>
      </header>
      {m.state === 'loading' && (
        <Empty glyph="⣾" title="Loading the ledgers…">
          Folding the usage ledgers, the meters and the scores.
        </Empty>
      )}
      {m.state === 'failed' && (
        <Empty glyph="✕" title="The metrics could not be read">
          <p>
            <Safe text={m.reason} />.
          </p>
        </Empty>
      )}
      {m.state === 'ready' && (
        <>
          <p className="metrics-total mono">
            <span className="visually-hidden">Totals: </span>
            {m.data.usage.value ? (
              <>
                {totals(m.data.usage.value.total)} · {m.data.usage.value.total.calls} calls · in{' '}
                {tokens(m.data.usage.value.total.input)} · out{' '}
                {tokens(m.data.usage.value.total.output)} · cache read{' '}
                {tokens(m.data.usage.value.total.cacheRead)} · write{' '}
                {tokens(m.data.usage.value.total.cacheWrite)} · {m.data.window}
              </>
            ) : (
              <Unknown why={m.data.usage.why} />
            )}
          </p>
          <Charts m={m.data} />
          <div className="panels metrics-tables">
            <RunsSummary m={m.data} />
            <RunHealthTable m={m.data} names={names} />
            <Races m={m.data} />
            {m.data.usage.value && (
              <SpentTable
                title="Per model"
                of="model"
                rows={m.data.usage.value.models}
                s={m.data.usage}
              />
            )}
            {m.data.usage.value && (
              <SpentTable
                title="Per body"
                of="body"
                rows={m.data.usage.value.bodies}
                s={m.data.usage}
              />
            )}
            <Meters m={m.data} names={names} />
            <Sessions m={m.data} names={names} />
            <Scores m={m.data} />
            <BenchTable m={m.data} />
          </div>
        </>
      )}
    </div>
  );
}
