import { Fragment, useId } from "react";

import { DisclosureGlyph } from "../../components/DisclosureGlyph";
import { StatusText } from "../../components/StatusText";
import { LabelStatementLine } from "./LabelStatementLine";
import type { ReasonFolds } from "./reasonFolds";
import {
  type LabelStatement,
  type SentenceStatement,
  statementGroups,
  statementText,
} from "./viewRun";

/** Props of {@link LabelStatements}. */
export interface LabelStatementsProps {
  readonly statements: ReadonlyArray<LabelStatement>;
  /** Which notes' reasons stand opened, held above the plate (`reasonFolds.ts`). */
  readonly folds: ReasonFolds;
}

/** Props of {@link FoldedNote}. */
interface FoldedNoteProps {
  readonly note: string;
  readonly reasons: ReadonlyArray<SentenceStatement>;
  readonly folds: ReasonFolds;
}

/**
 * A note with its reasons folded behind it: the note as it always stands, then on its line a
 * display control naming the reasons' count, which opens them under the note on a press and folds
 * them on another.
 */
function FoldedNote({ note, reasons, folds }: FoldedNoteProps) {
  const reasonsId = useId();
  const opened = folds.opened.has(note);
  const count = reasons.length;
  return (
    <>
      <p className="view-label__statement view-label__statement--folds">
        <span className="view-label__note">
          <StatusText text={note} />
        </span>{" "}
        <button
          type="button"
          className="control disclosure view-label__reasons"
          aria-expanded={opened}
          aria-controls={reasonsId}
          onClick={() => {
            folds.toggle(note);
          }}
        >
          <DisclosureGlyph expanded={opened} />
          {/* A count, set in B612 Mono with tabular figures as every number is. */}
          <span className="field__value">{String(count)}</span>
          {count === 1 ? " REASON" : " REASONS"}
        </button>
      </p>
      {/* Its lines are made only while opened, so that a scroll counts no folded line. */}
      <div className="view-label__reasons-list" id={reasonsId} hidden={!opened}>
        {opened
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
 * The note stands as it always does, steady, in `--text`; its reasons' control after it, on its
 * line where it fits, `4 REASONS` led by the disclosure chevron, is a display control, which opens
 * them, each on a line of its own under the note, when pressed by pointer, touch or key, and folds
 * them when pressed again. Each note's fold is its own, held above the plate in `folds`, so that the
 * room's hidden copy of the plate stands as it does: the stage's for the `PRIMARY` view, kept
 * while the scene is, and an instrument's slot's, kept while it is held.
 */
export function LabelStatements({ statements, folds }: LabelStatementsProps) {
  return statementGroups(statements).map(({ statement, reasons }) => (
    <Fragment key={statementText(statement)}>
      {reasons.length === 0 || typeof statement !== "string" ? (
        <LabelStatementLine statement={statement} />
      ) : (
        <FoldedNote note={statement} reasons={reasons} folds={folds} />
      )}
    </Fragment>
  ));
}
