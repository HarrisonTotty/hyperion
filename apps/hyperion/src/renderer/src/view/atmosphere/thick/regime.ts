/**
 * The thick regime's thresholds and router (plan R08, Design note 9; Provides, "Thick
 * atmospheres").
 *
 * @remarks
 * R08.T5.b created this file for the deck split's optical depth, which its stand-in label reads
 * (Design note 12). R08.T10.a adds the provisional thick line, {@link PROVISIONAL_THICK_OPTICAL_DEPTH},
 * and R08.T10.c moves the line's reason here from `assemble.ts`, {@link THICK_REASON}. R08.T13.c2
 * adds the measured boundary, {@link THICK_MS_BOUNDARY}, and the router, {@link classifyRegime},
 * which replace the provisional line once R08.T13.d moves its call sites. Its imports stay light,
 * `medium.ts` alone (and type imports), so that the render thread can test a kept scene's prebuilt
 * medium without the optics data: the forward delta's share and a table's mean cosine are read
 * here, not through `tables.ts` (which brings WGSL and the engine) or `aerosol.ts` (which imports
 * this file, and brings the optics data).
 */

import type { Rgb } from "../../photometry/toneCurve";
import {
  type AtmosphereMedium,
  columnLengthM,
  extinction,
  type MediumTerm,
  type PhaseTable,
} from "../medium";

/**
 * The vertical optical depth at 550 nm above which a body-wide cloud deck splits the atmosphere in
 * two, Hillaire's tables above it and the deck's bake below (Design note 9).
 *
 * @remarks
 * The rendering brainstorm's figure ("a cloud deck of optical depth above about 10 splits the
 * atmosphere in two", its "Atmosphere" section and open question 3). The stand-in label takes the
 * same threshold: a stand-in material lying deeper than it carries no label
 * (`decision-r08-licences.md` row 5; `materials/materials.ts`). Light from beneath is not nil
 * there: a conservative deck of τ 10 transmits 31–47% diffusely for g 0.7–0.85, by the two-stream
 * T = 1 ÷ (1 + ¾(1 − g)τ). The deck's bake (R08.T15) carries that light.
 */
export const CLOUD_DECK_SPLIT_OPTICAL_DEPTH = 10;

/**
 * The provisional thick line: a medium whose vertical optical depth from the ground to the top
 * reaches 1 on its worst channel, over every term but the absorbers, is drawn by the analytic
 * term under `ATMOSPHERE: APPROXIMATE` (plan R08, Design note 9's routing; R08.T10.a,
 * `decision-r08-t10a-leans.md` item 1). Provisional: R08.T13's `classifyRegime` retires it.
 *
 * @remarks
 * 1 is the column's optical thickness, the conventional line between thin and thick, not a
 * measured boundary. For Rayleigh air it falls at about 3.9 Earth columns (Earth's Rayleigh τ is
 * about 0.26 at 433.6 nm, `earth.ts`'s τ_R(550 nm) = 0.0969 scaled by λ^−4.08), inside the fifty
 * times Earth's density Hillaire tested to (Design note 9) and past the 3 Earth columns at which
 * R08.T4.a measured the thin path's three-channel transmittance 0.0151 off in u′v′ at low sun.
 * Absorbers are left out: the analytic term fails in multiple scattering,
 * which absorption alone makes none of, and their depth is each light's curve of growth, so that
 * counting it would make the label depend on the body's stars.
 *
 * @deprecated Replaced by {@link classifyRegime} and {@link THICK_MS_BOUNDARY} (R08.T13.c2);
 *   R08.T13.d moves `assemble.ts` and `AtmosphereCache.ts` to them and retires this line.
 */
export const PROVISIONAL_THICK_OPTICAL_DEPTH = 1;

/**
 * A medium's vertical optical depth per channel from the ground to its top over every term but
 * the absorbers, each at its whole extinction (an aerosol's forward peak included): the depth the
 * provisional thick line reads.
 *
 * @remarks
 * Along the zenith from the ground a ray's height is its distance, whatever the ground's radius,
 * so this is Σ (scattering + absorption) ∫₀^top ρ dh, in closed form by `medium.ts`'s
 * `columnLengthM`: `opticalDepth.ts`'s `opticalDepthToTop(medium, R, R, 1)`, which takes the same
 * integral by Simpson's rule (held equal to it to 10⁻⁶ by the tests), without that module's
 * optics data.
 */
