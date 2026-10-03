/**
 * The smoke page's lit-body checks (plan R07, T4.c): `shaders/litBody.wgsl`'s `body_brdf`, run in
 * the catalogue's probe kernel on the run's adapter, equals `view/appearance/brdf.ts` at five pinned
 * geometries to 10⁻⁵ relative, one of them with a law built per texel.
 */

import {
  type BrdfProbeCase,
  expectedBrdf,
  LIT_BODY_PROBE,
  packBrdfProbeCases,
  packPhaseFactorRows,
} from "../view/appearance/litBodyProbe";
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

/** T4.c: `body_brdf` against `brdf`. */
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
  engine.dispatch(
    kernel,
    {
      uniforms: {},
      buffers: { cases: input, results },
      sampled: { phase_factor_table: table },
      storage: {},
    },
    [1, 1, 1],
    LIT_BODY_PROBE.name,
  );
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
