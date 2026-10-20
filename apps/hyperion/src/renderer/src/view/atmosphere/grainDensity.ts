/**
 * A substance's grain density by phase and temperature, read from the substance registry's public
 * mirror, `packages/protocol/fixtures/substances.json` (galaxy plan 14, P14.T54.a;
 * decision-p14-t54a-grain-density §2).
 *
 * @remarks
 * The sim takes a mode's grain density at its phase and temperature (`Substance::grain_density`,
 * which `aerosol_optics` reads for κ_ext = 3 Q̄_ext ÷ (4 ρ r_eff)), so the mirror carries each
 * row's density as tables of (T, ρ) nodes, linear in T, which hold the sim's within 10⁻⁴
 * (relative) wherever both give one. R08.T19's fifth row, the client's τ(550) against the
 * record's at generator version 24, reads ρ here at a mode's `phase` and `temperature_k`.
 *
 * The rule, as the fixture's `description` and the sim's test (`substance_golden.rs`'s
 * `mirror_density`) state it:
 *
 * 1. If the phase's member is `null`, there is no density.
 * 2. Take the piece with `from_k ≤ T < to_k`; the member's last piece also covers `T = to_k`, and a
 *    `null` bound is unbounded. If no piece covers T, there is no density.
 * 3. Within the piece, ρ is the first node's at or below it, the last node's above it, and between
 *    the nodes T₀ < T ≤ T₁ it is ρ₀ + f (ρ₁ − ρ₀) with f = (T − T₀) ÷ (T₁ − T₀).
 *
 * A row with a phase diagram holds its saturated liquid from its triple temperature to 0.99 of its
 * critical temperature only: the sim gives one above it too, which no linear table follows to the
 * critical density, and no droplet is plausible there. A density is of the row's own composition,
 * `grain_density_variant` (`75wt%` for H₂SO₄, the substance itself otherwise), and none is given
 * for another variant, as the sim refuses one.
 */
import type { MaterialPhase } from "./materials/materials";

/** One piece of a phase's grain-density table, as the fixture writes it. */
export interface GrainDensityPiece {
  /** The lowest temperature the piece covers, K, or `null` for no bound. */
  readonly from_k: number | null;
  /**
   * The temperature, K, below which the piece covers, or to which the member's last piece covers,
   * or `null` for no bound.
   */
  readonly to_k: number | null;
  /** The nodes' temperatures, K, strictly rising: at least one. */
  readonly temperatures_k: ReadonlyArray<number>;
  /** The nodes' grain densities, kg m⁻³, finite and positive: one a node. */
  readonly densities_kg_per_m3: ReadonlyArray<number>;
}

/** A row of the fixture as the reader reads it: its grain density's variant and tables. */
export interface GrainDensityRow {
  /** The index-file variant whose composition the densities are, or `null` for the substance. */
  readonly grain_density_variant: string | null;
  /** The solid's pieces, rising and not overlapping, or `null` where none was read. */
  readonly solid_grain_density: ReadonlyArray<GrainDensityPiece> | null;
  /** The liquid's, as the solid's. */
  readonly liquid_grain_density: ReadonlyArray<GrainDensityPiece> | null;
}

/** The phase's member of a row. */
function memberOf(
  row: GrainDensityRow,
  phase: MaterialPhase,
): ReadonlyArray<GrainDensityPiece> | null {
  let member: ReadonlyArray<GrainDensityPiece> | null;
  switch (phase) {
    case "solid":
      member = row.solid_grain_density;
      break;
    case "liquid":
      member = row.liquid_grain_density;
      break;
  }
  return member;
}

/** Whether a piece covers a temperature, K, the member's last piece its upper bound too. */
function covers(piece: GrainDensityPiece, temperatureK: number, isLast: boolean): boolean {
  const above = piece.from_k === null || piece.from_k <= temperatureK;
  const below =
    piece.to_k === null || temperatureK < piece.to_k || (isLast && temperatureK <= piece.to_k);
  return above && below;
}

/** A node of a piece, at an index the caller holds inside it. */
function nodeOf(piece: GrainDensityPiece, index: number): readonly [number, number] {
  const temperatureK = piece.temperatures_k[index];
  const densityKgPerM3 = piece.densities_kg_per_m3[index];
  if (temperatureK === undefined || densityKgPerM3 === undefined) {
    throw new Error(`a grain-density piece has node ${index} in both of its arrays`);
  }
  return [temperatureK, densityKgPerM3];
}

/** The piece's density at a temperature it covers, K: linear between its nodes, held beyond them. */
function interpolate(piece: GrainDensityPiece, temperatureK: number): number {
  const count = piece.temperatures_k.length;
  if (count === 0 || piece.densities_kg_per_m3.length !== count) {
    throw new Error("a grain-density piece has one or more nodes, its arrays of equal length");
  }
  let [lowK, lowDensity] = nodeOf(piece, 0);
  if (temperatureK <= lowK) {
    return lowDensity;
  }
  for (let index = 1; index < count; index++) {
    const [highK, highDensity] = nodeOf(piece, index);
    if (temperatureK <= highK) {
      const f = (temperatureK - lowK) / (highK - lowK);
      return lowDensity + f * (highDensity - lowDensity);
    }
    [lowK, lowDensity] = [highK, highDensity];
  }
  return lowDensity;
}

/**
 * A row's grain density, without pore space, as a phase at a temperature, by the fixture's rule.
 *
 * @param temperatureK - The temperature, K.
 * @param variant - The index file's variant (`75wt%`, `S35`), or undefined for the substance
 *   itself, as a mode's material names it.
 * @returns The density, kg m⁻³, or undefined where the row has none at that phase and temperature,
 *   or none of that variant.
 */
export function grainDensityKgPerM3(
  row: GrainDensityRow,
  phase: MaterialPhase,
  temperatureK: number,
  variant?: string,
): number | undefined {
  if ((variant ?? null) !== row.grain_density_variant) {
    return undefined;
  }
  const pieces = memberOf(row, phase);
  if (pieces === null) {
    return undefined;
  }
  const last = pieces.length - 1;
  const piece = pieces.find((candidate, index) => covers(candidate, temperatureK, index === last));
  return piece === undefined ? undefined : interpolate(piece, temperatureK);
}
