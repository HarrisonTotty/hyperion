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
  /**
   * Whether it announces its own changes. Set it to `false` only for a status line rendered inside
   * another live region, which then governs what is read.
   */
  readonly announce?: boolean | undefined;
}

/**
 * The one presentation of a status that stands in for a result: its words in an `output`, and
 * the control that acts on it.
 *
 * @remarks
 * Every state is in words, never a spinner. A fault is in `--status-caution` with its words, since
 * the guide keeps yellow for alerts, limits and failed systems; waiting and a refusal are plain
 * text. The control is a display control (`.control`), not a command.
 *
 * An `output` is a live region of its own, so one nested inside another is read differently by
 * every screen reader: twice, or only the inner change, or the whole outer region. A status line
 * that stands inside another region therefore takes `announce={false}`, which keeps the `output`
 * and its semantics but leaves the announcing to the nearest region around it.
 */
export function StatusLine({ text, standing, id, action, announce = true }: StatusLineProps) {
  return (
    <div className="request-status">
      <output
        id={id}
        className={
          standing === "fault"
            ? "request-status__text request-status__text--fault"
            : "request-status__text"
        }
        // Off, and so not a region of its own: the nearest announcing ancestor governs the change.
        aria-live={announce ? undefined : "off"}
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
