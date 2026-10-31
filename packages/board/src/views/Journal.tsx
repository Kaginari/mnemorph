/**
 * The journal: `log.md` entry by entry, newest first, with a table of contents and a search. Each
 * entry's text is a body's Markdown, rendered through the board's inert subset — raw HTML stays
 * text, links are allowlisted, images never load. The server reads only the file's end and cuts
 * each entry; the page says both.
 */
import type { BoardWorld, JournalEntry } from '@mnemorph/protocol';
import { useEffect, useState } from 'react';
import { type JournalQuery, useJournal } from '../data/bodies.ts';
import { Markdown } from '../safe/Markdown.tsx';
import { Safe } from '../safe/Safe.tsx';
import { Empty } from '../ui/Empty.tsx';
import { Mark } from '../ui/Mark.tsx';
import { Source } from './Bodies.tsx';
import { Pager } from './Memory.tsx';
import './bodies.css';

const anchor = (n: number) => `journal-entry-${n}`;

function Entry({ e }: { e: JournalEntry }) {
  return (
    <article className="panel journal-entry" id={anchor(e.n)} tabIndex={-1}>
      <header className="journal-head">
        <h2 className="journal-title">
          <Safe text={e.title} />
        </h2>
        <p className="panel-note">
          {e.at && (
            <span className="mono">
              <Safe text={e.at} />
            </span>
          )}
          {e.body && (
            <>
              {' '}
              · <Safe text={e.body} />
            </>
          )}{' '}
          · entry {e.n}
        </p>
      </header>
      <Markdown text={e.text} />
      {e.cut && (
        <p className="panel-note">
          <Mark tone="gold" glyph="✂">
            cut: the rest of this entry is in log.md
          </Mark>
        </p>
      )}
    </article>
  );
}

export function JournalView({ w }: { w: BoardWorld }) {
  const [q, setQ] = useState<JournalQuery>({});
  const [typed, setTyped] = useState('');
  const [jump, setJump] = useState<number>();
  const load = useJournal(w.id, q);
  const j = load.state === 'loading' ? undefined : load.data;
  const go = (n: number) => {
    if (!j) return;
    const at = j.toc.findIndex((t) => t.n === n);
    if (at < 0) return;
    const offset = Math.floor(at / j.limit) * j.limit;
    const focus = () => document.getElementById(anchor(n))?.focus();
    if (offset === j.offset) focus();
    else {
      setJump(n);
      setQ((was) => ({ ...was, offset }));
    }
  };
  useEffect(() => {
    if (jump === undefined || !j?.entries.some((e) => e.n === jump)) return;
    document.getElementById(anchor(jump))?.focus();
    setJump(undefined);
  }, [j, jump]);
  return (
    <div className="page page-wide">
      <header className="page-head">
        <h1 className="page-title">
          Journal
          <span className="page-crumb">
            <Safe text={w.name} />
          </span>
        </h1>
      </header>
      <form
        className="filters journal-search"
        aria-label="search the journal"
        onSubmit={(e) => {
          e.preventDefault();
          setQ({ q: typed.trim() === '' ? undefined : typed.trim(), offset: 0 });
        }}
      >
        <label className="filter">
          <span className="filter-label">search</span>
          <input
            type="search"
            value={typed}
            maxLength={200}
            placeholder="a word, a body, a verdict"
            onChange={(e) => setTyped(e.target.value)}
          />
        </label>
        <button type="submit" className="button button-small">
          Search
        </button>
        {q.q && (
          <button
            type="button"
            className="button button-small"
            onClick={() => {
              setTyped('');
              setQ({});
            }}
          >
            Clear
          </button>
        )}
      </form>
      {load.state === 'loading' && (
        <Empty glyph="⣾" title="Loading the journal…">
          Asking the board server for log.md.
        </Empty>
      )}
      {load.state === 'failed' && (
        <p className="hint" role="status">
          <Mark tone="alarm" glyph="✕">
            The journal could not be read: <Safe text={load.reason} />
          </Mark>
        </p>
      )}
      {j && (
        <p className="knowledge-strip mono">
          <span>{j.q ? `${j.total} of ${j.all} entries match` : `${j.all} entries`}</span>
          {j.older && (
            <span>
              {' '}
              ·{' '}
              <Mark tone="gold" glyph="⚠">
                only the end of log.md is read: older entries are not shown
              </Mark>
            </span>
          )}
          {j.unreadable > 0 && <span> · {j.unreadable} row(s) did not read</span>}
        </p>
      )}
      {j &&
        (j.entries.length === 0 ? (
          <Empty glyph="▤" title={j.bytes === null ? 'No journal yet' : 'No entry here'}>
            {j.bytes === null
              ? 'This world has no .mnemorph/log.md yet.'
              : 'No entry matches the search.'}
          </Empty>
        ) : (
          <div className="journal-grid">
            <nav className="panel journal-toc" aria-label="entries">
              <h2 className="panel-title">Entries</h2>
              <ol>
                {j.toc.map((t) => (
                  <li key={t.n}>
                    <button type="button" className="link-button" onClick={() => go(t.n)}>
                      {t.at && (
                        <span className="mono journal-toc-at">
                          <Safe text={t.at.slice(0, 10)} />
                        </span>
                      )}{' '}
                      <Safe text={t.title} />
                    </button>
                  </li>
                ))}
              </ol>
            </nav>
            <div className="journal-entries">
              {j.entries.map((e) => (
                <Entry key={e.n} e={e} />
              ))}
              <Pager
                offset={j.offset}
                limit={j.limit}
                total={j.total}
                onMove={(offset) => setQ((was) => ({ ...was, offset }))}
              />
            </div>
          </div>
        ))}
      {j && (
        <Source
          what="journal"
          p={{
            source: j.source,
            trusted: false,
            why: 'a body can write it: shown, granting nothing',
          }}
        />
      )}
    </div>
  );
}