export function verticalOpticalDepth(medium: AtmosphereMedium): Rgb {
  let red = 0;
  let green = 0;
  let blue = 0;
  for (const term of medium.terms) {
    if (term.absorber !== undefined) {
      continue;
    }
    const lengthM = columnLengthM(term.density, medium.topHeightM);
    const [r, g, b] = extinction(term);
    red += r * lengthM;
    green += g * lengthM;
    blue += b * lengthM;
  }
  return [red, green, blue];
}

/**
 * Whether a medium is thick by the provisional line: its {@link verticalOpticalDepth} reaches
 * {@link PROVISIONAL_THICK_OPTICAL_DEPTH} on its worst channel, so that it carries
 * `atmosphereApproximate` until R08.T13's gate. Every medium the cache draws is tested, a kept
 * scene's fixtures among them.
 *
 * @deprecated Replaced by {@link classifyRegime} (R08.T13.c2), whose `thickScattering` and
 *   `cloudDeck` carry the label in its place; R08.T13.d moves the call sites and retires this.
 */
export function provisionalThick(medium: AtmosphereMedium): boolean {
  return Math.max(...verticalOpticalDepth(medium)) >= PROVISIONAL_THICK_OPTICAL_DEPTH;
}

/**
 * Why a medium {@link provisionalThick} finds thick carries `atmosphereApproximate`, as player
 * text, as `assemble.ts`'s other reasons are (R08.T10.a): the assembly gives it a record's body,
 * and since R08.T10.c the atmosphere cache a kept scene's medium, so that the label block can name
 * the reason beside the note (`decision-r08-approximate-plate.md`, item 1b). It lives here, beside
 * the line, so that the render thread reads it without `assemble.ts`'s optics data.
 */
export const THICK_REASON = "a thick atmosphere drawn by the thin-atmosphere method";

// --- The measured boundary (R08.T13.c2) ---------------------------------------------------------

/**
 * What set a cell's τ* in the sweep's walk (`decision-r08-t13-scope.md`, ruling 5): `drift`, the
 * least traced τ at which the column fails the metric; `reach`, the last verified passing τ where a
 * cap or a refusal stopped the walk; `top`, 8, where no stop came.
 */
export type BoundaryKind = "drift" | "reach" | "top";

/**
 * The thick boundary over the sweep's grid of single-scattering albedo ω and asymmetry g (plan
 * R08, Design note 9; Provides, "Thick atmospheres", as built by R08.T13.c2).
 *
 * @remarks
 * Its cells are row-major by ω: cell (i, j), ω row i and g column j, at i × `g.length` + j. The ω
 * rows stand in the sweep's own order, 1 first and descending, and the g columns ascending, as
 * the sweep's files list them (`ms-sweep-w<ω×1000>.json`, a file an ω row, its columns by g).
 */
export interface ThickBoundary {
  /** The single-scattering albedos of the rows, descending from 1. */
  readonly omega: Float64Array;
  /** The asymmetries of the columns, ascending from 0. */
  readonly g: Float64Array;
  /** τ*(ω, g): the vertical extinction optical depth, ground to top, at which the term drifts. */
  readonly tauStar: Float64Array;
  /** A*(ω, g): the least ground albedo at which the floor's τ already misses, 1 where none does. */
  readonly albedoStar: Float64Array;
  /** What set each cell's τ*. */
  readonly kind: ReadonlyArray<BoundaryKind>;
}

/** 1/8, 2⁻³: a grid value of the sweep's half-octave ladder 2^(k ÷ 2). */
const EIGHTH = 2 ** -3;
/** √2/16, 2^(−3.5). */
const ROOT2_SIXTEENTH = 2 ** -3.5;
/** √2/8, 2^(−2.5). */
const ROOT2_EIGHTH = 2 ** -2.5;
/** ¼, 2⁻². */
const QUARTER = 2 ** -2;
/** √2/4, 2^(−1.5). */
const ROOT2_QUARTER = 2 ** -1.5;
/** ½, 2⁻¹. */
const HALF = 2 ** -1;

