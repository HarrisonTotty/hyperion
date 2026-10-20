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

  it("covers a piece's lower bound", () => {
    expect([0, 20].map((t) => grainDensityKgPerM3(PIECES, "solid", t))).toEqual([100, 80]);
  });

  it("does not cover a piece's upper bound", () => {
    expect(grainDensityKgPerM3(PIECES, "solid", 10)).toBeUndefined();
  });

  it("covers the member's last piece's upper bound", () => {
    expect(grainDensityKgPerM3(PIECES, "solid", 30)).toBe(70);
  });

  it("holds a bounded piece's end nodes' densities beyond them, inside its bounds", () => {
    const inner: GrainDensityRow = {
      grain_density_variant: null,
      solid_grain_density: [
        { from_k: 0, to_k: 10, temperatures_k: [2, 8], densities_kg_per_m3: [100, 94] },
      ],
      liquid_grain_density: null,
    };
    expect([1, 9].map((t) => grainDensityKgPerM3(inner, "solid", t))).toEqual([100, 94]);
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

  /**
   * The densities the sim's mirror of the rule (`substance_golden.rs`'s `mirror_density`) gives
   * between two nodes, and a fixed row's, written as Rust's `{:?}` prints them, so that the two
   * evaluators cannot split silently: `substance_fixture_grain_density_is_pinned_between_nodes`
   * pins the same literals.
   */
  it.each<
    readonly [
      label: string,
      key: string,
      phase: "solid" | "liquid",
      temperatureK: number,
      densityKgPerM3: number,
    ]
  >([
    ["ice Ih between its nodes at 87.15 and 110.18 K", "H2O", "solid", 100, 932.996995807952],
    [
      "liquid water between its nodes at 295.04 and 301.51 K",
      "H2O",
      "liquid",
      300,
      996.4737200950051,
    ],
    ["basalt's one node, held", "basalt", "solid", 150, 3010],
  ])("give %s the sim's mirror's density to the bit", (_label, key, phase, t, expected) => {
    expect(Object.is(grainDensityKgPerM3(row(key), phase, t), expected)).toBe(true);
  });

  it("give the 75 wt% acid's 1,662.6 kg m⁻³ at Venus's cloud temperatures, of its variant only", () => {
    const acid = row("H2SO4");
    expect([
      grainDensityKgPerM3(acid, "liquid", 250, "75wt%"),
      grainDensityKgPerM3(acid, "liquid", 250, "84.5wt%"),
      grainDensityKgPerM3(acid, "liquid", 250),
    ]).toEqual([1662.6, undefined, undefined]);
  });

  it("give the 75 wt% acid no solid density", () => {
    expect(grainDensityKgPerM3(row("H2SO4"), "solid", 200, "75wt%")).toBeUndefined();
  });

  it.each<readonly [label: string, key: string, phase: "solid" | "liquid", temperatureK: number]>([
    // The saturated liquid starts at the triple point.
    ["supercooled water", "H2O", "liquid", 260],
    // A rock's melt is not a registry column.
    ["a rock's melt", "basalt", "liquid", 1400],
    // Solid argon's density was not read.
    ["solid argon", "Ar", "solid", 60],
    // β-O₂ runs from 23.14 to 43.78 K.
    ["β-O₂", "O2", "solid", 30],
    // β-CO runs from 61.585 K to the triple point.
    ["β-CO", "CO", "solid", 65],
    ["β-CO at its start", "CO", "solid", 61.585],
  ])("give none for %s, as the sim gives none", (_label, key, phase, t) => {
    expect(grainDensityKgPerM3(row(key), phase, t)).toBeUndefined();
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
