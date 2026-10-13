import { beforeAll, describe, expect, it } from "vitest";

import { gaussLegendre } from "../lighting/quadrature";
import type { Rgb } from "../photometry/toneCurve";
import { aerosolModeOptics, aerosolPhaseTable } from "./aerosol";
import {
  besselJ0,
  type ForwardLobe,
  FORWARD_LOBE_ENTRIES,
  FORWARD_LOBE_RAD,
  FORWARD_ORDERS_ENTRIES,
  FORWARD_WIDE_ENTRIES,
  forwardLobeAt,
  forwardLobeOf,
  forwardLobeOfTable,
  slabOrders,
} from "./forwardLobe";
import { type AtmosphereMedium, CHANNEL_WAVELENGTHS_NM, type PhaseTable } from "./medium";
import { maxDistanceM } from "./opticalDepth";
import type { ModeOptics } from "./sizeDistribution";
import { kernelMedium, withoutForwardDelta } from "./tables";
import {
  marchTwin,
  placedSteps,
  planetTwin,
  tableSources,
  type TwinRay,
  type TwinSources,
} from "./tablesCpu";

/**
 * Mars dust as R08.T5.c draws it: gamma r_eff 0.8974 µm, v_eff 0.3, on the hexahedra
 * (`science-r08-mmf.md` §3).
 */
const MARS_DUST = {
  kind: "distribution",
  mode: {
    material: "mars_dust",
    sizes: { kind: "gamma", effectiveRadiusUm: 0.8974, effectiveVariance: 0.3 },
  },
} as const;

/** The Mars mode's optics at the kernels' three channel wavelengths. */
function marsOptics(): [ModeOptics, ModeOptics, ModeOptics] {
  const [r, g, b] = CHANNEL_WAVELENGTHS_NM;
  return [
    aerosolModeOptics(MARS_DUST, r),
    aerosolModeOptics(MARS_DUST, g),
    aerosolModeOptics(MARS_DUST, b),
  ];
}

/** The Mars mode's table as the kernels read it: its forward delta, if any, taken out. */
function marsTable(): PhaseTable {
  const optics = marsOptics();
  const term = withoutForwardDelta({
    name: "Mars dust",
    density: { kind: "exponential", scaleHeightM: 11_000 },
    scattering: [1, 1, 1],
    absorption: [0, 0, 0],
    phase: { kind: "tabulated", table: aerosolPhaseTable(optics) },
  });
  if (term.phase.kind !== "tabulated") {
    throw new Error("the dust stays tabulated");
  }
  return term.phase.table;
}

/** The radial grid of the direct convolution: 0.1° steps to 90° on the plane of small angles. */
const STEP_RAD = (0.1 * Math.PI) / 180;
const RADII = 901;

/** One channel's forward part on the radial grid, linear in u as the table reads it, 0 past 30°. */
function forwardPart(table: PhaseTable, c: 0 | 1 | 2): Float64Array {
  const values = table.values[c];
  const lastU = table.u[FORWARD_LOBE_ENTRIES - 1] ?? Number.NaN;
  return Float64Array.from({ length: RADII }, (_, i) => {
    const theta = i * STEP_RAD;
    const u = Math.sqrt(theta / Math.PI);
    if (u > lastU) {
      return 0;
    }
    const place = u * 255;
    const k = Math.min(Math.floor(place), 254);
    const a = values[k] ?? Number.NaN;
    return a + ((values[k + 1] ?? Number.NaN) - a) * (place - k);
  });
}

/** One channel's whole phase on the radial grid, linear in u as the table reads it, to 89.7°. */
function widePart(table: PhaseTable, c: 0 | 1 | 2): Float64Array {
  const values = table.values[c];
  const lastU = table.u[FORWARD_WIDE_ENTRIES - 1] ?? Number.NaN;
  return Float64Array.from({ length: RADII }, (_, i) => {
    const theta = i * STEP_RAD;
    const u = Math.sqrt(theta / Math.PI);
    if (u > lastU) {
      return 0;
    }
    const place = u * 255;
    const k = Math.min(Math.floor(place), 254);
    const a = values[k] ?? Number.NaN;
    return a + ((values[k + 1] ?? Number.NaN) - a) * (place - k);
  });
}

