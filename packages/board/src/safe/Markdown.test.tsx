import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { tokens } from './Code.tsx';
import { blocks, inlines, Markdown } from './Markdown.tsx';

const html = (text: string) => renderToStaticMarkup(<Markdown text={text} />);

describe('the Markdown subset', () => {
  it('reads paragraphs, headings, lists, quotes, rules and fences', () => {
    expect(
      blocks('# T\n\npara\nmore\n\n- a\n- b\n\n1. x\n\n> q\n\n---\n\n```rs\nfn f() {}\n```').map(
        (b) => b.kind,
      ),
    ).toEqual(['h', 'p', 'list', 'list', 'quote', 'rule', 'code']);
  });

  it('reads emphasis, code, links and images', () => {
    expect(
      inlines('**b** *i* `c` ~~d~~ [l](https://x.example) ![a](https://x.example/p.png)').map(
        (x) => x.kind,
      ),
    ).toEqual([
      'strong',
      'text',
      'em',
      'text',
      'code',
      'text',
      'del',
      'text',
      'link',
      'text',
      'image',
    ]);
    expect(inlines('snake_case_name').map((x) => x.kind)).toEqual(['text']);
  });

  it('keeps raw HTML as text', () => {
    const out = html(
      '<img src=x onerror="alert(1)"> <script>alert(2)</script> <a href="javascript:x">y</a>',
    );
    expect(out).not.toMatch(/<img|<script|<a /);
    expect(out).toContain('&lt;img src=x');
  });

  it('follows only http, https and mailto, and never loads an image', () => {
    const out = html(
      '[ok](https://example.com) [bad](javascript:alert(1)) [data](data:text/html,x) ![i](https://evil.example/beacon.png)',
    );
    expect(out).toContain('href="https://example.com/"');
    expect(out).toContain('rel="noopener noreferrer"');
    expect(out).not.toContain('javascript:alert(1)"');
    expect(out).not.toMatch(/href="(javascript|data):/);
    expect(out).not.toContain('<img');
    expect(out).toContain('image not loaded');
  });

  it('escapes bidi and terminal escapes', () => {
    const out = html('name ‮gnp.exe and \u001b[31mred\u001b[0m');
    expect(out).not.toContain('‮');
    expect(out).not.toContain('\u001b');
    expect(out).toContain('\\u{202e}');
    expect(out).toContain('\\u{1b}');
  });

  it('parses hostile depth and length without falling over', () => {
    expect(() => html(`${'> '.repeat(5000)}x`)).not.toThrow();
    expect(() => html('*a'.repeat(50_000))).not.toThrow();
    expect(() => html('['.repeat(50_000))).not.toThrow();
  });
});

describe('the code highlighter', () => {
  it('gives every character back, its kinds named', () => {
    const src = 'fn main() { let s = "x"; // hi\n 42 }';
    const ts = tokens(src, 'rust');
    expect(ts.map(([, s]) => s).join('')).toBe(src);
    expect(ts.filter(([k]) => k !== 'plain').map(([k, s]) => [k, s.trim()])).toEqual([
      ['key', 'fn'],
      ['key', 'let'],
      ['str', '"x"'],
      ['com', '// hi'],
      ['num', '42'],
    ]);
  });
  it('reads a hash comment in a shell', () => {
    expect(tokens('ls # x', 'sh').at(-1)).toEqual(['com', '# x']);
  });
});
