import { beforeAll, describe, expect, it } from "vitest";

import { dot, normalise, type Vec3 } from "../../geometry/vec3";
import {
  BOTTOM_M,
  CONVERGED,
  describeFailures,
  errorOf,
  errors,
  failures,
  type Gate,
  gateRays,
  type GateRay,
  GROUND_ALBEDO,
  HIGH,
  kernelOf,
  LOW,
  evenAt,
  oldFactorF32,
  overLowBounds,
  type Placement,
  REFERENCE_STEPS,
  type Rgb3,
  runGate,
  singleScatteringPixel,
  stepFactorF32,
  stepsFor,
  sunTransmittance,
  twilight,
} from "../../test/atmosphereGate";
import {
  amountValue,
  fourAbsorberMedium,
  planckBakeSpectrum,
  r05EarthWithOzoneAbsorber,
  venusClassMedium,
} from "../../test/atmosphereFixtures";
import { quaternion } from "../camera/quaternion";
import { fromHalfBits, HALF_MAX, toHalfBits } from "../sky/half";
import {
  type AbsorberCurve,
  absorberCurve,
  absorberTransmittance,
  scaledAmount,
} from "./absorbers";
import { hydrostaticColumn } from "./column";
import { EARTH_REFERENCE } from "./earth";
import {
  type AtmosphereCamera,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
} from "./hillaire";
import { marchSplit, marchStep, TWILIGHT_TOLERANCE } from "./marchSteps";
import {
  type AtmosphereMedium,
  type MediumTerm,
  type PhaseFunction,
  phaseTable,
  tabulatedDensity,
} from "./medium";
import {
  absorberAmounts,
  absorberAmountsToTop,
  curvesOpticalDepth,
  maxDistanceM,
  opticalDepth,
  opticalDepthToTop,
  ORACLE_STEPS,
  transmittanceRMuToUv,
  transmittanceUvToRMu,
  unitFromTexelUv,
} from "./opticalDepth";
import commonWgsl from "./shaders/common.wgsl?raw";
import multiScatteringWgsl from "./shaders/multiScattering.wgsl?raw";
import type { BakeSpectrum } from "./spectralColour";
import {
  absorberSlots,
  MULTI_SCATTERING_SIZE,
  NO_ABSORBER_CURVES,
  TRANSMITTANCE_SIZE,
} from "./tables";
import {
  aerialPerspectiveTwin,
  curveOpticalDepth,
  kernelOpticalDepth,
  marchTwin,
  MULTI_SCATTERING_DIRECTIONS,
  MULTI_SCATTERING_GROUND_OFFSET_M,
  MULTI_SCATTERING_KERNEL_OPTIONS,
  multiScatteringTwin,
  type MultiScatteringOptions,
  placedSteps,
  planetTwin,
  type PlanetTwin,
  rayMarchRayTwin,
  rayMarchTwin,
  skyViewRayTwin,
  skyViewTwin,
  tableSources,
  type TwinRay,
  type TwinSources,
  type TwinTable,
  type TwinTransmittance,
  transmittanceTablesTwin,
  transmittanceTwin,
  twinView,
  type TwinView,
} from "./tablesCpu";

/** WGS 84 (NIMA TR8350.2): the test planet's figure, and R₁ = (2a + c) ÷ 3, the tables' ground. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};
const R1 = tableRadiusM(WGS84);

/** Earth's per-planet twin on R₁, built once for the tests that read it. */
let earth: PlanetTwin = {
  medium: EARTH_REFERENCE,
  bottomRadiusM: R1,
  transmittance: { widthTexels: 0, heightTexels: 0, texels: new Float64Array(0) },
  absorberColumns: { widthTexels: 0, heightTexels: 0, texels: new Float64Array(0) },
  curves: [],
  multiScattering: { widthTexels: 0, heightTexels: 0, texels: new Float64Array(0) },
};

beforeAll(() => {
  earth = planetTwin(EARTH_REFERENCE, R1);
}, 60_000);

const CHANNELS = [0, 1, 2] as const;

/** The largest relative difference between two tables' texels, over texels where `b` is not 0. */
function worstRelative(a: Float64Array, b: Float64Array): number {
  let worst = 0;
  for (const [i, value] of b.entries()) {
    const other = a[i] ?? Number.NaN;
    const e = value === 0 ? Math.abs(other) : Math.abs(other / value - 1);
    worst = Math.max(worst, Number.isNaN(e) ? Number.POSITIVE_INFINITY : e);
  }
  return worst;
}

// --- R05's constants as a medium -----------------------------------------------------------------

/** R05.T12.b's smoke texels: five across μ by four across r. */
const ORACLE_TEXELS = [0, 64, 128, 192, 255].flatMap((x) =>
  [0, 16, 40, 63].map((y) => [x, y] as const),
);

/** A gate ray as the twin marches it. */
function twinRayOf(ray: GateRay): TwinRay {
  return {
    originM: ray.o,
    direction: ray.d,
    sun: ray.s,
    tStartM: ray.tStartM,
    tEndM: ray.tEndM,
    // A segment starts at the camera exactly when the camera is inside the atmosphere.
    fromCamera: ray.tStartM <= 0,
  };
}

/**
 * A gate ray's pixel through the twin: its march over `sources`, with the twin's own placement for
 * placed steps, plus the gate's lit ground through the ray's transmittance.
 */
function twinPixel(medium: AtmosphereMedium, sources: TwinSources) {
  return (ray: GateRay, placement: Placement, n: number): Rgb3 => {
    const twinRay = twinRayOf(ray);
    const steps =
      placement === "placed"
        ? placedSteps(twinRay, n)
        : stepsFor(placement, ray.o, ray.d, ray.tStartM, ray.tEndM, n);
    const { luminance: l, transmittance: t } = marchTwin(
      medium,
      sources,
      { kind: "sphere", bottomRadiusM: BOTTOM_M },
      twinRay,
      steps,
    );
    if (!ray.ground) {
      return [l[0], l[1], l[2]];
    }
    const end = normalise({
      x: ray.o.x + ray.tEndM * ray.d.x,
      y: ray.o.y + ray.tEndM * ray.d.y,
      z: ray.o.z + ray.tEndM * ray.d.z,
    });
    const mu = dot(end, ray.s);
    const toSun: Rgb3 = [0, 0, 0];
    sources.sunTransmittance(0, mu, toSun);
    const lit = (c: 0 | 1 | 2): number => (mu > 0 ? (GROUND_ALBEDO / Math.PI) * mu * toSun[c] : 0);
    return [l[0] + t[0] * lit(0), l[1] + t[1] * lit(1), l[2] + t[2] * lit(2)];
  };
}

/** R05.T12.e's twin's sources: its sun from the 512 × 128 grid, and no multiple scattering. */
const R05_SOURCES: TwinSources = {
  bottomRadiusM: BOTTOM_M,
  sunTransmittance: (heightM, muSun, out) => {
    sunTransmittance(BOTTOM_M + heightM, muSun, out);
  },
  multiScattering: (_heightM, _muSun, out) => {
    out.fill(0);
  },
  absorberPathDepth: (_columns, out) => {
    out.fill(0);
  },
};

