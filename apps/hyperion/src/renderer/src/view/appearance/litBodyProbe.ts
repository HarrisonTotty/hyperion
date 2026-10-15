/**
 * The lit-body shading's WGSL registered in the catalogue, and the probe that compares it with its
 * TypeScript twin (plan R07, T4.c, T6.c and T11; plan R08, R08.T9.b).
 *
 * @remarks
 * `shaders/litBody.wgsl` is a library of functions, not a material, so the catalogue holds it
 * inside a compute kernel that calls each function at pinned inputs and writes the results: the
 * smoke harness compiles it on SwiftShader with every other shader, and `smoke/litBody.ts`
 * dispatches it and compares the read-back results with `brdf` (T4.c) and with
 * `sphereIrradianceFactor` and `annulusVisibleFraction` (T6.c), and with `planetshineIrradiance`
 * (T11). Since R08.T9.b the kernel composes R08's `surfaceLighting.wgsl` in place of the stubs it
 * replaced, with its tables at group 0 bindings 5 to 11: the hooks' case holds R11's ring-shadow
 * stub and R08's lookups against a body's lighting reader ({@link expectedLighting}), and the
 * `atmosphere` cases each of R08's three lookups, which `smoke/atmosphere.ts` holds against the
 * tables' twin (`tablesCpu.ts`'s `surfaceLightingTwin`) and `smoke/litBody.ts` against a body
 * without an atmosphere. The kernel's results are a pure function of its inputs on one path, so
 * its readback is `bit-exact`.
 */
import type { KernelPair } from "../engine/kernels";
import type { Vec3 } from "../../geometry/vec3";
import {
  AIRLESS_LIGHTING,
  SURFACE_LIGHTING_WGSL,
  type SurfaceLightingReader,
} from "../atmosphere/surfaceLighting";
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
   * A law built per texel from a synthetic normal albedo A_N (R10's class law, Design note 24),
   * whose L(α) its row carries (decision-r07-t8b), or `null` to shade with
   * {@link BrdfProbeCase.law} as given.
   */
  readonly perTexel: { readonly normalAlbedo: number } | null;
}

/** The synthetic per-texel law's colour, A = A_N × (1, 0.9, 0.8) in r, g, b. */
const PER_TEXEL_TINT: Rgb = [1, 0.9, 0.8];

