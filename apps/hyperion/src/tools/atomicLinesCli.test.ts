import { describe, expect, it } from "vitest";

import atomicBounds from "../renderer/src/view/atmosphere/absorbers/atomicBounds.json" with { type: "json" };
import { type AtomicStage, type KuruczLine } from "./atomicLines";
import {
  ASD_ZN2,
  ATOMIC_ANSWERS,
  ATOMIC_BOUNDS_DESCRIPTION,
  ATOMIC_NODES_K,
  atomicAnswers,
  atomicBoundsText,
  belowKOf,
  HOT_NODES_K,
  KURUCZ_LISTS,
  type LargestShare,
  largestShares,
  nodesFrom,
  stageCellMeans,
  SHARE_TEMPERATURES_K,
  ZN2_BOUND_A_PER_S,
  ZN2_LEVELS_PER_CM,
  ZN2_TRANSITION_PROBABILITIES,
  zincIonLines,
} from "./atomicLinesCli";
import { lineAt, toySahaData } from "./atomicLinesToys";

/** Shares at {@link SHARE_TEMPERATURES_K}, the largest of each given in order. */
function sharesOf(values: ReadonlyArray<number>): LargestShare[] {
  return SHARE_TEMPERATURES_K.map((temperatureK, i) => ({
    temperatureK,
    share: values[i] ?? 0,
    bin: 0,
    columnPerM2: 1,
    leading: [],
    linesSummed: 1,
  }));
}

describe("the third reason's rule (ruled 2026-10-19)", () => {
  it("states a stage none below 5,000 K when it stays under 10⁻³ at all seven temperatures", () => {
    expect(belowKOf(sharesOf([0, 0, 0, 0, 1e-6, 5e-4, 9.9e-4]))).toBe(5_000);
  });

  it("states it none below 4,600 K when it passes 10⁻³ only at 5,000 K", () => {
    expect(belowKOf(sharesOf([0, 0, 0, 0, 1e-6, 9.9e-4, 2e-3]))).toBe(4_600);
  });

  it("pends a stage that reaches 10⁻³ at any of 2,000–4,600 K", () => {
    expect(belowKOf(sharesOf([0, 0, 0, 1e-3, 0, 0, 0]))).toBeUndefined();
  });

  it("gives a stated none's entry its limit and He's nodes above it", () => {
    expect([nodesFrom(5_000), nodesFrom(4_600)]).toEqual([HOT_NODES_K, [4_600, ...HOT_NODES_K]]);
  });
});

describe("the answers' stages", () => {
  it("cover R08.T4.h's rows as R08.T4.e leaves them, Zn drawn, and O, each with a Kurucz list", () => {
    expect([
      ATOMIC_ANSWERS.map((a) => a.key).toSorted(),
      ATOMIC_ANSWERS.every((a) => KURUCZ_LISTS.some((l) => l.key === a.key)),
    ]).toEqual([
      [
        "Al",
        "Al+",
        "Cr+",
        "K+",
        "Mg+",
        "Mn",
        "Mn+",
        "Na+",
        "Ni",
        "Ni+",
        "O",
        "Si+",
        "V",
        "V+",
        "Zn+",
      ],
      true,
    ]);
  });

  it("name each list by its spectrum's code, gfZZII.all", () => {
    for (const list of KURUCZ_LISTS) {
      const [z, stage] = [list.name.slice(2, 4), list.name.slice(4, 6)];
      expect([Number(stage), list.sha256.length]).toEqual([list.stage, 64]);
      // O I's gf0800.all the lowest, R08.T4.e's.
      expect(Number(z)).toBeGreaterThanOrEqual(8);
    }
  });
});

/** A toy of ASD's CSV, its fields as ASD writes them: one Zn II line of each source. */
const ASD_TOY = [
  "obs_wl_air(nm),ritz_wl_air(nm),intens,Aki(s^-1),Acc,Ei(eV),Ek(eV),conf_i,term_i,J_i,conf_k,term_k,J_k,g_i,g_k,Type,",
  '"=""407.814""","=""407.813+""","=""40""","=""8.9e+06""",C,"=""12.56777""","=""15.60713""","=""3d10.5p""","=""2P*""","=""1/2""","=""3d10.7s""","=""2S""","=""1/2""",2,2,,',
  '"=""589.43607""","=""589.437+""","=""500""","=""""",,"=""6.010878""","=""8.113729""","=""3d10.4p""","=""2P*""","=""1/2""","=""3d9.4s2""","=""2D""","=""3/2""",2,4,,',
  '"=""747.87691""","=""747.882+""","=""200""","=""""",,"=""6.119245""","=""7.776593""","=""3d10.4p""","=""2P*""","=""3/2""","=""3d9.4s2""","=""2D""","=""5/2""",4,6,,',
  '"=""758.84648""","=""758.85+""","=""300""","=""5.4e+07""",D+,"=""10.964809""","=""12.59821""","=""3d10.5s""","=""2S""","=""1/2""","=""3d10.5p""","=""2P*""","=""3/2""",2,4,,',
].join("\n");

