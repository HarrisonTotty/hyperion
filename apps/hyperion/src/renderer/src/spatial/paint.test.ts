import { describe, expect, it, vi } from "vitest";

import {
  lineScale,
  markShiftDevicePx,
  markStrokeDevicePx,
  minReticleGapDevicePx,
} from "../lib/strokes";
import { BANNED_SPATIAL_MEMBERS, type ContextCall, stubCanvas } from "../test/RecordingContext2D";
import type { Camera, Viewport } from "./camera";
import { buildDrawList, type DrawList, type DrawOp, type SymbolOp } from "./drawList";
import { localFrameAt } from "../geometry/frame";
import type { SpatialScene } from "./marks";
import { type ColourTokens, paint, readTokens, sameTokens, staleTokens } from "./paint";
import {
  type OutlinePoint,
  SIZE_CLASS_REM,
  SYMBOL_STROKE_PX,
  symbolOutline,
  unitInradius,
} from "./symbols";
import { vec3 } from "../geometry/vec3";

/** Tokens with values unlike the stylesheet's, so that each stroke can be traced to its token. */
const TOKENS: ColourTokens = {
  text: "#111111",
  accent: "#222222",
  target: "#333333",
  line: "#444444",
  surface0: "#555555",
  textMuted: "#666666",
};

function listOf(ops: ReadonlyArray<DrawOp>): DrawList {
  return { ops, anchors: [], curveLabels: [] };
}

function symbolOp(overrides: Partial<SymbolOp> = {}): SymbolOp {
  return {
    kind: "symbol",
    id: "a",
    centre: { xPx: 50, yPx: 40 },
    shape: "circle",
    radiusPx: 5,
    stroke: "text",
    fill: "text",
    widthPx: 1.5,
    ...overrides,
  };
}

/** A recorded context on a canvas of its own, 200 × 100 device pixels. */
function recordedContext() {
  const recorder = stubCanvas();
  const canvas = document.createElement("canvas");
  canvas.width = 200;
  canvas.height = 100;
  const context = canvas.getContext("2d");
  if (context === null) {
    throw new Error("the stubbed canvas gave no 2D context");
  }
  return { recorder, context };
}

/**
 * Paints `ops` at a pixel ratio, 2 unless given, where every width and outline is as written and
 * none is moved, and returns what was recorded after the clear.
 */
function paintOps(ops: ReadonlyArray<DrawOp>, pixelRatio = 2) {
  const { recorder, context } = recordedContext();
  paint(context, listOf(ops), TOKENS, pixelRatio);
  const records = recorder.records.slice(
    recorder.records.findIndex((record) => record.name === "fillRect") + 2,
  );
  return {
    recorder,
    /** The path calls made, as `[name, ...args]`. */
    path: records
      .filter((record): record is ContextCall => record.type === "call" && record.name !== "stroke")
      .map((record) => Array.of<unknown>(record.name).concat(record.args)),
  };
}

/** A scene that yields every kind of op: marks either side of the plane, spheres, a reticle. */
const EVERY_KIND: SpatialScene = {
  frame: localFrameAt(vec3(26_000, 0, 0)),
  points: [
    {
      id: "above",
      position: vec3(-10, 5, 8),
      shape: "circle",
      sizeClass: 2,
      status: "available",
      label: "ABOVE",
      labelPriority: 2,
    },
    {
      id: "below",
      position: vec3(10, -5, -8),
      shape: "diamond",
      sizeClass: 4,
      status: "plain",
      label: "BELOW",
      labelPriority: 1,
    },
  ],
  spheres: [
    { radius: 20, role: "range", label: "RANGE 20 ly SET" },
    { radius: 40, role: "data_edge", label: "QUERY EDGE 40 ly" },
  ],
  plane: { spacing: 10, extent: 40, rings: [{ radius: 20, label: "PLANE 20 ly" }] },
  selectedId: "above",
  destinationId: "below",
};

