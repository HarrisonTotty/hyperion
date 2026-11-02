import { describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import { IDENTITY_QUATERNION, quaternion } from "../camera/quaternion";
import type { Rgb } from "../photometry/toneCurve";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES, referenceSunIlluminanceLx } from "./earth";
import {
  type AtmosphereCamera,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
} from "./hillaire";
import {
  aerialPerspectiveTwin,
  marchTwin,
  type PlanetTwin,
  placedSteps,
  rayMarchRayAlongTwin,
  rayMarchTexelsTwin,
  tableSources,
  transmittanceTablesTwin,
  type TwinMarchedTexel,
  twinView,
  type TwinView,
} from "./tablesCpu";

/**
 * A drawn surface's transmittance per channel (plan R08, R08.T9.c): the aerial-perspective volume's
 * and the ray march's second textures, which the composite dims a surface seen through the air by
 * in place of the channels' mean, held to a converged march of the same ray within 1% of each
 * channel's optical depth, relative where the path is opaque (`compositeTransmittance.test.ts`'s
 * measure): 1% of T itself where τ ≤ 1. The measured Earth (`EARTH_REFERENCE`, its ozone under the
 * reference Sun's curve) on a sphere of WGS 84's R₁, at the high setting's counts: from 2 m over
 * 5 km of air, and from 400 km through the limb to a surface at the tangent point, or on the ground
 * short of it.
 */

const CHANNELS = [0, 1, 2] as const;

/** The sphere the tables are built on, m: WGS 84's R₁. */
const RADIUS_M = tableRadiusM({ equatorialRadiusM: 6_378_137, polarRadiusM: 6_356_752.314_245 });

const SPHERE: SpheroidFigure = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M };

/** The converged march's steps, which each twin's transmittance is held against. */
const CONVERGED_STEPS = 4_096;

/**
 * The most |ln T − ln T_ref| ÷ max(1, −ln T_ref) may reach in a channel: 1%, of T itself where the
 * path is thin.
 */
const TOLERANCE = 0.01;

/** The least converged transmittance a channel is compared at: below it the surface is gone. */
const TRANSMITTANCE_FLOOR = 1e-6;

/** The planet's tables, its multiple scattering unread: a transmittance takes none of it. */
const PLANET: PlanetTwin = {
  medium: EARTH_REFERENCE,
  bottomRadiusM: RADIUS_M,
  ...transmittanceTablesTwin(EARTH_REFERENCE, RADIUS_M, EARTH_REFERENCE_CURVES),
  multiScattering: { widthTexels: 2, heightTexels: 2, texels: new Float64Array(16) },
};

const SOURCES = tableSources(PLANET);

/** The sun, high over the camera's meridian: a transmittance does not depend on it. */
const SUN: SunState = {
  directionBodyFixed: { x: Math.SQRT1_2, y: 0, z: Math.SQRT1_2 },
  illuminanceLx: referenceSunIlluminanceLx(),
  angularRadiusRad: 0.004_650_5,
};

/** The converged march's transmittance per channel from the view's camera along `direction`. */
function convergedTransmittance(view: TwinView, direction: Vec3, surfaceM: number): Rgb {
  const clipped = rayMarchRayAlongTwin(EARTH_REFERENCE, view, view.cameraM, direction, surfaceM);
  if (clipped === null) {
    throw new Error("the ray meets the air");
  }
  return marchTwin(
    EARTH_REFERENCE,
    SOURCES,
    { kind: "spheroid", figure: SPHERE },
    clipped.ray,
    placedSteps(clipped.ray, CONVERGED_STEPS),
  ).transmittance;
}

/**
 * The worst |ln T − ln T_ref| ÷ max(1, −ln T_ref) over the channels whose converged transmittance
 * is at or above the floor: the optical depth's error, relative where the path is opaque.
 */
function worstDepthError(t: ArrayLike<number>, converged: Rgb): number {
  let worst = 0;
  for (const c of CHANNELS) {
    if (converged[c] >= TRANSMITTANCE_FLOOR) {
      const depth = -Math.log(converged[c]);
      worst = Math.max(worst, Math.abs(Math.log(t[c] ?? Number.NaN) + depth) / Math.max(1, depth));
    }
  }
  return worst;
}

/** The worst |mean ÷ T_c − 1| over the channels: how far the grey mean stands from them. */
function meanApart(mean: number, t: Rgb): number {
  return Math.max(...CHANNELS.map((c) => Math.abs(mean / t[c] - 1)));
}

/** The surface 5 km ahead of 2 m: its transmittance in the volume's twin and the converged march's. */
interface NearSurface {
  /** The second volume's froxel at the surface: each channel's transmittance and their mean. */
  readonly volume: Float64Array;
  /** The first volume's alpha there, the mean. */
  readonly alpha: number;
  readonly converged: Rgb;
}

/**
 * Looking along the horizon from 2 m over the equator, north, through a field so narrow that the
 * volume's froxel columns all take the screen's centre ray: the high setting's volume, its far faces
 * 1 km apart, read at the fifth, the surface's, beside the converged march to it.
 */