/**
 * Hillaire's analytic multiple-scattering term's measured boundary, τ*(ω, g) and A*(ω, g), from
 * R08.T13.b's sweep (plan R08, Design note 9; `decision-r08-t13-scope.md`, rulings 5 and 9 and
 * "#107's floor misses"): the 49 cells of the sweep's grid, ω ∈ {1, 0.99, 0.95, 0.9, 0.8, 0.6,
 * 0.4} in its rows (1 first) and g ∈ {0, 0.3, 0.5, 0.65, 0.75, 0.85, 0.9} in its columns
 * ({@link ThickBoundary}'s order).
 *
 * @remarks
 * - **τ\*** (ruling 5): a column is a single-layer grey medium, e^(−h ÷ 8 km) to 100 km over a
 *   grey Lambertian ground of 0.3 on Earth's sphere, its phase Rayleigh's at g 0 and
 *   Henyey–Greenstein's tabulated above (`test/atmosphereSweep.ts`). τ\* is the least traced
 *   vertical extinction optical depth at which the CPU twin (`reference/twin.ts`) leaves Design
 *   note 10's metric (`gate.ts`) at some geometry against the reference tracer: a `drift`. A
 *   geometry that already fails at the column's floor, τ 1/32, is a floor miss and sets no τ\*.
 *   Where a cap or a refusal stopped the walk first, τ\* is the last verified passing τ (`reach`),
 *   and where no stop came, the last octave, 8 (`top`). Every value is its exact grid value,
 *   2^(k ÷ 2). Every cell of this sweep is a measured `drift`.
 * - **A\*** (the floor scan, "#107's floor misses", items 1–3): each column's floor, τ 1/32, was
 *   traced again over grey grounds of 0, 0.02, 0.05, 0.1, 0.2, 0.5 and 0.8 beside the walk's
 *   0.3. A geometry that fails over the black ground is a fixed miss (the kernels'); one that
 *   passes there and fails over a brighter ground is a ground miss, Hillaire's ground bounce
 *   scattered as if isotropic, about 1 + cA of a forward-peaked medium's sky, which no τ shrinks.
 *   A\* is the least ground at which a ground miss occurs, and 1 where none does through 0.8.
 * - **Provenance**: the eight files `reference/ms-sweep-w1000.json`, `-w990`, `-w950`, `-w900`,
 *   `-w800`, `-w600`, `-w400` and `-w1000-a800`, with their cases `ms-sweep-w*.case.json`, landed
 *   in 3d1cf4c8 (R08.T13.b), traced by `hyperion-fit atmosphere-sweep` of SHA-256
 *   1551629a76781ac80ba700308d88782ebbb150d9e253770d36729a9f7475aec8. The walk's ground, 0.3, is
 *   a hand value near Earth's and Mars's Bond albedos. The bright column, (1, 0) over a ground of
 *   0.8 (`-w1000-a800`), a drift at √2/16 with A\* 1, is recorded and not in the table.
 *   `reference/sweepBoundary.test.ts` recomputes every cell from the files with `gate.ts` itself,
 *   and holds this table equal to that recomputation, cell by cell with its kind.
 * - **The table**, τ\* then A\* in each cell (r = √2):
 *
 *   ```text
 *   ω \ g    0          0.3        0.5          0.65         0.75         0.85         0.9
 *   1        1/8, 1     1/8, 0.2   r/16, 0.1    r/16, 0.02   r/16, 0.02   r/16, 0.02   r/16, 0.02
 *   0.99     1/8, 1     1/8, 0.2   1/8, 0.05    r/16, 0.02   r/16, 0.02   r/16, 0.02   r/16, 0.02
 *   0.95     1/8, 1     1/8, 0.2   1/8, 0.05    1/8, 0.02    r/16, 0.02   r/16, 0.02   r/16, 0.02
 *   0.9      1/8, 1     r/8, 0.2   1/8, 0.1     1/8, 0.02    r/16, 0.02   r/16, 0.02   r/16, 0.02
 *   0.8      r/8, 1     r/8, 0.2   r/8, 0.05    r/8, 0.02    1/8, 0.02    r/16, 0.02   r/16, 0.02
 *   0.6      r/4, 1     1/4, 0.2   1/4, 0.05    1/4, 0.02    r/8, 0.02    r/16, 0.02   r/16, 0.02
 *   0.4      1/2, 1     1/2, 0.2   1/4, 0.05    r/4, 0.02    1/4, 0.02    1/8, 0.02    r/16, 0.02
 *   ```
 *
 * - **Against R08.T4.a's depth note**: (1, 0)'s τ\* 1/8 stands at about half an Earth column of
 *   dry air on its worst channel (blue; `thick/regime.test.ts` computes it), well inside the two
 *   or three Earth columns past which no three wavelengths hold the transmittance (R08.T4.a:
 *   Δu′v′ 0.0106, 0.0151 and 0.100 at 2, 3 and 10 columns, at the fitted triple and all twelve
 *   suns). So a medium the boundary keeps `thin` by depth is shallower than the depth at which
 *   its three-channel beam fails, and its scattered light past the boundary is the spectral
 *   bakes' (Design note 5).
 */
