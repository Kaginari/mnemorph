/** How long ago `iso` was, short: `now`, `5m`, `2h`, `3d`, then the date. */
export function ago(iso: string, now: number): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return 'unknown';
  const s = Math.max(0, Math.floor((now - t) / 1000));
  if (s < 60) return 'now';
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 86400 * 30) return `${Math.floor(s / 86400)}d`;
  return iso.slice(0, 10);
}

/** When `iso` was, for "missing since …": the day, and how long ago while it is recent. */
export function since(iso: string, now: number): string {
  const day = iso.slice(0, 10);
  const a = ago(iso, now);
  return a === day || a === 'unknown' ? day : `${day} (${a} ago)`;
}

/** `text` as one shell word: as it is when it is plain, else in single quotes. */
export function shellWord(text: string): string {
  return /^[A-Za-z0-9_./~@%+=:,-]+$/.test(text) ? text : `'${text.replace(/'/g, `'\\''`)}'`;
}
