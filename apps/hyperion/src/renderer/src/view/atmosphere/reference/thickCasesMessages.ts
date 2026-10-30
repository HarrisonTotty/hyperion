/**
 * The thick cases' worker's message (plan R08, R08.T12.b.2): what `thickCases.worker.ts` posts the
 * render thread of `thickCases.ts`'s four cases, for `ATMOSPHERE TEST`'s thick bodies.
 *
 * @remarks
 * Beside type imports it holds only {@link thickCasesReplyOf}, which builds nothing itself, so
 * that the render thread takes nothing of the cases' optics from this module.
 */

import type { ReferenceCase } from "./cases";
import type { ThickReferenceCases } from "./thickCases";

/** What a scene's body takes of a case: its name, medium, figure, curves and sources. */
export type FixtureCase = Pick<ReferenceCase, "name" | "medium" | "figure" | "curves" | "sources">;

/** The thick cases' fixture cases, by case name. */
export type ThickFixtureCases = Readonly<Record<keyof ThickReferenceCases, FixtureCase>>;

/**
 * The worker's one message: the four cases, their buffers transferred, or why they could not be
 * built.
 */
export type ThickCasesReply =
  | { readonly kind: "built"; readonly cases: ThickFixtureCases }
  | { readonly kind: "failed"; readonly message: string };

/** The fields of `referenceCase` a scene's body takes. */
function fixtureCaseOf(referenceCase: ReferenceCase): FixtureCase {
  const { name, medium, figure, curves, sources } = referenceCase;
  return sources === undefined
    ? { name, medium, figure, curves }
    : { name, medium, figure, curves, sources };
}

/**
 * The worker's reply: the four cases `build` makes, each as a scene's body takes it, or `failed`
 * with the message of what `build` threw.
 *
 * @param build - Builds the cases: `thickCases.ts`'s `thickReferenceCases` in the worker.
 */
export function thickCasesReplyOf(build: () => ThickReferenceCases): ThickCasesReply {
  try {
    const cases = build();
    return {
      kind: "built",
      cases: {
        "venus-92bar": fixtureCaseOf(cases["venus-92bar"]),
        "venus-58bar": fixtureCaseOf(cases["venus-58bar"]),
        "titan-class": fixtureCaseOf(cases["titan-class"]),
        "giant-deck": fixtureCaseOf(cases["giant-deck"]),
      },
    };
  } catch (error: unknown) {
    return { kind: "failed", message: error instanceof Error ? error.message : String(error) };
  }
}
