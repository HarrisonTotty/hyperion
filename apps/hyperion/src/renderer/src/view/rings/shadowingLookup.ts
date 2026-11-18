/**
 * The rings' shadowing table as the slab samples it (plan R11, R11.T9.c; Design note 15): R11.T9.b's
 * codes laid out as two `rgba16float` textures, and the CPU twin of `ringSlab.wgsl`'s lookups.
 *
 * @remarks
 * The table's nodes are uneven in τ_phot, μ_eff and the phase fraction v (`shadowingTable.ts`'
 * **Axes**), so each lookup turns its quantity into its fractional node index by the axis's own
 * spacing and filters linearly between nodes in index space: one trilinear fetch, as Design note 15
 * asks of both settings (a lean, 2026-11-18). The quantities texture is 3D, v along x, μ along y and
 * τ along z, the lit face's ten τ layers (f_e and the lit orders 2 and 3) then the unlit face's ten
 * (its orders 2 and 3), each index clamped to its face's nodes so that no fetch blends the two
 * faces; the ratios texture is 2D, μ along x and τ along y, the lit face's q_e and q_o and the
 * unlit face's in r, g, b and a. The twin reads the same half-floats with the same weights, in
 * `f64`, so that the smoke page holds the GPU's filtering to it.
 */
import { SHADOWING_CODES, SHADOWING_HEADER } from "./shadowingTable";

const { tau, mu, phase, quantities, ratios, codesPerUnit } = SHADOWING_HEADER;

/** The table's nodes on each axis: τ_phot, μ_eff and the phase fraction v. */
export const SHADOWING_NODES = { tau: tau.nodes, mu: mu.nodes, phase: phase.nodes } as const;

/** The face of a ring a lookup reads: the one the star lights, or the other. */
export type RingFace = "lit" | "unlit";

/** τ_phot's fractional node index k ∈ [0, n_τ − 1]: n_τ − 1 times ln(1 + τ ÷ s_τ) ÷ ln(1 + τ_max ÷ s_τ). */
export function tauIndex(tauPhot: number): number {
  const k =
    ((tau.nodes - 1) * Math.log1p(Math.max(tauPhot, 0) / tau.scale)) /
    Math.log1p(tau.max / tau.scale);
  return Math.min(k, tau.nodes - 1);
}

/** μ_eff's fractional node index m ∈ [0, n_μ − 1]: n_μ − 1 times √((μ_eff − μ_min) ÷ (1 − μ_min)). */
export function muIndex(muEff: number): number {
  const share = Math.min(Math.max((muEff - mu.min) / (1 - mu.min), 0), 1);
  return (mu.nodes - 1) * Math.sqrt(share);
}

/** v's fractional node index j ∈ [0, n_v − 1]: n_v − 1 times ln(1 + v ÷ s_v) ÷ ln(1 + 1 ÷ s_v). */
export function phaseIndex(v: number): number {
  const clamped = Math.min(Math.max(v, 0), 1);
  return ((phase.nodes - 1) * Math.log1p(clamped / phase.scale)) / Math.log1p(1 / phase.scale);
}

/** The cells of one quantity over (τ, μ, v). */
const CELLS = tau.nodes * mu.nodes * phase.nodes;

/** Quantity `q`'s value at node (k, m, j), decoded from its code. */
function quantityAt(q: number, k: number, m: number, j: number): number {
  const perUnit = q === 0 ? codesPerUnit.shadowing : codesPerUnit.orders;
  const code = SHADOWING_CODES[q * CELLS + (k * mu.nodes + m) * phase.nodes + j] ?? 0;
  return code / perUnit;
}

/** Ratio `r`'s value at node (k, m), decoded from its code. */
function ratioAt(r: number, k: number, m: number): number {
  const code =
    SHADOWING_CODES[quantities.length * CELLS + r * tau.nodes * mu.nodes + k * mu.nodes + m];
  return (code ?? 0) / codesPerUnit.ratios;
}

/** Each face's quantities in the quantity texture's channels, by `SHADOWING_HEADER.quantities`. */
const FACE_QUANTITIES: Readonly<Record<RingFace, ReadonlyArray<number>>> = {
  lit: [
    quantities.indexOf("shadowing"),
    quantities.indexOf("litOrder2"),
    quantities.indexOf("litOrder3"),
  ],
  unlit: [quantities.indexOf("unlitOrder2"), quantities.indexOf("unlitOrder3")],
};

/** The ratios in the ratio texture's channels, by `SHADOWING_HEADER.ratios`. */
const RATIO_CHANNELS: ReadonlyArray<number> = [
  ratios.indexOf("litEvenRatio"),
  ratios.indexOf("litOddRatio"),
  ratios.indexOf("unlitEvenRatio"),
  ratios.indexOf("unlitOddRatio"),
];

/** The quantity texture's size: v, μ and both faces' τ layers. */
export const SHADOWING_TEXTURE_SIZE = [phase.nodes, mu.nodes, 2 * tau.nodes] as const;

/** The ratio texture's size: μ and τ. */
export const SHADOWING_RATIO_TEXTURE_SIZE = [mu.nodes, tau.nodes] as const;

