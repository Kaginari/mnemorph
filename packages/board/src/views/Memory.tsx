/**
 * A world's memory: every note with its route and proof, its place in the claim graph, the
 * notes the gate retired and the events that freeze a creature's writes — filtered by creature,
 * scope, standing and review. The human's two routes, confirm and wake, are behind a dialog that
 * names the note. Every word a note holds is a body's: shown through `<Safe>`, granting nothing.
 */
import type { BoardWorld, MemoryNote, NoteScope, NoteStanding } from '@mnemorph/protocol';
import { useState } from 'react';
import {
  confirmNote,
  type MemoryPage,
  type MemoryQuery,
  NOTE_SCOPES,
  useMemory,
  wakeNote,
} from '../data/knowledge.ts';
import { Safe } from '../safe/Safe.tsx';
import { Confirm } from '../ui/Confirm.tsx';
import { Empty } from '../ui/Empty.tsx';
import { Mark, type Tone } from '../ui/Mark.tsx';
import './knowledge.css';

/** How each standing is drawn: a glyph, a tone and its words. */
export const STANDING: Record<NoteStanding, { glyph: string; tone: Tone; label: string }> = {
  proposed: { glyph: '○', tone: 'gold', label: 'proposed' },
  grounded: { glyph: '✓', tone: 'holo', label: 'grounded' },
  retired: { glyph: '☾', tone: 'muted', label: 'retired' },
  dormant: { glyph: '☾', tone: 'muted', label: 'dormant' },
  tombstoned: { glyph: '✕', tone: 'muted', label: 'tombstoned' },
};

const STANDINGS: readonly NoteStanding[] = ['proposed', 'grounded', 'retired'];

/** What a note's standing rests on, in words. */
function rests(n: MemoryNote): string {
  if (n.standing === 'proposed')
    return 'awaits a route: checked, accepted, corroborated or confirmed';
  if (n.standing === 'retired') return 'the gate retired it: kept dormant for you to review';
  if (n.route) return `${n.route} ← ${n.proof?.slice(n.route.length + 2) ?? ''}`;
  return n.proof ?? 'no proof cited';
}

function NoteRow({
  n,
  onConfirm,
  onWake,
}: {
  n: MemoryNote;
  onConfirm: (n: MemoryNote) => void;
  onWake: (n: MemoryNote) => void;
}) {
  const s = STANDING[n.standing];
  const scope = n.scope === 'shared' ? 'shared' : 'personal';
  return (
    <li className="note" data-standing={n.standing}>
      <div className="note-head">
        <Mark tone={s.tone} glyph={s.glyph}>
          {s.label}
        </Mark>
        <span className="note-scope">{scope}</span>
        <span className="note-who">
          <Safe text={n.creature} />
        </span>
        <code className="mono note-id">
          <Safe text={n.id} />
        </code>
        {n.version > 1 && <span className="note-version">v{n.version}</span>}
        <span className="note-acts">
          {n.confirm && (
            <button type="button" className="button button-small" onClick={() => onConfirm(n)}>
              Confirm
            </button>
          )}
          {n.wake && (
            <button type="button" className="button button-small" onClick={() => onWake(n)}>
              Wake
            </button>
          )}
        </span>
      </div>
      <p className="note-text">
        “<Safe text={n.text} lines />”{n.cut && <span className="panel-note"> (cut)</span>}
      </p>
      <p className="note-proof">
        <span className="note-label">{n.route ? 'route' : 'standing'}</span>{' '}
        <Safe text={rests(n)} />
        {n.groundings.length > 1 && (
          <span className="panel-note"> · grounded {n.groundings.length}×</span>
        )}
      </p>
      {n.review && (
        <p className="note-review">
          <Mark tone="gold" glyph="⚠">
            needs review: <Safe text={n.review} />
          </Mark>
        </p>
      )}
      {(n.depends.length > 0 || n.versions.length > 0) && (
        <details className="note-graph">
          <summary>
            claim graph · {n.depends.length}{' '}
            {n.depends.length === 1 ? 'dependency' : 'dependencies'} · {n.versions.length} earlier{' '}
            {n.versions.length === 1 ? 'version' : 'versions'}
          </summary>
          {n.depends.length > 0 && (
            <ul className="note-deps mono">
              {n.depends.map((d) => (
                <li key={`${d.on}:${d.what}`}>
                  rests on {d.on} <Safe text={d.what} />
                </li>
              ))}
            </ul>
          )}
          {n.versions.length > 0 && (
            <ol className="note-versions">
              {n.versions.map((v) => (
                <li key={v.n}>
                  v{v.n}: “<Safe text={v.text} />”
                </li>
              ))}
            </ol>
          )}
        </details>
      )}
    </li>
  );
}

