import { type KeyboardEvent, useId } from "react";

import { StatusLine } from "../../components/StatusLine";
import { pick } from "../../spatial/pick";
import type { AppearanceLabel } from "../../view/appearance/fromWire";
import type { CameraTarget, RenderStyle } from "../../view/camera/state";
import type { LightingState } from "../../view/lighting/hostLights";
import type { ExposureControl } from "../../view/photometry/exposure";
import { styleName } from "../../view/photoreal/style";
import { cameraSceneOf } from "../../view/scene/model";
import type { Instrument } from "./useInstruments";
import { ViewCanvas } from "./ViewCanvas";
import { ViewLabelBlock } from "./ViewLabelBlock";
import { PRIMARY_NAME } from "./viewNames";
import {
  type LabelLine,
  labelLines,
  labelStatements,
  photorealStatements,
  PHOTOREAL_PREPARING,
  POSITIONS_FROM_SHIP,
  PRESET_NAMES,
  targetKey,
} from "./viewRun";

/** Props of {@link InstrumentView}. */
export interface InstrumentViewProps {
  readonly instrument: Instrument;
  /** The primary view's exposure, which the instrument draws at and names as its source. */
  readonly exposure: ExposureControl;
  /** The style its budget draws (`ViewBudget.style`). */
  readonly budgetStyle: RenderStyle;
  /** Whether the server's scene is stale: the time is muted with its `S`. */
  readonly stale: boolean;
  /** The scene's lighting and the lit bodies' labels, the primary's, for a photorealistic frame. */
  readonly lighting: LightingState;
  readonly litLabels: ReadonlyArray<AppearanceLabel>;
  /** A graphics fault standing for this view (its own `GRAPHICS VIEW REFUSED`), or `null`. */
  readonly fault: string | null;
  /** The ID of the text describing the canvases' keys. */
  readonly legendId: string;
  readonly onKeyDown: (event: KeyboardEvent<HTMLCanvasElement>) => void;
  readonly onKeyUp: (event: KeyboardEvent<HTMLCanvasElement>) => void;
  readonly onBlur: () => void;
  /** Called with a click's mark, or `null` where it picks none; either way the view is acted on. */
  readonly onPick: (target: CameraTarget | null) => void;
}

/**
 * The lines of an instrument's label block (decision-r07-t19, item 2b): every line of the
 * primary's but `SCENE`, which the header's `TRAINING` banner states for the display, and the
 * exposure's `SOURCE`, the primary view, before `STARS`.
 */
const INSTRUMENT_LINES: ReadonlyArray<string> = [
  "FRAME",
  "TIME",
  "STYLE",
  "CAMERA",
  "FOV",
  "EXPOSURE",
  "SOURCE",
  "STARS",
];

/** The pointer's reach to a mark, rem: half the guide's 2 rem target, as the primary's. */
const PICK_REM = 1;

/** Where the engine refused the instrument's canvas (the primary's wording). */
const NOT_MADE = "GRAPHICS NOT AVAILABLE: views could not be made, relaunch to retry";

function instrumentLines(
  lines: ReadonlyArray<LabelLine>,
  drawn: RenderStyle,
  skyLabel: string | null,
): ReadonlyArray<LabelLine> {
  const byLabel = new Map<string, LabelLine>(lines.map((line) => [line.label, line]));
  byLabel.set("SOURCE", { label: "SOURCE", value: PRIMARY_NAME });
  byLabel.set("STYLE", { label: "STYLE", value: styleName(drawn) });
  const stars = byLabel.get("STARS");
  if (skyLabel !== null && stars !== undefined) {
    byLabel.set("STARS", { ...stars, value: skyLabel });
  }
  return INSTRUMENT_LINES.flatMap((label) => {
    const line = byLabel.get(label);
    return line === undefined ? [] : [line];
  });
}

