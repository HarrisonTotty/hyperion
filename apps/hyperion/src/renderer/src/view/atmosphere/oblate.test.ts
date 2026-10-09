import { describe, expect, it } from "vitest";

import { EARTH_MASS_KG } from "../../lib/system/bodiesWire";
import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "../../lib/system/constants";
import { type SpheroidFigure, tableRadiusM } from "./hillaire";
import {
  BAND_STEP,
  bodyGravity,
  type BodyGravity,
  type BodyGravityInput,
  darwinRadauSpinRadS,
  directionalCurvatureRadiusM,
  type FigureLawInput,
  gravityRatio,
  KAPPA_STEP,
  type LevelSpheroid,
  normalGravity,
  oblateSlicing,
  ONE_BAND_BELOW,
  ONE_SLICE_BELOW,
  referenceGravity,
} from "./oblate";

const DEG = Math.PI / 180;
const HOUR_S = 3_600;
const DAY_S = 86_400;

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
  figureLaw: null,
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
  figureLaw: null,
};

/** Jupiter's mass, kg: IAU 2015 Resolution B3's nominal GM (`JUPITER`'s) by the simulation's G. */
const JUPITER_MASS_KG = 1.266_865_3e17 / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2;

/** Jupiter's volumetric mean radius, m (NASA's Jupiter fact sheet, 69,911 km). */
const JUPITER_RADIUS_M = 69_911e3;

/** Plan 14's flattening cap (`FLATTENING_CAP`, `planetary/params.rs`). */
const FLATTENING_CAP = 0.2;

/** A figure as plan 14 builds it, with how it came out. */
interface PlanFourteenFigure {
  readonly figure: SpheroidFigure;
  readonly gmM3S2: number;
  readonly capped: boolean;
}

/**
 * The figure plan 14 gives a body of volumetric radius `radiusM` spinning at ω with moment of
 * inertia C ÷ Ma²: `planetary/derive/figure.rs`'s `hydrostatic_flattening` (P14.T46.c), ported.
 *
 * @remarks
 * From a = R, each pass takes q = ω²a³ ÷ GM, the Darwin–Radau f = (5 ÷ 2) q ÷ [1 + (25 ÷ 4)(1 −
 * (3 ÷ 2) C ÷ Ma²)²], times 2.5 for a synchronous body (`SYNCHRONOUS_TIDAL_FACTOR`), held at
 * {@link FLATTENING_CAP}, and a = R (1 − f)^(−⅓), until f moves by under 10⁻¹² (at most 50 passes);
 * the spheroid is then `Spheroid::from_volumetric(R, f)`: a = R (1 − f)^(−⅓), c = a (1 − f).
 */
function planFourteenFigure(
  radiusM: number,
  massKg: number,
  omegaRadS: number,
  momentOfInertiaFactor: number,
  tidal: boolean,
): PlanFourteenFigure {
  const gm = GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 * massKg;
  const x = 1 - 1.5 * momentOfInertiaFactor;
  const factor = tidal ? 2.5 : 1;
  let flattening = 0;
  let capped = false;
  let a = radiusM;
  for (let pass = 0; pass < 50; pass += 1) {
    const q = (omegaRadS * omegaRadS * a ** 3) / gm;
    const free = ((2.5 * q) / (1 + 6.25 * x * x)) * factor;
    capped = free > FLATTENING_CAP;
    const next = capped ? FLATTENING_CAP : free;
    const change = Math.abs(next - flattening);
    flattening = next;
    a = radiusM / Math.cbrt(1 - flattening);
    if (change < 1e-12) {
      break;
    }
  }
  const equatorialRadiusM = radiusM / Math.cbrt(1 - flattening);
  return {
    figure: { equatorialRadiusM, polarRadiusM: equatorialRadiusM * (1 - flattening) },
    gmM3S2: gm,
    capped,
  };
}