describe("R05's constants as a medium", () => {
  it("give R05's oracle optical depth at R05.T12.b's 20 texels to 10⁻⁶, at the oracle's steps", () => {
    const shell = { bottomRadiusM: R1, topRadiusM: R1 + EARTH_REFERENCE.topHeightM };
    let worst = 0;
    for (const [x, y] of ORACLE_TEXELS) {
      const { rM, mu } = transmittanceUvToRMu(
        shell,
        unitFromTexelUv((x + 0.5) / TRANSMITTANCE_SIZE.widthTexels, TRANSMITTANCE_SIZE.widthTexels),
        unitFromTexelUv(
          (y + 0.5) / TRANSMITTANCE_SIZE.heightTexels,
          TRANSMITTANCE_SIZE.heightTexels,
        ),
      );
      const twin = kernelOpticalDepth(EARTH_REFERENCE, R1, rM, mu, ORACLE_STEPS);
      const oracle = opticalDepthToTop(EARTH_REFERENCE, R1, rM, mu);
      for (const c of CHANNELS) {
        // The top row's zenith ray has no air to cross: both are 0 there.
        worst = Math.max(
          worst,
          oracle[c] === 0 ? Math.abs(twin[c]) : Math.abs(twin[c] / oracle[c] - 1),
        );
      }
    }
    expect(worst).toBeLessThan(1e-6);
  });

  it("give R05.T12.e's twin's pixels on its 1,046 rays to 10⁻⁶, with its sun and no multiple scattering", () => {
    const pixelOf = twinPixel(EARTH_REFERENCE, R05_SOURCES);
    const off: string[] = [];
    for (const ray of gateRays()) {
      for (const scheme of [HIGH, evenAt(HIGH), LOW, evenAt(LOW)]) {
        const n = kernelOf(ray) === "skyView" ? scheme.skySteps : scheme.marchSteps;
        const twin = pixelOf(ray, scheme.placement, n);
        const r05 = singleScatteringPixel(ray, scheme.placement, n);
        const agrees = CHANNELS.every((c) =>
          r05[c] === 0 ? twin[c] === 0 : Math.abs(twin[c] / r05[c] - 1) < 1e-6,
        );
        if (!agrees) {
          off.push(
            `${ray.name}, ${scheme.placement} ${n}: ${twin.join(", ")} against ${r05.join(", ")}`,
          );
        }
      }
    }
    expect(off).toEqual([]);
  });

  it("hold R05.T12.b's 20 texels within its 1% of the oracle's optical depth at the kernel's 256 steps", () => {
    const table = earth.transmittance;
    const shell = { bottomRadiusM: R1, topRadiusM: R1 + EARTH_REFERENCE.topHeightM };
    let worst = 0;
    for (const [x, y] of ORACLE_TEXELS) {
      const { rM, mu } = transmittanceUvToRMu(
        shell,
        unitFromTexelUv((x + 0.5) / table.widthTexels, table.widthTexels),
        unitFromTexelUv((y + 0.5) / table.heightTexels, table.heightTexels),
      );
      const oracle = opticalDepthToTop(EARTH_REFERENCE, R1, rM, mu);
      for (const c of CHANNELS) {
        // The table stores optical depth since R08.T6.c; the top row's zenith ray holds none.
        const twin = table.texels[(y * table.widthTexels + x) * 4 + c] ?? Number.NaN;
        worst = Math.max(worst, oracle[c] === 0 ? Math.abs(twin) : Math.abs(twin / oracle[c] - 1));
      }
    }
    // Recorded in plan R08's Risks ("Deviations in T6.a, as built"): the midpoint rule's own error.
    expect(worst).toBeLessThan(0.01);
  });
});

// --- R05.T12.e's gate, with the multiple-scattering term -----------------------------------------

/**
 * The ray R08.T6.g pinned as the gate's one exception (`T6C_EXCEPTION`, 6.02%), which R08.T6.c
 * removes (plan R08's Risks, "Deviations in T6.g, as built" and "Deviations in T6.c, as built").
 *
 * @remarks
 * Bruneton's texel mapping puts the transmittance table's last column on the ray tangent to the
 * ground, where T falls by orders of magnitude between texels. Read bilinearly as T, as it was
 * stored until R08.T6.c, the ray stood at 6.01% at high's 75 sky-view steps; the stored optical
 * depth, read bilinearly and exponentiated, holds it at 0.22% (measured 2026-10-11), within the
 * twilight rays' 5%.
 */
const T6G_PINNED_RAY = "band, 20000 m, view zenith 94.441°, sun 95°/0°";

describe("the sky view's and the march's twins under R05.T12.e's quadrature gate", () => {
  let gate: Gate = { results: [], lMax: new Map() };
  /** The daylit sky rays' references without the multiple-scattering term, by ray. */
  let singleOnly = new Map<string, Rgb3>();

  beforeAll(() => {
    const sources = tableSources(planetTwin(EARTH_REFERENCE, BOTTOM_M));
    const rays = gateRays();
    gate = runGate(rays, twinPixel(EARTH_REFERENCE, sources), [HIGH, LOW]);
    const noMultiple = twinPixel(EARTH_REFERENCE, {
      ...sources,
      multiScattering: (_heightM, _muSun, out) => {
        out.fill(0);
      },
    });
    singleOnly = new Map(
      rays
        .filter((ray) => ray.family === "sky" && ray.sunZenithDeg < 80)
        .map((ray) => [ray.name, noMultiple(ray, "placed", REFERENCE_STEPS)]),
    );
  }, 600_000);

  it("includes the multiple-scattering term: every daylit sky ray is brighter with it", () => {
    const dimmer = gate.results.filter(({ ray, reference }) => {
      const single = singleOnly.get(ray.name);
      return single !== undefined && !CHANNELS.every((c) => reference[c] > single[c]);
    });
    expect(singleOnly.size).toBe(40);
    expect(dimmer.map(({ ray }) => ray.name)).toEqual([]);
  });

  it("has a reference of 2,048, 4,096 or 8,192 placed steps within 0.05% of twice its steps", () => {
    const unconverged = gate.results
      .map(({ ray, reference, confirm }) => ({
        ray,
        e: errorOf(reference, confirm, gate.lMax.get(kernelOf(ray)) ?? [0, 0, 0]),
      }))
      .filter(({ e }) => !(e < CONVERGED));
    expect(describeFailures(unconverged)).toEqual([]);
  });

  it("takes the longer reference only for twilight rays", () => {
    const longer = gate.results.filter(({ referenceSteps }) => referenceSteps > REFERENCE_STEPS);
    expect(longer.filter(({ ray }) => !twilight(ray)).map(({ ray }) => ray.name)).toEqual([]);
  });

  it("holds every ray within 2%, and twilight rays within 5%, at high's counts", () => {
    expect(describeFailures(failures(gate, HIGH))).toEqual([]);
  });

  it("holds R08.T6.g's pinned twilight ray within its 5%, with no exception (R08.T6.c)", () => {
    const pinned = errors(gate, HIGH).find(({ ray }) => ray.name === T6G_PINNED_RAY);
    expect(pinned).toBeDefined();
    expect(pinned?.e ?? Number.NaN).toBeLessThanOrEqual(TWILIGHT_TOLERANCE);
  });

  it("keeps low's counts within the bounds low's GPU agreement check holds them to", () => {
    expect(describeFailures(overLowBounds(gate, LOW))).toEqual([]);
  });
});

// --- The multiple-scattering kernel --------------------------------------------------------------

/** The texels the steps are measured at: every fourth row and column, and the last of each. */
const MEASURED_TEXELS = [0, 4, 8, 12, 16, 20, 24, 28, 31].flatMap((y) =>
  [0, 4, 8, 12, 16, 20, 24, 28, 31].map((x) => [x, y] as const),
);

/** The placed reference's steps, confirmed against twice as many. */
const MS_REFERENCE_STEPS = 512;

/** A table's texel (x, y), RGB. */
function at(t: TwinTable, x: number, y: number): Rgb3 {
  const i = (y * t.widthTexels + x) * 4;
  return [t.texels[i] ?? Number.NaN, t.texels[i + 1] ?? Number.NaN, t.texels[i + 2] ?? Number.NaN];
}

/**
 * The worst e = max over channels of |L − L_ref| ÷ max(L_ref, 10⁻³ L_max) over the measured
 * texels, L_max the reference's brightest measured texel per channel (R05.T12.e's metric).
 */
function worstOver(table: TwinTable, reference: TwinTable): number {
  const lMax: Rgb3 = [0, 0, 0];
  for (const [x, y] of MEASURED_TEXELS) {
    const ref = at(reference, x, y);
    for (const c of CHANNELS) {
      lMax[c] = Math.max(lMax[c], ref[c]);
    }
  }
  return Math.max(
    ...MEASURED_TEXELS.map(([x, y]) => errorOf(at(table, x, y), at(reference, x, y), lMax)),
  );
}

