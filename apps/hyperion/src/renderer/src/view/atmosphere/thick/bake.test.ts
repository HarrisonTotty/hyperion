/**
 * The thick bake (plan R08, R08.T14.b): its table on the Venus-class and Titan-class fixtures, its
 * source against the solver's own fields, its bands, its absorbers' sums and its refusals.
 */
import { describe, expect, it } from "vitest";

import {
  gravityOfSpheroid,
  planckBakeSpectrum,
  SUN_TEFF_K,
  TITAN_FRACTIONS,
  URANUS_SPHEROID,
} from "../../../test/atmosphereFixtures";
import { absorberBinCurves, type AbsorberBinCurves } from "../absorbers";
import { mediumTableRadiusM } from "../assemble";
import { CHANNEL_WAVELENGTHS_NM } from "../medium";
import { normalizedAssociatedLegendre } from "./discreteOrdinates";
import { tablesPlanOf } from "../tables";
import {
  BAKE_CEILING_S,
  bakeBinSolve,
  bakeMultipleScattering,
  binGroundAlbedo,
  type MsSourceTable,
  prepareBake,
  type PreparedBake,
  THICK_BAKE_GRID,
  THICK_BAKE_MAX_TERMS_PER_BIN,
  THICK_BAKE_MODES,
  thickBakeBands,
  type ThickBakeBody,
  type ThickBakeGrid,
  type ThickBakeLight,
} from "./bake";
import {
  lightOf,
  type ThickFixture,
  titanBody,
  titanClassColumn,
  titanClassFixture,
  venusClassFixture,
} from "./bakeFixtures";
import { doubleGaussQuadrature } from "./discreteOrdinates";
import { type SpectralColumn, spectralOptics, type SpectralTerm } from "./spectralColumn";

/** A value computed on its first use, so that a describe block's setup runs inside its tests. */
function once<T>(make: () => T): () => T {
  let value: { readonly made: T } | undefined;
  return () => {
    value ??= { made: make() };
    return value.made;
  };
}

/** A fixture's band-0 bake on the default grid. */
interface Baked {
  readonly fixture: ThickFixture;
  readonly prepared: PreparedBake;
  readonly table: MsSourceTable;
}

function bakedOf(fixture: ThickFixture): Baked {
  const prepared = prepareBake(fixture.column, [lightOf(fixture)]);
  return { fixture, prepared, table: bakeMultipleScattering(prepared, fixture.body, 0) };
}

/** A fixture's bake and the bounds its records are held to, as measured and recorded. */
interface FixtureCase {
  readonly baked: () => Promise<Baked>;
  /** The largest |ΔT| of its absorbers' sums over every row, and over the rows their bulk reads. */
  readonly fitBounds: readonly [number, number];
  /** The largest IMS share of a point's light. */
  readonly imsBound: number;
}

const venus = once(async (): Promise<Baked> => bakedOf(await venusClassFixture()));
const titan = once(async (): Promise<Baked> => Promise.resolve(bakedOf(titanClassFixture())));

/**
 * The thick fixtures: measured 3.9 × 10⁻² and 1.2 × 10⁻² for Venus's CO₂, 1.7 × 10⁻³ for Titan's
 * methane; IMS 1.2% and 5.0% (2026-10-26).
 */
const FIXTURES: ReadonlyArray<readonly [string, FixtureCase]> = [
  ["Venus-class column", { baked: venus, fitBounds: [0.04, 0.013], imsBound: 0.02 }],
  ["Titan-class column", { baked: titan, fitBounds: [2e-3, 2e-3], imsBound: 0.06 }],
];

/** A table's one slot, or a test failure. */
function slotOf(table: MsSourceTable): MsSourceTable["slots"][number] {
  const slot = table.slots[0];
  if (slot === undefined) {
    throw new Error("the bake has one slot");
  }
  return slot;
}

