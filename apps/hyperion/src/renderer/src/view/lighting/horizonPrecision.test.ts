/**
 * R07.T6.d's precision test (plan R07, Design note 6; decision-r10-t8b §2; R08 Design note 18):
 * `shaders/litBody.wgsl`'s horizon term emulated in `f32`, operation for operation in the WGSL's
 * order, against its `f64` twin `sphereIrradianceFactor` on the same `f32` inputs, as R08's
 * `atmosphere/precision.test.ts` holds the atmosphere's helpers.
 *
 * @remarks
 * The new form calls no built-in trigonometry, so Mesa's lowerings do not reach it. It runs on
 * correctly rounded arithmetic and at five corners of WGSL's bounds for +, −, ×, ÷ and `sqrt`
 * (`test/preciseTrigF32.ts`' {@link wgslArithmetics}): each sum, difference and product rounded to
 * one neighbour, each quotient 2 ULP off, and `sqrt` as 1 ÷ `inverseSqrt` with each step 2 ULP off.
 *
 * The form as built by T6.c, which took WGSL's `atan`, `acos`, `asin`, `cos`, `sin` and `tan`, runs
 * on IEEE-rounded arithmetic with its built-ins taken three ways, as decision-r10-t8b §2 measured it:
 * - correctly rounded;
 * - Mesa's NIR lowerings, which ANV (the UHD 620's driver on Linux), RADV and lavapipe run for
 *   GLSL.std.450's `Atan`, `Acos` and `Asin`, the SPIR-V that Tint emits for WGSL's:
 *   `nir_atan`'s polynomial, leading coefficient 0.9999793128310355, and `build_asin`'s
 *   Abramowitz and Stegun 4.4.45, piecewise with x + x³ ÷ 6 below 0.21502245 for `asin` only
 *   (Mesa, `src/compiler/nir/nir_builtin_builder.c`, read 2026-10-16); `sin`, `cos` and `tan`
 *   correctly rounded, which flatters the old form;
 * - one corner of WGSL's bounds (W3C WGSL, CRD 2026-09-21, §15.7.4.1): `atan` 4,096 ULP low,
 *   `acos` and `asin` high by the absolute terms of their bounds, 6.77 and 6.81 × 10⁻⁵ (each bound is
 *   the worse of that and atan2's 4,096 ULP, so this understates the old form's worst), and `sin`
 *   and `cos` 2⁻¹¹ low.
 *
 * The old form's defects are asserted, so that the test is known to see what it guards against.
 */

import { describe, expect, it } from "vitest";

import { FORBIDDEN_IN_ATMOSPHERE_WGSL, wgslCode } from "../../test/atmosphereWgsl";
import {
  type Arithmetic,
  nudge,
  preciseAtan2,
  preciseSinCos,
  ROUNDED,
  TRIG_PI,
  wgslArithmetics,
} from "../../test/preciseTrigF32";
import bodyDiscWgsl from "../shaders/bodyDisc.wgsl?raw";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import { sphereIrradianceFactor } from "./sphereIrradiance";

const f = Math.fround;

const RAD_PER_DEG = Math.PI / 180;

/** H at 1 au: the au (IAU 2012 B2) over the Sun's nominal radius (IAU 2015 B3). */
const H_AT_1_AU = 1.495_978_707e11 / 6.957e8;

/** Correctly rounded arithmetic and the corners of WGSL's bounds, by name. */
const ARITHMETICS = wgslArithmetics();

// --- view/shaders/litBody.wgsl, R07.T6.d ------------------------------------------------------

/** `atan_tail`'s series in t², from t²'s coefficient, after its −1/3. */
const TAIL_SERIES: ReadonlyArray<number> = [1 / 5, -1 / 7, 1 / 9, -1 / 11].map(f);
const MINUS_THIRD = f(-1 / 3);
const LIT_PI = TRIG_PI;

