import { formatSignificant } from "../../lib/format";
import type { DrawAnchor } from "../../view/wireframe/drawList";
import { type MarkRow, targetKey } from "./viewRun";

/** Props of {@link ViewMarkLabels}. */
export interface ViewMarkLabelsProps {
  /** The marks as the latest published frame placed them, in device px. */
  readonly anchors: ReadonlyArray<DrawAnchor>;
  /** Device pixels in one CSS pixel, by which the anchors are placed in the DOM. */
  readonly devicePixelRatio: number;
  /** The list's rows, whose names and ranges (with their unit's hysteresis) the labels repeat. */
  readonly rows: ReadonlyArray<MarkRow>;
}

/** The space between a mark and its label, rem. */
const LABEL_OFFSET_REM = 0.75;

/** A closure rate with its sign, since direction matters (the guide's "Numbers"): `+3.40 m/s`. */
function closureText(closureMPerS: number): string {
  const sign = closureMPerS < 0 ? "-" : "+";
  return `${sign}${formatSignificant(Math.abs(closureMPerS))} m/s`;
}

/**
 * The marks' DOM labels over the canvas (plan R02, R02.T15.b; T12 and T13 as built): another
 * craft's target mark with its range and closure rate, and a body drawn as its symbol with its
 * designation (Design note 13), each on a `--surface-0` plate, refreshed with the readouts at 4 Hz.
 *
 * @remarks
 * Hidden from assistive technology: the list beside the view carries the same names and ranges.
 */
export function ViewMarkLabels({ anchors, devicePixelRatio, rows }: ViewMarkLabelsProps) {
  return (
    <div className="view-marks" aria-hidden="true">
      {anchors.map((anchor) => {
        const { label } = anchor;
        const key = targetKey(anchor.target);
        const row = rows.find((each) => each.key === key);
        if (label === null || row === undefined) {
          return null;
        }
        const parts = label.kind === "target" ? [row.name, row.range] : [row.name];
        if (label.kind === "target" && label.closureMPerS !== null) {
          parts.push(closureText(label.closureMPerS));
        }
        return (
          <span
            key={key}
            className="view-marks__label"
            style={{
              transform: `translate(calc(${String(anchor.xPx / devicePixelRatio)}px + ${String(LABEL_OFFSET_REM)}rem), ${String(anchor.yPx / devicePixelRatio)}px)`,
            }}
          >
            {parts.join(" ")}
          </span>
        );
      })}
    </div>
  );
}
