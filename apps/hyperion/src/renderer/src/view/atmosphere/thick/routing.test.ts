/**
 * The thick bake's router (plan R08, R08.T14.c, R08.T14.d.4 and R08.T15.b.2; Design note 9's
 * routing): a `thickScattering` world routed within the range its passing gates cover, a Titan-class
 * haze's opened by `titan-class`'s gate alone, its verdict passing with its named exceptions, a
 * `cloudDeck` world by its floor's range, a deck over a surface's opened by the Venus-class cases'
 * gates and a gas envelope's by `giant-deck`'s envelope variant once its recorded verdict passes,
 * widened to the cool giants' on the `envelope` giant route once `jupiter-envelope`'s passes with its
 * named exceptions (R08.T16.a.2), and the polarisation correction asked of every world with air once
 * routed.
 */
import { decodeServerMessage, type ResponseFor, type SystemIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { venusClassMedium } from "../../../test/atmosphereFixtures";
import { type DetailAnswer, detailOf, recordInput } from "../../../test/atmosphereRecords";
import {
  handIceGiantDetail,
  handJupiterDetail,
  handSaturnDetail,
} from "../../../test/giantFixtures";
import { jupiterEnvelopeCase } from "../../../test/jupiterEnvelopeCase";
import { hotJupiterDetail, jupiterDetail } from "../../../test/planetaryFixture";
import { assembleMedium } from "../assemble";
import { R05_EARTH_REFERENCE } from "../earth";
import servedGiants from "../fixtures/served-giants.json" with { type: "json" };
import * as cases from "../reference/cases";
import {
  giantDeckReferenceCase,
  thickReferenceCases,
  titanClassReferenceCase,
  venusDeckCover,
} from "../reference/thickCases";
import committedGate from "../reference/vectorGate.json" with { type: "json" };
import type { AtmosphereMedium } from "../medium";
import { HORIZON_SUN_COSINE } from "./bake";
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
  ABOVE_DECK_GATE_TALLIES,
  bakedAsHaze,
  channelWithinRouteRange,
  CHROMOPHORE_GATE_EXCEPTIONS,
  CHROMOPHORE_GATE_MISSES,
  CHROMOPHORE_GATE_TALLIES,
  chromophoreGatePasses,
  COOL_GIANT_DECK_ROUTE_RANGE,
  COOL_GIANT_ROUTE_RANGE,
  coolGiantRouteRangeOf,
  CORRECTION_ROUTED,
  correctionRouted,
  DECK_LOWEST_SUN_SINE,
  DECK_ROUTE_RANGE,
  deckGatePasses,
  deckRouted,
  type DeckRouteRange,
  deckRouteRangeOf,
  ENVELOPE_GATE_TALLIES,
  ENVELOPE_ROUTE_RANGE,
  type GateException,
  type GateImageTally,
  type GateMiss,
  GIANT_ROUTE,
  giantDeckRouteRange,
  HAZE_GATE_EXCEPTIONS,
  HAZE_GATE_MISSES,
  HAZE_GATE_TALLIES,
  HAZE_ROUTE_RANGE,
  HAZE_UNJUDGED_IMAGES,
  hazeGatePasses,
  LOW_SUN_SINE,
  SURFACE_DECK_GATE_TALLIES,
  SURFACE_DECK_ROUTE_RANGE,
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

/** What a range bounds of a channel. */
type ChannelReading = Pick<
  Channel,
  "opticalDepth" | "singleScatteringAlbedo" | "asymmetry" | "groundAlbedo"
>;

/** Each bound of a {@link ThickRouteRange}, its side, and what it bounds of a channel. */
const BOUNDS: ReadonlyArray<
  readonly [keyof ThickRouteRange, "most" | "least", (c: ChannelReading) => number]
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

  it("leaves a deeper column out of the kept cases' range, which no Venus- or Titan-class depth reaches", () => {
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

  it("routes a deck over a surface in the product within the Venus-class cases' range and an envelope within giant-deck's, Earth's air within neither", () => {
    expect([
      DECK_ROUTE_RANGE,
      REGIMES.flatMap((regime) => [deckRouted(regime, air, false), deckRouted(regime, air, true)]),
    ]).toEqual([
      { ground: SURFACE_DECK_ROUTE_RANGE, envelope: ENVELOPE_ROUTE_RANGE },
      [false, false, false, false, false, false],
    ]);
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

  it("opens the route on the recorded verdict, every image passing (R08.T16.a.2's floor placement)", () => {
    expect([
      deckGatePasses(ENVELOPE_GATE_TALLIES),
      deckRouteRangeOf(ENVELOPE_GATE_TALLIES, []),
      DECK_ROUTE_RANGE?.envelope,
    ]).toEqual([true, { ground: null, envelope: ENVELOPE_ROUTE_RANGE }, ENVELOPE_ROUTE_RANGE]);
  });

  it("opens the envelope's route alone on a recorded pass, and routes giant-deck at its envelope floor", () => {
    const opened = deckRouteRangeOf(passing, []);
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

  it("keeps it closed where an image misses, as the limb's 0.80–0.85 would, or every image is refused, and passes over one image refused throughout (Titan's rule, R08.T15.b.2)", () => {
    const missing = passing.map((image) =>
      image.image === "limb" ? tallied(image, 13, 2, 0) : image,
    );
    const refused = passing.map((image) =>
      image.image === "disc-p85" ? tallied(image, 0, 0, 3) : image,
    );
    const allRefused = passing.map((image) => tallied(image, 0, 0, image.comparisons));
    const refusedMissing = missing.map((image) =>
      image.image === "disc-p85" ? tallied(image, 0, 0, 3) : image,
    );
    expect([
      deckGatePasses(missing),
      deckGatePasses(refused),
      deckGatePasses(allRefused),
      deckGatePasses(refusedMissing),
      deckGatePasses([]),
    ]).toEqual([false, true, false, false, false]);
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

  // Its NH₃ haze and chromophore are 30's (`test/giantFixtures.ts`'s `coldGiantHazes`: τ(550) 7 and
  // 1.53 × 10⁻⁴ kg m⁻²). Measured 2026-11-05: τ (1 − ω̄) 0.016, 0.041 and 0.119 in red, green and
  // blue above its floor (27's: 0.025–0.187; 26's: 0.065–0.49), each over the range's 0.
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
      regimeChannels(upper).map(
        (c) => Math.round(c.opticalDepth * (1 - c.singleScatteringAlbedo) * 1000) / 1000,
      ),
    ]).toEqual(["cloudDeck", false, [0.016, 0.041, 0.119]]);
    expect(ENVELOPE_ROUTE_RANGE.mostAbsorbedDepth).toBe(0);
  }, 180_000);
});

/** The Venus-class cases, and the regime each routes under its deck's cover, how its gate reads it. */
function venusCases(): ReadonlyArray<{
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly regime: AtmosphereRegime;
}> {
  const built = thickReferenceCases();
  return (["venus-92bar", "venus-58bar"] as const).map((name) => {
    const { medium } = built[name];
    return { name, medium, regime: classifyRegime(medium, venusDeckCover(medium)) };
  });
}

/** An image's tallies as `vectorGate.json` records them: `[image, comparisons, passes, misses, refusals, …]`. */
function talliesOf(images: ReadonlyArray<ReadonlyArray<unknown>>): ReadonlyArray<GateImageTally> {
  return images.map(([image, comparisons, passes, misses, refusals]) => ({
    image: String(image),
    comparisons: Number(comparisons),
    passes: Number(passes),
    misses: Number(misses),
    refusals: Number(refusals),
  }));
}

describe("the Venus-class cases' deck gate (R08.T15.b.2)", () => {
  const recorded = committedGate.deck.venusClass.cases;

  it("holds the recorded verdicts to vectorGate.json's deck.venusClass, case by case and image by image", () => {
    expect(SURFACE_DECK_GATE_TALLIES).toEqual({
      "venus-92bar": talliesOf(recorded["venus-92bar"].routed.images),
      "venus-58bar": talliesOf(recorded["venus-58bar"].routed.images),
    });
  });

  it("holds giant-deck's verdict above its split to vectorGate.json's deck.routed, image by image", () => {
    expect(ABOVE_DECK_GATE_TALLIES).toEqual(talliesOf(committedGate.deck.routed.images));
  });

  it("keeps a deck over a surface closed where the gate above a deck misses, both of R08.T15's gates opening it", () => {
    const missing = ABOVE_DECK_GATE_TALLIES.map((image) =>
      image.image === "limb" ? tallied(image, 14, 1, 0) : image,
    );
    const venus = Object.values(SURFACE_DECK_GATE_TALLIES);
    expect([
      deckRouteRangeOf(ENVELOPE_GATE_TALLIES, [ABOVE_DECK_GATE_TALLIES, ...venus])?.ground,
      deckRouteRangeOf(ENVELOPE_GATE_TALLIES, [missing, ...venus])?.ground,
    ]).toEqual([SURFACE_DECK_ROUTE_RANGE, null]);
  });

  it("opens a deck over a surface on both verdicts, passing over the images each refuses throughout", () => {
    const refusedThroughout = Object.values(SURFACE_DECK_GATE_TALLIES).map((tallies) =>
      tallies.filter((image) => image.refusals === image.comparisons).map((image) => image.image),
    );
    const wholly = [
      "ground-s0",
      "ground-s30",
      "ground-s60",
      "ground-s80",
      "disc-p0",
      "disc-p30",
      "disc-p60",
      "disc-p75",
      "disc-p85",
      "aerial",
    ];
    expect([
      Object.values(SURFACE_DECK_GATE_TALLIES).map(deckGatePasses),
      refusedThroughout,
      DECK_ROUTE_RANGE?.ground,
    ]).toEqual([[true, true], [wholly, wholly], SURFACE_DECK_ROUTE_RANGE]);
  });

  it("routes venus-92bar and venus-58bar through their deck over the ground, and neither where either case's gate misses", () => {
    const [first, second] = Object.values(SURFACE_DECK_GATE_TALLIES);
    const missing = (first ?? []).map((image) =>
      image.image === "limb" ? tallied(image, image.passes - 1, 1, image.refusals) : image,
    );
    const closed = deckRouteRangeOf(ENVELOPE_GATE_TALLIES, [missing, second ?? []]);
    expect(
      venusCases().map(({ name, medium, regime }) => [
        name,
        regime.kind,
        deckRouted(regime, medium, false),
        deckRouted(regime, medium, true),
        deckRouted(regime, medium, false, closed),
      ]),
    ).toEqual([
      ["venus-92bar", "cloudDeck", true, false, false],
      ["venus-58bar", "cloudDeck", true, false, false],
    ]);
  }, 180_000);

  it("leaves giant-deck below its split, whose recorded s90 misses, and titan-class's haze out of the range", () => {
    const below = talliesOf(committedGate.deck.routedBelow.routed.images);
    expect([
      deckGatePasses(below),
      below.find((image) => image.image === "ground-s90")?.misses,
      withinRouteRange(giantDeckReferenceCase().medium, SURFACE_DECK_ROUTE_RANGE),
      withinRouteRange(titan().medium, SURFACE_DECK_ROUTE_RANGE),
    ]).toEqual([false, 42, false, false]);
  }, 180_000);

  it.each(BOUNDS)(
    "sets the surface deck's %s at the Venus-class cases' extreme, rounded outward at its third figure",
    (key, side, read) => {
      const channels = venusCases().flatMap(({ medium }) => regimeChannels(medium));
      expect(outwardSteps(SURFACE_DECK_ROUTE_RANGE[key], channels.map(read), side)).toEqual([
        expect.toSatisfy((steps: number) => steps >= 0 && steps < 1),
      ]);
    },
    180_000,
  );

  it("labels a low sun beneath the deck from giant-deck's lowest passing sun, s80, 10° up", () => {
    const below = talliesOf(committedGate.deck.routedBelow.routed.images);
    expect([
      DECK_LOWEST_SUN_SINE,
      below.find((image) => image.image === "ground-s80")?.passes,
    ]).toEqual([expect.closeTo(Math.cos((80 * Math.PI) / 180), 15), 63]);
  });
});

describe("the absorbing envelope's gate (R08.T16.a.2)", () => {
  it("holds the recorded verdict to vectorGate.json's jupiterEnvelope, image by image", () => {
    expect(CHROMOPHORE_GATE_TALLIES).toEqual(
      committedGate.jupiterEnvelope.tallies.images.map(
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

  // Retraced on 30's NH₃ haze and chromophore it passes by the rule on its 2 judged values of 24,
  // 22 refused: its widening is not built, and the range stays giant-deck's.
  it("leaves the envelope's range giant-deck's alone, jupiter-envelope's gate passing on 2 values of 24", () => {
    const judged = CHROMOPHORE_GATE_TALLIES.reduce(
      (sum, image) => sum + image.comparisons - image.refusals,
      0,
    );
    expect([deckGatePasses(CHROMOPHORE_GATE_TALLIES), judged, DECK_ROUTE_RANGE?.envelope]).toEqual([
      true,
      2,
      ENVELOPE_ROUTE_RANGE,
    ]);
  });

  // Each channel's τ, τ (1 − ω̄) and ḡ above the giant's floor, each within the range or not.
  it.each([
    [
      "the protocol fixture's generated Jupiter, by its depth and its chromophore",
      jupiterDetail,
      [
        [true, false, false],
        [true, false, false],
        [false, false, true],
      ],
    ],
    [
      "the hand Saturn, by its blue τ and its chromophore",
      handSaturnDetail,
      [
        [true, false, true],
        [true, false, true],
        [false, false, true],
      ],
    ],
    [
      "the hand ice giant, by its blue τ and its red ḡ",
      handIceGiantDetail,
      [
        [true, true, false],
        [true, true, true],
        [false, true, true],
      ],
    ],
  ] as const)(
    "leaves %s out of the envelope's range",
    (_, detail, within) => {
      const assembled = assembleMedium(recordInput(detailOf(detail())), { envelopes: true });
      if (assembled.medium === undefined || assembled.regime.kind !== "cloudDeck") {
        throw new Error("the giant draws an envelope's medium, split at its floor");
      }
      const range = ENVELOPE_ROUTE_RANGE;
      const channels = regimeChannels(
        upperMediumOf(assembled.medium, assembled.regime.deckAltitudeM),
      );
      expect([
        deckRouted(assembled.regime, assembled.medium, true),
        channels.map((c) => [
          c.opticalDepth <= range.mostOpticalDepth,
          c.opticalDepth * (1 - c.singleScatteringAlbedo) <= range.mostAbsorbedDepth,
          c.asymmetry <= range.mostAsymmetry,
        ]),
      ]).toEqual([false, within]);
    },
    180_000,
  );
});

/** Whether a value on file is a giant's channels, [τ, ω̄, ḡ] a channel. */
function isChannels(value: unknown): value is ReadonlyArray<ReadonlyArray<number>> {
  return (
    Array.isArray(value) &&
    value.every(
      (channel) => Array.isArray(channel) && channel.every((figure) => typeof figure === "number"),
    )
  );
}

/** The `Dev Fixture`'s giants on file (`fixtures/served-giants.json`), as the router reads them. */
const SERVED = servedGiants.giants.map(([id, selfLuminous, channels]) => {
  if (!isChannels(channels)) {
    throw new Error(`the served giant ${String(id)}'s channels are on file as [tau, omega, g]`);
  }
  return {
    id: String(id),
    selfLuminous: selfLuminous === true,
    channels: channels.map(([tau, omega, g]) => ({
      opticalDepth: tau ?? Number.NaN,
      singleScatteringAlbedo: omega ?? Number.NaN,
      asymmetry: g ?? Number.NaN,
      groundAlbedo: 0,
    })),
  };
});

/**
 * The served giants on file the envelope route routes, as `giantDeckRouteRange` reads each one's
 * regime: neither self-luminous nor out of its range.
 */
const SERVED_COOL = SERVED.filter((each) => {
  const range = giantDeckRouteRange(
    { kind: "cloudDeck", deckAltitudeM: 1, ...(each.selfLuminous ? { selfLuminous: true } : {}) },
    "envelope",
  )?.envelope;
  return (
    range !== null &&
    range !== undefined &&
    each.channels.every((c) => channelWithinRouteRange(c, range))
  );
});

/** The served cool giant on file, `41ffecb1ffc00013.0400`, as its frame decodes. */
function servedCoolGiant(): ResponseFor<"body_detail"> {
  const message = decodeServerMessage(JSON.stringify(servedGiants.frame));
  if (message.type !== "response" || message.body.kind !== "body_detail") {
    throw new Error("the served giant's frame is a body_detail response");
  }
  return message.body;
}

/** An answer's record assembled as the optics worker assembles it, its regime and medium. */
function assembledGiant(
  answer: DetailAnswer,
  system?: SystemIdHex,
): {
  readonly medium: AtmosphereMedium;
  readonly regime: AtmosphereRegime;
} {
  const detail = system === undefined ? detailOf(answer) : detailOf(answer, system);
  const assembled = assembleMedium(recordInput(detail), { envelopes: true });
  if (assembled.medium === undefined || assembled.regime.kind !== "cloudDeck") {
    throw new Error("the giant draws an envelope's medium, split at its floor");
  }
  return { medium: assembled.medium, regime: assembled.regime };
}

/** A split envelope's channels above its floor. */
function channelsAbove(split: {
  readonly medium: AtmosphereMedium;
  readonly regime: AtmosphereRegime;
}): ReadonlyArray<ChannelReading> {
  const { medium, regime } = split;
  return regimeChannels(
    upperMediumOf(medium, regime.kind === "cloudDeck" ? regime.deckAltitudeM : Number.NaN),
  );
}

describe("the cool giants' route (R08.T16.a.2; research-leans-43's lean, kept by main)", () => {
  const SERVED_ID = "41ffecb1ffc00013.0400";

  it("holds the chromophore gate's misses to vectorGate.json's jupiterEnvelope, value by value", () => {
    const channels = ["r", "g", "b"];
    const recorded = committedGate.jupiterEnvelope.geometries.flatMap((row) =>
      channels.flatMap((channel, c) =>
        row[19 + c] === "miss"
          ? [{ image: row[1], value: `${String(row[0])}/${channel}`, ratio: row[22 + c] }]
          : [],
      ),
    );
    expect(CHROMOPHORE_GATE_MISSES).toEqual(recorded);
  });

  it("bounds each exception at its recorded miss, rounded outward at its third figure", () => {
    expect(
      CHROMOPHORE_GATE_EXCEPTIONS.map((exception) => {
        const miss = CHROMOPHORE_GATE_MISSES.find((each) => each.value === exception.value);
        return [
          exception.value,
          miss === undefined ? null : outwardSteps(exception.bound, [miss.ratio], "most")[0],
          exception.cause.length > 0,
        ];
      }),
    ).toEqual(
      CHROMOPHORE_GATE_MISSES.map((miss) => [
        miss.value,
        expect.toSatisfy((steps: number) => steps >= 0 && steps < 1),
        true,
      ]),
    );
  });

  it("passes the chromophore gate with its named exceptions, and not without them", () => {
    expect([
      deckGatePasses(CHROMOPHORE_GATE_TALLIES),
      chromophoreGatePasses(CHROMOPHORE_GATE_TALLIES, CHROMOPHORE_GATE_MISSES),
      chromophoreGatePasses(CHROMOPHORE_GATE_TALLIES, CHROMOPHORE_GATE_MISSES, []),
    ]).toEqual([false, true, false]);
  });

  /** The chromophore gate's verdict with one miss more on the limb, in a pass's place. */
  const unlisted: GateMiss = { image: "limb", value: "limb-3H/g", ratio: 1.01 };
  const withUnlisted = CHROMOPHORE_GATE_TALLIES.map((image) =>
    image.image === "limb" ? tallied(image, image.passes - 1, image.misses + 1, 4) : image,
  );
  it.each([
    ["an unlisted miss", withUnlisted, [...CHROMOPHORE_GATE_MISSES, unlisted]],
    [
      "a listed miss past its bound",
      CHROMOPHORE_GATE_TALLIES,
      CHROMOPHORE_GATE_MISSES.map((miss) =>
        miss.value === "disc-p75/r" ? { ...miss, ratio: 1.551 } : miss,
      ),
    ],
    ["a miss its list leaves out", CHROMOPHORE_GATE_TALLIES, CHROMOPHORE_GATE_MISSES.slice(1)],
  ] as const)("closes the chromophore gate on %s", (_, tallies, misses) => {
    expect(chromophoreGatePasses(tallies, misses)).toBe(false);
  });

  /** A cool giant's split regime, and a self-luminous one's. */
  const cool: AtmosphereRegime = { kind: "cloudDeck", deckAltitudeM: 5_000 };
  const luminous: AtmosphereRegime = { ...cool, selfLuminous: true };

  it("takes the envelope route on both settings, its deck routes widened to the cool giants' range", () => {
    expect([
      GIANT_ROUTE,
      COOL_GIANT_DECK_ROUTE_RANGE,
      giantDeckRouteRange(cool, "envelope"),
    ]).toEqual([
      "envelope",
      { ground: SURFACE_DECK_ROUTE_RANGE, envelope: COOL_GIANT_ROUTE_RANGE },
      COOL_GIANT_DECK_ROUTE_RANGE,
    ]);
  });

  it.each([
    ["on the closed route", () => giantDeckRouteRange(cool, "closed")],
    [
      "for a self-luminous body on the envelope route",
      () => giantDeckRouteRange(luminous, "envelope"),
    ],
    [
      "where the chromophore gate's verdict does not pass",
      () => coolGiantRouteRangeOf(CHROMOPHORE_GATE_TALLIES, CHROMOPHORE_GATE_MISSES.slice(1)),
    ],
  ] as const)("keeps giant-deck's envelope range %s", (_, rangeOf) => {
    expect(rangeOf()).toBe(DECK_ROUTE_RANGE);
  });

  it("opens no cool giants' range where giant-deck's envelope gate does not pass", () => {
    const surfaceOnly: DeckRouteRange = { ground: SURFACE_DECK_ROUTE_RANGE, envelope: null };
    expect([
      coolGiantRouteRangeOf(CHROMOPHORE_GATE_TALLIES, CHROMOPHORE_GATE_MISSES, surfaceOnly),
      coolGiantRouteRangeOf(CHROMOPHORE_GATE_TALLIES, CHROMOPHORE_GATE_MISSES, null),
    ]).toEqual([surfaceOnly, null]);
  });

  it("holds the served giants on file to their assembly: the cool giant's frame gives its row", () => {
    const row = SERVED.find((each) => each.id === SERVED_ID);
    const served = assembledGiant(servedCoolGiant(), "41ffecb1ffc00013");
    const channels = channelsAbove(served);
    expect([
      SERVED.length,
      served.regime.selfLuminous ?? false,
      channels.map((c) => [c.opticalDepth, c.singleScatteringAlbedo, c.asymmetry]),
    ]).toEqual([
      35,
      row?.selfLuminous,
      (row?.channels ?? []).map((c) => [
        expect.closeTo(c.opticalDepth, 4),
        expect.closeTo(c.singleScatteringAlbedo, 6),
        expect.closeTo(c.asymmetry, 6),
      ]),
    ]);
  }, 180_000);

  it("refuses by name the Dev Fixture's two self-luminous giants and the two absorbing past the range, and routes its 31 others", () => {
    const refused = SERVED.filter((each) => !SERVED_COOL.includes(each));
    expect([SERVED_COOL.length, refused.map((each) => [each.id, each.selfLuminous])]).toEqual([
      31,
      [
        ["41ffecb1ffc0000d.0200", true],
        ["41ffecb1ffc00013.0200", false],
        ["41ffecb200000004.0100", true],
        ["41ffecb200000011.0100", false],
      ],
    ]);
  });

  it.each(BOUNDS)(
    "sets the cool giants' %s at the extreme of the cases, the hand and generated giants and the served cool giants, rounded outward at its third figure",
    (key, side, read) => {
      const held = [
        regimeChannels(upperMediumOf(giant().medium, envelopeFloorM(giant().medium))),
        regimeChannels(
          upperMediumOf(
            jupiterEnvelopeCase().referenceCase.medium,
            jupiterEnvelopeCase().split.altitudeM,
          ),
        ),
        ...[handJupiterDetail(), jupiterDetail(), handSaturnDetail(), handIceGiantDetail()].map(
          (detail) => channelsAbove(assembledGiant(detail)),
        ),
      ].flat();
      const served = SERVED_COOL.flatMap((each) => each.channels);
      const values = [...held.map(read), ...served.map(read)];
      expect(outwardSteps(COOL_GIANT_ROUTE_RANGE[key], values, side)).toEqual([
        expect.toSatisfy((steps: number) => steps >= 0 && steps < 1),
      ]);
    },
    180_000,
  );

  it("routes the served cool giant, the generated and the hand Jupiter, the hand Saturn and the ice giant through their envelopes' decks on the envelope route alone", () => {
    const giants = [
      assembledGiant(servedCoolGiant(), "41ffecb1ffc00013"),
      ...[jupiterDetail(), handJupiterDetail(), handSaturnDetail(), handIceGiantDetail()].map(
        (detail) => assembledGiant(detail),
      ),
    ];
    expect(
      giants.map(({ medium, regime }) => [
        deckRouted(regime, medium, true, giantDeckRouteRange(regime, "envelope")),
        deckRouted(regime, medium, true, giantDeckRouteRange(regime, "closed")),
      ]),
    ).toEqual(giants.map(() => [true, false]));
  }, 180_000);

  it("refuses the hot Jupiter by name on the envelope route: self-luminous, and absorbing past the cool giants' range in every channel", () => {
    const { medium, regime } = assembledGiant(hotJupiterDetail());
    expect([
      regime.selfLuminous,
      deckRouted(regime, medium, true, giantDeckRouteRange(regime, "envelope")),
      channelsAbove({ medium, regime }).map((c) =>
        channelWithinRouteRange(c, COOL_GIANT_ROUTE_RANGE),
      ),
    ]).toEqual([true, false, [false, false, false]]);
  }, 180_000);
});

/** `titan-class`, and the regime it routes under a clear sky, how its gate reads it. */
function titan(): { readonly medium: AtmosphereMedium; readonly regime: AtmosphereRegime } {
  const { medium } = titanClassReferenceCase();
  return { medium, regime: classifyRegime(medium, CLEAR_COVER) };
}

/** Whether one of the haze gate's named exceptions passes `miss`, within its bound. */
function excusedByTheGate(miss: GateMiss): boolean {
  return HAZE_GATE_EXCEPTIONS.some(
    (exception) => exception.value === miss.value && miss.ratio <= exception.bound,
  );
}

describe("the Titan-class haze's gate (R08.T14.d.4)", () => {
  /**
   * Every image the gate judges passing, one value of each refused, and those it does not judge
   * refused throughout: a fixture verdict, with no miss.
   */
  const passing: ReadonlyArray<GateImageTally> = HAZE_GATE_TALLIES.map((image) =>
    HAZE_UNJUDGED_IMAGES.includes(image.image)
      ? tallied(image, 0, 0, image.comparisons)
      : tallied(image, image.comparisons - 1, 0, 1),
  );

  /**
   * `passing` with `misses` added to their images' tallies, each in a pass's place, or a refusal's
   * on an image the gate does not judge.
   */
  const missing = (misses: ReadonlyArray<GateMiss>): ReadonlyArray<GateImageTally> =>
    passing.map((image) => {
      const here = misses.filter((miss) => miss.image === image.image).length;
      if (here === 0) {
        return image;
      }
      return HAZE_UNJUDGED_IMAGES.includes(image.image)
        ? tallied(image, image.passes, here, image.refusals - here)
        : tallied(image, image.passes - here, here, image.refusals);
    });

  /** The committed record's titan-class section. */
  const titanClass = committedGate.titanClass;

  it("holds the recorded verdict to vectorGate.json's titanClass, image by image", () => {
    expect(HAZE_GATE_TALLIES).toEqual(
      titanClass.routed.images.map(([image, comparisons, passes, misses, refusals]) => ({
        image,
        comparisons,
        passes,
        misses,
        refusals,
      })),
    );
  });

  it("holds the misses on the judged images to vectorGate.json's titanClass, value by value", () => {
    const channels = ["r", "g", "b"];
    const recordedMisses = titanClass.geometries.flatMap((row) =>
      HAZE_UNJUDGED_IMAGES.includes(String(row[1]))
        ? []
        : channels.flatMap((channel, c) =>
            row[19 + c] === "miss"
              ? [{ image: row[1], value: `${String(row[0])}/${channel}`, ratio: row[22 + c] }]
              : [],
          ),
    );
    expect(HAZE_GATE_MISSES).toEqual(recordedMisses);
  });

  it("bounds each exception at its recorded value, rounded outward at its third figure, for a value its route draws better than the analytic path, image by image", () => {
    const worst = new Map(titanClass.worstByGeometry.map((row) => [String(row[0]), row]));
    expect(
      HAZE_GATE_EXCEPTIONS.map((exception) => {
        const miss = HAZE_GATE_MISSES.find((each) => each.value === exception.value);
        const [geometry] = exception.value.split("/");
        const row = worst.get(geometry ?? "");
        return [
          exception.value,
          miss === undefined ? null : outwardSteps(exception.bound, [miss.ratio], "most")[0],
          Number(row?.[3]) > Number(row?.[1]),
          exception.cause.length > 0,
        ];
      }),
    ).toEqual(
      HAZE_GATE_EXCEPTIONS.map((exception) => [
        exception.value,
        expect.toSatisfy((steps: number) => steps >= 0 && steps < 1),
        true,
        true,
      ]),
    );
  });

  it("closes the haze's route on the recorded verdict (R08.T17): titan-class misses past its named exceptions on the images it judges", () => {
    const unexcused = HAZE_GATE_MISSES.filter(
      (miss) => !HAZE_UNJUDGED_IMAGES.includes(miss.image) && !excusedByTheGate(miss),
    );
    expect([
      hazeGatePasses(HAZE_GATE_TALLIES, HAZE_GATE_MISSES),
      THICK_ROUTE_RANGES,
      unexcused.length > 0,
    ]).toEqual([false, [THICK_ROUTE_RANGE], true]);
  });

  it("routes titan-class if and only if its gate passes with the listed exceptions", () => {
    const { medium, regime } = titan();
    const unlisted: GateMiss = { image: "limb", value: "limb-3H/g", ratio: 1.01 };
    const closed = thickRouteRangesOf(missing([unlisted]), [unlisted]);
    expect([
      regime.kind,
      thickBakeRouted(regime, medium),
      thickBakeRouted(regime, medium, thickRouteRangesOf(HAZE_GATE_TALLIES, HAZE_GATE_MISSES)),
      thickBakeRouted(regime, medium, thickRouteRangesOf(passing, [])),
      closed,
      thickBakeRouted(regime, medium, closed),
    ]).toEqual(["thickScattering", false, false, true, [THICK_ROUTE_RANGE], false]);
  }, 180_000);

  it("closes on an unlisted miss, a listed one past its bound, or a miss its list leaves out", () => {
    // A fixture's exceptions, so that the rule is held whatever the recorded verdict.
    const exceptions: ReadonlyArray<GateException> = [
      { value: "ground-s30-v30-a45/r", bound: 1.04, cause: "a fixture's" },
      { value: "disc-p60/r", bound: 1.08, cause: "a fixture's" },
    ];
    const listed: ReadonlyArray<GateMiss> = [
      { image: "ground-s30", value: "ground-s30-v30-a45/r", ratio: 1.039 },
      { image: "disc-p60", value: "disc-p60/r", ratio: 1.075 },
    ];
    const unlisted: GateMiss = { image: "ground-s60", value: "ground-s60-v0-a0/b", ratio: 1.001 };
    const past = listed.map((miss) =>
      miss.value === "ground-s30-v30-a45/r" ? { ...miss, ratio: 1.041 } : miss,
    );
    const atBound = listed.map((miss) =>
      miss.value === "ground-s30-v30-a45/r" ? { ...miss, ratio: 1.04 } : miss,
    );
    expect([
      hazeGatePasses(missing(listed), listed, exceptions),
      hazeGatePasses(missing(atBound), atBound, exceptions),
      hazeGatePasses(missing([...listed, unlisted]), [...listed, unlisted], exceptions),
      hazeGatePasses(missing(past), past, exceptions),
      hazeGatePasses(missing([...listed, unlisted]), listed, exceptions),
      hazeGatePasses(missing(listed), listed, []),
    ]).toEqual([true, true, false, false, false, false]);
  });

  it("never routes a Mars- or Venus-class medium, and routes the Earth- and Saturn-class cases as before, open or closed", () => {
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
      routedOn([THICK_ROUTE_RANGE]),
    ]).toEqual([
      "thickScattering",
      [true, true, true, false, false, false],
      [true, true, true, false, false, false],
    ]);
  });

  it("bakes a haze-routed world as a haze route's bake is, and no world the kept cases' range routes", () => {
    const { medium, regime } = titan();
    const earth = cases.EARTH_CASE.medium;
    const earthRegime = classifyRegime(earth, CLEAR_COVER);
    expect([
      bakedAsHaze(regime, medium, [THICK_ROUTE_RANGE, HAZE_ROUTE_RANGE]),
      bakedAsHaze(regime, medium),
      bakedAsHaze(regime, medium, [THICK_ROUTE_RANGE]),
      bakedAsHaze(earthRegime, earth),
      bakedAsHaze({ kind: "thin" }, medium),
    ]).toEqual([true, false, false, false, false]);
  }, 180_000);

  it("judges no image whose sun stands at most 0.35 over the horizon, and closes on an image refused throughout", () => {
    const lowSun: ReadonlyArray<GateMiss> = [
      { image: "ground-s80", value: "ground-s80-v60-a0/r", ratio: 1.1 },
      { image: "ground-s90", value: "ground-s90-v0-a0/r", ratio: 3.4 },
      { image: "ground-s95", value: "ground-s95-v0-a0/r", ratio: 5 },
    ];
    const refused = passing.map((image) =>
      image.image === "limb" ? tallied(image, 0, 0, 15) : image,
    );
    expect([
      HAZE_UNJUDGED_IMAGES,
      hazeGatePasses(passing, []),
      hazeGatePasses(missing(lowSun), lowSun),
      hazeGatePasses(missing(lowSun), []),
      hazeGatePasses(refused, []),
      hazeGatePasses([], []),
    ]).toEqual([["ground-s80", "ground-s90", "ground-s95"], true, true, true, false, false]);
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

describe("a haze's low sun (R08.T14.d.4)", () => {
  it("labels a haze's sky from the sun of the bake's horizon rows, HORIZON_SUN_COSINE", () => {
    expect(LOW_SUN_SINE).toBe(HORIZON_SUN_COSINE);
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
