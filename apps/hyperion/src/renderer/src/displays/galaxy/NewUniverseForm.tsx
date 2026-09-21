import type { SeedHex } from "@hyperion/protocol";
import { type FormEvent, useId, useLayoutEffect, useRef, useState } from "react";

import { DisclosureGlyph } from "../../components/DisclosureGlyph";
import { RequestStatus } from "../../components/RequestStatus";
import { parseSeedHex } from "../../lib/seed";
import type { UniverseCommandState } from "../../lib/universe";

/** The longest name the server accepts, in characters after trimming (plan 04, design note 16). */
const NAME_MAX_CHARACTERS = 48;

// Control characters are refused by the server; the message asks for printable characters.
const CONTROL_CHARACTER = /\p{Cc}/u;

const SURROGATE_PAIR = /[\uD800-\uDBFF][\uDC00-\uDFFF]/g;

type SeedMode = "random" | "entered";

interface Errors {
  readonly name: boolean;
  readonly seed: boolean;
}

const NO_ERRORS: Errors = { name: false, seed: false };

/**
 * The number of Unicode code points in `text`, which is how the server counts a name's
 * characters (Rust's `chars().count()`), not the number of UTF-16 units or of graphemes.
 */
function codePointCount(text: string): number {
  return text.replaceAll(SURROGATE_PAIR, "_").length;
}

/** The name as the server will store it, or `null` when the server would refuse it. */
function checkedName(text: string): string | null {
  const name = text.trim();
  const characters = codePointCount(name);
  return characters >= 1 && characters <= NAME_MAX_CHARACTERS && !CONTROL_CHARACTER.test(name)
    ? name
    : null;
}

function describedBy(ids: ReadonlyArray<string | null>): string | undefined {
  const present = ids.filter((id) => id !== null);
  return present.length > 0 ? present.join(" ") : undefined;
}

interface NewUniverseFormProps {
  /** The state of a create command given from this form, or `null` when none has been. */
  readonly command: UniverseCommandState | null;
  /** The ID the command's status takes, so that the controls it holds back can refer to it. */
  readonly commandStatusId: string;
  /** The IDs of the elements that say why `CREATE` is held back, if anything does. */
  readonly inhibitedBy: ReadonlyArray<string>;
  /** Creates the universe; the name is trimmed and checked, the seed in wire form. */
  readonly onCreate: (name: string, seed: SeedHex | null) => void;
  /** Whether the fields are shown; folded, the form is one line, its title and control. */
  readonly expanded: boolean;
  /** Shows or folds the fields. A display control: it changes nothing the server holds. */
  readonly onToggle: () => void;
}

/**
 * The `NEW UNIVERSE` form: a name, a seed drawn by the server or entered, and `CREATE`.
 *
 * @remarks
 * Its title is a display control that shows or folds the fields, with a chevron that shows which,
 * so that the rarely used form can give its room to the parameters; the panel decides when it is
 * shown. Folded fields keep what was typed in them. If the fields fold while the focus is in them,
 * as when a create opens its universe, the focus moves to the title control rather than being
 * lost. Nothing is disabled silently: `CREATE` stays available and an invalid entry is refused with a
 * message that says what is valid, tied to its field, and nothing is sent. The seed field is
 * disabled while the seed is `RANDOM`, and its hint says that the server draws it. `Enter` in
 * either field submits. `CREATE` changes what the server holds, so it has the command outline;
 * it shows `PENDING` and then the server's answer, and is held back, saying why, while the link is
 * down or a command is pending. Each hint sits on its field's label line, so that a 16-digit seed
 * has the field's full width.
 */
