/**
 * The clouds' probe (plan R11, R11.T7.b and T7.c): the field (`shaders/cloudField.wgsl`), the `f32`
 * δ-Eddington (`shaders/cloudOptics.wgsl`), the ground's tap (`shaders/cloudShadow.wgsl`) and the
 * volume's marches (`shaders/cloudVolume.wgsl`) run at pinned cases on the GPU, for the smoke
 * harness to hold against their `f64` twins (`cloudDensity.ts`' `cloudFieldAt`, `cloudOptics.ts`'
 * `deltaEddington`, `cloudShadow.ts`' `cloudLightAt`, and `cloudVolume.ts`' `marchColumn`,
 * `octaveCorrectionAt` and `marchDeckRay`).
 *
 * @remarks
 * They are libraries of functions, not a material, so the catalogue holds them inside a
 * material that draws one case a row into an `r32uint` target, each pixel of the row one of the
 * case's {@link CLOUD_PROBE_OUTPUTS} values as its `f32` bits: a fragment stage, since the field
 * filters its textures through a sampler, which the engine's compute bindings do not take. The
 * results are a function of the adapter's filtering and `f32` arithmetic, so the harness compares
 * them within tolerances, never bit for bit.
 */
import type { Vec3 } from "../../geometry/vec3";
import type { WgslMaterialSpec } from "../engine/types";
import frameWgsl from "../shaders/frame.wgsl?raw";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import { cloudFieldAt, sampleCoverage } from "./cloudDensity";
import { deltaEddington } from "./cloudOptics";
import { type CloudLightInput, cloudLightAt } from "./cloudShadow";
import {
  marchColumn,
  marchDeckRay,
  type OctaveCorrection,
  octaveCorrectionAt,
} from "./cloudVolume";
import cloudFieldWgsl from "./shaders/cloudField.wgsl?raw";
import cloudOpticsWgsl from "./shaders/cloudOptics.wgsl?raw";
import cloudShadowWgsl from "./shaders/cloudShadow.wgsl?raw";
import cloudVolumeWgsl from "./shaders/cloudVolume.wgsl?raw";

/** The values a case writes, one a pixel of its row. */
export const CLOUD_PROBE_OUTPUTS = 8;

/** `vec4f` rows a case takes in the probe's case buffer. */
export const CLOUD_PROBE_CASE_ROWS = 4;

/**
 * One case of the probe:
 * - `field`: `cloud_field_at` at a place on the layer, body-fixed, m, over a footprint, with an
 *   octave cap; its outputs f, the mean τ, the in-cloud τ, z_r and s, then the coverage texture's
 *   z_c, N and eastward wind there;
 * - `optics`: `cloud_delta_eddington`; its outputs R, T_dir and T_diff;
 * - `ground`: `terrain_cloud_light` at a place ÷ a towards a source; its outputs the direct, sky
 *   and diffuse factors;
 * - `column`: `cloud_column_march`, a homogeneous column's single scattering and its octaves'
 *   multiple scattering uncorrected, then `cloud_octave_correction_at` at its τ and μ₀;
 * - `deck`: `cloud_volume_march` of a view ray, body-fixed m, through the deck; its outputs the
 *   radiance and the view's transmittance.
 */
export type CloudProbeCase =
  | {
      readonly kind: "field";
      readonly pointM: Vec3;
      readonly footprintM: number;
      readonly octaveCap: number;
    }
  | {
      readonly kind: "optics";
      readonly tau: number;
      readonly asymmetry: number;
      readonly coAlbedo: number;
      readonly mu0: number;
    }
  | {
      readonly kind: "ground";
      readonly placeOverA: Vec3;
      readonly sun: Vec3;
      readonly sourceRadiusRad: number;
    }
  | {
      readonly kind: "column";
      readonly tau: number;
      readonly mu0: number;
      readonly muView: number;
      readonly cosTheta: number;
      readonly asymmetry: number;
      readonly coAlbedo: number;
      readonly steps: number;
    }
  | {
      readonly kind: "deck";
      readonly originM: Vec3;
      readonly direction: Vec3;
      readonly sun: Vec3;
      readonly baseM: number;
      readonly topM: number;
      readonly steps: number;
      readonly footprintPerM: number;
    };

/** The kernel's code of each kind. */
const KIND_CODES: Readonly<Record<CloudProbeCase["kind"], number>> = {
  field: 0,
  optics: 1,
  ground: 2,
  column: 3,
  deck: 4,
};