describe("the multiple-scattering kernel's steps", () => {
  const tables = new Map<string, TwinTable>();
  const build = (options: Partial<MultiScatteringOptions>): TwinTable =>
    multiScatteringTwin(
      EARTH_REFERENCE,
      R1,
      earth,
      { ...MULTI_SCATTERING_KERNEL_OPTIONS, ...options },
      MEASURED_TEXELS,
    );

  beforeAll(() => {
    // The kernel's own steps are the planet's table: `planetTwin` builds it by the same
    // `multiScatteringTwin` of the same medium, radius, transmittance and options, each texel on
    // its own, so at the measured texels it is this table's `build({})` bit for bit.
    tables.set("even", earth.multiScattering);
    tables.set("placed", build({ placement: "placed" }));
    tables.set("reference", build({ placement: "placed", samples: MS_REFERENCE_STEPS }));
    tables.set("confirm", build({ placement: "placed", samples: 2 * MS_REFERENCE_STEPS }));
  }, 600_000);

  const table = (name: string): TwinTable => {
    const found = tables.get(name);
    if (found === undefined) {
      throw new Error(`no ${name} table`);
    }
    return found;
  };

  it("has a placed reference of 512 steps within 0.05% of 1,024", () => {
    expect(worstOver(table("reference"), table("confirm"))).toBeLessThan(CONVERGED);
  });

  // Measured on 2026-10-09 and recorded in plan R08's Risks ("Deviations in T6.a, as built"): over
  // these 81 texels 10.1% for the kernel's even steps and 2.0% for placed ones (over all 1,024,
  // 10.5% and 2.6%), the even steps' worst at the ground with the sun 84°–88° from the zenith.
  it("measures the kernel's 32 even steps against it: about 10% at the ground at sunset", () => {
    const e = worstOver(table("even"), table("reference"));
    expect(e).toBeGreaterThan(0.09);
    expect(e).toBeLessThan(0.11);
  });

  it("measures 32 placed steps against it, for comparison: about 2%", () => {
    expect(worstOver(table("placed"), table("reference"))).toBeLessThan(0.025);
  });
});

describe("the multiple-scattering kernel's step factor", () => {
  it("takes R05.T12.e's stable step factor from common.wgsl", () => {
    expect(multiScatteringWgsl).toMatch(
      /luminance \+= throughput \* source \* dt \* stepFactor\(stepDepth\);/,
    );
    expect(multiScatteringWgsl).toMatch(
      /transferSum \+= throughput \* local\.scattering \* dt \* stepFactor\(stepDepth\);/,
    );
    expect(multiScatteringWgsl).not.toMatch(/source - source \* stepTransmittance/);
    expect(multiScatteringWgsl).not.toMatch(/max\(local\.extinction, vec3f\(1e-12\)\)/);
  });

  it("moves the f32 table by up to 5% through the old form's cancellation, and the new by 10⁻⁶", () => {
    const exact = earth.multiScattering;
    const withFactor = (stepFactor: (x: number) => number): TwinTable =>
      multiScatteringTwin(EARTH_REFERENCE, R1, earth, {
        ...MULTI_SCATTERING_KERNEL_OPTIONS,
        stepFactor,
      });
    const stable = worstRelative(withFactor(stepFactorF32).texels, exact.texels);
    // A ray of no length from the top adds 0 in the old form, (S − S) ÷ σ_t, where 0 ÷ 0 is NaN.
    const old = worstRelative(
      withFactor((x) => (x > 0 ? oldFactorF32(x) : 1)).texels,
      exact.texels,
    );
    // Measured on 2026-10-09 and recorded in plan R08's Risks ("Deviations in T6.a, as built"):
    // 9.7 × 10⁻⁷ and 4.9%, the old form's worst on the night side, the sun 13° below the horizon.
    expect(stable).toBeLessThan(2e-6);
    expect(old).toBeGreaterThan(0.01);
    expect(old).toBeLessThan(0.06);
  }, 60_000);
});

// --- Over N terms --------------------------------------------------------------------------------

/** A gas from the ground: a tabulated exponential of an 8 km scale height, to 100 km in 250 m. */
const GAS_LEVELS_M = Float64Array.from({ length: 401 }, (_, i) => i * 250);
const GAS: MediumTerm = {
  name: "gas",
  density: tabulatedDensity(
    GAS_LEVELS_M,
    GAS_LEVELS_M.map((h) => Math.exp(-h / 8_000)),
  ),
  scattering: [5e-6, 1.2e-5, 3e-5],
  absorption: [0, 0, 0],
  phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
};

/** A Henyey–Greenstein phase of asymmetry g tabulated on 256 entries even in u = √(θ ÷ π). */
function henyeyGreensteinTable(g: number): PhaseFunction {
  const u = Float64Array.from({ length: 256 }, (_, i) => i / 255);
  const values = u.map((x) => {
    const mu = Math.cos(Math.PI * x * x);
    return (1 - g * g) / (4 * Math.PI * (1 + g * g - 2 * g * mu) ** 1.5);
  });
  return { kind: "tabulated", table: phaseTable(u, [values, values.map((v) => v * 0.9), values]) };
}

/** A haze layer from 1 to 6 km, peaking at 2 km, with a tabulated phase. */
const HAZE: MediumTerm = {
  name: "haze",
  density: tabulatedDensity(Float64Array.of(1_000, 2_000, 6_000), Float64Array.of(0, 1, 0)),
  scattering: [3e-5, 3.5e-5, 4e-5],
  absorption: [3e-6, 2e-6, 1e-6],
  phase: henyeyGreensteinTable(0.7),
};

/** An absorber with no phase, a tent from 15 to 45 km. */
const ABSORBER: MediumTerm = {
  name: "absorber",
  density: { kind: "tent", bottomM: 15_000, peakM: 25_000, topM: 45_000 },
  scattering: [0, 0, 0],
  absorption: [1e-6, 2e-6, 0.2e-6],
  phase: { kind: "none" },
};

const medium = (terms: readonly MediumTerm[]): AtmosphereMedium => ({
  name: "test",
  topHeightM: 100_000,
  groundAlbedo: [0.2, 0.15, 0.1],
  terms,
});

/** A term at a fraction of its density: its relative density scaled. */
function scaledTerm(term: MediumTerm, fraction: number, name: string): MediumTerm {
  const profile = term.density;
  if (profile.kind !== "tabulated") {
    throw new Error(`term ${term.name} is not tabulated`);
  }
  return {
    ...term,
    name,
    density: tabulatedDensity(
      profile.altitudesM,
      profile.relative.map((d) => d * fraction),
    ),
  };
}

/** A camera `heightM` above the equator at longitude 0 of `figure`, looking north along the horizon. */
function equatorCamera(figure: SpheroidFigure, heightM: number): AtmosphereCamera {
  return {
    positionM: { x: figure.equatorialRadiusM + heightM, y: 0, z: 0 },
    // Camera −z (forward) to body +z (north), camera +y (up) to body +x (the normal): a half turn
    // about (1, 1, 0) ÷ √2.
    orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
    fovXRad: 2 * Math.atan(2),
    viewport: { widthPx: 16, heightPx: 8 },
  };
}

/** The sun at an elevation above an equator camera's horizon, towards the north. */
function sunAt(elevationDeg: number): SunState {
  const e = (elevationDeg * Math.PI) / 180;
  return {
    directionBodyFixed: { x: Math.sin(e), y: 0, z: Math.cos(e) },
    distanceAu: 1,
    angularRadiusRad: 0.004_650_5,
  };
}

/** A small transmittance table for the tests that build many. */
const SMALL_TRANSMITTANCE = { widthTexels: 32, heightTexels: 8 } as const;

/** Every twin's output for a medium, at small sizes, from a camera at 3 km and from 300 km. */
function everyTwin(m: AtmosphereMedium): readonly TwinTable[] {
  const tables = transmittanceTablesTwin(m, R1, NO_ABSORBER_CURVES, SMALL_TRANSMITTANCE, 64);
  const multiScattering = multiScatteringTwin(m, R1, tables, {
    ...MULTI_SCATTERING_KERNEL_OPTIONS,
    size: { widthTexels: 8, heightTexels: 8 },
    samples: 16,
  });
  const planet: PlanetTwin = { medium: m, bottomRadiusM: R1, ...tables, multiScattering };
  const transmittance = tables.transmittance;
  const low = twinView(equatorCamera(WGS84, 3_000), sunAt(20), WGS84, 0.1);
  const high = twinView(equatorCamera(WGS84, 300_000), sunAt(5), WGS84, 0.1);
  const settings = { ...TABLE_SIZES.low, rayMarchSamples: 24 };
  return [
    transmittance,
    multiScattering,
    skyViewTwin(planet, low, { widthTexels: 16, heightTexels: 12 }, 24),
    aerialPerspectiveTwin(planet, low, { widthTexels: 8, heightTexels: 4, slices: 6 }, 2, 32_000),
    rayMarchTwin(planet, high, settings, () => 0),
    rayMarchTwin(planet, low, settings, (_x, y) => (y < 4 ? 0 : 0.1 / 60_000)),
  ];
}

