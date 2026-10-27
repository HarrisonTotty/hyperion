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
  datumSpheroidOfFigure,
  type GraticuleBody,
  type GraticuleLine,
  graticule,
  graticuleRotation,
  graticuleStepDeg,
  isSymbolSized,
  outlineLeastDiameterPx,
  perpendicularPair,
  ringEllipse,
  SYMBOL_BELOW_PX,
} from "./bodies";
import { cosineWindows, type Polyline } from "./curve";
import { CASING_PX, STROKE_PX } from "./drawList";
import { datumSpheroidOf } from "./spheroid";

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
      spheroid: null,
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
    spheroid: null,
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
      { centreM, radiusM: EARTH_RADIUS_M, rotation: null, unmodelledPole: null, spheroid: null },
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
      {
        centreM: facingCentre,
        radiusM: EARTH_RADIUS_M,
        rotation,
        unmodelledPole: null,
        spheroid: null,
      },
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
        spheroid: null,
      },
      camera,
      VIEWPORT,
      AS_BUILT_SCALE,
    );
    const low = graticule(
      { centreM, radiusM: EARTH_RADIUS_M, rotation: null, unmodelledPole: null, spheroid: null },
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
        spheroid: null,
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
      { centreM, radiusM: EARTH_RADIUS_M, rotation: null, unmodelledPole: pole, spheroid: null },
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
  return {
    centreM: scale(direction, rangeM),
    radiusM,
    rotation: TILTED,
    unmodelledPole: null,
    spheroid: null,
  };
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
  spheroid: null,
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

  it.each([
    [2, 24, 23],
    [3, 35, 34],
  ])(
    "draws the datum over the image at a line scale of %s at %s px of apparent radius and none at %s px, the ruling's points",
    (strokeScale, drawnPx, undrawnPx) => {
      const leastPx = DATUM_OVER_IMAGE_RADIUS_PER_REACH * reachesAt(strokeScale).reachPx.thin;
      const linesAt = (radiusPx: number): number =>
        atShareOfTheLeastRadius(strokeScale, radiusPx / leastPx).lines;
      expect([linesAt(drawnPx) > 0, linesAt(undrawnPx)]).toEqual([true, 0]);
    },
  );

  it.each(VIEW_SCALES)(
    "covers at most half the disc with the inside limb's cased band at the least radius, at a line scale of %s",
    (strokeScale) => {
      const reachPx = reachesAt(strokeScale).reachPx.thin;
      const radiusPx = DATUM_OVER_IMAGE_RADIUS_PER_REACH * reachPx;
      // The band, 2δ wide from the limb in, covers 1 − (1 − 2δ ÷ R)² of the disc.
      expect(1 - (1 - (2 * reachPx) / radiusPx) ** 2).toBeLessThanOrEqual(0.5 + 1e-12);
    },
  );
});

/** `TEST SATURN`'s figure, Saturn's spheroid at 1 bar, 60,268 by 54,364 km (`ATMOSPHERE_FIXTURES`). */
function testSaturnFigure(): { readonly equatorialRadiusM: number; readonly polarRadiusM: number } {
  const saturn = ATMOSPHERE_FIXTURES.find((fixture) => fixture.designation === "TEST SATURN");
  if (saturn === undefined) {
    throw new Error("ATMOSPHERE_FIXTURES has no TEST SATURN");
  }
  return saturn.figure;
}

const SATURN = testSaturnFigure();

/** The range of R10.T16.g's measurement on `TEST SATURN`, m from its centre: 482 Mm. */
const SATURN_RANGE_M = 482e6;

/**
 * `TEST SATURN` at {@link SATURN_RANGE_M}, its centre at pixel (`xPx`, `yPx`) of the field, about
 * `pole`, as the draw list gives it: its datum on its spheroid, and with no rotation its graticule
 * drawn about the spheroid's pole.
 */
function saturnAt(xPx: number, yPx: number, pole: Vec3): GraticuleBody {
  const direction = normalise(
    vec3((xPx - FIELD.widthPx / 2) / FOCAL_PX, -(yPx - FIELD.heightPx / 2) / FOCAL_PX, -1),
  );
  return {
    centreM: scale(direction, SATURN_RANGE_M),
    radiusM: SATURN.equatorialRadiusM,
    rotation: null,
    unmodelledPole: pole,
    spheroid: datumSpheroidOf(SATURN, pole),
  };
}

/** The ways `TEST SATURN` is seen: equator-on, its pole up the field; pole-on; and tilted. */
const SATURN_VIEWS = [
  ["equator-on", 960, 540, vec3(0, 1, 0)],
  ["pole-on", 960, 540, vec3(0, 0, 1)],
  // Its rings' opening at 26.7°, near the field's corner where T16.g placed `TEST EARTH`.
  ["tilted, near a corner", 1640, 280, normalise(vec3(0.3, 1, Math.tan((26.7 * Math.PI) / 180)))],
] as const;

/**
 * `TEST SATURN` on {@link WIDE}'s axis, about the tilted pole, so near that its equatorial limb
 * stands 62.6° off the axis and its polar one 60.0°, both past the field's sides and by its
 * corners, 63.3° off it.
 */
const SATURN_NEAR_THE_CORNERS: GraticuleBody = {
  ...saturnAt(960, 540, SATURN_VIEWS[2][3]),
  centreM: vec3(0, 0, -SATURN.equatorialRadiusM / Math.sin((62.6 * Math.PI) / 180)),
};

/** The lengths of each line's runs: the shape of a body's wireframe. */
function shape(lines: ReadonlyArray<GraticuleLine>): number[][] {
  return lines.map((line) => line.runs.map((run) => run.length));
}

