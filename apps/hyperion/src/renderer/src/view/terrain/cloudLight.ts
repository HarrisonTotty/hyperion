/**
 * The lit terrain's cloudy forms (plan R11, R11.T7.b; Design note 10): each lit form of R10's
 * terrain with R11's clouds' light on the ground, for a body whose clouds the frame draws under the
 * setting's `cloudShadows`.
 *
 * @remarks
 * - **One tap a source.** Each source's light takes `cloudShadow.wgsl`'s
 *   `terrain_cloud_light(q, towards, star_radius)` at the texel, once: its x multiplies the direct
 *   sun in place of `terrain_shadow_factor`, its y the sky's light from the source, and its z, the
 *   share of the source's direct light the cloud scatters down, adds μ₀ E T times it to the sky's,
 *   on the same tilt and eclipse, its T the source's transmittance to the ground, the lookup the
 *   direct term then takes again (plan R11's T7.b records: one hook for both of R10's).
 * - **The plain forms stay as they were, byte for byte.** A cloudy form is its plain form's source
 *   with three lines of `shade_terrain` replaced ({@link withCloudLight}) and the clouds' field,
 *   optics and tap composed after it with their bindings, so a body without clouds, or with
 *   `cloudShadows` off, draws the plain form, which writes neither hook.
 * - **Which forms.** The lit material, its interior patches' form, and both on the cluster path,
 *   on each vertex path. The emissive forms and the visibility buffer's resolve take none, so a body
 *   above the thermal cut, and every patch of a frame shaded by IDs, draws its ground without its
 *   clouds' light (lean, 2026-11-18; the resolve's sampled textures stand at 15 of 16, plan R14's
 *   ruling of 2026-11-13).
 * - **Bindings.** Its fragment stage reads the body's rows ({@link TERRAIN_LIT_BINDINGS}'
 *   `cloudLayer`), its coverage and the noise tile through a linear, repeating sampler: 15 sampled
 *   textures, two samplers and five fragment storage buffers, inside WebGPU's defaults.
 */
import { CLOUD_TEXTURES } from "../clouds/cloudLayer";
import cloudFieldWgsl from "../clouds/shaders/cloudField.wgsl?raw";
import cloudOpticsWgsl from "../clouds/shaders/cloudOptics.wgsl?raw";
import cloudShadowWgsl from "../clouds/shaders/cloudShadow.wgsl?raw";
import type { WgslMaterialSpec } from "../engine/types";
import type { TerrainVertexPath } from "../quality/qualitySetting";
import {
  TERRAIN_LIT_BINDINGS,
  terrainLitClusterMaterialSpec,
  terrainLitFinMaterialSpec,
  terrainLitInteriorMaterialSpec,
  terrainLitMaterialSpec,
} from "./terrainMaterial";

/** `shade_terrain`'s sky light from a source, which a cloudy form takes through the cloud. */
const SKY_LIGHT =
  "    out.radiance = out.radiance + light.xyz * misc.y * mean_a * sky * visible;\n";

/**
 * {@link SKY_LIGHT} under the clouds: the tap at the texel, the source's transmittance to the
 * ground looked up once, and the sky's light through the cloud with the light it scatters down.
 */
const CLOUDED_SKY_LIGHT =
  "    let cloud = terrain_cloud_light(q, towards, star_radius);\n" +
  "    let transmitted = atmosphere_sun_transmittance(\n" +
  "      height, mu_datum, local.latitude, local_azimuth(local, towards), j\n" +
  "    );\n" +
  "    let clouded_sky = sky * cloud.y + cloud.z * max(mu_datum, 0.0) * transmitted * sky_factor;\n" +
  "    out.radiance = out.radiance + light.xyz * misc.y * mean_a * clouded_sky * visible;\n";

/** `shade_terrain`'s direct sun's shadows, R11's factor among them. */
const SHADOWED = "    var shadowed = terrain_shadow_factor(q) * terrain_ring_shadow(q, towards);\n";

/** {@link SHADOWED} with the cloud's direct factor in R11's place. */
const CLOUDED_SHADOWED = "    var shadowed = cloud.x * terrain_ring_shadow(q, towards);\n";

/** The direct term's lookup of the source's transmittance, which a cloudy form has made already. */
const DIRECT_TRANSMITTED =
  "      let transmitted = atmosphere_sun_transmittance(\n" +
  "        height, mu_datum, local.latitude, local_azimuth(local, towards), j\n" +
  "      );\n";

/** The clouds' bindings in a cloudy form, as `cloudField.wgsl`'s header asks its includer. */
const CLOUD_BINDINGS_WGSL = `
// R11's clouds over the terrain (plan R11, R11.T7.b; \`terrain/cloudLight.ts\`): the body's rows,
// its coverage texture, the noise tile and their linear, repeating sampler.
@group(2) @binding(${String(TERRAIN_LIT_BINDINGS.cloudLayer)}) var<storage, read> cloud_layer : array<vec4f>;
@group(2) @binding(${String(TERRAIN_LIT_BINDINGS.cloudCoverage)}) var cloud_coverage : texture_2d<f32>;
@group(2) @binding(${String(TERRAIN_LIT_BINDINGS.cloudNoise)}) var cloud_noise : texture_3d<f32>;
@group(2) @binding(${String(TERRAIN_LIT_BINDINGS.cloudSampler)}) var cloud_sampler : sampler;
`;

