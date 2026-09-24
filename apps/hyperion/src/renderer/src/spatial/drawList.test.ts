import { describe, expect, it } from "vitest";

import { between, seededRandom } from "../test/seededRandom";
import { type Camera, project, viewBasis, type Viewport } from "./camera";
import { buildDrawList, type DrawOp, type PolylineOp, type TicksOp } from "./drawList";
import { fromLocal, localFrameAt, planeFrame, toLocal } from "./frame";
import type {
  AnnulusMark,
  PathMark,
  PointMark,
  SizeClass,
  SpatialScene,
  SphereMark,
} from "./marks";
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

/** A path mark, a reference line unless overridden, unlabelled. */
function path(
  id: string,
  points: ReadonlyArray<Vec3>,
  overrides: Partial<PathMark> = {},
): PathMark {
  return {
    id,
    points,
    role: "reference",
    label: "",
    labelAt: points[0] ?? vec3(0, 0, 0),
    ...overrides,
  };
}

/** An annulus about the view centre, unticked and unlabelled unless overridden. */
function annulus(
  id: string,
  innerRadius: number,
  outerRadius: number,
  overrides: Partial<AnnulusMark> = {},
): AnnulusMark {
  return { id, innerRadius, outerRadius, ticks: false, label: "", ...overrides };
}

/** A circle of `radius` about the centre in a plane turned `tiltDeg` about coreward. */
function tiltedCircle(radius: number, tiltDeg: number, segments = 96): Vec3[] {
  const tilt = (tiltDeg * Math.PI) / 180;
  return Array.from({ length: segments + 1 }, (_, i) => {
    const angle = i === segments ? 0 : (2 * Math.PI * i) / segments;
    return local(
      radius * Math.cos(angle),
      radius * Math.sin(angle) * Math.cos(tilt),
      radius * Math.sin(angle) * Math.sin(tilt),
    );
  });
}

/** A scene with no grid or rings, so that every polyline in its draw list is one of its own. */
function bareScene(overrides: Partial<SpatialScene> = {}): SpatialScene {
  return sceneOf([], { plane: { spacing: 1, extent: 0, rings: [] }, ...overrides });
}

function polylines(ops: ReadonlyArray<DrawOp>): PolylineOp[] {
  return ops.filter((op): op is PolylineOp => op.kind === "polyline");
}

const TOP: Camera = { azimuthDeg: 0, elevationDeg: 90, pxPerUnit: K };