/** The input a record of law `law` gives, with no bulk gravity needed. */
function lawInput(
  built: PlanFourteenFigure,
  omegaRadS: number,
  figureLaw: FigureLawInput | null,
): BodyGravityInput {
  return {
    figure: built.figure,
    massKg: built.gmM3S2 / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2,
    angularVelocityRadS: omegaRadS,
    bulkGravityMS2: Number.NaN,
    figureLaw,
  };
}

/** Earth's volumetric mean radius, m (NASA's Earth fact sheet, 6,371 km). */
const EARTH_RADIUS_M = 6_371e3;

/** Plan 14's rocky moment of inertia (`ROCKY_MOMENT_OF_INERTIA`, `planetary/params.rs`). */
const ROCKY_C = 0.33;

/**
 * HD 209458 b-like, science-r08-oblate's illustrative case: 0.69 nominal Jovian masses and 1.38
 * times Jupiter's volumetric radius (96,477 km, 1.35 of IAU 2015 B3's equatorial R_eJ; Torres et
 * al. 2008, ApJ 677, 1324, give 0.685 M_J and 1.359 R_eJ, Southworth 2010 0.714 and 1.380), locked
 * 1:1 on its 3.5247 d orbit, with plan 14's gas-giant C ÷ Ma² of 0.25
 * (`GAS_GIANT_MOMENT_OF_INERTIA`): m = ω²R³ ÷ GM = 4.4 × 10⁻³.
 */
const HOT_JUPITER_C = 0.25;
const HOT_JUPITER_RADIUS_M = 1.38 * JUPITER_RADIUS_M;
const HOT_JUPITER_OMEGA_RAD_S = (2 * Math.PI) / (3.524_7 * DAY_S);
const HOT_JUPITER = planFourteenFigure(
  HOT_JUPITER_RADIUS_M,
  0.69 * JUPITER_MASS_KG,
  HOT_JUPITER_OMEGA_RAD_S,
  HOT_JUPITER_C,
  true,
);

/** {@link HOT_JUPITER}'s m = ω²R³ ÷ GM, R its volumetric radius. */
const HOT_JUPITER_M =
  (HOT_JUPITER_OMEGA_RAD_S ** 2 * HOT_JUPITER_RADIUS_M ** 3) / HOT_JUPITER.gmM3S2;

/** {@link HOT_JUPITER}'s gravity, under the law given (`null` for its true ω). */
function hotJupiterGravity(figureLaw: FigureLawInput | null): BodyGravity {
  return bodyGravity(
    lawInput(HOT_JUPITER, HOT_JUPITER_OMEGA_RAD_S, figureLaw),
    tableRadiusM(HOT_JUPITER.figure),
  );
}

/** {@link HOT_JUPITER}'s own law. */
const HOT_JUPITER_LAW: FigureLawInput = {
  law: "rotational_and_tidal",
  momentOfInertiaFactor: HOT_JUPITER_C,
};

/**
 * A Saturn-density giant that plan 14 caps: ρ = 690 kg m⁻³ (science-r08-oblate's `oblate_check.py`;
 * Saturn's 687 kg m⁻³, NASA's Saturn fact sheet) and plan 14's Saturn-like C ÷ Ma² = 0.21
 * (`SATURN_LIKE_MOMENT_OF_INERTIA`), 7 × 10⁷ m in radius, turning in 1.1 break-up periods
 * 2π √(R³ ÷ GM) (the ruling's case, where the true ω gives γ_e ≤ 0).
 */
const CAPPED_C = 0.21;
const CAPPED_RADIUS_M = 7e7;
const CAPPED_MASS_KG = (4 / 3) * Math.PI * CAPPED_RADIUS_M ** 3 * 690;
const CAPPED_OMEGA_RAD_S =
  1 /
  (1.1 * Math.sqrt(CAPPED_RADIUS_M ** 3 / (GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 * CAPPED_MASS_KG)));
const CAPPED = planFourteenFigure(
  CAPPED_RADIUS_M,
  CAPPED_MASS_KG,
  CAPPED_OMEGA_RAD_S,
  CAPPED_C,
  false,
);

