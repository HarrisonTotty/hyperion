import { beforeAll, describe, expect, it } from "vitest";

import bakeSpectra from "../../../../../../../packages/protocol/fixtures/bake_spectra.json" with { type: "json" };
import { cross, normalise, type Vec3 } from "../../geometry/vec3";
import { fourAbsorberMedium } from "../../test/atmosphereFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { quaternion } from "../camera/quaternion";
import type { LightAtPoint } from "../lighting/hostLights";
import { aHostDisc } from "../../test/litFixtures";
import type { SecondarySource } from "../lighting/planetshine";
import { exposureScale } from "../photometry/exposure";
import { fromHalfBits, HALF_MAX, toHalfBits } from "../sky/half";
import type { Rgb } from "../photometry/toneCurve";
import {
  EARTH_FIT_COLUMN,
  earthFitAir,
  molecularAir,
  type SpectralAir,
  viewResponse,
} from "./channels";
import {
  EARTH_DRY_AIR,
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
  referenceSunIlluminanceLx,
} from "./earth";
import {
  type AtmosphereCamera,
  type AtmosphereScene,
  COMPOSITE_MATERIAL,
  HillaireAtmosphere,
  MAX_SKY_SOURCES,
  skyViewHeight,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
} from "./hillaire";
import { type AtmosphereMedium, CHANNEL_WAVELENGTHS_NM } from "./medium";
import { maxDistanceM, opticalDepthToTop, texelUvFromUnit, unitFromTexelUv } from "./opticalDepth";
import { skySources, twilightLimitRad } from "./sources";
import {
  bakeSpectrumOf,
  deltaUv,
  PIPELINE_WAVELENGTHS_NM,
  sunWeights,
  weightedRgb,
} from "./spectralColour";
import type { TableSize } from "./tables";
import {
  aerialPerspectiveTwin,
  frameTwinView,
  marchTwin,
  mediumMarchEdgesM,
  piecedSteps,
  type PlanetTwin,
  planetTwin,
  rayMarchTwin,
  skyViewReadTwin,
  skyViewTexelsTwin,
  skyViewTwin,
  tableSources,
  type TwinRay,
  type TwinTable,
  type TwinView,
  transmittanceTablesTwin,
  twinView,
} from "./tablesCpu";

const RAD = Math.PI / 180;

/** WGS 84 (NIMA TR8350.2): the test planet's figure, and R₁ = (2a + c) ÷ 3, the tables' ground. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};
const R1 = tableRadiusM(WGS84);

/** The frames' pre-exposure, EV100 15, the smoke's. */
const EXPOSURE = exposureScale(15);

/** A small sky-view table for the identities, which hold at any size, and its steps. */
const SMALL: TableSize = { widthTexels: 24, heightTexels: 16 };
const SMALL_STEPS = 32;

/** A camera `heightM` above the equator at longitude 0, looking north along the horizon. */
function camera(heightM: number): AtmosphereCamera {
  return {
    positionM: { x: WGS84.equatorialRadiusM + heightM, y: 0, z: 0 },
    orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
    fovXRad: Math.PI / 2,
    viewport: { widthPx: 8, heightPx: 4 },
  };
}

/**
 * A direction from the equatorial camera at an elevation and an azimuth from north towards east:
 * the camera's normal is +x, north +z and east +y.
 */
function directionAt(elevationDeg: number, azimuthDeg: number): Vec3 {
  const e = elevationDeg * RAD;
  const a = azimuthDeg * RAD;
  return { x: Math.sin(e), y: Math.cos(e) * Math.sin(a), z: Math.cos(e) * Math.cos(a) };
}

/** A source at an elevation and azimuth with its illuminance and light slot. */
function sourceAt(
  elevationDeg: number,
  azimuthDeg: number,
  illuminanceLx: Rgb,
  curveSlot = 0,
): SunState {
  return {
    directionBodyFixed: directionAt(elevationDeg, azimuthDeg),
    illuminanceLx,
    // The Sun's angular radius at 1 au, R☉ ÷ au (IAU 2015 B3, IAU 2012 B2) rounded.
    angularRadiusRad: 0.004_65,
    curveSlot,
  };
}

/** The view a frame draws for `suns`, as `HillaireAtmosphere.drawFrame` fills it. */
function frameView(
  medium: AtmosphereMedium,
  lights: number,
  at: AtmosphereCamera,
  suns: ReadonlyArray<SunState>,
): TwinView {
  const absorbers = medium.terms.some((term) => term.absorber !== undefined);
  return frameTwinView(at, suns, WGS84, 0.1, EXPOSURE, absorbers, lights);
}

/** The same view with only some of its sources, on its own sky axis. */
function withSources(view: TwinView, indices: ReadonlyArray<number>): TwinView {
  return { ...view, sources: indices.flatMap((i) => view.sources[i] ?? []) };
}

/** The Rec. 709 luminance of a linear colour. */
function luminanceOf(rgb: ReadonlyArray<number>): number {
  return (
    0.2126 * (rgb[0] ?? Number.NaN) +
    0.7152 * (rgb[1] ?? Number.NaN) +
    0.0722 * (rgb[2] ?? Number.NaN)
  );
}

/** The half float's least normal, 2⁻¹⁴. */
const LEAST_NORMAL = 2 ** -14;

/** The mean of a table's colour values. */
function meanOf(table: TwinTable): number {
  let sum = 0;
  for (let i = 0; i < table.texels.length; i += 4) {
    sum += (table.texels[i] ?? 0) + (table.texels[i + 1] ?? 0) + (table.texels[i + 2] ?? 0);
  }
  return sum / ((table.texels.length / 4) * 3);
}

/** The largest colour value of a table's texels. */
function largestOf(table: TwinTable): number {
  let largest = 0;
  for (let i = 0; i < table.texels.length; i += 4) {
    for (let c = 0; c < 3; c += 1) {
      largest = Math.max(largest, table.texels[i + c] ?? 0);
    }
  }
  return largest;
}

/**
 * The worst |a − b| over max(|b|, floor × b's largest) over every colour channel of two tables of
 * one shape: a relative difference, floored where b is dark.
 */
function worstRelative(a: TwinTable, b: TwinTable, floor: number): number {
  const scale = floor * largestOf(b);
  let worst = 0;
  for (let i = 0; i < b.texels.length; i += 4) {
    for (let c = 0; c < 3; c += 1) {
      const x = a.texels[i + c] ?? Number.NaN;
      const y = b.texels[i + c] ?? Number.NaN;
      const e = Math.abs(x - y) / Math.max(Math.abs(y), scale);
      worst = Math.max(worst, Number.isNaN(e) ? Number.POSITIVE_INFINITY : e);
    }
  }
  return worst;
}

