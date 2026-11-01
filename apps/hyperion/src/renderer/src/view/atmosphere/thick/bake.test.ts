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
import {
  ABSORBER_TRANSMITTANCE_FLOOR,
  absorberBinCurves,
  type AbsorberBinCurves,
  type AbsorberCurve,
} from "../absorbers";
import { mediumTableRadiusM } from "../assemble";
import { MARS_FIT_COLUMN } from "../channels";
import { BAKE_BIN_COUNT, BAKE_WAVELENGTHS_NM, CHANNEL_WAVELENGTHS_NM } from "../medium";
import { MARS_CASE, MARS_REFERENCE_RADIUS_M } from "../reference/cases";
import { normalizedAssociatedLegendre } from "./discreteOrdinates";
import { tablesPlanOf } from "../tables";
import {
  BAKE_CEILING_S,
  bakeBins,
  bakeBinSolve,
  bakeLayering,
  bakeMultipleScattering,
  bakePointsOf,
  binGroundAlbedo,
  curvedStreamShares,
  EVERY_BAKE_BIN,
  gatherBins,
  NO_OWN_CHANNELS,
  OWN_WAVELENGTH_COLUMNS,
  type OwnChannels,
  ownChannelPoint,
  ownWavelengthChannels,
  type MsSourceTable,
  prepareBake,
  type PreparedBake,
  THICK_BAKE_GRID,
  THICK_BAKE_MAX_TERMS_PER_BIN,
  THICK_BAKE_MODES,
  THICK_BAKE_REFERENCE_BIN,
  THICK_BAKE_STREAMS,
  thickBakeBands,
  type ThickBakeBody,
  type ThickBakeGrid,
  type ThickBakeLight,
  thickBakeLights,
} from "./bake";
import {
  lightOf,
  type ThickFixture,
  titanBody,
  titanClassColumn,
  titanClassFixture,
  venusClassFixture,
} from "../../../test/bakeFixtures";
import { doubleGaussQuadrature } from "./discreteOrdinates";
import { keptColumnOf, sphereBakeBody } from "./keptBake";
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
 * methane; IMS 1.2% and 5.0% (2026-10-26), and Titan's 10.45% once the grid took its rows below the
 * horizontal (R08.T14.d.2).
 */
