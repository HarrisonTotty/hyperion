import { beforeAll, describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import {
  gravityOfSpheroid,
  SATURN_SPHEROID,
  saturnClassMedium,
  saturnScaleHeightM,
  WGS84_SPHEROID,
} from "../../test/atmosphereFixtures";
import { quaternion, quaternionFromRows } from "../camera/quaternion";
import type { Rgb } from "../photometry/toneCurve";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES, referenceSunIlluminanceLx } from "./earth";
import {
  type AtmosphereCamera,
  geodeticOf,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
} from "./hillaire";
import type { AtmosphereMedium } from "./medium";
import { gravityRatio, type LevelSpheroid, oblateSlicing } from "./oblate";
import { transmittanceUvToRMu, unitFromTexelUv } from "./opticalDepth";
import { type AbsorberCurveSlots, multiScatteringLayersOf, type TableSize } from "./tables";
import {
  aerialPerspectiveTwin,
  bandMultiScattering,
  kernelOpticalDepth,
  MULTI_SCATTERING_GROUND_OFFSET_M,
  MULTI_SCATTERING_KERNEL_OPTIONS,
  type MultiScatteringOptions,
  oblateAerialPerspectiveTwin,
  type OblatePlanetTwin,
  oblatePlanetTwin,
  oblateRayMarchRayAlongTwin,
  oblateRayMarchTexelTwin,
  oblateRayMarchTwin,
  oblateSkyViewRayTwin,
  oblateSkyViewReadTwin,
  oblateSkyViewTexelsTwin,
  oblateSkyViewTwin,
  type OblateTwinSizes,
  OBLATE_KERNEL_SIZES,
  multiScatteringTwin,
  type PlanetTwin,
  rayMarchTwin,
  type SlicedRay,
  slicedStoredOpticalDepth,
  slicedSunTransmittance,
  skyViewTwin,
  spheroidOpticalDepth,
  type TwinBody,
  type TwinTable,
  transmittanceTablesTwin,
  twinBandOf,
  twinTableTexels,
  twinView,
} from "./tablesCpu";

const CHANNELS = [0, 1, 2] as const;
const DEG = Math.PI / 180;

/** A body's gravity by R08.T3.d's `bodyGravity`, from a level spheroid with no figure law. */
const gravityOf = gravityOfSpheroid;

/** WGS 84 (NIMA TR8350.2, Table 3.1): a, 1 ÷ f, GM with the atmosphere, and ω. */
const WGS84: LevelSpheroid = WGS84_SPHEROID;

/** Saturn at 1 bar (`atmosphereFixtures.ts`'s, R08.T3.d's tests'). */
const SATURN: LevelSpheroid = SATURN_SPHEROID;
const SATURN_BODY: TwinBody = { figure: SATURN, gravity: gravityOf(SATURN) };
const SATURN_R_REF = tableRadiusM(SATURN);

/** The Saturn-class air's isothermal scale height at g_ref, m: 49.7 km. */
const SATURN_H = saturnScaleHeightM();

/**
 * The Saturn-class medium (`atmosphereFixtures.ts`'s `saturnClassMedium`): the H₂–He air's
 * molecular term on e^(−h\* ÷ H), its top 16 H up. Vertical optical depth 0.142, 0.285 and 0.728.
 */
const SATURN_MEDIUM: AtmosphereMedium = saturnClassMedium();

/** The body-fixed point at geodetic latitude φ, longitude 0 and height h (TR8350.2 eq. 4-14). */
function pointAt(figure: SpheroidFigure, latitudeRad: number, heightM: number): Vec3 {
  const a = figure.equatorialRadiusM;
  const e2 = 1 - (figure.polarRadiusM / a) ** 2;
  const sin = Math.sin(latitudeRad);
  const n = a / Math.sqrt(1 - e2 * sin * sin);
  return { x: (n + heightM) * Math.cos(latitudeRad), y: 0, z: (n * (1 - e2) + heightM) * sin };
}

/** The unit direction at zenith cosine μ and azimuth α from north at latitude φ, longitude 0. */
function directionAt(latitudeRad: number, azimuthRad: number, mu: number): Vec3 {
  const horizontal = Math.sqrt(Math.max(0, 1 - mu * mu));
  const north = Math.cos(azimuthRad) * horizontal;
  const east = Math.sin(azimuthRad) * horizontal;
  return {
    x: mu * Math.cos(latitudeRad) - north * Math.sin(latitudeRad),
    y: east,
    z: mu * Math.sin(latitudeRad) + north * Math.cos(latitudeRad),
  };
}

/** The decision's component bound, max(0.01, 2 × 10⁻³ τ), as a ratio: ≤ 1 passes. */
function boundRatio(lookup: Rgb, march: Rgb): number {
  return Math.max(
    ...CHANNELS.map((c) => Math.abs(lookup[c] - march[c]) / Math.max(0.01, 2e-3 * march[c])),
  );
}

// --- Earth's slicing -----------------------------------------------------------------------------

describe("Earth's slicing (R08.T6.e)", () => {
  it("is one slice and one band, its s within ±0.27% of 1 from the equator to the pole", () => {
    const gravity = gravityOf(WGS84);
    expect([...gravity.slicing.kappa]).toEqual([1]);
    expect([...gravity.slicing.bandGravityRatio]).toEqual([1]);
    // Recorded: s_e = 0.997 36 and s_p = 1.002 65, WGS 84's γ_e and γ_p about their mean.
    expect(Math.abs(gravityRatio(gravity, 0) - 1)).toBeLessThan(0.0027);
    expect(Math.abs(gravityRatio(gravity, Math.PI / 2) - 1)).toBeLessThan(0.0027);
  });
});

