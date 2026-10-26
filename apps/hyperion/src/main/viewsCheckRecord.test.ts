import { describe, expect, it } from "vitest";

import type { ViewsCheckRecord } from "../preload/api";
import { ATMOSPHERE_RECORD, MOONLIT_NIGHT } from "./fixtures/atmosphereRecord";
import { HANDOVER_RECORD, passingLook } from "./fixtures/handoverRecord";
import { TERRAIN_RECORD } from "./fixtures/terrainRecord";
import {
  isCaptureLabel,
  readAtmosphereCheckRecord,
  readHandoverCheckRecord,
  readTerrainCheckRecord,
  readViewsCheckRecord,
} from "./viewsCheckRecord";

const RECORD: ViewsCheckRecord = {
  timer: "full",
  devicePixelRatio: 0.78125,
  phases: [
    {
      name: "photoreal-two-wireframe",
      startMs: 1000,
      endMs: 21_000,
      views: [
        {
          name: "view",
          style: "photorealistic",
          canvas: { widthPx: 1126, heightPx: 906 },
          draws: 1200,
          gpuMs: [2.1, 2.2],
          submitMs: [1.1, 1.2],
          passLabels: ["bloom", "tonemap"],
          scales: [1, 0.9],
        },
      ],
      frameIntervalsMs: [16.7, 16.7],
      primaryIntervalsMs: [16.7],
      mainThreadMs: [8, 9],
      frameGpuMs: [2.3],
      untimedFrames: 0,
      droppedResolves: 0,
      unattributedGpuMs: 0.01,
    },
  ],
  resize: {
    before: [{ name: "view", canvas: { widthPx: 1126, heightPx: 906 } }],
    steps: [
      {
        widthFraction: 0.8,
        longestFrameMs: 41,
        views: [{ name: "view", canvas: { widthPx: 901, heightPx: 906 } }],
      },
    ],
    allocations: [{ kind: "created", name: "view:depth" }],
  },
  faults: [],
  perCanvasOverheadMs: 0.3,
};

/** `node` with the property at `path` replaced by `value`, copied along the way. */
function replaced(node: unknown, path: ReadonlyArray<string | number>, value: unknown): unknown {
  const [key, ...rest] = path;
  if (key === undefined) {
    return value;
  }
  if (Array.isArray(node)) {
    return node.map((each: unknown, i) => (i === key ? replaced(each, rest, value) : each));
  }
  if (typeof node === "object" && node !== null) {
    return Object.fromEntries(
      Object.entries(node).map(([name, each]: [string, unknown]) => [
        name,
        name === key ? replaced(each, rest, value) : each,
      ]),
    );
  }
  return node;
}

/** `RECORD` with one property path replaced. */
function withField(path: ReadonlyArray<string | number>, value: unknown): unknown {
  return replaced(RECORD, path, value);
}

/** `TERRAIN_RECORD` with one property path replaced. */
function withTerrainField(path: ReadonlyArray<string | number>, value: unknown): unknown {
  return replaced(TERRAIN_RECORD, path, value);
}

describe("the views check's record", () => {
  it("is read back whole", () => {
    expect(readViewsCheckRecord(structuredClone(RECORD))).toEqual(RECORD);
  });

  it.each([
    [["timer"], "fast"],
    [["devicePixelRatio"], 0],
    [["phases", 0, "name"], "photoreal-three"],
    [["phases", 0, "endMs"], 0],
    [["phases", 0, "views", 0, "style"], "cel"],
    [
      ["phases", 0, "views", 0, "gpuMs"],
      [1, "2"],
    ],
    [["phases", 0, "views", 0, "canvas", "widthPx"], -1],
    [["phases", 0, "droppedResolves"], 0.5],
    [["resize", "allocations", 0, "kind"], "uploaded"],
    [["resize", "steps", 0, "views"], null],
    [["faults"], [3]],
  ])("refuses a record whose %j is %j", (path, value) => {
    expect(readViewsCheckRecord(withField(path, value))).toBeNull();
  });

  it("refuses what is not an object", () => {
    expect(readViewsCheckRecord("record")).toBeNull();
  });
});

