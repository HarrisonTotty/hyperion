import { type FormEvent, useEffect, useId, useState } from "react";

import { formatBearingDeg, formatLengthLy, formatNumber, formatSigned } from "../../lib/format";
import { type CentreLy, ROOT_CUBE_HALF_LY } from "../../lib/galaxy/model";
import { isTextEntry } from "../../lib/textEntry";
import { AXIS_TOLERANCE_LY, cylindrical } from "../../spatial/frame";
import { vec3 } from "../../spatial/vec3";

/** A coordinate of the cursor, by its place in a {@link CentreLy}. */
type Axis = 0 | 1 | 2;

const AXES: ReadonlyArray<{ readonly axis: Axis; readonly label: string }> = [
  { axis: 0, label: "X" },
  { axis: 1, label: "Y" },
  { axis: 2, label: "Z" },
];

/** A number as typed: digits with optional grouping commas, a decimal part and a sign. */
const COORDINATE = /^[+\-−]?(?:\d+(?:\.\d*)?|\.\d+)$/u;

/**
 * The most decimals a typed coordinate keeps: 0.0001 ly, finer than the smallest chart, 0.01 ly
 * across. An entry is rounded to it before it is checked, so what is kept is what is shown.
 */
const MAX_DECIMALS = 4;

/** Decimals a coordinate is shown with unless it was typed with more. */
const DEFAULT_DECIMALS = 1;

/**
 * The largest coordinate an entry can hold in the root cube: the cube runs from −65,536 ly up to,
 * but not including, 65,536 ly (plan 01's `in_root_cube`), and an entry keeps four decimals.
 */
const LARGEST_ENTRY_LY = ROOT_CUBE_HALF_LY - 10 ** -MAX_DECIMALS;

/** Whether a coordinate lies in the root cube, where the server accepts a chart centre. */
function inRootCube(valueLy: number): boolean {
  return valueLy >= -ROOT_CUBE_HALF_LY && valueLy < ROOT_CUBE_HALF_LY;
}

/** A typed coordinate, and the decimals it was typed with, for showing it as it was entered. */
interface Entry {
  readonly valueLy: number;
  readonly decimals: number;
}

/**
 * The coordinate a field's text stands for, or `null` when it is not a number or lies outside the
 * root cube, where the server refuses a chart centre (plan 04, design note 24).
 */
function parseCoordinate(text: string): Entry | null {
  const compact = text.trim().replaceAll(",", "");
  if (!COORDINATE.test(compact)) {
    return null;
  }
  const scale = 10 ** MAX_DECIMALS;
  const valueLy = Math.round(Number(compact.replace("−", "-")) * scale) / scale;
  const typedDecimals = compact.split(".")[1]?.length ?? 0;
  return Number.isFinite(valueLy) && inRootCube(valueLy)
    ? {
        valueLy,
        decimals: Math.min(MAX_DECIMALS, Math.max(DEFAULT_DECIMALS, typedDecimals)),
      }
    : null;
}

/**
 * What the operator has typed in a field and not yet entered, the cursor's coordinate it was typed
 * over, and whether it was entered and refused.
 */
interface Draft {
  readonly text: string;
  readonly forLy: number;
  readonly refused: boolean;
}

/** Per field: what is typed and not entered, and what was entered, for its precision. */
interface FieldState {
  readonly draft: Draft | null;
  readonly entered: Entry | null;
}

type Fields = Readonly<Record<Axis, FieldState>>;

const EMPTY_FIELD: FieldState = { draft: null, entered: null };

const EMPTY_FIELDS: Fields = { 0: EMPTY_FIELD, 1: EMPTY_FIELD, 2: EMPTY_FIELD };

/**
 * The fields once the cursor has moved: a field whose coordinate changed drops its draft, which it
 * would contradict, and the precision of an entry that is no longer the cursor's.
 */
function followCursor(fields: Fields, cursorLy: CentreLy): Fields {
  const follow = (axis: Axis): FieldState => {
    const { draft, entered } = fields[axis];
    const valueLy = cursorLy[axis];
    const keptDraft = draft !== null && draft.forLy === valueLy ? draft : null;
    const keptEntry = entered !== null && entered.valueLy === valueLy ? entered : null;
    return keptDraft === draft && keptEntry === entered
      ? fields[axis]
      : { draft: keptDraft, entered: keptEntry };
  };
  const next: Fields = { 0: follow(0), 1: follow(1), 2: follow(2) };
  return next[0] === fields[0] && next[1] === fields[1] && next[2] === fields[2] ? fields : next;
}

function withAxis(cursorLy: CentreLy, axis: Axis, valueLy: number): CentreLy {
  const [xLy, yLy, zLy] = cursorLy;
  if (axis === 0) {
    return [valueLy, yLy, zLy];
  }
  return axis === 1 ? [xLy, valueLy, zLy] : [xLy, yLy, valueLy];
}

/**
 * Keeps the form from submitting: a form of three fields and no submit button submits nothing on
 * `Enter` in any case, and each field enters its own value.
 */
