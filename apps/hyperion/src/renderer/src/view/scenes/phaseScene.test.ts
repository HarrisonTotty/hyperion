import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import { compositeDiscPixels, rasteriseDisc, viewRay } from "../bodies/discShading";
import { planLitBodies } from "../bodies/draw";
import { lookAlong } from "../camera/quaternion";
import { type ProjectionCamera, toViewAxes, type Viewport } from "../camera/projection";
import { hostLights } from "../lighting/hostLights";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import {
  PHASE_CAMERA_M,
  PHASE_PLANETS,
  PHASE_PLANET_RADIUS_M,
  PHASE_SCENE_PHASES_DEG,
  PHASE_STAR,
  phaseScene,
} from "./phaseScene";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const ROW_PX = VIEWPORT.heightPx / 2;

const scene = phaseScene().sceneAt(0);

/** A body's centre from the camera, m. */
function fromCamera(id: string): Vec3 {
  const body = scene.bodies.find((b) => b.id === id);
  if (body === undefined) {
    throw new Error(`no body ${id}`);
  }
  return sub(body.centreM, PHASE_CAMERA_M);
}

/** A camera on the planet with the star's side of it to the right of the view. */
function cameraOn(planet: Vec3, star: Vec3): ProjectionCamera {
  const forward = normalise(planet);
  const towardsStar = normalise(sub(star, planet));
  const across = sub(towardsStar, scale(forward, dot(towardsStar, forward)));
  const right = norm(across) > 1e-6 ? normalise(across) : vec3(1, 0, 0);
  return { orientation: lookAlong(forward, cross(right, forward)), fovXRad: Math.PI / 3 };
}

/** The first x along the row, px, where `inside` turns from false to true, by bisection. */
function crossing(from: number, to: number, inside: (x: number) => boolean): number {
  let a = from;
  let b = to;
  for (let i = 0; i < 60; i += 1) {
    const m = (a + b) / 2;
    if (inside(m) === inside(a)) {
      a = m;
    } else {
      b = m;
    }
  }
  return (a + b) / 2;
}

/** The planet at `index` drawn, its centre row, and the oracle's limb crossings. */
function drawPlanet(index: number) {
  const id = PHASE_PLANETS[index] ?? PHASE_PLANETS[0];
  const planet = fromCamera(id);
  const star = fromCamera(PHASE_STAR);
  const camera = cameraOn(planet, star);
  const disc = scene.hostDiscs?.[0];
  if (disc === undefined) {
    throw new Error("the scene has no host disc");
  }
  const figure = {
    equatorialRadiusM: PHASE_PLANET_RADIUS_M,
    polarRadiusM: PHASE_PLANET_RADIUS_M,
    pole: null,
  };
  const plan = planLitBodies(
    [{ id, centreM: planet, figure, photometry: PROVISIONAL_PHOTOMETRY, lighting: undefined }],
    [{ disc, centreM: star }],
    {
      camera,
      viewport: VIEWPORT,
      exposureScale: 1e-4,
      annuli: 4,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high" as const,
    },
    new Map([[id, "disc"]]),
  );
  const record = plan.discs[0];
  if (record === undefined) {
    throw new Error("the planet is not a disc");
  }
  const row = compositeDiscPixels(rasteriseDisc(record, camera, VIEWPORT))
    .filter((p) => p.yPx === ROW_PX)
    .toSorted((a, b) => a.xPx - b.xPx);

  // The oracle, in f64 along the row's centre line: where its rays meet the sphere, and where the
  // light ends, at φ = 90° + the star's angular radius (the horizon term's last light).
  const hitAt = (x: number): Vec3 | null => {
    const ray = viewRay(x, ROW_PX + 0.5, camera, VIEWPORT);
    const toCentre = toViewAxes(planet, camera.orientation);
    const b = dot(ray, toCentre);
    const disc2 = b * b - (dot(toCentre, toCentre) - PHASE_PLANET_RADIUS_M ** 2);
    if (disc2 < 0) {
      return null;
    }
    return sub(scale(ray, b - Math.sqrt(disc2)), toCentre);
  };
  const centreX = VIEWPORT.widthPx / 2;
  const leftLimb = crossing(0, centreX, (x) => hitAt(x) !== null);
  const rightLimb = crossing(centreX, VIEWPORT.widthPx, (x) => hitAt(x) === null);
  const first = row[0];
  const last = row.at(-1);
  if (first === undefined || last === undefined) {
    throw new Error("the row is empty");
  }
  return { row, first, last, leftLimb, rightLimb, hitAt, camera, planet, star, disc };
}

