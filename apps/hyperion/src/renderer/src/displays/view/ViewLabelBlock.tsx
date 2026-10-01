import { StatusLine } from "../../components/StatusLine";
import type { LabelLine } from "./viewRun";

/** Props of {@link ViewLabelBlock}. */
export interface ViewLabelBlockProps {
  /** The readings, refreshed at 4 Hz (`useThrottledValue` upstream). */
  readonly lines: ReadonlyArray<LabelLine>;
  /** Steady statements while their conditions hold: `POSITIONS AS SEEN FROM SHIP` and the rest. */
  readonly statements: ReadonlyArray<string>;
  /** A graphics fault standing while the view draws (`GRAPHICS DEVICE LOST: re-creating`), or `null`. */
  readonly fault: string | null;
}

/**
 * The view's label block (plan R02, R02.T15.c; Design note 16): the display class `VIEW` and what
 * the picture is, its frame, time, style, camera, field of view, exposure and star source, as DOM
 * text on a `--surface-0` plate over the canvas, never drawn into it.
 *
 * @remarks
 * Each reading is an `output`; none is announced as it changes, since they change continuously. A
 * graphics fault is set as `StatusLine`'s fault, in `--status-caution`, apart from the steady
 * statements (the guide's "Alerts": a console's report on its own graphics is never an alert).
 */
export function ViewLabelBlock({ lines, statements, fault }: ViewLabelBlockProps) {
  return (
    <div className="view-label">
      <p className="view-label__class">VIEW</p>
      <dl className="view-label__lines">
        {lines.map((line) => (
          <div className="field view-label__line" key={line.label}>
            <dt className="field__label">{line.label}</dt>
            <dd>
              <output aria-live="off">{line.value}</output>
            </dd>
          </div>
        ))}
      </dl>
      {statements.map((statement) => (
        <p className="view-label__statement" key={statement}>
          {statement}
        </p>
      ))}
      {fault === null ? null : <StatusLine text={fault} standing="fault" />}
    </div>
  );
}
