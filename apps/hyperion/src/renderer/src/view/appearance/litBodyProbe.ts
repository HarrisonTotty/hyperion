/**
 * The lit-body shading's WGSL registered in the catalogue, and the probe that compares it with its
 * TypeScript twin (plan R07, T4.c, T6.c and T11).
 *
 * @remarks
 * `shaders/litBody.wgsl` is a library of functions, not a material, so the catalogue holds it
 * inside a compute kernel that calls each function at pinned inputs and writes the results: the
 * smoke harness compiles it on SwiftShader with every other shader, and `smoke/litBody.ts`
 * dispatches it and compares the read-back results with `brdf` (T4.c) and with
 * `sphereIrradianceFactor`, `annulusVisibleFraction` and the stubs' values (T6.c), and with
 * `planetshineIrradiance` (T11). The kernel's
 * results are a pure function of its inputs on one path, so its readback is `bit-exact`.
 */
import type { KernelPair } from "../engine/kernels";
import type { Vec3 } from "../../geometry/vec3";
import { annulusEdges, annulusVisibleFraction } from "../lighting/annuli";
import { planetshineIrradiance } from "../lighting/planetshine";
import { sphereIrradianceFactor } from "../lighting/sphereIrradiance";
import type { Rgb } from "../photometry/toneCurve";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import { brdfFromTable } from "./brdf";
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
  // A per-texel law reads the row of the law it borrows, as the kernel does.
  return cases.map((probe) =>
    brdfFromTable(
      perTexelLaw(probe),
      phaseFactorTableOf(probe.law),
      Math.fround(probe.mu0),
      Math.fround(probe.mu),
      Math.fround(probe.phaseRad),
    ),
  );
}

/** One pinned evaluation of the lighting terms or of the hooks' stubs (T6.c), or of planetshine's (T11). */
export type LightingProbeCase =
  | {
      readonly kind: "sphere";
      readonly h: number;
      readonly phiRad: number;
      readonly horizonRad: number;
    }
  | {
      readonly kind: "eclipse";
      readonly starRadiusRad: number;
      readonly limbC: number;
      readonly limbAlpha: number;
      readonly annuli: 1 | 2 | 3 | 4;
      readonly occluderRadiusRad: number;
      readonly separationRad: number;
    }
  | { readonly kind: "stubs" }
  | {
      readonly kind: "planetshine";
      /** From the point to the source's centre, in the unit of the two lengths below. */
      readonly toSource: Vec3;
      readonly sourceRadius: number;
      readonly centreDistance: number;
      /** The point's unit normal. */
      readonly normal: Vec3;
      readonly horizonRad: number;
    };

/**
 * Bytes per case in the kernel's `LightingCase` array: four 16-byte rows, the eclipse case's
 * annuli in the last two, or planetshine's direction to the source and normal.
 */
export const LIGHTING_CASE_BYTES = 64;

/** The cases packed as the kernel's `array<LightingCase>`: kind, annuli, then up to six values. */
export function packLightingProbeCases(cases: ReadonlyArray<LightingProbeCase>): ArrayBuffer {
  const bytes = new ArrayBuffer(cases.length * LIGHTING_CASE_BYTES);
  const f32 = new Float32Array(bytes);
  const u32 = new Uint32Array(bytes);
  cases.forEach((probe, index) => {
    const base = (index * LIGHTING_CASE_BYTES) / 4;
    switch (probe.kind) {
      case "sphere":
        u32[base] = 1;
        f32.set([probe.h, probe.phiRad, probe.horizonRad], base + 2);
        break;
      case "eclipse":
        u32[base] = 2;
        u32[base + 1] = probe.annuli;
        f32.set(
          [
            probe.starRadiusRad,
            probe.limbC,
            probe.limbAlpha,
            probe.occluderRadiusRad,
            probe.separationRad,
          ],
          base + 2,
        );
        {
          const { edges, flux } = annulusEdges(
            Math.fround(probe.limbC),
            Math.fround(probe.limbAlpha),
            probe.annuli,
          );
          f32.set(edges.subarray(1), base + 8);
          f32.set(flux, base + 12);
        }
        break;
      case "stubs":
        u32[base] = 3;
        break;
      case "planetshine":
        u32[base] = 4;
        f32.set([probe.sourceRadius, probe.centreDistance, probe.horizonRad], base + 2);
        f32.set([probe.toSource.x, probe.toSource.y, probe.toSource.z], base + 8);
        f32.set([probe.normal.x, probe.normal.y, probe.normal.z], base + 12);
        break;
    }
  });
  return bytes;
}

