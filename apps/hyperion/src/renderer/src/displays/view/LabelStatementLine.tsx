import { StatusText } from "../../components/StatusText";
import { type LabelStatement, statementText } from "./viewRun";

/** Props of {@link LabelStatementLine}. */
export interface LabelStatementLineProps {
  readonly statement: LabelStatement;
}

/**
 * One steady statement under a view's label block (plan R02, R02.T15.c): set through `StatusText`,
 * in `--text`, or in `--status-caution` for a `CautionStatement` (a server's own failure,
 * `ATMOSPHERE REJECTED`'s, plan R08, R08.T10.b), whose words say what it is as well, so that its
 * state is never told by colour alone.
 */
export function LabelStatementLine({ statement }: LabelStatementLineProps) {
  const caution = typeof statement !== "string";
  return (
    <p
      className={
        caution ? "view-label__statement view-label__statement--caution" : "view-label__statement"
      }
    >
      <StatusText text={statementText(statement)} />
    </p>
  );
}
