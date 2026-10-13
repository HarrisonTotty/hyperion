import { beforeAll, describe, expect, it } from "vitest";

import { fourAbsorberMedium } from "../../test/atmosphereFixtures";
import {
  EARTH_COLUMN,
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
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

/** The bound in optical depth the table is held to: max(0.005, 10⁻³ τ) (§9.2). */
function bound(tau: number): number {
  return Math.max(0.005, 1e-3 * tau);
}

/** The ozone of the rebuilt Earth, a layer between 70 and 3 hPa (`earth.ts`). */
const OZONE = termNamed(EARTH_REFERENCE, "absorber:O3");

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
    const absorbed = curveOpticalDepth(curve, columns[k] ?? 0);
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

function worstAgainst(table: TwinTransmittance, converged: TwinTransmittance): Worst {
  let worst: Worst = { ratio: 0, at: "none" };
  const { widthTexels: w, heightTexels: h } = TRANSMITTANCE_SIZE;
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const a = readDepth(table, x, y);
      const b = readDepth(converged, x, y);
      for (const c of [0, 1, 2] as const) {
        const ratio = Math.abs(a[c] - b[c]) / bound(b[c]);
        if (!(ratio <= worst.ratio)) {
          worst = { ratio, at: `(${x}, ${y}) c${c}: τ ${b[c]}, off by ${Math.abs(a[c] - b[c])}` };
        }
      }
    }
  }
  return worst;
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
    const absorbed = curveOpticalDepth(curve, ray.columns[k] ?? 0);
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
    const top = EARTH_REFERENCE.topHeightM;
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
    expect([...packLayerEdges(R05_EARTH_REFERENCE)]).toEqual(Array(4 * MAX_TERMS).fill(-1));
    const packed = [...packLayerEdges(EARTH_REFERENCE)];
    const ozone = EARTH_REFERENCE.terms.indexOf(OZONE);
    const read = kernelMedium(EARTH_REFERENCE).terms[ozone];
    expect(read).toBeDefined();
    expect(packed.slice(4 * ozone, 4 * ozone + 4)).toEqual(
      layerEdgesM(read?.density ?? OZONE.density).map(Math.fround),
    );
    expect(packed.filter((h) => h !== -1)).toHaveLength(4);
  });

  it("are refused past the kernels' terms, as the terms are", () => {
    const tooMany = {
      ...EARTH_REFERENCE,
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
  const shell = { bottomRadiusM: R1, topRadiusM: R1 + EARTH_REFERENCE.topHeightM };
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
    { name: "the rebuilt Earth", medium: EARTH_REFERENCE, curves: EARTH_REFERENCE_CURVES },
    { name: "R05's Earth", medium: R05_EARTH_REFERENCE, curves: NO_ABSORBER_CURVES },
  ];
  const worst = new Map<string, Worst>();

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
      worst.set(name, worstAgainst(table, converged));
    }
  }, 900_000);

  it.each(cases.map(({ name }) => [name]))(
    "holds every texel of %s within max(0.005, 10⁻³ τ), curves included",
    (name) => {
      const found = worst.get(name);
      expect(found).toBeDefined();
      // The worst texel, named where it is over the bound.
      expect((found?.ratio ?? Number.POSITIVE_INFINITY) <= 1 ? "within" : found?.at).toBe("within");
    },
  );

  // The table the kernels build reads the medium resampled (`kernelMedium`), its ramps spread over
  // the density table's levels within the edges `resampledDensity` widens to them: held at every
  // fourth texel of each axis, which costs a sixteenth of the whole table. R05's Earth has no
  // tabulated density to resample.
  const layered = cases.filter(({ medium }) => packLayerEdges(medium).some((h) => h !== -1));
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
