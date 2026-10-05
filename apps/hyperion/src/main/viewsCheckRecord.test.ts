import { describe, expect, it } from "vitest";

import type { ViewsCheckRecord } from "../preload/api";
import { readViewsCheckRecord } from "./viewsCheckRecord";

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
