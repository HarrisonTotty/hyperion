/**
 * The tolerance patch selection runs at (plan R05, T11.c; decision-r05-patch-demand.md, 4d), free
 * of the engine, so that the terrain pass and the spike's CPU-only demand record take the one value
 * (decision-r05-record-tau.md).
 *
 * @remarks
 * The terrain pass re-selects only when the camera has moved more than {@link RESELECT_FRACTION}
 * of the distance to the nearest selected patch that is not at the finest level, so it selects at
 * τ_sel = τ ÷ (1 + {@link RESELECT_FRACTION}) for the drawn error to stay within about τ between
 * selections.
 */

/** The fraction of the nearest selected patch's distance the camera may move between selections. */
export const RESELECT_FRACTION = 0.1;

/**
 * The tolerance selection runs at for a setting's τ, pixels: τ_sel = τ ÷ (1 +
 * {@link RESELECT_FRACTION}), as the terrain pass selects and T13.a's demand record measures.
 *
 * @param tauPx - The setting's screen-space tolerance τ, pixels.
 */
export function selectionTolerancePx(tauPx: number): number {
  return tauPx / (1 + RESELECT_FRACTION);
}
