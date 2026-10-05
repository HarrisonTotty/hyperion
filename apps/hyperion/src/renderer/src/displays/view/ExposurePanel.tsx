import { useId, useRef, useState } from "react";

import { formatNumber, formatSignificant } from "../../lib/format";
import { isEmptyEntry, showSelected } from "../../lib/textEntry";
import {
  controlEv100,
  type ExposureCommandResult,
  type ExposureControl,
  type ExposureTriple,
  enable,
  inhibit,
  MAN_EV100_MAX,
  MAN_EV100_MIN,
  setManualEv100,
  shownTriple,
  VIEW_CAMERA,
} from "../../view/photometry/exposure";
import { exposureReading } from "./viewRun";

/** Props of {@link ExposurePanel}. */
export interface ExposurePanelProps {
  /** The panel's ID, by which a disclosure button controls it (R07.T19.b), or none. */
  readonly id?: string | undefined;
  /** Whether the panel is folded behind its disclosure button in the compact layout (R07.T19.b). */
  readonly hidden?: boolean | undefined;
  /**
   * The view the panel acts on, its system designator on the title row (`PRIMARY`,
   * `INSTRUMENT 1`; R07.T19), or none.
   */
  readonly designator?: string | undefined;
  readonly exposure: ExposureControl;
  /**
   * The metering source's value, EV100, or `null` where there is none: always `null` for a
   * wireframe view until R07's photorealistic view accompanies it.
   */
  readonly meteredEv100: number | null;
  /** Called with the control after an accepted command. */
  readonly onChange: (exposure: ExposureControl) => void;
}

/** The decimals a `MAN` entry keeps and shows: one, the reading's precision. */
const MAN_DECIMALS = 1;

/** The span a `MAN` entry may take, as its field's hint shows it: `-14.0 to 42.0`. */
const MAN_SPAN = [MAN_EV100_MIN, MAN_EV100_MAX]
  .map((ev100) => formatNumber(ev100, MAN_DECIMALS))
  .join(" to ");

/**
 * The status while there is no image to meter; it stands under the compact layout's row while this
 * panel is folded (R07.T19.b).
 */
export const AUTO_NOT_AVAILABLE = "AUTO NOT AVAILABLE: NO IMAGE TO METER";

/** Why a command is held back or an entry refused, in the guide's `STATUS: clause` form. */
const REFUSAL_WORDS: Readonly<
  Record<Extract<ExposureCommandResult, { kind: "refused" }>["reason"], string>
> = {
  no_image_to_meter: "NO IMAGE TO METER",
  not_automatic: "NOT AVAILABLE: the exposure is MAN",
  already_auto: "NOT AVAILABLE: the exposure is AUTO",
  invalid_triple: "NOT AVAILABLE: the triple is not valid",
  invalid_ev100: `EV100 INVALID: enter ${MAN_SPAN}`,
};

/** An entry that is not a number, refused as one outside the span is. */
const NOT_A_NUMBER: ExposureCommandResult = { kind: "refused", reason: "invalid_ev100" };

/**
 * A number as typed, as `CURSOR`'s fields take it, once its grouping commas are dropped: digits, a
 * decimal part and a sign, `+`, `-` or `−`.
 */
const TYPED_NUMBER = /^[+\-−]?(?:\d+(?:\.\d*)?|\.\d+)$/u;

/**
 * The EV100 a `MAN` entry's text stands for, kept to {@link MAN_DECIMALS} before it is checked so
 * that what is entered is what is shown, or `null` where it is not a number.
 */
function parseEv100(text: string): number | null {
  const compact = text.trim().replaceAll(",", "");
  if (!TYPED_NUMBER.test(compact)) {
    return null;
  }
  const scale = 10 ** MAN_DECIMALS;
  return Math.round(Number(compact.replace("−", "-")) * scale) / scale;
}

/** What entering a `MAN` field's text gives: `MAN` at its EV100, or the refusal. */
function manualEntry(text: string): ExposureCommandResult {
  const ev100 = parseEv100(text);
  return ev100 === null ? NOT_A_NUMBER : setManualEv100(ev100);
}

/**
 * The cost of leaving `AUTO` by the operator's act, which the `MAN` field's consequence and
 * `INHIBIT`'s share, written once (decision-r07-t13d).
 */
