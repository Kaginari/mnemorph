/**
 * A world's ontology: its facts by status — the facts table's partition, one tab each — with a
 * contested fact's supporters and counters side by side, each with its anchor or its reason;
 * what `onto check` finds; who owns each path a fact is about. **Settle** asks for a proof: a
 * passing gate verdict that landed on the fact's path, or the human's own statement. Every word a
 * fact holds is a body's: shown through `<Safe>`, granting nothing.
 */
import type {
  BoardFact,
  BoardFactStatus,
  BoardWorld,
  FactAnchor,
  FactSide,
  ProofVerdict,
} from '@mnemorph/protocol';
import { useState } from 'react';
import {
  FACT_STATUSES,
  type FactsPage,
  type SettleProof,
  settleFact,
  useFacts,
} from '../data/knowledge.ts';
import { Safe } from '../safe/Safe.tsx';
import { shown } from '../safe/shown.ts';
import { Confirm } from '../ui/Confirm.tsx';
import { Empty } from '../ui/Empty.tsx';
import { Mark, type Tone } from '../ui/Mark.tsx';
import { Pager } from './Memory.tsx';
import './knowledge.css';

/** How each status is drawn: a glyph, a tone, its words and what it means. */
export const FACT_STATUS: Record<
  BoardFactStatus,
  { glyph: string; tone: Tone; label: string; means: string }
> = {
  contested: {
    glyph: '⚔',
    tone: 'alarm',
    label: 'contested',
    means: 'support and counters, neither side carrying it: a check settles it, never recency',
  },
  single: { glyph: '○', tone: 'muted', label: 'single', means: 'one voice' },
  agreed: {
    glyph: '◆',
    tone: 'holo',
    label: 'agreed',
    means: 'named twice, one dissenter in three tolerated',
  },
  settled: {
    glyph: '✓',
    tone: 'holo',
    label: 'settled',
    means: 'a proof settled it: final until a newer counter',
  },
  countered: { glyph: '✕', tone: 'muted', label: 'countered', means: 'the counters carry it' },
};

const SEVERITY: Record<string, Tone> = {
  critical: 'alarm',
  high: 'alarm',
  medium: 'gold',
  low: 'muted',
};

function Anchor({ a }: { a: FactAnchor | null }) {
  if (!a) return <span className="panel-note">no anchor</span>;
  return (
    <span className="fact-anchor">
      <code className="mono">
        <Safe text={a.at} />
      </code>
      {a.stale && (
        <>
          {' '}
          <Mark tone="gold" glyph="⚠">
            moved since
          </Mark>
        </>
      )}
    </span>
  );
}