/** Two tables' texels summed, colour by colour; alpha the first's. */
function summed(a: TwinTable, b: TwinTable): TwinTable {
  const texels = a.texels.map((v, i) => (i % 4 === 3 ? v : v + (b.texels[i] ?? Number.NaN)));
  return { ...a, texels };
}

/** A table's colour scaled per channel. */
function scaledBy(table: TwinTable, factor: Rgb): TwinTable {
  const texels = table.texels.map((v, i) => (i % 4 === 3 ? v : v * (factor[i % 4] ?? Number.NaN)));
  return { ...table, texels };
}

describe("several suns summed in one march (R08.T7, Design note 7)", () => {
  let earth: PlanetTwin;
  const sun = sourceAt(30, 0, referenceSunIlluminanceLx());

  beforeAll(() => {
    earth = planetTwin(R05_EARTH_REFERENCE, R1);
  }, 120_000);

  it("doubles the sky with a second equal sun at the same direction", () => {
    const one = frameView(R05_EARTH_REFERENCE, 0, camera(2), [sun]);
    const two = frameView(R05_EARTH_REFERENCE, 0, camera(2), [sun, sun]);
    const sky = skyViewTwin(earth, one, SMALL, SMALL_STEPS);
    const doubled = scaledBy(sky, [2, 2, 2]);
    expect(worstRelative(skyViewTwin(earth, two, SMALL, SMALL_STEPS), doubled, 1e-9)).toBeLessThan(
      1e-12,
    );
    const volume = { widthTexels: 4, heightTexels: 4, slices: 4 };
    const aerialOne = aerialPerspectiveTwin(earth, one, volume, 2, 32_000);
    const aerialTwo = aerialPerspectiveTwin(earth, two, volume, 2, 32_000);
    for (let i = 0; i < aerialOne.texels.length; i += 4) {
      for (let c = 0; c < 3; c += 1) {
        expect(
          (aerialTwo.texels[i + c] ?? 0) / (2 * (aerialOne.texels[i + c] ?? 1)) - 1,
        ).toBeCloseTo(0, 12);
      }
      // The mean transmittance is the view's own, whatever lights it.
      expect(aerialTwo.texels[i + 3]).toBe(aerialOne.texels[i + 3]);
    }
  });

  it("weights a sun's light by its illuminance times the pre-exposure, the tables' unit", () => {
    const perUnit = skyViewTwin(earth, twinView(camera(2), sun, WGS84, 0.1), SMALL, SMALL_STEPS);
    const drawn = skyViewTwin(
      earth,
      frameView(R05_EARTH_REFERENCE, 0, camera(2), [sun]),
      SMALL,
      SMALL_STEPS,
    );
    const [r, g, b] = sun.illuminanceLx;
    const weights: Rgb = [r * EXPOSURE, g * EXPOSURE, b * EXPOSURE];
    expect(worstRelative(drawn, scaledBy(perUnit, weights), 1e-9)).toBeLessThan(1e-12);
    // Pre-exposed: about 3 at EV100 15 in each channel, the sky's own of order 0.1.
    expect(Math.min(...weights)).toBeGreaterThan(1);
    expect(largestOf(drawn)).toBeLessThan(10);
  });

  it("holds the source loop at MAX_SKY_SOURCES, each light where it is", () => {
    const suns = Array.from({ length: MAX_SKY_SOURCES }, (_, i) =>
      sourceAt(5 + 12 * i, 47 * i, [1 + i, 2 + i, 3 + i]),
    );
    const all = frameView(R05_EARTH_REFERENCE, 0, camera(2), suns);
    let total: TwinTable | null = null;
    for (let i = 0; i < MAX_SKY_SOURCES; i += 1) {
      const one = skyViewTwin(earth, withSources(all, [i]), SMALL, SMALL_STEPS);
      total = total === null ? one : summed(total, one);
    }
    if (total === null) {
      throw new Error("the loop drew a source");
    }
    expect(all.sources).toHaveLength(MAX_SKY_SOURCES);
    expect(worstRelative(skyViewTwin(earth, all, SMALL, SMALL_STEPS), total, 1e-9)).toBeLessThan(
      1e-12,
    );
  });
});

describe("a K5 primary and an M3 companion under ozone and methane (R08.T7; decision-r08-t6b-floor.md §8)", () => {
  // Planck spectra at 4,450 K and 3,300 K, each light's own curves of growth for the four
  // absorbers (ozone, methane with O₂, the O₂ pair, tropospheric ozone).
  const binary = fourAbsorberMedium([4_450, 3_300]);
  const primary = (elevationDeg: number): SunState =>
    sourceAt(elevationDeg, 0, [9.1e4, 8.0e4, 5.9e4], 0);
  const companion = (elevationDeg: number, slot = 1): SunState =>
    sourceAt(elevationDeg, 140, [2.9e3, 1.8e3, 0.8e3], slot);
  let planet: PlanetTwin;

  beforeAll(() => {
    planet = planetTwin(binary.medium, R1, binary.curves);
  }, 600_000);

  it("builds one multiple-scattering layer a light, each with its own curves", () => {
    expect(planet.multiScatteringBySlot).toHaveLength(2);
    expect(planet.multiScatteringBySlot?.[0]).toBe(planet.multiScattering);
    const [first, second] = planet.multiScatteringBySlot ?? [];
    expect(first?.texels).not.toEqual(second?.texels);
  });

  it("sums each star's single-sun sky, each with its own curves and layer, to 10⁻⁶", () => {
    const both = frameView(binary.medium, 2, camera(2), [primary(25), companion(12)]);
    const sky = skyViewTwin(planet, both, SMALL, SMALL_STEPS);
    const alone = summed(
      skyViewTwin(planet, withSources(both, [0]), SMALL, SMALL_STEPS),
      skyViewTwin(planet, withSources(both, [1]), SMALL, SMALL_STEPS),
    );
    expect(worstRelative(sky, alone, 1e-6)).toBeLessThan(1e-6);
    // The companion's own curves move its sky: on slot 0's it differs.
    const borrowed = skyViewTwin(
      planet,
      withSources(frameView(binary.medium, 2, camera(2), [primary(25), companion(12, 0)]), [1]),
      SMALL,
      SMALL_STEPS,
    );
    const own = skyViewTwin(planet, withSources(both, [1]), SMALL, SMALL_STEPS);
    expect(worstRelative(borrowed, own, 1e-6)).toBeGreaterThan(1e-4);
  });

  it.each([5, -3])(
    "records the companion's sky %d° up on slot 0's layer and curves against its own, the primary set",
    (elevationDeg) => {
      // The primary below the twilight limit, so the companion lights the sky alone.
      const set = -twilightLimitRad(WGS84, 2, binary.medium.topHeightM) / RAD - 1;
      const own = frameView(binary.medium, 2, camera(2), [primary(set), companion(elevationDeg)]);
      const ownSky = skyViewTwin(planet, withSources(own, [1]), SMALL, SMALL_STEPS);
      const borrowed = frameView(binary.medium, 2, camera(2), [
        primary(set),
        companion(elevationDeg, 0),
      ]);
      const borrowedSky = skyViewTwin(planet, withSources(borrowed, [1]), SMALL, SMALL_STEPS);
      const e = worstRelative(borrowedSky, ownSky, 1e-3);
      // Recorded in plan R08's Risks, "Deviations in T7, as built"; slot 0's curves were not adopted.
      expect(e).toBeGreaterThan(SLOT_ZERO_COMPANION[elevationDeg === 5 ? "up" : "below"][0]);
      expect(e).toBeLessThan(SLOT_ZERO_COMPANION[elevationDeg === 5 ? "up" : "below"][1]);
    },
  );
});

