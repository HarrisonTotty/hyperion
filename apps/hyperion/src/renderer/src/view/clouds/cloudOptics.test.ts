import { describe, expect, it } from "vitest";

import {
  CONSERVATIVE_CO_ALBEDO,
  deltaEddington,
  type DeltaEddingtonInput,
  type SlabFluxFractions,
} from "./cloudOptics";
import { SLAB_CASES, type SlabCase } from "./slabFixtures";

function conservativeAt(c: SlabCase): SlabFluxFractions {
  return deltaEddington({ tau: c.tau, asymmetry: c.g, singleScatterAlbedo: 1, mu0: c.mu0 });
}

function totalTransmittance(r: SlabFluxFractions): number {
  return r.directTransmittance + r.diffuseTransmittance;
}

/** The four parts by name, for {@link misses}. */
function labelled(r: SlabFluxFractions): Readonly<Record<string, number>> {
  return {
    reflectance: r.reflectance,
    directTransmittance: r.directTransmittance,
    diffuseTransmittance: r.diffuseTransmittance,
    absorptance: r.absorptance,
  };
}

/**
 * The parts of `actual` further than `tolerance` from `expected`, each named, so that a failure
 * says which part missed; empty when all meet.
 */
function misses(
  actual: Readonly<Record<string, number>>,
  expected: Readonly<Record<string, number>>,
  tolerance: number,
): string[] {
  const off: string[] = [];
  for (const [name, want] of Object.entries(expected)) {
    const got = actual[name];
    if (got === undefined || !(Math.abs(got - want) <= tolerance)) {
      off.push(`${name}: ${got} against ${want}`);
    }
  }
  return off;
}

/**
 * Plain Eddington's conservative reflectance: the δ-Eddington formula with g and τ in place of g′
 * and τ′ (Meador and Weaver 1980's conservative solution, eq. 24, unscaled).
 */
function plainEddingtonReflectance(tau: number, g: number, mu0: number): number {
  const g1 = (3 * (1 - g)) / 4;
  const g3 = (2 - 3 * g * mu0) / 4;
  return (g1 * tau + (g3 - g1 * mu0) * (1 - Math.exp(-tau / mu0))) / (1 + g1 * tau);
}

/** The shooting's state: [P, M], the source-driven solution, then [Pₕ, Mₕ], the source-free one. */
type ShootingState = readonly [number, number, number, number];

/** `s + scale × d`, element by element. */
function axpy(s: ShootingState, d: ShootingState, scale: number): ShootingState {
  return [s[0] + scale * d[0], s[1] + scale * d[1], s[2] + scale * d[2], s[3] + scale * d[3]];
}

/** RK4's step from `s` over `h`: s + h(k₁ + 2k₂ + 2k₃ + k₄) ÷ 6. */
function rk4Step(
  s: ShootingState,
  h: number,
  k: readonly [ShootingState, ShootingState, ShootingState, ShootingState],
): ShootingState {
  const [k1, k2, k3, k4] = k;
  return axpy(axpy(axpy(axpy(s, k1, h / 6), k2, h / 3), k3, h / 3), k4, h / 6);
}

/** R and the total transmittance T of the scaled problem. */
interface TwoStreamSolution {
  readonly reflectance: number;
  readonly transmittance: number;
}

/**
 * R and T of the δ-scaled two-stream equations with the Eddington coefficients, by RK4 shooting:
 * the formulas' independent check of the closed forms.
 *
 * @remarks
 * The equations (Meador and Weaver 1980, eqs. 12 and 13; Toon et al. 1989 state the same form, not
 * read here), t from 0 at the top to τ′, F⁺ up and F⁻ down, πF₀ = 1:
 *
 * - dF⁺/dt = γ₁F⁺ − γ₂F⁻ − ϖ′γ₃e^(−t÷μ₀);
 * - dF⁻/dt = γ₂F⁺ − γ₁F⁻ + ϖ′γ₄e^(−t÷μ₀);
 * - F⁻(0) = 0 and F⁺(τ′) = 0.
 *
 * The system is linear, so the solution with F⁺(0) = a is the source-driven one from (0, 0) plus a
 * times the source-free one from (1, 0), and a is solved for F⁺(τ′) = 0. Then R = F⁺(0) ÷ μ₀ and
 * T = [F⁻(τ′) + μ₀e^(−τ′÷μ₀)] ÷ μ₀.
 */
