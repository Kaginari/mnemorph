/**
 * The colony net page (`#/net`): the races and their ascended forms as a live 3D neural net on a
 * canvas, a card for the node picked, and the colony's shape in words beneath.
 *
 * A thin shell over the pure geometry in `data/net.ts`. Its canvas loop only runs in a browser
 * (jsdom has no 2D context), so the headless tests exercise the math and the words, not the
 * paint. Every colour is a theme token, read from the page, so both themes draw right.
 */
import './net.css';
import { useEffect, useRef, useState } from 'react';
import {
  type Camera,
  type ColonyNode,
  colour,
  DEFAULT_CAMERA,
  EDGES,
  FOV,
  GRID,
  NODES,
  nearest,
  node,
  orbit,
  type Projected,
  paintOrder,
  project,
  spin,
  toScreen,
  zoomed,
} from '../data/net.ts';

/** 0..1 how visible a depth is; z is the rotated depth, larger = nearer. */
function depthAlpha(z: number): number {
  return Math.min(1, Math.max(0, (z + 4.4) / 8.8));
}

const HINT = 'drag to orbit · scroll to zoom · click a node';

/** A grid point within this of the eye (depth FOV) is clipped: its line breaks there. */
const FOV_NEAR = 1.2;

function kindWords(n: ColonyNode): string {
  if (n.kind === 'ascended') return `ascended from ${node(n.ascendsFrom ?? '')?.name ?? ''}`;
  return n.kind;
}

/** Whether the page asks for less motion, following it. */
function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    setReduced(mq.matches);
    const on = (e: MediaQueryListEvent) => setReduced(e.matches);
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  }, []);
  return reduced;
}