describe("the terrain run's record (R10.T16.d)", () => {
  it("is read back whole", () => {
    expect(readTerrainCheckRecord(structuredClone(TERRAIN_RECORD))).toEqual(TERRAIN_RECORD);
  });

  it("is read back with the terrain session's faults (R10.T16.e)", () => {
    const record = {
      ...TERRAIN_RECORD,
      terrainFaults: [
        {
          atMs: 40_000,
          text: "TERRAIN NOT AVAILABLE: terrain computation failed, reopen VIEW to retry",
          views: ["view", "instrument-1"],
        },
      ],
    };
    expect(readTerrainCheckRecord(structuredClone(record))).toEqual(record);
  });

  it("is read back with a session whose compile did not end (R10.T14.d)", () => {
    const record = {
      ...TERRAIN_RECORD,
      sessions: [{ madeAtMs: 5, compile: { kind: "pending" } }],
    };
    expect(readTerrainCheckRecord(structuredClone(record))).toEqual(record);
  });

  it("is read back with no hand-over range and an undestroyed device", () => {
    const record = {
      ...TERRAIN_RECORD,
      handover: null,
      loss: { ...TERRAIN_RECORD.loss, destroyedAtMs: null, notDestroyed: "no device was held" },
    };
    expect(readTerrainCheckRecord(structuredClone(record))).toEqual(record);
  });

  it.each([
    [["timer"], "fast"],
    [["phases", 0, "name"], "photoreal-alone"],
    [["phases", 0, "endMs"], 0],
    [["phases", 0, "views", 0, "missedFrames"], -1],
    [["phases", 0, "views", 0, "firstTerrainMs"], "soon"],
    [["phases", 0, "patchesDrawn"], [1.5]],
    [["phases", 0, "bakes", "baked"], null],
    [["phases", 0, "memory", "liveBytes", 0, "bytes"], -2],
    [["faults", 0, "kind"], "device-gone"],
    [["faults", 0, "reason"], 3],
    [["restoresAtMs"], ["soon"]],
    [["terrainFaults"], null],
    [["terrainFaults"], [{ atMs: "soon", text: "a fault", views: [] }]],
    [["terrainFaults"], [{ atMs: 1, text: 2, views: [] }]],
    [["terrainFaults"], [{ atMs: 1, text: "a fault", views: [3] }]],
    [["loss", "after", "fieldChunks"], 1.5],
    [["loss", "masks", "beforeKept"], "yes"],
    [["teardown", "workersLive"], -1],
    [["handover", "rangeM"], 0],
    [["phases", 0, "views", 0, "longestGapEndScriptS"], "late"],
    [["sessions"], null],
    [["sessions", 0, "madeAtMs"], null],
    [["sessions", 0, "compile"], null],
    [["sessions", 0, "compile", "kind"], "done"],
    [["sessions", 0, "compile", "compileMs"], -1],
    [["sessions", 0, "compile", "compiled"], 1.5],
    [["sessions", 0, "compile", "compiled"], null],
    [["sessions", 0, "compile", "longestFrameGapMs"], "long"],
    [["opening"], null],
    [["span"], null],
    [["span", "toMs"], 0],
    [["span", "longestFrameGapMs"], -1],
    [["span", "longestGapEndScriptS"], "late"],
    [["span", "harnessMs"], -1],
    [["opening", "preparedMs"], -1],
    [["opening", "preparations"], 0.5],
    [["opening", "longestFrameGapMs"], "long"],
  ])("refuses a record whose %j is %j", (path, value) => {
    expect(readTerrainCheckRecord(withTerrainField(path, value))).toBeNull();
  });

  it("refuses T20's record, and T20's reader refuses it", () => {
    expect(readTerrainCheckRecord(RECORD)).toBeNull();
    expect(readViewsCheckRecord(TERRAIN_RECORD)).toBeNull();
  });
});