/** A function on the radial grid at radius r, linear between nodes, 0 past the grid. */
function at(f: Float64Array, r: number): number {
  const x = r / STEP_RAD;
  const i = Math.floor(x);
  if (i >= RADII - 1) {
    return 0;
  }
  const a = f[i] ?? 0;
  return a + ((f[i + 1] ?? 0) - a) * (x - i);
}

/**
 * The 2D convolution of two radial functions on the plane, f's support within `supportRad` (30°
 * unless given), at every radius of the grid, or at `radii` alone.
 */
function convolved(
  f: Float64Array,
  g: Float64Array,
  supportRad = FORWARD_LOBE_RAD,
  radii: ReadonlyArray<number> | null = null,
): Float64Array {
  const azimuths = gaussLegendre(48);
  const support = Math.min(Math.round(supportRad / STEP_RAD), RADII - 1);
  const which = radii ?? Array.from({ length: RADII }, (_, i) => i * STEP_RAD);
  return Float64Array.from(which, (r) => {
    let sum = 0;
    for (let j = 0; j <= support; j += 1) {
      const rho = j * STEP_RAD;
      const weight = (j === 0 || j === support ? 0.5 : 1) * STEP_RAD * rho * (f[j] ?? 0);
      if (weight === 0) {
        continue;
      }
      // ∫₀^2π g(|r − ρ|) dφ, symmetric about φ = 0: 2 ∫₀^π, by Gauss–Legendre on [0, π].
      let ring = 0;
      for (let a = 0; a < azimuths.x.length; a += 1) {
        const phi = 0.5 * Math.PI * (1 + (azimuths.x[a] ?? 0));
        const d = Math.sqrt(Math.max(r * r + rho * rho - 2 * r * rho * Math.cos(phi), 0));
        ring += 0.5 * Math.PI * (azimuths.w[a] ?? 0) * at(g, d);
      }
      sum += weight * 2 * ring;
    }
    return sum;
  });
}

/** Σ_{n≥2} xⁿ ÷ n! p^{*n} by direct convolution on the radial grid, to the 25th order. */
function directOrders(p: Float64Array, xs: ReadonlyArray<number>): Float64Array[] {
  const out = xs.map(() => new Float64Array(RADII));
  let power = p;
  for (let n = 2; n <= 25; n += 1) {
    power = convolved(p, power);
    let factorial = 1;
    for (let m = 2; m <= n; m += 1) {
      factorial *= m;
    }
    for (const [i, x] of xs.entries()) {
      const into = out[i];
      const scale = x ** n / factorial;
      for (let k = 0; k < RADII; k += 1) {
        if (into !== undefined) {
          into[k] = (into[k] ?? 0) + scale * (power[k] ?? 0);
        }
      }
    }
  }
  return out;
}

/** The table's forward entry nearest to an angle, and that angle. */
function entryNear(table: PhaseTable, thetaDeg: number): number {
  const u = Math.sqrt((thetaDeg * Math.PI) / 180 / Math.PI);
  let best = 0;
  for (let k = 0; k < FORWARD_LOBE_ENTRIES; k += 1) {
    if (Math.abs((table.u[k] ?? 0) - u) < Math.abs((table.u[best] ?? 0) - u)) {
      best = k;
    }
  }
  return best;
}

/** The slab's slant depths of `science-r08-mmf.md` §3, and the dust's single-scattering albedo. */
const SLANT_DEPTHS = [1, 2.5, 4.5] as const;
const ALBEDO = 0.97;

describe("the lobes kept (R08.T7)", () => {
  it("makes one lobe for equal tables that are other objects, as a twin worker's clone is", () => {
    const table = marsTable();
    const clone = structuredClone(table);
    expect(clone).not.toBe(table);
    expect(forwardLobeOfTable(clone)).toBe(forwardLobeOfTable(table));
  }, 60_000);
});

describe("J₀ (Numerical Recipes' bessj0)", () => {
  it("is its integral (1 ÷ π) ∫₀^π cos(x sin t) dt to 10⁻⁷, from 0 to 200", () => {
    const rule = gaussLegendre(160);
    let worst = 0;
    for (let x = 0; x <= 200; x += 0.37) {
      let sum = 0;
      for (let i = 0; i < rule.x.length; i += 1) {
        const t = 0.5 * Math.PI * (1 + (rule.x[i] ?? 0));
        sum += 0.5 * Math.PI * (rule.w[i] ?? 0) * Math.cos(x * Math.sin(t));
      }
      worst = Math.max(worst, Math.abs(besselJ0(x) - sum / Math.PI));
    }
    expect(worst).toBeLessThan(1e-7);
  });
});

