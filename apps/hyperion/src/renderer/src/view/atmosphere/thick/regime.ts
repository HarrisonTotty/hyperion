/**
 * The thick regime's thresholds (plan R08, Design note 9; Provides, "Thick atmospheres").
 *
 * @remarks
 * R08.T5.b created this file for the deck split's optical depth, which its stand-in label reads
 * (Design note 12). R08.T10.a adds the provisional thick line, {@link PROVISIONAL_THICK_OPTICAL_DEPTH},
 * which R08.T13's `classifyRegime` and `THICK_MS_BOUNDARY` replace. Its imports stay light,
 * `medium.ts` alone, so that the render thread can test a kept scene's prebuilt medium without the
 * optics data.
 */

import type { Rgb } from "../../photometry/toneCurve";
import { type AtmosphereMedium, columnLengthM, extinction } from "../medium";

/**
 * The vertical optical depth at 550 nm above which a body-wide cloud deck splits the atmosphere in
 * two, Hillaire's tables above it and the deck's bake below (Design note 9).
 *
 * @remarks
 * The rendering brainstorm's figure ("a cloud deck of optical depth above about 10 splits the
 * atmosphere in two", its "Atmosphere" section and open question 3). The stand-in label takes the
 * same threshold: a stand-in material lying deeper than it carries no label
 * (`decision-r08-licences.md` row 5; `materials/materials.ts`). Light from beneath is not nil
 * there: a conservative deck of τ 10 transmits 31–47% diffusely for g 0.7–0.85, by the two-stream
 * T = 1 ÷ (1 + ¾(1 − g)τ). The deck's bake (R08.T15) carries that light.
 */
export const CLOUD_DECK_SPLIT_OPTICAL_DEPTH = 10;

/**
 * The provisional thick line: a medium whose vertical optical depth from the ground to the top
 * reaches 1 on its worst channel, over every term but the absorbers, is drawn by the analytic
 * term under `ATMOSPHERE: APPROXIMATE` (plan R08, Design note 9's routing; R08.T10.a,
 * `decision-r08-t10a-leans.md` item 1). Provisional: R08.T13's `classifyRegime` retires it.
 *
 * @remarks
 * 1 is the column's optical thickness, the conventional line between thin and thick, not a
 * measured boundary. For Rayleigh air it falls at about 3.9 Earth columns (Earth's Rayleigh τ is
 * about 0.26 at 433.6 nm, `earth.ts`'s τ_R(550 nm) = 0.0969 scaled by λ^−4.08), inside the fifty
 * times Earth's density Hillaire tested to (Design note 9) and past the 3 Earth columns at which
 * R08.T4.a measured the thin path's three-channel transmittance 0.0151 off in u′v′ at low sun.
 * Absorbers are left out: the analytic term fails in multiple scattering,
 * which absorption alone makes none of, and their depth is each light's curve of growth, so that
 * counting it would make the label depend on the body's stars.
 */
export const PROVISIONAL_THICK_OPTICAL_DEPTH = 1;

/**
 * A medium's vertical optical depth per channel from the ground to its top over every term but
 * the absorbers, each at its whole extinction (an aerosol's forward peak included): the depth the
 * provisional thick line reads.
 *
 * @remarks
 * Along the zenith from the ground a ray's height is its distance, whatever the ground's radius,
 * so this is Σ (scattering + absorption) ∫₀^top ρ dh, in closed form by `medium.ts`'s
 * `columnLengthM`: `opticalDepth.ts`'s `opticalDepthToTop(medium, R, R, 1)`, which takes the same
 * integral by Simpson's rule (held equal to it to 10⁻⁶ by the tests), without that module's
 * optics data.
 */
export function verticalOpticalDepth(medium: AtmosphereMedium): Rgb {
  let red = 0;
  let green = 0;
  let blue = 0;
  for (const term of medium.terms) {
    if (term.absorber !== undefined) {
      continue;
    }
    const lengthM = columnLengthM(term.density, medium.topHeightM);
    const [r, g, b] = extinction(term);
    red += r * lengthM;
    green += g * lengthM;
    blue += b * lengthM;
  }
  return [red, green, blue];
}

/**
 * Whether a medium is thick by the provisional line: its {@link verticalOpticalDepth} reaches
 * {@link PROVISIONAL_THICK_OPTICAL_DEPTH} on its worst channel, so that it carries
 * `atmosphereApproximate` until R08.T13's gate. Every medium the cache draws is tested, a kept
 * scene's fixtures among them.
 */
export function provisionalThick(medium: AtmosphereMedium): boolean {
  return Math.max(...verticalOpticalDepth(medium)) >= PROVISIONAL_THICK_OPTICAL_DEPTH;
}
