/**
 * The bracket reticle that marks the selection, as a legend sample at the size of the largest mark.
 *
 * @remarks
 * Hidden from assistive technology: the words beside it say what it marks.
 */
export function LegendReticle() {
  return (
    <svg
      className="symbol-legend__mark"
      style={{ width: "1.5rem", height: "1.5rem" }}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M4 9V4H9M15 4H20V9M20 15V20H15M9 20H4V15" />
    </svg>
  );
}