const CAMERA: Camera = { azimuthDeg: 30, elevationDeg: 30, pxPerUnit: 2 };
const VIEWPORT: Viewport = { widthPx: 200, heightPx: 100, remPx: 16 };
/** The reticles' least gap at a ratio of 1, CSS px. */
const GAP_PX = minReticleGapDevicePx(1);

/** The path call each op starts with: an arc for circles, a move for everything else. */
function leadingCall(op: DrawOp): "arc" | "moveTo" {
  let call: "arc" | "moveTo";
  switch (op.kind) {
    case "circle":
      call = "arc";
      break;
    case "symbol":
      call = symbolOutline(op.shape).kind === "circle" ? "arc" : "moveTo";
      break;
    case "line":
    case "polyline":
    case "reticle":
    case "ticks":
      call = "moveTo";
      break;
  }
  return call;
}

describe("readTokens", () => {
  it("reads the view's colours from the tokens in effect", () => {
    const element = document.createElement("div");
    document.body.append(element);

    expect(readTokens(element)).toEqual({
      text: "#c8d6e5",
      accent: "#5cc8e6",
      target: "#e879f9",
      line: "#1c2a3a",
      surface0: "#05080d",
      textMuted: "#8a9db3",
    });
    element.remove();
  });

  it("trims the space a computed custom property keeps", () => {
    // jsdom trims custom properties itself, where a browser keeps the stylesheet's leading space.
    const computed: Readonly<Record<string, string>> = {
      "--text": " #c8d6e5",
      "--accent": " #5cc8e6",
      "--target": " #e879f9 ",
      "--line": " #1c2a3a",
      "--surface-0": " #05080d",
      "--text-muted": " #8a9db3 ",
    };
    vi.spyOn(CSSStyleDeclaration.prototype, "getPropertyValue").mockImplementation(
      (property: string) => computed[property] ?? "",
    );

    expect(readTokens(document.body)).toEqual({
      text: "#c8d6e5",
      accent: "#5cc8e6",
      target: "#e879f9",
      line: "#1c2a3a",
      surface0: "#05080d",
      textMuted: "#8a9db3",
    });
  });

  it("names a token that is not set", () => {
    document.documentElement.style.removeProperty("--accent");

    expect(() => readTokens(document.body)).toThrow("--accent");
  });
});

describe("sameTokens", () => {
  it("holds for sets equal in every token and fails for one that differs in any", () => {
    expect(sameTokens(TOKENS, { ...TOKENS })).toBe(true);
    for (const name of ["text", "accent", "target", "line", "surface0", "textMuted"] as const) {
      expect(sameTokens(TOKENS, { ...TOKENS, [name]: "#000000" })).toBe(false);
    }
  });
});

describe("staleTokens", () => {
  it("draws every mark and curve of a stale view in --text-muted, keeping the grid and background", () => {
    expect(staleTokens(TOKENS)).toEqual({
      text: TOKENS.textMuted,
      accent: TOKENS.textMuted,
      target: TOKENS.textMuted,
      line: TOKENS.line,
      surface0: TOKENS.surface0,
      textMuted: TOKENS.textMuted,
    });
  });
});