describe("the twin over N terms", () => {
  // The three-term medium's tables, built once for the two tests that compare against them: a
  // pure function of the medium, which neither test writes to.
  let wholeTables: readonly TwinTable[] | undefined;
  const wholeTwins = (): readonly TwinTable[] =>
    (wholeTables ??= everyTwin(medium([GAS, HAZE, ABSORBER])));

  it("gives two identical half-density terms the tables of one", () => {
    const whole = wholeTwins();
    const halves = everyTwin(
      medium([GAS, scaledTerm(HAZE, 0.5, "haze a"), scaledTerm(HAZE, 0.5, "haze b"), ABSORBER]),
    );
    for (const [i, table] of whole.entries()) {
      const other = halves[i];
      expect(other?.texels.length).toBe(table.texels.length);
      expect(worstRelative(other?.texels ?? new Float64Array(0), table.texels)).toBeLessThan(1e-12);
    }
  }, 60_000);

  it("takes more terms than the kernels' uniform holds: a term split in ten is the term", () => {
    const tenths = Array.from({ length: 10 }, (_, k) => scaledTerm(HAZE, 0.1, `haze ${k}`));
    const whole = wholeTwins();
    const split = everyTwin(medium([GAS, ...tenths, ABSORBER]));
    for (const [i, table] of whole.entries()) {
      expect(worstRelative(split[i]?.texels ?? new Float64Array(0), table.texels)).toBeLessThan(
        1e-10,
      );
    }
  }, 60_000);

  it("adds optical depth across terms", () => {
    const terms = [GAS, HAZE, ABSORBER];
    for (const [rM, mu] of [
      [R1 + 2, 1],
      [R1 + 2, 0.05],
      [R1 + 2_500, -0.02],
      [R1 + 40_000, -0.3],
    ] as const) {
      const all = kernelOpticalDepth(medium(terms), R1, rM, mu);
      const each = terms.map((term) => kernelOpticalDepth(medium([term]), R1, rM, mu));
      for (const c of CHANNELS) {
        const sum = each.reduce((total, depth) => total + depth[c], 0);
        expect(Math.abs(all[c] / sum - 1)).toBeLessThan(1e-12);
      }
    }
  });
});

// --- Tabulated densities and phases --------------------------------------------------------------

describe("tabulated densities and phases", () => {
  it("reads a finely tabulated density as its closed form", () => {
    const levels = Float64Array.from({ length: 4_001 }, (_, i) => i * 25);
    const closed: MediumTerm = { ...GAS, density: { kind: "exponential", scaleHeightM: 8_000 } };
    const table: MediumTerm = {
      ...GAS,
      density: tabulatedDensity(
        levels,
        levels.map((h) => Math.exp(-h / 8_000)),
      ),
    };
    for (const mu of [1, 0.2, 0.01, -0.01]) {
      const a = kernelOpticalDepth(medium([table]), R1, R1 + 10, mu);
      const b = kernelOpticalDepth(medium([closed]), R1, R1 + 10, mu);
      // Linear interpolation of e^(−h/H) at Δh = 25 m errs by (Δh/H)² ÷ 8 = 1.2 × 10⁻⁶.
      expect(Math.abs(a[1] / b[1] - 1)).toBeLessThan(2e-6);
    }
  });

  it("reads a finely tabulated phase as its closed form", () => {
    const u = Float64Array.from({ length: 256 }, (_, i) => i / 255);
    const rayleigh = u.map((x) => (3 / (16 * Math.PI)) * (1 + Math.cos(Math.PI * x * x) ** 2));
    const tabulated: MediumTerm = {
      ...GAS,
      phase: { kind: "tabulated", table: phaseTable(u, [rayleigh, rayleigh, rayleigh]) },
    };
    const view = twinView(equatorCamera(WGS84, 2), sunAt(30), WGS84, 0.1);
    const sky = (term: MediumTerm): TwinTable => {
      const m = medium([term]);
      const size = { widthTexels: 64, heightTexels: 16 };
      const tables = transmittanceTablesTwin(m, R1, NO_ABSORBER_CURVES, size, 64);
      const multiScattering = multiScatteringTwin(m, R1, tables);
      return skyViewTwin(
        { medium: m, bottomRadiusM: R1, ...tables, multiScattering },
        view,
        { widthTexels: 24, heightTexels: 16 },
        32,
      );
    };
    // Linear in u at 1/255 errs by under max|p''| ÷ p × Δu² ÷ 8 = 42.2 ÷ (8 × 255²) = 8.1 × 10⁻⁵
    // of the phase, at θ ≈ 98°.
    expect(worstRelative(sky(tabulated).texels, sky(GAS).texels)).toBeLessThan(1e-4);
  }, 60_000);

  it("builds the tables on R08.T3.a's column, whose last level bounds the medium's top", () => {
    const column = hydrostaticColumn({
      surfacePa: 101_325,
      temperature: { kind: "isothermal", temperatureK: 250 },
      meanMolarMassGPerMol: 28.97,
      referenceGravityMS2: 9.806,
      referenceRadiusM: R1,
    });
    const term: MediumTerm = { ...GAS, density: column.density };
    const fits: AtmosphereMedium = { ...medium([term]), topHeightM: column.topHeightM };
    const tables = transmittanceTwin(fits, R1, { widthTexels: 16, heightTexels: 8 }, 64);
    expect(tables.texels.every((v) => Number.isFinite(v) && v >= 0)).toBe(true);
    const over: AtmosphereMedium = { ...fits, topHeightM: column.topHeightM + 1 };
    expect(() => transmittanceTwin(over, R1)).toThrow(RangeError);
    expect(() => planetTwin(over, R1)).toThrow(/holds a density/);
  });

  it("lets a tabulated layer that falls to 0 end below the top", () => {
    expect(() => kernelOpticalDepth(medium([HAZE]), R1, R1, 1)).not.toThrow();
  });
});

// --- The steps the twins place -------------------------------------------------------------------