/**
 * The companion's sky on light slot 0's layer and curves against its own, worst relative over the
 * small table (floored at 10⁻³ of its largest), the primary set: 5.85% with the companion 5° up and
 * 12.06% 3° below the horizon (2026-10-13; 3.59% at 12° up with the primary 25° up). A bracket
 * about each, so that a change shows.
 */
const SLOT_ZERO_COMPANION = { up: [0.055, 0.062], below: [0.115, 0.126] } as const;

describe("the Sun and a full Moon on the rebuilt Earth (R08.T7, Design note 7)", () => {
  let planet: PlanetTwin;
  const sunLx = referenceSunIlluminanceLx();
  // A full Moon's illuminance, a test's tint about Krisciunas and Schaefer 1991's (PASP 103, 1033,
  // eqs. 8–9 at m = −12.73) 0.313 lx, without the opposition surge.
  const moonLx: Rgb = [0.3, 0.32, 0.33];
  const limitDeg = twilightLimitRad(WGS84, 2, EARTH_REFERENCE.topHeightM) / RAD;
  const lights = (sunElevationDeg: number): readonly [LightAtPoint, SecondarySource] => {
    const toStarM = {
      // 1 au, the IAU 2012 B2 value rounded.
      x: Math.sin(sunElevationDeg * RAD) * 1.496e11,
      y: 0,
      z: Math.cos(sunElevationDeg * RAD) * 1.496e11,
    };
    return [
      { host: { disc: aHostDisc(), centreM: toStarM }, toStarM, illuminance: sunLx },
      {
        body: "0200080020000000.0301",
        direction: directionAt(45, 0),
        // The Moon's mean radius and distance, NASA's Moon fact sheet: 1,737.4 km at 384,400 km.
        angularRadiusRad: Math.asin(1.7374e6 / 3.844e8),
        distanceM: 3.844e8,
        radiusM: 1.7374e6,
        illuminance: moonLx,
      },
    ];
  };

  beforeAll(() => {
    planet = planetTwin(EARTH_REFERENCE, R1, EARTH_REFERENCE_CURVES);
  }, 600_000);

  /** The sky's sources with the Sun 1° beyond the twilight limit and the full Moon at 45°. */
  const pastTheLimit = (): ReturnType<typeof skySources> => {
    const [star, moon] = lights(-limitDeg - 1);
    return skySources([star], [moon], camera(2), WGS84, EARTH_REFERENCE, quaternion(1, 0, 0, 0));
  };

  it("skips the Sun 1° beyond the twilight limit and keeps the full Moon", () => {
    expect(pastTheLimit().map((s) => s.kind)).toEqual(["planetshine"]);
  });

  it("draws the moonlit sky as the noon sky's twin at the Moon's direction, scaled per channel, to 10⁻⁶", () => {
    const drawn = pastTheLimit();
    const moonSource = drawn[0];
    if (moonSource === undefined) {
      throw new Error("the Moon is drawn");
    }
    const sky = skyViewTwin(
      planet,
      frameView(EARTH_REFERENCE, 1, camera(2), drawn),
      SMALL,
      SMALL_STEPS,
    );
    // The noon sky's twin: the Sun where the Moon is, at the Sun's own illuminance.
    const sunThere: SunState = { ...moonSource, illuminanceLx: sunLx };
    const noon = skyViewTwin(
      planet,
      frameView(EARTH_REFERENCE, 1, camera(2), [sunThere]),
      SMALL,
      SMALL_STEPS,
    );
    const ratio: Rgb = [
      moonSource.illuminanceLx[0] / sunLx[0],
      moonSource.illuminanceLx[1] / sunLx[1],
      moonSource.illuminanceLx[2] / sunLx[2],
    ];
    expect(worstRelative(sky, scaledBy(noon, ratio), 1e-6)).toBeLessThan(1e-6);
  });

  // R08.T7 asks under 10⁻⁶; the twin gives 2.42 × 10⁻⁵, all of it Hillaire's isotropic term at the
  // samples' own sun elevations (0 with the table zeroed): a deferred finding (plan R08's Risks).
  it("changes the moonlit sky by 2.4 × 10⁻⁵ of the Moon's with the Sun 1° beyond the limit kept, through multiple scattering alone", () => {
    const [star, moon] = lights(-limitDeg - 1);
    const listed = skySources(
      [],
      [moon],
      camera(2),
      WGS84,
      EARTH_REFERENCE,
      quaternion(1, 0, 0, 0),
    );
    const moonOnly = frameView(EARTH_REFERENCE, 1, camera(2), listed);
    // The Sun kept, by hand: carried to the camera as skySources carries it.
    const sun = sourceAt(-limitDeg - 1, 0, star.illuminance, 0);
    const kept = frameView(EARTH_REFERENCE, 1, camera(2), [...listed, sun]);
    const moonSky = skyViewTwin(planet, moonOnly, SMALL, SMALL_STEPS);
    const keptSky = skyViewTwin(planet, kept, SMALL, SMALL_STEPS);
    expect(worstRelative(keptSky, moonSky, 1e-3)).toBeLessThan(KEPT_SUN_CHANGE);
    // Single scattering adds nothing beyond the limit: with the table zeroed the skies are equal.
    const dark = new Float64Array(planet.multiScattering.texels.length);
    const single = { ...planet, multiScattering: { ...planet.multiScattering, texels: dark } };
    expect(skyViewTwin(single, kept, SMALL, SMALL_STEPS).texels).toEqual(
      skyViewTwin(single, moonOnly, SMALL, SMALL_STEPS).texels,
    );
  });

  it("skips the Moon by day, where the sky changes by under 10⁻⁴", () => {
    const [star, moon] = lights(30);
    const drawn = skySources(
      [star],
      [moon],
      camera(2),
      WGS84,
      EARTH_REFERENCE,
      quaternion(1, 0, 0, 0),
    );
    expect(drawn.map((s) => s.kind)).toEqual(["star"]);
    const sun = drawn[0];
    if (sun === undefined) {
      throw new Error("the Sun is drawn");
    }
    const moonSource = skySources(
      [],
      [moon],
      camera(2),
      WGS84,
      EARTH_REFERENCE,
      quaternion(1, 0, 0, 0),
    )[0];
    if (moonSource === undefined) {
      throw new Error("the Moon is drawn alone");
    }
    const day = skyViewTwin(
      planet,
      frameView(EARTH_REFERENCE, 1, camera(2), [sun]),
      SMALL,
      SMALL_STEPS,
    );
    const both = skyViewTwin(
      planet,
      frameView(EARTH_REFERENCE, 1, camera(2), [sun, moonSource]),
      SMALL,
      SMALL_STEPS,
    );
    expect(worstRelative(both, day, 1e-6)).toBeLessThan(1e-4);
  });

  // --- The per-view tables' range at twilight and by moonlight (decision-r08-t6b-floor.md §6) ---

  /** The scenes of the record: the Sun alone at −6°, −8°, −10° and −12°, and the moonlit night. */
  const rangeScenes = (): ReadonlyArray<readonly [string, ReadonlyArray<SunState>]> => {
    const [, moon] = lights(-limitDeg - 1);
    const moonlit = skySources(
      [],
      [moon],
      camera(2),
      WGS84,
      EARTH_REFERENCE,
      quaternion(1, 0, 0, 0),
    );
    return [
      ...[-6, -8, -10, -12].map((e) => [`the Sun at ${e}°`, [sourceAt(e, 0, sunLx)]] as const),
      ["the moonlit night", moonlit],
    ];
  };

  /** Unit directions over the upper hemisphere at the equatorial camera: a Fibonacci lattice. */
  const upward = Array.from({ length: 2_000 }, (_, i) => {
    const up = (i + 0.5) / 2_000;
    const phi = i * Math.PI * (3 - Math.sqrt(5));
    const across = Math.sqrt(1 - up * up);
    return { x: up, y: across * Math.sin(phi), z: across * Math.cos(phi) };
  });

  /**
   * A scene's sky at a pre-exposure of 1 (cd/m²) on high's sky-view steps at 48 × 27, its aerial
   * volume, and its ray march over a surface 60 km ahead in the lower half of the frame; with the
   * sky's mean luminance over the upper hemisphere, the meter's, and its least above the horizon.
   */
  const rangeOf = (suns: ReadonlyArray<SunState>) => {
    const high = TABLE_SIZES.high;
    const size = { widthTexels: 48, heightTexels: 27 };
    const at = { ...camera(2), viewport: { widthPx: 32, heightPx: 16 } };
    const view = frameTwinView(at, suns, WGS84, 0.1, 1, true, 1);
    const sky = skyViewTwin(planet, view, size, high.skyViewSamples);
    const texels = skyViewTexelsTwin(planet, view, size, high.skyViewSamples);
    const above = upward.map((d) => skyViewReadTwin(texels, size, EARTH_REFERENCE, view, d));
    const aerial = aerialPerspectiveTwin(
      planet,
      view,
      high.aerialPerspective,
      high.aerialPerspectiveSamplesPerSlice,
      high.aerialPerspectiveReachM,
    );
    const march = rayMarchTwin(planet, view, high, (_x, y) => (y < 8 ? 0 : 0.1 / 60_000));
    return {
      largest: { skyView: largestOf(sky), aerial: largestOf(aerial), rayMarch: largestOf(march) },
      meteredCdPerM2: above.reduce((sum, l) => sum + luminanceOf(l), 0) / above.length,
      leastAbove: Math.min(...above.flatMap((l) => [l[0], l[1], l[2]])),
    };
  };

  // Measured 2026-10-13 (high's 192 × 108 sky view within 5% of these at 48 × 27): the sky view's
  // largest stored value per unit of the Sun's green illuminance, as R05 stored it, at EV100 15
  // and at the metered exposure (EV100 = log₂(8 L̄), L̄ the sky's mean over the upper hemisphere),
  // in units of 2⁻¹⁴:
  //   −6°: 1.33, 4.44 and 1.8 × 10⁴ (EV100 3.0); −8°: 0.22, 0.72 and 1.5 × 10⁴ (0.65);
  //   −10°: 0.026, 0.085 and 5.4 × 10⁴ (−4.3); −12°: 0.0032, 0.011 and 3.5 × 10⁴ (−6.7);
  //   the moonlit night: 0.0061, 0.020 and 6.4 × 10³ (−3.3).
  // The aerial volume's at the metered exposure, 1.7–5.9 × 10³ (0.0003–0.12 per unit Sun); the ray
  // march's over the surface 60 km ahead, 11–38 (under 0.0011 per unit Sun). At the metered
  // exposure every table's largest lies far above 2⁻¹⁴ and under the half's 65,504, and the sky's
  // least above the horizon at 149–329 × 2⁻¹⁴.
  it("records each per-view table's largest stored value against 2⁻¹⁴ at twilight and by moonlight", () => {
    for (const [name, suns] of rangeScenes()) {
      const { largest, meteredCdPerM2, leastAbove } = rangeOf(suns);
      const metered = exposureScale(Math.log2(meteredCdPerM2 * 8));
      const perUnitSun = 1 / sunLx[1];
      // R05's storage: subnormal from −8°, and by moonlight, as Risks's entry has it.
      const stored = largest.skyView * perUnitSun;
      expect(stored < LEAST_NORMAL, `${name}: per unit Sun ${stored / LEAST_NORMAL} × 2⁻¹⁴`).toBe(
        name !== "the Sun at -6°",
      );
      for (const [table, value] of Object.entries(largest)) {
        const at = value * metered;
        expect(at, `${name}, ${table}: metered ${at / LEAST_NORMAL} × 2⁻¹⁴`).toBeGreaterThan(
          10 * LEAST_NORMAL,
        );
        expect(at).toBeLessThan(HALF_MAX);
      }
      // What a metered sky keeps above the horizon: its least texel there, recorded.
      expect(
        leastAbove * metered,
        `${name}: least above the horizon ${(leastAbove * metered) / LEAST_NORMAL} × 2⁻¹⁴`,
      ).toBeGreaterThan(LEAST_NORMAL);
    }
  }, 300_000);

  it("draws the moonlit sky within two stops of the moonlit ground", () => {
    const moonlit = rangeScenes().at(-1)?.[1];
    const moon = moonlit?.[0];
    if (moonlit === undefined || moon === undefined) {
      throw new Error("the moonlit night has the Moon");
    }
    const { meteredCdPerM2 } = rangeOf(moonlit);
    // The ground under the Moon at 45°: Lambertian, lit through the air's transmittance and by the
    // sky, π times its mean (a uniform sky's irradiance on a horizontal surface).
    const toMoon: [number, number, number] = [0, 0, 0];
    tableSources(planet).sunTransmittance(0, Math.SQRT1_2, toMoon, 0);
    const irradianceLx =
      moon.illuminanceLx[1] * toMoon[1] * Math.SQRT1_2 + Math.PI * meteredCdPerM2;
    const ground = (irradianceLx * EARTH_REFERENCE.groundAlbedo[1]) / Math.PI;
    // Measured 2026-10-13: the sky's mean 1.30 × 10⁻² cd/m² against the ground's 6.6 × 10⁻³ (the
    // Moon's 5.3 × 10⁻³ and the sky's 1.3 × 10⁻³), 1.0 stop brighter.
    expect(Math.abs(Math.log2(meteredCdPerM2 / ground))).toBeLessThan(2);
  }, 60_000);

  // The per-planet multiple-scattering table, per unit illuminance in rgba16float, cannot take a
  // frame's scale. Measured 2026-10-13 on the rebuilt Earth: 90% of its night side's channels (the
  // Sun below the horizon) lie under 2⁻¹⁴, the largest 5.1 × 10⁻³ at −1.8°. At the ground Ψ falls
  // from 2.5 × 10⁻⁴ at −3° to 6.0 × 10⁻⁶ at −6°, 5.5 × 10⁻⁷ at −9° and 1.2 × 10⁻⁸ at −12°, under
  // half of the half's least subnormal 2⁻²⁴, so stored as zero; at 60 km it is 1.7 × 10⁻⁵ at −12°.
  it("records the multiple-scattering table's night side against 2⁻¹⁴: most of its channels under it", () => {
    const ms = planet.multiScattering;
    let night = 0;
    let under = 0;
    for (let y = 0; y < ms.heightTexels; y += 1) {
      for (let x = 0; x < ms.widthTexels / 2; x += 1) {
        for (let c = 0; c < 3; c += 1) {
          night += 1;
          under +=
            (ms.texels[(y * ms.widthTexels + x) * 4 + c] ?? Number.NaN) < LEAST_NORMAL ? 1 : 0;
        }
      }
    }
    expect(under / night).toBeGreaterThan(0.85);
  });

  it("records the twilight sky the half float moves through the table's night side", () => {
    const ms = planet.multiScattering;
    // The table through the half float, as the GPU stores it, against f64, in the sky's mean.
    const halved: PlanetTwin = {
      ...planet,
      multiScattering: {
        ...ms,
        texels: ms.texels.map((v) => fromHalfBits(toHalfBits(v))),
      },
    };
    const moved = [-6, -9, -12].map((e) => {
      const view = frameView(EARTH_REFERENCE, 1, camera(2), [sourceAt(e, 0, sunLx)]);
      const exact = skyViewTwin(planet, view, SMALL, SMALL_STEPS);
      const stored = skyViewTwin(halved, view, SMALL, SMALL_STEPS);
      return meanOf(stored) / meanOf(exact) - 1;
    });
    // Recorded: the sky's mean moves by −0.02%, +0.73% and +8.9% at −6°, −9° and −12°, each
    // subnormal texel rounded to its step of 2⁻²⁴ either way and the least flushed to zero.
    expect(Math.abs(moved[0] ?? Number.NaN)).toBeLessThan(1e-3);
    expect(Math.abs(moved[2] ?? Number.NaN)).toBeGreaterThan(0.03);
  }, 120_000);
});

