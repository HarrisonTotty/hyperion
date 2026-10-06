import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { add, cross, norm, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { centresJustBeyond, limbPastRightEdge } from "../../test/beyondView";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { angularDiameterPx } from "../wireframe/bodies";
import { OUTSIDE_VIEW_MARGIN_PX, sphereOutsideView, sphereScreenRect } from "../wireframe/submit";
import { type DiscRecord, rasteriseDisc } from "./discShading";
import {
  type LitRegime,
  type LitSphere,
  litRegimes,
  GAS_GIANT_FULL_PASS_BOUNDARY_M,
  POINT_BELOW_PX,
  promoteOverlapping,
  type ScreenCircle,
  sphereFootprint,
} from "./regime";

const RAD_PER_DEG = Math.PI / 180;

const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: 60 * RAD_PER_DEG };

const VIEWPORTS = {
  "720p": { widthPx: 1280, heightPx: 720 },
  "1080p": { widthPx: 1920, heightPx: 1080 },
  "4K": { widthPx: 3840, heightPx: 2160 },
} as const satisfies Record<string, Viewport>;

const HD: Viewport = VIEWPORTS["1080p"];

const BODY: BodyIdHex = "0200080020000000.0300";

/** Jupiter's equatorial radius, m (NASA Jupiter fact sheet). */
const JUPITER_RADIUS_M = 71_492e3;

/** The centre-pixel scale of R02's projection, px per rad. */
function pxPerRad(viewport: Viewport): number {
  return viewport.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
}

/** A body of `radiusM` placed so that it subtends `diameterPx` on `viewport`, straight ahead. */
function sphereOf(diameterPx: number, viewport: Viewport, radiusM = 1e6): LitSphere {
  const angle = diameterPx / pxPerRad(viewport);
  return { id: BODY, centreM: vec3(0, 0, -radiusM / Math.sin(angle / 2)), radiusM };
}

/** Orders regime entries by body identifier. */
function byId(a: readonly [string, LitRegime], b: readonly [string, LitRegime]): number {
  return a[0].localeCompare(b[0]);
}

function regimeOf(
  sphere: LitSphere,
  viewport: Viewport,
  previous?: LitRegime,
): LitRegime | undefined {
  const before = new Map<BodyIdHex, LitRegime>(previous === undefined ? [] : [[BODY, previous]]);
  return litRegimes([sphere], CAMERA, viewport, before).get(BODY);
}

describe("litRegimes", () => {
  it.each(Object.entries(VIEWPORTS))(
    "draws a body as a disc above 3 px and a point below at %s, settled either side",
    (_, viewport) => {
      expect(regimeOf(sphereOf(2.99, viewport), viewport, "disc")).toBe("disc");
      expect(regimeOf(sphereOf(2.69, viewport), viewport, "disc")).toBe("point");
      expect(regimeOf(sphereOf(3.01, viewport), viewport, "point")).toBe("point");
      expect(regimeOf(sphereOf(3.31, viewport), viewport, "point")).toBe("disc");
      expect(POINT_BELOW_PX).toBe(3);
    },
  );

  it("holds a body at the boundary in its regime, by the 10% hysteresis", () => {
    const viewport = HD;
    const atBoundary = sphereOf(3, viewport);
    expect(regimeOf(atBoundary, viewport, "point")).toBe("point");
    expect(regimeOf(atBoundary, viewport, "disc")).toBe("disc");
  });

  it("starts a new body as a point, so it must clear 3.3 px", () => {
    const viewport = HD;
    expect(regimeOf(sphereOf(3.2, viewport), viewport)).toBe("point");
  });

  it("draws a body seen from inside its sphere as a disc", () => {
    const around: LitSphere = { id: BODY, centreM: vec3(0, 0, -1e5), radiusM: 1e6 };
    expect(regimeOf(around, HD)).toBe("disc");
  });

  it("keeps a former mesh on the disc side of the hysteresis", () => {
    expect(regimeOf(sphereOf(2.9, HD), HD, "mesh")).toBe("disc");
    expect(regimeOf(sphereOf(2.6, HD), HD, "mesh")).toBe("point");
  });

  it("does not depend on the bodies' order", () => {
    const viewport = HD;
    const bodies: LitSphere[] = [2, 3.5, 10, 0.5].map((px, index) => {
      const { centreM, radiusM } = sphereOf(px, viewport);
      return { id: `0200080020000000.0${String(index + 1)}00`, centreM, radiusM };
    });
    const forward = litRegimes(bodies, CAMERA, viewport, new Map());
    const backward = litRegimes(bodies.toReversed(), CAMERA, viewport, new Map());
    expect([...backward].toSorted(byId)).toEqual([...forward].toSorted(byId));
  });
});