describe("buildDrawList paths", () => {
  it("projects a circular path tilted 60° as an ellipse with axes 1 and 0.5", () => {
    const scene = bareScene({ paths: [path("orbit", tiltedCircle(1, 60))] });

    const points = polylines(buildDrawList(scene, TOP, VIEWPORT).ops).flatMap((op) => op.points);

    // From the top coreward is up and spinward right: the unit circle keeps its extent along
    // coreward and is foreshortened by cos 60° along spinward.
    expect(points.length).toBeGreaterThan(96);
    for (const point of points) {
      const across = (point.xPx - CENTRE.xPx) / (0.5 * K);
      const up = (CENTRE.yPx - point.yPx) / K;
      expect(across * across + up * up).toBeCloseTo(1, 9);
    }
    const acrossPx = points.map((point) => Math.abs(point.xPx - CENTRE.xPx));
    const upPx = points.map((point) => Math.abs(point.yPx - CENTRE.yPx));
    expect(Math.max(...acrossPx)).toBeCloseTo(0.5 * K, 9);
    expect(Math.max(...upPx)).toBeCloseTo(K, 9);
  });

  it("cuts a path where it crosses the plane, at the crossing", () => {
    const scene = bareScene({ paths: [path("cross", [local(-10, 0, -5), local(10, 0, 5)])] });

    const pieces = polylines(buildDrawList(scene, TOP, VIEWPORT).ops);

    // Seen from the top the crossing, on the plane at the centre, is the centre of the screen.
    expect(pieces).toHaveLength(2);
    expect(pieces[0]?.points.at(-1)?.xPx).toBeCloseTo(CENTRE.xPx, 9);
    expect(pieces[0]?.points.at(-1)?.yPx).toBeCloseTo(CENTRE.yPx, 9);
    expect(pieces[1]?.points[0]).toEqual(pieces[0]?.points.at(-1));
  });

  it.each([
    ["above", 30, ["below", "grid", "above"]],
    ["in", 0, ["below", "grid", "above"]],
    ["below", -30, ["above", "grid", "below"]],
  ] as const)(
    "draws each piece in its own half of the order with the camera %s the plane",
    (_, elevationDeg, order) => {
      const scene = sceneOf([], {
        paths: [path("cross", [local(-10, 0, -5), local(10, 0, 5)], { role: "selected" })],
      });

      const camera = cameraAt(elevationDeg);
      const { ops } = buildDrawList(scene, camera, VIEWPORT);

      // The path is the only thing drawn in --text, and its piece below the plane starts where it
      // does; the grid and rings are --line.
      const start = project(local(-10, 0, -5), viewBasis(FRAME, camera), camera, VIEWPORT);
      const sequence: string[] = [];
      for (const op of ops) {
        let section = "grid";
        if (op.kind === "polyline" && op.stroke === "text") {
          const first = op.points[0];
          section = first?.xPx === start.xPx && first.yPx === start.yPx ? "below" : "above";
        }
        if (sequence.at(-1) !== section) {
          sequence.push(section);
        }
      }
      expect(sequence).toEqual(order);
    },
  );

  it("draws a closed path whose ends lie on one side of the plane as one piece there", () => {
    const scene = bareScene({ paths: [path("orbit", tiltedCircle(10, 30))] });

    // The circle starts on the plane at its coreward point, rises, and comes back from below.
    expect(polylines(buildDrawList(scene, cameraAt(30), VIEWPORT).ops)).toHaveLength(2);
  });

  it("draws a reference path in --text-muted and the selected one in --text, after it in its half", () => {
    const scene = bareScene({
      paths: [
        path("selected", tiltedCircle(20, 0), { role: "selected" }),
        path("reference", tiltedCircle(10, 0)),
      ],
    });

    const strokes = polylines(buildDrawList(scene, cameraAt(30), VIEWPORT).ops).map(
      (op) => op.stroke,
    );

    // The orchestrator's ruling 35: an orbit says where something lies, so it takes 6:1, which
    // --text-muted reaches and --line does not.
    expect(strokes).toEqual(["textMuted", "text"]);
  });

  it("draws a half's path pieces before its marks, so that no line crosses a symbol", () => {
    const scene = bareScene({
      points: [mark("planet", local(10, 0, 0))],
      paths: [path("orbit", tiltedCircle(10, 0))],
    });

    const kinds = buildDrawList(scene, cameraAt(30), VIEWPORT).ops.map((op) => op.kind);

    expect(kinds).toEqual(["polyline", "line", "symbol"]);
  });

  it("draws a reference path 1 px wide", () => {
    const scene = bareScene({ paths: [path("orbit", tiltedCircle(10, 0))] });

    expect(polylines(buildDrawList(scene, TOP, VIEWPORT).ops).map((op) => op.widthPx)).toEqual([1]);
  });

  it("draws the selected path 2 px wide, so that width and not colour alone carries it", () => {
    const scene = bareScene({
      paths: [
        path("selected", tiltedCircle(20, 0), { role: "selected" }),
        path("reference", tiltedCircle(10, 0)),
      ],
    });

    // The orchestrator's ruling 44.2: --text against --text-muted is only 1.88:1, and a stale view
    // mutes both.
    expect(polylines(buildDrawList(scene, TOP, VIEWPORT).ops).map((op) => op.widthPx)).toEqual([
      1, 2,
    ]);
  });

  it("gives a path no anchor, so that a pick on it finds nothing", () => {
    const scene = bareScene({ paths: [path("orbit", tiltedCircle(10, 0))] });

    const { ops, anchors } = buildDrawList(scene, TOP, VIEWPORT);

    const onTheOrbit = polylines(ops)[0]?.points[5] ?? CENTRE;
    expect(anchors).toEqual([]);
    expect(pick(anchors, onTheOrbit, 16)).toBeNull();
  });

  it("labels a path at its anchor, after the rings' labels", () => {
    const scene = sceneOf([], {
      plane: { spacing: 20, extent: 50, rings: [{ radius: 20, label: "20 AU" }] },
      paths: [
        path("b", tiltedCircle(10, 0), { label: "b", labelAt: local(0, 10, 0) }),
        path("c", tiltedCircle(15, 0)),
      ],
    });

    const { curveLabels } = buildDrawList(scene, TOP, VIEWPORT);

    // Spinward is to the right from the top; the unlabelled path has no label.
    expect(curveLabels.map((label) => label.key)).toEqual(["ring:0", "path:b"]);
    expect(curveLabels[1]).toEqual({
      key: "path:b",
      text: "b",
      placement: "ring",
      xPx: CENTRE.xPx + 10 * K,
      yPx: CENTRE.yPx,
      stack: 0,
    });
  });
});

