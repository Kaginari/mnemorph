/**
 * The protocol between mnemorphd and its clients. The types are generated from the Rust crates
 * (`crates/mnemorph-server/src/protocol.rs`, and the board's API in `src/board/mod.rs`) and checked
 * in; the check fails when they drift.
 */
export type { Activity } from './generated/Activity.js';
export type { Bench } from './generated/Bench.js';
export type { BenchRow } from './generated/BenchRow.js';
export type { BoardBodies } from './generated/BoardBodies.js';
export type { BoardBody } from './generated/BoardBody.js';
export type { BoardDone } from './generated/BoardDone.js';
export type { BoardFact } from './generated/BoardFact.js';
export type { BoardFactStatus } from './generated/BoardFactStatus.js';
export type { BoardFacts } from './generated/BoardFacts.js';
export type { BoardIntegrations } from './generated/BoardIntegrations.js';
export type { BoardJournal } from './generated/BoardJournal.js';
export type { BoardMemory } from './generated/BoardMemory.js';
export type { BoardMetrics } from './generated/BoardMetrics.js';
export type { BoardMissing } from './generated/BoardMissing.js';
export type { BoardOrder } from './generated/BoardOrder.js';
export type { BoardOverview } from './generated/BoardOverview.js';
export type { BoardQueue } from './generated/BoardQueue.js';
export type { BoardRole } from './generated/BoardRole.js';
export type { BoardRun } from './generated/BoardRun.js';
export type { BoardRuns } from './generated/BoardRuns.js';
export type { BoardSession } from './generated/BoardSession.js';
export type { BoardSessions } from './generated/BoardSessions.js';
export type { BoardSettings } from './generated/BoardSettings.js';
export type { BoardStarted } from './generated/BoardStarted.js';
export type { BoardTimeline } from './generated/BoardTimeline.js';
export type { BoardTranscript } from './generated/BoardTranscript.js';
export type { BoardWorld } from './generated/BoardWorld.js';
export type { BoardWorlds } from './generated/BoardWorlds.js';
export type { BodyMind } from './generated/BodyMind.js';
export type { BodyState } from './generated/BodyState.js';
export type { Budget } from './generated/Budget.js';
export type { ChallengeCounts } from './generated/ChallengeCounts.js';
export type { CommissionState } from './generated/CommissionState.js';
export type { ConfigView } from './generated/ConfigView.js';
export type { CreatureMeters } from './generated/CreatureMeters.js';
export type { DayRaces } from './generated/DayRaces.js';
export type { Envelope } from './generated/Envelope.js';
export type { Event } from './generated/Event.js';
export type { EventsParams } from './generated/EventsParams.js';
export type { EventsResult } from './generated/EventsResult.js';
export type { FactAnchor } from './generated/FactAnchor.js';
export type { FactCounts } from './generated/FactCounts.js';
export type { FactFinding } from './generated/FactFinding.js';
export type { FactSide } from './generated/FactSide.js';
export type { Gate } from './generated/Gate.js';
export type { GateReply } from './generated/GateReply.js';
export type { GateRequest } from './generated/GateRequest.js';
export type { GateVerdict } from './generated/GateVerdict.js';
export type { Ground } from './generated/Ground.js';
export type { Hello } from './generated/Hello.js';
export type { HoleView } from './generated/HoleView.js';
export type { JournalEntry } from './generated/JournalEntry.js';
export type { JournalToc } from './generated/JournalToc.js';
export type { KeySource } from './generated/KeySource.js';
export type { LawBarView } from './generated/LawBarView.js';
export type { LawMeter } from './generated/LawMeter.js';
export type { LawPost } from './generated/LawPost.js';
export type { LawSpeciality } from './generated/LawSpeciality.js';
export type { LiveState } from './generated/LiveState.js';
export type { McpServerView } from './generated/McpServerView.js';
export type { McpView } from './generated/McpView.js';
export type { Memory } from './generated/Memory.js';
export type { MemoryCreature } from './generated/MemoryCreature.js';
export type { MemoryNote } from './generated/MemoryNote.js';
export type { Meter } from './generated/Meter.js';
export type { MetricsCreature } from './generated/MetricsCreature.js';
export type { MindStanding } from './generated/MindStanding.js';
export type { ModelView } from './generated/ModelView.js';
export type { Named } from './generated/Named.js';
export type { NoteDependency } from './generated/NoteDependency.js';
export type { NoteScope } from './generated/NoteScope.js';
export type { NoteStanding } from './generated/NoteStanding.js';
export type { NoteVersion } from './generated/NoteVersion.js';
export type { OrderCounts } from './generated/OrderCounts.js';
export type { OrderState } from './generated/OrderState.js';
export type { PathOwner } from './generated/PathOwner.js';
export type { PriceView } from './generated/PriceView.js';
export type { PromptParams } from './generated/PromptParams.js';
export type { PromptResult } from './generated/PromptResult.js';
export type { ProofVerdict } from './generated/ProofVerdict.js';
export type { Provenance } from './generated/Provenance.js';
export type { ProviderView } from './generated/ProviderView.js';
export type { RaceLaw } from './generated/RaceLaw.js';
export type { Role } from './generated/Role.js';
export type { RunState } from './generated/RunState.js';
export type { Score } from './generated/Score.js';
export type { SessionSpent } from './generated/SessionSpent.js';
export type { Setting } from './generated/Setting.js';
export type { Sourced } from './generated/Sourced.js';
export type { Spend } from './generated/Spend.js';
export type { Spent } from './generated/Spent.js';
export type { JsonValue } from './generated/serde_json/JsonValue.js';
export type { ThroneLive } from './generated/ThroneLive.js';
export type { TimelineEntry } from './generated/TimelineEntry.js';
export type { TimelineKind } from './generated/TimelineKind.js';
export type { UnprovenEvent } from './generated/UnprovenEvent.js';
export type { Usage } from './generated/Usage.js';
export type { WorldState } from './generated/WorldState.js';
export type { WorldStatus } from './generated/WorldStatus.js';

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