const ENABLE_ONLY = "AUTO resumes only on ENABLE";

/** What entering a value does, which the `MAN` field states while the exposure is not `MAN`. */
const MAN_CONSEQUENCE = `Entering a value sets MAN: ${ENABLE_ONLY}`;

/**
 * What `INHIBIT` does, which it states beside its button under `AUTO` and under a system inhibit,
 * which it takes over (decision-r07-t13d). "Then" makes it the press's sequel, not a held-back
 * reason, whose place and look it takes.
 */
const INHIBIT_CONSEQUENCE = `Then ${ENABLE_ONLY}`;

interface ExposureCommandProps {
  readonly label: string;
  readonly result: ExposureCommandResult;
  /**
   * The consequence the command states beside its button while it is accepted, in a held-back
   * reason's place, or none.
   */
  readonly consequence?: string | undefined;
  readonly onChange: (exposure: ExposureControl) => void;
}

/**
 * A command button, held back with its reason where the command would be refused, or offered with
 * its consequence, if it has one, in the reason's place; either describes the button.
 */
function ExposureCommand({ label, result, consequence, onChange }: ExposureCommandProps) {
  const noteId = useId();
  const refused = result.kind === "refused";
  const note = refused ? REFUSAL_WORDS[result.reason] : consequence;
  return (
    <div className="view-exposure__command">
      <button
        type="button"
        className="control"
        // Held back rather than disabled, so that it keeps focus and can say why.
        aria-disabled={refused ? "true" : undefined}
        aria-describedby={note === undefined ? undefined : noteId}
        onClick={() => {
          if (result.kind === "accepted") {
            onChange(result.control);
          }
        }}
      >
        {label}
      </button>
      {note === undefined ? null : (
        <span
          className={
            refused
              ? "view-exposure__reason"
              : "view-exposure__reason view-exposure__reason--consequence"
          }
          id={noteId}
        >
          {note}
        </span>
      )}
    </div>
  );
}

/** The guide's off-scale mark after a pegged value, `↑`, announced as "off scale high". */
function OffScaleHigh() {
  return (
    <>
      {" "}
      <span aria-hidden="true">↑</span>
      <span className="visually-hidden"> off scale high</span>
    </>
  );
}

interface CameraSettingProps {
  readonly triple: ExposureTriple;
}

/**
 * The camera's `APERTURE`, `SHUTTER`, `ND` and `ISO`, a member beyond the view camera's range
 * pegged at its end with the guide's off-scale `↑` (decision-r07-exposure-camera, item d).
 */
function CameraSetting({ triple }: CameraSettingProps) {
  const ndEv = triple.ndEv ?? 0;
  const ndPegged = ndEv > VIEW_CAMERA.maxNdEv;
  const isoPegged = triple.iso > VIEW_CAMERA.maxIso;
  return (
    <dl className="readout view-exposure__triple">
      <dt>APERTURE</dt>
      <dd>
        <output>f/{formatNumber(triple.aperture, 1)}</output>
      </dd>
      <dt>SHUTTER</dt>
      <dd>
        <output>{formatSignificant(triple.shutterS)} s</output>
      </dd>
      <dt>ND</dt>
      <dd>
        <output>
          {ndEv === 0 ? (
            "CLEAR"
          ) : (
            <>
              {formatNumber(ndPegged ? VIEW_CAMERA.maxNdEv : ndEv, 1)} EV
              {ndPegged ? <OffScaleHigh /> : null}
            </>
          )}
        </output>
      </dd>
      <dt>ISO</dt>
      <dd>
        <output>
          {formatNumber(isoPegged ? VIEW_CAMERA.maxIso : triple.iso, 0)}
          {isoPegged ? <OffScaleHigh /> : null}
        </output>
      </dd>
    </dl>
  );
}

/** The `MAN` field's input, a number field. */
const MAN_INPUT = "form-field__input form-field__input--number view-exposure__man-input";

interface ManualEntryProps {
  readonly exposure: ExposureControl;
  readonly onChange: (exposure: ExposureControl) => void;
}

