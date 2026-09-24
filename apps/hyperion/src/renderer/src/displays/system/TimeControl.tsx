import { useEffect, useId } from "react";

import { formatUniverseTimeDhms } from "../../lib/format";
import { isTextEntry } from "../../lib/textEntry";
import { DISPLAY_TIME_LABEL, TIME_STEPS } from "./displayTime";
import type { DisplayTime } from "./useDisplayTime";

/** The keys that step the display time back and ahead. */
const BACK_KEY = "[";
const AHEAD_KEY = "]";

/** The key that returns the display time to the time the display was opened at. */
const RESET_KEY = "R";

/** The words said when the display time has reached the edge of the clock window (plan 14, D24). */
const CLOCK_WINDOW_LIMIT = "CLOCK WINDOW LIMIT";

/** Props of {@link TimeControl}. */
export interface TimeControlProps {
  readonly time: DisplayTime;
}

/**
 * The `DISPLAY TIME` panel: the display's own universe time, and the controls that step it (plan 14,
 * D24 and P14.T44.a).
 *
 * @remarks
 * These controls change what the console shows and nothing on the ship, so they are display
 * controls (`.control`), as the guide requires. The time reads `DISPLAY TIME UT +12 yr 183/14:08:33`
 * in the `MET` form of D24, the owner's draft, and never moves unless it is stepped; it stands in
 * an `output`, so that each step's new time is announced once. A step is
 * chosen from `1 h` to `100 yr`, keys `1` to `6`, and the time moves one step back or ahead with
 * `[` and `]`, which repeat while held; `RESET`, key `R`, returns it to the time the display was
 * opened at. At the edge of the clock window, ±1000 yr, the time stops, `CLOCK WINDOW LIMIT` is said,
 * and the step towards the edge is held back and described by it; the words stand in an `output`
 * that is always there, so that they are announced as they appear. Every control shows its key.
 */
export function TimeControl({ time }: TimeControlProps) {
  const titleId = useId();
  const limitId = useId();
  const labelId = useId();
  const { step, reset, chooseStep, stepIndex, limit } = time;
  const chosen = TIME_STEPS[stepIndex];
  const backHeld = limit === "start";
  const aheadHeld = limit === "end";

  // The keys act from anywhere on the display but a text field; the steps repeat while held, and
  // the redraw scheduler takes them a frame at a time.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.ctrlKey || event.altKey || event.metaKey || isTextEntry(event.target)) {
        return;
      }
      if (event.key === BACK_KEY || event.key === AHEAD_KEY) {
        event.preventDefault();
        step(event.key === BACK_KEY ? -1 : 1);
        return;
      }
      if (event.repeat || event.shiftKey) {
        return;
      }
      if (event.key.toUpperCase() === RESET_KEY) {
        event.preventDefault();
        reset();
        return;
      }
      const index = TIME_STEPS.findIndex((candidate) => candidate.key === event.key);
      if (index >= 0) {
        event.preventDefault();
        chooseStep(index);
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [step, reset, chooseStep]);

  return (
    <section className="panel system__time time-control" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        Display time
      </h2>
      <p className="field time-control__time">
        <span className="field__label" id={labelId}>
          {DISPLAY_TIME_LABEL}
        </span>{" "}
        {/*
         * A live value, announced once as each step lands, since the step keys act from anywhere on
         * the display; the copy in the map's furniture is not a live region.
         */}
        <output className="field__value time-control__value" aria-labelledby={labelId}>
          {formatUniverseTimeDhms(time.time)}
        </output>
      </p>
      <div className="time-control__controls">
        <fieldset className="time-control__steps" aria-label="Step">
          {TIME_STEPS.map((candidate, index) => (
            <button
              key={candidate.label}
              type="button"
              className="control"
              aria-pressed={index === stepIndex}
              aria-keyshortcuts={candidate.key}
              onClick={() => {
                chooseStep(index);
              }}
            >
              <span className="control__key">{candidate.key}</span> {candidate.label}
            </button>
          ))}
        </fieldset>
        <div className="time-control__moves">
          <button
            type="button"
            className="control"
            aria-keyshortcuts={BACK_KEY}
            // Held back rather than disabled, so that it keeps its focus and can say why.
            aria-disabled={backHeld ? "true" : undefined}
            aria-describedby={backHeld ? limitId : undefined}
            onClick={() => {
              step(-1);
            }}
          >
            <span className="control__key">{BACK_KEY}</span> -{chosen?.label}
          </button>
          <button
            type="button"
            className="control"
            aria-keyshortcuts={AHEAD_KEY}
            aria-disabled={aheadHeld ? "true" : undefined}
            aria-describedby={aheadHeld ? limitId : undefined}
            onClick={() => {
              step(1);
            }}
          >
            <span className="control__key">{AHEAD_KEY}</span> +{chosen?.label}
          </button>
          <button type="button" className="control" aria-keyshortcuts={RESET_KEY} onClick={reset}>
            <span className="control__key">{RESET_KEY}</span> RESET
          </button>
        </div>
        <output className="time-control__limit" id={limitId} aria-label="Clock window">
          {limit === null ? null : CLOCK_WINDOW_LIMIT}
        </output>
      </div>
    </section>
  );
}
