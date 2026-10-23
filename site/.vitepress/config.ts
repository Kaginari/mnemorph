import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitepress';
import { withMermaid } from 'vitepress-plugin-mermaid';

// The book lives in docs/ and stays readable on GitHub as it is; the site only renders it.
// Its pages sit outside this package, so `vue` is a root devDependency: resolution from docs/
// reaches the root node_modules under pnpm's isolated layout.
export default withMermaid(
  defineConfig({
    title: 'Tempest',
    description: 'An agent harness grown from isekai: Rust core, TypeScript CLI and board.',
    srcDir: '../docs',
    rewrites: { 'README.md': 'index.md' },
    cleanUrls: true,
    // The race portraits are the site's public files: Rimuru is the logo and the favicon.
    vite: { publicDir: fileURLToPath(new URL('../../assets/portraits', import.meta.url)) },
    head: [['link', { rel: 'icon', type: 'image/png', href: '/rimuru@32.png' }]],
    lastUpdated: true,
    themeConfig: {
      logo: '/rimuru@64.png',
      nav: [
        { text: 'Book', link: '/' },
        { text: 'Roadmap', link: '/roadmap' },
        { text: 'GitHub', link: 'https://github.com/Kaginari/tempest' },
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
            { text: 'Roadmap', link: '/roadmap' },
          ],
        },
        {
          text: 'Reference',
          items: [
            { text: 'Lessons from isekai', link: '/lessons' },
            { text: 'Race cards brief', link: '/race-cards-prompt' },
            { text: 'Race icons prompts', link: '/race-icons-prompt' },
          ],
        },
      ],
      outline: { level: [2, 3] },
      search: { provider: 'local' },
    },
    mermaid: { look: 'handDrawn', theme: 'neutral' },
  }),
);
