/**
 * The thick bake's router (plan R08, R08.T14.c and R08.T14.d.4; Design note 9's routing): a
 * `thickScattering` world routed within the range its passing gates cover, and the polarisation
 * correction asked of every world with air once routed.
 */
import { describe, expect, it } from "vitest";

import { R05_EARTH_REFERENCE } from "../earth";
import * as cases from "../reference/cases";
import { spectralColumnOf } from "./columnLayout";
import { type AtmosphereRegime, CLEAR_COVER, classifyRegime, regimeChannels } from "./regime";
import {
  CORRECTION_ROUTED,
  correctionRouted,
  THICK_ROUTE_RANGE,
  thickBakeRouted,
  type ThickRouteRange,
  withinRouteRange,
} from "./routing";
import { channelSource } from "./spectralColumn";

/** Every regime the router names. */
const REGIMES: ReadonlyArray<AtmosphereRegime> = [
  { kind: "thin" },
  { kind: "thickScattering" },
  { kind: "cloudDeck", deckAltitudeM: 5_000 },
];

/** The kept cases whose route passes R08.T14.d's vector gate, and the one whose does not. */
const PASSING = [cases.EARTH_CASE, cases.EARTH_OZONE_SLAB_CASE, cases.SATURN_OBLATE_CASE];

describe("thickBakeRouted (R08.T14.d.4)", () => {
  it("routes thickScattering alone, and only within the range, never cloudDeck nor thin", () => {
    const earth = cases.EARTH_CASE.medium;
    expect([
      REGIMES.map((regime) => thickBakeRouted(regime, earth)),
      REGIMES.map((regime) => thickBakeRouted(regime, earth, null)),
    ]).toEqual([
      [false, true, false],
      [false, false, false],
    ]);
  });

  it("routes the passing kept cases and leaves Mars's dust out, as their gates do", () => {
    const routed = [...PASSING, cases.MARS_CASE].map((each) => [
      each.name,
      classifyRegime(each.medium, CLEAR_COVER).kind,
      thickBakeRouted(classifyRegime(each.medium, CLEAR_COVER), each.medium),
    ]);
    expect(routed).toEqual([
      ["earth", "thickScattering", true],
      ["earth-ozone-slab", "thickScattering", true],
      ["saturn-oblate", "thickScattering", true],
      ["mars", "thickScattering", false],
    ]);
  });

  it("holds every passing case within the range", () => {
    expect(PASSING.map((each) => withinRouteRange(each.medium, THICK_ROUTE_RANGE))).toEqual([
      true,
      true,
      true,
    ]);
  });

  const channels = PASSING.flatMap((each) => regimeChannels(each.medium));
  type Channel = (typeof channels)[number];
  /** Each bound, its side, and what it bounds of a channel. */
  const BOUNDS: ReadonlyArray<
    readonly [keyof ThickRouteRange, "most" | "least", (c: Channel) => number]
  > = [
    ["mostOpticalDepth", "most", (c) => c.opticalDepth],
    ["leastSingleScatteringAlbedo", "least", (c) => c.singleScatteringAlbedo],
    ["mostAsymmetry", "most", (c) => c.asymmetry],
    ["mostGroundAlbedo", "most", (c) => c.groundAlbedo],
    ["mostForwardDepth", "most", (c) => c.opticalDepth * c.asymmetry],
    ["mostAbsorbedDepth", "most", (c) => c.opticalDepth * (1 - c.singleScatteringAlbedo)],
  ];

  it.each(BOUNDS)(
    "sets %s at the passing cases' extreme, rounded outward at its third figure",
    (key, side, read) => {
      const values = channels.map(read);
      const extreme = side === "most" ? Math.max(...values) : Math.min(...values);
      const bound = THICK_ROUTE_RANGE[key];
      const figure = 10 ** (Math.floor(Math.log10(extreme)) - 2);
      expect((side === "most" ? bound - extreme : extreme - bound) / figure).toBeGreaterThanOrEqual(
        0,
      );
      expect((side === "most" ? bound - extreme : extreme - bound) / figure).toBeLessThan(1);
    },
  );

  it("leaves Mars's dust out by its forward peak", () => {
    const mars = regimeChannels(cases.MARS_CASE.medium);
    expect(
      mars.map(
        (c) =>
          c.asymmetry > THICK_ROUTE_RANGE.mostAsymmetry &&
          c.opticalDepth * c.asymmetry > THICK_ROUTE_RANGE.mostForwardDepth,
      ),
    ).toEqual([true, true, true]);
  });

  it("leaves a deeper column out of the range, as the Venus- and Titan-class cases' gates pass no depth", () => {
    const earth = cases.EARTH_CASE.medium;
    const deeper = {
      ...earth,
      terms: earth.terms.map((term) => ({
        ...term,
        scattering: [
          term.scattering[0] * 4,
          term.scattering[1] * 4,
          term.scattering[2] * 4,
        ] as const,
      })),
    };
    expect([
      withinRouteRange(earth, THICK_ROUTE_RANGE),
      withinRouteRange(deeper, THICK_ROUTE_RANGE),
    ]).toEqual([true, false]);
  });
});

describe("correctionRouted (R08.T14.f, R08.T14.d.4)", () => {
  const air = spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource));

  it("asks every world with air its correction in the product since the route", () => {
    expect([CORRECTION_ROUTED, correctionRouted(air)]).toEqual([true, true]);
  });

  it("asks an airless column none, and none where the route is off", () => {
    expect([correctionRouted({ ...air, terms: [] }), correctionRouted(air, false)]).toEqual([
      false,
      false,
    ]);
  });
});
