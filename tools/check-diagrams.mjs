// Parses every Mermaid diagram in the book with Mermaid itself, so a diagram GitHub cannot
// render fails the check instead of the page (a `;` in a sequence message ends the statement).
import { readdirSync, readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';

const dom = new JSDOM('<!doctype html><html><body></body></html>');
globalThis.window = dom.window;
globalThis.document = dom.window.document;
const { default: mermaid } = await import('mermaid');
mermaid.initialize({ startOnLoad: false });

const dir = 'docs';
let count = 0;
const failures = [];
for (const file of readdirSync(dir).filter((f) => f.endsWith('.md'))) {
  const text = readFileSync(`${dir}/${file}`, 'utf8');
  for (const match of text.matchAll(/```mermaid\n([\s\S]*?)```/g)) {
    count++;
    const line = text.slice(0, match.index).split('\n').length;
    try {
      await mermaid.parse(match[1]);
    } catch (error) {
      const message = String(error?.message ?? error)
        .split('\n')
        .slice(0, 3)
        .join(' | ');
      failures.push(`${dir}/${file}:${line}: ${message}`);
    }
  }
}
if (failures.length) {
  console.error(`diagram check failed:\n  ${failures.join('\n  ')}`);
  process.exit(1);
}
console.log(`diagram check: ${count} diagrams parse`);
