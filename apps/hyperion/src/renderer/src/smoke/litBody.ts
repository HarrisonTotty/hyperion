/**
 * The smoke page's lit-body checks (plan R07, T4.c and T6.c): `shaders/litBody.wgsl`, run in the
 * catalogue's probe kernel on the run's adapter. `body_brdf` equals `view/appearance/brdf.ts` at
 * five pinned geometries to 10⁻⁵ relative, one of them with a law built per texel;
 * `sphere_irradiance` and `eclipse_visible` equal their TypeScript twins (themselves checked
 * against the `f64` oracles) at five points each, and the horizon term at six more, the smooth
 * terminator, a terrain shadow's edge and a point star (T6.d); R11's stub and R08's lookups (plan R08's
 * `surfaceLighting.wgsl` since R08.T9.b) return 1, 1 and 0 on a body without an atmosphere, its
 * one-texel tables bound; and `planetshine_irradiance` equals `planetshineIrradiance` at four
 * points (T11). R08.T9.b's own group holds the lookups against a body's tables' twin
 * ({@link probeSurfaceLighting}, `smoke/atmosphere.ts`).
 */

import {
  type BrdfProbeCase,
  expectedBrdf,
  expectedLighting,
  type LightingProbeCase,
  LIT_BODY_PROBE,
  packBrdfProbeCases,
  packLightingProbeCases,
  packPhaseFactorRows,
} from "../view/appearance/litBodyProbe";
import { type Vec3, vec3 } from "../geometry/vec3";
import {
  AIRLESS_LIGHTING,
  createAirlessTables,
  surfaceLightingKernelTextures,
  type SurfaceLightingTables,
} from "../view/atmosphere/surfaceLighting";
import { PHASE_TABLE_SAMPLES, type PhotometricLaw } from "../view/appearance/law";
import { lawFor } from "../view/appearance/phase";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { RenderEngine } from "../view/engine/types";
import type { Checks } from "./harness";

/** The agreement T4.c asks of the twins, relative. */
const BRDF_TOLERANCE = 1e-5;

const RAD_PER_DEG = Math.PI / 180;

/** The probe's laws, one table row each: airless, cloudy, thin-aired and a giant. */
const LAWS: ReadonlyArray<PhotometricLaw> = [
  lawFor([0.17, 0.14, 0.1], [0.5, 0.48, 0.46], "mercury"),
  lawFor([0.71, 0.69, 0.66], [1.34, 1.34, 1.34], "venus"),
  lawFor([0.29, 0.17, 0.09], [1.1, 1.08, 1.05], "mars"),
  lawFor([0.5, 0.54, 0.44], [1.31, 1.31, 1.31], "jupiter"),
];

/** Five geometries: every law, off-sample phases, the clamp's region and a per-texel law. */
function probeCases(): BrdfProbeCase[] {
  const at = (row: number): PhotometricLaw => {
    const law = LAWS[row];
    if (law === undefined) {
      throw new Error(`no probe law in row ${row}`);
    }
    return law;
  };
  return [
    { law: at(0), row: 0, mu0: 0.8, mu: 0.6, phaseRad: 30.2 * RAD_PER_DEG, perTexel: null },
    { law: at(1), row: 1, mu0: 0.3, mu: 0.9, phaseRad: 155.3 * RAD_PER_DEG, perTexel: null },
    { law: at(2), row: 2, mu0: 0.5, mu: 0.5, phaseRad: 0, perTexel: null },
    { law: at(3), row: 3, mu0: 0.9, mu: 0.2, phaseRad: 100.33 * RAD_PER_DEG, perTexel: null },
    {
      law: at(0),
      row: 0,
      mu0: 0.7,
      mu: 0.4,
      phaseRad: 61.7 * RAD_PER_DEG,
      perTexel: { normalAlbedo: 0.23 },
    },
  ];
}

/**
 * The eclipse term's agreement with its twin, absolute on factors of order one: it calls WGSL's
 * `f32` `acos`, allowed the worse of 6.77 × 10⁻⁵ and atan2's 4,096 ULP (WGSL §15.7.4.1's accuracy
 * table), which a correctly rounded implementation such as SwiftShader comes far inside.
 */
const ECLIPSE_TOLERANCE = 2e-4;

