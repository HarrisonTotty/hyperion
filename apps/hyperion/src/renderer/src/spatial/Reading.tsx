/** A value as a spatial view shows it: formatted digits and their unit. */
export interface SpatialQuantity {
  /** The formatted value, with a degree sign written against its digits: `26,000.0`, `045.0°`. */
  readonly value: string;
  /** The unit, set after a space, or `""` for none: `ly`, `yr`. */
  readonly unit: string;
}

/** A labelled value around a spatial view: `RADIUS 26,000.0 ly`, `UT +0.00 yr`. */
export interface SpatialReading {
  /** The label, in upper case: `RADIUS`, or a time system such as `UT`. */
  readonly label: string;
  /** The formatted value, or `null` when it is missing, as an angle is on the galactic axis. */
  readonly value: string | null;
  /** The unit, set after a space, or `""` for none. */
  readonly unit: string;
  /**
   * The width of the value's slot, in characters of its monospaced figures: room for its longest
   * value, so that a new value never moves what follows it (`-65,536.0` is 9).
   */
  readonly widthCh: number;
}

/** Props of {@link Reading}. */
export interface ReadingProps {
  readonly reading: SpatialReading;
}

/**
 * One labelled value of a spatial view's readouts: a term and its definition, for a `dl`.
 *
 * @remarks
 * Set as the console's other labelled values (`.field`): the label muted, the value in monospaced
 * figures, right-aligned in a slot of fixed width, and its unit muted after it. A missing value is
 * an em dash in `--text-muted`, with no unit. Not a live region: the values around a view change as
 * the operator moves it, and are not announced as they do.
 */
export function Reading({ reading }: ReadingProps) {
  return (
    <div className="field spatial-reading">
      <dt className="field__label">{reading.label}</dt>{" "}
      <dd className="field__value">
        <span className="spatial-reading__value" style={{ minWidth: `${reading.widthCh}ch` }}>
          {reading.value ?? <span className="readout__missing">—</span>}
        </span>
        {reading.unit.length > 0 && reading.value !== null ? (
          <>
            {" "}
            <span className="spatial-reading__unit">{reading.unit}</span>
          </>
        ) : null}
      </dd>
    </div>
  );
}
