/**
 * The colony net, drawn: the model of `data/colony.ts` as SVG — a band per race lane, a curve per
 * command, a box per body with its name and, beneath, its race and live state in words. Each body
 * is a button: a click, Enter or Space opens its card. Names are a body's own text: drawn through
 * the same escapes as `<Safe>` (hidden characters shown), never markup.
 */
import { type Colony, NODE_H, NODE_W, STATE_WORDS } from '../data/colony.ts';
import { runs, shown } from '../safe/shown.ts';

/** `text` for an SVG `<text>`: hidden characters escaped and set apart, cut to `max`. */
export function SvgSafe({ text, max = 22 }: { text: string; max?: number }) {
  const parts = runs(text, false);
  let left = max;
  const out: { text: string; escaped: boolean }[] = [];
  for (const p of parts) {
    if (left <= 0) break;
    const t = [...p.text].slice(0, left).join('');
    left -= [...t].length;
    out.push({ text: t, escaped: p.escaped });
  }
  const cut = parts.reduce((n, p) => n + [...p.text].length, 0) > max;
  return (
    <>
      {out.map((p, i) =>
        p.escaped ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: runs never reorder
          <tspan key={i} className="svg-esc">
            {p.text}
          </tspan>
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: runs never reorder
          <tspan key={i}>{p.text}</tspan>
        ),
      )}
      {cut && <tspan>…</tspan>}
    </>
  );
}

function edgePath(from: { x: number; y: number }, to: { x: number; y: number }): string {
  if (from.y === to.y) {
    // Sideways, inside one lane: an arc over the boxes.
    const top = from.y - NODE_H / 2;
    return `M ${from.x} ${top} C ${from.x} ${top - 26}, ${to.x} ${top - 26}, ${to.x} ${top}`;
  }
  const a = from.y + NODE_H / 2;
  const b = to.y - NODE_H / 2;
  const m = (a + b) / 2;
  return `M ${from.x} ${a} C ${from.x} ${m}, ${to.x} ${m}, ${to.x} ${b}`;
}

export function ColonyNet({
  c,
  selected,
  onPick,
}: {
  c: Colony;
  selected: string | undefined;
  onPick: (name: string) => void;
}) {
  const at = new Map(c.nodes.map((n) => [n.name, n]));
  return (
    <figure
      className="colony-scroll"
      aria-label="the colony net: race lanes and the chain of command"
    >
      <svg
        className="colony"
        width={c.width}
        height={c.height}
        viewBox={`0 0 ${c.width} ${c.height}`}
      >
        <title>the colony net</title>
        {c.lanes.map((l, i) => (
          <g key={l.race} className="colony-lane" data-band={i % 2 === 0 ? 'even' : 'odd'}>
            <rect x={0} y={l.y - 46} width={c.width} height={92} className="colony-band" />
            <text x={12} y={l.y + 4} className="colony-lane-title">
              {l.title}
            </text>
          </g>
        ))}
        {c.edges.map((e) => {
          const from = at.get(e.from);
          const to = at.get(e.to);
          if (!from || !to) return null;
          return (
            <path
              key={`${e.from}>${e.to}`}
              d={edgePath(from, to)}
              className="colony-edge"
              data-from={e.from}
              data-to={e.to}
            />
          );
        })}
        {c.nodes.map((n) => {
          const s = STATE_WORDS[n.state];
          const pick = () => onPick(n.name);
          return (
            // biome-ignore lint/a11y/useSemanticElements: an SVG group cannot be a <button>
            <g
              key={n.name}
              className="colony-node race"
              data-race={n.race}
              data-state={n.state}
              data-name={n.name}
              role="button"
              tabIndex={0}
              aria-label={shown(n.label)}
              aria-pressed={selected === n.name}
              transform={`translate(${n.x - NODE_W / 2} ${n.y - NODE_H / 2})`}
              onClick={pick}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  pick();
                }
              }}
            >
              <title>{shown(n.label)}</title>
              <rect width={NODE_W} height={NODE_H} rx={6} className="colony-box" />
              <rect width={5} height={NODE_H} rx={2} className="colony-swatch" />
              <text x={14} y={19} className="colony-name mark-label">
                <SvgSafe text={n.name} />
              </text>
              <text x={14} y={36} className="colony-sub">
                {s.glyph} {n.synthetic ? 'the throne' : n.title} · {s.words}
              </text>
            </g>
          );
        })}
      </svg>
    </figure>
  );
}
