/**
 * An instrument's `GROUND` page in the camera panel's place (plan R10, R10.T12.d; guide draft D2):
 * which page stands, its parts and how far they give way, and what the instruments' loop reads of
 * it.
 *
 * @remarks
 * `GROUND` is offered while the instrument states a body's terrain (`groundBodyOf` of its run),
 * else held back with its reason, the place showing `CAMERA` until it can. The page shows while it
 * stands and the compact layout does not fold the place. Its parts are the instrument's terrain
 * lines (`useTerrainLines` of its own readout, published only while the page shows), which give
 * way and return as the lower plate's do (`lowerStepIn`), from the page as last laid out
 * (`measurePage`) and its step before, the hysteresis's. The tripod's point and the list's note
 * stand only while the page states the readout; the display tells the loop which slot to ask the
 * readout for, and its point, after each commit (`InstrumentGround`).
 */
import { type RefObject, useLayoutEffect, useRef, useState, type ReactNode } from "react";

import type { InstrumentSlot } from "./viewNames";
import { groundBodyOf, lineOfSightPointOf } from "./groundReadout";
import { groundHeldBack, type PlacePage } from "./PlaceTabs";
import {
  measurePage,
  PAGE_NOT_MEASURED,
  type PageMeasures,
  samePageMeasures,
} from "./plateMeasures";
import { terrainLowerParts } from "./terrainPlate";
import type { InstrumentShown } from "./useInstruments";
import { standingReadout, useTerrainLines } from "./useTerrainLines";
import { LINE_SPARE_REM, type LowerPlatePart, lowerStepIn, type PlateStep } from "./viewRoom";

/** The instrument `CONTROLS` names, as the page reads it. */
export interface GroundPageInstrument {
  readonly slot: InstrumentSlot;
  /** Its name, the place's designator (`INSTRUMENT 1`). */
  readonly name: string;
  /** What it last drew, its `GROUND` snapshot among it. */
  readonly shown: InstrumentShown;
  /** The faults for its plates, the terrain's among them. */
  readonly faults: ReadonlyArray<string>;
}

/** What {@link useGroundPage} reads. */
export interface GroundPageInput {
  /** The instrument `CONTROLS` names, or `null` while it names `PRIMARY`. */
  readonly instrument: GroundPageInstrument | null;
  /** The page the operator chose, the display's own. */
  readonly chosen: PlacePage;
  /** Whether the compact layout folds the camera panel's place. */
  readonly folded: boolean;
}

/** The page's state, for `InstrumentControls`. */
export interface GroundPageState {
  /** The page standing: `GROUND` only where it is chosen and offered. */
  readonly page: PlacePage;
  /** Why `GROUND` is held back, or `null` where it is offered. */
  readonly groundReason: string | null;
  readonly parts: ReadonlyArray<LowerPlatePart<ReactNode>>;
  readonly readout: "stands" | "yields";
  readonly survey: "stands" | "yields";
  /** Whether the instrument's list carries the readout's note: while the page states its point. */
  readonly groundNote: boolean;
  /** The slot whose page shows, which the loop asks a readout for, or `null`. */
  readonly pagedSlot: InstrumentSlot | null;
  /**
   * Whether its readout's point is over ground, where the page states the readout; absent while
   * it states none (`LineOfSightPoint.hasPoint`).
   */
  readonly pointHas: boolean | undefined;
  /** The point's slant range, m, where known (`LineOfSightPoint.rangeM`). */
  readonly pointRangeM: number | null;
  readonly plateRef: RefObject<HTMLDivElement | null>;
  readonly sizerRef: RefObject<HTMLDivElement | null>;
  readonly targetsRef: RefObject<HTMLElement | null>;
}

/** No faults, for a page with no instrument. */
const NO_FAULTS: ReadonlyArray<string> = [];

/** The `GROUND` page of the instrument `CONTROLS` names (see the module's remarks). */
export function useGroundPage(input: GroundPageInput): GroundPageState {
  const { instrument, chosen, folded } = input;
  const groundReason =
    instrument === null || groundBodyOf(instrument.shown.run) !== null
      ? null
      : groundHeldBack(instrument.name);
  const page: PlacePage =
    instrument !== null && chosen === "ground" && groundReason === null ? "ground" : "camera";
  const shows = page === "ground" && !folded;
  const lines = useTerrainLines(instrument?.shown.ground ?? null, instrument?.faults ?? NO_FAULTS);
  const parts = terrainLowerParts(lines);
  const [measures, setMeasures] = useState<PageMeasures>(PAGE_NOT_MEASURED);
  const [step, setStep] = useState<PlateStep>(0);
  const decided =
    !shows || measures.roomPx === null
      ? 0
      : lowerStepIn(measures.lower, measures.roomPx, LINE_SPARE_REM * measures.remPx, step);
  if (decided !== step) {
    setStep(decided);
  }
  const readout =
    decided >= 1 && parts.some((part) => part.kind === "readout") ? "yields" : "stands";
  const survey =
    decided >= 2 && parts.some((part) => part.kind === "survey" || part.kind === "datum")
      ? "yields"
      : "stands";
  const answer = shows ? standingReadout(lines, readout) : null;
  const point = answer === null ? undefined : lineOfSightPointOf(answer);
  const hasPoint = point?.hasPoint;
  const rangeM = point?.rangeM ?? null;
  const plateRef = useRef<HTMLDivElement | null>(null);
  const sizerRef = useRef<HTMLDivElement | null>(null);
  const targetsRef = useRef<HTMLElement | null>(null);
  // The page as laid out after each render, for its next step: read from the DOM, an external
  // system, and kept only where it changed.
  useLayoutEffect(() => {
    const measure = (): void => {
      const measured = shows
        ? measurePage({
            plate: plateRef.current,
            sizer: sizerRef.current,
            targets: targetsRef.current,
          })
        : PAGE_NOT_MEASURED;
      setMeasures((previous) => (samePageMeasures(previous, measured) ? previous : measured));
    };
    measure();
  });
  return {
    page,
    groundReason,
    parts,
    readout,
    survey,
    groundNote: hasPoint === true,
    pagedSlot: shows ? (instrument?.slot ?? null) : null,
    pointHas: hasPoint,
    pointRangeM: rangeM,
    plateRef,
    sizerRef,
    targetsRef,
  };
}
