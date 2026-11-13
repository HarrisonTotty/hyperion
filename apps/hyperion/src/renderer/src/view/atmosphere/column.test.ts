import { describe, expect, it } from "vitest";

import { GOLDEN_ENVELOPES, goldenEnvelope } from "../../test/envelopeProfileGolden";
import {
  GOLDEN_STRUCTURES,
  type GoldenStructure,
  goldenStructure,
} from "../../test/verticalStructureGolden";
import {
  type AtmosphereColumn,
  COLUMN_INTERVALS,
  COLUMN_TOP_PRESSURE_RATIO,
  type ColumnInput,
  dryAdiabatExponent,
  GAS_HEAT_CAPACITY,
  gasProperties,
  type GasProperties,
  hydrostaticColumn,
  meanMolarMassGPerMol,
  type MixtureComponent,
  temperatureAt,
  type TemperatureProfile,
} from "./column";
import { tableRadiusM } from "./hillaire";
import { columnLengthM, densityAt } from "./medium";
import {
  bodyGravity,
  gravityRatio,
  type LevelSpheroid,
  normalGravity,
  referenceGravity,
} from "./oblate";
import { type Gas, GASES } from "./rayleigh";

/** The Boltzmann constant, J K⁻¹, exact in the 2019 SI (CODATA 2022). */
const K_B = 1.380_649e-23;
/** The atomic mass constant, kg (CODATA 2022). */
const M_U = 1.660_539_068_92e-27;
/** The Newtonian constant of gravitation, m³ kg⁻¹ s⁻² (CODATA 2018 and 2022). */
const G = 6.674_3e-11;
const BAR_PA = 1e5;
const DEG = Math.PI / 180;

/**
 * WGS 84's four defining parameters (NIMA TR8350.2, Third Edition, Amendment 1, Table 3.1), as
 * `oblate.test.ts` takes them.
 */
const WGS84: LevelSpheroid = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257_223_563),
  gmM3S2: 3.986_004_418e14,
  angularVelocityRadS: 7.292_115e-5,
};

/**
 * Saturn at 1 bar, as `oblate.test.ts` and R08.T3.d's test give it: a and c of Archinal et al.
 * 2018, GM of Jacobson et al. 2006 to five figures, and a 10.656 h day.
 */
const SATURN: LevelSpheroid = {
  equatorialRadiusM: 60_268e3,
  polarRadiusM: 54_364e3,
  gmM3S2: 3.793_1e16,
  angularVelocityRadS: (2 * Math.PI) / (10.656 * 3_600),
};

/** A mixture of one gas. */
function pure(properties: GasProperties): ReadonlyArray<MixtureComponent> {
  return [{ properties, moleFraction: 1 }];
}

/**
 * A Saturn-class air by volume: the NSSDCA Saturn fact sheet's 96.3% H₂, 3.25% He and 4,500 ppm
 * CH₄.
 */
const SATURN_AIR: ReadonlyArray<MixtureComponent> = [
  { properties: gasProperties("H2"), moleFraction: 0.963 },
  { properties: gasProperties("He"), moleFraction: 0.0325 },
  { properties: gasProperties("CH4"), moleFraction: 0.0045 },
];

/** The Saturn-class skin: 2^(−1/4) × the NSSDCA fact sheet's 81.0 K black-body temperature, K. */
const SATURN_SKIN_K = 2 ** -0.25 * 81;

/** The Saturn-class β: α = 0.85, Robinson and Catling 2012's Jupiter (§4.2), times R ÷ c_p. */
const SATURN_BETA = 0.85 * dryAdiabatExponent(SATURN_AIR);

/** A Saturn-class profile: 134 K at 1 bar (the NSSDCA fact sheet), {@link SATURN_BETA}. */
const SATURN_PROFILE: TemperatureProfile = {
  kind: "radiativeConvective",
  surfaceK: 134,
  surfacePa: BAR_PA,
  beta: SATURN_BETA,
  skinK: SATURN_SKIN_K,
};

/** The Saturn-class column at its 1-bar datum, at Saturn's g_ref on its R_ref. */
const SATURN_COLUMN: ColumnInput = {
  surfacePa: BAR_PA,
  temperature: SATURN_PROFILE,
  meanMolarMassGPerMol: meanMolarMassGPerMol(SATURN_AIR),
  referenceGravityMS2: referenceGravity(SATURN),
  referenceRadiusM: tableRadiusM(SATURN),
};

/**
 * Saturn's gravity as a record gives it (R08.T3.d's `bodyGravity`): its mass by G, its spin, and a
 * bulk gravity of 10.44 m s⁻², which the spheroid's g_ref replaces.
 */
const SATURN_GRAVITY = bodyGravity(
  {
    figure: SATURN,
    massKg: SATURN.gmM3S2 / G,
    angularVelocityRadS: SATURN.angularVelocityRadS,
    bulkGravityMS2: 10.44,
    figureLaw: null,
  },
  tableRadiusM(SATURN),
);

/**
 * The vertical column above the datum at a latitude, kg m⁻²: ∫ρ dh over the geodetic height h,
 * with the medium read at the gravity-scaled height h* = s h.
 */
function massAboveDatumKgM2(column: AtmosphereColumn, s: number): number {
  const densityToKgM3 = column.surfaceNumberDensityPerM3 * column.meanMolarMassGPerMol * M_U;
  return (
    densityToKgM3 *
    simpson((heightM) => densityAt(column.density, s * heightM), 0, column.topHeightM / s, 400_000)
  );
}

/**
 * Design note 3's Venus: T_s 730 K, α 0.8 and pure CO₂, under a surface pressure. The skin does not
 * bind at 1 bar: 2^(−1/4) × the NSSDCA Venus fact sheet's 226.6 K black-body temperature.
 */
function venusProfile(surfacePa: number): TemperatureProfile {
  return {
    kind: "radiativeConvective",
    surfaceK: 730,
    surfacePa,
    beta: 0.8 * dryAdiabatExponent(pure(gasProperties("CO2"))),
    skinK: 2 ** -0.25 * 226.6,
  };
}

