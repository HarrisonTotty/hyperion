import { describe, expect, it } from "vitest";

import { EARTH_MASS_KG } from "../../lib/system/bodiesWire";
import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "../../lib/system/constants";
import { tableRadiusM } from "./hillaire";
import {
  BAND_STEP,
  bodyGravity,
  type BodyGravityInput,
  directionalCurvatureRadiusM,
  gravityRatio,
  KAPPA_STEP,
  type LevelSpheroid,
  normalGravity,
  oblateSlicing,
  ONE_SLICE_BELOW,
  referenceGravity,
} from "./oblate";

const DEG = Math.PI / 180;
const HOUR_S = 3_600;

/**
 * WGS 84's four defining parameters (NIMA TR8350.2, Third Edition, 4 July 1997, Amendment 1,
 * 3 January 2000, Table 3.1): a, 1 ÷ f, GM with the atmosphere, and ω.
 */
const WGS84: LevelSpheroid = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257_223_563),
  gmM3S2: 3.986_004_418e14,
  angularVelocityRadS: 7.292_115e-5,
};

/** WGS 84's normal gravity at the equator and the pole, m s⁻² (NIMA TR8350.2, Table 3.4). */
const WGS84_GAMMA_E = 9.780_325_335_9;
const WGS84_GAMMA_P = 9.832_184_937_8;

/**
 * Saturn at 1 bar, as the plan's test gives it: a and c of Archinal et al. 2018 (Celest. Mech.
 * Dyn. Astron. 130, 22), GM of Jacobson et al. 2006 (AJ 132, 2520) to five figures, and 10.656 h,
 * System III's 810.7939024° d⁻¹ (Archinal et al. 2018), 10.6562 h, rounded, from Voyager's
 * 10 h 39 min 24 s ± 7 s (Desch and Kaiser 1981, GRL 8, 253).
 */
const SATURN: LevelSpheroid = {
  equatorialRadiusM: 60_268e3,
  polarRadiusM: 54_364e3,
  gmM3S2: 3.793_1e16,
  angularVelocityRadS: (2 * Math.PI) / (10.656 * HOUR_S),
};

/**
 * Jupiter at 1 bar: a and c of Archinal et al. 2018, the nominal GM of IAU 2015 Resolution B3 (the
 * simulation's `GM_JUPITER`), and System III's 870.536° d⁻¹ (Archinal et al. 2018), 9.925 h.
 */
const JUPITER: LevelSpheroid = {
  equatorialRadiusM: 71_492e3,
  polarRadiusM: 66_854e3,
  gmM3S2: 1.266_865_3e17,
  angularVelocityRadS: (870.536 * DEG) / 86_400,
};

/**
 * Uranus at 1 bar: a and c of Archinal et al. 2018, GM of Jacobson 2014 (AJ 148, 76) to four
 * figures, and the rotation of 501.1600928° d⁻¹, retrograde, 17.24 h (Archinal et al. 2018, from
 * Voyager's radio emission: Desch, Connerney and Kaiser 1986, Nature 322, 42).
 */
const URANUS: LevelSpheroid = {
  equatorialRadiusM: 25_559e3,
  polarRadiusM: 24_973e3,
  gmM3S2: 5.794e15,
  angularVelocityRadS: (501.160_092_8 * DEG) / 86_400,
};

/** A non-rotating sphere of Earth's mean radius and GM. */
const STILL_SPHERE: LevelSpheroid = {
  equatorialRadiusM: 6_371_000,
  polarRadiusM: 6_371_000,
  gmM3S2: 3.986_004_418e14,
  angularVelocityRadS: 0,
};

/** WGS 84 as a record gives it: its mass by the simulation's G, and a bulk gravity. */
const EARTH_INPUT: BodyGravityInput = {
  figure: WGS84,
  massKg: WGS84.gmM3S2 / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2,
  angularVelocityRadS: WGS84.angularVelocityRadS,
  bulkGravityMS2: 9.82,
};

/**
 * A round body with no rotation section and its bulk gravity: round, since the wire sends no
 * figure without a rotation.
 */
const NO_ROTATION_INPUT: BodyGravityInput = {
  figure: STILL_SPHERE,
  massKg: STILL_SPHERE.gmM3S2 / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2,
  angularVelocityRadS: null,
  bulkGravityMS2: 9.81,
};

const LATITUDES_DEG = [0, 10, 23.5, 45, 60, 75, 89, 90];

function relative(value: number, expected: number): number {
  return Math.abs(value / expected - 1);
}