describe("the forward lobe's higher orders on a homogeneous slab of the Mars mode (R08.T7)", () => {
  const table = marsTable();
  const xs = SLANT_DEPTHS.map((tau) => ALBEDO * tau);
  // The entries nearest 3°, 5°, 8°, 12° and 20° from the source.
  const entries = [3, 5, 8, 12, 20].map((deg) => entryNear(table, deg));
  // Red's and blue's direct orders, and the kernels' lobe, formed once for the tests that read them.
  let direct: Record<0 | 2, Float64Array[]> = { 0: [], 2: [] };
  let lobe: ForwardLobe = forwardLobeOf(table);

  beforeAll(() => {
    direct = {
      0: directOrders(forwardPart(table, 0), xs),
      2: directOrders(forwardPart(table, 2), xs),
    };
    lobe = forwardLobeOf(table);
  }, 300_000);

  it("match a direct numerical n-fold convolution of the table's forward part to 1% at 3–20°", () => {
    for (const c of [0, 2] as const) {
      for (const [i, x] of xs.entries()) {
        const orders = slabOrders(table, x)[c];
        for (const k of entries) {
          const theta = Math.PI * (table.u[k] ?? Number.NaN) ** 2;
          const reference = at(direct[c][i] ?? new Float64Array(0), theta);
          expect(Math.abs((orders[k] ?? Number.NaN) / reference - 1)).toBeLessThan(0.01);
        }
      }
    }
  });

  it("scatter the forward-diffuse light once more by the whole phase, to 1% at 3–55° against a direct convolution", () => {
    const degrees = [3, 5, 8, 12, 20, 30, 40, 55];
    const radii = degrees.map((deg) => (deg * Math.PI) / 180);
    for (const c of [0, 2] as const) {
      const forward = forwardPart(table, c);
      const whole = widePart(table, c);
      for (const [i, x] of xs.entries()) {
        // D = x p + Σ_{n≥2} xⁿ ÷ n! p^{*n}, the forward-diffuse light, then D * p_w.
        const orders = direct[c][i] ?? new Float64Array(0);
        const diffuse = Float64Array.from(orders, (v, r) => v + x * (forward[r] ?? 0));
        const reference = convolved(whole, diffuse, Math.PI / 2, radii);
        for (const [r, deg] of degrees.entries()) {
          const entry = Math.sqrt(deg / 180) * 255;
          const kernels = Math.exp(x * lobe.share[c]) * forwardLobeAt(lobe, c, x, entry, 0);
          expect(Math.abs(kernels / (reference[r] ?? Number.NaN) - 1)).toBeLessThan(0.01);
        }
      }
    }
  });

  // Measured 2026-10-13: 0.998 to 1.024 of it, the light the last scattering's wide part (the phase
  // past 30°) adds to the forward part's own series there, beside the x nodes' interpolation.
  it("give the slab's forward orders through the kernels' table within the lobe, and the wide part's light, within −1% and +3% at 3–12°", () => {
    for (const c of [0, 2] as const) {
      const p = lobe.share[c];
      for (const x of xs) {
        const exact = slabOrders(table, x)[c];
        // ∫₀ˣ E dx′, E = Ẽ e^(x′P), by Gauss–Legendre.
        const rule = gaussLegendre(24);
        for (const k of entries.slice(0, 4)) {
          let sum = 0;
          for (let g = 0; g < rule.x.length; g += 1) {
            const xx = 0.5 * x * (1 + (rule.x[g] ?? 0));
            sum += 0.5 * x * (rule.w[g] ?? 0) * forwardLobeAt(lobe, c, xx, k, 0) * Math.exp(xx * p);
          }
          const ratio = sum / (exact[k] ?? Number.NaN);
          expect(ratio).toBeGreaterThan(0.99);
          expect(ratio).toBeLessThan(1.03);
        }
      }
    }
  });

  it("leaves the slab's forward light unchanged to 1%: the orders move the lobe's light, adding none", () => {
    for (const c of [0, 2] as const) {
      const p = lobe.share[c];
      expect(p).toBeGreaterThan(0.3);
      expect(p).toBeLessThan(1);
      for (const x of xs) {
        const orders = slabOrders(table, x)[c];
        // 2π ∫ G θ dθ over the orders' entries, trapezoid in θ on the plane.
        let flux = 0;
        for (let k = 0; k + 1 < FORWARD_ORDERS_ENTRIES; k += 1) {
          const t0 = Math.PI * (table.u[k] ?? 0) ** 2;
          const t1 = Math.PI * (table.u[k + 1] ?? 0) ** 2;
          flux += Math.PI * ((orders[k] ?? 0) * t0 + (orders[k + 1] ?? 0) * t1) * (t1 - t0);
        }
        // Direct, single and higher orders together: e^(−τ)(1 + xP + flux) against e^(−τ + xP).
        expect(Math.abs((1 + x * p + flux) / Math.exp(x * p) - 1)).toBeLessThan(0.01);
      }
    }
  });

  it("tend far from the lobe to the forward-diffuse light scattered by the phase itself", () => {
    for (const c of [0, 2] as const) {
      const p = lobe.share[c];
      for (const x of xs) {
        for (const deg of [90, 120, 150]) {
          const entry = Math.sqrt(deg / 180) * 255;
          const k = Math.floor(entry);
          const f = entry - k;
          const phase =
            (table.values[c][k] ?? Number.NaN) * (1 - f) +
            (table.values[c][k + 1] ?? Number.NaN) * f;
          const orders = Math.exp(x * p) * forwardLobeAt(lobe, c, x, entry, phase);
          const ratio = orders / (Math.expm1(x * p) * phase);
          expect(ratio, `channel ${c}, x ${x}, ${deg}°`).toBeGreaterThan(0.9);
          expect(ratio, `channel ${c}, x ${x}, ${deg}°`).toBeLessThan(1.2);
        }
      }
    }
  });

  // Measured 2026-10-13: up to 2.6% over at x 4.4, the plane's geometry standing in for the
  // sphere's far from the source.
  it("scatter as δ-M's beam does: the phase and the orders integrate to e^(xP) over the sphere, to 3%", () => {
    for (const c of [0, 2] as const) {
      const p = lobe.share[c];
      const values = table.values[c];
      for (const x of xs) {
        // ∫ (p + e^(xP) Ẽ) dΩ, trapezoid in θ over the table's entries on the sphere.
        const integrand = (k: number): number => {
          const phase = values[k] ?? Number.NaN;
          return phase + Math.exp(x * p) * forwardLobeAt(lobe, c, x, k, phase);
        };
        let total = 0;
        for (let k = 0; k + 1 < 256; k += 1) {
          const t0 = Math.PI * (table.u[k] ?? 0) ** 2;
          const t1 = Math.PI * (table.u[k + 1] ?? 0) ** 2;
          total +=
            Math.PI * (integrand(k) * Math.sin(t0) + integrand(k + 1) * Math.sin(t1)) * (t1 - t0);
        }
        expect(Math.abs(total / Math.exp(x * p) - 1)).toBeLessThan(0.03);
      }
    }
  });
});

