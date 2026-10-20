import { describe, expect, it } from "vitest";

import registry from "../../../../../../../packages/protocol/fixtures/substances.json" with { type: "json" };
import { type GrainDensityRow, grainDensityKgPerM3 } from "./grainDensity";

/** The relative error within which the fixture holds the sim's grain density. */
const TOLERANCE = 1e-4;

/** The fixture's rows, as the reader reads them. */
const ROWS: ReadonlyArray<GrainDensityRow & { readonly key: string }> = registry.substances;

/** The fixture's row of a key. */
function row(key: string): GrainDensityRow {
  const found = ROWS.find((r) => r.key === key);
  if (found === undefined) {
    throw new Error(`${key} is a row of the fixture`);
  }
  return found;
}

/** The relative difference of a value from the one expected. */
function relative(value: number | undefined, expected: number): number {
  return Math.abs((value ?? Number.NaN) - expected) / expected;
}

describe("grainDensityKgPerM3, by the fixture's rule", () => {
  /** Two pieces with a gap between them, the second the member's last. */
  const PIECES: GrainDensityRow = {
    grain_density_variant: null,
    solid_grain_density: [
      { from_k: 0, to_k: 10, temperatures_k: [0, 4, 10], densities_kg_per_m3: [100, 104, 90] },
      { from_k: 20, to_k: 30, temperatures_k: [20, 30], densities_kg_per_m3: [80, 70] },
    ],
    liquid_grain_density: [
      { from_k: null, to_k: null, temperatures_k: [300], densities_kg_per_m3: [1000] },
    ],
  };

  it("is linear between a piece's nodes", () => {
    expect([1, 7].map((t) => grainDensityKgPerM3(PIECES, "solid", t))).toEqual([101, 97]);
  });

  it("covers a piece's lower bound but not its upper, but for the member's last piece", () => {
    expect([0, 10, 20, 30].map((t) => grainDensityKgPerM3(PIECES, "solid", t))).toEqual([
      100,
      undefined,
      80,
      70,
    ]);
  });

  it("gives none in a gap, below the first piece and above the last", () => {
    expect([15, -1, 30.5].map((t) => grainDensityKgPerM3(PIECES, "solid", t))).toEqual([
      undefined,
      undefined,
      undefined,
    ]);
  });

  it("holds an unbounded piece's one node at every temperature", () => {
    expect([0, 300, 3000].map((t) => grainDensityKgPerM3(PIECES, "liquid", t))).toEqual([
      1000, 1000, 1000,
    ]);
  });

  it("gives none for a variant that is not the row's", () => {
    expect(grainDensityKgPerM3(PIECES, "liquid", 300, "S35")).toBeUndefined();
  });
});

describe("the fixture's grain densities", () => {
  it.each<readonly [string, "solid" | "liquid", number, number]>([
    // IAPWS R10-06's Gibbs function for ice Ih.
    ["H2O", "solid", 100, 933.049],
    // Mangan et al. 2017's quadratic, at a Mars polar cap's frost point.
    ["CO2", "solid", 148, 1623.6],
    // β-N₂ at the triple point, and its first temperature.
    ["N2", "solid", 63.151, 956.82],
    ["N2", "solid", 35.61, 1005.22],
    // CH₄ II, below its transition at 20.4 K.
    ["CH4", "solid", 20, 532.38],
    // The saturated liquid at the normal boiling point (IAPWS-95's 958.37).
    ["H2O", "liquid", 373.15, 958.35],
    // Basalt's room-temperature grain density, held at every temperature.
    ["basalt", "solid", 150, 3010],
  ])("give %s as a %s at %s K the sim's %s kg m⁻³ within 10⁻⁴", (key, phase, t, expected) => {
    expect(relative(grainDensityKgPerM3(row(key), phase, t), expected)).toBeLessThan(TOLERANCE);
  });

  it("give the 75 wt% acid's 1,662.6 kg m⁻³ at Venus's cloud temperatures, of its variant only", () => {
    const acid = row("H2SO4");
    expect([
      grainDensityKgPerM3(acid, "liquid", 250, "75wt%"),
      grainDensityKgPerM3(acid, "liquid", 250, "84.5wt%"),
      grainDensityKgPerM3(acid, "liquid", 250),
      grainDensityKgPerM3(acid, "solid", 200, "75wt%"),
    ]).toEqual([1662.6, undefined, undefined, undefined]);
  });

  it("give none where the sim gives none", () => {
    expect([
      // Supercooled water: the saturated liquid starts at the triple point.
      grainDensityKgPerM3(row("H2O"), "liquid", 260),
      // A rock's melt is not a registry column.
      grainDensityKgPerM3(row("basalt"), "liquid", 1400),
      // Solid argon's density was not read.
      grainDensityKgPerM3(row("Ar"), "solid", 60),
      // β-O₂ (23.14–43.78 K) and β-CO (61.585 K to the triple point) have none.
      grainDensityKgPerM3(row("O2"), "solid", 30),
      grainDensityKgPerM3(row("CO"), "solid", 65),
      grainDensityKgPerM3(row("CO"), "solid", 61.585),
    ]).toEqual([undefined, undefined, undefined, undefined, undefined, undefined]);
  });

  it("are pieces of rising nodes with finite positive densities, rising and apart", () => {
    const broken = ROWS.flatMap((r) =>
      [r.solid_grain_density, r.liquid_grain_density].flatMap((pieces) => {
        if (pieces === null) {
          return [];
        }
        const faults = pieces.flatMap((piece, i) => {
          const ts = piece.temperatures_k;
          const rhos = piece.densities_kg_per_m3;
          const next = pieces[i + 1];
          const ok =
            ts.length > 0 &&
            ts.length === rhos.length &&
            ts.every((t, j) => j === 0 || t > (ts[j - 1] ?? Number.POSITIVE_INFINITY)) &&
            rhos.every((rho) => Number.isFinite(rho) && rho > 0) &&
            (piece.from_k === null || piece.to_k === null || piece.from_k < piece.to_k) &&
            (next === undefined ||
              (piece.to_k !== null && next.from_k !== null && piece.to_k <= next.from_k));
          return ok ? [] : [`${r.key} piece ${i}`];
        });
        return pieces.length === 0 ? [`${r.key}: an empty member`] : faults;
      }),
    );
    expect(broken).toEqual([]);
  });
});
