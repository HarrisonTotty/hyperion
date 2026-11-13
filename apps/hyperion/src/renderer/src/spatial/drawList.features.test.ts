import { describe, expect, it } from "vitest";

import { localFrameAt } from "../geometry/frame";
import { add, scale, vec3, type Vec3 } from "../geometry/vec3";
import { minReticleGapDevicePx } from "../lib/strokes";
import type { Camera, Viewport } from "./camera";
import { buildDrawList, type CircleOp, type DrawOp, type SymbolOp } from "./drawList";
import type { FeatureMark, PointMark, ShellMark, SpatialScene } from "./marks";
import { pick } from "./pick";
import { SIZE_CLASS_REM } from "./symbols";

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

function system(id: string, position: Vec3): PointMark {
  return {
    id,
    position,
    shape: "circle",
    sizeClass: 2,
    status: "plain",
    label: id,
    labelPriority: 1,
  };
}

function feature(id: string, position: Vec3, radius: number): FeatureMark {
  return {
    id,
    position,
    shape: "greek-cross",
    sizeClass: 4,
    status: "plain",
    label: `OC ${id}`,
    labelPriority: 1e12,
    radius,
  };
}

function shell(id: string, centre: Vec3, radius: number, label = `SUPERBUBBLE ${id}`): ShellMark {
  return { id, centre, radius, label };
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

function circles(ops: ReadonlyArray<DrawOp>): CircleOp[] {
  return ops.filter((op): op is CircleOp => op.kind === "circle");
}

function symbols(ops: ReadonlyArray<DrawOp>): SymbolOp[] {
  return ops.filter((op): op is SymbolOp => op.kind === "symbol");
}

/** Half a feature mark's width at 100%: the largest size class's. */
const FEATURE_MARK_RADIUS_PX = (SIZE_CLASS_REM[4] * VIEWPORT.remPx) / 2;

describe("buildDrawList with features and shells (plan 09, P09.T41.a)", () => {
  it("draws a feature's mark as the Greek cross, filled north of the plane and open south of it", () => {
    const scene = sceneOf({
      features: [feature("north", local(10, 0, 2), 0.1), feature("south", local(-10, 0, -2), 0.1)],
    });

    const marks = symbols(buildDrawList(scene, TOP, VIEWPORT, GAP_PX).ops);

    expect(marks.map((op) => [op.id, op.shape, op.fill])).toEqual([
      ["south", "greek-cross", null],
      ["north", "greek-cross", "text"],
    ]);
  });

  it("draws a feature's circle at its true radius about its mark where it is wider than the mark", () => {
    const scene = sceneOf({ features: [feature("cluster", local(10, -4, 0), 20)] });

    const drawn = circles(buildDrawList(scene, TOP, VIEWPORT, GAP_PX).ops);

    expect(drawn).toHaveLength(1);
    expect(drawn[0]?.centre.xPx).toBeCloseTo(onScreen(10, -4).xPx, 9);
    expect(drawn[0]?.centre.yPx).toBeCloseTo(onScreen(10, -4).yPx, 9);
    expect(drawn[0]?.radiusPx).toBeCloseTo(20 * K, 9);
    expect(drawn[0]?.stroke).toBe("textMuted");
    expect(drawn[0]?.widthPx).toBe(1);
  });

  it("draws no circle for a feature no wider on the screen than its mark", () => {
    const radius = FEATURE_MARK_RADIUS_PX / K;
    const scene = sceneOf({ features: [feature("small", local(0, 0, 0), radius)] });

    expect(circles(buildDrawList(scene, TOP, VIEWPORT, GAP_PX).ops)).toEqual([]);
  });

  it("draws a shell's circle at its true radius about its centre, labelled at the circle", () => {
    const scene = sceneOf({ shells: [shell("GPF F207", local(4, 2, 0), 30)] });

    const list = buildDrawList(scene, TOP, VIEWPORT, GAP_PX);

    expect(circles(list.ops).map((op) => [op.radiusPx, op.stroke])).toEqual([
      [30 * K, "textMuted"],
    ]);
    const label = list.curveLabels.find((each) => each.key === "shell:GPF F207");
    expect(label?.text).toBe("SUPERBUBBLE GPF F207");
    expect(label?.placement).toBe("circle-top");
    expect(label?.xPx).toBeCloseTo(onScreen(4, 2).xPx, 9);
    expect(label?.yPx).toBeCloseTo(onScreen(4, 2).yPx - 30 * K, 9);
  });

  it("labels no shell whose label is empty", () => {
    const scene = sceneOf({ shells: [shell("quiet", local(0, 0, 0), 30, "")] });

    expect(buildDrawList(scene, TOP, VIEWPORT, GAP_PX).curveLabels).toEqual([]);
  });

  it("draws every circle at true radius before every mark, so that the marks lie on top", () => {
    const scene = sceneOf({
      points: [system("a", local(1, 1, 0))],
      features: [feature("cluster", local(0, 0, 0), 20)],
      shells: [shell("bubble", local(0, 0, 0), 40)],
    });

    const { ops } = buildDrawList(scene, TOP, VIEWPORT, GAP_PX);
    const firstSymbol = ops.findIndex((op) => op.kind === "symbol");

    expect(ops.slice(0, 2).map((op) => op.kind)).toEqual(["circle", "circle"]);
    expect(firstSymbol).toBeGreaterThan(1);
  });

  it("anchors a feature's mark with the systems', so that it is picked, and never a shell", () => {
    const scene = sceneOf({
      points: [system("a", local(10, 0, 0))],
      features: [feature("cluster", local(-10, 0, 0), 20)],
      shells: [shell("bubble", local(0, 20, 0), 5)],
    });

    const { anchors } = buildDrawList(scene, TOP, VIEWPORT, GAP_PX);

    expect(anchors.map((anchor) => anchor.id).toSorted()).toEqual(["a", "cluster"]);
    expect(anchors.find((anchor) => anchor.id === "cluster")?.radiusPx).toBe(
      FEATURE_MARK_RADIUS_PX,
    );
    expect(pick(anchors, onScreen(-10, 0), 16)).toBe("cluster");
    expect(pick(anchors, onScreen(0, 20), 16)).toBeNull();
  });

  it("brackets a selected feature as it brackets a selected system", () => {
    const scene = sceneOf({
      features: [feature("cluster", local(-10, 0, 0), 20)],
      selectedId: "cluster",
    });

    const reticles = buildDrawList(scene, TOP, VIEWPORT, GAP_PX).ops.filter(
      (op) => op.kind === "reticle",
    );

    expect(reticles.map((op) => op.id)).toEqual(["cluster"]);
  });

  it("draws a scene without features or shells as before", () => {
    const bare = sceneOf({ points: [system("a", local(1, 1, 0))] });

    expect(buildDrawList({ ...bare, features: [], shells: [] }, TOP, VIEWPORT, GAP_PX)).toEqual(
      buildDrawList(bare, TOP, VIEWPORT, GAP_PX),
    );
  });
});