describe("the forward lobe's orders in the twin's Mars sky (R08.T7, science-r08-mmf.md §3)", () => {
  // Mars's mean radius (Seidelmann et al. 2007) and a 100 km top over an 11 km scale height.
  const radiusM = 3_389_500;
  const topM = 100_000;

  /**
   * A dusty Mars as a test's medium: the Mars mode at τ(550) 0.45, so slant τ ≈ 4.0 at a 5° Sun
   * (an airmass of 8.96 on this shell), its extinction per channel by its Q_ext and its
   * single-scattering albedo, under a CO₂ Rayleigh term of about 0.003 in green (a test's
   * coefficients, of CO₂'s order, its depolarisation a test's 0.08, about Sneep and Ubachs 2005's
   * 0.075), both on an 11 km scale height; a test's ground albedo.
   */
  function dustyMars(): AtmosphereMedium {
    const optics = marsOptics();
    const scaleHeightM = 11_000;
    const green = optics[1].qExt;
    const extinction = optics.map((o) => (0.45 / scaleHeightM) * (o.qExt / green));
    const part = (c: 0 | 1 | 2, of: (e: number, w: number) => number): number =>
      of(extinction[c] ?? Number.NaN, optics[c].singleScatteringAlbedo);
    const rgb = (of: (e: number, w: number) => number): Rgb => [
      part(0, of),
      part(1, of),
      part(2, of),
    ];
    return kernelMedium({
      name: "dusty Mars",
      topHeightM: topM,
      groundAlbedo: [0.3, 0.2, 0.12],
      terms: [
        {
          name: "CO₂",
          density: { kind: "exponential", scaleHeightM },
          scattering: [1.6e-7, 3.1e-7, 7.5e-7],
          absorption: [0, 0, 0],
          phase: { kind: "rayleigh", depolarisation: [0.08, 0.08, 0.08] },
        },
        {
          name: "dust",
          density: { kind: "exponential", scaleHeightM },
          scattering: rgb((e, w) => e * w),
          absorption: rgb((e, w) => e * (1 - w)),
          phase: { kind: "tabulated", table: aerosolPhaseTable(optics) },
        },
      ],
    });
  }

  /** The sky 3°, 5°, 8° and 12° above a 5° Sun, with the orders and without them. */
  let skies: ReadonlyArray<{ readonly deg: number; readonly sky: Rgb; readonly alone: Rgb }> = [];

  beforeAll(() => {
    const medium = dustyMars();
    const full = tableSources(planetTwin(medium, radiusM));
    const withoutOrders: TwinSources = {
      bottomRadiusM: full.bottomRadiusM,
      sunTransmittance: full.sunTransmittance,
      multiScattering: full.multiScattering,
      absorberPathDepth: full.absorberPathDepth,
    };
    const sunRad = (5 * Math.PI) / 180;
    const sun = { x: Math.cos(sunRad), y: 0, z: Math.sin(sunRad) };
    const shell = { bottomRadiusM: radiusM, topRadiusM: radiusM + topM };
    const skyAbove = (deg: number, sources: TwinSources): Rgb => {
      const a = sunRad + (deg * Math.PI) / 180;
      const ray: TwinRay = {
        originM: { x: 0, y: 0, z: radiusM + 2 },
        direction: { x: Math.cos(a), y: 0, z: Math.sin(a) },
        sun,
        tStartM: 0,
        tEndM: maxDistanceM(shell, radiusM + 2, Math.sin(a)),
        fromCamera: true,
      };
      const geometry = { kind: "sphere", bottomRadiusM: radiusM } as const;
      return marchTwin(medium, sources, geometry, ray, placedSteps(ray, 400)).luminance;
    };
    skies = [3, 5, 8, 12].map((deg) => ({
      deg,
      sky: skyAbove(deg, full),
      alone: skyAbove(deg, withoutOrders),
    }));
  }, 300_000);

  // Measured 2026-10-13: the orders' share of red runs 0.36, 0.43, 0.53 and 0.60 at 3°, 5°, 8° and
  // 12° above the Sun, of blue 0.39 to 0.66; the slab estimate of science-r08-mmf.md §3, at slant
  // τ 4.5, was 0.47–0.81.
  it("carry a third to three fifths of the radiance 3–12° from a 5° Sun, more with the angle", () => {
    let last = 0;
    for (const { sky, alone } of skies) {
      const shares = [0, 1, 2].map((c) => 1 - (alone[c] ?? Number.NaN) / (sky[c] ?? Number.NaN));
      for (const share of shares) {
        expect(share).toBeGreaterThan(0.3);
        expect(share).toBeLessThan(0.7);
      }
      expect(shares[0]).toBeGreaterThan(last);
      last = shares[0] ?? Number.NaN;
    }
  });

  // Measured 2026-10-13: blue over red 8° from the Sun rises from 0.80 to 0.95.
  it("widen the blue aureole: blue over red 8° from a 5° Sun rises by over 15%", () => {
    const at8 = skies.find(({ deg }) => deg === 8);
    if (at8 === undefined) {
      throw new Error("the sky 8° above the Sun is formed");
    }
    const { sky, alone } = at8;
    expect(sky[2] / sky[0] / (alone[2] / alone[0])).toBeGreaterThan(1.15);
  });
});
