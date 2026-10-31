/**
 * Bodies & queue: the colony net (race lanes, the chain of command, live state) with a body's card
 * — its race art, its law, its Minds, its latest acts — the race cards of the compiled law, the
 * shared queue, and the timeline of commissions and verdicts. Every word read inside the world is
 * a body's: shown through `<Safe>`, granting nothing; each part says where it was read.
 */
import type {
  BoardBody,
  BoardOrder,
  BoardWorld,
  BodyState,
  MindStanding,
  OrderState,
  Provenance,
  RaceLaw,
  TimelineEntry,
  TimelineKind,
} from '@mnemorph/protocol';
import { useMemo, useState } from 'react';
import {
  type BodiesPage,
  ORDER_STATES,
  type TimelineQuery,
  useBodies,
  useQueue,
  useTimeline,
} from '../data/bodies.ts';
import { colony, STATE_WORDS } from '../data/colony.ts';
import type { KnowledgeLoad } from '../data/knowledge.ts';
import { BODIES_TABS, type BodiesTab, format } from '../router.ts';
import { Safe } from '../safe/Safe.tsx';
import { Empty } from '../ui/Empty.tsx';
import { Mark, type Tone } from '../ui/Mark.tsx';
import { ColonyNet } from './ColonyNet.tsx';
import { verdictTone } from './facts.tsx';
import { Pager } from './Memory.tsx';
import './knowledge.css';
import './bodies.css';

const TAB_WORDS: Record<BodiesTab, string> = {
  colony: 'Colony',
  races: 'Race cards',
  queue: 'Queue',
  timeline: 'Timeline',
};

export const STATE_TONE: Record<BodyState, Tone> = {
  working: 'holo',
  idle: 'muted',
  cut: 'gold',
  never: 'muted',
  retired: 'muted',
};

const MIND: Record<MindStanding, { tone: Tone; glyph: string; words: string }> = {
  adopted: { tone: 'holo', glyph: '✓', words: 'adopted: loads' },
  unadopted: { tone: 'gold', glyph: '○', words: 'unadopted: does not load' },
  changed: { tone: 'alarm', glyph: '⚠', words: 'changed since adoption: does not load' },
  retired: { tone: 'muted', glyph: '☾', words: 'retired' },
  unverified: { tone: 'alarm', glyph: '✕', words: 'witness unverified: does not load' },
  absent: { tone: 'muted', glyph: '·', words: 'named, but not in the world' },
};

const ORDER: Record<OrderState, { tone: Tone; glyph: string }> = {
  held: { tone: 'gold', glyph: '◇' },
  claimed: { tone: 'holo', glyph: '◆' },
  done: { tone: 'muted', glyph: '✓' },
  dropped: { tone: 'muted', glyph: '✕' },
};

/** Only the art the law names by its exact, plain path: `/art/cards/<race>.webp`. */
export function artOf(r: RaceLaw | undefined): string | undefined {
  return r && /^\/art\/cards\/[a-z-]{1,32}\.webp$/.test(r.art) ? r.art : undefined;
}

/** Where a part was read, and whether that source grants anything. */
export function Source({ what, p }: { what: string; p: Provenance }) {
  return (
    <p className="panel-source">
      {p.trusted ? (
        <Mark tone="holo" glyph="◆">
          {what}: trusted
        </Mark>
      ) : (
        <Mark tone="muted" glyph="◇">
          {what}: in-world, shown only
        </Mark>
      )}{' '}
      <span className="mono">
        <Safe text={p.source} />
      </span>
      {p.why && (
        <span className="panel-note">
          {' '}
          · <Safe text={p.why} />
        </span>
      )}
    </p>
  );
}