/** `source` with `part` replaced by `by`, which it holds exactly once. */
function replacedOnce(source: string, part: string, by: string, what: string): string {
  const parts = source.split(part);
  if (parts.length !== 2) {
    throw new Error(
      `a lit terrain's cloudy form needs terrainLit.wgsl's ${what} once, found ${String(parts.length - 1)}`,
    );
  }
  return parts.join(by);
}

/**
 * A lit terrain source's cloudy form (the module's remarks): `shade_terrain`'s sky light, its
 * direct shadows and its direct term's lookup replaced, then the clouds' bindings, field, optics and
 * tap appended.
 *
 * @throws Error unless each replaced line occurs exactly once in `source`.
 */
export function withCloudLight(source: string): string {
  const clouded = replacedOnce(
    replacedOnce(
      replacedOnce(source, SKY_LIGHT, CLOUDED_SKY_LIGHT, "sky light"),
      SHADOWED,
      CLOUDED_SHADOWED,
      "direct shadows",
    ),
    DIRECT_TRANSMITTED,
    "",
    "direct transmittance",
  );
  return `${clouded}\n${CLOUD_BINDINGS_WGSL}\n${cloudFieldWgsl}\n${cloudOpticsWgsl}\n${cloudShadowWgsl}`;
}

/** The console names of the cloudy forms, by the plain form's name. */
const CLOUDY_DISPLAY_NAMES: Readonly<Record<string, string>> = {
  "terrain-lit-baked-offsets": "CLOUDED TERRAIN OFFSETS",
  "terrain-lit-face-differences": "CLOUDED TERRAIN",
  "terrain-lit-interior-baked-offsets": "INTERIOR CLOUDED OFFSETS",
  "terrain-lit-interior-face-differences": "INTERIOR CLOUDED TERRAIN",
  "terrain-lit-cluster-baked-offsets": "CLUSTER CLOUDED OFFSETS",
  "terrain-lit-cluster-face-differences": "CLOUDED TERRAIN CLUSTERS",
  "terrain-lit-fin-baked-offsets": "FIN CLOUDED OFFSETS",
  "terrain-lit-fin-face-differences": "CLOUDED TERRAIN FINS",
};

/**
 * A plain lit form's cloudy form ({@link withCloudLight}): its bindings, uniforms and states, with
 * the clouds' rows to the fragment stage, their two textures and their sampler.
 *
 * @throws Error for a form this module does not name, or as {@link withCloudLight}.
 */
export function cloudLitMaterialSpec(plain: WgslMaterialSpec): WgslMaterialSpec {
  const displayName = CLOUDY_DISPLAY_NAMES[plain.name];
  if (displayName === undefined) {
    throw new Error(`the lit terrain's form ${plain.name} has no cloudy form`);
  }
  const b = TERRAIN_LIT_BINDINGS;
  const vertexWgsl = withCloudLight(plain.vertexWgsl);
  const fragmentWgsl =
    plain.fragmentWgsl === plain.vertexWgsl ? vertexWgsl : withCloudLight(plain.fragmentWgsl);
  return {
    ...plain,
    name: `${plain.name}-clouds`,
    displayName,
    vertexWgsl,
    fragmentWgsl,
    samplers: [
      ...plain.samplers,
      {
        name: CLOUD_TEXTURES.sampler,
        filter: "linear",
        address: "repeat",
        binding: b.cloudSampler,
      },
    ],
    textures: [
      ...(plain.textures ?? []),
      { name: CLOUD_TEXTURES.coverage, binding: b.cloudCoverage },
      { name: CLOUD_TEXTURES.noise, binding: b.cloudNoise, viewDimension: "3d" },
    ],
    storageBuffers: [
      ...(plain.storageBuffers ?? []),
      { name: CLOUD_TEXTURES.layer, binding: b.cloudLayer, stages: ["fragment"] },
    ],
  };
}

/** The lit material's cloudy form on a vertex path. */
export function terrainLitCloudMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  return cloudLitMaterialSpec(terrainLitMaterialSpec(path));
}

/** The interior patches' lit material's cloudy form on a vertex path (plan R14, R14.T1.a). */
export function terrainLitInteriorCloudMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  return cloudLitMaterialSpec(terrainLitInteriorMaterialSpec(path));
}

/** The interior patches' lit form on the cluster path's cloudy form (plan R14, R14.T2.a). */
export function terrainLitClusterCloudMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  return cloudLitMaterialSpec(terrainLitClusterMaterialSpec(path, false));
}

/** The edge patches' may-discard clusters' lit form's cloudy form (plan R14, R14.T1.g). */
export function terrainLitFinCloudMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  return cloudLitMaterialSpec(terrainLitFinMaterialSpec(path, false));
}

/** Every cloudy form on both vertex paths, for the catalogue (compiled in the smoke's part 26). */
export const TERRAIN_LIT_CLOUD_MATERIALS: ReadonlyArray<WgslMaterialSpec> = (
  ["baked-offsets", "face-differences"] as const
).flatMap((path) => [
  terrainLitCloudMaterialSpec(path),
  terrainLitInteriorCloudMaterialSpec(path),
  terrainLitClusterCloudMaterialSpec(path),
  terrainLitFinCloudMaterialSpec(path),
]);