/**
 * A Venus-class column of 92 bar of CO₂, on the NSSDCA Venus fact sheet's 6,051.8 km volumetric
 * mean radius and 8.87 m s⁻² mean surface gravity.
 */
const VENUS_COLUMN: ColumnInput = {
  surfacePa: 92 * BAR_PA,
  temperature: venusProfile(92 * BAR_PA),
  meanMolarMassGPerMol: gasProperties("CO2").molarMassGPerMol,
  referenceGravityMS2: 8.87,
  referenceRadiusM: 6_051.8e3,
};

/** Earth's g_ref and R_ref: WGS 84's √(γ_e γ_p) and (2a + c) ÷ 3. */
const EARTH_GRAVITY_MS2 = referenceGravity(WGS84);
const EARTH_RADIUS_M = tableRadiusM(WGS84);

/** Earth isothermal at the U.S. Standard Atmosphere's sea level: 288.15 K, 1013.25 hPa, μ 28.97. */
const EARTH_ISOTHERMAL: ColumnInput = {
  surfacePa: 101_325,
  temperature: { kind: "isothermal", temperatureK: 288.15 },
  meanMolarMassGPerMol: 28.97,
  referenceGravityMS2: EARTH_GRAVITY_MS2,
  referenceRadiusM: EARTH_RADIUS_M,
};

/**
 * Titan's mean radius, m (Archinal et al. 2018, as JPL SSD's satellite physical parameters quote
 * it), and its gravity there, m s⁻²: GM ÷ R² with JPL's SAT441 GM, 8,978.1371 km³ s⁻².
 */
const TITAN_RADIUS_M = 2_574.76e3;
const TITAN_GRAVITY_MS2 = 8_978.137_1e9 / TITAN_RADIUS_M ** 2;

/** A structure of the sim's golden as the `nodes` profile it publishes. */
function nodesProfile(structure: GoldenStructure): TemperatureProfile {
  return { kind: "nodes", nodes: structure.nodes, skinK: structure.skinK };
}

/**
 * A golden structure's column at its p_s: Earth's on WGS 84's g_ref and R_ref with dry air's μ of
 * 28.97, Venus's on the Venus-class column's CO₂, radius and gravity, and Titan's on its N₂, radius
 * and gravity.
 */
function goldenColumn(name: "earth" | "venus" | "titan"): ColumnInput {
  const structure = goldenStructure(name);
  const body = {
    earth: {
      meanMolarMassGPerMol: 28.97,
      referenceGravityMS2: EARTH_GRAVITY_MS2,
      referenceRadiusM: EARTH_RADIUS_M,
    },
    venus: {
      meanMolarMassGPerMol: gasProperties("CO2").molarMassGPerMol,
      referenceGravityMS2: 8.87,
      referenceRadiusM: 6_051.8e3,
    },
    titan: {
      meanMolarMassGPerMol: gasProperties("N2").molarMassGPerMol,
      referenceGravityMS2: TITAN_GRAVITY_MS2,
      referenceRadiusM: TITAN_RADIUS_M,
    },
  }[name];
  return { surfacePa: structure.surfacePa, temperature: nodesProfile(structure), ...body };
}

/** Titan-class, isothermal at 94 K and 1.4 bar of N₂ (Design note 3's T_s and p_s). */
const TITAN_ISOTHERMAL: ColumnInput = {
  surfacePa: 1.4 * BAR_PA,
  temperature: { kind: "isothermal", temperatureK: 94 },
  meanMolarMassGPerMol: gasProperties("N2").molarMassGPerMol,
  referenceGravityMS2: TITAN_GRAVITY_MS2,
  referenceRadiusM: TITAN_RADIUS_M,
};

function relative(value: number, expected: number): number {
  return Math.abs(value / expected - 1);
}

/** H = kT ÷ (μ m_u g), m, from this test's own constants. */
function scaleHeightM(temperatureK: number, molarMassGPerMol: number, gravityMS2: number): number {
  return (K_B * temperatureK) / (molarMassGPerMol * M_U * gravityMS2);
}

/** The vertical column above the datum, kg m⁻²: ∫ρ dz from the column's own density. */
function columnMassKgM2(column: AtmosphereColumn): number {
  return (
    column.surfaceNumberDensityPerM3 *
    column.meanMolarMassGPerMol *
    M_U *
    columnLengthM(column.density, column.topHeightM)
  );
}

/** Composite Simpson's rule of `f` over [a, b] in `n` (even) intervals. */
function simpson(f: (x: number) => number, a: number, b: number, n: number): number {
  const h = (b - a) / n;
  let sum = f(a) + f(b);
  for (let i = 1; i < n; i += 1) {
    sum += (i % 2 === 1 ? 4 : 2) * f(a + i * h);
  }
  return (sum * h) / 3;
}

/**
 * The tropopause by bisection on {@link temperatureAt}: the highest level where the profile is
 * still warmer than its skin, Pa.
 */
function tropopausePa(profile: TemperatureProfile, skinK: number, surfacePa: number): number {
  let low = Math.log(surfacePa * 1e-6);
  let high = Math.log(surfacePa);
  for (let i = 0; i < 200; i += 1) {
    const middle = 0.5 * (low + high);
    if (temperatureAt(profile, Math.exp(middle)) > skinK) {
      high = middle;
    } else {
      low = middle;
    }
  }
  return Math.exp(0.5 * (low + high));
}

/**
 * The hydrostatic equation integrated by steps, independent of the module's closed form:
 * dz ÷ d ln p = −kT(p) ÷ (μ m_u g_ref) × (1 + z ÷ R)², by the classical fourth-order Runge–Kutta
 * rule in ln p from the datum, with `stepsPerLevel` steps between each two of the column's levels.
 */
