import { Fragment, useId, useState } from "react";

import { DisclosureGlyph } from "../../components/DisclosureGlyph";
import { StatusText } from "../../components/StatusText";
import { LabelStatementLine } from "./LabelStatementLine";
import { type LabelStatement, type SentenceStatement, statementText } from "./viewRun";

/** Props of {@link LabelStatements}. */
export interface LabelStatementsProps {
  readonly statements: ReadonlyArray<LabelStatement>;
}

/** A note with the sentences that follow it, `ATMOSPHERE: APPROXIMATE` and its reasons. */
interface StatementGroup {
  readonly statement: LabelStatement;
  readonly reasons: ReadonlyArray<SentenceStatement>;
}

/** Whether a statement is a sentence, one of a note's reasons. */
function isReason(statement: LabelStatement): statement is SentenceStatement {
  return typeof statement !== "string" && statement.standing === "sentence";
}

/** The statements in order, each sentence with the note before it. */
function groupsOf(statements: ReadonlyArray<LabelStatement>): ReadonlyArray<StatementGroup> {
  const groups: StatementGroup[] = [];
  for (const statement of statements) {
    const last = groups.at(-1);
    if (isReason(statement) && last !== undefined && typeof last.statement === "string") {
      groups[groups.length - 1] = { ...last, reasons: [...last.reasons, statement] };
    } else {
      groups.push({ statement, reasons: [] });
    }
  }
  return groups;
}

/** The count of a note's reasons as its control reads it: `1 REASON`, `4 REASONS`. */
export function reasonsLabel(count: number): string {
  return count === 1 ? "1 REASON" : `${String(count)} REASONS`;
}

/** Props of {@link FoldedNote}. */
interface FoldedNoteProps {
  readonly note: string;
  readonly reasons: ReadonlyArray<SentenceStatement>;
}

/**
 * A note with its reasons folded behind it: the note as it always stands, then on its line a
 * display control naming the reasons' count, which shows them under the note while a press has
 * opened them, or while the pointer rests on it until a press folds them.
 */
function FoldedNote({ note, reasons }: FoldedNoteProps) {
  const reasonsId = useId();
  const [opened, setOpened] = useState(false);
  const [pointed, setPointed] = useState(false);
  const shown = opened || pointed;
  return (
    <>
      <p className="view-label__statement view-label__statement--folds">
        <span className="view-label__note">
          <StatusText text={note} />
        </span>{" "}
        <button
          type="button"
          className="control disclosure view-label__reasons"
          aria-expanded={shown}
          aria-controls={reasonsId}
          onClick={() => {
            // A press decides, the pointer resting on the control or not, until it next comes.
            setOpened((was) => !was);
            setPointed(false);
          }}
          // A shortcut only: a press, by pointer, touch or key, opens them as well.
          onPointerEnter={() => {
            setPointed(true);
          }}
          onPointerLeave={() => {
            setPointed(false);
          }}
        >
          <DisclosureGlyph expanded={shown} />
          {reasonsLabel(reasons.length)}
        </button>
      </p>
      {/* Its lines are made only while shown, so that a scroll counts no folded line. */}
      <div className="view-label__reasons-list" id={reasonsId} hidden={!shown}>
        {shown
          ? reasons.map((reason) => <LabelStatementLine statement={reason} key={reason.text} />)
          : null}
      </div>
    </>
  );
}

/**
 * A view's steady statements under its label block, each on a line of its own
 * ({@link LabelStatementLine}), with a note's reasons, `ATMOSPHERE: APPROXIMATE`'s, folded behind
 * it as the view opens, so that they do not cover the picture (the owner's request, 2026-11-01).
 *
 * @remarks
 * The note stands as it always does, steady, in `--text`; its reasons' control after it on its
 * line, `4 REASONS` led by the disclosure chevron, is a display control, which opens them, each on
 * a line of its own under the note, when pressed and closes them when pressed again, and shows
 * them while the pointer rests on it, so that nothing depends on the pointer alone. Each note's
 * fold is its own and is kept while its block stays mounted; a new scene's block starts folded.
 */
export function LabelStatements({ statements }: LabelStatementsProps) {
  return groupsOf(statements).map(({ statement, reasons }) => (
    <Fragment key={statementText(statement)}>
      {reasons.length === 0 || typeof statement !== "string" ? (
        <LabelStatementLine statement={statement} />
      ) : (
        <FoldedNote note={statement} reasons={reasons} />
      )}
    </Fragment>
  ));
}