/** `atan_tail`: (atan z − z) ÷ z³. */
function atanTail(ops: Arithmetic, z: number): number {
  const p0 = ops.add(1, ops.sqrt(ops.add(1, ops.mul(z, z))));
  const t1 = ops.div(z, p0);
  const p1 = ops.add(1, ops.sqrt(ops.add(1, ops.mul(t1, t1))));
  const t2 = ops.div(t1, p1);
  const p2 = ops.add(1, ops.sqrt(ops.add(1, ops.mul(t2, t2))));
  const t3 = ops.div(t2, p2);
  const t = ops.mul(t3, t3);
  // −1/3 + t (1/5 + t (−1/7 + t (1/9 + t (−1/11)))), innermost first.
  let acc = ops.mul(t, TAIL_SERIES.at(-1) ?? Number.NaN);
  for (let k = TAIL_SERIES.length - 2; k >= 0; k -= 1) {
    acc = ops.mul(t, ops.add(TAIL_SERIES[k] ?? Number.NaN, acc));
  }
  let b = ops.add(MINUS_THIRD, acc);
  for (const p of [p2, p1, p0]) {
    b = ops.div(ops.sub(ops.mul(2, b), p), ops.mul(ops.mul(p, p), p));
  }
  return b;
}

/** `howell_irradiance`: H² F, unclamped. */
function howellIrradiance(ops: Arithmetic, h: number, phiIn: number): number {
  const phi = Math.min(Math.abs(phiIn), LIT_PI);
  const q = ops.div(1, h);
  const cosPhi = preciseSinCos(phi, ops)[1];
  if (cosPhi >= q) {
    return cosPhi;
  }
  if (cosPhi <= -q) {
    return 0;
  }
  const c = Math.min(Math.max(ops.mul(cosPhi, h), -1), 1);
  const cosRho = ops.sqrt(ops.mul(ops.sub(1, q), ops.add(1, q)));
  const chord = ops.sqrt(ops.mul(ops.sub(1, c), ops.add(1, c)));
  const u = ops.div(chord, cosRho);
  const arc = preciseAtan2(chord, ops.mul(-c, cosRho), ops);
  const inner = ops.add(1, ops.mul(ops.mul(u, u), atanTail(ops, ops.mul(q, u))));
  return ops.mul(ops.div(q, LIT_PI), ops.add(ops.mul(c, arc), ops.mul(u, inner)));
}

/** `disc_visible_fraction`. */
function discVisibleFraction(ops: Arithmetic, h: number, phi: number): number {
  const q = ops.div(1, h);
  const rho = preciseAtan2(q, ops.sqrt(ops.mul(ops.sub(1, q), ops.add(1, q))), ops);
  const offset = ops.sub(ops.mul(0.5, LIT_PI), phi);
  if (Math.abs(offset) >= rho) {
    return offset > 0 ? 1 : offset < 0 ? 0 : 0.5;
  }
  const x = Math.min(Math.max(ops.div(offset, rho), -1), 1);
  const root = ops.sqrt(ops.mul(ops.sub(1, x), ops.add(1, x)));
  return ops.div(ops.add(preciseAtan2(root, -x, ops), ops.mul(x, root)), LIT_PI);
}

/** `sphere_irradiance`, as R07.T6.d builds it. */
function newTerm(ops: Arithmetic, h: number, phi: number, horizon: number): number {
  const eta = Math.max(horizon, 0);
  if (eta === 0) {
    return Math.max(0, howellIrradiance(ops, h, phi));
  }
  const tilted = Math.min(Math.max(ops.add(phi, eta), 0), LIT_PI);
  const [sinEta, cosEta] = preciseSinCos(Math.min(eta, LIT_PI), ops);
  const direct = ops.mul(cosEta, howellIrradiance(ops, h, tilted));
  const tangential = ops.mul(
    ops.mul(sinEta, preciseSinCos(tilted, ops)[0]),
    discVisibleFraction(ops, h, tilted),
  );
  return Math.max(0, ops.add(direct, tangential));
}

// --- litBody.wgsl as T6.c built it, its built-ins three ways ---------------------------------

const HALF_PI32 = f(Math.PI / 2);

/** WGSL's built-ins that the old form called, each as one implementation runs it. */
interface Builtins {
  readonly atan: (x: number) => number;
  readonly acos: (x: number) => number;
  readonly asin: (x: number) => number;
  readonly cos: (x: number) => number;
  readonly sin: (x: number) => number;
}

