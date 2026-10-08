import { CommandGlyph } from "../../components/CommandGlyph";
import type { PrimaryModifier } from "../../lib/platform";

/**
 * The legend's keys before the rate's chord: where they act, the turn a drag or the arrows give
 * in every preset, and what acts in `FREE` only (plan R07, T19.f).
 */
const KEY_LEGEND_LEAD = "FOCUSED VIEW: DRAG/ARROWS TURN · FREE: W/S A/D R/F MOVE, Q/E ROLL, ";

/** The legend's last word, after the rate's chord. */
const KEY_LEGEND_RATE = " RATE";

/** The rate's chord as Linux and Windows read it: Ctrl with the up or down arrow. */
const CTRL_RATE_CHORD = "CTRL+↑/↓";

/** Props of {@link KeyLegend}. */
export interface KeyLegendProps {
  /** The legend's ID, by which every view's canvas is described. */
  readonly id: string;
  /** The platform's primary modifier, whose chord with the arrows steps the rate. */
  readonly modifier: PrimaryModifier;
}

/** Props of {@link RateChord}. */
interface RateChordProps {
  /** The platform's primary modifier. */
  readonly modifier: PrimaryModifier;
}

/**
 * The rate's chord as the platform reads it: `⌘↑/↓` on macOS, its `⌘` drawn (B612 has none) and
 * read by assistive technology as "Command+", and `CTRL+↑/↓` elsewhere.
 */
function RateChord({ modifier }: RateChordProps) {
  if (modifier === "ctrl") {
    return CTRL_RATE_CHORD;
  }
  return (
    <>
      {/* The sign is drawn and hidden; its name is read in its place, as `CTRL+` is read. */}
      <span className="visually-hidden">Command+</span>
      <CommandGlyph />
      ↑/↓
    </>
  );
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
 * need. `PAGE UP` and `PAGE DOWN` still step the rate, and the camera panel's limit reason names
 * them.
 */
export function KeyLegend({ id, modifier }: KeyLegendProps) {
  return (
    <p className="view__keys" id={id}>
      {KEY_LEGEND_LEAD}
      <RateChord modifier={modifier} />
      {KEY_LEGEND_RATE}
    </p>
  );
}