// --- A sphere with ω = 0 is T6.d's twin ----------------------------------------------------------

/** A sphere of Earth's mean radius R₁, turning not at all: one slice, one band, s = 1 everywhere. */
const STILL_SPHERE: LevelSpheroid = {
  equatorialRadiusM: 6_371_008.8,
  polarRadiusM: 6_371_008.8,
  gmM3S2: 3.986_004_418e14,
  angularVelocityRadS: 0,
};

/** Small sizes for the tests that build a body's tables more than once. */
const SMALL_TRANSMITTANCE: TableSize = { widthTexels: 32, heightTexels: 8 };
const SMALL_MULTI_SCATTERING: MultiScatteringOptions = {
  ...MULTI_SCATTERING_KERNEL_OPTIONS,
  size: { widthTexels: 8, heightTexels: 8 },
  samples: 16,
};
const SMALL_SIZES: OblateTwinSizes = {
  transmittance: SMALL_TRANSMITTANCE,
  transmittanceSamples: 64,
  multiScattering: SMALL_MULTI_SCATTERING,
};

/** T6.d's per-planet twin at the small sizes, as `tablesCpu.test.ts`'s `everyTwin` builds it. */
function smallPlanetTwin(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  curves: AbsorberCurveSlots,
): PlanetTwin {
  const tables = transmittanceTablesTwin(medium, bottomRadiusM, curves, SMALL_TRANSMITTANCE, 64);
  const bySlot = Array.from({ length: multiScatteringLayersOf(medium, curves) }, (_, slot) =>
    multiScatteringTwin(medium, bottomRadiusM, tables, SMALL_MULTI_SCATTERING, undefined, slot),
  );
  const first = bySlot[0];
  if (first === undefined) {
    throw new Error("a multiple-scattering table has a layer");
  }
  return {
    medium,
    bottomRadiusM,
    ...tables,
    multiScattering: first,
    ...(bySlot.length > 1 ? { multiScatteringBySlot: bySlot } : {}),
  };
}

/** A camera `heightM` above the equator at longitude 0, looking north along the horizon. */
function equatorCamera(figure: SpheroidFigure, heightM: number): AtmosphereCamera {
  return {
    positionM: { x: figure.equatorialRadiusM + heightM, y: 0, z: 0 },
    orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
    fovXRad: 2 * Math.atan(2),
    viewport: { widthPx: 16, heightPx: 8 },
  };
}

/** The sun at an elevation above an equator camera's horizon, towards the north-east. */
function sunAt(elevationDeg: number): SunState {
  const e = elevationDeg * DEG;
  const horizontal = Math.cos(e) * Math.SQRT1_2;
  return {
    directionBodyFixed: { x: Math.sin(e), y: horizontal, z: horizontal },
    illuminanceLx: referenceSunIlluminanceLx(),
    angularRadiusRad: 0.004_650_5,
  };
}

/** The largest |a − b| ÷ max(|a|, |b|) over two tables' texels, 0 where both are 0. */
function worstRelative(a: TwinTable, b: TwinTable): number {
  let worst = 0;
  for (const [i, value] of b.texels.entries()) {
    const other = a.texels[i] ?? Number.NaN;
    const scale = Math.max(Math.abs(value), Math.abs(other));
    worst = Math.max(worst, scale === 0 ? 0 : Math.abs(other - value) / scale);
  }
  return worst;
}

/** A scene's depth: sky in the top half of a 16 × 8 view, a surface 60 km away below. */
function skyThenSurface(_x: number, y: number): number {
  return y < 4 ? 0 : 0.1 / 60_000;
}

describe("a sphere with ω = 0 (R08.T6.e)", () => {
  const body: TwinBody = { figure: STILL_SPHERE, gravity: gravityOf(STILL_SPHERE) };
  const radius = tableRadiusM(STILL_SPHERE);
  let oblate: OblatePlanetTwin | undefined;
  let spherical: PlanetTwin | undefined;
  beforeAll(() => {
    oblate = oblatePlanetTwin(EARTH_REFERENCE, body, EARTH_REFERENCE_CURVES, SMALL_SIZES);
    spherical = smallPlanetTwin(EARTH_REFERENCE, radius, EARTH_REFERENCE_CURVES);
  }, 120_000);
  const twins = (): { oblate: OblatePlanetTwin; spherical: PlanetTwin } => {
    if (oblate === undefined || spherical === undefined) {
      throw new Error("the twins are built before the tests");
    }
    return { oblate, spherical };
  };

  it("is sliced into one slice at κ = 1 and one band at s = 1", () => {
    expect([...body.gravity.slicing.kappa]).toEqual([1]);
    expect([...body.gravity.slicing.bandGravityRatio]).toEqual([1]);
  });

  it("reproduces T6.d's per-planet tables to 10⁻¹²", () => {
    const { oblate: o, spherical: p } = twins();
    const slice = o.slices[0];
    const band = o.bands[0];
    expect(slice).toBeDefined();
    expect(band?.transmittance).toBe(slice);
    expect(worstRelative(slice?.transmittance ?? p.transmittance, p.transmittance)).toBeLessThan(
      1e-12,
    );
    expect(
      worstRelative(slice?.absorberColumns ?? p.absorberColumns, p.absorberColumns),
    ).toBeLessThan(1e-12);
    const layers = p.multiScatteringBySlot ?? [p.multiScattering];
    expect(band?.multiScatteringBySlot.length).toBe(layers.length);
    for (const [k, layer] of layers.entries()) {
      expect(worstRelative(band?.multiScatteringBySlot[k] ?? layer, layer)).toBeLessThan(1e-12);
    }
  });

  for (const [heightM, elevationDeg] of [
    [2, 20],
    [2_000, -2],
  ] as const) {
    it(`reproduces T6.d's sky view and aerial perspective to 10⁻¹² from ${heightM} m, the sun ${elevationDeg}° up`, () => {
      const { oblate: o, spherical: p } = twins();
      const view = twinView(
        equatorCamera(STILL_SPHERE, heightM),
        sunAt(elevationDeg),
        STILL_SPHERE,
        0.1,
      );
      const size = { widthTexels: 16, heightTexels: 12 };
      expect(
        worstRelative(oblateSkyViewTwin(o, view, size, 24), skyViewTwin(p, view, size, 24)),
      ).toBeLessThan(1e-12);
      const volume = { widthTexels: 8, heightTexels: 4, slices: 6 };
      expect(
        worstRelative(
          oblateAerialPerspectiveTwin(o, view, volume, 2, 32_000),
          aerialPerspectiveTwin(p, view, volume, 2, 32_000),
        ),
      ).toBeLessThan(1e-12);
    });
  }

  it("reproduces T6.d's ray march to 10⁻¹² from orbit and from inside the air", () => {
    const { oblate: o, spherical: p } = twins();
    const settings = { ...TABLE_SIZES.low, rayMarchSamples: 24 };
    const orbit = twinView(equatorCamera(STILL_SPHERE, 400_000), sunAt(30), STILL_SPHERE, 0.1);
    expect(
      worstRelative(
        oblateRayMarchTwin(o, orbit, settings, () => 0),
        rayMarchTwin(p, orbit, settings, () => 0),
      ),
    ).toBeLessThan(1e-12);
    const low = twinView(equatorCamera(STILL_SPHERE, 3_000), sunAt(20), STILL_SPHERE, 0.1);
    expect(
      worstRelative(
        oblateRayMarchTwin(o, low, settings, skyThenSurface),
        rayMarchTwin(p, low, settings, skyThenSurface),
      ),
    ).toBeLessThan(1e-12);
  });
});

