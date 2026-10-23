import { createRequire } from 'node:module';
import { dirname } from 'node:path';
import { defineConfig } from 'vitepress';
import { withMermaid } from 'vitepress-plugin-mermaid';

const require = createRequire(import.meta.url);

// The book lives in docs/ and stays readable on GitHub as it is; the site only renders it.
export default withMermaid(
  defineConfig({
    title: 'Tempest',
    description: 'An agent harness grown from isekai: Rust core, TypeScript CLI and board.',
    srcDir: '../docs',
    rewrites: { 'README.md': 'index.md' },
    cleanUrls: true,
    lastUpdated: true,
    themeConfig: {
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
    // The pages sit outside this package, where pnpm's strict node_modules has no vue:
    // resolve it from here.
    vite: { resolve: { alias: { vue: dirname(require.resolve('vue/package.json')) } } },
    mermaid: { look: 'handDrawn', theme: 'neutral' },
  }),
);
