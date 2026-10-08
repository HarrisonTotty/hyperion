import { useId, useState } from "react";

import { StaleMark } from "../../components/StaleMark";
import type { BodyDistanceUnit } from "../../lib/format";
import { maxFreeRateStep } from "../../view/camera/freeCamera";
import type { ViewKeyAction } from "../../view/camera/keys";
import { type CameraTarget, offeredPresets } from "../../view/camera/state";
import { cameraSceneOf } from "../../view/scene/model";
import { CameraControls } from "./CameraControls";
import { useCameraPlace } from "./cameraPlace";
import type { InstrumentShown } from "./useInstruments";
import type { SideFolds } from "./viewLayout";
import { ViewMarkList } from "./ViewMarkList";
import { type MarkRow, markRows, rangesFromCamera, targetKey, type ViewRun } from "./viewRun";

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
}

/** The rows of the list, with the units they were last shown in kept for their hysteresis. */
interface ShownRows {
  readonly run: ViewRun;
  readonly rows: ReadonlyArray<MarkRow>;
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
 * name as its designator.
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
}: InstrumentControlsProps) {
  const titleId = useId();
  const { run } = shown;
  // The ranges switch unit with hysteresis, adjusted during render as each run arrives.
  const [shownRows, setShownRows] = useState<ShownRows>(() => ({ run, rows: markRows(run) }));
  let rows = shownRows.rows;
  if (shownRows.run !== run) {
    rows = markRows(run, unitsOf(shownRows.rows));
    setShownRows({ run, rows });
  }
  const cameraScene = cameraSceneOf(run.scene);
  const place = useCameraPlace(run);
  return (
    <>
      <section className="panel view-targets" aria-labelledby={titleId}>
        <h2 className="panel__title" id={titleId}>
          Targets{stale ? <StaleMark /> : null}{" "}
          <span className="panel__designator">{designator}</span>
        </h2>
        <ViewMarkList
          rows={rows}
          fromCamera={rangesFromCamera(run.scene)}
          stale={stale}
          selectedKey={selection === null ? null : targetKey(selection)}
          onSelect={(row) => {
            onSelect(row.target);
          }}
        />
      </section>
      {folds.row}
      <CameraControls
        preset={run.camera.preset}
        offered={offeredPresets(cameraScene)}
        fovDeg={run.camera.fovDeg}
        rateStep={run.camera.free.rateStep}
        maxRateStep={maxFreeRateStep(cameraScene)}
        place={place}
        sceneStale={stale}
        easedMoves={easedMoves}
        reducedMotion={reducedMotion}
        onAction={onAction}
        onEasedMovesChange={onEasedMovesChange}
        designator={designator}
        id={folds.cameraId}
        hidden={folds.cameraHidden}
      />
    </>
  );
}
