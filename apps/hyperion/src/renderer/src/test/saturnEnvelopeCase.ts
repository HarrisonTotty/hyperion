/**
 * `saturn-envelope`, the reference tracer's second absorbing envelope case (plan R08, R08.T16.a.2;
 * research-leans-43 §2): the hand Saturn's medium (`giantFixtures.ts`'s `handSaturnDetail`, the
 * sim's own Saturn as the golden prints it), built as `jupiter-envelope` is
 * (`jupiterEnvelopeCase.ts`'s `handEnvelopeCase`), for `hyperion-fit atmosphere-reference` to
 * trace and the envelope route's twin to be read against, on the deep side of the class.
 *
 * @remarks
 * - **The medium** is the hand Saturn's as its envelope assembles it, cut at green τ 20 from its
 *   top over a black ground, its H₂O, NH₄SH and NH₃ decks at 1 µm and the fixture's chromophore
 *   haze above its floor, 126,066 m above the cut. Above the floor it reads, at generator version
 *   27, τ 6.35/10.35/23.01, ω̄ 0.996/0.994/0.992 and ḡ 0.282/0.162/0.070 (`thick/regime.ts`'s
 *   `regimeChannels`): deeper in blue than `jupiter-envelope`'s 16.67, past `thick/routing.ts`'s
 *   `ENVELOPE_ROUTE_RANGE`, its chromophore's τ (1 − ω̄) 0.025–0.187. Its levels at its floor and
 *   its decks' bases stand 0.30–0.83 mm apart, each held 1 cm above the one below it: the H₂O
 *   sheet loses 0.38% of its column (τ 0.0024), NH₄SH's 0.062% and NH₃'s 0.036%, the other terms
 *   under 10⁻⁶.
 * - **The figure** is a sphere through the cut, the drawn tables' ground radius raised by it.
 * - **The geometries** are `jupiter-envelope`'s: the disc and the limb from orbit. The medium's
 *   scale height at its ground is 202.1 km (`medium.ts`'s `localScaleHeightM`), so that 10 H
 *   stands past its top at 911.8 km and the limb keeps 0.1–3 H.
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
