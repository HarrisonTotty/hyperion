/**
 * How a status line reads: neutral while something waits, a refusal in plain text, or a fault,
 * a failed or overloaded system or an unknown result, in `--status-caution`.
 */
export type StatusStanding = "waiting" | "refused" | "fault";

/** A control that follows a status's text, such as `RETRY` or `DISMISS`. */
export interface StatusAction {
  /** The control's label, upper case. */
  readonly label: string;
  readonly onAction: () => void;
}

interface StatusLineProps {
  /** The annunciation, in words: `PENDING`, `TIMED OUT`, `MAP DATA INVALID: <cause>`. */
  readonly text: string;
  readonly standing: StatusStanding;
  /** An ID for the status, so that the controls it holds back can be described by it. */
  readonly id?: string | undefined;
  /** What the operator can do about it, offered after the text. */
  readonly action?: StatusAction | undefined;
}

/**
 * The one presentation of a status that stands in for a result: its words in an `output`, and
 * the control that acts on it.
 *
 * @remarks
 * Every state is in words, never a spinner. A fault is in `--status-caution` with its words, since
 * the guide keeps yellow for alerts, limits and failed systems; waiting and a refusal are plain
 * text. The control is a display control (`.control`), not a command.
 */
export function StatusLine({ text, standing, id, action }: StatusLineProps) {
  return (
    <div className="request-status">
      <output
        id={id}
        className={
          standing === "fault"
            ? "request-status__text request-status__text--fault"
            : "request-status__text"
        }
      >
        {text}
      </output>
      {action === undefined ? null : (
        <button type="button" className="control" onClick={action.onAction}>
          {action.label}
        </button>
      )}
    </div>
  );
}