/**
 * The moonlit sky's largest change, relative and floored at 10⁻³ of its largest, with the Sun 1°
 * beyond the twilight limit kept in the sky loop: measured 2.42 × 10⁻⁵ (2026-10-13), against R08.T7's
 * 10⁻⁶, from the multiple-scattering table's night side; held under 3 × 10⁻⁵.
 */
const KEPT_SUN_CHANGE = 3e-5;

/** Directions within `withinDeg` of a direction: rings at the given angles, twelve to a ring. */
function ringsAbout(centre: Vec3, anglesDeg: ReadonlyArray<number>): Vec3[] {
  // Two unit vectors across the centre.
  const helper = Math.abs(centre.x) < 0.9 ? { x: 1, y: 0, z: 0 } : { x: 0, y: 1, z: 0 };

  const e1 = normalise(cross(centre, helper));
  const e2 = cross(centre, e1);
  const out: Vec3[] = [centre];
  for (const angleDeg of anglesDeg) {
    const a = angleDeg * RAD;
    for (let k = 0; k < 12; k += 1) {
      const phi = (k * Math.PI) / 6;
      out.push(
        normalise({
          x: Math.cos(a) * centre.x + Math.sin(a) * (Math.cos(phi) * e1.x + Math.sin(phi) * e2.x),
          y: Math.cos(a) * centre.y + Math.sin(a) * (Math.cos(phi) * e1.y + Math.sin(phi) * e2.y),
          z: Math.cos(a) * centre.z + Math.sin(a) * (Math.cos(phi) * e1.z + Math.sin(phi) * e2.z),
        }),
      );
    }
  }
  return out;
}

