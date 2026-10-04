/**
 * The smoke page's lit-body checks (plan R07, T4.c and T6.c): `shaders/litBody.wgsl`, run in the
 * catalogue's probe kernel on the run's adapter. `body_brdf` equals `view/appearance/brdf.ts` at
 * five pinned geometries to 10⁻⁵ relative, one of them with a law built per texel;
 * `sphere_irradiance` and `eclipse_visible` equal their TypeScript twins (themselves checked
 * against the `f64` oracles) at five points each; the R08 and R11 stubs return 1, 1 and 0; and
 * `planetshine_irradiance` equals `planetshineIrradiance` at four points (T11).
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
 * The lighting terms' agreement with their twins, absolute on factors of order one: WGSL's `f32`
 * `acos` and `atan` are allowed some 10⁻⁴ of error (WGSL §17.5.3's accuracy table), and both terms
 * call them.
 */
const LIGHTING_TOLERANCE = 2e-4;

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
    case "stubs":
      name = "T6.c the stubs";
      break;
    case "planetshine":
      name = "T11 planetshine_irradiance";
      break;
  }
  return name;
}

/** T6.c: five horizon factors, five eclipse terms and the stubs; T11: four planetshine factors. */
const LIGHTING_CASES: ReadonlyArray<LightingProbeCase> = [
  { kind: "sphere", h: 3, phiRad: 0.4, horizonRad: 0 },
  { kind: "sphere", h: 3, phiRad: 100 * RAD_PER_DEG, horizonRad: 0 },
  { kind: "sphere", h: 11.5, phiRad: 90 * RAD_PER_DEG, horizonRad: 0 },
  { kind: "sphere", h: H_AT_1_AU, phiRad: 89.9 * RAD_PER_DEG, horizonRad: 0 },
  { kind: "sphere", h: 11.5, phiRad: 70 * RAD_PER_DEG, horizonRad: 5 * RAD_PER_DEG },
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
  { kind: "stubs" },
  // T11: a neighbour 30 of its radii off, wholly up, then cut by the horizon, then at Earth from the
  // Moon's scale (in lunar radii) in its soft band, then behind a local horizon of 3°.
  planetshineCase(vec3(30, 0.5, 0), 1, 30, 40, 0),
  planetshineCase(vec3(30, 0.5, 0), 1, 30, 91, 0),
  planetshineCase(vec3(221.25, 0.6, 0), 3.667, 221.25, 90.5, 0),
  planetshineCase(vec3(30, 0.5, 0), 1, 30, 80, 3),
];

/** T6.c: `sphere_irradiance`, `eclipse_visible` and the stubs; T11: `planetshine_irradiance`. */
function checkLighting(checks: Checks, gpu: Float32Array): void {
  const expected = expectedLighting(LIGHTING_CASES);
  LIGHTING_CASES.forEach((probe, index) => {
    const reference = expected[index] ?? [];
    const read = reference.map((_, c) => gpu[4 * index + c] ?? Number.NaN);
    const worst = Math.max(
      ...reference.map((value, c) => Math.abs((read[c] ?? Number.NaN) - value)),
    );
    const exact = probe.kind === "stubs";
    checks.check(
      exact
        ? "T6.c the stubs return 1, 1 and 0"
        : `${lightingTermName(probe)} equals its reference to ${LIGHTING_TOLERANCE} at point ${index + 1}`,
      exact ? worst === 0 : worst <= LIGHTING_TOLERANCE,
      `GPU ${read.map((value) => value.toPrecision(7)).join(", ")} against ${reference.map((value) => value.toPrecision(7)).join(", ")}, off by ${worst.toExponential(2)}`,
    );
  });
}

/** T4.c: `body_brdf` against `brdf`; T6.c: the lighting terms and stubs. */
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
      sampled: { phase_factor_table: table },
      storage: {},
    },
    [Math.ceil(Math.max(cases.length, LIGHTING_CASES.length) / 8), 1, 1],
    LIT_BODY_PROBE.name,
  );
  checkLighting(checks, new Float32Array(await engine.readBuffer(lightingResults)));
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