/**
 * The `MAN` field (decision-r07-man-exposure): the operator's exposure in EV100, `-14.0` to `42.0`
 * at one decimal, which sets `MAN` from any level when it is entered.
 *
 * @remarks
 * It shows `MAN`'s value, or `—` at another level, where there is no operator value. Focused, it
 * fills with the exposure as it stands, selected so that typing replaces it; `Enter` enters what it
 * shows, typed or filled in, and a typed value is also entered when the field is left, as
 * `CURSOR`'s fields are; tabbing through enters nothing. An entry starts from the exposure as it
 * stands, so entering the value filled in changes the image by at most its rounding. While the
 * exposure is not `MAN` the field states the consequence. A value that is not a number or lies
 * outside the span is refused, saying what is valid, and the exposure is unchanged. It is never
 * held back: a view with nothing to meter offers `MAN`.
 *
 * Its way out, every entry field's (decision-r07-t13d), enters nothing. `Escape` drops what was
 * typed and any refusal and takes the fill again, selected. Empty text is no entry and is never
 * refused: `Enter` on it takes the fill again, and leaving the field with it shows `MAN`'s value or
 * `—`. A refused text that was typed is the operator's and stays, marked, until it is edited or
 * dropped; a refused fill, the exposure as it stands beyond the span, where `AUTO` can run, is the
 * console's, not a draft, and leaving the field drops it and its refusal. The fill is never clamped
 * into the span, which would show a value the exposure does not have.
 */
function ManualEntry({ exposure, onChange }: ManualEntryProps) {
  const fieldId = useId();
  const unitId = useId();
  const hintId = useId();
  const consequenceId = useId();
  const errorId = useId();
  // What the operator has typed and not entered, kept while it is refused.
  const [draft, setDraft] = useState<string | null>(null);
  // The exposure as it stood when the field took focus, shown while it holds focus untyped.
  const [fill, setFill] = useState<string | null>(null);
  const [refused, setRefused] = useState(false);
  // Whether a press is focusing the field, whose release must not undo the fill's selection.
  const pressFocusing = useRef(false);

  const manual = exposure.kind === "manual";
  const operatorValue = manual ? formatNumber(controlEv100(exposure), MAN_DECIMALS) : null;

  // A value set from elsewhere, by ENABLE or an entry, replaces what was typed and its refusal.
  const [seenValue, setSeenValue] = useState(operatorValue);
  if (seenValue !== operatorValue) {
    setSeenValue(operatorValue);
    setDraft(null);
    setRefused(false);
  }

  // Drops what was typed and any refusal: the way out, which enters nothing.
  const drop = (): void => {
    setDraft(null);
    setRefused(false);
  };

  // Takes the fill, the exposure as it stands, selected so that typing replaces it.
  const takeFill = (input: HTMLInputElement): void => {
    const text = formatNumber(controlEv100(exposure), MAN_DECIMALS);
    setFill(text);
    showSelected(input, text);
  };

  // Enters what the field shows, typed or the fill: `MAN` at its value, or its refusal; empty text
  // is no entry.
  const enter = (text: string): void => {
    if (isEmptyEntry(text)) {
      drop();
      return;
    }
    const result = manualEntry(text);
    if (result.kind === "refused") {
      // The draft stays as it is: a typed text is kept, and a refused fill is never made a draft,
      // so that leaving the field drops it.
      setRefused(true);
      return;
    }
    setDraft(null);
    setFill(null);
    setRefused(false);
    onChange(result.control);
  };

  const shown = draft ?? fill ?? operatorValue;
  const describedBy = [unitId, hintId, manual ? null : consequenceId, refused ? errorId : null]
    .filter((id): id is string => id !== null)
    .join(" ");

  return (
    <div className="form-field view-exposure__man">
      <div className="view-exposure__man-field">
        <label className="form-field__label" htmlFor={fieldId}>
          MAN
        </label>
        <input
          id={fieldId}
          className={shown === null ? `${MAN_INPUT} view-exposure__man-input--missing` : MAN_INPUT}
          type="text"
          inputMode="decimal"
          autoComplete="off"
          spellCheck={false}
          value={shown ?? "—"}
          aria-invalid={refused ? "true" : undefined}
          aria-describedby={describedBy}
          onFocus={(event) => {
            if (draft === null) {
              takeFill(event.currentTarget);
            } else {
              // A refused text met again, selected so that typing replaces it.
              event.currentTarget.select();
            }
          }}
          onMouseDown={(event) => {
            pressFocusing.current =
              event.currentTarget.ownerDocument.activeElement !== event.currentTarget;
          }}
          onMouseUp={(event) => {
            // A press that focuses the field would otherwise collapse the fill's selection.
            if (pressFocusing.current) {
              pressFocusing.current = false;
              event.preventDefault();
            }
          }}
          onChange={(event) => {
            setDraft(event.target.value);
            setRefused(false);
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              drop();
              takeFill(event.currentTarget);
            } else if (event.key === "Enter") {
              event.preventDefault();
              if (shown !== null) {
                enter(shown);
                if (isEmptyEntry(shown)) {
                  takeFill(event.currentTarget);
                }
              }
            }
          }}
          onBlur={() => {
            if (draft === null) {
              // Only a refused fill can stand untyped, and it is not the operator's to keep.
              setRefused(false);
            } else {
              enter(draft);
            }
            setFill(null);
          }}
        />
        <span className="view-exposure__man-unit" id={unitId}>
          EV100
        </span>
        <span className="form-field__hint view-exposure__man-hint" id={hintId}>
          {MAN_SPAN}
        </span>
      </div>
      {manual ? null : (
        <p className="view-exposure__consequence" id={consequenceId}>
          {MAN_CONSEQUENCE}
        </p>
      )}
      {refused ? (
        <p className="form-field__error view-exposure__man-error" id={errorId}>
          {REFUSAL_WORDS.invalid_ev100}
        </p>
      ) : null}
    </div>
  );
}

