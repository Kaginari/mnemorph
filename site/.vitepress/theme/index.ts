import DefaultTheme from 'vitepress/theme';
import '@mnemorph/theme/tokens.css';
import './style.css';

// VitePress marks dark mode with `html.dark`; the tokens read `data-theme`. Mirror the one onto
// the other, so the appearance toggle switches the whole theme.
export default {
  extends: DefaultTheme,
  enhanceApp() {
    if (typeof document === 'undefined') return;
    const root = document.documentElement;
    const sync = () =>
      root.setAttribute('data-theme', root.classList.contains('dark') ? 'dark' : 'light');
    sync();
    new MutationObserver(sync).observe(root, { attributes: true, attributeFilter: ['class'] });
  },
};