describe("paint", () => {
  it("clears the whole backing store to --surface-0 before anything else", () => {
    const { recorder, context } = recordedContext();

    paint(context, listOf([symbolOp()]), TOKENS, 2);

    expect(recorder.calls("fillRect").map(({ args }) => args)).toEqual([[0, 0, 200, 100]]);
    const names = recorder.records.map((record) => record.name);
    expect(names.indexOf("fillRect")).toBeLessThan(names.indexOf("arc"));
    expect(recorder.sets("fillStyle")[0]).toBe(TOKENS.surface0);
  });

  it("clears unscaled, then draws the ops scaled by the pixel ratio", () => {
    const { recorder, context } = recordedContext();

    paint(context, listOf([symbolOp()]), TOKENS, 2);

    const names = recorder.records.map((record) => record.name);
    expect(names.slice(0, names.indexOf("arc"))).toEqual([
      "setTransform",
      "fillStyle",
      "fillRect",
      "setTransform",
      "beginPath",
    ]);
    expect(recorder.calls("setTransform").map(({ args }) => args)).toEqual([
      [1, 0, 0, 1, 0, 0],
      [2, 0, 0, 2, 0, 0],
    ]);
  });

  it("fills and strokes a symbol above the plane", () => {
    const { recorder } = paintOps([symbolOp({ fill: "accent", stroke: "accent" })]);

    const names = recorder.records.map((record) => record.name);
    expect(names.slice(names.indexOf("arc"))).toEqual([
      "arc",
      "fillStyle",
      "fill",
      "strokeStyle",
      "lineWidth",
      "stroke",
    ]);
    expect(recorder.sets("fillStyle").at(-1)).toBe(TOKENS.accent);
  });

  it("only strokes a symbol below the plane, with the same outline", () => {
    const { recorder, path } = paintOps([symbolOp({ fill: null })]);

    expect(recorder.calls("fill")).toEqual([]);
    expect(path).toEqual([["beginPath"], ["arc", 50, 40, 5, 0, 2 * Math.PI]]);
    expect(recorder.calls("stroke")).toHaveLength(1);
  });

  it("draws a polygon symbol as its unit outline scaled to the radius, closed", () => {
    const { path } = paintOps([symbolOp({ shape: "diamond", radiusPx: 4, fill: null })]);

    // The diamond's corners: up, right, down, left, and up again.
    expect(path).toEqual([
      ["beginPath"],
      ["moveTo", 50, 36],
      ["lineTo", 54, 40],
      ["lineTo", 50, 44],
      ["lineTo", 46, 40],
      ["lineTo", 50, 36],
      ["closePath"],
    ]);
  });

  it("fills a ringed circle's disc alone, then strokes the disc and its ring together", () => {
    const { recorder, path } = paintOps([symbolOp({ shape: "ringed-circle", radiusPx: 6 })]);

    const names = recorder.records.map((record) => record.name);
    expect(names.slice(names.indexOf("arc"))).toEqual([
      "arc",
      "fillStyle",
      "fill",
      "moveTo",
      "arc",
      "strokeStyle",
      "lineWidth",
      "stroke",
    ]);
    expect(path).toEqual([
      ["beginPath"],
      ["arc", 50, 40, 2, 0, 2 * Math.PI],
      ["fill"],
      ["moveTo", 56, 40],
      ["arc", 50, 40, 6, 0, 2 * Math.PI],
    ]);
  });

  it.each(["ringed-circle", "triangle-down", "pentagon", "hexagon"] as const)(
    "draws the %s open with the same path as filled, and no fill",
    (shape) => {
      const filled = paintOps([symbolOp({ shape, fill: "text" })]);
      const open = paintOps([symbolOp({ shape, fill: null })]);

      expect(open.recorder.calls("fill")).toEqual([]);
      expect(filled.recorder.calls("fill")).toHaveLength(1);
      expect(open.path).toEqual(filled.path.filter(([name]) => name !== "fill"));
      expect(open.recorder.calls("stroke")).toHaveLength(1);
    },
  );

  it("draws a line from its start to its end", () => {
    const { path } = paintOps([
      {
        kind: "line",
        from: { xPx: 3, yPx: 4 },
        to: { xPx: 30, yPx: 40 },
        stroke: "line",
        widthPx: 1,
        markId: null,
      },
    ]);

    expect(path).toEqual([["beginPath"], ["moveTo", 3, 4], ["lineTo", 30, 40]]);
  });

  it("draws a polyline as one path through its points", () => {
    const { recorder, path } = paintOps([
      {
        kind: "polyline",
        points: [
          { xPx: 0, yPx: 0 },
          { xPx: 10, yPx: 0 },
          { xPx: 10, yPx: 10 },
          { xPx: 0, yPx: 0 },
        ],
        stroke: "line",
        widthPx: 1,
      },
    ]);

    expect(path).toEqual([
      ["beginPath"],
      ["moveTo", 0, 0],
      ["lineTo", 10, 0],
      ["lineTo", 10, 10],
      ["lineTo", 0, 0],
    ]);
    expect(recorder.calls("stroke")).toHaveLength(1);
  });

  it("draws a circle as an arc of its own centre and radius", () => {
    const { path } = paintOps([
      { kind: "circle", centre: { xPx: 100, yPx: 50 }, radiusPx: 30, stroke: "text", widthPx: 1.5 },
    ]);

    expect(path).toEqual([["beginPath"], ["arc", 100, 50, 30, 0, 2 * Math.PI]]);
  });

  it("draws ticks as separate segments, stroked together", () => {
    const { recorder, path } = paintOps([
      {
        kind: "ticks",
        segments: [
          { from: { xPx: 0, yPx: 0 }, to: { xPx: 0, yPx: 4 } },
          { from: { xPx: 10, yPx: 0 }, to: { xPx: 10, yPx: 4 } },
        ],
        stroke: "text",
        widthPx: 1,
      },
    ]);

    expect(path).toEqual([
      ["beginPath"],
      ["moveTo", 0, 0],
      ["lineTo", 0, 4],
      ["moveTo", 10, 0],
      ["lineTo", 10, 4],
    ]);
    expect(recorder.calls("stroke")).toHaveLength(1);
  });

  it("draws a reticle as four corner brackets of its square", () => {
    const { recorder, path } = paintOps([
      {
        kind: "reticle",
        id: "a",
        centre: { xPx: 50, yPx: 40 },
        halfSizePx: 9,
        stroke: "accent",
        widthPx: 1.5,
      },
    ]);

    // Each corner is one arm, the corner, and the other arm, a third of the 18 px side each.
    expect(path).toEqual([
      ["beginPath"],
      ["moveTo", 41, 37],
      ["lineTo", 41, 31],
      ["lineTo", 47, 31],
      ["moveTo", 59, 37],
      ["lineTo", 59, 31],
      ["lineTo", 53, 31],
      ["moveTo", 59, 43],
      ["lineTo", 59, 49],
      ["lineTo", 53, 49],
      ["moveTo", 41, 43],
      ["lineTo", 41, 49],
      ["lineTo", 47, 49],
    ]);
    expect(recorder.calls("stroke")).toHaveLength(1);
  });

  it("strokes each op in the token value it names", () => {
    const { recorder } = paintOps([
      {
        kind: "line",
        from: { xPx: 0, yPx: 0 },
        to: { xPx: 10, yPx: 10 },
        stroke: "line",
        widthPx: 1,
        markId: null,
      },
      { kind: "circle", centre: { xPx: 100, yPx: 50 }, radiusPx: 30, stroke: "text", widthPx: 1 },
      symbolOp({ stroke: "accent", fill: null }),
      {
        kind: "reticle",
        id: "a",
        centre: { xPx: 50, yPx: 40 },
        halfSizePx: 9,
        stroke: "target",
        widthPx: 1,
      },
      {
        kind: "polyline",
        points: [
          { xPx: 0, yPx: 0 },
          { xPx: 10, yPx: 0 },
        ],
        stroke: "textMuted",
        widthPx: 1,
      },
    ]);

    expect(recorder.sets("strokeStyle")).toEqual([
      TOKENS.line,
      TOKENS.text,
      TOKENS.accent,
      TOKENS.target,
      TOKENS.textMuted,
    ]);
  });

  it("strokes each op at its own width", () => {
    const { recorder } = paintOps([
      { kind: "circle", centre: { xPx: 100, yPx: 50 }, radiusPx: 30, stroke: "text", widthPx: 1.5 },
      { kind: "circle", centre: { xPx: 100, yPx: 50 }, radiusPx: 40, stroke: "text", widthPx: 1 },
    ]);

    expect(recorder.sets("lineWidth")).toEqual([1.5, 1]);
  });

  it("paints the ops in the order of the draw list", () => {
    const { recorder, context } = recordedContext();
    const drawList = buildDrawList(EVERY_KIND, CAMERA, VIEWPORT, GAP_PX);

    paint(context, drawList, TOKENS, 1);

    // Each op begins a path with its own leading call and is stroked once in its own token.
    const leading = recorder.records
      .filter((_record, index) => recorder.records[index - 1]?.name === "beginPath")
      .map((record) => record.name);
    expect(leading).toEqual(drawList.ops.map(leadingCall));
    expect(recorder.sets("strokeStyle")).toEqual(drawList.ops.map((op) => TOKENS[op.stroke]));
    expect(recorder.calls("stroke")).toHaveLength(drawList.ops.length);
  });

  it("is checked against a scene that has every kind of op", () => {
    const kinds = new Set(
      buildDrawList(EVERY_KIND, CAMERA, VIEWPORT, GAP_PX).ops.map((op) => op.kind),
    );

    expect(kinds).toEqual(new Set(["line", "polyline", "circle", "symbol", "reticle", "ticks"]));
  });

  it("never writes text, and nothing is translucent, blended, blurred, shadowed or graded", () => {
    const { recorder, context } = recordedContext();

    paint(context, buildDrawList(EVERY_KIND, CAMERA, VIEWPORT, GAP_PX), TOKENS, 1);

    const names = recorder.names();
    for (const banned of BANNED_SPATIAL_MEMBERS) {
      expect(names).not.toContain(banned);
    }
  });
});

