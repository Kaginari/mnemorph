import '@mnemorph/theme/tokens.css';
import './board.css';
import './chat.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App.tsx';
import { takeKey } from './data/session.ts';
import { apply, stored } from './theme.ts';

// The key leaves the address bar before anything reads it; the theme is set before the first paint, so a light system never flashes dark.
takeKey();
apply(stored());

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