/** {@link CAPPED}'s gravity under a `capped` law of the factor given. */
function cappedGravity(momentOfInertiaFactor: number): BodyGravity {
  return bodyGravity(
    lawInput(CAPPED, CAPPED_OMEGA_RAD_S, { law: "capped", momentOfInertiaFactor }),
    tableRadiusM(CAPPED.figure),
  );
}

/**
 * An Earth-density rocky world of Earth's radius turning in `periodH` hours, as plan 14 builds it
 * (`rotational`, C ÷ Ma² 0.33), as its own level spheroid.
 */
function rockyWorld(periodH: number): LevelSpheroid {
  const angularVelocityRadS = (2 * Math.PI) / (periodH * HOUR_S);
  const { figure, gmM3S2 } = planFourteenFigure(
    EARTH_RADIUS_M,
    EARTH_MASS_KG,
    angularVelocityRadS,
    ROCKY_C,
    false,
  );
  return { ...figure, gmM3S2, angularVelocityRadS };
}

/** An exact Roche figure, its truth by latitude, and its record. */
interface RocheCase {
  /** The record, with no law: the true ω. */
  readonly input: BodyGravityInput;
  /** Geodetic latitude, rad, and the longitude-mean gravity there, m s⁻². */
  readonly truth: ReadonlyArray<readonly [number, number]>;
  /** √(g_e g_p) of the truth, m s⁻². */
  readonly truthReferenceMS2: number;
}

/**
 * The Roche model, independent of first-order theory (science-r08-oblate's exact check): a
 * point-mass planet (k_f = 0) locked on a circular orbit about a primary of 1,000 times its mass,
 * at m = n²c³ ÷ GM = 0.01, c the polar radius (0.0103 at the volumetric radius).
 *
 * @remarks
 * Its level surfaces are those of the restricted three-body problem's pseudo-potential in the
 * corotating frame, W = GM ÷ r + GM_p ÷ r_p + ½n²[(x − x_b)² + y²], the primary along x at d and
 * x_b the barycentre's offset (Murray and Dermott 1999, _Solar System Dynamics_, ch. 3), found by
 * bisection along each direction. The drawn figure is the spheroid of the level surface's mean
 * equatorial radius and its polar radius, plan 14's (a + b) ÷ 2 against c; the truth is |∇W| on
 * the level surface averaged over 72 longitudes, every 5° of geocentric latitude ψ, placed at the
 * spheroid's geodetic latitude tan φ = (a ÷ c)² tan ψ.
 */
function rocheCase(): RocheCase {
  const gm = 3.986e14;
  const gmPrimary = 1_000 * gm;
  const r0 = 6.371e6;
  const distance = Math.cbrt(((gm + gmPrimary) * r0 ** 3) / (0.01 * gm));
  const n2 = (gm + gmPrimary) / distance ** 3;
  const barycentre = (distance * gmPrimary) / (gm + gmPrimary);
  const potential = (x: number, y: number, z: number): number =>
    gm / Math.hypot(x, y, z) +
    gmPrimary / Math.hypot(x - distance, y, z) +
    0.5 * n2 * ((x - barycentre) ** 2 + y * y);
  const gravityAt = (x: number, y: number, z: number): number => {
    const r3 = Math.hypot(x, y, z) ** 3;
    const rp3 = Math.hypot(x - distance, y, z) ** 3;
    return Math.hypot(
      (-gm * x) / r3 - (gmPrimary * (x - distance)) / rp3 + n2 * (x - barycentre),
      (-gm * y) / r3 - (gmPrimary * y) / rp3 + n2 * y,
      (-gm * z) / r3 - (gmPrimary * z) / rp3,
    );
  };
  const level = potential(0, 0, r0);
  const surfacePoint = (psi: number, lambda: number): readonly [number, number, number] => {
    const ux = Math.cos(psi) * Math.cos(lambda);
    const uy = Math.cos(psi) * Math.sin(lambda);
    const uz = Math.sin(psi);
    let low = 0.8 * r0;
    let high = 1.2 * r0;
    for (let i = 0; i < 64; i += 1) {
      const mid = 0.5 * (low + high);
      if (potential(mid * ux, mid * uy, mid * uz) > level) {
        low = mid;
      } else {
        high = mid;
      }
    }
    const r = 0.5 * (low + high);
    return [r * ux, r * uy, r * uz];
  };
  let equatorSum = 0;
  for (let j = 0; j < 360; j += 1) {
    equatorSum += Math.hypot(...surfacePoint(0, ((j + 0.5) / 360) * 2 * Math.PI));
  }
  const figure: SpheroidFigure = { equatorialRadiusM: equatorSum / 360, polarRadiusM: r0 };
  const aOverC2 = (figure.equatorialRadiusM / r0) ** 2;
  const truth: Array<readonly [number, number]> = [];
  for (let latitudeDeg = 0; latitudeDeg <= 90; latitudeDeg += 5) {
    const psi = latitudeDeg * DEG;
    let sum = 0;
    for (let j = 0; j < 72; j += 1) {
      sum += gravityAt(...surfacePoint(psi, ((j + 0.5) / 72) * 2 * Math.PI));
    }
    truth.push([Math.atan2(aOverC2 * Math.sin(psi), Math.cos(psi)), sum / 72]);
  }
  const truthEquator = truth[0]?.[1] ?? Number.NaN;
  const truthPole = truth.at(-1)?.[1] ?? Number.NaN;
  return {
    input: {
      figure,
      massKg: gm / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2,
      angularVelocityRadS: Math.sqrt(n2),
      bulkGravityMS2: Number.NaN,
      figureLaw: null,
    },
    truth,
    truthReferenceMS2: Math.sqrt(truthEquator * truthPole),
  };
}

