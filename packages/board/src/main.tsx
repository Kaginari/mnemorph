import '@mnemorph/theme/tokens.css';
import './board.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App.tsx';
import { apply, stored } from './theme.ts';

// The theme is set before the first paint, so a light system never flashes dark.
apply(stored());

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
