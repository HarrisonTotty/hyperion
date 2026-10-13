import { beforeAll, describe, expect, it } from "vitest";

import { fourAbsorberMedium } from "../../test/atmosphereFixtures";
import type { AbsorberCurve } from "./absorbers";
import {
  EARTH_COLUMN,
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
  WGS84_LEVEL_SPHEROID,
} from "./earth";
import { tableRadiusM } from "./hillaire";
import {
  type AtmosphereMedium,
  densityAt,
  layerEdgesM,
  type MediumTerm,
  type TabulatedDensity,
  termNamed,
} from "./medium";
import {
  distanceToTopM,
  localRadiusM,
  transmittanceUvToRMu,
  unitFromTexelUv,
} from "./opticalDepth";
import transmittanceWgsl from "./shaders/transmittance.wgsl?raw";
import {
  type AbsorberCurveSlots,
  densityTableLevelsM,
  kernelMedium,
  MAX_TERMS,
  NO_ABSORBER_CURVES,
  packLayerEdges,
  resampledDensity,
  TRANSMITTANCE_SAMPLES,
  TRANSMITTANCE_SIZE,
  TRANSMITTANCE_STEPS_A_PIECE,
} from "./tables";
import {
  curveOpticalDepth,
  kernelRayToTop,
  shellCrossingsM,
  transmittancePieces,
  transmittanceTablesTwin,
  type TwinTransmittance,
} from "./tablesCpu";

/** R₁ = (2a + c) ÷ 3 on WGS 84: the tables' ground. */
const R1 = tableRadiusM(WGS84_LEVEL_SPHEROID);

/** The steps of the table's own converged value (`decision-r08-t6b-floor.md` §9.2). */
const CONVERGED_STEPS = 4_096;

/**
 * The greatest ozone column a transmittance texel's ray crosses on the measured Earth and on the
 * slab Earth, in vertical columns (R08.T6.h), as measured on 2026-10-13.
 */
const MEASURED_GREATEST_COLUMNS = 41.2;
const SLAB_GREATEST_COLUMNS = 81.6;

/** The bound in optical depth the table is held to: max(0.005, 10⁻³ τ) (§9.2). */
function bound(tau: number): number {
  return Math.max(0.005, 1e-3 * tau);
}

/**
 * T6.d's ozone slab, a layer between 70 and 3 hPa (`earth.ts`'s `EARTH_OZONE_SLAB`), the test
 * medium of layer edges since R08.T6.h gave Earth its measured ozone, which has none.
 */
const OZONE = termNamed(EARTH_SLAB_REFERENCE, "absorber:O3");

/** A term's tabulated density. */
function tabulated(term: MediumTerm): TabulatedDensity {
  if (term.density.kind !== "tabulated") {
    throw new Error(`term ${term.name}'s density is tabulated`);
  }
  return term.density;
}

/** A medium whose tabulated densities carry no edges: R05's one piece of equal steps a ray. */
function withoutEdges(medium: AtmosphereMedium): AtmosphereMedium {
  return {
    ...medium,
    terms: medium.terms.map((term) =>
      term.density.kind === "tabulated"
        ? { ...term, density: { ...term.density, edgesM: [] } }
        : term,
    ),
  };
}

/**
 * The optical depth per channel at a stored texel as `transmittanceThroughTables` reads it there,
 * the light slot 0's curves added to the stored optical depth through the stored columns.
 */
function readDepth(tables: TwinTransmittance, x: number, y: number): [number, number, number] {
  const at = (y * tables.transmittance.widthTexels + x) * 4;
  const stored = tables.transmittance.texels;
  const more = tables.absorberColumns.texels;
  const depth: [number, number, number] = [
    stored[at] ?? Number.NaN,
    stored[at + 1] ?? Number.NaN,
    stored[at + 2] ?? Number.NaN,
  ];
  const columns = [stored[at + 3] ?? 0, ...[0, 1, 2, 3].map((k) => more[at + k] ?? 0)];
  for (const [k, curve] of (tables.curves[0] ?? []).entries()) {
    const absorbed = curveOpticalDepth(curve, columns[k] ?? 0, undefined);
    for (const c of [0, 1, 2] as const) {
      depth[c] += absorbed[c];
    }
  }
  return depth;
}