/** R05's Earth without its aerosol: a Rayleigh sky. */
const RAYLEIGH_SKY: AtmosphereMedium = {
  ...R05_EARTH_REFERENCE,
  name: "R05's Earth, Rayleigh alone",
  terms: R05_EARTH_REFERENCE.terms.filter((term) => term.phase.kind === "rayleigh"),
};

/** A view's sky frame: x its sky axis, y = normal × x, z the datum's normal. */
function inSkyFrame(view: TwinView, s: Vec3): Vec3 {
  const n = view.normal;
  const ax = view.skyAxis;
  const ay = cross(n, ax);
  return {
    x: s.x * ax.x + s.y * ax.y + s.z * ax.z,
    y: s.x * ay.x + s.y * ay.y + s.z * ay.z,
    z: s.x * n.x + s.y * n.y + s.z * n.z,
  };
}

/**
 * The sky view's march along one direction of its frame, as `skyView.wgsl` marches a texel's ray
 * (`tablesCpu.ts`'s `skyViewRayTwin`) but at any direction: the view's sources summed, on the
 * camera's own sphere, at `samples` steps cut at the medium's layer edges.
 */
function skyMarch(planet: PlanetTwin, view: TwinView, inFrame: Vec3, samples: number): Rgb {
  const medium = planet.medium;
  const bottom = view.cameraRadiusM;
  const rM = bottom + skyViewHeight(view.heightM, medium.topHeightM);
  const sources = view.sources.map((s) => ({ ...s, direction: inSkyFrame(view, s.direction) }));
  const shell = { bottomRadiusM: bottom, topRadiusM: bottom + medium.topHeightM };
  const ray: TwinRay = {
    originM: { x: 0, y: 0, z: rM },
    direction: inFrame,
    sun: sources[0]?.direction ?? { x: 0, y: 0, z: 1 },
    sources,
    tStartM: 0,
    tEndM: maxDistanceM(shell, rM, inFrame.z),
    fromCamera: true,
  };
  const geometry = { kind: "sphere", bottomRadiusM: bottom } as const;
  const steps = piecedSteps(ray, samples, mediumMarchEdgesM(medium), geometry);
  return marchTwin(medium, tableSources(planet), geometry, ray, steps).luminance;
}

