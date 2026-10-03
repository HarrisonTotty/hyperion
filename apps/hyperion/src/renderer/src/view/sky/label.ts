/**
 * The view label block's sky line (plan R06, Design note 23, T13.f), to R06.T15's draft of the
 * guide's `STARS` row: the limit as a V magnitude with its kind, then each stand-in that holds.
 *
 * @remarks
 * `STARS V 7.4 EYE` for the eye (its deepest limit, at the view's field factor) and
 * `STARS V 9.5 CAM` for a camera, never a magnitude alone; a stand-in follows after a middle dot,
 * `STARS V 9.5 CAM · CLUSTERS NOT MODELLED · WD NOT MODELLED`. Until the sky has arrived the line
 * stays R02's (`STAR_SOURCE` and `STARS_WITHOUT_POSITION` in `displays/view/viewRun.ts`), which the
 * label block chooses between.
 */

import type { SkyGapDto } from "@hyperion/protocol";

import { formatNumber } from "../../lib/format";

/** A view's role for its star limit: the eye's threshold or a camera's noise floor. */
export type SkyLimitKind = "eye" | "camera";

/** Each limit kind's word on the line, as the guide's `EYE` and `CAM` rows name them. */
const KIND_WORDS: Readonly<Record<SkyLimitKind, string>> = { eye: "EYE", camera: "CAM" };

/**
 * Each gap's stand-in phrase. The centre's members are a feature's members too, so both read
 * `CLUSTERS NOT MODELLED`, once.
 */
const GAP_PHRASES: Readonly<Record<SkyGapDto, string>> = {
  feature_members: "CLUSTERS NOT MODELLED",
  centre_members: "CLUSTERS NOT MODELLED",
  white_dwarfs: "WD NOT MODELLED",
};

/**
 * The `STARS` line's reading, after its label, once the sky has arrived.
 *
 * @param limitV - The view's limit: a camera's, or the eye's deepest over the view.
 * @param gaps - The response's `not_modelled`.
 */
export function skyLabelValue(
  limitV: number,
  limitKind: SkyLimitKind,
  gaps: ReadonlyArray<SkyGapDto>,
): string {
  const phrases = [...new Set(gaps.map((gap) => GAP_PHRASES[gap]))];
  return [`V ${formatNumber(limitV, 1)} ${KIND_WORDS[limitKind]}`, ...phrases].join(" · ");
}
