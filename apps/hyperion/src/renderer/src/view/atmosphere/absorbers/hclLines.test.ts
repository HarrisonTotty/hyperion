/**
 * HCl's table from HITRAN2024 on R08.T4.d's machinery (plan R08, R08.T4.h; decision-r08-t4ef-storage
 * §4): its range, its bands and its overtones part, and the tiers and budget its sidecar records.
 */
import { beforeAll, describe, expect, it } from "vitest";

import { inlinePart } from "../../../test/kDistributionFixtures";
import {
  cellMeansAt,
  cellWeights,
  largestChange,
  RANGE_SUNS,
  rangeCrossingK,
  saturatedColumnPerM2,
} from "../../../test/lineRanges";
import sidecar from "./kDistributions/HCl.json" with { type: "json" };
import sidecarText from "./kDistributions/HCl.json?raw";
import { type KDistributionTable, kDistributionAt, loadKDistributions } from "./kDistribution";
import { PENDING_ABSORBERS } from "./transparency";

/**
 * HCl's saturation pressure, Pa, as its reduction's cap takes it (`src/tools/kDistributionsCli.ts`'s
 * `HCL_SATURATION`): Clausius–Clapeyron from the triple point, 159.07 K (Thol et al. 2018) and
 * 13.800 kPa (Henderson et al. 1986), at ΔH_sub 19.7 kJ mol⁻¹ (Ser and Larher 1990) below it and
 * ΔH_vap 17.227 kJ mol⁻¹ (Thol et al. 2018, at the triple point) above.
 */
function hydrogenChlorideSaturationPa(temperatureK: number): number {
  const enthalpy = temperatureK < 159.07 ? 19_700 : 17_227;
  return 13_800 * Math.exp((-enthalpy / 8.314_462_618) * (1 / temperatureK - 1 / 159.07));
}

/** H³⁵Cl's molecular mass, Da (HITRAN's isotopologue table). */
const HCL_MASS_DA = 35.976_678;

let table: KDistributionTable | undefined;

beforeAll(async () => {
  [table] = await loadKDistributions(["HCl"], inlinePart);
}, 120_000);

/** The loaded table, or a test failure. */
function loaded(): KDistributionTable {
  if (table === undefined) {
    throw new Error("HCl's committed table did not load");
  }
  return table;
}

describe("HCl's table from HITRAN2024 (R08.T4.h)", () => {
  it("starts where a pure vapour's saturated column is negligible, Δ_c under 10⁻³ at k = 100", () => {
    const first = loaded().temperaturesK[0] ?? Number.NaN;
    const column = saturatedColumnPerM2(hydrogenChlorideSaturationPa(first), HCL_MASS_DA);
    for (const [, sun] of RANGE_SUNS) {
      expect(largestChange(cellWeights(sun), cellMeansAt(loaded(), first), column)).toBeLessThan(
        1e-3,
      );
    }
  });

  it("finds T_min, where Δ_c at k = 100 reaches 10⁻³, within its first interval", () => {
    // The nodes run down from 500 K in 50 K steps to the first at or below T_min: 50 K, T_min
    // lying near 93 K (the plan records it).
    const nodes = loaded().temperaturesK;
    const least = Math.min(
      ...RANGE_SUNS.map(([, sun]) =>
        rangeCrossingK(loaded(), sun, hydrogenChlorideSaturationPa, HCL_MASS_DA),
      ),
    );
    expect([nodes[0], least > (nodes[0] ?? 0), least < (nodes[1] ?? 0)]).toEqual([50, true, true]);
  }, 60_000);

  it("runs to HITRAN's 500 K, measured to 296 K and held beyond, estimated", () => {
    expect([loaded().temperaturesK.at(-1), loaded().measuredRangeK]).toEqual([500, [50, 296]]);
    expect(kDistributionAt(loaded(), 600).provenance).toBe("estimated");
  });

  it("holds the 5-0 to 7-0 bands from 525 nm, its 8-0 band in its overtones part", () => {
    const cells = loaded().cells.map((c) => c.cellNm);
    const overtones = PENDING_ABSORBERS.find((p) => p.key === "HCl" && p.part === "overtones");
    expect([cells[0], cells.at(-1), overtones?.task, overtones?.bound.basis]).toEqual([
      525,
      760,
      "R08.T4.f",
      "estimated",
    ]);
  });
});

describe("HCl's table's records (T4.d's tiers)", () => {
  it("passes the node tier, 10⁻³, and the 16-bit test, 2 × 10⁻⁴", () => {
    expect([sidecar.worstNodeError <= 1e-3, sidecar.worstQuantisationError <= 2e-4]).toEqual([
      true,
      true,
    ]);
  });

  it("passes the chain tier, 2 × 10⁻³, between rows and nodes within the cap", () => {
    expect([sidecar.worstChainError <= 2e-3, sidecar.estimatedTemperaturesK]).toEqual([true, []]);
  });

  it("keeps its 10 Pa table measured below the grid by its 1 Pa check", () => {
    expect(sidecar.measuredBelowGrid).toBe(true);
  });

  it("stays within its 2 MB budget, parts and sidecar", () => {
    const bytes =
      sidecar.parts.reduce((sum, part) => sum + part.bytes, 0) +
      new TextEncoder().encode(sidecarText).length;
    expect(bytes).toBeLessThanOrEqual(2_000_000);
  });
});