function twoStreamByShooting(input: DeltaEddingtonInput, steps: number): TwoStreamSolution {
  const { tau, asymmetry: g, singleScatterAlbedo: w0, mu0 } = input;
  const f = g * g;
  const t = (1 - w0 * f) * tau;
  const w = ((1 - f) * w0) / (1 - w0 * f);
  const gs = (g - f) / (1 - f);
  const g1 = (7 - w * (4 + 3 * gs)) / 4;
  const g2 = -(1 - w * (4 - 3 * gs)) / 4;
  const g3 = (2 - 3 * gs * mu0) / 4;
  const g4 = 1 - g3;
  const derivative = (x: number, s: ShootingState): ShootingState => {
    const [p, m, ph, mh] = s;
    const beam = Math.exp(-x / mu0);
    return [
      g1 * p - g2 * m - w * g3 * beam,
      g2 * p - g1 * m + w * g4 * beam,
      g1 * ph - g2 * mh,
      g2 * ph - g1 * mh,
    ];
  };
  const h = t / steps;
  let state: ShootingState = [0, 0, 1, 0];
  for (let n = 0; n < steps; n += 1) {
    const x = n * h;
    const k1 = derivative(x, state);
    const k2 = derivative(x + h / 2, axpy(state, k1, h / 2));
    const k3 = derivative(x + h / 2, axpy(state, k2, h / 2));
    const k4 = derivative(x + h, axpy(state, k3, h));
    state = rk4Step(state, h, [k1, k2, k3, k4]);
  }
  const [p, m, ph, mh] = state;
  const a = -p / ph;
  return {
    reflectance: a / mu0,
    transmittance: (m + a * mh + mu0 * Math.exp(-t / mu0)) / mu0,
  };
}

const ODE_STEPS = 20_000;

/** R and T of `deltaEddington` and of the shooting that are further than 1e-9 apart. */
function missesOfTheOde(input: DeltaEddingtonInput): string[] {
  const closed = deltaEddington(input);
  const ode = twoStreamByShooting(input, ODE_STEPS);
  return misses(
    { reflectance: closed.reflectance, transmittance: totalTransmittance(closed) },
    { reflectance: ode.reflectance, transmittance: ode.transmittance },
    1e-9,
  );
}

/** The closure grid: ϖ₀, g and μ₀ as ruled, and τ at ten values a decade over [10⁻³, 10³]. */
const GRID_ALBEDOS: ReadonlyArray<number> = [0.99, 0.999, 1];
const GRID_ASYMMETRIES: ReadonlyArray<number> = [0, 0.5, 0.75, 0.85, 0.9];
const GRID_MU0: ReadonlyArray<number> = [0.05, 0.2, 0.5, 1];
const GRID_TAUS: ReadonlyArray<number> = Array.from({ length: 61 }, (_, i) => 10 ** (-3 + i / 10));

interface GridColumn {
  readonly label: string;
  readonly singleScatterAlbedo: number;
  /** The results along τ, in {@link GRID_TAUS}' order. */
  readonly results: ReadonlyArray<SlabFluxFractions>;
}

function gridColumns(): ReadonlyArray<GridColumn> {
  return GRID_ALBEDOS.flatMap((singleScatterAlbedo) =>
    GRID_ASYMMETRIES.flatMap((asymmetry) =>
      GRID_MU0.map((mu0) => ({
        label: `ϖ₀ ${singleScatterAlbedo}, g ${asymmetry}, μ₀ ${mu0}`,
        singleScatterAlbedo,
        results: GRID_TAUS.map((tau) =>
          deltaEddington({ tau, asymmetry, singleScatterAlbedo, mu0 }),
        ),
      })),
    ),
  );
}

/**
 * The μ₀ at which kμ₀ = 1, from γ₁ and γ₂ directly: the rulings' example is ϖ₀ 0.5 and g 0.5, where
 * k = 1.212183.
 */
function singularMu0(w0 = 0.5, g = 0.5): number {
  const f = g * g;
  const w = ((1 - f) * w0) / (1 - w0 * f);
  const gs = g / (1 + g);
  const g1 = (7 - w * (4 + 3 * gs)) / 4;
  const g2 = -(1 - w * (4 - 3 * gs)) / 4;
  return 1 / Math.sqrt(g1 * g1 - g2 * g2);
}

