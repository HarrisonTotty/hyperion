/**
 * The orbit map's units: its scene is in astronomical units, and its scale bar reads in `km`, `Mm`,
 * `Gm` and `AU` (plan 14, P14.T42.b).
 */
import { KM_PER_AU } from "../../lib/format";
import type { ScaleUnit } from "../../spatial/scale";

/** Metres in one astronomical unit, 149,597,870,700 exactly (IAU 2012 Resolution B2). */
export const METRES_PER_AU = KM_PER_AU * 1_000;

/** Gigametres in one astronomical unit. */
const GM_PER_AU = KM_PER_AU / 1e6;

/** Megametres in one astronomical unit. */
const MM_PER_AU = KM_PER_AU / 1e3;

/**
 * The shortest bar read in astronomical units, 0.1 AU: where `formatBodyDistance` gives way from
 * `Gm` to `AU`, so that a bar and a distance near it read in the same unit.
 */
const AU_FROM = 0.1;

/**
 * The scale bar's unit ladder, largest first, on `formatBodyDistance`'s bands: `AU` from 0.1 AU,
 * then `Gm` from 1 Gm, `Mm` from 1 Mm, and `km` below.
 *
 * @remarks
 * The bar is a 1-2-5 length, so it steps between units rather than drifting across a band's edge,
 * and needs none of the hysteresis `formatBodyDistance` keeps for a distance that moves with the
 * display time.
 */
export const ORBIT_SCALE_UNITS: ReadonlyArray<ScaleUnit> = [
  { perSceneUnit: 1, minSceneLength: AU_FROM },
  { perSceneUnit: GM_PER_AU, minSceneLength: 1 / GM_PER_AU },
  { perSceneUnit: MM_PER_AU, minSceneLength: 1 / MM_PER_AU },
  { perSceneUnit: KM_PER_AU, minSceneLength: 0 },
];

// A tolerance keeps a computed 1-2-5 length from falling to the unit below through rounding in the
// last bit, as the chart's scale-length format has.
const EDGE_TOLERANCE = 1e-9;

const lengthFormat = new Intl.NumberFormat("en-US", {
  maximumSignificantDigits: 3,
  useGrouping: "min2",
});

/**
 * Writes a scale bar's length, given in astronomical units, in the unit {@link ORBIT_SCALE_UNITS}
 * reads it in: `2 AU`, `0.5 AU`, `50 Gm`, `200 Mm`, `500 km`.
 *
 * @throws RangeError when the length is not positive and finite, which no scale bar is.
 */
export function formatOrbitScaleLength(lengthAu: number): string {
  if (!(Number.isFinite(lengthAu) && lengthAu > 0)) {
    throw new RangeError(`a scale bar cannot be ${String(lengthAu)} AU long`);
  }
  const low = 1 - EDGE_TOLERANCE;
  if (lengthAu >= AU_FROM * low) {
    return `${lengthFormat.format(lengthAu)} AU`;
  }
  if (lengthAu * GM_PER_AU >= low) {
    return `${lengthFormat.format(lengthAu * GM_PER_AU)} Gm`;
  }
  if (lengthAu * MM_PER_AU >= low) {
    return `${lengthFormat.format(lengthAu * MM_PER_AU)} Mm`;
  }
  return `${lengthFormat.format(lengthAu * KM_PER_AU)} km`;
}
