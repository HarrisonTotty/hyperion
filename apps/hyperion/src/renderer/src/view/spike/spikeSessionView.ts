/**
 * A view of the descent spike over a generated world (plan R10, R10.T14.c;
 * decision-r10-t14c-leans §3): `VIEW`'s own frame drawer (`ViewFrameDrawer`) over the terrain
 * session, its frames reported to the session and drawn as `VIEW`'s are.
 *
 * @remarks
 * The spike's main view is one. Each frame goes in `VIEW`'s primary's order (`ViewDisplay`'s
 * loop): the view's report is noted with the session ({@link SpikeSessionView.note}:
 * `terrainReportOf` of the frame's inputs, the drawer's last regimes and the style it last drew
 * in), then the session is stepped once (the run's, `stepTerrainSession` under its guard), then
 * the view draws ({@link SpikeSessionView.draw}) its disc, its reference surface, the class map,
 * the mask and the session's terrain, in either style.
 *
 * A second view in the same session, T14.b's drawer under `instrumentViewId(slot)`, is another
 * instance on the same engine and session: noted before the step as the main view is, or after its
 * draw as `VIEW`'s instruments are, and drawn after the step, the session stepped once a frame
 * whatever the views; the views at one pose state one contour interval through a shared
 * {@link SpikeSessionFrame.intervals}.
 *
 * Nothing of R06's sky is drawn: the views are given no interim stars and no sky (the spike's scene
 * carries no stars either), so the drawer draws neither star sprites nor a sky cube. The exposure
 * is the spike's fixed `MAN` day exposure (`spikeExposure`), and no view meters.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import {
  type GroundSnapshot,
  groundBodyOf,
  surfaceUnsetOf,
} from "../../displays/view/groundReadout";
import {
  terrainReportOf,
  ViewFrameDrawer,
  type ViewFrameDrawn,
  type ViewFrameInputs,
} from "../../displays/view/viewFrameDrawer";
import { slantRangeFromCamera, type ViewRun } from "../../displays/view/viewRun";
import type { ElementSize } from "../../lib/useElementSize";
import type { ColourTokens } from "../../spatial/paint";
import type { ViewBudget } from "../budget/viewBudget";
import type { CameraTarget, RenderStyle, ViewId } from "../camera/state";
import type { StyleAvailability } from "../engine/platform";
import type { RenderEngine, RenderView } from "../engine/types";
import type { ExposureControl } from "../photometry/exposure";
import type { PhotorealStatus } from "../photoreal/renderer";
import type { QualitySetting } from "../quality/qualitySetting";
import type { ViewTerrainSession } from "../terrain/frameTerrain";
import type { PoseIntervals } from "../terrain/poseInterval";
import type { TerrainSession } from "../terrain/session";
import { spikeExposure } from "./litView";

/** What a session view reads of its terrain session: the drawer's, and its report's. */
export interface SpikeViewSession
  extends
    ViewTerrainSession,
    Pick<TerrainSession, "noteView" | "forgetView" | "classMaps" | "reliefs"> {}

/** Who a session view is. */
export interface SpikeSessionViewOptions {
  /** The view's ID in the session: its reports' and its draws' (`SPIKE_VIEW_NAMES.main`'s). */
  readonly id: ViewId;
  /** The drawer's name on the engine: its renderers', and its hold on the sky's cube. */
  readonly name: string;
  /** Its budget's terrain streaming priority: `primary` for the main view (R05 Design note 24). */
  readonly streamPriority: ViewBudget["streamPriority"];
  /**
   * The body whose terrain readout it states (`TerrainViewReport.readoutBody`), wanted wherever its
   * camera is and whose terrain fault it shows, or `null` for none.
   */
  readonly readoutBody: BodyIdHex | null;
}

/** What one frame of a session view is drawn from. */
export interface SpikeSessionFrame {
  /** Its run: the kept scene at the script's time, its camera holding the scripted pose. */
  readonly run: ViewRun;
  /** Its stage as laid out, or `null` before it is, when it draws nothing. */
  readonly stage: ElementSize | null;
  readonly tokens: ColourTokens;
  readonly selection: CameraTarget | null;
  readonly setting: QualitySetting;
  /** The frame's monotonic time, ms. */
  readonly nowMs: number;
  /** The holder through which the views at one pose state one contour interval; absent, its own. */
  readonly intervals?: PoseIntervals;
}

/** The styles a spike view may draw: both, as `--style` asks. */
const BOTH_STYLES: StyleAvailability = { wireframe: true, photorealistic: true };

/**
 * What a spike view's survey line is made from at a publish (plan R10, R10.T14.c;
 * decision-r10-t14c-guide Q1): its `FRAME` body's survey (`TerrainSession.survey`), read on the
 * render thread, and the session's terrain fault, as `VIEW`'s `GroundReadouts.publish` gives them,
 * with no readout: the spike asks the height workers nothing for its label block, so that its
 * `useTerrainLines` states the survey line alone, no `DATUM` and no `GROUND AT LINE OF SIGHT`.
 *
 * @remarks
 * `null` where its `FRAME` is not a body with a surface (`groundBodyOf`) or there is no session; a
 * kept scene's body it leaves unset reads `TERRAIN: NOT YET MODELLED` in the line's place, as on
 * `VIEW`.
 */
