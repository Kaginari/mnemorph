/** Finding the core to start. */
import { accessSync, constants } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The mnemorphd to start: `--core`, else `MNEMORPH_CORE`, else — when this runs from the
 * source checkout, not an install — the one built there (release first), else `mnemorphd` on
 * the PATH. */
export function findCore(flag?: string, env: NodeJS.ProcessEnv = process.env): string {
  if (flag) return resolve(flag);
  if (env.MNEMORPH_CORE) return resolve(env.MNEMORPH_CORE);
  const here = dirname(fileURLToPath(import.meta.url));
  // Only from the checkout: installed under node_modules, three levels up is someone's project.
  if (here.includes(`${'node_modules'}`)) return 'mnemorphd';
  for (const profile of ['release', 'debug']) {
    const built = resolve(here, '../../..', 'target', profile, 'mnemorphd');
    if (runnable(built)) return built;
  }
  return 'mnemorphd';
}

function runnable(path: string): boolean {
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}