function Standing({ m }: { m: MemoryPage }) {
  const s = m.standing;
  return (
    <p className="knowledge-strip mono">
      <span>grounded {s.grounded}</span> · <span>shared {s.shared}</span> ·{' '}
      <span>proposed {s.proposed}</span> · <span>retired {s.retired}</span>
      {s.unproven > 0 && (
        <>
          {' '}
          ·{' '}
          <Mark tone="gold" glyph="⚠">
            unproven {s.unproven}
          </Mark>
        </>
      )}
    </p>
  );
}

function Frozen({ m }: { m: MemoryPage }) {
  const frozen = m.creatures.filter((c) => c.unproven > 0 || c.pathologies.length > 0);
  if (frozen.length === 0) return null;
  return (
    <section className="panel knowledge-block" aria-label="unproven events">
      <h2 className="panel-title">Unproven events and pathologies</h2>
      <ul className="knowledge-list">
        {frozen.flatMap((c) => [
          ...c.events.map((e) => (
            <li key={`${c.creature}:${e.event}`}>
              <Mark tone="gold" glyph="⚠">
                unproven
              </Mark>{' '}
              <Safe text={c.creature} />{' '}
              <code className="mono">
                #<Safe text={e.event} />
              </code>{' '}
              on <Safe text={e.note} /> · <Safe text={e.why} />
              {c.frozen && <span className="panel-note"> · writes frozen</span>}
            </li>
          )),
          ...c.pathologies.map((p) => (
            <li key={`${c.creature}:${p}`}>
              <Mark tone="muted" glyph="◇">
                recall
              </Mark>{' '}
              <Safe text={c.creature} /> · <Safe text={p} />
            </li>
          )),
        ])}
      </ul>
    </section>
  );
}

/** The dialog for an act on `n`: it names the note exactly. */
function Act({
  world,
  n,
  kind,
  onDone,
  onClose,
}: {
  world: BoardWorld;
  n: MemoryNote;
  kind: 'confirm' | 'wake';
  onDone: (said: string) => void;
  onClose: () => void;
}) {
  const confirm = kind === 'confirm';
  return (
    <Confirm
      title={confirm ? 'Confirm this claim' : 'Wake this retired note'}
      verb={confirm ? 'Confirm' : 'Wake'}
      onClose={onClose}
      onAct={async () => {
        const out = await (confirm ? confirmNote(world.id, n) : wakeNote(world.id, n));
        if (!out.ok) return out.why;
        onDone(out.done);
        return undefined;
      }}
    >
      <p>
        {confirm ? (
          <>
            You vouch for{' '}
            <strong>
              <Safe text={n.creature} />
            </strong>
            's claim — the <em>confirmed</em> route. It is grounded and shared, so every body
            recalls it, citing your word from the board.
          </>
        ) : (
          <>
            The gate retired this note. Waking it grounds it again in every tier that holds it,
            cited as yours from the board.
          </>
        )}
      </p>
      <dl className="confirm-what">
        <dt>note</dt>
        <dd className="mono">
          <Safe text={n.id} />
          {n.version > 1 && ` (version ${n.version})`}
        </dd>
        <dt>of</dt>
        <dd>
          <Safe text={n.creature} />
        </dd>
        <dt>says</dt>
        <dd className="confirm-quote">
          <Safe text={n.text} lines />
          {n.cut && ' (cut)'}
        </dd>
      </dl>
      <p className="panel-note">
        The words are a body's: the server checks again that this note still says exactly this
        before it acts. As the CLI: <code className="mono">mnemorphd memory {kind} </code>
        <code className="mono">
          <Safe text={n.id} />
        </code>
      </p>
    </Confirm>
  );
}

function Select<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T | undefined;
  options: readonly (readonly [T, string])[];
  onChange: (v: T | undefined) => void;
}) {
  return (
    <label className="filter">
      <span className="filter-label">{label}</span>
      <select
        value={value ?? ''}
        onChange={(e) => onChange(e.target.value === '' ? undefined : (e.target.value as T))}
      >
        <option value="">all</option>
        {options.map(([v, words]) => (
          <option key={v} value={v}>
            {words}
          </option>
        ))}
      </select>
    </label>
  );
}

