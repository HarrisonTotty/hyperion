import { CommandGlyph } from "../../components/CommandGlyph";
import type { PrimaryModifier } from "../../lib/platform";

/** Props of {@link RateChord}. */
export interface RateChordProps {
  /** The platform's primary modifier. */
  readonly modifier: PrimaryModifier;
  /** The arrows the chord takes: both, in the legend, or the one a limit reason names. */
  readonly arrows: "↑/↓" | "↑" | "↓";
}

/**
 * The free camera's rate chord as the platform reads it (plan R07, T19.f;
 * decision-r07-t19f-position, item 5): `CTRL+↑/↓` on Linux and Windows, and `⌘↑/↓` on macOS, its
 * `⌘` drawn (B612 has none) and read by assistive technology as "Command+", as `CTRL+` is read.
 *
 * @remarks
 * The one source of the chord for the key legend and the camera panel's limit reason, so that the
 * screen gives the rate's step one name.
 */
export function RateChord({ modifier, arrows }: RateChordProps) {
  if (modifier === "ctrl") {
    return `CTRL+${arrows}`;
  }
  return (
    <>
      {/* The sign is drawn and hidden; its name is read in its place, as `CTRL+` is read. */}
      <span className="visually-hidden">Command+</span>
      <CommandGlyph />
      {arrows}
    </>
  );
}
