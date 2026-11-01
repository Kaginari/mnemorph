/**
 * The page a tab without a board session shows in place of the board: why, and the one thing to
 * do. Each link `mnemorphd board` prints works once, in one browser, so a link another browser
 * opened first (VS Code's, say) is spent here.
 */
import type { Access } from '../data/session.ts';
import { Wordmark } from '../ui/Wordmark.tsx';

export function SignedOut({ why }: { why: Exclude<Access, 'open'> }) {
  return (
    <main className="signed-out" aria-labelledby="signed-out-title">
      <div className="signed-out-card">
        <Wordmark />
        {why === 'link-used' ? (
          <>
            <h1 id="signed-out-title" className="signed-out-title">
              This link was already used
            </h1>
            <p>
              Perhaps by another browser, such as VS Code’s. Run{' '}
              <code className="mono">mnemorphd board</code> for a new one, and open it in this
              browser.
            </p>
          </>
        ) : (
          <>
            <h1 id="signed-out-title" className="signed-out-title">
              This tab isn’t signed in
            </h1>
            <p>
              Run <code className="mono">mnemorphd board</code> and open the link it prints, in this
              browser. Each link works once.
            </p>
          </>
        )}
        <p className="hint">
          The board answers at <code className="mono">127.0.0.1</code>,{' '}
          <code className="mono">[::1]</code> and <code className="mono">localhost</code>; each
          signs in on its own.
        </p>
      </div>
    </main>
  );
}