/** The law a per-texel case shades with, as the kernel builds it: its A, the row's L(α). */
export function perTexelLaw(probe: BrdfProbeCase): PhotometricLaw {
  if (probe.perTexel === null) {
    return probe.law;
  }
  const albedo = Math.fround(probe.perTexel.normalAlbedo);
  return {
    ...probe.law,
    a: [albedo * PER_TEXEL_TINT[0], albedo * PER_TEXEL_TINT[1], albedo * PER_TEXEL_TINT[2]],
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

/**
 * A frame's phase table: each law's f as one `rgba32float` row, f in r, g and b and its L(α) in a
 * (`law.ts`'s `PhaseFactorTable`; plan R10, R10.T10.b, decision-r07-t8b), L(0) at every sample for
 * a law without a curve.
 */
export function packPhaseFactorRows(laws: ReadonlyArray<PhotometricLaw>): Float32Array {
  const texels = new Float32Array(laws.length * PHASE_TABLE_SAMPLES * 4);
  laws.forEach((law, row) => {
    const { rgb, alpha } = phaseFactorTableOf(law);
    for (let i = 0; i < PHASE_TABLE_SAMPLES; i += 1) {
      const at = (row * PHASE_TABLE_SAMPLES + i) * 4;
      texels[at] = rgb[3 * i] ?? 0;
      texels[at + 1] = rgb[3 * i + 1] ?? 0;
      texels[at + 2] = rgb[3 * i + 2] ?? 0;
      texels[at + 3] = alpha[i] ?? 0;
    }
  });
  return texels;
}

/** The reference's I/F for each case, at the case's inputs rounded to `f32` as the GPU reads them. */
export function expectedBrdf(cases: ReadonlyArray<BrdfProbeCase>): Rgb[] {
  // A per-texel law reads its f and L(α) from the row of the law it is made from, as the kernel does.
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

/** R08's three lookups (`surfaceLighting.wgsl`; R08.T9.b), by the codes the probe kernel reads. */
export type AtmosphereLookup = "sun" | "sky" | "source";

/** The kernel's code of each lookup. */
const LOOKUP_CODES: Readonly<Record<AtmosphereLookup, number>> = { sun: 0, sky: 1, source: 2 };

/**
 * The hooks' case's fixed arguments (R08.T9.b): a point 1 km up at latitude 0.3 rad, a source at
 * zenith cosine 0.5 and azimuth 1.2 rad, light slot 0.
 */
const HOOKS_ARGUMENTS = { altitudeM: 1_000, mu: 0.5, latitudeRad: 0.3, azimuthRad: 1.2 } as const;

/**
 * One pinned evaluation of the lighting terms (T6.c), of the hooks (R11's ring-shadow stub and
 * R08's lookups at fixed arguments), of planetshine's (T11), or of one of R08's lookups (R08.T9.b).
 */
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
  | { readonly kind: "hooks" }
  | {
      readonly kind: "planetshine";
      /** From the point to the source's centre, in the unit of the two lengths below. */
      readonly toSource: Vec3;
      readonly sourceRadius: number;
      readonly centreDistance: number;
      /** The point's unit normal. */
      readonly normal: Vec3;
      readonly horizonRad: number;
    }
  | {
      readonly kind: "atmosphere";
      readonly lookup: AtmosphereLookup;
      /** The lit point's geodetic height above the datum, m. */
      readonly altitudeM: number;
      /** The source's zenith cosine against the datum's normal. */
      readonly mu: number;
      readonly latitudeRad: number;
      /** The source's azimuth from north towards east; the sky's lookup takes none. */
      readonly azimuthRad: number;
      /** The source's angular radius, for the `source` lookup alone. */
      readonly angularRadiusRad: number;
      /** The light's absorber-curve slot. */
      readonly curve: number;
    };

/**
 * Bytes per case in the kernel's `LightingCase` array: four 16-byte rows, the eclipse case's
 * annuli in the last two, or planetshine's direction to the source and normal.
 */
export const LIGHTING_CASE_BYTES = 64;

/**
 * The cases packed as the kernel's `array<LightingCase>`: kind, annuli (an R08 lookup's code), then
 * up to six values.
 */
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
      case "hooks":
        u32[base] = 3;
        break;
      case "planetshine":
        u32[base] = 4;
        f32.set([probe.sourceRadius, probe.centreDistance, probe.horizonRad], base + 2);
        f32.set([probe.toSource.x, probe.toSource.y, probe.toSource.z], base + 8);
        f32.set([probe.normal.x, probe.normal.y, probe.normal.z], base + 12);
        break;
      case "atmosphere":
        u32[base] = 5;
        u32[base + 1] = LOOKUP_CODES[probe.lookup];
        f32.set(
          [
            probe.altitudeM,
            probe.mu,
            probe.latitudeRad,
            probe.azimuthRad,
            probe.angularRadiusRad,
            probe.curve,
          ],
          base + 2,
        );
        break;
    }
  });
  return bytes;
}

/** An R08 lookup at a case's arguments rounded to `f32`, as the GPU reads them (R08.T9.b). */
function lookupOf(
  lighting: SurfaceLightingReader,
  lookup: AtmosphereLookup,
  altitudeM: number,
  mu: number,
  latitudeRad: number,
  azimuthRad: number,
  angularRadiusRad: number,
  curve: number,
): Rgb {
  const h = Math.fround(altitudeM);
  const m = Math.fround(mu);
  const lat = Math.fround(latitudeRad);
  const az = Math.fround(azimuthRad);
  const rho = Math.fround(angularRadiusRad);
  let value: Rgb;
  switch (lookup) {
    case "sun":
      value = lighting.sunTransmittance(h, m, lat, az, curve);
      break;
    case "sky":
      value = lighting.skyIrradiance(h, m, lat, curve);
      break;
    case "source":
      value = lighting.sourceTransmittance(h, m, lat, az, rho, curve);
      break;
  }
  return value;
}

/** A vector rounded to `f32`, as the GPU reads it. */
function froundVec3(v: Vec3): Vec3 {
  return { x: Math.fround(v.x), y: Math.fround(v.y), z: Math.fround(v.z) };
}

/**
 * The reference's values for each lighting case, at its inputs rounded to `f32`: the horizon
 * factor, the eclipse term's visible fraction, the hooks' (R11's ring-shadow stub, 1, and R08's
 * sun transmittance and sky irradiance at the hooks' arguments, each's first channel), planetshine's
 * irradiance factor, or an R08 lookup's three channels (R08.T9.b).
 *
 * @param lighting - The lookups of the body whose tables the kernel binds: a twin of them, or
 *   {@link AIRLESS_LIGHTING} (R07's stubs' 1 and 0) for the one-texel defaults.
 */
