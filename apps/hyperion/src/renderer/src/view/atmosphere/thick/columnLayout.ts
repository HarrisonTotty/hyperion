/**
 * A drawn medium's spectral column laid out from its terms' sources (plan R08, R08.T14.c): the
 * layout alone, apart from the optics that read a column at a bake bin (`spectralColumn.ts`, which
 * re-exports it), so that the render thread's atmosphere cache forms a thick bake's request without
 * loading Mie, the MMF or the phase files, as `thick/regime.ts` holds the router's reason apart
 * from the assembly; and whether a column needs the polarisation correction by its sources alone
 * (R08.T14.f), which the cache reads to ask for it.
 */

import type { AtmosphereMedium } from "../medium";
import type { MediumTemperature } from "../thermalSource";
import type { SpectralColumn, SpectralSource, SpectralTerm } from "./spectralColumn";

/**
 * A drawn medium's spectral column: its name, top, ground, pressure profile and each term's name
 * and density, with `sources[i]` the source of `medium.terms[i]` (R08.T14.c's hand-off: the
 * assembly gives the sources beside its terms), and the column's `temperature` where it emits
 * (R08.T18.b).
 *
 * @throws RangeError for a source count other than the terms', an absorber term without an
 *   absorber source of its kind, or an absorber source on a term that absorbs by none.
 */
export function spectralColumnOf(
  medium: AtmosphereMedium,
  sources: ReadonlyArray<SpectralSource>,
  temperature?: MediumTemperature,
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
    ...(temperature === undefined ? {} : { temperature }),
  };
}

/**
 * Whether a channel source scatters by a matrix whose b₁ is not 0, read by its phase: Rayleigh's
 * does wherever it scatters, as a molecular term's; a table's where it scatters in green and its
 * matrix's b₁ there is not 0, as an aerosol's is read; Cornette–Shanks's and none's never (the
 * correction takes them as depolarisers).
 */
function channelSourcePolarises(source: Extract<SpectralSource, { kind: "channels" }>): boolean {
  const { phase } = source;
  let polarises: boolean;
  switch (phase.kind) {
    case "rayleigh":
      polarises = source.scattering.some((s) => s > 0);
      break;
    case "tabulated":
      polarises =
        source.scattering[1] > 0 && phase.table.matrix?.b1[1].some((value) => value !== 0) === true;
      break;
    case "cornette-shanks":
    case "none":
      polarises = false;
      break;
  }
  return polarises;
}

/**
 * Whether a column needs the polarisation correction, by its sources alone (plan R08, R08.T14.f;
 * Design note 10: "every world with a term whose b₁ ≠ 0, thin or thick"): `true` for a molecular or
 * Thomson term with density, by Rayleigh's matrix (b₁ = −¾Δ sin²Θ, Δ > 0 for every ρ < 1), so every
 * world with air, or for a channel source that polarises; `false` for a column with neither and no
 * aerosol of any extinction; and `null` where only an aerosol's optics, its Mie, MMF or phase file
 * at the green channel, can tell (`polarisation.ts`'s `needsPolarisationCorrection`, which reads
 * this first), which the render thread does not load.
 */
export function correctionBySources(column: SpectralColumn): boolean | null {
  let aerosols = false;
  for (const { source } of column.terms) {
    switch (source.kind) {
      case "molecular":
        if (source.numberDensityPerM3 > 0) {
          return true;
        }
        break;
      case "thomson":
        if (source.electronDensityPerM3 > 0) {
          return true;
        }
        break;
      case "aerosol":
        aerosols ||= source.extinction550PerM > 0;
        break;
      case "absorber":
        break;
      case "channels":
        if (channelSourcePolarises(source)) {
          return true;
        }
        break;
    }
  }
  return aerosols ? null : false;
}
