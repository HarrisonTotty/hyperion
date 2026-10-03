/**
 * The photorealistic style's pass list (plan R07, Design note 8): what draws into a view's HDR scene
 * target, in order, and the labels R12's `PASS_ROWS` keys on.
 *
 * @remarks
 * In order: R06's sky; mesh bodies (opaque, depth); R10's terrain; host discs, disc bodies and
 * point bodies in one painter order (Design note 2); R08's atmosphere; R11's rings, clouds and
 * oceans; the histogram; bloom of the light above the display's range; the tone-mapping pass; the
 * symbology cased over the result. A style owns no scene, camera or projection, so switching style
 * is a change of pass list. The slots of R06, R08, R10 and R11 are listed with the labels their
 * plans give or (for R06, R08 and R11, whose passes are not built) the lane's provisional names,
 * marked `built: false` until those plans fill them; the settings share one order today, their
 * differences being each pass's parameters (Design note 18).
 */
import type { QualitySetting } from "../quality/qualitySetting";
import { TERRAIN_PASS_LABEL } from "../terrain/gpu/material";
import { BLOOM_PASS } from "../post/bloomChain";
import { HISTOGRAM_PASS } from "../post/histogram";
import { TONEMAP_PASS } from "../post/tonemap";

/**
 * This plan's pass labels, fixed across frames and releases, so that R12's `PASS_ROWS` keys on
 * them (each the `FrameSubmission.label` or `dispatch` pass of its own submission).
 */
export const PHOTOREAL_PASS_LABELS = {
  bodies: "bodies",
  discs: "discs",
  histogram: HISTOGRAM_PASS,
  bloom: BLOOM_PASS,
  tonemap: TONEMAP_PASS,
  symbology: "symbology",
} as const;

/** The plan that owns a pass. */
export type PassOwner = "R06" | "R07" | "R08" | "R10" | "R11";

/** One pass of the list. */
export interface PassEntry {
  readonly label: string;
  readonly owner: PassOwner;
  /** `false` for a slot its owning plan has not built yet. */
  readonly built: boolean;
}

/** The photorealistic style's passes, in order. */
export interface PassList {
  readonly passes: ReadonlyArray<PassEntry>;
}

/** A pass of this plan's, built. */
function own(label: string): PassEntry {
  return { label, owner: "R07", built: true };
}

/** Another plan's slot. */
function slot(label: string, owner: PassOwner, built: boolean): PassEntry {
  return { label, owner, built };
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
      slot("sky", "R06", false),
      own(PHOTOREAL_PASS_LABELS.bodies),
      slot(TERRAIN_PASS_LABEL, "R10", true),
      own(PHOTOREAL_PASS_LABELS.discs),
      slot("atmosphere", "R08", false),
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