function Failed<T>({ load, what }: { load: KnowledgeLoad<T>; what: string }) {
  if (load.state === 'loading') {
    return (
      <Empty glyph="⣾" title={`Loading the ${what}…`}>
        Asking the board server.
      </Empty>
    );
  }
  if (load.state === 'failed') {
    return (
      <p className="hint" role="status">
        <Mark tone="alarm" glyph="✕">
          The {what} could not be read: <Safe text={load.reason} />
        </Mark>
        {load.data && ' What was read before is shown below.'}
      </p>
    );
  }
  return null;
}

function Unreadable({ n }: { n: number }) {
  if (n === 0) return null;
  return (
    <p className="hint">
      <Mark tone="gold" glyph="⚠">
        {n} row(s) of the answer did not read and are not shown
      </Mark>
    </p>
  );
}

/** One act: a commission or a verdict. */
export function Act({ e }: { e: TimelineEntry }) {
  return (
    <li className="act" data-kind={e.kind}>
      <span className="act-when mono">
        <Safe text={e.at} />
      </span>
      {e.kind === 'verdict' ? (
        <Mark tone={verdictTone(e.verdict ?? '')} glyph="◆">
          verdict #{e.n} <Safe text={e.verdict ?? ''} />
        </Mark>
      ) : (
        <Mark tone={e.state === 'open' ? 'gold' : 'muted'} glyph="↳">
          commission {e.state === 'open' ? '(no end yet)' : (e.ending ?? '')}
        </Mark>
      )}{' '}
      <span className="act-who">
        <Safe text={e.body} />
      </span>
      {e.ask && (
        <span className="act-ask">
          {' '}
          “<Safe text={e.ask} />”
        </span>
      )}
      {e.kind === 'commission' && e.verdict && (
        <span className="panel-note">
          {' '}
          · its gate <Safe text={e.verdict} />
        </span>
      )}
      {e.changed > 0 && <span className="panel-note"> · {e.changed} changed</span>}
      {e.failures.length > 0 && (
        <span className="panel-note">
          {' '}
          · failed: <Safe text={e.failures.join(', ')} />
        </span>
      )}
      {e.why && (
        <span className="panel-note">
          {' '}
          · <Safe text={e.why} />
        </span>
      )}
      <span className="act-source">
        {e.trusted ? 'trusted' : 'in-world'} · <Safe text={e.source} />
      </span>
    </li>
  );
}

function Law({ r }: { r: RaceLaw }) {
  const bar = (b: RaceLaw['bars'][number]) =>
    `${b.kind}: ${b.meter} ${b.op} ${b.scale === 'rate' ? `${b.threshold}%` : b.threshold}`;
  return (
    <dl className="law">
      <dt>reports to</dt>
      <dd>{r.reports_to ?? 'the human (Veldora)'}</dd>
      <dt>commands</dt>
      <dd>
        {r.commands.length > 0 ? r.commands.join(', ') : 'nobody'}
        {r.sideways && ', and its own race sideways'}
      </dd>
      <dt>shelf</dt>
      <dd className="mono">{r.shelf.join(' · ')}</dd>
      <dt>soil</dt>
      <dd>{r.soil.join('; ')}</dd>
      <dt>gate</dt>
      <dd>{r.gate}</dd>
      <dt>budget</dt>
      <dd>
        {r.steps} steps · at most {r.max_share}% of its parent's · {r.tier} model
      </dd>
      <dt>meter</dt>
      <dd>
        {r.meter.label}: ok ≥ {r.meter.ok}%, warn ≥ {r.meter.warn}%
      </dd>
      {r.bars.length > 0 && (
        <>
          <dt>bars</dt>
          <dd>
            <ul className="law-bars mono">
              {r.bars.map((b) => (
                <li key={bar(b)}>{bar(b)}</li>
              ))}
            </ul>
          </dd>
        </>
      )}
      {r.ascends_to && (
        <>
          <dt>ascends to</dt>
          <dd>{r.ascends_to}</dd>
        </>
      )}
    </dl>
  );
}

