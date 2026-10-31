/**
 * The board's shell: the rail on the left, the centre, the foldable pane on the right — and at
 * phone width the rail and the pane as drawers over the centre.
 */
import type { BoardWorld } from '@mnemorph/protocol';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { type Chats, useChats } from './data/chats.ts';
import { commands } from './data/commands.ts';
import { type Load, useWorlds } from './data/worlds.ts';
import { type Route, useRoute, worldOf } from './router.ts';
import { Safe } from './safe/Safe.tsx';
import { shown } from './safe/shown.ts';
import { type Choice, useTheme } from './theme.ts';
import { Announcer } from './ui/Announcer.tsx';
import { CommandPalette } from './ui/CommandPalette.tsx';
import { cardUp, dialogUp, typing } from './ui/modal.ts';
import { Pane } from './ui/Pane.tsx';
import { Rail } from './ui/Rail.tsx';
import { ShortcutMap } from './ui/ShortcutMap.tsx';
import { BodiesView } from './views/Bodies.tsx';
import { LiveChat, PastChat, type Side } from './views/Chat.tsx';
import { IntegrationsView } from './views/Integrations.tsx';
import { JournalView } from './views/Journal.tsx';
import { MemoryView } from './views/Memory.tsx';
import { MetricsView } from './views/Metrics.tsx';
import { OntologyView } from './views/Ontology.tsx';
import { SettingsView, UnknownView } from './views/Pages.tsx';
import { NoWorld, SectionView, WorldView } from './views/World.tsx';
import { WorldsView } from './views/Worlds.tsx';

const PANE_KEY = 'mnemorph.pane';
const PHONE = '(max-width: 760px)';

function initialPane(): boolean {
  try {
    const v = window.localStorage.getItem(PANE_KEY);
    if (v === 'open' || v === 'closed') return v === 'open';
  } catch {
    // no storage: fall through to the width's default
  }
  return !window.matchMedia(PHONE).matches;
}

/** Whether the page is at phone width, following it. */
function usePhone(): boolean {
  const [phone, setPhone] = useState(() => window.matchMedia(PHONE).matches);
  useEffect(() => {
    const mq = window.matchMedia(PHONE);
    const on = () => setPhone(mq.matches);
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  }, []);
  return phone;
}

/**
 * A drawer's focus: into the drawer when it opens, back to the button that opened it when it
 * closes. Only while it is a drawer (phone width).
 */
function useDrawerFocus(
  open: boolean,
  active: boolean,
  drawer: React.RefObject<HTMLElement | null>,
  trigger: React.RefObject<HTMLElement | null>,
) {
  const was = useRef(open);
  useEffect(() => {
    if (active && open && !was.current) {
      const el = drawer.current;
      (el?.querySelector<HTMLElement>('a[href], button:not([disabled])') ?? el)?.focus();
    } else if (active && !open && was.current) {
      trigger.current?.focus();
    }
    was.current = open;
  }, [open, active, drawer, trigger]);
}

function useNow(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(t);
  }, []);
  return now;
}

function Centre({
  route,
  load,
  world,
  now,
  theme,
  onTheme,
  chats,
  onSide,
}: {
  route: Route;
  load: Load;
  world: BoardWorld | undefined;
  now: number;
  theme: Choice;
  onTheme: (c: Choice) => void;
  chats: Chats;
  onSide: (s: Side | undefined) => void;
}) {
  switch (route.name) {
    case 'worlds':
      return <WorldsView load={load} now={now} />;
    case 'metrics':
      return <MetricsView load={load} />;
    case 'integrations':
      return <IntegrationsView theme={theme} />;
    case 'settings':
      return <SettingsView theme={theme} onTheme={onTheme} />;
    case 'unknown':
      return <UnknownView path={route.path} />;
  }
  if (load.state !== 'ready') return <WorldsView load={load} now={now} />;
  if (!world) return <NoWorld id={route.world} />;
  if (route.name === 'section' && route.section === 'bodies') {
    return <BodiesView key={world.id} w={world} tab={route.tab} />;
  }
  if (route.name === 'section' && route.section === 'journal') {
    return <JournalView key={world.id} w={world} />;
  }
  if (route.name === 'section' && route.section === 'memory') {
    return <MemoryView key={world.id} w={world} />;
  }
  if (route.name === 'section' && route.section === 'ontology') {
    return <OntologyView key={world.id} w={world} />;
  }
  if (route.name === 'section') return <SectionView w={world} section={route.section} />;
  if (route.name === 'session') {
    return (
      <LiveChat key={route.session} w={world} id={route.session} chats={chats} onSide={onSide} />
    );
  }
  if (route.name === 'run') {
    return <PastChat key={route.run} w={world} run={route.run} chats={chats} onSide={onSide} />;
  }
  return <WorldView key={world.id} w={world} now={now} />;
}

/** The centre's title, for the top bar and the tab. */
function title(route: Route, world: BoardWorld | undefined): string {
  switch (route.name) {
    case 'worlds':
      return 'Worlds';
    case 'metrics':
      return 'Metrics';
    case 'integrations':
      return 'Integrations';
    case 'settings':
      return 'Settings';
    case 'unknown':
      return 'Not found';
    default:
      return world?.name ?? route.world;
  }
}

/** Which of the board's own dialogs is open: the palette, the shortcut map, or neither. */
type Open = 'palette' | 'shortcuts' | undefined;

/**
 * The board's shortcuts: ⌘K or Ctrl+K the palette, `?` the map. Neither opens while an approval
 * card (or any other dialog) holds the page, and no key here answers anything.
 */