describe("the hand-over check's record (R10.T10.g)", () => {
  it("is read back whole", () => {
    expect(readHandoverCheckRecord(structuredClone(HANDOVER_RECORD))).toEqual(HANDOVER_RECORD);
  });

  it("is read back with a look that was not locked and compared nothing", () => {
    const look = {
      ...passingLook("handover-90", "HAND-OVER 90°", true),
      exposure: { lockedEv100: null, lockedReading: null, endReading: null, held: false },
      switchesS: { figure: null, terrain: null, disc: null },
      comparisons: [],
      failures: ["the exposure was not locked: AUTO did not resume on ENABLE"],
    };
    const record = { ...HANDOVER_RECORD, looks: [look] };
    expect(readHandoverCheckRecord(structuredClone(record))).toEqual(record);
  });

  it.each([
    [["world"], 6],
    [["devicePixelRatio"], 0],
    [["canvas"], null],
    [["target", "heightPx"], -1],
    [["rangeM"], 0],
    [["radPerPx"], Number.NaN],
    [["cellLevel"], 6.5],
    [["approach", "ratePerS"], 0],
    [["approach", "closestRatio"], 4],
    [["looks"], null],
    [["looks", 0, "name"], "handover-45"],
    [["looks", 1, "name"], "handover-30"],
    [["looks", 0, "judged"], "yes"],
    [["looks", 0, "judged"], false],
    [["looks", 2, "judged"], true],
    [["looks", 0, "scene"], "HAND-OVER 90°"],
    [["looks", 0, "nominal", "phaseDeg"], -1],
    [["looks", 0, "exposure", "held"], null],
    [["looks", 0, "exposure", "lockedEv100"], "13.5"],
    [["looks", 0, "surveyedCells"], -1],
    [["looks", 0, "switchesS", "terrain"], "late"],
    [["looks", 0, "failures"], [1]],
    [["looks", 0, "comparisons", 1, "name"], "disc-to-figure"],
    [["looks", 0, "comparisons", 0, "name"], "figure-to-disc"],
    [["looks", 0, "comparisons", 0, "framesApart"], 0],
    [["looks", 0, "comparisons", 0, "beforeWasLast"], 1],
    [["looks", 0, "comparisons", 0, "before", "regime"], "mesh"],
    [["looks", 0, "comparisons", 0, "before", "regime"], "figure"],
    [["looks", 0, "comparisons", 0, "after", "regime"], "disc"],
    [["looks", 0, "comparisons", 1, "after", "regime"], "figure"],
    [["looks", 0, "comparisons", 2, "before", "regime"], "disc"],
    [["looks", 0, "comparisons", 0, "before", "rangeRatio"], 0],
    [["looks", 0, "comparisons", 0, "after", "bodyMisses"], 50_000],
    [["looks", 0, "comparisons", 0, "after", "angles"], null],
    [["looks", 0, "comparisons", 0, "body", "own", "meanAfter"], -0.1],
    [["looks", 0, "comparisons", 0, "survey", "shared", "pixels"], 1.5],
    [["looks", 0, "comparisons", 0, "texels", 0, "sharedPixels"], 0],
    [["looks", 0, "comparisons", 0, "texels", 1, "texel"], 19_200],
    [["looks", 0, "comparisons", 0, "texels", 2, "meanAfter"], Number.POSITIVE_INFINITY],
    [["faults", 0], { atMs: 1 }],
  ])("refuses a record whose %j is %j", (path, value) => {
    const record =
      path[0] === "faults"
        ? {
            ...HANDOVER_RECORD,
            faults: [{ atMs: 0, kind: "device-lost", reason: "unknown", line: "a" }],
          }
        : HANDOVER_RECORD;
    expect(readHandoverCheckRecord(replaced(record, path, value))).toBeNull();
  });

  it("is read back with the regime's switch from the disc straight to the terrain", () => {
    const record = replaced(
      HANDOVER_RECORD,
      ["looks", 0, "comparisons", 0, "after", "regime"],
      "terrain",
    );
    expect(readHandoverCheckRecord(record)).toEqual(record);
  });

  it("refuses the terrain run's record", () => {
    expect(readHandoverCheckRecord(TERRAIN_RECORD)).toBeNull();
  });

  it("is refused by the terrain run's reader", () => {
    expect(readTerrainCheckRecord(HANDOVER_RECORD)).toBeNull();
  });
});

