/**
 * `node scripts/nonSpherical.mjs`: reduces TAMUdust2020's data kernels and Yang et al. 2013's
 * roughened ice aggregate to the phase files of `view/atmosphere/materials/phase/` (plan R08,
 * R08.T5.c; science-r08-nonspherical.md §2 and §4).
 *
 * @remarks
 * Both sources are CC BY 4.0, and each phase file is adapted material under it: attributed, linked
 * to the licence, its changes stated in its header, and offered under CC BY 4.0. The archives are
 * fetched once, by URL and MD5, into their own directories, read here as data only, and deleted
 * after the reduction; nothing in CI needs them.
 *
 * - **TAMUdust2020** (M. Saito, P. Yang, J. Ding and X. Liu, J. Atmos. Sci. 78 (2021) 2089–2111,
 *   DOI 10.1175/JAS-D-20-0338.1; Zenodo DOI 10.5281/zenodo.4711247, v1.0.0): the ensemble of 20
 *   irregular hexahedra, tabulated over size parameter x = 2πD ÷ λ (D the maximum dimension, the
 *   v1.1.0 README's definition), the real and imaginary index and the sphericity. The record layout
 *   ({@link KernelLayout}) was read from the archive's GPL-3.0 Fortran as format facts only, and
 *   none of it is ported: each `TAMUdust2020isca.bin` record is 9 little-endian float32, x, A, V,
 *   C_ext, C_sca, C_sca·g, n, k and the sphericity, the areas and cross-sections in units of
 *   (λ ÷ 2π)² and the volume in (λ ÷ 2π)³; each `TAMUdust2020PMat.bin` record is 6 × 498 float32,
 *   C_sca·P₁₁, C_sca·P₁₂, C_sca·P₂₂, C_sca·P₃₃, C_sca·P₄₃ and C_sca·P₄₄ at the 498 angles, with
 *   ∫ P₁₁ dΩ = 4π; records run size outermost, then the index with the real part outer, then the
 *   sphericity. The 498 angles are the source archive's `examples/params/TAMUdust2020_Angle`.
 * - **Yang et al. 2013, version 2** (P. Yang et al., J. Atmos. Sci. 70 (2013) 330–347, DOI
 *   10.1175/JAS-D-12-039.1; L. Bi and P. Yang, J. Quant. Spectrosc. Radiat. Transfer 189 (2017)
 *   228–237; Zenodo record 5348402): `8_columns/Rough050/`, the aggregate of eight columns,
 *   severely roughened (σ = 0.5), as ASCII: `isca.dat` (λ µm, D µm, V µm³, A µm², Q_ext, ω, g) and
 *   `P11.dat`, `P12.dat`, `P22.dat`, `P33.dat`, `P43.dat`, `P44.dat`, each a line of the 498 angles
 *   and then a line per wavelength and size, size varying fastest, the others normalised by P₁₁.
 *
 * Each phase file holds, per wavelength node, the table over size parameter of C_ext, C_sca and
 * C_sca·g (in (λ ÷ 2π)²), V and A, a₁ = P₁₁ at {@link TAMUDUST_PHASE_ANGLES} or {@link YANG_PHASE_ANGLES} angles and the five ratios a₂ ÷ a₁,
 * a₃ ÷ a₁, a₄ ÷ a₁, b₁ ÷ a₁ and b₂ ÷ a₁ at {@link MATRIX_ANGLES_DEG}, with a₂ = P₂₂, a₃ = P₃₃,
 * a₄ = P₄₄, b₁ = P₁₂ and b₂ = {@link B2_SIGN} × P₄₃. The node's table is the kernel interpolated
 * at the node's index and the sphericity {@link SPHERICITY}: the extensive quantities (the
 * cross-sections, V, A and C_sca·Pᵢⱼ) linear in n, ln k and ln S between the kernel's nodes.
 */

