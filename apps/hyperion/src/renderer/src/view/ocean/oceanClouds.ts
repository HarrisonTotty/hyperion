/**
 * The sea under its body's clouds (plan R11, R11.T7.c; Design note 10): the `ocean` pass's cloudy
 * forms, the far sea's and the near sea's, which light a sea beneath the clouds as the lit terrain's
 * cloudy forms light the ground (`terrain/cloudLight.ts`), the landing's wiring of the sea (main's
 * ruling, 2026-11-19).
 *
 * @remarks
 * - **One read a source.** Each source's light takes `cloudGround.wgsl`'s
 *   `cloud_ground_light(p, s, source_radius)` at the sea's point once, p the point ÷ the clouds'
 *   datum radius: the cloud volume's shadow map where it holds the place for that source, else
 *   `cloudShadow.wgsl`'s `terrain_cloud_light`, the layer's tap. Its x multiplies the source's direct
 *   light, so the glint, the light into the water and the foam's all take the cloud's shadow; its y
 *   the sky's light from the source; and its z, the share of the source's direct light the cloud
 *   scatters down, adds μ₀ E T times it to the sky's, as the terrain's sky term does
 *   ({@link oceanCloudedLight}, the twin).
 * - **The meter's class** stays the clear sky's: a sea beneath a cloud is lit by the light the cloud
 *   scatters down, so it stays a lit body's for R07's meter wherever its source is up.
 * - **The plain forms stay as they were, byte for byte.** A cloudy form is its plain form's source
 *   with its sky line and its light's line replaced ({@link withOceanCloudLight}) and the clouds'
 *   field, optics, tap and shadow-map read composed after it with their bindings, so a sea whose
 *   body has no clouds drawn, or with the setting's `cloudShadows` off, is drawn by the plain form.
 * - **Bindings** at `@group(2)` 14 to 19, past both seas' own ({@link OCEAN_CLOUD_BINDINGS}): the
 *   body's cloud rows, its coverage, the noise tile, their linear, repeating sampler, the volume's
 *   shadow map and its rows, the rows and the map's to the fragment stage alone. The far form then
 *   binds 13 sampled textures and the near form 15, inside WebGPU's 16.
 * - **Each source's angular radius**, the tap's footprint, is its light's w (`oceanPassUniforms`).
 * - **The glare source** (`oceanPass.ts`' `OceanGlare`), formed on the CPU, takes the layer's CPU
 *   tap at the specular point (`CloudBodyGpu.reference`): the volume's shadow map is the GPU's
 *   alone, so under the volume the glare follows the layer's mean shadow (a lean, 2026-11-19).
 */
import type { CloudBodyGpu } from "../clouds/cloudLayer";
import { CLOUD_TEXTURES } from "../clouds/cloudLayer";
import type { CloudGroundLight } from "../clouds/cloudShadow";
import cloudFieldWgsl from "../clouds/shaders/cloudField.wgsl?raw";
import cloudGroundWgsl from "../clouds/shaders/cloudGround.wgsl?raw";
import cloudOpticsWgsl from "../clouds/shaders/cloudOptics.wgsl?raw";
import cloudShadowWgsl from "../clouds/shaders/cloudShadow.wgsl?raw";
import type { BufferHandle, TextureHandle, WgslMaterialSpec } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";

/** The cloudy forms' clouds in `@group(2)`, past the near sea's 11 to 13 (the module's remarks). */
export const OCEAN_CLOUD_BINDINGS = {
  cloudLayer: 14,
  cloudCoverage: 15,
  cloudNoise: 16,
  cloudSampler: 17,
  cloudShadowMap: 18,
  cloudShadowRows: 19,
} as const;

/** Both seas' sky line from a source, after which a cloudy form takes its source's clouds. */
const SKY_LINE =
  "    let sky = visible * atmosphere_sky_irradiance(height, mu0, latitude, slot);\n";

/** Both seas' light from a source, which a cloudy form takes through its clouds. */
const LIGHT = "OceanLight(light * transmitted, light * sky)";

