// tldraw exports each label as HTML inside a <foreignObject>. The book's canvases carry no HTML, so
// every label is transcribed into plain SVG <text>: the exported SVG is opened alone in Chromium,
// which lays its labels out exactly as an image viewer would, and each laid-out line becomes one
// <text> at the line's measured place, in the label's font, size, weight and colour. A thin stroke
// in the canvas's background colour keeps a label legible where an arrow crosses it, as tldraw's
// own text outline does. Runs in the browser (page.evaluate), so it is self-contained.
export function flattenLabels() {
  const SVG_NS = 'http://www.w3.org/2000/svg';
  const svg = document.querySelector('svg');
  const background = getComputedStyle(svg).backgroundColor;
  const segmenter = new Intl.Segmenter();
  const canvas = document.createElement('canvas').getContext('2d');

  for (const fo of [...svg.querySelectorAll('foreignObject')]) {
    const parent = fo.parentNode;
    const toLocal = parent.getScreenCTM().inverse();
    const scale = Math.hypot(toLocal.a, toLocal.b);
    const group = document.createElementNS(SVG_NS, 'g');
    const walker = document.createTreeWalker(fo, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      if (!node.data.trim()) continue;
      const style = getComputedStyle(node.parentElement);
      canvas.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      const ascent = canvas.measureText('Hg').fontBoundingBoxAscent;
      const lines = [];
      let offset = 0;
      for (const { segment } of segmenter.segment(node.data)) {
        const range = document.createRange();
        range.setStart(node, offset);
        range.setEnd(node, offset + segment.length);
        offset += segment.length;
        const rect = [...range.getClientRects()].find((r) => r.width > 0);
        if (!rect) continue;
        let line = lines.at(-1);
        if (!line || Math.abs(rect.top - line.top) > rect.height / 2) {
          if (!segment.trim()) continue;
          line = { top: rect.top, left: rect.left, text: '' };
          lines.push(line);
        }
        line.text += segment;
      }
      for (const line of lines) {
        const at = new DOMPoint(line.left, line.top + ascent).matrixTransform(toLocal);
        const text = document.createElementNS(SVG_NS, 'text');
        text.setAttribute('x', at.x.toFixed(1));
        text.setAttribute('y', at.y.toFixed(1));
        text.setAttribute('font-family', style.fontFamily);
        text.setAttribute('font-size', (Number.parseFloat(style.fontSize) * scale).toFixed(2));
        if (style.fontWeight !== '400') text.setAttribute('font-weight', style.fontWeight);
        if (style.fontStyle !== 'normal') text.setAttribute('font-style', style.fontStyle);
        text.setAttribute('fill', style.color);
        text.setAttribute('stroke', background);
        text.setAttribute('stroke-width', '3');
        text.setAttribute('paint-order', 'stroke');
        text.setAttribute('xml:space', 'preserve');
        text.textContent = line.text.trimEnd();
        group.append(text);
      }
    }
    parent.replaceChild(group, fo);
  }
  return new XMLSerializer().serializeToString(svg);
}