// The Venus-class preparation runs Mie in 15 bins: 6.1 s, and the bake 1.8 s (load 0.8,
// 2026-10-26); 17 s and 4.5 s beside #107's sweep. Ten times the quiet figure, rounded up.
const BAKE_TIMEOUT_MS = 300_000;

describe("the thick bake on the thick fixtures", () => {
  it.for(FIXTURES)(
    "lays the %s's table out on Design note 9's grid, ground and downward first",
    { timeout: BAKE_TIMEOUT_MS },
    async ([, { baked }]) => {
      const { table } = await baked();
      const altitudes = THICK_BAKE_GRID.layers / THICK_BAKE_GRID.altitudeStride + 1;
      const suns = THICK_BAKE_GRID.sunCosines.length;
      const views = 2 * THICK_BAKE_GRID.upwardCosines.length;
      expect([
        table.altitudesM.length,
        table.sunCosines.length,
        table.travelCosines.length,
        slotOf(table).values.length,
        table.altitudesM[0],
        table.travelCosines[0],
      ]).toEqual([
        altitudes,
        suns,
        views,
        altitudes * suns * views * THICK_BAKE_MODES * 3,
        0,
        -(THICK_BAKE_GRID.upwardCosines.at(-1) ?? Number.NaN),
      ]);
    },
  );

  it.for(FIXTURES)(
    "bakes the %s's table finite",
    { timeout: BAKE_TIMEOUT_MS },
    async ([, { baked }]) => {
      expect(slotOf((await baked()).table).values.every(Number.isFinite)).toBe(true);
    },
  );

  it.for(FIXTURES)(
    "never stores a negative m = 0 for the %s",
    { timeout: BAKE_TIMEOUT_MS },
    async ([, { baked }]) => {
      const { values } = slotOf((await baked()).table);
      const negative = values.filter(
        (value, i) => Math.floor(i / 3) % THICK_BAKE_MODES === 0 && !(value >= 0),
      );
      expect(negative.length).toBe(0);
    },
  );

  it.for(FIXTURES)(
    "solves the %s's bins once a term of their gases",
    { timeout: BAKE_TIMEOUT_MS },
    async ([, { baked }]) => {
      const slot = slotOf((await baked()).table);
      expect(slot.solves).toBe(slot.bins.reduce((sum, bin) => sum + bin.terms, 0));
    },
  );

  it.for(FIXTURES)(
    "fits the %s's absorbers within the bounds recorded",
    { timeout: BAKE_TIMEOUT_MS },
    async ([, { baked, fitBounds }]) => {
      const { bins } = slotOf((await baked()).table);
      expect([
        Math.max(...bins.map((b) => b.largestFitError)) <= fitBounds[0],
        Math.max(...bins.map((b) => b.readFitError)) <= fitBounds[1],
      ]).toEqual([true, true]);
    },
  );

  it.for(FIXTURES)(
    "leaves out at most the IMS share recorded for the %s",
    { timeout: BAKE_TIMEOUT_MS },
    async ([, { baked, imsBound }]) => {
      expect(slotOf((await baked()).table).doubleForwardShare).toBeLessThanOrEqual(imsBound);
    },
  );

  it(
    "builds the Venus-class record fixture's column to its drawn medium's coefficients at the channels",
    { timeout: BAKE_TIMEOUT_MS },
    async () => {
      const { fixture } = await venus();
      const optics = spectralOptics(fixture.column, CHANNEL_WAVELENGTHS_NM);
      let worst = 0;
      for (const [c, terms] of optics.entries()) {
        for (const [t, own] of terms.entries()) {
          const drawn = fixture.medium.terms[t];
          const pairs = [
            [own.scatteringPerM, drawn?.scattering[c] ?? Number.NaN],
            [own.absorptionPerM, drawn?.absorption[c] ?? Number.NaN],
          ] as const;
          for (const [value, expected] of pairs) {
            const scale = Math.max(Math.abs(value), Math.abs(expected));
            worst = Math.max(worst, scale === 0 ? 0 : Math.abs(value - expected) / scale);
          }
        }
      }
      // n_s is read back from the molecular term's green coefficient, a rounding or two.
      expect(worst).toBeLessThanOrEqual(8 * Number.EPSILON);
    },
  );

  it("exports the ceiling of Design note 9", () => {
    expect(BAKE_CEILING_S).toBe(5);
  });
});