/** The worst texel of a table against its converged value, as a fraction of the bound. */
interface Worst {
  readonly ratio: number;
  readonly at: string;
}

/** A texel's stored columns, u ÷ U a slot: slot 0's in alpha, slots 1–4's in the second table. */
function storedColumns(tables: TwinTransmittance, x: number, y: number): ReadonlyArray<number> {
  const at = (y * tables.transmittance.widthTexels + x) * 4;
  return [
    tables.transmittance.texels[at + 3] ?? 0,
    ...[0, 1, 2, 3].map((k) => tables.absorberColumns.texels[at + k] ?? 0),
  ];
}

/** The growth exponent past which a slot's own column is held too: 2, τ ∝ u². */
const STEEPEST_IN_DEPTH = 2;

/** The stored column's half-float rounding, relative: 2⁻¹¹, rgba16float's unit roundoff. */
const HALF_ROUNDING = 2 ** -11;

/**
 * A curve's node interval about a column u ÷ U, found from ln(u ÷ U) on its even grid: the
 * interval's first node and its width in ln u.
 */
function intervalAbout(
  curve: AbsorberCurve,
  relativeColumn: number,
): { readonly j: number; readonly dx: number } {
  const { logColumns } = curve;
  const nodes = logColumns.length;
  const logU = 0.5 * ((logColumns[0] ?? Number.NaN) + (logColumns[nodes - 1] ?? Number.NaN));
  const at = Math.log(relativeColumn) + logU;
  let j = 0;
  while (j < nodes - 2 && (logColumns[j + 1] ?? Number.NaN) <= at) {
    j += 1;
  }
  return { j, dx: (logColumns[j + 1] ?? Number.NaN) - (logColumns[j] ?? Number.NaN) };
}

/**
 * The largest d ln τ ÷ d ln u of a curve's channels between the nodes about a column u ÷ U, from
 * node to node (`decision-r08-t6d-findings.md` §2.2): 1 in a channel's linear regime, falling as
 * its band saturates; 15 in the red curve's leap where it leaves Rec. 709 under R06's Sun; and
 * infinite where a channel leaves its clip at T_c = 1, its τ rising from 0, as ozone's blue does
 * past about 27 vertical columns, its signed sum above 1 until then. A channel at 0 at both nodes
 * does not rise.
 */
function growthExponent(curve: AbsorberCurve, relativeColumn: number): number {
  const { j, dx } = intervalAbout(curve, relativeColumn);
  return Math.max(
    ...curve.opticalDepth.map((depth) => {
      const from = depth[j] ?? Number.NaN;
      const to = depth[j + 1] ?? Number.NaN;
      if (!(to > 0)) {
        return 0;
      }
      return from > 0 ? Math.log(to / from) / dx : Number.POSITIVE_INFINITY;
    }),
  );
}

/** A curve's dτ ÷ d ln u in one channel between the nodes about a column u ÷ U, node to node. */
function depthSlope(curve: AbsorberCurve, relativeColumn: number, c: 0 | 1 | 2): number {
  const { j, dx } = intervalAbout(curve, relativeColumn);
  const depth = curve.opticalDepth[c];
  return ((depth[j + 1] ?? Number.NaN) - (depth[j] ?? Number.NaN)) / dx;
}

/**
 * A table's texels held to their converged values (`decision-r08-t6d-findings.md` §2.2 and §10.3):
 * every texel's optical depth in each channel but where the stored column's own rounding, carried
 * through a slot's curve, would alone pass the bound, (dτ_c ÷ d ln u) · 2⁻¹¹ > max(0.005, 10⁻³ τ);
 * and the column of each slot whose own curve rises faster than u² about the texel's column.
 */
interface Held {
  /** The worst held channel in optical depth, as a fraction of max(0.005, 10⁻³ τ). */
  readonly depth: Worst;
  /** The channels exempt from it, each with the slot whose curve exempts it: τ not held there. */
  readonly exempt: ReadonlyArray<Exempt>;
  /** The texels where some slot's curve rises faster than u², whose column is held there too. */
  readonly steep: number;
  /** The worst such column, as a fraction of 10⁻³ of itself. */
  readonly column: Worst;
}

