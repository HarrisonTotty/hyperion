/**
 * The photorealistic style's pass list (plan R07, Design note 8): what draws into a view's HDR scene
 * target, in order, and the labels R12's `PASS_ROWS` keys on.
 *
 * @remarks
 * In order: R06's sky; the small discs' cells and the larger discs' limbs' and their faces'
 * near-limb bands' (R07.T8.d–g, a compute pass, a dispatch a set of bound textures, whose sums the
 * bodies' draws read); R10's terrain shadows (R10.T9.b, the shadow cascades' four caster passes, depth
 * alone into the engine's layers, which the terrain's draws read); mesh bodies (opaque, depth);
 * R10's terrain; host discs, disc bodies and point bodies in one painter order (Design note 2);
 * R08's atmosphere, each body's composite over everything drawn before it (R08.T10.b); R11's
 * rings, clouds and oceans; the histogram; bloom of the light above the
 * display's range; the tone-mapping pass; the symbology cased over the result. A style owns no
 * scene, camera or projection, so switching style is a change of pass list. The slots of R06, R08,
 * R10 and R11 are listed with the labels their plans give or (for R11, whose passes are not built)
 * the lane's provisional names, marked `built: false` until those plans fill them (R06's sky, drawn
 * by R07.T8.a's frame, is; R08's atmosphere since R08.T10.b, its slot carrying its passes' labels); the settings share one order today, their differences being
 * each pass's parameters (Design note 18): on the high setting the terrain's cascade passes are
 * submitted before the bodies pass, while the low setting, which has no cascades, submits none in
 * their slot.
 */
import { ATMOSPHERE_COMPOSITE_PASS, ATMOSPHERE_VIEW_PASS } from "../atmosphere/hillaire";
import { ATMOSPHERE_TABLES_PASS } from "../atmosphere/tables";
import { DISC_CELLS_PASS } from "../bodies/draw";
import type { QualitySetting } from "../quality/qualitySetting";
import { TERRAIN_PASS_LABEL, TERRAIN_SHADOW_PASS_LABEL } from "../terrain/gpu/material";
import { BLOOM_PASS } from "../post/bloomChain";
import { HISTOGRAM_PASS } from "../post/histogram";
import { TONEMAP_PASS } from "../post/tonemap";

/**
 * This plan's pass labels, fixed across frames and releases, so that R12's `PASS_ROWS` keys on
 * them (each the `FrameSubmission.label` or `dispatch` pass of its own submission).
 */
export const PHOTOREAL_PASS_LABELS = {
  /**
   * The small discs' cells and the larger discs' limbs' and their faces' near-limb bands', shaded
   * in parallel for the draws that read them, a dispatch a set of bound textures under one label
   * (R07.T8.d–g).
   */
  discCells: DISC_CELLS_PASS,
  bodies: "bodies",
  discs: "discs",
  histogram: HISTOGRAM_PASS,
  bloom: BLOOM_PASS,
  tonemap: TONEMAP_PASS,
  symbology: "symbology",
} as const;

/**
 * R06's sky pass's label: its band, its baked cube and the star sprites, which R07.T8.a's frame
 * draws first into the scene target.
 */
export const SKY_PASS_LABEL = "sky";

/**
 * The labels R08's atmosphere submits its passes under (plan R08, R08.T10.b), in the order a frame
 * submits them: a body's per-planet table builds where it builds them (the atmosphere cache's, at a
 * body's arrival or a new light set, not every frame), then each body's sky view, aerial perspective
 * and ray march, and its composite over the scene, farthest body first.
 */
export const ATMOSPHERE_PASS_LABELS: ReadonlyArray<string> = [
  ATMOSPHERE_TABLES_PASS,
  ATMOSPHERE_VIEW_PASS,
  ATMOSPHERE_COMPOSITE_PASS,
];

/** The plan that owns a pass. */
export type PassOwner = "R06" | "R07" | "R08" | "R10" | "R11";

/** One pass of the list. */
export interface PassEntry {
  readonly label: string;
  readonly owner: PassOwner;
  /** `false` for a slot its owning plan has not built yet. */
  readonly built: boolean;
  /**
   * The labels its passes are submitted under where a slot's passes are several, R08's
   * atmosphere's ({@link ATMOSPHERE_PASS_LABELS}); absent where its one pass is `label`.
   */
  readonly labels?: ReadonlyArray<string>;
}

/** The photorealistic style's passes, in order. */
export interface PassList {
  readonly passes: ReadonlyArray<PassEntry>;
}

/** A pass of this plan's, built. */
function own(label: string): PassEntry {
  return { label, owner: "R07", built: true };
}

/** Another plan's slot, with its passes' labels where they are several. */
function slot(
  label: string,
  owner: PassOwner,
  built: boolean,
  labels?: ReadonlyArray<string>,
): PassEntry {
  return labels === undefined ? { label, owner, built } : { label, owner, built, labels };
}

/**
 * The pass list of a photorealistic view on a setting.
 *
 * @param setting - The view's quality setting; both share one order today.
 */
export function photorealisticPasses(setting: QualitySetting): PassList {
  // A new setting must choose its pass order (Design note 18): the exhaustive switch says so.
  switch (setting) {
    case "high":
    case "low":
      break;
  }
  return {
    passes: [
      slot(SKY_PASS_LABEL, "R06", true),
      // Before the mesh bodies' figures, which read its sums as the discs' draws do.
      own(PHOTOREAL_PASS_LABELS.discCells),
      // Before the bodies, whose resolve times them, and the terrain, which reads the layers.
      slot(TERRAIN_SHADOW_PASS_LABEL, "R10", true),
      own(PHOTOREAL_PASS_LABELS.bodies),
      slot(TERRAIN_PASS_LABEL, "R10", true),
      own(PHOTOREAL_PASS_LABELS.discs),
      // Each body's air over everything opaque drawn before it (R08.T10.b).
      slot("atmosphere", "R08", true, ATMOSPHERE_PASS_LABELS),
      slot("rings", "R11", false),
      slot("clouds", "R11", false),
      slot("ocean", "R11", false),
      own(PHOTOREAL_PASS_LABELS.histogram),
      own(PHOTOREAL_PASS_LABELS.bloom),
      own(PHOTOREAL_PASS_LABELS.tonemap),
      own(PHOTOREAL_PASS_LABELS.symbology),
    ],
  };
}
