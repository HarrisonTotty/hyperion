import { describe, expect, it } from "vitest";

import { ABSORBER_CROSS_SECTIONS } from "../absorbers";
import atomicBounds from "./atomicBounds.json" with { type: "json" };
import exomolBounds from "./exomol/bounds.json" with { type: "json" };
import { committedLineSpecies } from "./kDistribution";
import {
  ABSORBER_BOUNDS,
  type AbsorberBound,
  type AbsorberBoundsFile,
  isTransparent,
  NO_VISIBLE_ABSORPTION,
  PENDING_ABSORBERS,
  PENDING_CELL_COUNT,
  readAbsorberBounds,
} from "./transparency";

/** A bound's cell means at each node, in its kind's unit. */
function cellMeans(bound: AbsorberBound): ReadonlyArray<Float64Array> {
  return bound.kind === "species" ? bound.cellMeansM2 : bound.cellMeansM5;
}

/** A pending key's cell means, or `undefined` if it is not pending. */
function pendingBound(key: string): AbsorberBound | undefined {
  return PENDING_ABSORBERS.find((p) => p.key === key)?.bound;
}

describe("the stated transparency", () => {
  it("names He below 5,000 K, N₂ below 1,800 K, e⁻ at every temperature and seven ions by their lines' share, and nothing else", () => {
    expect(NO_VISIBLE_ABSORPTION.map((n) => [n.species, n.reason, n.belowK])).toEqual([
      ["He", "bandsOutsideRange", 5_000],
      ["N2", "bandsOutsideRange", 1_800],
      ["e-", "noBoundStates", undefined],
      ["Na+", "underEquivalentWidthBound", 5_000],
      ["Mg+", "underEquivalentWidthBound", 4_600],
      ["Al+", "underEquivalentWidthBound", 5_000],
      ["Si+", "underEquivalentWidthBound", 4_600],
      ["K+", "underEquivalentWidthBound", 5_000],
      ["Ni+", "underEquivalentWidthBound", 4_600],
      ["Zn+", "underEquivalentWidthBound", 5_000],
    ]);
  });

  it("pends each ion stated none by its lines' share from its own limit, in atomicBounds.json", () => {
    const atomic = readAbsorberBounds(atomicBounds, "atomicBounds.json");
    const byShare = NO_VISIBLE_ABSORPTION.filter((n) => n.reason === "underEquivalentWidthBound");
    expect(
      byShare.map((n) => [n.species, atomic.find((b) => b.key === n.species)?.temperaturesK[0]]),
    ).toEqual(byShare.map((n) => [n.species, n.belowK]));
  });

  it("holds below a species' limit and not above it", () => {
    expect([
      isTransparent("He", 4_000),
      isTransparent("He", 6_000),
      isTransparent("N2", 300),
      isTransparent("N2", 2_000),
      isTransparent("e-", 1e4),
      isTransparent("CO2", 300),
    ]).toEqual([true, false, true, false, true, false]);
  });

  it("states nothing the client holds cross-sections for", () => {
    expect(
      NO_VISIBLE_ABSORPTION.filter((n) =>
        ABSORBER_CROSS_SECTIONS.some((t) => t.species === n.species),
      ),
    ).toEqual([]);
  });

  it("gives each statement its evidence", () => {
    for (const entry of NO_VISIBLE_ABSORPTION) {
      expect(entry.evidence.length).toBeGreaterThan(100);
    }
  });

  it("lists a temperature-limited species as pending from its limit", () => {
    for (const entry of NO_VISIBLE_ABSORPTION.filter((n) => n.belowK !== undefined)) {
      const pending = PENDING_ABSORBERS.find((p) => p.key === entry.species);
      expect(pending?.bound.temperaturesK[0]).toBeGreaterThanOrEqual(entry.belowK ?? Number.NaN);
    }
  });
});