describe("the per-frame twins' steps", () => {
  const view = twinView(equatorCamera(WGS84, 5_000), sunAt(10), WGS84, 0.1);

  it("place the sky view's steps from the camera, as skyView.wgsl calls marchSplit", () => {
    const size = { widthTexels: 12, heightTexels: 10 };
    const table = skyViewTwin(earth, view, size, 16);
    const shell = {
      bottomRadiusM: view.cameraRadiusM,
      topRadiusM: view.cameraRadiusM + EARTH_REFERENCE.topHeightM,
    };
    for (const [x, y] of [
      [0, 0],
      [5, 4],
      [11, 5],
      [3, 9],
    ] as const) {
      const ray = skyViewRayTwin(EARTH_REFERENCE, view, x, y, size);
      const rM = ray.originM.z;
      expect(ray.fromCamera).toBe(true);
      expect(ray.tStartM).toBe(0);
      expect(ray.tEndM).toBe(maxDistanceM(shell, rM, ray.direction.z));
      const split = marchSplit(0, ray.tEndM, -rM * ray.direction.z, 16, true);
      const steps = Array.from({ length: 16 }, (_, i) => marchStep(split, i));
      const marched = marchTwin(
        EARTH_REFERENCE,
        tableSources(earth),
        { kind: "sphere", bottomRadiusM: view.cameraRadiusM },
        ray,
        steps,
      );
      for (const c of CHANNELS) {
        expect(table.texels[(y * size.widthTexels + x) * 4 + c]).toBe(marched.luminance[c]);
      }
    }
  });

  it("start the ray march from orbit toward its lowest point, and end it on the bare ground", () => {
    const orbit = twinView(equatorCamera(WGS84, 400_000), sunAt(30), WGS84, 0.1);
    const down = rayMarchRayTwin(EARTH_REFERENCE, orbit, 0.5, 0.9, 0, 32_000);
    expect(down?.ray.fromCamera).toBe(false);
    expect(down?.bareGroundM).not.toBeNull();
  });

  it("start the ray march at a camera inside the air, toward a surface beyond the reach", () => {
    const inside = rayMarchRayTwin(EARTH_REFERENCE, view, 0.5, 0.5, 0.1 / 60_000, 32_000);
    expect(inside?.ray.fromCamera).toBe(true);
    expect(inside?.ray.tStartM).toBe(0);
  });

  it("march nothing from inside the air to a surface within the aerial-perspective reach", () => {
    expect(rayMarchRayTwin(EARTH_REFERENCE, view, 0.5, 0.5, 0.1 / 20_000, 32_000)).toBeNull();
  });

  it("march each ray-march texel on the spheroid over its placed steps, as rayMarch.wgsl does", () => {
    const orbit = twinView(equatorCamera(WGS84, 400_000), sunAt(30), WGS84, 0.1);
    const settings = { ...TABLE_SIZES.high, rayMarchSamples: 24 };
    const target = rayMarchTwin(earth, orbit, settings, () => 0);
    const sources = tableSources(earth);
    let compared = 0;
    for (let i = 0; i < target.widthTexels * target.heightTexels; i += 1) {
      const x = i % target.widthTexels;
      const y = Math.floor(i / target.widthTexels);
      const marched = rayMarchRayTwin(
        EARTH_REFERENCE,
        orbit,
        (x + 0.5) / target.widthTexels,
        (y + 0.5) / target.heightTexels,
        0,
        settings.aerialPerspectiveReachM,
      );
      if (marched === null || marched.bareGroundM !== null) {
        continue;
      }
      const { luminance } = marchTwin(
        EARTH_REFERENCE,
        sources,
        { kind: "spheroid", figure: WGS84 },
        marched.ray,
        placedSteps(marched.ray, settings.rayMarchSamples),
      );
      expect(at(target, x, y)).toEqual([...luminance]);
      compared += 1;
    }
    expect(compared).toBeGreaterThan(0);
  });

  it("reduce the ray march's spheroid to the sky view's sphere when a = c", () => {
    const sphere: SpheroidFigure = { equatorialRadiusM: R1, polarRadiusM: R1 };
    const orbit = twinView(equatorCamera(sphere, 400_000), sunAt(30), sphere, 0.1);
    // A ray through the middle column dipping to a tangent point 20 km up: the view's tan(fov_y ÷ 2)
    // is 1, so v = (1 + tan(dip)) ÷ 2.
    const dip = Math.acos((R1 + 20_000) / (R1 + 400_000));
    const marched = rayMarchRayTwin(
      EARTH_REFERENCE,
      orbit,
      0.5,
      (1 + Math.tan(dip)) / 2,
      0,
      32_000,
    );
    if (marched === null) {
      throw new Error("the limb ray misses the atmosphere");
    }
    expect(marched.bareGroundM).toBeNull();
    const steps = placedSteps(marched.ray, 32);
    const sources = tableSources(earth);
    const onSpheroid = marchTwin(
      EARTH_REFERENCE,
      sources,
      { kind: "spheroid", figure: sphere },
      marched.ray,
      steps,
    );
    const onSphere = marchTwin(
      EARTH_REFERENCE,
      sources,
      { kind: "sphere", bottomRadiusM: R1 },
      marched.ray,
      steps,
    );
    for (const c of CHANNELS) {
      expect(onSpheroid.luminance[c] / onSphere.luminance[c]).toBeCloseTo(1, 10);
    }
  });
});

// --- Earth at R05's sizes ------------------------------------------------------------------------

describe("Earth's tables at R05's sizes", () => {
  it("builds the per-planet tables at Hillaire's 256 × 64 and 32²", () => {
    const { transmittance, multiScattering } = earth;
    expect([transmittance.widthTexels, transmittance.heightTexels]).toEqual([256, 64]);
    expect([multiScattering.widthTexels, multiScattering.heightTexels]).toEqual([32, 32]);
  });

  it("keeps every stored optical depth finite and non-negative, and no absorber column", () => {
    const texels = earth.transmittance.texels;
    expect(texels.every((v) => Number.isFinite(v) && v >= 0)).toBe(true);
    expect(texels.filter((_, i) => i % 4 === 3).every((column) => column === 0)).toBe(true);
    expect(earth.absorberColumns.texels.every((v) => v === 0)).toBe(true);
  });

  it("keeps every multiple-scattering texel finite and non-negative", () => {
    expect(earth.multiScattering.texels.every((v) => Number.isFinite(v) && v >= 0)).toBe(true);
  });

  it("lights multiple scattering at the ground under an overhead sun", () => {
    // The first row's last texel: the sun overhead, at the ground.
    const [r, g, b] = at(earth.multiScattering, 31, 0);
    expect(Math.min(r, g, b)).toBeGreaterThan(0);
  });

  it("draws a noon sky from the ground, bluer than red, on both settings", () => {
    for (const setting of ["high", "low"] as const) {
      const sizes = TABLE_SIZES[setting];
      const view = twinView(equatorCamera(WGS84, 2), sunAt(60), WGS84, 0.1);
      const sky = skyViewTwin(earth, view, sizes.skyView, sizes.skyViewSamples);
      expect(sky.texels.every((v) => Number.isFinite(v) && v >= 0)).toBe(true);
      // The first row is the zenith (sebh's parameterisation puts it at v = 0).
      const [red, , blue] = at(sky, 0, 0);
      expect(blue).toBeGreaterThan(red);
    }
  }, 60_000);

  it("thickens the aerial perspective's haze and thins its transmittance slice by slice", () => {
    const sizes = TABLE_SIZES.high;
    const view = twinView(equatorCamera(WGS84, 2), sunAt(30), WGS84, 0.1);
    const volume = aerialPerspectiveTwin(
      earth,
      view,
      sizes.aerialPerspective,
      sizes.aerialPerspectiveSamplesPerSlice,
      sizes.aerialPerspectiveReachM,
    );
    const { widthTexels: w, heightTexels: h, slices } = volume;
    for (const [x, y] of [
      [16, 16],
      [3, 20],
    ] as const) {
      for (let z = 1; z < slices; z += 1) {
        const near = ((z - 1) * h + y) * w + x;
        const far = (z * h + y) * w + x;
        expect(volume.texels[far * 4 + 2] ?? 0).toBeGreaterThan(volume.texels[near * 4 + 2] ?? 0);
        expect(volume.texels[far * 4 + 3] ?? 1).toBeLessThan(volume.texels[near * 4 + 3] ?? 0);
      }
    }
  });

  it("marches the disc and the limb from orbit, seeing no space through the ground", () => {
    const view = twinView(equatorCamera(WGS84, 400_000), sunAt(30), WGS84, 0.1);
    const target = rayMarchTwin(earth, view, TABLE_SIZES.high, () => 0);
    expect([target.widthTexels, target.heightTexels]).toEqual([16, 8]);
    expect(target.texels.every((v) => Number.isFinite(v) && v >= 0)).toBe(true);
    // The bottom row looks down at the lit disc: radiance, and no transmittance to space.
    const disc = (7 * 16 + 8) * 4;
    expect(target.texels[disc + 2] ?? 0).toBeGreaterThan(0);
    expect(target.texels[disc + 3]).toBe(0);
  });
});

// --- The kernels' constants ----------------------------------------------------------------------

describe("the kernels' constants", () => {
  it("are the twin's: the multiple-scattering table's ground offset and directions", () => {
    expect(commonWgsl).toContain(
      `const GROUND_OFFSET_M: f32 = ${MULTI_SCATTERING_GROUND_OFFSET_M.toFixed(1)};`,
    );
    expect(multiScatteringWgsl).toContain(
      `const DIRECTIONS : u32 = ${MULTI_SCATTERING_DIRECTIONS}u;`,
    );
    expect(multiScatteringWgsl).toContain(
      `const SQRT_DIRECTIONS : u32 = ${Math.sqrt(MULTI_SCATTERING_DIRECTIONS)}u;`,
    );
  });
});

/** The unit vectors the tests build their views from are unit length. */
function unitLength(v: Vec3): number {
  return Math.hypot(v.x, v.y, v.z);
}

describe("twinView", () => {
  it("fills the view as HillaireAtmosphere does: a unit forward, the camera's Gaussian sphere", () => {
    const view: TwinView = twinView(equatorCamera(WGS84, 2), sunAt(30), WGS84, 0.1);
    expect(unitLength(view.forward)).toBeCloseTo(1, 12);
    expect(view.forward.z).toBeCloseTo(1, 12);
    expect(view.heightM).toBeCloseTo(2, 6);
    // √(MN) at the equator is b = c: M = c² ÷ a and N = a there.
    expect(view.cameraRadiusM).toBeCloseTo(WGS84.polarRadiusM, 3);
  });
});