/** The two textures' texels, `rgba16float`, x fastest. */
export interface ShadowingTexels {
  /** (n_v, n_μ, 2 n_τ): the lit face's (f_e, I₂, I₃, 0), then the unlit face's (I₂, I₃, 0, 0). */
  readonly quantities: Float16Array;
  /** (n_μ, n_τ): the lit face's q_e and q_o, then the unlit face's. */
  readonly ratios: Float16Array;
}

/** The shadowing table's texels for the GPU, made once. */
export function shadowingTexels(): ShadowingTexels {
  const out = new Float16Array(CELLS * 2 * 4);
  for (const [f, face] of (["lit", "unlit"] as const).entries()) {
    const channels = FACE_QUANTITIES[face];
    for (let k = 0; k < tau.nodes; k += 1) {
      for (let m = 0; m < mu.nodes; m += 1) {
        for (let j = 0; j < phase.nodes; j += 1) {
          const texel = ((f * tau.nodes + k) * mu.nodes + m) * phase.nodes + j;
          channels.forEach((q, c) => {
            out[texel * 4 + c] = quantityAt(q, k, m, j);
          });
        }
      }
    }
  }
  const ratioTexels = new Float16Array(tau.nodes * mu.nodes * 4);
  for (let k = 0; k < tau.nodes; k += 1) {
    for (let m = 0; m < mu.nodes; m += 1) {
      RATIO_CHANNELS.forEach((r, c) => {
        ratioTexels[(k * mu.nodes + m) * 4 + c] = ratioAt(r, k, m);
      });
    }
  }
  return { quantities: out, ratios: ratioTexels };
}

/** The texels the twin reads, the GPU's own half-floats, made at the first lookup. */
let held: ShadowingTexels | null = null;

function texels(): ShadowingTexels {
  held ??= shadowingTexels();
  return held;
}

/** The two nodes about a fractional index and the upper one's weight. */
function bracket(index: number, nodes: number): readonly [number, number, number] {
  const lower = Math.min(Math.floor(index), nodes - 1);
  const upper = Math.min(lower + 1, nodes - 1);
  return [lower, upper, index - lower];
}

/** What one lookup of the quantity texture gives for a face. */
export interface ShadowingSample {
  /** f_e, the shadowing factor on the lit face's single scattering; 1 on the unlit face. */
  readonly shadowing: number;
  /** I/F at ϖ = 1 of the second order, per unit geometry: μ₀ ÷ (μ + μ₀) lit, μ₀ unlit. */
  readonly order2: number;
  /** The third order's, likewise. */
  readonly order3: number;
}

/**
 * The quantity texture's trilinear lookup for `face` at (τ_phot, μ_eff, v), as `ringSlab.wgsl`'s
 * `shadowing_at` takes it: each index clamped to the face's nodes, the half-floats' values.
 */
export function shadowingLookup(
  face: RingFace,
  tauPhot: number,
  muEff: number,
  v: number,
): ShadowingSample {
  const [k0, k1, wk] = bracket(tauIndex(tauPhot), tau.nodes);
  const [m0, m1, wm] = bracket(muIndex(muEff), mu.nodes);
  const [j0, j1, wj] = bracket(phaseIndex(v), phase.nodes);
  const layer = face === "lit" ? 0 : tau.nodes;
  const data = texels().quantities;
  const at = (k: number, m: number, j: number, c: number): number =>
    data[(((layer + k) * mu.nodes + m) * phase.nodes + j) * 4 + c] ?? 0;
  const channel = (c: number): number => {
    const along = (k: number, m: number): number =>
      at(k, m, j0, c) * (1 - wj) + at(k, m, j1, c) * wj;
    const across = (k: number): number => along(k, m0) * (1 - wm) + along(k, m1) * wm;
    return across(k0) * (1 - wk) + across(k1) * wk;
  };
  return face === "lit"
    ? { shadowing: channel(0), order2: channel(1), order3: channel(2) }
    : { shadowing: 1, order2: channel(0), order3: channel(1) };
}

/** The even and odd orders' ratios q_e and q_o of a face. */
export interface ShadowingRatios {
  readonly even: number;
  readonly odd: number;
}

/** The ratio texture's bilinear lookup for `face` at (τ_phot, μ_eff), as `ratios_at` takes it. */
export function shadowingRatios(face: RingFace, tauPhot: number, muEff: number): ShadowingRatios {
  const [k0, k1, wk] = bracket(tauIndex(tauPhot), tau.nodes);
  const [m0, m1, wm] = bracket(muIndex(muEff), mu.nodes);
  const data = texels().ratios;
  const at = (k: number, m: number, c: number): number => data[(k * mu.nodes + m) * 4 + c] ?? 0;
  const channel = (c: number): number => {
    const across = (k: number): number => at(k, m0, c) * (1 - wm) + at(k, m1, c) * wm;
    return across(k0) * (1 - wk) + across(k1) * wk;
  };
  const first = face === "lit" ? 0 : 2;
  return { even: channel(first), odd: channel(first + 1) };
}