/** {@link LIGHT} beneath the clouds: the direct light through the cloud, the sky's with its share. */
const CLOUDED_LIGHT = "OceanLight(light * transmitted * cloud.x, light * clouded_sky)";

/**
 * {@link SKY_LINE} with the clouds' one read at the sea's point `place` (m from the body's centre,
 * body-fixed) and the sky's light through the cloud with the light it scatters down.
 */
function cloudedSkyLines(place: string): string {
  return (
    SKY_LINE +
    `    let cloud = ocean_cloud_light(${place}, s, select(draw.sourceLight0.w, draw.sourceLight1.w, j == 1u));\n` +
    "    let clouded_sky = sky * cloud.y + cloud.z * max(mu0, 0.0) * transmitted;\n"
  );
}

/** The clouds' bindings in a cloudy form, as `cloudField.wgsl`'s and `cloudGround.wgsl`'s headers ask. */
const CLOUD_BINDINGS_WGSL = `
// R11's clouds over the sea (plan R11, R11.T7.c; \`ocean/oceanClouds.ts\`): the body's rows, its
// coverage texture, the noise tile and their linear, repeating sampler, and the volume's shadow map
// and its rows.
@group(2) @binding(${String(OCEAN_CLOUD_BINDINGS.cloudLayer)}) var<storage, read> cloud_layer : array<vec4f>;
@group(2) @binding(${String(OCEAN_CLOUD_BINDINGS.cloudCoverage)}) var cloud_coverage : texture_2d<f32>;
@group(2) @binding(${String(OCEAN_CLOUD_BINDINGS.cloudNoise)}) var cloud_noise : texture_3d<f32>;
@group(2) @binding(${String(OCEAN_CLOUD_BINDINGS.cloudSampler)}) var cloud_sampler : sampler;
@group(2) @binding(${String(OCEAN_CLOUD_BINDINGS.cloudShadowMap)}) var cloud_shadow_map : texture_2d<f32>;
@group(2) @binding(${String(OCEAN_CLOUD_BINDINGS.cloudShadowRows)}) var<storage, read> cloud_shadow_rows : array<vec4f>;
`;

/** The sea's read of its clouds: its point, m, ÷ the clouds' datum radius, as the ground's read takes it. */
const OCEAN_CLOUD_LIGHT_WGSL = `
// The clouds' light at a sea's point from the body's centre, body-fixed, m, towards the source \`sun\`
// of angular radius \`source_radius\`, rad: \`cloud_ground_light\` at the point ÷ the clouds' datum
// radius a, its three factors (\`cloudShadow.wgsl\`'s header).
fn ocean_cloud_light(point : vec3f, sun : vec3f, source_radius : f32) -> vec3f {
  return cloud_ground_light(point / cloud_row(CLOUD_ROW_NORM).y, sun, source_radius);
}
`;

/** `source` with `part` replaced by `by`, which it holds exactly once. */
function replacedOnce(source: string, part: string, by: string, what: string): string {
  const parts = source.split(part);
  if (parts.length !== 2) {
    throw new Error(
      `a sea's cloudy form needs its pass's ${what} once, found ${String(parts.length - 1)}`,
    );
  }
  return parts.join(by);
}

/**
 * A sea pass's source in its cloudy form (the module's remarks): its sky line followed by the
 * clouds' read at `place`, its light's line through the clouds, then the clouds' bindings, field,
 * optics, tap and shadow-map read appended.
 *
 * @param place - The WGSL expression of the sea's point from the body's centre, body-fixed, m.
 * @throws Error unless the sky line and the light each occur exactly once in `source`.
 */
export function withOceanCloudLight(source: string, place: string): string {
  const clouded = replacedOnce(
    replacedOnce(source, SKY_LINE, cloudedSkyLines(place), "sky line"),
    LIGHT,
    CLOUDED_LIGHT,
    "light",
  );
  return `${clouded}\n${CLOUD_BINDINGS_WGSL}\n${cloudFieldWgsl}\n${cloudOpticsWgsl}\n${cloudShadowWgsl}\n${cloudGroundWgsl}\n${OCEAN_CLOUD_LIGHT_WGSL}`;
}

