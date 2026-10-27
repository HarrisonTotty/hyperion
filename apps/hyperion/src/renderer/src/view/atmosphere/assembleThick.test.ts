/**
 * The router at its two call sites (plan R08, R08.T13.d; `decision-r08-t13c2-classes.md` §5): the
 * assembly routes a record's medium under the body's cover (`bodyCoverOf`), and the atmosphere
 * cache a kept scene's under its own, each carrying the regime it computed and, for both thick
 * regimes, `ATMOSPHERE: APPROXIMATE` with `THICK_REASON`. The ruling's label changes, row by row;
 * the cover's decks on a synthetic inventory; and the kernels' resampling, which moves no class.
 */
import type { AerosolInventoryDto, AerosolModeDto, BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { ATOMIC_MASS_CONSTANT_KG } from "../../lib/system/constants";
import type { BodyDetail } from "../../lib/system/model";
import { titanClassMethaneMedium, venusClassMedium } from "../../test/atmosphereFixtures";
import {
  detailOf,
  gammaMode,
  marsLikeDetail,
  recordInput,
  SUN_LIGHT,
  venus58Detail,
  venus92Detail,
  withInventory,
  withSurface,
} from "../../test/atmosphereRecords";
import { sweepMedium } from "../../test/atmosphereSweep";
import { earthDetail, FIXTURE_SYSTEM } from "../../test/planetaryFixture";
import { ATMOSPHERE_FIXTURES, AtmosphereFixtures } from "../scenes/atmosphereScene";
import { type AssembledMedium, assembleMedium, bodyCoverOf, THICK_REASON } from "./assemble";
import { AtmosphereCache, type CachedTables, type KeptBodyAtmosphere } from "./AtmosphereCache";
import { hydrostaticColumn } from "./column";
import { EARTH_DRY_AIR, EARTH_REFERENCE, EARTH_SLAB_REFERENCE } from "./earth";
import { tableRadiusM } from "./hillaire";
import { atmosphereApproximateLines, type AtmosphereLabel } from "./labels";
import type { AtmosphereMedium } from "./medium";
import { opticalDepthToTop } from "./opticalDepth";
import { BOLTZMANN_J_PER_K, molecularTerm } from "./rayleigh";
import * as cases from "./reference/cases";
import {
  EARTH_REFERENCE_RADIUS_M,
  MARS_REFERENCE,
  MARS_REFERENCE_RADIUS_M,
  SATURN_OBLATE_REFERENCE,
  SATURN_OBLATE_SPHEROID,
} from "./reference/cases";
import { kernelMedium } from "./tables";
import {
  approximateRegime,
  type AtmosphereRegime,
  type BodyCover,
  CLEAR_COVER,
  classifyRegime,
  regimeChannels,
  verticalOpticalDepth,
} from "./thick/regime";

/**
 * An Earth-class column: the U.S. Standard Atmosphere 1976's T₀, K, M₀, g mol⁻¹, and g₀, m s⁻², on
 * a sphere of 6,371 km.
 */
const EARTH_CLASS = {
  temperatureK: 288.15,
  meanMolarMassGPerMol: 28.9644,
  referenceGravityMS2: 9.806_65,
  referenceRadiusM: 6_371_000,
} as const;

/** An Earth-class isothermal column of dry air at `surfacePa`. */
function earthClassColumn(surfacePa: number): ReturnType<typeof hydrostaticColumn> {
  return hydrostaticColumn({
    surfacePa,
    temperature: { kind: "isothermal", temperatureK: EARTH_CLASS.temperatureK },
    meanMolarMassGPerMol: EARTH_CLASS.meanMolarMassGPerMol,
    referenceGravityMS2: EARTH_CLASS.referenceGravityMS2,
    referenceRadiusM: EARTH_CLASS.referenceRadiusM,
  });
}

/** An Earth-class column of dry air at `surfacePa` under its molecular term alone, over a ground of 0.1. */
function earthClassAir(surfacePa: number): AtmosphereMedium {
  const column = earthClassColumn(surfacePa);
  return {
    name: "Earth-class air",
    topHeightM: column.topHeightM,
    groundAlbedo: [0.1, 0.1, 0.1],
    terms: [molecularTerm(column, EARTH_DRY_AIR).term],
  };
}

/** The surface pressure at which {@link earthClassAir}'s worst channel reaches τ 1, Pa. */
function earthClassAirAtOnePa(): number {
  // The air's τ is proportional to its surface pressure on an isothermal column.
  return 101_325 / Math.max(...verticalOpticalDepth(earthClassAir(101_325)));
}

/** R08.T10.a's provisional thick line, retired: a vertical optical depth of 1 on the worst channel. */
function retiredLineThick(medium: AtmosphereMedium): boolean {
  return Math.max(...verticalOpticalDepth(medium)) >= 1;
}

/** A tenuous air: a Rayleigh layer of τ 1/64 over a ground of 0.3, under the boundary's floor. */
function tenuousAir(): AtmosphereMedium {
  return sweepMedium({ singleScatteringAlbedo: 1, asymmetry: 0, tau: 1 / 64, groundAlbedo: 0.3 });
}

/** What a call site gives a medium: its regime, labels and reasons. */
interface Routed {
  readonly regime: AtmosphereRegime | null;
  readonly labels: ReadonlyArray<AtmosphereLabel>;
  readonly reasons: ReadonlyArray<string>;
}

/** A class of the ruling's table: the router's regimes that carry no deck's altitude. */
type RowClass = "thin" | "thickScattering";

/** What the cache gives a kept medium of no labels of its own routed `kind`. */
function atTheCache(kind: RowClass): Routed {
  return kind === "thin"
    ? { regime: { kind }, labels: [], reasons: [] }
    : { regime: { kind }, labels: ["atmosphereApproximate"], reasons: [THICK_REASON] };
}

/** A body of the fixture's system, by its index. */
function bodyId(n: number): BodyIdHex {
  return `${FIXTURE_SYSTEM}.${(0x0400 + n).toString(16).padStart(4, "0")}`;
}

/** A cache of kept scenes alone: it asks for no record and builds no medium. */
function keptCache(): AtmosphereCache<CachedTables> {
  return new AtmosphereCache<CachedTables>({
    requests: {
      request: () => {
        throw new Error("a kept scene asks for no record");
      },
    },
    builder: {
      build: () => {
        throw new Error("a kept scene's medium is built by the scene");
      },
      terminate: () => undefined,
    },
    tables: () => ({ rowBytes: 0, dispose: () => undefined }),
  });
}

/** What the cache holds for each kept setting after one frame, and the frame's reasons by body. */
function keptRouted(settings: ReadonlyArray<KeptBodyAtmosphere>): {
  readonly routed: ReadonlyArray<Routed>;
  readonly reasons: ReadonlyMap<BodyIdHex, ReadonlyArray<string>>;
} {
  const cache = keptCache();
  const report = cache.openView().noteFrame({
    nowMs: 0,
    scene: null,
    bodies: settings.map((kept, i) => ({
      id: bodyId(i),
      regime: "disc" as const,
      lights: [SUN_LIGHT],
      kept,
    })),
  });
  const routed = settings.map((_, i) => {
    const entry = cache.entryOf(bodyId(i));
    if (entry === null) {
      throw new Error(`kept body ${i} holds no entry`);
    }
    return { regime: entry.regime, labels: entry.labels, reasons: entry.reasons };
  });
  cache.dispose();
  return { routed, reasons: report.reasons };
}

/** A kept setting of `medium` on a sphere, with no labels of the scene's own. */
function keptMedium(medium: AtmosphereMedium, cover?: BodyCover): KeptBodyAtmosphere {
  const built = { medium, bottomRadiusM: 6_371_008.8, curves: [], body: null };
  return cover === undefined
    ? { kind: "medium", built, labels: [] }
    : { kind: "medium", built, labels: [], cover };
}

/** An assembly's regime, labels and reasons. */
function routedOf(result: AssembledMedium): Routed {
  if (result.medium === undefined) {
    throw new Error("the record draws no medium");
  }
  return { regime: result.regime, labels: result.labels, reasons: result.reasons };
}

/** A record's assembly. */
function assembled(detail: BodyDetail): AssembledMedium {
  return assembleMedium(recordInput(detail));
}

/** The kept rows of the ruling's table (§5, "The labels that change"): a medium and its class. */
function keptRows(): ReadonlyArray<readonly [string, RowClass, AtmosphereMedium]> {
  const atOnePa = earthClassAirAtOnePa();
  return [
    ["TEST EARTH's medium, the measured Earth", "thickScattering", EARTH_REFERENCE],
    ["TEST EARTH O3 SLAB's medium", "thickScattering", EARTH_SLAB_REFERENCE],
    ["TEST MARS's medium, its dust included", "thickScattering", MARS_REFERENCE],
    ["TEST SATURN's medium", "thickScattering", SATURN_OBLATE_REFERENCE],
    ["an Earth-class air at 0.999 of τ 1", "thickScattering", earthClassAir(atOnePa * 0.999)],
    ["an Earth-class air at 1.001 of τ 1", "thickScattering", earthClassAir(atOnePa * 1.001)],
    ["the Venus-class medium", "thickScattering", venusClassMedium().medium],
    ["a Titan-class column", "thickScattering", titanClassMethaneMedium().medium],
    ["a tenuous Rayleigh air of τ 1/64", "thin", tenuousAir()],
  ];
}

/** The record rows of the ruling's table: a record and its class. */
const RECORD_ROWS: ReadonlyArray<readonly [string, RowClass, () => BodyDetail]> = [
  ["the generated Earth", "thickScattering", () => detailOf(earthDetail())],
  ["Venus at 92 bar", "thickScattering", venus92Detail],
  ["Venus at 58 bar", "thickScattering", venus58Detail],
  ["a Mars-like air of gas alone", "thin", () => marsLikeDetail()],
];

describe("the whole-extinction vertical optical depth", () => {
  const depthCases: ReadonlyArray<readonly [string, AtmosphereMedium, number]> = [
    ["the measured Earth", EARTH_REFERENCE, EARTH_REFERENCE_RADIUS_M],
    ["the Mars fixture", MARS_REFERENCE, MARS_REFERENCE_RADIUS_M],
    ["the Saturn-class fixture", SATURN_OBLATE_REFERENCE, tableRadiusM(SATURN_OBLATE_SPHEROID)],
    ["the Venus-class medium", venusClassMedium().medium, venusClassMedium().radiusM],
  ];

  it.each(depthCases)(
    "is opticalDepthToTop's from the ground at μ = 1 for %s, to 10⁻⁶",
    (_, medium, radiusM) => {
      const closed = verticalOpticalDepth(medium);
      const simpson = opticalDepthToTop(medium, radiusM, radiusM, 1);
      for (const [c, depth] of closed.entries()) {
        expect(Math.abs(depth / (simpson[c] ?? Number.NaN) - 1)).toBeLessThan(1e-6);
      }
    },
  );
});

describe("the reason", () => {
  it("reads the ruling's words, with no 'thick' in them", () => {
    expect([THICK_REASON, THICK_REASON.includes("thick")]).toEqual([
      "multiple scattering: drawn by the thin-atmosphere method beyond its checked range",
      false,
    ]);
  });
});

describe("a kept scene's medium at the cache (R08.T13.d)", () => {
  const rows = keptRows();

  it.each(rows)(
    "routes %s %s, with the label and the reason where it is thick",
    (_, kind, medium) => {
      const [routed] = keptRouted([keptMedium(medium)]).routed;
      expect(routed).toEqual(atTheCache(kind));
    },
  );

  it("takes a kept medium under CLEAR_COVER where the scene gives no cover", () => {
    const medium = venusClassMedium().medium;
    const { routed } = keptRouted([keptMedium(medium), keptMedium(medium, CLEAR_COVER)]);
    expect(routed[0]).toEqual(routed[1]);
  });

  it("routes a kept medium under a τ 30 deck at a cloud fraction of 1 cloudDeck at the deck's top, labelled", () => {
    const deck = { opticalDepth550: 30, baseAltitudeM: 2_000, topAltitudeM: 5_000 };
    const cover: BodyCover = { cloudFraction: 1, decks: [deck], envelope: false };
    const [routed] = keptRouted([keptMedium(venusClassMedium().medium, cover)]).routed;
    expect(routed).toEqual({
      regime: { kind: "cloudDeck", deckAltitudeM: 5_000 },
      labels: ["atmosphereApproximate"],
      reasons: [THICK_REASON],
    });
  });

  it("does not split a kept medium under the same deck at a cloud fraction of 0.6", () => {
    const deck = { opticalDepth550: 30, baseAltitudeM: 2_000, topAltitudeM: 5_000 };
    const cover: BodyCover = { cloudFraction: 0.6, decks: [deck], envelope: false };
    const [routed] = keptRouted([keptMedium(venusClassMedium().medium, cover)]).routed;
    expect(routed).toEqual(atTheCache("thickScattering"));
  });

  it("names TEST EARTH, TEST EARTH O3 SLAB, TEST MARS and TEST SATURN on one line of the plate, in the new words", async () => {
    const fixtures = new AtmosphereFixtures(() => Promise.resolve(cases));
    await fixtures.ready();
    const { media } = fixtures;
    if (media.kind !== "ready") {
      throw new Error("ATMOSPHERE TEST's media did not load");
    }
    const settings = ATMOSPHERE_FIXTURES.map(({ id, designation }) => {
      const held = media.media.get(id);
      if (held === undefined) {
        throw new Error(`ATMOSPHERE TEST holds no medium for ${designation}`);
      }
      return held.setting;
    });
    const { reasons } = keptRouted(settings);
    const lines = atmosphereApproximateLines(
      ATMOSPHERE_FIXTURES.map(({ designation }, i) => ({
        designation,
        reasons: reasons.get(bodyId(i)) ?? [],
      })),
    );
    expect(lines).toEqual([
      "Multiple scattering: drawn by the thin-atmosphere method beyond its checked range, for TEST EARTH, TEST EARTH O3 SLAB, TEST MARS and TEST SATURN",
    ]);
  });
});

describe("a record's medium at the assembly (R08.T13.d)", () => {
  it.each(RECORD_ROWS)("routes %s %s", (_, kind, detail) => {
    expect(routedOf(assembled(detail())).regime?.kind).toBe(kind);
  });

  it.each(RECORD_ROWS)(
    "gives %s, routed %s, the label and the reason where it is routed past thin, and the reason nowhere else",
    (_, _kind, detail) => {
      const { regime, labels, reasons } = routedOf(assembled(detail()));
      const thick = regime !== null && approximateRegime(regime);
      expect([
        thick && labels.includes("atmosphereApproximate"),
        reasons.includes(THICK_REASON),
      ]).toEqual([thick, thick]);
    },
  );

  it("carries the regime of the medium it gives, under the body's cover", () => {
    const result = assembled(detailOf(earthDetail()));
    if (result.medium === undefined) {
      throw new Error("the generated Earth draws a medium");
    }
    // The generated Earth's inventory is not modelled, so its cover holds no deck.
    expect(result.regime).toEqual(classifyRegime(result.medium, CLEAR_COVER));
  });
});

describe("the ruling's label changes, at both call sites", () => {
  it("leaves no medium the router routes past thin without the label and the reason", () => {
    const kept = keptRouted(keptRows().map(([, , medium]) => keptMedium(medium))).routed;
    const records = RECORD_ROWS.map(([, , detail]) => routedOf(assembled(detail())));
    const unlabelled = [...kept, ...records].filter(
      ({ regime, labels, reasons }) =>
        regime !== null &&
        approximateRegime(regime) &&
        !(labels.includes("atmosphereApproximate") && reasons.includes(THICK_REASON)),
    );
    expect(unlabelled).toEqual([]);
  });

  it("takes the label from no medium the retired line labelled", () => {
    const media = [
      ...keptRows().map(([name, , medium]) => [name, medium] as const),
      ...RECORD_ROWS.map(([name, , detail]) => {
        const { medium } = assembled(detail());
        if (medium === undefined) {
          throw new Error(`${name} draws a medium`);
        }
        return [name, medium] as const;
      }),
    ];
    const lost = media.filter(
      ([, medium]) =>
        retiredLineThick(medium) && classifyRegime(medium, CLEAR_COVER).kind === "thin",
    );
    expect(lost.map(([name]) => name)).toEqual([]);
  });
});

/** The wire's deck. */
type DeckDto = AerosolInventoryDto["decks"][number];

/** A water deck from 800 to 600 hPa, as the wire gives it. */
const WATER_DECK: DeckDto = {
  condensate: "H2O",
  phase: "liquid",
  base_pa: 80_000,
  // About the US Standard Atmosphere's (1976) at 800 hPa; not read by the cover.
  base_temperature_k: 275,
  top_pa: 60_000,
  vapour_mole_fraction_below: 0.0102,
  vapour_mole_fraction_above: 0.001,
  f_sed: 0,
  extrapolated: false,
};

/** A CO₂ ice deck from 200 to 100 hPa, as the wire gives it. */
const ICE_DECK: DeckDto = {
  condensate: "CO2",
  phase: "solid",
  base_pa: 20_000,
  // A hand value; not read by the cover.
  base_temperature_k: 200,
  top_pa: 10_000,
  vapour_mole_fraction_below: 0.0003,
  vapour_mole_fraction_above: 0.0001,
  f_sed: 0,
  extrapolated: false,
};

/** A mode with its optical depth not modelled, as the wire sends one whose optics are not. */
function withoutDepth(mode: AerosolModeDto): AerosolModeDto {
  const { tau_550: _depth, tau_basis: _basis, ...rest } = mode;
  return { ...rest, not_modelled: { type: "optics_not_modelled" } };
}

/**
 * The cover's inventory: the water deck's three modes, τ 20 over the whole sky, τ 20 over half and
 * one whose depth is not modelled; the CO₂ deck's one, τ 4 over a quarter; and a sulphate layer of
 * no deck, τ 0.5.
 */
function coverInventory(): AerosolInventoryDto {
  const water = {
    material: "H2O",
    phase: "liquid",
    deck: "H2O",
    basePa: 80_000,
    topPa: 60_000,
  } as const;
  return {
    modes: [
      gammaMode({ ...water, effectiveRadiusM: 1e-5, tau550: 20 }),
      { ...gammaMode({ ...water, effectiveRadiusM: 5e-6, tau550: 20 }), coverage: 0.5 },
      withoutDepth(gammaMode({ ...water, effectiveRadiusM: 2e-5, tau550: 5 })),
      {
        ...gammaMode({
          material: "CO2",
          phase: "solid",
          deck: "CO2",
          effectiveRadiusM: 2e-6,
          tau550: 4,
          basePa: 20_000,
          topPa: 10_000,
        }),
        coverage: 0.25,
      },
      gammaMode({
        material: "H2SO4",
        phase: "liquid",
        effectiveRadiusM: 2e-7,
        tau550: 0.5,
        basePa: 3_000,
        topPa: 1_000,
      }),
    ],
    decks: [WATER_DECK, ICE_DECK],
    absorbers: [],
    not_formed: [],
  };
}

/** The generated Earth's surface with `inventory`, at a cloud fraction of `cloudFraction`. */
function surfaceWith(inventory: AerosolInventoryDto | undefined, cloudFraction: number) {
  const answer = withSurface(earthDetail(), (surface) => ({
    ...surface,
    cloud_fraction: cloudFraction,
  }));
  const detail = detailOf(inventory === undefined ? answer : withInventory(answer, inventory));
  const { surface } = detail.record;
  if (surface.state !== "ok") {
    throw new Error("the generated Earth's surface is ok");
  }
  return { detail, surface: surface.value };
}

/**
 * The height of `pressurePa` on {@link earthClassColumn}'s isothermal column: its geopotential
 * height Φ = kT ÷ (μ m_u g_ref) ln(p_s ÷ p) at the height z = R Φ ÷ (R − Φ), with `column.ts`'s own
 * k and m_u.
 */
function isothermalHeightM(surfacePa: number, pressurePa: number): number {
  const scaleHeightM =
    (BOLTZMANN_J_PER_K * EARTH_CLASS.temperatureK) /
    (EARTH_CLASS.meanMolarMassGPerMol * ATOMIC_MASS_CONSTANT_KG * EARTH_CLASS.referenceGravityMS2);
  const phiM = scaleHeightM * Math.log(surfacePa / pressurePa);
  return (EARTH_CLASS.referenceRadiusM * phiM) / (EARTH_CLASS.referenceRadiusM - phiM);
}

describe("bodyCoverOf (R08.T13.d)", () => {
  const column = earthClassColumn(101_325);

  it("maps each deck in the inventory's order: its modes' τ(550) at their coverage, its base and top as heights on the column", () => {
    const { surface } = surfaceWith(coverInventory(), 0.8);
    const cover = bodyCoverOf(surface, column);
    const expected = (base: number, top: number) => ({
      base: isothermalHeightM(101_325, base),
      top: isothermalHeightM(101_325, top),
    });
    const water = expected(80_000, 60_000);
    const ice = expected(20_000, 10_000);
    expect([cover.cloudFraction, cover.envelope, cover.decks.length]).toEqual([0.8, false, 2]);
    expect(cover.decks[0]?.opticalDepth550).toBeCloseTo(30, 12);
    expect(cover.decks[1]?.opticalDepth550).toBeCloseTo(1, 12);
    expect(cover.decks[0]?.baseAltitudeM).toBeCloseTo(water.base, 2);
    expect(cover.decks[0]?.topAltitudeM).toBeCloseTo(water.top, 2);
    expect(cover.decks[1]?.baseAltitudeM).toBeCloseTo(ice.base, 2);
    expect(cover.decks[1]?.topAltitudeM).toBeCloseTo(ice.top, 2);
  });

  it("gives a surface whose inventory is not modelled its cloud fraction and no deck", () => {
    const { surface } = surfaceWith(undefined, 0.67);
    expect(bodyCoverOf(surface, column)).toEqual({
      cloudFraction: 0.67,
      decks: [],
      envelope: false,
    });
  });

  it("lays a deck below the datum at 0 and one above the column's top at the top", () => {
    const decks = [
      { ...WATER_DECK, base_pa: 300_000, top_pa: 200_000 },
      { ...ICE_DECK, base_pa: 1e-3, top_pa: 1e-4 },
    ];
    const { surface } = surfaceWith({ modes: [], decks, absorbers: [], not_formed: [] }, 1);
    const cover = bodyCoverOf(surface, column);
    expect(cover.decks.map((d) => [d.baseAltitudeM, d.topAltitudeM])).toEqual([
      [0, 0],
      [column.topHeightM, column.topHeightM],
    ]);
  });
});

describe("the deck rule at the assembly (R08.T13.d)", () => {
  /** The generated Earth under a water deck of τ 30 at `cloudFraction`. */
  function deckEarth(cloudFraction: number): AssembledMedium {
    const water = {
      material: "H2O",
      phase: "liquid",
      deck: "H2O",
      basePa: 80_000,
      topPa: 60_000,
    } as const;
    const inventory: AerosolInventoryDto = {
      modes: [gammaMode({ ...water, effectiveRadiusM: 1e-6, tau550: 30 })],
      decks: [WATER_DECK],
      absorbers: [],
      not_formed: [],
    };
    return assembled(surfaceWith(inventory, cloudFraction).detail);
  }

  it("routes a body under a τ 30 deck at a cloud fraction of 1 cloudDeck inside its column, with the label and the reason", () => {
    const result = deckEarth(1);
    const { regime, labels, reasons } = routedOf(result);
    const topHeightM = result.medium?.topHeightM ?? Number.NaN;
    const altitudeM = regime?.kind === "cloudDeck" ? regime.deckAltitudeM : Number.NaN;
    expect([
      regime?.kind,
      altitudeM > 0 && altitudeM < topHeightM,
      labels.includes("atmosphereApproximate"),
      reasons.includes(THICK_REASON),
    ]).toEqual(["cloudDeck", true, true, true]);
  });

  it("does not split the same body at a cloud fraction of 0.6", () => {
    expect(routedOf(deckEarth(0.6)).regime?.kind).toBe("thickScattering");
  });
});

/** A medium's regime under a clear sky and each channel's two lines, as the router reads them. */
function classOf(
  medium: AtmosphereMedium,
): readonly [AtmosphereRegime, ReadonlyArray<readonly [boolean, boolean]>] {
  return [
    classifyRegime(medium, CLEAR_COVER),
    regimeChannels(medium).map((c) => [c.thickByDepth, c.thickByGround] as const),
  ];
}

describe("the kernels' resampling (`decision-r08-t13c2-classes.md` §4)", () => {
  /** Each kept fixture and record medium, by name. */
  function media(): ReadonlyArray<readonly [string, AtmosphereMedium]> {
    const records = RECORD_ROWS.map(([name, , detail]) => {
      const { medium } = assembled(detail());
      if (medium === undefined) {
        throw new Error(`${name} draws a medium`);
      }
      return [name, medium] as const;
    });
    return [
      ["the measured Earth", EARTH_REFERENCE],
      ["Earth with the ozone slab", EARTH_SLAB_REFERENCE],
      ["the Mars fixture", MARS_REFERENCE],
      ["the Saturn-class fixture", SATURN_OBLATE_REFERENCE],
      ["the Venus-class medium", venusClassMedium().medium],
      ["the Titan-class column", titanClassMethaneMedium().medium],
      ...records,
    ];
  }

  it("classifies each kept fixture and record medium alike on kernelMedium's read and on its own, channel by channel", () => {
    const all = media();
    expect(all.map(([name, medium]) => [name, classOf(kernelMedium(medium))])).toEqual(
      all.map(([name, medium]) => [name, classOf(medium)]),
    );
  });
});
