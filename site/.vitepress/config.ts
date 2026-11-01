import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitepress';
import { withMermaid } from 'vitepress-plugin-mermaid';

// The book lives in docs/ and stays readable on GitHub as it is; the site only renders it.
// Its pages sit outside this package, so `vue` is a root devDependency: resolution from docs/
// reaches the root node_modules under pnpm's isolated layout.
export default withMermaid(
  defineConfig({
    title: 'Mnemorph',
    description:
      'An agent harness grown from isekai: Rust core and terminal app, TypeScript board.',
    srcDir: '../docs',
    rewrites: { 'README.md': 'index.md' },
    // Local-only notes (ignored by git) never reach the site.
    srcExclude: ['study-maki.md'],
    cleanUrls: true,
    // Formulas are written as ```math fences: GitHub renders those untouched (a `$$` block is
    // parsed as Markdown first, so `\,` loses its backslash and a line starting with `+` becomes a
    // list). Here each fence goes to the same MathJax renderer `$$` uses.
    markdown: {
      math: true,
      config(md) {
        const fence = md.renderer.rules.fence;
        const mathBlock = md.renderer.rules.math_block;
        md.renderer.rules.fence = (tokens, idx, options, env, self) =>
          tokens[idx].info.trim() === 'math' && mathBlock
            ? mathBlock(tokens, idx, options, env, self)
            : fence
              ? fence(tokens, idx, options, env, self)
              : self.renderToken(tokens, idx, options);
      },
    },
    // The brand's files are the site's public files: the emblem is the logo and the favicon.
    vite: { publicDir: fileURLToPath(new URL('../../assets/brand', import.meta.url)) },
    head: [
      ['link', { rel: 'icon', href: '/favicon.ico', sizes: 'any' }],
      ['link', { rel: 'icon', type: 'image/png', href: '/mnemorph-icon@64.png' }],
      ['link', { rel: 'preconnect', href: 'https://fonts.googleapis.com' }],
      [
        'link',
        {
          rel: 'stylesheet',
          href: 'https://fonts.googleapis.com/css2?family=Cinzel:wght@600;700&family=Rajdhani:wght@600;700&family=JetBrains+Mono:wght@400;600&display=swap',
        },
      ],
    ],
    // Dark first, like the banner; the toggle still offers light.
    appearance: 'dark',
    lastUpdated: true,
    themeConfig: {
      logo: '/mnemorph-icon@64.png',
      nav: [
        { text: 'Book', link: '/' },
        { text: 'Roadmap', link: '/roadmap' },
        { text: 'GitHub', link: 'https://github.com/Kaginari/mnemorph' },
      ],
      sidebar: [
        {
          text: 'The book',
          items: [
            { text: 'Overview', link: '/01-overview' },
            { text: 'Races', link: '/02-races' },
            { text: 'The loop', link: '/03-loop' },
            { text: 'Communication', link: '/04-communication' },
            { text: 'Architecture', link: '/05-architecture' },
            { text: 'Consumption', link: '/06-consumption' },
            { text: 'MCP', link: '/07-mcp' },
            { text: 'Memory', link: '/08-memory' },
            { text: 'Ontology', link: '/09-ontology' },
            { text: 'Roadmap', link: '/roadmap' },
          ],
        },
        {
          text: 'Reference',
          items: [
            { text: 'Lessons from isekai', link: '/lessons' },
            { text: 'Releasing', link: '/releasing' },
            { text: 'Race cards brief', link: '/race-cards-prompt' },
            { text: 'Research: DIKWP-MESH', link: '/research/dikwp-mesh' },
          ],
        },
      ],
      outline: { level: [2, 3] },
      search: { provider: 'local' },
    },
    mermaid: { look: 'handDrawn', theme: 'neutral' },
  }),
);
