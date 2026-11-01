// The last pass over a flattened canvas SVG: each embedded font is cut down to the characters its
// labels use (tldraw embeds whole faces, ~150 KB each), a face no label uses is dropped, and the
// editor's own attributes (classes, data-*) go. What is left is paths, plain <text> and one
// <style> of @font-face rules whose fonts are data: URLs, so nothing is fetched at render time.
import subsetFont from 'subset-font';

const FACE = /@font-face\s*\{([^}]*)\}/g;

function faceKey(family, weight = '400', style = 'normal') {
  const w = { normal: '400', bold: '700' }[weight] ?? weight;
  return `${family.replace(/["']/g, '').trim()}|${w}|${style}`;
}

// family|weight|style → the characters drawn in it
function usedCharacters(svg) {
  const used = new Map();
  for (const m of svg.matchAll(/<text\b([^>]*)>([^<]*)<\/text>/g)) {
    const attr = (name) => m[1].match(new RegExp(`\\b${name}="([^"]*)"`))?.[1];
    const family = (attr('font-family') ?? '').split(',')[0];
    const key = faceKey(family, attr('font-weight'), attr('font-style'));
    const text = m[2]
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/&quot;/g, '"')
      .replace(/&#39;|&apos;/g, "'")
      .replace(/&amp;/g, '&');
    used.set(key, (used.get(key) ?? '') + text);
  }
  return used;
}

export async function finishSvg(svg, title) {
  const used = usedCharacters(svg);
  const faces = [];
  for (const [, body] of svg.matchAll(FACE)) {
    const prop = (name) => body.match(new RegExp(`${name}:\\s*([^;]+);`))?.[1].trim();
    const key = faceKey(prop('font-family') ?? '', prop('font-weight'), prop('font-style'));
    const text = used.get(key);
    const data = body.match(/url\("data:font\/woff2;base64,([^"]+)"\)/)?.[1];
    if (!text || !data) continue;
    const subset = await subsetFont(Buffer.from(data, 'base64'), text, { targetFormat: 'woff2' });
    const [family, weight, style] = key.split('|');
    faces.push(
      `@font-face{font-family:"${family}";font-weight:${weight};font-style:${style};` +
        `src:url("data:font/woff2;base64,${subset.toString('base64')}") format("woff2")}`,
    );
  }
  let out = svg
    .replace(/<style>[\s\S]*?<\/style>/g, '')
    .replace(/<defs>\s*<\/defs>/g, '')
    .replace(/\s(?:class|data-[\w-]+|direction)="[^"]*"/g, '')
    .replace(/url\(&quot;(#[^&]+)&quot;\)/g, 'url($1)');
  // The export's ids carry a per-run counter: renumbered, the same canvas makes the same file.
  const ids = new Map();
  for (const [, id] of out.matchAll(/\sid="([^"]+)"/g)) ids.set(id, `c${ids.size}`);
  out = out.replace(
    /(\sid="|url\(#|href="#)([^")]+)/g,
    (_match, lead, id) => `${lead}${ids.get(id) ?? id}`,
  );
  const open = out.indexOf('>', out.indexOf('<svg')) + 1;
  const escaped = title.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  out = `${out.slice(0, open)}<title>${escaped}</title><style>${faces.join('')}</style>${out.slice(open)}`;
  return `${out}\n`;
}