describe("the brainstorm's figures, re-derived", () => {
  // The brainstorm scales by width ÷ field, 1,833 px/rad at 1080p across 60°; R02's projection
  // uses the centre pixel's tan-based 1,663 px/rad, which brings each distance in by 9.3%.
  const viewport = HD;
  const linear = viewport.widthPx / CAMERA.fovXRad;

  it("keeps a Jupiter at least 3 px across to about 9 × 10¹⁰ m, 7.9 × 10¹⁰ m at the centre scale", () => {
    expect((2 * JUPITER_RADIUS_M * linear) / 3 / 1e10).toBeCloseTo(8.7, 1);
    const centre = (2 * JUPITER_RADIUS_M) / (3 / pxPerRad(viewport));
    expect(centre / 1e10).toBeCloseTo(7.9, 1);
    const diameterAt = (distance: number): number =>
      angularDiameterPx(vec3(0, 0, -distance), JUPITER_RADIUS_M, CAMERA, viewport);
    expect(diameterAt(centre * 0.999)).toBeGreaterThan(POINT_BELOW_PX);
    expect(diameterAt(centre * 1.001)).toBeLessThan(POINT_BELOW_PX);
  });

  it.each([
    // Scale heights, km: Jupiter 27, Saturn 59.5 (NASA planetary fact sheets).
    ["Jupiter", 27e3, 2.5e8, 2.245e8],
    ["Saturn", 59.5e3, 5.5e8, 4.947e8],
  ] as const)(
    "spans %s's ten scale heights over 2 px at both scales",
    (_, heightM, distanceM, centreDistanceM) => {
      expect(Math.abs((10 * heightM * linear) / 2 / distanceM - 1)).toBeLessThan(0.02);
      // R02's centre-pixel scale, the reference (the orchestrator's ruling, 2026-10-03).
      const atCentre = (10 * heightM * pxPerRad(viewport)) / 2;
      expect(Math.abs(atCentre / centreDistanceM - 1)).toBeLessThan(1e-3);
      expect(GAS_GIANT_FULL_PASS_BOUNDARY_M).toBeGreaterThan(distanceM);
      expect(GAS_GIANT_FULL_PASS_BOUNDARY_M).toBeGreaterThan(
        (10 * heightM * pxPerRad(viewport)) / 2,
      );
    },
  );
});

describe("promoteOverlapping", () => {
  const FIRST: BodyIdHex = "0200080020000000.0100";
  const SECOND: BodyIdHex = "0200080020000000.0200";
  const THIRD: BodyIdHex = "0200080020000000.0300";
  const footprints = new Map<BodyIdHex, ScreenCircle>([
    [FIRST, { xPx: 100, yPx: 100, radiusPx: 50 }],
    [SECOND, { xPx: 160, yPx: 100, radiusPx: 20 }],
    [THIRD, { xPx: 400, yPx: 400, radiusPx: 20 }],
  ]);

  it("promotes a disc overlapping a mesh body", () => {
    const regimes = new Map<BodyIdHex, LitRegime>([
      [FIRST, "mesh"],
      [SECOND, "disc"],
      [THIRD, "disc"],
    ]);
    const promoted = promoteOverlapping(regimes, footprints, []);
    expect([promoted.get(SECOND), promoted.get(THIRD)]).toEqual(["mesh", "disc"]);
  });

  it("promotes a disc overlapping other depth-writing geometry", () => {
    const regimes = new Map<BodyIdHex, LitRegime>([[THIRD, "disc"]]);
    const craft: ScreenCircle = { xPx: 410, yPx: 400, radiusPx: 5 };
    expect(promoteOverlapping(regimes, footprints, [craft]).get(THIRD)).toBe("mesh");
  });

  it("spreads through a chain of overlaps", () => {
    const chained = new Map(footprints);
    chained.set(THIRD, { xPx: 190, yPx: 100, radiusPx: 20 });
    const regimes = new Map<BodyIdHex, LitRegime>([
      [FIRST, "mesh"],
      [SECOND, "disc"],
      [THIRD, "disc"],
    ]);
    expect(promoteOverlapping(regimes, chained, []).get(THIRD)).toBe("mesh");
  });

  it("never promotes a point", () => {
    const regimes = new Map<BodyIdHex, LitRegime>([
      [FIRST, "mesh"],
      [SECOND, "point"],
    ]);
    expect(promoteOverlapping(regimes, footprints, []).get(SECOND)).toBe("point");
  });
});