/** The ops drawn in `--line`. */
function inLine(ops: ReadonlyArray<DrawOp>): DrawOp[] {
  return ops.filter((op) => op.stroke === "line");
}

describe("buildDrawList annuli", () => {
  it("draws an annulus as its two edges, at their radii on the plane", () => {
    const edges = polylines(
      buildDrawList(bareScene({ annuli: [annulus("hz", 10, 20)] }), TOP, VIEWPORT).ops,
    );

    expect(edges).toHaveLength(2);
    edges.forEach((edge, index) => {
      for (const point of edge.points) {
        expect(Math.hypot(point.xPx - CENTRE.xPx, point.yPx - CENTRE.yPx)).toBeCloseTo(
          (index === 0 ? 10 : 20) * K,
          9,
        );
      }
    });
  });

  it("draws an annulus's edges as 1 px --text-muted hairlines", () => {
    const edges = polylines(
      buildDrawList(bareScene({ annuli: [annulus("hz", 10, 20)] }), TOP, VIEWPORT).ops,
    );

    expect(edges.map((edge) => [edge.stroke, edge.widthPx])).toEqual([
      ["textMuted", 1],
      ["textMuted", 1],
    ]);
  });

  it("leaves --line to the plane's grid and rings when paths and annuli are drawn", () => {
    const withRing = { spacing: 20, extent: 50, rings: [{ radius: 20, label: "20 AU" }] };
    const furniture = buildDrawList(bareScene({ plane: withRing }), cameraAt(30), VIEWPORT).ops;
    const scene = bareScene({
      plane: withRing,
      paths: [path("orbit", tiltedCircle(10, 30))],
      annuli: [annulus("hz", 10, 20), annulus("belt", 30, 32, { ticks: true })],
    });

    const { ops } = buildDrawList(scene, cameraAt(30), VIEWPORT);

    // The orchestrator's ruling 35: what is drawn in --line is the plane's own furniture, the same
    // with the paths and annuli as without them.
    expect(inLine(ops)).toEqual(inLine(furniture));
    expect(inLine(furniture)).toHaveLength(furniture.length);
  });

  it("draws an annulus with the plane, after its grid and rings and before the marks above it", () => {
    const scene = sceneOf([mark("above", local(0, 0, 5))], { annuli: [annulus("hz", 10, 20)] });

    const { ops } = buildDrawList(scene, TOP, VIEWPORT);

    // The grid's two rings come first among the polylines, then the annulus's two edges.
    const polylineAt = ops.flatMap((op, index) => (op.kind === "polyline" ? [index] : []));
    const [innerAt = -1, outerAt = -1] = polylineAt.slice(-2);
    const lastGrid = ops.findLastIndex((op) => op.kind === "line" && op.markId === null);
    expect(polylineAt).toHaveLength(4);
    expect(innerAt).toBeGreaterThan(lastGrid);
    expect(ops.findIndex((op) => op.kind === "symbol")).toBeGreaterThan(outerAt);
  });

  it("draws no ticks for an annulus that is not a belt", () => {
    const { ops } = buildDrawList(bareScene({ annuli: [annulus("hz", 10, 20)] }), TOP, VIEWPORT);

    expect(ops.some((op) => op.kind === "ticks")).toBe(false);
  });

  it("joins a belt's edges with radial ticks every 10° from coreward", () => {
    const scene = bareScene({ annuli: [annulus("belt", 10, 12, { ticks: true })] });

    const ticks = buildDrawList(scene, TOP, VIEWPORT).ops.filter(
      (op): op is TicksOp => op.kind === "ticks",
    );

    expect(ticks).toHaveLength(1);
    const segments = ticks[0]?.segments ?? [];
    expect(segments).toHaveLength(36);
    expect(ticks[0]?.stroke).toBe("textMuted");
    // The first joins the edges at their coreward points, straight up the screen from the top.
    expect(segments[0]?.from.xPx).toBeCloseTo(CENTRE.xPx, 9);
    expect(segments[0]?.from.yPx).toBeCloseTo(CENTRE.yPx - 10 * K, 9);
    expect(segments[0]?.to.yPx).toBeCloseTo(CENTRE.yPx - 12 * K, 9);
    segments.forEach((segment, index) => {
      const angle = Math.atan2(segment.to.xPx - CENTRE.xPx, CENTRE.yPx - segment.to.yPx);
      const expected = (index * 10 * Math.PI) / 180;
      expect(Math.cos(angle - expected)).toBeCloseTo(1, 9);
      expect(Math.hypot(segment.from.xPx - CENTRE.xPx, segment.from.yPx - CENTRE.yPx)).toBeCloseTo(
        10 * K,
        9,
      );
      expect(Math.hypot(segment.to.xPx - CENTRE.xPx, segment.to.yPx - CENTRE.yPx)).toBeCloseTo(
        12 * K,
        9,
      );
    });
  });

  it("draws one edge for equal radii and none of radius 0", () => {
    const scene = bareScene({
      annuli: [annulus("snow", 15, 15, { ticks: true }), annulus("inside", 0, 5)],
    });

    const { ops } = buildDrawList(scene, TOP, VIEWPORT);

    expect(polylines(ops)).toHaveLength(2);
    expect(ops.some((op) => op.kind === "ticks")).toBe(false);
  });

  it("draws an annulus about its own centre, on the plane beneath it", () => {
    const scene = bareScene({ annuli: [annulus("zone", 5, 5, { centre: local(10, 0, 7) })] });

    const [edge] = polylines(buildDrawList(scene, cameraAt(0, 90), VIEWPORT).ops);

    // Seen from the front, the plane is a line across the middle of the screen.
    for (const point of edge?.points ?? []) {
      expect(point.yPx).toBeCloseTo(CENTRE.yPx, 9);
    }
  });

  it("labels an annulus at its outer edge's rimward point", () => {
    const scene = bareScene({
      annuli: [annulus("hz", 10, 20, { label: "HABITABLE ZONE" }), annulus("unlabelled", 3, 4)],
    });

    const { curveLabels } = buildDrawList(scene, TOP, VIEWPORT);

    // Rimward is straight down the screen from the top.
    expect(curveLabels).toEqual([
      {
        key: "annulus:hz",
        text: "HABITABLE ZONE",
        placement: "ring",
        xPx: expect.closeTo(CENTRE.xPx, 9),
        yPx: expect.closeTo(CENTRE.yPx + 20 * K, 9),
        stack: 0,
      },
    ]);
  });

  it("gives an annulus no anchor, so that it is never picked", () => {
    const scene = bareScene({ annuli: [annulus("belt", 10, 20, { ticks: true })] });

    expect(buildDrawList(scene, TOP, VIEWPORT).anchors).toEqual([]);
  });
});

