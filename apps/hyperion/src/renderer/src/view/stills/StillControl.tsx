import { useId } from "react";

import { StatusText } from "../../components/StatusText";
import { formatNumber } from "../../lib/format";
import { STILL_TERRAIN_STREAMING } from "./stillLabel";

/** What a still in progress is doing, with the job's whole passes done of all it has. */
export interface StillStep {
  readonly kind: "preparing" | "streaming" | "rendering" | "saving";
  readonly donePasses: number;
  readonly totalPasses: number;
}

/** How the last still ended, which the control states until the next one starts. */
export type StillEnd =
  | { readonly kind: "saved" }
  /** The still was not saved: why, and what to do, as a sentence (`the disk is full, …`). */
  | { readonly kind: "not-saved"; readonly reason: string }
  /** The graphics device was lost while it was in flight (Design note 17). */
  | { readonly kind: "abandoned" };

/** Whether a still runs, its step, or none does, with how the last one ended, if any. */
export type StillActivity =
  | { readonly kind: "running"; readonly step: StillStep }
  | { readonly kind: "idle"; readonly last: StillEnd | null };

/**
 * The stills' standing for the session (Design note 17): `available`; `suspended` after the first
 * device or GPU-process loss while a still was in flight, when only an explicit request makes one,
 * at the smallest batch; `disabled` after a second.
 */
export type StillStanding = "available" | "suspended" | "disabled";

/** What {@link StillControl} shows. */
export interface StillControlState {
  readonly standing: StillStanding;
  readonly activity: StillActivity;
  /**
   * Why `TAKE STILL` is held back besides a still in progress or stills disabled, in the guide's
   * `NOT AVAILABLE: <cause>` form, or `null`.
   */
  readonly heldBack: string | null;
}

/** Props of {@link StillControl}. */
export interface StillControlProps {
  readonly state: StillControlState;
  /** Called by `TAKE STILL` where it is offered: the request for a still. */
  readonly onTake: () => void;
}

/** The control before any still: offered, nothing in progress. */
export const STILL_CONTROL_IDLE: StillControlState = {
  standing: "available",
  activity: { kind: "idle", last: null },
  heldBack: null,
};

/**
 * The annunciation while stills are suspended for the session (Design note 17): each later still
 * made at a reduced GPU load, `TAKE STILL` still offered, `LIMITED` as the guide's own drawing held
 * below normal is (the UX ruling, 2026-11-18).
 */
export const STILLS_LIMITED = "STILLS LIMITED";

/** The annunciation once stills are disabled for the session (Design note 17). */
export const STILLS_DISABLED = "STILLS DISABLED";

/** The control's command, a still of the view asked for. */
export const TAKE_STILL = "TAKE STILL";

/** Why `TAKE STILL` is held back while a still runs: one a client at a time (Design note 17). */
const IN_PROGRESS = "NOT AVAILABLE: a still is in progress";

/** Why `TAKE STILL` is held back once stills are disabled: a third loss would disable WebGPU. */
const DISABLED_REASON = "NOT AVAILABLE: stills disabled by a second device loss, relaunch to retry";

/** Why stills are disabled, and what to do, after `STILLS DISABLED:` away from the panel. */
const DISABLED_CAUSE = "a second device loss, relaunch to retry";

/** Each step's words on the control's status line. */
const STEP_WORDS: Readonly<Record<StillStep["kind"], string>> = {
  preparing: "PREPARING",
  streaming: STILL_TERRAIN_STREAMING,
  rendering: "RENDERING",
  saving: "SAVING",
};

/**
 * How the last still ended, with what the operator can do: take another, or, once stills are
 * disabled, relaunch.
 */
function endLine(last: StillEnd, standing: StillStanding): string {
  const retry = standing === "disabled" ? "relaunch to retry" : `${TAKE_STILL} to retry`;
  let line: string;
  switch (last.kind) {
    case "saved":
      line = "STILL SAVED";
      break;
    case "not-saved":
      line = `STILL NOT SAVED: ${last.reason}`;
      break;
    case "abandoned":
      // One name for every still not saved, its cause after it (the UX ruling, 2026-11-18).
      line = `STILL NOT SAVED: the graphics device was lost, ${retry}`;
      break;
  }
  return line;
}

