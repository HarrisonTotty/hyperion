import { SunGlyph } from "./SunGlyph";

/**
 * The unit symbol for solar masses, an M followed by the drawn sun sign, announced as "solar
 * masses".
 *
 * @remarks
 * The sun sign (U+2609) is never typed into a string that reaches the screen, because B612 lacks
 * it and a fallback glyph would not match. See "Typography" in `docs/frontend/ux-guidelines.md`.
 */
export function SolarMassUnit() {
  return (
    // An `img` element cannot hold the letter and the drawn sign, so the pair is an image by role,
    // which gives the unit one accessible name.
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <span className="unit" role="img" aria-label="solar masses">
      <span aria-hidden="true">M</span>
      <SunGlyph />
    </span>
  );
}
