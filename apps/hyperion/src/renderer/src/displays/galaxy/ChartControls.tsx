import type { MassLayer } from "@hyperion/protocol";
import { type ReactNode, useEffect, useId, useState } from "react";

import { SolarMassUnit } from "../../components/SolarMassUnit";
import { formatNumber, formatScaleLength, formatUniverseTimeYr } from "../../lib/format";
import { CLOCK_WINDOW_YR } from "../../lib/galaxy/model";
import { isTextEntry } from "../../lib/textEntry";
import { RADIUS_STEPS_LY } from "../../spatial/scale";
import {
  formatBandMsun,
  formatChartLengthLy,
  type LayerBand,
  nextStarFilter,
  STAR_FILTERS,
  type StarFilter,
  starFilterLabel,
} from "./chartModel";

/**
 * The `STARS` selector's single key, which steps to the next filter: `K` for the kind of star, since
 * the view's presets hold `S` and `CENTRE CHART` holds `C`.
 */
export const STARS_KEY = "K";

/** The narrowest and widest drive range the field accepts, in light-years. */
const DRIVE_RANGE_MIN_LY = 0.01;
const DRIVE_RANGE_MAX_LY = 500;

/** Decimals a drive range keeps: the narrowest chart is 0.01 ly across. */
const DRIVE_RANGE_DECIMALS = 2;

/** Decimals a chart time keeps, as `formatUniverseTimeYr` writes it. */
const TIME_DECIMALS = 2;

/** A number as typed: digits with optional grouping commas, a decimal part and a sign. */
const TYPED_NUMBER = /^[+\-−]?(?:\d+(?:\.\d*)?|\.\d+)$/u;

/** The number a field's text stands for, rounded to `decimals`, or `null` when it is not one. */
function parseNumber(text: string, decimals: number): number | null {
  const compact = text.trim().replaceAll(",", "").replace("−", "-");
  if (!TYPED_NUMBER.test(compact)) {
    return null;
  }
  const step = 10 ** decimals;
  const value = Math.round(Number(compact) * step) / step;
  return Number.isFinite(value) ? value : null;
}

interface NumberFieldProps {
  /** The field's label, in upper case: `DRIVE RANGE`. */
  readonly label: string;
  /**
   * What a refusal calls the field: the quantity's name and no other word, which is the label
   * itself for as long as no field's label carries one (`DRIVE RANGE`).
   */
  readonly name: string;
  /** The value on show, formatted without its unit. */
  readonly text: string;
  /** The format the field expects, shown under it: `0.01-500 ly`. */
  readonly hint: string;
  /** What the operator may enter, in words, for a refusal: `enter 0.01 to 500 ly`. */
  readonly valid: string;
  /** The unit after the value, or an element such as a time system's label before it. */
  readonly unit: string;
  readonly before?: ReactNode;
  /** Decimals an entry is rounded to before it is checked. */
  readonly decimals: number;
  /** Whether an entered number is accepted. */
  readonly accepts: (value: number) => boolean;
  readonly onEnter: (value: number) => void;
  /** Why the field is held back, such as the link's reason, with the ID that says so. */
  readonly heldBackId: string | null;
}

/**
 * One number the operator sets: entered when the field is left or with `Enter`, never at each
 * keystroke, so that a chart is not queried for every digit typed on the way.
 */
function NumberField({
  label,
  name,
  text,
  hint,
  valid,
  unit,
  before,
  decimals,
  accepts,
  onEnter,
  heldBackId,
}: NumberFieldProps) {
  const fieldId = useId();
  const hintId = useId();
  const errorId = useId();
  const [draft, setDraft] = useState<string | null>(null);
  const [refused, setRefused] = useState(false);
  const [seen, setSeen] = useState(text);

  // A value set from elsewhere, such as a radius the drive range moved, replaces what was typed.
  if (seen !== text) {
    setSeen(text);
    setDraft(null);
    setRefused(false);
  }

  const enter = (): void => {
    if (draft === null || heldBackId !== null) {
      return;
    }
    const value = parseNumber(draft, decimals);
    if (value === null || !accepts(value)) {
      setRefused(true);
      return;
    }
    setDraft(null);
    setRefused(false);
    onEnter(value);
  };

  return (
    <div className="form-field chart-controls__field">
      <label className="form-field__label" htmlFor={fieldId}>
        {label}
      </label>
      {before}
      <input
        id={fieldId}
        className="form-field__input form-field__input--number chart-controls__input"
        type="text"
        inputMode="decimal"
        autoComplete="off"
        spellCheck={false}
        value={draft ?? text}
        aria-invalid={refused ? "true" : undefined}
        aria-disabled={heldBackId === null ? undefined : "true"}
        aria-describedby={[hintId, refused ? errorId : null, heldBackId]
          .filter((id): id is string => id !== null)
          .join(" ")}
        onChange={(event) => {
          if (heldBackId !== null) {
            return;
          }
          setDraft(event.target.value);
          setRefused(false);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            enter();
          }
        }}
        onBlur={enter}
      />
      <span className="chart-controls__unit">{unit}</span>
      <span className="form-field__hint chart-controls__hint" id={hintId}>
        {hint}
      </span>
      {refused ? (
        <p className="form-field__error chart-controls__error" id={errorId}>
          {name} INVALID: {valid}
        </p>
      ) : null}
    </div>
  );
}