function steppedHeightsM(
  column: AtmosphereColumn,
  input: ColumnInput,
  stepsPerLevel: number,
): Float64Array {
  const r = input.referenceRadiusM;
  const slope = (lnP: number, z: number): number =>
    -scaleHeightM(
      temperatureAt(input.temperature, Math.exp(lnP)),
      input.meanMolarMassGPerMol,
      input.referenceGravityMS2,
    ) *
    (1 + z / r) ** 2;
  const heights = new Float64Array(column.pressuresPa.length);
  let z = 0;
  for (let i = 1; i < heights.length; i += 1) {
    const from = Math.log(column.pressuresPa[i - 1] ?? Number.NaN);
    const to = Math.log(column.pressuresPa[i] ?? Number.NaN);
    const h = (to - from) / stepsPerLevel;
    for (let s = 0; s < stepsPerLevel; s += 1) {
      const x = from + s * h;
      const k1 = slope(x, z);
      const k2 = slope(x + h / 2, z + (h / 2) * k1);
      const k3 = slope(x + h / 2, z + (h / 2) * k2);
      const k4 = slope(x + h, z + h * k3);
      z += (h / 6) * (k1 + 2 * k2 + 2 * k3 + k4);
    }
    heights[i] = z;
  }
  return heights;
}

/** The largest relative difference between the column's heights and the stepped ones. */
function worstAgainstSteps(input: ColumnInput): number {
  const column = hydrostaticColumn(input);
  const stepped = steppedHeightsM(column, input, 8);
  let worst = 0;
  for (let i = 1; i < stepped.length; i += 1) {
    worst = Math.max(worst, relative(column.altitudesM[i] ?? Number.NaN, stepped[i] ?? Number.NaN));
  }
  return worst;
}