describe("buildDrawList without paths or annuli", () => {
  it("draws a scene that leaves them out exactly as one with none", () => {
    const camera = cameraAt(30);

    expect(buildDrawList({ ...MIXED, paths: [], annuli: [] }, camera, VIEWPORT)).toEqual(
      buildDrawList(MIXED, camera, VIEWPORT),
    );
  });
});

describe("buildDrawList with the outlines the later plans add", () => {
  it.each(["ringed-circle", "triangle-down", "pentagon", "hexagon"] as const)(
    "draws the %s filled above the plane and open below it, differing only by fill",
    (shape) => {
      // From the top the two marks, 5 units either side of the plane, fall on one point.
      const scene = sceneOf([
        mark("above", local(0, 0, 5), { shape }),
        mark("below", local(0, 0, -5), { shape }),
      ]);

      const { ops, anchors } = buildDrawList(scene, TOP, VIEWPORT);

      const [open, filled] = symbols(ops);
      expect(open?.fill).toBeNull();
      expect(filled?.fill).toBe("text");
      expect({ ...open, id: "", fill: null }).toEqual({ ...filled, id: "", fill: null });
      expect(anchors[0]?.radiusPx).toBe(anchors[1]?.radiusPx);
    },
  );
});

/**
 * Where `actual` departs from `expected`: a number more than `tolerance` from the one in the same
 * place, or any other difference of shape or value, each named by its path.
 */