/** A small grid for the checks that need no full table. */
function smallGrid(upwardCosines: Float64Array): ThickBakeGrid {
  return { layers: 16, altitudeStride: 2, sunCosines: Float64Array.of(0.3, 0.7, 1), upwardCosines };
}

/** A Rayleigh column of Titan's air, cheap to bake. */
function rayleighColumn(): SpectralColumn {
  const column = titanClassColumn();
  return {
    name: "titan-class air",
    topHeightM: column.topHeightM,
    groundAlbedo: [0.3, 0.3, 0.3],
    terms: [
      {
        name: "rayleigh",
        density: column.density,
        source: {
          kind: "molecular",
          fractions: TITAN_FRACTIONS,
          numberDensityPerM3: column.surfaceNumberDensityPerM3,
        },
      },
    ],
  };
}

/** The Sun with no absorber's curves. */
const UNABSORBED: ThickBakeLight = { spectrum: planckBakeSpectrum(SUN_TEFF_K), absorbers: [] };

describe("the stored source against the solver's own fields", () => {
  it("integrates over all directions to σ_s times the diffuse field's whole mean intensity and the aureole, to 10⁻¹²", () => {
    const fixture = titanClassFixture();
    const quadrature = doubleGaussQuadrature(16);
    const prepared = prepareBake(fixture.column, [lightOf(fixture)], smallGrid(quadrature.nodes));
    const band = thickBakeBands(fixture.body)[0];
    if (band === undefined) {
      throw new Error("Titan has one band");
    }
    const bin = 13;
    const terms = prepared.lights[0]?.bins[bin]?.terms ?? [];
    const n = quadrature.nodes.length;
    const suns = prepared.grid.sunCosines.length;
    const views = 2 * n;
    let worst = 0;
    for (const term of terms) {
      const solve = bakeBinSolve(prepared, band, bin, term);
      for (const [a, point] of (prepared.pointOptics[bin] ?? []).entries()) {
        const k = prepared.grid.layers - a * prepared.grid.altitudeStride;
        for (let s = 0; s < suns; s += 1) {
          let integral = 0;
          for (let u = 0; u < n; u += 1) {
            const cell = (a * suns + s) * views;
            const up = solve.source[(cell + n + u) * THICK_BAKE_MODES] ?? Number.NaN;
            const down = solve.source[(cell + n - 1 - u) * THICK_BAKE_MODES] ?? Number.NaN;
            integral += 2 * Math.PI * (quadrature.weights[u] ?? Number.NaN) * (up + down);
          }
          const beam = solve.solution.beams[s];
          const scaled = solve.scaledBeams[s]?.interfaceTransmission[k] ?? Number.NaN;
          const kernel = solve.kernelBeams[s]?.interfaceTransmission[k] ?? Number.NaN;
          const mean = (beam?.fluxes.meanIntensity[k] ?? Number.NaN) - scaled / (4 * Math.PI);
          const expected =
            point.scatteringPerM *
            ((1 - point.forwardDelta) * 4 * Math.PI * mean +
              point.kept * Math.max(scaled - kernel, 0));
          worst = Math.max(worst, Math.abs(integral - expected) / Math.abs(expected));
        }
      }
    }
    expect([terms.length > 1, worst < 1e-12]).toEqual([true, true]);
  });

  it("is σ_s ∫ p I_d dΩ′ at each azimuth on a Rayleigh column, by a direct quadrature over the solver's own field, to 10⁻¹²", () => {
    const viewCosines = Float64Array.of(0.2, 0.6, 0.9);
    const prepared = prepareBake(rayleighColumn(), [UNABSORBED], smallGrid(viewCosines));
    const band = thickBakeBands(titanBody())[0];
    const term = prepared.lights[0]?.bins[7]?.terms[0];
    if (band === undefined || term === undefined) {
      throw new Error("one band, one term");
    }
    const solve = bakeBinSolve(prepared, band, 7, term);
    const { nodes, weights } = solve.solution.quadrature;
    const n = nodes.length;
    const suns = prepared.grid.sunCosines.length;
    const views = 2 * viewCosines.length;
    const azimuths = 32;
    let worst = 0;
    for (const [a, point] of (prepared.pointOptics[7] ?? []).entries()) {
      const k = prepared.grid.layers - a * prepared.grid.altitudeStride;
      // Rayleigh's phase from its one moment: (1 + 5 χ₂ P₂(cos Θ)) ÷ 4π.
      const chi2 = point.moments[2] ?? Number.NaN;
      const phase = (cosine: number): number =>
        (1 +
          5 *
            chi2 *
            (normalizedAssociatedLegendre(0, 2, Math.min(Math.max(cosine, -1), 1))[2] ??
              Number.NaN)) /
        (4 * Math.PI);
      for (let s = 0; s < suns; s += 1) {
        const modes = solve.solution.beams[s]?.modes ?? [];
        const field = (mu: number, j: number, azimuth: number): number => {
          let sum = 0;
          for (const [m, mode] of modes.entries()) {
            const values = mu > 0 ? mode.quadratureUp : mode.quadratureDown;
            sum += (values[k * n + j] ?? Number.NaN) * Math.cos(m * azimuth);
          }
          return sum;
        };
        for (let v = 0; v < views; v += 1) {
          const mu =
            v < views / 2
              ? -(viewCosines[views / 2 - 1 - v] ?? 0)
              : (viewCosines[v - views / 2] ?? 0);
          const cell = ((a * suns + s) * views + v) * THICK_BAKE_MODES;
          for (const deltaPhi of [0, Math.PI / 2, Math.PI]) {
            const stored =
              (solve.source[cell] ?? Number.NaN) +
              (solve.source[cell + 1] ?? Number.NaN) * Math.cos(deltaPhi);
            let direct = 0;
            for (const [j, node] of nodes.entries()) {
              for (const sign of [1, -1]) {
                const muPrime = sign * node;
                for (let q = 0; q < azimuths; q += 1) {
                  const phiPrime = (2 * Math.PI * q) / azimuths;
                  const cosine =
                    mu * muPrime +
                    Math.sqrt((1 - mu * mu) * (1 - muPrime * muPrime)) *
                      Math.cos(deltaPhi - phiPrime);
                  direct +=
                    (weights[j] ?? Number.NaN) *
                    ((2 * Math.PI) / azimuths) *
                    phase(cosine) *
                    field(muPrime, j, phiPrime);
                }
              }
            }
            direct *= point.scatteringPerM;
            worst = Math.max(worst, Math.abs(stored - direct) / Math.abs(direct));
          }
        }
      }
    }
    expect(worst).toBeLessThan(1e-12);
  });

  it("holds no m = 1 at an overhead sun, where Λ_l¹(1) = 0", () => {
    const fixture = titanClassFixture();
    const prepared = prepareBake(
      fixture.column,
      [lightOf(fixture)],
      smallGrid(Float64Array.of(0.2, 0.9)),
    );
    const { values } = slotOf(bakeMultipleScattering(prepared, fixture.body, 0));
    const suns = prepared.grid.sunCosines.length;
    const overhead = suns - 1;
    const perSun = 2 * prepared.grid.upwardCosines.length * THICK_BAKE_MODES * 3;
    let largest = 0;
    let modeOne = 0;
    for (let i = 0; i < values.length; i += 1) {
      const s = Math.floor(i / perSun) % suns;
      const m = Math.floor(i / 3) % THICK_BAKE_MODES;
      const value = Math.abs(values[i] ?? Number.NaN);
      largest = Math.max(largest, m === 0 ? value : 0);
      modeOne = Math.max(modeOne, s === overhead && m === 1 ? value : 0);
    }
    expect([prepared.grid.sunCosines[overhead], modeOne <= 1e-12 * largest]).toEqual([1, true]);
  });
});

