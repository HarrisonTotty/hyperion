import { describe, expect, it } from "vitest";

import { add, dot, norm, normalise, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import {
  DEFAULT_FOV_DEG,
  type ProjectionCamera,
  project,
  type Viewport,
} from "../camera/projection";
import { IDENTITY_QUATERNION, lookAlong, rotationRows } from "../camera/quaternion";
import { rotateToBody, rotation3FromRows } from "../coords/rotation";
import { ATMOSPHERE_FIXTURES } from "../scenes/atmosphereScene";
import {
  angularDiameterPx,
  bodyRegime,
  DATUM_OVER_IMAGE_RADIUS_PER_REACH,
  type DatumInside,
  type GraticuleBody,
  graticule,
  graticuleStepDeg,
  isSymbolSized,
  perpendicularPair,
  ringEllipse,
  SYMBOL_BELOW_PX,
} from "./bodies";
import type { Polyline } from "./curve";
import { CASING_PX, STROKE_PX } from "./drawList";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const FOV_X_RAD = Math.PI / 3;
const EARTH_RADIUS_M = 6.371e6;

/** The line scale R02 built the wireframe at: a device px for each CSS px of a line. */
const AS_BUILT_SCALE = 1;

/** The distance from `(x, y)` to the screen segment from `a` to `b`, px. */
function toSegmentPx(
  x: number,
  y: number,
  a: { xPx: number; yPx: number },
  b: { xPx: number; yPx: number },
): number {
  const dx = b.xPx - a.xPx;
  const dy = b.yPx - a.yPx;
  const l2 = dx * dx + dy * dy;
  const t = l2 > 0 ? Math.min(1, Math.max(0, ((x - a.xPx) * dx + (y - a.yPx) * dy) / l2)) : 0;
  return Math.hypot(x - (a.xPx + t * dx), y - (a.yPx + t * dy));
}

describe("bodyRegime", () => {
  it("draws a body under 3 px as its symbol", () => {
    expect([bodyRegime(2.99, AS_BUILT_SCALE), bodyRegime(SYMBOL_BELOW_PX, AS_BUILT_SCALE)]).toEqual(
      ["symbol", "limb"],
    );
  });

  it("draws a body from 8 px with its graticule", () => {
    expect([bodyRegime(7.99, AS_BUILT_SCALE), bodyRegime(8, AS_BUILT_SCALE)]).toEqual([
      "limb",
      "graticule",
    ]);
  });
});

/** The view's line scales: 2 at every ratio up to 2, 3 at 3, and R02's 1 as built. */
const LINE_SCALES = [
  [1, 8, 64],
  [2, 16, 128],
  [3, 24, 192],
] as const;

describe("a body's regime at the view's line scale (R07.T16.g; decision-r07-t16d-followups, (c))", () => {
  it.each(LINE_SCALES)(
    "at a line scale of %s has its limb alone below %s px and its graticule from it",
    (strokeScale, fromPx) => {
      expect([bodyRegime(fromPx - 0.1, strokeScale), bodyRegime(fromPx, strokeScale)]).toEqual([
        "limb",
        "graticule",
      ]);
    },
  );

  it.each(LINE_SCALES)(
    "at a line scale of %s draws its graticule at 30° below %s px and at 15° from it",
    (strokeScale, _, finePx) => {
      expect([
        graticuleStepDeg(finePx - 0.1, strokeScale, false),
        graticuleStepDeg(finePx, strokeScale, false),
        graticuleStepDeg(finePx, strokeScale, true),
      ]).toEqual([30, 15, 30]);
    },
  );

  it("is a body's symbol under 3 px at every line scale, the image's point regime", () => {
    expect(
      LINE_SCALES.map(([strokeScale]) => [
        bodyRegime(2.9, strokeScale),
        bodyRegime(SYMBOL_BELOW_PX, strokeScale),
        isSymbolSized(2.9),
      ]),
    ).toEqual(LINE_SCALES.map(() => ["symbol", "limb", true]));
  });

  it("draws a body's lines by the regime at the scale it is given", () => {
    const camera: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: FOV_X_RAD };
    const pxPerRad = VIEWPORT.widthPx / (2 * Math.tan(FOV_X_RAD / 2));
    // A body straight ahead, its apparent diameter `diameterPx`.
    const at = (diameterPx: number): GraticuleBody => ({
      centreM: vec3(0, 0, -EARTH_RADIUS_M / Math.sin(diameterPx / pxPerRad / 2)),
      radiusM: EARTH_RADIUS_M,
      rotation: null,
      unmodelledPole: null,
    });
    const drawn = (diameterPx: number): [string, number] => {
      const wireframe = graticule(at(diameterPx), camera, VIEWPORT, 2);
      return [wireframe.regime, meridians(wireframe.lines)];
    };
    expect([drawn(15.9), drawn(16.1), drawn(127.9), drawn(128.1)]).toEqual([
      ["limb", 0],
      ["graticule", 12],
      ["graticule", 12],
      ["graticule", 24],
    ]);
  });
});

