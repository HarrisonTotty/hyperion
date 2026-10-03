/**
 * The view camera's limiting magnitude: the faintest star its peak pixel lifts above the noise
 * floor (plan R06, Design note 18, T13.a).
 *
 * @remarks
 * The CCD equation after Merline and Howell (1995, Experimental Astronomy 6, 163): a star is
 * detected when its peak pixel's signal S reaches k √(S + N_b), with S = f_pk Φ₀ A η t 10^(−0.4 V)
 * electrons and N_b the pixel's sky electrons plus dark current plus the read noise's variance,
 * σ_r² = σ_pre² + (σ_post S_base ÷ S_iso)², so that sensitivity acts as gain: it cuts the read
 * noise referred to the input, not the photons. The aperture is f ÷ N with f = (sensor width ÷ 2)
 * ÷ tan(fov ÷ 2), so the signal grows as f² as the view narrows. Φ₀ = 8.8 × 10⁹ photons s⁻¹ m⁻²
 * for V = 0 in V (Bessell, Castelli and Plez 1998's zero point). η is the Sun's, η☉; a star's own is
 * η☉ × 10^(−0.4 c) with c its camera band term, so a star is in the view when V + c is brighter
 * than the limit. The defaults give V 9.85–10.1 at 60° over μ 22.4–24, 11.5–11.75 at 30° and
 * 13.4–13.6 at 13° at high gain (9.4, 11.05 and 12.9 at base ISO), the Design note's figures as
 * re-computed with η☉ = 3.02 (decision-camera-eta.md; the fitted 2.956 is 0.02 mag shallower); checked against Vida et al. 2021's measured
 * limits for the Global Meteor Network's cameras (V 6.0 ± 0.5 measured, about 6.1–6.7 modelled).
 * Every figure is stated with its field of view. The parameters are one table,
 * {@link DEFAULT_VIEW_CAMERA}; open question 16 leaves them open.
 */

import type { ExposureTriple } from "../photometry/exposure";

/** The photon flux of a V = 0 star in the V band, photons s⁻¹ m⁻²: 8.8 × 10⁹ (Bessell et al. 1998). */
export const V0_PHOTON_FLUX_PER_S_M2 = 8.8e9;

/** One radian in arcseconds. */
const ARCSEC_PER_RAD = (180 / Math.PI) * 3_600;

/**
 * A view camera's sensor and detection rule (Design note 18).
 *
 * @remarks
 * η☉ is electrons per V-band-equivalent photon for the Sun's spectrum, not a quantum efficiency:
 * the default sensor is unfiltered back-illuminated silicon, QE(λ) = 0.60 (1 − e^(−α(λ) 16 µm))
 * over 400–1,100 nm with Green 2008's α, which gives η☉ ≈ 3.0 (2.956 in the fitted table); a star's spectrum moves it by the
 * wire's camera band term, −2.5 log₁₀(η ÷ η☉).
 */
export interface ViewCameraSensor {
  /** The sensor's width, m. */
  readonly widthM: number;
  /** Pixels across the sensor's width. */
  readonly widthPx: number;
  /** Electrons per V-band-equivalent photon for the Sun's spectrum, η☉ (`CAMERA_ETA_SUN`). */
  readonly etaSun: number;
  /** The read noise before the gain stage, e⁻ rms, σ_pre. */
  readonly readNoisePreE: number;
  /** The read noise after the gain stage at base sensitivity, e⁻ rms, σ_post. */
  readonly readNoisePostE: number;
  /** The base sensitivity, ISO, S_base. */
  readonly baseIso: number;
  /** The dark current, e⁻ s⁻¹ per pixel. */
  readonly darkCurrentEPerS: number;
  /** The fraction of a star's electrons in its peak pixel, f_pk. */
  readonly peakFraction: number;
  /** The detection threshold, signal over noise, k. */
  readonly threshold: number;
}

/**
 * The default view camera: a full-frame video camera of today (Design note 18).
 *
 * @remarks
 * 36 mm across 1,920 px (18.75 µm pixels); η☉ = 2.9557, the colour table's `CAMERA_ETA_SUN`
 * (R06.T3.c, the ATLAS9 grid at 5,772 K and log g 4.438), which
 * `packages/protocol/fixtures/camera_eta_sun.json` pins on both sides; σ_pre = 1.2 e⁻ and 5 e⁻ in all at base ISO
 * 100, so σ_post = √(5² − 1.2²) = 4.854 e⁻; f_pk = 0.35; k = 3. Design note 18 names no
 * dark current, and it is taken as 0: at 1/30 s a dark current of 0.1 e⁻ s⁻¹ adds 0.003 e⁻, a
 * thousandth of the sky's 6 e⁻ a pixel at 60° (R06 Risks, "The camera model's defaults").
 */