/** The measured and the oracle's terminator of the planet at `index`, px along the row. */
function terminatorOf(index: number): { readonly measured: number; readonly oracle: number } {
  const { row, leftLimb, rightLimb, hitAt, camera, planet, star } = drawPlanet(index);
  const starView = toViewAxes(star, camera.orientation);
  const planetView = toViewAxes(planet, camera.orientation);
  const lit = (x: number): boolean => {
    const q = hitAt(x);
    if (q === null) {
      return false;
    }
    const towards = normalise(sub(starView, add(planetView, q)));
    // The geometric terminator, μ₀ = 0: where the whole star is up the horizon term is μ₀,
    // whose zero the lit pixels extrapolate to (the soft band past it, 0.25 px here, is not).
    return dot(normalise(q), towards) > 0;
  };
  const oracle = crossing(leftLimb + 1e-9, rightLimb - 1e-9, lit);
  // The light's profile across the terminator, Lambert's μ₀ on the centre line, is linear in x at
  // 90° and nearly so at 150° (0.1 px of curvature); its zero, extrapolated from the first two
  // pixel centres lit past the soft band, is the measured terminator.
  const litPixels = row.filter((p) => p.rgb[1] > 0 && p.coverage === 1);
  // The first lit pixel may hold the soft band; the next two are past it.
  const [, p1, p2] = litPixels;
  if (p1 === undefined || p2 === undefined) {
    throw new Error("too few lit pixels");
  }
  const x1 = p1.xPx + 0.5;
  const x2 = p2.xPx + 0.5;
  const measured = x1 - (p1.rgb[1] * (x2 - x1)) / (p2.rgb[1] - p1.rgb[1]);
  return { measured, oracle };
}

describe("the phase scene", () => {
  it("lights its planets by its own Sun", () => {
    const lights = hostLights(scene, scene.hostDiscs ?? []);
    expect(lights.map((light) => light.body)).toEqual([PHASE_STAR]);
  });

  it("holds one host disc through every frame, as a server scene's sky holds its hosts", () => {
    expect(phaseScene().sceneAt(5).hostDiscs).toBe(scene.hostDiscs);
  });

  for (const [index, phaseDeg] of PHASE_SCENE_PHASES_DEG.entries()) {
    it(`draws the ${String(phaseDeg)}° planet's limbs where the oracle puts them, to half a pixel`, () => {
      const { first, last, leftLimb, rightLimb } = drawPlanet(index);
      expect(Math.abs(first.xPx + 1 - first.coverage - leftLimb)).toBeLessThan(0.5);
      expect(Math.abs(last.xPx + last.coverage - rightLimb)).toBeLessThan(0.5);
    });
  }

  it("lights the full planet across its whole disc", () => {
    expect(drawPlanet(0).row.every((p) => p.rgb[1] > 0)).toBe(true);
  });

  for (const index of [1, 2]) {
    it(`draws the ${String(PHASE_SCENE_PHASES_DEG[index])}° planet's terminator where the oracle puts it, to half a pixel`, () => {
      const { measured, oracle } = terminatorOf(index);
      expect(Math.abs(measured - oracle)).toBeLessThan(0.5);
    });
  }
});