/** The meridional and prime-vertical radii, m, hand-computed from a and c. */
function meridianAndPrimeVertical(
  body: LevelSpheroid,
  latitudeRad: number,
): readonly [number, number] {
  const a = body.equatorialRadiusM;
  const c = body.polarRadiusM;
  const e2 = 1 - (c * c) / (a * a);
  const w = Math.sqrt(1 - e2 * Math.sin(latitudeRad) ** 2);
  return [(a * (1 - e2)) / w ** 3, a / w];
}

function slicingCounts(body: LevelSpheroid): readonly [number, number] {
  const { kappa, bandGravityRatio } = oblateSlicing(body, tableRadiusM(body));
  return [kappa.length, bandGravityRatio.length];
}

/** s at the equator and the pole, the extremes of a body whose gravity rises poleward. */
function equatorAndPoleRatios(body: LevelSpheroid): readonly [number, number] {
  const g = referenceGravity(body);
  return [normalGravity(body, 0) / g, normalGravity(body, Math.PI / 2) / g];
}

/** κ_min = s_e M_e ÷ R_ref and κ_max = s_p (a² ÷ c) ÷ R_ref. */
function kappaExtremes(body: LevelSpheroid): readonly [number, number] {
  const r = tableRadiusM(body);
  const [sEquator, sPole] = equatorAndPoleRatios(body);
  const [mEquator] = meridianAndPrimeVertical(body, 0);
  const a2OverC = body.equatorialRadiusM ** 2 / body.polarRadiusM;
  return [(sEquator * mEquator) / r, (sPole * a2OverC) / r];
}

/** The steps between neighbours, in their logarithm. */
function logGaps(values: Float64Array): number[] {
  const gaps: number[] = [];
  for (let i = 1; i < values.length; i += 1) {
    gaps.push(Math.log((values[i] ?? Number.NaN) / (values[i - 1] ?? Number.NaN)));
  }
  return gaps;
}

describe("normalGravity", () => {
  it("gives WGS 84's γ_e and γ_p of NIMA TR8350.2 to 10⁻⁹", () => {
    expect(relative(normalGravity(WGS84, 0), WGS84_GAMMA_E)).toBeLessThan(1e-9);
    expect(relative(normalGravity(WGS84, Math.PI / 2), WGS84_GAMMA_P)).toBeLessThan(1e-9);
  });

  it("gives Saturn's 9.08 and 12.04 m s⁻² at the equator and the pole to 0.5%", () => {
    expect(relative(normalGravity(SATURN, 0), 9.08)).toBeLessThan(0.005);
    expect(relative(normalGravity(SATURN, Math.PI / 2), 12.04)).toBeLessThan(0.005);
  });

  it("gives Jupiter's 23.12 and 26.98 m s⁻² at the equator and the pole to 0.5%", () => {
    expect(relative(normalGravity(JUPITER, 0), 23.12)).toBeLessThan(0.005);
    expect(relative(normalGravity(JUPITER, Math.PI / 2), 26.98)).toBeLessThan(0.005);
  });

  it("is GM ÷ R² at every latitude of a sphere that does not turn", () => {
    for (const latitude of LATITUDES_DEG) {
      expect(
        relative(normalGravity(STILL_SPHERE, latitude * DEG), 3.986_004_418e14 / 6_371_000 ** 2),
      ).toBeLessThan(1e-15);
    }
  });

  it("is the same north and south of the equator", () => {
    for (const latitude of LATITUDES_DEG) {
      expect(normalGravity(SATURN, -latitude * DEG)).toBe(normalGravity(SATURN, latitude * DEG));
    }
  });

  it("joins its series and closed forms smoothly where they meet, at e′ = 0.25", () => {
    const a = 1e7;
    for (const latitude of [0, 90]) {
      const at = (ePrime: number): number =>
        normalGravity(
          {
            equatorialRadiusM: a,
            polarRadiusM: a / Math.sqrt(1 + ePrime * ePrime),
            gmM3S2: 1e17,
            angularVelocityRadS: 1e-4,
          },
          latitude * DEG,
        );
      expect(relative(at(0.25 - 1e-13), at(0.25 + 1e-13))).toBeLessThan(1e-12);
    }
  });

  it("refuses a polar radius above the equatorial one", () => {
    expect(() => normalGravity({ ...WGS84, polarRadiusM: 6_400_000 }, 0)).toThrow(RangeError);
  });

  it("refuses radii that are not finite and positive", () => {
    expect(() => normalGravity({ ...WGS84, polarRadiusM: 0 }, 0)).toThrow(RangeError);
    expect(() => normalGravity({ ...WGS84, equatorialRadiusM: Number.NaN }, 0)).toThrow(RangeError);
  });

  it("refuses a GM that is not finite and positive", () => {
    expect(() => normalGravity({ ...WGS84, gmM3S2: 0 }, 0)).toThrow(RangeError);
    expect(() => normalGravity({ ...WGS84, gmM3S2: Number.POSITIVE_INFINITY }, 0)).toThrow(
      RangeError,
    );
  });

  it("refuses an ω that is not finite", () => {
    expect(() => normalGravity({ ...WGS84, angularVelocityRadS: Number.NaN }, 0)).toThrow(
      RangeError,
    );
  });

  it("refuses a spin that takes the equator past breakup", () => {
    expect(() => normalGravity({ ...WGS84, angularVelocityRadS: 1e-2 }, 0)).toThrow(RangeError);
  });
});