/**
 * A one-source sky-view table in R05's layout, read in a direction: sebh's zenith rows and his
 * sun-relative azimuth u = sin(φ ÷ 2), φ in [0, π] from the source's azimuth, compressed towards
 * it (0.60° a column next to it at 192, against the summed table's even 1.885° over the circle),
 * each texel marched when first read, through Bruneton's texel mapping and filtered bilinearly:
 * the per-sun fallback of Design note 7, as R05 would build it for each source.
 */
function r05SkyView(
  planet: PlanetTwin,
  view: TwinView,
  size: TableSize,
  samples: number,
): (direction: Vec3) => Rgb {
  const bottom = view.cameraRadiusM;
  const heightM = skyViewHeight(view.heightM, planet.medium.topHeightM);
  const rM = bottom + heightM;
  const beta = Math.acos(Math.min(Math.sqrt(heightM * (2 * bottom + heightM)) / rM, 1));
  const zenithHorizon = Math.PI - beta;
  const { widthTexels: w, heightTexels: h } = size;
  const kept = new Map<number, Rgb>();
  const texelAt = (x: number, y: number): Rgb => {
    let texel = kept.get(y * w + x);
    if (texel === undefined) {
      const u = Math.min(Math.max(unitFromTexelUv((x + 0.5) / w, w), 0), 1);
      const v = Math.min(Math.max(unitFromTexelUv((y + 0.5) / h, h), 0), 1);
      const c = v < 0.5 ? 1 - 2 * v : v * 2 - 1;
      const zenith = v < 0.5 ? zenithHorizon * (1 - c * c) : zenithHorizon + beta * c * c;
      const phi = 2 * Math.asin(u);
      const inFrame = {
        x: Math.sin(zenith) * Math.cos(phi),
        y: Math.sin(zenith) * Math.sin(phi),
        z: Math.cos(zenith),
      };
      texel = skyMarch(planet, view, inFrame, samples);
      kept.set(y * w + x, texel);
    }
    return texel;
  };
  return (direction) => {
    const d = inSkyFrame(view, direction);
    const zenith = Math.atan2(Math.hypot(d.x, d.y), d.z);
    const v =
      zenith <= zenithHorizon
        ? (1 - Math.sqrt(Math.max(1 - zenith / zenithHorizon, 0))) * 0.5
        : Math.sqrt(Math.max((zenith - zenithHorizon) / beta, 0)) * 0.5 + 0.5;
    const across = Math.hypot(d.x, d.y);
    const u = across > 0 ? Math.hypot(d.x / across - 1, d.y / across) / 2 : 0;
    const px = Math.min(Math.max(texelUvFromUnit(Math.min(u, 1), w) * w - 0.5, 0), w - 1);
    const py = Math.min(Math.max(texelUvFromUnit(Math.min(v, 1), h) * h - 0.5, 0), h - 1);
    const x0 = Math.floor(px);
    const y0 = Math.floor(py);
    const fx = px - x0;
    const fy = py - y0;
    const a = texelAt(x0, y0);
    const b = texelAt(Math.min(x0 + 1, w - 1), y0);
    const c = texelAt(x0, Math.min(y0 + 1, h - 1));
    const e = texelAt(Math.min(x0 + 1, w - 1), Math.min(y0 + 1, h - 1));
    const filtered = (k: 0 | 1 | 2): number =>
      (a[k] * (1 - fx) + b[k] * fx) * (1 - fy) + (c[k] * (1 - fx) + e[k] * fx) * fy;
    return [filtered(0), filtered(1), filtered(2)];
  };
}

/** How the summed sky view near each source stands against R05's per-source tables and the march. */
interface SummedCheck {
  /** The worst |summed − Σ per-source| ÷ Σ per-source: Design note 7's 2%. */
  readonly againstPerSource: number;
  /** The worst |summed − the march along the direction| ÷ the march. */
  readonly summedAgainstMarch: number;
  /** The worst |Σ per-source − the march| ÷ the march. */
  readonly perSourceAgainstMarch: number;
}

/**
 * The summed sky view (the local azimuth about the first source, R08.T7) read within 10° of each
 * source, on a setting's table and steps, against the sum of R05's per-source tables
 * ({@link r05SkyView}) and the march along each direction.
 */
