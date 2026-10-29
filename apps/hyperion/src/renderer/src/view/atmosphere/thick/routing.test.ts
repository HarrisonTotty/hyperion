/**
 * The thick bake's router (plan R08, R08.T14.c; Design note 9's routing): no regime routed until
 * its gate passes, and only `thickScattering` routed by this route once it has.
 */
import { describe, expect, it } from "vitest";

import { R05_EARTH_REFERENCE } from "../earth";
import { spectralColumnOf } from "./columnLayout";
import type { AtmosphereRegime } from "./regime";
import { CORRECTION_ROUTED, correctionRouted, GATED_REGIMES, thickBakeRouted } from "./routing";
import { channelSource } from "./spectralColumn";

/** Every regime the router names. */
const REGIMES: ReadonlyArray<AtmosphereRegime> = [
  { kind: "thin" },
  { kind: "thickScattering" },
  { kind: "cloudDeck", deckAltitudeM: 5_000 },
];

describe("thickBakeRouted", () => {
  it("routes no regime before R08.T14.d's gates pass", () => {
    expect([[...GATED_REGIMES], REGIMES.map((regime) => thickBakeRouted(regime))]).toEqual([
      [],
      [false, false, false],
    ]);
  });

  it("routes thickScattering alone once its gate has passed", () => {
    const gated = new Set<AtmosphereRegime["kind"]>(["thickScattering"]);
    expect(REGIMES.map((regime) => thickBakeRouted(regime, gated))).toEqual([false, true, false]);
  });

  it("never routes cloudDeck, whose bake is R08.T15's, nor thin, whatever is gated", () => {
    const gated = new Set<AtmosphereRegime["kind"]>(["thin", "thickScattering", "cloudDeck"]);
    expect(REGIMES.map((regime) => thickBakeRouted(regime, gated))).toEqual([false, true, false]);
  });
});

describe("correctionRouted (R08.T14.f)", () => {
  const air = spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource));

  it("asks no correction in the product until main turns the route on", () => {
    expect([CORRECTION_ROUTED, correctionRouted(air)]).toEqual([false, false]);
  });

  it("asks a world with air its correction once routed, and an airless column none", () => {
    expect([correctionRouted(air, true), correctionRouted({ ...air, terms: [] }, true)]).toEqual([
      true,
      false,
    ]);
  });
});