interface ChartControlsProps {
  /** The radius the operator chose, or `null` while it follows the drive range. */
  readonly radiusChoiceLy: number | null;
  /** The radius on show, which is what the chart asks for. */
  readonly queryRadiusLy: number;
  readonly onQueryRadius: (radiusLy: number) => void;
  readonly minLayer: MassLayer;
  readonly onMinLayer: (layer: MassLayer) => void;
  readonly driveRangeLy: number;
  readonly onDriveRange: (rangeLy: number) => void;
  /** The chart time, in years from the epoch. */
  readonly timeYr: number;
  readonly onTime: (timeYr: number) => void;
  /**
   * The mass bands from the last census, which label the mass floors; `null` before the first
   * answer, when the floors are named by their layer letters alone.
   */
  readonly bands: ReadonlyArray<LayerBand> | null;
  /** Which systems are shown, by what their primary is now. */
  readonly starFilter: StarFilter;
  readonly onStarFilter: (filter: StarFilter) => void;
  /**
   * Why the controls that query the server are held back, such as `NO CARRIER`, shown above them;
   * `null` when they act. The `STARS` filter asks nothing of the server and is never held back.
   */
  readonly heldBack: string | null;
}

/** The five layers, lightest first, for the mass floors before the first census arrives. */
const LAYERS: ReadonlyArray<MassLayer> = ["a", "b", "c", "d", "e"];

/** The floors a chart offers, from the census's bands or from the layer letters alone. */
function floors(
  bands: ReadonlyArray<LayerBand> | null,
): ReadonlyArray<{ readonly layer: MassLayer; readonly text: string }> {
  if (bands === null) {
    return LAYERS.map((layer) => ({ layer, text: layer.toUpperCase() }));
  }
  return bands.map((band) => ({ layer: band.layer, text: formatBandMsun(band.minMsun) }));
}

/**
 * What a chart is asked for: its query radius, its mass floor, the drive range it colours by, and
 * its time.
 *
 * @remarks
 * The radius steps 1-2-5 from 0.01 ly to 500 ly, the scale from the galactic centre out; until the
 * operator chooses one it follows the drive range, raised to the next step. The mass floor is a
 * radio group of the five layers' lower edges, which is also the declutter control; its unit is
 * drawn once on the group, since five options with their own `M☉` do not fit the chart's column.
 * The floors read as layer letters until the first census brings their bands. The drive range is an
 * operator setting, not a reading (plan 05, design note D9), and says `SET` wherever it is written.
 * The time is universe time, labelled `UT`, within the clock window of ±1,000 years. Both numbers
 * are entered when their field is left or with `Enter`; one outside its range is refused in words
 * and nothing is asked. While the link is down every control that asks the server is held back and
 * says why.
 *
 * `STARS` chooses which systems the chart, the list and the HR diagram show, `ALL`, `LIVING` or
 * `REMNANTS`, from the answer on show: a display control that asks nothing of the server, so it acts
 * with the link down. Its key, `K`, steps to the next filter from anywhere on the page but a text
 * field, and is shown on the group, as the guide asks of a frequent action.
 */