/** The spheroid of `body`: its pole and polar radius, the quadric's two terms at a point. */
function spheroidOfBody(body: GraticuleBody): { readonly pole: Vec3; readonly c: number } {
  const spheroid = body.spheroid ?? null;
  if (spheroid === null) {
    throw new Error("the body is drawn on no spheroid");
  }
  return { pole: spheroid.pole, c: body.radiusM / spheroid.stretch };
}

/**
 * How far off the spheroid a point lies, as a fraction of its radius: √(ρ² ÷ a² + z² ÷ c²) − 1, ρ
 * and z its distance from the pole's axis and along it, from the body's centre.
 */
function offSpheroid(body: GraticuleBody, p: Vec3): number {
  const { pole, c } = spheroidOfBody(body);
  const q = sub(p, body.centreM);
  const z = dot(q, pole);
  const rho2 = dot(q, q) - z * z;
  return Math.abs(Math.sqrt(rho2 / body.radiusM ** 2 + (z * z) / (c * c)) - 1);
}

/** The spheroid's outward normal at a point of it, unit: the quadric's gradient. */
function normalAt(body: GraticuleBody, p: Vec3): Vec3 {
  const { pole, c } = spheroidOfBody(body);
  const q = sub(p, body.centreM);
  const z = dot(q, pole);
  const across = sub(q, scale(pole, z));
  return normalise(add(scale(across, 1 / body.radiusM ** 2), scale(pole, z / (c * c))));
}

/**
 * The discriminant of a line of sight u with the spheroid, (uᵀSc)² − (cᵀSc − 1)(uᵀSu), S its
 * quadric's matrix, (I − ppᵀ) ÷ a² + ppᵀ ÷ c²: positive where the line meets it. Written from the
 * quadric alone, apart from the code's stretched sphere.
 */
function discriminant(body: GraticuleBody, u: Vec3): number {
  const { pole, c } = spheroidOfBody(body);
  const s = (v: Vec3): Vec3 => {
    const z = dot(v, pole);
    return add(scale(sub(v, scale(pole, z)), 1 / body.radiusM ** 2), scale(pole, z / (c * c)));
  };
  const centre = body.centreM;
  const sc = s(centre);
  return dot(u, sc) ** 2 - (dot(centre, sc) - 1) * dot(u, s(u));
}

/**
 * How far inside the spheroid's outline on the field a point is seen, device px: its distance
 * from the outline's nearest point, negative where its line of sight misses the spheroid. The
 * outline is found from the quadric alone: along each of 720 directions from the centre's pixel,
 * the radius at which the pixel's line of sight grazes it, by bisection; the nearest is refined by
 * golden section.
 */
function insetFromSpheroidOutline(
  body: GraticuleBody,
  field: Field = SIXTY,
): (point: Vec3) => number {
  const { camera, viewport } = field;
  const focalPx = viewport.widthPx / (2 * Math.tan(camera.fovXRad / 2));
  const centrePx = project(body.centreM, camera, viewport);
  const rayAt = (xPx: number, yPx: number): Vec3 =>
    vec3((xPx - viewport.widthPx / 2) / focalPx, -(yPx - viewport.heightPx / 2) / focalPx, -1);
  const farPx = 4 * focalPx * Math.tan(Math.asin(body.radiusM / norm(body.centreM)));
  const outlineAt = (theta: number): { xPx: number; yPx: number } => {
    const [dx, dy] = [Math.cos(theta), Math.sin(theta)];
    let inPx = 0;
    let outPx = farPx;
    for (let i = 0; i < 80; i += 1) {
      const mid = (inPx + outPx) / 2;
      if (discriminant(body, rayAt(centrePx.xPx + mid * dx, centrePx.yPx + mid * dy)) >= 0) {
        inPx = mid;
      } else {
        outPx = mid;
      }
    }
    const radiusPx = (inPx + outPx) / 2;
    return { xPx: centrePx.xPx + radiusPx * dx, yPx: centrePx.yPx + radiusPx * dy };
  };
  const step = (2 * Math.PI) / 720;
  const coarse = Array.from({ length: 720 }, (_, i) => outlineAt(i * step));
  return (point) => {
    const q = project(point, camera, viewport);
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
    for (let i = 0; i < 60; i += 1) {
      const a = hi - golden * (hi - lo);
      const b = lo + golden * (hi - lo);
      if (squared(outlineAt(a)) < squared(outlineAt(b))) {
        hi = b;
      } else {
        lo = a;
      }
    }
    const distancePx = Math.sqrt(squared(outlineAt((lo + hi) / 2)));
    return discriminant(body, rayAt(q.xPx, q.yPx)) >= 0 ? distancePx : -distancePx;
  };
}

