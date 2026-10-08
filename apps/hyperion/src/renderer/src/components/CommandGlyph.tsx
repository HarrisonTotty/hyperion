/**
 * The Command key's sign (U+2318, ⌘), drawn because neither B612 nor B612 Mono contains it (plan
 * R07, T19.f).
 *
 * @remarks
 * A square whose sides run on into a loop at each corner, stroked in the current text colour and
 * sized to the text, as the drawn sun sign is (`SunGlyph`). It is hidden from assistive
 * technology: the key chord it belongs to reads its name, "Command", in its place. See
 * "Typography" in `docs/frontend/ux-guidelines.md`.
 */
export function CommandGlyph() {
  return (
    <svg className="glyph glyph--command" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path
        d="M5.5 5.5V3.5A2 2 0 1 0 3.5 5.5H12.5A2 2 0 1 0 10.5 3.5V12.5A2 2 0 1 0 12.5 10.5H3.5A2 2 0 1 0 5.5 12.5Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
    </svg>
  );
}