/** The cases as the probe's `array<vec4f>`, {@link CLOUD_PROBE_CASE_ROWS} rows a case. */
export function packCloudProbeCases(cases: ReadonlyArray<CloudProbeCase>): Float32Array {
  const out = new Float32Array(4 * CLOUD_PROBE_CASE_ROWS * cases.length);
  cases.forEach((probe, index) => {
    const at = 4 * CLOUD_PROBE_CASE_ROWS * index;
    let values: number[];
    switch (probe.kind) {
      case "field":
        values = [
          probe.pointM.x,
          probe.pointM.y,
          probe.pointM.z,
          probe.footprintM,
          probe.octaveCap,
        ];
        break;
      case "optics":
        values = [probe.tau, probe.asymmetry, probe.coAlbedo, probe.mu0];
        break;
      case "ground":
        values = [
          probe.placeOverA.x,
          probe.placeOverA.y,
          probe.placeOverA.z,
          probe.sun.x,
          probe.sun.y,
          probe.sun.z,
          probe.sourceRadiusRad,
        ];
        break;
      case "column":
        values = [
          probe.tau,
          probe.mu0,
          probe.muView,
          probe.cosTheta,
          probe.asymmetry,
          probe.coAlbedo,
          probe.steps,
        ];
        break;
      case "deck":
        values = [
          probe.originM.x,
          probe.originM.y,
          probe.originM.z,
          probe.direction.x,
          probe.direction.y,
          probe.direction.z,
          probe.sun.x,
          probe.sun.y,
          probe.sun.z,
          probe.baseM,
          probe.topM,
          probe.steps,
          probe.footprintPerM,
        ];
        break;
    }
    out[at] = KIND_CODES[probe.kind];
    out.set(values, at + 1);
  });
  return out;
}

/** A vector with each component rounded to `f32`, as the probe reads it. */
function f32Of(v: Vec3): Vec3 {
  return { x: Math.fround(v.x), y: Math.fround(v.y), z: Math.fround(v.z) };
}

/**
 * The twins' values of each case, {@link CLOUD_PROBE_OUTPUTS} a case (0 past a case's own), at its
 * inputs rounded to `f32` as the GPU reads them.
 *
 * @param clouds - The body's clouds the probe's bindings hold: its coverage texels as the texture
 *   holds them, the noise tile, its field's parameters, shell and optics, and its datum's radius.
 * @param correction - Its condensate's octave correction, as the probe's table holds it.
 */
export function expectedCloudProbe(
  cases: ReadonlyArray<CloudProbeCase>,
  clouds: CloudLightInput,
  correction: OctaveCorrection,
): number[][] {
  return cases.map((probe) => {
    let values: number[];
    switch (probe.kind) {
      case "field": {
        const point = f32Of(probe.pointM);
        const sample = cloudFieldAt(
          clouds.coverage,
          clouds.noise,
          clouds.params,
          point,
          Math.fround(probe.footprintM),
          probe.octaveCap,
        );
        const cell = sampleCoverage(clouds.coverage, point);
        values = [
          sample.fraction,
          sample.meanTau,
          sample.inCloudTau,
          sample.resolved,
          sample.unresolvedSd,
          cell.threshold,
          cell.scale,
          cell.eastMps,
        ];
        break;
      }
      case "optics": {
        const slab = deltaEddington({
          tau: Math.fround(probe.tau),
          asymmetry: Math.fround(probe.asymmetry),
          singleScatterAlbedo: 1 - Math.fround(probe.coAlbedo),
          mu0: Math.fround(probe.mu0),
        });
        values = [
          slab.reflectance,
          slab.directTransmittance,
          Math.max(slab.diffuseTransmittance, 0),
        ];
        break;
      }
      case "ground": {
        const light = cloudLightAt(
          clouds,
          f32Of(probe.placeOverA),
          f32Of(probe.sun),
          Math.fround(probe.sourceRadiusRad),
        );
        values = [light.direct, light.sky, light.diffuse];
        break;
      }
      case "column": {
        const tau = Math.fround(probe.tau);
        const mu0 = Math.fround(probe.mu0);
        const light = marchColumn({
          tau,
          mu0,
          muView: Math.fround(probe.muView),
          cosTheta: Math.fround(probe.cosTheta),
          optics: {
            asymmetry: Math.fround(probe.asymmetry),
            coAlbedo: Math.fround(probe.coAlbedo),
          },
          steps: probe.steps,
        });
        values = [light.single, light.multiple, octaveCorrectionAt(correction, tau, mu0)];
        break;
      }
      case "deck": {
        const march = marchDeckRay(
          clouds,
          { baseM: Math.fround(probe.baseM), topM: Math.fround(probe.topM) },
          correction,
          {
            originM: f32Of(probe.originM),
            direction: f32Of(probe.direction),
            sun: f32Of(probe.sun),
            steps: probe.steps,
            footprintPerM: Math.fround(probe.footprintPerM),
          },
        );
        values = [march.radiance, march.transmittance];
        break;
      }
    }
    return [...values, ...Array.from({ length: CLOUD_PROBE_OUTPUTS - values.length }, () => 0)];
  });
}

/** The probe's read-back `r32uint` texels as each case's values. */
export function decodeCloudProbe(bytes: ArrayBuffer, cases: number): number[][] {
  const values = new Float32Array(bytes);
  return Array.from({ length: cases }, (_, row) =>
    Array.from(values.subarray(row * CLOUD_PROBE_OUTPUTS, (row + 1) * CLOUD_PROBE_OUTPUTS)),
  );
}

/** The probe's bindings in `@group(2)`. */
export const CLOUD_PROBE_BINDINGS = {
  coverage: 0,
  noise: 1,
  sampler: 2,
  layer: 3,
  cases: 4,
  correction: 5,
} as const;

