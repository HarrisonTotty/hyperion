/**
 * The smoke page's atmosphere checks (plan R05, R05.T12.b): Earth's per-planet tables built on the
 * GPU and read back with the harness's tolerance access, since their kernels are
 * `presentation-only`.
 *
 * @remarks
 * The transmittance table agrees with the `f64` oracle (`view/atmosphere/opticalDepth.ts`) to 1%
 * at 20 fixed texels, and every texel of both tables is finite, transmittance within [0, 1] and
 * multiple scattering non-negative.
 */

import { EARTH_REFERENCE } from "../view/atmosphere/earth";
import { opticalDepth, transmittanceUvToRMu } from "../view/atmosphere/opticalDepth";
import {
  AtmosphereTables,
  MULTI_SCATTERING_SIZE,
  TRANSMITTANCE_SIZE,
} from "../view/atmosphere/tables";
import { PresentationOnlyReadback, type RenderEngine } from "../view/engine/types";
import { type Checks, halfTexels, show, texel } from "./harness";

/** The ground's radius the check builds Earth's tables on, m: WGS 84's mean radius R₁. */
const EARTH_MEAN_RADIUS_M = 6_371_008.8;

/**
 * Below rgba16float's least normal, 2⁻¹⁴, a stored transmittance has lost its relative precision,
 * so a texel whose oracle lies below it is held to that absolute floor instead of to 1%.
 */
const HALF_NORMAL_MIN = 2 ** -14;

/** The 20 texels compared with the oracle: five across μ by four across r. */
const ORACLE_TEXELS: ReadonlyArray<readonly [number, number]> = [0, 64, 128, 192, 255].flatMap(
  (x) => [0, 16, 40, 63].map((y) => [x, y] as const),
);

/** Builds Earth's tables and checks them against the oracle and their ranges. */
export async function checkAtmosphereTables(engine: RenderEngine, checks: Checks): Promise<void> {
  const tables = new AtmosphereTables(engine, EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);

  let refused = false;
  try {
    await engine.readTexture(tables.transmittance);
  } catch (error: unknown) {
    refused = error instanceof PresentationOnlyReadback;
  }
  checks.check(
    "R05.T12.b the transmittance table is refused to an ordinary readback",
    refused,
    `refused ${String(refused)}`,
  );

  const transmittance = halfTexels(
    await engine.readTexture(tables.transmittance, 0, undefined, "tolerance"),
  );
  const shell = {
    bottomRadiusM: EARTH_MEAN_RADIUS_M,
    topRadiusM: EARTH_MEAN_RADIUS_M + EARTH_REFERENCE.topHeightM,
  };
  const width = TRANSMITTANCE_SIZE.widthTexels;
  for (const [x, y] of ORACLE_TEXELS) {
    const { rM, mu } = transmittanceUvToRMu(
      shell,
      (x + 0.5) / width,
      (y + 0.5) / TRANSMITTANCE_SIZE.heightTexels,
    );
    const expected = opticalDepth(EARTH_REFERENCE, EARTH_MEAN_RADIUS_M, rM, mu).map((tau) =>
      Math.exp(-tau),
    );
    const actual = texel(transmittance, width, x, y).slice(0, 3);
    const agrees = expected.every((e, c) => {
      const a = actual[c] ?? Number.NaN;
      return Math.abs(a - e) <= Math.max(0.01 * e, HALF_NORMAL_MIN);
    });
    checks.check(
      `R05.T12.b transmittance texel (${x}, ${y}) agrees with the f64 oracle to 1%`,
      agrees,
      `h ${((rM - EARTH_MEAN_RADIUS_M) / 1_000).toFixed(2)} km, mu ${mu.toFixed(4)}: GPU ${show(actual)}, oracle ${show(expected)}`,
    );
  }
  const inRange = transmittance.every((v) => Number.isFinite(v) && v >= 0 && v <= 1);
  checks.check(
    "R05.T12.b every transmittance texel is finite and within [0, 1]",
    inRange,
    `${transmittance.length / 4} texels, first ${show(transmittance.subarray(0, 4))}`,
  );

  const multiScattering = halfTexels(
    await engine.readTexture(tables.multiScattering, 0, undefined, "tolerance"),
  );
  checks.check(
    "R05.T12.b every multiple-scattering texel is finite and non-negative",
    multiScattering.every((v) => Number.isFinite(v) && v >= 0),
    `${multiScattering.length / 4} texels`,
  );
  // The first row's last texel: the sun overhead, at the ground, where the sky is lit.
  const overhead = texel(
    multiScattering,
    MULTI_SCATTERING_SIZE.widthTexels,
    MULTI_SCATTERING_SIZE.widthTexels - 1,
    0,
  );
  checks.check(
    "R05.T12.b multiple scattering at the ground under an overhead sun is positive",
    overhead.slice(0, 3).every((v) => v > 0),
    `texel ${show(overhead)}`,
  );
  checks.check(
    "R05.T12.b the tables are not rebuilt for the same medium",
    !tables.setMedium(EARTH_REFERENCE, EARTH_MEAN_RADIUS_M) && tables.builds === 1,
    `builds ${tables.builds}`,
  );
}