// --- The sliced lookup on a Saturn-class figure --------------------------------------------------

/** The ray set of `decision-r08-spheroid.md` §4(f): where they clear the ground. */
interface SetRay {
  readonly name: string;
  readonly latitudeDeg: number;
  readonly towards: string;
  readonly positionM: Vec3;
  readonly direction: Vec3;
}

const TOWARDS = [
  ["north", 0],
  ["north-east", 45],
  ["south", 180],
  ["east", 90],
] as const;

function raySet(): SetRay[] {
  const rays: SetRay[] = [];
  const places: Array<readonly [number, string, number]> = [];
  for (const latitudeDeg of [0, 15, 30, 45, 60, 75]) {
    for (const [towards, azimuthDeg] of TOWARDS) {
      places.push([latitudeDeg, towards, azimuthDeg]);
    }
  }
  places.push([90, "north", 0]);
  for (const [latitudeDeg, towards, azimuthDeg] of places) {
    const phi = latitudeDeg * DEG;
    const s = gravityRatio(SATURN_BODY.gravity, phi);
    for (const scaleHeights of [0, 0.3, 1, 3]) {
      for (const mu of [-0.035, -0.02, -0.01, 0, 0.05, 0.2, 0.5]) {
        rays.push({
          name: `${latitudeDeg}° ${towards}, h* ${scaleHeights} H, μ ${mu}`,
          latitudeDeg,
          towards,
          positionM: pointAt(SATURN, phi, (scaleHeights * SATURN_H) / s),
          direction: directionAt(phi, azimuthDeg * DEG, mu),
        });
      }
    }
  }
  return rays;
}

/**
 * A sliced lookup's optical depth with each slice it reads integrated along its own ray, R05's
 * midpoint rule at 8,192 steps, in place of the slice's table: the lookup's sphere and slicing
 * alone, as `decision-r08-spheroid.md` measured them.
 */
function exactSlices(planet: OblatePlanetTwin, ray: SlicedRay): Rgb {
  const out: [number, number, number] = [0, 0, 0];
  const heightM = Math.min(Math.max(ray.scaledHeightM, 0), planet.medium.topHeightM);
  for (const read of ray.reads) {
    const radiusM = (planet.kappa[read.slice] ?? Number.NaN) * planet.referenceRadiusM;
    const tau = kernelOpticalDepth(planet.medium, radiusM, radiusM + heightM, read.cosine, 8_192);
    for (const c of CHANNELS) {
      out[c] += (read.weight * tau[c]) / ray.gravityRatio;
    }
  }
  return out;
}

/** One ray's lookups against the march, as ratios to the bound. */
interface SetResult {
  readonly ray: SetRay;
  readonly march: Rgb;
  readonly fitted: number;
  readonly sample: number;
  readonly throughTables: number;
  readonly beyondRange: number;
}

/** Saturn's slices at the kernel's size, built once for the tests that read them; no band is read. */
let saturnSlices: OblatePlanetTwin | undefined;

/** {@link saturnSlices}, built on first use. */
function saturnSlicesPlanet(): OblatePlanetTwin {
  saturnSlices ??= oblatePlanetTwin(SATURN_MEDIUM, SATURN_BODY, [], {
    ...OBLATE_KERNEL_SIZES,
    multiScattering: { ...SMALL_MULTI_SCATTERING, size: { widthTexels: 2, heightTexels: 2 } },
  });
  return saturnSlices;
}