/** The probe's names for the buffer of its cases and its textures, as a draw binds them. */
export const CLOUD_PROBE_NAMES = {
  coverage: "cloudCoverage",
  noise: "cloudNoise",
  sampler: "cloudSampler",
  layer: "cloudLayer",
  cases: "cloudProbeCases",
  correction: "cloudOctaveCorrection",
} as const;

/** The probe's own source, after the libraries it runs. */
const PROBE_WGSL = `
struct Draw {
  offsetFromCameraM : vec3f,
}
@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var cloud_coverage : texture_2d<f32>;
@group(2) @binding(1) var cloud_noise : texture_3d<f32>;
@group(2) @binding(2) var cloud_sampler : sampler;
@group(2) @binding(3) var<storage, read> cloud_layer : array<vec4f>;
@group(2) @binding(4) var<storage, read> cloud_probe_cases : array<vec4f>;
@group(2) @binding(5) var cloud_octave_correction : texture_2d<f32>;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return frame.clipProjection * frame.viewRotation * vec4f(position + draw.offsetFromCameraM, 1.0);
}

// One case a row, one of its values a pixel, as its f32 bits (cloudProbe.ts).
@fragment fn fragmentMain(@builtin(position) at : vec4f) -> @location(0) u32 {
  let row = u32(at.y);
  let column = min(u32(at.x), 7u);
  if (row >= arrayLength(&cloud_probe_cases) / 4u) {
    return 0u;
  }
  let head = cloud_probe_cases[4u * row];
  let a = cloud_probe_cases[4u * row + 1u];
  let b = cloud_probe_cases[4u * row + 2u];
  let e = cloud_probe_cases[4u * row + 3u];
  var values = array<f32, 8>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
  let kind = u32(head.x);
  if (kind == 0u) {
    let sample = cloud_field_at(head.yzw, a.x, u32(a.y));
    let cell = cloud_coverage_at(head.yzw);
    values = array<f32, 8>(
      sample.fraction, sample.mean_tau, sample.in_cloud_tau, sample.resolved, sample.unresolved_sd,
      cell.x, cell.y, cell.z,
    );
  } else if (kind == 1u) {
    let slab = cloud_delta_eddington(head.y, head.z, head.w, a.x);
    values[0] = slab.reflectance;
    values[1] = slab.direct;
    values[2] = slab.diffuse;
  } else if (kind == 2u) {
    let light = terrain_cloud_light(head.yzw, a.xyz, a.w);
    values[0] = light.x;
    values[1] = light.y;
    values[2] = light.z;
  } else if (kind == 3u) {
    let light = cloud_column_march(head.y, head.z, head.w, a.x, a.y, a.z, u32(a.w));
    values[0] = light.x;
    values[1] = light.y;
    values[2] = cloud_octave_correction_at(head.y, head.z);
  } else {
    let march = cloud_volume_march(head.yzw, a.xyz, vec3f(a.w, b.xy), b.z, b.w, u32(e.x), e.y);
    values[0] = march.x;
    values[1] = march.y;
  }
  return bitcast<u32>(values[column]);
}
`;

/**
 * The probe's WGSL: the frame, `preciseTrig.wgsl`, the field, the optics, the tap, the volume and the
 * probe.
 */
export const CLOUD_PROBE_WGSL = `${frameWgsl}\n${preciseTrigWgsl}\n${cloudFieldWgsl}\n${cloudOpticsWgsl}\n${cloudShadowWgsl}\n${cloudVolumeWgsl}\n${PROBE_WGSL}`;

/** The probe's material, drawn into an `r32uint` target of {@link CLOUD_PROBE_OUTPUTS} columns. */
export const CLOUD_PROBE_MATERIAL: WgslMaterialSpec = {
  name: "cloud probe",
  displayName: "CLOUD PROBE",
  vertexWgsl: CLOUD_PROBE_WGSL,
  fragmentWgsl: CLOUD_PROBE_WGSL,
  uniforms: [],
  samplers: [
    {
      name: CLOUD_PROBE_NAMES.sampler,
      filter: "linear",
      address: "repeat",
      binding: CLOUD_PROBE_BINDINGS.sampler,
    },
  ],
  textures: [
    { name: CLOUD_PROBE_NAMES.coverage, binding: CLOUD_PROBE_BINDINGS.coverage },
    { name: CLOUD_PROBE_NAMES.noise, binding: CLOUD_PROBE_BINDINGS.noise, viewDimension: "3d" },
    {
      name: CLOUD_PROBE_NAMES.correction,
      binding: CLOUD_PROBE_BINDINGS.correction,
      sampleType: "unfilterable-float",
    },
  ],
  storageBuffers: [
    { name: CLOUD_PROBE_NAMES.layer, binding: CLOUD_PROBE_BINDINGS.layer, stages: ["fragment"] },
    { name: CLOUD_PROBE_NAMES.cases, binding: CLOUD_PROBE_BINDINGS.cases, stages: ["fragment"] },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};