describe("hydrostaticColumn", () => {
  it("gives an isothermal column the scale height kT/(μ m_u g_ref) at every level", () => {
    const column = hydrostaticColumn(EARTH_ISOTHERMAL);
    const expected = scaleHeightM(288.15, 28.97, EARTH_GRAVITY_MS2);
    const r = EARTH_RADIUS_M;
    let worst = 0;
    for (let i = 1; i < column.altitudesM.length; i += 1) {
      const z = column.altitudesM[i] ?? Number.NaN;
      const geopotentialM = (r * z) / (r + z);
      const lnRatio = Math.log(101_325 / (column.pressuresPa[i] ?? Number.NaN));
      worst = Math.max(worst, relative(geopotentialM / lnRatio, expected));
    }
    expect(worst).toBeLessThan(1e-6);
  });

  it("gives Earth at 288.15 K, 1013.25 hPa and μ = 28.97 a scale height of about 8.4 km", () => {
    const column = hydrostaticColumn(EARTH_ISOTHERMAL);
    const z1 = column.altitudesM[1] ?? Number.NaN;
    const surfaceScaleHeightM = z1 / Math.log(101_325 / (column.pressuresPa[1] ?? Number.NaN));
    // The U.S. Standard Atmosphere 1976's sea-level scale height R*T₀ ÷ (M₀g₀), 8,434.5 m (R05's
    // `RAYLEIGH_SCALE_HEIGHT_M`), at its R* = 8.31432, M₀ = 28.9644 and g₀ = 9.80665, against
    // μ = 28.97 and Earth's g_ref = 9.8062 here.
    const standardM = (8.314_32 * 288.15) / (28.964_4e-3 * 9.806_65);
    expect(relative(surfaceScaleHeightM, standardM)).toBeLessThan(1e-3);
  });

  it.each<[string, ColumnInput]>([
    ["a Venus-class adiabat", VENUS_COLUMN],
    ["a Saturn-class adiabat and skin", SATURN_COLUMN],
    // A datum below the profile's reference pressure, where the adiabat continues.
    ["a datum below the profile's p_s", { ...SATURN_COLUMN, surfacePa: 2 * BAR_PA }],
    ["an isothermal column", TITAN_ISOTHERMAL],
    // β = 0: isothermal at T_s, with no tropopause.
    ["a profile with β = 0", { ...SATURN_COLUMN, temperature: { ...SATURN_PROFILE, beta: 0 } }],
    // The skin warmer than the datum: skin all the way up.
    [
      "a skin warmer than the datum",
      { ...SATURN_COLUMN, temperature: { ...SATURN_PROFILE, skinK: 200 } },
    ],
    ["the golden's Earth on its nodes", goldenColumn("earth")],
    ["the golden's Venus on its nodes", goldenColumn("venus")],
    ["the golden's Titan on its nodes", goldenColumn("titan")],
    // A datum below the ground, where the first segment's power law continues.
    [
      "a datum below the golden Earth's ground",
      { ...goldenColumn("earth"), surfacePa: 1.5 * goldenStructure("earth").surfacePa },
    ],
  ])("integrates the hydrostatic equation as steps in ln p do, to 1e-9, for %s", (_, input) => {
    expect(worstAgainstSteps(input)).toBeLessThan(1e-9);
  });

  it("holds p_s/g of mass per area for a thin atmosphere to 0.5%", () => {
    const column = hydrostaticColumn(EARTH_ISOTHERMAL);
    expect(relative(columnMassKgM2(column), 101_325 / EARTH_GRAVITY_MS2)).toBeLessThan(0.005);
  });

  it("holds the spherical excess over p_s/g for a thick atmosphere", () => {
    const column = hydrostaticColumn(TITAN_ISOTHERMAL);
    const h = scaleHeightM(94, TITAN_ISOTHERMAL.meanMolarMassGPerMol, TITAN_GRAVITY_MS2);
    const r = TITAN_RADIUS_M;
    const pOverG = TITAN_ISOTHERMAL.surfacePa / TITAN_GRAVITY_MS2;
    // ∫ρ dz with z = RΦ ÷ (R − Φ) and ρ = ρ_s e^(−Φ ÷ H), the exact isothermal column under
    // inverse-square gravity, over Φ from 0 to the top's H ln(10⁷). To second order its excess
    // over p_s ÷ g is 2H ÷ R + 6(H ÷ R)², here 1.6%.
    const phiTopM = h * Math.log(1 / COLUMN_TOP_PRESSURE_RATIO);
    const exact =
      (pOverG / h) * simpson((phi) => Math.exp(-phi / h) / (1 - phi / r) ** 2, 0, phiTopM, 200_000);
    const epsilon = h / r;
    expect(relative(exact, pOverG * (1 + 2 * epsilon + 6 * epsilon ** 2))).toBeLessThan(1e-4);
    expect(relative(columnMassKgM2(column), exact)).toBeLessThan(1e-4);
  });

  it.each([0, 45, 90])(
    "puts p_s/g(φ) above the datum at %s° on a Saturn-class figure",
    (latitudeDeg) => {
      const latitude = latitudeDeg * DEG;
      const column = hydrostaticColumn({
        ...SATURN_COLUMN,
        referenceGravityMS2: SATURN_GRAVITY.referenceGravityMS2,
      });
      const mass = massAboveDatumKgM2(column, gravityRatio(SATURN_GRAVITY, latitude));
      expect(relative(mass, BAR_PA / normalGravity(SATURN, latitude))).toBeLessThan(0.005);
    },
  );

  it.each([0, 45, 90])(
    "holds g(φ) times the column at %s° to p_s and the reference column's hydrostatic excess",
    (latitudeDeg) => {
      // The column read at h* = s h, s = g(φ) ÷ g_ref, times g(φ), is g_ref times the reference
      // column, ∫ (1 + z ÷ R_ref)² dp, with z from the stepped hydrostatic integration: 1.0014 p_s
      // at every latitude. A column built at the bulk 10.44 m s⁻² misses it by 1.2 × 10⁻³, and one
      // medium read at the geodetic height by up to ±14%.
      const latitude = latitudeDeg * DEG;
      const input: ColumnInput = {
        ...SATURN_COLUMN,
        referenceGravityMS2: SATURN_GRAVITY.referenceGravityMS2,
      };
      const column = hydrostaticColumn(input);
      const stepped = steppedHeightsM(column, input, 8);
      let weightPa = 0;
      for (let i = 1; i < stepped.length; i += 1) {
        const lower = column.pressuresPa[i - 1] ?? Number.NaN;
        const upper = column.pressuresPa[i] ?? Number.NaN;
        const at = (p: number, z: number): number => p * (1 + z / input.referenceRadiusM) ** 2;
        // ∫ f dp as ∫ f p d ln p, by the trapezoid rule in ln p.
        weightPa +=
          0.5 *
          Math.log(lower / upper) *
          (at(lower, stepped[i - 1] ?? Number.NaN) + at(upper, stepped[i] ?? Number.NaN));
      }
      const mass = massAboveDatumKgM2(column, gravityRatio(SATURN_GRAVITY, latitude));
      expect(relative(normalGravity(SATURN, latitude) * mass, weightPa)).toBeLessThan(1e-4);
    },
  );

  it("starts at the datum's pressure, height and ideal-gas density", () => {
    const column = hydrostaticColumn(TITAN_ISOTHERMAL);
    expect([column.altitudesM[0], column.pressuresPa[0], column.density.relative[0]]).toEqual([
      0,
      1.4 * BAR_PA,
      1,
    ]);
    expect(relative(column.surfaceNumberDensityPerM3, (1.4 * BAR_PA) / (K_B * 94))).toBeLessThan(
      1e-15,
    );
  });

  it("stops at p_s × 1e-7, its top level", () => {
    const column = hydrostaticColumn(TITAN_ISOTHERMAL);
    const last = column.altitudesM.length - 1;
    expect(column.pressuresPa[last]).toBe(1.4 * BAR_PA * COLUMN_TOP_PRESSURE_RATIO);
    expect(column.topHeightM).toBe(column.altitudesM[last]);
  });

  it("lays its density over its own levels", () => {
    const column = hydrostaticColumn(TITAN_ISOTHERMAL);
    expect(column.density.altitudesM).toBe(column.altitudesM);
  });

  it("puts a level at the tropopause, where the adiabat meets the skin", () => {
    const column = hydrostaticColumn(SATURN_COLUMN);
    const tropopause = tropopausePa(SATURN_PROFILE, SATURN_SKIN_K, BAR_PA);
    const level = column.pressuresPa.findIndex((p) => relative(p, tropopause) < 1e-9);
    expect(level).toBeGreaterThan(0);
    expect(column.temperaturesK[level]).toBeCloseTo(SATURN_SKIN_K, 9);
    expect(column.temperaturesK[level - 1] ?? 0).toBeGreaterThan(SATURN_SKIN_K);
    expect(column.temperaturesK[level + 1]).toBe(SATURN_SKIN_K);
  });

  it("adds no level for a tropopause within a thousandth of an interval of one", () => {
    // The skin met by the adiabat 2 × 10⁻⁴ of an interval above level 300.
    const step = Math.log(COLUMN_TOP_PRESSURE_RATIO) / COLUMN_INTERVALS;
    const skinK = 134 * Math.exp(SATURN_BETA * step * 300.000_2);
    const column = hydrostaticColumn({
      ...SATURN_COLUMN,
      temperature: { ...SATURN_PROFILE, skinK },
    });
    expect(column.altitudesM).toHaveLength(COLUMN_INTERVALS + 1);
  });

  it.each<[string, ColumnInput]>([
    ["a datum pressure of 0", { ...EARTH_ISOTHERMAL, surfacePa: 0 }],
    ["a negative molar mass", { ...EARTH_ISOTHERMAL, meanMolarMassGPerMol: -1 }],
    ["a gravity that is not a number", { ...EARTH_ISOTHERMAL, referenceGravityMS2: Number.NaN }],
    ["a radius of 0", { ...EARTH_ISOTHERMAL, referenceRadiusM: 0 }],
    [
      "a temperature of 0 K",
      { ...EARTH_ISOTHERMAL, temperature: { kind: "isothermal", temperatureK: 0 } },
    ],
  ])("refuses %s", (_, input) => {
    expect(() => hydrostaticColumn(input)).toThrow(RangeError);
  });

  it("refuses a column that is not bound below its top", () => {
    // 16 scale heights of 8.4 km do not fit under a 100 km radius.
    expect(() => hydrostaticColumn({ ...EARTH_ISOTHERMAL, referenceRadiusM: 100e3 })).toThrow(
      /not bound/,
    );
  });
});

