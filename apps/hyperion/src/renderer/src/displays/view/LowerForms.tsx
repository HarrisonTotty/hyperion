import type { ReactNode } from "react";

import { ViewLowerPlate } from "./ViewLowerPlate";
import type { LowerPlatePart } from "./viewRoom";

/** Props of {@link LowerForms}. */
export interface LowerFormsProps {
  readonly parts: ReadonlyArray<LowerPlatePart<ReactNode>>;
  /**
   * The plates' class: beside the slots, in columns, or the `GROUND` page's (R10.T12.d); none at
   * the full width.
   */
  readonly className: string | undefined;
}

/**
 * The lower plate's forms at its tallest: whole, its readout given way, and its survey too, each in
 * an element whose `data-form` names it, as `measureLowerForms` reads them (plan R10, R10.T12.b;
 * the `GROUND` page's too, R10.T12.d).
 */
export function LowerForms({ parts, className }: LowerFormsProps) {
  const hasReadout = parts.some((part) => part.kind === "readout");
  const hasSurvey = parts.some((part) => part.kind === "survey" || part.kind === "datum");
  return (
    <>
      <div data-form="whole">
        <ViewLowerPlate
          parts={parts}
          readout="stands"
          survey="stands"
          tallest
          className={className}
        />
      </div>
      {hasReadout ? (
        <div data-form="readout">
          <ViewLowerPlate
            parts={parts}
            readout="yields"
            survey="stands"
            tallest
            className={className}
          />
        </div>
      ) : null}
      {hasSurvey ? (
        <div data-form="survey">
          <ViewLowerPlate
            parts={parts}
            readout="yields"
            survey="yields"
            tallest
            className={className}
          />
        </div>
      ) : null}
    </>
  );
}
