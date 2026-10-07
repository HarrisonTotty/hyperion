import { describe, expect, it } from "vitest";

import { markStrokeDevicePx, minReticleGapDevicePx } from "../lib/strokes";
import type { DrawOp } from "../spatial/drawList";
import { paint } from "../spatial/paint";
import { stubCanvas } from "../test/RecordingContext2D";
import {
  arcPoints,
  linearOfCode,
  type PaintedStroke,
  recordingContext,
  screenStrokeOf,
  SPATIAL_KINDS,
  SPATIAL_RATIOS,
  spatialFrames,
} from "./spatial";
import { srgb8 } from "./strokeContrast";

/** Tokens unlike the stylesheet's; the arithmetic here reads no colour. */
const TOKENS = {
  text: "#111111",
  accent: "#222222",
  target: "#333333",
  line: "#444444",
  surface0: "#555555",
  textMuted: "#666666",
};

/** Paints `ops` at `ratio` through the recording context, onto a stubbed canvas. */
function painted(ops: ReadonlyArray<DrawOp>, ratio: number): PaintedStroke[] {
  stubCanvas();
  const canvas = document.createElement("canvas");
  canvas.width = 200;
  canvas.height = 200;
  const context = canvas.getContext("2d");
  if (context === null) {
    throw new Error("the stubbed canvas gave no 2D context");
  }
  const strokes: PaintedStroke[] = [];
  paint(
    recordingContext(context, ratio, strokes),
    { ops, anchors: [], curveLabels: [] },
    TOKENS,
    ratio,
  );
  return strokes;
}

describe("the canvas check's read-back", () => {
  it("decodes every 8-bit code to a linear value the reader encodes back to it", () => {
    const codes = Array.from({ length: 256 }, (_, code) => code);

    expect(codes.map((code) => srgb8(Math.fround(linearOfCode(code))))).toEqual(codes);
  });
});

describe("arcPoints", () => {
  it("reads a full turn in chords of at most a pixel, ending where it starts", () => {
    const points = arcPoints({ x: 10, y: 20 }, 30, 0, 2 * Math.PI);
    const chords = points.slice(1).map((p, i) => {
      const a = points[i] ?? p;
      return Math.hypot(p.x - a.x, p.y - a.y);
    });

    expect(points.at(-1)).toEqual(points[0]);
    expect(Math.max(...chords)).toBeLessThanOrEqual(1);
  });

  it("reads a small circle in no fewer than 12 chords", () => {
    expect(arcPoints({ x: 0, y: 0 }, 0.5, 0, 2 * Math.PI)).toHaveLength(13);
  });
});

describe("the painted strokes", () => {
  it("records each op's stroke, in order, at its device width", () => {
    const strokes = painted(
      [
        {
          kind: "line",
          from: { xPx: 10, yPx: 10 },
          to: { xPx: 90, yPx: 10 },
          stroke: "textMuted",
          widthPx: 1,
          markId: null,
        },
        {
          kind: "reticle",
          id: "a",
          centre: { xPx: 50, yPx: 50 },
          halfSizePx: 9,
          stroke: "accent",
          widthPx: 1.5,
        },
      ],
      0.78125,
    );

    expect(strokes.map((stroke) => Math.round(stroke.widthPx * 1e9) / 1e9)).toEqual([
      2,
      markStrokeDevicePx(0.78125),
    ]);
    expect(strokes[0]?.subpaths).toEqual([
      {
        points: [
          { x: 7.8125, y: 7.8125 },
          { x: 70.3125, y: 7.8125 },
        ],
        closed: false,
      },
    ]);
  });

  it("keeps a line's two ends and each of a reticle's arms' ends a pixel clear, and no curve's", () => {
    const [line, reticle, ringed] = painted(
      [
        {
          kind: "line",
          from: { xPx: 10, yPx: 10 },
          to: { xPx: 90, yPx: 10 },
          stroke: "textMuted",
          widthPx: 1,
          markId: null,
        },
        {
          kind: "reticle",
          id: "a",
          centre: { xPx: 50, yPx: 50 },
          halfSizePx: 9,
          stroke: "accent",
          widthPx: 1.5,
        },
        {
          kind: "symbol",
          id: "b",
          centre: { xPx: 50, yPx: 50 },
          shape: "ringed-circle",
          radiusPx: 6,
          stroke: "accent",
          fill: null,
          widthPx: 1.5,
        },
      ],
      1,
    ).map((stroke, index) => screenStrokeOf(String(index), stroke));

    expect(line?.cutEnds).toHaveLength(2);
    expect(reticle?.cutEnds).toHaveLength(8);
    expect(ringed?.cutEnds).toHaveLength(0);
  });

  it("traces a polygon symbol closed, with no cut end", () => {
    const [triangle] = painted(
      [
        {
          kind: "symbol",
          id: "a",
          centre: { xPx: 50, yPx: 50 },
          shape: "triangle-down",
          radiusPx: 6,
          stroke: "text",
          fill: null,
          widthPx: 1.5,
        },
      ],
      1,
    ).map((stroke) => screenStrokeOf("triangle", stroke));

    expect(triangle?.cutEnds).toEqual([]);
    expect(triangle?.stroke.segments).toHaveLength(4);
  });
});

describe("spatialFrames", () => {
  it.each(SPATIAL_RATIOS)("holds every kind the ruling names at a ratio of %s", (ratio) => {
    const kinds = new Set(
      spatialFrames(ratio).flatMap((frame) => frame.ops.map((entry) => entry.kind)),
    );

    expect(kinds).toEqual(new Set(Object.values(SPATIAL_KINDS)));
  });

  it.each(SPATIAL_RATIOS)(
    "stands the destination on the selection the least gap outside its bracket at %s",
    (ratio) => {
      const marks = spatialFrames(ratio).find((frame) => frame.name === "marks");
      const halfSize = (kind: string): number => {
        const op = marks?.ops.find((entry) => entry.kind === kind)?.op;
        return op?.kind === "reticle" ? op.halfSizePx : Number.NaN;
      };

      expect(
        halfSize(SPATIAL_KINDS.pairDestination) - halfSize(SPATIAL_KINDS.pairBracket),
      ).toBeCloseTo(Math.max(4, minReticleGapDevicePx(ratio) / ratio), 9);
    },
  );

  it.each(SPATIAL_RATIOS)("centres each mark on a device pixel at %s", (ratio) => {
    const holes = spatialFrames(ratio).flatMap((frame) => frame.holes);

    expect(holes).toHaveLength(2);
    for (const hole of holes) {
      expect((hole.xPx * ratio) % 1).toBeCloseTo(0.5, 9);
      expect((hole.yPx * ratio) % 1).toBeCloseTo(0.5, 9);
    }
  });
});