describe("the datum on the body's spheroid (R02.T19), on TEST SATURN at 482 Mm", () => {
  const cases = SATURN_VIEWS.flatMap(([how, xPx, yPx, pole]) =>
    VIEW_SCALES.map((strokeScale) => [how, strokeScale, xPx, yPx, pole] as const),
  );

  /** Every point of every line of the body's datum, in either style. */
  function pointsOf(
    body: GraticuleBody,
    strokeScale: number,
    inside: DatumInside | null,
  ): ReadonlyArray<Vec3> {
    return graticule(body, FIELD_CAMERA, FIELD, strokeScale, false, inside).lines.flatMap((line) =>
      line.runs.flat(),
    );
  }

  it.each(cases)(
    "strokes the limb, seen %s at a line scale of %s, its reach to half a pixel more inside the figure's outline at every point",
    (_, strokeScale, xPx, yPx, pole) => {
      const body = saturnAt(xPx, yPx, pole);
      const inside = reachesAt(strokeScale);
      const limb = graticule(body, FIELD_CAMERA, FIELD, strokeScale, false, inside).lines.find(
        (line) => line.kind === "limb",
      );
      const insets = alongChords(limb?.runs ?? []).map(insetFromSpheroidOutline(body));
      const reach = inside.reachPx.thin;
      expect({
        points: insets.length > 200,
        leastOverReach: Math.min(...insets) - reach >= 0,
        mostOverReach: Math.max(...insets) - reach <= 0.5,
      }).toEqual({ points: true, leastOverReach: true, mostOverReach: true });
    },
  );

  it.each(cases)(
    "puts every point of the limb and graticule on the spheroid to 10⁻⁹ of its radius, seen %s at a line scale of %s, in both styles",
    (_, strokeScale, xPx, yPx, pole) => {
      const body = saturnAt(xPx, yPx, pole);
      const styles = [null, reachesAt(strokeScale)].map((inside) =>
        pointsOf(body, strokeScale, inside).map((p) => offSpheroid(body, p)),
      );
      expect(styles.map((off) => [off.length > 400, Math.max(...off) <= 1e-9])).toEqual([
        [true, true],
        [true, true],
      ]);
    },
  );

  it.each(cases)(
    "faces every point by the spheroid's normal, none on its far side, seen %s at a line scale of %s, in both styles",
    (_, strokeScale, xPx, yPx, pole) => {
      const body = saturnAt(xPx, yPx, pole);
      // −p̂ · n̂ is the cosine of the line of sight's angle to the normal: 0 on the horizon.
      const facing = [null, reachesAt(strokeScale)].map((inside) =>
        pointsOf(body, strokeScale, inside).map((p) => -dot(normalise(p), normalAt(body, p))),
      );
      expect(facing.map((cosines) => Math.min(...cosines) >= -1e-9)).toEqual([true, true]);
    },
  );

  /**
   * How far inside the spheroid's outline each point of a body's meridians and parallels stands
   * past its own reach on a field, px, and each of their cut ends: not at a pole, nor where the
   * line's next run goes on (a parallel across its λ = 0).
   */
  function spheroidMarginsOf(
    body: GraticuleBody,
    strokeScale: number,
    field: Field,
  ): { readonly lines: number; readonly points: number[]; readonly cutEnds: number[] } {
    const inset = insetFromSpheroidOutline(body, field);
    const inside = reachesAt(strokeScale);
    const lines = graticule(
      body,
      field.camera,
      field.viewport,
      strokeScale,
      false,
      inside,
    ).lines.filter((line) => line.kind !== "limb");
    const { pole } = spheroidOfBody(body);
    const reachOf = (major: boolean): number =>
      major ? inside.reachPx.heavy : inside.reachPx.thin;
    return {
      lines: lines.length,
      points: lines.flatMap((line) => line.runs.flat().map((p) => inset(p) - reachOf(line.major))),
      cutEnds: lines.flatMap((line) => {
        const all = line.runs.flatMap((run) => [run[0], run.at(-1)]).filter((p) => p !== undefined);
        return all
          .filter((end) => {
            const atPole =
              Math.abs(Math.abs(dot(normalise(sub(end, body.centreM)), pole)) - 1) < 1e-9;
            const shared = all.filter((o) => norm(sub(o, end)) < 1e-6 * body.radiusM).length > 1;
            return !atPole && !shared;
          })
          .map((end) => inset(end) - reachOf(line.major));
      }),
    };
  }

  it.each(cases)(
    "keeps every point of every meridian and parallel its own reach inside the figure's outline, seen %s at a line scale of %s",
    (_, strokeScale, xPx, yPx, pole) => {
      const { lines, points } = spheroidMarginsOf(saturnAt(xPx, yPx, pole), strokeScale, SIXTY);
      expect({ lines, points: points.length > 300, leastMargin: Math.min(...points) >= 0 }).toEqual(
        {
          lines: 24 + 11,
          points: true,
          leastMargin: true,
        },
      );
    },
  );

  it.each(cases)(
    "ends each meridian and parallel cut short of the limb at its own reach inside the figure's outline, seen %s at a line scale of %s",
    (_, strokeScale, xPx, yPx, pole) => {
      const ends = spheroidMarginsOf(saturnAt(xPx, yPx, pole), strokeScale, SIXTY).cutEnds;
      expect({
        cutEnds: ends.length > 10,
        atTheReach: ends.every((margin) => margin >= 0 && margin <= 0.05),
      }).toEqual({ cutEnds: true, atTheReach: true });
    },
  );

  it.each(VIEW_SCALES)(
    "keeps every point of every meridian and parallel its own reach inside the figure's outline, its cut ends at the reach, in a 120° field with the limb by its corners, at a line scale of %s",
    (strokeScale) => {
      const margins = spheroidMarginsOf(SATURN_NEAR_THE_CORNERS, strokeScale, WIDE);
      const { points } = margins;
      const ends = margins.cutEnds;
      expect([
        points.length > 200,
        Math.min(...points) >= 0,
        ends.length > 10,
        ends.every((margin) => margin >= 0 && margin <= 0.05),
      ]).toEqual([true, true, true, true]);
    },
  );

  it.each(SATURN_VIEWS)(
    "draws the wireframe's limb on the spheroid's horizon, seen %s: every line of sight to it grazes the figure",
    (_, xPx, yPx, pole) => {
      const body = saturnAt(xPx, yPx, pole);
      const limb = graticule(body, FIELD_CAMERA, FIELD, 2).lines.find(
        (line) => line.kind === "limb",
      );
      const points = (limb?.runs ?? []).flat();
      const inset = insetFromSpheroidOutline(body);
      expect({
        points: points.length > 50,
        grazing:
          Math.max(...points.map((p) => Math.abs(dot(normalise(p), normalAt(body, p))))) < 1e-9,
        onOutline: Math.max(...points.map((p) => Math.abs(inset(p)))) < 1e-4,
        betweenPoints:
          Math.max(...alongChords(limb?.runs ?? []).map((p) => Math.abs(inset(p)))) < 0.25,
      }).toEqual({ points: true, grazing: true, onOutline: true, betweenPoints: true });
    },
  );

  it.each(SATURN_VIEWS)(
    "draws the parallels at planetocentric latitudes every 15°, and the meridians every 15° of longitude, seen %s",
    (_, xPx, yPx, pole) => {
      const body = saturnAt(xPx, yPx, pole);
      const lines = graticule(body, FIELD_CAMERA, FIELD, 2).lines;
      // The graticule's prime meridian's axis and the one a quarter-turn east, about the pole.
      const rotation = graticuleRotation(body);
      const east = [vec3(1, 0, 0), vec3(0, 1, 0)].map((axis) => rotateToBody(rotation, axis));
      const fromCentre = (p: Vec3): Vec3 => normalise(sub(p, body.centreM));
      const latitudeDeg = (p: Vec3): number =>
        (Math.asin(dot(fromCentre(p), pole)) * 180) / Math.PI;
      const longitudeDeg = (p: Vec3): number =>
        (Math.atan2(dot(fromCentre(p), east[1] ?? pole), dot(fromCentre(p), east[0] ?? pole)) *
          180) /
        Math.PI;
      // Each parallel's points off its latitude, −75° + 15° k, and each meridian's off its
      // longitude, 15° k, but at the poles, degrees.
      const parallels = lines.filter((line) => line.kind === "parallel");
      const offLatitude = parallels.flatMap((line, k) =>
        line.runs.flat().map((p) => Math.abs(latitudeDeg(p) - (-75 + 15 * k))),
      );
      const offLongitude = lines
        .filter((line) => line.kind === "meridian")
        .flatMap((line, k) =>
          line.runs
            .flat()
            .filter((p) => Math.abs(latitudeDeg(p)) < 89.9)
            .map((p) => Math.abs(((longitudeDeg(p) - 15 * k + 540) % 360) - 180)),
        );
      expect([
        parallels.length,
        offLatitude.length > 50,
        Math.max(...offLatitude) < 1e-9,
        offLongitude.length > 50,
        Math.max(...offLongitude) < 1e-9,
      ]).toEqual([11, true, true, true, true]);
    },
  );

  it("leaves the sphere's limb, the datum as R10.T16.g drew it, 17 px beyond the figure at the poles seen equator-on, as T16.g measured", () => {
    const body = saturnAt(960, 540, vec3(0, 1, 0));
    const inside = reachesAt(2);
    const limbOf = (each: GraticuleBody): ReadonlyArray<Vec3> =>
      graticule(each, FIELD_CAMERA, FIELD, 2, false, inside)
        .lines.find((line) => line.kind === "limb")
        ?.runs.flat() ?? [];
    const inset = insetFromSpheroidOutline(body);
    const sphere = limbOf({ ...body, spheroid: null }).map(inset);
    const spheroid = limbOf(body).map(inset);
    // The centreline 17.0 px beyond the figure, its casing's outer edge 20.5 (plan 10's record).
    expect([
      Math.round(Math.min(...sphere) * 10) / 10,
      Math.round(Math.min(...spheroid) * 100) / 100,
    ]).toEqual([-17.0, 3.53]);
  });

  it.each([
    ["equator-on", vec3(0, 1, 0)],
    ["pole-on", vec3(0, 0, 1)],
  ] as const)(
    "measures the outline's least apparent diameter seen %s: 2 atan(c ÷ √(d² − a²)) equator-on, 2 atan(a ÷ √(d² − c²)) pole-on",
    (how, pole) => {
      const body = saturnAt(960, 540, pole);
      const a = SATURN.equatorialRadiusM;
      const c = SATURN.polarRadiusM;
      const d = SATURN_RANGE_M;
      // The tangents from the camera to the meridian's ellipse, a across the line of sight and c
      // along it pole-on, the other way about equator-on.
      const want =
        how === "equator-on"
          ? 2 * Math.atan(c / Math.sqrt(d * d - a * a)) * FOCAL_PX
          : 2 * Math.atan(a / Math.sqrt(d * d - c * c)) * FOCAL_PX;
      const seen = outlineLeastDiameterPx(body, FIELD_CAMERA, FIELD);
      expect(Math.abs(seen / want - 1)).toBeLessThan(1e-9);
    },
  );

  it("measures a sphere's outline as its apparent diameter", () => {
    const body = sphereAt(960, 540, GLOW_RANGE_M, TEST_EARTH_RADIUS_M);
    expect(outlineLeastDiameterPx(body, FIELD_CAMERA, FIELD)).toBe(
      angularDiameterPx(body.centreM, body.radiusM, FIELD_CAMERA, FIELD),
    );
  });

  it.each(VIEW_SCALES)(
    "cuts the datum over the image on the outline's least semi-axis seen equator-on, at a line scale of %s",
    (strokeScale) => {
      const inside = reachesAt(strokeScale);
      const leastPx = DATUM_OVER_IMAGE_RADIUS_PER_REACH * inside.reachPx.thin;
      const a = SATURN.equatorialRadiusM;
      const c = SATURN.polarRadiusM;
      // The range at which the polar semi-axis, atan(c ÷ √(d² − a²)) at the centre's pixel scale, is
      // `share` of the least radius.
      const at = (share: number): GraticuleBody => {
        const tan = Math.tan((leastPx * share) / FOCAL_PX);
        const d = Math.sqrt((c / tan) ** 2 + a * a);
        const body = saturnAt(960, 540, vec3(0, 1, 0));
        return { ...body, centreM: vec3(0, 0, -d) };
      };
      const linesAt = (share: number): number =>
        graticule(at(share), FIELD_CAMERA, FIELD, strokeScale, false, inside).lines.length;
      const equatorialPx = (share: number): number =>
        angularDiameterPx(at(share).centreM, a, FIELD_CAMERA, FIELD) / 2;
      expect([linesAt(1 + 1e-9) > 0, linesAt(1 - 1e-9), equatorialPx(1 - 1e-9) > leastPx]).toEqual([
        true,
        0,
        true,
      ]);
    },
  );

  it.each(cases)(
    "draws a sphere's lines as before: a figure as wide at its poles as at its equator is no spheroid, seen %s at a line scale of %s",
    (_, strokeScale, xPx, yPx, pole) => {
      const { centreM, radiusM, rotation, unmodelledPole } = saturnAt(xPx, yPx, pole);
      const sphere: GraticuleBody = { centreM, radiusM, rotation, unmodelledPole, spheroid: null };
      const round = datumSpheroidOfFigure({
        equatorialRadiusM: SATURN.equatorialRadiusM,
        polarRadiusM: SATURN.equatorialRadiusM,
        pole,
      });
      const inside = reachesAt(strokeScale);
      expect(round).toBeNull();
      expect(
        graticule({ ...sphere, spheroid: round }, FIELD_CAMERA, FIELD, strokeScale, false, inside),
      ).toEqual(graticule(sphere, FIELD_CAMERA, FIELD, strokeScale, false, inside));
    },
  );

  it.each(cases)(
    "meets the sphere's lines as the figure rounds: at a stretch of 1 + 10⁻¹² every point within (s − 1) d of the sphere's, in both styles, seen %s at a line scale of %s",
    (_, strokeScale, xPx, yPx, pole) => {
      const sphere: GraticuleBody = { ...saturnAt(xPx, yPx, pole), spheroid: null };
      const nearly: GraticuleBody = { ...sphere, spheroid: { pole, stretch: 1 + 1e-12 } };
      const gaps = [null, reachesAt(strokeScale)].map((inside) => {
        const linesOf = (body: GraticuleBody): ReadonlyArray<GraticuleLine> =>
          graticule(body, FIELD_CAMERA, FIELD, strokeScale, false, inside).lines;
        const want = linesOf(sphere);
        const seen = linesOf(nearly);
        const points = want.flatMap((line, i) =>
          line.runs.flatMap((run, j) =>
            run.map((p, k) => {
              const q = seen[i]?.runs[j]?.[k];
              return q === undefined ? Number.POSITIVE_INFINITY : norm(sub(p, q));
            }),
          ),
        );
        // The stretch moves the centre's image by up to (s − 1) d, half a millimetre here.
        return [shape(seen), shape(want), Math.max(...points) < 1e-12 * SATURN_RANGE_M];
      });
      for (const [seenShape, wantShape, within] of gaps) {
        expect([seenShape, within]).toEqual([wantShape, true]);
      }
    },
  );

  /** An orbit normal well off `TEST SATURN`'s pole, as a kept body's with no rotation may be. */
  const OFF_POLE_NORMAL = normalise(vec3(1, 0.4, 0.7));

  /** The equator's points, the major parallel's, and their heights off a plane through the centre. */
  function equatorOffPlane(body: GraticuleBody, normal: Vec3): number[] {
    const equator = graticule(body, FIELD_CAMERA, FIELD, 2).lines.find(
      (line) => line.kind === "parallel" && line.major,
    );
    return (equator?.runs.flat() ?? []).map((p) => Math.abs(dot(sub(p, body.centreM), normal)));
  }

  it("draws a flattened body with no rotation about the pole its figure is drawn about, its orbit normal off it: its equator in the figure's, to 10⁻⁹ of a", () => {
    const pole = normalise(vec3(0, 1, 0.2));
    const body: GraticuleBody = {
      ...saturnAt(960, 540, pole),
      unmodelledPole: OFF_POLE_NORMAL,
      spheroid: datumSpheroidOfFigure({ ...SATURN, pole }),
    };
    const off = equatorOffPlane(body, pole);
    expect([off.length > 20, Math.max(...off) <= 1e-9 * SATURN.equatorialRadiusM]).toEqual([
      true,
      true,
    ]);
  });

  it("draws a sphere set up the same way about its orbit normal, as before", () => {
    const pole = normalise(vec3(0, 1, 0.2));
    const body: GraticuleBody = {
      ...saturnAt(960, 540, pole),
      unmodelledPole: OFF_POLE_NORMAL,
      spheroid: datumSpheroidOfFigure({ ...SATURN, polarRadiusM: SATURN.equatorialRadiusM, pole }),
    };
    const off = equatorOffPlane(body, OFF_POLE_NORMAL);
    expect([
      body.spheroid,
      off.length > 20,
      Math.max(...off) <= 1e-9 * SATURN.equatorialRadiusM,
    ]).toEqual([null, true, true]);
  });

  it("takes the figure's axis as the image does: its pole, or +z where it has none", () => {
    expect([
      datumSpheroidOfFigure({ ...SATURN, pole: vec3(0, 2, 0) })?.pole,
      datumSpheroidOfFigure({ ...SATURN, pole: null })?.pole,
      datumSpheroidOfFigure(null),
    ]).toEqual([vec3(0, 1, 0), vec3(0, 0, 1), null]);
  });

  it.each(VIEW_SCALES)(
    "draws the datum from over the pole below the equatorial radius, inside the sphere and outside the figure, at a line scale of %s",
    (strokeScale) => {
      // 1,000 km above the north pole, 4,904 km inside the sphere of radius a, looking down at it.
      const pole = vec3(0, 0, 1);
      const body: GraticuleBody = {
        centreM: vec3(0, 0, -(SATURN.polarRadiusM + 1e6)),
        radiusM: SATURN.equatorialRadiusM,
        rotation: null,
        unmodelledPole: pole,
        spheroid: datumSpheroidOf(SATURN, pole),
      };
      const styles = [null, reachesAt(strokeScale)].map((inside) => {
        const wireframe = graticule(body, FIELD_CAMERA, FIELD, strokeScale, false, inside);
        const points = wireframe.lines.flatMap((line) => line.runs.flat());
        return [
          wireframe.regime,
          points.length > 100,
          points.every((p) => Number.isFinite(p.x + p.y + p.z)),
          Math.max(...points.map((p) => offSpheroid(body, p))) <= 1e-9,
          Math.min(...points.map((p) => -dot(normalise(p), normalAt(body, p)))) >= -1e-9,
        ];
      });
      expect(styles).toEqual([
        ["graticule", true, true, true, true],
        ["graticule", true, true, true, true],
      ]);
    },
  );
});