function BodyCard({
  b,
  r,
  onClose,
}: {
  b: BoardBody;
  r: RaceLaw | undefined;
  onClose: () => void;
}) {
  const art = artOf(r);
  const s = STATE_WORDS[b.live.state];
  return (
    <section className="panel body-card" aria-label={`body card: ${b.name}`}>
      <header className="body-card-head">
        <h2 className="body-card-name">
          <Safe text={b.name} />
        </h2>
        <button type="button" className="button button-small" onClick={onClose}>
          Close
        </button>
      </header>
      <div className="body-card-top">
        {art && (
          <img
            className="body-card-art"
            src={art}
            alt={`${r?.title ?? b.race} race card`}
            width={150}
            height={210}
          />
        )}
        <div className="body-card-facts">
          <p>
            <span className="mark race" data-race={b.race}>
              <span className="race-swatch" aria-hidden="true" />
              <span className="mark-label">{r?.title ?? b.race}</span>
            </span>
            {b.post && (
              <span>
                {' '}
                · post <Safe text={b.post} />{' '}
                <span className="panel-note">(by its doc's name, in-world)</span>
              </span>
            )}
            {b.speciality && (
              <span>
                {' '}
                · speciality <Safe text={b.speciality} />{' '}
                <span className="panel-note">(by its doc's name, in-world)</span>
              </span>
            )}
          </p>
          <p>
            <Mark tone={STATE_TONE[b.live.state]} glyph={s.glyph}>
              {s.words}
            </Mark>{' '}
            <span className="panel-note">
              <Safe text={b.live.why} />
            </span>
          </p>
          <p>
            reports to{' '}
            {b.parent ? <Safe text={b.parent} /> : <span className="panel-note">no one named</span>}
            {b.heir_of && (
              <>
                {' '}
                · heir of <Safe text={b.heir_of} />
              </>
            )}
          </p>
          {b.ground.length > 0 && (
            <ul className="body-ground mono">
              {b.ground.map((g) => (
                <li key={`${g.kind}:${g.path}`}>
                  {g.kind} <Safe text={g.path} />
                </li>
              ))}
            </ul>
          )}
          <p className="panel-note">
            {b.verify} verify line(s) · doc <Safe text={b.doc} />
          </p>
        </div>
      </div>
      {r && (
        <details className="body-law" open>
          <summary>the law of the {r.title}</summary>
          <Law r={r} />
        </details>
      )}
      <h3 className="panel-title">Minds</h3>
      <p className="panel-note">
        Standing from the witness; each Mind's line and shared mark are its doc's, in-world.
      </p>
      {b.minds.length === 0 ? (
        <p className="panel-note">It wears no Mind.</p>
      ) : (
        <ul className="body-minds">
          {b.minds.map((m) => {
            const d = MIND[m.standing];
            return (
              <li key={m.name}>
                <Safe text={m.name} />
                {m.shared && <span className="panel-note"> (shared)</span>}{' '}
                <Mark tone={d.tone} glyph={d.glyph}>
                  {d.words}
                </Mark>
                {m.about && (
                  <span className="panel-note">
                    {' '}
                    · <Safe text={m.about} />
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
      <h3 className="panel-title">Recent acts</h3>
      {b.acts.length === 0 ? (
        <p className="panel-note">No commission or verdict names it yet.</p>
      ) : (
        <ul className="acts">
          {b.acts.map((e) => (
            <Act key={`${e.kind}:${e.run ?? e.n}:${e.at}`} e={e} />
          ))}
        </ul>
      )}
    </section>
  );
}

function ColonyTab({ p }: { p: BodiesPage }) {
  const [picked, setPicked] = useState<string>();
  const c = useMemo(() => colony(p.bodies, p.races, p.throne), [p]);
  const body = p.bodies.find((b) => b.name === picked);
  const counts = new Map<BodyState, number>();
  for (const b of p.bodies) counts.set(b.live.state, (counts.get(b.live.state) ?? 0) + 1);
  return (
    <>
      <p className="knowledge-strip mono">
        <span>{p.bodies.length} bodies</span>
        {[...counts].map(([s, n]) => (
          <span key={s}>
            {' '}
            ·{' '}
            <Mark tone={STATE_TONE[s]} glyph={STATE_WORDS[s].glyph}>
              {STATE_WORDS[s].words} {n}
            </Mark>
          </span>
        ))}{' '}
        ·{' '}
        <Mark tone={p.throne.live ? 'holo' : 'muted'} glyph={p.throne.live ? '●' : '○'}>
          throne {p.throne.live ? 'live' : 'idle'}
        </Mark>
      </p>
      {p.bodies.length === 0 ? (
        <Empty glyph="⛬" title="No body here yet">
          No creature doc under <code className="mono">.mnemorph/&lt;race&gt;/</code>.
        </Empty>
      ) : (
        <div className={body ? 'colony-grid colony-grid-open' : 'colony-grid'}>
          <section className="panel colony-panel" aria-label="colony net">
            <ColonyNet c={c} selected={picked} onPick={setPicked} />
            <p className="panel-note">Pick a body to open its card.</p>
          </section>
          {body && (
            <BodyCard
              b={body}
              r={p.races.find((r) => r.race === body.race)}
              onClose={() => setPicked(undefined)}
            />
          )}
        </div>
      )}
      {(c.dropped.length > 0 || p.loops.length > 0 || p.problems.length > 0) && (
        <section className="panel knowledge-block" aria-label="roster problems">
          <h2 className="panel-title">Not drawn, and why</h2>
          <ul className="knowledge-list">
            {c.dropped.map((d) => (
              <li key={`${d.from}>${d.to}`}>
                <Mark tone="gold" glyph="⚠">
                  {d.why === 'loop' ? 'loop' : 'unknown commander'}
                </Mark>{' '}
                <Safe text={d.to} /> reports to <Safe text={d.from} />
                {d.why === 'loop'
                  ? ': the chain would come back to itself'
                  : ': no body has that name'}
              </li>
            ))}
            {p.problems.map((x) => (
              <li key={x}>
                <Mark tone="gold" glyph="⚠">
                  doc
                </Mark>{' '}
                <Safe text={x} />
              </li>
            ))}
          </ul>
        </section>
      )}
      <Source what="roster" p={p.roster} />
      <Source what="Minds" p={p.minds} />
      <Source what="live states" p={p.live} />
      <Source what="acts" p={p.acts} />
    </>
  );
}

function RacesTab({ p }: { p: BodiesPage }) {
  return (
    <>
      <p className="panel-note">
        Every race of the law, born in this world or not: what it may do, where it may write, who
        judges it, and how it rises and falls.
      </p>
      <ul className="race-cards">
        {p.races.map((r) => {
          const art = artOf(r);
          return (
            <li key={r.race} className="panel race-card" data-born={r.living > 0}>
              {art && (
                <img
                  className="race-card-art"
                  src={art}
                  alt={`${r.title} race card`}
                  width={180}
                  height={252}
                  loading="lazy"
                />
              )}
              <h2 className="race-card-title">
                <span className="mark race" data-race={r.race}>
                  <span className="race-swatch" aria-hidden="true" />
                  <span className="mark-label">{r.title}</span>
                </span>
              </h2>
              <p className="panel-note">
                {r.ascended
                  ? `ascended ${r.base}`
                  : r.race === 'rimuru'
                    ? 'the throne'
                    : 'base race'}{' '}
                · {r.living > 0 ? `${r.living} living here` : 'unborn here'}
              </p>
              <Law r={r} />
              {r.posts.length > 0 && (
                <details>
                  <summary>posts · {r.posts.length}</summary>
                  <ul className="law-bars mono">
                    {r.posts.map((x) => (
                      <li key={x.creature}>
                        {x.creature}: {x.acts.join(', ')}
                      </li>
                    ))}
                  </ul>
                </details>
              )}
              {r.specialities.length > 0 && (
                <details>
                  <summary>specialities · {r.specialities.length}</summary>
                  <ul className="law-bars mono">
                    {r.specialities.map((x) => (
                      <li key={x.creature}>
                        {x.creature}: {x.classes.length > 0 ? x.classes.join(', ') : 'chains'}
                      </li>
                    ))}
                  </ul>
                </details>
              )}
            </li>
          );
        })}
      </ul>
      <Source what="race law" p={p.law} />
    </>
  );
}

function Order({ o }: { o: BoardOrder }) {
  const d = ORDER[o.state];
  return (
    <li className="order" data-state={o.state}>
      <div className="order-head">
        <Mark tone={d.tone} glyph={d.glyph}>
          {o.state}
        </Mark>
        <code className="mono">
          <Safe text={o.id} />
        </code>
        <span>
          for{' '}
          <strong>
            <Safe text={o.holder} />
          </strong>
          {o.throne && <span className="panel-note"> (the throne)</span>}
        </span>
        <span className="panel-note">
          by <Safe text={o.by} /> · rank {o.rank} · rev {o.rev}
        </span>
        {o.witnessed === true && (
          <Mark tone="holo" glyph="✓">
            witnessed
          </Mark>
        )}
        {o.witnessed === false && (
          <Mark tone="alarm" glyph="⚠">
            not witnessed: no harness or human wrote it
          </Mark>
        )}
      </div>
      <p className="order-ask">
        <Safe text={o.ask} lines />
      </p>
      {o.notes.length > 0 && (
        <details>
          <summary>notes · {o.notes.length}</summary>
          <ol className="order-notes">
            {o.notes.map((n, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: notes only grow
              <li key={i}>
                <Safe text={n} />
              </li>
            ))}
          </ol>
        </details>
      )}
    </li>
  );
}

function QueueTab({ w }: { w: BoardWorld }) {
  const load = useQueue(w.id);
  const [only, setOnly] = useState<'open' | 'closed' | undefined>();
  const q = load.state === 'loading' ? undefined : load.data;
  const shown = q?.orders.filter((o) =>
    only === undefined
      ? true
      : only === 'open'
        ? o.state === 'held' || o.state === 'claimed'
        : o.state === 'done' || o.state === 'dropped',
  );
  return (
    <>
      {q && (
        <p className="knowledge-strip mono">
          {ORDER_STATES.map((s, i) => (
            <span key={s}>
              {i > 0 && ' · '}
              <Mark tone={ORDER[s].tone} glyph={ORDER[s].glyph}>
                {s} {q.counts[s]}
              </Mark>
            </span>
          ))}
        </p>
      )}
      <fieldset className="filters">
        <legend className="visually-hidden">Filter the orders</legend>
        <label className="filter">
          <span className="filter-label">orders</span>
          <select
            value={only ?? ''}
            onChange={(e) =>
              setOnly(e.target.value === '' ? undefined : (e.target.value as 'open' | 'closed'))
            }
          >
            <option value="">all</option>
            <option value="open">open: held, claimed</option>
            <option value="closed">closed: done, dropped</option>
          </select>
        </label>
      </fieldset>
      <Failed load={load} what="queue" />
      {q && <Unreadable n={q.unreadable} />}
      {shown &&
        (shown.length === 0 ? (
          <Empty glyph="◇" title="No order here">
            {q?.orders.length === 0
              ? 'The queue holds nothing. Add one at a terminal: mnemorphd q add <ask> --for <body>.'
              : 'No order matches the filter.'}
          </Empty>
        ) : (
          <ul className="orders">
            {shown.map((o) => (
              <Order key={o.id} o={o} />
            ))}
          </ul>
        ))}
      {q && (
        <>
          <Source
            what="queue"
            p={{
              source: q.source,
              trusted: false,
              why: 'folded as mnemorphd q list --all folds it',
            }}
          />
          <Source
            what="witness"
            p={{
              source: q.witness ?? 'no witness',
              trusted: q.witness !== null,
              why: q.witness_why,
            }}
          />
        </>
      )}
    </>
  );
}

function TimelineTab({ w }: { w: BoardWorld }) {
  const [q, setQ] = useState<TimelineQuery>({});
  const load = useTimeline(w.id, q);
  const t = load.state === 'loading' ? undefined : load.data;
  const set = (p: Partial<TimelineQuery>) => setQ((was) => ({ ...was, ...p, offset: 0 }));
  return (
    <>
      {t && (
        <p className="knowledge-strip mono">
          <span>commissions {t.commissions}</span> · <span>verdicts {t.verdicts}</span>
          {t.older > 0 && <span> · {t.older} older journal(s) not read</span>}
        </p>
      )}
      <fieldset className="filters">
        <legend className="visually-hidden">Filter the timeline</legend>
        <label className="filter">
          <span className="filter-label">kind</span>
          <select
            value={q.kind ?? ''}
            onChange={(e) =>
              set({ kind: e.target.value === '' ? undefined : (e.target.value as TimelineKind) })
            }
          >
            <option value="">all</option>
            <option value="commission">commissions</option>
            <option value="verdict">verdicts</option>
          </select>
        </label>
        <label className="filter">
          <span className="filter-label">body</span>
          <select
            value={q.body ?? ''}
            onChange={(e) => set({ body: e.target.value === '' ? undefined : e.target.value })}
          >
            <option value="">all</option>
            {(t?.bodies ?? []).map((b) => (
              <option key={b} value={b}>
                {b}
              </option>
            ))}
          </select>
        </label>
      </fieldset>
      <Failed load={load} what="timeline" />
      {t && <Unreadable n={t.unreadable} />}
      {t?.verdicts_why && (
        <p className="hint">
          <Mark tone="gold" glyph="⚠">
            <Safe text={t.verdicts_why} />
          </Mark>
        </p>
      )}
      {t &&
        (t.entries.length === 0 ? (
          <Empty glyph="↳" title="Nothing on the timeline">
            {t.commissions + t.verdicts === 0
              ? 'No body has been commissioned and the gate has given no verdict yet.'
              : 'Nothing matches the filter.'}
          </Empty>
        ) : (
          <>
            <ul className="acts timeline">
              {t.entries.map((e) => (
                <Act key={`${e.kind}:${e.run ?? e.n}:${e.at}`} e={e} />
              ))}
            </ul>
            <Pager
              offset={t.offset}
              limit={t.limit}
              total={t.total}
              onMove={(offset) => setQ((was) => ({ ...was, offset }))}
            />
          </>
        ))}
    </>
  );
}

export function BodiesView({ w, tab = 'colony' }: { w: BoardWorld; tab?: BodiesTab | undefined }) {
  const load = useBodies(w.id);
  const p = load.state === 'loading' ? undefined : load.data;
  return (
    <div className="page page-wide">
      <header className="page-head">
        <h1 className="page-title">
          Bodies &amp; queue
          <span className="page-crumb">
            <Safe text={w.name} />
          </span>
        </h1>
      </header>
      <nav className="tabs" aria-label="bodies and queue">
        {BODIES_TABS.map((t) => (
          <a
            key={t}
            className="tab"
            href={format({ name: 'section', world: w.id, section: 'bodies', tab: t })}
            aria-current={tab === t ? 'page' : undefined}
          >
            {TAB_WORDS[t]}
          </a>
        ))}
      </nav>
      {(tab === 'colony' || tab === 'races') && <Failed load={load} what="colony" />}
      {(tab === 'colony' || tab === 'races') && p && <Unreadable n={p.unreadable} />}
      {tab === 'colony' && p && <ColonyTab p={p} />}
      {tab === 'races' && p && <RacesTab p={p} />}
      {tab === 'queue' && <QueueTab w={w} />}
      {tab === 'timeline' && <TimelineTab w={w} />}
    </div>
  );
}
