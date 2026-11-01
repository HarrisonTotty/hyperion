/**
 * The thick bake's router (plan R08, R08.T14.c, R08.T14.d.4 and R08.T15.b.2; Design note 9's
 * routing): a `thickScattering` world routed within the range its passing gates cover, a Titan-class
 * haze's opened by `titan-class`'s gate alone once its recorded verdict passes, a `cloudDeck`
 * world by its floor's range, none yet, a gas envelope's opened by `giant-deck`'s envelope variant
 * alone once its recorded verdict passes (R08.T16.a.2), and the polarisation correction asked of
 * every world with air once routed.
 */
import { describe, expect, it } from "vitest";

import { venusClassMedium } from "../../../test/atmosphereFixtures";
import { detailOf, recordInput } from "../../../test/atmosphereRecords";
import { handJupiterDetail } from "../../../test/giantFixtures";
import { assembleMedium } from "../assemble";
import { R05_EARTH_REFERENCE } from "../earth";
import * as cases from "../reference/cases";
import { giantDeckReferenceCase, titanClassReferenceCase } from "../reference/thickCases";
import committedGate from "../reference/vectorGate.json" with { type: "json" };
import type { AtmosphereMedium } from "../medium";
import { spectralColumnOf } from "./columnLayout";
import { upperMediumOf } from "./deckRead";
import {
  type AtmosphereRegime,
  CLEAR_COVER,
  classifyRegime,
  envelopeFloorM,
  regimeChannels,
} from "./regime";
import {
  bakedAsHaze,
  BELOW_THE_HORIZON_IMAGES,
  CORRECTION_ROUTED,
  correctionRouted,
  DECK_ROUTE_RANGE,
  deckGatePasses,
  deckRouted,
  type DeckRouteRange,
  deckRouteRangeOf,
  ENVELOPE_GATE_TALLIES,
  ENVELOPE_ROUTE_RANGE,
  type GateImageTally,
  HAZE_GATE_TALLIES,
  HAZE_ROUTE_RANGE,
  hazeGatePasses,
  THICK_ROUTE_RANGE,
  THICK_ROUTE_RANGES,
  thickBakeRouted,
  thickRouteRangesOf,
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

/** One channel as the router reads it. */
type Channel = ReturnType<typeof regimeChannels>[number];

/** Each bound of a {@link ThickRouteRange}, its side, and what it bounds of a channel. */
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

/**
 * How far `bound` lies outward of the extreme of `values` on its `side`, in steps of the extreme's
 * third figure, one value: in [0, 1) where it is the extreme rounded outward at its third figure,
 * and 0 for an extreme of 0, which no figure rounds.
 */
function outwardSteps(
  bound: number,
  values: ReadonlyArray<number>,
  side: "most" | "least",
): ReadonlyArray<number> {
  const extreme = side === "most" ? Math.max(...values) : Math.min(...values);
  const beyond = side === "most" ? bound - extreme : extreme - bound;
  return [extreme === 0 ? beyond : beyond / 10 ** (Math.floor(Math.log10(extreme)) - 2)];
}

/** The kept cases whose route passes R08.T14.d's vector gate, and the one whose does not. */
const PASSING = [cases.EARTH_CASE, cases.EARTH_OZONE_SLAB_CASE, cases.SATURN_OBLATE_CASE];

describe("thickBakeRouted (R08.T14.d.4)", () => {
  it("routes thickScattering alone, and only within the range, never cloudDeck nor thin", () => {
    const earth = cases.EARTH_CASE.medium;
    expect([
      REGIMES.map((regime) => thickBakeRouted(regime, earth)),
      REGIMES.map((regime) => thickBakeRouted(regime, earth, [])),
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

  it.each(BOUNDS)(
    "sets %s at the passing cases' extreme, rounded outward at its third figure",
    (key, side, read) => {
      expect(outwardSteps(THICK_ROUTE_RANGE[key], channels.map(read), side)).toEqual([
        expect.toSatisfy((steps: number) => steps >= 0 && steps < 1),
      ]);
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

describe("deckRouted (R08.T15.b.2)", () => {
  // R05's Earth's air, as a deck's medium: the route reads the medium's channels alone.
  const air = cases.EARTH_CASE.medium;
  const deck: AtmosphereRegime = { kind: "cloudDeck", deckAltitudeM: 5_000 };
  /** A range wide enough for any medium: a test's opening, never the product's. */
  const open: ThickRouteRange = {
    mostOpticalDepth: Number.POSITIVE_INFINITY,
    leastSingleScatteringAlbedo: 0,
    mostAsymmetry: 1,
    mostGroundAlbedo: 1,
    mostForwardDepth: Number.POSITIVE_INFINITY,
    mostAbsorbedDepth: Number.POSITIVE_INFINITY,
  };
  const both: DeckRouteRange = { ground: open, envelope: open };

  it("routes nothing in the product: no deck gate passes", () => {
    expect([
      DECK_ROUTE_RANGE,
      REGIMES.flatMap((regime) => [deckRouted(regime, air, false), deckRouted(regime, air, true)]),
    ]).toEqual([null, [false, false, false, false, false, false]]);
  });

  it("routes cloudDeck alone under a test's open range, never thin nor thickScattering", () => {
    expect(REGIMES.map((regime) => deckRouted(regime, air, false, both))).toEqual([
      false,
      false,
      true,
    ]);
  });

  it("routes a deck over a surface and an envelope each by its own floor's range", () => {
    const grounds: DeckRouteRange = { ground: open, envelope: null };
    const envelopes: DeckRouteRange = { ground: null, envelope: open };
    expect([
      [deckRouted(deck, air, false, grounds), deckRouted(deck, air, true, grounds)],
      [deckRouted(deck, air, false, envelopes), deckRouted(deck, air, true, envelopes)],
    ]).toEqual([
      [true, false],
      [false, true],
    ]);
  });

  it("routes no split at the ground, where no deck stands", () => {
    const atGround: AtmosphereRegime = { kind: "cloudDeck", deckAltitudeM: 0 };
    expect([deckRouted(atGround, air, false, both), deckRouted(atGround, air, true, both)]).toEqual(
      [false, false],
    );
  });

  it("routes a medium only within its floor's range", () => {
    const shallow: DeckRouteRange = { ground: THICK_ROUTE_RANGE, envelope: THICK_ROUTE_RANGE };
    const deeper = {
      ...air,
      terms: air.terms.map((term) => ({
        ...term,
        scattering: [
          term.scattering[0] * 4,
          term.scattering[1] * 4,
          term.scattering[2] * 4,
        ] as const,
      })),
    };
    expect([
      deckRouted(deck, air, false, shallow),
      deckRouted(deck, deeper, false, shallow),
    ]).toEqual([true, false]);
  });
});

/** `giant-deck`, split at its envelope floor as an envelope's route takes it. */
function giant(): { readonly medium: AtmosphereMedium; readonly regime: AtmosphereRegime } {
  const { medium } = giantDeckReferenceCase();
  const regime = classifyRegime(medium, { cloudFraction: 1, decks: [], envelope: true });
  return { medium, regime };
}

/** An image's tally of `passes`, `misses` and `refusals` among `image`'s recorded comparisons. */
function tallied(
  image: GateImageTally,
  passes: number,
  misses: number,
  refusals: number,
): GateImageTally {
  return { image: image.image, comparisons: image.comparisons, passes, misses, refusals };
}

describe("the envelope's gate (R08.T16.a.2)", () => {
  /** Every image of the variant passing, one value of each refused: a fixture verdict. */
  const passing: ReadonlyArray<GateImageTally> = ENVELOPE_GATE_TALLIES.map((image) =>
    tallied(image, image.comparisons - 1, 0, 1),
  );

  it("holds the recorded verdict to vectorGate.json's deck.asAnEnvelope, image by image", () => {
    expect(ENVELOPE_GATE_TALLIES).toEqual(
      committedGate.deck.asAnEnvelope.tallies.images.map(
        ([image, comparisons, passes, misses, refusals]) => ({
          image,
          comparisons,
          passes,
          misses,
          refusals,
        }),
      ),
    );
  });

  it("keeps the route closed on the recorded verdict: the disc misses at p75 and p85", () => {
    expect([
      deckGatePasses(ENVELOPE_GATE_TALLIES),
      deckRouteRangeOf(ENVELOPE_GATE_TALLIES),
    ]).toEqual([false, null]);
  });

  it("opens the envelope's route alone on a recorded pass, and routes giant-deck at its envelope floor", () => {
    const opened = deckRouteRangeOf(passing);
    const { medium, regime } = giant();
    expect([
      opened,
      regime.kind === "cloudDeck" ? regime.deckAltitudeM : null,
      deckRouted(regime, medium, true, opened),
      deckRouted(regime, medium, false, opened),
    ]).toEqual([
      { ground: null, envelope: ENVELOPE_ROUTE_RANGE },
      expect.closeTo(committedGate.deck.asAnEnvelope.split.altitudeM, 6),
      true,
      false,
    ]);
  }, 180_000);

  it("keeps it closed where an image misses, as the limb's 0.80–0.85 would, or is refused throughout", () => {
    const missing = passing.map((image) =>
      image.image === "limb" ? tallied(image, 13, 2, 0) : image,
    );
    const refused = passing.map((image) =>
      image.image === "disc-p85" ? tallied(image, 0, 0, 3) : image,
    );
    expect([deckGatePasses(missing), deckGatePasses(refused), deckGatePasses([])]).toEqual([
      false,
      false,
      false,
    ]);
  });

  it.each(BOUNDS)(
    "sets the envelope's %s at giant-deck's extreme above its floor, rounded outward at its third figure",
    (key, side, read) => {
      const { medium } = giant();
      const channels = regimeChannels(upperMediumOf(medium, envelopeFloorM(medium)));
      expect(outwardSteps(ENVELOPE_ROUTE_RANGE[key], channels.map(read), side)).toEqual([
        expect.toSatisfy((steps: number) => steps >= 0 && steps < 1),
      ]);
    },
    180_000,
  );

  it("leaves the hand Jupiter out of the envelope's range: its chromophore absorbs", () => {
    const assembled = assembleMedium(recordInput(detailOf(handJupiterDetail())), {
      envelopes: true,
    });
    if (assembled.medium === undefined) {
      throw new Error("the hand Jupiter draws a medium");
    }
    const open: DeckRouteRange = { ground: null, envelope: ENVELOPE_ROUTE_RANGE };
    const upper = upperMediumOf(
      assembled.medium,
      assembled.regime.kind === "cloudDeck" ? assembled.regime.deckAltitudeM : Number.NaN,
    );
    expect([
      assembled.regime.kind,
      deckRouted(assembled.regime, assembled.medium, true, open),
      regimeChannels(upper).map((c) => c.opticalDepth * (1 - c.singleScatteringAlbedo) > 0.05),
    ]).toEqual(["cloudDeck", false, [true, true, true]]);
  }, 180_000);
});

/** `titan-class`, and the regime it routes under a clear sky, how its gate reads it. */
function titan(): { readonly medium: AtmosphereMedium; readonly regime: AtmosphereRegime } {
  const { medium } = titanClassReferenceCase();
  return { medium, regime: classifyRegime(medium, CLEAR_COVER) };
}

describe("the Titan-class haze's gate (R08.T14.d.4)", () => {
  /**
   * Every image of the gate above the horizon passing, one value of each refused, and those below
   * it refused throughout: a fixture verdict.
   */
  const passing: ReadonlyArray<GateImageTally> = HAZE_GATE_TALLIES.map((image) =>
    BELOW_THE_HORIZON_IMAGES.includes(image.image)
      ? tallied(image, 0, 0, image.comparisons)
      : tallied(image, image.comparisons - 1, 0, 1),
  );

  it("holds the recorded verdict to vectorGate.json's titanClass, image by image", () => {
    expect(HAZE_GATE_TALLIES).toEqual(
      committedGate.titanClass.routed.images.map(
        ([image, comparisons, passes, misses, refusals]) => ({
          image,
          comparisons,
          passes,
          misses,
          refusals,
        }),
      ),
    );
  });

  it("keeps the haze's route closed on the recorded verdict: titan-class misses at the horizon, the limb and in daylight", () => {
    expect([hazeGatePasses(HAZE_GATE_TALLIES), THICK_ROUTE_RANGES]).toEqual([
      false,
      [THICK_ROUTE_RANGE],
    ]);
  });

  it("routes titan-class if and only if its gate passes", () => {
    const { medium, regime } = titan();
    expect([
      regime.kind,
      thickBakeRouted(regime, medium),
      thickBakeRouted(regime, medium, thickRouteRangesOf(HAZE_GATE_TALLIES)),
      thickRouteRangesOf(passing),
      thickBakeRouted(regime, medium, thickRouteRangesOf(passing)),
    ]).toEqual(["thickScattering", false, false, [THICK_ROUTE_RANGE, HAZE_ROUTE_RANGE], true]);
  }, 180_000);

  it("never routes a Mars- or Venus-class medium, and routes the Earth- and Saturn-class cases as before, on either verdict", () => {
    const venus = venusClassMedium().medium;
    const routedOn = (ranges: ReadonlyArray<ThickRouteRange>): ReadonlyArray<unknown> => [
      ...[...PASSING, cases.MARS_CASE].map((each) =>
        thickBakeRouted(classifyRegime(each.medium, CLEAR_COVER), each.medium, ranges),
      ),
      thickBakeRouted(classifyRegime(venus, CLEAR_COVER), venus, ranges),
      thickBakeRouted({ kind: "cloudDeck", deckAltitudeM: 70_000 }, venus, ranges),
    ];
    expect([
      classifyRegime(venus, CLEAR_COVER).kind,
      routedOn(THICK_ROUTE_RANGES),
      routedOn(thickRouteRangesOf(passing)),
    ]).toEqual([
      "thickScattering",
      [true, true, true, false, false, false],
      [true, true, true, false, false, false],
    ]);
  });

  it("bakes a haze-routed world as a haze route's bake is, and no world the kept cases' range routes", () => {
    const { medium, regime } = titan();
    const opened = thickRouteRangesOf(passing);
    const earth = cases.EARTH_CASE.medium;
    const earthRegime = classifyRegime(earth, CLEAR_COVER);
    expect([
      bakedAsHaze(regime, medium),
      bakedAsHaze(regime, medium, opened),
      bakedAsHaze(earthRegime, earth, opened),
      bakedAsHaze({ kind: "thin" }, medium, opened),
    ]).toEqual([false, true, false, false]);
  }, 180_000);

  it("judges no image whose sun stands below the horizon, and closes on a miss at the horizon or an image refused throughout", () => {
    const twilit = passing.map((image) =>
      image.image === "ground-s95" ? tallied(image, 0, 3, 60) : image,
    );
    const horizon = passing.map((image) =>
      image.image === "ground-s90" ? tallied(image, 62, 1, 0) : image,
    );
    const refused = passing.map((image) =>
      image.image === "limb" ? tallied(image, 0, 0, 15) : image,
    );
    expect([
      BELOW_THE_HORIZON_IMAGES,
      hazeGatePasses(passing),
      hazeGatePasses(twilit),
      hazeGatePasses(horizon),
      hazeGatePasses(refused),
      hazeGatePasses([]),
    ]).toEqual([["ground-s95"], true, true, false, false, false]);
  });

  it.each(BOUNDS)(
    "sets the haze's %s at titan-class's extreme, rounded outward at its third figure",
    (key, side, read) => {
      const channels = regimeChannels(titan().medium);
      expect(outwardSteps(HAZE_ROUTE_RANGE[key], channels.map(read), side)).toEqual([
        expect.toSatisfy((steps: number) => steps >= 0 && steps < 1),
      ]);
    },
    180_000,
  );
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