describe("referenceGravity", () => {
  it("is √(γ_e γ_p)", () => {
    expect(
      relative(referenceGravity(WGS84), Math.sqrt(WGS84_GAMMA_E * WGS84_GAMMA_P)),
    ).toBeLessThan(1e-9);
  });
});

describe("directionalCurvatureRadiusM", () => {
  it("is M along the meridian", () => {
    for (const body of [WGS84, SATURN]) {
      for (const latitude of LATITUDES_DEG) {
        const [m] = meridianAndPrimeVertical(body, latitude * DEG);
        expect(relative(directionalCurvatureRadiusM(body, latitude * DEG, 0), m)).toBeLessThan(
          1e-12,
        );
      }
    }
  });

  it("is N along the prime vertical", () => {
    for (const body of [WGS84, SATURN]) {
      for (const latitude of LATITUDES_DEG) {
        const [, n] = meridianAndPrimeVertical(body, latitude * DEG);
        expect(
          relative(directionalCurvatureRadiusM(body, latitude * DEG, Math.PI / 2), n),
        ).toBeLessThan(1e-12);
      }
    }
  });

  it("lies between M and N at every azimuth", () => {
    for (const body of [WGS84, SATURN]) {
      for (const latitude of LATITUDES_DEG) {
        const [m, n] = meridianAndPrimeVertical(body, latitude * DEG);
        for (let azimuthDeg = 0; azimuthDeg < 360; azimuthDeg += 15) {
          const r = directionalCurvatureRadiusM(body, latitude * DEG, azimuthDeg * DEG);
          expect(r).toBeGreaterThanOrEqual(m * (1 - 1e-12));
          expect(r).toBeLessThanOrEqual(n * (1 + 1e-12));
        }
      }
    }
  });

  it("is a² ÷ c at every azimuth at the pole", () => {
    for (const body of [WGS84, SATURN]) {
      const a2OverC = body.equatorialRadiusM ** 2 / body.polarRadiusM;
      for (let azimuthDeg = 0; azimuthDeg < 360; azimuthDeg += 15) {
        expect(
          relative(directionalCurvatureRadiusM(body, Math.PI / 2, azimuthDeg * DEG), a2OverC),
        ).toBeLessThan(1e-12);
      }
    }
  });

  it("refuses a polar radius above the equatorial one", () => {
    expect(() =>
      directionalCurvatureRadiusM({ equatorialRadiusM: 1, polarRadiusM: 2 }, 0, 0),
    ).toThrow(RangeError);
  });
});

