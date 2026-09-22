import type { ErrorCode, RequestKind } from "@hyperion/protocol";

import type { RequestState } from "../lib/useServerRequest";
import { StatusLine, type StatusStanding } from "./StatusLine";

interface RequestStatusProps {
  readonly state: RequestState<RequestKind>;
  /** An ID for the status, so that the controls it holds back can be described by it. */
  readonly id?: string;
  /** Sends the request again; offered as `RETRY` after a rejection or a timeout. */
  readonly onRetry?: () => void;
}

/**
 * A request that has not answered, in words, and how it stands: waiting while it waits or while
 * the link is down, refused when the server turned it down for a reason in the request itself, and
 * a fault when the server failed, was overloaded or did not answer.
 */
interface Annunciation {
  readonly text: string;
  readonly standing: StatusStanding;
}

/**
 * Codes by which the server refuses a request for what it asks, not because anything failed: the
 * operator's name is taken, the universe is not there or is of another generator version, the
 * request was malformed. Every other code, and any the protocol adds, reports a failed or
 * overloaded system.
 */
const REFUSALS: ReadonlySet<string> = new Set<ErrorCode>([
  "bad_request",
  "name_taken",
  "unknown_universe",
  "generator_version_mismatch",
]);

/** The words for a request state that is not `ok` or `idle`, and how it stands. */
function annunciation(state: RequestState<RequestKind>): Annunciation | null {
  let shown: Annunciation | null;
  switch (state.kind) {
    case "idle":
    case "ok":
      shown = null;
      break;
    case "pending":
      shown = { text: "PENDING", standing: "waiting" };
      break;
    case "rejected":
      shown = {
        text: `REJECTED: ${state.reason}`,
        standing: REFUSALS.has(state.code) ? "refused" : "fault",
      };
      break;
    case "timed_out":
      shown = { text: "TIMED OUT", standing: "fault" };
      break;
    case "link_down":
      // The link's state, which the header annunciates; not a failure of this request.
      shown = { text: state.reason, standing: "waiting" };
      break;
  }
  return shown;
}

/**
 * The shared presentation of a request that has not answered: `PENDING`, `REJECTED: <reason>`,
 * `TIMED OUT`, or the link's state such as `NO CARRIER`.
 *
 * @remarks
 * Closed-loop feedback as "Controls and commanding" in `docs/frontend/ux-guidelines.md` requires,
 * never a spinner or "Loading". Every state is in words. `PENDING`, the link's state and a
 * refusal (a name taken, an unknown universe) are plain text, since the guide keeps yellow for
 * alerts, limits and failed systems. A timeout, and a rejection because the server failed or is
 * overloaded, report a failed system and are in `--status-caution`. After either kind of failure,
 * when the caller can send the request again, `RETRY` follows. Nothing is rendered for `ok` and
 * `idle`.
 */
export function RequestStatus({ state, id, onRetry }: RequestStatusProps) {
  const shown = annunciation(state);
  if (shown === null) {
    return null;
  }
  return (
    <StatusLine
      text={shown.text}
      standing={shown.standing}
      id={id}
      action={
        shown.standing !== "waiting" && onRetry !== undefined
          ? { label: "RETRY", onAction: onRetry }
          : undefined
      }
    />
  );
}