function summedAgainstPerSource(
  planet: PlanetTwin,
  suns: ReadonlyArray<SunState>,
  setting: "high" | "low",
): SummedCheck {
  const sizes = TABLE_SIZES[setting];
  const at = camera(2);
  const medium = planet.medium;
  const summedView = frameView(medium, 0, at, suns);
  const summedTexels = skyViewTexelsTwin(planet, summedView, sizes.skyView, sizes.skyViewSamples);
  const own = suns.map((sun) =>
    r05SkyView(planet, frameView(medium, 0, at, [sun]), sizes.skyView, sizes.skyViewSamples),
  );
  let againstPerSource = 0;
  let summedAgainstMarch = 0;
  let perSourceAgainstMarch = 0;
  for (const sun of suns) {
    for (const d of ringsAbout(sun.directionBodyFixed, [1, 2, 4, 7, 10])) {
      const s = skyViewReadTwin(summedTexels, sizes.skyView, medium, summedView, d);
      const p: [number, number, number] = [0, 0, 0];
      for (const read of own) {
        const one = read(d);
        p[0] += one[0];
        p[1] += one[1];
        p[2] += one[2];
      }
      const m = skyMarch(planet, summedView, inSkyFrame(summedView, d), sizes.skyViewSamples);
      for (const c of [0, 1, 2] as const) {
        againstPerSource = Math.max(againstPerSource, Math.abs(s[c] - p[c]) / p[c]);
        summedAgainstMarch = Math.max(summedAgainstMarch, Math.abs(s[c] - m[c]) / m[c]);
        perSourceAgainstMarch = Math.max(perSourceAgainstMarch, Math.abs(p[c] - m[c]) / m[c]);
      }
    }
  }
  return { againstPerSource, summedAgainstMarch, perSourceAgainstMarch };
}

describe("the summed sky view near each source (R08.T7, Design note 7's check)", () => {
  // The Sun and a second star at its own azimuth and elevation, and a third low in the sky.
  const suns = [
    sourceAt(30, 0, [1e5, 1e5, 1e5]),
    sourceAt(15, 117, [3e4, 3e4, 3e4]),
    sourceAt(4, 238, [1e4, 1e4, 1e4]),
  ];
  let rayleigh: PlanetTwin;
  let aerosol: PlanetTwin;

  beforeAll(() => {
    rayleigh = planetTwin(RAYLEIGH_SKY, R1);
    aerosol = planetTwin(R05_EARTH_REFERENCE, R1);
  }, 120_000);

  // Each sky's check on each setting, formed once for the tests that read it.
  const checks = new Map<string, SummedCheck>();
  const checkOf = (
    sky: "a Rayleigh sky" | "Earth's aerosol",
    setting: "high" | "low",
  ): SummedCheck => {
    const key = `${sky} ${setting}`;
    let check = checks.get(key);
    if (check === undefined) {
      check = summedAgainstPerSource(sky === "a Rayleigh sky" ? rayleigh : aerosol, suns, setting);
      checks.set(key, check);
    }
    return check;
  };

  // Measured 2026-10-13 against R05's per-source tables: on high 6.4 × 10⁻⁵ (Rayleigh) and
  // 9.2 × 10⁻⁴ (aerosol), on low 1.4 × 10⁻⁴ and 2.0 × 10⁻³. So the per-sun fallback is not built.
  // Against the march along each direction both layouts err alike, 1.1% on high and 5.4% on low,
  // where the 7° and 10° rings about the 4° source cross the horizon's coarse rows (R05's v).
  it.each([
    ["a Rayleigh sky", "high"],
    ["Earth's aerosol", "high"],
    ["a Rayleigh sky", "low"],
    ["Earth's aerosol", "low"],
  ] as const)(
    "agrees with R05's per-source tables to 2% within 10° of each source, on %s, on %s",
    (sky, setting) => {
      expect(checkOf(sky, setting).againstPerSource).toBeLessThan(0.02);
    },
  );

  it("errs no more against the march than R05's per-source tables do, to 0.1%", () => {
    for (const sky of ["a Rayleigh sky", "Earth's aerosol"] as const) {
      for (const setting of ["high", "low"] as const) {
        const check = checkOf(sky, setting);
        expect(check.summedAgainstMarch).toBeLessThan(check.perSourceAgainstMarch + 1e-3);
      }
    }
  });
});

