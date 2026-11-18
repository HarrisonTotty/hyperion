import { describe, expect, it } from "vitest";

import { TEST_STILL_REQUEST, testStillLabel } from "../../test/stillFixtures";
import type { TerrainAnnunciation } from "../terrain/annunciation";
import {
  parseStillLabel,
  STILL_ANNUNCIATIONS_MAX,
  STILL_LABEL_SCHEMA,
  STILL_MAX_SIDE_PX,
  STILL_TERRAIN_DETAIL_LIMITED,
  STILL_TERRAIN_STREAMING,
  stillLabel,
  stillLabelText,
} from "./stillLabel";

/** The label as plain JSON, to be broken a field at a time. */
function labelJson(): Record<string, unknown> {
  const parsed: unknown = JSON.parse(stillLabelText(testStillLabel()));
  if (typeof parsed !== "object" || parsed === null) {
    throw new Error("a label's text is a JSON object");
  }
  return { ...parsed };
}

/** The test request's one coverage, with `change` made. */
function coverage(change: Record<string, unknown>): ReadonlyArray<Record<string, unknown>> {
  return [{ ...TEST_STILL_REQUEST.survey[0], ...change }];
}

describe("a still's label (R11.T10.c, Design note 17)", () => {
  it("carries every field of Design note 17, and the view's readings of them", () => {
    expect(testStillLabel()).toEqual({
      schema: STILL_LABEL_SCHEMA,
      time: { seconds: 412_345_678, nanos: 250_000_000 },
      camera: {
        frame: { kind: "body", body: "0200080020000000.0100" },
        positionM: [6_771_000, -12_500.25, 340.5],
        orientation: { w: 0.5, x: 0.5, y: -0.5, z: 0.5 },
        fovXDeg: 60,
      },
      exposure: { kind: "auto", ev100: 12.4 },
      readings: [
        { label: "FRAME", value: "BODY TEST EARTH" },
        { label: "TIME", value: "UT +13 yr 024/06:27:58" },
        { label: "CAMERA", value: "ORBIT" },
        { label: "FOV", value: "60°" },
        { label: "EXPOSURE", value: "EV100 12.4 AUTO" },
      ],
      setting: "high",
      decoration: true,
      survey: [
        {
          body: "0200080020000000.0100",
          name: "TEST EARTH",
          level: 7,
          surveyedFraction: 0.34,
          bestResolutionM: 1200,
        },
      ],
      annunciations: ["ATMOSPHERE: APPROXIMATE"],
      size: { widthPx: 16, heightPx: 8 },
    });
  });

  it("is labelled TERRAIN: STREAMING, once and last, where its frustum had not streamed", () => {
    expect([
      testStillLabel({}, false).annunciations,
      testStillLabel({ annunciations: [STILL_TERRAIN_STREAMING] }, false).annunciations,
      testStillLabel({}, true).annunciations,
    ]).toEqual([
      ["ATMOSPHERE: APPROXIMATE", "TERRAIN: STREAMING"],
      ["TERRAIN: STREAMING"],
      ["ATMOSPHERE: APPROXIMATE"],
    ]);
  });

  it("says TERRAIN: STREAMING in the view's own words", () => {
    const streaming: TerrainAnnunciation = STILL_TERRAIN_STREAMING;
    expect(streaming).toBe("TERRAIN: STREAMING");
  });

  it("puts TERRAIN: STREAMING in TERRAIN: DETAIL LIMITED's place where its frustum had not streamed (R11.T10.e.3)", () => {
    const limited: TerrainAnnunciation = STILL_TERRAIN_DETAIL_LIMITED;
    const annunciations = ["ATMOSPHERE: APPROXIMATE", limited];
    expect([
      testStillLabel({ annunciations }, false).annunciations,
      testStillLabel({ annunciations }, true).annunciations,
    ]).toEqual([
      ["ATMOSPHERE: APPROXIMATE", "TERRAIN: STREAMING"],
      ["ATMOSPHERE: APPROXIMATE", "TERRAIN: DETAIL LIMITED"],
    ]);
  });

  it("states each annunciation once, as the view stated them", () => {
    expect(
      testStillLabel({ annunciations: ["IMAGE: DETAIL LIMITED", "IMAGE: DETAIL LIMITED"] })
        .annunciations,
    ).toEqual(["IMAGE: DETAIL LIMITED"]);
  });

  it("records an inhibited exposure with who or what inhibited it", () => {
    const label = testStillLabel({
      exposure: { kind: "inhibited", ev100: 3, cause: "OPERATOR" },
    });
    expect(label.exposure).toEqual({ kind: "inhibited", ev100: 3, cause: "OPERATOR" });
  });

  it("has one text, its fields in the schema's order whatever order it was built in", () => {
    const label = testStillLabel();
    const reordered = {
      size: label.size,
      annunciations: label.annunciations,
      survey: label.survey,
      decoration: label.decoration,
      setting: label.setting,
      readings: label.readings.map(({ label: name, value }) => ({ value, label: name })),
      exposure: { ev100: label.exposure.ev100, kind: "auto" as const },
      camera: {
        fovXDeg: label.camera.fovXDeg,
        orientation: label.camera.orientation,
        positionM: label.camera.positionM,
        frame: label.camera.frame,
      },
      time: { nanos: label.time.nanos, seconds: label.time.seconds },
      schema: label.schema,
    };
    expect(stillLabelText(reordered)).toBe(stillLabelText(label));
  });

  it("reads back from its text equal, field for field", () => {
    const label = testStillLabel();
    expect(parseStillLabel(JSON.parse(stillLabelText(label)))).toEqual(label);
  });

  it("reads −0 as 0, as its text writes it, so that a label equals its text's", () => {
    const label = testStillLabel({
      camera: { ...TEST_STILL_REQUEST.camera, positionM: [-0, 0, 0] },
    });
    expect(Object.is(label.camera.positionM[0], 0)).toBe(true);
  });

  it("refuses a typed array where an object belongs, without walking its keys", () => {
    // Ten million keys, were they walked: the main process would hold them all.
    const huge = new Uint8Array(10_000_000);
    expect([
      parseStillLabel(huge),
      parseStillLabel({ ...labelJson(), time: huge }),
      parseStillLabel({ ...labelJson(), size: new Map([["widthPx", 16]]) }),
    ]).toEqual([null, null, null]);
  });

  it.each([
    ["a missing field", { size: undefined }],
    ["another schema", { schema: "hyperion-still/2" }],
    ["a field besides the schema's", { extra: true }],
    ["a time of fractional seconds", { time: { seconds: 1.5, nanos: 0 } }],
    ["nanoseconds of a whole second", { time: { seconds: 1, nanos: 1e9 } }],
    ["a body ID not in its wire form", { survey: coverage({ body: "earth" }) }],
    ["a coverage level R09 does not serve", { survey: coverage({ level: 9 }) }],
    ["a coverage over the whole", { survey: coverage({ surveyedFraction: 1.01 }) }],
    ["a coverage of a body with no name", { survey: coverage({ name: "" }) }],
    ["a body's coverage twice", { survey: [...coverage({}), ...coverage({ level: 6 })] }],
    ["an exposure without its level", { exposure: { kind: "metered", ev100: 3 } }],
    ["an inhibit without its cause", { exposure: { kind: "inhibited", ev100: 3 } }],
    ["a reading of no value", { readings: [{ label: "FOV", value: "" }] }],
    [
      "a reading twice",
      {
        readings: [
          { label: "FOV", value: "60°" },
          { label: "FOV", value: "30°" },
        ],
      },
    ],
    ["a setting of no name", { setting: "medium" }],
    ["decoration as a word", { decoration: "on" }],
    ["an annunciation of a control character", { annunciations: ["TERRAIN:\u0007STREAMING"] }],
    [
      "an annunciation twice",
      { annunciations: ["IMAGE: DETAIL LIMITED", "IMAGE: DETAIL LIMITED"] },
    ],
    [
      "too many annunciations",
      {
        annunciations: Array.from(
          { length: STILL_ANNUNCIATIONS_MAX + 1 },
          (_, i) => `NOTE ${String(i)}`,
        ),
      },
    ],
    ["a side past the largest still's", { size: { widthPx: STILL_MAX_SIDE_PX + 1, heightPx: 8 } }],
    ["a side of a fraction", { size: { widthPx: 16.5, heightPx: 8 } }],
    ["a field of view of half the sky", { camera: { ...TEST_STILL_REQUEST.camera, fovXDeg: 180 } }],
    [
      "a position that is not a number",
      { camera: { ...TEST_STILL_REQUEST.camera, positionM: [0, Number.NaN, 0] } },
    ],
  ])("refuses %s", (_, change) => {
    expect(parseStillLabel({ ...labelJson(), ...change })).toBeNull();
  });

  it("refuses a request that makes no label of the schema, the caller's bug", () => {
    expect(() =>
      stillLabel({ ...TEST_STILL_REQUEST, size: { widthPx: 0, heightPx: 8 } }, true),
    ).toThrow(RangeError);
  });
});
