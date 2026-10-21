import { useEffect, useId } from "react";

import { isTextEntry } from "../lib/textEntry";
import { StatusText } from "./StatusText";

/**
 * What a display shows (plan 12, P12.T11): `now`, the present, or `observed`, what an observer's
 * light shows.
 */
export type DisplayMode = "now" | "observed";

/**
 * The mode control's single key, which steps to the other mode: `M`, for mode, since the chart's
 * page holds `C`, `K`, `T`, `S`, `F`, `O` and `Z` and the map's `Q` and `D`.
 */
export const MODE_KEY = "M";

/** Each mode's button, in the order the control offers them. */
const MODES: ReadonlyArray<{ readonly mode: DisplayMode; readonly label: string }> = [
  { mode: "now", label: "NOW" },
  { mode: "observed", label: "OBSERVED" },
];

/** Props of {@link ModeControl}. */
export interface ModeControlProps {
  /** The mode chosen, whose button is pressed. */
  readonly mode: DisplayMode;
  readonly onMode: (mode: DisplayMode) => void;
  /**
   * The ID of the element that says why the whole control is held back, such as the link's reason
   * shown above the controls it stands among; `null` when it acts.
   */
  readonly heldBackId: string | null;
  /**
   * Why `OBSERVED` alone is held back, shown beside the control, such as there being no chart on
   * show to observe from; `null` when it acts, or when the whole control is held back.
   */
  readonly observedHeldBack: string | null;
}

/**
 * Chooses what a display shows: `NOW` or `OBSERVED` (plan 12, P12.T11).
 *
 * @remarks
 * A display control, not a ship command: it changes only what the console shows and asks, so its
 * buttons are `.control`s. The chosen mode's button is pressed (`aria-pressed`, and a rule under it,
 * a shape as well as the state), so that the state is never carried by colour alone. Its single
 * key, `M`, steps to the other mode from anywhere on the page but a text field, and is shown on the
 * control and named by `aria-keyshortcuts`, as the `STARS` control's `K` is; a key held down or
 * pressed with a modifier is left alone. Held back, a button keeps its focus and says why: the
 * whole control with the link's reason, and `OBSERVED` alone with its own reason beside it; the
 * key then does nothing. The listener goes with the control, so a page under `Activity` drops it
 * while another page is shown.
 */
export function ModeControl({ mode, onMode, heldBackId, observedHeldBack }: ModeControlProps) {
  const reasonId = useId();
  const observedHeld = heldBackId ?? (observedHeldBack === null ? null : reasonId);
  const heldOf = (each: DisplayMode): string | null => (each === "now" ? heldBackId : observedHeld);
  const next: DisplayMode = mode === "now" ? "observed" : "now";
  const nextHeld = heldOf(next) !== null;

  // The key reaches the control from anywhere on its page but a text field.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const modified = event.ctrlKey || event.altKey || event.metaKey || event.shiftKey;
      if (event.repeat || modified || event.key.toUpperCase() !== MODE_KEY) {
        return;
      }
      if (isTextEntry(event.target)) {
        return;
      }
      event.preventDefault();
      if (!nextHeld) {
        onMode(next);
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [next, nextHeld, onMode]);

  return (
    <fieldset className="mode-control" aria-keyshortcuts={MODE_KEY}>
      <legend className="mode-control__legend">
        <span className="control__key">{MODE_KEY}</span> MODE
      </legend>
      <div className="mode-control__options">
        {MODES.map(({ mode: each, label }) => {
          const held = heldOf(each);
          return (
            <button
              key={each}
              type="button"
              className="control mode-control__button"
              aria-pressed={each === mode}
              // Held back rather than disabled, so that it keeps its focus and can say why.
              aria-disabled={held === null ? undefined : "true"}
              aria-describedby={held ?? undefined}
              onClick={() => {
                if (held === null) {
                  onMode(each);
                }
              }}
            >
              {label}
            </button>
          );
        })}
        {heldBackId !== null || observedHeldBack === null ? null : (
          <span className="mode-control__reason" id={reasonId}>
            <StatusText text={observedHeldBack} />
          </span>
        )}
      </div>
    </fieldset>
  );
}