// --- Bruneton's texel mapping (plan R08, Design note 19, R08.T6.g) ---------------------------------

/** A table's texel channel, row by row. */
function texelOf(table: TwinTable, x: number, y: number, c: number): number {
  return table.texels[(y * table.widthTexels + x) * 4 + c] ?? Number.NaN;
}

/** A bilinear read at uv in texel coordinates, centres at (i + ½) ÷ size, clamped: `bilinear`. */
function bilinearAt(table: TwinTable, u: number, v: number, c: number): number {
  const { widthTexels: w, heightTexels: h } = table;
  const px = Math.min(Math.max(u * w - 0.5, 0), w - 1);
  const py = Math.min(Math.max(v * h - 0.5, 0), h - 1);
  const x0 = Math.floor(px);
  const y0 = Math.floor(py);
  const x1 = Math.min(x0 + 1, w - 1);
  const y1 = Math.min(y0 + 1, h - 1);
  const fx = px - x0;
  const fy = py - y0;
  const a = texelOf(table, x0, y0, c) * (1 - fx) + texelOf(table, x1, y0, c) * fx;
  const b = texelOf(table, x0, y1, c) * (1 - fx) + texelOf(table, x1, y1, c) * fx;
  return a * (1 - fy) + b * fy;
}

/**
 * R05's transmittance table, kept as a reference: texel centres at (i + ½) ÷ size standing for
 * the unit range itself, with no texel mapping, and read the same way, so that the clamp held the
 * outer half texels.
 */
function r05Transmittance(bottomM: number): TwinTable {
  const { widthTexels: w, heightTexels: h } = TRANSMITTANCE_SIZE;
  const shell = { bottomRadiusM: bottomM, topRadiusM: bottomM + EARTH_REFERENCE.topHeightM };
  const texels = new Float64Array(w * h * 4);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const { rM, mu } = transmittanceUvToRMu(shell, (x + 0.5) / w, (y + 0.5) / h);
      const tau = kernelOpticalDepth(EARTH_REFERENCE, bottomM, rM, mu);
      texels.set([Math.exp(-tau[0]), Math.exp(-tau[1]), Math.exp(-tau[2]), 1], (y * w + x) * 4);
    }
  }
  return { widthTexels: w, heightTexels: h, texels };
}

/** The sun's transmittance from R05's table at a height and zenith cosine, as R05 read it. */
function r05SunTransmittance(table: TwinTable, bottomM: number, heightM: number, mu: number): Rgb3 {
  const shell = { bottomRadiusM: bottomM, topRadiusM: bottomM + EARTH_REFERENCE.topHeightM };
  const [u, v] = transmittanceRMuToUv(shell, bottomM + heightM, mu);
  return [bilinearAt(table, u, v, 0), bilinearAt(table, u, v, 1), bilinearAt(table, u, v, 2)];
}

/** The heights and elevations above the horizontal at which the sun's transmittance is held. */
const EDGE_HEIGHTS_M = [2, 20, 200, 2_000] as const;
const EDGE_ELEVATIONS_DEG = [0, 0.25, 1, 2, 5, 45, 85, 90] as const;

/** The worst relative error per channel of a sun transmittance against the oracle's. */
function edgeError(
  transmittanceAt: (heightM: number, mu: number) => Rgb3,
  heightM: number,
  elevationDeg: number,
): number {
  const mu = Math.sin((elevationDeg * Math.PI) / 180);
  const oracle = opticalDepth(EARTH_REFERENCE, R1, R1 + heightM, mu).map((tau) => Math.exp(-tau));
  const t = transmittanceAt(heightM, mu);
  return Math.max(...CHANNELS.map((c) => Math.abs(t[c] / (oracle[c] ?? Number.NaN) - 1)));
}

/** A table's largest colour channel. */
function largestOf(table: TwinTable): number {
  let largest = 0;
  for (const [i, value] of table.texels.entries()) {
    largest = i % 4 === 3 ? largest : Math.max(largest, value);
  }
  return largest;
}

describe("Bruneton's texel mapping in the twin (Design note 19)", () => {
  // A read stands within about 10⁻¹² of a texel of its centre in f64, so a texel beside one 10⁵ to
  // 10⁷ times larger reads back off by that much of itself: T near 10⁻¹⁸ at the grazing column,
  // multiple scattering near 10⁻²² on the night side. They are held to 10⁻⁹ of the twin bound's
  // floor, 10⁻⁶ of the table's largest (Verification), far under anything a table stores.
  it("reads every transmittance texel back at its own (h, μ) to 10⁻⁹, as e^(−τ) of its τ", () => {
    const sources = tableSources(earth);
    const shell = { bottomRadiusM: R1, topRadiusM: R1 + EARTH_REFERENCE.topHeightM };
    const { widthTexels: w, heightTexels: h } = earth.transmittance;
    // The table's largest transmittance is 1, at the top row's zenith.
    const floor = 1e-6;
    const out: Rgb3 = [0, 0, 0];
    let worst = 0;
    for (let y = 0; y < h; y += 1) {
      for (let x = 0; x < w; x += 1) {
        const { rM, mu } = transmittanceUvToRMu(
          shell,
          unitFromTexelUv((x + 0.5) / w, w),
          unitFromTexelUv((y + 0.5) / h, h),
        );
        sources.sunTransmittance(rM - R1, mu, out);
        for (const c of CHANNELS) {
          const stored = Math.exp(-texelOf(earth.transmittance, x, y, c));
          worst = Math.max(worst, Math.abs(out[c] - stored) / Math.max(stored, floor));
        }
      }
    }
    expect(worst).toBeLessThan(1e-9);
  });

  it("reads every multiple-scattering texel back at its own (h, μ_s) to 10⁻⁹", () => {
    const sources = tableSources(earth);
    const { widthTexels: w, heightTexels: h } = earth.multiScattering;
    const top = EARTH_REFERENCE.topHeightM;
    const floor = 1e-6 * largestOf(earth.multiScattering);
    const out: Rgb3 = [0, 0, 0];
    let worst = 0;
    for (let y = 0; y < h; y += 1) {
      for (let x = 0; x < w; x += 1) {
        const heightM =
          MULTI_SCATTERING_GROUND_OFFSET_M +
          unitFromTexelUv((y + 0.5) / h, h) * (top - MULTI_SCATTERING_GROUND_OFFSET_M);
        const muSun = unitFromTexelUv((x + 0.5) / w, w) * 2 - 1;
        sources.multiScattering(heightM, muSun, out);
        for (const c of CHANNELS) {
          const stored = texelOf(earth.multiScattering, x, y, c);
          worst = Math.max(worst, Math.abs(out[c] - stored) / Math.max(stored, floor));
        }
      }
    }
    expect(worst).toBeLessThan(1e-9);
  });

  // Read as T, as the table was stored until R08.T6.c, the bottom rows and last columns change by
  // ×3 (red) to ×10 (blue) a texel near the horizon, and a bilinear read overshot: from 2, 20 and
  // 200 m at 0°, +48.7%, +41.0% and +12.0% (R05's mapping: −37.5%, +50.1% and +12.3%), so that
  // R08.T6.g held the points below 2° on the table read as optical depth alone. Stored as optical
  // depth since R08.T6.c, the stored read holds decision-r08-remap.md §6's 3% at every point (plan
  // R08's Risks, "Deviations in T6.g, as built" and "Deviations in T6.c, as built").
  it("holds the stored sun's transmittance within 3% of the oracle up to 5°, and 0.3% from 45° up", () => {
    const sources = tableSources(earth);
    const stored = (heightM: number, mu: number): Rgb3 => {
      const out: Rgb3 = [0, 0, 0];
      sources.sunTransmittance(heightM, mu, out);
      return out;
    };
    const off: string[] = [];
    for (const heightM of EDGE_HEIGHTS_M) {
      for (const elevationDeg of EDGE_ELEVATIONS_DEG) {
        const e = edgeError(stored, heightM, elevationDeg);
        if (!(e <= (elevationDeg >= 45 ? 0.003 : 0.03))) {
          off.push(`${heightM} m at ${elevationDeg}°: ${(e * 100).toFixed(3)}%`);
        }
      }
    }
    expect(off).toEqual([]);
  });

  it("finds R05's mapping over 10% off from 2 m at the horizon and over 0.3% off at the zenith", () => {
    const table = r05Transmittance(R1);
    const r05 = (heightM: number, mu: number): Rgb3 => r05SunTransmittance(table, R1, heightM, mu);
    expect(edgeError(r05, 2, 0)).toBeGreaterThan(0.1);
    expect(edgeError(r05, 2, 90)).toBeGreaterThan(0.003);
  });

  it("maps no axis of fewer than 2 texels", () => {
    const axes = [
      TRANSMITTANCE_SIZE.widthTexels,
      TRANSMITTANCE_SIZE.heightTexels,
      MULTI_SCATTERING_SIZE.widthTexels,
      MULTI_SCATTERING_SIZE.heightTexels,
      ...(["high", "low"] as const).flatMap((setting) => {
        const sizes = TABLE_SIZES[setting];
        return [
          sizes.skyView.widthTexels,
          sizes.skyView.heightTexels,
          sizes.aerialPerspective.widthTexels,
          sizes.aerialPerspective.heightTexels,
          sizes.aerialPerspective.slices,
        ];
      }),
    ];
    expect(Math.min(...axes)).toBeGreaterThanOrEqual(2);
  });
});