function nearSurface(): NearSurface {
  const sizes = TABLE_SIZES.high;
  const surfaceM = 5_000;
  const camera: AtmosphereCamera = {
    positionM: { x: RADIUS_M + 2, y: 0, z: 0 },
    orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
    fovXRad: 1e-6,
    viewport: { widthPx: 2, heightPx: 2 },
  };
  const view = twinView(camera, SUN, SPHERE, 0.1);
  const size = { widthTexels: 2, heightTexels: 2, slices: sizes.aerialPerspective.slices };
  const reachM = sizes.aerialPerspectiveReachM;
  const volume = aerialPerspectiveTwin(
    PLANET,
    view,
    size,
    sizes.aerialPerspectiveSamplesPerSlice,
    reachM,
  );
  const slice = Math.round((surfaceM / reachM) * size.slices) - 1;
  // Froxel (0, 0) of the slice.
  const at = slice * 4 * 4;
  return {
    volume: volume.transmittance.subarray(at, at + 4),
    alpha: volume.texels[at + 3] ?? Number.NaN,
    converged: convergedTransmittance(view, view.forward, surfaceM),
  };
}

/** The limb's surface from 400 km: the march's twin texel and the converged march to it. */
interface LimbSurface {
  readonly texel: TwinMarchedTexel;
  readonly converged: Rgb;
}

/**
 * From 400 km over the equator, the ray whose tangent point stands `tangentM` up, to a surface at
 * the tangent point, or to the ground where the ray meets it first: the high setting's march.
 */
function limbSurface(tangentM: number): LimbSurface {
  const distanceM = RADIUS_M + 400_000;
  const camera: AtmosphereCamera = {
    positionM: { x: distanceM, y: 0, z: 0 },
    orientation: IDENTITY_QUATERNION,
    fovXRad: Math.PI / 2,
    viewport: { widthPx: 64, heightPx: 32 },
  };
  const view = twinView(camera, SUN, SPHERE, 0.1);
  const sin = (RADIUS_M + tangentM) / distanceM;
  const direction = { x: -Math.sqrt(1 - sin * sin), y: sin, z: 0 };
  const surfaceM = distanceM * Math.sqrt(1 - sin * sin);
  const marched = rayMarchRayAlongTwin(EARTH_REFERENCE, view, view.cameraM, direction, surfaceM);
  return {
    texel: rayMarchTexelsTwin(PLANET, view, TABLE_SIZES.high, marched),
    converged: convergedTransmittance(view, direction, surfaceM),
  };
}

/** The limb's surfaces: on the ground short of it, and at its tangent point 0.3, 1 and 3 H up. */
const LIMBS = [
  ["on the ground short of the limb", -2_000],
  ["at the tangent point 0.3 H up", 2_400],
  ["at the tangent point 1 H up", 8_000],
  ["at the tangent point 3 H up", 24_000],
] as const;

describe("a drawn surface's transmittance per channel over 5 km from 2 m (R08.T9.c)", () => {
  const surface = nearSurface();

  it("holds the volume's within 1% of a 4,096-step march", () => {
    expect(worstDepthError(surface.volume, surface.converged)).toBeLessThan(TOLERANCE);
  });

  it("holds it to 1% of T itself, every channel there thin", () => {
    expect(Math.max(...surface.converged.map((v) => -Math.log(v)))).toBeLessThan(1);
  });

  it("keeps the mean, the first volume's alpha, in the second's", () => {
    expect(surface.volume[3]).toBe(surface.alpha);
  });

  it("passes plan R08's 0.691, 0.617 and 0.487 in red, green and blue", () => {
    expect(surface.converged.map((v) => Number(v.toFixed(3)))).toEqual([0.691, 0.617, 0.487]);
  });

  it("leaves a surface 13% too dim in red and 23% too bright in blue under the mean", () => {
    const [red, green, blue] = surface.converged;
    const mean = (red + green + blue) / 3;
    expect([mean / red - 1, mean / blue - 1].map((v) => Number(v.toFixed(2)))).toEqual([
      -0.13, 0.23,
    ]);
  });
});

describe("a drawn surface's transmittance per channel from 400 km through the limb (R08.T9.c)", () => {
  it.each(LIMBS)("holds the march's within 1% of a 4,096-step march, %s", (_name, tangentM) => {
    const { texel, converged } = limbSurface(tangentM);
    expect(worstDepthError(texel.transmittance, converged)).toBeLessThan(TOLERANCE);
  });

  it.each(LIMBS)(
    "keeps the mean, the first target's alpha, in the second's, %s",
    (_n, tangentM) => {
      const { texel } = limbSurface(tangentM);
      expect(texel.transmittance[3]).toBe(texel.light[3]);
    },
  );

  it.each(LIMBS)("stands the mean 20% or more from some channel's own, %s", (_n, tangentM) => {
    const { texel, converged } = limbSurface(tangentM);
    expect(meanApart(texel.light[3], converged)).toBeGreaterThan(0.2);
  });
});
