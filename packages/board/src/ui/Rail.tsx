/**
 * The left rail: the wordmark, search, the worlds, then the board-wide pages. Inside a world it
 * becomes that world's: back to all worlds, its chats and its sections.
 */
import type { BoardRun, BoardSession, BoardWorld } from '@mnemorph/protocol';
import { useState } from 'react';
import { type Chats, startChat } from '../data/chats.ts';
import type { Load } from '../data/worlds.ts';
import { format, type Route, type WorldSection, worldOf } from '../router.ts';
import { Safe } from '../safe/Safe.tsx';
import { CHOICES, type Choice } from '../theme.ts';
import { ago } from '../time.ts';
import { Mark, STATUS } from './Mark.tsx';
import { Wordmark } from './Wordmark.tsx';

const PAGES = [
  ['metrics', '▤', 'Metrics'],
  ['integrations', '⚇', 'Integrations'],
  ['settings', '⚙', 'Settings'],
] as const;

export const SECTIONS: readonly [WorldSection, string, string][] = [
  ['bodies', '⛬', 'Bodies & queue'],
  ['memory', '✧', 'Memory'],
  ['ontology', '◇', 'Ontology'],
  ['journal', '▤', 'Journal'],
];

const THEME_GLYPH: Record<Choice, string> = { auto: '◐', light: '☀', dark: '☾' };

function Item({
  href,
  glyph,
  current,
  children,
}: {
  href: string;
  glyph: string;
  current: boolean;
  children: React.ReactNode;
}) {
  return (
    <a className="rail-item" href={href} aria-current={current ? 'page' : undefined}>
      <span className="rail-glyph" aria-hidden="true">
        {glyph}
      </span>
      {children}
    </a>
  );
}

function WorldRow({ w, current, now }: { w: BoardWorld; current: boolean; now: number }) {
  const s = STATUS[w.state];
  const when =
    w.state === 'missing' ? s.label : w.last === null ? 'never opened' : ago(w.last, now);
  return (
    <a
      className="rail-item rail-world"
      href={format({ name: 'world', world: w.id })}
      aria-current={current ? 'page' : undefined}
    >
      <Mark tone={s.tone} glyph={s.glyph}>
        <span className="rail-world-name">
          <Safe text={w.name} />
        </span>
      </Mark>
      <span className="rail-meta">{when}</span>
    </a>
  );
}

/** A live chat's words: the ask its run journal holds, else that it is new. */
function liveTitle(s: BoardSession, runs: readonly BoardRun[]): string {
  const r = runs.find((x) => x.id === s.run);
  return r?.ask || 'new chat';
}

function ChatList({
  world,
  route,
  chats,
  now,
}: {
  world: string;
  route: Route;
  chats: Chats;
  now: number;
}) {
  if (!chats.ready) return <p className="rail-note">Reading chats…</p>;
  const liveRuns = new Set(chats.live.map((s) => s.run));
  const past = chats.runs.filter((r) => !liveRuns.has(r.id));
  if (chats.live.length === 0 && past.length === 0) {
    return (
      <p className="rail-note">
        {chats.failed ? (
          <Mark tone="alarm" glyph="✕">
            <Safe text={chats.failed} />
          </Mark>
        ) : (
          'No chats yet.'
        )}
      </p>
    );
  }
  return (
    <>
      {chats.live.map((s) => (
        <a
          key={s.id}
          className="rail-item rail-chat"
          href={format({ name: 'session', world, session: s.id })}
          aria-current={route.name === 'session' && route.session === s.id ? 'page' : undefined}
        >
          <Mark tone={s.asks > 0 ? 'gold' : 'holo'} glyph={s.asks > 0 ? '◆' : '●'}>
            <span className="rail-chat-name">
              <Safe text={liveTitle(s, chats.runs)} />
            </span>
          </Mark>
          <span className="rail-meta">{s.asks > 0 ? `${s.asks} waiting` : 'live'}</span>
        </a>
      ))}
      {past.map((r) => (
        <a
          key={r.id}
          className="rail-item rail-chat"
          href={format({ name: 'run', world, run: r.id })}
          aria-current={route.name === 'run' && route.run === r.id ? 'page' : undefined}
        >
          <Mark
            tone={r.state === 'stopped' ? 'gold' : r.state === 'live' ? 'holo' : 'muted'}
            glyph={r.state === 'live' ? '●' : r.state === 'stopped' ? '◇' : '○'}
          >
            <span className="rail-chat-name">
              <Safe text={r.ask || r.id} />
            </span>
          </Mark>
          <span className="rail-meta">
            {r.state === 'live' ? 'terminal' : r.state === 'stopped' ? 'stopped' : ago(r.when, now)}
          </span>
        </a>
      ))}
      {chats.more > 0 && <p className="rail-note">{chats.more} older runs not listed</p>}
    </>
  );
}

