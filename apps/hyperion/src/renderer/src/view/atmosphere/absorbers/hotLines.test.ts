/**
 * R08.T4.f's hot molecules' tables (plan R08, R08.T4.f; decision-r08-t4ef-storage and
 * decision-r08-t4e-budget): the species drawn and left pending, their grids on E1′'s ladder and E3's
 * pressures, their recorded tiers, their bytes against T4.f's budget, and their curves under a
 * 2,500 K and a 30,000 K sun.
 */
import { beforeAll, describe, expect, it } from "vitest";

import { earthLikeColumn, planckBakeSpectrum } from "../../../test/atmosphereFixtures";
import { inlinePart } from "../../../test/kDistributionFixtures";
import { memoised } from "../../../test/memo";
import {
  ABSORBER_TRANSMITTANCE_FLOOR,
  absorberCurve,
  absorberSpecs,
  type ColumnAbsorberSpec,
} from "../absorbers";
import {
  cellSigmasAt,
  cellTransmittance,
  curveRowsPa,
  handOverK,
  type KDistributionFile,
  type KDistributionTable,
  kDistributionAt,
  loadKDistributions,
  PRESSURE_LATTICE_PA,
  STANDARD_PRESSURES_PA,
  tableAt,
} from "./kDistribution";
import { NO_VISIBLE_ABSORPTION, PENDING_ABSORBERS } from "./transparency";

/** The species R08.T4.f draws from ExoMol's lists (decision-r08-t4ef-storage §5.4's order). */
const DRAWN = ["FeH", "TiO", "CO", "N2"] as const;

/** A drawn species. */
type Drawn = (typeof DRAWN)[number];

/**
 * Each drawn table's first and last node, K: its floor (E2: TiO, FeH and SiO from 1,000 K; N₂ from
 * its stated none's 1,800 K), H₂S's 296 K, or its hand-over; and 4,000 K, R08.T4.f's top, or its
 * list's limit (AYT2's 1,500 K).
 */
const RANGES: Readonly<Record<Drawn, readonly [number, number]>> = {
  FeH: [1_000, 4_000],
  TiO: [1_000, 4_000],
  CO: [300, 4_000],
  N2: [1_800, 4_000],
};

/**
 * Where a drawn table's ladder starts after an off-ladder first node, K: H₂S's, whose first node is
 * AYT2's 296 K and whose ladder runs from the next, 350 K.
 */
const LADDER_FROM: Readonly<Partial<Record<string, number>>> = { H2S: 350 };

/**
 * The drawn species whose tables are `estimated`: CO's, its intensities taking a stated scale;
 * HCN's, its hot bands incomplete in the range; N₂'s, its model carried past its fitted range.
 */
const ESTIMATED: ReadonlyArray<Drawn> = ["CO", "N2"];

/**
 * The drawn species whose hot tables continue R08.T4.d's HITRAN2024 tables, handing over at their
 * first node, 300 K.
 */
const HANDED_OVER: ReadonlyArray<Drawn> = ["CO"];

/**
 * The species R08.T4.f leaves pending, each keeping its entry: VO and hot H₂O, whose lists (HyVO,
 * POKAZATEL) the reducing machine could not fetch; hot CO₂ (UCL-4000, its χ far wings beyond the
 * super-lines); hot NH₃ and N₂O (decision-r08-t4ef-storage §5.4).
 */
const PENDING = ["VO", "H2O", "CO2", "NH3", "N2O"] as const;

/**
 * T4.f's budget, bytes (decision-r08-t4ef-storage §1.3): the 150,000,000-byte cap less the
 * 78,633,763 committed when its projection was made, after R08.T4.e.
 */
const T4F_BUDGET_BYTES = 150_000_000 - 78_633_763;

/** A sidecar with the records the reduction writes beside the grid the client reads. */
interface RecordedSidecar extends KDistributionFile {
  readonly worstNodeError: number;
  readonly worstChainError: number;
  readonly worstQuantisationError: number;
  readonly linearCells: number;
}

/**
 * ExoMol's committed sidecars, by file. Typing them as {@link RecordedSidecar} is a cast at the
 * committed-data boundary, as `kDistribution.ts`' loader's is: the files are the tools' own writing,
 * and each test reads only the fields it names.
 */
const SIDECARS = import.meta.glob<RecordedSidecar>("./exomol/kDistributions/*.json", {
  import: "default",
  eager: true,
});

