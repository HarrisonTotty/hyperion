/**
 * The sky's settings per quality setting (plan R06, Design note 22, T13.f), carried as R05's
 * `ViewSettings.sky` with their values in R05's `SETTINGS`, and which of the sky's layers each
 * render style draws.
 *
 * @remarks
 * The low setting bakes 1,024² faces against 3,072² (34 MB with mips against about 300 MB), asks
 * an N_max of 10⁵ against 3 × 10⁵, and draws 2,048 sprites against 4,096. Those three figures are
 * provisional starting values, not sourced: open question 16 leaves them open and R06.T17 decides
 * them from measurements. A baked star is re-baked once the camera's motion shifts the nearest of
 * them by a tenth of a pixel (Design note 20) on both settings.
 */

/** The sky's settings for one quality setting. */
export interface SkySettings {
  /** The baked cube's face side, texels: 3,072 (high) or 1,024 (low). */
  readonly faceSizePx: number;
  /** The most stars drawn as sprites each frame. */
  readonly spriteBudget: number;
  /** The most stars a `sky` request asks to be listed (`n_max`). */
  readonly nMax: number;
  /**
   * The re-bake cadence: the shift of the nearest baked star, px, at which the cube is baked again
   * from the camera's new position (Design note 20).
   */
  readonly rebakeShiftPx: number;
}

/** The high setting's sky, the recommended specification's. */
export const HIGH_SKY: SkySettings = {
  faceSizePx: 3_072,
  spriteBudget: 4_096,
  nMax: 300_000,
  rebakeShiftPx: 0.1,
};

/** The low setting's sky, the UHD 620 class's. */
export const LOW_SKY: SkySettings = {
  faceSizePx: 1_024,
  spriteBudget: 2_048,
  nMax: 100_000,
  rebakeShiftPx: 0.1,
};

/** A view's render style as the sky draws it: R02's wireframe, or R07's photorealistic style. */
export type SkyStyle = "wireframe" | "photorealistic";

/** Which of the sky's layers a style draws. */
export interface SkyLayers {
  /** The bright and near stars as pixel-integrated sprites. */
  readonly sprites: boolean;
  /** The baked cube of the faint majority. */
  readonly cube: boolean;
  /** The unresolved band. */
  readonly band: boolean;
  /** The host stars' limb-darkened discs. */
  readonly discs: boolean;
}

/**
 * The sky's layers per style (Design note 23; decision record item 1 of 2026-10-02): the
 * wireframe draws the stars as exposed sprites and the baked cube, each tone-mapped per pixel,
 * and the band and the discs only in the photorealistic style.
 */
export const SKY_LAYERS: Readonly<Record<SkyStyle, SkyLayers>> = {
  wireframe: { sprites: true, cube: true, band: false, discs: false },
  photorealistic: { sprites: true, cube: true, band: true, discs: true },
};