/**
 * The view's exposure as an instrument (plan R02, R02.T15.c; Design note 11): the value with its
 * unit and automation level, `EV100 -1.0 MAN`, the camera's setting at every level (`APERTURE`,
 * `SHUTTER`, `ND`, `ISO`; R07.T13.c), the `MAN` field that enters the operator's value (R07.T13.d),
 * and the congruent pair `ENABLE` and `INHIBIT`, in the guide's order, each held back with its
 * reason where it would be refused. There is no button named `AUTO` nor one named `MAN` (the
 * guide's "Controls and commanding"); while there is no image to meter, `AUTO NOT AVAILABLE`
 * stands with `NO IMAGE TO METER`.
 *
 * @remarks
 * Display controls: the exposure is the view's own, not the ship's, so a command acts at once. The
 * setting is `MAN`'s triple, else the view camera's program at the shown EV100, so it changes only
 * with the published control. Under `AUTO` and a system inhibit `INHIBIT` states its consequence,
 * `Then AUTO resumes only on ENABLE`, beside the button (R07.T13.d's follow-up).
 */
export function ExposurePanel({
  exposure,
  meteredEv100,
  onChange,
  designator,
  id,
  hidden,
}: ExposurePanelProps) {
  const titleId = useId();
  // `INHIBIT` shuts the automation down under `AUTO` and takes a system inhibit over, so it states
  // its consequence there; under the operator's own inhibit it changes nothing (decision-r07-t13d).
  const inhibitStates =
    exposure.kind === "auto" || (exposure.kind === "inhibited" && exposure.reason !== "operator");
  return (
    <section className="panel view-exposure" aria-labelledby={titleId} id={id} hidden={hidden}>
      <h2 className="panel__title" id={titleId}>
        Exposure
        {designator === undefined ? null : (
          <>
            {" "}
            <span className="panel__designator">{designator}</span>
          </>
        )}
      </h2>
      <p className="view-exposure__reading">
        <output>{exposureReading(exposure)}</output>
      </p>
      <CameraSetting triple={shownTriple(exposure)} />
      <ManualEntry exposure={exposure} onChange={onChange} />
      {meteredEv100 === null ? <p className="view-exposure__reason">{AUTO_NOT_AVAILABLE}</p> : null}
      <div className="view-exposure__commands">
        <ExposureCommand
          label="ENABLE"
          result={enable(exposure, meteredEv100)}
          onChange={onChange}
        />
        <ExposureCommand
          label="INHIBIT"
          result={inhibit(exposure)}
          consequence={inhibitStates ? INHIBIT_CONSEQUENCE : undefined}
          onChange={onChange}
        />
      </div>
    </section>
  );
}