/** A sphere's view as the random sample below draws it: its body, camera, field and line scale. */
interface SphereView {
  readonly body: GraticuleBody;
  readonly camera: ProjectionCamera;
  readonly strokeScale: number;
}

/** A body's rotation looking along `forward`, +z up, as the scratch comparison drew them. */
function lookingRotation(forward: Vec3): GraticuleBody["rotation"] {
  return rotation3FromRows(rotationRows(lookAlong(normalise(forward), vec3(0, 0, 1))));
}

/**
 * A sphere of `radiusM` at `rangeM` along `direction`, its graticule turned to look along
 * `forward`, on {@link FIELD} at `fovDeg` across: a pose of the scratch comparison's 20,000.
 */
function sphereView(
  radiusM: number,
  rangeM: number,
  direction: Vec3,
  forward: Vec3,
  fovDeg: number,
  strokeScale: number,
): SphereView {
  return {
    body: {
      centreM: scale(normalise(direction), rangeM),
      radiusM,
      rotation: lookingRotation(forward),
      unmodelledPole: null,
      spheroid: null,
    },
    camera: { orientation: IDENTITY_QUATERNION, fovXRad: (fovDeg * Math.PI) / 180 },
    strokeScale,
  };
}

/** A sphere whose 30° parallel's facing arc straddles λ = 0, its safe arc wholly on one side. */
const SEAM_CASE = sphereView(
  21_697_264.450974762,
  410_392_519.836727,
  vec3(0.0873615180165723, 0.1594170580265957, -0.9833382768813514),
  vec3(0.1592391536119229, 0.614269140478741, -0.7728624166127287),
  66.14651962183416,
  3,
);