/** The ratios in use: the development machine's and the UHD 620's, 100%, a Retina display, and 3. */
const RATIOS = [0.78125, 1, 2, 3] as const;

/** A figure to the hundredth, as the ruling gives its figures. */
function hundredths(value: number): number {
  // Plus zero, so that a shift a hair under zero reads as none rather than -0.
  return Math.round(value * 100) / 100 + 0;
}

/** The first `lineWidth` a paint of `ops` at `pixelRatio` set. */
function firstWidth(ops: ReadonlyArray<DrawOp>, pixelRatio: number): number {
  return Number(paintOps(ops, pixelRatio).recorder.sets("lineWidth")[0]);
}

/** The radius of the `index`th arc a paint of `op` at `pixelRatio` drew. */
function arcRadius(op: DrawOp, pixelRatio: number, index = 0): number {
  return Number(paintOps([op], pixelRatio).recorder.calls("arc")[index]?.args[2]);
}

/** The points a paint of a polygon symbol at `pixelRatio` traced, before it closed. */
function tracedCorners(op: SymbolOp, pixelRatio: number): Array<{ x: number; y: number }> {
  return paintOps([op], pixelRatio)
    .path.filter(([name]) => name === "moveTo" || name === "lineTo")
    .slice(0, -1)
    .map(([, x, y]) => ({ x: Number(x), y: Number(y) }));
}