describe("a column on P14.T24.e's published nodes (R08.T3.e)", () => {
  /** The column of a golden structure, and the index of its tropopause level, the last node's. */
  function withTropopause(name: "earth" | "venus" | "titan"): {
    readonly input: ColumnInput;
    readonly column: AtmosphereColumn;
    readonly structure: GoldenStructure;
    readonly level: number;
  } {
    const input = goldenColumn(name);
    const column = hydrostaticColumn(input);
    const structure = goldenStructure(name);
    const lastNodePa = structure.nodes.at(-1)?.pressurePa ?? Number.NaN;
    const level = column.pressuresPa.findIndex((p) => relative(p, lastNodePa) < 1e-12);
    return { input, column, structure, level };
  }

  it.each(["earth", "venus", "titan"] as const)(
    "puts the golden %s's tropopause at the height its segments sum to",
    (name) => {
      const { input, column, structure, level } = withTropopause(name);
      // Each segment's ∫ T d ln p in its own closed form, (T₀ − T₁) ln(p₀ ÷ p₁) ÷ ln(T₀ ÷ T₁): the
      // log-mean temperature times its span.
      let integralK = 0;
      for (const [k, upper] of structure.nodes.slice(1).entries()) {
        const lower = structure.nodes[k] ?? upper;
        integralK +=
          ((lower.temperatureK - upper.temperatureK) *
            Math.log(lower.pressurePa / upper.pressurePa)) /
          Math.log(lower.temperatureK / upper.temperatureK);
      }
      const r = input.referenceRadiusM;
      const phiM =
        scaleHeightM(1, input.meanMolarMassGPerMol, input.referenceGravityMS2) * integralK;
      expect(level).toBeGreaterThan(0);
      expect(
        relative(column.altitudesM[level] ?? Number.NaN, (r * phiM) / (r - phiM)),
      ).toBeLessThan(1e-12);
    },
  );

  it.each(["earth", "venus", "titan"] as const)(
    "holds the golden %s's skin at every level above its tropopause and none below",
    (name) => {
      const { column, structure, level } = withTropopause(name);
      const above = [...column.temperaturesK.subarray(level + 1)];
      expect(above.every((t) => t === structure.skinK)).toBe(true);
      expect(above.length).toBeGreaterThan(0);
      expect(column.temperaturesK[level - 1] ?? 0).toBeGreaterThan(structure.skinK);
    },
  );

  it.each(["earth", "venus", "titan"] as const)(
    "lays the golden %s's column continuously through its nodes",
    (name) => {
      // Between two levels the column's own mean temperature, its geopotential step over k ÷
      // (μ m_u g_ref) Δ ln p, lies between theirs: a step in ∫ T d ln p at a node would leave it.
      const { input, column } = withTropopause(name);
      const r = input.referenceRadiusM;
      const perKelvinM = scaleHeightM(1, input.meanMolarMassGPerMol, input.referenceGravityMS2);
      const phiM = [...column.altitudesM].map((z) => (r * z) / (r + z));
      let worst = 0;
      for (let i = 1; i < phiM.length; i += 1) {
        const meanK =
          ((phiM[i] ?? Number.NaN) - (phiM[i - 1] ?? Number.NaN)) /
          (perKelvinM *
            Math.log(
              (column.pressuresPa[i - 1] ?? Number.NaN) / (column.pressuresPa[i] ?? Number.NaN),
            ));
        const ends = [
          column.temperaturesK[i - 1] ?? Number.NaN,
          column.temperaturesK[i] ?? Number.NaN,
        ];
        const outside = Math.max(Math.min(...ends) - meanK, meanK - Math.max(...ends), 0);
        worst = Math.max(worst, outside / meanK);
      }
      expect(worst).toBeLessThan(1e-9);
    },
  );

  it("starts at the ground's own temperature, bit for bit", () => {
    const structure = goldenStructure("earth");
    const column = hydrostaticColumn(goldenColumn("earth"));
    expect([column.temperaturesK[0], column.surfaceNumberDensityPerM3]).toEqual([
      structure.surfaceK,
      structure.surfacePa / (K_B * structure.surfaceK),
    ]);
  });
});

/**
 * A golden envelope's column, a gas giant's from 10⁴ bar up to 10⁻² Pa at its own gravity on
 * Jupiter's 1-bar radius, with levels added at `levelsPa`: H₂ with the golden's He ÷ H₂ of 0.15.
 */
function envelopeColumn(
  name: "jupiter" | "hot_jupiter",
  levelsPa: ReadonlyArray<number> = [],
): ColumnInput {
  const { profile } = goldenEnvelope(name);
  return {
    surfacePa: 1e9,
    topPa: 1e-2,
    levelsPa,
    temperature: profile,
    meanMolarMassGPerMol: 0.87 * 2.016 + 0.13 * 4.003,
    referenceGravityMS2: profile.gravityMS2,
    referenceRadiusM: 71_492e3,
  };
}