/** A channel exempt from the bound in τ (`decision-r08-t6d-findings.md` §10.3), and its slot. */
interface Exempt {
  readonly c: 0 | 1 | 2;
  readonly slot: number;
}

function heldAgainst(table: TwinTransmittance, converged: TwinTransmittance): Held {
  let depth: Worst = { ratio: 0, at: "none" };
  let column: Worst = { ratio: 0, at: "none" };
  const exempt: Exempt[] = [];
  let steep = 0;
  const curves = converged.curves[0] ?? [];
  const { widthTexels: w, heightTexels: h } = TRANSMITTANCE_SIZE;
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const columns = storedColumns(converged, x, y);
      const steepSlots = curves.flatMap((curve, k) =>
        (columns[k] ?? 0) > 0 && growthExponent(curve, columns[k] ?? 0) > STEEPEST_IN_DEPTH
          ? [k]
          : [],
      );
      if (steepSlots.length > 0) {
        steep += 1;
        const stored = storedColumns(table, x, y);
        for (const k of steepSlots) {
          const u = columns[k] ?? 0;
          const ratio = Math.abs((stored[k] ?? 0) - u) / (1e-3 * u);
          if (!(ratio <= column.ratio)) {
            column = { ratio, at: `(${x}, ${y}) slot ${k}: u ÷ U ${u}` };
          }
        }
      }
      const a = readDepth(table, x, y);
      const b = readDepth(converged, x, y);
      for (const c of [0, 1, 2] as const) {
        const slot = curves.findIndex(
          (curve, k) =>
            (columns[k] ?? 0) > 0 &&
            depthSlope(curve, columns[k] ?? 0, c) * HALF_ROUNDING > bound(b[c]),
        );
        if (slot >= 0) {
          exempt.push({ c, slot });
          continue;
        }
        const ratio = Math.abs(a[c] - b[c]) / bound(b[c]);
        if (!(ratio <= depth.ratio)) {
          depth = { ratio, at: `(${x}, ${y}) c${c}: τ ${b[c]}, off by ${Math.abs(a[c] - b[c])}` };
        }
      }
    }
  }
  return { depth, exempt, steep, column };
}

/** A table's greatest stored column of absorber slot 0 over its texels, u ÷ U. */
function greatestColumn(table: TwinTransmittance): number {
  let greatest = 0;
  for (let i = 3; i < table.transmittance.texels.length; i += 4) {
    greatest = Math.max(greatest, table.transmittance.texels[i] ?? 0);
  }
  return greatest;
}

/** The (r, μ) of transmittance texel (x, y), Bruneton's mapping on R₁. */
function texelRay(medium: AtmosphereMedium, x: number, y: number): { rM: number; mu: number } {
  const shell = { bottomRadiusM: R1, topRadiusM: R1 + medium.topHeightM };
  const { widthTexels: w, heightTexels: h } = TRANSMITTANCE_SIZE;
  return transmittanceUvToRMu(
    shell,
    unitFromTexelUv((x + 0.5) / w, w),
    unitFromTexelUv((y + 0.5) / h, h),
  );
}

/** The optical depth per channel one texel's ray takes, its absorbers' curves added. */
function rayDepths(
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
  x: number,
  y: number,
  samples: number,
): [number, number, number] {
  const { rM, mu } = texelRay(medium, x, y);
  const ray = kernelRayToTop(medium, R1, rM, mu, samples);
  const depth: [number, number, number] = [...ray.opticalDepth];
  for (const [k, curve] of (curves[0] ?? []).entries()) {
    const absorbed = curveOpticalDepth(curve, ray.columns[k] ?? 0, undefined);
    for (const c of [0, 1, 2] as const) {
      depth[c] += absorbed[c];
    }
  }
  return depth;
}

/** The optical depth one texel's ray takes in red, its absorbers' curves added. */
function rayDepth(
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
  x: number,
  y: number,
  samples: number,
): number {
  return rayDepths(medium, curves, x, y, samples)[0];
}

/**
 * The transmittance kernel's part of the packed edges, four a term (R08.T6.d); the per-frame
 * marches' follows (R08.T6.i, `tables.test.ts`).
 */
