/**
 * The render channels' fitted wavelengths as the renderer reads them (plan R08, Design note 5;
 * R08.T4.a): `channels.json`, which `channels.test.ts` fits and records.
 *
 * @remarks
 * A leaf module, importing nothing of the atmosphere's, so that `medium.ts` can read it when
 * R08.T6.d makes `CHANNEL_WAVELENGTHS_NM` the fitted triple, with no import cycle through
 * `channels.ts` (which imports `medium.ts`) and without bundling the fit or the matching functions.
 */

import type { Rgb } from "../photometry/toneCurve";
import recorded from "./channels.json" with { type: "json" };

/** What the renderer reads of `channels.json`. */
export interface RecordedChannels {
  /** The fitted wavelengths, nm, red, green and blue. */
  readonly wavelengthsNm: ReadonlyArray<number>;
}

/**
 * The triple a record holds.
 *
 * @throws Error unless it holds three finite wavelengths, red above green above blue, a broken
 *   invariant of the committed file.
 */
export function fittedTriple(file: RecordedChannels): Rgb {
  const [red, green, blue] = file.wavelengthsNm;
  if (
    file.wavelengthsNm.length !== 3 ||
    red === undefined ||
    green === undefined ||
    blue === undefined ||
    !(Number.isFinite(red) && red > green && green > blue && blue > 0)
  ) {
    throw new Error(
      `channels.json records three wavelengths, red above green above blue, not [${file.wavelengthsNm.join(", ")}]`,
    );
  }
  return [red, green, blue];
}

/**
 * The fitted channel wavelengths, nm, red, green and blue: `channels.json`, which
 * `channels.test.ts` writes under a bless and otherwise checks unchanged (R08.T4.a). R08.T6.d makes
 * `CHANNEL_WAVELENGTHS_NM` read them when it rebuilds Earth at them.
 */
export const FITTED_CHANNEL_WAVELENGTHS_NM: Rgb = fittedTriple(recorded);
