/**
 * The envelope cases' asymmetry rule (plan R08, R08.T16.a.2): `jupiterEnvelopeCase.ts`'s
 * `asReadWhereApart` keeps a mode's g where it lies within the tracer's 10⁻⁴ of its table's mean
 * cosine as read, and takes the table's elsewhere, channel by channel.
 */

import { describe, expect, it } from "vitest";

import type { PhaseTable } from "../view/atmosphere/medium";
import { asReadWhereApart, PHASE_ASYMMETRY_TOLERANCE } from "./jupiterEnvelopeCase";
import { linearInUMoments } from "./mieMatrices";

/** An isotropic table, 1 ÷ 4π sr⁻¹ in each channel on 65 entries of u. */
function isotropic(): PhaseTable {
  const u = Float64Array.from({ length: 65 }, (_, i) => i / 64);
  const flat = (): Float64Array => u.map(() => 1 / (4 * Math.PI));
  return { u, values: [flat(), flat(), flat()], matrix: undefined, forwardPeak: [0, 0, 0] };
}

describe("the envelope cases' asymmetry", () => {
  it("keeps a channel's g within the tracer's tolerance of its table as read, and takes the table's past it, each channel alone", () => {
    const table = isotropic();
    const read = (c: 0 | 1 | 2): number => linearInUMoments(table.u, table.values[c]).meanCosine;
    const own = [read(0) + 0.5e-4, read(1) + 2.5e-4, read(2) - 0.5] as const;
    expect(asReadWhereApart(own, table)).toEqual([own[0], read(1), read(2)]);
    expect(PHASE_ASYMMETRY_TOLERANCE).toBe(1e-4);
  });
});
