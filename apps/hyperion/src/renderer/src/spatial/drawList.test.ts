import { describe, expect, it } from "vitest";

import { between, seededRandom } from "../test/seededRandom";
import type { Camera, Viewport } from "./camera";
import { buildDrawList, type DrawOp } from "./drawList";
import { localFrameAt } from "./frame";
import type { PointMark, SizeClass, SpatialScene, SphereMark } from "./marks";
import { pick } from "./pick";
import { SIZE_CLASS_REM, SYMBOL_STROKE_PX } from "./symbols";
import { add, dot, scale, vec3, type Vec3 } from "./vec3";

const FRAME = localFrameAt(vec3(26_000, 0, 0));
const VIEWPORT: Viewport = { widthPx: 800, heightPx: 600, remPx: 16 };
const CENTRE = { xPx: 400, yPx: 300 };
const K = 5;

/** A position from coreward, spinward and north components. */
function local(coreward: number, spinward: number, north: number): Vec3 {
  return add(
    add(scale(FRAME.coreward, coreward), scale(FRAME.spinward, spinward)),
    scale(FRAME.north, north),
  );
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

function sceneOf(
  points: ReadonlyArray<PointMark>,
  overrides: Partial<SpatialScene> = {},
): SpatialScene {
  return {
    frame: FRAME,
    points,
    spheres: [],
    plane: {
      spacing: 20,
      extent: 50,
      rings: [
        { radius: 20, label: "" },
        { radius: 40, label: "" },
      ],
    },
    selectedId: null,
    destinationId: null,
    ...overrides,
  };
}

function cameraAt(elevationDeg: number, azimuthDeg = 30): Camera {
  return { azimuthDeg, elevationDeg, pxPerUnit: K };
}

const MIXED = sceneOf(
  [
    mark("a-high", local(10, 5, 20)),
    mark("b-low", local(-10, 5, -20)),
    mark("c-high", local(-30, -5, 3)),
    mark("d-low", local(30, 10, -1)),
    mark("e-plane", local(0, 30, 0)),
  ],
  {
    spheres: [
      { radius: 50, role: "range", label: "RANGE 50 ly SET" },
      { radius: 80, role: "data_edge", label: "QUERY EDGE 80 ly" },
    ],
    selectedId: "c-high",
  },
);

type Section = "below" | "plane" | "above" | "annotation";

/** The mark an op belongs to: a symbol's own, or the mark whose stalk a line is. */
function markIdOf(op: DrawOp): string | null {
  if (op.kind === "symbol") {
    return op.id;
  }
  return op.kind === "line" ? op.markId : null;
}

function sectionOf(op: DrawOp, scene: SpatialScene): Section {
  const id = markIdOf(op);
  if (id !== null) {
    const found = scene.points.find((point) => point.id === id);
    if (found === undefined) {
      throw new Error(`no mark ${id}`);
    }
    return dot(found.position, FRAME.north) >= 0 ? "above" : "below";
  }
  return op.kind === "line" || op.kind === "polyline" ? "plane" : "annotation";
}

function sections(scene: SpatialScene, camera: Camera): Section[] {
  const order: Section[] = [];
  for (const op of buildDrawList(scene, camera, VIEWPORT).ops) {
    const section = sectionOf(op, scene);
    if (order.at(-1) !== section) {
      order.push(section);
    }
  }
  return order;
}

function symbols(ops: ReadonlyArray<DrawOp>): Array<Extract<DrawOp, { kind: "symbol" }>> {
  return ops.filter((op): op is Extract<DrawOp, { kind: "symbol" }> => op.kind === "symbol");
}

describe("buildDrawList order", () => {
  it("draws below, plane, above, then annotations with the camera above the plane", () => {
    expect(sections(MIXED, cameraAt(30))).toEqual(["below", "plane", "above", "annotation"]);
  });

  it("swaps the halves with the camera below the plane", () => {
    expect(sections(MIXED, cameraAt(-30))).toEqual(["above", "plane", "below", "annotation"]);
  });

  it("keeps the above-the-plane order with the camera in the plane", () => {
    expect(sections(MIXED, cameraAt(0))).toEqual(["below", "plane", "above", "annotation"]);
  });

  it("draws each half far to near", () => {
    const random = seededRandom(12);
    const points = Array.from({ length: 200 }, (_, index) =>
      mark(
        `m${String(index).padStart(3, "0")}`,
        local(between(random, -50, 50), between(random, -50, 50), between(random, -50, 50)),
      ),
    );
    const scene = sceneOf(points);
    const { ops, anchors } = buildDrawList(scene, cameraAt(30), VIEWPORT);
    const depthOf = new Map(anchors.map((anchor) => [anchor.id, anchor.depth]));

    for (const half of ["below", "above"] as const) {
      const depths = symbols(ops)
        .filter((op) => sectionOf(op, scene) === half)
        .map((op) => depthOf.get(op.id) ?? Number.NaN);
      expect(depths.length).toBeGreaterThan(0);
      for (let i = 1; i < depths.length; i += 1) {
        expect(depths[i]).toBeLessThanOrEqual(depths[i - 1] ?? Number.NaN);
      }
    }
  });

  it("puts each stalk immediately before its own symbol", () => {
    const { ops } = buildDrawList(MIXED, cameraAt(30), VIEWPORT);

    const pairs = ops.flatMap((op, index) => {
      const previous = ops[index - 1];
      return op.kind === "symbol" ? [[op.id, previous?.kind, previous && markIdOf(previous)]] : [];
    });

    expect(pairs).toHaveLength(MIXED.points.length);
    expect(pairs).toEqual(pairs.map(([id]) => [id, "line", id]));
  });

  it("breaks depth ties by ID, drawing the lower ID last", () => {
    // Seen from the top, depth is height alone, so marks at one height tie wherever they are.
    const scene = sceneOf([
      mark("mike", local(0, -7, 2)),
      mark("alpha", local(-5, 3, 2)),
      mark("zulu", local(5, 5, 2)),
    ]);

    const ids = symbols(buildDrawList(scene, cameraAt(90, 0), VIEWPORT).ops).map((op) => op.id);

    expect(ids).toEqual(["zulu", "mike", "alpha"]);
  });

  it("draws on top the one of two coincident marks that a click picks", () => {
    const scene = sceneOf([
      mark("b-small", local(3, 4, 5), { sizeClass: 0 }),
      mark("a-large", local(3, 4, 5), { sizeClass: 4 }),
    ]);
    const { ops, anchors } = buildDrawList(scene, cameraAt(30), VIEWPORT);
    const [only] = anchors;

    const picked = pick(anchors, { xPx: only?.xPx ?? 0, yPx: only?.yPx ?? 0 }, 16);

    expect(symbols(ops).at(-1)?.id).toBe(picked);
  });
});

describe("buildDrawList marks", () => {
  it("fills a symbol above the plane and leaves one below it open, with the same outline", () => {
    const ops = symbols(buildDrawList(MIXED, cameraAt(30), VIEWPORT).ops);
    const byId = new Map(ops.map((op) => [op.id, op]));

    expect(byId.get("a-high")?.fill).toBe("text");
    expect(byId.get("e-plane")?.fill).toBe("text");
    expect(byId.get("b-low")?.fill).toBeNull();
    expect(byId.get("b-low")?.shape).toBe(byId.get("a-high")?.shape);
  });

  it("colours available marks and their stalks in accent and others in text", () => {
    const scene = sceneOf([
      mark("near", local(1, 1, 1), { status: "available" }),
      mark("far", local(1, 1, -1), { status: "plain" }),
    ]);
    const { ops } = buildDrawList(scene, cameraAt(30), VIEWPORT);

    const colours = ops.flatMap((op) =>
      op.kind === "symbol" || (op.kind === "line" && op.markId !== null)
        ? [`${op.kind} ${markIdOf(op) ?? ""} ${op.stroke} ${op.kind === "line" ? op.widthPx : ""}`]
        : [],
    );

    expect(colours.toSorted()).toEqual([
      "line far text 1",
      "line near accent 1",
      "symbol far text ",
      "symbol near accent ",
    ]);
  });

  it("drops each stalk to the mark's foot on the plane", () => {
    const scene = sceneOf([mark("high", local(0, 0, 10))]);
    const { ops } = buildDrawList(
      scene,
      { azimuthDeg: 0, elevationDeg: 0, pxPerUnit: K },
      VIEWPORT,
    );
    const stalk = ops.find((op) => op.kind === "line" && op.markId === "high");

    expect(stalk).toMatchObject({
      from: { xPx: CENTRE.xPx, yPx: CENTRE.yPx - 10 * K },
      to: { xPx: CENTRE.xPx, yPx: CENTRE.yPx },
    });
  });

  it("sizes a symbol by its class and the root font size alone, whatever the angle or depth", () => {
    const sizes = new Set<number>();
    for (const elevationDeg of [-90, -30, 0, 45, 90]) {
      for (const azimuthDeg of [0, 100, 250]) {
        const scene = sceneOf([mark("near", local(-40, 0, 30)), mark("far", local(40, 0, -30))]);
        for (const op of symbols(
          buildDrawList(scene, cameraAt(elevationDeg, azimuthDeg), VIEWPORT).ops,
        )) {
          sizes.add(op.radiusPx);
        }
      }
    }

    expect([...sizes]).toEqual([(SIZE_CLASS_REM[2] * 16) / 2 - SYMBOL_STROKE_PX / 2]);
  });

  it("carries no opacity or depth shading on any op", () => {
    const { ops } = buildDrawList(MIXED, cameraAt(30), VIEWPORT);
    const allowed = new Set([
      "kind",
      "from",
      "to",
      "points",
      "centre",
      "radiusPx",
      "halfSizePx",
      "segments",
      "stroke",
      "fill",
      "widthPx",
      "markId",
      "id",
      "shape",
    ]);

    for (const op of ops) {
      for (const key of Object.keys(op)) {
        expect(allowed).toContain(key);
      }
    }
  });

  it.each<[SizeClass, number]>([
    [0, 4],
    [4, 8],
  ])("anchors a class-%i mark with its full radius of %f px", (sizeClass, radiusPx) => {
    const scene = sceneOf([mark("only", local(1, 2, 3), { sizeClass })]);

    expect(buildDrawList(scene, cameraAt(30), VIEWPORT).anchors).toEqual([
      expect.objectContaining({ id: "only", radiusPx }),
    ]);
  });

  it("builds 4,000 marks in one call", () => {
    const random = seededRandom(4_000);
    const classes: ReadonlyArray<SizeClass> = [0, 1, 2, 3, 4];
    const points = Array.from({ length: 4_000 }, (_, index) =>
      mark(
        `s${index}`,
        local(between(random, -50, 50), between(random, -50, 50), between(random, -50, 50)),
        { sizeClass: classes[index % classes.length] ?? 0 },
      ),
    );

    const list = buildDrawList(sceneOf(points), cameraAt(30), VIEWPORT);

    expect(list.anchors).toHaveLength(4_000);
    expect(symbols(list.ops)).toHaveLength(4_000);
  });
});

describe("buildDrawList annotations", () => {
  function circles(
    scene: SpatialScene,
    camera: Camera,
  ): Array<Extract<DrawOp, { kind: "circle" }>> {
    return buildDrawList(scene, camera, VIEWPORT).ops.filter(
      (op): op is Extract<DrawOp, { kind: "circle" }> => op.kind === "circle",
    );
  }

  it.each([
    [0, 90],
    [30, 30],
    [200, -60],
  ])("draws the range sphere as a circle of k·R about the centre at %f°, %f°", (az, el) => {
    const found = circles(MIXED, cameraAt(el, az));

    expect(found).toEqual([
      { kind: "circle", centre: CENTRE, radiusPx: 50 * K, stroke: "text", widthPx: 1.5 },
      { kind: "circle", centre: CENTRE, radiusPx: 80 * K, stroke: "text", widthPx: 1 },
    ]);
  });

  it("ticks the edge of the data every 10°, outwards", () => {
    const segments = buildDrawList(MIXED, cameraAt(30), VIEWPORT).ops.flatMap((op) =>
      op.kind === "ticks" ? op.segments : [],
    );
    const fromCentre = (point: { xPx: number; yPx: number }): number =>
      Math.hypot(point.xPx - CENTRE.xPx, point.yPx - CENTRE.yPx);

    expect(segments).toHaveLength(36);
    for (const segment of segments) {
      expect(fromCentre(segment.from)).toBeCloseTo(80 * K, 9);
      expect(fromCentre(segment.to)).toBeCloseTo(80 * K + 4, 9);
    }
  });

  it("draws one ticked circle and both labels when the radii are equal", () => {
    const spheres: SphereMark[] = [
      { radius: 50, role: "range", label: "RANGE 50 ly SET" },
      { radius: 50, role: "data_edge", label: "QUERY EDGE 50 ly" },
    ];
    const list = buildDrawList(sceneOf([], { spheres }), cameraAt(30), VIEWPORT);

    expect(list.ops.filter((op) => op.kind === "circle")).toEqual([
      expect.objectContaining({ radiusPx: 50 * K, widthPx: 1.5 }),
    ]);
    expect(list.ops.filter((op) => op.kind === "ticks")).toHaveLength(1);
    expect(list.curveLabels.map((label) => [label.text, label.stack])).toEqual([
      ["RANGE 50 ly SET", 0],
      ["QUERY EDGE 50 ly", 1],
    ]);
  });

  it("puts sphere labels at the top of their circle and ring labels at the ring's coreward point", () => {
    const scene = sceneOf([], {
      spheres: [{ radius: 30, role: "range", label: "RANGE 30 ly SET" }],
      plane: { spacing: 20, extent: 50, rings: [{ radius: 30, label: "PLANE 30 ly" }] },
    });

    const labels = buildDrawList(
      scene,
      { azimuthDeg: 0, elevationDeg: 90, pxPerUnit: K },
      VIEWPORT,
    ).curveLabels;

    expect(labels).toEqual([
      expect.objectContaining({
        text: "RANGE 30 ly SET",
        placement: "circle-top",
        xPx: 400,
        yPx: 150,
      }),
      expect.objectContaining({ text: "PLANE 30 ly", placement: "ring" }),
    ]);
    const ring = labels[1];
    expect(ring?.xPx).toBeCloseTo(400, 9);
    expect(ring?.yPx).toBeCloseTo(150, 9);
  });

  it("gives no label to an unlabelled grid ring", () => {
    expect(
      buildDrawList(MIXED, cameraAt(30), VIEWPORT).curveLabels.map((label) => label.text),
    ).toEqual(["RANGE 50 ly SET", "QUERY EDGE 80 ly"]);
  });

  it("draws the selection reticle in accent, half a rem larger than the symbol", () => {
    const reticles = buildDrawList(MIXED, cameraAt(30), VIEWPORT).ops.filter(
      (op) => op.kind === "reticle",
    );

    expect(reticles).toEqual([
      expect.objectContaining({ id: "c-high", stroke: "accent", halfSizePx: 6 + 4 }),
    ]);
  });

  it("draws the destination in target, outside the selection when a mark is both", () => {
    const scene = { ...MIXED, destinationId: "c-high" };

    const reticles = buildDrawList(scene, cameraAt(30), VIEWPORT).ops.filter(
      (op) => op.kind === "reticle",
    );

    expect(reticles).toEqual([
      expect.objectContaining({ stroke: "accent", halfSizePx: 10 }),
      expect.objectContaining({ stroke: "target", halfSizePx: 14 }),
    ]);
  });

  it("draws the reticles after the spheres, last of all", () => {
    const { ops } = buildDrawList(MIXED, cameraAt(30), VIEWPORT);

    expect(ops.at(-1)?.kind).toBe("reticle");
  });

  it("draws the grid and rings in the line token", () => {
    const { ops } = buildDrawList(MIXED, cameraAt(30), VIEWPORT);
    const plane = ops.filter((op) => sectionOf(op, MIXED) === "plane");

    expect(plane.filter((op) => op.kind === "polyline")).toHaveLength(2);
    expect(plane.filter((op) => op.kind === "line")).toHaveLength(10);
    for (const op of plane) {
      expect(op.kind === "line" || op.kind === "polyline" ? op.stroke : null).toBe("line");
    }
  });
});