describe("Zn⁺'s lines (§4.1)", () => {
  const [mayo, kisielius, bound, nandy] = zincIonLines(ASD_TOY);

  it("take Martin and Kaufman's levels, the vacuum wavelength their difference", () => {
    const levels = ZN2_LEVELS_PER_CM;
    expect([bound?.lowerPerCm, bound?.upperPerCm, bound?.vacuumNm]).toEqual([
      levels["3d10.4p 2P* 3/2"],
      levels["3d9.4s2 2D 5/2"],
      expect.closeTo(1e7 / (62_722.45 - 49_355.04), 9),
    ]);
  });

  it("take ASD's A where its primary paper is closed: gf = g_u A λ² ÷ 8π² r_e c", () => {
    // 1.499 × 10⁻¹⁴ λ² g_u A with λ in nm: the usual conversion, to its four figures.
    const nm = 1e7 / (125_880.0 - 101_365.9);
    expect(10 ** (mayo?.logGf ?? Number.NaN) / (1.4992e-14 * nm * nm * 2 * 8.9e6)).toBeCloseTo(
      1,
      4,
    );
  });

  it("take Kisielius et al.'s line strength for 589.60 nm: gf = 303.76 S ÷ λ in Å", () => {
    const angstrom = 1e8 / (65_441.64 - 48_481.0);
    expect(10 ** (kisielius?.logGf ?? Number.NaN) / ((303.756 * 1.196e-2) / angstrom)).toBeCloseTo(
      1,
      4,
    );
  });

  it("take the bound where no A is found, and Nandy and Sahoo's computed A where ASD cites them", () => {
    const nm748 = 1e7 / (62_722.45 - 49_355.04);
    const nm759 = 1e7 / (101_611.4 - 88_437.15);
    expect([
      10 ** (bound?.logGf ?? Number.NaN) / (1.4992e-14 * nm748 ** 2 * 6 * ZN2_BOUND_A_PER_S),
      10 ** (nandy?.logGf ?? Number.NaN) / (1.4992e-14 * nm759 ** 2 * 4 * 5.36e7),
      [mayo?.reference, kisielius?.reference, bound?.reference, nandy?.reference],
    ]).toEqual([expect.closeTo(1, 4), expect.closeTo(1, 4), ["ASD", "K15", "bound", "NS15"]]);
  });

  it("give every one of ASD's 17 transitions a source, and none beyond them", () => {
    expect(Object.keys(ZN2_TRANSITION_PROBABILITIES)).toHaveLength(17);
  });

  it("refuse a line the tables do not hold", () => {
    const [header = "", row = ""] = ASD_TOY.split("\n");
    expect(() => zincIonLines(`${header}\n${row.replace("3d10.7s", "3d10.9s")}`)).toThrow(
      "not in the tool's tables",
    );
  });
});

