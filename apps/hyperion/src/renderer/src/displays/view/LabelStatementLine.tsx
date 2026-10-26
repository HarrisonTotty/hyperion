import { StatusLine } from "../../components/StatusLine";
import { StatusText } from "../../components/StatusText";
import type { LabelStatement } from "./viewRun";

/** Props of {@link LabelStatementLine}. */
export interface LabelStatementLineProps {
  readonly statement: LabelStatement;
}

/**
 * One steady statement under a view's label block (plan R02, R02.T15.c): a note set through
 * `StatusText`, in `--text`; or a request's status (`StatusStatement`, `ATMOSPHERE REJECTED`'s,
 * plan R08, R08.T10.b), set as every request's `REJECTED` is, through `StatusLine`: `refused` in
 * `--text`, `fault` in `--status-caution`, its words saying which, so that its state is never told
 * by colour alone.
 */
export function LabelStatementLine({ statement }: LabelStatementLineProps) {
  if (typeof statement === "string") {
    return (
      <p className="view-label__statement">
        <StatusText text={statement} />
      </p>
    );
  }
  return <StatusLine text={statement.text} standing={statement.standing} />;
}