describe("deltaEddington on plan R11's twelve cases", () => {
  it.each(SLAB_CASES)("meets the δ-Eddington column to 1e-4 at τ $tau, g $g, μ₀ $mu0", (c) => {
    expect(Math.abs(conservativeAt(c).reflectance - c.deltaEddington)).toBeLessThanOrEqual(1e-4);
  });

  it.each(SLAB_CASES.filter((c) => c.tau >= 10))(
    "is within 3% of the Monte Carlo column at τ $tau, g $g, μ₀ $mu0",
    (c) => {
      const r = conservativeAt(c).reflectance;
      expect(Math.abs(r - c.monteCarlo)).toBeLessThanOrEqual(0.03 * c.monteCarlo);
    },
  );

  it.each(SLAB_CASES.filter((c) => c.tau === 1))(
    "is within 10% of the Monte Carlo column at τ $tau, g $g, μ₀ $mu0",
    (c) => {
      const r = conservativeAt(c).reflectance;
      expect(Math.abs(r - c.monteCarlo)).toBeLessThanOrEqual(0.1 * c.monteCarlo);
    },
  );
});

describe("deltaEddington against plain Eddington", () => {
  it("has a plain Eddington control that reflects less than nothing at τ 1, g 0.85, μ₀ 1", () => {
    expect(plainEddingtonReflectance(1, 0.85, 1)).toBeLessThan(0);
  });

  it("reflects a positive fraction at τ 1, g 0.85, μ₀ 1", () => {
    const r = deltaEddington({ tau: 1, asymmetry: 0.85, singleScatterAlbedo: 1, mu0: 1 });
    expect(r.reflectance).toBeGreaterThan(0);
  });
});

describe("deltaEddington's closure", () => {
  const columns = gridColumns();

  it("puts all of the beam somewhere, its four parts summing to 1 within 1e-12", () => {
    let worst = 0;
    for (const column of columns) {
      for (const r of column.results) {
        const sum = r.reflectance + r.directTransmittance + r.diffuseTransmittance + r.absorptance;
        worst = Math.max(worst, Math.abs(sum - 1));
      }
    }
    expect(worst).toBeLessThanOrEqual(1e-12);
  });

  it("gives no part below 0, beyond 1e-12 of round-off", () => {
    const negative: string[] = [];
    for (const column of columns) {
      for (const [i, r] of column.results.entries()) {
        for (const [name, part] of Object.entries(labelled(r))) {
          if (part < -1e-12) {
            negative.push(`${column.label}, τ ${GRID_TAUS[i]}: ${name} ${part}`);
          }
        }
      }
    }
    expect(negative).toEqual([]);
  });

  it("absorbs nothing at ϖ₀ = 1", () => {
    let worst = 0;
    for (const column of columns.filter((c) => c.singleScatterAlbedo === 1)) {
      for (const r of column.results) {
        worst = Math.max(worst, Math.abs(r.absorptance));
      }
    }
    expect(worst).toBeLessThanOrEqual(1e-12);
  });

  it("reflects no less as τ grows", () => {
    const falls: string[] = [];
    for (const column of columns) {
      for (let i = 1; i < column.results.length; i += 1) {
        const before = column.results[i - 1]?.reflectance ?? Number.NaN;
        const after = column.results[i]?.reflectance ?? Number.NaN;
        // A plateau's round-off, a few ulps of R ≤ 1, is not a fall; Math.exp may differ by an
        // ulp between platforms.
        if (!(after >= before - 1e-15)) {
          falls.push(`${column.label}, τ ${GRID_TAUS[i]}: ${before} → ${after}`);
        }
      }
    }
    expect(falls).toEqual([]);
  });
});

describe("deltaEddington's direct part", () => {
  it("is e^(−τ ÷ μ₀) with the unscaled τ, not the scaled τ′", () => {
    // τ′ = (1 − 0.85²) × 10 = 2.775: e^(−τ′) is 0.062 and e^(−τ) 4.5e-5.
    const r = deltaEddington({ tau: 10, asymmetry: 0.85, singleScatterAlbedo: 1, mu0: 1 });
    expect(Math.abs(r.directTransmittance / Math.exp(-10) - 1)).toBeLessThanOrEqual(1e-12);
  });
});