/** A sphere whose meridian at 150° runs its middle within its reach of the limb. */
const MERIDIAN_CASE = sphereView(
  6_804_794.964753091,
  215_361_309.96516234,
  vec3(0.15591618458855092, 0.2957451683595898, -0.9424568630846226),
  vec3(0.7302866178783345, 0.4292454950964605, 0.5314412109417351),
  74.0277921129018,
  2,
);

/**
 * The scratch comparison's poses whose datum over the image the middle-only rule left short, of its
 * 114 (those with a rotation, no orbit normal and the high setting): parallels split at λ = 0 and
 * meridians whose middle runs near the limb. {@link SEAM_CASE} and {@link MERIDIAN_CASE} are among
 * them.
 */
const SHORT_POSES: ReadonlyArray<SphereView> = [
  sphereView(
    21697264.450974762,
    410392519.836727,
    vec3(0.0873615180165723, 0.1594170580265957, -0.9833382768813514),
    vec3(0.1592391536119229, 0.614269140478741, -0.7728624166127287),
    66.14651962183416,
    3,
  ),
  sphereView(
    63923306.2508516,
    2022524044.417224,
    vec3(0.3455955872438431, 0.10642606529063317, -0.9323289026434487),
    vec3(-0.5083954500311934, -0.8505658311968924, 0.13443151858072466),
    34.93845705408603,
    3,
  ),
  sphereView(
    36101533.28618035,
    1834567479.0548995,
    vec3(-0.05419592928226836, -0.2696936726873286, -0.9614198480173225),
    vec3(-0.34178822266918535, 0.6482640134644536, -0.6803929597604594),
    70.49024692270905,
    2,
  ),
  sphereView(
    37132529.02254462,
    1500372020.942132,
    vec3(0.5664211097412477, 0.16436768565571386, -0.8075582891356377),
    vec3(0.47040032755617905, -0.8135509996032524, 0.34184543711974325),
    26.95413388311863,
    3,
  ),
  sphereView(
    6804794.964753091,
    215361309.96516234,
    vec3(0.15591618458855092, 0.2957451683595898, -0.9424568630846226),
    vec3(0.7302866178783345, 0.4292454950964605, 0.5314412109417351),
    74.0277921129018,
    2,
  ),
  sphereView(
    39238676.66851729,
    556390834.7703439,
    vec3(-0.03247075277254835, -0.5529937280856517, -0.832552452949552),
    vec3(0.32403927982490094, 0.36235557623775505, -0.8738975806694765),
    63.05120245553553,
    2,
  ),
  sphereView(
    56936195.583455265,
    1676633165.6748164,
    vec3(-0.3453789103986816, -0.3287304007525952, -0.8790049669216082),
    vec3(0.4745400222685626, 0.27143987596973557, -0.8373363487863684),
    90.42921469546854,
    2,
  ),
  sphereView(
    65379846.554622054,
    1227687020.2590027,
    vec3(0.12808842446438476, 0.4665445488376551, -0.8751740052515845),
    vec3(0.35792755007062654, -0.640046314547323, -0.6798739472393602),
    74.62291045114398,
    2,
  ),
  sphereView(
    26072860.311158,
    299347108.6981777,
    vec3(-0.4398736750303532, 0.7462211284427513, -0.4996650652996648),
    vec3(-0.15022517689804932, 0.4847921087916533, -0.8616315961472695),
    27.62480548582971,
    2,
  ),
  sphereView(
    15392986.475396901,
    440919838.29349756,
    vec3(0.7379197695488376, -0.046043685742762984, -0.673315967961708),
    vec3(0.34324012387740716, -0.8166749651150391, -0.4639269540724827),
    85.99348864983767,
    2,
  ),
  sphereView(
    39777560.21535024,
    1731399331.428559,
    vec3(-0.4881048363902541, 0.5494324511215839, -0.6781427949532247),
    vec3(0.2137106792729321, 0.47080207094747933, 0.8559632910097644),
    52.27254601661116,
    3,
  ),
  sphereView(
    67347559.35370922,
    4015072315.1497664,
    vec3(-0.4269585570156825, -0.02938865794954042, -0.9037935037247232),
    vec3(0.16316627427713848, 0.7518563776309362, -0.6388182482946182),
    63.27884782105684,
    2,
  ),
  sphereView(
    58920177.66358331,
    984893396.0803636,
    vec3(0.5816775584092049, -0.379820580359328, -0.7192965624682277),
    vec3(0.08156942917878035, -0.7895301603501469, 0.6082668445024102),
    24.57411732058972,
    3,
  ),
];