const CORRECTLY_ROUNDED: Builtins = {
  atan: (x) => f(Math.atan(x)),
  acos: (x) => f(Math.acos(x)),
  asin: (x) => f(Math.asin(x)),
  cos: (x) => f(Math.cos(x)),
  sin: (x) => f(Math.sin(x)),
};

/** `nir_atan`'s coefficients, highest power first. */
const NIR_ATAN: ReadonlyArray<number> = [
  -0.0121323213173444, 0.0536813784310406, -0.1173503194786851, 0.193892497711561,
  -0.3326756418091246, 0.9999793128310355,
].map(f);

/** `nir_atan`: range-reduced at 1 by a reciprocal, then a polynomial in u², times |u|. */
function nirAtan(z: number): number {
  const small = Math.abs(z) <= 1;
  const u = small ? z : f(1 / z);
  const u2 = f(u * u);
  let res = NIR_ATAN[0] ?? Number.NaN;
  for (const c of NIR_ATAN.slice(1)) {
    res = f(f(res * u2) + c);
  }
  const bias = small ? 0 : f(-Math.PI / 2);
  const tmp = f(f(Math.abs(u) * res) + bias);
  return Math.sign(z) * Math.abs(tmp);
}

/**
 * `build_asin` (A&S 4.4.45), `piecewise` taking x + x³ ÷ 6 below 0.21502245, as `nir_asin` does.
 * Its sign is `fsign(x)`, 0 at 0, so `nir_acos` is π/2 at 0 itself and 6.75 × 10⁻⁵ off on either
 * side of it.
 */
function nirBuildAsin(x: number, piecewise: boolean): number {
  const a = Math.abs(x);
  const p = f(f(a * f(-0.0187293)) + f(0.074261));
  const tail = f(f(a * f(f(a * p) + f(-0.2121144))) + f(1.5707288));
  const result = Math.sign(x) * f(HALF_PI32 - f(f(Math.sqrt(f(1 - a))) * tail));
  if (piecewise && a < 0.21502245) {
    return f(x * f(f(f(x * x) * f(1 / 6)) + 1));
  }
  return result;
}

const MESA: Builtins = {
  atan: nirAtan,
  acos: (x) => f(HALF_PI32 - nirBuildAsin(x, false)),
  asin: (x) => nirBuildAsin(x, true),
  cos: CORRECTLY_ROUNDED.cos,
  sin: CORRECTLY_ROUNDED.sin,
};

/** One corner of WGSL's bounds: atan 4,096 ULP low, acos and asin high, sin and cos 2⁻¹¹ low. */
const WGSL_CORNER: Builtins = {
  atan: (x) => nudge(f(Math.atan(x)), -4096),
  acos: (x) => f(Math.acos(x) + 6.77e-5),
  asin: (x) => f(Math.asin(x) + 6.81e-5),
  cos: (x) => f(Math.cos(x) - 2 ** -11),
  sin: (x) => f(Math.sin(x) - 2 ** -11),
};

/** `howell_view_factor` as T6.c built it: F, its `tan` as WGSL inherits it, sin ÷ cos. */
function oldHowell(b: Builtins, h: number, phiIn: number): number {
  const phi = Math.min(Math.abs(phiIn), TRIG_PI);
  const h2 = f(h * h);
  const edge = b.acos(f(1 / h));
  if (phi <= edge) {
    return f(b.cos(phi) / h2);
  }
  if (phi >= f(TRIG_PI - edge)) {
    return 0;
  }
  const x = f(Math.sqrt(f(h2 - 1)));
  const y = f(-x / f(b.sin(phi) / b.cos(phi)));
  const root = f(Math.sqrt(Math.max(0, f(1 - f(y * y)))));
  const first = f(
    f(f(b.cos(phi) * b.acos(Math.min(Math.max(y, -1), 1))) - f(f(x * b.sin(phi)) * root)) /
      f(TRIG_PI * h2),
  );
  const second = f(b.atan(f(f(b.sin(phi) * root) / x)) / TRIG_PI);
  return f(first + second);
}

function oldVisible(b: Builtins, h: number, phi: number): number {
  const rho = b.asin(f(1 / h));
  const x = Math.min(Math.max(f(f(f(0.5 * TRIG_PI) - phi) / rho), -1), 1);
  return f(f(b.acos(-x) + f(x * f(Math.sqrt(f(1 - f(x * x)))))) / TRIG_PI);
}

