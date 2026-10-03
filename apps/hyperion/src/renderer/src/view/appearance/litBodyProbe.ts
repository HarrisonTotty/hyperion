/**
 * The lit-body shading's WGSL registered in the catalogue, and the probe that compares it with its
 * TypeScript twin (plan R07, T4.c and T6.c).
 *
 * @remarks
 * `shaders/litBody.wgsl` is a library of functions, not a material, so the catalogue holds it
 * inside a compute kernel that calls each function at pinned inputs and writes the results: the
 * smoke harness compiles it on SwiftShader with every other shader, and `smoke/litBody.ts`
 * dispatches it and compares the read-back results with `brdf` at 10⁻⁵ relative. The kernel's
 * results are a pure function of its inputs on one path, so its readback is `bit-exact`.
 */
import type { KernelPair } from "../engine/kernels";
import type { Rgb } from "../photometry/toneCurve";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import { brdf } from "./brdf";
import { PHASE_TABLE_SAMPLES, phaseFactorTableOf, type PhotometricLaw } from "./law";

/** One pinned evaluation of `body_brdf`. */
export interface BrdfProbeCase {
  /** The law, or for a per-texel case the law whose table row and s it borrows. */
  readonly law: PhotometricLaw;
  /** The row of the probe's table holding the law's f. */
  readonly row: number;
  readonly mu0: number;
  readonly mu: number;
  readonly phaseRad: number;
  /**
   * A law built per texel from a synthetic normal albedo A_N and L(α) (R10's hook, Design note
   * 24), or `null` to shade with {@link BrdfProbeCase.law} as given.
   */
  readonly perTexel: { readonly normalAlbedo: number } | null;
}

/** The synthetic per-texel law's colour, A = A_N × (1, 0.9, 0.8) in r, g, b. */
const PER_TEXEL_TINT: Rgb = [1, 0.9, 0.8];

/** The synthetic per-texel L(α) = 1 − 0.5 α ÷ π, a stand-in for R10's phase-dependent share. */
export function syntheticShare(phaseRad: number): number {
  return 1 - (0.5 * phaseRad) / Math.PI;
}

/** The law a per-texel case shades with, as the kernel builds it. */
export function perTexelLaw(probe: BrdfProbeCase): PhotometricLaw {
  if (probe.perTexel === null) {
    return probe.law;
  }
  const albedo = Math.fround(probe.perTexel.normalAlbedo);
  return {
    ...probe.law,
    a: [albedo * PER_TEXEL_TINT[0], albedo * PER_TEXEL_TINT[1], albedo * PER_TEXEL_TINT[2]],
    lommelSeeligerShare: syntheticShare(Math.fround(probe.phaseRad)),
  };
}

/** Bytes per case in the kernel's `ProbeCase` array: three 16-byte rows. */
export const PROBE_CASE_BYTES = 48;

/** The cases packed as the kernel's `array<ProbeCase>`. */
export function packBrdfProbeCases(cases: ReadonlyArray<BrdfProbeCase>): ArrayBuffer {
  const bytes = new ArrayBuffer(cases.length * PROBE_CASE_BYTES);
  const f32 = new Float32Array(bytes);
  const u32 = new Uint32Array(bytes);
  cases.forEach((probe, index) => {
    const base = (index * PROBE_CASE_BYTES) / 4;
    f32.set(probe.law.a, base);
    f32[base + 3] = probe.law.lommelSeeligerShare;
    f32.set(probe.law.phaseExponent, base + 4);
    u32[base + 7] = probe.row;
    f32[base + 8] = probe.mu0;
    f32[base + 9] = probe.mu;
    f32[base + 10] = probe.phaseRad;
    if (probe.perTexel !== null) {
      u32[base + 11] = 1;
      f32[base] = probe.perTexel.normalAlbedo;
    }
  });
  return bytes;
}

/** The probe's table: each law's f as one `rgba32float` row, alpha 0. */
export function packPhaseFactorRows(laws: ReadonlyArray<PhotometricLaw>): Float32Array {
  const texels = new Float32Array(laws.length * PHASE_TABLE_SAMPLES * 4);
  laws.forEach((law, row) => {
    const { rgb } = phaseFactorTableOf(law);
    for (let i = 0; i < PHASE_TABLE_SAMPLES; i += 1) {
      const at = (row * PHASE_TABLE_SAMPLES + i) * 4;
      texels[at] = rgb[3 * i] ?? 0;
      texels[at + 1] = rgb[3 * i + 1] ?? 0;
      texels[at + 2] = rgb[3 * i + 2] ?? 0;
    }
  });
  return texels;
}

/** The reference's I/F for each case, at the case's inputs rounded to `f32` as the GPU reads them. */
export function expectedBrdf(cases: ReadonlyArray<BrdfProbeCase>): Rgb[] {
  return cases.map((probe) =>
    brdf(
      perTexelLaw(probe),
      Math.fround(probe.mu0),
      Math.fround(probe.mu),
      Math.fround(probe.phaseRad),
    ),
  );
}

/** The probe kernel: `body_brdf` at each case, r, g, b and 0 per result. */
export const LIT_BODY_PROBE: KernelPair = {
  name: "R07 lit body twin",
  reference: `${litBodyWgsl}
struct ProbeCase {
  a : vec3f,
  l : f32,
  s : vec3f,
  row : u32,
  mu0 : f32,
  mu : f32,
  alpha : f32,
  per_texel : u32,
}

@group(0) @binding(0) var<storage, read> cases : array<ProbeCase>;
@group(0) @binding(1) var<storage, read_write> results : array<vec4f>;
@group(0) @binding(2) var phase_factor_table : texture_2d<f32>;

@compute @workgroup_size(8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x >= arrayLength(&cases)) {
    return;
  }
  let c = cases[id.x];
  var law = LunarLambert(c.a, c.l, c.s, c.row);
  if (c.per_texel == 1u) {
    // A law built per texel (R10's hook): A from a normal albedo, L from the phase.
    law = LunarLambert(c.a.x * vec3f(1.0, 0.9, 0.8), 1.0 - 0.5 * c.alpha / LIT_PI, c.s, c.row);
  }
  results[id.x] = vec4f(body_brdf(law, c.mu0, c.mu, c.alpha), 0.0);
}
`,
  subgroup: null,
  readback: "bit-exact",
};
