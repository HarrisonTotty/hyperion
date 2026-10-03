/**
 * The test planet's figure (plan R05, Design notes 5 and 12): WGS 84's reference ellipsoid, the
 * datum the test planet's heights are measured from.
 */

import type { BodyFigure } from "../terrain/planet";

/**
 * WGS 84's figure (NIMA TR8350.2, Table 3.1): a = 6,378,137 m, f = 1 ÷ 298.257223563, so
 * c = a (1 − f); the pole along the body-fixed z axis is unknown in any wider frame here.
 */
export const TEST_PLANET_FIGURE: BodyFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257_223_563),
  pole: null,
};