/** The scene the counting engine's frames are laid over. */
async function sceneOf(
  engine: Awaited<ReturnType<typeof countingRenderEngine>>,
): Promise<AtmosphereScene> {
  const target = engine.createRenderTarget({
    name: "scene",
    size: { widthPx: 64, heightPx: 32 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const depth = target.depth;
  if (depth === null) {
    throw new Error("the scene target has depth");
  }
  return { colour: target.colour, depth, nearM: 0.1, exposureScale: EXPOSURE };
}

describe("the per-view tables and several suns (R08.T7)", () => {
  const view: AtmosphereCamera = { ...camera(2), viewport: { widthPx: 64, heightPx: 32 } };
  const suns = Array.from({ length: MAX_SKY_SOURCES }, (_, i) =>
    sourceAt(10 + 10 * i, 60 * i, [1e4, 1e4, 1e4]),
  );

  it("do not grow with the number of suns", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const scene = await sceneOf(engine);
    atmosphere.drawFrame(view, suns.slice(0, 1), scene);
    const textures = engine.counts.textures;
    const frameBytes = (): number =>
      engine.textureSpecs
        .filter((spec) => spec.category === "atmosphere-view")
        .reduce((sum, spec) => {
          const size: ReadonlyArray<number> = Array.isArray(spec.size) ? spec.size : [];
          return sum + size.reduce((a, b) => a * b, 1);
        }, 0);
    const bytes = frameBytes();
    atmosphere.drawFrame(view, suns, scene);
    expect([engine.counts.textures, frameBytes()]).toEqual([textures, bytes]);
  });

  it("are equal for two views with the same camera", async () => {
    const engine = await countingRenderEngine();
    const scene = await sceneOf(engine);
    const blocks = [0, 1].map(() => {
      const atmosphere = new HillaireAtmosphere(
        engine,
        R05_EARTH_REFERENCE,
        TABLE_SIZES.high,
        WGS84,
      );
      engine.dispatched.length = 0;
      atmosphere.drawFrame(view, suns, scene);
      return engine.dispatched.map((d) => Array.from(d.bindings.uniforms["view"] ?? []));
    });
    expect(blocks[1]).toEqual(blocks[0]);
    // And their twins, bit for bit.
    const planet = planetTwin(R05_EARTH_REFERENCE, R1);
    const [a, b] = [0, 1].map(() =>
      skyViewTwin(planet, frameView(R05_EARTH_REFERENCE, 0, view, suns), SMALL, SMALL_STEPS),
    );
    expect(a?.texels).toEqual(b?.texels);
  }, 60_000);
});

/**
 * The worst Δu′v′ over R06's twelve suns between a source's light through `air` at a zenith angle
 * and its three-channel transmittance at the fitted triple: the star's colour times T at each
 * channel's wavelength, as the per-frame tables draw it.
 */
function worstBeam(air: SpectralAir, sunZenithDeg: number): number {
  const response = (nm: number): number => viewResponse(air, { kind: "sun", sunZenithDeg }, nm);
  const spectral = PIPELINE_WAVELENGTHS_NM.map(response);
  const channels: Rgb = [
    response(CHANNEL_WAVELENGTHS_NM[0]),
    response(CHANNEL_WAVELENGTHS_NM[1]),
    response(CHANNEL_WAVELENGTHS_NM[2]),
  ];
  return Math.max(
    ...bakeSpectra.suns.map((row) => {
      const weights = sunWeights(bakeSpectrumOf(row.bake_spectrum));
      const [r, g, b] = weights.rgb;
      return deltaUv(weightedRgb(weights, spectral), [
        r * channels[0],
        g * channels[1],
        b * channels[2],
      ]);
    }),
  );
}

/** Air `n` Earth columns deep: each term's coefficients times n. */
function deeper(air: SpectralAir, n: number): SpectralAir {
  return {
    ...air,
    terms: air.terms.map((term) => ({
      ...term,
      scatteringPerM: (nm: number) => n * term.scatteringPerM(nm),
      absorptionPerM: (nm: number) => n * term.absorptionPerM(nm),
    })),
  };
}

/** Dry air `n` Earth columns deep: R08.T4.a's fit column, its coefficients times n. */
function dryAir(n: number): SpectralAir {
  return molecularAir("Earth", EARTH_FIT_COLUMN, EARTH_DRY_AIR, n);
}

describe("starlight and the scene through the air (R08.T7)", () => {
  it("dims a star at the zenith from Earth's ground by e^(−τ) of the column", () => {
    const planet: PlanetTwin = {
      medium: EARTH_REFERENCE,
      bottomRadiusM: R1,
      ...transmittanceTablesTwin(EARTH_REFERENCE, R1, EARTH_REFERENCE_CURVES),
      multiScattering: { widthTexels: 2, heightTexels: 2, texels: new Float64Array(16) },
    };
    const out: [number, number, number] = [0, 0, 0];
    tableSources(planet).sunTransmittance(0, 1, out, 0);
    // The column's optical depth by the oracle, the ozone's by its curve at its vertical column.
    const tau = opticalDepthToTop(EARTH_REFERENCE, R1, R1, 1);
    const ozone = EARTH_REFERENCE_CURVES[0]?.[0];
    if (ozone === undefined) {
      throw new Error("the rebuilt Earth's ozone has a curve");
    }
    const at = ozone.logColumns.findIndex(
      (x, j) => j > 0 && x > (ozone.logColumns[0] ?? 0) + 3 * Math.LN10 - 1e-9,
    );
    for (const c of [0, 1, 2] as const) {
      const absorbed = ozone.opticalDepth[c][at] ?? Number.NaN;
      expect(out[c] / Math.exp(-(tau[c] + absorbed)) - 1).toBeCloseTo(0, 2);
    }
  });

  const code = COMPOSITE_MATERIAL.fragmentWgsl.replaceAll(/\/\/[^\n]*/gu, "");

  it("multiplies what the scene holds behind the sky by its transmittance to space, a disc's own light's inside it", () => {
    expect(code).toMatch(/behind = transmittanceToSpace\(v, body, dir, slotHere\);/u);
    expect(code).toMatch(/var luminance = colour \* behind \+ inscattered \* factor;/u);
    expect(code).toMatch(
      /if \(withinDisc\(dir, v\.sources\[k\]\)\) \{\s*slotHere = skySlotOf\(v\.sources\[k\]\);/u,
    );
    expect(code).toMatch(/toSpace = transmittanceToSpace\(v, body, dir, skySlotOf\(source\)\);/u);
  });

  it("keeps the scene's METER_CLASS in the HDR target's alpha on every path", () => {
    expect(code).toMatch(/let meterClass = scene\.a;/u);
    const returns = [...code.matchAll(/return vec4f\(([^;]*)\);/gu)].map((m) => m[1] ?? "");
    const fragment = code.slice(code.indexOf("fn fragmentMain"));
    const fragmentReturns = [...fragment.matchAll(/return vec4f\(([^;]*)\);/gu)].map(
      (m) => m[1] ?? "",
    );
    expect(fragmentReturns).toHaveLength(3);
    for (const value of fragmentReturns) {
      expect(value).toMatch(/, meterClass$/u);
    }
    expect(returns.length).toBeGreaterThanOrEqual(fragmentReturns.length);
  });

  // Measured 2026-10-13, the worst over the twelve suns: dry air at a sun zenith of 85°, 0.0106,
  // 0.0151 and 0.1000 at 2, 3 and 10 columns (R08.T4.a's, reproduced); with R05's aerosol,
  // 0.0146, 0.0293 and 0.141; at the zenith, dry air's 0.0025, 0.0036 and 0.0073. So beyond two
  // or three Earth columns a low sun's three-channel beam is off by more than 0.01, and such a
  // medium's scattered light is the spectral bakes' (Design note 5).
  it("records the colour error of a source's three-channel transmittance two, three and ten Earth columns deep", () => {
    const low = [2, 3, 10].map((n) => worstBeam(dryAir(n), 85));
    expect(low[0]).toBeCloseTo(0.0106, 4);
    expect(low[1]).toBeCloseTo(0.0151, 4);
    expect(low[2]).toBeCloseTo(0.1, 3);
    const hazy = [2, 3, 10].map((n) => worstBeam(deeper(earthFitAir(), n), 85));
    expect(hazy.every((e, i) => e > (low[i] ?? Number.NaN))).toBe(true);
    const overhead = [2, 3, 10].map((n) => worstBeam(dryAir(n), 0));
    expect(Math.max(...overhead)).toBeLessThan(0.01);
  }, 120_000);
});