/** A sphere's lines over the image on its field. */
function insideLines(view: SphereView): ReadonlyArray<GraticuleLine> {
  return graticule(
    view.body,
    view.camera,
    FIELD,
    view.strokeScale,
    false,
    reachesAt(view.strokeScale),
  ).lines;
}

/** The margins past their reach of a line's run ends cut short of the limb, px. */
function cutEndMargins(view: SphereView, line: GraticuleLine): number[] {
  const inset = insetFromOutline(view.body, { camera: view.camera, viewport: FIELD });
  const inside = reachesAt(view.strokeScale);
  const reach = line.major ? inside.reachPx.heavy : inside.reachPx.thin;
  const pole = rotateToBody(view.body.rotation ?? graticuleRotation(view.body), vec3(0, 0, 1));
  const ends = line.runs.flatMap((run) => [run[0], run.at(-1)]).filter((p) => p !== undefined);
  return ends
    .filter((end) => {
      const n = normalise(sub(end, view.body.centreM));
      const atPole = Math.abs(Math.abs(dot(n, pole)) - 1) < 1e-9;
      const shared = ends.filter((o) => norm(sub(o, end)) < 1e-6 * view.body.radiusM).length > 1;
      return !atPole && !shared;
    })
    .map((end) => inset(end) - reach);
}

