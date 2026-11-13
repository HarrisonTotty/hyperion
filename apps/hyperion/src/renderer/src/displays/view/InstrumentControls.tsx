import { type ReactNode, type Ref, useId, useState } from "react";

import { StaleMark } from "../../components/StaleMark";
import type { BodyDistanceUnit } from "../../lib/format";
import type { PrimaryModifier } from "../../lib/platform";
import { maxFreeRateStep } from "../../view/camera/freeCamera";
import type { ViewKeyAction } from "../../view/camera/keys";
import { type CameraTarget, offeredPresets } from "../../view/camera/state";
import { cameraSceneOf } from "../../view/scene/model";
import type { DrawnSurveyEdge } from "../../view/wireframe/drawList";
import { CameraControls } from "./CameraControls";
import { useCameraPlace } from "./cameraPlace";
import { GroundPage } from "./GroundPage";
import { PlacePanel } from "./PlacePanel";
import type { PlacePage } from "./PlaceTabs";
import type { InstrumentShown } from "./useInstruments";
import type { SideFolds } from "./viewLayout";
import { ViewMarkList } from "./ViewMarkList";
import type { LowerPlatePart } from "./viewRoom";
import {
  type MarkRow,
  markRows,
  rangesFromCamera,
  type SurveyEdgeListNote,
  surveyEdgeNotes,
  targetKey,
  type ViewRun,
} from "./viewRun";

/** Props of {@link InstrumentControls}. */
export interface InstrumentControlsProps {
  /** The instrument's name, each panel's designator (`INSTRUMENT 1`). */
  readonly designator: string;
  /** What the instrument last drew. */
  readonly shown: InstrumentShown;
  readonly selection: CameraTarget | null;
  /** Whether the server's scene is stale: the ranges are muted with their `S`. */
  readonly stale: boolean;
  readonly easedMoves: boolean;
  readonly reducedMotion: boolean;
  readonly onAction: (action: ViewKeyAction) => void;
  readonly onEasedMovesChange: (easedMoves: boolean) => void;
  readonly onSelect: (target: CameraTarget) => void;
  /** How the camera panel stands in the side column's layout. */
  readonly folds: SideFolds;
  /** The platform's primary modifier, whose chord the rate's limit reason names (R07.T19.f). */
  readonly modifier: PrimaryModifier;
  /** The camera panel's place: its tabs and, while `GROUND` stands, the page (R10.T12.d). */
  readonly place: InstrumentPlace;
  /** Receives its `Targets` panel, whose list gives way down to its floor for the page. */
  readonly targetsRef: Ref<HTMLElement>;
  /**
   * Whether its list carries the terrain readout's note (`GROUND AT LINE OF SIGHT`): while its page
   * states the readout with its point (R10.T12.d).
   */
  readonly groundNote: boolean;
}

/**
 * The camera panel's place while `CONTROLS` names an instrument (plan R10, R10.T12.d; guide draft
 * D2): the page standing, why `GROUND` is held back, and the `GROUND` page's parts, their standing
 * and the elements the display measures it by.
 */
export interface InstrumentPlace {
  /** The page standing: `GROUND` only where it is chosen and offered. */
  readonly page: PlacePage;
  /** Why `GROUND` is held back, or `null` where it is offered. */
  readonly groundReason: string | null;
  readonly onPage: (page: PlacePage) => void;
  /** The parts of its terrain lines (`terrainLowerParts`). */
  readonly parts: ReadonlyArray<LowerPlatePart<ReactNode>>;
  readonly readout: "stands" | "yields";
  readonly survey: "stands" | "yields";
  readonly plateRef: Ref<HTMLDivElement>;
  readonly sizerRef: Ref<HTMLDivElement>;
}

/**
 * The rows and notes of the list, with the units they were last shown in kept for their
 * hysteresis.
 */
interface ShownRows {
  readonly run: ViewRun;
  readonly rows: ReadonlyArray<MarkRow>;
  /** The survey edges its notes were made from, and the notes (R10.T16.c). */
  readonly edges: ReadonlyArray<DrawnSurveyEdge>;
  readonly notes: ReadonlyArray<SurveyEdgeListNote>;
}

