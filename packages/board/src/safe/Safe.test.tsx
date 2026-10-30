import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { safeHref } from './links.ts';
import { Safe, SafeImage, SafeLink } from './Safe.tsx';

const html = (node: React.ReactElement) => renderToStaticMarkup(node);

describe('Safe', () => {
  it('renders markup as text, never as elements', () => {
    const out = html(<Safe text={'<img src=x onerror="alert(1)"><script>alert(2)</script>'} />);
    expect(out).not.toContain('<img');
    expect(out).not.toContain('<script');
    expect(out).toContain('&lt;img src=x onerror=&quot;alert(1)&quot;&gt;');
  });
  it('sets every escape apart, so a reversed name reads as written', () => {
    const out = html(<Safe text={'evil\u202egnp.exe'} />);
    expect(out).not.toContain('\u202e');
    expect(out).toContain('<span class="safe-esc"');
    expect(out).toContain('\\u{202e}</span>gnp.exe');
  });
  it('drops no character and adds none but escapes', () => {
    const out = html(<Safe text={'a\u0000b'} />);
    expect(out.replace(/<[^>]+>/g, '')).toBe('a\\u{0}b');
  });
  it('keeps newlines in prose', () => {
    expect(html(<Safe text={'a\nb'} lines />)).toContain('a\nb');
  });
});

describe('links', () => {
  it.each([
    ['https://example.com/a?b#c', 'https://example.com/a?b#c'],
    ['http://127.0.0.1:7878/', 'http://127.0.0.1:7878/'],
    ['mailto:someone@example.com', 'mailto:someone@example.com'],
  ])('follows %s', (raw, want) => expect(safeHref(raw)).toBe(want));

  it.each([
    'javascript:alert(1)',
    'JaVaScRiPt:alert(1)',
    ' javascript:alert(1)',
    'java\nscript:alert(1)',
    'data:text/html,<script>alert(1)</script>',
    'vbscript:msgbox(1)',
    'file:///etc/passwd',
    'blob:https://example.com/x',
    '//evil.example/x',
    '/relative',
    'https://bank.example@evil.example/',
    'https://user:pass@example.com/',
    '',
  ])('refuses %j', (raw) => expect(safeHref(raw)).toBeUndefined());

  it('opens a followed link with no opener and no referrer', () => {
    const out = html(<SafeLink href="https://example.com" text="docs" />);
    expect(out).toBe(
      '<a href="https://example.com/" rel="noopener noreferrer" target="_blank"><span class="safe">docs</span></a>',
    );
  });
  it('shows a refused link as text, with the reason', () => {
    // biome-ignore lint/security/noScriptUrl: the hostile link under test
    const out = html(<SafeLink href="javascript:alert(1)" text="click" />);
    expect(out).not.toContain('<a');
    expect(out).toContain('link not followed');
  });
});

describe('images', () => {
  it('never loads one: no img element, its description as text', () => {
    const out = html(<SafeImage src="https://tracker.example/p.gif" alt="a chart" />);
    expect(out).not.toContain('<img');
    expect(out).not.toContain('tracker.example');
    expect(out).toContain('a chart');
  });
  it('shows the address as text when there is no description', () => {
    const out = html(<SafeImage src="https://tracker.example/p.gif" />);
    expect(out).not.toContain('<img');
    expect(out).toContain('https://tracker.example/p.gif');
  });
});
