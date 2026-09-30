import { EarthGlyph } from "./EarthGlyph";

/**
 * The unit symbol for Earth masses, an M followed by the drawn Earth sign, announced as "Earth
 * masses".
 *
 * @remarks
 * The Earth sign (U+2295) is never typed into a string that reaches the screen, because B612 lacks
 * it and a fallback glyph would not match. Every planetary mass is in this unit (plan 13, P13.T8.b;
 * plan 14, D19). See "Typography" in `docs/frontend/ux-guidelines.md`.
 */
export function EarthMassUnit() {
  return (
    // An `img` element cannot hold the letter and the drawn sign, so the pair is an image by role,
    // which gives the unit one accessible name.
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <span className="unit" role="img" aria-label="Earth masses">
      <span aria-hidden="true">M</span>
      <EarthGlyph />
    </span>
  );
}
