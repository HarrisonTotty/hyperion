/**
 * The clouds' light on the ground below them (plan R11, R11.T7.b; Design note 10): what of a source's
 * direct light the layer lets through to a place, and what it turns into diffuse light there; the
 * CPU reference of `shaders/cloudShadow.wgsl`'s `terrain_cloud_light`.
 *
 * @remarks
 * Shadows take only the direct part, e^(−τ ÷ μ₀) of the covered share with the unscaled τ, into the
 * terrain's direct sun (R10's `terrain_shadow_factor`, Design note 11); the diffuse part lights the
 * ground through the sky's term (R10's `terrain_sky_factor` on R08's `atmosphere_sky_irradiance`),
 * since multiplying the total into the sun would draw a sharp, half-bright sun under an overcast
 * (Frostbite bakes cloud shadow the same way, Hillaire 2016 §5.9). The two hooks take the one ray's
 * crossing of the layer, so they are one function, `terrain_cloud_light(p, sun, radius)`, whose three
 * parts the terrain applies per source (lean, 2026-11-02):
 *
 * - **direct**, (1 − f) + f T_dir, on the source's direct light;
 * - **sky**, (1 − f) + f (T_dir + T_diff), on the sky's light, which reaches the ground through the
 *   cloud as the beam does;
 * - **diffuse**, f T_diff, the share of the source's direct light on a horizontal surface above the
 *   layer, μ₀ E T, that the cloud scatters down, added to the sky's light.
 *
 * So under a whole cover the ground's irradiance is T_total times the clear sky's, its sun and its
 * sky, the source's transmittance to the layer taken as to the ground.
 *
 * The tap ({@link cloudLightAt}) is where the ray from the place towards the source leaves the
 * layer's shell, the field there over the source's disc seen across that distance, so that a cloud's
 * shadow is as soft as its penumbra; a place on or above the shell keeps {@link CLEAR_GROUND}.
 */
import type { Vec3 } from "../../geometry/vec3";
import { type CloudFieldParams, cloudFieldAt, type CoverageTexels } from "./cloudDensity";
import type { CloudShell } from "./cloudLayer";
import { deltaEddington } from "./cloudOptics";
import type { CloudOptics } from "./condensates";

/** What of a source's light a place below the layer keeps: each part's factor. */
export interface CloudGroundLight {
  /** On the source's direct light. */
  readonly direct: number;
  /** On the sky's diffuse light. */
  readonly sky: number;
  /** The source's direct light on a horizontal surface above the layer that it scatters down. */
  readonly diffuse: number;
}

/** No cloud: the ground keeps every part, and gains no diffuse light. */
export const CLEAR_GROUND: CloudGroundLight = { direct: 1, sky: 1, diffuse: 0 };

/**
 * The least zenith cosine the layer's optics are taken at for a source on or below its horizon,
 * `cloudShadow.wgsl`'s CLOUD_SHADOW_LEAST_MU and `cloudLayer.wgsl`'s CLOUD_LEAST_MU.
 */
export const CLOUD_LEAST_MU = 0.02;

/**
 * The ground light under a footprint of covered share `fraction` and in-cloud τ `inCloudTau`, the
 * source at zenith cosine `mu0` against the layer's normal.
 */
export function cloudGroundLight(
  fraction: number,
  inCloudTau: number,
  optics: CloudOptics,
  mu0: number,
): CloudGroundLight {
  const f = Math.min(Math.max(fraction, 0), 1);
  if (!(f > 0)) {
    return CLEAR_GROUND;
  }
  const slab = deltaEddington({
    tau: Math.max(inCloudTau, 0),
    asymmetry: optics.asymmetry,
    singleScatterAlbedo: 1 - optics.coAlbedo,
    mu0: Math.min(Math.max(mu0, CLOUD_LEAST_MU), 1),
  });
  const diffuse = Math.max(slab.diffuseTransmittance, 0);
  return {
    direct: 1 - f + f * slab.directTransmittance,
    sky: 1 - f + f * (slab.directTransmittance + diffuse),
    diffuse: f * diffuse,
  };
}

/** The least footprint of a shadow's tap, m (`cloudShadow.wgsl`'s CLOUD_SHADOW_LEAST_FOOTPRINT_M). */
export const CLOUD_SHADOW_LEAST_FOOTPRINT_M = 1;

/** A body's clouds as the ground's tap reads them: its field, its shell and its optics. */
export interface CloudLightInput {
  readonly coverage: CoverageTexels;
  readonly noise: Uint8Array;
  readonly params: CloudFieldParams;
  readonly shell: CloudShell;
  readonly optics: CloudOptics;
  /** a, the datum's equatorial radius, m, the unit of the place. */
  readonly datumRadiusM: number;
}

/**
 * The ground light at a place under a body's clouds from a source (the module's remarks), the
 * reference of `terrain_cloud_light`.
 *
 * @param placeOverA - The place from the body's centre ÷ a, body-fixed, as the terrain gives it.
 * @param sun - The unit direction to the source, body-fixed.
 * @param sourceRadiusRad - The source's angular radius, rad.
 */
export function cloudLightAt(
  input: CloudLightInput,
  placeOverA: Vec3,
  sun: Vec3,
  sourceRadiusRad: number,
): CloudGroundLight {
  const { shell, datumRadiusM: a } = input;
  const k = shell.equatorialRadiusM / shell.polarRadiusM;
  const outer = shell.equatorialRadiusM / a;
  const sq = { x: placeOverA.x, y: placeOverA.y, z: placeOverA.z * k };
  const sd = { x: sun.x, y: sun.y, z: sun.z * k };
  const qa = sd.x * sd.x + sd.y * sd.y + sd.z * sd.z;
  const qb = sq.x * sd.x + sq.y * sd.y + sq.z * sd.z;
  const qc = sq.x * sq.x + sq.y * sq.y + sq.z * sq.z - outer * outer;
  if (!(qc < 0)) {
    return CLEAR_GROUND;
  }
  const t = (-qb + Math.sqrt(Math.max(qb * qb - qa * qc, 0))) / qa;
  const crossing = {
    x: (placeOverA.x + t * sun.x) * a,
    y: (placeOverA.y + t * sun.y) * a,
    z: (placeOverA.z + t * sun.z) * a,
  };
  const n = { x: crossing.x, y: crossing.y, z: crossing.z * k * k };
  const length = Math.hypot(n.x, n.y, n.z);
  const mu0 = Math.min(
    Math.max((n.x * sun.x + n.y * sun.y + n.z * sun.z) / length, CLOUD_LEAST_MU),
    1,
  );
  const footprintM = Math.max((2 * sourceRadiusRad * t * a) / mu0, CLOUD_SHADOW_LEAST_FOOTPRINT_M);
  const field = cloudFieldAt(input.coverage, input.noise, input.params, crossing, footprintM);
  return cloudGroundLight(field.fraction, field.inCloudTau, input.optics, mu0);
}