export function surveySnapshotOf(
  session: Pick<TerrainSession, "survey" | "terrainFault"> | null,
  run: ViewRun,
): GroundSnapshot | null {
  const body = groundBodyOf(run);
  if (body === null) {
    return null;
  }
  const slantFromCamera = slantRangeFromCamera(run.camera, run.scene);
  if (surfaceUnsetOf(run, body)) {
    return {
      body: body.id,
      surfaceUnset: true,
      survey: null,
      readout: null,
      sessionFault: null,
      slantFromCamera,
    };
  }
  if (session === null) {
    return null;
  }
  return {
    body: body.id,
    surfaceUnset: false,
    survey: session.survey(body.id),
    readout: null,
    sessionFault: session.terrainFault,
    slantFromCamera,
  };
}

/** One view of the spike over a generated world: its drawer, and its standing in the session. */
export class SpikeSessionView {
  readonly #drawer: ViewFrameDrawer;
  readonly #options: SpikeSessionViewOptions;
  readonly #exposure: ExposureControl = spikeExposure();
  /** The style its last frame was drawn in, by which the session hands bodies to its passes (S8). */
  #drawnStyle: RenderStyle = "wireframe";
  /** The session its reports went to, which forgets it at its disposal. */
  #reportedTo: SpikeViewSession | null = null;

  /**
   * Draws into `view`, made by `engine`; disposes of the view with itself.
   *
   * @throws `EngineUnavailable` while the engine has no device, as `ViewFrameDrawer` does.
   */
  constructor(engine: RenderEngine, view: RenderView, options: SpikeSessionViewOptions) {
    this.#drawer = new ViewFrameDrawer(engine, view, options.name);
    this.#options = options;
  }

  /** Its ID in the session. */
  get id(): ViewId {
    return this.#options.id;
  }

  /**
   * The style its last frame was drawn in (`ViewFrameDrawn.drawnStyle`): the wireframe before any,
   * and the photorealistic view's stand-in while its pipelines compile.
   */
  get drawnStyle(): RenderStyle {
    return this.#drawnStyle;
  }

  /** Its photorealistic renderer's standing (`ViewFrameDrawer.photorealStatus`). */
  get photorealStatus(): PhotorealStatus {
    return this.#drawer.photorealStatus;
  }

  /**
   * The drawer's inputs for `frame` over `session`, as `VIEW`'s primary builds them: in its run's
   * camera's style at the setting, unscaled, with no stars, no sky and no meter, the session's class
   * maps and reliefs, and its own terrain of the session's passes.
   */
  inputs(session: SpikeViewSession, frame: SpikeSessionFrame): ViewFrameInputs {
    return {
      run: frame.run,
      size: frame.stage,
      tokens: frame.tokens,
      exposure: this.#exposure,
      stars: [],
      sky: null,
      selection: frame.selection,
      style: frame.run.camera.style,
      setting: frame.setting,
      renderScale: 1,
      availability: BOTH_STYLES,
      meter: null,
      classMaps: session.classMaps,
      reliefs: session.reliefs,
      terrain: {
        source: session,
        view: this.#options.id,
        nowMs: frame.nowMs,
        intervals: frame.intervals,
      },
    };
  }

  /**
   * Notes what it draws this frame with `session` (`TerrainSession.noteView`), for the step that
   * follows: `terrainReportOf` of `inputs`, its drawer's regimes, its streaming priority and the
   * style it last drew in, with its readout body; nothing while its stage is not laid out.
   */
  note(session: SpikeViewSession, inputs: ViewFrameInputs): void {
    const report = terrainReportOf(
      inputs,
      this.#drawer.regimes,
      this.#options.streamPriority,
      this.#drawnStyle,
    );
    if (report === null) {
      return;
    }
    if (this.#reportedTo !== session) {
      this.#reportedTo?.forgetView(this.#options.id);
      this.#reportedTo = session;
    }
    session.noteView(this.#options.id, { ...report, readoutBody: this.#options.readoutBody });
  }

  /** Draws its frame of `inputs`, after the session's step; `null` while its stage is not laid out. */
  draw(inputs: ViewFrameInputs): ViewFrameDrawn | null {
    const drawn = this.#drawer.draw(inputs);
    if (drawn !== null) {
      this.#drawnStyle = drawn.drawnStyle;
    }
    return drawn;
  }

  /** Forgets it in the session it reported to, and disposes of the drawer with its view. */
  dispose(): void {
    this.#reportedTo?.forgetView(this.#options.id);
    this.#reportedTo = null;
    this.#drawer.dispose();
  }
}