/** ExoMol's committed sidecars' texts, by file, for their bytes. */
const SIDECAR_TEXTS = import.meta.glob<string>("./exomol/kDistributions/*.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** A drawn species' hot sidecar, or a test failure. */
function sidecarOf(species: string): RecordedSidecar {
  const found = SIDECARS[`./exomol/kDistributions/${species}.json`];
  if (found === undefined) {
    throw new Error(`no committed hot table for ${species}`);
  }
  return found;
}

/** A file's bytes, UTF-8. */
function bytesOf(text: string): number {
  return new TextEncoder().encode(text).length;
}

/** A drawn species' committed bytes, its parts and its sidecar. */
function tableBytes(species: string): number {
  const file = `./exomol/kDistributions/${species}.json`;
  return (
    sidecarOf(species).parts.reduce((sum, p) => sum + p.bytes, 0) +
    bytesOf(SIDECAR_TEXTS[file] ?? "")
  );
}

/** A sidecar's pressure, written to 6 figures, as its index on the lattice, or −1 off it. */
function latticeIndexOf(pressurePa: number): number {
  return PRESSURE_LATTICE_PA.findIndex((s) => Math.abs(s / pressurePa - 1) < 1e-5);
}

/**
 * Whether a table's pressures meet the lattice rule (decision-r08-t4e-unsold's follow-up): on the
 * lattice, holding E3's base nodes (2 a decade below 1 MPa, every node from it), and a midpoint
 * (odd j below 1 MPa) only where both its 4-a-decade neighbours are held.
 */
function meetsLatticeRule(pressuresPa: ReadonlyArray<number>): boolean {
  const held = new Set(pressuresPa.map(latticeIndexOf));
  if (held.has(-1)) {
    return false;
  }
  return PRESSURE_LATTICE_PA.every((_, j) => {
    const base = j >= 40 || j % 4 === 0;
    const midpoint = j < 40 && j % 2 === 1;
    return (
      (!base || held.has(j)) && (!midpoint || !held.has(j) || (held.has(j - 1) && held.has(j + 1)))
    );
  });
}

/** E1′'s nodes over a range, K (decision-r08-t4e-budget §1.4), its top the last. */
function ladder(fromK: number, toK: number): number[] {
  const nodes: number[] = [];
  for (let t = 50; t < toK; t += t < 1_000 ? 50 : Math.floor((0.1 * t) / 50 + 1e-9) * 50) {
    if (t >= fromK) {
      nodes.push(t);
    }
  }
  nodes.push(toK);
  return nodes;
}

describe("the species R08.T4.f draws and leaves pending", () => {
  it("commit a hot table under exomol/ for each drawn species, and none for a pending one", () => {
    expect([
      DRAWN.filter((key) => SIDECARS[`./exomol/kDistributions/${key}.json`] === undefined),
      PENDING.filter(
        (key) =>
          key !== "NH3" &&
          key !== "N2O" &&
          SIDECARS[`./exomol/kDistributions/${key}.json`] !== undefined,
      ),
    ]).toEqual([[], []]);
  });

  it("leave each drawn species no R08.T4.f entry but its parts' (CO's overtones)", () => {
    expect(
      PENDING_ABSORBERS.filter(
        (p) => p.task === "R08.T4.f" && p.part === undefined && DRAWN.some((key) => key === p.key),
      ).map((p) => p.key),
    ).toEqual([]);
  });

  it("keep each pending species' R08.T4.f entry", () => {
    expect(
      PENDING.filter(
        (key) => !PENDING_ABSORBERS.some((p) => p.key === key && p.task === "R08.T4.f"),
      ),
    ).toEqual([]);
  });

  it("stay within T4.f's budget, the cap less what was committed before it", () => {
    const t4f = DRAWN.reduce((sum, key) => sum + tableBytes(key), 0);
    expect(t4f).toBeLessThanOrEqual(T4F_BUDGET_BYTES);
  });
});

describe("the drawn tables' grids", () => {
  it.each(DRAWN)("run %s from its floor or hand-over to its top", (key) => {
    const nodes = sidecarOf(key).temperaturesK;
    expect([nodes[0], nodes.at(-1)]).toEqual(RANGES[key]);
  });

  it.each(DRAWN)("take %s's E1′ nodes from its first, refined where the test asked", (key) => {
    const [first, last] = RANGES[key];
    const from = LADDER_FROM[key] ?? first;
    const own = from === first ? ladder(first, last) : [first, ...ladder(from, last)];
    expect(own.filter((t) => !sidecarOf(key).temperaturesK.includes(t))).toEqual([]);
  });

  it.each(DRAWN)(
    "store %s on E3's thinned pressures and any midpoints, read at the standard rows",
    (key) => {
      const pressures = sidecarOf(key).pressuresPa;
      const standard = pressures.filter(
        (p) => latticeIndexOf(p) >= 40 || latticeIndexOf(p) % 2 === 0,
      );
      expect([
        meetsLatticeRule(pressures),
        standard.length < STANDARD_PRESSURES_PA.length,
        curveRowsPa(pressures).length,
      ]).toEqual([true, true, 30]);
    },
  );

  it("start N₂'s table at its stated none's limit, the belowK contract's table branch", () => {
    const n2 = NO_VISIBLE_ABSORPTION.find((n) => n.species === "N2");
    expect([n2?.belowK, sidecarOf("N2").temperaturesK[0]]).toEqual([1_800, 1_800]);
  });

  it.each(DRAWN)("estimate every node's line shape of %s: its widths the list's default", (key) => {
    expect(sidecarOf(key).measuredShapeTo.every((p) => p === -1)).toBe(true);
  });
});

describe("the drawn tables' recorded tiers (T4.d's)", () => {
  it.each(DRAWN)("pass %s's node tier, 10⁻³", (key) => {
    expect(sidecarOf(key).worstNodeError).toBeLessThanOrEqual(1e-3);
  });

  it.each(DRAWN)("pass %s's 16-bit test, 2 × 10⁻⁴", (key) => {
    expect(sidecarOf(key).worstQuantisationError).toBeLessThanOrEqual(2e-4);
  });

  it.each(DRAWN)("pass %s's chain tier, 2 × 10⁻³", (key) => {
    expect(sidecarOf(key).worstChainError).toBeLessThanOrEqual(2e-3);
  });

  it.each(DRAWN)("keep each of %s's parts under 480,000 bytes", (key) => {
    expect(sidecarOf(key).parts.filter((p) => p.bytes >= 480_000)).toEqual([]);
  });

  it("are measured, but CO's, HCN's and N₂'s, each estimated for its stated reason", () => {
    expect(DRAWN.filter((key) => sidecarOf(key).basis !== "measured")).toEqual(ESTIMATED);
  });
});

/** The drawn species' tables, every one of each loaded once: the hot tables and the temperate ones they continue. */
let tables: ReadonlyArray<KDistributionTable> = [];

beforeAll(async () => {
  tables = await loadKDistributions([...DRAWN], inlinePart);
}, 300_000);

/** A drawn species' hot table, the one under exomol/, or a test failure. */
function hotTable(species: Drawn): KDistributionTable {
  const [first] = RANGES[species];
  const table = tables.find((t) => t.species === species && t.temperaturesK[0] === first);
  if (table === undefined) {
    throw new Error(`${species}'s committed hot table did not load`);
  }
  return table;
}

/** The suns the curves are read under: a 2,500 K and a 30,000 K Planck sun. */
const CURVE_SUNS = [
  ["a 2,500 K", 2_500],
  ["a 30,000 K", 30_000],
] as const;

/**
 * A drawn species' curves under a Planck sun, at its hot table's middle node through an Earth-like
 * column of H₂ and He: their least and largest T_c, and their largest rise along a row.
 */
const curveFigures = memoised(
  ([key, sunK]: readonly [Drawn, number]): {
    readonly least: number;
    readonly most: number;
    readonly rise: number;
  } => {
    const nodes = hotTable(key).temperaturesK;
    const temperatureK = nodes[Math.floor(nodes.length / 2)] ?? Number.NaN;
    const column = earthLikeColumn();
    const specs = absorberSpecs(
      [{ species: key, columnPerM2: 1e24, basePa: 101_325, topPa: 0 }],
      { ...column, temperaturesK: column.temperaturesK.map(() => temperatureK) },
      [],
      [],
      {
        kDistributions: tables,
        bulkMoleFractions: new Map([
          ["H2", 0.86],
          ["He", 0.14],
        ]),
      },
    );
    const spec = specs.specs.find((s): s is ColumnAbsorberSpec => s.kind === "column");
    if (spec === undefined) {
      throw new Error(`no column spec for ${key}`);
    }
    const curve = absorberCurve(spec, planckBakeSpectrum(sunK));
    const columns = curve.logColumns.length;
    let least = Infinity;
    let most = -Infinity;
    let rise = 0;
    for (const channel of curve.transmittance) {
      for (let r = 0; r < curve.logPressures.length; r += 1) {
        for (let j = 0; j < columns; j += 1) {
          const t = channel[r * columns + j] ?? Number.NaN;
          least = Math.min(least, t);
          most = Math.max(most, t);
          rise = Math.max(rise, j > 0 ? t - (channel[r * columns + j - 1] ?? Number.NaN) : 0);
        }
      }
    }
    return { least, most, rise };
  },
  ([key, sunK]) => `${key} ${String(sunK)}`,
);

describe.each(DRAWN)("%s's curves", (key) => {
  it.each(CURVE_SUNS)(
    "lie in [10⁻⁶, 1] under %s Planck sun",
    (_, sunK) => {
      const { least, most } = curveFigures([key, sunK]);
      expect([least >= ABSORBER_TRANSMITTANCE_FLOOR, most <= 1]).toEqual([true, true]);
    },
    60_000,
  );

  it.each(CURVE_SUNS)(
    "are non-increasing in u along each row under %s Planck sun",
    (_, sunK) => {
      expect(curveFigures([key, sunK]).rise).toBe(0);
    },
    60_000,
  );
});

/**
 * A key's two tables at their hand-over, at a pressure node both hold: each shared cell's
 * transmittance through the columns at which the temperate table's lies in [0.2, 0.8], the hot
 * table's against it, the largest difference over the cells, and how many cells and columns were
 * compared.
 */
function handOverDifference(
  species: Drawn,
  pressurePa: number,
): { readonly worst: number; readonly compared: number } {
  const atK = handOverK(tables, species);
  if (atK === undefined) {
    throw new Error(`${species} has no hand-over`);
  }
  const temperate = tableAt(tables, species, atK);
  if (temperate === undefined || temperate === hotTable(species)) {
    throw new Error(`${species}'s temperate table does not serve at its hand-over`);
  }
  const cold = kDistributionAt(temperate, atK);
  const hot = kDistributionAt(hotTable(species), atK);
  const coldSigmas = cellSigmasAt(cold, pressurePa);
  const hotSigmas = cellSigmasAt(hot, pressurePa);
  let worst = 0;
  let compared = 0;
  for (const [c, cell] of cold.cells.entries()) {
    const h = hot.cells.findIndex((x) => x.cellNm === cell.cellNm);
    const hotCell = hot.cells[h];
    if (hotCell === undefined) {
      continue;
    }
    const coldCell = coldSigmas[c];
    const hotCellSigmas = hotSigmas[h];
    if (coldCell === undefined || hotCellSigmas === undefined) {
      throw new Error(`${species}'s ${cell.cellNm} nm cell has no σ at ${pressurePa} Pa`);
    }
    for (let k = 0; k <= 160; k += 1) {
      const columnPerM2 = 10 ** (16 + k / 8);
      const t = cellTransmittance(cell.weights, coldCell, columnPerM2);
      if (t >= 0.2 && t <= 0.8) {
        compared += 1;
        worst = Math.max(
          worst,
          Math.abs(cellTransmittance(hotCell.weights, hotCellSigmas, columnPerM2) - t),
        );
      }
    }
  }
  return { worst, compared };
}

describe.each(HANDED_OVER)("%s's hand-over from R08.T4.d's HITRAN2024 table", (key) => {
  it("lies at the hot table's first node, the temperate table serving at and below it", () => {
    expect([
      handOverK(tables, key),
      tableAt(tables, key, 300) === hotTable(key),
      tableAt(tables, key, 301) === hotTable(key),
    ]).toEqual([RANGES[key][0], false, true]);
  });
});

// CO's steps compound each cell's own ratio to HITRAN2024 (1.40–2.22) with Li2015's default width
// over HITRAN's air widths, its lines saturated where the transmittance is 0.2–0.8 (CO's notes).
describe("the hand-overs' agreement at 300 K (the recorded figures)", () => {
  it("holds CO's two tables within 0.03 of each other at 1 kPa", () => {
    const { worst, compared } = handOverDifference("CO", 1e3);
    expect([worst < 0.03, compared > 0]).toEqual([true, true]);
  });

  it("holds CO's within 0.14 at 0.1 MPa", () => {
    const { worst, compared } = handOverDifference("CO", 1e5);
    expect([worst < 0.14, compared > 0]).toEqual([true, true]);
  });
});