describe("the pending absorbers", () => {
  it("leave R08.T4.d's and R08.T4.g's lists once a species' line table is committed", () => {
    const tables = committedLineSpecies();
    expect(
      PENDING_ABSORBERS.filter(
        (p) => (p.task === "R08.T4.d" || p.task === "R08.T4.g") && tables.includes(p.key),
      ).map((p) => p.key),
    ).toEqual([]);
  });

  it("hold no R08.T4.g entry now that NH₃'s and N₂O's tables are committed", () => {
    expect([
      PENDING_ABSORBERS.filter((p) => p.task === "R08.T4.g").map((p) => p.key),
      ["NH3", "N2O"].filter((key) => committedLineSpecies().includes(key)),
    ]).toEqual([[], ["NH3", "N2O"]]);
  });

  it("hold hot NH₃ above C2018's 300 K for R08.T4.f, at the table's 300 K row, estimated", () => {
    const hot = PENDING_ABSORBERS.find((p) => p.key === "NH3" && p.task === "R08.T4.f");
    expect([hot?.bound.basis, hot?.bound.temperaturesK]).toEqual(["estimated", [300]]);
  });

  it("each carry cell means for their own key, task and part", () => {
    for (const pending of PENDING_ABSORBERS) {
      expect([pending.bound.key, pending.bound.task, pending.bound.part]).toEqual([
        pending.key,
        pending.task,
        pending.part,
      ]);
    }
  });

  it("list each key once a task and part", () => {
    const listings = PENDING_ABSORBERS.map((p) => `${p.key} ${p.task} ${p.part ?? ""}`);
    expect(new Set(listings).size).toBe(listings.length);
  });

  it("name no transparent species but above its limit", () => {
    const always = PENDING_ABSORBERS.filter((p) =>
      NO_VISIBLE_ABSORPTION.some((n) => n.species === p.key && n.belowK === undefined),
    );
    expect(always).toEqual([]);
  });

  it("give a pair's values in m⁵ and a species' in m²", () => {
    // A pair's key is two members' joined by +; an ion's ends in its sign (Ca+).
    for (const { key, bound } of PENDING_ABSORBERS) {
      expect(bound.kind).toBe(/^[^+]+\+[^+]+$/u.test(key) ? "pair" : "species");
    }
  });

  it("hold no values beyond the listings", () => {
    for (const bound of ABSORBER_BOUNDS) {
      expect(PENDING_ABSORBERS.some((p) => p.bound === bound)).toBe(true);
    }
  });

  it("hold 381 cells a node, each rounded up to 3 significant figures", () => {
    for (const bound of ABSORBER_BOUNDS) {
      for (const row of cellMeans(bound)) {
        expect(row.length).toBe(PENDING_CELL_COUNT);
        expect(row.filter((v) => Number(v.toPrecision(3)) !== v)).toEqual(new Float64Array(0));
      }
    }
  });

  it("hold K's far wing as pairs with H₂ and He, estimated, beside K's own lines per atom", () => {
    expect(
      ["K", "K+H2", "K+He"].map((key) => {
        const bound = pendingBound(key);
        return [bound?.kind, bound?.basis, bound?.task];
      }),
    ).toEqual([
      ["species", "measured", "R08.T4.e"],
      ["pair", "estimated", "R08.T4.e"],
      ["pair", "estimated", "R08.T4.e"],
    ]);
  });

  it("give K's wing pairs their largest value in the range at 760 nm, nearest the doublet", () => {
    for (const key of ["K+H2", "K+He"]) {
      const bound = pendingBound(key);
      const row =
        bound === undefined ? new Float64Array(0) : (cellMeans(bound)[1] ?? new Float64Array(0));
      expect(row.indexOf(Math.max(...row)) + 380).toBe(760);
    }
  });

  it("put H₂–H₂ among the drawn pairs, no longer pending", () => {
    expect(PENDING_ABSORBERS.some((p) => p.key === "H2+H2")).toBe(false);
  });

  it("keep ExoMol's values in their own file, under its licence", () => {
    expect(exomolBounds.description).toMatch(/CC BY-SA 4\.0/u);
    for (const bound of readAbsorberBounds(exomolBounds, "exomol/bounds.json")) {
      expect(bound.source).toMatch(/^ExoMol line list/u);
    }
  });
});

/**
 * Every JSON file under `absorbers/`, as text (`?raw`): the bounds files, the k-distribution
 * sidecars and the reference tables.
 */
