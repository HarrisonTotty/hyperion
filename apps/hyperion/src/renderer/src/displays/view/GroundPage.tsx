import type { ReactNode, Ref } from "react";

import { LowerForms } from "./LowerForms";
import { ViewLowerPlate } from "./ViewLowerPlate";
import type { LowerPlatePart } from "./viewRoom";

/** The class of the page's plate and its forms' copies, a panel's content, not an overlay. */
export const GROUND_PLATE_CLASS = "view-ground__plate";

/** Props of {@link GroundPage}. */
export interface GroundPageProps {
  /** The parts of the instrument's terrain lines (`terrainLowerParts`), in the lower plate's order. */
  readonly parts: ReadonlyArray<LowerPlatePart<ReactNode>>;
  /** Whether the readout stands, or has given way for the place's room. */
  readonly readout: "stands" | "yields";
  /** Whether the survey line's values and `DATUM` stand. */
  readonly survey: "stands" | "yields";
  /** Receives the live plate, whose height `measurePage` reads. */
  readonly plateRef: Ref<HTMLDivElement>;
  /** Receives the forms' copies' root, which `measurePage` reads. */
  readonly sizerRef: Ref<HTMLDivElement>;
}

/**
 * An instrument's `GROUND` page (plan R10, R10.T12.d; guide draft D2), the body of the camera
 * panel's place while `CONTROLS` names the instrument and `GROUND` is chosen (`PlacePanel`): the
 * survey line of the instrument's own `FRAME` body, its `DATUM` and its `GROUND AT LINE OF SIGHT`,
 * in the lower plate's order and forms, which the instrument's slot has no room for.
 *
 * @remarks
 * Its parts give way and return as the lower plate's do (`lowerStepIn`), reckoned at their tallest
 * from the hidden copies of their forms, laid out at the plate's width, against the place's room
 * with the list at its floor (`measurePage`). Its copies carry no `id`, are never seen and never
 * read by assistive technology.
 */
export function GroundPage({ parts, readout, survey, plateRef, sizerRef }: GroundPageProps) {
  return (
    <>
      <ViewLowerPlate
        parts={parts}
        readout={readout}
        survey={survey}
        className={GROUND_PLATE_CLASS}
        ref={plateRef}
      />
      <div className="view-ground__sizer" aria-hidden="true" ref={sizerRef}>
        <LowerForms parts={parts} className={GROUND_PLATE_CLASS} />
      </div>
    </>
  );
}