// --- Optical depth and curves of growth (plan R08, Design notes 5 and 8, R08.T6.c) ----------------

/** The relative difference of a over b, or |a| where b is 0. */
function relative(a: number, b: number): number {
  return b === 0 ? Math.abs(a) : Math.abs(a / b - 1);
}

/** A transmittance texel's transmittance per channel, its stored τ and amounts under slot 0's curves. */
function texelTransmittance(tables: TwinTransmittance, of: AtmosphereMedium, i: number): Rgb3 {
  const relativeAmounts = Float64Array.from([
    tables.transmittance.texels[i * 4 + 3] ?? Number.NaN,
    ...tables.absorberColumns.texels.subarray(i * 4, i * 4 + 4),
  ]);
  const slots = absorberSlots(of);
  const curves = tables.curves[0] ?? [];
  const amounts = slots.map((slot, k) =>
    scaledAmount(slot.verticalAmount, relativeAmounts[k] ?? 0),
  );
  const depth: Rgb3 = slots.length === 0 ? [0, 0, 0] : [...curvesOpticalDepth(curves, amounts)];
  const tau = (c: number): number => tables.transmittance.texels[i * 4 + c] ?? Number.NaN;
  return [
    Math.exp(-(tau(0) + depth[0])),
    Math.exp(-(tau(1) + depth[1])),
    Math.exp(-(tau(2) + depth[2])),
  ];
}

describe("R05's Earth with its ozone as an absorber curve in the linear regime", () => {
  const { medium: withOzone, curves } = r05EarthWithOzoneAbsorber();
  let planet: PlanetTwin = earth;

  beforeAll(() => {
    planet = planetTwin(withOzone, R1, curves);
  }, 60_000);

  it("reproduces R05's transmittance, as R08.T6.g's twin leaves it, at every texel to 10⁻³", () => {
    // R05's: e^(−τ) of R05's medium at the same texels, the table R08.T6.g's twin stored.
    let worst = 0;
    const count = planet.transmittance.texels.length / 4;
    for (let i = 0; i < count; i += 1) {
      const t = texelTransmittance(planet, withOzone, i);
      for (const c of CHANNELS) {
        const r05 = Math.exp(-(earth.transmittance.texels[i * 4 + c] ?? Number.NaN));
        worst = Math.max(worst, relative(t[c], r05));
      }
    }
    expect(worst).toBeLessThan(1e-3);
  });

  it("stores the ozone's column in alpha, its whole 300 DU straight up from the ground", () => {
    // Texel (0, 0): the ground, straight up; a tent's midpoint-rule column is exact to its kinks.
    expect(planet.transmittance.texels[3] ?? Number.NaN).toBeCloseTo(1, 3);
  });

  it("reproduces R05's multiple scattering to 10⁻³", () => {
    expect(worstRelative(planet.multiScattering.texels, earth.multiScattering.texels)).toBeLessThan(
      1e-3,
    );
  });

  it("reproduces R05's sky to 10⁻³, where the view's own path takes the curve", () => {
    const view = twinView(equatorCamera(WGS84, 2), sunAt(4), WGS84, 0.1);
    const size = { widthTexels: 24, heightTexels: 16 };
    const sky = skyViewTwin(planet, view, size, 32);
    const r05 = skyViewTwin(earth, view, size, 32);
    expect(worstRelative(sky.texels, r05.texels)).toBeLessThan(1e-3);
  }, 60_000);
});

describe("a Venus-class transmittance stored as optical depth", () => {
  const { medium: venus, radiusM } = venusClassMedium();
  const table = transmittanceTwin(venus, radiusM, { widthTexels: 64, heightTexels: 16 });

  const depths = table.texels.filter((_, i) => i % 4 !== 3);

  it("is finite and within rgba16float's range at every texel", () => {
    expect(depths.every((tau) => Number.isFinite(tau) && tau >= 0 && tau <= HALF_MAX)).toBe(true);
  });

  it("is held to the half float's precision, 2⁻¹¹ of itself, at every texel", () => {
    let worst = 0;
    for (const tau of depths) {
      const stored = fromHalfBits(toHalfBits(tau));
      worst = Math.max(worst, Math.abs(stored - tau) / Math.max(tau, 2 ** -14));
    }
    // Half a step of the half float, past `toHalfBits`' rounding through f32.
    expect(worst).toBeLessThanOrEqual(2 ** -11 * (1 + 1e-6));
  });

  it("has texels whose stored transmittance would have underflowed to 0", () => {
    // A stored transmittance under half the half float's least step rounds to 0.
    expect(depths.filter((tau) => Math.exp(-tau) < 2 ** -25).length).toBeGreaterThan(0);
  });

  it("holds a path deeper than the half float's largest to it, as the kernel stores it", () => {
    // The deck a hundred times as deep: its grazing paths pass 65,504, which stored unclamped
    // would be infinite, and an infinite texel's bilinear read at the table's edge NaN.
    const cloud = venus.terms.find((term) => term.name === "cloud");
    if (cloud === undefined) {
      throw new Error("the Venus-class medium has its cloud deck");
    }
    const [s0, s1, s2] = cloud.scattering;
    const [a0, a1, a2] = cloud.absorption;
    const deeper: AtmosphereMedium = {
      ...venus,
      terms: [
        ...venus.terms.filter((term) => term !== cloud),
        {
          ...cloud,
          scattering: [100 * s0, 100 * s1, 100 * s2],
          absorption: [100 * a0, 100 * a1, 100 * a2],
        },
      ],
    };
    const deep = transmittanceTwin(deeper, radiusM, { widthTexels: 64, heightTexels: 16 });
    const stored = deep.texels.filter((_, i) => i % 4 !== 3);
    expect([Math.max(...stored), stored.every((tau) => Number.isFinite(tau))]).toEqual([
      HALF_MAX,
      true,
    ]);
  });
});