/**
 * The points of a sphere's meridians and parallels over the image that the datum should draw but
 * does not: each line's facing arcs sampled at `samples` points each, every point on the view at
 * least its reach + 1/32 + `epsPx` px inside the exact outline that lies farther than `drawnPx`
 * from every drawn run of its line, on screen. A lower bound on the inset, f (α − β) (the
 * gnomonic scale is at least f, and the outline's lines of sight lie α − β off the point's),
 * settles most points; the rest are measured on the exact outline.
 */
function undrawnPoints(
  view: SphereView,
  samples: number,
  epsPx: number,
): { readonly checked: number; readonly undrawn: number } {
  const { body, camera, strokeScale } = view;
  const field: Field = { camera, viewport: FIELD };
  const inside = reachesAt(strokeScale);
  const wireframe = graticule(body, camera, FIELD, strokeScale, false, inside);
  if (wireframe.lines.length === 0) {
    return { checked: 0, undrawn: 0 };
  }
  const exactInset = insetFromOutline(body, field);
  const d = norm(body.centreM);
  const alpha = Math.asin(body.radiusM / d);
  const toCentre = scale(body.centreM, 1 / d);
  const focalPx = FIELD.widthPx / (2 * Math.tan(camera.fovXRad / 2));
  const rotation = graticuleRotation(body);
  const axes = [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)].map((axis) =>
    rotateToBody(rotation, axis),
  );
  const [x, y, z] = [axes[0] ?? toCentre, axes[1] ?? toCentre, axes[2] ?? toCentre];
  const [fx, fy, fz] = [x, y, z].map((axis) => -dot(axis, body.centreM));
  const stepRad = (graticuleStepDeg(wireframe.diameterPx, strokeScale, false) * Math.PI) / 180;
  const meridianLines = wireframe.lines.filter((line) => line.kind === "meridian");
  const parallelLines = wireframe.lines.filter((line) => line.kind === "parallel");
  // Each line: its point at a parameter, its facing coefficients and range, and its drawn runs.
  const unitOf = (lat: number, lon: number): Vec3 =>
    add(
      add(scale(x, Math.cos(lat) * Math.cos(lon)), scale(y, Math.cos(lat) * Math.sin(lon))),
      scale(z, Math.sin(lat)),
    );
  const specs = [
    ...meridianLines.map((line, k) => {
      const lon = k * stepRad;
      return {
        line,
        point: (lat: number): Vec3 => unitOf(lat, lon),
        windows: cosineWindows(
          Math.cos(lon) * (fx ?? 0) + Math.sin(lon) * (fy ?? 0),
          fz ?? 0,
          body.radiusM,
          -Math.PI / 2,
          Math.PI / 2,
        ),
      };
    }),
    ...parallelLines.map((line, k) => {
      const lat = -Math.PI / 2 + (k + 1) * stepRad;
      return {
        line,
        point: (lon: number): Vec3 => unitOf(lat, lon),
        windows: cosineWindows(
          Math.cos(lat) * (fx ?? 0),
          Math.cos(lat) * (fy ?? 0),
          body.radiusM - Math.sin(lat) * (fz ?? 0),
          0,
          2 * Math.PI,
        ),
      };
    }),
  ];
  let checked = 0;
  let undrawn = 0;
  for (const { line, point, windows } of specs) {
    const reach = line.major ? inside.reachPx.heavy : inside.reachPx.thin;
    const threshold = reach + 1 / 32 + epsPx;
    const drawn = line.runs.map((run) => run.map((p) => project(p, camera, FIELD)));
    const drawnNear = (q: { xPx: number; yPx: number }): boolean =>
      drawn.some((run) =>
        run.some((a, i) => {
          const b = run[i + 1];
          return b !== undefined && toSegmentPx(q.xPx, q.yPx, a, b) <= 0.5;
        }),
      );
    for (const [lo, hi] of windows) {
      for (let i = 1; i < samples; i += 1) {
        const p = add(body.centreM, scale(point(lo + ((hi - lo) * i) / samples), body.radiusM));
        const q = project(p, camera, FIELD);
        const onView =
          q.inFront &&
          q.xPx >= 0 &&
          q.xPx <= FIELD.widthPx &&
          q.yPx >= 0 &&
          q.yPx <= FIELD.heightPx;
        if (!onView) {
          continue;
        }
        const beta = Math.acos(Math.min(1, dot(normalise(p), toCentre)));
        const lowerPx = focalPx * (alpha - beta);
        if (lowerPx < threshold && exactInset(p) < threshold) {
          continue;
        }
        checked += 1;
        undrawn += drawnNear(q) ? 0 : 1;
      }
    }
  }
  return { checked, undrawn };
}

