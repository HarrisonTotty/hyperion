import { galacticPositionFromLy } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import type { SceneModel, SystemPlace } from "../../lib/scene/model";
import { toSceneModel } from "../../lib/scene/sceneWire";
import type { SceneSnapshot, SceneStatus } from "../../lib/scene/useScene";
import { FIXTURE_SYSTEM } from "../../test/planetaryFixture";
import { designateFixture, stateAfterHeartbeat, stateInSpace } from "../../test/sceneFixture";
import {
  cameraAnnunciation,
  serverSceneStanding,
  systemPlace,
  viewProvenance,
} from "./serverScene";
import { SERVER_SCENE_NAME } from "./viewRun";

function modelOf(state: Parameters<typeof toSceneModel>[0]): SceneModel {
  const result = toSceneModel(state, designateFixture);
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return result.model;
}

/** The model with its system's tidal radius unsent, as a server that does not send it gives it. */
function withoutTidalRadius(model: SceneModel): SceneModel {
  if (model.system === null) {
    throw new Error("the fixture's model holds no system");
  }
  return { ...model, system: { ...model.system, tidalRadiusM: null } };
}

function snapshot(status: SceneStatus, model: SceneModel | null): SceneSnapshot {
  return { status, model, cameraFault: null };
}

describe("where the server's scene stands", () => {
  it("is drawn, with nothing said, while it is live in a system", () => {
    expect(serverSceneStanding(snapshot({ kind: "live" }, modelOf(stateAfterHeartbeat())))).toEqual(
      { drawable: true, stale: false, annunciation: null },
    );
  });

  it("is drawn stale, saying why, while it is held through a stale period", () => {
    const model = modelOf(stateAfterHeartbeat());
    expect(
      (["link_down", "resubscribing", "silent"] as const).map((reason) =>
        serverSceneStanding(snapshot({ kind: "stale", reason }, model)),
      ),
    ).toEqual([
      {
        drawable: true,
        stale: true,
        annunciation: { text: "SCENE STALE: NO CARRIER", standing: "waiting" },
      },
      {
        drawable: true,
        stale: true,
        annunciation: { text: "SCENE STALE: reopening the scene", standing: "waiting" },
      },
      {
        drawable: true,
        stale: true,
        annunciation: { text: "SCENE STALE: nothing received for 2 s", standing: "waiting" },
      },
    ]);
  });

  it("is not drawn with the ship in no system", () => {
    expect(serverSceneStanding(snapshot({ kind: "live" }, modelOf(stateInSpace())))).toEqual({
      drawable: false,
      stale: false,
      annunciation: {
        text: "SCENE NOT AVAILABLE: the ship is in no system",
        standing: "waiting",
      },
    });
  });

  it("is not drawn, a kept scene standing in, when the system's tidal radius was not sent", () => {
    const model = withoutTidalRadius(modelOf(stateAfterHeartbeat()));
    const live = snapshot({ kind: "live" }, model);
    expect([serverSceneStanding(live), viewProvenance(SERVER_SCENE_NAME, live)]).toEqual([
      {
        drawable: false,
        stale: false,
        annunciation: {
          text: "SCENE NOT AVAILABLE: the system's tidal radius was not sent",
          standing: "fault",
        },
      },
      "kept",
    ]);
  });

  it("is not drawn once refused or unanswered, even with a scene held", () => {
    const model = modelOf(stateAfterHeartbeat());
    expect(
      [
        serverSceneStanding(
          snapshot({ kind: "rejected", code: "unusable", reason: "a bad push" }, model),
        ),
        serverSceneStanding(snapshot({ kind: "timed_out" }, model)),
      ].map(({ drawable, annunciation }) => [drawable, annunciation?.text]),
    ).toEqual([
      [false, "SCENE REJECTED: a bad push"],
      [false, "SCENE TIMED OUT"],
    ]);
  });

  it("shows a refusal plain and a failure as a fault, each offering RETRY", () => {
    expect(
      [
        snapshot({ kind: "rejected", code: "unknown_universe", reason: "no such universe" }, null),
        snapshot({ kind: "rejected", code: "internal", reason: "the server failed" }, null),
        snapshot({ kind: "timed_out" }, null),
      ].map((each) => serverSceneStanding(each).annunciation),
    ).toEqual([
      { text: "SCENE REJECTED: no such universe", standing: "refused", retry: true },
      { text: "SCENE REJECTED: the server failed", standing: "fault", retry: true },
      { text: "SCENE TIMED OUT", standing: "fault", retry: true },
    ]);
  });
});

describe("a camera report the server did not accept", () => {
  it("reads as rejected with its reason, or as timed out", () => {
    expect([
      cameraAnnunciation({ kind: "refused", reason: "a camera is outside the scene's reach" }),
      cameraAnnunciation({ kind: "timed_out" }),
    ]).toEqual([
      {
        text: "CAMERA REPORT REJECTED: a camera is outside the scene's reach",
        standing: "refused",
      },
      { text: "CAMERA REPORT TIMED OUT", standing: "fault" },
    ]);
  });
});

describe("where the scene VIEW draws comes from", () => {
  it("is the server's only while it is chosen and can be drawn", () => {
    const live = snapshot({ kind: "live" }, modelOf(stateAfterHeartbeat()));
    expect([
      viewProvenance(SERVER_SCENE_NAME, live),
      viewProvenance("PRECISION TEST", live),
      viewProvenance(SERVER_SCENE_NAME, snapshot({ kind: "pending" }, null)),
      viewProvenance(SERVER_SCENE_NAME, snapshot({ kind: "live" }, modelOf(stateInSpace()))),
    ]).toEqual(["server", "kept", "kept", "kept"]);
  });

  it("is waited for with no universe, while pending and with the link down", () => {
    expect(
      [{ kind: "idle" } as const, { kind: "pending" } as const, { kind: "link_down" } as const].map(
        (status) => serverSceneStanding(snapshot(status, null)).annunciation?.text,
      ),
    ).toEqual([
      "SCENE NOT AVAILABLE: no universe open",
      "SCENE PENDING",
      "SCENE NOT AVAILABLE: NO CARRIER",
    ]);
  });
});

describe("the place of a server scene's system", () => {
  const known: SystemPlace = {
    kind: "charted",
    system: FIXTURE_SYSTEM,
    designation: "H7K 4C0RFZ D-7",
    barycentre: galacticPositionFromLy([1, 2, 3]),
  };

  it("is the place the scene states, whatever the client was told of", () => {
    const stated: SystemPlace = {
      kind: "stated",
      system: "0200080020000001",
      designation: "Vorth AB-C e4-17",
      barycentre: galacticPositionFromLy([4, 5, 6]),
      velocityMPerS: vec3(1e4, 2e5, -3e3),
      time: { seconds: 3_400, nanos: 0 },
    };
    expect(systemPlace("0200080020000001", stated, known)).toBe(stated);
    expect(systemPlace(FIXTURE_SYSTEM, { ...stated, system: FIXTURE_SYSTEM }, known)).not.toBe(
      known,
    );
  });

  it("without one, is the system the client was told of, or its ID with no position", () => {
    expect([
      systemPlace(FIXTURE_SYSTEM, null, known),
      systemPlace("0200080020000001", null, known),
    ]).toEqual([
      known,
      { kind: "unknown", system: "0200080020000001", designation: "0200080020000001" },
    ]);
  });
});