describe("deltaEddington against the two-stream equations solved by RK4 shooting", () => {
  const cases: ReadonlyArray<DeltaEddingtonInput> = [0.5, 0.9, 0.99, 0.999, 1].flatMap(
    (singleScatterAlbedo) =>
      [1, 10].flatMap((tau) =>
        [0.75, 0.85].flatMap((asymmetry) =>
          [1, 0.5].map((mu0) => ({ tau, asymmetry, singleScatterAlbedo, mu0 })),
        ),
      ),
  );

  it.each(cases)(
    "meets them to 1e-9 at ϖ₀ $singleScatterAlbedo, τ $tau, g $asymmetry, μ₀ $mu0",
    (input) => {
      expect(missesOfTheOde(input)).toEqual([]);
    },
  );

  // The rulings' example, and a forward-scattering layer that reaches kμ₀ = 1 at ϖ₀ 0.9 (k 1.11).
  it.each([
    [0.5, 0.5, 1],
    [0.5, 0.5, 10],
    [0.9, 0.95, 1],
    [0.9, 0.95, 10],
  ])("meets them to 1e-9 at kμ₀ = 1, ϖ₀ %f, g %f and τ %d", (w0, g, tau) => {
    const input = { tau, asymmetry: g, singleScatterAlbedo: w0, mu0: singularMu0(w0, g) };
    expect(missesOfTheOde(input)).toEqual([]);
  });
});

describe("deltaEddington's spot values", () => {
  // Main's check of 2026-10-20, to six decimals; transmittance is the total, T_dir + T_diff.
  it.each([
    {
      input: { singleScatterAlbedo: 0.99, tau: 10, asymmetry: 0.85, mu0: 1 },
      expected: {
        reflectance: 0.349234,
        transmittance: 0.487126,
        absorptance: 0.16364,
        diffuseTransmittance: 0.487081,
      },
    },
    {
      input: { singleScatterAlbedo: 0.99, tau: 10, asymmetry: 0.85, mu0: 0.5 },
      expected: {
        reflectance: 0.509476,
        transmittance: 0.329223,
        absorptance: 0.161301,
        diffuseTransmittance: 0.329223,
      },
    },
    {
      input: { singleScatterAlbedo: 0.999, tau: 50, asymmetry: 0.85, mu0: 1 },
      expected: {
        reflectance: 0.748839,
        transmittance: 0.148413,
        absorptance: 0.102747,
        diffuseTransmittance: 0.148413,
      },
    },
    {
      input: { singleScatterAlbedo: 0.99999, tau: 8, asymmetry: 0.85, mu0: 1 },
      expected: {
        reflectance: 0.35634,
        transmittance: 0.64352,
        absorptance: 0.00014,
        diffuseTransmittance: 0.643184,
      },
    },
  ])(
    "matches the record to 1e-6 at ϖ₀ $input.singleScatterAlbedo, τ $input.tau, μ₀ $input.mu0",
    ({ input, expected }) => {
      const r = deltaEddington(input);
      const actual = {
        reflectance: r.reflectance,
        transmittance: totalTransmittance(r),
        absorptance: r.absorptance,
        diffuseTransmittance: r.diffuseTransmittance,
      };
      expect(misses(actual, expected, 1e-6)).toEqual([]);
    },
  );
});