function NewChat({ world, chats, disabled }: { world: string; chats: Chats; disabled: boolean }) {
  const [busy, setBusy] = useState(false);
  const [why, setWhy] = useState<string | undefined>(undefined);
  return (
    <>
      <button
        type="button"
        className="rail-new"
        disabled={disabled || busy}
        onClick={async () => {
          setBusy(true);
          setWhy(undefined);
          try {
            const started = await startChat(world);
            chats.refresh();
            window.location.hash = format({ name: 'session', world, session: started.session.id });
          } catch (e) {
            setWhy(e instanceof Error ? e.message : String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        {busy ? 'Opening a chat…' : '+ New chat'}
      </button>
      {why && (
        <p className="rail-note" role="alert">
          <Mark tone="alarm" glyph="✕">
            <Safe text={why} />
          </Mark>
        </p>
      )}
    </>
  );
}

function Worlds({ load, route, now }: { load: Load; route: Route; now: number }) {
  if (load.state === 'loading') return <p className="rail-note">Reading worlds…</p>;
  if (load.state === 'failed') {
    return (
      <p className="rail-note">
        <Mark tone="alarm" glyph="✕">
          unreachable
        </Mark>
      </p>
    );
  }
  if (load.data.worlds.length === 0) return <p className="rail-note">No worlds yet.</p>;
  const here = worldOf(route);
  return (
    <>
      {load.data.worlds.map((w) => (
        <WorldRow key={w.id} w={w} current={here === w.id} now={now} />
      ))}
    </>
  );
}

function Theme({ choice, onChoice }: { choice: Choice; onChoice: (c: Choice) => void }) {
  return (
    // biome-ignore lint/a11y/useSemanticElements: a segmented control, not a form fieldset
    <div className="theme-switch" role="group" aria-label="Theme">
      {CHOICES.map((c) => (
        <button
          key={c}
          type="button"
          className="theme-option"
          aria-pressed={choice === c}
          onClick={() => onChoice(c)}
        >
          <span aria-hidden="true">{THEME_GLYPH[c]}</span> {c}
        </button>
      ))}
    </div>
  );
}

export function Rail({
  ref,
  inert,
  route,
  load,
  now,
  theme,
  onTheme,
  chats,
}: {
  ref: React.Ref<HTMLElement>;
  inert: boolean;
  route: Route;
  load: Load;
  now: number;
  theme: Choice;
  onTheme: (c: Choice) => void;
  chats: Chats;
}) {
  const here = worldOf(route);
  const world =
    here !== undefined && load.state === 'ready'
      ? load.data.worlds.find((w) => w.id === here)
      : undefined;
  return (
    <nav ref={ref} inert={inert} className="rail" id="rail" aria-label="Board">
      <div className="rail-head">
        <Wordmark />
      </div>
      <div className="rail-search" title="Search arrives with the command palette">
        <span aria-hidden="true">⌕</span>
        <span className="rail-search-text">Search</span>
        <kbd>⌘K</kbd>
      </div>
      <div className="rail-scroll">
        {here === undefined ? (
          <section className="rail-section" aria-labelledby="rail-worlds">
            <h2 className="rail-title" id="rail-worlds">
              <a href="#/" aria-current={route.name === 'worlds' ? 'page' : undefined}>
                ▾ Worlds
              </a>
            </h2>
            <Worlds load={load} route={route} now={now} />
          </section>
        ) : (
          <>
            <a className="rail-back" href="#/">
              <span aria-hidden="true">◂</span> All worlds
            </a>
            <div className="rail-world-head">
              {world ? (
                <Mark tone={STATUS[world.state].tone} glyph={STATUS[world.state].glyph}>
                  <Safe text={world.name} />
                </Mark>
              ) : (
                <Safe text={here} />
              )}
            </div>
            <NewChat world={here} chats={chats} disabled={world?.state !== 'present'} />
            <section className="rail-section" aria-labelledby="rail-chats">
              <h2 className="rail-title" id="rail-chats">
                ▾ Chats
              </h2>
              <ChatList world={here} route={route} chats={chats} now={now} />
            </section>
            <section className="rail-section" aria-labelledby="rail-world">
              <h2 className="rail-title" id="rail-world">
                <a
                  href={format({ name: 'world', world: here })}
                  aria-current={route.name === 'world' ? 'page' : undefined}
                >
                  ▾ World
                </a>
              </h2>
              {SECTIONS.map(([section, glyph, label]) => (
                <Item
                  key={section}
                  href={format({ name: 'section', world: here, section })}
                  glyph={glyph}
                  current={route.name === 'section' && route.section === section}
                >
                  {label}
                </Item>
              ))}
            </section>
          </>
        )}
        <section className="rail-section rail-pages" aria-label="Board pages">
          {PAGES.map(([name, glyph, label]) => (
            <Item key={name} href={`#/${name}`} glyph={glyph} current={route.name === name}>
              {label}
            </Item>
          ))}
        </section>
      </div>
      <div className="rail-foot">
        <Theme choice={theme} onChoice={onTheme} />
      </div>
    </nav>
  );
}
