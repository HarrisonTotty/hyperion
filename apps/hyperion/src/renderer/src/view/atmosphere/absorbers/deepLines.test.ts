import { beforeAll, describe, expect, it } from "vitest";

import { inlinePart } from "../../../test/kDistributionFixtures";
import { planckBakeSpectrum, SUN_TEFF_K, venusClassColumn } from "../../../test/atmosphereFixtures";
import {
  ABSORBER_TRANSMITTANCE_FLOOR,
  type AbsorberCurve,
  absorberCurve,
  absorberSpecs,
  absorberTransmittance,
  type ColumnAbsorberSpec,
} from "../absorbers";
import { columnLengthM } from "../medium";
import { loadKDistributions } from "./kDistribution";

/** A Venus-class body's whole CO₂ column, m⁻²: its surface number density over its column length. */
function venusCo2PerM2(): number {
  const column = venusClassColumn();
  return column.surfaceNumberDensityPerM3 * columnLengthM(column.density, column.topHeightM);
}

/** The read's τ along u between a curve's rows, as `earthLines.test.ts`'s `readFalls` takes it. */
interface ReadFalls {
  /** The largest fall of τ from one column to the next, at fixed p̄. */
  readonly largestFall: number;
  /** Reads whose τ lies outside [0, −ln 10⁻⁶]. */
  readonly outside: number;
  /** The highest row pressure the read covered, Pa. */
  readonly deepestPa: number;
}

/**
 * A curve read at its rows and midway between them in ln p̄, at each column node and midway
 * between nodes in ln u, by `absorberTransmittance`, the kernels' and the twin's rule.
 */
function readFalls(curve: AbsorberCurve): ReadFalls {
  const ceiling = -Math.log(ABSORBER_TRANSMITTANCE_FLOOR);
  let largestFall = 0;
  let outside = 0;
  const lnP = curve.logPressures;
  const lnU = curve.logColumns;
  for (let r = 0; r + 1 < lnP.length; r += 1) {
    for (const f of [0, 0.5]) {
      const p = Math.exp((lnP[r] ?? Number.NaN) * (1 - f) + (lnP[r + 1] ?? Number.NaN) * f);
      let previous: readonly number[] | undefined;
      for (let j = 0; j + 1 < 2 * lnU.length; j += 1) {
        const k = Math.floor(j / 2);
        const x =
          j % 2 === 0
            ? (lnU[k] ?? Number.NaN)
            : 0.5 * ((lnU[k] ?? Number.NaN) + (lnU[k + 1] ?? Number.NaN));
        const t = absorberTransmittance(curve, { kind: "column", columnPerM2: Math.exp(x) }, p);
        const tau = t.map((v) => -Math.log(v));
        for (const [c, v] of tau.entries()) {
          if (!(v >= 0 && v <= ceiling + 1e-12)) {
            outside += 1;
          }
          largestFall = Math.max(largestFall, (previous?.[c] ?? v) - v);
        }
        previous = tau;
      }
    }
  }
  return { largestFall, outside, deepestPa: Math.exp(lnP.at(-1) ?? Number.NaN) };
}

describe("a Venus-class body's CO₂ curve read between its deep rows (R08.T4.d)", () => {
  let falls: ReadFalls = { largestFall: Number.NaN, outside: Number.NaN, deepestPa: Number.NaN };
  beforeAll(async () => {
    const column = venusClassColumn();
    const tables = await loadKDistributions(["CO2"], inlinePart);
    const { specs } = absorberSpecs(
      [{ species: "CO2", columnPerM2: venusCo2PerM2(), basePa: 92e5, topPa: 0 }],
      column,
      [],
      [],
      { kDistributions: tables },
    );
    const spec = specs.find((s): s is ColumnAbsorberSpec => s.kind === "column");
    if (spec === undefined) {
      throw new Error("no CO₂ profile was formed");
    }
    falls = readFalls(absorberCurve(spec, planckBakeSpectrum(SUN_TEFF_K)));
  }, 120_000);

  it("takes rows to its 92 bar base and past it, where T at fixed u is not monotone in p", () => {
    expect(falls.deepestPa).toBeGreaterThanOrEqual(92e5);
  });

  it("reads τ in [0, −ln 10⁻⁶] at its rows and between them", () => {
    expect(falls.outside).toBe(0);
  });

  it("records the read's largest fall of τ along u between rows, under the chain's tier", () => {
    // Recorded in R08's Risks, "Deviations in T4.d, as built": a fall δ raises T by at most
    // T δ ≤ δ, within the chain's 2 × 10⁻³ while δ is.
    expect(falls.largestFall).toBeLessThanOrEqual(2e-3);
  });
});