describe("a body 400 km below the camera", () => {
  const altitudeM = 4e5;
  const centreM = vec3(0, -(EARTH_RADIUS_M + altitudeM), 0);
  const dip = Math.acos(EARTH_RADIUS_M / (EARTH_RADIUS_M + altitudeM));
  const camera: ProjectionCamera = {
    orientation: lookAlong(vec3(Math.cos(dip), -Math.sin(dip), 0), vec3(0, 1, 0)),
    fovXRad: FOV_X_RAD,
  };
  const body: GraticuleBody = {
    centreM,
    radiusM: EARTH_RADIUS_M,
    rotation: null,
    unmodelledPole: null,
  };
  const limb = graticule(body, camera, VIEWPORT, AS_BUILT_SCALE).lines.find(
    (line) => line.kind === "limb",
  );
  const runs: readonly Polyline[] = limb?.runs ?? [];

  it("has its limb on the true horizon: every point on the sphere, its line of sight tangent", () => {
    const points = runs.flat();
    const offSphere = points.map((p) => Math.abs(norm(sub(p, centreM)) - EARTH_RADIUS_M));
    const offTangent = points.map(
      (p) => Math.abs(dot(p, sub(p, centreM))) / (norm(p) * EARTH_RADIUS_M),
    );
    expect({
      points: points.length > 0,
      offSphereM: Math.max(...offSphere) < 1e-6,
      offTangent: Math.max(...offTangent) < 1e-9,
    }).toEqual({ points: true, offSphereM: true, offTangent: true });
  });

  it("keeps its limb within 0.25 px of the true horizon circle between its points", () => {
    const toCentre = scale(centreM, 1 / norm(centreM));
    const [u, v] = perpendicularPair(toCentre);
    const d = norm(centreM);
    const r = EARTH_RADIUS_M;
    const circleCentre = add(centreM, scale(toCentre, -(r * r) / d));
    const across = r * Math.sqrt(1 - (r / d) ** 2);
    const thetaOf = (p: Vec3): number => {
      const q = sub(p, circleCentre);
      return Math.atan2(dot(q, v), dot(q, u));
    };
    let worstPx = 0;
    let checked = 0;
    for (const run of runs) {
      for (let i = 1; i < run.length; i += 1) {
        const a = run[i - 1];
        const b = run[i];
        if (a === undefined || b === undefined) {
          continue;
        }
        const pa = project(a, camera, VIEWPORT);
        const pb = project(b, camera, VIEWPORT);
        const onScreen = (p: { xPx: number; yPx: number }): boolean =>
          p.xPx >= 0 && p.xPx <= VIEWPORT.widthPx && p.yPx >= 0 && p.yPx <= VIEWPORT.heightPx;
        if (!(pa.inFront && pb.inFront && (onScreen(pa) || onScreen(pb)))) {
          continue;
        }
        let ta = thetaOf(a);
        let tb = thetaOf(b);
        if (tb - ta > Math.PI) {
          tb -= 2 * Math.PI;
        } else if (ta - tb > Math.PI) {
          ta -= 2 * Math.PI;
        }
        for (let k = 1; k < 8; k += 1) {
          const theta = ta + ((tb - ta) * k) / 8;
          const truePoint = add(
            circleCentre,
            add(scale(u, across * Math.cos(theta)), scale(v, across * Math.sin(theta))),
          );
          const p = project(truePoint, camera, VIEWPORT);
          worstPx = Math.max(worstPx, toSegmentPx(p.xPx, p.yPx, pa, pb));
          checked += 1;
        }
      }
    }
    expect({ checked: checked > 100, within: worstPx < 0.25 }).toEqual({
      checked: true,
      within: true,
    });
  });
});