/**
 * The horizon term's agreement with its twin, absolute, for a star's and planetshine's (R07.T6.d).
 * Neither calls WGSL's trigonometry. At the star's points here the term, emulated in `f32` at
 * WGSL's bounds, meets the twin within 4 × 10⁻⁷ of face-on (`lighting/horizonPrecision.test.ts`);
 * planetshine's angle and inverse square are not emulated, and rest on this check alone.
 */
const HORIZON_TOLERANCE = 2e-6;

/** The agreement a lighting case's term is held to; 0 for those that must return a value exactly. */
function lightingTolerance(probe: LightingProbeCase): number {
  let tolerance: number;
  switch (probe.kind) {
    case "sphere":
    case "planetshine":
      tolerance = HORIZON_TOLERANCE;
      break;
    case "eclipse":
      tolerance = ECLIPSE_TOLERANCE;
      break;
    case "hooks":
    case "atmosphere":
      tolerance = 0;
      break;
  }
  return tolerance;
}

/** One AU over the Sun's radius: H at 1 au. */
const H_AT_1_AU = 1.495_978_707e11 / 6.957e8;

/** T11: planetshine's factor at a point whose normal lies `normalDeg` from x in the xy plane. */
function planetshineCase(
  toSource: Vec3,
  sourceRadius: number,
  centreDistance: number,
  normalDeg: number,
  horizonDeg: number,
): LightingProbeCase {
  return {
    kind: "planetshine",
    toSource,
    sourceRadius,
    centreDistance,
    normal: vec3(Math.cos(normalDeg * RAD_PER_DEG), Math.sin(normalDeg * RAD_PER_DEG), 0),
    horizonRad: horizonDeg * RAD_PER_DEG,
  };
}

/** The check's name for a lighting case's term. */
function lightingTermName(probe: LightingProbeCase): string {
  let name: string;
  switch (probe.kind) {
    case "sphere":
      name = "T6.c sphere_irradiance";
      break;
    case "eclipse":
      name = "T6.c eclipse_visible";
      break;
    case "hooks":
      name = "T6.c the hooks";
      break;
    case "atmosphere":
      name = `R08.T9.b the ${probe.lookup} lookup`;
      break;
    case "planetshine":
      name = "T11 planetshine_irradiance";
      break;
  }
  return name;
}

/**
 * T6.c: five horizon factors, five eclipse terms and the stubs; T6.d: six more horizon factors;
 * T11: four planetshine factors.
 */
const LIGHTING_CASES: ReadonlyArray<LightingProbeCase> = [
  { kind: "sphere", h: 3, phiRad: 0.4, horizonRad: 0 },
  { kind: "sphere", h: 3, phiRad: 100 * RAD_PER_DEG, horizonRad: 0 },
  { kind: "sphere", h: 11.5, phiRad: 90 * RAD_PER_DEG, horizonRad: 0 },
  { kind: "sphere", h: H_AT_1_AU, phiRad: 89.9 * RAD_PER_DEG, horizonRad: 0 },
  { kind: "sphere", h: 11.5, phiRad: 70 * RAD_PER_DEG, horizonRad: 5 * RAD_PER_DEG },
  // T6.d: the smooth terminator at 1 au, at 90° and half the Sun's radius sunward of it, where
  // Mesa's `atan` took the old form to −4.4 × 10⁻⁴ and 1.28 × 10⁻³ against 9.87 × 10⁻⁴ and
  // 2.51 × 10⁻³; a flat element with the Sun's centre on a 1° horizon; and a point star, H 10³⁰,
  // whose H² overflows f32, wholly up, then up through a 5° horizon, then down behind it.
  { kind: "sphere", h: H_AT_1_AU, phiRad: Math.PI / 2, horizonRad: 0 },
  { kind: "sphere", h: H_AT_1_AU, phiRad: Math.PI / 2 - 0.5 / H_AT_1_AU, horizonRad: 0 },
  { kind: "sphere", h: H_AT_1_AU, phiRad: 89 * RAD_PER_DEG, horizonRad: RAD_PER_DEG },
  { kind: "sphere", h: 1e30, phiRad: 60 * RAD_PER_DEG, horizonRad: 0 },
  { kind: "sphere", h: 1e30, phiRad: 80 * RAD_PER_DEG, horizonRad: 5 * RAD_PER_DEG },
  { kind: "sphere", h: 1e30, phiRad: 88 * RAD_PER_DEG, horizonRad: 5 * RAD_PER_DEG },
  ...[
    [0.3, 0],
    [0.3, 0.8],
    [1, 0.5],
    [2, 1.5],
    [0.5, 1.4],
  ].map(([ratio = 0, separation = 0]): LightingProbeCase => ({
    kind: "eclipse",
    // The Sun from 1 au, B band: Maxted 2018, Table 2 (CDS J/A+A/616/A39).
    starRadiusRad: 4.65e-3,
    limbC: 0.846,
    limbAlpha: 0.83,
    annuli: 4,
    occluderRadiusRad: ratio * 4.65e-3,
    separationRad: separation * 4.65e-3,
  })),
  { kind: "hooks" },
  // T11: a neighbour 30 of its radii off, wholly up, then cut by the horizon, then at Earth from the
  // Moon's scale (in lunar radii) in its soft band, then behind a local horizon of 3°.
  planetshineCase(vec3(30, 0.5, 0), 1, 30, 40, 0),
  planetshineCase(vec3(30, 0.5, 0), 1, 30, 91, 0),
  planetshineCase(vec3(221.25, 0.6, 0), 3.667, 221.25, 90.5, 0),
  planetshineCase(vec3(30, 0.5, 0), 1, 30, 80, 3),
  // R08.T9.b: R08's three lookups on a body without an atmosphere, a source below the horizon
  // among them, read through the one-texel defaults as R07's stubs: 1, 0 and 1.
  ...(["sun", "sky", "source"] as const).flatMap((lookup) =>
    [0.4, -0.2].map((mu): LightingProbeCase => ({
      kind: "atmosphere",
      lookup,
      altitudeM: 3_000,
      mu,
      latitudeRad: 0.7,
      azimuthRad: -2.1,
      angularRadiusRad: 0.1,
      curve: 1,
    })),
  ),
];

