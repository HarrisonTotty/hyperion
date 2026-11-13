import { StaleSlot } from "../components/StaleSlot";

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

/**
 * Where a spatial reading's value comes from, and so whether it can go stale (plan 12, P12.T11.d
 * and P12.T11.f):
 *
 * - `live`: read from a source that can go stale, such as the server's answer, which backs it now;
 * - `stale`: read from such a source that no longer backs it, as after the link is lost;
 * - `own`: a value the console keeps itself, such as the orbit map's display time, which `RUN` and
 *   `HOLD` move: no lost link makes it stale.
 */
export type ReadingState = "live" | "stale" | "own";

/** The state of a reading whose value comes from a source that can go stale: never `own`. */
export type SourceState = Exclude<ReadingState, "own">;

/**
 * The state of a reading from a source that can go stale, for a caller that holds whether its
 * source still backs it: `stale` while `stale` holds, else `live`.
 */
export function sourceState(stale: boolean): SourceState {
  return stale ? "stale" : "live";
}

/** Props of {@link Reading}. */
export interface ReadingProps {
  /** The labelled value to show. */
  readonly reading: SpatialReading;
  /**
   * Where the value comes from (see {@link ReadingState}): `live` and `stale` keep a slot for the
   * guide's trailing `S`, holding the mark while `stale`; `own` marks the reading
   * `spatial-reading--own`, so that a stale view leaves its value in `--text`, and keeps no slot.
   * Absent, there is no slot and no `spatial-reading--own`, as on `AZM` and `ELV`, the camera's own
   * angles, which stand outside the furniture a stale view mutes.
   */
  readonly state?: ReadingState | undefined;
}

/** How a reading is set in its state. */
interface ReadingForm {
  /** Whether it is the console's own, `spatial-reading--own`. */
  readonly own: boolean;
  /** Whether it keeps a slot after its value for the guide's trailing stale `S`. */
  readonly slot: boolean;
  /** Whether the slot holds the mark, where the value is not missing. */
  readonly marked: boolean;
}

/** How a reading in `state` is set, each state told apart (see {@link ReadingProps.state}). */
function formOf(state: ReadingState | undefined): ReadingForm {
  let form: ReadingForm;
  switch (state) {
    case undefined:
      form = { own: false, slot: false, marked: false };
      break;
    case "live":
      form = { own: false, slot: true, marked: false };
      break;
    case "stale":
      form = { own: false, slot: true, marked: true };
      break;
    case "own":
      form = { own: true, slot: false, marked: false };
      break;
  }
  return form;
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
 * Where `state` is `live` or `stale`, the console's shared {@link StaleSlot} (`.stale-slot`), 2 ch
 * wide in the value's monospaced figures, stands after the unit, or after the value where it has
 * none: it holds the guide's `S`, right-aligned, while `state` is `stale` and the value is not
 * missing, and is kept empty while it is `live`, so that the mark's coming moves nothing after it
 * (the guide's "Numbers": a field is sized for its longest value and its status marks). The em
 * dash, which has no last value, keeps its slot and takes no mark. With no `state` there is no
 * slot, as on the camera's `AZM` and `ELV`.
 */
export function Reading({ reading, state }: ReadingProps) {
  const form = formOf(state);
  return (
    <div
      className={form.own ? "field spatial-reading spatial-reading--own" : "field spatial-reading"}
    >
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
        {form.slot ? <StaleSlot stale={form.marked && reading.value !== null} /> : null}
      </dd>
    </div>
  );
}