/** A column's terms but its absorbers, then `count` copies of its absorber under new names. */
function withAbsorberCopies(column: SpectralColumn, count: number): SpectralColumn {
  const absorber = column.terms.find((t) => t.source.kind === "absorber");
  if (absorber === undefined) {
    throw new Error("the column has an absorber");
  }
  const copies: SpectralTerm[] = [];
  for (let i = 0; i < count; i += 1) {
    copies.push({
      name: `${absorber.name}:${i}`,
      density: absorber.density,
      source: absorber.source,
    });
  }
  return { ...column, terms: [...column.terms.filter((t) => t !== absorber), ...copies] };
}

describe("the bake's exponential sums in its bins", () => {
  // Five preparations' fits of Titan's methane: 3.0 s beside #107's sweep (load 5.9, 2026-10-26).
  it(
    `takes the weakest absorber as one term while a bin's product of terms passes ${THICK_BAKE_MAX_TERMS_PER_BIN}`,
    { timeout: 60_000 },
    () => {
      const fixture = titanClassFixture();
      const spec = fixture.specs[0];
      if (spec === undefined) {
        throw new Error("the Titan-class column has methane");
      }
      const curves = absorberBinCurves(spec, fixture.sun);
      const alone = prepareBake(fixture.column, [{ spectrum: fixture.sun, absorbers: [curves] }]);
      const copies = 4;
      const absorbers: AbsorberBinCurves[] = Array.from({ length: copies }, () => curves);
      const together = prepareBake(withAbsorberCopies(fixture.column, copies), [
        { spectrum: fixture.sun, absorbers },
      ]);
      const expected = (alone.lights[0]?.bins ?? []).map((bin) => {
        // The rule's oracle: the copies' terms multiply, one copy at a time taken as one term.
        const counts = Array.from({ length: copies }, () => bin.terms.length);
        for (const [i] of counts.entries()) {
          if (counts.reduce((p, c) => p * c, 1) <= THICK_BAKE_MAX_TERMS_PER_BIN) {
            break;
          }
          counts[i] = 1;
        }
        return counts.reduce((p, c) => p * c, 1);
      });
      const got = (together.lights[0]?.bins ?? []).map((bin) => bin.terms.length);
      expect([got, Math.max(...got) <= THICK_BAKE_MAX_TERMS_PER_BIN]).toEqual([expected, true]);
    },
  );
});