export const DEFAULT_VIEW_CAMERA: ViewCameraSensor = {
  widthM: 0.036,
  widthPx: 1_920,
  etaSun: 2.955707565385169,
  readNoisePreE: 1.2,
  readNoisePostE: Math.sqrt(5 ** 2 - 1.2 ** 2),
  baseIso: 100,
  darkCurrentEPerS: 0,
  peakFraction: 0.35,
  threshold: 3,
};

/** The V surface brightness of a luminance, mag arcsec⁻²: μ = −2.5 log₁₀ B + 12.58 (Crumey 2014). */
export function surfaceBrightnessV(luminanceCdM2: number): number {
  return -2.5 * Math.log10(luminanceCdM2) + 12.58;
}

/** The parts of a camera's limit, for a readout or a test. */
export interface CameraLimitParts {
  /** The limiting V magnitude. */
  readonly limitV: number;
  /** The sky's electrons in a pixel. */
  readonly skyElectrons: number;
  /** The read noise, e⁻ rms. */
  readonly readNoiseE: number;
  /** The peak pixel's electrons from a V = 0 star. */
  readonly v0PeakElectrons: number;
}

/**
 * A view camera's limiting V magnitude, with its parts.
 *
 * @param fovDeg - The view's horizontal field of view, degrees, in (0, 180).
 * @param backgroundCdM2 - The sky's luminance behind the stars, cd/m², 0 or more (the band
 *   texel's).
 * @throws RangeError for a non-finite or non-positive input.
 */
export function cameraLimitParts(
  sensor: ViewCameraSensor,
  exposure: ExposureTriple,
  fovDeg: number,
  backgroundCdM2: number,
): CameraLimitParts {
  for (const [name, value] of [
    ["aperture", exposure.aperture],
    ["shutter", exposure.shutterS],
    ["ISO", exposure.iso],
  ] as const) {
    if (!(Number.isFinite(value) && value > 0)) {
      throw new RangeError(`a camera limit needs a positive ${name}, not ${value}`);
    }
  }
  // A black background is physical: the sky adds nothing and read noise sets the limit.
  if (!(Number.isFinite(backgroundCdM2) && backgroundCdM2 >= 0)) {
    throw new RangeError(`a camera limit needs a background of 0 or more, not ${backgroundCdM2}`);
  }
  if (!(fovDeg > 0 && fovDeg < 180)) {
    throw new RangeError(`a camera limit needs a field of view in (0°, 180°), not ${fovDeg}`);
  }
  const focalM = sensor.widthM / 2 / Math.tan(((fovDeg / 2) * Math.PI) / 180);
  const apertureM = focalM / exposure.aperture;
  const areaM2 = Math.PI * (apertureM / 2) ** 2;
  const collected = areaM2 * sensor.etaSun * exposure.shutterS;
  const pixelArcsec = (sensor.widthM / sensor.widthPx / focalM) * ARCSEC_PER_RAD;
  const skyPhotonsPerArcsec2 =
    backgroundCdM2 > 0
      ? V0_PHOTON_FLUX_PER_S_M2 * 10 ** (-0.4 * surfaceBrightnessV(backgroundCdM2))
      : 0;
  const skyElectrons = skyPhotonsPerArcsec2 * pixelArcsec ** 2 * collected;
  const readNoiseE = Math.sqrt(
    sensor.readNoisePreE ** 2 + ((sensor.readNoisePostE * sensor.baseIso) / exposure.iso) ** 2,
  );
  const floor = skyElectrons + sensor.darkCurrentEPerS * exposure.shutterS + readNoiseE ** 2;
  const k = sensor.threshold;
  // S = k √(S + N_b), solved for S.
  const needed = (k * k + Math.sqrt(k ** 4 + 4 * k * k * floor)) / 2;
  const v0PeakElectrons = sensor.peakFraction * V0_PHOTON_FLUX_PER_S_M2 * collected;
  return {
    limitV: -2.5 * Math.log10(needed / v0PeakElectrons),
    skyElectrons,
    readNoiseE,
    v0PeakElectrons,
  };
}

/**
 * A view camera's limit for a Sun-coloured star, V (Design note 18): a star is in the view when V
 * plus its camera band term is brighter. It is the `camera_limit_v` a `sky` request sends, clamped
 * there at `MAX_CUT_V`.
 *
 * @param fovDeg - The view's horizontal field of view, degrees, in (0, 180).
 * @param backgroundCdM2 - The sky's luminance behind the stars, cd/m², 0 or more.
 * @throws RangeError as {@link cameraLimitParts} does.
 */
export function cameraLimitV(
  sensor: ViewCameraSensor,
  exposure: ExposureTriple,
  fovDeg: number,
  backgroundCdM2: number,
): number {
  return cameraLimitParts(sensor, exposure, fovDeg, backgroundCdM2).limitV;
}