export function MemoryView({ w }: { w: BoardWorld }) {
  const [q, setQ] = useState<MemoryQuery>({});
  const [again, setAgain] = useState(0);
  const [acting, setActing] = useState<{ n: MemoryNote; kind: 'confirm' | 'wake' }>();
  const [said, setSaid] = useState<string>();
  const load = useMemory(w.id, q, again);
  const m = load.state === 'loading' ? undefined : load.data;
  const set = (p: Partial<MemoryQuery>) => setQ((was) => ({ ...was, ...p, offset: 0 }));
  const creatures = m ? [...new Set(m.creatures.map((c) => c.creature))] : [];
  return (
    <div className="page page-wide">
      <header className="page-head">
        <h1 className="page-title">
          Memory
          <span className="page-crumb">
            <Safe text={w.name} />
          </span>
        </h1>
      </header>
      {m && <Standing m={m} />}
      <fieldset className="filters">
        <legend className="visually-hidden">Filter the notes</legend>
        <Select
          label="creature"
          value={q.creature}
          options={creatures.map((c) => [c, c] as const)}
          onChange={(creature) => set({ creature })}
        />
        <Select<NoteScope>
          label="scope"
          value={q.scope}
          options={NOTE_SCOPES.map((s) => [s, s] as const)}
          onChange={(scope) => set({ scope })}
        />
        <Select<NoteStanding>
          label="standing"
          value={q.standing}
          options={STANDINGS.map((s) => [s, s] as const)}
          onChange={(standing) => set({ standing })}
        />
        <Select<'yes' | 'no'>
          label="review"
          value={q.review === undefined ? undefined : q.review ? 'yes' : 'no'}
          options={[
            ['yes', 'needs review'],
            ['no', 'not under review'],
          ]}
          onChange={(r) => set({ review: r === undefined ? undefined : r === 'yes' })}
        />
      </fieldset>
      {said && (
        <p className="hint" role="status">
          <Mark tone="holo" glyph="✓">
            <Safe text={said} />
          </Mark>
        </p>
      )}
      {load.state === 'loading' && (
        <Empty glyph="⣾" title="Loading the memory…">
          Asking the board server what this world remembers.
        </Empty>
      )}
      {load.state === 'failed' && (
        <p className="hint" role="status">
          <Mark tone="alarm" glyph="✕">
            The memory could not be read: <Safe text={load.reason} />
          </Mark>
          {load.data && ' What was read before is shown below.'}
        </p>
      )}
      {m && m.unreadable > 0 && (
        <p className="hint">
          <Mark tone="gold" glyph="⚠">
            {m.unreadable} row(s) of the answer did not read and are not shown
          </Mark>
        </p>
      )}
      {m && <Frozen m={m} />}
      {m &&
        (m.notes.length === 0 ? (
          <Empty glyph="✧" title="No note here">
            {m.total === 0 && Object.values(q).every((v) => v === undefined || v === 0)
              ? 'No creature has memory in this world yet.'
              : 'No note matches the filter.'}
          </Empty>
        ) : (
          <>
            <ul className="notes">
              {m.notes.map((n) => (
                <NoteRow
                  key={`${n.scope}:${n.id}`}
                  n={n}
                  onConfirm={(x) => setActing({ n: x, kind: 'confirm' })}
                  onWake={(x) => setActing({ n: x, kind: 'wake' })}
                />
              ))}
            </ul>
            <Pager
              offset={m.offset}
              limit={m.limit}
              total={m.total}
              onMove={(offset) => setQ((was) => ({ ...was, offset }))}
            />
          </>
        ))}
      {m && (
        <p className="panel-source">
          <Mark tone="muted" glyph="◇">
            in-world, shown only
          </Mark>{' '}
          <span className="mono">
            <Safe text={m.source} />
          </span>
        </p>
      )}
      {acting && (
        <Act
          world={w}
          n={acting.n}
          kind={acting.kind}
          onClose={() => setActing(undefined)}
          onDone={(done) => {
            setSaid(done);
            setAgain((a) => a + 1);
          }}
        />
      )}
    </div>
  );
}

/** Previous and next pages, with where this one stands. */
export function Pager({
  offset,
  limit,
  total,
  onMove,
}: {
  offset: number;
  limit: number;
  total: number;
  onMove: (offset: number) => void;
}) {
  if (total <= limit && offset === 0) return null;
  return (
    <nav className="pager" aria-label="pages">
      <button
        type="button"
        className="button button-small"
        disabled={offset === 0}
        onClick={() => onMove(Math.max(0, offset - limit))}
      >
        Previous
      </button>
      <span className="panel-note">
        {offset + 1}–{Math.min(offset + limit, total)} of {total}
      </span>
      <button
        type="button"
        className="button button-small"
        disabled={offset + limit >= total}
        onClick={() => onMove(offset + limit)}
      >
        Next
      </button>
    </nav>
  );
}