describe("oblateSlicing", () => {
  it("gives 1 slice and 1 band for Earth, 2 and 1 for Uranus, 4 and 3 for Jupiter, 5 and 4 for Saturn", () => {
    expect([WGS84, URANUS, JUPITER, SATURN].map(slicingCounts)).toEqual([
      [1, 1],
      [2, 1],
      [4, 3],
      [5, 4],
    ]);
  });

  it("gives a sphere that does not turn one slice at κ = 1 and one band at s = 1", () => {
    const { kappa, bandGravityRatio } = oblateSlicing(STILL_SPHERE, tableRadiusM(STILL_SPHERE));
    expect([...kappa, ...bandGravityRatio]).toEqual([1, 1]);
  });

  it("serves Earth's whole range of κ with one slice, under ONE_SLICE_BELOW", () => {
    const [kappaMin, kappaMax] = kappaExtremes(WGS84);
    expect(Math.log(kappaMax / kappaMin)).toBeLessThan(ONE_SLICE_BELOW);
  });

  it("ends Saturn's slices at the extremes of κ", () => {
    const { kappa } = oblateSlicing(SATURN, tableRadiusM(SATURN));
    const [kappaMin, kappaMax] = kappaExtremes(SATURN);
    expect(relative(kappa[0] ?? Number.NaN, kappaMin)).toBeLessThan(1e-12);
    expect(relative(kappa.at(-1) ?? Number.NaN, kappaMax)).toBeLessThan(1e-12);
  });

  it("spaces Saturn's slices upward within KAPPA_STEP", () => {
    const gaps = logGaps(oblateSlicing(SATURN, tableRadiusM(SATURN)).kappa);
    expect(Math.min(...gaps)).toBeGreaterThan(0);
    expect(Math.max(...gaps)).toBeLessThanOrEqual(KAPPA_STEP);
  });

  it("ends Saturn's bands at the equator's and the pole's s", () => {
    const { bandGravityRatio } = oblateSlicing(SATURN, tableRadiusM(SATURN));
    const [sEquator, sPole] = equatorAndPoleRatios(SATURN);
    expect(relative(bandGravityRatio[0] ?? Number.NaN, sEquator)).toBeLessThan(1e-12);
    expect(relative(bandGravityRatio.at(-1) ?? Number.NaN, sPole)).toBeLessThan(1e-12);
  });

  it("spaces Saturn's bands upward within BAND_STEP", () => {
    const gaps = logGaps(oblateSlicing(SATURN, tableRadiusM(SATURN)).bandGravityRatio);
    expect(Math.min(...gaps)).toBeGreaterThan(0);
    expect(Math.max(...gaps)).toBeLessThanOrEqual(BAND_STEP);
  });

  it("refuses an R_ref that is not finite and positive", () => {
    expect(() => oblateSlicing(SATURN, 0)).toThrow(RangeError);
    expect(() => oblateSlicing(SATURN, Number.NaN)).toThrow(RangeError);
  });
});

describe("bodyGravity", () => {
  it("builds the level spheroid from the record's mass by the simulation's G", () => {
    const gravity = bodyGravity({ ...EARTH_INPUT, massKg: EARTH_MASS_KG }, tableRadiusM(WGS84));
    expect(relative(gravity.spheroid?.gmM3S2 ?? Number.NaN, 3.986_004e14)).toBeLessThan(1e-15);
  });

  it("is built at √(γ_e γ_p)", () => {
    const gravity = bodyGravity(EARTH_INPUT, tableRadiusM(WGS84));
    expect(relative(gravity.referenceGravityMS2, referenceGravity(WGS84))).toBeLessThan(1e-12);
  });

  it("takes the bulk gravity as g_ref with no rotation section", () => {
    const gravity = bodyGravity(NO_ROTATION_INPUT, tableRadiusM(STILL_SPHERE));
    expect([gravity.spheroid, gravity.referenceGravityMS2]).toEqual([null, 9.81]);
  });

  it("gives one slice and one band with no rotation section", () => {
    const { slicing } = bodyGravity(NO_ROTATION_INPUT, tableRadiusM(STILL_SPHERE));
    expect([...slicing.kappa, ...slicing.bandGravityRatio]).toEqual([1, 1]);
  });

  it("refuses a mass that is not finite and positive when the body turns", () => {
    expect(() => bodyGravity({ ...EARTH_INPUT, massKg: 0 }, tableRadiusM(WGS84))).toThrow(
      RangeError,
    );
  });

  it("refuses a bulk gravity that is not finite and positive with no rotation section", () => {
    expect(() =>
      bodyGravity({ ...NO_ROTATION_INPUT, bulkGravityMS2: Number.NaN }, tableRadiusM(STILL_SPHERE)),
    ).toThrow(RangeError);
  });
});

describe("gravityRatio", () => {
  it("scales by g(φ) ÷ g_ref", () => {
    const gravity = bodyGravity(EARTH_INPUT, tableRadiusM(WGS84));
    for (const latitude of LATITUDES_DEG) {
      expect(
        relative(
          gravityRatio(gravity, latitude * DEG),
          normalGravity(WGS84, latitude * DEG) / referenceGravity(WGS84),
        ),
      ).toBeLessThan(1e-12);
    }
  });

  it("is 1 at every latitude of a sphere that does not turn", () => {
    const gravity = bodyGravity(
      { ...NO_ROTATION_INPUT, angularVelocityRadS: 0 },
      tableRadiusM(STILL_SPHERE),
    );
    for (const latitude of LATITUDES_DEG) {
      expect(Math.abs(gravityRatio(gravity, latitude * DEG) - 1)).toBeLessThan(1e-15);
    }
  });

  it("is 1 at every latitude with no rotation section", () => {
    const gravity = bodyGravity(NO_ROTATION_INPUT, tableRadiusM(STILL_SPHERE));
    for (const latitude of LATITUDES_DEG) {
      expect(gravityRatio(gravity, latitude * DEG)).toBe(1);
    }
  });
});