describe("deltaEddington's limits", () => {
  it.each([0, 0.5, 0.999, 1])("lets the whole beam through at τ = 0 and ϖ₀ %f", (w0) => {
    const r = deltaEddington({ tau: 0, asymmetry: 0.85, singleScatterAlbedo: w0, mu0: 0.5 });
    const whole = {
      reflectance: 0,
      directTransmittance: 1,
      diffuseTransmittance: 0,
      absorptance: 0,
    };
    expect(misses(labelled(r), whole, 1e-12)).toEqual([]);
  });

  it.each([0.999, 1])("stays finite and in [0, 1] at τ = 10⁴ and ϖ₀ %f", (w0) => {
    const r = deltaEddington({ tau: 1e4, asymmetry: 0.85, singleScatterAlbedo: w0, mu0: 0.5 });
    const outside = Object.entries(labelled(r)).filter(
      ([, p]) => !(Number.isFinite(p) && p >= 0 && p <= 1),
    );
    expect(outside).toEqual([]);
  });

  it.each([1, 10])("is finite at kμ₀ = 1 and τ %d", (tau) => {
    const r = deltaEddington({ tau, asymmetry: 0.5, singleScatterAlbedo: 0.5, mu0: singularMu0() });
    const infinite = Object.entries(labelled(r)).filter(([, p]) => !Number.isFinite(p));
    expect(infinite).toEqual([]);
  });

  it.each([1, 10])("lies within 1e-6 of its neighbours' mean at kμ₀ = 1 and τ %d", (tau) => {
    const mu0 = singularMu0();
    const at = (m: number): SlabFluxFractions =>
      deltaEddington({ tau, asymmetry: 0.5, singleScatterAlbedo: 0.5, mu0: m });
    const below = at(mu0 - 1e-6);
    const above = at(mu0 + 1e-6);
    const mean = {
      reflectance: (below.reflectance + above.reflectance) / 2,
      directTransmittance: (below.directTransmittance + above.directTransmittance) / 2,
      diffuseTransmittance: (below.diffuseTransmittance + above.diffuseTransmittance) / 2,
      absorptance: (below.absorptance + above.absorptance) / 2,
    };
    expect(misses(labelled(at(mu0)), mean, 1e-6)).toEqual([]);
  });

  it.each([
    [1, -1],
    [1, 1],
    [10, -1],
    [10, 1],
  ])(
    "is continuous within 1e-9 across an edge of the kμ₀ = 1 window, at τ %d (side %d)",
    (tau, side) => {
      // The window is |1 − kμ₀| < 1e-5 (cloudOptics.ts); the two points straddle its edge by 1e-11.
      const k = 1 / singularMu0();
      const at = (kMu0: number): SlabFluxFractions =>
        deltaEddington({ tau, asymmetry: 0.5, singleScatterAlbedo: 0.5, mu0: kMu0 / k });
      const inside = at(1 + side * 1e-5 * (1 - 1e-6));
      const outside = at(1 + side * 1e-5 * (1 + 1e-6));
      expect(misses(labelled(inside), labelled(outside), 1e-9)).toEqual([]);
    },
  );

  it.each([1, 10])(
    "neither reflects nor scatters through at ϖ₀ = 0 and kμ₀ = 1, at τ %d",
    (tau) => {
      // k = √3 at ϖ₀ = 0, so μ₀ = 1 ÷ √3 lies inside the interpolation window.
      const mu0 = singularMu0(0, 0.5);
      const r = deltaEddington({ tau, asymmetry: 0.5, singleScatterAlbedo: 0, mu0 });
      const unscattered = {
        reflectance: 0,
        directTransmittance: Math.exp(-tau / mu0),
        diffuseTransmittance: 0,
        absorptance: 1 - Math.exp(-tau / mu0),
      };
      expect(misses(labelled(r), unscattered, 1e-15)).toEqual([]);
    },
  );

  it("is continuous in R across the conservative switch, within 1e-9 at τ ≤ 50", () => {
    const general = 1 - 1.001 * CONSERVATIVE_CO_ALBEDO;
    const conservative = 1 - 0.999 * CONSERVATIVE_CO_ALBEDO;
    if (!(1 - general > CONSERVATIVE_CO_ALBEDO && 1 - conservative <= CONSERVATIVE_CO_ALBEDO)) {
      throw new Error("the two albedos do not straddle the conservative switch");
    }
    let worst = 0;
    for (const tau of [0.01, 0.1, 1, 10, 50]) {
      for (const asymmetry of [0, 0.75, 0.85]) {
        for (const mu0 of [0.05, 0.5, 1]) {
          const a = deltaEddington({ tau, asymmetry, singleScatterAlbedo: general, mu0 });
          const b = deltaEddington({ tau, asymmetry, singleScatterAlbedo: conservative, mu0 });
          worst = Math.max(worst, Math.abs(a.reflectance - b.reflectance));
        }
      }
    }
    expect(worst).toBeLessThanOrEqual(1e-9);
  });
});

describe("deltaEddington's domain", () => {
  const valid: DeltaEddingtonInput = { tau: 1, asymmetry: 0.85, singleScatterAlbedo: 1, mu0: 1 };
  it.each<[string, Partial<DeltaEddingtonInput>]>([
    ["a negative g", { asymmetry: -0.1 }],
    ["g = 1", { asymmetry: 1 }],
    ["g above 1", { asymmetry: 1.5 }],
    ["a negative ϖ₀", { singleScatterAlbedo: -0.01 }],
    ["ϖ₀ above 1", { singleScatterAlbedo: 1.01 }],
    ["μ₀ = 0", { mu0: 0 }],
    ["a negative μ₀", { mu0: -0.5 }],
    ["μ₀ above 1", { mu0: 1.01 }],
    ["a negative τ", { tau: -1 }],
    ["an infinite τ", { tau: Number.POSITIVE_INFINITY }],
    ["a NaN τ", { tau: Number.NaN }],
    ["a NaN g", { asymmetry: Number.NaN }],
    ["a NaN ϖ₀", { singleScatterAlbedo: Number.NaN }],
    ["a NaN μ₀", { mu0: Number.NaN }],
  ])("refuses %s with a RangeError", (_, change) => {
    expect(() => deltaEddington({ ...valid, ...change })).toThrow(RangeError);
  });
});
