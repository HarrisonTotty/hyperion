interface DisclosureGlyphProps {
  /** Whether the part of the panel that the control shows or folds is shown. */
  readonly expanded: boolean;
}

/**
 * The mark of a control that shows or folds part of a panel: a chevron pointing right while the
 * part is folded, and down while it is shown.
 *
 * @remarks
 * Drawn, because B612 has no triangle or right-pointing arrow, and a shape, so that the state is
 * not told by colour. It is decorative: the control's label names it, and `aria-expanded` gives
 * its state to assistive technology.
 */
export function DisclosureGlyph({ expanded }: DisclosureGlyphProps) {
  return (
    <svg
      className="glyph glyph--disclosure"
      viewBox="0 0 10 10"
      aria-hidden="true"
      focusable="false"
    >
      <polyline
        points={expanded ? "2,3.5 5,6.5 8,3.5" : "3.5,2 6.5,5 3.5,8"}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
      />
    </svg>
  );
}
