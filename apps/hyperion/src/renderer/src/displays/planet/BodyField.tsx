import { type BodyIdHex, isBodyId } from "@hyperion/protocol";
import { useId } from "react";

import { RequestStatus } from "../../components/RequestStatus";
import { StatusText } from "../../components/StatusText";
import { isEmptyEntry, showSelected } from "../../lib/textEntry";
import { describedBy } from "./describedBy";
import { useEntryDraft } from "./useEntryDraft";
import type { CommandState } from "./usePlanetViewer";

/** Props of {@link BodyField}. */
export interface BodyFieldProps {
  /** The galaxy's planet on show, whose ID the field shows; `null` while none is. */
  readonly shownBody: BodyIdHex | null;
  /** Where the last `OPEN BODY` stands, until its planet is shown; else `null`. */
  readonly status: Exclude<CommandState, { readonly kind: "shown" }> | null;
  /** Why the commands are held back, or `null`, and the ID of the words that say so. */
  readonly heldBack: string | null;
  readonly heldBackId: string;
  /** Whether `SHOWN` is folded, which drops what was typed and its refusal. */
  readonly folded: boolean;
  readonly onOpen: (body: BodyIdHex) => void;
}

/** What the operator typed as a body ID, in the wire's lower case, or `null` if it is none. */
function typedBodyId(text: string): BodyIdHex | null {
  const id = text.trim().toLowerCase();
  return isBodyId(id) ? id : null;
}

/** The refusal of a typed ID of another form, with what is valid. */
const BODY_INVALID = "BODY INVALID: enter 16 hexadecimal digits, a full stop and 4 more (0-9, A-F)";

/** The form a body's ID is entered in, which the field states while it is empty. */
const BODY_FORMAT = "a body's ID: 16 hexadecimal digits, a full stop and 4 more";

/**
 * The `INPUTS` panel's `BODY` (plan R15, T5): a planet of the galaxy by its ID and `OPEN BODY`,
 * which shows it and reads its inputs back onto the panel.
 *
 * @remarks
 * The field takes the wire's form, 16 hexadecimal digits, a full stop and 4, in either case, states
 * that form while it is empty, and shows the galaxy's planet on show, in upper case as `SEED` shows
 * its seed. `OPEN BODY`, or `Enter` in the field, opens what was typed, refusing an ID of another
 * form with what is valid; `Escape`, `Enter` in the emptied field, or leaving it emptied, drops
 * what was typed, as folding `SHOWN` does. Its refusal is the panel's one ({@link useEntryDraft}).
 * `OPEN BODY`'s `PENDING`, then its outcome, stands under the row. It is held back, as `GENERATE`
 * is, with the reason while no universe is open or the link is down.
 */
export function BodyField({
  shownBody,
  status,
  heldBack,
  heldBackId,
  folded,
  onOpen,
}: BodyFieldProps) {
  const fieldId = useId();
  const errorId = useId();
  const statusId = useId();
  const formatId = useId();
  const entry = useEntryDraft(folded);
  const { draft, refusal } = entry;
  const shownText = shownBody?.toUpperCase() ?? "";
  const text = draft ?? shownText;
  /** Drops what was typed, the field showing its value again, selected for typing to replace. */
  const drop = (input: HTMLInputElement | null): void => {
    entry.drop();
    if (input !== null) {
      showSelected(input, shownText);
    }
  };
  const open = (input: HTMLInputElement | null): void => {
    if (isEmptyEntry(text)) {
      drop(input);
      return;
    }
    if (heldBack !== null) {
      return;
    }
    const id = typedBodyId(text);
    if (id === null) {
      entry.refuse(BODY_INVALID);
      return;
    }
    entry.drop();
    onOpen(id);
  };
  return (
    <>
      <div className="planet-inputs__generate">
        <div className="planet-inputs__row planet-inputs__row--seed">
          <label className="planet-inputs__label" htmlFor={fieldId}>
            BODY
          </label>
          <input
            id={fieldId}
            className="form-field__input planet-inputs__body"
            type="text"
            autoComplete="off"
            spellCheck={false}
            value={text}
            aria-invalid={refusal === null ? undefined : "true"}
            aria-describedby={describedBy([
              refusal === null ? null : errorId,
              text.length === 0 ? formatId : null,
            ])}
            onChange={(event) => {
              entry.type(event.target.value);
            }}
            onFocus={(event) => {
              event.currentTarget.select();
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                open(event.currentTarget);
              } else if (event.key === "Escape") {
                event.preventDefault();
                drop(event.currentTarget);
              }
            }}
            onBlur={() => {
              if (draft !== null && isEmptyEntry(draft)) {
                drop(null);
              }
            }}
          />
        </div>
        <button
          type="button"
          className="command"
          aria-disabled={heldBack === null ? undefined : "true"}
          aria-describedby={describedBy([
            heldBack === null ? null : heldBackId,
            status === null ? null : statusId,
          ])}
          onClick={() => {
            open(null);
          }}
        >
          OPEN BODY
        </button>
      </div>
      {text.length === 0 ? (
        <p className="planet-inputs__note" id={formatId}>
          {BODY_FORMAT}
        </p>
      ) : null}
      {refusal === null ? null : (
        <p className="form-field__error planet-inputs__error" id={errorId}>
          <StatusText text={refusal} />
        </p>
      )}
      {status === null ? null : <RequestStatus state={status} id={statusId} subject="OPEN BODY" />}
    </>
  );
}
