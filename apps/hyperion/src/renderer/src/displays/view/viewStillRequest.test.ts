import { describe, expect, it } from "vitest";

import { DEFAULT_EXPOSURE, ev100FromTriple } from "../../view/photometry/exposure";
import { precisionScene } from "../../view/scenes/precision";
import { parseStillLabel, STILL_LABEL_SCHEMA } from "../../view/stills/stillLabel";
import { CoverageView } from "../../view/terrain/coverage";
import { runPose, startRun } from "./viewRun";
import {
  stillAnnunciationsOf,
  stillCameraOf,
  stillExposureOf,
  stillReadingsOf,
  stillRequestOf,
  stillSurveyOf,
} from "./viewStillRequest";

describe("a still's label from the view (R11.T10.b)", () => {
  it("takes the label block's readings, each label once, leaving out what a label refuses", () => {
    expect(
      stillReadingsOf([
        { label: "FRAME", value: "BODY TEST EARTH" },
        { label: "FOV", value: " 60° " },
        { label: "FRAME", value: "SYSTEM SOL" },
        { label: "EMPTY", value: "" },
        { label: "LONG", value: "X".repeat(241) },
        { label: "BROKEN", value: "A\u0007B" },
      ]),
    ).toEqual([
      { label: "FRAME", value: "BODY TEST EARTH" },
      { label: "FOV", value: "60°" },
    ]);
  });

  it("takes at most 32 readings", () => {
    const lines = Array.from({ length: 40 }, (_, i) => ({ label: `L${String(i)}`, value: "V" }));
    expect(stillReadingsOf(lines)).toHaveLength(32);
  });

  it("marks a stale reading and a stale field with their S, as the block shows them", () => {
    expect(
      stillReadingsOf([
        { label: "TIME", value: "UT +12 yr 001/00:00:00", stale: true },
        {
          label: "STARS",
          value: "V 10.0 mag CAM · 2000 ly",
          fields: [
            { text: "V 10.0 mag", widthCh: 12, stale: true },
            { text: "2000 ly", widthCh: 9, stale: false },
          ],
        },
      ]),
    ).toEqual([
      { label: "TIME", value: "UT +12 yr 001/00:00:00 S" },
      { label: "STARS", value: "V 10.0 mag S CAM · 2000 ly" },
    ]);
  });

  it("takes the display's status lines, then the block's faults and statements, each once", () => {
    expect(
      stillAnnunciationsOf(
        ["SCENE STALE: NO CARRIER"],
        ["GRAPHICS LOST"],
        ["TERRAIN: STREAMING", { text: "GRAPHICS LOST", standing: "fault" }, "TERRAIN: STREAMING"],
      ),
    ).toEqual(["SCENE STALE: NO CARRIER", "GRAPHICS LOST", "TERRAIN: STREAMING"]);
  });

  it("records the exposure's level, an inhibit with its cause in its reading's words", () => {
    expect([
      stillExposureOf(DEFAULT_EXPOSURE),
      stillExposureOf({ kind: "auto", ev100: 11.5 }),
      stillExposureOf({ kind: "inhibited", ev100: 9, reason: "operator" }),
      stillExposureOf({ kind: "inhibited", ev100: 9, reason: "no_image_to_meter" }),
    ]).toEqual([
      {
        kind: "manual",
        ev100: DEFAULT_EXPOSURE.kind === "manual" ? ev100FromTriple(DEFAULT_EXPOSURE.triple) : 0,
      },
      { kind: "auto", ev100: 11.5 },
      { kind: "inhibited", ev100: 9, cause: "OPERATOR" },
      { kind: "inhibited", ev100: 9, cause: "NO IMAGE TO METER" },
    ]);
  });

  it("records the run's camera: its frame, position, orientation and field of view", () => {
    const run = startRun(precisionScene());
    const pose = runPose(run);
    const camera = stillCameraOf(run);
    expect([camera.frame.kind, camera.positionM, camera.orientation, camera.fovXDeg]).toEqual([
      pose.frame.kind,
      [pose.positionM.x, pose.positionM.y, pose.positionM.z],
      {
        w: pose.orientation.w,
        x: pose.orientation.x,
        y: pose.orientation.y,
        z: pose.orientation.z,
      },
      run.camera.fovDeg,
    ]);
  });

  it("takes each body's survey coverage from its CoverageView, leaving out a body with none", () => {
    const run = startRun(precisionScene());
    const [first, second] = run.scene.bodies;
    if (first === undefined || second === undefined) {
      throw new Error("the scene has fewer than two bodies");
    }
    const held = CoverageView.empty(6);
    const survey = stillSurveyOf(run.scene, (body) => (body === first.id ? held : null));
    expect(survey).toEqual([
      {
        body: first.id,
        name: first.designation.toUpperCase(),
        level: 6,
        surveyedFraction: 0,
        bestResolutionM: null,
      },
    ]);
  });

  it("makes a request whose label, with a size, is one of the schema's", () => {
    const run = startRun(precisionScene());
    const request = stillRequestOf({
      run,
      exposure: { kind: "auto", ev100: 12 },
      lines: [{ label: "FRAME", value: "SYSTEM TEST" }],
      scene: [],
      faults: [],
      statements: ["PHOTOREALISTIC"],
      setting: "low",
      decoration: false,
      coverage: () => null,
    });
    const label = parseStillLabel({
      schema: STILL_LABEL_SCHEMA,
      ...request,
      size: { widthPx: 100, heightPx: 70 },
    });
    expect([label?.setting, label?.readings, label?.annunciations, label?.time]).toEqual([
      "low",
      [{ label: "FRAME", value: "SYSTEM TEST" }],
      ["PHOTOREALISTIC"],
      run.scene.time,
    ]);
  });
});
