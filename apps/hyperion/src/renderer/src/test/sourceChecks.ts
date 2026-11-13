/**
 * Whether a drawn medium's terms' sources reproduce it (plan R08, R08.T14.c): the spectral
 * column's optics at the render channels against each drawn term's coefficients and phase, for the
 * kept media's and the assembly's tests.
 */

import {
  type AtmosphereMedium,
  CHANNEL_WAVELENGTHS_NM,
  type PhaseFunction,
} from "../view/atmosphere/medium";
import {
  CHANNEL_SOURCE_MOMENT_ORDER,
  cornetteShanksMoments,
  type SpectralPhase,
  type SpectralSource,
  spectralColumnOf,
  spectralOptics,
} from "../view/atmosphere/thick/spectralColumn";

/** Whether two arrays hold the same numbers, entry by entry, to the bit. */
function sameEntries(values: ArrayLike<number>, expected: ArrayLike<number>): boolean {
  if (values.length !== expected.length) {
    return false;
  }
  for (let i = 0; i < values.length; i += 1) {
    if (!Object.is(values[i], expected[i])) {
      return false;
    }
  }
  return true;
}

/**
 * Whether a source's phase at a channel's wavelength is the drawn phase in that channel, to the
 * bit: Rayleigh's ρ; a table's values and forward delta, read as its channel 0; Cornette–Shanks's
 * closed-form moments with no forward delta; none.
 */
function phaseMatches(drawn: PhaseFunction, phase: SpectralPhase, channel: 0 | 1 | 2): boolean {
  let matches: boolean;
  switch (drawn.kind) {
    case "rayleigh":
      matches =
        phase.kind === "rayleigh" && Object.is(phase.depolarisation, drawn.depolarisation[channel]);
      break;
    case "tabulated":
      matches =
        phase.kind === "tabulated" &&
        sameEntries(phase.table.u, drawn.table.u) &&
        sameEntries(phase.table.values[0], drawn.table.values[channel]) &&
        Object.is(phase.table.forwardPeak[0], drawn.table.forwardPeak[channel]);
      break;
    case "cornette-shanks":
      matches =
        phase.kind === "moments" &&
        phase.forwardDelta === 0 &&
        sameEntries(
          phase.moments,
          cornetteShanksMoments(drawn.asymmetry, CHANNEL_SOURCE_MOMENT_ORDER),
        );
      break;
    case "none":
      matches = phase.kind === "none";
      break;
  }
  return matches;
}

/**
 * Where a medium's sources fail to reproduce it at the render channels: one line a term, channel and
 * quantity whose spectral optics (`thick/spectralColumn.ts`'s `spectralOptics` of
 * `spectralColumnOf(medium, sources)` at `CHANNEL_WAVELENGTHS_NM`) are not the drawn term's to
 * the bit; none where every coefficient and phase is.
 *
 * @throws RangeError as `spectralColumnOf` and `spectralOptics` refuse the sources.
 */
export function sourceMisses(
  medium: AtmosphereMedium,
  sources: ReadonlyArray<SpectralSource>,
): ReadonlyArray<string> {
  const optics = spectralOptics(spectralColumnOf(medium, sources), CHANNEL_WAVELENGTHS_NM);
  const misses: string[] = [];
  for (const channel of [0, 1, 2] as const) {
    for (const [t, term] of medium.terms.entries()) {
      const own = optics[channel]?.[t];
      if (own === undefined) {
        misses.push(`${term.name} has no optics in channel ${channel}`);
        continue;
      }
      if (!Object.is(own.scatteringPerM, term.scattering[channel])) {
        misses.push(
          `${term.name}'s scattering in channel ${channel}: ${own.scatteringPerM} for ${term.scattering[channel]}`,
        );
      }
      if (!Object.is(own.absorptionPerM, term.absorption[channel])) {
        misses.push(
          `${term.name}'s absorption in channel ${channel}: ${own.absorptionPerM} for ${term.absorption[channel]}`,
        );
      }
      if (!phaseMatches(term.phase, own.phase, channel)) {
        misses.push(`${term.name}'s ${term.phase.kind} phase in channel ${channel}`);
      }
    }
  }
  return misses;
}