function Side({ title, sides, empty }: { title: string; sides: FactSide[]; empty: string }) {
  return (
    <div className="fact-side">
      <h3 className="fact-side-title">
        {title} · {sides.length}
      </h3>
      {sides.length === 0 ? (
        <p className="panel-note">{empty}</p>
      ) : (
        <ul className="knowledge-list">
          {sides.map((s) => (
            <li key={s.creature}>
              <strong>
                <Safe text={s.creature} />
              </strong>{' '}
              {s.reason !== null ? (
                <>
                  — <Safe text={s.reason} />
                </>
              ) : (
                <Anchor a={s.anchor} />
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** A path as a landing is matched against it; `undefined` for one naming nothing a landing changes. */
export function normalPath(path: string): string | undefined {
  const parts = path.split('/').filter((p) => p !== '' && p !== '.');
  if (parts.length === 0 || parts.includes('..') || path.startsWith('/')) return undefined;
  return parts.join('/');
}

/** Whether `v` landed on the path `about`: the rule the server holds a verdict to. */
export function landedOn(v: ProofVerdict, about: string): boolean {
  const a = normalPath(about);
  if (a === undefined) return false;
  return v.changed.some((c) => {
    const p = normalPath(c);
    return p !== undefined && (p === a || p.startsWith(`${a}/`));
  });
}

function FactRow({ f, onSettle }: { f: BoardFact; onSettle: (f: BoardFact) => void }) {
  const s = FACT_STATUS[f.status];
  const sided = f.status === 'contested' || f.counters.length > 0;
  return (
    <li className="fact" data-status={f.status}>
      <div className="note-head">
        <Mark tone={s.tone} glyph={s.glyph}>
          {s.label}
        </Mark>
        <code className="mono note-id">
          #<Safe text={f.id} />
        </code>
        <code className="mono fact-about">
          <Safe text={f.about} />
        </code>
        <span className="panel-note">
          owner <Safe text={f.owner.owner} />
        </span>
        <span className="note-acts">
          {f.status !== 'settled' && (
            <button type="button" className="button button-small" onClick={() => onSettle(f)}>
              Settle…
            </button>
          )}
        </span>
      </div>
      <p className="note-text">
        “<Safe text={f.text} lines />”
      </p>
      {sided ? (
        <div className="fact-sides">
          <Side title="Supporters" sides={f.supporters} empty="no supporter left" />
          <Side title="Counters" sides={f.counters} empty="no counter" />
        </div>
      ) : (
        <p className="note-proof">
          <span className="note-label">said by</span>{' '}
          {f.supporters.map((x, i) => (
            <span key={x.creature}>
              {i > 0 && ', '}
              <Safe text={x.creature} />
            </span>
          ))}{' '}
          · <Anchor a={f.anchor} />
        </p>
      )}
      {f.settled && (
        <p className="note-proof">
          <span className="note-label">settled by</span> <Safe text={f.settled} />
        </p>
      )}
    </li>
  );
}

/** The settle dialog: it names the fact and asks for a proof; with none, Settle stays disabled. */
function SettleDialog({
  world,
  f,
  verdicts,
  onDone,
  onClose,
}: {
  world: BoardWorld;
  f: BoardFact;
  verdicts: ProofVerdict[];
  onDone: (said: string) => void;
  onClose: () => void;
}) {
  const landed = verdicts.filter((v) => landedOn(v, f.about));
  const [kind, setKind] = useState<'verdict' | 'statement'>(
    landed.length > 0 ? 'verdict' : 'statement',
  );
  const [verdict, setVerdict] = useState<number | undefined>(landed[0]?.n);
  const [statement, setStatement] = useState('');
  const proof: SettleProof | undefined =
    kind === 'verdict'
      ? verdict === undefined
        ? undefined
        : { verdict }
      : statement.trim() === ''
        ? undefined
        : { statement: statement.trim() };
  return (
    <Confirm
      title="Settle this fact"
      verb="Settle"
      ready={proof !== undefined}
      onClose={onClose}
      onAct={async () => {
        if (!proof) return 'a settle needs a proof';
        const out = await settleFact(world.id, f, proof);
        if (!out.ok) return out.why;
        onDone(out.done);
        return undefined;
      }}
    >
      <dl className="confirm-what">
        <dt>fact</dt>
        <dd className="mono">
          #<Safe text={f.id} />
        </dd>
        <dt>about</dt>
        <dd className="mono">
          <Safe text={f.about} />
        </dd>
        <dt>says</dt>
        <dd className="confirm-quote">
          <Safe text={f.text} lines />
        </dd>
        <dt>now</dt>
        <dd>
          {FACT_STATUS[f.status].label} · {f.supporters.length} for, {f.counters.length} against
        </dd>
      </dl>
      <fieldset className="proof">
        <legend>The proof</legend>
        <label className="radio">
          <input
            type="radio"
            name="proof"
            checked={kind === 'verdict'}
            disabled={landed.length === 0}
            onChange={() => setKind('verdict')}
          />
          <span>
            A passing gate verdict that landed on{' '}
            <code className="mono">
              <Safe text={f.about} />
            </code>
            {landed.length === 0 && <span className="panel-note"> (none yet)</span>}
          </span>
        </label>
        {kind === 'verdict' && landed.length > 0 && (
          <select
            aria-label="the verdict"
            value={verdict ?? ''}
            onChange={(e) => setVerdict(Number(e.target.value))}
          >
            {landed.map((v) => (
              <option key={v.n} value={v.n}>
                {shown(`verdict ${v.n} · PASS on ${v.body}'s landing · ${v.at}`)}
              </option>
            ))}
          </select>
        )}
        <label className="radio">
          <input
            type="radio"
            name="proof"
            checked={kind === 'statement'}
            onChange={() => setKind('statement')}
          />
          My own statement
        </label>
        {kind === 'statement' && (
          <textarea
            aria-label="your statement"
            className="proof-text"
            maxLength={1000}
            rows={3}
            placeholder="What you checked, and what it showed"
            value={statement}
            onChange={(e) => setStatement(e.target.value)}
          />
        )}
      </fieldset>
      <p className="panel-note">
        Filed <code className="mono">by: human (board)</code> with its proof, as{' '}
        <code className="mono">mnemorphd onto settle</code> files yours. The server checks again
        that this fact still says exactly this.
      </p>
    </Confirm>
  );
}

function Findings({ p }: { p: FactsPage }) {
  return (
    <section className="panel knowledge-block" aria-label="onto check">
      <h2 className="panel-title">
        onto check ·{' '}
        <Mark tone={p.failed ? 'alarm' : 'holo'} glyph={p.failed ? '✕' : '✓'}>
          {p.failed ? 'FAIL' : 'PASS'}
        </Mark>{' '}
        · {p.found} {p.found === 1 ? 'finding' : 'findings'}
      </h2>
      {!p.git && (
        <p className="panel-note">The world is no git repository: unowned soil is not checked.</p>
      )}
      {p.problems.length > 0 && (
        <ul className="knowledge-list">
          {p.problems.map((x) => (
            <li key={x}>
              <Mark tone="alarm" glyph="✕">
                roster
              </Mark>{' '}
              <Safe text={x} />
            </li>
          ))}
        </ul>
      )}
      {p.findings.length === 0 ? (
        <p className="panel-note">No finding.</p>
      ) : (
        <ul className="knowledge-list findings">
          {p.findings.map((f, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: findings never reorder within an answer
            <li key={i}>
              <Mark tone={SEVERITY[f.severity] ?? 'muted'} glyph="◆">
                <Safe text={f.severity} />
              </Mark>{' '}
              <code className="mono">
                <Safe text={f.code} />
              </code>{' '}
              <code className="mono panel-note">
                <Safe text={f.at} />
              </code>
              <br />
              <Safe text={f.message} />
            </li>
          ))}
          {p.found > p.findings.length && (
            <li className="panel-note">{p.found - p.findings.length} more not listed</li>
          )}
        </ul>
      )}
    </section>
  );
}

function Owners({ p }: { p: FactsPage }) {
  if (p.owners.length === 0) return null;
  return (
    <section className="panel knowledge-block" aria-label="path owners">
      <h2 className="panel-title">Path owners</h2>
      <dl className="panel-rows">
        {p.owners.map((o) => (
          <div key={o.path} className="panel-row">
            <dt className="mono">
              <Safe text={o.path} />
            </dt>
            <dd>
              <Safe text={o.owner} />{' '}
              <span className="panel-note">
                (<Safe text={o.why} />)
              </span>
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

export function OntologyView({ w }: { w: BoardWorld }) {
  const [status, setStatus] = useState<BoardFactStatus>('contested');
  const [offset, setOffset] = useState(0);
  const [again, setAgain] = useState(0);
  const [settling, setSettling] = useState<BoardFact>();
  const [said, setSaid] = useState<string>();
  const load = useFacts(w.id, status, offset, again);
  const p = load.state === 'loading' ? undefined : load.data;
  return (
    <div className="page page-wide">
      <header className="page-head">
        <h1 className="page-title">
          Ontology
          <span className="page-crumb">
            <Safe text={w.name} />
          </span>
        </h1>
      </header>
      <div className="tabs" role="tablist" aria-label="fact statuses">
        {FACT_STATUSES.map((s) => {
          const d = FACT_STATUS[s];
          return (
            <button
              key={s}
              type="button"
              role="tab"
              className="tab"
              aria-selected={status === s}
              onClick={() => {
                setStatus(s);
                setOffset(0);
              }}
            >
              <Mark tone={d.tone} glyph={d.glyph}>
                {d.label} {p ? p.counts[s] : ''}
              </Mark>
            </button>
          );
        })}
      </div>
      <p className="panel-note tab-means">{FACT_STATUS[status].means}</p>
      {said && (
        <p className="hint" role="status">
          <Mark tone="holo" glyph="✓">
            <Safe text={said} />
          </Mark>
        </p>
      )}
      {load.state === 'loading' && (
        <Empty glyph="⣾" title="Reading the ontology…">
          Asking the board server what this world holds as fact.
        </Empty>
      )}
      {load.state === 'failed' && (
        <p className="hint" role="status">
          <Mark tone="alarm" glyph="✕">
            The ontology could not be read: <Safe text={load.reason} />
          </Mark>
          {load.data && ' What was read before is shown below.'}
        </p>
      )}
      {p && p.unreadable > 0 && (
        <p className="hint">
          <Mark tone="gold" glyph="⚠">
            {p.unreadable} row(s) of the answer did not read and are not shown
          </Mark>
        </p>
      )}
      {p && (
        <div className="knowledge-grid">
          <div role="tabpanel" aria-label={`${status} facts`}>
            {p.facts.length === 0 ? (
              <Empty glyph="◇" title={`No ${status} fact`}>
                {FACT_STATUS[status].means}.
              </Empty>
            ) : (
              <>
                <ul className="notes">
                  {p.facts.map((f) => (
                    <FactRow key={f.id} f={f} onSettle={setSettling} />
                  ))}
                </ul>
                <Pager offset={p.offset} limit={p.limit} total={p.total} onMove={setOffset} />
              </>
            )}
          </div>
          <aside className="knowledge-side">
            <Findings p={p} />
            <Owners p={p} />
          </aside>
        </div>
      )}
      {p && (
        <p className="panel-source">
          <Mark tone="muted" glyph="◇">
            in-world, shown only
          </Mark>{' '}
          <span className="mono">
            <Safe text={p.source} />
          </span>
        </p>
      )}
      {settling && p && (
        <SettleDialog
          world={w}
          f={settling}
          verdicts={p.verdicts}
          onClose={() => setSettling(undefined)}
          onDone={(done) => {
            setSaid(done);
            setAgain((a) => a + 1);
          }}
        />
      )}
    </div>
  );
}
