import { describe, expect, it, vi } from "vitest";

import { BANNED_SPATIAL_MEMBERS, type ContextCall, stubCanvas } from "../test/RecordingContext2D";
import type { Camera, Viewport } from "./camera";
import { buildDrawList, type DrawList, type DrawOp, type SymbolOp } from "./drawList";
import { localFrameAt } from "./frame";
import type { SpatialScene } from "./marks";
import { type ColourTokens, paint, readTokens, sameTokens, staleTokens } from "./paint";
import { symbolOutline } from "./symbols";
import { vec3 } from "./vec3";

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

/** Paints `ops` at a pixel ratio of 1 and returns what was recorded after the clear. */
function paintOps(ops: ReadonlyArray<DrawOp>) {
  const { recorder, context } = recordedContext();
  paint(context, listOf(ops), TOKENS, 1);
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
    const drawList = buildDrawList(EVERY_KIND, CAMERA, VIEWPORT);

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
    const kinds = new Set(buildDrawList(EVERY_KIND, CAMERA, VIEWPORT).ops.map((op) => op.kind));

    expect(kinds).toEqual(new Set(["line", "polyline", "circle", "symbol", "reticle", "ticks"]));
  });

  it("never writes text, and nothing is translucent, blended, blurred, shadowed or graded", () => {
    const { recorder, context } = recordedContext();

    paint(context, buildDrawList(EVERY_KIND, CAMERA, VIEWPORT), TOKENS, 1);

    const names = recorder.names();
    for (const banned of BANNED_SPATIAL_MEMBERS) {
      expect(names).not.toContain(banned);
    }
  });
});
