/**
 * `saturn-envelope`, the reference tracer's second absorbing envelope case (plan R08, R08.T16.a.2;
 * research-leans-43 §2): the hand Saturn's medium (`giantFixtures.ts`'s `handSaturnDetail`, the
 * sim's own Saturn as the golden prints it), built as `jupiter-envelope` is
 * (`jupiterEnvelopeCase.ts`'s `handEnvelopeCase`), for `hyperion-fit atmosphere-reference` to
 * trace and the envelope route's twin to be read against, on the deep side of the class.
 *
 * @remarks
 * - **The medium** is the hand Saturn's as its envelope assembles it, cut at green τ 20 from its
 *   top over a black ground, its NH₄SH and NH₃ decks at 1 µm, its NH₃ haze and its chromophore
 *   (`giantFixtures.ts`'s `coldGiantHazes`, 30's rules) above its floor, 257,671 m above the cut.
 *   Above the floor it reads τ 9.16/10.08/12.33, ω̄ 0.993/0.983/0.960 and ḡ 0.723/0.656/0.526
 *   (`thick/regime.ts`'s `regimeChannels`), its chromophore's τ (1 − ω̄) 0.065–0.490. Levels under
 *   a millimetre apart are each held 1 cm above the one below it.
 * - **The figure** is a sphere through the cut, the drawn tables' ground radius raised by it.
 * - **The geometries** are `jupiter-envelope`'s: the disc and the limb from orbit. The medium's
 *   scale height at its ground is 281.7 km (`medium.ts`'s `localScaleHeightM`), so that 3 H and
 *   10 H stand past its top at 833.8 km and the limb keeps 0.1–1 H.
 *
 * Its Mie runs for a few seconds; the case is built once a module and held.
 */

import { type EnvelopeCase, handEnvelopeCase } from "./jupiterEnvelopeCase";
import { handSaturnDetail } from "./giantFixtures";

/** The case's name: its files are `saturn-envelope.case.json` and `.reference.json`. */
export const SATURN_ENVELOPE_CASE = "saturn-envelope";

/** The case once built, for the module's life. */
let built: EnvelopeCase | undefined;

/**
 * `saturn-envelope` (the module's remarks), built on the first call and held.
 *
 * @throws Error if the hand Saturn draws no envelope's medium: a broken fixture.
 */
export function saturnEnvelopeCase(): EnvelopeCase {
  built ??= handEnvelopeCase(SATURN_ENVELOPE_CASE, handSaturnDetail());
  return built;
}
