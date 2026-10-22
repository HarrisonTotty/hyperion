import { describe, expect, it } from "vitest";

import { localFrameAt } from "../geometry/frame";
import { add, scale, vec3, type Vec3 } from "../geometry/vec3";
import { minReticleGapDevicePx } from "../lib/strokes";
import type { Camera, Viewport } from "./camera";
import {
  buildDrawList,
  type DrawOp,
  type LineOp,
  OBSERVER_MARK_ID,
  observerBoxPx,
  SEGMENT_DASH_PX,
} from "./drawList";
import type { PointMark, SegmentMark, SpatialScene } from "./marks";
import { pick } from "./pick";
import { SIZE_CLASS_REM, SYMBOL_STROKE_PX } from "./symbols";

const FRAME = localFrameAt(vec3(26_000, 0, 0));
const VIEWPORT: Viewport = { widthPx: 800, heightPx: 600, remPx: 16 };
const GAP_PX = minReticleGapDevicePx(1);
const K = 5;

/** Straight down onto the plane, coreward up the screen and spinward to its right. */
const TOP: Camera = { azimuthDeg: 0, elevationDeg: 90, pxPerUnit: K };

/** A position from coreward, spinward and north components. */
function local(coreward: number, spinward: number, north: number): Vec3 {
  return add(
    add(scale(FRAME.coreward, coreward), scale(FRAME.spinward, spinward)),
    scale(FRAME.north, north),
  );
}

/** Where `TOP` puts a point on the plane `coreward` and `spinward` of the centre. */
function onScreen(coreward: number, spinward: number): { xPx: number; yPx: number } {
  return { xPx: 400 + K * spinward, yPx: 300 - K * coreward };
}

function mark(id: string, position: Vec3, overrides: Partial<PointMark> = {}): PointMark {
  return {
    id,
    position,
    shape: "circle",
    sizeClass: 2,
    status: "plain",
    label: id,
    labelPriority: 1,
    ...overrides,
  };
}

function segment(from: Vec3, to: Vec3, overrides: Partial<SegmentMark> = {}): SegmentMark {
  return {
    id: "segment",
    from,
    to,
    line: "solid",
    stroke: "textMuted",
    widthPx: 1,
    ...overrides,
  };
}

function sceneOf(overrides: Partial<SpatialScene>): SpatialScene {
  return {
    frame: FRAME,
    points: [],
    spheres: [],
    plane: { spacing: 20, extent: 50, rings: [] },
    selectedId: null,
    destinationId: null,
    ...overrides,
  };
}

/** The ops a scene adds to the draw list of the same scene without its segments and observer. */
function addedOps(scene: SpatialScene, camera: Camera = TOP): DrawOp[] {
  const bare = buildDrawList(
    { ...scene, segments: undefined, observer: undefined },
    camera,
    VIEWPORT,
    GAP_PX,
  ).ops;
  const full = buildDrawList(scene, camera, VIEWPORT, GAP_PX).ops;
  return full.filter((op) => !bare.includes(op) && !bare.some((each) => sameOp(each, op)));
}

function sameOp(a: DrawOp, b: DrawOp): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function lines(ops: ReadonlyArray<DrawOp>): LineOp[] {
  return ops.filter((op): op is LineOp => op.kind === "line");
}

/** A point to the nanopixel, its zeros unsigned, so that two computations of it compare equal. */
function nanopixels(
  point: { xPx: number; yPx: number } | undefined,
): { xPx: number; yPx: number } | undefined {
  return point === undefined
    ? undefined
    : { xPx: Math.round(point.xPx * 1e9) / 1e9 + 0, yPx: Math.round(point.yPx * 1e9) / 1e9 + 0 };
}

function expectPoint(actual: { xPx: number; yPx: number } | undefined, xPx: number, yPx: number) {
  expect(actual?.xPx).toBeCloseTo(xPx, 9);
  expect(actual?.yPx).toBeCloseTo(yPx, 9);
}

