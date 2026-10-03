/**
 * The client's eye observer (plan R06, Design notes 2–4): the mirror of the sim's
 * `sky::eye::EyeObserver` defaults, which a `sky` request's `EyeDto` sends and R07's glare spread
 * reads (as its `EyeObserver`, age and pigmentation).
 */

import type { EyeDto } from "@hyperion/protocol";

/** An eye: Crumey's (2014) field factor F and the CIE 146:2002 glare's age and pigmentation. */
export interface SkyEyeObserver {
  /** The field factor F, at least 1: every limit moves by −2.5 log₁₀ F. */
  readonly fieldFactor: number;
  /** The observer's age, years. */
  readonly ageYears: number;
  /** The eye's pigmentation p, 0 to 1.2 (0.5 brown). */
  readonly pigmentation: number;
}

/**
 * The default eye: F = 1.4, 25 years, pigmentation 0.5, the sim's `EyeObserver::default()`
 * (Crumey 2014's F for a typical observer; CIE 146:2002's young brown eye).
 */
export const DEFAULT_EYE_OBSERVER: SkyEyeObserver = {
  fieldFactor: 1.4,
  ageYears: 25,
  pigmentation: 0.5,
};

/** An eye as a `sky` request carries it. */
export function eyeDto(eye: SkyEyeObserver): EyeDto {
  return { field_factor: eye.fieldFactor, age_years: eye.ageYears, pigmentation: eye.pigmentation };
}