describe("a sphere's footprint (T9)", () => {
  /** The projected points of a sphere's silhouette, the circle of tangency, from the camera. */
  function silhouette(centreM: Vec3, radiusM: number): Array<{ xPx: number; yPx: number }> {
    const d = Math.hypot(centreM.x, centreM.y, centreM.z);
    const axis = normalise(centreM);
    const u = normalise(cross(axis, vec3(0, 1, 0)));
    const v = cross(axis, u);
    const sinA = radiusM / d;
    const cosA = Math.sqrt(1 - sinA * sinA);
    const points: Array<{ xPx: number; yPx: number }> = [];
    for (let i = 0; i < 64; i += 1) {
      const t = (2 * Math.PI * i) / 64;
      const point = add(
        scale(axis, d * cosA * cosA),
        add(scale(u, radiusM * cosA * Math.cos(t)), scale(v, radiusM * cosA * Math.sin(t))),
      );
      const p = project(point, CAMERA, HD);
      points.push({ xPx: p.xPx, yPx: p.yPx });
    }
    return points;
  }

  it.each([
    { name: "at the view's centre", centre: vec3(0, 0, -2e7) },
    { name: "in a corner", centre: vec3(0.9e7, 0.44e7, -2e7) },
  ])("holds the sphere's silhouette $name", ({ centre }) => {
    // A sphere 5° across, wholly on the view, its centre 24° and 12° off the axis.
    const footprint = sphereFootprint(centre, 1e6, CAMERA, HD);
    const outside = silhouette(centre, 1e6).filter(
      (p) =>
        footprint === null ||
        Math.hypot(p.xPx - footprint.xPx, p.yPx - footprint.yPx) > footprint.radiusPx,
    );
    expect(outside).toEqual([]);
  });

  it("holds the part on the view of a sphere the view's edge cuts", () => {
    // 31° off the axis and 15° in radius: the view's edge, 30° off it, cuts it.
    const centre = vec3(1.2e7, 0, -2e7);
    const footprint = sphereFootprint(centre, 6.371e6, CAMERA, HD);
    const onView = silhouette(centre, 6.371e6).filter((p) => p.xPx >= 0 && p.xPx <= HD.widthPx);
    expect(onView.length).toBeGreaterThan(0);
    expect(
      onView.every(
        (p) =>
          footprint !== null &&
          Math.hypot(p.xPx - footprint.xPx, p.yPx - footprint.yPx) <= footprint.radiusPx,
      ),
    ).toBe(true);
  });

  it("is null for a sphere behind the camera", () => {
    expect(sphereFootprint(vec3(0, 0, 2e7), 6.371e6, CAMERA, HD)).toBeNull();
  });
});

