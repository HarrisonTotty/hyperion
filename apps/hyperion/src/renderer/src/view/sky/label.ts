/**
 * The view label block's sky line (plan R06, Design note 23, T13.f), to the guide's `STARS` row as
 * signed off in R06.T15: the limit as a V magnitude in `mag` with its kind, then what the sky
 * leaves out.
 *
 * @remarks
 * `STARS V 7.4 mag EYE` for the eye (its deepest limit, at the view's field factor) and
 * `STARS V 9.5 mag CAM` for a camera, never a magnitude alone; what is left out follows after a
 * middle dot as one composed note, `STARS V 9.5 mag CAM · CLUSTERS AND WHITE DWARFS: NOT YET
 * MODELLED`. Until the sky has arrived the line
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
 * Each gap's subject in the composed `NOT YET MODELLED` note. The centre's members are a
 * feature's members too, so both read `CLUSTERS`, once.
 */
const GAP_PHRASES: Readonly<Record<SkyGapDto, string>> = {
  feature_members: "CLUSTERS",
  centre_members: "CLUSTERS",
  white_dwarfs: "WHITE DWARFS",
};

/** The subjects' fixed order in the note, whatever the gaps' order: clusters, then white dwarfs. */
const SUBJECT_ORDER: ReadonlyArray<string> = ["CLUSTERS", "WHITE DWARFS"];

/** `A`, `A AND B`, `A, B AND C`: the guide's composed list of what is not yet modelled. */
function joinSubjects(subjects: ReadonlyArray<string>): string {
  if (subjects.length <= 1) {
    return subjects.join("");
  }
  return `${subjects.slice(0, -1).join(", ")} AND ${subjects[subjects.length - 1]}`;
}

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
  const present = new Set(gaps.map((gap) => GAP_PHRASES[gap]));
  const subjects = SUBJECT_ORDER.filter((subject) => present.has(subject));
  const limit = `V ${formatNumber(limitV, 1)} mag ${KIND_WORDS[limitKind]}`;
  return subjects.length === 0 ? limit : `${limit} · ${joinSubjects(subjects)}: NOT YET MODELLED`;
}
