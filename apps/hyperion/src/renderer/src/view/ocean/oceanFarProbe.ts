/**
 * The far ocean's WGSL as the catalogue holds it, its uniform's packing, and the probe kernel that
 * compares it with its CPU twin (plan R11, R11.T8.b).
 *
 * @remarks
 * `oceanFar.wgsl` is a library of functions, not a pass (the `ocean` pass that draws it is wired
 * later, wave 2), so the catalogue holds it inside a compute kernel, as R07's lit-body library is
 * held (`litBodyProbe.ts`): each invocation meets one ray with the ocean through
 * `ocean_far_sample`, over a body's ocean map, and writes its radiance and its hit, which the smoke
 * harness (`smoke/ocean.ts`, part 27) holds against `oceanFarSample`. One path, so its read-back is
 * `bit-exact`.
 */
import type { Vec3 } from "../../geometry/vec3";
import type { KernelPair } from "../engine/kernels";
import cubeTexelWgsl from "../shaders/cubeTexel.wgsl?raw";
import type { OceanFarOptics } from "./oceanFar";
import oceanFarWgsl from "./oceanFar.wgsl?raw";

/** The far ocean's library, composed after the cube sphere's texel lookup it calls. */
export const OCEAN_FAR_WGSL = `${cubeTexelWgsl}\n${oceanFarWgsl}`;

/** The floats of `OceanFar`, eight `vec4f`. */
export const OCEAN_FAR_FLOATS = 32;

/** The largest `f32` the shader takes in place of ∞ (an airless sea's threshold, no cap). */
const F32_INFINITE = 1e30;

/** `x`, ∞ as {@link F32_INFINITE}. */
function finite(x: number): number {
  return Math.min(x, F32_INFINITE);
}

/** An {@link OceanFarOptics} as the shader's `OceanFar` uniform: eight `vec4f`, body-fixed. */
export function packOceanFar(optics: OceanFarOptics): Float32Array<ArrayBuffer> {
  const { sunDirection: s, sunLux, skyLux, refractiveIndex: n, deepReflectance: deep } = optics;
  const attenuation = optics.attenuationPerM;
  const bed = optics.bedAlbedo;
  return Float32Array.from([
    optics.equatorialRadiusM,
    optics.polarRadiusM,
    optics.pixelAngleRad,
    optics.whitecaps ? 1 : 0,
    s.x,
    s.y,
    s.z,
    finite(optics.glintCapCdM2),
    sunLux[0],
    sunLux[1],
    sunLux[2],
    finite(optics.sea.thresholdMps),
    skyLux[0],
    skyLux[1],
    skyLux[2],
    optics.sea.equivalentPerMps,
    n[0],
    n[1],
    n[2],
    optics.sea.coxMunkPerMps,
    attenuation?.[0] ?? 0,
    attenuation?.[1] ?? 0,
    attenuation?.[2] ?? 0,
    attenuation === null ? 0 : 1,
    deep[0],
    deep[1],
    deep[2],
    optics.skyReflectance,
    bed[0],
    bed[1],
    bed[2],
    0,
  ]);
}

/** One ray of the probe: from the body's centre, body-fixed, m, along a unit direction. */
export interface OceanProbeRay {
  readonly originM: Vec3;
  readonly direction: Vec3;
}

/** The probe's uniform: the optics, then the ray count. */
export function packOceanFarProbe(
  optics: OceanFarOptics,
  count: number,
): Float32Array<ArrayBuffer> {
  const out = new Float32Array(OCEAN_FAR_FLOATS + 4);
  out.set(packOceanFar(optics));
  out[OCEAN_FAR_FLOATS] = count;
  return out;
}

/** The probe's rays, two `vec4f` a ray: its origin, then its direction. */
export function packOceanRays(rays: ReadonlyArray<OceanProbeRay>): Float32Array<ArrayBuffer> {
  const out = new Float32Array(8 * Math.max(rays.length, 1));
  rays.forEach(({ originM: o, direction: d }, i) => {
    out.set([o.x, o.y, o.z, 0, d.x, d.y, d.z, 0], 8 * i);
  });
  return out;
}

/** The probe's invocations a workgroup. */
export const OCEAN_PROBE_WORKGROUP = 64;

/**
 * The far ocean's probe: ray i against the ocean, writing (radiance, found) then (t, hit point) at
 * `samples[2 i]` and `samples[2 i + 1]`.
 */
export const OCEAN_FAR_PROBE: KernelPair = {
  name: "R11 ocean far twin",
  reference: `${OCEAN_FAR_WGSL}
struct ProbeParams {
  far : OceanFar,
  // x the ray count.
  counts : vec4f,
}

@group(0) @binding(0) var<uniform> params : ProbeParams;
@group(0) @binding(1) var oceanMap : texture_2d_array<f32>;
@group(0) @binding(2) var<storage, read> rays : array<vec4f>;
@group(0) @binding(3) var<storage, read_write> samples : array<vec4f>;

@compute @workgroup_size(${OCEAN_PROBE_WORKGROUP})
fn main(@builtin(global_invocation_id) id : vec3u) {
  let i = id.x;
  if (i >= u32(params.counts.x)) {
    return;
  }
  let s = ocean_far_sample(oceanMap, params.far, rays[2u * i].xyz, rays[2u * i + 1u].xyz);
  samples[2u * i] = vec4f(s.radiance, select(0.0, 1.0, s.found));
  samples[2u * i + 1u] = vec4f(s.t, s.point);
}
`,
  subgroup: null,
  readback: "bit-exact",
};
