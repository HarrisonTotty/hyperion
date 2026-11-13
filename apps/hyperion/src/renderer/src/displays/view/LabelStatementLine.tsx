import type { ReactElement } from "react";

import { StatusLine } from "../../components/StatusLine";
import { StatusText } from "../../components/StatusText";
import type { LabelStatement } from "./viewRun";

/** Props of {@link LabelStatementLine}. */
export interface LabelStatementLineProps {
  readonly statement: LabelStatement;
}

/**
 * One steady statement under a view's label block (plan R02, R02.T15.c).
 *
 * @remarks
 * A note is set through `StatusText`, in `--text`. A sentence (`SentenceStatement`,
 * `ATMOSPHERE: APPROXIMATE`'s reasons, plan R08, R08.T10.d) is set whole in `--text` in its own
 * case with no letter spacing, its head included, so that no part of it is tracked as an
 * annunciation, colon or not. A request's status (`StatusStatement`, `ATMOSPHERE REJECTED`'s,
 * R08.T10.b) is set as every request's `REJECTED` is, through `StatusLine`: `refused` in `--text`,
 * `fault` in `--status-caution`, its words saying which, so that its state is never told by colour
 * alone.
 */
export function LabelStatementLine({ statement }: LabelStatementLineProps) {
  if (typeof statement === "string") {
    return (
      <p className="view-label__statement">
        <StatusText text={statement} />
      </p>
    );
  }
  let line: ReactElement;
  switch (statement.standing) {
    case "sentence":
      line = (
        <p className="view-label__statement view-label__statement--sentence">{statement.text}</p>
      );
      break;
    case "refused":
    case "fault":
      line = <StatusLine text={statement.text} standing={statement.standing} />;
      break;
  }
  return line;
}