export function ChartControls({
  radiusChoiceLy,
  queryRadiusLy,
  onQueryRadius,
  minLayer,
  onMinLayer,
  driveRangeLy,
  onDriveRange,
  timeYr,
  onTime,
  bands,
  starFilter,
  onStarFilter,
  heldBack,
}: ChartControlsProps) {
  const radiusId = useId();
  const floorName = useId();
  const filterName = useId();
  const heldBackId = useId();
  const held = heldBack === null ? null : heldBackId;

  // The key reaches the selector from anywhere on the chart's page but a text field; the page is
  // under `Activity`, so the listener goes while another page is shown.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const modified = event.ctrlKey || event.altKey || event.metaKey || event.shiftKey;
      if (event.repeat || modified || event.key.toUpperCase() !== STARS_KEY) {
        return;
      }
      if (isTextEntry(event.target)) {
        return;
      }
      event.preventDefault();
      onStarFilter(nextStarFilter(starFilter));
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [starFilter, onStarFilter]);

  return (
    <div className="chart-controls">
      {heldBack === null ? null : (
        <p className="panel__inhibit" id={heldBackId}>
          {heldBack}
        </p>
      )}
      <div className="form-field chart-controls__field">
        <label className="form-field__label" htmlFor={radiusId}>
          QUERY RADIUS
        </label>
        <select
          id={radiusId}
          className="form-field__input form-field__input--number chart-controls__select"
          value={radiusChoiceLy ?? queryRadiusLy}
          aria-disabled={held === null ? undefined : "true"}
          aria-describedby={held ?? undefined}
          onChange={(event) => {
            if (held !== null) {
              return;
            }
            onQueryRadius(Number(event.target.value));
          }}
        >
          {RADIUS_STEPS_LY.map((step) => (
            <option key={step} value={step}>
              {formatScaleLength(step)}
            </option>
          ))}
        </select>
      </div>
      <fieldset className="form-choice chart-controls__floors">
        <legend>
          MIN MASS <SolarMassUnit />
        </legend>
        <div className="form-choice__options">
          {floors(bands).map(({ layer, text }) => (
            <label className="form-choice__option" key={layer}>
              <input
                type="radio"
                name={floorName}
                value={layer}
                checked={layer === minLayer}
                aria-disabled={held === null ? undefined : "true"}
                aria-describedby={held ?? undefined}
                onChange={() => {
                  if (held === null) {
                    onMinLayer(layer);
                  }
                }}
              />
              {layer === "a" ? `ALL ${text}` : text}
            </label>
          ))}
        </div>
      </fieldset>
      <fieldset className="form-choice chart-controls__stars" aria-keyshortcuts={STARS_KEY}>
        <legend>
          <span className="control__key">{STARS_KEY}</span> STARS
        </legend>
        <div className="form-choice__options">
          {STAR_FILTERS.map((filter) => (
            <label className="form-choice__option" key={filter}>
              <input
                type="radio"
                name={filterName}
                value={filter}
                checked={filter === starFilter}
                onChange={() => {
                  onStarFilter(filter);
                }}
              />
              {starFilterLabel(filter)}
            </label>
          ))}
        </div>
      </fieldset>
      {/*
       * `DRIVE RANGE`, not `DRIVE RANGE SET`: `SET` marks a drawn value as an operator setting
       * rather than a measurement (design note D9), and a field the operator types into already
       * says as much (the orchestrator's ruling 15).
       */}
      <NumberField
        label="DRIVE RANGE"
        name="DRIVE RANGE"
        text={formatChartLengthLy(driveRangeLy)}
        hint={`${formatNumber(DRIVE_RANGE_MIN_LY, 2)}-${formatNumber(DRIVE_RANGE_MAX_LY, 0)} ly`}
        valid={`enter ${formatNumber(DRIVE_RANGE_MIN_LY, 2)} to ${formatNumber(DRIVE_RANGE_MAX_LY, 0)} ly`}
        unit="ly"
        decimals={DRIVE_RANGE_DECIMALS}
        accepts={(value) => value >= DRIVE_RANGE_MIN_LY && value <= DRIVE_RANGE_MAX_LY}
        onEnter={onDriveRange}
        heldBackId={held}
      />
      <NumberField
        label="CHART TIME"
        name="CHART TIME"
        text={formatUniverseTimeYr(timeYr)}
        hint={`±${formatNumber(CLOCK_WINDOW_YR, 0)} yr`}
        valid={`enter ${formatNumber(-CLOCK_WINDOW_YR, 0)} to ${formatNumber(CLOCK_WINDOW_YR, 0)} yr`}
        unit="yr"
        before={<span className="chart-controls__time-system">UT</span>}
        decimals={TIME_DECIMALS}
        accepts={(value) => Math.abs(value) <= CLOCK_WINDOW_YR}
        onEnter={onTime}
        heldBackId={held}
      />
    </div>
  );
}