function termEdges(medium: AtmosphereMedium): number[] {
  return Array.from(packLayerEdges(medium).slice(0, 4 * MAX_TERMS));
}

describe("a term's layer edges", () => {
  it("are an absorber layer's four: where it leaves 0, its base, its top, where it reaches 0", () => {
    const { altitudesM } = tabulated(OZONE);
    const n = altitudesM.length;
    expect(layerEdgesM(OZONE.density)).toEqual([
      altitudesM[0],
      altitudesM[1],
      altitudesM[n - 2],
      altitudesM[n - 1],
    ]);
  });

  it("are none for a column, an exponential or R05's tent", () => {
    expect(layerEdgesM(EARTH_COLUMN.density)).toEqual([]);
    expect(layerEdgesM({ kind: "exponential", scaleHeightM: 1_200 })).toEqual([]);
    expect(layerEdgesM({ kind: "tent", bottomM: 10_000, peakM: 25_000, topM: 40_000 })).toEqual([]);
  });

  it("are a layer's upper two alone where it stands on the ground", () => {
    const deck = tabulated({
      ...OZONE,
      density: {
        kind: "tabulated",
        altitudesM: Float64Array.of(0, 3_000, 3_100),
        relative: Float64Array.of(1, 1, 0),
      },
    });
    expect(layerEdgesM(deck)).toEqual([3_000, 3_100]);
  });

  it("widen outward to a resampled table's levels, so that its whole ramp lies between them", () => {
    const original = layerEdgesM(OZONE.density);
    const top = EARTH_SLAB_REFERENCE.topHeightM;
    const resampled = resampledDensity(tabulated(OZONE), top);
    const edges = resampled.edgesM ?? [];
    const levels = [...densityTableLevelsM(top)];
    expect(edges).toHaveLength(4);
    for (const [i, edge] of edges.entries()) {
      expect(levels).toContain(edge);
      const from = original[i] ?? Number.NaN;
      expect(i % 2 === 0 ? edge <= from : edge >= from).toBe(true);
    }
    // 0 outside, and the layer's own density, ramp-free, inside.
    const [lowest = 0, base = 0, top_ = 0, highest = 0] = edges;
    const outside = levels.filter((h) => h <= lowest || h >= highest);
    const inside = levels.filter((h) => h >= base && h <= top_);
    expect(outside.map((h) => densityAt(resampled, h))).toEqual(outside.map(() => 0));
    expect(inside.map((h) => densityAt(resampled, h))).toEqual(
      inside.map((h) => densityAt(OZONE.density, h)),
    );
  });

  it("travel to the transmittance kernel as the kernels' tables read them, −1 where unused", () => {
    expect(termEdges(R05_EARTH_REFERENCE)).toEqual(Array(4 * MAX_TERMS).fill(-1));
    // Earth's measured ozone has none (R08.T6.h).
    expect(termEdges(EARTH_REFERENCE)).toEqual(Array(4 * MAX_TERMS).fill(-1));
    const packed = termEdges(EARTH_SLAB_REFERENCE);
    const ozone = EARTH_SLAB_REFERENCE.terms.indexOf(OZONE);
    const read = kernelMedium(EARTH_SLAB_REFERENCE).terms[ozone];
    expect(read).toBeDefined();
    expect(packed.slice(4 * ozone, 4 * ozone + 4)).toEqual(
      layerEdgesM(read?.density ?? OZONE.density).map(Math.fround),
    );
    expect(packed.filter((h) => h !== -1)).toHaveLength(4);
  });

  it("are refused past the kernels' terms, as the terms are", () => {
    const tooMany = {
      ...EARTH_SLAB_REFERENCE,
      terms: Array.from({ length: MAX_TERMS + 1 }, () => OZONE),
    };
    expect(() => packLayerEdges(tooMany)).toThrow(`at most ${MAX_TERMS} fit`);
  });

  it("take the kernel's least steps a piece", () => {
    expect(transmittanceWgsl).toContain(
      `const STEPS_A_PIECE : u32 = ${TRANSMITTANCE_STEPS_A_PIECE}u;`,
    );
    expect(transmittanceWgsl).toContain("var<uniform> layerEdges : LayerEdges;");
  });
});