const ABSORBER_JSON = import.meta.glob<string>("./**/*.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

describe("the absorbers' JSON files (R08.T4.h)", () => {
  // The 500 kB hook checks only the files a commit adds, so a file that grows passes it unseen.
  it("are found, the three bounds files among them", () => {
    expect(
      ["./bounds.json", "./atomicBounds.json", "./exomol/bounds.json"].filter(
        (file) => ABSORBER_JSON[file] === undefined,
      ),
    ).toEqual([]);
  });

  it.each(Object.entries(ABSORBER_JSON))("keep %s under 500,000 bytes", (_, text) => {
    expect(new TextEncoder().encode(text).length).toBeLessThan(500_000);
  });
});

/** A row of 381 cell means: `value` in the cell centred on 600 nm, 0 elsewhere. */
function cellRow(value: number): ReadonlyArray<number> {
  return Array.from({ length: PENDING_CELL_COUNT }, (_, c) => (c === 220 ? value : 0));
}

/** A minimal valid bounds file: one species at two nodes. */
const TOY: AbsorberBoundsFile = {
  bounds: [
    {
      key: "X",
      kind: "species",
      task: "R08.T4.d",
      basis: "measured",
      temperaturesK: [296, 500],
      cellMeansM2: [cellRow(1e-27), cellRow(2e-27)],
      source: "a test",
      method: "none",
    },
  ],
};

/** {@link TOY} with its one entry changed. */
function toyWith(change: Partial<AbsorberBoundsFile["bounds"][number]>): AbsorberBoundsFile {
  const [entry] = TOY.bounds;
  if (entry === undefined) {
    throw new Error("the toy file has an entry");
  }
  return { bounds: [{ ...entry, ...change }] };
}

describe("reading a bounds file", () => {
  it("reads a valid file", () => {
    expect(readAbsorberBounds(TOY, "toy").map((b) => [b.key, b.kind, b.basis])).toEqual([
      ["X", "species", "measured"],
    ]);
  });

  it("reads each node's cells in order", () => {
    const [bound] = readAbsorberBounds(TOY, "toy");
    expect(bound === undefined ? [] : cellMeans(bound).map((r) => r[220])).toEqual([1e-27, 2e-27]);
  });

  it.each([
    ["an unknown task", toyWith({ task: "R08.T4.z" }), /task or basis/u],
    ["an unknown basis", toyWith({ basis: "guessed" }), /task or basis/u],
    ["temperatures that do not ascend", toyWith({ temperaturesK: [500, 296] }), /ascend/u],
    [
      "a node with no value above 0",
      toyWith({ cellMeansM2: [cellRow(0), cellRow(1e-27)] }),
      /one above 0/u,
    ],
    [
      "a negative cell mean",
      toyWith({ cellMeansM2: [cellRow(-1e-27), cellRow(1e-27)] }),
      /at least 0/u,
    ],
    [
      "a node short of its cells",
      toyWith({ cellMeansM2: [cellRow(1e-27).slice(1), cellRow(1e-27)] }),
      /381 cell means/u,
    ],
    ["a node missing", toyWith({ cellMeansM2: [cellRow(1e-27)] }), /381 cell means/u],
    ["a pair without m⁵ values", toyWith({ kind: "pair" }), /381 cell means/u],
    ["an unknown kind", toyWith({ kind: "ion" }), /neither species nor pair/u],
  ])("refuses %s", (_, file, message) => {
    expect(() => readAbsorberBounds(file, "toy")).toThrow(message);
  });

  it("refuses a key and task listed twice", () => {
    expect(() => readAbsorberBounds({ bounds: [...TOY.bounds, ...TOY.bounds] }, "toy")).toThrow(
      /listed twice/u,
    );
  });

  it("keeps two parts of one key and task apart, each read with its part", () => {
    const parts = readAbsorberBounds(
      { bounds: [...TOY.bounds, ...toyWith({ part: "overtones" }).bounds] },
      "toy",
    );
    expect(parts.map((b) => b.part)).toEqual([undefined, "overtones"]);
  });
});
