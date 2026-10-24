/** mnemorph — the terminal client. */
export const CLI_NAME = 'mnemorph';
export const VERSION = '0.0.0';
// Stamped by the bundler: a stale build is never judged as the new one.
declare const __MNEMORPH_COMMIT__: string | undefined;
/** The commit this build was made from; `source` when run unbundled. */
export const COMMIT = typeof __MNEMORPH_COMMIT__ === 'string' ? __MNEMORPH_COMMIT__ : 'source';
/** The version and its commit: `0.0.0 (a1b2c3d4e5)`. */
export const RELEASE = `${VERSION} (${COMMIT})`;
export { parse } from './args.js';
export { fold, initial } from './transcript.js';