describe("the sliced lookup on a Saturn-class figure (decision-r08-spheroid)", () => {
  let results: SetResult[] = [];
  let convergence = Number.NaN;
  beforeAll(() => {
    const p = saturnSlicesPlanet();
    convergence = 0;
    results = raySet().flatMap((ray) => {
      const march = spheroidOpticalDepth(SATURN_MEDIUM, SATURN_BODY, ray.positionM, ray.direction);
      if (march.hitsGround) {
        return [];
      }
      const fine = spheroidOpticalDepth(
        SATURN_MEDIUM,
        SATURN_BODY,
        ray.positionM,
        ray.direction,
        2_500,
      );
      for (const c of CHANNELS) {
        convergence = Math.max(
          convergence,
          Math.abs(march.opticalDepth[c] / fine.opticalDepth[c] - 1),
        );
      }
      const fitted = slicedSunTransmittance(p, ray.positionM, ray.direction, "fitted");
      const sample = slicedSunTransmittance(p, ray.positionM, ray.direction, "sample");
      return [
        {
          ray,
          march: march.opticalDepth,
          fitted: boundRatio(exactSlices(p, fitted.ray), march.opticalDepth),
          sample: boundRatio(exactSlices(p, sample.ray), march.opticalDepth),
          throughTables: boundRatio(fitted.storedOpticalDepth, march.opticalDepth),
          beyondRange: fitted.ray.beyondRange,
        },
      ];
    });
  }, 300_000);

  it("is sliced into Design note 17's five slices and four bands", () => {
    expect(SATURN_BODY.gravity.slicing.kappa.length).toBe(5);
    expect(SATURN_BODY.gravity.slicing.bandGravityRatio.length).toBe(4);
    expect(oblateSlicing(SATURN, SATURN_R_REF).kappa.length).toBe(5);
  });

  it("marches the true spheroid converged below 10⁻⁷ on every ray, its pieces halved", () => {
    // Recorded: 1.4 × 10⁻¹⁴, the profile being smooth.
    expect(convergence).toBeLessThan(1e-7);
  });

  it("holds every ray of the set within max(0.01, 2 × 10⁻³ τ) on the lookup's own sphere and slices, each slice integrated exactly", () => {
    // 598 rays clear the ground of the set's 700. Recorded: the worst is 0.782 of the bound (30°
    // north from 1 H at μ −0.035), the slices interpolated linearly in √κ (R08.T6.f; 0.905 in ln κ).
    expect(results.length).toBe(598);
    const over = results.filter((r) => !(r.fitted <= 1)).map((r) => r.ray.name);
    expect(over).toEqual([]);
    expect(Math.max(...results.map((r) => r.fitted))).toBeLessThan(0.85);
  });

  it("misses the bound at the sample's own s and R_α along the meridian, which the fit takes up", () => {
    // Recorded beside the fitted lookup: the sample's own sphere is over the bound on 110 of the 192
    // rays at 30–75° looking north or north-east, at most 12.8 times.
    const meridian = results.filter(
      (r) => r.ray.latitudeDeg >= 30 && r.ray.latitudeDeg <= 75 && r.ray.towards === "north",
    );
    expect(Math.max(...meridian.map((r) => r.sample))).toBeGreaterThan(2);
    const parallel = results.filter(
      (r) => r.ray.latitudeDeg === 0 || r.ray.latitudeDeg === 90 || r.ray.towards === "east",
    );
    expect(Math.max(...parallel.map((r) => r.fitted))).toBeLessThan(1);
  });

  it("pins the twin's own read through the slices' 256 × 64 tables, which misses the bound on 1 ray", () => {
    // Recorded and pinned (plan R08's Risks, "Deviations in T6.f, as built"): the tables' bilinear
    // read adds up to a third of the bound where 64 rows span 16 scale heights. With the slices
    // linear in √κ (R08.T6.f; 1.244 and 5 rays in ln κ) one of the 598 rays passes over it, 1.072
    // times: 30° north from 3 H at μ −0.035, which dips deeply (τ 0.925, 1.857 and 4.747). More rows
    // do not cure it in order: 72 put it at 1.116 with a second ray over, 80 at 0.906, 88 at 1.031,
    // 96 at 0.815 and 128 at 0.876; 512 columns at 1.023.
    const worst = Math.max(...results.map((r) => r.throughTables));
    expect(worst).toBeLessThan(1.1);
    expect(results.filter((r) => r.throughTables > 1).map((r) => r.ray.name)).toEqual([
      "30° north, h* 3 H, μ -0.035",
    ]);
  });

  it("holds κ* within 0.2 of the slices' range in ln κ, the steep rays along the meridian furthest", () => {
    // Recorded: 99 rays leave the range, by under 0.003 at the equator and the pole, and by up to
    // 0.176 at μ 0.2 and 0.5 along the meridian, where a steep ray hardly curves.
    expect(Math.max(...results.map((r) => Math.abs(r.beyondRange)))).toBeLessThan(0.2);
  });
});

// --- Interpolating between slices and between bands ----------------------------------------------

/** The bands' sizes for the interpolation test: their own tables small, the kernel's steps. */
const BAND_SIZES: OblateTwinSizes = {
  transmittance: { widthTexels: 128, heightTexels: 32 },
  transmittanceSamples: 128,
  multiScattering: {
    ...MULTI_SCATTERING_KERNEL_OPTIONS,
    size: { widthTexels: 16, heightTexels: 16 },
  },
};

/**
 * The radiance Ψ adds, per unit of the sun's illuminance, along a path of h\* `heightsM` and sun
 * cosines `cosines` at points `stepM` apart in the band's frame (the division by s is the same for
 * both reads compared), Ψ read at each point's h\* through `bands` at s, times the scattering, through
 * the view's transmittance from the path's first point.
 */
