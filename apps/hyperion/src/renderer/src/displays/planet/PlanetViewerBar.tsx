import type { SeedHex } from "@hyperion/protocol";
import { useId, useRef, useState } from "react";

import { RequestStatus } from "../../components/RequestStatus";
import { StatusText } from "../../components/StatusText";
import { formatHex64 } from "../../lib/seed";
import { isEmptyEntry, showSelected } from "../../lib/textEntry";
import { formatInputsString, parseInputsString, type PlanetInputs } from "./planetInputs";
import { describedBy } from "./describedBy";
import { SeedField } from "./SeedField";
import type { ViewerCommand } from "./usePlanetViewer";

/** The key that draws a real planet's inputs from anywhere on the display but a text field. */
export const RANDOMIZE_KEY = "n";

/** Props of {@link PlanetViewerBar}. */
export interface PlanetViewerBarProps {
  /** The panel's inputs, whose string `SHARE` holds. */
  readonly inputs: PlanetInputs;
  /** Puts a pasted string's inputs on the panel. */
  readonly onLoad: (inputs: PlanetInputs) => void;
  /** The seed `RANDOMIZE` draws with next. */
  readonly drawSeed: SeedHex;
  readonly onDrawSeedChange: (seed: SeedHex) => void;
  /** The draw seed of the last draw, or `null` before the first. */
  readonly lastDraw: SeedHex | null;
  /** Why the commands are held back (no universe open, the link down), or `null`. */
  readonly heldBack: string | null;
  /** The ID of the words that say why, which the bar shows and the panel's commands cite. */
  readonly heldBackId: string;
  readonly onRandomize: () => void;
  /** The viewer's last command, whose state stands here while it is a draw. */
  readonly command: ViewerCommand | null;
  /** Whether the viewer holds no inputs (a moon on show), which holds `SHARE`'s string back. */
  readonly noInputs: boolean;
  /** The ID of the panel's words saying so (`NO INPUTS: …`), which `SHARE` cites. */
  readonly noInputsId: string;
}

/**
 * The `PLANET VIEWER`'s bar above its view (plan R15), where `VIEW`'s `SCENE` selector stands on
 * `VIEW`: `RANDOMIZE` with its `DRAW SEED` and `LAST DRAW`, and `SHARE`.
 *
 * @remarks
 * - `RANDOMIZE` (`N`) puts a real planet's inputs, drawn from the galaxy with `DRAW SEED`, on the
 *   panel and shows them; once the planet is shown, `LAST DRAW` says which seed draws it again and
 *   `DRAW SEED` moves on by one, so that each press draws another planet. Its `PENDING`, then its
 *   outcome, stand beside it. It is held back while no universe is open or the link is down, as the
 *   panel's `GENERATE` and `OPEN BODY` are, with the reason the bar states under its rows for all
 *   three, across the view's width rather than in the panel's narrow column.
 * - `SHARE` holds the panel's inputs' string, which reproduces the planet in any universe of this
 *   generator version: selected whole as it takes the focus, to copy; or paste one and `LOAD` it
 *   (`Enter`), which puts its inputs on the panel, refusing a string it cannot read with what is
 *   valid. While the viewer holds no inputs, a moon on show, its last string, another planet's, is
 *   held back with the panel's reason: muted, and refused to a copy. `LOAD` stays offered, since a
 *   pasted string is one way to enter one (the guide's Controls: a held control's reason says what
 *   to do), and its inputs on the panel offer `SHARE` again; so does `RANDOMIZE`.
 */