describe("the atmosphere run's record (R08.T10.b)", () => {
  it("is read back whole", () => {
    expect(readAtmosphereCheckRecord(structuredClone(ATMOSPHERE_RECORD))).toEqual(
      ATMOSPHERE_RECORD,
    );
  });

  it("is read back with fault lines, no capture and no frame times", () => {
    const night = {
      ...MOONLIT_NIGHT,
      exposure: null,
      camera: null,
      frames: {
        ...MOONLIT_NIGHT.frames,
        draws: 1,
        intervalMedianMs: null,
        intervalP95Ms: null,
        gpuMedianMs: null,
        gpuP95Ms: null,
      },
      faults: ["ATMOSPHERE NOT AVAILABLE: computation failed for TEST EARTH, reopen VIEW to retry"],
    };
    const faulted = { ...ATMOSPHERE_RECORD, captures: [night], faults: ["device-lost: unknown"] };
    const empty = { ...ATMOSPHERE_RECORD, captures: [] };
    expect([
      readAtmosphereCheckRecord(structuredClone(faulted)),
      readAtmosphereCheckRecord(structuredClone(empty)),
    ]).toEqual([faulted, empty]);
  });

  it.each([
    [["timer"], "fast"],
    [["devicePixelRatio"], 0],
    [["canvas"], null],
    [["canvas", "widthPx"], -1],
    [["mediaReadyMs"], -1],
    [["mediaReadyMs"], Number.NaN],
    [["captures"], null],
    [["captures", 0, "label"], "Fixture TEST EARTH"],
    [["captures", 0, "label"], "../escape"],
    [["captures", 0, "label"], "x".repeat(49)],
    [["captures", 1, "label"], "fixture-test-earth"],
    [["captures", 0, "designation"], 3],
    [["captures", 0, "camera"], false],
    [["captures", 0, "rangeM"], 0],
    [["captures", 0, "rangeM"], "far"],
    [["captures", 0, "phaseDeg"], 181],
    [["captures", 0, "phaseDeg"], -1],
    [["captures", 0, "exposure"], 12],
    [["captures", 0, "atMs"], Number.POSITIVE_INFINITY],
    [["captures", 0, "frames"], null],
    [["captures", 0, "frames", "toMs"], 0],
    [["captures", 0, "frames", "draws"], 1.5],
    [["captures", 0, "frames", "intervalMedianMs"], -1],
    [["captures", 0, "frames", "gpuP95Ms"], "slow"],
    [["captures", 0, "statements"], [1]],
    [["captures", 0, "faults"], null],
    [["captures", 0, "faults"], [{ line: "a fault" }]],
    [["faults"], [3]],
  ])("refuses a record whose %j is %j", (path, value) => {
    expect(readAtmosphereCheckRecord(replaced(ATMOSPHERE_RECORD, path, value))).toBeNull();
  });

  it("takes the labels the capture's channel takes, and no other", () => {
    expect([isCaptureLabel("fixture-test-earth-slab"), isCaptureLabel("A/B")]).toEqual([
      true,
      false,
    ]);
  });

  it("refuses the other runs' records, and their readers refuse it", () => {
    expect([
      readAtmosphereCheckRecord(RECORD),
      readAtmosphereCheckRecord(TERRAIN_RECORD),
      readAtmosphereCheckRecord(HANDOVER_RECORD),
      readViewsCheckRecord(ATMOSPHERE_RECORD),
      readTerrainCheckRecord(ATMOSPHERE_RECORD),
      readHandoverCheckRecord(ATMOSPHERE_RECORD),
    ]).toEqual([null, null, null, null, null, null]);
  });
});