describe("the bake's lights", () => {
  it("shares one unabsorbed solve between lights whose bins have no gases, converting each under its own sun", () => {
    const red = { spectrum: planckBakeSpectrum(3_200), absorbers: [] };
    const prepared = prepareBake(
      rayleighColumn(),
      [UNABSORBED, red],
      smallGrid(Float64Array.of(0.5)),
    );
    const [sun, dwarf] = prepared.lights;
    const shared = (sun?.bins ?? []).every((bin, b) => bin.terms[0] === dwarf?.bins[b]?.terms[0]);
    const { slots } = bakeMultipleScattering(prepared, titanBody(), 0);
    expect([
      shared,
      slots.length,
      slots[0]?.values.some((v, i) => v !== slots[1]?.values[i]),
    ]).toEqual([true, 2, true]);
  });
});

describe("the bake's refusals and ground", () => {
  const titanAir = once(() => titanClassFixture());

  it("refuses a grid whose layers do not take whole strides, or whose cosines leave (0, 1]", () => {
    const { column } = titanAir();
    const light = lightOf(titanAir());
    expect(() =>
      prepareBake(column, [light], { ...smallGrid(Float64Array.of(0.5)), layers: 15 }),
    ).toThrow(RangeError);
    expect(() =>
      prepareBake(column, [light], {
        ...smallGrid(Float64Array.of(0.5)),
        sunCosines: Float64Array.of(0, 1),
      }),
    ).toThrow(RangeError);
  });

  it("refuses a light whose absorbers' curves do not match the column's absorber terms", () => {
    const { column } = titanAir();
    const curves = lightOf(titanAir()).absorbers[0];
    if (curves === undefined) {
      throw new Error("the Titan-class column has methane");
    }
    const sun = titanAir().sun;
    expect(() => prepareBake(column, [{ spectrum: sun, absorbers: [] }])).toThrow(RangeError);
    expect(() =>
      prepareBake(column, [{ spectrum: sun, absorbers: [{ ...curves, kind: "pair" }] }]),
    ).toThrow(RangeError);
    const rows = { ...curves, logPressures: Float64Array.of(10, 11) };
    expect(() => prepareBake(column, [{ spectrum: sun, absorbers: [rows] }])).toThrow(RangeError);
  });

  it("takes a bin's ground albedo linear in wavelength between the channels', and the nearer channel's beyond them", () => {
    const albedo = [0.1, 0.5, 0.9] as const;
    expect(
      [400, 433.625, (433.625 + 544) / 2, 544, (544 + 645) / 2, 645, 750].map(
        (nm) => Math.round(binGroundAlbedo(albedo, nm) * 1e12) / 1e12,
      ),
    ).toEqual([0.9, 0.9, 0.7, 0.5, 0.3, 0.1, 0.1]);
  });
});