function multipleScatteringRadiance(
  bands: OblatePlanetTwin,
  s: number,
  heightsM: ReadonlyArray<number>,
  cosines: ReadonlyArray<number>,
  stepM: number,
): Rgb {
  const term = SATURN_MEDIUM.terms[0];
  if (term === undefined) {
    throw new Error("the medium has its gas");
  }
  const out: [number, number, number] = [0, 0, 0];
  const depth: [number, number, number] = [0, 0, 0];
  for (const [i, heightM] of heightsM.entries()) {
    const density = Math.exp(-heightM / SATURN_H);
    const psi = bandMultiScattering(bands, heightM, s, cosines[i] ?? Number.NaN);
    for (const c of CHANNELS) {
      const sigma = term.scattering[c] * density;
      out[c] += Math.exp(-(depth[c] + 0.5 * sigma * stepM)) * sigma * psi[c] * stepM;
      depth[c] += sigma * stepM;
    }
  }
  return out;
}

/** A view from the datum at zenith cosine `cosView`, plane-parallel, the sun at `muSun`. */
function groundView(
  cosView: number,
  muSun: number,
): {
  heightsM: number[];
  cosines: number[];
  stepM: number;
} {
  const steps = 800;
  const dh = SATURN_MEDIUM.topHeightM / steps;
  const heightsM = Array.from({ length: steps }, (_, i) => (i + 0.5) * dh);
  return { heightsM, cosines: heightsM.map(() => muSun), stepM: dh / cosView };
}

/**
 * A limb from orbit at tangent h\* `tangentM` on a sphere of radius `radiusM` in the band's frame,
 * the sun at elevation `elevationDeg` above the tangent point's horizon, in the plane of the view
 * or across it: each point's h\* and sun cosine against its own vertical.
 */
function limbView(
  radiusM: number,
  tangentM: number,
  elevationDeg: number,
  across: boolean,
): { heightsM: number[]; cosines: number[]; stepM: number } {
  const steps = 4_000;
  const reach = radiusM + tangentM;
  const half = Math.sqrt((radiusM + SATURN_MEDIUM.topHeightM) ** 2 - reach * reach);
  const stepM = (2 * half) / steps;
  const e = elevationDeg * DEG;
  const heightsM: number[] = [];
  const cosines: number[] = [];
  for (let i = 0; i < steps; i += 1) {
    const x = -half + (i + 0.5) * stepM;
    const r = Math.hypot(x, reach);
    heightsM.push(r - radiusM);
    cosines.push(across ? (Math.sin(e) * reach) / r : (Math.cos(e) * x + Math.sin(e) * reach) / r);
  }
  return { heightsM, cosines, stepM };
}

describe("interpolating between slices and between bands (R08.T6.e)", () => {
  let bands: OblatePlanetTwin | undefined;
  const bandsPlanet = (): OblatePlanetTwin => {
    if (bands === undefined) {
      throw new Error("the bands are built before the tests");
    }
    return bands;
  };
  let exact: ReadonlyArray<{ readonly s: number; readonly planet: OblatePlanetTwin }> = [];
  beforeAll(() => {
    saturnSlicesPlanet();
    const p = oblatePlanetTwin(SATURN_MEDIUM, SATURN_BODY, [], BAND_SIZES);
    bands = p;
    const ratios = SATURN_BODY.gravity.slicing.bandGravityRatio;
    exact = Array.from({ length: ratios.length - 1 }, (_, b) => {
      // Halfway in ln s, where linear interpolation is worst.
      const s = Math.sqrt((ratios[b] ?? Number.NaN) * (ratios[b + 1] ?? Number.NaN));
      return {
        s,
        planet: { ...p, bands: [twinBandOf(SATURN_MEDIUM, SATURN_BODY, s, [], BAND_SIZES)] },
      };
    });
  }, 300_000);

  it("costs under 0.1% in grazing optical depth between slices, against the exact integral at κ", () => {
    // In place of a table built at the exact κ, whose texels would add their own 1.4 × 10⁻³: the
    // twin's read of the slices' tables is held to the exact interpolation beside it.
    const planet = saturnSlicesPlanet();
    let worst = 0;
    for (let k = 0; k + 1 < planet.kappa.length; k += 1) {
      const lower = planet.kappa[k] ?? Number.NaN;
      const upper = planet.kappa[k + 1] ?? Number.NaN;
      // Halfway in ln κ and halfway in √κ.
      for (const kappa of [
        Math.sqrt(lower * upper),
        ((Math.sqrt(lower) + Math.sqrt(upper)) / 2) ** 2,
      ]) {
        // The slices' weight, linear in √κ (R08.T6.f).
        const weight =
          (Math.sqrt(kappa) - Math.sqrt(lower)) / (Math.sqrt(upper) - Math.sqrt(lower));
        const radiusM = kappa * planet.referenceRadiusM;
        for (const scaleHeights of [0, 0.3, 1, 3]) {
          const heightM = scaleHeights * SATURN_H;
          const read = slicedStoredOpticalDepth(planet, kappa, heightM, 0);
          const truth = kernelOpticalDepth(SATURN_MEDIUM, radiusM, radiusM + heightM, 0, 8_192);
          const neighbours = [k, k + 1].map((j) => {
            const r = (planet.kappa[j] ?? Number.NaN) * planet.referenceRadiusM;
            return kernelOpticalDepth(SATURN_MEDIUM, r, r + heightM, 0, 8_192);
          });
          for (const c of CHANNELS) {
            const interpolated =
              (1 - weight) * (neighbours[0]?.[c] ?? Number.NaN) +
              weight * (neighbours[1]?.[c] ?? Number.NaN);
            worst = Math.max(worst, Math.abs(interpolated / truth[c] - 1));
            expect(Math.abs(read[c] / interpolated - 1)).toBeLessThan(2e-3);
          }
        }
      }
    }
    // Recorded: the slices linear in √κ (R08.T6.f), exact for Chapman's grazing √R; 6.84 × 10⁻⁴ in
    // ln κ (R08.T6.e).
    expect(worst).toBeLessThan(1e-3);
  });

  it("costs under 0.5% in multiple-scattering radiance between bands, from the datum, against a band at the exact s", () => {
    let worst = 0;
    for (const { s, planet } of exact) {
      for (const cosView of [1, 0.5, 0.17]) {
        for (const muSun of [1, 0.5, 0.2, 0.1, 0.05]) {
          const { heightsM, cosines, stepM } = groundView(cosView, muSun);
          const read = multipleScatteringRadiance(bandsPlanet(), s, heightsM, cosines, stepM);
          const truth = multipleScatteringRadiance(planet, s, heightsM, cosines, stepM);
          for (const c of CHANNELS) {
            worst = Math.max(worst, Math.abs(read[c] / truth[c] - 1));
          }
        }
      }
    }
    // Recorded: 3.2 × 10⁻⁴.
    expect(worst).toBeLessThan(5e-3);
  });

  it("costs under 0.5% in multiple-scattering radiance between bands on the limb from orbit up to 3 H, and records 10 H", () => {
    const worstAt = (tangentScaleHeights: number): number => {
      let worst = 0;
      for (const { s, planet } of exact) {
        const radiusM = planet.bands[0]?.bottomRadiusM ?? Number.NaN;
        for (const elevationDeg of [0, 10, 30]) {
          for (const across of [false, true]) {
            const { heightsM, cosines, stepM } = limbView(
              radiusM,
              tangentScaleHeights * SATURN_H,
              elevationDeg,
              across,
            );
            const read = multipleScatteringRadiance(bandsPlanet(), s, heightsM, cosines, stepM);
            const truth = multipleScatteringRadiance(planet, s, heightsM, cosines, stepM);
            for (const c of CHANNELS) {
              worst = Math.max(worst, Math.abs(read[c] / truth[c] - 1));
            }
          }
        }
      }
      return worst;
    };
    // Recorded: 0.45%, 0.24% and 0.094% at 0.3, 1 and 3 H, and 0.19% at 5 H.
    expect(worstAt(0.3)).toBeLessThan(5e-3);
    expect(worstAt(1)).toBeLessThan(5e-3);
    expect(worstAt(3)).toBeLessThan(5e-3);
    // Recorded and deferred at 10 H, where Ψ is curved in s (plan R08's Risks, R08.T6.e).
    expect(worstAt(10)).toBeLessThan(LIMB_10H_BAND_WORST);
  });
});

