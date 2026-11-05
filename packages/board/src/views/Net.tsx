/**
 * The colony net page (`#/net`): the colony as a hex city around the throne, drawn in 3D on a
 * canvas — the terraced streets, the races and a world's living bodies on them, waves through the
 * ground, signals along who commissions whom. A race picked lights the city it reaches.
 *
 * A thin shell over the pure geometry in `data/net.ts`. Its canvas loop only runs in a browser
 * (jsdom has no 2D context), so the headless tests exercise the math and the words, not the
 * paint. Every colour is a theme token, read from the page, so both themes draw right: the dark
 * adds light, the light lays it on.
 */
import './net.css';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useBodies } from '../data/bodies.ts';
import {
  type Camera,
  CITY,
  type ColonyEdge,
  type ColonyNode,
  colour,
  crowdedLabels,
  DEFAULT_CAMERA,
  EDGES,
  energy,
  FOV,
  LATTICE,
  NODES,
  nearest,
  netOf,
  node,
  orbit,
  type Projected,
  paintOrder,
  project,
  raceOf,
  reachedCells,
  related,
  spin,
  toScreen,
  type Vec,
  WAVE_LIFE_S,
  type Wave,
  zoomed,
} from '../data/net.ts';
import { useWorlds } from '../data/worlds.ts';
import { shown } from '../safe/shown.ts';

const HINT = 'drag to orbit · scroll to zoom · click a race';

/** A point within this of the eye is clipped: its street breaks there. */
const FOV_NEAR = 1.2;

/** The net with no world to read: the races alone. */
const STILL = netOf([]);

type Net = ReturnType<typeof netOf>;

const SIGNALS = 14;
const COIL_STEPS = 96;
const MAX_WAVES = 4;
const STARS = Array.from({ length: 150 }, (_, i) => {
  const a = Math.sin(i * 12.9898) * 43758.5453;
  const b = Math.sin(i * 78.233) * 12543.1234;
  return { x: a - Math.floor(a), y: b - Math.floor(b), size: 0.4 + (i % 3) * 0.3, phase: i };
});