describe("temperatureAt over P14.T24.d's envelopes (R08.T16.a)", () => {
  it("reads the sim's golden: six envelopes, their nodes and levels", () => {
    // Saturn's adiabat has 18 nodes from generator version 27 (19 before): κ₁ refitted and its own
    // Bond albedo (P14.T24.h.b) put its join at 0.612 bar, 0.584 before, one segment fewer.
    expect(
      GOLDEN_ENVELOPES.map(({ name, profile, levels }) => [
        name,
        profile.nodes.length,
        levels.length,
      ]),
    ).toEqual([
      ["jupiter", 17, 15],
      ["saturn", 18, 15],
      ["uranus", 17, 15],
      ["neptune", 18, 15],
      ["sub_neptune", 17, 15],
      ["hot_jupiter", 17, 15],
    ]);
  });

  it.each(GOLDEN_ENVELOPES.map((e) => [e.name, e] as const))(
    "gives the sim's temperature at every level of the golden's %s above 0 Pa, bit for bit",
    (_, { profile, levels }) => {
      // temperatureAt refuses 0 Pa, where the sim's grey profile gives its skin.
      const above = levels.filter(({ pressurePa }) => pressurePa > 0);
      expect(above.map(({ pressurePa }) => temperatureAt(profile, pressurePa))).toEqual(
        above.map(({ temperatureK }) => temperatureK),
      );
    },
  );

  it.each(GOLDEN_ENVELOPES.map((e) => [e.name, e] as const))(
    "is continuous at the golden %s's join and nodes",
    (_, { profile }) => {
      let worst = 0;
      for (const { pressurePa, temperatureK } of profile.nodes) {
        for (const side of [1 - 1e-12, 1 + 1e-12]) {
          worst = Math.max(
            worst,
            relative(temperatureAt(profile, pressurePa * side), temperatureK),
          );
        }
      }
      expect(worst).toBeLessThan(1e-9);
    },
  );

  it.each<[string, ColumnInput]>([
    ["the golden Jupiter's envelope", envelopeColumn("jupiter")],
    ["the golden hot Jupiter's envelope", envelopeColumn("hot_jupiter")],
  ])("integrates the hydrostatic equation as steps in ln p do, to 1e-9, for %s", (_, input) => {
    expect(worstAgainstSteps(input)).toBeLessThan(1e-9);
  });

  const JUPITER = goldenEnvelope("jupiter").profile;
  it.each<[string, TemperatureProfile]>([
    ["its nodes in descending order", { ...JUPITER, nodes: JUPITER.nodes.toReversed() }],
    ["one node", { ...JUPITER, nodes: JUPITER.nodes.slice(0, 1) }],
    ["an n of 0.5", { ...JUPITER, grey: { ...JUPITER.grey, opticalDepthExponent: 0.5 } }],
    ["a γ of 1.5", { ...JUPITER, grey: { ...JUPITER.grey, opacityRatio: 1.5 } }],
    ["an f of 0", { ...JUPITER, grey: { ...JUPITER.grey, redistributionFactor: 0 } }],
    ["a negative T_int", { ...JUPITER, internalK: -1 }],
    ["a g of 0", { ...JUPITER, gravityMS2: 0 }],
  ])("refuses the golden Jupiter's envelope with %s", (_, profile) => {
    expect(() => temperatureAt(profile, 1e5)).toThrow(RangeError);
  });
});

describe("a column to a top of its own, with added levels (R08.T16.a)", () => {
  const asked = [6.414_977_329_982_213e4, 6.414_977_329_982_213e4 * (1 + 1e-7), 1.080_774e4];

  it("ends at its top and lays a level at each pressure asked, exactly", () => {
    const column = hydrostaticColumn(envelopeColumn("jupiter", asked));
    expect([column.pressuresPa[0], column.pressuresPa.at(-1)]).toEqual([1e9, 1e-2]);
    expect(asked.map((p) => column.pressuresPa.includes(p))).toEqual([true, true, true]);
  });

  it("keeps its levels no wider than COLUMN_INTERVALS' over a span of 10⁷", () => {
    const column = hydrostaticColumn(envelopeColumn("jupiter"));
    const widest = Math.log(1 / COLUMN_TOP_PRESSURE_RATIO) / COLUMN_INTERVALS;
    let worst = 0;
    for (let i = 1; i < column.pressuresPa.length; i += 1) {
      worst = Math.max(
        worst,
        Math.log((column.pressuresPa[i - 1] ?? Number.NaN) / (column.pressuresPa[i] ?? Number.NaN)),
      );
    }
    expect(worst).toBeLessThanOrEqual(widest * (1 + 1e-12));
    expect(column.altitudesM.every((h, i) => i === 0 || h > (column.altitudesM[i - 1] ?? 0))).toBe(
      true,
    );
  });

  it("lays an asked level in place of an even one within 10⁻⁹ of it in ln p, exactly", () => {
    const even = hydrostaticColumn(envelopeColumn("jupiter"));
    const at = even.pressuresPa[700] ?? Number.NaN;
    const column = hydrostaticColumn(envelopeColumn("jupiter", [at * (1 + 1e-10)]));
    expect(column.pressuresPa).toHaveLength(even.pressuresPa.length);
    expect(column.pressuresPa[700]).toBe(at * (1 + 1e-10));
    expect(column.altitudesM.every((h, i) => i === 0 || h > (column.altitudesM[i - 1] ?? 0))).toBe(
      true,
    );
  });

  it("lays no asked level within 10⁻⁹ in ln p of its top or its datum, nor one asked twice", () => {
    const even = hydrostaticColumn(envelopeColumn("jupiter"));
    const column = hydrostaticColumn(
      envelopeColumn("jupiter", [1e-2 * (1 + 1e-10), 1e9 * (1 - 1e-10), 5e4, 5e4]),
    );
    expect(column.pressuresPa).toHaveLength(even.pressuresPa.length + 1);
    expect([column.pressuresPa[0], column.pressuresPa.at(-1)]).toEqual([1e9, 1e-2]);
  });

  it.each<[string, ColumnInput]>([
    ["a top below its datum", { ...envelopeColumn("jupiter"), topPa: 2e9 }],
    ["a level below its datum", envelopeColumn("jupiter", [2e9])],
    ["a level above its top", envelopeColumn("jupiter", [1e-3])],
  ])("refuses %s", (_, input) => {
    expect(() => hydrostaticColumn(input)).toThrow(RangeError);
  });
});