const ROCHE = rocheCase();

/**
 * The Roche planet's own law. C ÷ Ma² = 2 ÷ 15 is the factor Darwin–Radau maps to k_f = 0 (η² = 4);
 * only `capped` reads it.
 */
const ROCHE_LAW: FigureLawInput = { law: "rotational_and_tidal", momentOfInertiaFactor: 2 / 15 };

/** The Roche planet's gravity under the law given (`null` for its true ω). */
function rocheGravity(figureLaw: FigureLawInput | null): BodyGravity {
  return bodyGravity({ ...ROCHE.input, figureLaw }, tableRadiusM(ROCHE.input.figure));
}

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
  it("gives 1 slice and 1 band for Earth, 2 and 2 for Uranus, 4 and 3 for Jupiter, 5 and 4 for Saturn", () => {
    expect([WGS84, URANUS, JUPITER, SATURN].map(slicingCounts)).toEqual([
      [1, 1],
      [2, 2],
      [4, 3],
      [5, 4],
    ]);
  });

  it("serves a body with ln(s_max ÷ s_min) ≤ ONE_BAND_BELOW with one band", () => {
    // The 12.5 h rocky world sits just inside the threshold (its span 0.0195).
    const body = rockyWorld(12.5);
    const [sEquator, sPole] = equatorAndPoleRatios(body);
    expect(Math.log(sPole / sEquator)).toBeGreaterThan(0.9 * ONE_BAND_BELOW);
    expect(Math.log(sPole / sEquator)).toBeLessThanOrEqual(ONE_BAND_BELOW);
    expect([...oblateSlicing(body, tableRadiusM(body)).bandGravityRatio]).toEqual([1]);
  });

  it("keeps |ln s| ≤ 0.01 at every latitude of a body under ONE_BAND_BELOW", () => {
    const body = rockyWorld(12.5);
    const gRef = referenceGravity(body);
    let worst = 0;
    for (let latitudeDeg = 0; latitudeDeg <= 90; latitudeDeg += 0.5) {
      worst = Math.max(worst, Math.abs(Math.log(normalGravity(body, latitudeDeg * DEG) / gRef)));
    }
    expect(worst).toBeLessThanOrEqual(0.01);
  });

  // The 12.5 h world's κ spans past ONE_SLICE_BELOW while its s does not; at 12 h (0.0212) both do.
  it.each([
    { periodH: 12.5, counts: [2, 1] },
    { periodH: 12, counts: [2, 2] },
  ])("decides slices and bands apart: $periodH h gives $counts", ({ periodH, counts }) => {
    expect(slicingCounts(rockyWorld(periodH))).toEqual(counts);
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

/**
 * First-order hydrostatic theory's longitude-mean gravity on a synchronous body, g ÷ g₀
 * (science-r08-oblate, derived there; Design note 17).
 *
 * @remarks
 * g ÷ g₀ = 1 − ⅔m + (k_f − 4) m(¾ cos²φ − ½ sin²φ − ⅓), with g₀ = GM ÷ R², m = ω²R³ ÷ GM, R the
 * volumetric radius, and k_f = (4 − η²) ÷ (1 + η²), η = (5 ÷ 2)(1 − (3 ÷ 2) C ÷ Ma²), the fluid
 * Love number Darwin–Radau implies. It is g ÷ g₀ = 1 − 2Q₀ + (k_f − 4)Q₂ on the level surface
 * ε = (1 + k_f)Q₂ under the spin's and the primary's static tide, Q = m[(3 ÷ 2)x² − ½z²] (primary
 * along x, GM_p ÷ d³ = ω²), Q₀ = m ÷ 3 its mean over the sphere and Q₂ its zonal degree-2 part. φ
 * is read as the geodetic latitude, which first order does not tell from the geocentric one, as
 * the ruling's check reads it.
 */
function synchronousFirstOrderGravity(
  m: number,
  momentOfInertiaFactor: number,
  latitudeRad: number,
): number {
  const eta2 = (2.5 * (1 - 1.5 * momentOfInertiaFactor)) ** 2;
  const loveNumber = (4 - eta2) / (1 + eta2);
  const cos2 = Math.cos(latitudeRad) ** 2;
  const sin2 = Math.sin(latitudeRad) ** 2;
  return 1 - (2 * m) / 3 + (loveNumber - 4) * m * (0.75 * cos2 - 0.5 * sin2 - 1 / 3);
}

/** The largest |ln s − ln s_theory| over latitude, every half degree. */
function worstLogRatio(
  s: (latitudeRad: number) => number,
  theory: (latitudeRad: number) => number,
): number {
  let worst = 0;
  for (let latitudeDeg = 0; latitudeDeg <= 90; latitudeDeg += 0.5) {
    worst = Math.max(worst, Math.abs(Math.log(s(latitudeDeg * DEG) / theory(latitudeDeg * DEG))));
  }
  return worst;
}

/** The laws whose spin is the true ω, with a name for the test's title. */
const TRUE_SPIN_LAWS: ReadonlyArray<{
  readonly name: string;
  readonly figureLaw: FigureLawInput | null;
}> = [
  { name: "no law", figureLaw: null },
  { name: "a rotational figure", figureLaw: { law: "rotational", momentOfInertiaFactor: ROCKY_C } },
  { name: "a sphere", figureLaw: { law: "sphere", momentOfInertiaFactor: ROCKY_C } },
];

/** First-order theory's s on {@link HOT_JUPITER}: its g ÷ g₀ over √(g_e g_p) of the same. */
function hotJupiterTheoryRatio(latitudeRad: number): number {
  const theory = (phi: number): number =>
    synchronousFirstOrderGravity(HOT_JUPITER_M, HOT_JUPITER_C, phi);
  return theory(latitudeRad) / Math.sqrt(theory(0) * theory(Math.PI / 2));
}

/** The largest |ln s − ln s_truth| over the Roche planet's latitudes. */
function worstAgainstRoche(gravity: BodyGravity): number {
  return Math.max(
    ...ROCHE.truth.map(([phi, g]) =>
      Math.abs(Math.log(gravityRatio(gravity, phi) / (g / ROCHE.truthReferenceMS2))),
    ),
  );
}

describe("bodyGravity by figure law (Design note 17)", () => {
  it.each(TRUE_SPIN_LAWS)("takes the true ω for $name", ({ figureLaw }) => {
    const gravity = bodyGravity({ ...EARTH_INPUT, figureLaw }, tableRadiusM(WGS84));
    expect(gravity.spheroid?.angularVelocityRadS).toBe(WGS84.angularVelocityRadS);
  });

  it.each(TRUE_SPIN_LAWS)(
    "leaves WGS 84's γ_e and γ_p unchanged to 10⁻⁹ for $name",
    ({ figureLaw }) => {
      const gravity = bodyGravity({ ...EARTH_INPUT, figureLaw }, tableRadiusM(WGS84));
      const gEquator = gravityRatio(gravity, 0) * gravity.referenceGravityMS2;
      const gPole = gravityRatio(gravity, Math.PI / 2) * gravity.referenceGravityMS2;
      expect(relative(gEquator, WGS84_GAMMA_E)).toBeLessThan(1e-9);
      expect(relative(gPole, WGS84_GAMMA_P)).toBeLessThan(1e-9);
    },
  );

  it("inverts plan 14's rotational figure to its ω", () => {
    const omega = (870.536 * DEG) / DAY_S;
    const built = planFourteenFigure(JUPITER_RADIUS_M, JUPITER_MASS_KG, omega, 0.25, false);
    expect(built.capped).toBe(false);
    expect(relative(darwinRadauSpinRadS(built.figure, built.gmM3S2, 0.25), omega)).toBeLessThan(
      1e-9,
    );
  });

  it("inverts plan 14's synchronous figure to √2.5 ω", () => {
    expect(HOT_JUPITER.capped).toBe(false);
    expect(
      relative(
        darwinRadauSpinRadS(HOT_JUPITER.figure, HOT_JUPITER.gmM3S2, HOT_JUPITER_C),
        Math.sqrt(2.5) * HOT_JUPITER_OMEGA_RAD_S,
      ),
    ).toBeLessThan(1e-9);
  });

  it("turns a synchronous figure's spheroid at √2.5 ω", () => {
    expect(
      relative(
        hotJupiterGravity(HOT_JUPITER_LAW).spheroid?.angularVelocityRadS ?? Number.NaN,
        Math.sqrt(2.5) * HOT_JUPITER_OMEGA_RAD_S,
      ),
    ).toBeLessThan(1e-15);
  });

  it("adds ω²R, R the volumetric radius, to a synchronous figure's gravity", () => {
    expect(
      relative(
        hotJupiterGravity(HOT_JUPITER_LAW).gravityOffsetMS2,
        HOT_JUPITER_OMEGA_RAD_S ** 2 * HOT_JUPITER_RADIUS_M,
      ),
    ).toBeLessThan(1e-12);
  });

  it("gives a synchronous figure first-order theory's s to 10⁻⁴", () => {
    expect(HOT_JUPITER_M).toBeCloseTo(4.4e-3, 4);
    const gravity = hotJupiterGravity(HOT_JUPITER_LAW);
    expect(worstLogRatio((phi) => gravityRatio(gravity, phi), hotJupiterTheoryRatio)).toBeLessThan(
      1e-4,
    );
  });

  it("gives a synchronous figure first-order theory's g_ref to 10⁻⁴", () => {
    const theory = (phi: number): number =>
      synchronousFirstOrderGravity(HOT_JUPITER_M, HOT_JUPITER_C, phi);
    const g0 = HOT_JUPITER.gmM3S2 / HOT_JUPITER_RADIUS_M ** 2;
    expect(
      relative(
        hotJupiterGravity(HOT_JUPITER_LAW).referenceGravityMS2,
        g0 * Math.sqrt(theory(0) * theory(Math.PI / 2)),
      ),
    ).toBeLessThan(1e-4);
  });

  it("misses a synchronous figure's s by at least 0.8% at the true ω", () => {
    const gravity = hotJupiterGravity(null);
    expect(
      worstLogRatio((phi) => gravityRatio(gravity, phi), hotJupiterTheoryRatio),
    ).toBeGreaterThanOrEqual(0.008);
  });

  it("ends a synchronous figure's bands at its g(φ) ÷ g_ref with ω²R added", () => {
    // The ruling's inflated hot Saturn: 0.3 nominal Jovian masses, 1.5 times Jupiter's volumetric
    // radius, locked on a 3 d orbit, C ÷ Ma² 0.21 (`SATURN_LIKE_MOMENT_OF_INERTIA`); its ln s spans
    // 0.083, two bands.
    const omega = (2 * Math.PI) / (3 * DAY_S);
    const built = planFourteenFigure(
      1.5 * JUPITER_RADIUS_M,
      0.3 * JUPITER_MASS_KG,
      omega,
      0.21,
      true,
    );
    const gravity = bodyGravity(
      lawInput(built, omega, { law: "rotational_and_tidal", momentOfInertiaFactor: 0.21 }),
      tableRadiusM(built.figure),
    );
    const bands = gravity.slicing.bandGravityRatio;
    expect(bands.length).toBe(2);
    expect(relative(bands[0] ?? Number.NaN, gravityRatio(gravity, 0))).toBeLessThan(1e-12);
    expect(relative(bands.at(-1) ?? Number.NaN, gravityRatio(gravity, Math.PI / 2))).toBeLessThan(
      1e-12,
    );
  });

  it("finds no level figure at a capped giant's true ω", () => {
    expect(CAPPED.capped).toBe(true);
    expect(() =>
      normalGravity(
        { ...CAPPED.figure, gmM3S2: CAPPED.gmM3S2, angularVelocityRadS: CAPPED_OMEGA_RAD_S },
        0,
      ),
    ).toThrow(RangeError);
  });

  it("builds a capped giant's gravity, rising poleward, at its Darwin–Radau spin", () => {
    const gravity = cappedGravity(CAPPED_C);
    let previous = 0;
    let rising = true;
    for (let latitudeDeg = 0; latitudeDeg <= 90; latitudeDeg += 1) {
      const s = gravityRatio(gravity, latitudeDeg * DEG);
      rising &&= s > previous;
      previous = s;
    }
    expect(rising).toBe(true);
  });

  it("adds nothing to a capped figure's gravity", () => {
    expect(cappedGravity(CAPPED_C).gravityOffsetMS2).toBe(0);
  });

  it("slices a capped giant into at most 10 slices and 7 bands", () => {
    // The ruling's bound (science-r08-oblate: 9–10 slices and 6–7 bands at the cap, against 19
    // and 21 at the true ω).
    const { slicing } = cappedGravity(CAPPED_C);
    expect(slicing.kappa.length).toBeLessThanOrEqual(10);
    expect(slicing.bandGravityRatio.length).toBeLessThanOrEqual(7);
  });

  it("gives an exact Roche figure's s to 5 × 10⁻⁴ at ω_fig", () => {
    // science-r08-oblate's check gives 0.04%; here 0.018%.
    expect(worstAgainstRoche(rocheGravity(ROCHE_LAW))).toBeLessThan(5e-4);
  });

  it("gives an exact Roche figure's g_ref to 5 × 10⁻⁴ at ω_fig with ω²R", () => {
    // science-r08-oblate's check gives −0.017%; here −0.018%.
    expect(
      relative(rocheGravity(ROCHE_LAW).referenceGravityMS2, ROCHE.truthReferenceMS2),
    ).toBeLessThan(5e-4);
  });

  it("misses an exact Roche figure's s by over 1.5% at the true ω", () => {
    // science-r08-oblate's check gives 1.86%; here 1.9%.
    expect(worstAgainstRoche(rocheGravity(null))).toBeGreaterThan(0.015);
  });

  it.each([0, 0.41, Number.NaN])("refuses the inversion a C ÷ Ma² of %s", (factor) => {
    expect(() => darwinRadauSpinRadS(CAPPED.figure, CAPPED.gmM3S2, factor)).toThrow(RangeError);
  });

  it.each([0, Number.NaN, Number.POSITIVE_INFINITY])("refuses the inversion a GM of %s", (gm) => {
    expect(() => darwinRadauSpinRadS(CAPPED.figure, gm, CAPPED_C)).toThrow(RangeError);
  });

  it("refuses a capped record whose C ÷ Ma² lies outside (0, 0.4]", () => {
    expect(() => cappedGravity(0.5)).toThrow(RangeError);
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
