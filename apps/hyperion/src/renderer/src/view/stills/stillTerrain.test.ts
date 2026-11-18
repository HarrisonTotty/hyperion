import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { fakeStillTerrainSession, stillTerrainCapture } from "../../test/fakeStillTerrain";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { TextureHandle } from "../engine/types";
import { STILL_TERRAIN_VIEW_ID, type StillTerrainOptions, StillTerrainView } from "./stillTerrain";

const BODY: BodyIdHex = "0200080020000000.0100";
const ELSEWHERE: BodyIdHex = "0200080020000000.0200";
const MASK: TextureHandle = { kind: "texture", name: "coverage mask" };
const SIZE = { widthPx: 3840, heightPx: 2160 } as const;

/** A still's options over `bodies`, its mask `MASK` over `BODY` alone. */
function optionsOf(bodies: ReadonlyArray<BodyIdHex> = [BODY]): StillTerrainOptions {
  return {
    camera: { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
    size: SIZE,
    exposureScale: 1e-4,
    bodies,
    regimes: new Map([[BODY, "terrain"]]),
    coverage: (body) => (body === BODY ? { mask: MASK, whole: true } : null),
    nowMs: () => 0,
  };
}

describe("a still's own terrain view (R11.T10.e)", () => {
  it("notes the still's view at bulk priority, at its own size and plan and the high setting's τ, drawn photorealistic", () => {
    const session = fakeStillTerrainSession();
    const capture = stillTerrainCapture(session);
    const view = new StillTerrainView(capture, optionsOf());
    const [noted] = session.noted;
    expect([noted?.[0], noted?.[1]]).toEqual([
      STILL_TERRAIN_VIEW_ID,
      {
        scene: capture.report.scene,
        pose: capture.report.pose,
        fovXRad: capture.report.fovXRad,
        style: capture.report.style,
        setting: "high",
        regimesFromM: capture.report.regimesFromM,
        presentedPx: SIZE,
        selectionPx: SIZE,
        drawnStyle: "photorealistic",
        streamPriority: "bulk",
        regimes: new Map([[BODY, "terrain"]]),
      },
    ]);
    view.dispose();
  });

  it("holds the survey of the bodies it hands over from its making to its disposal, once", () => {
    const session = fakeStillTerrainSession();
    const view = new StillTerrainView(stillTerrainCapture(session), optionsOf());
    const held = [session.holds, session.releases.count];
    view.dispose();
    view.dispose();
    expect([held, session.releases.count, session.forgotten]).toEqual([
      [[[BODY]], 0],
      1,
      [STILL_TERRAIN_VIEW_ID],
    ]);
  });

  it("has streamed as its session says over the bodies it hands over, and always with none", () => {
    const session = fakeStillTerrainSession();
    const view = new StillTerrainView(stillTerrainCapture(session), optionsOf());
    const before = view.streamed();
    session.streamed = true;
    const none = new StillTerrainView(
      stillTerrainCapture(fakeStillTerrainSession()),
      optionsOf([]),
    );
    expect([before, view.streamed(), session.asked[0], none.streamed()]).toEqual([
      false,
      true,
      [STILL_TERRAIN_VIEW_ID, [BODY]],
      true,
    ]);
    view.dispose();
    none.dispose();
  });

  it("gives each sample the coverage masks the view's frame took at the request", () => {
    const view = new StillTerrainView(stillTerrainCapture(fakeStillTerrainSession()), optionsOf());
    const terrain = view.frameTerrain();
    expect([
      terrain.surveyCoverage(BODY),
      terrain.surveyedWhole(BODY),
      terrain.surveyCoverage(ELSEWHERE),
      terrain.surveyedWhole(ELSEWHERE),
    ]).toEqual([MASK, true, null, false]);
    view.dispose();
  });
});
