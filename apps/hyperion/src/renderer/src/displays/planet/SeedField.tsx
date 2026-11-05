import type { SeedHex } from "@hyperion/protocol";
import { useId } from "react";

import { StatusText } from "../../components/StatusText";
import { formatHex64, parseSeedHex } from "../../lib/seed";
import { isEmptyEntry, showSelected } from "../../lib/textEntry";
import { useEntryDraft } from "./useEntryDraft";

/** Props of {@link SeedField}. */
export interface SeedFieldProps {
  readonly label: string;
  readonly seed: SeedHex;
  readonly serverRefusal: string | null;
  readonly onEnter: (seed: SeedHex) => void;
}

/**
 * A seed's field and its label, on the `PLANET VIEWER` (plan R15): 1 to 16 hexadecimal digits in
 * either case, shown as 16 in upper case, entered with `Enter` or when it is left, refused with
 * what is valid; `Escape`, or `Enter` in the emptied field, drops what was typed. On the `INPUTS`
 * panel its refusal is the panel's one ({@link useEntryDraft}).
 */
export function SeedField({ label, seed, serverRefusal, onEnter }: SeedFieldProps) {
  const fieldId = useId();
  const errorId = useId();
  const { draft, refusal: entryRefusal, type, drop, refuse } = useEntryDraft(false);
  const text = formatHex64(seed);
  const enter = (input: HTMLInputElement): void => {
    if (draft === null) {
      return;
    }
    if (isEmptyEntry(draft)) {
      drop();
      showSelected(input, text);
      return;
    }
    const parsed = parseSeedHex(draft);
    if (parsed.ok) {
      drop();
      onEnter(parsed.seed);
    } else {
      refuse(`${label} INVALID: enter 1 to 16 hexadecimal digits (0-9, A-F)`);
    }
  };
  const refusal =
    entryRefusal ?? (serverRefusal === null ? null : `${label} REJECTED: ${serverRefusal}`);
  return (
    <div className="planet-inputs__row planet-inputs__row--seed">
      <label className="planet-inputs__label" htmlFor={fieldId}>
        {label}
      </label>
      <input
        id={fieldId}
        className="form-field__input form-field__input--number"
        type="text"
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
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            enter(event.currentTarget);
          } else if (event.key === "Escape") {
            event.preventDefault();
            drop();
            showSelected(event.currentTarget, text);
          }
        }}
        onBlur={(event) => {
          enter(event.currentTarget);
        }}
      />
      {refusal === null ? null : (
        <p className="form-field__error planet-inputs__error" id={errorId}>
          <StatusText text={refusal} />
        </p>
      )}
    </div>
  );
}
