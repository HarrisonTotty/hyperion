/**
 * The reference tracer's cloudy and hazy cases (plan R08, R08.T12.b.2): each `<name>.case.json` is
 * the case the client's optics write now, through `bless.ts`; their media are P14.T24.g's Venus
 * deck, the thick bake's Titan-class fixture and `saturn-oblate`'s air with a deck; and their
 * geometry sets are Design note 10's with the probes' aggregates.
 *
 * @remarks
 * A changed case is rewritten by `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/thickCases.test.ts`, and its reference traced afresh
 * from the repository's root by `test/atmosphereCases.ts`'s `traceCommand`. The cases' Mie runs for
 * about a minute, once a file.
 */

import { describe, expect, it } from "vitest";

import { caseJson, type SampleCounts, traceCommand } from "../../../test/atmosphereCases";
import { titanClassFixture } from "../../../test/bakeFixtures";
import { expectCommittedTable } from "../../../test/bless";
import { columnLengthM, extinction, type MediumTerm } from "../medium";
import giantDeckCase from "./giant-deck.case.json" with { type: "json" };
import titanClassCase from "./titan-class.case.json" with { type: "json" };
import venus58barCase from "./venus-58bar.case.json" with { type: "json" };
import venus92barCase from "./venus-92bar.case.json" with { type: "json" };
import sampleCounts from "./sampleCounts.json" with { type: "json" };
import { ATMOSPHERE_FIXTURES, builtFigureOf, builtOfCase } from "../../scenes/atmosphereScene";
import { EARTH_CASE, SATURN_OBLATE_REFERENCE } from "./cases";
import { angleBetweenDeg, LEAST_SUN_DISTANCE_DEG } from "./geometries";
import {
  GIANT_DECK,
  THICK_CASE_NAMES,
  THICK_CASE_SIGNIFICANT_FIGURES,
  thickCaseCovers,
  thickReferenceCases,
  VENUS_DECK,
  venusDeckCover,
  VENUS_DECK_MODES,
  venusDeckPressurePa,
} from "./thickCases";
import { thickCasesReplyOf } from "./thickCasesMessages";
import { classifyRegime } from "../thick/regime";

/** Each case's own counts, from its pilot (`decision-r08-t12b-scope.md`, ruling 8). */
const COUNTS: Readonly<Record<string, SampleCounts>> = sampleCounts;

/** The committed case files, by case. */
const COMMITTED: Readonly<Record<string, unknown>> = {
  "venus-92bar": venus92barCase,
  "venus-58bar": venus58barCase,
  "titan-class": titanClassCase,
  "giant-deck": giantDeckCase,
};

/** The cases' Mie, about a minute, paid by the first test that asks. */
const BUILD_MS = 300_000;

/** A term of a case's medium by name, or a thrown error naming it. */
function termOf(terms: ReadonlyArray<MediumTerm>, name: string): MediumTerm {
  const term = terms.find((t) => t.name === name);
  if (term === undefined) {
    throw new Error(`the medium has its ${name} term`);
  }
  return term;
}

/** A term's vertical optical depth per channel over its medium's column. */
function verticalDepth(term: MediumTerm, topHeightM: number): readonly number[] {
  const length = columnLengthM(term.density, topHeightM);
  return extinction(term).map((e) => e * length);
}

