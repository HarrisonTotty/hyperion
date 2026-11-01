/**
 * The body inspection's survey pass as its label block states it (the owner's request,
 * 2026-11-01; the guide's `SURVEY PASS` rows), so that the pass `VIEW BODY` sends is closed-loop:
 * `SURVEY PASS: PENDING` while it is out, the server's refusal once it refuses it, and nothing once
 * it is recorded, when the survey line reads the coverage it gave.
 */
import type { InspectionSurvey } from "../../view/scenes/inspectionSurvey";
import type { LabelStatement } from "./viewRun";

/** The statement while the inspection's survey pass is out. */
export const SURVEY_PASS_PENDING = "SURVEY PASS: PENDING";

/** How a refused pass is tried again: the inspection opened anew, which sends it again. */
export const SURVEY_PASS_REMEDY = "reopen with VIEW BODY to retry";

/**
 * The statement of the inspection's survey pass, `outcome` once it has ended, else `null` while it
 * is out: {@link SURVEY_PASS_PENDING}; nothing once recorded; or `SURVEY PASS REJECTED: <the
 * reason>, reopen with VIEW BODY to retry`, in `--text` for a refusal of what was asked and in
 * `--status-caution` for the server's own failure, as `SURVEY COVERAGE REJECTED` is.
 */
export function surveyPassStatement(outcome: InspectionSurvey | null): LabelStatement | null {
  if (outcome === null) {
    return SURVEY_PASS_PENDING;
  }
  let statement: LabelStatement | null;
  switch (outcome.kind) {
    case "recorded":
      statement = null;
      break;
    case "refused": {
      const said = outcome.reason.trim();
      const words = said.length > 0 ? `${said}, ${SURVEY_PASS_REMEDY}` : SURVEY_PASS_REMEDY;
      statement = { text: `SURVEY PASS REJECTED: ${words}`, standing: outcome.standing };
      break;
    }
  }
  return statement;
}