describe("temperatureAt over P14.T24.e's published nodes (R08.T3.e)", () => {
  it("reads the sim's golden: Earth's, Venus's and Titan's nodes and levels", () => {
    expect(
      GOLDEN_STRUCTURES.map(({ name, nodes, levels }) => [name, nodes.length, levels.length]),
    ).toEqual([
      ["earth", 17, 28],
      ["venus", 22, 33],
      ["titan", 17, 28],
    ]);
  });

  it.each(GOLDEN_STRUCTURES.map((s) => [s.name, s] as const))(
    "gives the sim's temperature at every level of the golden's %s, bit for bit",
    (_, structure) => {
      const profile = nodesProfile(structure);
      expect(structure.levels.map(({ pressurePa }) => temperatureAt(profile, pressurePa))).toEqual(
        structure.levels.map(({ temperatureK }) => temperatureK),
      );
    },
  );

  it.each(GOLDEN_STRUCTURES.map((s) => [s.name, s] as const))(
    "gives each of the golden %s's nodes its own temperature, bit for bit, and the skin at the last",
    (_, structure) => {
      const profile = nodesProfile(structure);
      const below = structure.nodes.slice(0, -1);
      expect(below.map(({ pressurePa }) => temperatureAt(profile, pressurePa))).toEqual(
        below.map(({ temperatureK }) => temperatureK),
      );
      expect(temperatureAt(profile, structure.nodes.at(-1)?.pressurePa ?? Number.NaN)).toBe(
        structure.skinK,
      );
    },
  );

  it.each(GOLDEN_STRUCTURES.map((s) => [s.name, s] as const))(
    "is continuous at each of the golden %s's nodes",
    (_, structure) => {
      const profile = nodesProfile(structure);
      let worst = 0;
      for (const { pressurePa, temperatureK } of structure.nodes) {
        for (const side of [1 - 1e-12, 1 + 1e-12]) {
          worst = Math.max(
            worst,
            relative(temperatureAt(profile, pressurePa * side), temperatureK),
          );
        }
      }
      expect(worst).toBeLessThan(1e-12);
    },
  );

  it("continues the first segment's power law below the ground", () => {
    const { nodes } = goldenStructure("venus");
    const [ground, next] = [nodes[0], nodes[1]];
    if (ground === undefined || next === undefined) {
      throw new Error("the golden's Venus has two nodes");
    }
    const exponent =
      Math.log(next.temperatureK / ground.temperatureK) /
      Math.log(next.pressurePa / ground.pressurePa);
    const profile = nodesProfile(goldenStructure("venus"));
    expect(
      relative(temperatureAt(profile, 3 * ground.pressurePa), ground.temperatureK * 3 ** exponent),
    ).toBeLessThan(1e-14);
  });

  it("holds the skin above the last node", () => {
    const structure = goldenStructure("titan");
    const lastNodePa = structure.nodes.at(-1)?.pressurePa ?? Number.NaN;
    const profile = nodesProfile(structure);
    expect([0.999 * lastNodePa, 1e3, 1, 1e-6].map((p) => temperatureAt(profile, p))).toEqual(
      Array.from({ length: 4 }, () => structure.skinK),
    );
  });

  it("is isothermal at the skin where the ground is the one node", () => {
    // The sim's skin, not the node's temperature, which its structure makes equal.
    const profile: TemperatureProfile = {
      kind: "nodes",
      nodes: [{ pressurePa: BAR_PA, temperatureK: 260 }],
      skinK: 250,
    };
    expect([10 * BAR_PA, BAR_PA, 1].map((p) => temperatureAt(profile, p))).toEqual([250, 250, 250]);
    expect(
      hydrostaticColumn({ ...EARTH_ISOTHERMAL, surfacePa: BAR_PA, temperature: profile }),
    ).toEqual(
      hydrostaticColumn({
        ...EARTH_ISOTHERMAL,
        surfacePa: BAR_PA,
        temperature: { kind: "isothermal", temperatureK: 250 },
      }),
    );
  });

  const EARTH_NODES = goldenStructure("earth").nodes;
  it.each<[string, TemperatureProfile]>([
    ["no nodes", { kind: "nodes", nodes: [], skinK: 200 }],
    ["pressures that rise", { kind: "nodes", nodes: EARTH_NODES.toReversed(), skinK: 200 }],
    [
      "a node at 0 K",
      { kind: "nodes", nodes: [...EARTH_NODES, { pressurePa: 1e3, temperatureK: 0 }], skinK: 200 },
    ],
    ["a skin of 0 K", { kind: "nodes", nodes: EARTH_NODES, skinK: 0 }],
  ])("refuses a profile of nodes with %s", (_, profile) => {
    expect(() => temperatureAt(profile, BAR_PA)).toThrow(RangeError);
  });
});

describe("temperatureAt", () => {
  it("puts Earth's tropopause at 0.179 bar", () => {
    const earth: TemperatureProfile = {
      kind: "radiativeConvective",
      surfaceK: 288,
      surfacePa: BAR_PA,
      beta: 0.6 * dryAdiabatExponent(pure(gasProperties("N2"))),
      skinK: 214.4,
    };
    expect(relative(tropopausePa(earth, 214.4, BAR_PA), 0.179 * BAR_PA)).toBeLessThan(0.01);
  });

  it("gives Venus 316.8 K at 1 bar", () => {
    expect(relative(temperatureAt(venusProfile(92 * BAR_PA), BAR_PA), 316.8)).toBeLessThan(0.01);
  });

  it("gives Venus about 345 K at 1 bar under plan 14's 58 bar", () => {
    expect(relative(temperatureAt(venusProfile(58 * BAR_PA), BAR_PA), 345)).toBeLessThan(0.01);
  });

  it("puts Titan's tropopause at 0.244 bar", () => {
    const titan: TemperatureProfile = {
      kind: "radiativeConvective",
      surfaceK: 94,
      surfacePa: 1.4 * BAR_PA,
      beta: 0.77 * dryAdiabatExponent(pure(gasProperties("N2"))),
      skinK: 64,
    };
    // Design note 3 rounds it to 0.24 bar.
    expect(relative(tropopausePa(titan, 64, 1.4 * BAR_PA), 0.244 * BAR_PA)).toBeLessThan(0.01);
  });

  it("is the surface temperature at p_s and the skin's far above", () => {
    expect([temperatureAt(SATURN_PROFILE, BAR_PA), temperatureAt(SATURN_PROFILE, 1)]).toEqual([
      134,
      SATURN_SKIN_K,
    ]);
  });

  it("is an isothermal profile's temperature at every pressure", () => {
    expect(temperatureAt({ kind: "isothermal", temperatureK: 250 }, 3e4)).toBe(250);
  });

  it.each<[string, number]>([
    ["0 Pa", 0],
    ["an infinite pressure", Number.POSITIVE_INFINITY],
  ])("refuses a pressure of %s", (_, pressurePa) => {
    expect(() => temperatureAt(SATURN_PROFILE, pressurePa)).toThrow(RangeError);
  });

  it.each<[string, TemperatureProfile]>([
    ["a negative β", { ...SATURN_PROFILE, beta: -0.1 }],
    ["a skin of 0 K", { ...SATURN_PROFILE, skinK: 0 }],
  ])("refuses a profile with %s", (_, profile) => {
    expect(() => temperatureAt(profile, BAR_PA)).toThrow(RangeError);
  });
});

