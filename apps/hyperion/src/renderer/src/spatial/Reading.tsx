import { StaleMark } from "../components/StaleMark";

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
  /** The labelled value to show. */
  readonly reading: SpatialReading;
  /**
   * Whether the value's source no longer backs it, as after the link is lost. Given, true or false,
   * the reading keeps a slot for the guide's trailing `S` and holds the mark in it while this is
   * true; absent, the reading has no slot, as a value the console keeps itself has none.
   */
  readonly stale?: boolean | undefined;
  /**
   * Whether the value is one the console keeps itself, which no lost link makes stale, as the orbit
   * map's display time is (plan 12, P12.T11.d): its reading is then marked
   * `spatial-reading--own`, so that a stale view leaves its value in `--text`, and it has no slot
   * and no mark, whatever `stale` says.
   */
  readonly own?: boolean | undefined;
}

/**
 * One labelled value of a spatial view's readouts: a term and its definition, for a `dl`.
 *
 * @remarks
 * Set as the console's other labelled values (`.field`): the label muted, the value in monospaced
 * figures, right-aligned in a slot of fixed width, and its unit muted after it. A missing value is
 * an em dash in `--text-muted`, with no unit. Not a live region: the values around a view change as
 * the operator moves it, and are not announced as they do.
 *
 * Where `stale` is given, true or false, a slot 2 ch wide in the value's monospaced figures stands
 * after the unit, or after the value where it has none (`spatial-reading__stale-slot`): it holds
 * {@link StaleMark}'s `S`, right-aligned, while `stale` holds and the value is not missing, and is
 * kept empty while it is live, so that the mark's coming moves nothing after it (the guide's
 * "Numbers": a field is sized for its longest value and its status marks). The em dash, which has
 * no last value, keeps its slot and takes no mark. With no `stale` there is no slot, as on the
 * camera's `AZM` and `ELV`, which are live; nor with `own`, which no lost link makes stale.
 */
export function Reading({ reading, stale, own = false }: ReadingProps) {
  // A value the console keeps itself is never stale, so `own` overrides any `stale` given with it.
  const slotted = own ? undefined : stale;
  return (
    <div className={own ? "field spatial-reading spatial-reading--own" : "field spatial-reading"}>
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
        {slotted === undefined ? null : (
          <span className="spatial-reading__stale-slot">
            {slotted && reading.value !== null ? <StaleMark /> : null}
          </span>
        )}
      </dd>
    </div>
  );
}