describe("a graticule", () => {
  const camera: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: FOV_X_RAD };
  const centreM = vec3(0, 0, -3 * EARTH_RADIUS_M);

  it("emits no point on the far hemisphere", () => {
    const wireframe = graticule(
      { centreM, radiusM: EARTH_RADIUS_M, rotation: null, unmodelledPole: null },
      camera,
      VIEWPORT,
      AS_BUILT_SCALE,
    );
    const lines = wireframe.lines.filter((line) => line.kind !== "limb");
    const facing = lines.flatMap((line) =>
      line.runs.flat().map((p) => -dot(sub(p, centreM), centreM) / EARTH_RADIUS_M),
    );
    expect({
      regime: wireframe.regime,
      lines: lines.length,
      farPoints: facing.filter((f) => f < EARTH_RADIUS_M * (1 - 1e-9)).length,
    }).toEqual({ regime: "graticule", lines: 24 + 11, farPoints: 0 });
  });

  it("turns its prime meridian with the body's rotation", () => {
    const turn = Math.PI / 6;
    const rotation = rotation3FromRows([
      vec3(Math.cos(turn), -Math.sin(turn), 0),
      vec3(Math.sin(turn), Math.cos(turn), 0),
      vec3(0, 0, 1),
    ]);
    // Seen from +x turned by 30°, so that the prime meridian faces the camera.
    const facingCentre = scale(rotateToBody(rotation, vec3(1, 0, 0)), -3 * EARTH_RADIUS_M);
    const looking: ProjectionCamera = {
      orientation: lookAlong(facingCentre, vec3(0, 0, 1)),
      fovXRad: FOV_X_RAD,
    };
    const prime = graticule(
      { centreM: facingCentre, radiusM: EARTH_RADIUS_M, rotation, unmodelledPole: null },
      looking,
      VIEWPORT,
      AS_BUILT_SCALE,
    ).lines.find((line) => line.kind === "meridian" && line.major);
    const planeNormal = rotateToBody(rotation, vec3(0, 1, 0));
    const offPlane = (prime?.runs.flat() ?? []).map(
      (p) => Math.abs(dot(sub(p, facingCentre), planeNormal)) / EARTH_RADIUS_M,
    );
    expect({ points: offPlane.length > 10, inPlane: Math.max(...offPlane) < 1e-9 }).toEqual({
      points: true,
      inPlane: true,
    });
  });

  it("is drawn at 30° on a body under 64 px and at the low setting", () => {
    const small = graticule(
      {
        centreM: vec3(0, 0, -200 * EARTH_RADIUS_M),
        radiusM: EARTH_RADIUS_M,
        rotation: null,
        unmodelledPole: null,
      },
      camera,
      VIEWPORT,
      AS_BUILT_SCALE,
    );
    const low = graticule(
      { centreM, radiusM: EARTH_RADIUS_M, rotation: null, unmodelledPole: null },
      camera,
      VIEWPORT,
      AS_BUILT_SCALE,
      true,
    );
    expect([meridians(small.lines), meridians(low.lines)]).toEqual([12, 12]);
  });
});

/** How many meridians a wireframe has. */
function meridians(lines: ReadonlyArray<{ readonly kind: string }>): number {
  return lines.filter((line) => line.kind === "meridian").length;
}

