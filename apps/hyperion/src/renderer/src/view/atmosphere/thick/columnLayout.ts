/**
 * A drawn medium's spectral column laid out from its terms' sources (plan R08, R08.T14.c): the
 * layout alone, apart from the optics that read a column at a bake bin (`spectralColumn.ts`, which
 * re-exports it), so that the render thread's atmosphere cache forms a thick bake's request without
 * loading Mie, the MMF or the phase files, as `thick/regime.ts` holds the router's reason apart
 * from the assembly.
 */

import type { AtmosphereMedium } from "../medium";
import type { SpectralColumn, SpectralSource, SpectralTerm } from "./spectralColumn";

/**
 * A drawn medium's spectral column: its name, top, ground, pressure profile and each term's name
 * and density, with `sources[i]` the source of `medium.terms[i]` (R08.T14.c's hand-off: the
 * assembly gives the sources beside its terms).
 *
 * @throws RangeError for a source count other than the terms', an absorber term without an
 *   absorber source of its kind, or an absorber source on a term that absorbs by none.
 */
export function spectralColumnOf(
  medium: AtmosphereMedium,
  sources: ReadonlyArray<SpectralSource>,
): SpectralColumn {
  if (sources.length !== medium.terms.length) {
    throw new RangeError(
      `medium ${medium.name} has ${medium.terms.length} terms, not ${sources.length} sources`,
    );
  }
  const terms = medium.terms.map((term, i): SpectralTerm => {
    const source = sources[i];
    if (source === undefined) {
      throw new RangeError(`medium ${medium.name}'s term ${term.name} has no source`);
    }
    const absorbs = term.absorber?.kind;
    const given = source.kind === "absorber" ? source.amount.kind : undefined;
    if (absorbs !== given) {
      throw new RangeError(
        `term ${term.name} absorbs by ${absorbs ?? "no curve"}, its source by ${given ?? "none"}`,
      );
    }
    return { name: term.name, density: term.density, source };
  });
  return {
    name: medium.name,
    topHeightM: medium.topHeightM,
    groundAlbedo: medium.groundAlbedo,
    terms,
    ...(medium.pressureBar === undefined ? {} : { pressureBar: medium.pressureBar }),
  };
}
