import { describe, expect, it } from "vitest";

import { add, dot, norm, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION, lookAlong } from "../camera/quaternion";
import { rotateToBody, rotation3FromRows } from "../coords/rotation";
import {
  bodyRegime,
  type GraticuleBody,
  graticule,
  graticuleStepDeg,
  isSymbolSized,
  perpendicularPair,
  ringEllipse,
  SYMBOL_BELOW_PX,
} from "./bodies";
import type { Polyline } from "./curve";

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