function departures(
  actual: unknown,
  expected: unknown,
  tolerance: number,
  where = "list",
): string[] {
  if (typeof expected === "number") {
    return typeof actual === "number" && Math.abs(actual - expected) <= tolerance
      ? []
      : [`${where}: ${String(actual)} is not within ${tolerance} of ${expected}`];
  }
  if (Array.isArray(expected)) {
    if (!Array.isArray(actual) || actual.length !== expected.length) {
      return [`${where}: the lists differ in length`];
    }
    return expected.flatMap((item: unknown, index) =>
      departures(actual[index], item, tolerance, `${where}[${index}]`),
    );
  }
  if (typeof expected === "object" && expected !== null) {
    if (typeof actual !== "object" || actual === null) {
      return [`${where}: not an object`];
    }
    const keys = Object.keys(expected).toSorted();
    if (Object.keys(actual).toSorted().join() !== keys.join()) {
      return [`${where}: the keys differ`];
    }
    return keys.flatMap((key) =>
      departures(
        Reflect.get(actual, key),
        Reflect.get(expected, key),
        tolerance,
        `${where}.${key}`,
      ),
    );
  }
  return Object.is(actual, expected)
    ? []
    : [`${where}: ${String(actual)} is not ${String(expected)}`];
}

describe("buildDrawList on a tilted frame", () => {
  // A system's plane, 30° off the galactic one, with galactic coreward laid onto it (plan 14, D21).
  const TILTED = planeFrame(
    add(scale(FRAME.north, Math.cos(Math.PI / 6)), scale(FRAME.spinward, 0.5)),
    FRAME.coreward,
  );

  /** The same point in the tilted frame's own coordinates as `point` has in the galactic one. */
  function tilted(point: Vec3): Vec3 {
    return fromLocal(TILTED, toLocal(FRAME, point));
  }

  const GALACTIC_SCENE: SpatialScene = {
    ...MIXED,
    paths: [path("orbit", tiltedCircle(25, 12), { label: "b", labelAt: local(25, 0, 0) })],
    annuli: [annulus("belt", 30, 35, { ticks: true, label: "BELT", centre: local(2, 1, 0) })],
  };
  function tiltedMark(point: PointMark): PointMark {
    return { ...point, position: tilted(point.position) };
  }

  function tiltedPath(each: PathMark): PathMark {
    return { ...each, points: each.points.map(tilted), labelAt: tilted(each.labelAt) };
  }

  function tiltedAnnulus(each: AnnulusMark): AnnulusMark {
    return { ...each, centre: tilted(each.centre ?? vec3(0, 0, 0)) };
  }

  const TILTED_SCENE: SpatialScene = {
    ...GALACTIC_SCENE,
    frame: TILTED,
    points: GALACTIC_SCENE.points.map(tiltedMark),
    paths: (GALACTIC_SCENE.paths ?? []).map(tiltedPath),
    annuli: (GALACTIC_SCENE.annuli ?? []).map(tiltedAnnulus),
  };

  it.each([
    [30, 30],
    [0, 90],
    [90, 0],
    [200, -45],
  ])(
    "draws the grid, marks, paths and annuli at azimuth %s° and elevation %s° as the galactic scene",
    (azimuthDeg, elevationDeg) => {
      const camera: Camera = { azimuthDeg, elevationDeg, pxPerUnit: K };

      const drawn = buildDrawList(TILTED_SCENE, camera, VIEWPORT);

      // The camera's angles are the frame's own, so the picture is the same to rounding.
      expect(departures(drawn, buildDrawList(GALACTIC_SCENE, camera, VIEWPORT), 1e-9)).toEqual([]);
    },
  );

  it("draws an orbit lying in a tilted plane as one piece, not one cut at every rounding error", () => {
    // A plane tilted off every galactic axis, in which a point's height comes out at ±1e-15 or so.
    const skew = planeFrame(vec3(0.3, -0.2, 0.9), FRAME.coreward);
    const orbit = Array.from({ length: 97 }, (_, i) => {
      const angle = i === 96 ? 0 : (2 * Math.PI * i) / 96;
      return fromLocal(skew, {
        coreward: 30 * Math.cos(angle),
        spinward: 30 * Math.sin(angle),
        north: 0,
      });
    });
    const scene = bareScene({ frame: skew, paths: [path("in-plane", orbit)] });

    expect(polylines(buildDrawList(scene, cameraAt(30), VIEWPORT).ops)).toHaveLength(1);
  });
});