export const THICK_MS_BOUNDARY: ThickBoundary = {
  omega: Float64Array.of(1, 0.99, 0.95, 0.9, 0.8, 0.6, 0.4),
  g: Float64Array.of(0, 0.3, 0.5, 0.65, 0.75, 0.85, 0.9),
  // One row of seven a line, ω 1 first, as the sweep's table reads.
  // prettier-ignore
  tauStar: Float64Array.of(
    EIGHTH, EIGHTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH,
    EIGHTH, EIGHTH, EIGHTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH,
    EIGHTH, EIGHTH, EIGHTH, EIGHTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH,
    EIGHTH, ROOT2_EIGHTH, EIGHTH, EIGHTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH,
    ROOT2_EIGHTH, ROOT2_EIGHTH, ROOT2_EIGHTH, ROOT2_EIGHTH, EIGHTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH,
    ROOT2_QUARTER, QUARTER, QUARTER, QUARTER, ROOT2_EIGHTH, ROOT2_SIXTEENTH, ROOT2_SIXTEENTH,
    HALF, HALF, QUARTER, ROOT2_QUARTER, QUARTER, EIGHTH, ROOT2_SIXTEENTH,
  ),
  // prettier-ignore
  albedoStar: Float64Array.of(
    1, 0.2, 0.1, 0.02, 0.02, 0.02, 0.02,
    1, 0.2, 0.05, 0.02, 0.02, 0.02, 0.02,
    1, 0.2, 0.05, 0.02, 0.02, 0.02, 0.02,
    1, 0.2, 0.1, 0.02, 0.02, 0.02, 0.02,
    1, 0.2, 0.05, 0.02, 0.02, 0.02, 0.02,
    1, 0.2, 0.05, 0.02, 0.02, 0.02, 0.02,
    1, 0.2, 0.05, 0.02, 0.02, 0.02, 0.02,
  ),
  kind: Array.from({ length: 49 }, (): BoundaryKind => "drift"),
};

/**
 * The least vertical optical depth the ground line reads, 1/32: the sweep's floor point, at which
 * its floor scan measured A\* ("#107's floor misses", item 3). Below it a medium stays `thin`,
 * untested, its sky dim.
 */
export const BOUNDARY_FLOOR_OPTICAL_DEPTH = 1 / 32;

/** The deepest optical depth the sweep's grid reaches, 8: a channel deeper than it is thick. */
export const BOUNDARY_TOP_OPTICAL_DEPTH = 8;

/** The boundary read at one (ω̄, ḡ), with the clamps it took (ruling 9). */
export interface BoundaryReading {
  /** τ\*, bilinear in ln τ\* over the cell. */
  readonly tauStar: number;
  /** A\*, bilinear over the cell. */
  readonly albedoStar: number;
  /** ω̄ lay below the grid's last row and read it, which is conservative: τ\* rises as ω falls. */
  readonly albedoBelowGrid: boolean;
  /** ḡ lay outside the grid's columns and read the nearest; past the last, an extrapolation. */
  readonly asymmetryOffGrid: boolean;
}

/**
 * (1 − w) a + w b, `a` alone at w 0 and `b` alone at w 1: a corner of weight 0 is not read, so that
 * a τ\* of 0 there (a walk stopped at its floor) gives no −∞ × 0. Between them it is the plain
 * linear blend, to the bit.
 */
function blend(a: () => number, b: () => number, w: number): number {
  if (w === 0) {
    return a();
  }
  return w === 1 ? b() : (1 - w) * a() + w * b();
}

/** Where x falls on an ascending or descending grid, clamped to it: the bracket and its weight. */
function bracketOf(grid: Float64Array, x: number): readonly [number, number, number] {
  const last = grid.length - 1;
  const first = grid[0] ?? Number.NaN;
  const final = grid[last] ?? Number.NaN;
  const descending = final < first;
  const held = Math.min(Math.max(x, Math.min(first, final)), Math.max(first, final));
  for (let i = 0; i < last; i += 1) {
    const a = grid[i] ?? Number.NaN;
    const b = grid[i + 1] ?? Number.NaN;
    const inside = descending ? held <= a && held >= b : held >= a && held <= b;
    if (inside) {
      return [i, i + 1, (held - a) / (b - a)];
    }
  }
  return [last - 1, last, 1];
}

