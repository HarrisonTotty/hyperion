import type { PrimaryModifier } from "../../lib/platform";
import { RateChord } from "./RateChord";

/**
 * The legend's keys before the rate's chord: where they act, the turn a drag or the arrows give
 * in every preset, and what acts in `FREE` only (plan R07, T19.f).
 */
const KEY_LEGEND_LEAD = "FOCUSED VIEW: DRAG/ARROWS TURN · FREE: W/S A/D R/F MOVE, Q/E ROLL, ";

/** The legend's last word, after the rate's chord. */
const KEY_LEGEND_RATE = " RATE";

/** The legend in `ORBIT`, whose keys hold no rate (plan R15, T4). */
const KEY_LEGEND_ORBIT =
  "FOCUSED VIEW: DRAG/ARROWS ORBIT · WHEEL OR W/S ZOOM, R/F TILT, Q/E HEADING";

/** Props of {@link KeyLegend}. */
export interface KeyLegendProps {
  /** The legend's ID, by which every view's canvas is described. */
  readonly id: string;
  /** The platform's primary modifier, whose chord with the arrows steps the rate. */
  readonly modifier: PrimaryModifier;
  /** Whether the `CONTROLS` view's camera is in `ORBIT`, whose keys the legend then names. */
  readonly orbit: boolean;
}

/**
 * `VIEW`'s key legend under its stage, which describes every view's canvas (plan R02, R02.T15;
 * R07.T19.f): `FOCUSED VIEW: DRAG/ARROWS TURN · FREE: W/S A/D R/F MOVE, Q/E ROLL, CTRL+↑/↓ RATE`,
 * with `⌘↑/↓` on macOS.
 *
 * @remarks
 * It says where the keys act, on the view whose canvas holds the focus, which a press gives it and
 * its ring then marks, and that the drag and the arrows turn in every preset while the rest act in
 * `FREE` only. It is one line on 1280 × 720's stage, which keeps the height two instrument slots
 * need. `PAGE UP` and `PAGE DOWN` still step the rate, but are named only in the guide: the
 * legend and the camera panel's limit reason name the chord alone (decision-r07-t19f-position,
 * item 5). In `ORBIT` (plan R15, T4) it names that preset's keys instead:
 * `FOCUSED VIEW: DRAG/ARROWS ORBIT · WHEEL OR W/S ZOOM, R/F TILT, Q/E HEADING`.
 */
export function KeyLegend({ id, modifier, orbit }: KeyLegendProps) {
  if (orbit) {
    return (
      <p className="view__keys" id={id}>
        {KEY_LEGEND_ORBIT}
      </p>
    );
  }
  return (
    <p className="view__keys" id={id}>
      {KEY_LEGEND_LEAD}
      <RateChord modifier={modifier} arrows="↑/↓" />
      {KEY_LEGEND_RATE}
    </p>
  );
}
