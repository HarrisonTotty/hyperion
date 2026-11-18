import { describe, expect, it } from "vitest";

import type { MemoryCategory } from "../view/engine/memory";
import { CATEGORIES } from "./tallyCategories";

/**
 * Every {@link MemoryCategory}, as a record's keys, which the type check holds to the union: a
 * category left out of it, or one not in it, fails the check.
 */
const EVERY_CATEGORY: Readonly<Record<MemoryCategory, true>> = {
  "render-targets": true,
  other: true,
  "atmosphere-tables": true,
  "atmosphere-view": true,
  clouds: true,
  "coarse-field-gpu": true,
  decoration: true,
  "geometry-clusters": true,
  "geometry-pages": true,
  "height-cache": true,
  ocean: true,
  rings: true,
  shadows: true,
  "sky-cube": true,
  "sky-scratch": true,
};

describe("the shared pass's allocation tally", () => {
  it("reads every memory category, each once", () => {
    expect(new Set(CATEGORIES)).toEqual(new Set(Object.keys(EVERY_CATEGORY)));
    expect(CATEGORIES).toHaveLength(Object.keys(EVERY_CATEGORY).length);
  });

  it("reads the clouds' bytes, which part 21 missed before (R11.T7.b)", () => {
    expect(CATEGORIES).toContain("clouds");
  });
});