import { createHash } from "node:crypto";
import {
  closeSync,
  createReadStream,
  openSync,
  readFileSync,
  readSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { parseArgs } from "node:util";

/** A TAMUdust2020 kernel's grid sizes and its range in k. */
export interface KernelLayout {
  readonly name: "shortwave" | "longwave";
  readonly sizes: number;
  readonly reals: number;
  readonly imaginaries: number;
  readonly sphericities: number;
  readonly angles: number;
  /** The kernel's smallest k, below which the floor rule applies. */
  readonly kFloor: number;
  /** Its largest k. */
  readonly kMax: number;
  /** Its range in n. */
  readonly nMin: number;
  readonly nMax: number;
}

/** The shortwave kernel (`tables/SW/`): x ≤ 11,800, n 1.37–1.70, k 10⁻⁴–0.1. */
export const SHORTWAVE: KernelLayout = {
  name: "shortwave",
  sizes: 169,
  reals: 8,
  imaginaries: 31,
  sphericities: 6,
  angles: 498,
  kFloor: 1e-4,
  kMax: 0.1,
  nMin: 1.37,
  nMax: 1.7,
};

/** The longwave kernel (`tables/LW/`): x ≤ 1,470, n 0.4–3.2, k 10⁻³–4. */
export const LONGWAVE: KernelLayout = {
  name: "longwave",
  sizes: 162,
  reals: 29,
  imaginaries: 19,
  sphericities: 6,
  angles: 498,
  kFloor: 1e-3,
  kMax: 4,
  nMin: 0.4,
  nMax: 3.2,
};

/** The sphericity the tables are made at: the best fit to Martian analogues (Martikainen et al. 2025, §5.1). */
export const SPHERICITY = 0.71;

/** The isca record's fields, in order. */
const ISCA_FIELDS = 9;
const ISCA_BYTES = 4 * ISCA_FIELDS;
/** The phase-matrix elements of a PMat record, in order. */
const PMAT_ELEMENTS = 6;

/**
 * a₁'s angles in TAMUdust2020's phase files: 48 targets evenly spaced in u = √(θ ÷ 180°), the
 * renderer's phase-table abscissa (Design note 6), each snapped to the nearest of the source's 498
 * angles. 48, not more, keeps Mars dust's 13 wavelength nodes under 400 kB.
 */
export const TAMUDUST_PHASE_ANGLES = 48;

/** a₁'s angles in Yang et al.'s phase file: 64, for large crystals' narrow forward peaks. */
export const YANG_PHASE_ANGLES = 64;

/** The other five elements' angles in the phase files, degrees: every 10° from 0° to 180°. */
export const MATRIX_ANGLES_DEG: ReadonlyArray<number> = Array.from(
  { length: 19 },
  (_, i) => 10 * i,
);

/**
 * The sign b₂ takes from the sources' P₄₃: +1, so that b₂ ÷ a₁ has the sign of the laboratory
 * F₃₄ ÷ F₁₁ in T5.b's convention (+0.137 at 90° for MGS-1 M, Martikainen et al. 2024; the
 * hexahedra give +0.199 there, science-r08-nonspherical.md §2.1).
 */
export const B2_SIGN = 1;

/**
 * The cut on a TAMUdust2020 table's maximum dimension at 380 nm, µm: past both kernels' last
 * nodes (x 11,810 is D 714 µm at 380 nm), so every size node is read ({@link kernelReachText}).
 */
export const TAMUDUST_D_MAX_UM = 1000;

/** The quantisation of a₁ in the files: round(1000 ln a₁), a 16-bit integer. */
export const A1_SCALE = 1000;
/** The quantisation of the ratios: round(30,000 × ratio), a 16-bit integer. */
export const RATIO_SCALE = 30_000;

/** The wavelength grid of the material files, nm: 380 to 780 every 5. */
export const MATERIAL_GRID_NM: ReadonlyArray<number> = Array.from(
  { length: 81 },
  (_, i) => 380 + 5 * i,
);

/** The node rule's largest change in n between wavelength nodes, per kernel. */
export const NODE_N_STEP: Readonly<Record<KernelLayout["name"] | "yang", number>> = {
  shortwave: 0.01,
  // The longwave kernel's own n step: nodes closer than the kernel's own linear interpolation in n
  // gain nothing, and the plan's 0.01 would need 56 nodes for iron (n 2.11 to 2.95), its file past
  // the 400 kB cap (0.05 gave 24 nodes; 0.1 gives 12).
  longwave: 0.1,
  yang: 0.01,
};

/** The node rule's largest factor in k between wavelength nodes: the shortwave kernel's own k step, 10^0.1. */
export const NODE_K_FACTOR = 1.26;

// ---------------------------------------------------------------------------------------------
// Reading the kernels.

/** A kernel's records, from files or from a fixture. */
export interface KernelSource {
  readonly layout: KernelLayout;
  /** The 9 isca values of one record. */
  isca(size: number, real: number, imaginary: number, sphericity: number): Float64Array;
  /** C_sca·Pᵢⱼ of one record at the angle indices asked, element-major (6 × indices). */
  pmat(
    size: number,
    real: number,
    imaginary: number,
    sphericity: number,
    angleIndices: ReadonlyArray<number>,
  ): Float64Array;
}

function recordIndex(layout: KernelLayout, s: number, r: number, i: number, p: number): number {
  return ((s * layout.reals + r) * layout.imaginaries + i) * layout.sphericities + p;
}

/**
 * A kernel read from its two binary files: `TAMUdust2020isca.bin` whole (9 or 19 MB), and
 * `TAMUdust2020PMat.bin` one record at a time by offset, so that the 3–6 GB file is never in memory.
 *
 * @throws Error if a file is missing or unreadable, `isca.bin` is not the layout's size, or a
 *   `PMat.bin` record is short.
 */
export function openKernelFiles(
  dir: string,
  layout: KernelLayout,
): KernelSource & { close(): void } {
  const isca = readFileSync(join(dir, "TAMUdust2020isca.bin"));
  const records = layout.sizes * layout.reals * layout.imaginaries * layout.sphericities;
  if (isca.length !== records * ISCA_BYTES) {
    throw new Error(`${dir}: isca.bin has ${isca.length} bytes, not ${records * ISCA_BYTES}`);
  }
  const pmatBytes = 4 * PMAT_ELEMENTS * layout.angles;
  const fd = openSync(join(dir, "TAMUdust2020PMat.bin"), "r");
  const buffer = Buffer.alloc(pmatBytes);
  return {
    layout,
    isca(s, r, i, p) {
      const at = recordIndex(layout, s, r, i, p) * ISCA_BYTES;
      return Float64Array.from({ length: ISCA_FIELDS }, (_, f) => isca.readFloatLE(at + 4 * f));
    },
    pmat(s, r, i, p, angleIndices) {
      const at = recordIndex(layout, s, r, i, p) * pmatBytes;
      const read = readSync(fd, buffer, 0, pmatBytes, at);
      if (read !== pmatBytes) {
        throw new Error(`${dir}: PMat.bin record ${at / pmatBytes} is short`);
      }
      const out = new Float64Array(PMAT_ELEMENTS * angleIndices.length);
      for (let e = 0; e < PMAT_ELEMENTS; e += 1) {
        for (const [j, a] of angleIndices.entries()) {
          out[e * angleIndices.length + j] = buffer.readFloatLE(4 * (e * layout.angles + a));
        }
      }
      return out;
    },
    close() {
      closeSync(fd);
    },
  };
}

/** A kernel's grid: its size parameters, real and imaginary indices and sphericities. */
export interface KernelGrid {
  readonly x: Float64Array;
  readonly n: Float64Array;
  readonly k: Float64Array;
  readonly sphericity: Float64Array;
}

/** The grid of a kernel, read from its isca records. */
export function kernelGrid(source: KernelSource): KernelGrid {
  const { layout } = source;
  return {
    x: Float64Array.from(
      { length: layout.sizes },
      (_, s) => source.isca(s, 0, 0, 0)[0] ?? Number.NaN,
    ),
    n: Float64Array.from(
      { length: layout.reals },
      (_, r) => source.isca(0, r, 0, 0)[6] ?? Number.NaN,
    ),
    k: Float64Array.from(
      { length: layout.imaginaries },
      (_, i) => source.isca(0, 0, i, 0)[7] ?? Number.NaN,
    ),
    sphericity: Float64Array.from(
      { length: layout.sphericities },
      (_, p) => source.isca(0, 0, 0, p)[8] ?? Number.NaN,
    ),
  };
}

/** The bracketing index and weight of a value on an ascending grid, linear in `transform`. */
function bracket(
  grid: Float64Array,
  value: number,
  transform: (v: number) => number,
): { readonly index: number; readonly weight: number } {
  const last = grid.length - 1;
  const lo = grid[0] ?? Number.NaN;
  const hi = grid[last] ?? Number.NaN;
  // The kernels store their grids as float32, so a value at a node may lie a rounding outside.
  const tolerance = 1e-6 * Math.max(Math.abs(lo), Math.abs(hi));
  if (!(value >= lo - tolerance && value <= hi + tolerance)) {
    throw new RangeError(`${value} lies outside the kernel's ${lo}–${hi}`);
  }
  let index = 0;
  while (index < last - 1 && (grid[index + 1] ?? Number.NaN) <= value) {
    index += 1;
  }
  const a = transform(grid[index] ?? Number.NaN);
  const b = transform(grid[index + 1] ?? Number.NaN);
  const weight = Math.min(Math.max((transform(value) - a) / (b - a), 0), 1);
  return { index, weight };
}

/** One size node of a kernel interpolated at an index and sphericity. */
export interface InterpolatedNode {
  readonly x: number;
  /** A, V, C_ext, C_sca, C_sca·g, dimensionless. */
  readonly area: number;
  readonly volume: number;
  readonly extinction: number;
  readonly scattering: number;
  readonly scatteringAsymmetry: number;
  /** C_sca·Pᵢⱼ at the angle indices asked, element-major (P₁₁, P₁₂, P₂₂, P₃₃, P₄₃, P₄₄). */
  readonly matrix: Float64Array;
}

/**
 * A kernel at one index and sphericity, at the size nodes and angle indices asked: each extensive
 * quantity linear in n, ln k and ln S between the kernel's eight neighbouring records.
 *
 * @remarks
 * Linear in n is this tool's choice. The provider's own code reads ln(n − 1) above n 1.1005; at
 * the files' indices the two weights differ by about 0.01, as if n moved by 5 × 10⁻⁴ (R08.T5.c's
 * science check).
 *
 * @throws RangeError for an index or sphericity outside the kernel.
 */
export function interpolateKernel(
  source: KernelSource,
  grid: KernelGrid,
  n: number,
  k: number,
  sphericity: number,
  sizes: ReadonlyArray<number>,
  angleIndices: ReadonlyArray<number>,
): InterpolatedNode[] {
  const re = bracket(grid.n, n, (v) => v);
  const im = bracket(grid.k, k, Math.log);
  const sp = bracket(grid.sphericity, sphericity, Math.log);
  const corners: Array<{ r: number; i: number; p: number; w: number }> = [];
  for (const [dr, wr] of [
    [0, 1 - re.weight],
    [1, re.weight],
  ] as const) {
    for (const [di, wi] of [
      [0, 1 - im.weight],
      [1, im.weight],
    ] as const) {
      for (const [dp, wp] of [
        [0, 1 - sp.weight],
        [1, sp.weight],
      ] as const) {
        const w = wr * wi * wp;
        if (w > 0) {
          corners.push({ r: re.index + dr, i: im.index + di, p: sp.index + dp, w });
        }
      }
    }
  }
  return sizes.map((s) => {
    const isca = new Float64Array(ISCA_FIELDS);
    const matrix = new Float64Array(PMAT_ELEMENTS * angleIndices.length);
    for (const c of corners) {
      const record = source.isca(s, c.r, c.i, c.p);
      for (let f = 0; f < ISCA_FIELDS; f += 1) {
        isca[f] = (isca[f] ?? 0) + c.w * (record[f] ?? 0);
      }
      const values = source.pmat(s, c.r, c.i, c.p, angleIndices);
      for (let j = 0; j < matrix.length; j += 1) {
        matrix[j] = (matrix[j] ?? 0) + c.w * (values[j] ?? 0);
      }
    }
    return {
      x: grid.x[s] ?? Number.NaN,
      area: isca[1] ?? Number.NaN,
      volume: isca[2] ?? Number.NaN,
      extinction: isca[3] ?? Number.NaN,
      scattering: isca[4] ?? Number.NaN,
      scatteringAsymmetry: isca[5] ?? Number.NaN,
      matrix,
    };
  });
}

// ---------------------------------------------------------------------------------------------
// Choices: angles, sizes, wavelength nodes.

/** The index of the source angle nearest each target, without repeats, ascending. */
export function phaseAngleIndices(anglesDeg: ReadonlyArray<number>, count: number): number[] {
  const chosen = new Set<number>();
  for (let i = 0; i < count; i += 1) {
    const u = i / (count - 1);
    const target = 180 * u * u;
    let best = 0;
    for (const [j, a] of anglesDeg.entries()) {
      if (Math.abs(a - target) < Math.abs((anglesDeg[best] ?? Number.NaN) - target)) {
        best = j;
      }
    }
    chosen.add(best);
  }
  return [...chosen].toSorted((a, b) => a - b);
}

/**
 * The indices of `targets` in `anglesDeg`, each exactly present.
 *
 * @throws Error naming an angle the source does not have.
 */
export function exactAngleIndices(
  anglesDeg: ReadonlyArray<number>,
  targets: ReadonlyArray<number>,
): number[] {
  return targets.map((t) => {
    const j = anglesDeg.findIndex((a) => Math.abs(a - t) < 1e-6);
    if (j < 0) {
      throw new Error(`angle ${t}° is not one of the source's`);
    }
    return j;
  });
}

/** Every size node up to and including the first at or past `xMax`, as indices. */
export function sizesUpTo(x: ArrayLike<number>, xMax: number): number[] {
  const out: number[] = [];
  for (let s = 0; s < x.length; s += 1) {
    out.push(s);
    if ((x[s] ?? Number.NaN) >= xMax) {
      break;
    }
  }
  return out;
}

/**
 * The tolerances by which {@link thinSizes} drops a size node: a dropped node must be reproduced,
 * by interpolation between the kept nodes beside it as the renderer interpolates (log-linear in
 * ln r̃ for the cross-sections, V, A and a₁; linear for g and the ratios), to within each.
 */
export const THIN_TOLERANCE = {
  /** a₁ at every phase angle, relative. */
  a1: 0.035,
  /** C_ext and C_sca, relative. */
  crossSection: 0.015,
  /** g and ω, absolute. */
  bulk: 0.004,
  /** The ratios a₂ ÷ a₁ … b₂ ÷ a₁ at every matrix angle, absolute. */
  ratio: 0.03,
} as const;

function albedo(v: NodeSizeValues): number {
  return v.scattering / v.extinction;
}

function asymmetry(v: NodeSizeValues): number {
  return v.scatteringAsymmetry / v.scattering;
}

/**
 * The size nodes kept of a table: from every node, the interior node whose loss costs least is
 * dropped while every node between its kept neighbours is still reproduced within
 * {@link THIN_TOLERANCE}; the first and last are always kept. Returns indices into `values`.
 */
export function thinSizes(values: ReadonlyArray<NodeSizeValues>): number[] {
  const count = values.length;
  const at = (i: number): NodeSizeValues => {
    const v = values[i];
    if (v === undefined) {
      throw new Error(`no size node ${i}`);
    }
    return v;
  };
  const lnR = values.map((v) => Math.log((0.75 * v.volume) / v.area));
  /** The worst normalised error at node m interpolated from nodes a and b. */
  const errorAt = (a: number, b: number, m: number): number => {
    const va = at(a);
    const vb = at(b);
    const vm = at(m);
    const t = ((lnR[m] ?? 0) - (lnR[a] ?? 0)) / ((lnR[b] ?? 0) - (lnR[a] ?? 0));
    const lnLerp = (x: number, y: number): number => Math.log(x) + t * (Math.log(y) - Math.log(x));
    const lerp = (x: number, y: number): number => x + t * (y - x);
    const ext = Math.exp(lnLerp(va.extinction, vb.extinction));
    const sca = Math.exp(lnLerp(va.scattering, vb.scattering));
    let worst = Math.max(
      Math.abs(Math.log(ext / vm.extinction)) / THIN_TOLERANCE.crossSection,
      Math.abs(Math.log(sca / vm.scattering)) / THIN_TOLERANCE.crossSection,
      Math.abs(sca / ext - albedo(vm)) / THIN_TOLERANCE.bulk,
      Math.abs(lerp(asymmetry(va), asymmetry(vb)) - asymmetry(vm)) / THIN_TOLERANCE.bulk,
    );
    for (let j = 0; j < vm.p11.length; j += 1) {
      const interpolated = lnLerp(va.p11[j] ?? Number.NaN, vb.p11[j] ?? Number.NaN);
      worst = Math.max(
        worst,
        Math.abs(interpolated - Math.log(vm.p11[j] ?? Number.NaN)) / Math.log1p(THIN_TOLERANCE.a1),
      );
    }
    for (const e of [0, 1, 2, 3, 4] as const) {
      const ra = va.ratios[e];
      const rb = vb.ratios[e];
      const rm = vm.ratios[e];
      for (let j = 0; j < rm.length; j += 1) {
        worst = Math.max(
          worst,
          Math.abs(lerp(ra[j] ?? Number.NaN, rb[j] ?? Number.NaN) - (rm[j] ?? Number.NaN)) /
            THIN_TOLERANCE.ratio,
        );
      }
    }
    return worst;
  };
  const kept = Array.from({ length: count }, (_, i) => i);
  /** The cost of dropping kept[k]: the worst error of every node between its neighbours. */
  const cost = (k: number): number => {
    const a = kept[k - 1] ?? 0;
    const b = kept[k + 1] ?? 0;
    let worst = 0;
    for (let m = a + 1; m < b; m += 1) {
      worst = Math.max(worst, errorAt(a, b, m));
    }
    return worst;
  };
  const costs = kept.map((_, k) => (k === 0 || k === count - 1 ? Infinity : cost(k)));
  for (;;) {
    let best = -1;
    for (let k = 1; k + 1 < kept.length; k += 1) {
      if (
        (costs[k] ?? Infinity) < 1 &&
        (best < 0 || (costs[k] ?? Infinity) < (costs[best] ?? Infinity))
      ) {
        best = k;
      }
    }
    if (best < 0) {
      return kept;
    }
    kept.splice(best, 1);
    costs.splice(best, 1);
    for (const k of [best - 1, best]) {
      if (k > 0 && k + 1 < kept.length) {
        costs[k] = cost(k);
      }
    }
  }
}

function percent(fraction: number): string {
  return `${Number((100 * fraction).toPrecision(3))}%`;
}

/** The size reach of a table whose largest size node is x, as the phase files' `reduction` states it. */
export function kernelReachText(xMax: number): string {
  const dAt = (nm: number): string =>
    Math.round((xMax * nm) / 1000 / (2 * Math.PI)).toLocaleString("en-US");
  return `every size node of the kernel to its largest, x = ${Math.round(xMax).toLocaleString("en-US")} (D = ${dAt(380)} µm at 380 nm, ${dAt(780)} µm at 780 nm)`;
}

/** The wavelength-node rule for an n step, as the phase files' `reduction` states it. */
export function nodeRuleText(nStep: number): string {
  return `the nodes chosen so that n moves by under ${nStep} and k by under a factor of ${NODE_K_FACTOR} between them, or neighbours on the index file's 5 nm grid where it steps faster`;
}

/**
 * Ice's absorption between nodes, as water ice's `reduction` states it: k's largest value and its
 * largest k·x at the archive's largest crystal, where weak absorption is linear in k.
 */
export function iceAbsorptionText(ice: Pick<MaterialIndexFile, "wavelengthsNm" | "k">): string {
  let kMax = 0;
  let kxMax = 0;
  for (const [i, nm] of ice.wavelengthsNm.entries()) {
    const k = ice.k[i] ?? Number.NaN;
    kMax = Math.max(kMax, k);
    kxMax = Math.max(kxMax, (k * 2 * Math.PI * YANG_D_MAX_UM * 1000) / nm);
  }
  return `k, at most ${scientific(kMax)}, keeps k·x under ${kxMax.toPrecision(2)} at D ≤ ${YANG_D_MAX_UM / 10_000} cm, where absorption is linear in k, so the renderer scales it by k between nodes`;
}

const SUPERSCRIPTS: Readonly<Record<string, string>> = {
  "-": "⁻",
  "0": "⁰",
  "1": "¹",
  "2": "²",
  "3": "³",
  "4": "⁴",
  "5": "⁵",
  "6": "⁶",
  "7": "⁷",
  "8": "⁸",
  "9": "⁹",
};

/** A value as m × 10ⁿ, m to two decimals. */
function scientific(value: number): string {
  const [mantissa = "", exponent = ""] = value.toExponential(2).split("e");
  const power = exponent
    .replace("+", "")
    .split("")
    .map((c) => SUPERSCRIPTS[c] ?? c)
    .join("");
  return `${mantissa} × 10${power}`;
}

/** {@link THIN_TOLERANCE} as the phase files' `reduction` states it. */
export function thinningText(): string {
  const t = THIN_TOLERANCE;
  return `thinned where interpolation between the kept nodes reproduces each dropped one to ${percent(t.a1)} in a₁, ${percent(t.crossSection)} in the cross-sections, ${t.bulk} in g and ω and ${t.ratio} in the ratios`;
}

/** A material file's index on its grid, as the renderer's files hold it. */
export interface MaterialIndexFile {
  readonly key: string;
  readonly phase: string;
  readonly variant: string | null;
  readonly name: string;
  readonly wavelengthsNm: ReadonlyArray<number>;
  readonly n: ReadonlyArray<number>;
  readonly k: ReadonlyArray<number>;
}

/**
 * The wavelength nodes of a material: 380 nm, then each next grid wavelength as far as n stays
 * within `nStep` and k (taken at no less than `kFloor`) within {@link NODE_K_FACTOR} of the last
 * node's, and 780 nm. Returns grid indices.
 */
export function wavelengthNodes(
  file: Pick<MaterialIndexFile, "n" | "k">,
  nStep: number,
  kFloor: number,
): number[] {
  const nodes = [0];
  const last = file.n.length - 1;
  let from = 0;
  const kAt = (i: number): number => Math.max(file.k[i] ?? 0, kFloor);
  for (let i = 1; i <= last; i += 1) {
    const nMoved = Math.abs((file.n[i] ?? Number.NaN) - (file.n[from] ?? Number.NaN));
    const kRatio = Math.abs(Math.log(kAt(i) / kAt(from)));
    if (nMoved >= nStep || kRatio >= Math.log(NODE_K_FACTOR)) {
      // The previous grid point is the last one inside the rule.
      const node = Math.max(i - 1, from + 1);
      nodes.push(node);
      from = node;
      i = node;
    }
  }
  if (nodes[nodes.length - 1] !== last) {
    nodes.push(last);
  }
  return nodes;
}

/**
 * The kernel that covers an index, and the k it is read at (the floor below the kernel's range).
 *
 * @throws RangeError for an index outside both kernels.
 */
export function kernelFor(
  n: number,
  k: number,
): { readonly layout: KernelLayout; readonly kTable: number } {
  for (const layout of [SHORTWAVE, LONGWAVE]) {
    if (n >= layout.nMin && n <= layout.nMax && k <= layout.kMax) {
      return { layout, kTable: Math.max(k, layout.kFloor) };
    }
  }
  throw new RangeError(`index ${n} + ${k}i lies outside both kernels`);
}

// ---------------------------------------------------------------------------------------------
// The phase files.

/** One wavelength node of a phase file. */
export interface PhaseNode {
  readonly wavelengthNm: number;
  /** The material's own index at the node. */
  readonly n: number;
  readonly k: number;
  /** The kernel read, or `null` for Yang et al.'s table. */
  readonly kernel: KernelLayout["name"] | null;
  /** The k the table was made at: the material's, or the kernel's floor beneath it. */
  readonly kTable: number;
  /** The size parameters x = 2πD ÷ λ of the size nodes. */
  readonly sizeParameters: ReadonlyArray<number>;
  /** V, A, C_ext, C_sca and C_sca·g per size node, dimensionless (powers of 2π ÷ λ). */
  readonly volumes: ReadonlyArray<number>;
  readonly areas: ReadonlyArray<number>;
  readonly extinction: ReadonlyArray<number>;
  readonly scattering: ReadonlyArray<number>;
  readonly scatteringAsymmetry: ReadonlyArray<number>;
  /** round(1000 ln a₁), size-major over `phaseAnglesDeg`, as base64 little-endian int16. */
  readonly a1: string;
  /** round(30,000 × element ÷ a₁), size-major over `matrixAnglesDeg`, as base64 int16. */
  readonly a2: string;
  readonly a3: string;
  readonly a4: string;
  readonly b1: string;
  readonly b2: string;
}

/** A phase file: its header, its angles and its wavelength nodes. */
export interface PhaseFile {
  /** The material index file the table is made for, and its key, phase and variant. */
  readonly material: string;
  readonly key: string;
  readonly phase: string;
  readonly variant: string | null;
  readonly model: "tamudust2020" | "yang2013";
  /** The particle model: the ensemble and its sphericity, or the habit and its roughness. */
  readonly particles: string;
  readonly sphericity: number | null;
  readonly paper: string;
  /** The archive, its DOI, MD5, SHA-256 and the date fetched. */
  readonly source: string;
  readonly licence: string;
  /** How the values were reduced, and the changes made to the source. */
  readonly reduction: string;
  readonly encoding: string;
  readonly phaseAnglesDeg: ReadonlyArray<number>;
  readonly matrixAnglesDeg: ReadonlyArray<number>;
  readonly nodes: ReadonlyArray<PhaseNode>;
}

function roundTo(value: number, figures: number): number {
  return Number(value.toPrecision(figures));
}

function int16Base64(values: ArrayLike<number>): string {
  const buffer = Buffer.alloc(2 * values.length);
  for (let i = 0; i < values.length; i += 1) {
    const v = Math.round(values[i] ?? Number.NaN);
    if (!(v >= -32_768 && v <= 32_767)) {
      throw new RangeError(`quantised value ${v} does not fit 16 bits`);
    }
    buffer.writeInt16LE(v, 2 * i);
  }
  return buffer.toString("base64");
}

/** The inverse of the files' int16 base64. */
export function decodeInt16(text: string): Int16Array {
  const buffer = Buffer.from(text, "base64");
  return Int16Array.from({ length: buffer.length / 2 }, (_, i) => buffer.readInt16LE(2 * i));
}

/** One size node's values for a phase node, before encoding. */
export interface NodeSizeValues {
  readonly x: number;
  readonly volume: number;
  readonly area: number;
  readonly extinction: number;
  readonly scattering: number;
  readonly scatteringAsymmetry: number;
  /** P₁₁ at the phase angles (∫ P₁₁ dΩ = 4π). */
  readonly p11: ArrayLike<number>;
  /** P₂₂ ÷ P₁₁, P₃₃ ÷ P₁₁, P₄₄ ÷ P₁₁, P₁₂ ÷ P₁₁ and P₄₃ ÷ P₁₁ at the matrix angles. */
  readonly ratios: readonly [
    ArrayLike<number>,
    ArrayLike<number>,
    ArrayLike<number>,
    ArrayLike<number>,
    ArrayLike<number>,
  ];
}

/**
 * Encodes a node's size values into its {@link PhaseNode}.
 *
 * @param crossSectionFigures - The significant figures C_ext and C_sca keep, so that their
 *   difference, the absorption, keeps its own: six for the kernels' float32, nine for ice.
 */
export function encodeNode(
  head: Pick<PhaseNode, "wavelengthNm" | "n" | "k" | "kernel" | "kTable">,
  sizes: ReadonlyArray<NodeSizeValues>,
  crossSectionFigures = 6,
): PhaseNode {
  const flat = (pick: (s: NodeSizeValues) => ArrayLike<number>, scale: number, log: boolean) => {
    const out: number[] = [];
    for (const s of sizes) {
      for (const v of Array.from(pick(s))) {
        out.push(scale * (log ? Math.log(v) : v));
      }
    }
    return int16Base64(out);
  };
  return {
    ...head,
    sizeParameters: sizes.map((s) => roundTo(s.x, 7)),
    volumes: sizes.map((s) => roundTo(s.volume, 6)),
    areas: sizes.map((s) => roundTo(s.area, 6)),
    extinction: sizes.map((s) => roundTo(s.extinction, crossSectionFigures)),
    scattering: sizes.map((s) => roundTo(s.scattering, crossSectionFigures)),
    scatteringAsymmetry: sizes.map((s) => roundTo(s.scatteringAsymmetry, 6)),
    a1: flat((s) => s.p11, A1_SCALE, true),
    a2: flat((s) => s.ratios[0], RATIO_SCALE, false),
    a3: flat((s) => s.ratios[1], RATIO_SCALE, false),
    a4: flat((s) => s.ratios[2], RATIO_SCALE, false),
    b1: flat((s) => s.ratios[3], RATIO_SCALE, false),
    b2: flat((s) => Array.from(s.ratios[4], (v) => B2_SIGN * v), RATIO_SCALE, false),
  };
}

/**
 * A kernel's interpolated size nodes as a node's size values: P₁₁ = C_sca·P₁₁ ÷ C_sca at the phase
 * angles, and the ratios at the matrix angles, the two angle sets read at once, the phase angles
 * first (`phaseCount` of them), then the matrix angles (`matrixCount`).
 */
export function kernelSizeValues(
  nodes: ReadonlyArray<InterpolatedNode>,
  phaseCount: number,
  matrixCount: number,
): NodeSizeValues[] {
  const width = phaseCount + matrixCount;
  return nodes.map((node) => {
    const element = (e: number, j: number): number => node.matrix[e * width + j] ?? Number.NaN;
    const p11 = Float64Array.from(
      { length: phaseCount },
      (_, j) => element(0, j) / node.scattering,
    );
    const ratio = (e: number): Float64Array =>
      Float64Array.from(
        { length: matrixCount },
        (_, j) => element(e, phaseCount + j) / element(0, phaseCount + j),
      );
    return {
      x: node.x,
      volume: node.volume,
      area: node.area,
      extinction: node.extinction,
      scattering: node.scattering,
      scatteringAsymmetry: node.scatteringAsymmetry,
      p11,
      // The PMat order is P11, P12, P22, P33, P43, P44.
      ratios: [ratio(2), ratio(3), ratio(5), ratio(1), ratio(4)],
    };
  });
}

/** A material reduced from the kernels. */
export interface TamudustMaterial {
  /** The material index file's name in `view/atmosphere/materials/`. */
  readonly file: string;
}

/** The materials T5.c's kernels cover: Mars dust, the two silicates and solid iron. */
export const TAMUDUST_MATERIALS: ReadonlyArray<TamudustMaterial> = [
  { file: "mars-dust.json" },
  { file: "enstatite-glass.json" },
  { file: "forsterite-amorphous.json" },
  { file: "iron.json" },
];

/** The fetched archives' identities, recorded at the fetch. */
export const TAMUDUST_ARCHIVE = {
  name: "TAMUdust2020-DataKernels-v1.0-20210422.tar.gz",
  url: "https://zenodo.org/api/records/4711247/files/TAMUdust2020-DataKernels-v1.0-20210422.tar.gz/content",
  bytes: 7_939_243_504,
  md5: "ff4abf3ee2919494d47679ffc4cba817",
  sha256: "eb4f308d8623cebe1bb9ac318332fbcdc754071ce610ad3719ec500a11d6e842",
  fetched: "2026-10-10",
} as const;

/** The source archive, read for the 498 angles and the record layout only. */
export const TAMUDUST_SOURCE_ARCHIVE = {
  name: "TAMUdust2020-SourceCodes-v1.0-20210422.tar.gz",
  md5: "dba7d1664ff5c0ee27192bd267bda8f3",
  sha256: "14fa98464de0606def894bb3d6c706ee8d08b6bfcbe67582804bf49004afba0a",
} as const;

const TAMUDUST_PAPER =
  'M. Saito, P. Yang, J. Ding and X. Liu, "A comprehensive database of the optical properties of irregular aerosol particles for radiative transfer simulations", J. Atmos. Sci. 78 (2021) 2089–2111, DOI 10.1175/JAS-D-20-0338.1; the sphericity from J. Martikainen et al., Mon. Not. R. Astron. Soc. 537 (2025) 1489–1503, DOI 10.1093/mnras/staf108';

const TAMUDUST_LICENCE =
  "Adapted from CC BY 4.0 data (https://creativecommons.org/licenses/by/4.0/): M. Saito, TAMUdust2020 Database, version 1.0.0, Zenodo, DOI 10.5281/zenodo.4711247. The scattering properties are obtained from TAMUdust2020. Changed: interpolated to this material's refractive index and to sphericity 0.71, resampled in size and angle, quantised; this table is offered under CC BY 4.0";

const ENCODING =
  "a1: round(1000 ln a₁) per size node and phase angle, size-major; a2, a3, a4, b1, b2: round(30000 × element ÷ a₁) per size node and matrix angle; each as base64 of little-endian 16-bit integers. a₁ is normalised to ∫ a₁ dΩ = 4π; b₂ = +P₄₃ of the source. Cross-sections, V and A are in units of (λ ÷ 2π)² and (λ ÷ 2π)³ at the node's wavelength, and x = 2πD ÷ λ, D the maximum dimension";

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Reads a material index file.
 *
 * @throws Error if the file is unreadable, not JSON, or lacks a field the reduction reads.
 */
export function readMaterialFile(dir: string, file: string): MaterialIndexFile {
  const raw: unknown = JSON.parse(readFileSync(join(dir, file), "utf8"));
  if (!isRecord(raw)) {
    throw new Error(`${file}: not a JSON object`);
  }
  const get = (field: string): unknown => raw[field];
  const numbers = (field: string): number[] => {
    const v = get(field);
    if (!Array.isArray(v) || !v.every((x) => typeof x === "number")) {
      throw new Error(`${file}: "${field}" must be an array of numbers`);
    }
    return v.map(Number);
  };
  const text = (field: string): string => {
    const v = get(field);
    if (typeof v !== "string") {
      throw new Error(`${file}: "${field}" must be a string`);
    }
    return v;
  };
  const variant = get("variant");
  return {
    key: text("key"),
    phase: text("phase"),
    variant: typeof variant === "string" ? variant : null,
    name: text("name"),
    wavelengthsNm: numbers("wavelengthsNm"),
    n: numbers("n"),
    k: numbers("k"),
  };
}

/**
 * Reduces one material from the kernels.
 *
 * @param only - Indices on {@link MATERIAL_GRID_NM} to make nodes at, in place of the node rule:
 *   the one-node test fixtures'.
 * @param sphericity - The sphericity to interpolate the kernel at, within its grid (0.695–0.785);
 *   {@link SPHERICITY} for every committed file, another value only for a diagnostic.
 * @throws RangeError for an index or sphericity outside the kernels, as {@link interpolateKernel}.
 */
export function reduceTamudust(
  material: MaterialIndexFile,
  file: string,
  kernels: Readonly<Record<KernelLayout["name"], KernelSource>>,
  anglesDeg: ReadonlyArray<number>,
  only?: ReadonlyArray<number>,
  sphericity: number = SPHERICITY,
): PhaseFile {
  const phaseIndices = phaseAngleIndices(anglesDeg, TAMUDUST_PHASE_ANGLES);
  const matrixIndices = exactAngleIndices(anglesDeg, MATRIX_ANGLES_DEG);
  const angleIndices = [...phaseIndices, ...matrixIndices];
  const first = kernelFor(material.n[0] ?? Number.NaN, material.k[0] ?? Number.NaN);
  const nodes =
    only ?? wavelengthNodes(material, NODE_N_STEP[first.layout.name], first.layout.kFloor);
  const grids: Partial<Record<KernelLayout["name"], KernelGrid>> = {};
  const phaseNodes = nodes.map((g) => {
    const wavelengthNm = material.wavelengthsNm[g] ?? Number.NaN;
    const n = material.n[g] ?? Number.NaN;
    const k = material.k[g] ?? Number.NaN;
    const { layout, kTable } = kernelFor(n, k);
    const source = kernels[layout.name];
    const grid = (grids[layout.name] ??= kernelGrid(source));
    const xMax = (2 * Math.PI * TAMUDUST_D_MAX_UM) / 0.38;
    const every = sizesUpTo(grid.x, xMax);
    const interpolated = interpolateKernel(
      source,
      grid,
      n,
      kTable,
      sphericity,
      every,
      angleIndices,
    );
    const values = kernelSizeValues(interpolated, phaseIndices.length, matrixIndices.length);
    const kept = thinSizes(values)
      .map((i) => values[i])
      .filter((v) => v !== undefined);
    return encodeNode(
      { wavelengthNm, n, k, kernel: layout.name, kTable: roundTo(kTable, 6) },
      kept,
    );
  });
  const kernelsUsed = [...new Set(phaseNodes.map((p) => p.kernel))].join(" and ");
  const floored = phaseNodes.some((p) => p.kTable > p.k);
  return {
    material: file,
    key: material.key,
    phase: material.phase,
    variant: material.variant,
    model: "tamudust2020",
    particles: `TAMUdust2020's ensemble of 20 irregular hexahedra, sphericity ${sphericity}`,
    sphericity,
    paper: TAMUDUST_PAPER,
    source: `${TAMUDUST_ARCHIVE.name}, ${TAMUDUST_ARCHIVE.url}, ${TAMUDUST_ARCHIVE.bytes} bytes, MD5 ${TAMUDUST_ARCHIVE.md5}, SHA-256 ${TAMUDUST_ARCHIVE.sha256}, fetched ${TAMUDUST_ARCHIVE.fetched}; the angles from ${TAMUDUST_SOURCE_ARCHIVE.name}, MD5 ${TAMUDUST_SOURCE_ARCHIVE.md5}`,
    licence: TAMUDUST_LICENCE,
    reduction: `The ${kernelsUsed} kernel at this material's index (${file}) at each wavelength node, ${nodeRuleText(NODE_N_STEP[first.layout.name])}; each extensive quantity linear in n, ln k and ln S between the kernel's records${sphericity === SPHERICITY ? "" : `, at sphericity ${sphericity}`}; ${kernelReachText(Math.max(...phaseNodes.map((p) => p.sizeParameters.at(-1) ?? 0)))}, ${thinningText()}; a₁ at ${phaseIndices.length} of the kernel's 498 angles, nearest to even steps in √θ, and the other elements every 10°.${floored ? ` Where k lies below the kernel's floor, the table is the floor's (kTable), and the renderer scales its absorption by k ÷ kTable (the floor rule).` : ""}`,
    encoding: ENCODING,
    phaseAnglesDeg: phaseIndices.map((j) => anglesDeg[j] ?? Number.NaN),
    matrixAnglesDeg: MATRIX_ANGLES_DEG,
    nodes: phaseNodes,
  };
}

/**
 * The fidelity test's sample: MGS-1 M, n 1.50 and k 4.33 × 10⁻⁴ at 650 nm (Martikainen et al.
 * 2025's computed database, `opticalPropertiesMGS1M_1.txt`), a test fixture with one node.
 */
export const MGS1M: MaterialIndexFile = {
  key: "MGS-1 M",
  phase: "solid",
  variant: null,
  name: "MGS-1 M, Martikainen et al. 2025's Mars global simulant, M size fraction (a test sample)",
  wavelengthsNm: MATERIAL_GRID_NM,
  n: MATERIAL_GRID_NM.map(() => 1.5),
  k: MATERIAL_GRID_NM.map(() => 4.33e-4),
};

/** The grid index of 650 nm, the fixture's one node. */
export const MGS1M_NODE = 54;

/**
 * The enstatite test's sample, Frattin et al. 2019's Mg₀.₈₅Fe₀.₀₈Si₀.₉₉O₃ (their §4.1.2), at its
 * own iron: Dorschner et al. 1995's pyroxene glasses, linear in n and log-linear in k between
 * Mg₀.₉₅ (1.588 + 2.9 × 10⁻⁴i) and Mg₀.₈₀ (1.612 + 1.7 × 10⁻³i) at 520 nm, 1.594 + 4.5 × 10⁻⁴i
 * (science-r08-nonspherical.md §7.1). A test fixture with one node; the MgSiO₃ file stays
 * iron-free.
 */
export const ENSTATITE_SAMPLE: MaterialIndexFile = {
  key: "enstatite (Frattin et al. 2019)",
  phase: "solid",
  variant: null,
  name: "Frattin et al. 2019's enstatite, Mg0.85Fe0.08Si0.99O3, at its iron (a test sample)",
  wavelengthsNm: MATERIAL_GRID_NM,
  n: MATERIAL_GRID_NM.map(() => 1.594),
  k: MATERIAL_GRID_NM.map(() => 4.5e-4),
};

/** The grid index of 520 nm, the enstatite fixture's one node. */
export const ENSTATITE_NODE = 28;

// ---------------------------------------------------------------------------------------------
// Yang et al. 2013's ice.

/** The ice archive's identity, recorded at the fetch. */
export const YANG_ARCHIVE = {
  name: "Data_0.2_15.25.tar.gz",
  url: "https://zenodo.org/api/records/5348402/files/Data_0.2_15.25.tar.gz/content",
  bytes: 27_420_579_422,
  md5: "2fb9bbab2c2c735a869c863a680e2f70",
  sha256: "9c8eb5d162f48392705588443d6251cf6557463f51b10f67cf544ef0477f8ab0",
  fetched: "2026-10-10",
} as const;

/** One row of Yang et al.'s `isca.dat`. */
export interface YangIscaRow {
  readonly wavelengthUm: number;
  readonly maximumDimensionUm: number;
  readonly volumeUm3: number;
  readonly areaUm2: number;
  readonly extinctionEfficiency: number;
  readonly singleScatteringAlbedo: number;
  readonly asymmetry: number;
}

/** Parses Yang et al.'s `isca.dat`: seven columns per line. */
export function parseYangIsca(text: string): YangIscaRow[] {
  const rows: YangIscaRow[] = [];
  for (const line of text.split("\n")) {
    const f = line.trim().split(/\s+/u).map(Number);
    if (f.length < 7 || f.some((v) => !Number.isFinite(v))) {
      continue;
    }
    rows.push({
      wavelengthUm: f[0] ?? Number.NaN,
      maximumDimensionUm: f[1] ?? Number.NaN,
      volumeUm3: f[2] ?? Number.NaN,
      areaUm2: f[3] ?? Number.NaN,
      extinctionEfficiency: f[4] ?? Number.NaN,
      singleScatteringAlbedo: f[5] ?? Number.NaN,
      asymmetry: f[6] ?? Number.NaN,
    });
  }
  return rows;
}

/**
 * The lines asked of one of Yang et al.'s element files (`P11.dat` …), streamed: the first line's
 * angles, and each asked data line (0-based after the angle line) as its numbers.
 */
export async function readYangElement(
  path: string,
  wanted: ReadonlySet<number>,
): Promise<{ readonly angles: number[]; readonly rows: Map<number, number[]> }> {
  const lines = createInterface({ input: createReadStream(path), crlfDelay: Infinity });
  let angles: number[] = [];
  const rows = new Map<number, number[]>();
  let index = -1;
  for await (const line of lines) {
    if (index === -1) {
      angles = line.trim().split(/\s+/u).map(Number);
    } else if (wanted.has(index)) {
      rows.set(index, line.trim().split(/\s+/u).map(Number));
    }
    index += 1;
  }
  return { angles, rows };
}

/** The wavelength nodes of Yang et al.'s table: its own wavelengths in 380–780 nm, by the n rule. */
export function yangWavelengthNodes(
  wavelengthsUm: ReadonlyArray<number>,
  ice: MaterialIndexFile,
): number[] {
  // The archive's wavelengths from the last at or below 380 nm to the first at or above 780 nm, so
  // that the nodes bracket the material grid.
  const below = Math.max(...wavelengthsUm.filter((w) => w <= 0.38 + 1e-9));
  const above = Math.min(...wavelengthsUm.filter((w) => w >= 0.78 - 1e-9));
  const inRange = wavelengthsUm
    .map((w, i) => ({ w, i }))
    .filter(({ w }) => w >= below - 1e-9 && w <= above + 1e-9);
  const nAt = (wUm: number): number => {
    const nm = 1000 * wUm;
    const g = Math.min(Math.max(Math.round((nm - 380) / 5), 0), 80);
    return ice.n[g] ?? Number.NaN;
  };
  const chosen: number[] = [];
  let from: number | undefined;
  for (const [j, { w, i }] of inRange.entries()) {
    if (from === undefined) {
      chosen.push(i);
      from = j;
      continue;
    }
    const start = inRange[from]?.w ?? Number.NaN;
    if (Math.abs(nAt(w) - nAt(start)) >= NODE_N_STEP.yang) {
      const prev = inRange[j - 1];
      if (prev !== undefined && prev.i !== chosen[chosen.length - 1]) {
        chosen.push(prev.i);
        from = j - 1;
      }
    }
  }
  const last = inRange[inRange.length - 1];
  if (last !== undefined && chosen[chosen.length - 1] !== last.i) {
    chosen.push(last.i);
  }
  return chosen;
}

const YANG_PAPER =
  'P. Yang, L. Bi, B. A. Baum, K.-N. Liou, G. W. Kattawar, M. I. Mishchenko and B. Cole, "Spectrally consistent scattering, absorption, and polarization properties of atmospheric ice crystals at wavelengths from 0.2 to 100 µm", J. Atmos. Sci. 70 (2013) 330–347, DOI 10.1175/JAS-D-12-039.1, in its version 2 (L. Bi and P. Yang, J. Quant. Spectrosc. Radiat. Transfer 189 (2017) 228–237)';

const YANG_LICENCE =
  "Adapted from CC BY 4.0 data (https://creativecommons.org/licenses/by/4.0/): Yang et al. 2013 version 2, Zenodo record 5348402. Changed: the 8-column aggregate, severely roughened, at the archive's own wavelengths in 380–780 nm, resampled in size and angle, quantised; this table is offered under CC BY 4.0";

/** The largest maximum dimension kept of Yang et al.'s 189, µm: all of them. */
export const YANG_D_MAX_UM = 10_000;

/** Reduces Yang et al.'s `8_columns/Rough050/` to water ice's phase file. */
export async function reduceYang(
  dir: string,
  ice: MaterialIndexFile,
  file: string,
): Promise<PhaseFile> {
  const isca = parseYangIsca(readFileSync(join(dir, "isca.dat"), "utf8"));
  const wavelengths = [...new Set(isca.map((r) => r.wavelengthUm))];
  const dims = [...new Set(isca.map((r) => r.maximumDimensionUm))];
  if (wavelengths.length * dims.length !== isca.length) {
    throw new Error(`isca.dat: ${isca.length} rows are not ${wavelengths.length} × ${dims.length}`);
  }
  const nodes = yangWavelengthNodes(wavelengths, ice);
  const sizes = sizesUpTo(dims, YANG_D_MAX_UM);
  const wanted = new Set<number>();
  for (const j of nodes) {
    for (const s of sizes) {
      wanted.add(j * dims.length + s);
    }
  }
  const elements = ["P11", "P12", "P22", "P33", "P43", "P44"] as const;
  const files = await Promise.all(
    elements.map((e) => readYangElement(join(dir, `${e}.dat`), wanted)),
  );
  const read: Record<string, Map<number, number[]>> = {};
  for (const [i, e] of elements.entries()) {
    read[e] = files[i]?.rows ?? new Map<number, number[]>();
  }
  const angles = files[0]?.angles ?? [];
  const phaseIndices = phaseAngleIndices(angles, YANG_PHASE_ANGLES);
  const matrixIndices = exactAngleIndices(angles, MATRIX_ANGLES_DEG);
  const phaseNodes = nodes.map((j) => {
    const wavelengthUm = wavelengths[j] ?? Number.NaN;
    const scale = (2 * Math.PI) / wavelengthUm;
    const g = Math.min(Math.max(Math.round((1000 * wavelengthUm - 380) / 5), 0), 80);
    const values: NodeSizeValues[] = sizes.map((s) => {
      const row = isca[j * dims.length + s];
      const line = j * dims.length + s;
      const pick = (e: (typeof elements)[number], indices: ReadonlyArray<number>): Float64Array => {
        const r = read[e]?.get(line);
        if (r === undefined) {
          throw new Error(`${e}.dat lacks line ${line}`);
        }
        return Float64Array.from(indices, (i) => r[i] ?? Number.NaN);
      };
      if (row === undefined) {
        throw new Error(`isca.dat lacks row ${line}`);
      }
      const area = row.areaUm2 * scale * scale;
      const extinction = row.extinctionEfficiency * area;
      const scattering = row.singleScatteringAlbedo * extinction;
      return {
        x: scale * row.maximumDimensionUm,
        volume: row.volumeUm3 * scale ** 3,
        area,
        extinction,
        scattering,
        scatteringAsymmetry: scattering * row.asymmetry,
        p11: pick("P11", phaseIndices),
        ratios: [
          pick("P22", matrixIndices),
          pick("P33", matrixIndices),
          pick("P44", matrixIndices),
          pick("P12", matrixIndices),
          pick("P43", matrixIndices),
        ],
      };
    });
    return encodeNode(
      {
        wavelengthNm: roundTo(1000 * wavelengthUm, 6),
        n: ice.n[g] ?? Number.NaN,
        k: ice.k[g] ?? Number.NaN,
        kernel: null,
        kTable: ice.k[g] ?? Number.NaN,
      },
      thinSizes(values)
        .map((i) => values[i])
        .filter((v) => v !== undefined),
      // Ice's 1 − ω is as small as 10⁻⁶, so its absorption C_ext − C_sca needs nine figures.
      9,
    );
  });
  return {
    material: file,
    key: ice.key,
    phase: ice.phase,
    variant: ice.variant,
    model: "yang2013",
    particles:
      "Yang et al. 2013's aggregate of eight hexagonal columns, severely roughened (σ = 0.5), version 2",
    sphericity: null,
    paper: YANG_PAPER,
    source: `${YANG_ARCHIVE.name}, ${YANG_ARCHIVE.url}, ${YANG_ARCHIVE.bytes} bytes, MD5 ${YANG_ARCHIVE.md5}, SHA-256 ${YANG_ARCHIVE.sha256}, fetched ${YANG_ARCHIVE.fetched}; only 8_columns/Rough050/ extracted`,
    licence: YANG_LICENCE,
    reduction: `The archive's wavelengths in 380–780 nm, the nodes chosen so that water ice's n moves by under ${NODE_N_STEP.yang} between them (${iceAbsorptionText(ice)}); its 189 maximum dimensions by the kernels' thinning, which keeps them all (the forward peak narrows past interpolation at every step); a₁ at ${phaseIndices.length} of its 498 angles, nearest to even steps in √θ, and the other elements every 10°. The archive's index is Warren and Brandt 2008's, as water-ice.json`,
    encoding: ENCODING,
    phaseAnglesDeg: phaseIndices.map((j) => angles[j] ?? Number.NaN),
    matrixAnglesDeg: MATRIX_ANGLES_DEG,
    nodes: phaseNodes,
  };
}

// ---------------------------------------------------------------------------------------------
// The excerpt, for the tests.

/**
 * A raw excerpt of a kernel for the tests: every size node's isca record, and C_sca·P₁₁ at the
 * phase angles, at one kernel index node and sphericity node, for each k listed.
 */
export interface KernelExcerpt {
  readonly source: string;
  readonly kernel: KernelLayout["name"];
  readonly n: number;
  readonly sphericity: number;
  readonly phaseAnglesDeg: ReadonlyArray<number>;
  readonly records: ReadonlyArray<{
    readonly k: number;
    /** Per size node: x, A, V, C_ext, C_sca, C_sca·g. */
    readonly isca: ReadonlyArray<ReadonlyArray<number>>;
    /** round(1000 ln P₁₁), size-major over the phase angles, base64 int16. */
    readonly a1: string;
  }>;
}

/** The excerpt at one (n, S) node and the k nodes listed. */
export function kernelExcerpt(
  source: KernelSource,
  real: number,
  imaginaries: ReadonlyArray<number>,
  sphericity: number,
  anglesDeg: ReadonlyArray<number>,
): KernelExcerpt {
  const grid = kernelGrid(source);
  const phaseIndices = phaseAngleIndices(anglesDeg, TAMUDUST_PHASE_ANGLES);
  return {
    source: `${TAMUDUST_ARCHIVE.name}, MD5 ${TAMUDUST_ARCHIVE.md5}: raw records, CC BY 4.0 (M. Saito, TAMUdust2020 Database, version 1.0.0, Zenodo, DOI 10.5281/zenodo.4711247; the scattering properties are obtained from TAMUdust2020), P₁₁ quantised as the phase files' a1`,
    kernel: source.layout.name,
    n: roundTo(grid.n[real] ?? Number.NaN, 7),
    sphericity: roundTo(grid.sphericity[sphericity] ?? Number.NaN, 7),
    phaseAnglesDeg: phaseIndices.map((j) => anglesDeg[j] ?? Number.NaN),
    records: imaginaries.map((i) => {
      const isca: number[][] = [];
      const a1: number[] = [];
      for (let s = 0; s < source.layout.sizes; s += 1) {
        const r = source.isca(s, real, i, sphericity);
        isca.push(Array.from(r.subarray(0, 6), (v) => roundTo(v, 7)));
        const p = source.pmat(s, real, i, sphericity, phaseIndices);
        const csca = r[4] ?? Number.NaN;
        for (let j = 0; j < phaseIndices.length; j += 1) {
          a1.push(A1_SCALE * Math.log((p[j] ?? Number.NaN) / csca));
        }
      }
      return { k: roundTo(grid.k[i] ?? Number.NaN, 7), isca, a1: int16Base64(a1) };
    }),
  };
}

// ---------------------------------------------------------------------------------------------
// The tool's own integral over a table, for its test of the reduction.

/** A gamma distribution's number density in r, unnormalised: r^((1 − 3b) ÷ b) e^(−r ÷ ab). */
export function gammaNumberDensity(
  effectiveRadiusUm: number,
  effectiveVariance: number,
): (rUm: number) => number {
  const a = effectiveRadiusUm;
  const b = effectiveVariance;
  return (rUm: number): number => rUm ** ((1 - 3 * b) / b) * Math.exp(-rUm / (a * b));
}

/** A size table at one wavelength: per node, r_VA (µm), C_ext, C_sca, C_sca·g (µm²) and P₁₁ at angles. */
export interface SizeTable {
  readonly radiiUm: ReadonlyArray<number>;
  readonly extinctionUm2: ReadonlyArray<number>;
  readonly scatteringUm2: ReadonlyArray<number>;
  readonly scatteringAsymmetryUm2: ReadonlyArray<number>;
  readonly p11: ReadonlyArray<ArrayLike<number>>;
}

/** The 8-point Gauss–Legendre rule on [−1, 1], as (node, weight). */
const GL8 = [
  [-0.960_289_856_497_536_2, 0.101_228_536_290_376_3],
  [-0.796_666_477_413_626_7, 0.222_381_034_453_374_5],
  [-0.525_532_409_916_329, 0.313_706_645_877_887_3],
  [-0.183_434_642_495_649_8, 0.362_683_783_378_362],
  [0.183_434_642_495_649_8, 0.362_683_783_378_362],
  [0.525_532_409_916_329, 0.313_706_645_877_887_3],
  [0.796_666_477_413_626_7, 0.222_381_034_453_374_5],
  [0.960_289_856_497_536_2, 0.101_228_536_290_376_3],
] as const;

/** Log-linear interpolation between two positive values. */
function logLerp(a: number, b: number, t: number): number {
  return Math.exp(Math.log(a) + t * (Math.log(b) - Math.log(a)));
}

/** The widest sub-panel of the size integral in ln r, so a narrow distribution is resolved. */
const SUB_PANEL_LN = 0.05;

/**
 * ∫ n(r) Q dr over a size table, between its first and last radii: the cross-sections and P₁₁
 * log-linear and g linear in ln r between nodes, by 8-point Gauss–Legendre on sub-panels no wider
 * than 0.05 in ln r. Returns the bulk ω, g and P₁₁.
 */
export function integrateSizeTable(
  table: SizeTable,
  numberDensity: (rUm: number) => number,
): { readonly omega: number; readonly g: number; readonly p11: Float64Array } {
  const angles = table.p11[0]?.length ?? 0;
  let ext = 0;
  let sca = 0;
  let scaG = 0;
  const p11 = new Float64Array(angles);
  for (let i = 0; i + 1 < table.radiiUm.length; i += 1) {
    const l0 = Math.log(table.radiiUm[i] ?? Number.NaN);
    const l1 = Math.log(table.radiiUm[i + 1] ?? Number.NaN);
    const panels = Math.max(1, Math.ceil((l1 - l0) / SUB_PANEL_LN));
    const g0 =
      (table.scatteringAsymmetryUm2[i] ?? Number.NaN) / (table.scatteringUm2[i] ?? Number.NaN);
    const g1 =
      (table.scatteringAsymmetryUm2[i + 1] ?? Number.NaN) /
      (table.scatteringUm2[i + 1] ?? Number.NaN);
    for (let p = 0; p < panels; p += 1) {
      const a0 = l0 + ((l1 - l0) * p) / panels;
      const half = (0.5 * (l1 - l0)) / panels;
      for (const [x, w] of GL8) {
        const y = a0 + half * (1 + x);
        const t = (y - l0) / (l1 - l0);
        const r = Math.exp(y);
        const weight = half * w * r * numberDensity(r);
        const e = logLerp(
          table.extinctionUm2[i] ?? Number.NaN,
          table.extinctionUm2[i + 1] ?? Number.NaN,
          t,
        );
        const s = logLerp(
          table.scatteringUm2[i] ?? Number.NaN,
          table.scatteringUm2[i + 1] ?? Number.NaN,
          t,
        );
        ext += weight * e;
        sca += weight * s;
        scaG += weight * s * (g0 + t * (g1 - g0));
        const lo = table.p11[i];
        const hi = table.p11[i + 1];
        for (let j = 0; j < angles; j += 1) {
          p11[j] =
            (p11[j] ?? 0) + weight * s * logLerp(lo?.[j] ?? Number.NaN, hi?.[j] ?? Number.NaN, t);
        }
      }
    }
  }
  return { omega: sca / ext, g: scaG / sca, p11: p11.map((v) => v / sca) };
}

/** A node's size values as a {@link SizeTable} at a wavelength: r = 3Ṽ ÷ 4Ã × λ ÷ 2π. */
export function sizeTableOf(sizes: ReadonlyArray<NodeSizeValues>, wavelengthNm: number): SizeTable {
  const length = wavelengthNm / 1000 / (2 * Math.PI);
  const um2 = length * length;
  return {
    radiiUm: sizes.map((s) => ((0.75 * s.volume) / s.area) * length),
    extinctionUm2: sizes.map((s) => s.extinction * um2),
    scatteringUm2: sizes.map((s) => s.scattering * um2),
    scatteringAsymmetryUm2: sizes.map((s) => s.scatteringAsymmetry * um2),
    p11: sizes.map((s) => s.p11),
  };
}

/** The worst differences between a distribution's integral over a reduced table and over every node. */
export interface ReductionError {
  /** max over angles of |ΔP₁₁| ÷ P₁₁. */
  readonly a1: number;
  readonly g: number;
  readonly omega: number;
}

/**
 * The reduction's error for one distribution: its integral over the size nodes kept against the
 * same integral over every node.
 */
export function reductionError(
  every: ReadonlyArray<NodeSizeValues>,
  kept: ReadonlyArray<number>,
  wavelengthNm: number,
  numberDensity: (rUm: number) => number,
): ReductionError {
  const full = integrateSizeTable(sizeTableOf(every, wavelengthNm), numberDensity);
  const reduced = integrateSizeTable(
    sizeTableOf(
      kept.map((s) => {
        const v = every[s];
        if (v === undefined) {
          throw new Error(`no size node ${s}`);
        }
        return v;
      }),
      wavelengthNm,
    ),
    numberDensity,
  );
  let a1 = 0;
  for (let j = 0; j < full.p11.length; j += 1) {
    a1 = Math.max(a1, Math.abs((reduced.p11[j] ?? Number.NaN) / (full.p11[j] ?? Number.NaN) - 1));
  }
  return { a1, g: Math.abs(reduced.g - full.g), omega: Math.abs(reduced.omega - full.omega) };
}

/**
 * Size values from an excerpt's raw records: P₁₁ decoded from its int16, and no ratios (the
 * excerpt holds P₁₁ alone).
 */
export function excerptSizeValues(excerpt: KernelExcerpt, record: number): NodeSizeValues[] {
  const rec = excerpt.records[record];
  if (rec === undefined) {
    throw new Error(`the excerpt has no record ${record}`);
  }
  const angles = excerpt.phaseAnglesDeg.length;
  const a1 = decodeInt16(rec.a1);
  const none = new Float64Array(0);
  return rec.isca.map((row, s) => ({
    x: row[0] ?? Number.NaN,
    area: row[1] ?? Number.NaN,
    volume: row[2] ?? Number.NaN,
    extinction: row[3] ?? Number.NaN,
    scattering: row[4] ?? Number.NaN,
    scatteringAsymmetry: row[5] ?? Number.NaN,
    p11: Float64Array.from({ length: angles }, (_, j) =>
      Math.exp((a1[s * angles + j] ?? 0) / A1_SCALE),
    ),
    ratios: [none, none, none, none, none],
  }));
}

/**
 * The reduction's worst errors over a material's nodes for gamma distributions of r_eff 0.3, 1, 3
 * and 10 µm and v_eff 0.1 and 0.3: a check run while the kernels are at hand.
 */
export function checkTamudust(
  material: MaterialIndexFile,
  kernels: Readonly<Record<KernelLayout["name"], KernelSource>>,
  anglesDeg: ReadonlyArray<number>,
): ReductionError {
  const phaseIndices = phaseAngleIndices(anglesDeg, TAMUDUST_PHASE_ANGLES);
  const matrixIndices = exactAngleIndices(anglesDeg, MATRIX_ANGLES_DEG);
  const first = kernelFor(material.n[0] ?? Number.NaN, material.k[0] ?? Number.NaN);
  const nodes = wavelengthNodes(material, NODE_N_STEP[first.layout.name], first.layout.kFloor);
  let worst = { a1: 0, g: 0, omega: 0 };
  for (const g of nodes) {
    const wavelengthNm = material.wavelengthsNm[g] ?? Number.NaN;
    const { layout, kTable } = kernelFor(material.n[g] ?? Number.NaN, material.k[g] ?? Number.NaN);
    const source = kernels[layout.name];
    const grid = kernelGrid(source);
    const every = sizesUpTo(grid.x, (2 * Math.PI * TAMUDUST_D_MAX_UM) / 0.38);
    const values = kernelSizeValues(
      interpolateKernel(source, grid, material.n[g] ?? Number.NaN, kTable, SPHERICITY, every, [
        ...phaseIndices,
        ...matrixIndices,
      ]),
      phaseIndices.length,
      matrixIndices.length,
    );
    const kept = thinSizes(values);
    for (const rEff of [0.3, 1, 3, 10]) {
      for (const vEff of [0.1, 0.3]) {
        const error = reductionError(values, kept, wavelengthNm, gammaNumberDensity(rEff, vEff));
        worst = {
          a1: Math.max(worst.a1, error.a1),
          g: Math.max(worst.g, error.g),
          omega: Math.max(worst.omega, error.omega),
        };
      }
    }
  }
  return worst;
}

// ---------------------------------------------------------------------------------------------
// The command.

/**
 * Reads the 498 angles of `TAMUdust2020_Angle`, one per line.
 *
 * @throws Error if the text holds other than 498 finite numbers.
 */
export function parseAngles(text: string): number[] {
  const angles = text
    .split(/\s+/u)
    .filter((t) => t.length > 0)
    .map(Number);
  if (angles.length !== 498 || angles.some((a) => !Number.isFinite(a))) {
    throw new Error(`expected 498 angles, read ${angles.length}`);
  }
  return angles;
}

function sha256Of(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/**
 * Runs the reduction: `--tamudust <tables dir> --angles <TAMUdust2020_Angle>` writes the kernels'
 * materials and, with `--excerpt <file>`, `--mgs1m <file>` and `--enstatite <file>` (with
 * `--enstatite-sphericity <S>`, a diagnostic at another sphericity), the tests' raw excerpt and
 * one-node fixtures; `--yang <8_columns/Rough050 dir>`
 * writes water ice's. `--materials` is the index files' directory and `--out` the phase files'.
 */
export async function main(args: readonly string[]): Promise<number> {
  try {
    const { values } = parseArgs({
      args: [...args],
      options: {
        materials: { type: "string" },
        out: { type: "string" },
        tamudust: { type: "string" },
        angles: { type: "string" },
        excerpt: { type: "string" },
        mgs1m: { type: "string" },
        enstatite: { type: "string" },
        "enstatite-sphericity": { type: "string" },
        check: { type: "boolean" },
        yang: { type: "string" },
      },
      strict: true,
    });
    const { materials, out } = values;
    if (materials === undefined || out === undefined) {
      throw new Error(
        "usage: nonSpherical --materials <dir> --out <dir> [--tamudust <tables> --angles <file> [--check] [--excerpt <file>] [--mgs1m <file>] [--enstatite <file> [--enstatite-sphericity <S>]]] [--yang <dir>]",
      );
    }
    if (values.tamudust !== undefined) {
      if (values.angles === undefined) {
        throw new Error("--tamudust needs --angles");
      }
      const anglesPath = values.angles;
      const angles = parseAngles(readFileSync(anglesPath, "utf8"));
      console.warn(`angles: SHA-256 ${sha256Of(anglesPath)}`);
      const sw = openKernelFiles(join(values.tamudust, "SW"), SHORTWAVE);
      const lw = openKernelFiles(join(values.tamudust, "LW"), LONGWAVE);
      try {
        for (const { file } of TAMUDUST_MATERIALS) {
          const material = readMaterialFile(materials, file);
          if (values.check === true) {
            const e = checkTamudust(material, { shortwave: sw, longwave: lw }, angles);
            console.warn(
              `${file}: reduction check, worst |Δa₁|/a₁ ${e.a1.toExponential(2)}, |Δg| ${e.g.toExponential(2)}, |Δω| ${e.omega.toExponential(2)}`,
            );
          }
          const phase = reduceTamudust(material, file, { shortwave: sw, longwave: lw }, angles);
          writeFileSync(join(out, file), `${JSON.stringify(phase, null, 2)}\n`);
          console.warn(
            `${file}: ${phase.nodes.length} nodes, ${phase.nodes[0]?.sizeParameters.length} sizes`,
          );
        }
        if (values.mgs1m !== undefined) {
          writeFileSync(
            values.mgs1m,
            `${JSON.stringify(reduceTamudust(MGS1M, "(test) MGS-1 M", { shortwave: sw, longwave: lw }, angles, [MGS1M_NODE]), null, 2)}
`,
          );
        }
        if (values.enstatite !== undefined) {
          const sphericity = Number(values["enstatite-sphericity"] ?? SPHERICITY);
          writeFileSync(
            values.enstatite,
            `${JSON.stringify(reduceTamudust(ENSTATITE_SAMPLE, "(test) enstatite", { shortwave: sw, longwave: lw }, angles, [ENSTATITE_NODE], sphericity), null, 2)}\n`,
          );
        }
        if (values.excerpt !== undefined) {
          // n = 1.60 (index 5), k = 10⁻⁴ and 10⁻³ (indices 0 and 10), S = 0.712 (index 1).
          const excerpt = kernelExcerpt(sw, 5, [0, 10], 1, angles);
          writeFileSync(values.excerpt, `${JSON.stringify(excerpt, null, 2)}\n`);
        }
      } finally {
        sw.close();
        lw.close();
      }
    }
    if (values.yang !== undefined) {
      const file = "water-ice.json";
      const ice = readMaterialFile(materials, file);
      const phase = await reduceYang(values.yang, ice, file);
      writeFileSync(join(out, file), `${JSON.stringify(phase, null, 2)}\n`);
      console.warn(
        `${file}: ${phase.nodes.length} nodes, ${phase.nodes[0]?.sizeParameters.length} sizes`,
      );
    }
    return 0;
  } catch (error: unknown) {
    console.error(`nonSpherical: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}