/**
 * The bands' worst in multiple-scattering radiance on the limb at 10 H, pinned over its measured
 * 3.25% (plan R08's Risks, deferred, R08.T6.e): Ψ is curved in s that high.
 */
const LIMB_10H_BAND_WORST = 0.035;

// --- The oblate ray march and aerial perspective on a Saturn-class figure ------------------------

/** The mean over the channels of e^(−τ): what a ray-march texel and a froxel keep in alpha. */
function meanTransmittance(tau: Rgb): number {
  return (Math.exp(-tau[0]) + Math.exp(-tau[1]) + Math.exp(-tau[2])) / 3;
}

/** A camera `heightM` above the geodetic latitude φ at longitude 0, looking north. */
function northCamera(latitudeRad: number, heightM: number): AtmosphereCamera {
  const up = { x: Math.cos(latitudeRad), y: 0, z: Math.sin(latitudeRad) };
  const north = { x: -Math.sin(latitudeRad), y: 0, z: Math.cos(latitudeRad) };
  // Camera +x to east (+y here), +y to up and −z to north: the rows of that rotation.
  return {
    positionM: pointAt(SATURN, latitudeRad, heightM),
    orientation: quaternionFromRows([
      { x: 0, y: up.x, z: -north.x },
      { x: 1, y: up.y, z: -north.y },
      { x: 0, y: up.z, z: -north.z },
    ]),
    fovXRad: Math.PI / 2,
    viewport: { widthPx: 3, heightPx: 3 },
  };
}

/** The sun 20° up towards the north-east from a point at latitude φ. */
function saturnSun(latitudeRad: number): SunState {
  return {
    directionBodyFixed: directionAt(latitudeRad, 45 * DEG, Math.sin(20 * DEG)),
    illuminanceLx: referenceSunIlluminanceLx(),
    angularRadiusRad: 0.000_5,
  };
}

