/**
 * A tabulated phase on the kernels' grid (plan R08, Design note 6): the grid, a table resampled onto
 * it, and a term's table with its forward delta taken out, as `tables.ts` packs them for the kernels
 * and the optics workers make the forward lobes of them (lane perf-page-long-tasks). Moved here
 * whole from `tables.ts`, which re-exports them, so that a worker takes them without the kernels'
 * sources.
 */

import { sin } from "../../lib/pinnedMath";
import { forwardPeakEntry } from "./forwardPeak";
import { type MediumTerm, type PhaseMatrixElement, type PhaseTable, phaseTable } from "./medium";

/**
 * The entries of a tabulated phase's table: 256, even in u = √(θ ÷ π) from 0 to 1 (Design note 6's
 * grid). One layer is 4 KB in `rgba32float`, the three channels in rgb.
 */
export const PHASE_TABLE_ENTRIES = 256;

/**
 * Simpson panels a phase table's interval is integrated over, per 1 ÷ 255 of u it spans, at least
 * one such share: 8 an interval of the kernels' grid, and more for a coarser source grid's.
 *
 * @remarks
 * Composite Simpson's error is (b − a) h⁴ max|f⁗| ÷ 180; with h ≤ 1 ÷ 2,040 it is under 2 × 10⁻⁹
 * for a Henyey–Greenstein table up to g = 0.99, where a fixed 8 panels an interval left 1.6 × 10⁻⁴
 * on a source grid of 5° in θ (R08.T6.b's science check).
 */
const PHASE_SIMPSON_PANELS_PER_ENTRY = 8;

/** The integrand of {@link phaseIntegral} at u = x, where the phase is p. */
function phaseIntegrand(x: number, p: number): number {
  return p * x * sin(Math.PI * x * x);
}

/**
 * A phase channel's integral over the sphere, linear in u between its entries:
 * ∫ p dΩ = 4π² ∫₀¹ p(u) u sin(πu²) du, since θ = πu² and dθ = 2πu du, by Simpson's rule.
 */
function phaseIntegral(u: Float64Array, values: Float64Array): number {
  let sum = 0;
  for (let i = 0; i + 1 < u.length; i += 1) {
    const u0 = u[i] ?? Number.NaN;
    const u1 = u[i + 1] ?? Number.NaN;
    const p0 = values[i] ?? Number.NaN;
    const p1 = values[i + 1] ?? Number.NaN;
    const panels =
      PHASE_SIMPSON_PANELS_PER_ENTRY *
      Math.max(1, Math.ceil((u1 - u0) * (PHASE_TABLE_ENTRIES - 1)));
    const h = (u1 - u0) / panels;
    let interval = 0;
    for (let k = 0; k <= panels; k += 1) {
      const f = k / panels;
      let weight = k % 2 === 1 ? 4 : 2;
      if (k === 0 || k === panels) {
        weight = 1;
      }
      interval += weight * phaseIntegrand(u0 + f * (u1 - u0), p0 + f * (p1 - p0));
    }
    sum += (interval * h) / 3;
  }
  return 4 * Math.PI * Math.PI * sum;
}

/** A channel linear in u between `u`'s entries, at each of `at`'s ascending points. */
function resampledChannel(u: Float64Array, values: Float64Array, at: Float64Array): Float64Array {
  const out = new Float64Array(at.length);
  let i = 0;
  for (const [k, x] of at.entries()) {
    while (i + 2 < u.length && (u[i + 1] ?? Number.NaN) <= x) {
      i += 1;
    }
    const u0 = u[i] ?? Number.NaN;
    const f = Math.min(Math.max((x - u0) / ((u[i + 1] ?? Number.NaN) - u0), 0), 1);
    const v0 = values[i] ?? Number.NaN;
    out[k] = v0 + ((values[i + 1] ?? Number.NaN) - v0) * f;
  }
  return out;
}

/** Whether a table's entries are already the kernels' grid: {@link PHASE_TABLE_ENTRIES}, even in u. */
export function onTableGrid(u: Float64Array): boolean {
  const last = PHASE_TABLE_ENTRIES - 1;
  return u.length === PHASE_TABLE_ENTRIES && u.every((x, i) => x === i / last);
}

/**
 * A phase table resampled onto the kernels' grid, {@link PHASE_TABLE_ENTRIES} entries even in u:
 * the table itself when it is on that grid already, otherwise each channel linear in u between its
 * entries, as `medium.ts`'s `phaseAt` reads it, and scaled so that its integral over the sphere is
 * the source's, so that a normalised table stays normalised (R08.T6.a's hand-off).
 *
 * @remarks
 * Each matrix element is resampled alike and scaled by its channel's factor, so that it stays
 * normalised as a₁ is. The integrals are taken by Simpson's rule, linear in u between entries.
 */