function useShortcuts(setOpen: (o: Open | ((was: Open) => Open)) => void) {
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.repeat || cardUp()) return;
      const palette = (e.key === 'k' || e.key === 'K') && (e.metaKey || e.ctrlKey) && !e.altKey;
      if (palette) {
        e.preventDefault();
        setOpen((was) => (was === 'palette' ? undefined : dialogUp() ? was : 'palette'));
        return;
      }
      if (e.key === '?' && !e.metaKey && !e.ctrlKey && !e.altKey && !typing(e.target)) {
        if (dialogUp()) return;
        e.preventDefault();
        setOpen('shortcuts');
      }
    };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, [setOpen]);
}

export function App() {
  const route = useRoute();
  const load = useWorlds();
  const now = useNow();
  const [theme, setTheme] = useTheme();
  const [rail, setRail] = useState(false);
  const [pane, setPane] = useState(initialPane);
  const phone = usePhone();
  const railRef = useRef<HTMLElement>(null);
  const paneRef = useRef<HTMLElement>(null);
  const menuRef = useRef<HTMLButtonElement>(null);
  const paneButtonRef = useRef<HTMLButtonElement>(null);
  useDrawerFocus(rail, phone, railRef, menuRef);
  useDrawerFocus(pane, phone, paneRef, paneButtonRef);

  const here = worldOf(route);
  const world =
    here !== undefined && load.state === 'ready'
      ? load.data.worlds.find((w) => w.id === here)
      : undefined;
  const name = title(route, world);
  const chats = useChats(here);
  const [side, setSide] = useState<Side | undefined>(undefined);
  const onSide = useCallback((s: Side | undefined) => setSide(s), []);
  const [open, setOpen] = useState<Open>(undefined);
  const closeDialog = useCallback(() => setOpen(undefined), []);
  useShortcuts(setOpen);
  const all = useMemo(
    () => commands(load.state === 'ready' ? load.data.worlds : [], here, chats.live, chats.runs),
    [load, here, chats.live, chats.runs],
  );
  const mainRef = useRef<HTMLElement>(null);

  // A drawer closes when the page moves; Esc closes either.
  // biome-ignore lint/correctness/useExhaustiveDependencies: closes on every route change
  useEffect(() => setRail(false), [route]);
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || e.defaultPrevented) return;
      setRail(false);
      if (window.matchMedia(PHONE).matches) setPane(false);
    };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, []);
  useEffect(() => {
    // The name is a world's own text, or a decoded address: escaped, never a live bidi mark.
    document.title = `${shown(name)} · Mnemorph`;
  }, [name]);

  const togglePane = () =>
    setPane((p) => {
      try {
        window.localStorage.setItem(PANE_KEY, p ? 'closed' : 'open');
      } catch {
        // kept for this page only
      }
      return !p;
    });

  return (
    <div
      className="shell"
      data-load={load.state}
      data-rail={rail ? 'open' : 'closed'}
      data-pane={pane ? 'open' : 'closed'}
    >
      <button
        type="button"
        className="skip"
        onClick={() => {
          mainRef.current?.focus();
        }}
      >
        Skip to the content
      </button>
      <Rail
        ref={railRef}
        inert={phone && !rail}
        route={route}
        load={load}
        now={now}
        theme={theme}
        onTheme={setTheme}
        chats={chats}
        onSearch={() => {
          if (!cardUp()) setOpen('palette');
        }}
      />
      <button
        type="button"
        className="scrim"
        aria-label="Close the menu"
        tabIndex={-1}
        onClick={() => {
          setRail(false);
          if (window.matchMedia(PHONE).matches) setPane(false);
        }}
      />
      <div className="centre">
        <header className="topbar">
          <button
            type="button"
            ref={menuRef}
            className="icon-button rail-toggle"
            aria-label="Menu"
            aria-controls="rail"
            aria-expanded={rail}
            onClick={() => setRail((r) => !r)}
          >
            ☰
          </button>
          <span className="topbar-title">
            <Safe text={name} />
          </span>
          <button
            type="button"
            className="icon-button topbar-search"
            aria-label="Search: jump to a world, a page, a chat or a run"
            aria-keyshortcuts="Control+K Meta+K"
            onClick={() => {
              if (!cardUp()) setOpen('palette');
            }}
          >
            ⌕
          </button>
          <button
            type="button"
            ref={paneButtonRef}
            className="icon-button"
            aria-label={pane ? 'Hide the side pane' : 'Show the side pane'}
            aria-controls="pane"
            aria-expanded={pane}
            onClick={togglePane}
          >
            ◨
          </button>
        </header>
        <main
          ref={mainRef}
          id="main"
          // biome-ignore lint/a11y/noNoninteractiveTabindex: the centre scrolls; the keys scroll it
          tabIndex={0}
          className={
            route.name === 'session' || route.name === 'run'
              ? 'centre-main centre-chat'
              : 'centre-main'
          }
        >
          <Centre
            route={route}
            load={load}
            world={world}
            now={now}
            theme={theme}
            onTheme={setTheme}
            chats={chats}
            onSide={onSide}
          />
        </main>
      </div>
      <aside
        ref={paneRef}
        tabIndex={-1}
        className="pane"
        id="pane"
        aria-label="Side pane"
        hidden={!pane}
      >
        <Pane route={route} load={load} world={world} now={now} side={side} />
      </aside>
      {open === 'palette' && <CommandPalette all={all} onClose={closeDialog} />}
      {open === 'shortcuts' && <ShortcutMap onClose={closeDialog} />}
      {createPortal(<Announcer />, document.body)}
    </div>
  );
}
