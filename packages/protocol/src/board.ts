/**
 * The board's HTTP API, as the page reads it. Written by hand, not generated: the board server
 * (the board server) owns these shapes, and its tests and the board's fixtures keep them equal.
 */

/** How a world stands right now. */
export type WorldStatus =
  /** A session served by the board is running in it. */
  | 'live'
  /** A terminal session is running in it, seen through its run journal. */
  | 'tui-live'
  /** Known and present, nothing running. */
  | 'idle'
  /** Its directory is gone; it stays listed, never silently dropped. */
  | 'missing';

/** One world on this machine, folded from the world registry (`worlds.jsonl`). */
export interface WorldSummary {
  /** Its id across the state directory: `<name>-<fnv64a of the path>`. */
  id: string;
  /** Its directory's name. */
  name: string;
  /** Its canonical path. */
  world: string;
  /** Its `origin` remote, normalised (`host/owner/repo`), when it has one. */
  remote?: string;
  /** When a session first opened in it, RFC 3339 in UTC. */
  first: string;
  /** When the latest did, RFC 3339 in UTC. */
  last: string;
  /** How many sessions opened in it. */
  sessions: number;
  status: WorldStatus;
}

/** `GET /api/worlds`. */
export interface WorldsResponse {
  worlds: WorldSummary[];
}

/** Every status a world can have, in the order the board sorts them. */
export const WORLD_STATUSES: readonly WorldStatus[] = ['live', 'tui-live', 'idle', 'missing'];