/** A soft disc of `colour`, for the glows: drawn once, stamped many times. */
function glowSprite(colour: string): HTMLCanvasElement {
  const sprite = document.createElement('canvas');
  sprite.width = 64;
  sprite.height = 64;
  const ctx = sprite.getContext('2d');
  if (!ctx) return sprite;
  const g = ctx.createRadialGradient(32, 32, 0, 32, 32, 32);
  g.addColorStop(0, colour);
  g.addColorStop(1, 'transparent');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, 64, 64);
  return sprite;
}

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
  net,
  pinned,
  onPin,
  onHover,
  reduced,
}: {
  net: Net;
  pinned: string | undefined;
  onPin: (id: string | undefined) => void;
  onHover: (id: string | undefined) => void;
  reduced: boolean;
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  // The loop reads these each frame; the page's state stays the one truth.
  const netRef = useRef(net);
  const pinnedRef = useRef(pinned);
  const reducedRef = useRef(reduced);
  const onPinRef = useRef(onPin);
  const onHoverRef = useRef(onHover);
  netRef.current = net;
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
    let lastWave = 0;
    const waves: Wave[] = [];
    const signals = Array.from({ length: SIGNALS }, (_, i) => ({ edge: i, t: i / SIGNALS }));
    const pictures = new Map<string, HTMLImageElement>();
    const sprites = new Map<string, HTMLCanvasElement>();
    // What the pin reaches, worked out once for each pin and each net.
    let reach = {
      pin: undefined as string | undefined,
      net: undefined as Net | undefined,
      near: new Set<string>(),
      cells: new Map<number, string>(),
    };

    const picture = (n: ColonyNode) => {
      const race = raceOf(n);
      let got = pictures.get(race);
      if (!got) {
        got = new Image();
        got.src = `/art/portraits/${race}.webp`;
        pictures.set(race, got);
      }
      return got;
    };
    const sprite = (c: string) => {
      let got = sprites.get(c);
      if (!got) {
        got = glowSprite(c);
        sprites.set(c, got);
      }
      return got;
    };
    const stamp = (c: string, x: number, y: number, size: number) =>
      ctx.drawImage(sprite(c), x - size, y - size, size * 2, size * 2);
    const pulse = (origin: Vec, token?: string) => {
      waves.push({ origin, start: performance.now(), token });
      if (waves.length > MAX_WAVES) waves.shift();
    };

    const dpr = () => Math.min(window.devicePixelRatio || 1, 2);
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
        gold: get('--mn-gold'),
        panel: get('--mn-panel'),
        muted: get('--mn-text-muted'),
        text: get('--mn-text'),
        ui: get('--mn-font-ui'),
        race: new Map(NODES.map((n) => [n.token, get(n.token)])),
      };
    };
    type Tokens = ReturnType<typeof tokens>;
    const tint = (t: Tokens, token: string | undefined) => (token && t.race.get(token)) || t.holo;

    const hexagon = (x: number, y: number, r: number) => {
      ctx.beginPath();
      for (let c = 0; c < 6; c++) {
        const a = (Math.PI / 3) * c + Math.PI / 6;
        const px = x + r * Math.cos(a);
        const py = y + r * Math.sin(a);
        if (c === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      }
      ctx.closePath();
    };

    const drawStars = (t: Tokens, w: number, h: number, time: number) => {
      ctx.fillStyle = t.holo;
      for (const s of STARS) {
        ctx.globalAlpha = 0.16 + 0.14 * Math.sin(time * 0.6 + s.phase);
        const x = (s.x * w + camera.yaw * 6) % w;
        ctx.fillRect(x < 0 ? x + w : x, s.y * h, s.size, s.size);
      }
    };

    /** The streets and their corners: the ground the waves run through. */
    const drawCity = (
      t: Tokens,
      w: number,
      h: number,
      now: number,
      faint: number,
      spot: number,
    ) => {
      const time = now / 1000;
      const still = reducedRef.current;
      const shell = LATTICE.points.map((pt, i) => {
        const lit = energy(pt, waves, now);
        const swell = still ? 0 : 0.008 * CITY * Math.sin(time * 0.9 + i * 0.31);
        const lifted: Vec = [pt[0], pt[1] + swell + 0.09 * CITY * lit.power, pt[2]];
        return { at: toScreen(lifted, camera, w, h), lit };
      });
      const dim = pinnedRef.current === undefined ? 1 : 0.5;
      for (const [from, to] of LATTICE.links) {
        const a = shell[from];
        const b = shell[to];
        if (!a || !b || a.at.z > FOV - FOV_NEAR || b.at.z > FOV - FOV_NEAR) continue;
        const depth = (a.at.z / CITY + 1) / 2;
        const lit = Math.max(a.lit.power, b.lit.power);
        const cell = reach.cells.get(from) ?? reach.cells.get(to);
        const flow = still ? 0.8 : 0.62 + 0.38 * Math.sin(time * 4 - (from % 37) * 0.5);
        ctx.lineWidth = cell ? 1.5 : 0.5;
        ctx.globalAlpha = cell ? flow : dim * faint * (0.05 + 0.1 * depth) + 0.75 * lit;
        ctx.strokeStyle = cell
          ? tint(t, cell)
          : tint(t, lit > 0.02 ? (a.lit.wave?.token ?? b.lit.wave?.token) : undefined);
        ctx.beginPath();
        ctx.moveTo(a.at.sx, a.at.sy);
        ctx.lineTo(b.at.sx, b.at.sy);
        ctx.stroke();
      }
      shell.forEach(({ at, lit }, i) => {
        const depth = (at.z / CITY + 1) / 2;
        const cell = reach.cells.get(i);
        const size = ((cell ? spot : 1.5) + 7 * lit.power) * at.scale;
        ctx.globalAlpha = cell ? 0.9 : faint * (0.1 + 0.3 * depth) + 0.6 * lit.power;
        stamp(tint(t, cell ?? lit.wave?.token), at.sx, at.sy, size);
      });
    };

    /** Two ribbons winding down from the throne: the colony's work, turning. */
    const drawCoil = (t: Tokens, w: number, h: number, time: number, busy: boolean) => {
      for (const [strand, c] of [
        [0, tint(t, '--mn-race-rimuru')],
        [Math.PI, t.gold],
      ] as const) {
        const turn = time * (busy ? 1 : 0.4);
        const at = Array.from({ length: COIL_STEPS }, (_, step) => {
          const u = step / (COIL_STEPS - 1);
          const angle = strand + u * Math.PI * 6 + turn;
          const out = (0.1 + 0.86 * u ** 1.2) * CITY;
          const y = (0.74 - 0.72 * u + 0.015 * Math.sin(time * 2 + step * 0.3)) * CITY;
          return toScreen([out * Math.cos(angle), y, out * Math.sin(angle)], camera, w, h);
        });
        ctx.strokeStyle = c;
        for (let step = 1; step < at.length; step++) {
          const a = at[step - 1];
          const b = at[step];
          if (!a || !b) continue;
          const fade = 1 - step / at.length;
          ctx.globalAlpha = (busy ? 0.85 : 0.5) * fade * (0.45 + 0.55 * ((b.z / CITY + 1) / 2));
          ctx.lineWidth = (0.6 + 2.4 * fade) * b.scale;
          ctx.beginPath();
          ctx.moveTo(a.sx, a.sy);
          ctx.lineTo(b.sx, b.sy);
          ctx.stroke();
        }
        const head = at[0];
        if (head) {
          ctx.globalAlpha = 1;
          stamp(c, head.sx, head.sy, 9 * head.scale);
        }
      }
    };

    const drawEdges = (t: Tokens, seen: Map<string, Projected>, now: number) => {
      const pin = pinnedRef.current;
      for (const e of netRef.current.edges) {
        const a = seen.get(e.from);
        const b = seen.get(e.to);
        if (!a || !b) continue;
        const touches = e.from === pin || e.to === pin || e.from === hoverId || e.to === hoverId;
        const lit = energy(b.node.position, waves, now).power;
        ctx.setLineDash(e.kind === 'ascends' ? [4, 6] : []);
        ctx.strokeStyle = tint(t, b.node.token);
        const rest = e.kind === 'seat' ? 0.32 : 0.42;
        ctx.globalAlpha = pin === undefined ? rest + 0.5 * lit : touches ? 0.95 : 0.06;
        ctx.lineWidth = touches ? 2 : e.kind === 'seat' ? 0.6 : 1;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.stroke();
      }
      ctx.setLineDash([]);
    };

    const drawSignals = (t: Tokens, seen: Map<string, Projected>, busy: boolean) => {
      const pin = pinnedRef.current;
      const all = netRef.current.edges;
      const on = pin === undefined ? all : all.filter((e) => e.from === pin || e.to === pin);
      if (on.length === 0) return;
      ctx.globalAlpha = 0.95;
      for (const s of signals) {
        s.t += busy ? 0.018 : 0.007;
        if (s.t >= 1) {
          s.t = 0;
          s.edge = Math.floor(Math.random() * on.length);
        }
        const e: ColonyEdge | undefined = on[s.edge % on.length];
        const a = e && seen.get(e.from);
        const b = e && seen.get(e.to);
        if (!a || !b) continue;
        const c = busy ? t.gold : tint(t, b.node.token);
        stamp(c, a.x + (b.x - a.x) * s.t, a.y + (b.y - a.y) * s.t, 5);
      }
    };

    const drawNodes = (t: Tokens, blend: GlobalCompositeOperation, now: number) => {
      const pin = pinnedRef.current;
      const { working } = netRef.current;
      const beat = 0.5 + 0.5 * Math.sin(now / 260);
      const lookedAt = (id: string) => id === pin || id === hoverId || reach.near.has(id);
      const hidden = crowdedLabels(
        projected
          .filter((p) => p.node.kind !== 'body')
          .map((p) => ({
            id: p.node.id,
            x: p.x,
            y: p.y + p.radius + 12,
            width: p.node.name.length * 6,
            pinned: lookedAt(p.node.id),
            near: p.depth,
          })),
      );
      for (const p of projected) {
        const n = p.node;
        const focus = n.id === pin || n.id === hoverId;
        const busy = working.has(n.id);
        const lit = energy(n.position, waves, now).power;
        const c = tint(t, n.token);
        const r = p.radius * (focus ? 1.5 : 1) * (1 + 0.25 * lit);
        const faded = pin !== undefined && n.id !== pin && !reach.near.has(n.id);
        const depth = (faded ? 0.16 : 1) * (0.55 + 0.45 * ((p.depth / CITY + 1) / 2));
        const halo = r * (busy ? 3.4 + beat : focus ? 3 : 2.2 + 1.6 * lit);
        ctx.globalCompositeOperation = blend;
        ctx.globalAlpha = (blend === 'lighter' ? 0.8 : 0.3) * depth;
        stamp(c, p.x, p.y, halo);
        ctx.globalCompositeOperation = 'source-over';
        ctx.globalAlpha = depth;
        ctx.fillStyle = t.panel;
        ctx.strokeStyle = n.id === pin ? t.holo : c;
        ctx.lineWidth = focus || busy ? 2 : 1;
        hexagon(p.x, p.y, r + 1.5);
        ctx.fill();
        ctx.stroke();
        const art = picture(n);
        if (art.complete && art.naturalWidth > 0) {
          ctx.save();
          hexagon(p.x, p.y, r);
          ctx.clip();
          ctx.drawImage(art, p.x - r, p.y - r, r * 2, r * 2);
          ctx.restore();
        }
        const named = lookedAt(n.id) || (n.kind !== 'body' && !faded && !hidden.has(n.id));
        if (named) {
          ctx.font = `${focus ? 13 : 11}px ${t.ui}`;
          ctx.textAlign = 'center';
          ctx.textBaseline = 'top';
          ctx.fillStyle = t.text;
          ctx.globalAlpha = lookedAt(n.id) ? 1 : 0.85 * depth;
          ctx.fillText(shown(n.name), p.x, p.y + r + 5);
        }
      }
      ctx.globalAlpha = 1;
    };

    const frame = (now: number) => {
      raf = requestAnimationFrame(frame);
      const dt = Math.min((now - last) / 1000, 0.05);
      last = now;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (w <= 0 || h <= 0 || document.hidden) return;
      const net = netRef.current;
      const still = reducedRef.current;
      const busy = net.working.size > 0;
      const pin = pinnedRef.current;
      if (reach.pin !== pin || reach.net !== net) {
        const near = related(pin, net.edges);
        const source = net.nodes.find((n) => n.id === pin);
        reach = { pin, net, near, cells: reachedCells(source, near, net.nodes) };
      }
      camera = spin(camera, dt, !still && !dragging, busy ? 0.2 : 0.1);
      if (!still && now - lastWave > (busy ? 1500 : 6500)) {
        lastWave = now;
        pulse(node('rimuru')?.position ?? [0, 0, 0]);
      }
      while (waves[0] && (now - waves[0].start) / 1000 > WAVE_LIFE_S) waves.shift();
      projected = paintOrder(net.nodes.map((n) => project(n, camera, w, h)));
      const seen = new Map(projected.map((p) => [p.node.id, p]));
      const t = tokens();
      const light = document.documentElement.dataset.theme === 'light';
      const blend: GlobalCompositeOperation = light ? 'source-over' : 'lighter';
      ctx.setTransform(dpr(), 0, 0, dpr(), 0, 0);
      ctx.clearRect(0, 0, w, h);
      ctx.globalCompositeOperation = blend;
      if (!light) drawStars(t, w, h, now / 1000);
      // Light adds up on the dark; on the light theme a lit corner is a small mark, not a glow.
      drawCity(t, w, h, now, light ? 1.6 : 2.2, light ? 1.8 : 4);
      if (!still) drawCoil(t, w, h, now / 1000, busy);
      drawEdges(t, seen, now);
      if (!still) drawSignals(t, seen, busy);
      drawNodes(t, blend, now);
    };

    /* ── interaction ── */

    const place = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      return [e.clientX - rect.left, e.clientY - rect.top] as const;
    };
    const at = (e: PointerEvent) => {
      const [x, y] = place(e);
      return nearest(projected, x, y, 28);
    };
    /** The corner of the city under a click: where a ripple starts. */
    const ground = (e: PointerEvent): Vec => {
      const [x, y] = place(e);
      let best: Vec = [0, 0, 0];
      let off = Number.POSITIVE_INFINITY;
      for (const pt of LATTICE.points) {
        const s = toScreen(pt, camera, canvas.clientWidth, canvas.clientHeight);
        const here = Math.hypot(s.sx - x, s.sy - y);
        if (here < off) {
          off = here;
          best = pt;
        }
      }
      return best;
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
      const hit = at(e)?.node;
      pulse(hit?.position ?? ground(e), hit?.token);
      onPinRef.current(hit?.id === pinnedRef.current ? undefined : hit?.id);
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
      aria-label="The colony as a 3D hex city: the arrow keys orbit, + and - zoom. The races are listed below the drawing, and open from there."
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

/** The net of one world, its bodies read live. */
function LiveNet({
  world,
  chooser,
  reduced,
}: {
  world: string;
  chooser: React.ReactNode;
  reduced: boolean;
}) {
  const load = useBodies(world);
  const rows = load.state === 'loading' ? undefined : load.data?.bodies;
  const net = useMemo(
    () =>
      rows
        ? netOf(
            rows.map((b) => ({
              name: b.name,
              race: b.race,
              retired: b.retired,
              working: b.live.state === 'working',
            })),
          )
        : STILL,
    [rows],
  );
  return <NetPage net={net} chooser={chooser} reduced={reduced} />;
}

export function NetView() {
  const reduced = useReducedMotion();
  const load = useWorlds();
  const worlds =
    load.state === 'ready' ? load.data.worlds.filter((w) => w.state === 'present') : [];
  const [chosen, setChosen] = useState<string | undefined>(undefined);
  const world = worlds.find((w) => w.id === chosen) ?? worlds[0];
  if (!world) return <NetPage net={STILL} chooser={undefined} reduced={reduced} />;
  const chooser =
    worlds.length > 1 ? (
      <label className="net-world">
        World{' '}
        <select value={world.id} onChange={(e) => setChosen(e.target.value)}>
          {worlds.map((w) => (
            <option key={w.id} value={w.id}>
              {shown(w.name)}
            </option>
          ))}
        </select>
      </label>
    ) : undefined;
  return <LiveNet key={world.id} world={world.id} chooser={chooser} reduced={reduced} />;
}

function NetPage({
  net,
  chooser,
  reduced,
}: {
  net: Net;
  chooser: React.ReactNode;
  reduced: boolean;
}) {
  const [pinned, setPinned] = useState<string | undefined>(undefined);
  const [hovered, setHovered] = useState<string | undefined>(undefined);
  const find = (id: string | undefined) => net.nodes.find((n) => n.id === id);
  const card = find(pinned);
  const over = find(hovered);
  const commands = EDGES.filter((e) => e.kind === 'command').length;
  const bodies = net.nodes.length - NODES.length;

  return (
    <div className="page">
      <header className="page-head">
        <h1 className="page-title">Colony net</h1>
        {chooser}
      </header>
      <p className="page-sub">
        The colony as a city around the throne: the six races and the five ascended forms, the same
        in every world
        {bodies > 0 ? `, and this world's ${bodies} living bodies, seated around their race` : ''}.
        Pick a race to light what it reaches.
      </p>
      <section className="net-stage" aria-label="the net, drawn">
        <NetCanvas
          net={net}
          pinned={pinned}
          onPin={setPinned}
          onHover={setHovered}
          reduced={reduced}
        />
        <p className="net-hint">
          {over ? `${shown(over.name)} — ${over.job}` : HINT}
          {reduced ? ' · still: motion reduced' : ' · turning'}
        </p>
        {card && (
          <aside className="net-card" style={{ borderLeftColor: colour(card) }} aria-live="polite">
            <button
              type="button"
              className="net-card-close"
              onClick={() => setPinned(undefined)}
              aria-label={`Close ${shown(card.name)}'s card`}
            >
              ×
            </button>
            <span className="net-card-kind">{kindWords(card)}</span>
            <h2 className="net-card-name">{shown(card.name)}</h2>
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
