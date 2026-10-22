import { beforeAll, describe, expect, it } from "vitest";

import { add, scale, type Vec3 } from "../../geometry/vec3";
import {
  gravityOfSpheroid,
  saturnBody,
  saturnClassMedium,
  saturnScaleHeightM,
} from "../../test/atmosphereFixtures";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { Rgb } from "../photometry/toneCurve";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  RAYLEIGH_SCALE_HEIGHT_M,
  referenceSunIlluminanceLx,
} from "./earth";
import {
  type AtmosphereCamera,
  COMPOSITE_MATERIAL,
  type SpheroidFigure,
  type SunState,
  tableRadiusM,
} from "./hillaire";
import type { LevelSpheroid } from "./oblate";
import {
  compositeTransmittanceTwin,
  marchTwin,
  MULTI_SCATTERING_KERNEL_OPTIONS,
  OBLATE_KERNEL_SIZES,
  oblateCompositeTransmittanceTwin,
  type OblatePlanetTwin,
  oblatePlanetTwin,
  oblateRayMarchRayAlongTwin,
  type OblateTwinSizes,
  type PlanetTwin,
  placedSteps,
  rayMarchRayAlongTwin,
  spheroidOpticalDepth,
  tableSources,
  type TwinSources,
  type TwinView,
  transmittanceTablesTwin,
  twinView,
} from "./tablesCpu";

/**
 * The composite's transmittance to space from above the top (plan R08, R08.T8.b): what lies behind
 * a sky pixel, and each source's disc, through the air per channel from the ray's entry into it
 * (`composite.wgsl`'s `transmittanceFromAbove`), where until R08.T8.b it took the march's mean over
 * the channels, which drew a sun seen through the limb grey and stepped its colour at the top.
 * The drawn Earth (`EARTH_REFERENCE`, its ozone under the reference Sun's curve) on a sphere of
 * WGS 84's mean radius; the oblate twin on that sphere as a body of one band, and on Saturn's
 * figure of four bands under the Saturn-class air.
 */

const CHANNELS = [0, 1, 2] as const;

/** The sphere the tables are built on, m: WGS 84's R₁. */
const RADIUS_M = tableRadiusM({ equatorialRadiusM: 6_378_137, polarRadiusM: 6_356_752.314_245 });

const SPHERE: SpheroidFigure = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M };

const TOP_M = EARTH_REFERENCE.topHeightM;

/** The limb set of Design note 10: tangent heights in Rayleigh scale heights of R05's Earth. */
const LIMB_SCALE_HEIGHTS = [0.1, 0.3, 1, 3, 10] as const;

/** The converged march's steps, which the transmittance along the chord is held against. */
const CONVERGED_STEPS = 4_096;

/** The least transmittance a channel is compared at: below it the sun is gone through the limb. */
const TRANSMITTANCE_FLOOR = 1e-6;

/** The planet's tables as the composite reads them, its multiple scattering unread here. */
const PLANET: PlanetTwin = {
  medium: EARTH_REFERENCE,
  bottomRadiusM: RADIUS_M,
  ...transmittanceTablesTwin(EARTH_REFERENCE, RADIUS_M, EARTH_REFERENCE_CURVES),
  multiScattering: { widthTexels: 2, heightTexels: 2, texels: new Float64Array(16) },
};

const SOURCES = tableSources(PLANET);

/** A view from `heightM` above the sphere on +x; the rays are given, so its orientation is any. */
function viewFrom(heightM: number): TwinView {
  const camera: AtmosphereCamera = {
    positionM: { x: RADIUS_M + heightM, y: 0, z: 0 },
    orientation: IDENTITY_QUATERNION,
    fovXRad: Math.PI / 2,
    viewport: { widthPx: 64, heightPx: 32 },
  };
  const sun: SunState = {
    directionBodyFixed: { x: 0, y: 1, z: 0 },
    illuminanceLx: referenceSunIlluminanceLx(),
    angularRadiusRad: 0,
  };
  return twinView(camera, sun, SPHERE, 0.1);
}

/** The unit ray from `distanceM` out on +x whose tangent point stands `tangentM` up. */
function limbRay(distanceM: number, tangentM: number): Vec3 {
  const sin = (RADIUS_M + tangentM) / distanceM;
  return { x: -Math.sqrt(1 - sin * sin), y: sin, z: 0 };
}

