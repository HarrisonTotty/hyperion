import { beforeAll, describe, expect, it } from "vitest";

import { dot, normalise, type Vec3 } from "../../geometry/vec3";
import {
  BOTTOM_M,
  CONVERGED,
  describeFailures,
  errorOf,
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
import { quaternion } from "../camera/quaternion";
import { hydrostaticColumn } from "./column";
import { EARTH_REFERENCE } from "./earth";
import {
  type AtmosphereCamera,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
} from "./hillaire";
import { marchSplit, marchStep } from "./marchSteps";
import {
  type AtmosphereMedium,
  type MediumTerm,
  type PhaseFunction,
  phaseTable,
  tabulatedDensity,
} from "./medium";
import { maxDistanceM, opticalDepth, ORACLE_STEPS, transmittanceUvToRMu } from "./opticalDepth";
import commonWgsl from "./shaders/common.wgsl?raw";
import multiScatteringWgsl from "./shaders/multiScattering.wgsl?raw";
import { TRANSMITTANCE_SIZE } from "./tables";
import {
  aerialPerspectiveTwin,
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
};

describe("R05's constants as a medium", () => {
  it("give R05's oracle optical depth at R05.T12.b's 20 texels to 10⁻⁶, at the oracle's steps", () => {
    const shell = { bottomRadiusM: R1, topRadiusM: R1 + EARTH_REFERENCE.topHeightM };
    let worst = 0;
    for (const [x, y] of ORACLE_TEXELS) {
      const { rM, mu } = transmittanceUvToRMu(
        shell,
        (x + 0.5) / TRANSMITTANCE_SIZE.widthTexels,
        (y + 0.5) / TRANSMITTANCE_SIZE.heightTexels,
      );
      const twin = kernelOpticalDepth(EARTH_REFERENCE, R1, rM, mu, ORACLE_STEPS);
      const oracle = opticalDepth(EARTH_REFERENCE, R1, rM, mu);
      for (const c of CHANNELS) {
        worst = Math.max(worst, Math.abs(twin[c] / oracle[c] - 1));
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

  it("hold R05.T12.b's 20 texels within its 1% of the oracle at the kernel's 256 steps", () => {
    const table = earth.transmittance;
    const shell = { bottomRadiusM: R1, topRadiusM: R1 + EARTH_REFERENCE.topHeightM };
    let worst = 0;
    for (const [x, y] of ORACLE_TEXELS) {
      const { rM, mu } = transmittanceUvToRMu(
        shell,
        (x + 0.5) / table.widthTexels,
        (y + 0.5) / table.heightTexels,
      );
      const oracle = opticalDepth(EARTH_REFERENCE, R1, rM, mu);
      for (const c of CHANNELS) {
        const twin = table.texels[(y * table.widthTexels + x) * 4 + c] ?? Number.NaN;
        worst = Math.max(worst, Math.abs(twin / Math.exp(-oracle[c]) - 1));
      }
    }
    // Recorded in plan R08's Risks ("Deviations in T6.a, as built"): the midpoint rule's own error.
    expect(worst).toBeLessThan(0.01);
  });
});

// --- R05.T12.e's gate, with the multiple-scattering term -----------------------------------------

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

  it("has a reference of 4,096 placed steps within 0.05% of 8,192, or of 8,192 within 16,384", () => {
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
      earth.transmittance,
      { ...MULTI_SCATTERING_KERNEL_OPTIONS, ...options },
      MEASURED_TEXELS,
    );

  beforeAll(() => {
    tables.set("even", build({}));
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
      multiScatteringTwin(EARTH_REFERENCE, R1, earth.transmittance, {
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
  phase: { kind: "rayleigh" },
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

/** Every twin's output for a medium, at small sizes, from a camera at 3 km and from 300 km. */
function everyTwin(m: AtmosphereMedium): readonly TwinTable[] {
  const transmittance = transmittanceTwin(m, R1, { widthTexels: 32, heightTexels: 8 }, 64);
  const multiScattering = multiScatteringTwin(m, R1, transmittance, {
    ...MULTI_SCATTERING_KERNEL_OPTIONS,
    size: { widthTexels: 8, heightTexels: 8 },
    samples: 16,
  });
  const planet: PlanetTwin = { medium: m, bottomRadiusM: R1, transmittance, multiScattering };
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
  it("gives two identical half-density terms the tables of one", () => {
    const whole = everyTwin(medium([GAS, HAZE, ABSORBER]));
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
    const whole = everyTwin(medium([GAS, HAZE, ABSORBER]));
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
      const transmittance = transmittanceTwin(m, R1, { widthTexels: 64, heightTexels: 16 }, 64);
      const multiScattering = multiScatteringTwin(m, R1, transmittance);
      return skyViewTwin(
        { medium: m, bottomRadiusM: R1, transmittance, multiScattering },
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
    expect(tables.texels.every((v) => Number.isFinite(v) && v > 0 && v <= 1)).toBe(true);
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

  it("keeps every transmittance texel finite and within [0, 1]", () => {
    expect(earth.transmittance.texels.every((v) => Number.isFinite(v) && v >= 0 && v <= 1)).toBe(
      true,
    );
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
