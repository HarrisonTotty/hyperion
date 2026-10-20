import { describe, expect, it } from "vitest";

import committed from "../../../../../../../crates/hyperion-fit/src/tasks/condensate_extinction/mie_cases.json" with { type: "json" };
import { expectCommittedTable } from "../../test/bless";
import { mieSphere } from "./mie";

/** The committed file, relative to this test file. */
const FILE =
  "../../../../../../../crates/hyperion-fit/src/tasks/condensate_extinction/mie_cases.json";

/** One case the client's Mie and the fit's share: a size parameter and an index n + ik. */
interface SharedCase {
  readonly name: string;
  readonly sizeParameter: number;
  readonly n: number;
  readonly k: number;
}

/**
 * The five cases (plan 14, P14.T49.e), one for each regime the fit's size averages cross: a
 * Rayleigh sphere; the first resonances of a weakly absorbing one, the 75 wt% sulphuric acid's
 * index at 550 nm (`sulphuric-acid-75.json`); a moderately absorbing dust (`mars-dust.json`'s
 * index); a strongly absorbing soot (`soot.json`'s); and a large lossless sphere of high index
 * (`titanium-dioxide.json`'s), near the geometric limit.
 */
const CASES: ReadonlyArray<SharedCase> = [
  { name: "rayleigh", sizeParameter: 0.05, n: 1.44, k: 0 },
  { name: "acid", sizeParameter: 4.5, n: 1.43105, k: 2.07e-8 },
  { name: "dust", sizeParameter: 8, n: 1.49952, k: 0.00303751 },
  { name: "soot", sizeParameter: 3, n: 1.57, k: 0.53 },
  { name: "large", sizeParameter: 5000, n: 2.43584, k: 5.38564e-8 },
];

/** The scattering angles at which the amplitudes are shared, degrees. */
const ANGLES_DEG: ReadonlyArray<number> = [0, 90, 180];

/** An amplitude's (re, im) at each of {@link ANGLES_DEG}, from Mie's interleaved values. */
function pairs(values: Float64Array): ReadonlyArray<readonly [number, number]> {
  return ANGLES_DEG.map(
    (_, j) => [values[2 * j] ?? Number.NaN, values[2 * j + 1] ?? Number.NaN] as const,
  );
}

/** The table the fit's Mie is held to: each case's efficiencies, asymmetry and amplitudes. */
function sharedCasesTable(): unknown {
  const mu = Float64Array.from(ANGLES_DEG, (deg) => Math.cos((deg * Math.PI) / 180));
  return {
    description:
      "Written by apps/hyperion/src/renderer/src/view/atmosphere/mieSharedCases.test.ts (plan 14, P14.T49.e) from the client's Mie; read by crates/hyperion-fit/src/tasks/condensate_extinction/mie.rs, which holds its own Mie to it to 1e-6. Do not edit by hand.",
    anglesDeg: ANGLES_DEG,
    cases: CASES.map(({ name, sizeParameter, n, k }) => {
      const result = mieSphere(sizeParameter, { n, k }, mu);
      return {
        name,
        sizeParameter,
        n,
        k,
        qExt: result.qExt,
        qSca: result.qSca,
        asymmetry: result.asymmetry,
        s1: pairs(result.s1),
        s2: pairs(result.s2),
      };
    }),
  };
}

describe("the Mie cases the fit's Mie shares", () => {
  it("are, whole, what the client's mieSphere gives now", async () => {
    await expect(
      expectCommittedTable(sharedCasesTable(), committed, FILE),
    ).resolves.toBeUndefined();
  });
});
