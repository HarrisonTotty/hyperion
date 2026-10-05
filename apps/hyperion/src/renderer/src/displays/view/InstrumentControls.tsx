import { useId, useState } from "react";

import { StaleMark } from "../../components/StaleMark";
import type { BodyDistanceUnit } from "../../lib/format";
import { maxFreeRateStep } from "../../view/camera/freeCamera";
import type { ViewKeyAction } from "../../view/camera/keys";
import { type CameraTarget, offeredPresets } from "../../view/camera/state";
import { cameraSceneOf } from "../../view/scene/model";
import { CameraControls } from "./CameraControls";
import { StyleControl } from "./StyleControl";
import type { StyleRefusals } from "./styleRefusals";
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
  /** Why each style is held back for the instrument, the adapter's reason first. */
  readonly refusals: StyleRefusals;
  /** Whether the photorealistic style's reason is the instrument's own fault. */
  readonly faulted: boolean;
  readonly onAction: (action: ViewKeyAction) => void;
  readonly onEasedMovesChange: (easedMoves: boolean) => void;
  readonly onSelect: (target: CameraTarget) => void;
  /** How the camera and style panels stand in the side column's layout. */
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
 * An instrument view's list, camera controls and style control (plan R07, T19), in the side
 * column while the `CONTROLS` selector points at the instrument, as the primary's are while it
 * points at `PRIMARY`: its targets, each with its range, from which the keyboard selects its mark,
 * its presets, targets and field of view, and its style, held back where the adapter or the
 * quality setting allows it no photorealistic view. Each panel carries the instrument's name as
 * its designator.
 */
export function InstrumentControls({
  designator,
  shown,
  selection,
  stale,
  easedMoves,
  reducedMotion,
  refusals,
  faulted,
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
        easedMoves={easedMoves}
        reducedMotion={reducedMotion}
        onAction={onAction}
        onEasedMovesChange={onEasedMovesChange}
        designator={designator}
        id={folds.cameraId}
        hidden={folds.cameraHidden}
      />
      <StyleControl
        renderStyle={run.camera.style}
        refusals={refusals}
        faulted={faulted}
        onStyle={(style) => {
          onAction({ kind: "style", style });
        }}
        designator={designator}
        id={folds.styleId}
        hidden={folds.styleHidden}
      />
    </>
  );
}