/**
 * A sea pass's plain material's cloudy form ({@link withOceanCloudLight}): its bindings, uniforms
 * and states, with the clouds' sampler, their three textures and their two rows to the fragment
 * stage.
 *
 * @param place - As {@link withOceanCloudLight}'s.
 * @throws Error as {@link withOceanCloudLight}.
 */
export function oceanCloudMaterialSpec(
  plain: WgslMaterialSpec,
  place: string,
  displayName: string,
): WgslMaterialSpec {
  const b = OCEAN_CLOUD_BINDINGS;
  const vertexWgsl = withOceanCloudLight(plain.vertexWgsl, place);
  const fragmentWgsl =
    plain.fragmentWgsl === plain.vertexWgsl
      ? vertexWgsl
      : withOceanCloudLight(plain.fragmentWgsl, place);
  return {
    ...plain,
    name: `${plain.name} clouds`,
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
      { name: CLOUD_TEXTURES.shadowMap, binding: b.cloudShadowMap },
    ],
    storageBuffers: [
      ...(plain.storageBuffers ?? []),
      { name: CLOUD_TEXTURES.layer, binding: b.cloudLayer, stages: ["fragment"] },
      { name: CLOUD_TEXTURES.shadowRows, binding: b.cloudShadowRows, stages: ["fragment"] },
    ],
  };
}

/** What a cloudy form's draw binds of its body's clouds, beside its sea's own. */
export interface OceanCloudDrawBindings {
  readonly textures: Readonly<Record<string, TextureHandle>>;
  readonly storageBuffers: Readonly<Record<string, BufferHandle>>;
}

/**
 * The textures and rows a cloudy form's draw binds of `clouds`, its body's clouds this frame: the
 * volume's shadow map and rows where the frame draws the volume, else the map that holds nothing.
 */
export function oceanCloudDrawBindings(clouds: CloudBodyGpu): OceanCloudDrawBindings {
  return {
    textures: {
      [CLOUD_TEXTURES.coverage]: clouds.coverage,
      [CLOUD_TEXTURES.noise]: clouds.noise,
      [CLOUD_TEXTURES.shadowMap]: clouds.shadowMap,
    },
    storageBuffers: {
      [CLOUD_TEXTURES.layer]: clouds.layer,
      [CLOUD_TEXTURES.shadowRows]: clouds.shadowRows,
    },
  };
}

/** A source's light on the sea, per channel, lx: its direct light face-on and the sky's. */
export interface OceanSourceLight {
  /** The source's illuminance face-on through the air, eclipses included. */
  readonly sunLux: Rgb;
  /** The sky's irradiance on the horizontal from the source. */
  readonly skyLux: Rgb;
}

/**
 * A source's light on the sea beneath its clouds, the twin of the cloudy forms' (the module's
 * remarks): the direct light times the cloud's direct factor, and the sky's times its sky factor
 * with the share the cloud scatters down of the direct light on the horizontal added.
 *
 * @param clear - The source's light as the plain form takes it.
 * @param mu0 - The cosine of the source's zenith angle at the sea's point.
 */
export function oceanCloudedLight(
  clear: OceanSourceLight,
  cloud: CloudGroundLight,
  mu0: number,
): OceanSourceLight {
  const down = cloud.diffuse * Math.max(mu0, 0);
  const sun = clear.sunLux;
  const sky = clear.skyLux;
  return {
    sunLux: [sun[0] * cloud.direct, sun[1] * cloud.direct, sun[2] * cloud.direct],
    skyLux: [
      sky[0] * cloud.sky + down * sun[0],
      sky[1] * cloud.sky + down * sun[1],
      sky[2] * cloud.sky + down * sun[2],
    ],
  };
}