describe("the thick cases", () => {
  it.each(THICK_CASE_NAMES)(
    "%s.case.json is the case the client's optics write now",
    { timeout: BUILD_MS },
    async (name) => {
      const c = thickReferenceCases()[name];
      try {
        await expect(
          expectCommittedTable(caseJson(c, COUNTS[name]), COMMITTED[name], `./${name}.case.json`),
        ).resolves.toBeUndefined();
      } catch (error: unknown) {
        const reason = error instanceof Error ? error.message : String(error);
        throw new Error(`${reason}; then trace its reference: \`${traceCommand(name)}\``, {
          cause: error,
        });
      }
    },
  );

  it.each(THICK_CASE_NAMES)("%s writes its terms to its figures", { timeout: BUILD_MS }, (name) => {
    expect(thickReferenceCases()[name].significantFigures).toBe(THICK_CASE_SIGNIFICANT_FIGURES);
  });

  it.each(THICK_CASE_NAMES)(
    "%s has Design note 10's 120 geometries, none within 3° of its sun",
    { timeout: BUILD_MS },
    (name) => {
      const { geometries } = thickReferenceCases()[name];
      const nearest = Math.min(...geometries.map((g) => angleBetweenDeg(g.view, g.sun)));
      expect([geometries.length, new Set(geometries.map((g) => g.name)).size]).toEqual([120, 120]);
      expect(nearest).toBeGreaterThan(LEAST_SUN_DISTANCE_DEG - 1e-9);
    },
  );

  it.each(THICK_CASE_NAMES)(
    "%s's counts each name one of its geometries or aggregates",
    { timeout: BUILD_MS },
    (name) => {
      const c = thickReferenceCases()[name];
      const counts = COUNTS[name];
      const names = new Set([...c.geometries, ...c.aggregates].map((o) => o.name));
      const strays = [
        ...Object.keys(counts?.geometries ?? {}),
        ...Object.keys(counts?.aggregates ?? {}),
      ].filter((key) => !names.has(key));
      expect(strays).toEqual([]);
    },
  );

  it(
    "give the Venus-class cases the probe's aggregate beside Design note 10's four",
    {
      timeout: BUILD_MS,
    },
    () => {
      const cases = thickReferenceCases();
      expect(
        THICK_CASE_NAMES.map((name) => [name, cases[name].aggregates.map((a) => a.name)]),
      ).toEqual([
        ["venus-92bar", ["flux-s0", "flux-s30", "flux-s60", "flux-s80", "flux-lsfr"]],
        ["venus-58bar", ["flux-s0", "flux-s30", "flux-s60", "flux-s80", "flux-lsfr"]],
        ["titan-class", ["flux-s0", "flux-s30", "flux-s60", "flux-s80"]],
        ["giant-deck", ["flux-s0", "flux-s30", "flux-s60", "flux-s80"]],
      ]);
    },
  );
});

describe("the Venus-class deck", () => {
  it("is P14.T24.g's four modes, mode 2 acid_uv_absorber and the rest H2SO4", () => {
    expect(VENUS_DECK_MODES.map((m) => m.material)).toEqual([
      "H2SO4",
      "acid_uv_absorber",
      "H2SO4",
      "H2SO4",
    ]);
  });

  it("lays each mode across its span of the deck's log-pressure", () => {
    expect([venusDeckPressurePa(0), venusDeckPressurePa(1)]).toEqual([
      VENUS_DECK.basePa,
      expect.closeTo(VENUS_DECK.topPa, 9),
    ]);
  });

  it.each(["venus-92bar", "venus-58bar"] as const)(
    "gives %s the modes' shares of τ(550) 34.5 at the same pressures",
    { timeout: BUILD_MS },
    (name) => {
      const c = thickReferenceCases()[name];
      const sources = c.sources ?? [];
      const tau550 = VENUS_DECK_MODES.map((m) => {
        const k = c.medium.terms.findIndex((t) => t.name === m.name);
        const source = sources[k];
        const term = c.medium.terms[k];
        return source?.kind === "aerosol" && term !== undefined
          ? source.extinction550PerM * columnLengthM(term.density, c.medium.topHeightM)
          : Number.NaN;
      });
      expect(tau550).toEqual(VENUS_DECK_MODES.map((m) => expect.closeTo(m.share * 34.5, 9)));
    },
  );
});