describe("a transmittance ray split at its layer edges", () => {
  const shell = { bottomRadiusM: R1, topRadiusM: R1 + EARTH_SLAB_REFERENCE.topHeightM };
  const edges = layerEdgesM(OZONE.density);

  it("crosses an edge's sphere where the ray stands at that height, from heights alone", () => {
    for (const [h0, mu, e] of [
      [2, 0.1, 24_000],
      [30_000, -0.2, 24_000],
      [60_000, -0.12, 30_800],
    ] as const) {
      const roots = shellCrossingsM(R1, h0, mu, e) ?? [];
      expect(roots.length).toBe(2);
      for (const t of roots.filter((root) => root > 0)) {
        expect(Math.abs(localRadiusM(R1 + h0, mu, t) - (R1 + e))).toBeLessThan(1e-6);
      }
    }
    // Upward from above the edge, the ray never meets it ahead.
    const away = shellCrossingsM(R1, 30_000, 0.5, 24_000) ?? [];
    expect(away.every((t) => t < 0)).toBe(true);
    // Downward short of it, there is none.
    expect(shellCrossingsM(R1, 40_000, -0.01, 24_000)).toBeNull();
  });

  it("spreads its 256 steps over its pieces by their length, at least 4 a piece", () => {
    for (const [h0, mu] of [
      [2, 0.03],
      [27_000, -0.05],
      [99_000, -0.1],
      [2, 1],
    ] as const) {
      const rM = R1 + h0;
      const tMax = distanceToTopM(shell, rM, mu);
      const { endsM, steps } = transmittancePieces(
        edges,
        shell,
        h0,
        mu,
        tMax,
        TRANSMITTANCE_SAMPLES,
      );
      expect(steps.reduce((sum, n) => sum + n, 0)).toBe(TRANSMITTANCE_SAMPLES);
      expect(steps.every((n) => n >= TRANSMITTANCE_STEPS_A_PIECE)).toBe(true);
      expect(endsM.at(-1)).toBe(tMax);
      for (const [p, end] of endsM.slice(0, -1).entries()) {
        expect(end).toBeGreaterThan(p === 0 ? 0 : (endsM[p - 1] ?? 0));
        const heightM = localRadiusM(rM, mu, end) - R1;
        expect(Math.min(...edges.map((e) => Math.abs(heightM - e)))).toBeLessThan(1e-5);
      }
    }
  });

  it("is one piece of equal steps where it crosses no edge, R05's march", () => {
    const tMax = distanceToTopM(shell, R1 + 2, 1);
    expect(transmittancePieces([], shell, 2, 1, tMax, TRANSMITTANCE_SAMPLES)).toEqual({
      endsM: [tMax],
      steps: [TRANSMITTANCE_SAMPLES],
    });
  });
});