/** A vector rounded to `f32`, as the GPU reads it. */
function froundVec3(v: Vec3): Vec3 {
  return { x: Math.fround(v.x), y: Math.fround(v.y), z: Math.fround(v.z) };
}

/**
 * The reference's values for each lighting case, at its inputs rounded to `f32`: the horizon
 * factor, the eclipse term's visible fraction, the stubs' 1, 1 and 0 (ring shadow, sun
 * transmittance and sky irradiance, each's first channel), or planetshine's irradiance factor.
 */
export function expectedLighting(cases: ReadonlyArray<LightingProbeCase>): number[][] {
  return cases.map((probe) => {
    let values: number[];
    switch (probe.kind) {
      case "sphere":
        values = [
          sphereIrradianceFactor(
            Math.fround(probe.h),
            Math.fround(probe.phiRad),
            Math.fround(probe.horizonRad),
          ),
        ];
        break;
      case "eclipse":
        values = [
          annulusVisibleFraction(
            annulusEdges(Math.fround(probe.limbC), Math.fround(probe.limbAlpha), probe.annuli),
            Math.fround(probe.occluderRadiusRad) / Math.fround(probe.starRadiusRad),
            Math.fround(probe.separationRad) / Math.fround(probe.starRadiusRad),
          ),
        ];
        break;
      case "stubs":
        values = [1, 1, 0];
        break;
      case "planetshine":
        values = [
          planetshineIrradiance(
            froundVec3(probe.toSource),
            Math.fround(probe.sourceRadius),
            Math.fround(probe.centreDistance),
            froundVec3(probe.normal),
            Math.fround(probe.horizonRad),
          ),
        ];
        break;
    }
    return values;
  });
}

/**
 * The probe kernel: `body_brdf` at each BRDF case (r, g, b and 0 per result), and the lighting
 * terms, the stubs or planetshine's irradiance at each lighting case (up to three values per
 * result).
 */
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

struct LightingCase {
  kind : u32,
  annuli : u32,
  p0 : f32,
  p1 : f32,
  p2 : f32,
  p3 : f32,
  p4 : f32,
  p5 : f32,
  outer : vec4f,
  flux : vec4f,
}

@group(0) @binding(0) var<storage, read> cases : array<ProbeCase>;
@group(0) @binding(1) var<storage, read_write> results : array<vec4f>;
@group(0) @binding(2) var phase_factor_table : texture_2d<f32>;
@group(0) @binding(3) var<storage, read> lighting_cases : array<LightingCase>;
@group(0) @binding(4) var<storage, read_write> lighting_results : array<vec4f>;

fn run_lighting(index : u32) {
  let c = lighting_cases[index];
  var value = vec4f(0.0);
  if (c.kind == 1u) {
    value.x = sphere_irradiance(c.p0, c.p1, c.p2);
  } else if (c.kind == 2u) {
    value.x = eclipse_visible(c.p0, DiscAnnuli(c.outer, c.flux, c.annuli), c.p3, c.p4);
  } else if (c.kind == 3u) {
    // Arbitrary arguments: the stubs ignore them.
    value.x = ring_shadow_on_body(vec3f(1.0, 2.0, 3.0), vec3f(0.0, 0.0, 1.0)).x;
    value.y = atmosphere_sun_transmittance(1000.0, 0.5, 0.3, 1.2).x;
    value.z = atmosphere_sky_irradiance(1000.0, 0.5, 0.3).x;
  } else if (c.kind == 4u) {
    value.x = planetshine_irradiance(c.outer.xyz, c.p0, c.p1, c.flux.xyz, c.p2);
  }
  lighting_results[index] = value;
}

@compute @workgroup_size(8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x < arrayLength(&lighting_cases)) {
    run_lighting(id.x);
  }
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