describe("the oblate ray march and aerial perspective on a Saturn-class figure (R08.T6.e)", () => {
  const top = SATURN_MEDIUM.topHeightM;
  const scaledHeightOf = (p: Vec3): number => {
    const g = geodeticOf(p, SATURN);
    return gravityRatio(SATURN_BODY.gravity, g.latitudeRad) * g.heightM;
  };

  it("enters the air at h* = the top over the equator and over the pole", () => {
    const planet = saturnSlicesPlanet();
    for (const latitudeRad of [0, Math.PI / 2]) {
      const view = twinView(
        northCamera(latitudeRad, 10 * top),
        saturnSun(latitudeRad),
        SATURN,
        0.1,
      );
      const down = {
        x: -Math.cos(latitudeRad),
        y: 0,
        z: -Math.sin(latitudeRad),
      };
      const marched = oblateRayMarchRayAlongTwin(
        planet,
        view,
        view.cameraM,
        down,
        Number.POSITIVE_INFINITY,
      );
      expect(marched).not.toBeNull();
      const t = marched?.ray.tStartM ?? Number.NaN;
      const entry = {
        x: view.cameraM.x + t * down.x,
        y: view.cameraM.y + t * down.y,
        z: view.cameraM.z + t * down.z,
      };
      expect(Math.abs(scaledHeightOf(entry) - top)).toBeLessThan(1e-6 * top);
    }
  });

  it("gives a limb ray's transmittance as the true spheroid's e^(−τ), the medium at h* of the geodetic height", () => {
    const planet = saturnSlicesPlanet();
    const phi = 45 * DEG;
    const view = twinView(northCamera(phi, 2 * top), saturnSun(phi), SATURN, 0.1);
    // Northwards, 0.22 below the camera's horizontal: a limb ray through a few H at mid-latitudes.
    const direction = directionAt(phi, 0, -0.22);
    const marched = oblateRayMarchRayAlongTwin(
      planet,
      view,
      view.cameraM,
      direction,
      Number.POSITIVE_INFINITY,
    );
    if (marched === null) {
      throw new Error("the limb ray enters the air");
    }
    const settings = { ...TABLE_SIZES.high, rayMarchSamples: 8_192 };
    const [, , , transmittance] = oblateRayMarchTexelTwin(planet, view, settings, marched);
    const start = marched.ray.tStartM;
    const entry = {
      x: view.cameraM.x + start * direction.x,
      y: view.cameraM.y + start * direction.y,
      z: view.cameraM.z + start * direction.z,
    };
    const tau = spheroidOpticalDepth(SATURN_MEDIUM, SATURN_BODY, entry, direction).opticalDepth;
    const truth = meanTransmittance(tau);
    // Recorded: τ 0.773, 1.55 and 3.97, and the two within 2.2 × 10⁻⁸.
    expect(tau[1]).toBeGreaterThan(0.5);
    expect(Math.abs(transmittance / truth - 1)).toBeLessThan(1e-4);
  });

  it("gives an aerial-perspective column's transmittance as the true spheroid's over its reach", () => {
    const planet = saturnSlicesPlanet();
    const phi = 45 * DEG;
    const view = twinView(northCamera(phi, 2_000), saturnSun(phi), SATURN, 0.1);
    const reachM = 32_000;
    const volume = oblateAerialPerspectiveTwin(
      planet,
      view,
      { widthTexels: 3, heightTexels: 3, slices: 2 },
      256,
      reachM,
    );
    // The centre froxel looks along the view's forward, north along the camera's horizontal.
    const alpha = volume.texels[((1 * 3 + 1) * 3 + 1) * 4 + 3] ?? Number.NaN;
    const term = SATURN_MEDIUM.terms[0];
    if (term === undefined) {
      throw new Error("the medium has its gas");
    }
    const steps = 20_000;
    const depth: [number, number, number] = [0, 0, 0];
    for (let i = 0; i < steps; i += 1) {
      const t = ((i + 0.5) / steps) * reachM;
      const density = Math.exp(
        -scaledHeightOf({
          x: view.cameraM.x + t * view.forward.x,
          y: view.cameraM.y + t * view.forward.y,
          z: view.cameraM.z + t * view.forward.z,
        }) / SATURN_H,
      );
      for (const c of CHANNELS) {
        depth[c] += (term.scattering[c] * density * reachM) / steps;
      }
    }
    const truth = meanTransmittance(depth);
    // Recorded: 5.5 × 10⁻¹⁰.
    expect(Math.abs(alpha / truth - 1)).toBeLessThan(1e-6);
  });
});

// --- The slices' and bands' texel mapping --------------------------------------------------------

