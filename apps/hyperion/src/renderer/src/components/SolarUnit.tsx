import { SunGlyph } from "./SunGlyph";

/** A solar unit other than mass, which `SolarMassUnit` draws: luminosity or radius. */
export type SolarQuantity = "luminosity" | "radius";

/** The letter and the accessible name of each solar unit (the owner's draft of the guide's units). */
const UNITS: Readonly<Record<SolarQuantity, { readonly letter: string; readonly name: string }>> = {
  luminosity: { letter: "L", name: "solar luminosities" },
  radius: { letter: "R", name: "solar radii" },
};

/** Props of {@link SolarUnit}. */
export interface SolarUnitProps {
  readonly quantity: SolarQuantity;
}

/**
 * The unit symbol for solar luminosities, `L☉`, or solar radii, `R☉`: the letter followed by the
 * drawn sun sign, announced as "solar luminosities" or "solar radii".
 *
 * @remarks
 * As `SolarMassUnit` draws `M☉`: the sun sign (U+2609) is not in B612 and is never typed into a
 * string that reaches the screen. The units are the owner's draft entry for the guide's "Numbers,
 * units and time", which the client is built to (the orchestrator's ruling 33).
 */
export function SolarUnit({ quantity }: SolarUnitProps) {
  const { letter, name } = UNITS[quantity];
  return (
    // An `img` element cannot hold the letter and the drawn sign, so the pair is an image by role,
    // which gives the unit one accessible name.
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <span className="unit" role="img" aria-label={name}>
      <span aria-hidden="true">{letter}</span>
      <SunGlyph />
    </span>
  );
}
