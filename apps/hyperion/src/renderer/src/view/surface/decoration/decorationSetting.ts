/**
 * Plan R11's ladder entries and whether a frame draws decoration (plan R11, R11.T1; Design notes 2
 * and 18).
 *
 * @remarks
 * - **The ladder.** R11's entries are fields of R05's `ViewSettings`, with their high and low values
 *   in `SETTINGS` (`view/quality/qualitySetting.ts`): `decoration`, `scatterDensity`, `clouds`,
 *   `cloudShadows`, `oceanComponents`, `oceanRefraction`, `whitecaps`, `rings` and
 *   `ringShadowOnBody` (Design note 18). This module holds their types ({@link R11Ladder}).
 * - **What decoration is.** What only the GPU draws on the ground below what the height function
 *   computes: normal detail, colour variation, ripples, small crater scars, the detail textures and
 *   scatter below 0.4 m across (Design note 2). The low setting draws none of it, so `DECORATION ON`
 *   means exactly what the ladder's "decoration off" means. Clouds and waves are not decoration.
 * - **The label follows what is drawn.** `DECORATION ON` stands while a frame drew decoration, not
 *   while the setting merely allows it ({@link decorationStatement}): a body whose detail sets are
 *   not yet generated draws undecorated and states nothing (Design note 21), and the frame-rate
 *   floor turns decoration off while it has stepped the view down ({@link decorationAllowed}).
 */

import type { RenderStyle } from "../../camera/state";

/**
 * The annunciation while any decoration is drawn (Design note 2; the guide's `DECORATION ON` row,
 * R02's nomenclature): after the survey line's middle dot, steady, in `--text`.
 */
export const DECORATION_ON = "DECORATION ON";

/**
 * How clouds are drawn (Design note 18): `volume`, the raymarched volume of the high setting, or
 * `layer`, the low setting's two-dimensional layer, both from one optical-depth field.
 */
export type CloudRendering = "volume" | "layer";

/**
 * How rings are drawn close to (Design note 18): `particles`, instanced once the largest subtends a
 * pixel, or `annulus`, the low setting's textured annulus sampling the same shadowing table.
 */
export type RingRendering = "particles" | "annulus";

/** R11's ladder entries, as fields of R05's `ViewSettings` (Design note 18). */
export interface R11Ladder {
  /**
   * Whether GPU decoration is drawn on the terrain, decorative scatter included: on high, off low,
   * so that the low setting draws no decoration of any kind and compiles none.
   */
  readonly decoration: boolean;
  /**
   * The fraction of the decorative instance density drawn, in [0, 1]: 1 high, 0 low, at which no
   * scatter pipeline is created; R12 may tune it.
   */
  readonly scatterDensity: number;
  /** How clouds are drawn: `volume` high, `layer` low. */
  readonly clouds: CloudRendering;
  /** Whether clouds shadow the terrain: on high, off low (Design note 10). */
  readonly cloudShadows: boolean;
  /** The Gerstner components of the near ocean: 32 high, about eight low (Design note 12). */
  readonly oceanComponents: number;
  /** Whether the near ocean refracts: on high, off low. */
  readonly oceanRefraction: boolean;
  /** Whether the ocean draws whitecaps: on high, off low; glint stands on both. */
  readonly whitecaps: boolean;
  /** How rings are drawn close to: `particles` high, `annulus` low. */
  readonly rings: RingRendering;
  /**
   * Whether the ring's shadow falls on its planet: on high; off low, which keeps the planet's
   * analytic shadow on the ring (the ladder's Rings row).
   */
  readonly ringShadowOnBody: boolean;
}

/** What a frame's decoration depends on beyond its setting. */
export interface DecorationFrame {
  /** The style the frame is drawn in: the wireframe draws no decoration (Design note 18). */
  readonly style: RenderStyle;
  /**
   * Whether the view's frame-rate floor has stepped it down (`BudgetedScale.airStepped`), which
   * turns decoration off until it is calm again (the owner's priority: the frame rate first).
   */
  readonly floorStepped: boolean;
}

/**
 * Whether a frame may draw decoration: in the photorealistic style, on a setting whose `decoration`
 * is on, while the frame-rate floor has not stepped the view down. What it draws then is what its
 * terrain's detail sets allow, which the label follows ({@link decorationStatement}).
 */
export function decorationAllowed(
  ladder: Pick<R11Ladder, "decoration">,
  frame: DecorationFrame,
): boolean {
  return ladder.decoration && frame.style === "photorealistic" && !frame.floorStepped;
}

/**
 * Where a view states `DECORATION ON` for what its last frame drew: after the survey line's middle
 * dot while that line stands as a reading (`surveyLines`; the guide's survey line), else among its
 * label block's statements (`labelStatements`), so that the label stands wherever decoration is
 * drawn, even on a body other than the `FRAME`'s; `null` while none was drawn.
 *
 * @param drawn - Whether the frame drew any decoration (`ViewFrameDrawn.decoration`).
 * @param surveyReading - Whether the view's survey line stands as a reading.
 */
export function decorationStatement(
  drawn: boolean,
  surveyReading: boolean,
): "survey-line" | "statement" | null {
  if (!drawn) {
    return null;
  }
  return surveyReading ? "survey-line" : "statement";
}