/**
 * T6.c: `sphere_irradiance`, `eclipse_visible` and the hooks; T11: `planetshine_irradiance`;
 * R08.T9.b: R08's lookups on a body without an atmosphere, which return R07's stubs' values to the
 * bit, as the hooks do.
 */
function checkLighting(checks: Checks, gpu: Float32Array): void {
  const expected = expectedLighting(LIGHTING_CASES, AIRLESS_LIGHTING);
  LIGHTING_CASES.forEach((probe, index) => {
    const reference = expected[index] ?? [];
    const read = reference.map((_, c) => gpu[4 * index + c] ?? Number.NaN);
    const worst = Math.max(
      ...reference.map((value, c) => Math.abs((read[c] ?? Number.NaN) - value)),
    );
    const tolerance = lightingTolerance(probe);
    let name = `${lightingTermName(probe)} equals its reference to ${tolerance} at point ${index + 1}`;
    if (probe.kind === "hooks") {
      name = "T6.c the hooks on a body without an atmosphere return 1, 1 and 0";
    } else if (probe.kind === "atmosphere") {
      name = `R08.T9.b the ${probe.lookup} lookup on a body without an atmosphere returns R07's stub at point ${index + 1}`;
    }
    checks.check(
      name,
      worst <= tolerance,
      `GPU ${read.map((value) => value.toPrecision(7)).join(", ")} against ${reference.map((value) => value.toPrecision(7)).join(", ")}, off by ${worst.toExponential(2)}`,
    );
  });
}

/**
 * R08.T9.b: the probe's lighting `cases` run with a body's atmosphere `tables` bound, their results
 * read back, one `vec4f` a case (r, g, b of an R08 lookup), for `smoke/atmosphere.ts` to hold
 * against the tables' twin. It makes and releases its own buffers and phase table.
 */
