/**
 * The tolerance patch selection runs at (plan R05, T11.c; decision-r05-patch-demand.md, 4d), free
 * of the engine, so that the terrain pass and the spike's CPU-only demand record take the one value
 * (decision-r05-record-tau.md), and the camera move after which the terrain pass selects again.
 *
 * @remarks
 * The terrain pass selects at τ_sel = τ ÷ (1 + m), for m = {@link RESELECT_FRACTION}, and selects
 * again once the camera has moved more than {@link RESELECT_MOVE_FRACTION} = m ÷ (1 + m) of the
 * box distance to the nearest selected patch that is not at the finest level. Together they keep
 * the drawn error within τ between selections.
 */

/**
 * The selection's margin m: selection runs at τ ÷ (1 + m), and the camera may move m ÷ (1 + m) of
 * the nearest selected non-finest patch's box distance between selections
 * ({@link RESELECT_MOVE_FRACTION}).
 */
export const RESELECT_FRACTION = 0.1;

/**
 * The fraction of d_min that the camera may move before the terrain pass selects again, where
 * d_min is the box distance to the nearest selected patch not at the finest level, floored at one
 * finest patch: m ÷ (1 + m) for m = {@link RESELECT_FRACTION}, about 0.0909
 * (decision-r05-record-tau.md, "Noted for lane C").
 *
 * @remarks
 * Selection leaves every baked leaf the view sees with ρ ≤ τ_sel = τ ÷ (1 + m), at a box distance
 * d. A box's distance falls by at most the camera's move. A move of f × d_min with d_min ≤ d
 * therefore leaves the leaf at least (1 − f) d away, with ρ at most τ_sel ÷ (1 − f). With
 * f = m ÷ (1 + m), 1 − f = 1 ÷ (1 + m), so the bound is τ exactly. The former f = m gave
 * τ ÷ (1 − m²), about 1.0101 τ.
 *
 * Finest-level leaves have no error to bound. The floor could loosen the bound only for a coarser
 * leaf nearer than one finest patch that meets τ_sel. On the test planet at the settings' views
 * there is none: a level-18 leaf meets τ_sel only beyond 45 m, on low at 640 px wide, and a finest
 * patch is 20.7 m.
 *
 * Where the budget binds, the leaves meet τ′ at selection and (1 + m) τ′ between selections,
 * the tolerance the morph bands are computed at. A coarse–fine edge then stays at or beyond its
 * band's end, at morph 1. τ_sel is unchanged, and so is T13.a's demand record, which selects
 * every frame.
 */
export const RESELECT_MOVE_FRACTION = RESELECT_FRACTION / (1 + RESELECT_FRACTION);

/**
 * The tolerance selection runs at for a setting's τ, pixels: τ_sel = τ ÷ (1 +
 * {@link RESELECT_FRACTION}), as the terrain pass selects and T13.a's demand record measures.
 *
 * @param tauPx - The setting's screen-space tolerance τ, pixels.
 */
export function selectionTolerancePx(tauPx: number): number {
  return tauPx / (1 + RESELECT_FRACTION);
}