/**
 * The boundary at single-scattering albedo `omega` and asymmetry `g` (ruling 9): τ\* bilinear in
 * ln τ\* over the (ω, g) cell, and A\* bilinear, ω below the grid's last row (0.4) reading that
 * row and g held to the grid's columns, [0, 0.9].
 */
export function readThickBoundary(
  omega: number,
  g: number,
  boundary: ThickBoundary = THICK_MS_BOUNDARY,
): BoundaryReading {
  const columns = boundary.g.length;
  const [r0, r1, s] = bracketOf(boundary.omega, omega);
  const [c0, c1, t] = bracketOf(boundary.g, g);
  const at = (values: Float64Array, row: number, column: number): number =>
    values[row * columns + column] ?? Number.NaN;
  const bilinear = (f: (row: number, column: number) => number): number =>
    blend(
      () =>
        blend(
          () => f(r0, c0),
          () => f(r0, c1),
          t,
        ),
      () =>
        blend(
          () => f(r1, c0),
          () => f(r1, c1),
          t,
        ),
      s,
    );
  const lastOmega = boundary.omega[boundary.omega.length - 1] ?? Number.NaN;
  const firstG = boundary.g[0] ?? Number.NaN;
  const lastG = boundary.g[columns - 1] ?? Number.NaN;
  return {
    tauStar: Math.exp(bilinear((row, column) => Math.log(at(boundary.tauStar, row, column)))),
    albedoStar: bilinear((row, column) => at(boundary.albedoStar, row, column)),
    albedoBelowGrid: omega < lastOmega,
    asymmetryOffGrid: g < firstG || g > lastG,
  };
}

// --- A medium's optics as the kernels draw it -----------------------------------------------------

/**
 * Gauss–Legendre nodes and weights on [−1, 1] by Newton's method on Pₙ, as (node, weight):
 * `lighting/quadrature.ts`'s `gaussLegendre`, step for step, here so that this file's imports stay
 * light.
 */
function gaussLegendreNodes(n: number): ReadonlyArray<readonly [number, number]> {
  const rule: Array<readonly [number, number]> = [];
  for (let i = 0; i < n; i += 1) {
    let z = Math.cos((Math.PI * (i + 0.75)) / (n + 0.5));
    let derivative = 0;
    for (let iteration = 0; iteration < 100; iteration += 1) {
      let p0 = 1;
      let p1 = 0;
      for (let k = 1; k <= n; k += 1) {
        const p2 = p1;
        p1 = p0;
        p0 = ((2 * k - 1) * z * p1 - (k - 1) * p2) / k;
      }
      derivative = (n * (z * p0 - p1)) / (z * z - 1);
      const step = p0 / derivative;
      z -= step;
      if (Math.abs(step) < 1e-15) {
        break;
      }
    }
    rule.push([z, 2 / ((1 - z * z) * derivative * derivative)]);
  }
  return rule;
}

/** The Gauss–Legendre rule on each interval of a table's moment (order 8), as (node, weight). */
const MOMENT_RULE: ReadonlyArray<readonly [number, number]> = gaussLegendreNodes(8);

/**
 * A table channel's first moment as the kernels read it, ∫ p cos θ dΩ, p linear in u = √(θ ÷ π)
 * between its entries: 2π ∫₀¹ p(u) cos(πu²) sin(πu²) 2πu du by Gauss–Legendre of order 8 on each
 * interval, as `forwardPeak.ts`'s `phaseTableMoment` (and so `aerosol.ts`'s `phaseTableAsymmetry`)
 * takes it, node for node.
 */
function tableMeanCosine(table: PhaseTable, channel: 0 | 1 | 2): number {
  const { u } = table;
  const values = table.values[channel];
  let sum = 0;
  for (let i = 0; i + 1 < u.length; i += 1) {
    const u0 = u[i] ?? Number.NaN;
    const u1 = u[i + 1] ?? Number.NaN;
    const v0 = values[i] ?? Number.NaN;
    const v1 = values[i + 1] ?? Number.NaN;
    const half = 0.5 * (u1 - u0);
    for (const [x, w] of MOMENT_RULE) {
      const f = 0.5 * (1 + x);
      const uu = u0 + f * (u1 - u0);
      const angle = Math.PI * uu * uu;
      sum += half * w * (v0 + f * (v1 - v0)) * Math.cos(angle) * Math.sin(angle) * 2 * Math.PI * uu;
    }
  }
  return 2 * Math.PI * sum;
}

