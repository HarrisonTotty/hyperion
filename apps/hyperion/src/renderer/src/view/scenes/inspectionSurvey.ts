/**
 * The body inspection's survey (the owner's request, 2026-11-01): the inspection is a testing tool,
 * so its body shows its generated terrain at once, with no survey to fly. As it opens, the whole
 * body is surveyed through the server's own survey path, the pass `just place-ship --survey`
 * records ({@link wholeBodySurveyPass}): every cell of its coarse field, orbital at 50 m, the
 * served descent spike's survey, at the inspection's time.
 *
 * @remarks
 * The server serves a body's field only where its survey log covers it (R09's gate on
 * `surface_field`), so without the pass the inspected ground stays the reference spheroid and its
 * line of sight not surveyed. The level is found from the figure of the body's record that the
 * `SYSTEM` display holds, as the server finds it from the same record's. The server records the pass
 * in the universe's survey log, as it records `--survey`'s: the wire marks no pass as training.
 */

import type { BodyIdHex, RequestClient } from "@hyperion/protocol";

import { isRefusal } from "../../lib/refusal";
import { figureLevel, type SurveyAsked, wholeBodySurveyPass } from "../../lib/wholeBodySurvey";
import { surfaceless } from "../scene/fromServer";
import type { BodyInspection } from "./inspectionScene";

/** How the inspection surveys its body: orbital at 50 m, as the served descent spike does. */
export const INSPECTION_SURVEY: SurveyAsked = { source: "orbital", resolutionM: 50 };

/** How the inspection's survey ended. */
export type InspectionSurvey =
  | {
      readonly kind: "recorded";
      readonly body: BodyIdHex;
      /** The level of the body's coarse field the pass covered, its every cell. */
      readonly level: number;
      /** The body's coverage revision with the pass folded in. */
      readonly revision: number;
    }
  /** No pass sent: the body has no solid surface to survey, a giant's (`surfaceless`). */
  | { readonly kind: "surfaceless"; readonly body: BodyIdHex }
  | {
      readonly kind: "refused";
      readonly body: BodyIdHex;
      /** Why: the body's record holds no figure, or the server's own reason. */
      readonly reason: string;
      /** A refusal of what was asked, or the server's own failure (`lib/refusal.ts`). */
      readonly standing: "refused" | "fault";
    };

/**
 * Surveys the inspected body whole ({@link INSPECTION_SURVEY}) through `requests`, and gives the
 * pass's revision, or why none was recorded; never rejects. A body with no solid surface, a
 * giant's, is not surveyed: the server refuses its pass.
 */
export async function surveyInspected(
  requests: Pick<RequestClient, "request">,
  inspection: BodyInspection,
): Promise<InspectionSurvey> {
  const { universe, body, time } = inspection;
  const record = inspection.bodies.bodies.find((each) => each.id === body);
  if (record !== undefined && surfaceless(record)) {
    return { kind: "surfaceless", body };
  }
  const figure = record?.figure;
  if (figure?.state !== "ok") {
    return {
      kind: "refused",
      body,
      reason: "its record holds no figure to give its level",
      standing: "refused",
    };
  }
  const level = figureLevel(figure.value);
  const outcome = await requests.request(
    wholeBodySurveyPass({ universe, body, level, survey: INSPECTION_SURVEY, time }),
  ).outcome;
  return outcome.ok
    ? { kind: "recorded", body, level, revision: outcome.response.revision }
    : {
        kind: "refused",
        body,
        reason: outcome.error.message,
        standing: isRefusal(outcome.error.code) ? "refused" : "fault",
      };
}