export function NewUniverseForm({
  command,
  commandStatusId,
  inhibitedBy,
  onCreate,
  expanded,
  onToggle,
}: NewUniverseFormProps) {
  const fieldsId = useId();
  const nameId = useId();
  const nameHintId = useId();
  const nameErrorId = useId();
  const seedModeName = useId();
  const seedId = useId();
  const seedHintId = useId();
  const seedErrorId = useId();
  const [name, setName] = useState("");
  const [seedMode, setSeedMode] = useState<SeedMode>("random");
  const [seedText, setSeedText] = useState("");
  const [errors, setErrors] = useState<Errors>(NO_ERRORS);
  const toggleRef = useRef<HTMLButtonElement>(null);
  const fieldsRef = useRef<HTMLDivElement>(null);
  const inhibited = inhibitedBy.length > 0;

  // A fold the operator did not ask for would leave the focus on a hidden control; before paint,
  // it moves to the control that shows the fields again.
  useLayoutEffect(() => {
    const focused = document.activeElement;
    if (!expanded && focused !== null && fieldsRef.current?.contains(focused) === true) {
      toggleRef.current?.focus();
    }
  }, [expanded]);

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (inhibited) {
      return;
    }
    const checked = checkedName(name);
    const seed = seedMode === "random" ? null : parseSeedHex(seedText);
    const seedValid = seed === null || seed.ok;
    setErrors({ name: checked === null, seed: !seedValid });
    if (checked === null || seed?.ok === false) {
      return;
    }
    onCreate(checked, seed === null ? null : seed.seed);
  };

  return (
    <form className="universe-form" noValidate onSubmit={submit}>
      <fieldset className="universe-form__fieldset">
        <legend className="universe-form__legend">
          <button
            ref={toggleRef}
            type="button"
            className="control disclosure"
            aria-expanded={expanded}
            aria-controls={fieldsId}
            onClick={onToggle}
          >
            <DisclosureGlyph expanded={expanded} />
            NEW UNIVERSE
          </button>
        </legend>
        <div className="universe-form__fields" id={fieldsId} ref={fieldsRef} hidden={!expanded}>
          <div className="form-field">
            <div className="form-field__head">
              <label className="form-field__label" htmlFor={nameId}>
                NAME
              </label>
              <span className="form-field__hint" id={nameHintId}>
                1-48 CHARACTERS
              </span>
            </div>
            <input
              id={nameId}
              className="form-field__input"
              type="text"
              autoComplete="off"
              spellCheck={false}
              value={name}
              aria-invalid={errors.name ? "true" : undefined}
              aria-describedby={describedBy([nameHintId, errors.name ? nameErrorId : null])}
              onChange={(event) => {
                setName(event.target.value);
                setErrors((previous) => ({ ...previous, name: false }));
              }}
            />
            {errors.name ? (
              <p className="form-field__error" id={nameErrorId}>
                NAME INVALID: enter 1 to 48 printable characters
              </p>
            ) : null}
          </div>
          <fieldset className="form-choice">
            <legend className="form-field__label">SEED</legend>
            <div className="form-choice__options">
              <label className="form-choice__option">
                <input
                  type="radio"
                  name={seedModeName}
                  value="random"
                  checked={seedMode === "random"}
                  onChange={() => {
                    setSeedMode("random");
                    setErrors((previous) => ({ ...previous, seed: false }));
                  }}
                />
                RANDOM
              </label>
              <label className="form-choice__option">
                <input
                  type="radio"
                  name={seedModeName}
                  value="entered"
                  checked={seedMode === "entered"}
                  onChange={() => {
                    setSeedMode("entered");
                  }}
                />
                ENTERED
              </label>
            </div>
          </fieldset>
          <div className="form-field">
            <div className="form-field__head">
              <label className="form-field__label" htmlFor={seedId}>
                SEED VALUE
              </label>
              <span className="form-field__hint" id={seedHintId}>
                {seedMode === "random" ? "DRAWN BY SERVER" : "1-16 HEXADECIMAL DIGITS"}
              </span>
            </div>
            <input
              id={seedId}
              className="form-field__input form-field__input--number"
              type="text"
              autoComplete="off"
              spellCheck={false}
              value={seedText}
              disabled={seedMode === "random"}
              aria-invalid={errors.seed ? "true" : undefined}
              aria-describedby={describedBy([seedHintId, errors.seed ? seedErrorId : null])}
              onChange={(event) => {
                setSeedText(event.target.value);
                setErrors((previous) => ({ ...previous, seed: false }));
              }}
            />
            {errors.seed ? (
              <p className="form-field__error" id={seedErrorId}>
                SEED INVALID: enter 1 to 16 hexadecimal digits (0-9, A-F)
              </p>
            ) : null}
          </div>
          <div className="universe-form__actions">
            <button
              type="submit"
              className="command"
              // Held back rather than disabled, so that it keeps focus and can say why.
              aria-disabled={inhibited ? "true" : undefined}
              aria-describedby={describedBy(inhibitedBy)}
            >
              CREATE
            </button>
            {command === null ? null : <RequestStatus state={command} id={commandStatusId} />}
          </div>
        </div>
      </fieldset>
    </form>
  );
}