/**
 * Cornette and Shanks's phase function's own mean cosine for its shape parameter g:
 * 3g(4 + g²) ÷ (5(2 + g²)) (Cornette and Shanks 1992, Appl. Opt. 31, 3152, not read; read in
 * Draine 2003, ApJ 598, 1017, eq. A2 at α = 1, by R08.T13.c2's science check; the form `earth.ts`'s
 * `AEROSOL_ASYMMETRY` cites, held here to a quadrature of `medium.ts`'s `phaseAt` by
 * `thick/regime.test.ts`). It is not g: 0.650 at g 0.584.
 */
export function cornetteShanksMeanCosine(shape: number): number {
  const g2 = shape * shape;
  return (3 * shape * (4 + g2)) / (5 * (2 + g2));
}

/** One channel's optics of one term: its column's scattering and absorption, and its mean cosine. */
interface ChannelOptics {
  readonly scatteringDepth: number;
  readonly absorptionDepth: number;
  readonly meanCosine: number;
}

/**
 * A term's column optics in one channel as the kernels draw it: its forward delta f (a tabulated
 * phase's `forwardPeak`) taken out by similarity, τ_s(1 − f) and (g − f) ÷ (1 − f), as `tables.ts`'s
 * `withoutForwardDelta` takes it out (the delta's own mean cosine, 1 − 8.3 × 10⁻⁸ as read, taken as
 * 1, which moves g by 8.3 × 10⁻⁸ f ÷ (1 − f): under 10⁻⁷ for f up to 0.55, 5.8 × 10⁻⁸ at the
 * committed optics' f of about 0.41).
 */
function channelOptics(term: MediumTerm, lengthM: number, channel: 0 | 1 | 2): ChannelOptics {
  const scattering = term.scattering[channel] * lengthM;
  const absorptionDepth = term.absorption[channel] * lengthM;
  const { phase } = term;
  let optics: ChannelOptics;
  switch (phase.kind) {
    case "rayleigh":
    case "none":
      optics = { scatteringDepth: scattering, absorptionDepth, meanCosine: 0 };
      break;
    case "cornette-shanks":
      optics = {
        scatteringDepth: scattering,
        absorptionDepth,
        meanCosine: cornetteShanksMeanCosine(phase.asymmetry),
      };
      break;
    case "tabulated": {
      const f = phase.table.forwardPeak[channel];
      const g = tableMeanCosine(phase.table, channel);
      optics = {
        scatteringDepth: scattering * (1 - f),
        absorptionDepth,
        meanCosine: (g - f) / (1 - f),
      };
      break;
    }
  }
  return optics;
}

/** One channel of a medium as {@link classifyRegime} reads it, with the boundary there. */
export interface ChannelRegime {
  /** τ: the vertical optical depth, ground to top, of every term but the absorbers, after the similarity. */
  readonly opticalDepth: number;
  /** ω̄ = τ_s ÷ τ; 1 for a channel of τ 0, which scatters nothing and is thin. */
  readonly singleScatteringAlbedo: number;
  /** ḡ, weighted by each term's scattering optical depth; 0 for a channel that scatters nothing. */
  readonly asymmetry: number;
  /** The boundary read at (ω̄, ḡ). */
  readonly boundary: BoundaryReading;
  /** The ground's albedo in the channel. */
  readonly groundAlbedo: number;
  /** τ > τ\*(ω̄, ḡ), or τ past the grid's 8: the depth line. */
  readonly thickByDepth: boolean;
  /** τ ≥ 1/32 and the ground's albedo at least A\*(ω̄, ḡ): the ground line. */
  readonly thickByGround: boolean;
}

/**
 * A medium's three channels as {@link classifyRegime} reads them (ruling 9 and "#107's floor
 * misses", item 3): per channel, over every term but the absorbers (whose absorption is each
 * light's curve of growth, which makes no multiple scattering), on the medium the kernels draw.
 *
 * @remarks
 * Each term's column is `columnLengthM` of its own density to the top, as
 * {@link verticalOpticalDepth} takes it; the kernels' resampling of a tabulated density onto its
 * table's levels and of a tabulated phase onto their grid (`tables.ts`'s `kernelMedium`) is not
 * repeated, a change `tables.test.ts` bounds. A table's forward delta is direct light, so its
 * share f comes out of τ_s and g by similarity ({@link channelOptics}); that lowers T10.a's
 * whole-extinction count by f τ_s. ω̄ = τ_s ÷ τ and ḡ = Σ τ_s g ÷ Σ τ_s, each term's g its mean
 * cosine as the kernels read its phase: 0 for Rayleigh's and `none`, Cornette and Shanks's own
 * ({@link cornetteShanksMeanCosine}), a table's first moment. The boundary is read there
 * ({@link readThickBoundary}), and a channel is thick by depth where τ > τ\* or τ > 8, and by the
 * ground where τ ≥ 1/32 and the ground's albedo is at least A\*.
 */