/** How many of the 15° meridians have some facing arc, worked out by sampling each densely. */
function meridiansFacing(body: GraticuleBody): number {
  let count = 0;
  for (let k = 0; k < 24; k += 1) {
    const lon = (k * 15 * Math.PI) / 180;
    const facing = Array.from({ length: 20_001 }, (_, i) => {
      const lat = -Math.PI / 2 + (Math.PI * i) / 20_000;
      const n = vec3(Math.cos(lat) * Math.cos(lon), Math.cos(lat) * Math.sin(lon), Math.sin(lat));
      return -dot(n, body.centreM) > body.radiusM;
    });
    count += facing.some(Boolean) ? 1 : 0;
  }
  return count;
}

describe("a graticule seen from low altitude", () => {
  /** The camera `altitudeM` above 7.5° N 4° E, looking straight down. */
  function fromAbove(altitudeM: number): { body: GraticuleBody; camera: ProjectionCamera } {
    const lat = (7.5 * Math.PI) / 180;
    const lon = (4 * Math.PI) / 180;
    const up = vec3(Math.cos(lat) * Math.cos(lon), Math.cos(lat) * Math.sin(lon), Math.sin(lat));
    return {
      body: {
        centreM: scale(up, -(EARTH_RADIUS_M + altitudeM)),
        radiusM: EARTH_RADIUS_M,
        rotation: null,
        unmodelledPole: null,
      },
      camera: { orientation: lookAlong(scale(up, -1), vec3(0, 0, 1)), fovXRad: FOV_X_RAD },
    };
  }

  it.each([4e5, 1e4])("draws every meridian with a facing arc from %d m", (altitudeM) => {
    const { body, camera } = fromAbove(altitudeM);
    const drawn = graticule(body, camera, VIEWPORT, AS_BUILT_SCALE).lines.filter(
      (line) => line.kind === "meridian" && line.runs.length > 0,
    ).length;
    expect(drawn).toBe(meridiansFacing(body));
  });
});

describe("an unrotated body's graticule", () => {
  it("is drawn about its orbit normal", () => {
    const pole = scale(vec3(0.3, 0.2, 1), 1 / norm(vec3(0.3, 0.2, 1)));
    const centreM = vec3(0, 0, -3 * EARTH_RADIUS_M);
    const equator = graticule(
      { centreM, radiusM: EARTH_RADIUS_M, rotation: null, unmodelledPole: pole },
      { orientation: IDENTITY_QUATERNION, fovXRad: FOV_X_RAD },
      VIEWPORT,
      AS_BUILT_SCALE,
    ).lines.find((line) => line.kind === "parallel" && line.major);
    const offEquator = (equator?.runs.flat() ?? []).map(
      (p) => Math.abs(dot(sub(p, centreM), pole)) / EARTH_RADIUS_M,
    );
    expect({ points: offEquator.length > 10, onEquator: Math.max(...offEquator) < 1e-9 }).toEqual({
      points: true,
      onEquator: true,
    });
  });
});

describe("ringEllipse", () => {
  it("draws both edges and a radial tick every 10°", () => {
    const camera: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: FOV_X_RAD };
    const lines = ringEllipse(
      {
        centreM: vec3(0, 0, -5e8),
        innerRadiusM: 7e7,
        outerRadiusM: 1.4e8,
        normal: vec3(0, 1, 0.2),
      },
      camera,
      VIEWPORT,
    );
    const ticks = lines.filter((line) => line.length === 2);
    expect({ ticks: ticks.length, edges: lines.length - ticks.length >= 2 }).toEqual({
      ticks: 36,
      edges: true,
    });
  });
});

/** `TEST EARTH`'s figure's radius, m, a sphere of 6,371,008.8 m (`ATMOSPHERE_FIXTURES`). */
function testEarthRadiusM(): number {
  const earth = ATMOSPHERE_FIXTURES.find((fixture) => fixture.designation === "TEST EARTH");
  if (earth === undefined) {
    throw new Error("ATMOSPHERE_FIXTURES has no TEST EARTH");
  }
  return earth.figure.equatorialRadiusM;
}

/** `TEST EARTH`'s radius, m. */
const TEST_EARTH_RADIUS_M = testEarthRadiusM();