/**
 * An instrument view in its slot over the primary view's right edge (plan R07, T19; Design note
 * 15; decision-r07-t19 item 2): a panel named for the slot, holding its label block beside its
 * canvas, a 4:3 picture flown by the flight keys while it has focus and picked from by a click.
 *
 * @remarks
 * The label block is static text on the panel's own surface, before the canvas in the reading
 * order, since a plate over so small a picture would hide most of it. It states the drawn style,
 * the camera and field of view, the primary's exposure with its `SOURCE`, since an instrument
 * meters no image of its own (Design note 11), and the stars of its own cull of the sky, at its
 * camera's limit; and, while they hold, `POSITIONS AS SEEN FROM SHIP`, `PHOTOREALISTIC: PREPARING`
 * and its graphics fault (decision-r07-t19, item 2b). The scene's own statements (its lighting, its
 * bodies' labels, `ROTATION NOT YET MODELLED`) are the primary's, which states them for every view.
 * Its list and its camera and style controls are the side column's while `CONTROLS` points at it.
 */
export function InstrumentView({
  instrument,
  exposure,
  budgetStyle,
  stale,
  lighting,
  litLabels,
  fault,
  legendId,
  onKeyDown,
  onKeyUp,
  onBlur,
  onPick,
}: InstrumentViewProps) {
  const titleId = useId();
  const labelId = useId();
  const { shown, name } = instrument;
  const drawn = shown?.drawnStyle ?? "wireframe";
  const run = shown?.run ?? null;
  const pickAt = (xPx: number, yPx: number): void => {
    if (shown === null) {
      return;
    }
    const ratio = instrument.size?.devicePixelRatio ?? 1;
    const remPx = instrument.size?.remPx ?? 16;
    const picked = pick(
      shown.anchors.map((anchor) => ({
        id: targetKey(anchor.target),
        xPx: anchor.xPx,
        yPx: anchor.yPx,
        depth: anchor.distanceM,
        radiusPx: 0,
      })),
      { xPx: xPx * ratio, yPx: yPx * ratio },
      PICK_REM * remPx * ratio,
    );
    onPick(
      cameraSceneOf(shown.run.scene).targets.find((each) => targetKey(each) === picked) ?? null,
    );
  };
  return (
    <section
      className={`panel view-instrument view-instrument--slot-${String(instrument.slot)}`}
      aria-labelledby={titleId}
    >
      <h2 className="panel__title view-instrument__title" id={titleId}>
        {name}
      </h2>
      <div className="view-instrument__body">
        <div className="view-instrument__label">
          {run === null ? null : (
            <ViewLabelBlock
              id={labelId}
              lines={instrumentLines(labelLines(run, exposure, stale), drawn, instrument.skyLabel)}
              statements={[
                ...labelStatements(run).filter((statement) => statement === POSITIONS_FROM_SHIP),
                // The view's own: the scene's lighting and its bodies' labels are the primary's.
                ...photorealStatements(
                  { ...run, camera: { ...run.camera, style: budgetStyle } },
                  lighting,
                  drawn,
                  litLabels,
                ).filter((statement) => statement === PHOTOREAL_PREPARING),
              ]}
              countLine={null}
              fault={fault}
            />
          )}
        </div>
        {instrument.refused ? (
          <div className="view__unavailable view-instrument__unavailable">
            <StatusLine text={NOT_MADE} standing="fault" />
          </div>
        ) : (
          <ViewCanvas
            canvasRef={instrument.canvasRef}
            stageRef={instrument.stageRef}
            accessibleName={`VIEW, ${styleName(drawn)}, ${name}${
              run === null ? "" : `, ${PRESET_NAMES[run.camera.preset]}`
            }`}
            describedBy={run === null ? legendId : `${labelId} ${legendId}`}
            onKeyDown={onKeyDown}
            onKeyUp={onKeyUp}
            onBlur={onBlur}
            onPick={pickAt}
          >
            {null}
          </ViewCanvas>
        )}
      </div>
    </section>
  );
}