export function resampledPhase(table: PhaseTable): PhaseTable {
  if (onTableGrid(table.u)) {
    return table;
  }
  const last = PHASE_TABLE_ENTRIES - 1;
  const grid = Float64Array.from({ length: PHASE_TABLE_ENTRIES }, (_, i) => i / last);
  const factors = table.values.map((channel) => {
    const source = phaseIntegral(table.u, channel);
    const resampled = phaseIntegral(grid, resampledChannel(table.u, channel, grid));
    return resampled > 0 ? source / resampled : 1;
  });
  const element = (channels: PhaseMatrixElement): PhaseMatrixElement => {
    const [r, g, b] = channels.map((channel, c) =>
      resampledChannel(table.u, channel, grid).map((v) => v * (factors[c] ?? Number.NaN)),
    );
    if (r === undefined || g === undefined || b === undefined) {
      throw new Error("a phase table has three channels");
    }
    return [r, g, b];
  };
  const matrix = table.matrix;
  return phaseTable(
    grid,
    element(table.values),
    matrix === undefined
      ? undefined
      : {
          a2: element(matrix.a2),
          a3: element(matrix.a3),
          a4: element(matrix.a4),
          b1: element(matrix.b1),
          b2: element(matrix.b2),
        },
    table.forwardPeak,
  );
}

/**
 * A term with its phase table's forward delta taken out (plan R08, Design note 6; R08.T7;
 * `decision-r08-aerosol-labels.md` item 3), or the term itself where its table holds none.
 *
 * @remarks
 * In each channel c with a delta of share f_c (`PhaseTable.forwardPeak`, laid on the table's first
 * entries by `aerosol.ts`'s `aerosolPhaseTable`, `forwardPeak.ts`'s shape), the delta's entries
 * are taken from a₁, a₂, a₃ and a₄, which carry the identity's matrix, every element of what remains
 * is divided by 1 − f_c, and the term's scattering is multiplied by 1 − f_c, so that its extinction
 * falls by f_c times its scattering and its table integrates to 1 again, with mean cosine
 * (g − f_c μ_δ) ÷ (1 − f_c), μ_δ the delta's own (`FORWARD_PEAK_COSINE`). That is exact for a
 * forward delta (Potter 1970, J. Atmos. Sci. 27, 943): the delta's light is the direct beam's, which
 * the shared transmittance then carries to the star's image, its glare, the light at every
 * scattering point and on the ground, and which no per-view table could sample.
 *
 * @throws RangeError for a delta on a table that is not on the kernels' grid, where its entries
 *   do not stand.
 */
export function withoutForwardDelta(term: MediumTerm): MediumTerm {
  if (term.phase.kind !== "tabulated") {
    return term;
  }
  const table = term.phase.table;
  const shares = table.forwardPeak;
  if (shares.every((share) => share === 0)) {
    return term;
  }
  if (!onTableGrid(table.u)) {
    throw new RangeError(
      `term ${term.name}'s forward delta stands on a table of ${table.u.length} entries, not the kernels' ${PHASE_TABLE_ENTRIES}`,
    );
  }
  // a₁ is a phase, never negative, and its rounding at the delta's entries is held at 0; the other
  // elements take their signs.
  const remaining = (
    channels: PhaseMatrixElement,
    delta: boolean,
    phase: boolean,
  ): PhaseMatrixElement => {
    const [r, g, b] = channels.map((channel, c) => {
      const share = shares[c] ?? 0;
      const keep = 1 - share;
      return channel.map((v, k) => {
        const left = delta ? v - forwardPeakEntry(share, k) : v;
        return (phase ? Math.max(0, left) : left) / keep;
      });
    });
    if (r === undefined || g === undefined || b === undefined) {
      throw new Error("a phase table has three channels");
    }
    return [r, g, b];
  };
  const matrix = table.matrix;
  const kept = phaseTable(
    table.u,
    remaining(table.values, true, true),
    matrix === undefined
      ? undefined
      : {
          a2: remaining(matrix.a2, true, false),
          a3: remaining(matrix.a3, true, false),
          a4: remaining(matrix.a4, true, false),
          b1: remaining(matrix.b1, false, false),
          b2: remaining(matrix.b2, false, false),
        },
  );
  const [r, g, b] = term.scattering;
  return {
    ...term,
    scattering: [r * (1 - shares[0]), g * (1 - shares[1]), b * (1 - shares[2])],
    phase: { kind: "tabulated", table: kept },
  };
}