/** `sphere_irradiance` as T6.c built it. */
function oldTerm(b: Builtins, h: number, phi: number, horizon: number): number {
  const eta = Math.max(horizon, 0);
  if (eta === 0) {
    return f(f(h * h) * oldHowell(b, h, phi));
  }
  const tilted = f(phi + eta);
  const direct = f(f(f(b.cos(eta) * h) * h) * oldHowell(b, h, tilted));
  const tangential = f(f(b.sin(eta) * b.sin(tilted)) * oldVisible(b, h, tilted));
  return Math.max(0, f(direct + tangential));
}

// --- the views --------------------------------------------------------------------------------

/** One `f32` input of the term, and the twin's value on it. */
interface Point {
  readonly h: number;
  readonly phiRad: number;
  readonly horizonRad: number;
  readonly exact: number;
}

function point(h: number, phiRad: number, horizonRad: number): Point {
  const [h32, phi32, horizon32] = [f(h), f(phiRad), f(horizonRad)];
  return {
    h: h32,
    phiRad: phi32,
    horizonRad: horizon32,
    exact: sphereIrradianceFactor(h32, phi32, horizon32),
  };
}

/** The smooth figure's band at `h`: φ from 90° − 1.5ρ to 90° + 1.5ρ in `steps` steps each side. */
function smoothBand(h: number, steps = 24): ReadonlyArray<Point> {
  const rho = Math.asin(1 / h);
  return Array.from({ length: 2 * steps + 1 }, (_, i) =>
    point(h, Math.PI / 2 + ((i - steps) / steps) * 1.5 * rho, 0),
  );
}

/**
 * A flat element's shadow edge under a horizon of `horizonDeg`: the Sun's centre at e★ within
 * ±1.5ρ of it, φ = π/2 − e★, and the horizon carried into the element's frame as R10.T8.b carries
 * it, e − (e★ − (π/2 − φ)) (`litFraction.ts`' `elementHorizonRad`).
 */
function flatEdge(h: number, horizonDeg: number, steps = 16): ReadonlyArray<Point> {
  const rho = Math.asin(1 / h);
  const e = horizonDeg * RAD_PER_DEG;
  return Array.from({ length: 2 * steps + 1 }, (_, i) => {
    const star = e + ((i - steps) / steps) * 1.5 * rho;
    const phiRad = f(Math.PI / 2 - star);
    return point(h, phiRad, e - (star - (Math.PI / 2 - phiRad)));
  });
}

/** The worst |term − twin| over `points`, of face-on. */
function worstError(points: ReadonlyArray<Point>, term: (p: Point) => number): number {
  return Math.max(...points.map((p) => Math.abs(term(p) - p.exact)));
}

/** The arithmetics under which `measure` is `bound` or more (or NaN), with its value. */
function failing(
  bound: number,
  measure: (ops: Arithmetic) => number,
): ReadonlyArray<{ readonly arithmetic: string; readonly value: number }> {
  return ARITHMETICS.map(([arithmetic, ops]) => ({ arithmetic, value: measure(ops) })).filter(
    ({ value }) => !(value < bound),
  );
}

/** The new form's worst error over `points` under `ops`, of face-on. */
function newWorst(points: ReadonlyArray<Point>, ops: Arithmetic): number {
  return worstError(points, (p) => newTerm(ops, p.h, p.phiRad, p.horizonRad));
}

/** WGSL's built-in trigonometry, as the atmosphere's static check finds it. */
const BUILTIN_TRIGONOMETRY = ((): RegExp => {
  const entry = FORBIDDEN_IN_ATMOSPHERE_WGSL.find(([name]) => name === "builtin trigonometry");
  if (entry === undefined) {
    throw new Error("test/atmosphereWgsl.ts no longer lists WGSL's builtin trigonometry");
  }
  return entry[1];
})();

/** The body of WGSL function `name` in `code`, from its signature to its closing brace. */
function functionBody(code: string, name: string): string {
  const start = code.indexOf(`fn ${name}(`);
  const open = code.indexOf("{", start);
  if (start < 0 || open < 0) {
    return "";
  }
  let depth = 0;
  for (let i = open; i < code.length; i += 1) {
    depth += code[i] === "{" ? 1 : code[i] === "}" ? -1 : 0;
    if (depth === 0) {
      return code.slice(start, i + 1);
    }
  }
  return "";
}

