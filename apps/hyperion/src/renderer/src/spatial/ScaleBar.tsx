import { SCENE_UNIT_ONLY, scaleBar, type ScaleUnit } from "./scale";

/** Props of {@link ScaleBar}. */
export interface ScaleBarProps {
  /** The picture's scale, CSS pixels per scene unit. */
  readonly pxPerUnit: number;
  /** The longest the bar may be, in CSS pixels. */
  readonly maxBarPx: number;
  /** Writes a length in scene units with its unit: `20 ly`, `0.05 ly`, `500 AU`. */
  readonly formatLength: (length: number) => string;
  /**
   * The units the bar may be read in, largest first, so that a light-year scene can step down to
   * astronomical units; the scene's own unit alone when absent.
   */
  readonly units?: ReadonlyArray<ScaleUnit> | undefined;
}

/**
 * A 1-2-5 scale bar: the longest length of 1, 2 or 5 times a power of ten, in the ladder's units,
 * that fits in `maxBarPx`, drawn as a bar with end ticks and labelled with that length (plan 05,
 * T10.g).
 *
 * @remarks
 * The same bar serves the galaxy map and every spatial view. It keeps one width however long the
 * bar is, the bar at the end of a slot as long as the longest and the label in a slot as wide as
 * the longest, so that it can stand in a row without moving its neighbours. It is an image to
 * assistive technology, named with its length.
 */
export function ScaleBar({ pxPerUnit, maxBarPx, formatLength, units }: ScaleBarProps) {
  const bar = scaleBar(pxPerUnit, maxBarPx, units ?? SCENE_UNIT_ONLY);
  const label = formatLength(bar.length);
  return (
    // An `img` element cannot hold the drawn bar and its label, so the pair is an image by role,
    // named with its length.
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <div className="scale-bar" role="img" aria-label={`Scale bar, ${label}`}>
      {/*
       * The bar stands at the end of a slot as long as the longest bar, and the label in a slot of
       * fixed width, so that a new length moves nothing beside the bar.
       */}
      <div className="scale-bar__slot" style={{ width: `${maxBarPx + 1}px` }}>
        {/* One pixel wider, so that the centres of the 1 px end ticks are the length apart. */}
        <div className="scale-bar__bar" style={{ width: `${bar.lengthPx + 1}px` }} />
      </div>
      <span className="scale-bar__label">{label}</span>
    </div>
  );
}
