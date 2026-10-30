/** The board-wide pages, and the page for an address the board does not have. */
import { Safe } from '../safe/Safe.tsx';
import { CHOICES, type Choice } from '../theme.ts';
import { Empty } from '../ui/Empty.tsx';
import { RACES, RaceChip } from '../ui/Mark.tsx';

export function MetricsView() {
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">Metrics</h1>
      </header>
      <Empty glyph="▤" title="No metrics drawn yet">
        Spend, tokens, cache and the races' value meters appear here once the board server reads the
        ledgers. Until then, <code className="mono">mnemorph status</code> and{' '}
        <code className="mono">mnemorphd meters</code> print them.
      </Empty>
    </div>
  );
}

export function IntegrationsView() {
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">Integrations</h1>
      </header>
      <Empty glyph="⚇" title="No integrations shown yet">
        Providers, models, prices, key sources and MCP servers appear here, read-only. Until then,{' '}
        <code className="mono">mnemorph status</code> lists them.
      </Empty>
    </div>
  );
}

export function SettingsView({ theme, onTheme }: { theme: Choice; onTheme: (c: Choice) => void }) {
  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">Settings</h1>
      </header>
      <section className="setting">
        <h2 className="setting-title" id="setting-theme">
          Theme
        </h2>
        <p className="setting-sub">Auto follows the system's light or dark preference.</p>
        <div className="radio-row" role="radiogroup" aria-labelledby="setting-theme">
          {CHOICES.map((c) => (
            <label key={c} className="radio">
              <input
                type="radio"
                name="theme"
                value={c}
                checked={theme === c}
                onChange={() => onTheme(c)}
              />
              {c}
            </label>
          ))}
        </div>
      </section>
      <section className="setting">
        <h2 className="setting-title">The races</h2>
        <p className="setting-sub">
          Each race has one colour across the board and the terminal, and is always named beside it.
        </p>
        <ul className="race-list">
          {RACES.map(([race, name]) => (
            <li key={race}>
              <RaceChip race={race} name={name} />
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

export function UnknownView({ path }: { path: string }) {
  return (
    <div className="page">
      <Empty glyph="⌀" title="Nothing at this address">
        <p>
          The board has no page at{' '}
          <code className="mono">
            <Safe text={path || '/'} />
          </code>
          .
        </p>
        <p>
          <a href="#/">See every world</a>
        </p>
      </Empty>
    </div>
  );
}