describe("titan-class's medium", () => {
  it("is the thick bake's Titan-class fixture's, term for term", { timeout: BUILD_MS }, () => {
    const { medium } = thickReferenceCases()["titan-class"];
    const fixture = titanClassFixture().medium;
    expect([medium.terms, medium.topHeightM, medium.groundAlbedo]).toEqual([
      fixture.terms,
      fixture.topHeightM,
      fixture.groundAlbedo,
    ]);
  });
});

describe("giant-deck's medium", { timeout: BUILD_MS }, () => {
  it("is saturn-oblate's air with its absorbers and the deck, in that order", () => {
    expect(thickReferenceCases()["giant-deck"].medium.terms.map((t) => t.name)).toEqual([
      "rayleigh",
      "absorber:H2+CH4",
      "pair:H2+He,H2+H2",
      GIANT_DECK.name,
    ]);
  });

  it("scatters as saturn-oblate's air", () => {
    const { medium } = thickReferenceCases()["giant-deck"];
    expect(termOf(medium.terms, "rayleigh").scattering).toEqual(
      termOf(SATURN_OBLATE_REFERENCE.terms, "rayleigh").scattering,
    );
  });

  it("has a deck past the split's optical depth", () => {
    const { medium } = thickReferenceCases()["giant-deck"];
    const deck = termOf(medium.terms, GIANT_DECK.name);
    expect(verticalDepth(deck, medium.topHeightM)[1]).toBeGreaterThan(10);
  });
});

describe("ATMOSPHERE TEST's thick bodies", () => {
  it(
    "reach the render thread whole through the worker's structured clone",
    { timeout: BUILD_MS },
    () => {
      const reply = thickCasesReplyOf(thickReferenceCases, thickCaseCovers);
      expect([reply.kind, structuredClone(reply)]).toEqual(["built", reply]);
    },
  );

  it(
    "are drawn on their cases' own spheres, shaded as their cases' grounds",
    { timeout: BUILD_MS },
    () => {
      const reply = thickCasesReplyOf(thickReferenceCases, thickCaseCovers);
      if (reply.kind !== "built") {
        throw new Error(`the thick cases were not built: ${reply.message}`);
      }
      const drawn = THICK_CASE_NAMES.map((name) => {
        const built = builtOfCase(reply.cases[name], name);
        return [name, builtFigureOf(built), built.medium.groundAlbedo];
      });
      const restated = THICK_CASE_NAMES.map((name) => {
        const fixture = ATMOSPHERE_FIXTURES.find(({ caseName }) => caseName === name);
        const ground = fixture?.groundAlbedo ?? Number.NaN;
        return [name, fixture?.figure, [ground, ground, ground]];
      });
      expect(drawn).toEqual(restated);
    },
  );

  it(
    "draws the Venus-class bodies under their deck's cover, split at its top, and the others under a clear sky (R08.T15.b.2)",
    { timeout: BUILD_MS },
    () => {
      const reply = thickCasesReplyOf(thickReferenceCases, thickCaseCovers);
      if (reply.kind !== "built") {
        throw new Error(`the thick cases were not built: ${reply.message}`);
      }
      const drawn = THICK_CASE_NAMES.map((name) => {
        const { medium, cover } = reply.cases[name];
        return [name, cover === undefined ? null : classifyRegime(medium, cover).kind];
      });
      const venus = reply.cases["venus-92bar"].cover?.decks[0];
      expect([drawn, venus?.opticalDepth550]).toEqual([
        [
          ["venus-92bar", "cloudDeck"],
          ["venus-58bar", "cloudDeck"],
          ["titan-class", null],
          ["giant-deck", null],
        ],
        expect.closeTo(0.99 * VENUS_DECK.opticalDepth550, 12),
      ]);
    },
  );

  it("refuses a deck's cover for a medium without the deck's tabulated modes", () => {
    expect(() => venusDeckCover(EARTH_CASE.medium)).toThrow(
      /no tabulated deck mode aerosol:H2SO4/u,
    );
  });
});