/** The range at which the datum covered the air's limb glow, m from the body's centre: 51 Mm. */
const GLOW_RANGE_M = 51e6;

/** The field the finding was seen on: 1920 × 1080 at the default 60°. */
const FIELD: Viewport = { widthPx: 1920, heightPx: 1080 };
const FIELD_CAMERA: ProjectionCamera = {
  orientation: IDENTITY_QUATERNION,
  fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180,
};
const FOCAL_PX = FIELD.widthPx / (2 * Math.tan(FIELD_CAMERA.fovXRad / 2));

/**
 * A rotation with no axis in common with the camera's, so that both meridians and parallels meet
 * the limb.
 */
const TILTED = rotation3FromRows(rotationRows(lookAlong(normalise(vec3(1, 2, 3)), vec3(0, 0, 1))));

/** A sphere of `radiusM` at `rangeM`, its centre at pixel (`xPx`, `yPx`) of the field. */
function sphereAt(xPx: number, yPx: number, rangeM: number, radiusM: number): GraticuleBody {
  const direction = normalise(
    vec3((xPx - FIELD.widthPx / 2) / FOCAL_PX, -(yPx - FIELD.heightPx / 2) / FOCAL_PX, -1),
  );
  return { centreM: scale(direction, rangeM), radiusM, rotation: TILTED, unmodelledPole: null };
}

/** Where `TEST EARTH` is placed on the field: at its centre, and near its top right corner. */
const PLACEMENTS = [
  ["the field's centre", 960, 540],
  ["near a corner", 1640, 280],
] as const;

/** The line scales the view draws at: 2 at every ratio up to 2, and 3 at 3. */
const VIEW_SCALES = [2, 3] as const;

/**
 * The datum's reaches at a line scale as the brief states them, δ = w ÷ 2 + c + 0.5 device px,
 * w the stroke's width and c its casing's on each side, both at the scale.
 */
function reachesAt(strokeScale: number): DatumInside {
  const casingPx = CASING_PX * strokeScale;
  return {
    reachPx: {
      thin: (STROKE_PX.thin * strokeScale) / 2 + casingPx + 0.5,
      heavy: (STROKE_PX.heavy * strokeScale) / 2 + casingPx + 0.5,
    },
  };
}

/** A view the datum is drawn on: its camera and its size. */
interface Field {
  readonly camera: ProjectionCamera;
  readonly viewport: Viewport;
}

/** The field the finding was seen on. */
const SIXTY: Field = { camera: FIELD_CAMERA, viewport: FIELD };

/** The widest field the camera steps to, 120° across, at 3840 × 2160. */
const WIDE: Field = {
  camera: { orientation: IDENTITY_QUATERNION, fovXRad: (120 * Math.PI) / 180 },
  viewport: { widthPx: 3840, heightPx: 2160 },
};

/**
 * `TEST EARTH` on {@link WIDE}'s axis, so near that its limb, 62.6° off the axis, passes by the
 * field's corners, 63.3° off it, where a pixel subtends a fifth of the angle it does at the centre.
 */
const NEAR_THE_CORNERS: GraticuleBody = {
  centreM: vec3(0, 0, -TEST_EARTH_RADIUS_M / Math.sin((62.6 * Math.PI) / 180)),
  radiusM: TEST_EARTH_RADIUS_M,
  rotation: TILTED,
  unmodelledPole: null,
};

/**
 * How far inside the body's outline on the field a point is seen, device px: its distance from
 * the outline's nearest point, the horizon's projection sampled every half degree and refined by
 * golden section; negative where its line of sight misses the sphere.
 */