export function expectedLighting(
  cases: ReadonlyArray<LightingProbeCase>,
  lighting: SurfaceLightingReader = AIRLESS_LIGHTING,
): number[][] {
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
      case "hooks": {
        const { altitudeM, mu, latitudeRad, azimuthRad } = HOOKS_ARGUMENTS;
        const args = [altitudeM, mu, latitudeRad, azimuthRad, 0, 0] as const;
        values = [1, lookupOf(lighting, "sun", ...args)[0], lookupOf(lighting, "sky", ...args)[0]];
        break;
      }
      case "atmosphere":
        values = [
          ...lookupOf(
            lighting,
            probe.lookup,
            probe.altitudeM,
            probe.mu,
            probe.latitudeRad,
            probe.azimuthRad,
            probe.angularRadiusRad,
            probe.curve,
          ),
        ];
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
 * terms, the hooks, planetshine's irradiance or an R08 lookup at each lighting case (up to three
 * values per result). It composes R08's surface lighting (`SURFACE_LIGHTING_WGSL`, R08.T9.b) and
 * declares its tables at group 0 bindings 5 to 11, by `surfaceLighting.wgsl`'s names.
 */
export const LIT_BODY_PROBE: KernelPair = {
  name: "R07 lit body twin",
  reference: `${SURFACE_LIGHTING_WGSL}${litBodyWgsl}
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

// R08's tables (surfaceLighting.wgsl, R08.T9.b): a body's, or the one-texel defaults.
@group(0) @binding(5) var atmosphere_body : texture_2d<f32>;
@group(0) @binding(6) var atmosphere_transmittance : texture_2d_array<f32>;
@group(0) @binding(7) var atmosphere_absorber_columns : texture_2d_array<f32>;
@group(0) @binding(8) var atmosphere_absorber_pressure_columns : texture_2d_array<f32>;
@group(0) @binding(9) var atmosphere_last_absorber_pressure_column : texture_2d_array<f32>;
@group(0) @binding(10) var atmosphere_absorber_curves : texture_2d<f32>;
@group(0) @binding(11) var atmosphere_irradiance : texture_2d_array<f32>;

fn run_lighting(index : u32) {
  let c = lighting_cases[index];
  var value = vec4f(0.0);
  if (c.kind == 1u) {
    value.x = sphere_irradiance(c.p0, c.p1, c.p2);
  } else if (c.kind == 2u) {
    value.x = eclipse_visible(c.p0, DiscAnnuli(c.outer, c.flux, c.annuli), c.p3, c.p4);
  } else if (c.kind == 3u) {
    // R11's stub ignores its arguments; R08's lookups take the hooks' (litBodyProbe.ts).
    value.x = ring_shadow_on_body(vec3f(1.0, 2.0, 3.0), vec3f(0.0, 0.0, 1.0)).x;
    value.y = atmosphere_sun_transmittance(1000.0, 0.5, 0.3, 1.2, 0u).x;
    value.z = atmosphere_sky_irradiance(1000.0, 0.5, 0.3, 0u).x;
  } else if (c.kind == 4u) {
    value.x = planetshine_irradiance(c.outer.xyz, c.p0, c.p1, c.flux.xyz, c.p2);
  } else if (c.kind == 5u) {
    // An R08 lookup (R08.T9.b): 0 the sun's transmittance, 1 the sky's irradiance, 2 a wide
    // source's transmittance; the light's slot in p5.
    let curve = u32(c.p5);
    if (c.annuli == 0u) {
      value = vec4f(atmosphere_sun_transmittance(c.p0, c.p1, c.p2, c.p3, curve), 0.0);
    } else if (c.annuli == 1u) {
      value = vec4f(atmosphere_sky_irradiance(c.p0, c.p1, c.p2, curve), 0.0);
    } else {
      value = vec4f(atmosphere_source_transmittance(c.p0, c.p1, c.p2, c.p3, c.p4, curve), 0.0);
    }
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
    // A law built per texel (R10's class law): A from a normal albedo, L(α) from the row's alpha.
    law = LunarLambert(c.a.x * vec3f(1.0, 0.9, 0.8), c.l, c.s, c.row);
  }
  results[id.x] = vec4f(body_brdf(law, c.mu0, c.mu, c.alpha), 0.0);
}
`,
  subgroup: null,
  readback: "bit-exact",
};