describe("a scene's segments (plan 12, P12.T11)", () => {
  it("draws a solid segment as a line between its projected ends, its colour and width its own", () => {
    const ops = lines(
      addedOps(sceneOf({ segments: [segment(local(0, 0, 0), local(10, 20, 0), { widthPx: 2 })] })),
    );

    expect(ops).toHaveLength(1);
    const [line] = ops;
    expectPoint(line?.from, 400, 300);
    expectPoint(line?.to, 500, 250);
    expect(line?.stroke).toBe("textMuted");
    expect(line?.widthPx).toBe(2);
    expect(line?.markId).toBeNull();
    expect(line).not.toHaveProperty("dashPx");
  });

  it("draws a dashed segment at a predicted path's dash, 6 px on and 4 off", () => {
    const [line] = lines(
      addedOps(
        sceneOf({ segments: [segment(local(0, 0, 0), local(10, 0, 0), { line: "dashed" })] }),
      ),
    );

    expect(line?.dashPx).toEqual(SEGMENT_DASH_PX);
    expect(SEGMENT_DASH_PX).toEqual({ onPx: 6, offPx: 4 });
  });

  it.each([
    ["left", local(0, 0, 0), local(0, -200, 0), 0, 300],
    ["right", local(0, 0, 0), local(0, 200, 0), 800, 300],
    ["top", local(0, 0, 0), local(100, 0, 0), 400, 0],
    ["bottom", local(0, 0, 0), local(-100, 0, 0), 400, 600],
  ] as const)("clips a segment at the view's %s edge", (_edge, from, to, xPx, yPx) => {
    const [line] = lines(addedOps(sceneOf({ segments: [segment(from, to)] })));

    expect([nanopixels(line?.from), nanopixels(line?.to)]).toEqual([
      { xPx: 400, yPx: 300 },
      { xPx, yPx },
    ]);
  });

  it("clips a segment that crosses the view at both ends", () => {
    const [line] = lines(
      addedOps(sceneOf({ segments: [segment(local(0, -200, 0), local(0, 200, 0))] })),
    );

    expect([nanopixels(line?.from), nanopixels(line?.to)]).toEqual([
      { xPx: 0, yPx: 300 },
      { xPx: 800, yPx: 300 },
    ]);
  });

  it("draws nothing for a segment wholly outside the view", () => {
    const outside = segment(local(0, 100, 0), local(10, 200, 0));

    expect(addedOps(sceneOf({ segments: [outside] }))).toEqual([]);
  });

  it("is never an anchor, and so never picked", () => {
    const scene = sceneOf({ segments: [segment(local(0, -20, 0), local(0, 20, 0))] });
    const list = buildDrawList(scene, TOP, VIEWPORT, GAP_PX);

    expect(list.anchors).toEqual([]);
    expect(pick(list.anchors, { xPx: 400, yPx: 300 }, VIEWPORT.remPx)).toBeNull();
  });

  it("draws only where it is longer on the screen than its least", () => {
    // 0.8 ly is 4 px at TOP's 5 px a light-year.
    const at = (lengthLy: number) =>
      addedOps(
        sceneOf({ segments: [segment(local(0, 0, 0), local(0, lengthLy, 0), { minLengthPx: 4 })] }),
      );

    expect(at(0.8)).toEqual([]);
    expect(lines(at(0.82))).toHaveLength(1);
  });

  it("starts outside the outline of the mark it leaves, and crosses its end with a tick", () => {
    const radiusPx = (SIZE_CLASS_REM[2] * VIEWPORT.remPx) / 2;
    const scene = sceneOf({
      points: [mark("m", local(0, 0, 0))],
      segments: [segment(local(0, 0, 0), local(0, 4, 0), { fromMark: "m", endTick: true })],
    });

    const [joiner, tick] = lines(addedOps(scene));

    expectPoint(joiner?.from, 400 + radiusPx, 300);
    expectPoint(joiner?.to, 420, 300);
    // Half a rem long, square to the joiner on the screen, and centred on its end (P12.T11.c).
    expectPoint(tick?.from, 420, 304);
    expectPoint(tick?.to, 420, 296);
    expect(tick).not.toHaveProperty("dashPx");
  });

  it("draws neither joiner nor tick with its end inside the mark's outline", () => {
    const scene = sceneOf({
      points: [mark("m", local(0, 0, 0))],
      segments: [segment(local(0, 0, 0), local(0, 1, 0), { fromMark: "m", endTick: true })],
    });

    expect(addedOps(scene)).toEqual([]);
  });

  it("draws the tick in the segment's colour and width, the selected one's included", () => {
    const scene = sceneOf({
      points: [mark("m", local(0, 0, 0))],
      segments: [
        segment(local(0, 0, 0), local(4, 0, 0), {
          fromMark: "m",
          endTick: true,
          line: "dashed",
          stroke: "text",
          widthPx: 2,
        }),
      ],
    });

    const [joiner, tick] = lines(addedOps(scene));
    expect(joiner?.dashPx).toEqual(SEGMENT_DASH_PX);
    expect([joiner?.stroke, joiner?.widthPx, tick?.stroke, tick?.widthPx]).toEqual([
      "text",
      2,
      "text",
      2,
    ]);
  });

  it("draws the segments before every mark, so that none crosses another's symbol", () => {
    const scene = sceneOf({
      points: [mark("m", local(0, 0, 0)), mark("n", local(30, 30, -5))],
      spheres: [{ radius: 40, role: "data_edge", label: "QUERY EDGE 40 ly" }],
      segments: [segment(local(0, 0, 0), local(0, 4, 0), { fromMark: "m", line: "dashed" })],
    });
    // The dashed line is the segment: no other op of the scene is dashed.
    const kinds = buildDrawList(scene, TOP, VIEWPORT, GAP_PX).ops.map((op) =>
      op.kind === "line" && op.dashPx !== undefined ? "segment" : op.kind,
    );

    expect(kinds[0]).toBe("segment");
    expect(kinds.indexOf("segment")).toBeLessThan(kinds.indexOf("symbol"));
  });

  it("draws the observer after every mark and before the sphere outlines", () => {
    const scene = sceneOf({
      points: [mark("m", local(0, 0, 0)), mark("n", local(30, 30, -5))],
      spheres: [{ radius: 40, role: "data_edge", label: "QUERY EDGE 40 ly" }],
      observer: { position: local(10, 10, 2), label: "OBSERVER" },
    });
    const kinds = buildDrawList(scene, TOP, VIEWPORT, GAP_PX).ops.map((op) =>
      op.kind === "polyline" && op.weight === "mark" ? "observer" : op.kind,
    );

    expect(kinds.indexOf("observer")).toBeGreaterThan(kinds.lastIndexOf("symbol"));
    expect(kinds.indexOf("observer")).toBeLessThan(kinds.indexOf("circle"));
  });
});