function insetFromOutline(body: GraticuleBody, field: Field = SIXTY): (point: Vec3) => number {
  const d = norm(body.centreM);
  const r = body.radiusM;
  const toCentre = scale(body.centreM, 1 / d);
  const [u, v] = perpendicularPair(toCentre);
  const circleCentre = add(body.centreM, scale(toCentre, -(r * r) / d));
  const across = r * Math.sqrt(1 - (r / d) ** 2);
  const outlineAt = (theta: number): { xPx: number; yPx: number } =>
    project(
      add(
        circleCentre,
        add(scale(u, across * Math.cos(theta)), scale(v, across * Math.sin(theta))),
      ),
      field.camera,
      field.viewport,
    );
  const step = (2 * Math.PI) / 720;
  const coarse = Array.from({ length: 720 }, (_, i) => outlineAt(i * step));
  const cosLimb = Math.sqrt(1 - (r / d) ** 2);
  return (point) => {
    const q = project(point, field.camera, field.viewport);
    const squared = (p: { xPx: number; yPx: number }): number =>
      (p.xPx - q.xPx) ** 2 + (p.yPx - q.yPx) ** 2;
    let nearest = 0;
    let nearestSquared = Number.POSITIVE_INFINITY;
    coarse.forEach((p, i) => {
      if (squared(p) < nearestSquared) {
        nearest = i;
        nearestSquared = squared(p);
      }
    });
    let lo = (nearest - 1) * step;
    let hi = (nearest + 1) * step;
    const golden = (Math.sqrt(5) - 1) / 2;
    for (let i = 0; i < 80; i += 1) {
      const a = hi - golden * (hi - lo);
      const b = lo + golden * (hi - lo);
      if (squared(outlineAt(a)) < squared(outlineAt(b))) {
        hi = b;
      } else {
        lo = a;
      }
    }
    const distancePx = Math.sqrt(squared(outlineAt((lo + hi) / 2)));
    return dot(point, toCentre) / norm(point) >= cosLimb ? distancePx : -distancePx;
  };
}

/** The points of a run's chords at their ends and their quarter points. */
function alongChords(runs: ReadonlyArray<Polyline>): Vec3[] {
  return runs.flatMap((run) =>
    run.flatMap((p, i) => {
      const next = run[i + 1];
      return next === undefined
        ? [p]
        : [0, 0.25, 0.5, 0.75].map((t) => add(p, scale(sub(next, p), t)));
    }),
  );
}

/**
 * The ends of a line's runs where its facing arc is cut: not at a pole, and not where another of
 * its runs goes on (a parallel across its λ = 0).
 */
function cutEnds(runs: ReadonlyArray<Polyline>, body: GraticuleBody): Vec3[] {
  const pole = rotateToBody(TILTED, vec3(0, 0, 1));
  const ends = runs.flatMap((run) => [run[0], run.at(-1)]).filter((p) => p !== undefined);
  return ends.filter((end) => {
    const n = scale(sub(end, body.centreM), 1 / body.radiusM);
    const atPole = Math.abs(Math.abs(dot(n, pole)) - 1) < 1e-9;
    const shared = ends.filter((other) => norm(sub(other, end)) < 1e-6 * body.radiusM).length > 1;
    return !atPole && !shared;
  });
}