export async function probeSurfaceLighting(
  engine: RenderEngine,
  cases: ReadonlyArray<LightingProbeCase>,
  tables: SurfaceLightingTables,
): Promise<Float32Array> {
  const kernel = await engine.createComputeAsync(LIT_BODY_PROBE);
  // One BRDF case and one law: the kernel's other half, which this run does not read.
  const brdfBytes = packBrdfProbeCases(probeCases().slice(0, 1));
  const brdf = engine.createBuffer({
    name: "surface lighting probe brdf case",
    bytes: brdfBytes.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(brdf, 0, new Uint8Array(brdfBytes));
  const brdfResults = engine.createBuffer({
    name: "surface lighting probe brdf result",
    bytes: 16,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  const table = engine.createTexture({
    name: "surface lighting probe phase table",
    size: { width: PHASE_TABLE_SAMPLES, height: LAWS.length },
    dimension: "2d",
    format: "rgba32float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeTexture(
    table,
    { x: 0, y: 0 },
    { width: PHASE_TABLE_SAMPLES, height: LAWS.length },
    packPhaseFactorRows(LAWS),
  );
  const bytes = packLightingProbeCases(cases);
  const input = engine.createBuffer({
    name: "surface lighting probe cases",
    bytes: bytes.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(input, 0, new Uint8Array(bytes));
  const results = engine.createBuffer({
    name: "surface lighting probe results",
    bytes: cases.length * 16,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  try {
    engine.dispatch(
      kernel,
      {
        uniforms: {},
        buffers: {
          cases: brdf,
          results: brdfResults,
          lighting_cases: input,
          lighting_results: results,
        },
        sampled: { phase_factor_table: table, ...surfaceLightingKernelTextures(tables) },
        storage: {},
      },
      [Math.ceil(cases.length / 8), 1, 1],
      LIT_BODY_PROBE.name,
    );
    return new Float32Array(await engine.readBuffer(results));
  } finally {
    for (const buffer of [brdf, brdfResults, input, results]) {
      engine.releaseBuffer(buffer);
    }
    engine.releaseTexture(table);
  }
}

/**
 * T4.c: `body_brdf` against `brdf`; T6.c: the lighting terms and hooks; R08.T9.b: R08's lookups on a
 * body without an atmosphere.
 */
export async function checkLitBody(engine: RenderEngine, checks: Checks): Promise<void> {
  const cases = probeCases();
  const kernel = await engine.createComputeAsync(LIT_BODY_PROBE);
  const input = engine.createBuffer({
    name: "lit body probe cases",
    bytes: packBrdfProbeCases(cases).byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(input, 0, new Uint8Array(packBrdfProbeCases(cases)));
  const results = engine.createBuffer({
    name: "lit body probe results",
    bytes: cases.length * 16,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  const table = engine.createTexture({
    name: "lit body probe phase table",
    size: { width: PHASE_TABLE_SAMPLES, height: LAWS.length },
    dimension: "2d",
    format: "rgba32float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeTexture(
    table,
    { x: 0, y: 0 },
    { width: PHASE_TABLE_SAMPLES, height: LAWS.length },
    packPhaseFactorRows(LAWS),
  );
  const lightingBytes = packLightingProbeCases(LIGHTING_CASES);
  const lightingInput = engine.createBuffer({
    name: "lit body probe lighting cases",
    bytes: lightingBytes.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(lightingInput, 0, new Uint8Array(lightingBytes));
  const lightingResults = engine.createBuffer({
    name: "lit body probe lighting results",
    bytes: LIGHTING_CASES.length * 16,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  // R08's tables of a body without an atmosphere (R08.T9.b), which the hooks read through.
  const airless = createAirlessTables(engine, "lit body probe: no atmosphere");
  engine.dispatch(
    kernel,
    {
      uniforms: {},
      buffers: {
        cases: input,
        results,
        lighting_cases: lightingInput,
        lighting_results: lightingResults,
      },
      sampled: { phase_factor_table: table, ...surfaceLightingKernelTextures(airless) },
      storage: {},
    },
    [Math.ceil(Math.max(cases.length, LIGHTING_CASES.length) / 8), 1, 1],
    LIT_BODY_PROBE.name,
  );
  try {
    checkLighting(checks, new Float32Array(await engine.readBuffer(lightingResults)));
  } finally {
    engine.releaseTexture(airless.body);
  }
  const gpu = new Float32Array(await engine.readBuffer(results));
  const expected = expectedBrdf(cases);
  expected.forEach((reference, index) => {
    const read = [
      gpu[4 * index] ?? Number.NaN,
      gpu[4 * index + 1] ?? Number.NaN,
      gpu[4 * index + 2] ?? Number.NaN,
    ];
    const worst = Math.max(
      ...reference.map((value, c) => Math.abs((read[c] ?? Number.NaN) - value) / Math.abs(value)),
    );
    const probe = cases[index];
    checks.check(
      `T4.c body_brdf equals brdf to 10⁻⁵ at geometry ${index + 1}${probe?.perTexel === null ? "" : " (a law per texel)"}`,
      worst <= BRDF_TOLERANCE,
      `GPU ${read.map((value) => value.toPrecision(7)).join(", ")} against ${reference.map((value) => value.toPrecision(7)).join(", ")}, worst ${worst.toExponential(2)}`,
    );
  });
}
