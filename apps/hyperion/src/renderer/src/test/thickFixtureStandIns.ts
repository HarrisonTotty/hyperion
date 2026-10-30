/**
 * Stand-ins for `ATMOSPHERE TEST`'s thick cases (plan R08, R08.T12.b.2), for tests of the scene's
 * loads that need not pay the cases' minute of Mie: each named as its case and on its sphere, each
 * carrying a medium of its own made from `earth`'s.
 */

import { EARTH_CASE } from "../view/atmosphere/reference/cases";
import {
  GIANT_RADIUS_M,
  TITAN_RADIUS_M,
  VENUS_RADIUS_M,
} from "../view/atmosphere/reference/thickCases";
import type {
  FixtureCase,
  ThickFixtureCases,
} from "../view/atmosphere/reference/thickCasesMessages";

/** A stand-in case named `name` on a sphere of `radiusM`, with a medium object of its own. */
function standIn(name: string, radiusM: number): FixtureCase {
  return {
    name,
    medium: { ...EARTH_CASE.medium },
    figure: { kind: "sphere", radiusM },
    curves: EARTH_CASE.curves,
  };
}

/** The four thick cases' stand-ins, by case, made anew at each call. */
export function standInThickCases(): ThickFixtureCases {
  return {
    "venus-92bar": standIn("venus-92bar", VENUS_RADIUS_M),
    "venus-58bar": standIn("venus-58bar", VENUS_RADIUS_M),
    "titan-class": standIn("titan-class", TITAN_RADIUS_M),
    "giant-deck": standIn("giant-deck", GIANT_RADIUS_M),
  };
}