export function PlanetViewerBar({
  inputs,
  onLoad,
  drawSeed,
  onDrawSeedChange,
  lastDraw,
  heldBack,
  heldBackId,
  onRandomize,
  command,
  noInputs,
  noInputsId,
}: PlanetViewerBarProps) {
  const statusId = useId();
  const status =
    command !== null && command.kind === "randomize" && command.state.kind !== "shown"
      ? command.state
      : null;
  return (
    <div className="planet-viewer__bar">
      <div className="planet-viewer__draw">
        <button
          type="button"
          className="command"
          aria-disabled={heldBack === null ? undefined : "true"}
          aria-describedby={describedBy([
            heldBack === null ? null : heldBackId,
            status === null ? null : statusId,
          ])}
          aria-keyshortcuts={RANDOMIZE_KEY.toUpperCase()}
          onClick={() => {
            if (heldBack === null) {
              onRandomize();
            }
          }}
        >
          <span className="control__key">{RANDOMIZE_KEY.toUpperCase()}</span> RANDOMIZE
        </button>
        <SeedField
          label="DRAW SEED"
          seed={drawSeed}
          serverRefusal={null}
          onEnter={onDrawSeedChange}
        />
        <span className="planet-viewer__last-draw">
          <span className="field__label">LAST DRAW</span>{" "}
          <span className="field__value">{lastDraw === null ? "—" : formatHex64(lastDraw)}</span>
        </span>
        {status === null ? null : (
          <RequestStatus state={status} id={statusId} subject="RANDOMIZE" />
        )}
      </div>
      <ShareField
        // A new state on each hold, so that what was typed before it does not come back after it.
        key={noInputs ? "held" : "offered"}
        inputs={inputs}
        onLoad={onLoad}
        heldId={noInputs ? noInputsId : null}
      />
      {heldBack === null ? null : (
        <p className="planet-viewer__held" id={heldBackId}>
          <StatusText text={`NOT AVAILABLE: ${heldBack}`} />
        </p>
      )}
    </div>
  );
}

/** Props of {@link ShareField}. */
interface ShareFieldProps {
  readonly inputs: PlanetInputs;
  readonly onLoad: (inputs: PlanetInputs) => void;
  /** The ID of the reason its string is held back, or `null` while it is offered. */
  readonly heldId: string | null;
}

/**
 * The inputs' string, to copy, or a pasted one to `LOAD`. Held back with `heldId`, the string
 * shown is muted and refused to a copy or a cut until another is typed or pasted over it, and
 * `LOAD` still takes a pasted one.
 */
function ShareField({ inputs, onLoad, heldId }: ShareFieldProps) {
  const fieldId = useId();
  const errorId = useId();
  const field = useRef<HTMLInputElement | null>(null);
  const [draft, setDraft] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const text = formatInputsString(inputs);
  // The string shown is another planet's while held, until one is typed or pasted over it.
  const stale = heldId !== null && draft === null;
  const load = (): void => {
    if (draft === null || isEmptyEntry(draft)) {
      setDraft(null);
      setRefusal(null);
      return;
    }
    const parsed = parseInputsString(draft);
    if (parsed.ok) {
      setDraft(null);
      setRefusal(null);
      onLoad(parsed.inputs);
    } else {
      setRefusal(parsed.reason);
    }
  };
  return (
    <div className="planet-viewer__share">
      <label className="planet-inputs__label" htmlFor={fieldId}>
        SHARE
      </label>
      <input
        id={fieldId}
        ref={field}
        className={`form-field__input planet-viewer__string${stale ? " planet-viewer__string--held" : ""}`}
        type="text"
        autoComplete="off"
        spellCheck={false}
        value={draft ?? text}
        aria-invalid={refusal === null ? undefined : "true"}
        aria-describedby={describedBy([heldId, refusal === null ? null : errorId])}
        onChange={(event) => {
          setDraft(event.target.value);
          setRefusal(null);
        }}
        onFocus={(event) => {
          event.currentTarget.select();
        }}
        onCopy={(event) => {
          if (stale) {
            event.preventDefault();
          }
        }}
        onCut={(event) => {
          if (stale) {
            event.preventDefault();
          }
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            load();
          } else if (event.key === "Escape") {
            event.preventDefault();
            setDraft(null);
            setRefusal(null);
            showSelected(event.currentTarget, text);
          }
        }}
      />
      <button
        type="button"
        className="command"
        onClick={() => {
          load();
          field.current?.focus();
        }}
      >
        LOAD
      </button>
      {refusal === null ? null : (
        <p className="form-field__error planet-inputs__error" id={errorId}>
          <StatusText text={refusal} />
        </p>
      )}
    </div>
  );
}