const FIXTURES: ReadonlyArray<readonly [string, FixtureCase]> = [
  ["Venus-class column", { baked: venus, fitBounds: [0.04, 0.013], imsBound: 0.02 }],
  ["Titan-class column", { baked: titan, fitBounds: [2e-3, 2e-3], imsBound: 0.11 }],
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
function smallGrid(
  upwardCosines: Float64Array,
  sunCosines: Float64Array = Float64Array.of(0.3, 0.7, 1),
): ThickBakeGrid {
  return { layers: 16, altitudeStride: 2, sunCosines, upwardCosines };
}

/** Sun cosines below the horizontal and above it, Titan-class air's lit upper air at each (R08.T14.d.2). */
const TWILIGHT_SUNS = Float64Array.of(-0.1, -1 / 64, 0.3, 1);

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
  it.each([
    ["above the horizontal", Float64Array.of(0.3, 0.7, 1)],
    ["below it too (R08.T14.d.2)", TWILIGHT_SUNS],
  ] as const)(
    "integrates over all directions to σ_s times the diffuse field's whole mean intensity and the aureole, to 10⁻¹², %s",
    (_name, sunCosines) => {
      const fixture = titanClassFixture();
      const quadrature = doubleGaussQuadrature(16);
      const prepared = prepareBake(
        fixture.column,
        [lightOf(fixture)],
        smallGrid(quadrature.nodes, sunCosines),
      );
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
    },
  );

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
          for (const deltaPhi of [0, Math.PI / 3, Math.PI / 2, Math.PI]) {
            let stored = 0;
            for (let m = 0; m < THICK_BAKE_MODES; m += 1) {
              stored += (solve.source[cell + m] ?? Number.NaN) * Math.cos(m * deltaPhi);
            }
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

  it("holds no m = 1 or 2 at an overhead sun, where Λ_lᵐ(1) = 0 for m ≥ 1", () => {
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
    const higher = [0, 0];
    for (let i = 0; i < values.length; i += 1) {
      const s = Math.floor(i / perSun) % suns;
      const m = Math.floor(i / 3) % THICK_BAKE_MODES;
      const value = Math.abs(values[i] ?? Number.NaN);
      largest = Math.max(largest, m === 0 ? value : 0);
      if (s === overhead && m > 0) {
        higher[m - 1] = Math.max(higher[m - 1] ?? 0, value);
      }
    }
    expect([THICK_BAKE_MODES, prepared.grid.sunCosines[overhead]]).toEqual([3, 1]);
    expect(higher.map((value) => value <= 1e-12 * largest)).toEqual([true, true]);
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

describe("the bake's channels (R08.T14.d.1)", () => {
  it(
    "stores each channel's J: each bin's E over its own kernel scattering σ_s (1 − f_c) at the altitude, converted to the channel, to f32's rounding",
    { timeout: BAKE_TIMEOUT_MS },
    () => {
      // TEST MARS's dust carries a forward delta, so that (1 − f_c) is not 1.
      const prepared = prepareBake(
        keptColumnOf(MARS_CASE),
        [UNABSORBED],
        smallGrid(Float64Array.of(0.2, 0.7)),
      );
      const body = sphereBakeBody(MARS_REFERENCE_RADIUS_M, MARS_FIT_COLUMN.referenceGravityMS2);
      const band = thickBakeBands(body)[0];
      const factors = prepared.lights[0]?.conversion.factors;
      if (band === undefined || factors === undefined) {
        throw new Error("Mars bakes one band under one light");
      }
      const { values } = slotOf(bakeMultipleScattering(prepared, body, 0));
      const perAltitude = values.length / 3 / prepared.altitudesM.length;
      const expected = new Float64Array(values.length);
      for (let b = 0; b < BAKE_BIN_COUNT; b += 1) {
        for (const term of prepared.lights[0]?.bins[b]?.terms ?? []) {
          for (const [i, emission] of bakeBinSolve(prepared, band, b, term).source.entries()) {
            const point = prepared.pointOptics[b]?.[Math.floor(i / perAltitude)];
            const kernel = (point?.scatteringPerM ?? Number.NaN) * (1 - (point?.forwardDelta ?? 0));
            const j = kernel > 0 ? (term.weight * emission) / kernel : 0;
            for (let c = 0; c < 3; c += 1) {
              expected[i * 3 + c] =
                (expected[i * 3 + c] ?? 0) + (factors[c]?.[b] ?? Number.NaN) * j;
            }
          }
        }
      }
      let largest = 0;
      let worst = 0;
      for (const [i, value] of values.entries()) {
        const want = expected[i] ?? Number.NaN;
        largest = Math.max(largest, Math.abs(want));
        worst = Math.max(worst, Math.abs(value - want));
      }
      const forward = prepared.pointOptics.some((bin) => bin.some((p) => p.forwardDelta > 0));
      expect([forward, largest > 0, worst <= 1e-6 * largest]).toEqual([true, true, true]);
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

  it("refuses a grid whose layers do not take whole strides, or whose sun cosines leave [−1, 1] or hold 0", () => {
    const { column } = titanAir();
    const light = lightOf(titanAir());
    expect(() =>
      prepareBake(column, [light], { ...smallGrid(Float64Array.of(0.5)), layers: 15 }),
    ).toThrow(RangeError);
    for (const sunCosines of [Float64Array.of(0, 1), Float64Array.of(-1.5, 1)]) {
      expect(() =>
        prepareBake(column, [light], { ...smallGrid(Float64Array.of(0.5)), sunCosines }),
      ).toThrow(/sun cosines/u);
    }
  });

  it("refuses a grid whose view cosines leave (0, 1]", () => {
    const { column } = titanAir();
    expect(() =>
      prepareBake(column, [lightOf(titanAir())], smallGrid(Float64Array.of(-0.5))),
    ).toThrow(/view cosines/u);
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

describe("the bake below the horizontal (R08.T14.d.2)", () => {
  it("holds Design note 9's grid: 14 rows of 1/64 below the horizontal to the cell edge −7/32, then the 32 daylit rows of 1/32", () => {
    const suns = THICK_BAKE_GRID.sunCosines;
    expect(suns.length).toBe(46);
    for (const [s, mu] of suns.entries()) {
      expect(mu).toBe(s < 14 ? -(13.5 - s) / 64 : (s - 13.5) / 32);
    }
    expect([(suns[0] ?? Number.NaN) - 0.5 / 64, suns[13], suns[14], suns.at(-1)]).toEqual([
      -7 / 32,
      -1 / 128,
      1 / 64,
      63 / 64,
    ]);
    expect(suns.every((mu) => mu !== 0)).toBe(true);
  });

  /** Titan-class Rayleigh air's bake at a μ₀ past its top's shadow, then {@link TWILIGHT_SUNS}. */
  const twilight = once(() => {
    const column = rayleighColumn();
    const body = titanBody();
    const band = thickBakeBands(body)[0];
    if (band === undefined) {
      throw new Error("Titan has one band");
    }
    // −√(1 − (R ÷ (R + h_top))²): the μ₀ past which the column's top lies in the planet's shadow.
    const r = band.bottomRadiusM;
    const limit = -Math.sqrt(1 - (r / (r + column.topHeightM)) ** 2);
    const sunCosines = Float64Array.of(limit - 0.05, ...TWILIGHT_SUNS);
    const views = Float64Array.of(0.2, 0.6, 0.9);
    const prepared = prepareBake(column, [UNABSORBED], smallGrid(views, sunCosines));
    const { values } = slotOf(bakeMultipleScattering(prepared, body, 0));
    const perSun = 2 * views.length * THICK_BAKE_MODES * 3;
    /** Each entry with its sun cosine's index and mode. */
    const entries = Array.from(values, (value, i) => ({
      value,
      sun: Math.floor(i / perSun) % sunCosines.length,
      mode: Math.floor(i / 3) % THICK_BAKE_MODES,
    }));
    return { entries, limit };
  });

  it("holds a finite J at every sun cosine", () => {
    expect(twilight().entries.every(({ value }) => Number.isFinite(value))).toBe(true);
  });

  it("holds a J whose m = 0 is at least 0 below the horizontal and above it, and lit", () => {
    const zeroth = twilight()
      .entries.filter(({ sun, mode }) => sun > 0 && mode === 0)
      .map(({ value }) => value);
    const largest = Math.max(...zeroth);
    expect([largest > 0, Math.min(...zeroth) >= -1e-12 * largest]).toEqual([true, true]);
  });

  it("joins J across the horizontal: at μ₀ ±10⁻⁶ every entry above 10⁻³ of the largest agrees to 10⁻³", () => {
    const views = Float64Array.of(0.2, 0.6, 0.9);
    const prepared = prepareBake(
      rayleighColumn(),
      [UNABSORBED],
      smallGrid(views, Float64Array.of(-1e-6, 1e-6)),
    );
    const { values } = slotOf(bakeMultipleScattering(prepared, titanBody(), 0));
    const perSun = 2 * views.length * THICK_BAKE_MODES * 3;
    const largest = Math.max(...Array.from(values, Math.abs));
    let worst = 0;
    for (let i = 0; i < values.length; i += 2 * perSun) {
      for (let j = 0; j < perSun; j += 1) {
        const below = values[i + j] ?? Number.NaN;
        const above = values[i + perSun + j] ?? Number.NaN;
        if (Math.abs(above) >= 1e-3 * largest) {
          worst = Math.max(worst, Math.abs(below - above) / Math.abs(above));
        }
      }
    }
    expect(worst).toBeLessThan(1e-3);
  });

  it("holds 0 past the shadow of the table's top", () => {
    const { entries, limit } = twilight();
    const past = entries.filter(({ sun }) => sun === 0).map(({ value }) => Math.abs(value));
    expect([limit < -0.1, Math.max(...past)]).toEqual([true, 0]);
  });
});

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

  it("refuses solves of fewer modes than the bake's or more than its streams (R08.T15.a.2)", () => {
    for (const solveModes of [THICK_BAKE_MODES - 1, THICK_BAKE_STREAMS + 1, 3.5]) {
      expect(() => bakeMultipleScattering(prepared(), titanBody(), 0, { solveModes })).toThrow(
        /modes, not/u,
      );
    }
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

describe("the bake's span and layout (R08.T15's deck split)", () => {
  const column = once(rayleighColumn);
  const grid = smallGrid(Float64Array.of(0.5));
  const reference = once(
    () =>
      spectralOptics(column(), [BAKE_WAVELENGTHS_NM[THICK_BAKE_REFERENCE_BIN] ?? Number.NaN])[0] ??
      [],
  );

  it("lays the whole column's layers to the bit at the default span", () => {
    const whole = { bottomM: 0, topM: column().topHeightM };
    expect(bakeLayering(column(), reference(), grid, whole)).toEqual(
      bakeLayering(column(), reference(), grid),
    );
  });

  it("refuses a span whose bottom is not below its top", () => {
    for (const span of [
      { bottomM: 5e3, topM: 5e3 },
      { bottomM: 6e3, topM: 5e3 },
      { bottomM: Number.NaN, topM: 5e3 },
    ]) {
      expect(() => bakeLayering(column(), reference(), grid, span)).toThrow(RangeError);
    }
  });

  it("refuses a layout whose table does not hold the grid's layers", () => {
    const layering = bakeLayering(column(), reference(), { ...grid, layers: 8 });
    expect(() =>
      prepareBake(column(), [UNABSORBED], grid, undefined, { layering, fitLayering: layering }),
    ).toThrow(RangeError);
  });

  it("refuses a layout whose fit departs from the table's layers", () => {
    const layering = bakeLayering(column(), reference(), grid);
    const other = bakeLayering(column(), reference(), grid, {
      bottomM: 1e3,
      topM: column().topHeightM,
    });
    expect(() =>
      prepareBake(column(), [UNABSORBED], grid, undefined, { layering, fitLayering: other }),
    ).toThrow(RangeError);
  });
});

/** A curve of one row whose channels' −ln T rise linearly in ln u, each to the floor at `floorAtU`. */
function curveReaching(floorAtU: readonly [number, number, number]): AbsorberCurve {
  const nodes = 61;
  const logColumns = Float64Array.from({ length: nodes }, (_, j) => (j - 30) * (Math.log(10) / 10));
  const floor = -Math.log(ABSORBER_TRANSMITTANCE_FLOOR);
  const depthOf = (at: number): Float64Array =>
    logColumns.map((lnU) => Math.min(floor, Math.max(0, (floor * (lnU + 7)) / (Math.log(at) + 7))));
  const depths = [depthOf(floorAtU[0]), depthOf(floorAtU[1]), depthOf(floorAtU[2])] as const;
  return {
    kind: "column",
    logColumns,
    logPressures: Float64Array.of(Math.log(1e5)),
    readTopLogPressure: Math.log(1e7),
    transmittance: [
      depths[0].map((d) => Math.exp(-d)),
      depths[1].map((d) => Math.exp(-d)),
      depths[2].map((d) => Math.exp(-d)),
    ],
    opticalDepth: depths,
  };
}

describe("the bake's channels at their own wavelengths (R08.T14.d.4)", () => {
  /** The Titan-class fixture's air and methane without its haze: cheap optics, a strong gas. */
  const methane = once(() => {
    const fixture = titanClassFixture();
    const column: SpectralColumn = {
      ...fixture.column,
      terms: fixture.column.terms.filter((term) => term.source.kind !== "aerosol"),
    };
    return { fixture, column, lights: thickBakeLights(column, fixture.specs, [fixture.sun]) };
  });
  const grid = smallGrid(Float64Array.of(0.25, 0.75));
  /** Green and blue at their own wavelengths, red by the bins: the Titan-class column's rule. */
  const OWN: OwnChannels = [false, true, true];

  it("takes a channel at its own wavelength where its curves stay off the floor out to 100 U, under every light", () => {
    const light = (at: readonly [number, number, number]): ThickBakeLight => ({
      ...UNABSORBED,
      absorbers: [],
      channels: [curveReaching(at)],
    });
    const past = OWN_WAVELENGTH_COLUMNS * 1.5;
    const short = OWN_WAVELENGTH_COLUMNS / 5;
    expect([
      ownWavelengthChannels([light([short, past, Number.POSITIVE_INFINITY])]),
      ownWavelengthChannels([light([past, past, past]), light([past, short, past])]),
      ownWavelengthChannels([{ ...UNABSORBED, channels: [] }]),
      ownWavelengthChannels([UNABSORBED]),
      ownWavelengthChannels([]),
    ]).toEqual([
      [false, true, true],
      [true, false, true],
      [true, true, true],
      [false, false, false],
      [false, false, false],
    ]);
  });

  it("keeps the Titan-class methane's red on the bins and takes its green and blue at their own wavelengths", () => {
    expect(ownWavelengthChannels(methane().lights)).toEqual(OWN);
  });

  it("solves no point and changes no bit where no channel is own", () => {
    expect(bakePointsOf(NO_OWN_CHANNELS)).toEqual(EVERY_BAKE_BIN);
  });

  it(
    "stores an own channel's J as its point's, E over its kernel scattering at the channel's wavelength, and a bins channel's to the bit",
    { timeout: BAKE_TIMEOUT_MS },
    () => {
      const { fixture, column, lights } = methane();
      const own = prepareBake(column, lights, grid, undefined, undefined, bakePointsOf(OWN), OWN);
      const bins = prepareBake(column, lights, grid);
      const band = thickBakeBands(fixture.body)[0];
      if (band === undefined) {
        throw new Error("the Titan-class body has one band");
      }
      const ownValues = slotOf(bakeMultipleScattering(own, fixture.body, 0)).values;
      const binValues = slotOf(bakeMultipleScattering(bins, fixture.body, 0)).values;
      const perAltitude = ownValues.length / 3 / own.altitudesM.length;
      // Green's and blue's J from their points' solves alone.
      const expected = new Float64Array(ownValues.length);
      for (const c of [1, 2] as const) {
        const point = ownChannelPoint(c);
        for (const term of own.lights[0]?.bins[point]?.terms ?? []) {
          for (const [i, emission] of bakeBinSolve(own, band, point, term).source.entries()) {
            const optics = own.pointOptics[point]?.[Math.floor(i / perAltitude)];
            const kernel =
              (optics?.scatteringPerM ?? Number.NaN) * (1 - (optics?.forwardDelta ?? 0));
            expected[i * 3 + c] =
              (expected[i * 3 + c] ?? 0) + (kernel > 0 ? (term.weight * emission) / kernel : 0);
          }
        }
      }
      let largest = 0;
      let worst = 0;
      let redSame = true;
      for (let i = 0; i < ownValues.length; i += 1) {
        if (i % 3 === 0) {
          redSame &&= Object.is(ownValues[i], binValues[i]);
          continue;
        }
        const want = expected[i] ?? Number.NaN;
        largest = Math.max(largest, Math.abs(want));
        worst = Math.max(worst, Math.abs((ownValues[i] ?? Number.NaN) - want));
      }
      expect([
        own.lights[0]?.bins.length,
        Array.from(own.groundAlbedo.slice(BAKE_BIN_COUNT)),
        redSame,
        largest > 0,
        worst <= 1e-6 * largest,
      ]).toEqual([BAKE_BIN_COUNT + 3, [0.3, 0.3, 0.3], true, true, true]);
    },
  );

  it(
    "gathers shares that bake the points beside bin 0 to the whole bake's bits",
    { timeout: BAKE_TIMEOUT_MS },
    () => {
      const { fixture, column, lights } = methane();
      const points = bakePointsOf(OWN).filter((b) => b >= BAKE_BIN_COUNT);
      const share = (bins: ReadonlyArray<number>): ReturnType<typeof bakeBins> =>
        bakeBins(
          prepareBake(column, lights, grid, undefined, undefined, bins, OWN),
          fixture.body,
          0,
        );
      const whole = bakeMultipleScattering(
        prepareBake(column, lights, grid, undefined, undefined, bakePointsOf(OWN), OWN),
        fixture.body,
        0,
      );
      const gathered = gatherBins([
        share([...EVERY_BAKE_BIN.filter((b) => b % 2 === 0), ...points]),
        share(EVERY_BAKE_BIN.filter((b) => b % 2 === 1)),
      ]);
      expect(gathered).toEqual(whole);
    },
  );

  it("refuses a channel's point unless the channel is own, and an own channel under a light without channel curves", () => {
    const { column, lights } = methane();
    const light = lights[0];
    if (light === undefined) {
      throw new Error("the methane column bakes under one light");
    }
    const { channels: _, ...bare } = light;
    expect(
      [
        () => prepareBake(column, lights, grid, undefined, undefined, [0, ownChannelPoint(0)], OWN),
        () => prepareBake(column, [bare], grid, undefined, undefined, bakePointsOf(OWN), OWN),
      ].map((make) => {
        try {
          make();
          return "made";
        } catch (error: unknown) {
          return error instanceof RangeError ? "refused" : "threw";
        }
      }),
    ).toEqual(["refused", "refused"]);
  });
});

/** An exponential column of `tau` over 16 layers of one scale height in 10, its interfaces' h, m. */
function exponentialColumn(tau: number): {
  readonly heights: Float64Array;
  readonly depths: Float64Array;
} {
  const scaleM = 20_000;
  const layers = 16;
  const heights = Float64Array.from({ length: layers + 1 }, (_, q) => (layers - q) * scaleM);
  const above = (h: number): number => tau * Math.exp(-h / scaleM);
  const depths = Float64Array.from(
    { length: layers },
    (_, q) => above(heights[q + 1] ?? 0) - above(heights[q] ?? 0),
  );
  return { heights, depths };
}

describe("the bake's curved streams (R08.T14.d.4)", () => {
  const cosines = Float64Array.of(0.02, 0.1, 0.5, 1);

  it("takes every share as 1 on a sphere so large that it is plane-parallel, either way, to 10⁻⁵", () => {
    const { heights, depths } = exponentialColumn(10);
    const shares = [
      ...curvedStreamShares(1e13, heights, depths, cosines, "up"),
      ...curvedStreamShares(1e13, heights, depths, cosines, "down"),
    ];
    expect(Math.max(...shares.map((share) => Math.abs(share - 1)))).toBeLessThan(1e-5);
  });

  it("takes a shallow stream up at altitude as its chord's thin share on a Titan-sized sphere, and 1 at the ground and deep in the column", () => {
    const { heights, depths } = exponentialColumn(10);
    const up = curvedStreamShares(2_575_000, heights, depths, cosines, "up");
    const count = cosines.length;
    const top = up[0] ?? Number.NaN;
    const ground = up[16 * count + 0] ?? Number.NaN;
    const deep = up[14 * count + 3] ?? Number.NaN;
    expect([top < 0.1, top > 0, ground, deep]).toEqual([true, true, 1, expect.closeTo(1, 6)]);
  });

  it("takes a shallow stream down below a thin top as its climbing ray's Chapman share, under 1", () => {
    const { heights, depths } = exponentialColumn(10);
    const down = curvedStreamShares(2_575_000, heights, depths, cosines, "down");
    const count = cosines.length;
    // At 8 scale heights, τ above some 3.4 × 10⁻³: the grazing stream's slant is the shorter.
    const high = down[8 * count + 0] ?? Number.NaN;
    const steep = down[8 * count + 3] ?? Number.NaN;
    expect([high < 1, high > 0.01, steep]).toEqual([true, true, expect.closeTo(1, 6)]);
  });

  it(
    "bakes to the bit without them, and moves the table with them",
    { timeout: BAKE_TIMEOUT_MS },
    () => {
      const fixture = titanClassFixture();
      const column: SpectralColumn = {
        ...fixture.column,
        terms: fixture.column.terms.filter((term) => term.source.kind !== "aerosol"),
      };
      const prepared = prepareBake(column, [lightOf(fixture)], smallGrid(Float64Array.of(0.25)));
      const plain = slotOf(bakeMultipleScattering(prepared, fixture.body, 0)).values;
      const off = slotOf(
        bakeMultipleScattering(prepared, fixture.body, 0, { curvedStreams: false }),
      ).values;
      const on = slotOf(
        bakeMultipleScattering(prepared, fixture.body, 0, { curvedStreams: true }),
      ).values;
      expect([
        off.every((value, i) => Object.is(value, plain[i])),
        on.some((value, i) => value !== plain[i]),
        on.every(Number.isFinite),
      ]).toEqual([true, true, true]);
    },
  );
});