describe("the transmittance table against its own 4,096-step value (decision-r08-t6b-floor.md §9.2)", () => {
  const four = fourAbsorberMedium();
  const cases: ReadonlyArray<{
    readonly name: string;
    readonly medium: AtmosphereMedium;
    readonly curves: AbsorberCurveSlots;
  }> = [
    { name: "the four-absorber fixture", medium: four.medium, curves: four.curves },
    { name: "the measured Earth", medium: EARTH_REFERENCE, curves: EARTH_REFERENCE_CURVES },
    {
      name: "the ozone-slab Earth",
      medium: EARTH_SLAB_REFERENCE,
      curves: EARTH_SLAB_REFERENCE_CURVES,
    },
    { name: "R05's Earth", medium: R05_EARTH_REFERENCE, curves: NO_ABSORBER_CURVES },
  ];
  const held = new Map<string, Held>();
  const greatest = new Map<string, number>();

  beforeAll(() => {
    for (const { name, medium, curves } of cases) {
      const table = transmittanceTablesTwin(medium, R1, curves);
      const converged = transmittanceTablesTwin(
        medium,
        R1,
        curves,
        TRANSMITTANCE_SIZE,
        CONVERGED_STEPS,
      );
      held.set(name, heldAgainst(table, converged));
      greatest.set(name, greatestColumn(converged));
    }
  }, 900_000);

  it.each(cases.map(({ name }) => [name]))(
    "holds every texel of %s within max(0.005, 10⁻³ τ), curves included, but where its column's own rounding alone would pass it",
    (name) => {
      const found = held.get(name)?.depth;
      expect(found).toBeDefined();
      // The worst texel, named where it is over the bound.
      expect((found?.ratio ?? Number.POSITIVE_INFINITY) <= 1 ? "within" : found?.at).toBe("within");
    },
  );

  // Where some curve rises faster than u² between the nodes about a texel's column, that slot's
  // column is held too, to 10⁻³ of itself (`decision-r08-t6d-findings.md` §10.3). On these tables
  // every such texel is where ozone's blue leaves its clip at T_c = 1, its τ rising from 0 past
  // 26.6 vertical columns, d ln τ ÷ d ln u above 2 to about 60, under the bound's 0.005 floor:
  // measured on 2026-10-13 at worst 0.20, 0.21 and 0.36 of 10⁻³ (the four absorbers, the measured
  // Earth, the slab Earth), with their τ 0.47–43.0 within its bound at worst 0.080, 0.041 and 0.101.
  it.each(cases.map(({ name }) => [name]))(
    "holds %s's column to 10⁻³ of itself wherever its curve rises faster than u² about it",
    (name) => {
      const found = held.get(name)?.column;
      expect((found?.ratio ?? Number.POSITIVE_INFINITY) <= 1 ? "within" : found?.at).toBe("within");
    },
  );

  // The texels each check takes, recorded (§10.4.2). τ is exempt, and recorded instead, only in a
  // channel where (dτ_c ÷ d ln u) · 2⁻¹¹ > max(0.005, 10⁻³ τ), the leap at a curve's gamut exit.
  // Measured on 2026-10-13: none on either Earth, ozone's deepest column there being 81.6 against
  // the red leap's node 94.4; on the four absorbers, 43 red channels, slot 1's, the ground's
  // horizontal rays (255, 21–63) at τ 24.0–29.4. Their methane columns, 70.9–73.9, lie between its
  // red curve's nodes at 70.8 and 75.0, where it climbs from τ 6.36 to the floor's 13.8 as its signed
  // sum leaves Rec. 709 (dτ ÷ d ln u = 130): methane's red gamut exit. τ meets its bound there all
  // the same, at worst 0.030 of it.
  it.each([
    ["the four-absorber fixture", 2_504, 43, ["slot 1 c0"]],
    ["the measured Earth", 1_858, 0, []],
    ["the ozone-slab Earth", 1_596, 0, []],
    ["R05's Earth", 0, 0, []],
  ] as const)(
    "counts the texels each check takes on %s: %i columns held, %i channels exempt",
    (name, steep, count, slots) => {
      const found = held.get(name);
      const exempt = found?.exempt ?? [];
      expect({
        steep: found?.steep,
        exempt: exempt.length,
        slots: [...new Set(exempt.map((e) => `slot ${e.slot} c${e.c}`))],
      }).toEqual({ steep, exempt: count, slots });
    },
  );

  it("records the greatest ozone column on a texel's ray: about 41 vertical columns, against 80", () => {
    // `decision-r08-t6d-findings.md` §2.1 estimated 40.8 and 80.5 on the 1976 standard's heights;
    // the red curve leaves Rec. 709 at about 99 under R06's Sun.
    expect([
      Number((greatest.get("the measured Earth") ?? Number.NaN).toFixed(1)),
      Number((greatest.get("the ozone-slab Earth") ?? Number.NaN).toFixed(1)),
    ]).toEqual([MEASURED_GREATEST_COLUMNS, SLAB_GREATEST_COLUMNS]);
  });

  // The table the kernels build reads the medium resampled (`kernelMedium`), its ramps spread over
  // the density table's levels within the edges `resampledDensity` widens to them: held at every
  // fourth texel of each axis, which costs a sixteenth of the whole table. R05's Earth has no
  // tabulated density to resample; the measured Earth's are resampled though they carry no edge.
  const layered = cases.filter(({ medium }) =>
    medium.terms.some((term) => term.density.kind === "tabulated"),
  );
  it.each(layered.map(({ name, medium, curves }) => [name, medium, curves] as const))(
    "holds %s as the kernels read it within the bound at every fourth texel",
    (_name, medium, curves) => {
      const read = kernelMedium(medium);
      let found: Worst = { ratio: 0, at: "none" };
      for (let y = 3; y < TRANSMITTANCE_SIZE.heightTexels; y += 4) {
        for (let x = 3; x < TRANSMITTANCE_SIZE.widthTexels; x += 4) {
          const table = rayDepths(read, curves, x, y, TRANSMITTANCE_SAMPLES);
          const converged = rayDepths(read, curves, x, y, CONVERGED_STEPS);
          for (const c of [0, 1, 2] as const) {
            const ratio = Math.abs(table[c] - converged[c]) / bound(converged[c]);
            if (!(ratio <= found.ratio)) {
              found = { ratio, at: `(${x}, ${y}) c${c}: τ ${converged[c]}` };
            }
          }
        }
      }
      expect(found.ratio <= 1 ? "within" : found.at).toBe("within");
    },
    120_000,
  );

  // R08.T6.c's finding (Risks, "Deviations in T6.c, as built"): at 256 equal steps the
  // four-absorber fixture's texel (242, 63) stood 0.042 in red optical depth, 4.1% in T, off its
  // own 4,096-step value. Without the split it still does; with it, it holds.
  it("brings T6.c's texel (242, 63), red, within the bound, which equal steps miss", () => {
    const equal = withoutEdges(four.medium);
    const before = Math.abs(
      rayDepth(equal, four.curves, 242, 63, TRANSMITTANCE_SAMPLES) -
        rayDepth(equal, four.curves, 242, 63, CONVERGED_STEPS),
    );
    const converged = rayDepth(four.medium, four.curves, 242, 63, CONVERGED_STEPS);
    const after = Math.abs(
      rayDepth(four.medium, four.curves, 242, 63, TRANSMITTANCE_SAMPLES) - converged,
    );
    expect(before).toBeGreaterThan(0.04);
    expect(before).toBeGreaterThan(bound(converged));
    expect(after).toBeLessThan(bound(converged));
  });
});