export function regimeChannels(
  medium: AtmosphereMedium,
  boundary: ThickBoundary = THICK_MS_BOUNDARY,
): readonly [ChannelRegime, ChannelRegime, ChannelRegime] {
  const channel = (c: 0 | 1 | 2): ChannelRegime => {
    let scattering = 0;
    let absorption = 0;
    let weighted = 0;
    for (const term of medium.terms) {
      if (term.absorber !== undefined) {
        continue;
      }
      const optics = channelOptics(term, columnLengthM(term.density, medium.topHeightM), c);
      scattering += optics.scatteringDepth;
      absorption += optics.absorptionDepth;
      weighted += optics.scatteringDepth * optics.meanCosine;
    }
    const tau = scattering + absorption;
    const omega = tau > 0 ? scattering / tau : 1;
    const g = scattering > 0 ? weighted / scattering : 0;
    const read = readThickBoundary(omega, g, boundary);
    const groundAlbedo = medium.groundAlbedo[c];
    return {
      opticalDepth: tau,
      singleScatteringAlbedo: omega,
      asymmetry: g,
      boundary: read,
      groundAlbedo,
      thickByDepth: tau > read.tauStar || tau > BOUNDARY_TOP_OPTICAL_DEPTH,
      thickByGround: tau >= BOUNDARY_FLOOR_OPTICAL_DEPTH && groundAlbedo >= read.albedoStar,
    };
  };
  return [channel(0), channel(1), channel(2)];
}

// --- The router (R08.T13.c2) ---------------------------------------------------------------------

/**
 * How a body's atmosphere is drawn (plan R08, Design note 9; Provides' sketch without its bake
 * key, which R08.T14 adds): `thin`, Hillaire's tables; `thickScattering`, the multiple-scattering
 * bake (R08.T14); `cloudDeck`, the deck split at `deckAltitudeM` (R08.T15).
 *
 * @remarks
 * Until R08.T14.d's and R08.T15's gates pass, a thick regime is drawn with the analytic term under
 * `ATMOSPHERE: APPROXIMATE` (Design note 9's routing), which clears only when its regime's gate
 * passes.
 */
export type AtmosphereRegime =
  | { readonly kind: "thin" }
  | { readonly kind: "thickScattering" }
  | {
      readonly kind: "cloudDeck";
      /**
       * Where the split lies, m above the medium's ground: a deck's top, the upper medium's
       * reflecting boundary, or an envelope's floor.
       */
      readonly deckAltitudeM: number;
    };

/**
 * A condensate deck of a body's cover, as the deck rule reads it (P14.T24.c's inventory; B1's
 * client model's `CondensateDeck`, which R08.T13.d maps onto it).
 */
export interface CoverDeck {
  /** Its vertical optical depth at 550 nm, its modes' at their coverage. */
  readonly opticalDepth550: number;
  /** Its base's height above the medium's ground, m. */
  readonly baseAltitudeM: number;
  /** Its top's height above the medium's ground, m. */
  readonly topAltitudeM: number;
}

/**
 * What of a body's clouds the router reads beside its medium (Design note 9's deck rule; Provides'
 * `BodyCover`, widened by R08.T13.c2).
 */
export interface BodyCover {
  /** The share of the sky its clouds cover, P14.T24.b's, in [0, 1] (B1's `cloudFraction`). */
  readonly cloudFraction: number;
  /** Its condensate decks in the inventory's order, hottest base (the deepest) first. */
  readonly decks: ReadonlyArray<CoverDeck>;
  /** Whether the body is a gas envelope, which has no ground and is always split at its floor. */
  readonly envelope: boolean;
}

/**
 * A ground under a clear sky: cloud fraction 0, no deck, not an envelope; how a reference case is
 * traced, with a ground beneath.
 */
export const CLEAR_COVER: BodyCover = { cloudFraction: 0, decks: [], envelope: false };

