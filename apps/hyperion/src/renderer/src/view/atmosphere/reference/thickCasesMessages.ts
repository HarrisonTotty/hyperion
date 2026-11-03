/**
 * The thick cases' worker's message (plan R08, R08.T12.b.2): what `thickCases.worker.ts` posts the
 * render thread of `thickCases.ts`'s four cases, for `ATMOSPHERE TEST`'s thick bodies.
 *
 * @remarks
 * Beside type imports it holds only {@link thickCasesReplyOf}, which builds nothing itself, so
 * that the render thread takes nothing of the cases' optics from this module.
 */

import type { BodyCover } from "../thick/regime";
import type { ReferenceCase } from "./cases";
import type { ThickCaseCovers, ThickReferenceCases } from "./thickCases";

/**
 * What a scene's body takes of a case: its name, medium, figure, curves and sources, and the cover
 * its body is drawn under (R08.T15.b.2), absent for a clear sky.
 */
export interface FixtureCase extends Pick<
  ReferenceCase,
  "name" | "medium" | "figure" | "curves" | "sources"
> {
  readonly cover?: BodyCover;
}

/** The thick cases' fixture cases, by case name. */
export type ThickFixtureCases = Readonly<Record<keyof ThickReferenceCases, FixtureCase>>;

/**
 * The worker's one message: the four cases, their buffers transferred, or why they could not be
 * built.
 */
export type ThickCasesReply =
  | { readonly kind: "built"; readonly cases: ThickFixtureCases }
  | { readonly kind: "failed"; readonly message: string };

/** The fields of `referenceCase` a scene's body takes, under `cover` where one is given. */
function fixtureCaseOf(referenceCase: ReferenceCase, cover: BodyCover | undefined): FixtureCase {
  const { name, medium, figure, curves, sources } = referenceCase;
  return {
    name,
    medium,
    figure,
    curves,
    ...(sources === undefined ? {} : { sources }),
    ...(cover === undefined ? {} : { cover }),
  };
}

/**
 * The worker's reply: the four cases `build` makes, each as a scene's body takes it under its cover
 * from `coversOf`, or `failed` with the message of what `build` or `coversOf` threw.
 *
 * @param build - Builds the cases: `thickCases.ts`'s `thickReferenceCases` in the worker.
 * @param coversOf - The cases' covers: `thickCases.ts`'s `thickCaseCovers` in the worker.
 */
export function thickCasesReplyOf(
  build: () => ThickReferenceCases,
  coversOf: (cases: ThickReferenceCases) => ThickCaseCovers,
): ThickCasesReply {
  try {
    const cases = build();
    const covers = coversOf(cases);
    return {
      kind: "built",
      cases: {
        "venus-92bar": fixtureCaseOf(cases["venus-92bar"], covers["venus-92bar"]),
        "venus-58bar": fixtureCaseOf(cases["venus-58bar"], covers["venus-58bar"]),
        "titan-class": fixtureCaseOf(cases["titan-class"], covers["titan-class"]),
        "giant-deck": fixtureCaseOf(cases["giant-deck"], covers["giant-deck"]),
      },
    };
  } catch (error: unknown) {
    return { kind: "failed", message: error instanceof Error ? error.message : String(error) };
  }
}