describe("the stored column's half-float rounding under the curves (decision-r08-t6d-findings.md §2.2)", () => {
  const four = fourAbsorberMedium();
  const curves: ReadonlyArray<readonly [string, AbsorberCurve]> = [
    ...(EARTH_REFERENCE_CURVES[0] ?? []).map((c) => ["the measured Earth's ozone", c] as const),
    ...(EARTH_SLAB_REFERENCE_CURVES[0] ?? []).map((c) => ["the slab's ozone", c] as const),
    ...(four.curves[0] ?? []).map((c, k) => [`the four absorbers' slot ${k}`, c] as const),
  ];

  // rgba16float rounds a stored column to 2⁻¹¹ of itself; where d ln τ ÷ d ln u ≤ 2 that moves τ
  // by at most 2 × 2⁻¹¹ < 10⁻³ of itself, so the bound in τ is reachable there whatever the column's
  // rounding: the threshold of the per-slot column check (`decision-r08-t6d-findings.md` §10.3).
  it("moves τ by at most 10⁻³ of itself wherever every channel of a curve rises at most as u²", () => {
    const over: string[] = [];
    for (const [name, curve] of curves) {
      const { logColumns } = curve;
      const logU = 0.5 * ((logColumns[0] ?? Number.NaN) + (logColumns.at(-1) ?? Number.NaN));
      for (let j = 0; j + 1 < logColumns.length; j += 1) {
        const mid = 0.5 * ((logColumns[j] ?? Number.NaN) + (logColumns[j + 1] ?? Number.NaN));
        const u = Math.exp(mid - logU);
        if (growthExponent(curve, u) > STEEPEST_IN_DEPTH) {
          continue;
        }
        const at = curveOpticalDepth(curve, u, undefined);
        const rounded = curveOpticalDepth(curve, u * (1 + HALF_ROUNDING), undefined);
        for (const c of [0, 1, 2] as const) {
          if (!(Math.abs(rounded[c] - at[c]) <= 1e-3 * at[c])) {
            over.push(`${name} c${c} at u ÷ U ${u.toPrecision(4)}`);
          }
        }
      }
    }
    expect(over).toEqual([]);
  });
});