describe("the gases' properties", () => {
  it("gives pure CO₂ R/c_p = 3/13", () => {
    expect(dryAdiabatExponent(pure(gasProperties("CO2")))).toBeCloseTo(3 / 13, 15);
  });

  it("gives pure N₂ R/c_p = 2/7", () => {
    expect(dryAdiabatExponent(pure(gasProperties("N2")))).toBeCloseTo(2 / 7, 15);
  });

  it("mixes R/c_p by c_p", () => {
    const mixture: ReadonlyArray<MixtureComponent> = [
      { properties: gasProperties("He"), moleFraction: 0.5 },
      { properties: gasProperties("CO2"), moleFraction: 0.5 },
    ];
    expect(dryAdiabatExponent(mixture)).toBeCloseTo(1 / (0.5 * 2.5 + 0.5 * (13 / 3)), 15);
  });

  it("does not mix R/c_p by γ", () => {
    const mixture: ReadonlyArray<MixtureComponent> = [
      { properties: gasProperties("He"), moleFraction: 0.5 },
      { properties: gasProperties("CO2"), moleFraction: 0.5 },
    ];
    const meanGamma = 0.5 * (5 / 3) + 0.5 * 1.3;
    expect(relative(dryAdiabatExponent(mixture), (meanGamma - 1) / meanGamma)).toBeGreaterThan(0.1);
  });

  it("takes plan 14's gases' molar masses from the sim's Gas::molar_mass_g_per_mol", () => {
    // planetary/derive/atmosphere.rs, Gas::molar_mass_g_per_mol, in Gas::ALL's order.
    const sim = [2.016, 4.003, 18.015, 16.043, 17.031, 28.014, 31.998, 44.009, 39.95];
    const planFourteen = GASES.slice(0, sim.length);
    expect(planFourteen.map((gas) => gasProperties(gas).molarMassGPerMol)).toEqual(sim);
  });

  /**
   * R ÷ c_p at 298.15 K from NIST-JANAF's c_p (Chase 1998): Design note 3's nine with NH₃ at
   * 8.314 ÷ 35.652 and N₂ at 8.314 ÷ 29.124, the atoms' exact 2/5, N₂O's 8.314 ÷ 38.617, CO's
   * 8.314 ÷ 29.149 and H₂S's 8.314 ÷ 34.197 (the WebBook's Shomate fits). C₂H₆'s is at 100 K,
   * 8.314 ÷ 35.70 (Gurvich, Veyts et al. 1989), the cold atmospheres it is taken for; at 298.15 K
   * it is 0.158 (52.49), which no fixed N follows.
   */
  const MEASURED: Readonly<Record<Gas, number>> = {
    H2: 0.288,
    He: 0.4,
    H2O: 0.248,
    CH4: 0.233,
    NH3: 0.233,
    N2: 0.285,
    O2: 0.283,
    CO2: 0.224,
    Ar: 0.4,
    Ne: 0.4,
    Kr: 0.4,
    Xe: 0.4,
    N2O: 0.215,
    CO: 0.285,
    H2S: 0.243,
    C2H6: 0.233,
  };

  it.each(GASES)("holds %s's R/c_p within Design note 3's 7% of its measured value", (gas) => {
    expect(relative(1 / GAS_HEAT_CAPACITY[gas].overR, MEASURED[gas])).toBeLessThan(0.075);
  });

  it("mixes the molar mass by mole fraction", () => {
    expect(meanMolarMassGPerMol(SATURN_AIR)).toBeCloseTo(
      0.963 * 2.016 + 0.0325 * 4.003 + 0.0045 * 16.043,
      12,
    );
  });

  it("refuses fractions that do not sum to 1 within 1e-9", () => {
    expect(() =>
      dryAdiabatExponent([{ properties: gasProperties("N2"), moleFraction: 0.9 }]),
    ).toThrow(RangeError);
  });

  it("accepts fractions that sum to 1 within 1e-9", () => {
    const n2 = gasProperties("N2");
    const justWithin: ReadonlyArray<MixtureComponent> = [
      { properties: n2, moleFraction: 0.5 + 5e-10 },
      { properties: n2, moleFraction: 0.5 },
    ];
    expect(meanMolarMassGPerMol(justWithin)).toBeCloseTo(28.014, 6);
  });

  it("refuses a fraction outside [0, 1], even where the fractions sum to 1", () => {
    const mixture: ReadonlyArray<MixtureComponent> = [
      { properties: gasProperties("N2"), moleFraction: 1.5 },
      { properties: gasProperties("O2"), moleFraction: -0.5 },
    ];
    expect(() => meanMolarMassGPerMol(mixture)).toThrow(RangeError);
  });

  it("refuses a species whose molar mass is not positive", () => {
    const massless = { ...gasProperties("N2"), molarMassGPerMol: 0 };
    expect(() => meanMolarMassGPerMol(pure(massless))).toThrow(RangeError);
  });

  it("refuses a species whose c_p is not positive", () => {
    const negative = { ...gasProperties("N2"), heatCapacityOverR: -1 };
    expect(() => dryAdiabatExponent(pure(negative))).toThrow(RangeError);
  });
});
