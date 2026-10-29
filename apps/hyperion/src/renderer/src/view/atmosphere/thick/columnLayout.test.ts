/**
 * The spectral column's layout's predicate of the polarisation correction (plan R08, R08.T14.f):
 * whether a column needs it, by its sources alone.
 */
import { describe, expect, it } from "vitest";

import { R05_EARTH_REFERENCE } from "../earth";
import { MARS_CASE } from "../reference/cases";
import { correctionBySources, spectralColumnOf } from "./columnLayout";
import { keptColumnOf } from "./keptBake";
import { channelSource } from "./spectralColumn";

describe("correctionBySources (R08.T14.f)", () => {
  const air = spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource));

  it("reads a world with air as needing the correction, by its molecular term or a Rayleigh channel source", () => {
    const mars = keptColumnOf(MARS_CASE);
    expect([correctionBySources(air), correctionBySources(mars)]).toEqual([true, true]);
  });

  it("reads a column with no term as needing none", () => {
    expect(correctionBySources({ ...air, terms: [] })).toBe(false);
  });

  it("leaves a column of aerosols alone to their optics, which the render thread does not load", () => {
    const mars = keptColumnOf(MARS_CASE);
    const dust = mars.terms.filter((term) => term.source.kind === "aerosol");
    expect(dust.length).toBeGreaterThan(0);
    expect(correctionBySources({ ...mars, terms: dust })).toBeNull();
  });

  it("reads a Cornette–Shanks channel source alone as a depolariser, needing none", () => {
    const haze = air.terms.filter(
      (term) => term.source.kind === "channels" && term.source.phase.kind === "cornette-shanks",
    );
    expect(haze.length).toBeGreaterThan(0);
    expect(correctionBySources({ ...air, terms: haze })).toBe(false);
  });
});
