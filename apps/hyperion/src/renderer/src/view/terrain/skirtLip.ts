/**
 * A skirt's lip (plan R10, R10.T8.c): the part of a lit patch's skirt foot strip near its edge,
 * which shades as the grid at its edge vertex; the TypeScript twin of `shaders/terrainLit.wgsl`'s
 * `skirt_lip_m` and `skirt_lip`.
 *
 * @remarks
 * Every patch's edge hangs a foot strip (`gpu/resources.ts`' `litPatchMeshData`), which inside the
 * survey fills the cracks between patches and at the survey's edge is the wall down to the
 * reference surface (R10.T10.c), drawn in the reference surface's appearance: law 0 under the
 * datum's normal, with no horizon map. Seen through a crack, that strip was lit inside the
 * terrain's shadow at a low sun: the single bright dots of the sunset's steps. A crack between
 * patches is shallow once the coarse–fine holds (`coarseFineHolds.ts`) have closed the morph's: a
 * T-junction's sagitta, or a rounding. So the strip within its lip of its edge shades as the grid
 * there, and the wall below the lip keeps the reference surface's appearance.
 */

import { LIT_VERTEX_CODE } from "./gpu/resources";

/** A part of the lit mesh: one of `LIT_VERTEX_CODE`'s codes. */
export type LitVertexCode = (typeof LIT_VERTEX_CODE)[keyof typeof LIT_VERTEX_CODE];

/** The least lip, metres: well above any rounding's crack. */
export const MIN_SKIRT_LIP_M = 1;

/**
 * The depth below its edge to which a skirt's foot strip stands in for the terrain, metres: 1 m,
 * or twice the sagitta of the parent's quad where that is more.
 *
 * @remarks
 * A quad spans at most (8 ÷ 3) a s, a the body's equatorial radius and s the slot record's step in
 * cell coordinates (the warp's du/ds is at most 8 ÷ 3, and a face's angle per unit of u at most
 * 1), so the parent's, twice as long, has a sagitta ℓ² ÷ 8a of at most (32 ÷ 9) a s².
 *
 * @param equatorialRadiusM - a, the slot record's `scale.x`.
 * @param step - The slot record's `step`, 2^−(L + 6) at level L.
 */
export function skirtLipM(equatorialRadiusM: number, step: number): number {
  return Math.max(MIN_SKIRT_LIP_M, (64 / 9) * equatorialRadiusM * step * step);
}

/**
 * Whether a fragment of the lit mesh's part `kind` shades as the grid at its edge vertex: the
 * grid's own, and a skirt's foot strip within `lipM` below its edge vertex's height
 * `gridHeightM`, at its own height `heightM` (metres, both above the datum).
 */
export function shadesAsGrid(
  kind: LitVertexCode,
  gridHeightM: number,
  heightM: number,
  lipM: number,
): boolean {
  return (
    kind === LIT_VERTEX_CODE.grid ||
    (kind === LIT_VERTEX_CODE.skirtFoot && gridHeightM - heightM <= lipM)
  );
}