/** The status line: the still's step while one runs, else how the last ended, or `null`. */
export function stillStatusLine(state: StillControlState): string | null {
  const { activity } = state;
  let line: string | null;
  switch (activity.kind) {
    case "running":
      line = STEP_WORDS[activity.step.kind];
      break;
    case "idle":
      line = activity.last === null ? null : endLine(activity.last, state.standing);
      break;
  }
  return line;
}

/**
 * Where a standing line stands: in the panel, beside `TAKE STILL`, whose held-back reason says
 * why; or `away`, under the fold row or MAXIMIZE VIEW's line, where the panel is hidden.
 */
export type StandingPlace = "panel" | "away";

/**
 * The session's standing as its line states it, `STILLS LIMITED` or `STILLS DISABLED`, or
 * `null`; away from the panel, `STILLS DISABLED` says its cause and remedy, as the guide's
 * `GRAPHICS DISABLED` does, since `TAKE STILL`'s reason is not on show there.
 */
export function stillsStandingLine(standing: StillStanding, place: StandingPlace): string | null {
  let line: string | null;
  switch (standing) {
    case "available":
      line = null;
      break;
    case "suspended":
      line = STILLS_LIMITED;
      break;
    case "disabled":
      line = place === "panel" ? STILLS_DISABLED : `${STILLS_DISABLED}: ${DISABLED_CAUSE}`;
      break;
  }
  return line;
}

/** Why `TAKE STILL` is held back, or `null` where it is offered. */
export function takeStillHeldBack(state: StillControlState): string | null {
  if (state.standing === "disabled") {
    return DISABLED_REASON;
  }
  return state.activity.kind === "running" ? IN_PROGRESS : state.heldBack;
}

interface StillProgressProps {
  readonly step: StillStep;
}

/**
 * The still's progress: a `progress` element labelled `PROGRESS`, of the job's whole passes, and
 * their count, `PASSES 3 OF 12`, its number of done right-aligned in a field as wide as the total's,
 * so that nothing moves as it counts.
 */
function StillProgress({ step }: StillProgressProps) {
  const progressId = useId();
  const total = formatNumber(step.totalPasses, 0);
  return (
    <div className="still-control__progress">
      <label className="still-control__progress-label" htmlFor={progressId}>
        PROGRESS
      </label>
      <progress
        id={progressId}
        className="still-control__bar"
        value={step.donePasses}
        max={Math.max(step.totalPasses, 1)}
      />
      <span className="still-control__count">
        PASSES{" "}
        <span className="still-control__done" style={{ minWidth: `${String(total.length)}ch` }}>
          {formatNumber(step.donePasses, 0)}
        </span>{" "}
        OF {total}
      </span>
    </div>
  );
}

/**
 * The view's still control (plan R11, R11.T10.c, Design note 17): `TAKE STILL`, the still's step
 * and its progress, and the session's standing, `STILLS LIMITED` or `STILLS DISABLED`.
 *
 * @remarks
 * Keyboard-operable as a native button: held back rather than disabled, so that it keeps focus and
 * says why, as the exposure's commands do. Suspended, it is still offered, the explicit request the
 * suspension allows. The progress stands only while a still runs, never indeterminate, since the
 * guide has no spinners. The status line is a live region.
 */
export function StillControl({ state, onTake }: StillControlProps) {
  const reasonId = useId();
  const held = takeStillHeldBack(state);
  const line = stillStatusLine(state);
  const standing = stillsStandingLine(state.standing, "panel");
  return (
    <div className="still-control">
      {standing === null ? null : (
        <p className="still-control__standing">
          <output>
            <StatusText text={standing} />
          </output>
        </p>
      )}
      <div className="still-control__command">
        <button
          type="button"
          className="control"
          aria-disabled={held === null ? undefined : "true"}
          aria-describedby={held === null ? undefined : reasonId}
          onClick={() => {
            if (held === null) {
              onTake();
            }
          }}
        >
          {TAKE_STILL}
        </button>
        {held === null ? null : (
          <span className="still-control__reason" id={reasonId}>
            <StatusText text={held} />
          </span>
        )}
      </div>
      {/* A live region throughout, so that each step and the end are announced as they come. */}
      <p className="still-control__status">
        <output>{line === null ? null : <StatusText text={line} />}</output>
      </p>
      {state.activity.kind === "running" ? <StillProgress step={state.activity.step} /> : null}
    </div>
  );
}
