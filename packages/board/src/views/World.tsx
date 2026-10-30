/** Inside a world: its overview, a section, or a session. */
import type { BoardWorld } from '@mnemorph/protocol';
import type { WorldSection } from '../router.ts';
import { Safe } from '../safe/Safe.tsx';
import { Empty } from '../ui/Empty.tsx';
import { Mark, STATUS } from '../ui/Mark.tsx';
import { SECTIONS } from '../ui/Rail.tsx';

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

export function WorldView({ w }: { w: BoardWorld }) {
  const s = STATUS[w.state];
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">
          <Safe text={w.name} />
        </h1>
        <Mark tone={s.tone} glyph={s.glyph}>
          {s.label}
        </Mark>
      </header>
      <p className="mono page-sub">
        <Safe text={w.path} />
      </p>
      {w.state === 'missing' ? (
        <Empty glyph="✕" title="This world's directory is gone">
          It stays listed, never silently dropped, until{' '}
          <code className="mono">mnemorphd board forget &lt;dir&gt;</code>.
        </Empty>
      ) : (
        <Empty glyph="◈" title="The world overview is not drawn yet">
          Its standing, findings and last verdict appear here once the board server reads them. The
          sections in the rail open their own pages.
        </Empty>
      )}
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