/** The distance from a symbol's centre to the nearest side of the polygon through `corners`. */
function nearestSide(op: SymbolOp, corners: ReadonlyArray<{ x: number; y: number }>): number {
  return Math.min(
    ...corners.map((a, i) => {
      const b = corners[(i + 1) % corners.length] ?? a;
      const ax = a.x - op.centre.xPx;
      const ay = a.y - op.centre.yPx;
      const bx = b.x - op.centre.xPx;
      const by = b.y - op.centre.yPx;
      return Math.abs(ax * by - ay * bx) / Math.hypot(bx - ax, by - ay);
    }),
  );
}

/** A reticle about (50, 40), half-size 9 px. */
const RETICLE: DrawOp = {
  kind: "reticle",
  id: "a",
  centre: { xPx: 50, yPx: 40 },
  halfSizePx: 9,
  stroke: "accent",
  widthPx: 1.5,
};

/** An orbit-like circle `widthPx` wide. */
function orbit(widthPx: number): DrawOp {
  return { kind: "circle", centre: { xPx: 100, yPx: 50 }, radiusPx: 30, stroke: "text", widthPx };
}

/** A polygon shape's unit corners. */
function polygonPoints(shape: SymbolOp["shape"]): ReadonlyArray<OutlinePoint> {
  const outline = symbolOutline(shape);
  if (outline.kind !== "polygon") {
    throw new Error(`the ${shape} is not a polygon`);
  }
  return outline.points;
}

