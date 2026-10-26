/**
 * The generated Earth's medium with its line tables (plan R08, R08.T10.a): its terms, its sun's
 * curves and the labels the bound and today's tables give it; and the cases of six absorber
 * profiles (`decision-r08-cmf-licence.md` §2.3) and of more terms than a medium holds.
 */
import { beforeAll, describe, expect, it } from "vitest";

import {
  detailOf,
  earthClassLinesDetail,
  pastTheCapDetail,
  recordInput,
  sixProfilesDetail,
} from "../../test/atmosphereRecords";
import { inlinePart } from "../../test/kDistributionFixtures";
import { testPatienceMs } from "../../test/patience";
import { earthDetail } from "../../test/planetaryFixture";
import { type KDistributionTable, loadKDistributions } from "./absorbers/kDistribution";
import {
  type AssembledMedium,
  absorberCurveSlots,
  absorberSpecsOf,
  assembleMedium,
  type CurveSlots,
  curveSlotsOf,
  lineSpeciesOf,
  withCurves,
} from "./assemble";
import { checkAbsorberCurves } from "./tables";

/** The line tables the cases here list: the generated Earth's, O₂'s and H₂'s. */
let tables: ReadonlyArray<KDistributionTable> = [];

/** The generated Earth, the wire fixture's, assembled with its tables. */
let earth: AssembledMedium;

/** Its curves under the Sun, whole on one thread. */
let earthCurves: CurveSlots = [];

beforeAll(async () => {
  const input = recordInput(detailOf(earthDetail()));
  tables = await loadKDistributions([...lineSpeciesOf(input), "O2", "H2"], inlinePart);
  earth = assembleMedium(input, { kDistributions: tables });
  if (earth.drawn !== undefined) {
    earthCurves = absorberCurveSlots(earth.drawn);
  }
}, testPatienceMs());

describe("the generated Earth (the wire fixture's) with its line tables", () => {
  it("builds the molecular term, its gases' lines on one profile and its water's continuum", () => {
    // N₂ has nothing visible to draw below 1,800 K; Ar, listed for the pairs' partners, no optics.
    expect(earth.medium?.terms.map((term) => term.name)).toEqual([
      "rayleigh",
      "absorber:H2O+Ar+CO2",
      "pair:H2O+H2O,H2O+air",
    ]);
  });

  it("carries the column's pressure for its line absorbers", () => {
    expect(earth.medium?.pressureBar).toBeDefined();
  });

  it("is APPROXIMATE for its water: lines CO2 broadens with no width on file, and its continuum below 567 nm", () => {
    expect([earth.labels, earth.reasons]).toEqual([
      ["aerosolsNotYetModelled", "atmosphereApproximate"],
      [
        "H2O: its absorption estimated, not measured",
        "H2O–H2O pairs: their absorption estimated, not measured",
        "H2O–air pairs: their absorption estimated, not measured",
      ],
    ]);
  });

  it("names CO2 negligible, its held χ and its fixed-g mix under the bound", () => {
    expect(earth.negligible).toEqual(["CO2"]);
  });

  it("takes one light slot of curves, one a term, which the tables accept", () => {
    const curved = withCurves(earth, earthCurves);
    if (curved.medium === undefined) {
      throw new Error("the generated Earth draws no medium");
    }
    expect([curved.drawn.curves.length, curved.drawn.curves[0]?.length]).toEqual([1, 2]);
    expect(() => {
      checkAbsorberCurves(curved.medium, curved.drawn.curves);
    }).not.toThrow();
  });

  it("joins a pool's curves, one list an absorber by light, into the same slots", () => {
    const perAbsorber = (earthCurves[0] ?? []).map((channels) => [{ channels }]);
    expect(curveSlotsOf(perAbsorber, 1)).toEqual(earthCurves);
  });

  it("gives a curve job on another worker the same specs, with no aerosols' optics", () => {
    const input = recordInput(detailOf(earthDetail()));
    expect(absorberSpecsOf(input, { kDistributions: tables }).specs).toEqual(
      earth.drawn?.absorbers,
    );
  });
});

describe("an Earth-class record with O2 among its gases", () => {
  it("draws O2's lines with the other gases' and its collision pairs with the water's continuum", () => {
    const result = assembleMedium(recordInput(earthClassLinesDetail()), { kDistributions: tables });
    expect(result.medium?.terms.map((term) => term.name)).toEqual([
      "rayleigh",
      "absorber:O2+H2O+Ar+CO2",
      "pair:O2+O2,O2+N2,O2+CO2,H2O+H2O,H2O+air",
    ]);
  });
});

// With no line tables, so that the record's six profiles are its cross-sections' and its pairs'.
describe("six absorber profiles (decision-r08-cmf-licence.md §2.3)", () => {
  it("keep the five of greatest depth and leave out H2's pairs, the least, saying so", () => {
    const result = assembleMedium(recordInput(sixProfilesDetail()));
    expect([
      result.drawn?.absorbers.map((spec) => spec.name),
      result.labels.includes("atmosphereApproximate"),
      result.reasons.filter((reason) => reason.includes("left out")),
    ]).toEqual([
      ["absorber:O3", "absorber:CH4", "absorber:SO2", "pair:H2O+H2O,H2O+air", "pair:O2+O2,O2+N2"],
      true,
      [
        "H2–He pairs: their absorption left out, the console draws no more layers of absorbing gas",
        "H2–H2 pairs: their absorption left out, the console draws no more layers of absorbing gas",
      ],
    ]);
  });
});

describe("more terms than a medium holds", () => {
  it("leaves out the term of least depth, here the thinnest haze, and says so", () => {
    const result = assembleMedium(recordInput(pastTheCapDetail()));
    expect([
      result.medium?.terms.length,
      result.reasons.findLast((reason) => reason.includes("draws no more layers")),
    ]).toEqual([8, "liquid H2SO4 particles left out: the console draws no more layers"]);
  });

  it("gives a curve job the specs the medium draws, the term limit applied", () => {
    const input = recordInput(pastTheCapDetail());
    expect(absorberSpecsOf(input).specs).toEqual(assembleMedium(input).drawn?.absorbers);
  });
});