describe("a sphere wholly off the view (R07.T19's per-draw cost)", () => {
  // Small, so that the disc's twin tests every pixel quickly; the margin is in pixels. The tall
  // view's height spans 144° at a 120° width.
  const SMALL: Viewport = { widthPx: 72, heightPx: 40 };
  const TALL: Viewport = { widthPx: 40, heightPx: 72 };
  const RADIUS_M = 6.371e6;

  /** A body's disc record over the whole of `viewport`, for the twin's own per-pixel test. */
  function wholeViewRecord(
    centreM: Vec3,
    pole: Vec3,
    flattening: number,
    viewport: Viewport = SMALL,
  ): DiscRecord {
    const distanceM = norm(centreM);
    return {
      body: BODY,
      rect: { leftPx: 0, topPx: 0, rightPx: viewport.widthPx, bottomPx: viewport.heightPx },
      direction: scale(centreM, 1 / distanceM),
      radiusOverDistance: RADIUS_M / distanceM,
      pole,
      polarOverEquatorial: 1 - flattening,
      interiorSamples: 1,
      limbSamples: 1,
      surface: { kind: "uniform", law: PROVISIONAL_PHOTOMETRY.law },
      tableRows: [0],
      exposureOverPi: 1,
      lights: [],
      occluders: [],
      secondaries: [],
    };
  }

  /** Centres just beyond each side plane widened by the margin, at `distanceM` along the plane. */
  function justBeyond(camera: ProjectionCamera, viewport: Viewport, distanceM: number): Vec3[] {
    return centresJustBeyond(
      camera.fovXRad,
      viewport,
      OUTSIDE_VIEW_MARGIN_PX,
      distanceM,
      RADIUS_M * (1 + 1e-6),
    );
  }

  const FIGURES = [
    { figure: "a sphere", pole: vec3(0, 1, 0), flattening: 0 },
    { figure: "Saturn's f = 0.098", pole: vec3(1, 0, 0), flattening: 0.098 },
    { figure: "the record's cap f = 0.2, its pole oblique", pole: vec3(1, 1, 1), flattening: 0.2 },
  ];
  const cases = [SMALL, TALL].flatMap((viewport) =>
    [10, 60, 120].flatMap((fovDeg) =>
      [3, 300].flatMap((distanceRadii) =>
        FIGURES.map(({ figure, pole, flattening }) => ({
          view: `${viewport.widthPx} × ${viewport.heightPx}`,
          viewport,
          fovDeg,
          distanceRadii,
          figure,
          pole: normalise(pole),
          flattening,
        })),
      ),
    ),
  );

  it.each(cases)(
    "leaves no pixel the disc would draw: $figure at $distanceRadii radii, $fovDeg° across $view",
    ({ viewport, fovDeg, distanceRadii, pole, flattening }) => {
      const camera = { orientation: IDENTITY_QUATERNION, fovXRad: fovDeg * RAD_PER_DEG };
      const centres = justBeyond(camera, viewport, distanceRadii * RADIUS_M);
      expect(centres.every((c) => sphereOutsideView(c, RADIUS_M, camera, viewport))).toBe(true);
      const drawn = centres.flatMap((c) =>
        rasteriseDisc(wholeViewRecord(c, pole, flattening, viewport), camera, viewport),
      );
      expect(drawn).toEqual([]);
    },
  );

  it("keeps a sphere the view's edge cuts", () => {
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: 60 * RAD_PER_DEG };
    const tanHalf = Math.tan(camera.fovXRad / 2);
    const onEdge = scale(normalise(vec3(tanHalf, 0, -1)), 30 * RADIUS_M);
    expect(sphereOutsideView(onEdge, RADIUS_M, camera, SMALL)).toBe(false);
    expect(rasteriseDisc(wholeViewRecord(onEdge, vec3(0, 1, 0), 0), camera, SMALL)).not.toEqual([]);
  });

  it("keeps a sphere whose limb is within the margin", () => {
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: 60 * RAD_PER_DEG };
    // Its limb a pixel past the edge: off the view, but within the margin.
    const pastEdge = limbPastRightEdge(camera.fovXRad, SMALL, 30 * RADIUS_M, RADIUS_M, 1);
    expect(sphereOutsideView(pastEdge, RADIUS_M, camera, SMALL)).toBe(false);
  });

  it("gives no footprint beside the view, level with a corner, across the camera's plane or behind it (T19.e)", () => {
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: 60 * RAD_PER_DEG };
    const centres = justBeyond(camera, HD, 30 * RADIUS_M);
    expect(centres.map((c) => sphereFootprint(c, RADIUS_M, camera, HD))).toEqual(
      centres.map(() => null),
    );
  });

  it("keeps the footprint of a sphere the edge cuts (T19.e)", () => {
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: 60 * RAD_PER_DEG };
    const onEdge = scale(normalise(vec3(Math.tan(camera.fovXRad / 2), 0, -1)), 30 * RADIUS_M);
    expect(sphereFootprint(onEdge, RADIUS_M, camera, HD)).not.toBeNull();
  });

  it("keeps the footprint of a sphere whose limb is a pixel past the edge (T19.e)", () => {
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: 60 * RAD_PER_DEG };
    const pastEdge = limbPastRightEdge(camera.fovXRad, HD, 30 * RADIUS_M, RADIUS_M, 1);
    expect(sphereFootprint(pastEdge, RADIUS_M, camera, HD)).not.toBeNull();
  });

  it("holds behind the camera and across its plane, where the rectangle is the whole view", () => {
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: 60 * RAD_PER_DEG };
    for (const centre of [vec3(2e8, 0, 0), vec3(0, 0, 2e8)]) {
      expect(sphereScreenRect(centre, RADIUS_M, camera, HD)).toEqual({
        leftPx: 0,
        topPx: 0,
        rightPx: HD.widthPx,
        bottomPx: HD.heightPx,
      });
      expect(sphereOutsideView(centre, RADIUS_M, camera, HD)).toBe(true);
    }
  });
});