describe("the horizon term in f32 (R07.T6.d)", () => {
  it("as T6.c built it on Mesa's lowerings, takes the smooth terminator negative sunward of 90°", () => {
    const at90 = point(H_AT_1_AU, Math.PI / 2, 0);
    expect(at90.exact).toBeCloseTo(9.868e-4, 6);
    expect(oldTerm(MESA, at90.h, at90.phiRad, 0)).toBeLessThan(-4e-4);
    // Already at a tenth of the Sun's radius sunward of 90° (decision-r10-t8b §2: from 0.16ρ).
    const rho = Math.asin(1 / H_AT_1_AU);
    const sunward = point(H_AT_1_AU, Math.PI / 2 - 0.1 * rho, 0);
    expect(sunward.exact).toBeGreaterThan(1e-3);
    expect(oldTerm(MESA, sunward.h, sunward.phiRad, 0)).toBeLessThan(0);
    // `nir_atan` itself: −2.1 × 10⁻⁵ of its argument at 10⁻³.
    expect(nirAtan(f(1e-3)) / f(1e-3) - 1).toBeCloseTo(-2.1e-5, 6);
  });

  // The acceptance's "no longer goes negative sunward of 90°" is the unclamped term's, so that it
  // holds without `sphere_irradiance`'s clamp at 0, which only the far edge's rounding needs.
  it.each([3, 11.5, H_AT_1_AU])(
    "keeps the smooth terminator positive sunward of 90° without the clamp at H = %f, at WGSL's bounds",
    (h) => {
      const sunward = smoothBand(h).filter((p) => p.phiRad <= Math.PI / 2);
      expect(
        failing(0, (ops) => Math.max(...sunward.map((p) => -howellIrradiance(ops, p.h, p.phiRad)))),
      ).toEqual([]);
    },
  );

  it.each([3, 11.5, H_AT_1_AU])(
    "meets the twin across the smooth terminator within 4 × 10⁻⁷ of face-on at H = %f, at WGSL's bounds",
    (h) => {
      const band = smoothBand(h);
      expect(failing(4e-7, (ops) => newWorst(band, ops))).toEqual([]);
    },
  );

  // decision-r10-t8b §2: on Mesa a flat element's edge erred by 8.1% of its light at 1° and 1.6% at
  // 5°; at WGSL's corner by more.
  it.each([
    [1, 0.08, 4e-5],
    [5, 0.016, 4e-5],
    [30, 0.0024, 4e-5],
  ])(
    "holds a flat element's shadow edge under a %f° horizon far below the old form's error",
    (horizonDeg, oldMesaAtLeast, newAtMost) => {
      const edge = flatEdge(H_AT_1_AU, horizonDeg);
      const light = Math.sin(horizonDeg * RAD_PER_DEG);
      const old = (b: Builtins): number =>
        worstError(edge, (p) => oldTerm(b, p.h, p.phiRad, p.horizonRad)) / light;
      expect(old(MESA)).toBeGreaterThan(oldMesaAtLeast);
      expect(old(WGSL_CORNER)).toBeGreaterThan(old(MESA));
      expect(failing(newAtMost, (ops) => newWorst(edge, ops) / light)).toEqual([]);
    },
  );

  // "Nothing changes beyond f32 noise where the old form was accurate": wholly lit, the two forms
  // differ by `preciseSinCos`'s error against a correctly rounded `cos`, a few ULP of 1.
  it.each([3, 11.5, H_AT_1_AU])(
    "changes nothing beyond f32 noise with the whole disc up, at H = %f",
    (h) => {
      const lit = Array.from({ length: 89 }, (_, deg) => point(h, deg * RAD_PER_DEG, 0)).filter(
        (p) => Math.cos(p.phiRad) >= 1 / p.h,
      );
      expect(lit.length).toBeGreaterThan(60);
      const shift = lit.map((p) =>
        Math.abs(newTerm(ROUNDED, p.h, p.phiRad, 0) - oldTerm(CORRECTLY_ROUNDED, p.h, p.phiRad, 0)),
      );
      expect(Math.max(...shift)).toBeLessThan(2e-7);
    },
  );

  // Across the band and under a horizon the new form's worst error is no more than 2 × 10⁻⁷ of
  // face-on above the old one's with correctly rounded built-ins. Point by point the old form can
  // come out closer, where its cancellation's error happens to offset the error both forms share:
  // f32's π/2 (4.4 × 10⁻⁸ rad) and the rounding of φ + η in x = (π/2 − φ − η) ÷ ρ.
  it.each([3, 11.5, H_AT_1_AU])(
    "changes nothing beyond f32 noise where the old form was accurate across the band, at H = %f",
    (h) => {
      for (const points of [smoothBand(h), flatEdge(h, 1), flatEdge(h, 5), flatEdge(h, 30)]) {
        const oldWorst = worstError(points, (p) =>
          oldTerm(CORRECTLY_ROUNDED, p.h, p.phiRad, p.horizonRad),
        );
        expect(newWorst(points, ROUNDED)).toBeLessThan(oldWorst + 2e-7);
      }
    },
  );

  it("meets the twin on the smooth figure from H = 1.01 to 10⁵ within 5 × 10⁻⁶ of face-on", () => {
    for (const h of [1.01, 1.5, 2_150, 1e5]) {
      const band = smoothBand(h);
      expect(failing(5e-6, (ops) => newWorst(band, ops))).toEqual([]);
    }
  });

  // Correctly rounded, 2.7 × 10⁻⁷; at WGSL's bounds the last halving's p³ and quotient dominate.
  it("takes (atan z − z) ÷ z³ to 2 × 10⁻⁶ of itself from 10⁻⁶ to 10⁴, at WGSL's bounds", () => {
    const zs = Array.from({ length: 101 }, (_, k) => f(10 ** ((k - 60) / 10)));
    const relative = (ops: Arithmetic, z: number): number => {
      const exact =
        z < 0.05 ? -1 / 3 + (z * z) / 5 - z ** 4 / 7 + z ** 6 / 9 : (Math.atan(z) - z) / z ** 3;
      return Math.abs(atanTail(ops, z) / exact - 1);
    };
    expect(failing(2e-6, (ops) => Math.max(...zs.map((z) => relative(ops, z))))).toEqual([]);
  });

  it("is finite for a point star, where the old form's H² overflows f32 to NaN", () => {
    for (const h of [1e18, 1e30]) {
      for (const [phiDeg, horizonDeg, expected] of [
        [60, 0, Math.cos(60 * RAD_PER_DEG)],
        [91, 0, 0],
        [80, 5, Math.cos(80 * RAD_PER_DEG)],
        [88, 5, 0],
      ] as const) {
        const p = point(h, phiDeg * RAD_PER_DEG, horizonDeg * RAD_PER_DEG);
        expect(
          failing(3e-7, (ops) => Math.abs(newTerm(ops, p.h, p.phiRad, p.horizonRad) - expected)),
        ).toEqual([]);
      }
    }
    expect(oldTerm(CORRECTLY_ROUNDED, f(1e30), f(60 * RAD_PER_DEG), 0)).toBeNaN();
  });

  it.each([
    "atan_tail",
    "howell_irradiance",
    "disc_visible_fraction",
    "sphere_irradiance",
    "planetshine_irradiance",
  ])("leaves WGSL's trigonometry out of litBody.wgsl's %s", (name) => {
    const body = functionBody(wgslCode(litBodyWgsl), name);
    expect(body).toContain("return");
    expect(body).not.toMatch(BUILTIN_TRIGONOMETRY);
  });

  it("gives the disc's horizon term an angle formed without WGSL's trigonometry", () => {
    const call = /sphere_irradiance\([^;]*;/u.exec(wgslCode(bodyDiscWgsl))?.[0] ?? "";
    expect(call).toContain("phi");
    expect(call).not.toMatch(BUILTIN_TRIGONOMETRY);
    expect(wgslCode(bodyDiscWgsl)).toMatch(
      /let phi = preciseAtan2\(length\(cross\(normal, towards\)\), mu0\);/u,
    );
  });
});