describe("a scene's observer (plan 12, P12.T11)", () => {
  const observed = (position: Vec3): SpatialScene =>
    sceneOf({ observer: { position, label: "OBSERVER" } });

  it.each([
    ["filled north of the plane", 6, "text"],
    ["open south of it", -6, null],
  ] as const)("draws its eye %s, as every mark is", (_where, northLy, fill) => {
    const [, eye] = addedOps(observed(local(10, 20, northLy)));

    expect(eye?.kind === "polyline" ? eye.fill : undefined).toBe(fill);
  });

  it("draws its stalk to its foot on the plane, then its eye at a mark's outline", () => {
    const ops = addedOps(observed(local(10, 20, 6)), {
      azimuthDeg: 0,
      elevationDeg: 0,
      pxPerUnit: K,
    });

    const [stalk, eye] = ops;
    expect(stalk?.kind).toBe("line");
    if (stalk?.kind !== "line" || eye?.kind !== "polyline") {
      throw new Error("the observer is not a stalk and a polyline");
    }
    // Seen from the side, north is up the screen and the plane its middle row.
    expectPoint(stalk.from, 400 + K * 20, 300 - K * 6);
    expectPoint(stalk.to, 400 + K * 20, 300);
    expect(stalk.markId).toBe(OBSERVER_MARK_ID);
    expect(stalk.stroke).toBe("text");
    expect(eye.weight).toBe("mark");
    expect(eye.widthPx).toBe(SYMBOL_STROKE_PX);
    expect(eye.stroke).toBe("text");
  });

  it("draws an eye 1 rem wide and half as tall, closed where its outline is smooth", () => {
    const [, eye] = addedOps(observed(local(0, 0, 0)));
    if (eye?.kind !== "polyline") {
      throw new Error("no eye");
    }
    const { points } = eye;
    const xs = points.map((point) => point.xPx);
    const ys = points.map((point) => point.yPx);
    const halfWidthPx = VIEWPORT.remPx / 2 - SYMBOL_STROKE_PX / 2;

    expect(Math.min(...xs)).toBeCloseTo(400 - halfWidthPx, 9);
    expect(Math.max(...xs)).toBeCloseTo(400 + halfWidthPx, 9);
    expect(Math.max(...ys) - Math.min(...ys)).toBeCloseTo(halfWidthPx, 9);
    expect(points[0]).toEqual(points.at(-1));
    expectPoint(points[0], 400, 300 - halfWidthPx / 2);
  });

  it("labels it 0.25 rem outside a bracket's place about it, as a mark is, and is no anchor", () => {
    const list = buildDrawList(observed(local(10, 0, 0)), TOP, VIEWPORT, GAP_PX);

    expect(list.anchors).toEqual([]);
    // The bracket about a mark 1 rem across stands 0.75 rem out; the text 0.25 rem beyond it.
    expect(list.curveLabels).toContainEqual({
      key: "observer",
      text: "OBSERVER",
      placement: "ring",
      ...onScreen(10, 0),
      stack: 0,
      clearOfPointPx: VIEWPORT.remPx,
    });
  });

  it("draws nothing for an observer outside the view", () => {
    const list = buildDrawList(observed(local(0, 100, 0)), TOP, VIEWPORT, GAP_PX);

    expect(addedOps(observed(local(0, 100, 0)))).toEqual([]);
    expect(list.curveLabels.some((label) => label.key === "observer")).toBe(false);
  });
});

describe("the observer's eye's box, which the marks' labels keep off (plan 12, P12.T11.c)", () => {
  const observed = (position: Vec3): SpatialScene =>
    sceneOf({ observer: { position, label: "OBSERVER" } });

  it("is the 1 rem square about the eye where it is drawn", () => {
    const { xPx, yPx } = onScreen(10, -4);

    expect(observerBoxPx(observed(local(10, -4, 0)), TOP, VIEWPORT)).toEqual({
      leftPx: xPx - VIEWPORT.remPx / 2,
      topPx: yPx - VIEWPORT.remPx / 2,
      widthPx: VIEWPORT.remPx,
      heightPx: VIEWPORT.remPx,
    });
  });

  it("is none in now, which has no observer", () => {
    expect(observerBoxPx(sceneOf({}), TOP, VIEWPORT)).toBeNull();
  });

  it("is none for an observer outside the view, where no eye is drawn", () => {
    expect(observerBoxPx(observed(local(0, 100, 0)), TOP, VIEWPORT)).toBeNull();
  });
});