function preventSubmit(event: FormEvent<HTMLFormElement>): void {
  event.preventDefault();
}

interface ReadingProps {
  readonly label: string;
  /** The digits, or `null` for a value that does not exist, shown as an em dash. */
  readonly value: string | null;
  readonly unit: string;
}

function Reading({ label, value, unit }: ReadingProps) {
  return (
    <>
      <dt>{label}</dt>
      <dd>
        {value === null ? (
          <span className="cursor-readout__value readout__missing">—</span>
        ) : (
          <span className="cursor-readout__value">{value}</span>
        )}
        {/* The degree sign is written against the digits; other units after a space. */}
        {unit === "°" ? null : " "}
        <span className="cursor-readout__unit">{value === null ? "" : unit}</span>
      </dd>
    </>
  );
}

/** A bearing's digits, without the degree sign, which takes the unit's place beside them. */
function bearingDigits(angleDeg: number): string {
  return formatBearingDeg(angleDeg, 1).replace(/°$/u, "");
}

interface CentreEntryProps {
  /** The map cursor, in the `GALACTIC` frame. */
  readonly cursorLy: CentreLy;
  /** Moves the cursor to a coordinate entered in range. */
  readonly onCursor: (cursorLy: CentreLy) => void;
  /** Makes the cursor the chart centre. */
  readonly onCentre: (centreLy: CentreLy) => void;
  /**
   * Why the chart cannot be centred now, such as the link's reason (`NO CARRIER`), shown in the
   * panel; `null` when it can.
   */
  readonly heldBack: string | null;
}

/**
 * The `CURSOR` panel: the map cursor's coordinates, entered or read, and `CENTRE CHART`, which
 * makes the cursor the chart's centre.
 *
 * @remarks
 * It stands at the head of the local chart's column, beside the map and after it in reading order,
 * between the map the cursor is picked on and the chart it centres. Three fields, `X`, `Y` and `Z`
 * in light-years, show the cursor. A value typed in one moves it when it is entered, as the field
 * is left or with `Enter`, so that the chart can be centred more finely than a map pixel (128 ly
 * at the finest) and the cursor never passes through the digits typed on the way; it is kept to
 * four decimals and shown with the decimals it was typed with, up to four, in a field wide enough
 * for the longest, `-65,535.9999`, with `ly` beside it.
 * A value that is not a number, or lies outside the root cube (−65,536 ly up to 65,536 ly, that
 * face excluded), is refused with a message naming its field, and leaves the cursor where it was; a
 * move of the cursor along that axis replaces it. Under them, the cursor's `RADIUS`, `ANGLE` and
 * `HEIGHT` in the `GALACTIC` frame, grouped under that heading as the guide asks; the angle is
 * missing on the galactic axis, where it is undefined (D11). The whole position is announced when
 * the cursor moves. `CENTRE CHART`, or the key `C` pressed outside a text field without a modifier
 * (D3), publishes the cursor as the chart centre; both are held back, saying why, while an entry is
 * refused or while the chart cannot be centred, as when the link is down, whose reason the panel
 * then shows. The key's listener is on the document for as long as the entry is mounted, which
 * under `Activity` is while `GALAXY` is shown with a universe open.
 */
