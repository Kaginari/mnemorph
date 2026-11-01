// The drawing page draw.mjs opens in headless Chromium: a bare tldraw editor (no persistence key,
// so nothing is cached in the browser), its fonts and icons served from @tldraw/assets, never a
// CDN. window.mnDraw lays out one book page's sketches with tldraw's own Mermaid converter, one
// tldraw page per sketch, and hands back the .tldr document and each sketch's light and dark SVG.
import { getAssetUrls } from '@tldraw/assets/selfHosted';
import { createMermaidDiagram } from '@tldraw/mermaid';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { getSnapshot, parseTldrawJsonFile, serializeTldrawJson, Tldraw } from 'tldraw';
import 'tldraw/tldraw.css';

const assetUrls = getAssetUrls({ baseUrl: '/tldraw-assets' });
const DOCUMENT_RECORDS = new Set(['document', 'page', 'shape', 'binding', 'asset']);
const DIGITS = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
let blank;

// The nth fractional index (n from 1): a1 … az, then b10 … bzz — tldraw's own are jittered.
function nthIndex(n) {
  return n < 62 ? `a${DIGITS[n]}` : `b${DIGITS[Math.floor(n / 62)]}${DIGITS[n % 62]}`;
}

// The document records only (no user, camera, pointer or selection), with fresh ids and stacking
// indices given in an order read from their content (tldraw's own are random), so the same
// sketches always make the same file.
function tidy(json) {
  const file = JSON.parse(json);
  const records = file.records.filter((record) => DOCUMENT_RECORDS.has(record.typeName));
  const of = (typeName) => records.filter((record) => record.typeName === typeName);
  const byKey = (key) => (a, b) => (key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0);
  const ids = new Map();
  const rename = (list, kind) => {
    for (const [n, record] of list.entries()) {
      ids.set(record.id, `${kind}:${String(n + 1).padStart(3, '0')}`);
    }
    return list;
  };

  // Siblings keep their stacking order under new evenly spaced indices.
  const shapes = of('shape');
  const siblings = Map.groupBy(shapes, (shape) => shape.parentId);
  for (const group of siblings.values()) {
    group.sort(byKey((shape) => shape.index));
    for (const [n, shape] of group.entries()) shape.index = nthIndex(n + 1);
  }
  const pages = rename(of('page').sort(byKey((page) => page.index)), 'page');
  for (const [n, page] of pages.entries()) page.index = nthIndex(n + 1);
  // A shape's key is where it sits (its page, then each enclosing group) and what it draws,
  // a group's what its children draw.
  const byId = new Map(shapes.map((shape) => [shape.id, shape]));
  const drawn = new Map();
  const draws = (shape) => {
    if (!drawn.has(shape.id)) {
      const children = (siblings.get(shape.id) ?? []).map(draws).sort();
      drawn.set(shape.id, JSON.stringify([shape.type, shape.x, shape.y, shape.props, children]));
    }
    return drawn.get(shape.id);
  };
  const place = (id) =>
    byId.has(id)
      ? [...place(byId.get(id).parentId), byId.get(id).x, byId.get(id).y]
      : [ids.get(id)];
  const key = (shape) => JSON.stringify([place(shape.parentId), draws(shape)]);
  rename(shapes.sort(byKey(key)), 'shape');
  const bindings = rename(
    of('binding').sort(
      byKey((b) => JSON.stringify([ids.get(b.fromId), ids.get(b.toId), b.type, b.props])),
    ),
    'binding',
  );
  const assets = rename(of('asset').sort(byKey((asset) => JSON.stringify(asset.props))), 'asset');
  file.records = [...of('document'), ...pages, ...shapes, ...bindings, ...assets];
  return JSON.stringify(file, null, '\t').replace(
    /"((?:page|shape|binding|asset):[\w-]+)"/g,
    (match, id) => (ids.has(id) ? `"${ids.get(id)}"` : match),
  );
}

window.mnDraw = async (sketches) => {
  const editor = window.mnEditor;
  editor.loadSnapshot(blank);
  for (const [index, sketch] of sketches.entries()) {
    if (index === 0) {
      editor.updatePage({ id: editor.getCurrentPageId(), name: sketch.title });
    } else {
      editor.createPage({ name: sketch.title });
      editor.setCurrentPage(editor.getPages().at(-1).id);
    }
    await createMermaidDiagram(editor, sketch.mermaid, {
      blueprintRender: { position: { x: 0, y: 0 }, centerOnPosition: false },
    });
    // tldraw's hand-drawn stroke for every solid line; dotted and dashed edges keep their meaning.
    editor.updateShapes(
      editor
        .getCurrentPageShapesSorted()
        .filter((shape) => shape.props.dash === 'solid')
        .map((shape) => ({ id: shape.id, type: shape.type, props: { dash: 'draw' } })),
    );
  }

  // The file is read back the way any tldraw app opens it, and the SVGs are drawn from that copy.
  const tldr = tidy(await serializeTldrawJson(editor));
  const parsed = parseTldrawJsonFile({ json: tldr, schema: editor.store.schema });
  if (!parsed.ok) throw new Error(`the .tldr does not read back: ${JSON.stringify(parsed.error)}`);
  editor.loadSnapshot(getSnapshot(parsed.value));

  const svgs = [];
  for (const page of editor.getPages()) {
    editor.setCurrentPage(page.id);
    const ids = [...editor.getCurrentPageShapeIds()];
    const out = {};
    for (const darkMode of [false, true]) {
      const svg = await editor.getSvgString(ids, { background: true, darkMode, padding: 32 });
      out[darkMode ? 'dark' : 'light'] = svg.svg;
    }
    svgs.push(out);
  }
  return { tldr, svgs };
};

createRoot(document.getElementById('root')).render(
  createElement(Tldraw, {
    assetUrls,
    onMount(editor) {
      blank = getSnapshot(editor.store);
      window.mnEditor = editor;
    },
  }),
);
