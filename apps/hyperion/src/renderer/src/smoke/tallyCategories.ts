/**
 * The memory categories the shared-pass check reads its allocation tally by (`sharedPass.ts`, plan
 * R10, R10.T16.b), in a module of its own, which a unit test can load without the smoke page.
 */

import type { MemoryCategory } from "../view/engine/memory";

/**
 * Every memory category, for the tally's bytes: `tallyCategories.test.ts` holds it equal to
 * {@link MemoryCategory}, so that a category a later plan adds is not left out of the tally, as
 * `clouds` (plan R11, R11.T7.b) was.
 */
export const CATEGORIES: ReadonlyArray<MemoryCategory> = [
  "render-targets",
  "other",
  "atmosphere-tables",
  "atmosphere-view",
  "clouds",
  "coarse-field-gpu",
  "decoration",
  "geometry-clusters",
  "geometry-pages",
  "height-cache",
  "ocean",
  "shadows",
  "sky-cube",
  "sky-scratch",
];