/**
 * The fixed sample of spheres the completeness check draws: `count` poses from a seeded linear
 * congruential sequence, as the lane's scratch comparison drew its 20,000, over the image at line
 * scales of 2 and 3 in fields of 20° to 120°.
 */
function sampledSphereViews(seed: number, count: number): SphereView[] {
  let state = seed >>> 0;
  const next = (): number => {
    state = (state * 1_664_525 + 1_013_904_223) >>> 0;
    return state / 2 ** 32;
  };
  return Array.from({ length: count }, () => {
    const radiusM = 1e5 + next() * 7e7;
    const rangeM = radiusM * (1.0001 + next() * 400);
    const direction = vec3(next() - 0.5, next() - 0.5, -0.2 - next());
    const forward = vec3(next() - 0.5, next() - 0.5, next() - 0.5);
    const fovDeg = 20 + next() * 100;
    const strokeScale = next() < 0.5 ? 2 : 3;
    return sphereView(radiusM, rangeM, direction, forward, fovDeg, strokeScale);
  });
}

describe("a window with no safe cut keeps its far stretch, on every body (R10.T16.g; R02.T19's finding)", () => {
  it("runs a parallel whose facing arc straddles λ = 0 across it, each cut end at its reach + 1/32 px inside the outline", () => {
    const parallels = insideLines(SEAM_CASE).filter((line) => line.kind === "parallel");
    // The 30° parallel: 12 meridians and 5 parallels at the 30° step, the fourth from the south.
    const seam = parallels[3];
    const ends = seam === undefined ? [] : cutEndMargins(SEAM_CASE, seam);
    // Its two runs meet at λ = 0, where each piece of its facing arc ends.
    const atSeam = (seam?.runs ?? [])
      .flatMap((run) => [run[0], run.at(-1)])
      .filter(
        (p, _, all) =>
          p !== undefined &&
          all.filter((o) => o !== undefined && norm(sub(o, p)) < 1e-3).length > 1,
      );
    expect({
      runs: seam?.runs.length,
      meetAtTheSeam: atSeam.length,
      cutEnds: ends.length,
      atTheReach: ends.every((margin) => margin >= 0 && margin <= 0.05),
    }).toEqual({ runs: 2, meetAtTheSeam: 2, cutEnds: 2, atTheReach: true });
  });

  it("keeps the far stretch of a meridian whose middle runs within its reach of the limb, its cut end at the reach", () => {
    const lines = insideLines(MERIDIAN_CASE).filter((line) => line.kind === "meridian");
    // The meridian at 150°: 12 meridians at the 30° step.
    const near = lines[5];
    const ends = near === undefined ? [] : cutEndMargins(MERIDIAN_CASE, near);
    expect({
      points: (near?.runs.flat().length ?? 0) > 2,
      cutEnds: ends.length > 0,
      atTheReach: ends.every((margin) => margin >= 0 && margin <= 0.05),
    }).toEqual({ points: true, cutEnds: true, atTheReach: true });
  });

  it.each([0, 1, 2, 3])(
    "draws every point of a meridian or parallel standing its reach + 1/32 + 0.02 px inside the outline, over a fixed sample of spheres (chunk %s of 4)",
    (chunk) => {
      const views = [...SHORT_POSES, ...sampledSphereViews(20_261_027, 200)];
      const mine = views.filter((_, i) => i % 4 === chunk);
      const counts = mine.map((view) => undrawnPoints(view, 1_024, 0.02));
      const checked = counts.reduce((sum, count) => sum + count.checked, 0);
      const undrawn = counts.reduce((sum, count) => sum + count.undrawn, 0);
      expect([checked > 10_000, undrawn]).toEqual([true, 0]);
    },
  );
});