describe("the datum stroked inside the figure over the image (R10.T16.g)", () => {
  const cases = PLACEMENTS.flatMap(([where, xPx, yPx]) =>
    VIEW_SCALES.map((strokeScale) => [where, strokeScale, xPx, yPx] as const),
  );

  it("takes the reaches w ÷ 2 + c + 0.5: 3.5 and 4 px at a line scale of 2, 5 and 5.75 at 3", () => {
    expect(VIEW_SCALES.map((strokeScale) => reachesAt(strokeScale).reachPx)).toEqual([
      { thin: 3.5, heavy: 4 },
      { thin: 5, heavy: 5.75 },
    ]);
  });

  it.each(cases)(
    "strokes TEST EARTH's limb at 51 Mm, at %s, at a line scale of %s, its reach to half a pixel more inside the outline at every point",
    (_, strokeScale, xPx, yPx) => {
      const body = sphereAt(xPx, yPx, GLOW_RANGE_M, TEST_EARTH_RADIUS_M);
      const inside = reachesAt(strokeScale);
      const limb = graticule(body, FIELD_CAMERA, FIELD, strokeScale, false, inside).lines.find(
        (line) => line.kind === "limb",
      );
      const insets = alongChords(limb?.runs ?? []).map(insetFromOutline(body));
      const reach = inside.reachPx.thin;
      expect({
        points: insets.length > 200,
        leastOverReach: Math.min(...insets) - reach >= 0,
        mostOverReach: Math.max(...insets) - reach <= 0.5,
      }).toEqual({ points: true, leastOverReach: true, mostOverReach: true });
    },
  );

  it.each(cases)(
    "lifts every point of the limb onto the sphere's near side, at %s, at a line scale of %s",
    (_, strokeScale, xPx, yPx) => {
      const body = sphereAt(xPx, yPx, GLOW_RANGE_M, TEST_EARTH_RADIUS_M);
      const limb = graticule(
        body,
        FIELD_CAMERA,
        FIELD,
        strokeScale,
        false,
        reachesAt(strokeScale),
      ).lines.find((line) => line.kind === "limb");
      const points = (limb?.runs ?? []).flat();
      const offSphere = points.map(
        (p) => Math.abs(norm(sub(p, body.centreM)) - body.radiusM) / body.radiusM,
      );
      expect({
        points: points.length > 50,
        onSphere: Math.max(...offSphere) <= 1e-9,
        nearSide: points.every((p) => dot(p, sub(p, body.centreM)) < 0),
      }).toEqual({ points: true, onSphere: true, nearSide: true });
    },
  );

  /** A body's meridians and parallels on a field, inside its figure, each with its reach. */
  function graticuleLinesOf(
    body: GraticuleBody,
    strokeScale: number,
    field: Field,
  ): ReadonlyArray<{ readonly runs: ReadonlyArray<Polyline>; readonly reachPx: number }> {
    const inside = reachesAt(strokeScale);
    return graticule(body, field.camera, field.viewport, strokeScale, false, inside)
      .lines.filter((line) => line.kind !== "limb")
      .map((line) => ({
        runs: line.runs,
        reachPx: line.major ? inside.reachPx.heavy : inside.reachPx.thin,
      }));
  }

  /**
   * How far inside the outline each point of a body's meridians and parallels stands past its
   * own reach, px, and each of their cut ends.
   */
  function marginsOf(
    body: GraticuleBody,
    strokeScale: number,
    field: Field,
  ): { readonly lines: number; readonly points: number[]; readonly cutEnds: number[] } {
    const inset = insetFromOutline(body, field);
    const lines = graticuleLinesOf(body, strokeScale, field);
    return {
      lines: lines.length,
      points: lines.flatMap((line) => line.runs.flat().map((p) => inset(p) - line.reachPx)),
      cutEnds: lines.flatMap((line) =>
        cutEnds(line.runs, body).map((p) => inset(p) - line.reachPx),
      ),
    };
  }

  it.each(cases)(
    "keeps every point of every meridian and parallel its own reach inside the outline, at %s, at a line scale of %s",
    (_, strokeScale, xPx, yPx) => {
      const body = sphereAt(xPx, yPx, GLOW_RANGE_M, TEST_EARTH_RADIUS_M);
      const { lines, points } = marginsOf(body, strokeScale, SIXTY);
      expect({
        lines,
        points: points.length > 500,
        leastMargin: Math.min(...points) >= 0,
      }).toEqual({ lines: 24 + 11, points: true, leastMargin: true });
    },
  );

  it.each(cases)(
    "ends each meridian and parallel cut short of the limb at its own reach inside the outline, under the limb's casing, at %s, at a line scale of %s",
    (_, strokeScale, xPx, yPx) => {
      const body = sphereAt(xPx, yPx, GLOW_RANGE_M, TEST_EARTH_RADIUS_M);
      const { cutEnds: ends } = marginsOf(body, strokeScale, SIXTY);
      expect({
        cutEnds: ends.length > 20,
        atTheReach: ends.every((margin) => margin >= 0 && margin <= 0.05),
      }).toEqual({ cutEnds: true, atTheReach: true });
    },
  );

  it.each(VIEW_SCALES)(
    "keeps every point of every meridian and parallel its own reach inside the outline in a 120° field, at a line scale of %s",
    (strokeScale) => {
      const { points } = marginsOf(NEAR_THE_CORNERS, strokeScale, WIDE);
      expect([points.length > 300, Math.min(...points) >= 0]).toEqual([true, true]);
    },
  );

  it.each(VIEW_SCALES)(
    "ends each meridian and parallel at its own reach inside the outline in a 120° field, the limb near its corners, at a line scale of %s",
    (strokeScale) => {
      const { cutEnds: ends } = marginsOf(NEAR_THE_CORNERS, strokeScale, WIDE);
      expect([ends.length > 20, ends.every((margin) => margin >= 0 && margin <= 0.05)]).toEqual([
        true,
        true,
      ]);
    },
  );

  it.each(cases)(
    "builds the wireframe's datum as built with the option off, at %s, at a line scale of %s",
    (_, strokeScale, xPx, yPx) => {
      const body = sphereAt(xPx, yPx, GLOW_RANGE_M, TEST_EARTH_RADIUS_M);
      expect(graticule(body, FIELD_CAMERA, FIELD, strokeScale, false, null)).toEqual(
        graticule(body, FIELD_CAMERA, FIELD, strokeScale),
      );
    },
  );

  it.each(cases)(
    "leaves the wireframe's limb on the horizon, at %s, at a line scale of %s",
    (_, strokeScale, xPx, yPx) => {
      const body = sphereAt(xPx, yPx, GLOW_RANGE_M, TEST_EARTH_RADIUS_M);
      const limb = graticule(body, FIELD_CAMERA, FIELD, strokeScale, false, null).lines.find(
        (line) => line.kind === "limb",
      );
      const insets = (limb?.runs ?? []).flat().map(insetFromOutline(body));
      expect({
        points: insets.length > 100,
        onOutline: Math.max(...insets.map(Math.abs)) < 1e-4,
      }).toEqual({ points: true, onOutline: true });
    },
  );

  /**
   * `TEST EARTH` at the field's centre at `share` of the least apparent radius of a datum over the
   * image, its regime, its lines' count and its apparent radius, px.
   */
  function atShareOfTheLeastRadius(
    strokeScale: number,
    share: number,
  ): { readonly regime: string; readonly lines: number; readonly radiusPx: number } {
    const inside = reachesAt(strokeScale);
    const radiusPx = DATUM_OVER_IMAGE_RADIUS_PER_REACH * inside.reachPx.thin * share;
    // Its apparent radius is asin(r ÷ d) at the centre's pixel scale.
    const body = sphereAt(
      960,
      540,
      TEST_EARTH_RADIUS_M / Math.sin(radiusPx / FOCAL_PX),
      TEST_EARTH_RADIUS_M,
    );
    const wireframe = graticule(body, FIELD_CAMERA, FIELD, strokeScale, false, inside);
    return {
      regime: wireframe.regime,
      lines: wireframe.lines.length,
      radiusPx: angularDiameterPx(body.centreM, body.radiusM, FIELD_CAMERA, FIELD) / 2,
    };
  }

  it.each([
    [2, 23.9],
    [3, 34.14],
  ])(
    "draws the datum over the image at a line scale of %s from 2(2 + √2) limb reaches of apparent radius, %s px",
    (strokeScale, radiusPx) => {
      // Past the rounding of the radius's round trip through the range.
      const at = atShareOfTheLeastRadius(strokeScale, 1 + 1e-12);
      // Its limb and a graticule at 30°: 12 meridians and 5 parallels.
      expect([at.regime, at.lines, Math.round(at.radiusPx * 100) / 100]).toEqual([
        "graticule",
        1 + 12 + 5,
        radiusPx,
      ]);
    },
  );

  it.each([2, 3])(
    "draws no datum over the image at a line scale of %s just under that radius, the body's regime kept",
    (strokeScale) => {
      const under = atShareOfTheLeastRadius(strokeScale, 1 - 1e-9);
      expect([under.regime, under.lines]).toEqual(["graticule", 0]);
    },
  );
});
