import { StatusLine } from "../../components/StatusLine";
import { StatusText } from "../../components/StatusText";

/** Props of {@link TerrainStatus}. */
export interface TerrainStatusProps {
  /** The status in words: `SURVEY COVERAGE: PENDING`, `NOT SURVEYED`. */
  readonly text: string;
  /**
   * What it reports, where it is a request's outcome (`ReadoutLine`'s `failure`): the server's
   * refusal, in `--text`, or a failure, in `--status-caution`; `null` for a statement, in `--text`.
   */
  readonly failure: "refused" | "fault" | null;
}

/**
 * A status line of a view's terrain lines (plan R10, R10.T12): the survey line's state in its place
 * or after it, or `NOT SURVEYED` or `NO GROUND` in place of the readout's readings, steady.
 *
 * @remarks
 * A statement (`SURVEY COVERAGE: PENDING`, `TERRAIN: NOT YET MODELLED`, `NOT SURVEYED`,
 * `NO GROUND`) is set as the label block's statements are, in `--text`, its words through
 * `StatusText`. The survey line's failures, `SURVEY COVERAGE REJECTED` and `SURVEY DATA INVALID`,
 * are set as every request's status is, through `StatusLine`, a status in its own `output`: a
 * refusal in `--text`, a failure in `--status-caution`, as the block's `TERRAIN NOT AVAILABLE` is
 * (decision-r10-t16-leans §12).
 */
export function TerrainStatus({ text, failure }: TerrainStatusProps) {
  return failure === null ? (
    <p className="view-label__statement">
      <StatusText text={text} />
    </p>
  ) : (
    <StatusLine text={text} standing={failure} />
  );
}