function NetCanvas({
  pinned,
  onPin,
  onHover,
  reduced,
}: {
  pinned: string | undefined;
  onPin: (id: string | undefined) => void;
  onHover: (id: string | undefined) => void;
  reduced: boolean;
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  // The loop reads these each frame; the page's state stays the one truth.
  const pinnedRef = useRef(pinned);
  const reducedRef = useRef(reduced);
  const onPinRef = useRef(onPin);
  const onHoverRef = useRef(onHover);
  pinnedRef.current = pinned;
  reducedRef.current = reduced;
  onPinRef.current = onPin;
  onHoverRef.current = onHover;

  useEffect(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;

    let camera: Camera = DEFAULT_CAMERA;
    let raf = 0;
    let last = performance.now();
    let projected: Projected[] = [];
    let dragging = false;
    let moved = 0;
    let downX = 0;
    let downY = 0;
    let hoverId: string | undefined;

    const dpr = () => window.devicePixelRatio || 1;
    const resize = () => {
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (w > 0 && h > 0) {
        canvas.width = Math.round(w * dpr());
        canvas.height = Math.round(h * dpr());
      }
    };
    resize();
    window.addEventListener('resize', resize);

    const hover = (id: string | undefined) => {
      if (hoverId !== id) {
        hoverId = id;
        onHoverRef.current(id);
      }
    };

    /* ── painting: tokens read from the page each frame, so a theme switch shows at once ── */

    const tokens = () => {
      const css = getComputedStyle(canvas);
      const get = (t: string) => css.getPropertyValue(t).trim();
      return {
        holo: get('--mn-holo'),
        line: get('--mn-text-muted'),
        text: get('--mn-text'),
        mono: get('--mn-font-mono'),
        race: new Map(NODES.map((n) => [n.id, get(n.token)])),
      };
    };
    type Tokens = ReturnType<typeof tokens>;

    const drawGrid = (t: Tokens, w: number, h: number) => {
      ctx.save();
      ctx.strokeStyle = t.holo;
      ctx.globalAlpha = 0.07;
      ctx.lineWidth = 1;
      for (const line of GRID) {
        ctx.beginPath();
        let first = true;
        for (const pt of line) {
          const s = toScreen(pt, camera, w, h);
          if (s.z > FOV - FOV_NEAR) {
            first = true;
            continue;
          }
          if (first) ctx.moveTo(s.sx, s.sy);
          else ctx.lineTo(s.sx, s.sy);
          first = false;
        }
        ctx.stroke();
      }
      ctx.restore();
    };

    const drawEdges = (t: Tokens, w: number, h: number) => {
      const pin = pinnedRef.current;
      for (const e of EDGES) {
        const a = node(e.from);
        const b = node(e.to);
        if (!a || !b) continue;
        const pa = toScreen(a.position, camera, w, h);
        const pb = toScreen(b.position, camera, w, h);
        const hot = [e.from, e.to].some((id) => id === hoverId || id === pin);
        ctx.save();
        if (e.kind === 'ascends') {
          ctx.setLineDash([4, 6]);
          ctx.strokeStyle = t.race.get(b.id) ?? t.holo;
          ctx.globalAlpha = hot ? 0.75 : 0.3;
          ctx.lineWidth = 1.2;
        } else {
          ctx.strokeStyle = t.holo;
          ctx.globalAlpha =
            0.14 + 0.4 * Math.min(depthAlpha(pa.z), depthAlpha(pb.z)) + (hot ? 0.3 : 0);
          ctx.lineWidth = hot ? 1.8 : 1;
        }
        ctx.beginPath();
        ctx.moveTo(pa.sx, pa.sy);
        ctx.lineTo(pb.sx, pb.sy);
        ctx.stroke();
        ctx.restore();
      }
    };

    const drawNode = (t: Tokens, p: Projected, hot: boolean, focus: boolean) => {
      const { x, y, radius } = p;
      const c = t.race.get(p.node.id) ?? t.holo;
      const alpha = depthAlpha(p.depth);
      const rr = radius * (hot || focus ? 1.35 : 1);
      ctx.save();
      if (focus) {
        ctx.setLineDash([2, 5]);
        ctx.strokeStyle = c;
        ctx.globalAlpha = 0.9;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.arc(x, y, rr + 8, 0, Math.PI * 2);
        ctx.stroke();
        ctx.setLineDash([]);
      }
      // The glow: the ring's own shadow, so no colour is ever built from parts.
      ctx.shadowColor = c;
      ctx.shadowBlur = rr * 2.2;
      ctx.beginPath();
      ctx.arc(x, y, rr * 0.82, 0, Math.PI * 2);
      ctx.fillStyle = c;
      ctx.globalAlpha = 0.18 * alpha;
      ctx.fill();
      ctx.strokeStyle = c;
      ctx.globalAlpha = 0.9 * alpha;
      ctx.lineWidth = hot || focus ? 2.2 : 1.4;
      ctx.stroke();
      ctx.shadowBlur = 0;
      // A faint neutral rim: a pale race (High Elf, Named Slime) still reads on the light theme.
      ctx.beginPath();
      ctx.arc(x, y, rr * 0.82 + 1.5, 0, Math.PI * 2);
      ctx.strokeStyle = t.line;
      ctx.globalAlpha = 0.35 * alpha;
      ctx.lineWidth = 0.75;
      ctx.stroke();
      ctx.font = `${hot || focus ? 13 : 11}px ${t.mono}`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'top';
      ctx.fillStyle = hot || focus ? c : t.text;
      ctx.globalAlpha = hot || focus ? 1 : 0.35 + 0.4 * alpha;
      ctx.fillText(p.node.name, x, y + rr + 5);
      ctx.restore();
    };

    const frame = (now: number) => {
      raf = requestAnimationFrame(frame);
      const dt = Math.min((now - last) / 1000, 0.05);
      last = now;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (w <= 0 || h <= 0) return;
      camera = spin(camera, dt, !reducedRef.current && !dragging);
      projected = paintOrder(NODES.map((n) => project(n, camera, w, h)));
      const t = tokens();
      ctx.setTransform(dpr(), 0, 0, dpr(), 0, 0);
      ctx.clearRect(0, 0, w, h);
      drawGrid(t, w, h);
      drawEdges(t, w, h);
      const pin = pinnedRef.current;
      for (const p of projected) drawNode(t, p, p.node.id === hoverId, p.node.id === pin);
    };

    /* ── interaction ── */

    const at = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      return nearest(projected, e.clientX - rect.left, e.clientY - rect.top);
    };

    const onMove = (e: PointerEvent) => {
      if (!dragging) {
        hover(at(e)?.node.id);
        return;
      }
      const dx = e.clientX - downX;
      const dy = e.clientY - downY;
      moved += Math.abs(dx) + Math.abs(dy);
      downX = e.clientX;
      downY = e.clientY;
      camera = orbit(camera, dx * 0.006, dy * 0.006);
    };
    const onDown = (e: PointerEvent) => {
      dragging = true;
      moved = 0;
      downX = e.clientX;
      downY = e.clientY;
      canvas.setPointerCapture(e.pointerId);
    };
    const onUp = (e: PointerEvent) => {
      if (!dragging) return;
      dragging = false;
      if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
      if (moved >= 6) return;
      const hit = at(e)?.node.id;
      onPinRef.current(hit === pinnedRef.current ? undefined : hit);
    };
    const onLeave = () => hover(undefined);
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      camera = zoomed(camera, Math.exp(-e.deltaY * 0.0012));
    };
    const onKey = (e: KeyboardEvent) => {
      const turn: Record<string, [number, number]> = {
        ArrowLeft: [-0.08, 0],
        ArrowRight: [0.08, 0],
        ArrowUp: [0, -0.08],
        ArrowDown: [0, 0.08],
      };
      const d = turn[e.key];
      if (d) {
        e.preventDefault();
        camera = orbit(camera, d[0], d[1]);
      } else if (e.key === '+' || e.key === '=') camera = zoomed(camera, 1.15);
      else if (e.key === '-') camera = zoomed(camera, 1 / 1.15);
    };

    canvas.addEventListener('pointermove', onMove);
    canvas.addEventListener('pointerdown', onDown);
    canvas.addEventListener('pointerup', onUp);
    canvas.addEventListener('pointerleave', onLeave);
    canvas.addEventListener('wheel', onWheel, { passive: false });
    canvas.addEventListener('keydown', onKey);
    raf = requestAnimationFrame(frame);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener('resize', resize);
      canvas.removeEventListener('pointermove', onMove);
      canvas.removeEventListener('pointerdown', onDown);
      canvas.removeEventListener('pointerup', onUp);
      canvas.removeEventListener('pointerleave', onLeave);
      canvas.removeEventListener('wheel', onWheel);
      canvas.removeEventListener('keydown', onKey);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="net-canvas"
      tabIndex={0}
      role="img"
      aria-label="The colony net in 3D: the arrow keys orbit, + and - zoom. The races are listed below the drawing."
    />
  );
}