/**
 * The channel an envelope's floor is found in: green, 1, at the fitted 544 nm (`medium.ts`'s
 * `CHANNEL_WAVELENGTHS_NM`), the nearest to the rule's 550 nm, at which the medium carries no
 * channel. Rayleigh's τ there is about 4.6% over 550 nm's (λ^−4.08; Bodhaine et al. 1999, J. Atmos.
 * Oceanic Technol. 16, 1854, eq. 30, read by R08.T13.c2's science check), the most any term moves
 * it, so the floor lies at most about 0.045 scale heights higher than at 550 nm.
 */
export const ENVELOPE_FLOOR_CHANNEL = 1;

/**
 * An envelope's floor (Design note 9, after decision-deep-decks-split): the shallowest height,
 * m above the medium's ground, at which the vertical optical depth from the top, of every term but
 * the absorbers at its whole extinction, reaches {@link CLOUD_DECK_SPLIT_OPTICAL_DEPTH}, read in
 * {@link ENVELOPE_FLOOR_CHANNEL}; the medium's ground, 0, where the whole column is shallower.
 *
 * @remarks
 * The absorbers are left out as the depth line leaves them out: an absorber's depth is each
 * light's curve of growth, so counting it would put the floor where the body's stars say.
 */
export function envelopeFloorM(medium: AtmosphereMedium): number {
  const top = medium.topHeightM;
  const terms = medium.terms.filter((term) => term.absorber === undefined);
  const columnsToTop = terms.map((term) => columnLengthM(term.density, top));
  const depthAbove = (heightM: number): number =>
    terms.reduce(
      (sum, term, i) =>
        sum +
        extinction(term)[ENVELOPE_FLOOR_CHANNEL] *
          ((columnsToTop[i] ?? Number.NaN) - columnLengthM(term.density, heightM)),
      0,
    );
  if (depthAbove(0) < CLOUD_DECK_SPLIT_OPTICAL_DEPTH) {
    return 0;
  }
  // τ from the top falls as the height rises: the floor is the highest height still at 10.
  let low = 0;
  let high = top;
  for (let step = 0; step < 64; step += 1) {
    const middle = 0.5 * (low + high);
    if (depthAbove(middle) >= CLOUD_DECK_SPLIT_OPTICAL_DEPTH) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return low;
}

/**
 * Routes a body's atmosphere (plan R08, Design note 9; `decision-r08-t13-scope.md`, ruling 9 and
 * "#107's floor misses", item 3): the deck rule per body first, then each channel by the boundary.
 *
 * @remarks
 * - **The deck rule**: a gas envelope is `cloudDeck` at its floor ({@link envelopeFloorM}); a
 *   body whose cloud fraction is 1 and which has a deck of τ(550) above
 *   {@link CLOUD_DECK_SPLIT_OPTICAL_DEPTH} is `cloudDeck` at the top of the shallowest such deck,
 *   though the inventory lists the deepest first: the deeper lies in the medium beneath it. A
 *   broken field, whatever its τ, stays R11's clouds.
 * - **The boundary**, per channel ({@link regimeChannels}): a channel is thick where τ > τ\*(ω̄, ḡ)
 *   (or τ > 8), or where τ ≥ 1/32 and the medium's ground albedo is at least A\*(ω̄, ḡ). The ground
 *   is the body's own, so the label does not depend on its stars. A body thick on any channel is
 *   `thickScattering`, and `thin` otherwise.
 *
 * Every thick regime carries `ATMOSPHERE: APPROXIMATE` until its gate passes (Design note 9's
 * routing).
 */
export function classifyRegime(medium: AtmosphereMedium, cover: BodyCover): AtmosphereRegime {
  if (cover.envelope) {
    return { kind: "cloudDeck", deckAltitudeM: envelopeFloorM(medium) };
  }
  if (cover.cloudFraction >= 1) {
    let shallowest: CoverDeck | undefined;
    for (const deck of cover.decks) {
      const qualifies = deck.opticalDepth550 > CLOUD_DECK_SPLIT_OPTICAL_DEPTH;
      if (qualifies && (shallowest === undefined || deck.topAltitudeM >= shallowest.topAltitudeM)) {
        shallowest = deck;
      }
    }
    if (shallowest !== undefined) {
      return { kind: "cloudDeck", deckAltitudeM: shallowest.topAltitudeM };
    }
  }
  const thick = regimeChannels(medium).some((c) => c.thickByDepth || c.thickByGround);
  return thick ? { kind: "thickScattering" } : { kind: "thin" };
}
