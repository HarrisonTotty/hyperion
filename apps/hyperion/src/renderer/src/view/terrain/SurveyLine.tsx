import type { ReactNode } from "react";

import type { ReadoutLine } from "./readouts";
import { TerrainReading } from "./TerrainReading";
import { TerrainStatus } from "./TerrainStatus";

/** Props of {@link SurveyLine}. */
export interface SurveyLineProps {
  /**
   * The line: the survey line's values (`surveyLines`), a state in its place or after it, or the
   * `DATUM` line (`datumLine`).
   */
  readonly line: ReadoutLine;
}

/**
 * One of the survey's lines on a view's lower plate (plan R10, R10.T12; the guide's survey line and
 * `DATUM`): a reading in the label block's line markup, `SURVEY COVERAGE 12.4 % · FINEST 2.0 m` or
 * `DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km`, or a state, `SURVEY COVERAGE: PENDING`, steady
 * in its colour.
 *
 * @remarks
 * Carries no `id`, since a copy of it stands in the view's hidden sizer.
 */
export function SurveyLine({ line }: SurveyLineProps) {
  let node: ReactNode;
  switch (line.kind) {
    case "reading":
      node = (
        <dl className="view-label__lines">
          <TerrainReading line={line} />
        </dl>
      );
      break;
    case "status":
      node = <TerrainStatus text={line.text} failure={line.failure} />;
      break;
    case "heading":
      node = <h3 className="view-label__heading">{line.text}</h3>;
      break;
  }
  return node;
}