function RaceRow({ n }: { n: ColonyNode }) {
  return (
    <li className="net-race" style={{ borderLeftColor: colour(n) }}>
      <strong>{n.name}</strong>
      <span className="net-race-job">{n.job}</span>
    </li>
  );
}

export function NetView() {
  const [pinned, setPinned] = useState<string | undefined>(undefined);
  const [hovered, setHovered] = useState<string | undefined>(undefined);
  const reduced = useReducedMotion();
  const card = pinned === undefined ? undefined : node(pinned);
  const over = hovered === undefined ? undefined : node(hovered);
  const commands = EDGES.filter((e) => e.kind === 'command').length;

  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">Colony net</h1>
      </header>
      <p className="page-sub">
        The six races and the five ascended forms, as a living net — the same in every world. A
        world's own bodies, live, are on its Bodies page.
      </p>
      <section className="net-stage" aria-label="the net, drawn">
        <NetCanvas pinned={pinned} onPin={setPinned} onHover={setHovered} reduced={reduced} />
        <p className="net-hint">
          {over ? `${over.name} — ${over.job}` : HINT}
          {reduced ? ' · still: motion reduced' : ' · turning'}
        </p>
        {card && (
          <aside className="net-card" style={{ borderLeftColor: colour(card) }} aria-live="polite">
            <button
              type="button"
              className="net-card-close"
              onClick={() => setPinned(undefined)}
              aria-label={`Close ${card.name}'s card`}
            >
              ×
            </button>
            <span className="net-card-kind">{kindWords(card)}</span>
            <h2 className="net-card-name">{card.name}</h2>
            <p className="net-card-job">{card.job}</p>
          </aside>
        )}
      </section>
      <nav className="net-legend" aria-label="the races">
        {NODES.map((n) => (
          <button
            key={n.id}
            type="button"
            className="net-legend-item"
            aria-pressed={pinned === n.id}
            onClick={() => setPinned(pinned === n.id ? undefined : n.id)}
          >
            <span className="net-dot" style={{ background: colour(n) }} aria-hidden="true" />
            {n.name}
          </button>
        ))}
      </nav>
      <section className="panel net-about">
        <h2 className="panel-title">The human and the six races</h2>
        <p className="panel-note">
          Every run of Mnemorph is a small colony: one creature per job, each with its own shelf,
          its own soil and its own gate. The edges are who commissions whom.
        </p>
        <ul className="net-races">
          {NODES.filter((n) => n.kind !== 'ascended').map((n) => (
            <RaceRow key={n.id} n={n} />
          ))}
        </ul>
        <h2 className="panel-title">The ascended forms</h2>
        <p className="panel-note">
          A creature that keeps doing its job well ascends — the same lane, less friction. Five
          forms rise from the five bases; a form that slips falls back to its base.
        </p>
        <ul className="net-races">
          {NODES.filter((n) => n.kind === 'ascended').map((n) => (
            <RaceRow key={n.id} n={n} />
          ))}
        </ul>
        <p className="panel-note">
          {commands} command edges · {EDGES.length - commands} ascension edges · one throne
        </p>
      </section>
    </div>
  );
}
