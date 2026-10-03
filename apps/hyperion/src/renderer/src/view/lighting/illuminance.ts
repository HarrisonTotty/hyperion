/**
 * The illuminance at a body from each star of its system, per display channel (plan R07, Design
 * note 4).
 *
 * @remarks
 * A host of radius R at distance d has angular radius ρ = asin(R ÷ d) and lights a surface face-on
 * to it with E_c = π L̄_c sin²ρ per channel, L̄_c being R06's mean disc luminance
 * (`HostDiscDto.mean_luminance_cd_m2`, never the central one). R06 builds each channel as the
 * photopic mean times the star's linear Rec. 709 colour at unit luminance, so the channels'
 * Rec. 709 luminance is the photopic illuminance, which equals 2.54 µlx × `lux_per_v0` ×
 * 10^(−0.4 V) for the star's apparent V (no extinction inside a system). The star is taken at the
 * body's own emitted time: neglecting the star-to-body light time turns the light by the star's
 * reflex speed over c, some 4 × 10⁻⁸ rad for a Sun pulled by a Jupiter.
 */
import type { HostDiscDto } from "@hyperion/protocol";

import type { Rgb } from "../photometry/toneCurve";

/**
 * A star lighting a body with less than this fraction of the brightest star's photopic illuminance
 * there does not light it (Design note 4); R08 applies the same constant to its suns.
 */
export const STAR_CUT_RELATIVE = 1e-4;

/**
 * The Rec. 709 luminance row R06 builds the disc's channels with (r, g, b), as
 * `hyperion_sim::tables::star_colour::LUMINANCE_RGB` holds it.
 */
export const CHANNEL_LUMINANCE: Rgb = [
  0.212_639_005_871_510_24, 0.715_168_678_767_755_9, 0.072_192_315_360_733_71,
];

/**
 * The illuminance at a point face-on to a host star, per display channel (r, g, b), lx.
 *
 * @param distanceM - The distance from the star's centre, m; inside the star the whole sky is the
 *   star (ρ = π/2).
 */
export function starIlluminance(disc: HostDiscDto, distanceM: number): Rgb {
  const sinRho = Math.min(disc.radius_m / distanceM, 1);
  const solidFactor = Math.PI * sinRho * sinRho;
  const [b, v, r] = disc.mean_luminance_cd_m2;
  return [solidFactor * r, solidFactor * v, solidFactor * b];
}

/** The photopic illuminance of a per-channel illuminance, lx: its Rec. 709 luminance. */
export function photopicIlluminance(illuminance: Rgb): number {
  return (
    CHANNEL_LUMINANCE[0] * illuminance[0] +
    CHANNEL_LUMINANCE[1] * illuminance[1] +
    CHANNEL_LUMINANCE[2] * illuminance[2]
  );
}

/**
 * The indices of the stars that light a body: those whose photopic illuminance there is at least
 * {@link STAR_CUT_RELATIVE} of the brightest's, in their given order.
 */
export function shiningStars(illuminances: ReadonlyArray<Rgb>): number[] {
  const photopic = illuminances.map(photopicIlluminance);
  const brightest = Math.max(0, ...photopic);
  const cut = brightest * STAR_CUT_RELATIVE;
  return photopic.flatMap((value, index) => (brightest > 0 && value >= cut ? [index] : []));
}
