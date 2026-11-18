import type { BodyIdHex } from "@hyperion/protocol";

import type { ViewId } from "../view/camera/state";
import type { StillTerrainCapture, StillTerrainSession } from "../view/stills/stillTerrain";
import type { TerrainViewReport } from "../view/terrain/session";
import { viewAt } from "./terrainSessionFixtures";

/** A terrain session as a still sees it, recording what the still asks of it. */
export interface FakeStillTerrainSession extends StillTerrainSession {
  /** Each view noted, with its report, in order. */
  readonly noted: Array<readonly [ViewId, TerrainViewReport]>;
  /** Each view forgotten, in order. */
  readonly forgotten: ViewId[];
  /** Each hold's bodies, in order, and how many holds were released. */
  readonly holds: Array<ReadonlyArray<BodyIdHex>>;
  readonly releases: { count: number };
  /** What {@link StillTerrainSession.streamedFor} answers, which a test sets. */
  streamed: boolean;
  /** Each `streamedFor` call's view and bodies. */
  readonly asked: Array<readonly [ViewId, ReadonlyArray<BodyIdHex>]>;
}

/** A session that draws nothing and records the still's view, holds and questions. */
export function fakeStillTerrainSession(): FakeStillTerrainSession {
  const session: FakeStillTerrainSession = {
    noted: [],
    forgotten: [],
    holds: [],
    releases: { count: 0 },
    streamed: false,
    asked: [],
    noteView: (id, report) => {
      session.noted.push([id, report]);
    },
    forgetView: (id) => {
      session.forgotten.push(id);
    },
    holdSurvey: (bodies) => {
      session.holds.push([...bodies]);
      return () => {
        session.releases.count += 1;
      };
    },
    streamedFor: (id, bodies) => {
      session.asked.push([id, [...bodies]]);
      return session.streamed;
    },
    drawTerrain: () => null,
    drawLateTerrain: () => [],
    coverageMask: () => null,
    coverageWhole: () => false,
    pass: () => null,
  };
  return session;
}

/** A view's capture of `session` at a still's request, its report the fixture's view's. */
export function stillTerrainCapture(session: StillTerrainSession): StillTerrainCapture {
  return {
    session,
    report: { ...viewAt(5e6), litTauScale: 1.25, readoutBody: null, floorStepped: true },
    occluderSlopePx: 3,
  };
}
