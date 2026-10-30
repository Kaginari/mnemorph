/** The right pane: what the centre's subject holds at a glance. Folds away; its state is kept. */
import type { WorldSummary } from '@mnemorph/protocol';
import type { ReactNode } from 'react';
import type { Load } from '../data/worlds.ts';
import type { Route } from '../router.ts';
import { Safe } from '../safe/Safe.tsx';
import { ago } from '../time.ts';
import { Mark, STATUS } from './Mark.tsx';

function Block({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="pane-block">
      <h3 className="pane-title">▾ {title}</h3>
      {children}
    </section>
  );
}

function Row({ k, children }: { k: string; children: ReactNode }) {
  return (
    <div className="pane-row">
      <dt>{k}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function Counts({ load }: { load: Load }) {
  if (load.state !== 'ready') return <p className="pane-note">—</p>;
  const n = (s: WorldSummary['status']) => load.data.worlds.filter((w) => w.status === s).length;
  return (
    <dl className="pane-rows">
      {(['live', 'tui-live', 'idle', 'missing'] as const).map((s) => (
        <Row key={s} k={STATUS[s].label}>
          <Mark tone={STATUS[s].tone} glyph={STATUS[s].glyph}>
            {n(s)}
          </Mark>
        </Row>
      ))}
    </dl>
  );
}

function World({ w, now }: { w: WorldSummary; now: number }) {
  return (
    <Block title="World">
      <dl className="pane-rows">
        <Row k="id">
          <span className="mono">
            <Safe text={w.id} />
          </span>
        </Row>
        <Row k="remote">{w.remote ? <Safe text={w.remote} /> : '—'}</Row>
        <Row k="sessions">{w.sessions}</Row>
        <Row k="first">{w.first.slice(0, 10)}</Row>
        <Row k="latest">{ago(w.last, now)}</Row>
      </dl>
    </Block>
  );
}

export function Pane({
  route,
  load,
  world,
  now,
}: {
  route: Route;
  load: Load;
  world: WorldSummary | undefined;
  now: number;
}) {
  if (route.name === 'session') {
    return (
      <>
        <h2 className="pane-head">Session</h2>
        {['vitals', 'memory', 'files', 'bodies'].map((t) => (
          <Block key={t} title={t}>
            <p className="pane-note">— nothing yet</p>
          </Block>
        ))}
      </>
    );
  }
  if (world) {
    return (
      <>
        <h2 className="pane-head">Details</h2>
        <World w={world} now={now} />
      </>
    );
  }
  return (
    <>
      <h2 className="pane-head">At a glance</h2>
      <Block title="Worlds">
        <Counts load={load} />
      </Block>
    </>
  );
}