/** One op of each kind that is a line, not a mark, 1.5 px wide. */
const LINE_OPS: ReadonlyArray<DrawOp> = [
  {
    kind: "line",
    from: { xPx: 3, yPx: 4 },
    to: { xPx: 30, yPx: 40 },
    stroke: "line",
    widthPx: 1.5,
    markId: null,
  },
  {
    kind: "polyline",
    points: [
      { xPx: 0, yPx: 0 },
      { xPx: 10, yPx: 0 },
    ],
    stroke: "textMuted",
    widthPx: 1.5,
  },
  { kind: "circle", centre: { xPx: 100, yPx: 50 }, radiusPx: 30, stroke: "text", widthPx: 1.5 },
  {
    kind: "ticks",
    segments: [{ from: { xPx: 0, yPx: 0 }, to: { xPx: 0, yPx: 4 } }],
    stroke: "text",
    widthPx: 1.5,
  },
];

describe("paint at the display's ratio (R07.T16.f; decision-thin-line-contrast, item 2)", () => {
  it.each(LINE_OPS.map((op) => [op.kind, op] as const))(
    "strokes a %s at 2.56, 2, 1 and 1 times its width",
    (_kind, op) => {
      expect(RATIOS.map((ratio) => hundredths(firstWidth([op], ratio) / 1.5))).toEqual([
        2.56, 2, 1, 1,
      ]);
    },
  );

  it("draws every line at no less than 2 device px, the lines keeping their ratios", () => {
    for (const ratio of RATIOS) {
      const thin = firstWidth([orbit(1)], ratio);
      const selected = firstWidth([orbit(2)], ratio);
      expect(thin * ratio).toBeCloseTo(lineScale(ratio), 9);
      expect(selected / thin).toBeCloseTo(2, 9);
    }
  });

  it("strokes a symbol's and a reticle's outline at 2.56, 2, 1.5 and 1.5 px, whatever its width", () => {
    const symbol = symbolOp({ widthPx: 1 });
    const reticle = { ...RETICLE, widthPx: 1 };

    expect(RATIOS.map((ratio) => hundredths(firstWidth([symbol], ratio)))).toEqual([
      2.56, 2, 1.5, 1.5,
    ]);
    expect(RATIOS.map((ratio) => hundredths(firstWidth([reticle], ratio)))).toEqual([
      2.56, 2, 1.5, 1.5,
    ]);
    for (const ratio of RATIOS) {
      expect(firstWidth([symbol], ratio) * ratio).toBeCloseTo(markStrokeDevicePx(ratio), 9);
    }
  });

  it("moves an open circle's arc out by 0.53, 0.25, 0 and 0 px", () => {
    const op = symbolOp({ fill: null, radiusPx: 5 });

    expect(RATIOS.map((ratio) => hundredths(arcRadius(op, ratio) - 5))).toEqual([0.53, 0.25, 0, 0]);
  });

  it("moves an open inverted triangle's corners out by 1.06, 0.5, 0 and 0 px, its sides by 0.53, 0.25, 0 and 0", () => {
    const op = symbolOp({ shape: "triangle-down", fill: null, radiusPx: 5 });
    const corners = RATIOS.map((ratio) => tracedCorners(op, ratio));

    expect(
      corners.map((points) =>
        hundredths(Math.hypot((points[0]?.x ?? 0) - 50, (points[0]?.y ?? 0) - 40) - 5),
      ),
    ).toEqual([1.06, 0.5, 0, 0]);
    // The triangle's sides lie at half its corners' radius from its centroid.
    expect(corners.map((points) => hundredths(nearestSide(op, points) - 2.5))).toEqual([
      0.53, 0.25, 0, 0,
    ]);
  });

  it.each(["diamond", "square", "triangle", "pentagon", "hexagon"] as const)(
    "moves every side of the %s out by the outline's shift",
    (shape) => {
      const op = symbolOp({ shape, fill: null, radiusPx: 5 });
      const inradius = unitInradius(polygonPoints(shape));
      for (const ratio of RATIOS) {
        expect(nearestSide(op, tracedCorners(op, ratio)) - 5 * inradius).toBeCloseTo(
          markShiftDevicePx(ratio) / ratio,
          9,
        );
      }
    },
  );

  it("moves a ringed circle's disc out by 0.53, 0.25, 0 and 0 px and its ring by 1.59, 0.75, 0 and 0", () => {
    const op = symbolOp({ shape: "ringed-circle", fill: null, radiusPx: 6 });

    expect(RATIOS.map((ratio) => hundredths(arcRadius(op, ratio, 0) - 2))).toEqual([
      0.53, 0.25, 0, 0,
    ]);
    expect(RATIOS.map((ratio) => hundredths(arcRadius(op, ratio, 1) - 6))).toEqual([
      1.59, 0.75, 0, 0,
    ]);
  });

  it("starts the ring's arc where the moved ring begins, so that no line joins it to the disc", () => {
    const op = symbolOp({ shape: "ringed-circle", fill: null, radiusPx: 6 });
    const { path } = paintOps([op], 0.78125);
    const ring = arcRadius(op, 0.78125, 1);

    expect(path.find(([name]) => name === "moveTo")).toEqual(["moveTo", 50 + ring, 40]);
  });

  it("stands a destination on the selection outside its bracket by the larger of 0.25 rem and 5.12, 4, 2.5 and 2.5 px", () => {
    const scene: SpatialScene = { ...EVERY_KIND, selectedId: "above", destinationId: "above" };
    const gaps = RATIOS.map((ratio) => {
      const list = buildDrawList(scene, CAMERA, VIEWPORT, minReticleGapDevicePx(ratio) / ratio);
      const lines = paintOps(
        list.ops.filter((op) => op.kind === "reticle"),
        ratio,
      ).path.filter(([name]) => name === "lineTo");
      // Each reticle traces four corners of two lines each, its corner point first: the first
      // corner's lies its half-size left of the mark, the bracket's eight lines before the other's.
      return hundredths(Number(lines[0]?.[1]) - Number(lines[8]?.[1]));
    });

    expect(gaps).toEqual([5.12, 4, 4, 4]);
  });

  it("moves a reticle's half-size out by 2.12, 1, 0 and 0 px", () => {
    const halfSizes = RATIOS.map((ratio) => {
      // The first corner's corner point, up and to the left of the centre.
      const corner = paintOps([RETICLE], ratio).path.find(([name]) => name === "lineTo");
      return 50 - Number(corner?.[1]);
    });

    expect(halfSizes.map((half) => hundredths(half - 9))).toEqual([2.12, 1, 0, 0]);
  });

  it.each([
    [1, 1.56],
    [0.8, 0.94],
  ] as const)(
    "keeps a class-2 open ringed circle's hole and the gap round its disc as built at 0.78125: at %s, %s device px",
    (interfaceScale, expectedPx) => {
      const ratio = 0.78125;
      const remPx = 16 * interfaceScale;
      // As the draw list places it: the size class's radius less half the 1.5 px outline.
      const radiusPx = (SIZE_CLASS_REM[2] * remPx) / 2 - SYMBOL_STROKE_PX / 2;
      const op = symbolOp({ shape: "ringed-circle", fill: null, radiusPx });
      const half = firstWidth([op], ratio) / 2;
      const disc = arcRadius(op, ratio, 0);
      const ring = arcRadius(op, ratio, 1);
      const asBuiltHole = 2 * (radiusPx / 3 - SYMBOL_STROKE_PX / 2) * ratio;

      expect(hundredths(asBuiltHole)).toBe(expectedPx);
      expect(hundredths(2 * (disc - half) * ratio)).toBe(expectedPx);
      expect(hundredths((ring - half - (disc + half)) * ratio)).toBe(expectedPx);
    },
  );
});
