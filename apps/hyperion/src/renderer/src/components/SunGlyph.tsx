/**
 * The astronomical sun sign (U+2609), drawn because neither B612 nor B612 Mono contains it.
 *
 * @remarks
 * A circle with a centre dot in the current text colour, sized to the text and set as a
 * subscript. It is decorative on its own: the unit it belongs to carries the accessible name, as
 * `SolarMassUnit` does. See "Typography" in `docs/frontend/ux-guidelines.md`.
 */
export function SunGlyph() {
  return (
    <svg className="glyph" viewBox="0 0 10 10" aria-hidden="true" focusable="false">
      <circle cx="5" cy="5" r="4" fill="none" stroke="currentColor" strokeWidth="1" />
      <circle cx="5" cy="5" r="1" fill="currentColor" />
    </svg>
  );
}