describe("four absorbers, one a molecule pair, under their curves of growth", () => {
  const { medium: four, curves } = fourAbsorberMedium();
  const size = { widthTexels: 16, heightTexels: 8 };
  // At 20,000 midpoint steps, so that the layers' sharp edges converge as the oracle's do.
  const tables = transmittanceTablesTwin(four, R1, curves, size, 20_000);

  it("fill the transmittance table's alpha and the absorber-column table's first three channels", () => {
    const shell = { bottomRadiusM: R1, topRadiusM: R1 + four.topHeightM };
    const slots = absorberSlots(four);
    expect(slots.map((slot) => slot.term.name)).toEqual([
      "absorber:O3",
      "absorber:CH4+O2",
      "pair:O2+O2",
      "absorber:O3 (troposphere)",
    ]);
    let worst = 0;
    for (let y = 0; y < size.heightTexels; y += 1) {
      for (let x = 0; x < size.widthTexels; x += 1) {
        const i = y * size.widthTexels + x;
        const { rM, mu } = transmittanceUvToRMu(
          shell,
          unitFromTexelUv((x + 0.5) / size.widthTexels, size.widthTexels),
          unitFromTexelUv((y + 0.5) / size.heightTexels, size.heightTexels),
        );
        const oracle = absorberAmountsToTop(four, R1, rM, mu);
        const stored = [
          tables.transmittance.texels[i * 4 + 3] ?? Number.NaN,
          tables.absorberColumns.texels[i * 4] ?? Number.NaN,
          tables.absorberColumns.texels[i * 4 + 1] ?? Number.NaN,
          tables.absorberColumns.texels[i * 4 + 2] ?? Number.NaN,
        ];
        for (const [k, slot] of slots.entries()) {
          const amount = oracle[k];
          const expected =
            (amount === undefined ? Number.NaN : amountValue(amount)) /
            amountValue(slot.verticalAmount);
          worst = Math.max(
            worst,
            Math.abs((stored[k] ?? Number.NaN) - expected) / Math.max(expected, 1e-3),
          );
        }
        expect(tables.absorberColumns.texels[i * 4 + 3]).toBe(0);
      }
    }
    // Measured 2.3 × 10⁻⁴: each layer's edge is a ramp over one of the column's levels, about
    // 120 m, which both quadratures resolve to a fraction of their steps.
    expect(worst).toBeLessThan(1e-3);
  });

  it("give every texel the oracle's transmittance, each column through its own curve", () => {
    const shell = { bottomRadiusM: R1, topRadiusM: R1 + four.topHeightM };
    let worst = 0;
    for (let y = 0; y < size.heightTexels; y += 1) {
      for (let x = 0; x < size.widthTexels; x += 1) {
        const { rM, mu } = transmittanceUvToRMu(
          shell,
          unitFromTexelUv((x + 0.5) / size.widthTexels, size.widthTexels),
          unitFromTexelUv((y + 0.5) / size.heightTexels, size.heightTexels),
        );
        const tau = opticalDepthToTop(four, R1, rM, mu);
        const absorbed = curvesOpticalDepth(
          curves[0] ?? [],
          absorberAmountsToTop(four, R1, rM, mu),
        );
        const t = texelTransmittance(tables, four, y * size.widthTexels + x);
        for (const c of CHANNELS) {
          const oracle = Math.exp(-(tau[c] + absorbed[c]));
          worst = Math.max(worst, Math.abs(t[c] - oracle) / Math.max(oracle, 1e-6));
        }
      }
    }
    expect(worst).toBeLessThan(1e-3);
  });

  it("are read on a march's own path by its whole column, never step by step (Design note 5)", () => {
    const planet: PlanetTwin = {
      medium: four,
      bottomRadiusM: R1,
      ...tables,
      multiScattering: { widthTexels: 1, heightTexels: 1, texels: new Float64Array(4) },
    };
    const view = twinView(equatorCamera(WGS84, 2_000), sunAt(10), WGS84, 0.1);
    const ray = skyViewRayTwin(four, view, 0, 15, { widthTexels: 24, heightTexels: 32 });
    const marched = marchTwin(
      four,
      tableSources(planet),
      { kind: "sphere", bottomRadiusM: view.cameraRadiusM },
      ray,
      placedSteps(ray, 4_096),
    );
    // The ray from the camera's sphere: its columns and smooth depth to the end, by the oracle.
    const rM = ray.originM.z;
    const mu = ray.direction.z;
    const tau = opticalDepth(four, view.cameraRadiusM, rM, mu);
    const columns = absorberAmounts(four, view.cameraRadiusM, rM, mu);
    const absorbed = curvesOpticalDepth(curves[0] ?? [], columns);
    for (const c of CHANNELS) {
      expect(relative(marched.transmittance[c], Math.exp(-(tau[c] + absorbed[c])))).toBeLessThan(
        1e-3,
      );
    }
  }, 60_000);

  it("give the sun's transmittance, read at each texel's own (h, μ), as the oracle's", () => {
    // tableSources' read, bilinear in τ and in each slot's column through Bruneton's mapping,
    // returns each texel at its own (h, μ): so every slot's column, the second table's included,
    // must reach its own curve (decision-r08-t6b-floor.md §5's read path).
    const planet: PlanetTwin = {
      medium: four,
      bottomRadiusM: R1,
      ...tables,
      multiScattering: { widthTexels: 1, heightTexels: 1, texels: new Float64Array(4) },
    };
    const sources = tableSources(planet);
    const shell = { bottomRadiusM: R1, topRadiusM: R1 + four.topHeightM };
    const out: Rgb3 = [0, 0, 0];
    let worst = 0;
    for (let y = 0; y < size.heightTexels; y += 1) {
      for (let x = 0; x < size.widthTexels; x += 1) {
        const { rM, mu } = transmittanceUvToRMu(
          shell,
          unitFromTexelUv((x + 0.5) / size.widthTexels, size.widthTexels),
          unitFromTexelUv((y + 0.5) / size.heightTexels, size.heightTexels),
        );
        sources.sunTransmittance(rM - R1, mu, out);
        const tau = opticalDepthToTop(four, R1, rM, mu);
        const absorbed = curvesOpticalDepth(
          curves[0] ?? [],
          absorberAmountsToTop(four, R1, rM, mu),
        );
        for (const c of CHANNELS) {
          const oracle = Math.exp(-(tau[c] + absorbed[c]));
          worst = Math.max(worst, Math.abs(out[c] - oracle) / Math.max(oracle, 1e-6));
        }
      }
    }
    expect(worst).toBeLessThan(1e-3);
  });

  it("are read by the kernels' even-grid rule exactly as absorbers.ts's absorberTransmittance reads them, the pair's too", () => {
    let worst = 0;
    for (const [k, slot] of absorberSlots(four).entries()) {
      const curve = (curves[0] ?? [])[k];
      if (curve === undefined) {
        throw new Error(`no curve for slot ${k}`);
      }
      const vertical = slot.verticalAmount;
      // Below the first node, on and between nodes, and past the last, 10⁻⁵ to 10⁴ of U.
      for (let i = 0; i <= 900; i += 1) {
        const x = 10 ** (-5 + i / 100);
        const rule = absorberTransmittance(curve, scaledAmount(vertical, x));
        const read = curveOpticalDepth(curve, x);
        for (const c of CHANNELS) {
          worst = Math.max(worst, Math.abs(read[c] + Math.log(rule[c])));
        }
      }
    }
    expect(worst).toBeLessThan(1e-12);
  });

  const options = {
    ...MULTI_SCATTERING_KERNEL_OPTIONS,
    size: { widthTexels: 4, heightTexels: 4 },
    samples: 8,
  };
  const other = fourAbsorberCurvesUnder(planckBakeSpectrum(3_300));
  const both = { ...tables, curves: [curves[0] ?? [], other] };

  it("build the multiple-scattering table with the curve slot it is given (decision-r08-t6b-floor.md §8)", () => {
    const second = multiScatteringTwin(four, R1, both, options, undefined, 1);
    const alone = multiScatteringTwin(four, R1, { ...tables, curves: [other] }, options);
    expect(worstRelative(second.texels, alone.texels)).toBe(0);
  }, 60_000);

  it("build another multiple-scattering table at another light slot's curves", () => {
    const first = multiScatteringTwin(four, R1, both, options);
    const second = multiScatteringTwin(four, R1, both, options, undefined, 1);
    expect(worstRelative(first.texels, second.texels)).toBeGreaterThan(1e-6);
  }, 60_000);

  it("refuse a curve slot the curves do not hold", () => {
    expect(() => multiScatteringTwin(four, R1, tables, options, undefined, 1)).toThrow(
      /no curves at light slot 1/u,
    );
  });

  it("are refused where no light gives curves for the medium's absorbers", () => {
    expect(() => transmittanceTablesTwin(four, R1, [], size, 8)).toThrow(/no light's curves/u);
  });

  it("are refused where a light gives fewer curves than the medium's absorbers", () => {
    expect(() => planetTwin(four, R1, [(curves[0] ?? []).slice(0, 2)])).toThrow(/gives 2 curves/u);
  });
});

/** The four-absorber medium's curves under another sun's spectrum, in its slots' order. */
function fourAbsorberCurvesUnder(spectrum: BakeSpectrum): readonly AbsorberCurve[] {
  return absorberSlots(fourAbsorberMedium().medium).map((slot) =>
    absorberCurve(slot.absorber.spec, spectrum),
  );
}
