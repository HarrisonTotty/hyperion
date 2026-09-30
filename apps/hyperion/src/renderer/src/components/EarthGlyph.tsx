/**
 * The astronomical Earth sign (U+2295, a circled plus), drawn because neither B612 nor B612 Mono
 * contains it.
 *
 * @remarks
 * A circle with a cross through its centre in the current text colour, sized to the text and set as
 * a subscript, as `SunGlyph` is. It is decorative on its own: the unit it belongs to carries the
 * accessible name, as `EarthMassUnit` does. See "Typography" in `docs/frontend/ux-guidelines.md`,
 * which reserves the sign for planets (plan 13, P13.T8.b).
 */
export function EarthGlyph() {
  return (
    <svg className="glyph" viewBox="0 0 10 10" aria-hidden="true" focusable="false">
      <circle cx="5" cy="5" r="4" fill="none" stroke="currentColor" strokeWidth="1" />
      <path d="M5 1V9M1 5H9" fill="none" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}