export function CentreEntry({ cursorLy, onCursor, onCentre, heldBack }: CentreEntryProps) {
  const titleId = useId();
  const hintId = useId();
  const errorId = useId();
  const fieldId = useId();
  const heldBackId = useId();
  const [fields, setFields] = useState<Fields>(EMPTY_FIELDS);
  const [seenCursor, setSeenCursor] = useState<CentreLy>(cursorLy);

  let current = fields;
  if (seenCursor !== cursorLy) {
    setSeenCursor(cursorLy);
    current = followCursor(fields, cursorLy);
    if (current !== fields) {
      setFields(current);
    }
  }

  const refusedLabels = AXES.filter(({ axis }) => current[axis].draft?.refused === true).map(
    ({ label }) => label,
  );
  const canCentre = heldBack === null && refusedLabels.length === 0 && cursorLy.every(inRootCube);
  const refusalId = refusedLabels.length > 0 ? errorId : null;

  /**
   * Enters every field's draft: an accepted value moves the cursor, a refused one stays, marked.
   * Returns the cursor as entered, or `null` if any value was refused.
   */
  const enter = (): CentreLy | null => {
    let entered = cursorLy;
    let accepted = true;
    const next: Record<Axis, FieldState> = { ...current };
    for (const { axis } of AXES) {
      const { draft } = current[axis];
      if (draft === null) {
        continue;
      }
      const entry = parseCoordinate(draft.text);
      if (entry === null) {
        accepted = false;
        next[axis] = { draft: { ...draft, refused: true }, entered: null };
      } else {
        entered = withAxis(entered, axis, entry.valueLy);
        next[axis] = { draft: null, entered: entry };
      }
    }
    setFields(next);
    if (entered !== cursorLy) {
      onCursor(entered);
    }
    return accepted ? entered : null;
  };

  const centre = (): void => {
    const entered = enter();
    if (entered !== null && heldBack === null && entered.every(inRootCube)) {
      onCentre(entered);
    }
  };

  // The key reaches the entry from anywhere on the display, so its handler follows the cursor.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const modified = event.ctrlKey || event.altKey || event.metaKey || event.shiftKey;
      if (event.repeat || modified || (event.key !== "c" && event.key !== "C")) {
        return;
      }
      if (isTextEntry(event.target)) {
        return;
      }
      event.preventDefault();
      if (canCentre) {
        onCentre(cursorLy);
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [canCentre, cursorLy, onCentre]);

  const [xLy, yLy, zLy] = cursorLy;
  const { radiusLy, angleDeg, heightLy } = cylindrical(vec3(xLy, yLy, zLy));
  const onAxis = !(radiusLy > AXIS_TOLERANCE_LY);
  const shown = (axis: Axis): string => {
    const { entered } = current[axis];
    return formatNumber(cursorLy[axis], entered?.decimals ?? DEFAULT_DECIMALS);
  };
  const radius = formatLengthLy(radiusLy, 1);
  const angle = onAxis ? null : bearingDigits(angleDeg);
  const height = formatSigned(heightLy, 1);

  return (
    <section className="panel galaxy__cursor centre-entry" aria-labelledby={titleId}>
      <div className="centre-entry__head">
        <h2 className="panel__title" id={titleId}>
          Cursor
        </h2>
        <span className="form-field__hint" id={hintId}>
          ±{formatNumber(ROOT_CUBE_HALF_LY, 0)} ly
        </span>
        <button
          type="button"
          className="control"
          // Held back rather than disabled, so that it keeps focus and can say why.
          aria-disabled={canCentre ? undefined : "true"}
          aria-describedby={heldBack === null ? (refusalId ?? undefined) : heldBackId}
          onClick={() => {
            if (heldBack === null) {
              centre();
            }
          }}
        >
          <span className="control__key">C</span> CENTRE CHART
        </button>
      </div>
      {heldBack === null ? null : (
        <p className="panel__inhibit" id={heldBackId}>
          {heldBack}
        </p>
      )}
      <form noValidate onSubmit={preventSubmit}>
        <div className="centre-entry__fields">
          {AXES.map(({ axis, label }) => {
            const { draft } = current[axis];
            const refused = draft?.refused === true;
            const id = `${fieldId}-${label}`;
            return (
              <div className="centre-entry__field" key={label}>
                <label className="form-field__label" htmlFor={id}>
                  {label}
                </label>
                <input
                  id={id}
                  className="form-field__input form-field__input--number centre-entry__input"
                  type="text"
                  inputMode="decimal"
                  autoComplete="off"
                  spellCheck={false}
                  value={draft?.text ?? shown(axis)}
                  aria-invalid={refused ? "true" : undefined}
                  aria-describedby={refused ? `${hintId} ${errorId}` : hintId}
                  onChange={(event) => {
                    const text = event.target.value;
                    setFields((previous) => ({
                      ...previous,
                      [axis]: {
                        draft: { text, forLy: cursorLy[axis], refused: false },
                        entered: previous[axis].entered,
                      },
                    }));
                  }}
                  onKeyDown={(event) => {
                    // `Enter` enters what was typed; centring is CENTRE CHART's and `C`'s alone.
                    if (event.key === "Enter") {
                      event.preventDefault();
                      enter();
                    }
                  }}
                  onBlur={() => {
                    if (current[axis].draft !== null) {
                      enter();
                    }
                  }}
                />
                <span className="centre-entry__unit">ly</span>
              </div>
            );
          })}
        </div>
        {refusalId === null ? null : (
          <p className="form-field__error" id={refusalId}>
            {refusedLabels.join(", ")} INVALID: enter {formatNumber(-ROOT_CUBE_HALF_LY, 0)} to{" "}
            {formatNumber(LARGEST_ENTRY_LY, MAX_DECIMALS)} ly
          </p>
        )}
      </form>
      {/* The guide's § Voice: a readout groups the three under the heading `GALACTIC`. */}
      <h3 className="cursor-readout__heading">GALACTIC</h3>
      <dl className="readout cursor-readout__values">
        <Reading label="RADIUS" value={radius} unit="ly" />
        <Reading label="ANGLE" value={angle} unit="°" />
        <Reading label="HEIGHT" value={height} unit="ly" />
      </dl>
      {/* Heard as a whole each time the cursor moves, from a map's arrow keys most of all. */}
      <p className="visually-hidden" aria-live="polite" aria-atomic="true">
        {`CURSOR X ${shown(0)} ly, Y ${shown(1)} ly, Z ${shown(2)} ly, ` +
          `GALACTIC RADIUS ${radius} ly, ` +
          `ANGLE ${angle === null ? "none" : `${angle}°`}, HEIGHT ${height} ly`}
      </p>
    </section>
  );
}