function unitsOf(rows: ReadonlyArray<MarkRow>): ReadonlyMap<string, BodyDistanceUnit> {
  return new Map(rows.map((row) => [row.key, row.unit]));
}

/**
 * An instrument view's list and camera controls (plan R07, T19), in the side column's first
 * column while the `CONTROLS` selector points at the instrument, as the primary's are while it
 * points at `PRIMARY`: its targets, each with its range, from which the keyboard selects its mark,
 * and its presets, targets and field of view, and where its camera is and looks (`POSITION`,
 * `POINTING`; R07.T19.f), which its slot has no room to state. Each panel carries the instrument's
 * name as its designator. Two page tabs, `CAMERA` and `GROUND`, head the camera panel's place
 * (`PlacePanel`; plan R10, R10.T12.d): on `GROUND` its survey line, `DATUM` and terrain readout
 * stand there, which its slot has no room for either.
 *
 * @remarks
 * Its style control stands at the head of the second column, where `ViewDisplay` sets the
 * `CONTROLS` view's style, the primary's or an instrument's (decision-r07-t19b-exposure-fit,
 * item 2).
 */
export function InstrumentControls({
  designator,
  shown,
  selection,
  stale,
  easedMoves,
  reducedMotion,
  onAction,
  onEasedMovesChange,
  onSelect,
  folds,
  modifier,
  place,
  targetsRef,
  groundNote,
}: InstrumentControlsProps) {
  const titleId = useId();
  const { run, surveyEdges } = shown;
  // The ranges switch unit with hysteresis, adjusted during render as each run arrives, and its
  // notes with them (R10.T16.c).
  const [shownRows, setShownRows] = useState<ShownRows>(() => ({
    run,
    rows: markRows(run),
    edges: surveyEdges,
    notes: surveyEdgeNotes(run.scene, surveyEdges),
  }));
  let { rows, notes } = shownRows;
  if (shownRows.run !== run || shownRows.edges !== surveyEdges) {
    rows = shownRows.run === run ? rows : markRows(run, unitsOf(shownRows.rows));
    notes = surveyEdgeNotes(run.scene, surveyEdges, shownRows.notes);
    setShownRows({ run, rows, edges: surveyEdges, notes });
  }
  const cameraScene = cameraSceneOf(run.scene);
  const cameraPlace = useCameraPlace(run);
  return (
    <>
      <section className="panel view-targets" aria-labelledby={titleId} ref={targetsRef}>
        <h2 className="panel__title" id={titleId}>
          Targets{stale ? <StaleMark /> : null}{" "}
          <span className="panel__designator">{designator}</span>
        </h2>
        <ViewMarkList
          rows={rows}
          notes={notes}
          groundNote={groundNote}
          fromCamera={rangesFromCamera(run.scene)}
          stale={stale}
          selectedKey={selection === null ? null : targetKey(selection)}
          onSelect={(row) => {
            onSelect(row.target);
          }}
        />
      </section>
      {folds.row}
      <PlacePanel
        page={place.page}
        groundReason={place.groundReason}
        onPage={place.onPage}
        designator={designator}
        id={folds.cameraId}
        hidden={folds.cameraHidden}
      >
        {place.page === "ground" ? (
          <GroundPage
            parts={place.parts}
            readout={place.readout}
            survey={place.survey}
            plateRef={place.plateRef}
            sizerRef={place.sizerRef}
          />
        ) : (
          <CameraControls
            bare
            preset={run.camera.preset}
            offered={offeredPresets(cameraScene)}
            fovDeg={run.camera.fovDeg}
            rateStep={run.camera.free.rateStep}
            maxRateStep={maxFreeRateStep(cameraScene)}
            place={cameraPlace}
            sceneStale={stale}
            modifier={modifier}
            easedMoves={easedMoves}
            reducedMotion={reducedMotion}
            onAction={onAction}
            onEasedMovesChange={onEasedMovesChange}
          />
        )}
      </PlacePanel>
    </>
  );
}
