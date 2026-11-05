import { type KeyboardEvent, useId } from "react";

import { StatusText } from "../../components/StatusText";
import { isEmptyEntry, showSelected } from "../../lib/textEntry";
import { useEntryDraft } from "./useEntryDraft";
import { formatSunDegrees, type SunSetting } from "./viewerScene";

/** Props of {@link SunControl}. */
export interface SunControlProps {
  readonly sun: SunSetting;
  readonly onSun: (sun: SunSetting) => void;
}

/** The held sun as it is first held: over the planet's prime meridian on its equator. */
const FIRST_HELD: SunSetting = { kind: "held", latitudeDeg: 0, longitudeDeg: 0 };

/**
 * The `PLANET VIEWER`'s `SUN` (plan R15, T4), in the slot the `INPUTS` panel keeps for it: where
 * the planet's light comes from, a viewing setting of the display and no input of the planet's.
 *
 * @remarks
 * `ORBITAL`, as the display mounts, stands the sun where the system's records put it, so that the
 * planet turns under it; `HELD` holds it over the subsolar point at `LATITUDE` and `LONGITUDE` on the
 * planet's ground (`viewerScene.ts`), so that its day side holds still, first over the prime
 * meridian on the equator. Each field enters its value with `Enter` or when it is left, refusing
 * one out of range with what is valid, as the panel's one refusal ({@link useEntryDraft}); `Escape`, or
 * `Enter` in the emptied field, drops what was typed. A change moves the light at the view's next
 * frame.
 */
export function SunControl({ sun, onSun }: SunControlProps) {
  return (
    <fieldset className="planet-inputs__group planet-sun">
      <legend className="planet-inputs__legend planet-sun__legend">
        <span>SUN</span>
        <span className="preset-buttons planet-sun__modes">
          <button
            type="button"
            className="control preset-buttons__button"
            aria-pressed={sun.kind === "orbital"}
            onClick={() => {
              onSun({ kind: "orbital" });
            }}
          >
            ORBITAL
          </button>
          <button
            type="button"
            className="control preset-buttons__button"
            aria-pressed={sun.kind === "held"}
            onClick={() => {
              if (sun.kind !== "held") {
                onSun(FIRST_HELD);
              }
            }}
          >
            HELD
          </button>
        </span>
      </legend>
      {sun.kind === "held" ? (
        <>
          <DegreesRow
            label="LATITUDE"
            valueDeg={sun.latitudeDeg}
            minDeg={-90}
            maxDeg={90}
            onEnter={(latitudeDeg) => {
              onSun({ ...sun, latitudeDeg });
            }}
          />
          <DegreesRow
            label="LONGITUDE"
            valueDeg={sun.longitudeDeg}
            minDeg={0}
            maxDeg={360}
            onEnter={(longitudeDeg) => {
              onSun({ ...sun, longitudeDeg });
            }}
          />
        </>
      ) : null}
    </fieldset>
  );
}

/** Props of {@link DegreesRow}. */
interface DegreesRowProps {
  readonly label: string;
  readonly valueDeg: number;
  readonly minDeg: number;
  readonly maxDeg: number;
  readonly onEnter: (valueDeg: number) => void;
}

/** A decimal as the operator types one: an optional sign, digits, an optional point and digits. */
const TYPED_DEGREES = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/u;

/** One angle's row: its label, its field in degrees and its unit. */
function DegreesRow({ label, valueDeg, minDeg, maxDeg, onEnter }: DegreesRowProps) {
  const fieldId = useId();
  const errorId = useId();
  const { draft, refusal, type, drop, refuse } = useEntryDraft(false);
  const text = formatSunDegrees(valueDeg);
  const enter = (input: HTMLInputElement): void => {
    if (draft === null) {
      return;
    }
    if (isEmptyEntry(draft)) {
      drop();
      showSelected(input, text);
      return;
    }
    const value = TYPED_DEGREES.test(draft.trim()) ? Number(draft.trim()) : Number.NaN;
    if (value >= minDeg && value <= maxDeg) {
      drop();
      onEnter(value);
    } else {
      refuse(`${label} INVALID: enter ${formatSunDegrees(minDeg)} to ${formatSunDegrees(maxDeg)}`);
    }
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "Enter") {
      event.preventDefault();
      enter(event.currentTarget);
    } else if (event.key === "Escape") {
      event.preventDefault();
      drop();
      showSelected(event.currentTarget, text);
    }
  };
  return (
    <div className="planet-inputs__row">
      <label className="planet-inputs__label" htmlFor={fieldId}>
        {label}
      </label>
      <input
        id={fieldId}
        className="form-field__input form-field__input--number"
        type="text"
        inputMode="decimal"
        autoComplete="off"
        spellCheck={false}
        value={draft ?? text}
        aria-invalid={refusal === null ? undefined : "true"}
        aria-describedby={refusal === null ? undefined : errorId}
        onChange={(event) => {
          type(event.target.value);
        }}
        onFocus={(event) => {
          event.currentTarget.select();
        }}
        onKeyDown={onKeyDown}
        onBlur={(event) => {
          enter(event.currentTarget);
        }}
      />
      <span className="planet-inputs__unit">°</span>
      <span className="planet-inputs__provenance" />
      {refusal === null ? null : (
        <p className="form-field__error planet-inputs__error" id={errorId}>
          <StatusText text={refusal} />
        </p>
      )}
    </div>
  );
}
