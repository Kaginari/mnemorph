/**
 * The protocol between mnemorphd and its clients. The types are generated from the Rust crates
 * (`crates/mnemorph-server/src/protocol.rs`, and the board's API in `src/board/mod.rs`) and checked
 * in; the check fails when they drift.
 */
export type { BoardRole } from './generated/BoardRole.js';
export type { BoardSession } from './generated/BoardSession.js';
export type { BoardSessions } from './generated/BoardSessions.js';
export type { BoardStarted } from './generated/BoardStarted.js';
export type { BoardWorld } from './generated/BoardWorld.js';
export type { BoardWorlds } from './generated/BoardWorlds.js';
export type { Budget } from './generated/Budget.js';
export type { Envelope } from './generated/Envelope.js';
export type { Event } from './generated/Event.js';
export type { EventsParams } from './generated/EventsParams.js';
export type { EventsResult } from './generated/EventsResult.js';
export type { GateReply } from './generated/GateReply.js';
export type { GateRequest } from './generated/GateRequest.js';
export type { Hello } from './generated/Hello.js';
export type { PromptParams } from './generated/PromptParams.js';
export type { PromptResult } from './generated/PromptResult.js';
export type { Role } from './generated/Role.js';
export type { JsonValue } from './generated/serde_json/JsonValue.js';
export type { WorldState } from './generated/WorldState.js';

import type { Envelope } from './generated/Envelope.js';
import type { Event } from './generated/Event.js';

/** The protocol's version: a client that speaks another stops rather than misread. */
export const PROTOCOL = 1;

/** An event as it arrives: its own fields, and the envelope's. */
export type Arrived = Event & Envelope;

/** The type an `event/<type>` method names. */
export type EventType = Event['type'];

/** The method an event arrives under. */
export function eventMethod(type: EventType): string {
  return `event/${type}`;
}

/** The type an `event/<type>` method carries, or `undefined` for another method. */
export function eventType(method: string): EventType | undefined {
  return method.startsWith('event/') ? (method.slice(6) as EventType) : undefined;
}
