/**
 * A still's own terrain view (plan R11, R11.T10.e; ruling 4a of decision-leans-54): the view's
 * terrain at the still's request, its cells frozen there, its patches selected at the still's own
 * size and streamed at bulk priority through R05, and whether its frustum has streamed to the level
 * its pixels warrant, the job's streaming wait (Design note 17).
 *
 * @remarks
 * At the request the view's terrain session takes the still as a view of its own,
 * {@link STILL_TERRAIN_VIEW_ID}, noted with the view's last report at the still's size and at
 * `bulk` priority: its demand streams after every request of a view that is not bulk, so that a
 * still never holds the live view's terrain back. The session holds the survey of the bodies the
 * still hands over as it stood at the request (`TerrainSession.holdSurvey`), so that no bake, mask
 * or selection the still draws meets a cell surveyed after it; their figures give way over the
 * coverage masks the view's frame took at the request. Each sample draws the terrain through a
 * frame terrain of its own ({@link StillTerrainView.frameTerrain}) at the time it is drawn, since
 * the passes' fades run on the clock. The still's view is forgotten and the hold released at its
 * disposal.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import type { BodyFramePlan, LitBodyInput } from "../bodies/draw";
import type { ProjectionCamera } from "../camera/projection";
import { type ViewId, viewId } from "../camera/state";
import type { FrameOcclusion } from "../clusters/hzb";
import type { DrawItem, TextureHandle, ViewSize } from "../engine/types";
import type { FrameTerrain, FrameVisibility } from "../photoreal/renderer";
import { type FrameTerrainSource, LiveFrameTerrain } from "../terrain/frameTerrain";
import type { TerrainViewReport } from "../terrain/session";

/** The view a still's terrain is selected, streamed and drawn for in its view's session. */
export const STILL_TERRAIN_VIEW_ID: ViewId = viewId("STILL");

/** What a still takes of its view's terrain session: `TerrainSession`'s, structurally. */
export interface StillTerrainSession extends FrameTerrainSource {
  /** Notes view `id`'s report (`TerrainSession.noteView`). */
  noteView(id: ViewId, report: TerrainViewReport): void;
  /** Forgets view `id` (`TerrainSession.forgetView`). */
  forgetView(id: ViewId): void;
  /** Holds `bodies`' survey as it stands (`TerrainSession.holdSurvey`); returns the release. */
  holdSurvey(bodies: Iterable<BodyIdHex>): () => void;
  /** Whether view `id` has streamed over each of `bodies` (`TerrainSession.streamedFor`). */
  streamedFor(id: ViewId, bodies: Iterable<BodyIdHex>): boolean;
}

/** What a view gives a still of its terrain at the request. */
export interface StillTerrainCapture {
  readonly session: StillTerrainSession;
  /** The view's last report to the session, which the still's view is noted from. */
  readonly report: TerrainViewReport;
  /**
   * The symbology's slope term its frame's terrain drew with (`LiveFrameTerrainInput`'s
   * `occluderSlopePx`), which the terrain's depth draw carries.
   */
  readonly occluderSlopePx: number;
}

/** A body's coverage mask as the view's frame took it at the request, or `null` with none. */
export type StillCoverage = (
  body: BodyIdHex,
) => { readonly mask: TextureHandle; readonly whole: boolean } | null;

/** What a {@link StillTerrainView} is made with, beyond the view's capture. */
export interface StillTerrainOptions {
  /** The still's camera: the frozen frame's orientation and field of view. */
  readonly camera: ProjectionCamera;
  /** The still's size, px: the size its terrain is selected at, its pixels the still's own. */
  readonly size: ViewSize;
  /** The still's pre-exposure. */
  readonly exposureScale: number;
  /** The bodies the still hands over to their terrain: the frozen plan's `terrain` regimes. */
  readonly bodies: ReadonlyArray<BodyIdHex>;
  /** The regimes of the frozen plan, by which the session's passes take the still's view. */
  readonly regimes: TerrainViewReport["regimes"];
  /** The coverage masks the view's frame took at the request. */
  readonly coverage: StillCoverage;
  /** A monotonic clock, ms, which each sample's terrain is drawn at. */
  readonly nowMs: () => number;
}

/**
 * A still's own terrain view in its view's session (see the module's remarks), from its making at
 * the request to its disposal with the still.
 */
export class StillTerrainView {
  readonly #session: StillTerrainSession;
  readonly #options: StillTerrainOptions;
  readonly #occluderSlopePx: number;
  readonly #release: () => void;
  #disposed = false;

  constructor(capture: StillTerrainCapture, options: StillTerrainOptions) {
    this.#session = capture.session;
    this.#options = options;
    this.#occluderSlopePx = capture.occluderSlopePx;
    // Frozen first: nothing surveyed from here on reaches the still.
    this.#release = capture.session.holdSurvey(options.bodies);
    const {
      litTauScale: _litTauScale,
      readoutBody: _readoutBody,
      floorStepped: _floorStepped,
      ...report
    } = capture.report;
    // Its pixels are the still's: counted at its own size, its τ the setting's own.
    capture.session.noteView(STILL_TERRAIN_VIEW_ID, {
      ...report,
      presentedPx: options.size,
      selectionPx: options.size,
      drawnStyle: "photorealistic",
      streamPriority: "bulk",
      regimes: options.regimes,
    });
  }

  /** The bodies it hands over to their terrain. */
  get bodies(): ReadonlyArray<BodyIdHex> {
    return this.#options.bodies;
  }

  /**
   * Whether its frustum has streamed to the level the still's pixels warrant over every body it
   * hands over, at the session's last step (`TerrainSession.streamedFor`); `true` with none.
   */
  streamed(): boolean {
    return (
      this.#options.bodies.length === 0 ||
      this.#session.streamedFor(STILL_TERRAIN_VIEW_ID, this.#options.bodies)
    );
  }

  /**
   * The terrain one sample draws: the session's draws for the still's view at the time it is
   * drawn, its figures giving way over the coverage masks taken at the request.
   */
  frameTerrain(): FrameTerrain {
    const { camera, size, exposureScale, coverage, nowMs } = this.#options;
    const live = new LiveFrameTerrain(this.#session, {
      view: STILL_TERRAIN_VIEW_ID,
      camera,
      presentedPx: size,
      selectionPx: size,
      exposureScale,
      occluderSlopePx: this.#occluderSlopePx,
      nowMs: nowMs(),
    });
    return {
      surveyCoverage: (body) => coverage(body)?.mask ?? null,
      surveyedWhole: (body) => coverage(body)?.whole ?? false,
      innerRadiusM: (body) => live.innerRadiusM(body),
      draws: (
        plan: BodyFramePlan,
        bodies: ReadonlyArray<LitBodyInput>,
        occlusion?: FrameOcclusion,
      ): ReadonlyArray<DrawItem> => live.draws(plan, bodies, occlusion),
      lateDraws: () => live.lateDraws(),
      symbologyDepth: () => live.symbologyDepth(),
      sceneDepth: () => live.sceneDepth(),
      visibility: (): FrameVisibility | null => live.visibility(),
      opaqueDraws: () => live.opaqueDraws(),
    };
  }

  /** Forgets the still's view and releases the survey's hold; a second call does nothing. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#session.forgetView(STILL_TERRAIN_VIEW_ID);
    this.#release();
  }
}