/** The converged march's transmittance along the ray as the ray march clips it, per channel. */
function marchedTransmittance(view: TwinView, direction: Vec3): Rgb {
  const clipped = rayMarchRayAlongTwin(
    EARTH_REFERENCE,
    view,
    view.cameraM,
    direction,
    Number.POSITIVE_INFINITY,
  );
  if (clipped === null) {
    return [1, 1, 1];
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
 * The worst |ln T − ln T_ref| ÷ max(1, −ln T_ref) over the channels whose reference is at or above
 * the floor: the optical depth's error, relative where the chord is opaque.
 */
function worstDepthError(t: Rgb, reference: Rgb): number {
  let worst = 0;
  for (const c of CHANNELS) {
    if (reference[c] >= TRANSMITTANCE_FLOOR) {
      const depth = -Math.log(reference[c]);
      worst = Math.max(worst, Math.abs(Math.log(t[c]) + depth) / Math.max(1, depth));
    }
  }
  return worst;
}

describe("the composite's transmittance to space from above the top (R08.T8.b)", () => {
  const view = viewFrom(400_000);
  const distanceM = RADIUS_M + 400_000;

  it("dims the sun through the limb channel by channel, as the march finds it", () => {
    const errors = LIMB_SCALE_HEIGHTS.map((n) => {
      const d = limbRay(distanceM, n * RAYLEIGH_SCALE_HEIGHT_M);
      return worstDepthError(
        compositeTransmittanceTwin(SOURCES, TOP_M, view, d),
        marchedTransmittance(view, d),
      );
    });
    // Measured 2026-10-21: 0.74%, 1.08%, 0.04%, 0.08% and 1.2 × 10⁻⁸ of max(1, τ) along the chord,
    // the table's bilinear read of it at the top (τ 11.8, 16.5 and 29.0 at 0.1 H; 5.7, 8.4 and
    // 16.8 at 0.3 H, red, green, blue).
    for (const error of errors) {
      expect(error).toBeLessThan(0.02);
    }
  });

  it("reddens the sun through the limb where the march's mean drew it grey", () => {
    const d = limbRay(distanceM, RAYLEIGH_SCALE_HEIGHT_M);
    const t = compositeTransmittanceTwin(SOURCES, TOP_M, view, d);
    const marched = marchedTransmittance(view, d);
    const mean = (marched[0] + marched[1] + marched[2]) / 3;
    // One scale height up the limb red passes 50 times blue's share (τ 2.67, 3.41, 6.66); the
    // march's mean is one grey for all three, twice red's dimming and 27 times blue's.
    expect([t[0] / t[2] > 20, t[0] / mean > 1.5, t[2] / mean < 0.1]).toEqual([true, true, true]);
  });

  it("takes no step in any channel as the camera crosses the top", () => {
    const below = viewFrom(TOP_M - 1);
    const above = viewFrom(TOP_M + 1);
    const steps = LIMB_SCALE_HEIGHTS.flatMap((n) => {
      const tangentM = n * RAYLEIGH_SCALE_HEIGHT_M;
      const inside = compositeTransmittanceTwin(
        SOURCES,
        TOP_M,
        below,
        limbRay(RADIUS_M + TOP_M - 1, tangentM),
      );
      const outside = compositeTransmittanceTwin(
        SOURCES,
        TOP_M,
        above,
        limbRay(RADIUS_M + TOP_M + 1, tangentM),
      );
      return CHANNELS.filter((c) => inside[c] >= TRANSMITTANCE_FLOOR).map((c) =>
        Math.abs(outside[c] / inside[c] - 1),
      );
    });
    expect(steps.filter((step) => !(step < 2e-3))).toEqual([]);
  });

  it("passes what lies behind a ray that misses the air whole", () => {
    const missing = limbRay(distanceM, TOP_M + 1_000);
    const away: Vec3 = { x: 1, y: 0, z: 0 };
    expect([
      compositeTransmittanceTwin(SOURCES, TOP_M, view, missing),
      compositeTransmittanceTwin(SOURCES, TOP_M, view, away),
    ]).toEqual([
      [1, 1, 1],
      [1, 1, 1],
    ]);
  });

  it("passes nothing from behind the ground", () => {
    const grounded = limbRay(distanceM, -10_000);
    expect(compositeTransmittanceTwin(SOURCES, TOP_M, view, grounded)).toEqual([0, 0, 0]);
  });

  // Since R08.T8.c both take it across the pixel's far-limb footprint
  // (`transmittanceAcrossFromAbove`), whose centre is the entry's lookup along the pixel's ray.
  it("dims what lies behind the sky and each disc from above through the entry's lookup", () => {
    const code = COMPOSITE_MATERIAL.fragmentWgsl.replaceAll(/\/\/[^\n]*/gu, "");
    expect([
      /behind = transmittanceAcrossFromAbove\(v, body, &footprint, dir, slotHere\);/u.test(code),
      /toSpace = transmittanceAcrossFromAbove\(v, body, &footprint, dir, skySlotOf\(source\)\);/u.test(
        code,
      ),
      /let centre = transmittanceFromAbove\(v, body, dir, slot\);/u.test(code),
      /vec3f\(marched\.a\)|marchedAlpha/u.test(code),
    ]).toEqual([true, true, true, false]);
  });
});

// --- The oblate twin on a sphere -----------------------------------------------------------------

/** The sphere as a body that does not turn: one slice at κ = 1 and one band at s = 1. */
const STILL_SPHERE: LevelSpheroid = {
  ...SPHERE,
  gmM3S2: 3.986_004_418e14,
  angularVelocityRadS: 0,
};

/** Small tables for two lookups held against each other; neither multiple scattering is read. */
const SMALL_SIZES: OblateTwinSizes = {
  transmittance: { widthTexels: 32, heightTexels: 8 },
  transmittanceSamples: 64,
  multiScattering: {
    ...MULTI_SCATTERING_KERNEL_OPTIONS,
    size: { widthTexels: 2, heightTexels: 2 },
    samples: 16,
  },
};

/**
 * The largest |ln T − ln T_ref| ÷ max(1, −ln T_ref) over the channels: the optical depth's
 * difference, relative where the chord is opaque, as {@link worstDepthError} takes it, every
 * channel counted.
 */
function worstDepthApart(t: Rgb, reference: Rgb): number {
  return Math.max(
    ...CHANNELS.map((c) => {
      const depth = -Math.log(reference[c]);
      return Math.abs(Math.log(t[c]) + depth) / Math.max(1, depth);
    }),
  );
}

describe("the oblate composite twin on a sphere of one band (R08.T8.b)", () => {
  let oblate: OblatePlanetTwin | undefined;
  let spherical: TwinSources | undefined;
  beforeAll(() => {
    const body = { figure: STILL_SPHERE, gravity: gravityOfSpheroid(STILL_SPHERE) };
    const radiusM = tableRadiusM(STILL_SPHERE);
    oblate = oblatePlanetTwin(EARTH_REFERENCE, body, EARTH_REFERENCE_CURVES, SMALL_SIZES);
    spherical = tableSources({
      medium: EARTH_REFERENCE,
      bottomRadiusM: radiusM,
      ...transmittanceTablesTwin(
        EARTH_REFERENCE,
        radiusM,
        EARTH_REFERENCE_CURVES,
        SMALL_SIZES.transmittance,
        SMALL_SIZES.transmittanceSamples,
      ),
      multiScattering: { widthTexels: 2, heightTexels: 2, texels: new Float64Array(16) },
    });
  });
  const twins = (): { oblate: OblatePlanetTwin; spherical: TwinSources } => {
    if (oblate === undefined || spherical === undefined) {
      throw new Error("the twins are built before the tests");
    }
    return { oblate, spherical };
  };
  /** Each ray's oblate lookup against the sphere's, from `view` ({@link worstDepthApart}). */
  const worstAlong = (view: TwinView, rays: ReadonlyArray<Vec3>): number[] => {
    const { oblate: o, spherical: p } = twins();
    return rays.map((d) =>
      worstDepthApart(
        oblateCompositeTransmittanceTwin(o, view, d),
        compositeTransmittanceTwin(p, TOP_M, view, d),
      ),
    );
  };

  it("reads the sphere's lookup along the limb set from above the top, to 10⁻¹²", () => {
    const distanceM = RADIUS_M + 400_000;
    const rays = LIMB_SCALE_HEIGHTS.map((n) => limbRay(distanceM, n * RAYLEIGH_SCALE_HEIGHT_M));
    // Measured 2026-10-22: 6.7 × 10⁻¹⁵ and 6.3 × 10⁻¹⁴ at 0.1 H and 0.3 H, 0 above them: the
    // entry's height formed two ways, |p| − a on the sphere and the geodetic conversion's here.
    expect(Math.max(...worstAlong(viewFrom(400_000), rays))).toBeLessThan(1e-12);
  });

  it("reads the sphere's lookup along the limb set from inside the air, to 10⁻¹²", () => {
    const distanceM = RADIUS_M + TOP_M - 1;
    const rays = LIMB_SCALE_HEIGHTS.map((n) => limbRay(distanceM, n * RAYLEIGH_SCALE_HEIGHT_M));
    // Measured 2026-10-22: 0 on every ray, the camera's height the view's own in both.
    expect(Math.max(...worstAlong(viewFrom(TOP_M - 1), rays))).toBeLessThan(1e-12);
  });

  it("passes what the sphere's passes where a ray misses the air or meets the ground", () => {
    const { oblate: o, spherical: p } = twins();
    const view = viewFrom(400_000);
    const distanceM = RADIUS_M + 400_000;
    const rays: Vec3[] = [
      limbRay(distanceM, TOP_M + 1_000),
      { x: 1, y: 0, z: 0 },
      limbRay(distanceM, -10_000),
    ];
    expect(rays.map((d) => oblateCompositeTransmittanceTwin(o, view, d))).toEqual(
      rays.map((d) => compositeTransmittanceTwin(p, TOP_M, view, d)),
    );
  });
});

// --- The oblate twin on a body of several bands --------------------------------------------------

/**
 * The decision's component bound on a lookup's optical depth, max(0.01, 2 × 10⁻³ τ), as a ratio
 * over the channels (`decision-r08-spheroid.md`; `tablesCpuOblate.test.ts`'s): ≤ 1 passes.
 */
function boundRatio(lookup: Rgb, march: Rgb): number {
  return Math.max(
    ...CHANNELS.map((c) => Math.abs(lookup[c] - march[c]) / Math.max(0.01, 2e-3 * march[c])),
  );
}

describe("the oblate composite twin from above a body of several bands (R08.T8.b)", () => {
  const medium = saturnClassMedium();
  const body = saturnBody();
  const figure = body.figure;
  const scaleHeightM = saturnScaleHeightM();
  // 1,000 km over the equator, h* 868 km over the top's 795 km, as the smoke's Saturn-class limb.
  const distanceM = figure.equatorialRadiusM + 1_000_000;
  let planet: OblatePlanetTwin | undefined;
  // The slices at the kernel's columns and steps, as the smoke's GPU tables hold them, and 8 rows:
  // from above, every read stands on the top row, and 8 rows give the kernel's 64 rows' figures
  // to every digit (2026-10-22), built in 1.2 s against 9.0 s at a load of about 2.5; the limit is
  // over ten times that. The bands' multiple scattering is unread here.
  beforeAll(() => {
    planet = oblatePlanetTwin(medium, body, [], {
      ...OBLATE_KERNEL_SIZES,
      transmittance: { ...OBLATE_KERNEL_SIZES.transmittance, heightTexels: 8 },
      multiScattering: SMALL_SIZES.multiScattering,
    });
  }, 30_000);

  it("dims each channel through the limb as the true spheroid's converged march", () => {
    if (planet === undefined) {
      throw new Error("the twin is built before the tests");
    }
    const p = planet;
    const camera: AtmosphereCamera = {
      positionM: { x: distanceM, y: 0, z: 0 },
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 2,
      viewport: { widthPx: 64, heightPx: 32 },
    };
    const sun: SunState = {
      directionBodyFixed: { x: 0, y: 1, z: 0 },
      illuminanceLx: referenceSunIlluminanceLx(),
      angularRadiusRad: 0,
    };
    const view = twinView(camera, sun, figure, 0.1);
    // Through the limb set in the scale heights of the Saturn-class air: east along the equator,
    // where the tangent stands n H up, and north across the latitudes, where s and R_α change
    // along the ray and the tangent stands higher over the flattened datum.
    const rays = LIMB_SCALE_HEIGHTS.flatMap((n) => {
      const sin = (figure.equatorialRadiusM + n * scaleHeightM) / distanceM;
      const cos = Math.sqrt(1 - sin * sin);
      return [
        { x: -cos, y: sin, z: 0 },
        { x: -cos, y: 0, z: sin },
      ];
    });
    const ratios = rays.map((d) => {
      const clipped = oblateRayMarchRayAlongTwin(
        p,
        view,
        view.cameraM,
        d,
        Number.POSITIVE_INFINITY,
      );
      if (clipped === null) {
        throw new Error("every ray of the set enters the air");
      }
      const entry = add(view.cameraM, scale(d, clipped.ray.tStartM));
      const march = spheroidOpticalDepth(medium, body, entry, d);
      if (march.hitsGround) {
        throw new Error("every ray of the set clears the ground");
      }
      const t = oblateCompositeTransmittanceTwin(p, view, d);
      return boundRatio([-Math.log(t[0]), -Math.log(t[1]), -Math.log(t[2])], march.opticalDepth);
    });
    // Measured 2026-10-22, of the bound at 0.1, 0.3, 1, 3 and 10 H: east 0.898, 0.253, 0.138,
    // 0.865 and 7.4 × 10⁻⁴ (τ 12.2, 24.5 and 62.5 at 0.1 H, red, green and blue; 0.98, 1.97 and
    // 5.05 at 3 H); north 0.737, 0.602, 0.347, 0.100 and 8.6 × 10⁻⁴ (τ 0.45, 0.90 and 2.31 at
    // 0.1 H, the tangent higher over the flattened datum). The Saturn-class air is isothermal, so
    // the table's local scale height at the entry is the medium's own: the test holds the branch
    // to the truth, not its scale height's source.
    expect(Math.max(...ratios)).toBeLessThan(1);
  });
});