describe("atomicBounds.json", () => {
  it("is in bounds.json's layout: two-space JSON, each node's cells on one line", () => {
    const entry = {
      key: "X",
      kind: "species",
      task: "R08.T4.e",
      basis: "measured",
      temperaturesK: [300, 1_000],
      source: "a test",
      method: "none",
      cellMeansM2: [
        [0, 1.5e-30],
        [0, 2e-30],
      ],
    } as const;
    const text = atomicBoundsText([entry]);
    expect([JSON.parse(text), text.split("\n").filter((l) => l.trim().startsWith("[0,"))]).toEqual([
      { description: ATOMIC_BOUNDS_DESCRIPTION, bounds: [entry] },
      ["        [0, 1.5e-30],", "        [0, 2e-30]"],
    ]);
  });

  it("holds an entry for each answer but the stages drawn from their limit by R08.T4.e's tables", () => {
    expect(atomicBounds.bounds.map((b) => b.key).toSorted()).toEqual(
      ATOMIC_ANSWERS.filter((a) => a.answer !== "drawnFromFloor")
        .map((a) => a.key)
        .toSorted(),
    );
  });

  it("names in each entry's source its input's SHA-256: its Kurucz list's, or ASD's for Zn⁺", () => {
    const unnamed = atomicBounds.bounds.filter((entry) => {
      const sha256 =
        entry.key === "Zn+"
          ? ASD_ZN2.sha256
          : KURUCZ_LISTS.find((l) => l.key === entry.key)?.sha256;
      return sha256 === undefined || !entry.source.includes(sha256);
    });
    expect(unnamed.map((e) => e.key)).toEqual([]);
  });

  it("puts each entry at T4.c's nodes or at a limit's", () => {
    const allowed = new Set(
      [ATOMIC_NODES_K, nodesFrom(5_000), nodesFrom(4_600)].map((n) => n.join()),
    );
    expect(
      atomicBounds.bounds.filter((e) => !allowed.has(e.temperaturesK.join())).map((e) => e.key),
    ).toEqual([]);
  });
});

/** Every list's stage given one line at 500 nm from 2,000 cm⁻¹ of a log gf, Mg⁺'s and Ni⁺'s their own. */
function linesBy(
  logGf: number,
  own: Readonly<Record<string, number>> = {},
): (list: AtomicStage) => ReadonlyArray<KuruczLine> {
  return (list) => [lineAt(500, own[list.key] ?? logGf, 2_000)];
}

describe("the answers on toy data", () => {
  const data = toySahaData();

  it("take each stage's largest share at the seven temperatures, its bin and column", () => {
    const shares = largestShares(data, { key: "Na+", symbol: "Na", stage: 1 }, [
      lineAt(500, -12, 2_000),
    ]);
    expect([shares.map((s) => s.temperatureK), shares.every((s) => s.bin === 4)]).toEqual([
      [...SHARE_TEMPERATURES_K],
      true,
    ]);
  });

  it("give a stage's cell means at its nodes, rounded up, and name its largest cell", () => {
    const { rows, largest } = stageCellMeans(
      data,
      { key: "Na+", symbol: "Na", stage: 1 },
      [lineAt(500, -1, 2_000)],
      ATOMIC_NODES_K,
    );
    expect([rows.length, largest.cell, largest.temperatureK, largest.line?.vacuumNm]).toEqual([
      ATOMIC_NODES_K.length,
      120,
      5_000,
      500,
    ]);
  });

  it("state the weak-lined stages none below 5,000 K and give the seven deferred stages T4.c's nodes", () => {
    const answers = atomicAnswers(data, linesBy(-12));
    const byKey = new Map(answers.map((a) => [a.key, a]));
    expect([
      ["Na+", "Al+", "Si+", "K+", "Ni+", "Zn+", "Mg+", "O"].map((k) => byKey.get(k)?.belowK),
      ["Al", "V", "Mn", "Ni", "V+", "Cr+", "Mn+"].map((k) => byKey.get(k)?.entry?.temperaturesK),
      ["Mg+", "Si+"].map((k) => byKey.get(k)?.entry),
      byKey.get("O")?.entry?.temperaturesK,
    ]).toEqual([
      Array(8).fill(5_000),
      Array(7).fill(ATOMIC_NODES_K),
      [undefined, undefined],
      nodesFrom(5_000),
    ]);
  });

  it("mark Zn⁺'s entry estimated, on its bound, and every other measured, each for R08.T4.e", () => {
    const answers = atomicAnswers(data, linesBy(-12));
    expect(
      answers
        .filter((a) => a.entry !== undefined)
        .map((a) => [a.key, a.entry?.basis, a.entry?.task])
        .filter(([, b]) => b !== "measured"),
    ).toEqual([["Zn+", "estimated", "R08.T4.e"]]);
  });

  it("pend a strong-lined ion at T4.c's nodes, and write no entry for Mg⁺, drawn by its table", () => {
    const answers = atomicAnswers(data, linesBy(-12, { "Ni+": 2, "Mg+": 2 }));
    const byKey = new Map(answers.map((a) => [a.key, a]));
    expect([
      byKey.get("Ni+")?.belowK,
      byKey.get("Ni+")?.entry?.temperaturesK,
      byKey.get("Mg+")?.entry,
    ]).toEqual([undefined, ATOMIC_NODES_K, undefined]);
  });
});