describe("the slices' and bands' texel mapping (Design note 19)", () => {
  const sizes: OblateTwinSizes = {
    transmittance: { widthTexels: 32, heightTexels: 16 },
    transmittanceSamples: 64,
    multiScattering: { ...SMALL_MULTI_SCATTERING, size: { widthTexels: 8, heightTexels: 8 } },
  };
  let planet: OblatePlanetTwin | undefined;
  beforeAll(() => {
    planet = oblatePlanetTwin(SATURN_MEDIUM, SATURN_BODY, [], sizes);
  }, 120_000);

  it("reads every texel of every slice back at its own (h*, μ) to 10⁻⁹", () => {
    const p = planet;
    if (p === undefined) {
      throw new Error("built before the tests");
    }
    let worst = 0;
    for (const [k, slice] of p.slices.entries()) {
      const kappa = p.kappa[k] ?? Number.NaN;
      const bottomRadiusM = kappa * p.referenceRadiusM;
      const shell = { bottomRadiusM, topRadiusM: bottomRadiusM + SATURN_MEDIUM.topHeightM };
      const { widthTexels: w, heightTexels: h } = slice.transmittance;
      for (let y = 0; y < h; y += 1) {
        for (let x = 0; x < w; x += 1) {
          const { rM, mu } = transmittanceUvToRMu(
            shell,
            unitFromTexelUv((x + 0.5) / w, w),
            unitFromTexelUv((y + 0.5) / h, h),
          );
          const read = slicedStoredOpticalDepth(p, kappa, rM - bottomRadiusM, mu);
          for (const c of CHANNELS) {
            const stored = slice.transmittance.texels[(y * w + x) * 4 + c] ?? Number.NaN;
            worst = Math.max(worst, Math.abs(read[c] - stored) / Math.max(stored, 1e-9));
          }
        }
      }
    }
    expect(worst).toBeLessThan(1e-9);
  });

  it("reads every texel of every band back at its own (h*, μ_s) and s to 10⁻⁹", () => {
    const p = planet;
    if (p === undefined) {
      throw new Error("built before the tests");
    }
    const top = SATURN_MEDIUM.topHeightM;
    let worst = 0;
    for (const band of p.bands) {
      const table = band.multiScatteringBySlot[0];
      if (table === undefined) {
        throw new Error("a band has a layer");
      }
      const { widthTexels: w, heightTexels: h } = table;
      for (let y = 0; y < h; y += 1) {
        for (let x = 0; x < w; x += 1) {
          const heightM =
            MULTI_SCATTERING_GROUND_OFFSET_M +
            unitFromTexelUv((y + 0.5) / h, h) * (top - MULTI_SCATTERING_GROUND_OFFSET_M);
          const muSun = unitFromTexelUv((x + 0.5) / w, w) * 2 - 1;
          const read = bandMultiScattering(p, heightM, band.gravityRatio, muSun);
          for (const c of CHANNELS) {
            const stored = table.texels[(y * w + x) * 4 + c] ?? Number.NaN;
            worst = Math.max(worst, Math.abs(read[c] - stored) / Math.max(stored, 1e-12));
          }
        }
      }
    }
    expect(worst).toBeLessThan(1e-9);
  });

  it("builds each band at its own latitude's s and √(MN), its sphere s √(MN)", () => {
    const p = planet;
    if (p === undefined) {
      throw new Error("built before the tests");
    }
    for (const band of p.bands) {
      expect(gravityRatio(SATURN_BODY.gravity, band.latitudeRad)).toBeCloseTo(
        band.gravityRatio,
        12,
      );
      // √(MN) runs from c at the equator to a² ÷ c at the pole.
      const gaussianM = band.bottomRadiusM / band.gravityRatio;
      expect(gaussianM).toBeGreaterThanOrEqual(SATURN.polarRadiusM * (1 - 1e-12));
      expect(gaussianM).toBeLessThanOrEqual(
        (SATURN.equatorialRadiusM ** 2 / SATURN.polarRadiusM) * (1 + 1e-12),
      );
    }
  });

  it("stands the sky view's samples at the camera's height in its shell from above the air (R08.T6.f)", () => {
    const p = planet;
    if (p === undefined) {
      throw new Error("built before the tests");
    }
    // From 1,000 km and from 1,500 km over the equator, both above the air's top (about 916 km
    // there), the sky view holds the camera 1 m inside its shell on the same vertical: every
    // sample, and every lookup's start, at the same body-fixed place, so the tables agree.
    const viewFrom = (heightM: number): ReturnType<typeof twinView> =>
      twinView(
        {
          positionM: pointAt(SATURN, 0, heightM),
          orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
          fovXRad: Math.PI / 2,
          viewport: { widthPx: 8, heightPx: 4 },
        },
        {
          directionBodyFixed: directionAt(0, 60 * DEG, Math.sin(10 * DEG)),
          illuminanceLx: referenceSunIlluminanceLx(),
          angularRadiusRad: 0.000_5,
        },
        SATURN,
        0.1,
      );
    const size = { widthTexels: 12, heightTexels: 10 };
    const low = oblateSkyViewTexelsTwin(p, viewFrom(1_000_000), size, 8);
    const high = oblateSkyViewTexelsTwin(p, viewFrom(1_500_000), size, 8);
    let worst = 0;
    for (let y = 0; y < size.heightTexels; y += 1) {
      for (let x = 0; x < size.widthTexels; x += 1) {
        const a = low(x, y);
        const b = high(x, y);
        for (const c of CHANNELS) {
          worst = Math.max(worst, Math.abs(b[c] - a[c]) / Math.max(a[c], 1e-12));
        }
      }
    }
    expect(worst).toBeLessThan(1e-9);
  });

  it("reads the oblate sky view back at each texel's own ray, its horizon on the column's R_α", () => {
    const p = planet;
    if (p === undefined) {
      throw new Error("built before the tests");
    }
    const phi = 45 * DEG;
    const camera: AtmosphereCamera = {
      positionM: pointAt(SATURN, phi, 2_000),
      orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
      fovXRad: Math.PI / 2,
      viewport: { widthPx: 8, heightPx: 4 },
    };
    const sun: SunState = {
      directionBodyFixed: directionAt(phi, 30 * DEG, Math.sin(10 * DEG)),
      illuminanceLx: referenceSunIlluminanceLx(),
      angularRadiusRad: 0.000_5,
    };
    const view = twinView(camera, sun, SATURN, 0.1);
    const size = { widthTexels: 12, heightTexels: 10 };
    const texelAt = oblateSkyViewTexelsTwin(p, view, size, 8);
    const table = oblateSkyViewTwin(p, view, size, 8);
    const n = view.normal;
    const ax = view.skyAxis;
    const ay = {
      x: n.y * ax.z - n.z * ax.y,
      y: n.z * ax.x - n.x * ax.z,
      z: n.x * ax.y - n.y * ax.x,
    };
    // Interior columns and rows, away from the zenith's and the nadir's rows and the seam.
    for (const x of [2, 5, 9]) {
      for (const y of [2, 4, 7]) {
        const d = oblateSkyViewRayTwin(p, view, x, y, size).ray.direction;
        const direction = {
          x: d.x * ax.x + d.y * ay.x + d.z * n.x,
          y: d.x * ax.y + d.y * ay.y + d.z * n.y,
          z: d.x * ax.z + d.y * ay.z + d.z * n.z,
        };
        const read = oblateSkyViewReadTwin(texelAt, size, p, view, direction);
        const stored = twinTableTexels(table)(x, y);
        for (const c of CHANNELS) {
          expect(Math.abs(read[c] / stored[c] - 1)).toBeLessThan(1e-6);
        }
      }
    }
  });
});