/** An ice giant's body: Uranus's level spheroid, two latitude bands. */
function uranusBody(): ThickBakeBody {
  const figure = {
    equatorialRadiusM: URANUS_SPHEROID.equatorialRadiusM,
    polarRadiusM: URANUS_SPHEROID.polarRadiusM,
  };
  return {
    figure,
    gravity: gravityOfSpheroid(URANUS_SPHEROID),
    bottomRadiusM: mediumTableRadiusM(figure),
  };
}

describe("the bake's latitude bands", () => {
  const prepared = once(() =>
    prepareBake(rayleighColumn(), [UNABSORBED], smallGrid(Float64Array.of(0.5))),
  );

  it("bakes a one-band body once", () => {
    const body = titanBody();
    expect([
      thickBakeBands(body).length,
      bakeMultipleScattering(prepared(), body, 0).band.gravityRatio,
    ]).toEqual([1, 1]);
  });

  it("refuses a band the body does not have", () => {
    expect(() => bakeMultipleScattering(prepared(), titanBody(), 1)).toThrow(RangeError);
  });

  it("gives a two-band body's bands vertical optical depths in the inverse ratio of their gravities, to 10⁻¹²", () => {
    const body = uranusBody();
    const [first, second] = [0, 1].map((b) => bakeMultipleScattering(prepared(), body, b));
    if (first === undefined || second === undefined) {
      throw new Error("two bands");
    }
    const ratio = second.band.gravityRatio / first.band.gravityRatio;
    const worst = Math.max(
      ...Array.from(first.verticalOpticalDepth, (tau, bin) =>
        Math.abs(tau / (second.verticalOpticalDepth[bin] ?? Number.NaN) / ratio - 1),
      ),
    );
    expect([thickBakeBands(body).length, worst < 1e-12]).toEqual([2, true]);
  });

  it.for([
    ["a sphere of one band", titanBody],
    ["an ice giant of two", uranusBody],
  ] as const)("lays %s's bands as the tables do (tables.ts's tablesPlanOf)", ([, bodyOf]) => {
    const body = bodyOf();
    expect(thickBakeBands(body)).toEqual(tablesPlanOf(body, body.bottomRadiusM).bands);
  });
});
