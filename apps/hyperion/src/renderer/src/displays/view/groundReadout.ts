/**
 * The `PRIMARY` view's terrain readout, asked at its readouts' rate (plan R10, R10.T12;
 * decision-r10-live-path, D10; decision-r10-t12-leans §3, §5–§7): the loop's half, which asks the
 * terrain session for the ground at the view's line of sight and gives each publish what its label
 * block's terrain lines are made from.
 *
 * @remarks
 * - **The primary alone** (§3). An instrument in its slot states no readout, so nothing here is
 *   asked for it.
 * - **The ask.** At each publish, the `FRAME` body (`frameBody`) where it is a lit body with a
 *   surface, its survey (`TerrainSession.survey`), the line of sight through the picture's centre
 *   (`lineOfSightOf`) and `TerrainSession.groundReadout` at the instant the view lights the body
 *   at, the scene's time less the body's light time (§6 (c)), with the 2 m baseline and the
 *   reference pad. One query is in flight at a time: a publish while one is out asks nothing, and
 *   the next asks afresh, so that the newest pose is the one asked.
 * - **The answer** is taken at the next publish, so that its readings change with the loop's
 *   readouts, and only where it is still the session's and the body's it was asked of; one for a
 *   session let go or a body no longer framed is dropped. Nothing stands before the first answer
 *   for the body (§7); a `null` answer (no seed or field held yet, or no worker left) leaves none.
 * - **The guard** (§5). A refused answer goes through the queries' `TerrainFailureGuard`, as the
 *   session's step and the survey edges do: one changes nothing, and the readings hold until the
 *   next; refusals lasting `ANNUNCIATION_ONSET_MS` fault the terrain
 *   (`TerrainSession.failComputation`). The ask's own work is guarded too, so that no throw of it
 *   stops the loop.
 * - **Nothing is asked** while no survey of the body is held, the body is not modelled, or the
 *   session's terrain is faulted (§5, §7): the readout does not stand then, and its answer is let
 *   go, so that it returns with a new first answer, as a cut.
 * - **A kept scene's body with no terrain** (decision-r10-t12-commit3 §5), a planet, dwarf planet or
 *   moon it gives no detail seed, reads `TERRAIN: NOT YET MODELLED` in the survey line's place,
 *   with or without a session, and nothing is asked of it; a giant (`ViewBody.surfaceless`) and a
 *   star state nothing.
 */

import type { BodyIdHex, UniverseTime } from "@hyperion/protocol";

import { spanFromSeconds, timeBefore } from "../../lib/scene/lightTime";
import type { ViewId } from "../../view/camera/state";
import { isLitBody } from "../../view/lighting/retarded";
import type { ViewBody } from "../../view/scene/model";
import type { TerrainFault } from "../../view/terrain/annunciation";
import { TerrainFailureGuard } from "../../view/terrain/failureGuard";
import { lineOfSightOf } from "../../view/terrain/readouts";
import type {
  GroundReadoutOptions,
  GroundSight,
  TerrainSession,
  TerrainSurvey,
} from "../../view/terrain/session";
import type { SurfaceGroundReadout } from "../../view/terrain/workers/messages";
import type { LineOfSightPoint } from "../../view/wireframe/drawList";
import { frameBody, runPose, slantRangeFromCamera, type ViewRun } from "./viewRun";

/**
 * The readout's settings (decision-r10-t12-leans §1): `SLOPE` over the 2 m `BASELINE` until
 * R10.T12.a's selector, and the capacity under the reference pad, 1.0 m wide until the own ship's
 * hull defines its pads (the guide's `PAD WIDTH`).
 */
export const GROUND_READOUT_OPTIONS: GroundReadoutOptions = { slopeBaselineM: 2, padWidthM: 1.0 };

/** What a publish gives the label block's terrain lines, the `FRAME` body's. */
export interface GroundSnapshot {
  /** The `FRAME` body, a body with a surface ({@link groundBodyOf}). */
  readonly body: BodyIdHex;
  /**
   * Whether a kept scene leaves the body's surface unset ({@link surfaceUnsetOf}), so that its
   * survey line's place reads `TERRAIN: NOT YET MODELLED`.
   */
  readonly surfaceUnset: boolean;
  /** Its survey (`TerrainSession.survey`), `null` while it is not wanted. */
  readonly survey: TerrainSurvey | null;
  /** The readout's latest answer for it, `null` before the first and while none may stand. */
  readonly readout: SurfaceGroundReadout | null;
  /** The session's terrain fault (`TerrainSession.terrainFault`), under which no readout stands. */
  readonly sessionFault: TerrainFault | null;
  /** Whether the slant range is the camera's off the hull (`slantRangeFromCamera`). */
  readonly slantFromCamera: boolean;
}

/**
 * The body whose terrain lines a view's run states: the body its `FRAME` line names
 * (`frameBody` of its camera's frame), where it is a body with a surface, a planet, dwarf planet or
 * moon with a radius (`isLitBody`) not marked as having no surface (`ViewBody.surfaceless`: a kept
 * scene's giant, or a served body in the gas-envelope state).
 */
export function groundBodyOf(run: ViewRun): ViewBody | null {
  const body = frameBody(run.camera.pose.frame, run.scene);
  return body !== null && isLitBody(body) && body.surfaceless !== true ? body : null;
}

/**
 * Whether a kept scene leaves `body`'s surface unset (decision-r10-t12-commit3 §5): it gives the
 * body no detail seed, so that no survey of it can arrive.
 */
export function surfaceUnsetOf(run: ViewRun, body: ViewBody): boolean {
  return run.scene.provenance.kind === "kept" && body.detailSeed === undefined;
}

/**
 * The body whose terrain readout a view's run states, which the terrain session keeps wanted and
 * whose terrain's fault its label block shows (`TerrainViewReport.readoutBody`): its
 * {@link groundBodyOf} where a kept scene does not leave its surface unset; else `null`.
 */
export function readoutBodyOf(run: ViewRun): BodyIdHex | null {
  const body = groundBodyOf(run);
  return body === null || surfaceUnsetOf(run, body) ? null : body.id;
}

/**
 * The instant a view lights `body` at (decision-r10-t12-leans §6 (c)): the scene's time less the
 * body's light time to the camera, its light's emission; the scene's time in a kept scene, where
 * that is 0, or where the body's retarded centre is not known.
 */
export function litTimeOf(run: ViewRun, body: ViewBody): UniverseTime {
  const lightTimeS = body.retarded?.lightTimeS ?? 0;
  return lightTimeS > 0 && Number.isFinite(lightTimeS)
    ? timeBefore(run.scene.time, spanFromSeconds(lightTimeS))
    : run.scene.time;
}

/**
 * What a terrain readout's answer gives the view's tripod (`ViewFrameInputs.lineOfSightPoint`): a
 * point over ground, surveyed or not, with its slant range from the camera where the answer gives
 * one, by which only a hull nearer than the point makes the tripod give way
 * (decision-r10-t12-commit3 §1); none at `NO GROUND`.
 */
export function lineOfSightPointOf(readout: SurfaceGroundReadout): LineOfSightPoint {
  let point: LineOfSightPoint;
  switch (readout.kind) {
    case "ground":
      point = { hasPoint: true, rangeM: readout.sight.rangeM };
      break;
    case "not-surveyed":
      point = { hasPoint: true, rangeM: null };
      break;
    case "no-ground":
      point = { hasPoint: false, rangeM: null };
      break;
  }
  return point;
}

/** A query asked: of which session and body. */
interface Asked {
  readonly session: TerrainSession;
  readonly body: BodyIdHex;
}

/** A query's outcome, held for the next publish. */
type Settled =
  | (Asked & { readonly kind: "answered"; readonly readout: SurfaceGroundReadout | null })
  | (Asked & { readonly kind: "refused"; readonly error: unknown });

/** The guard's report of the readout's queries. */
const DESCRIBE_QUERY = "the terrain readout's query failed:";

/** The guard's report of the ask's own work. */
const DESCRIBE_ASK = "the terrain readout could not be asked:";

/** A view's terrain readout, asked at each publish of its loop (see the module's remarks). */
export class GroundReadouts {
  readonly #view: ViewId;
  /** The refused answers' guard, and the ask's own; one each a session. */
  #answers = new TerrainFailureGuard();
  #asks = new TerrainFailureGuard();
  #session: TerrainSession | null = null;
  #body: BodyIdHex | null = null;
  #readout: SurfaceGroundReadout | null = null;
  #inFlight: Asked | null = null;
  #settled: Settled | null = null;

  /** @param view - The view whose readout it asks: the primary's (decision-r10-t12-leans §3). */
  constructor(view: ViewId) {
    this.#view = view;
  }

  /**
   * One publish: takes the answer that came since the last, asks the next where none is in flight,
   * and gives what the label block's terrain lines are made from; `null` where the view states
   * none, with a `FRAME` that is not a body with a surface, or no session for a body a kept scene
   * does not leave unset.
   *
   * @param nowMs - The frame's monotonic time, ms, by which the guards count a run of failures.
   */
  publish(session: TerrainSession | null, run: ViewRun, nowMs: number): GroundSnapshot | null {
    if (session !== this.#session) {
      this.#session = session;
      this.#answers = new TerrainFailureGuard();
      this.#asks = new TerrainFailureGuard();
      this.#forget();
    }
    const body = groundBodyOf(run);
    const slantFromCamera = slantRangeFromCamera(run.camera, run.scene);
    if (body !== null && surfaceUnsetOf(run, body)) {
      this.#forget();
      return {
        body: body.id,
        surfaceUnset: true,
        survey: null,
        readout: null,
        sessionFault: null,
        slantFromCamera,
      };
    }
    if (session === null || body === null) {
      this.#forget();
      return null;
    }
    if (body.id !== this.#body) {
      this.#forget();
      this.#body = body.id;
    }
    this.#take(session, body.id, nowMs);
    const survey = session.survey(body.id);
    const sessionFault = session.terrainFault;
    const header = survey?.header ?? null;
    if (
      survey === null ||
      header === null ||
      survey.status.notModelled !== null ||
      sessionFault !== null
    ) {
      // No readout may stand: what is held or still to come is let go, so that it returns with a
      // first answer of its own.
      this.#readout = null;
      this.#settled = null;
      this.#inFlight = null;
    } else if (this.#inFlight === null) {
      this.#asks.run(
        () => {
          this.#ask(session, run, body, header);
        },
        undefined,
        nowMs,
        {
          describe: DESCRIBE_ASK,
          onFault: () => {
            session.failComputation();
          },
        },
      );
    }
    return {
      body: body.id,
      surfaceUnset: false,
      survey,
      readout: this.#readout,
      sessionFault,
      slantFromCamera,
    };
  }

  /** Lets the answer held go, and any answer still to come with it. */
  #forget(): void {
    this.#body = null;
    this.#readout = null;
    this.#settled = null;
    this.#inFlight = null;
  }

  /** Takes the outcome that came since the last publish, where it is this session's and body's. */
  #take(session: TerrainSession, body: BodyIdHex, nowMs: number): void {
    const settled = this.#settled;
    this.#settled = null;
    if (settled === null || settled.session !== session || settled.body !== body) {
      return;
    }
    switch (settled.kind) {
      case "answered":
        this.#answers.succeeded();
        this.#readout = settled.readout;
        break;
      case "refused":
        this.#answers.failed(settled.error, nowMs, {
          describe: DESCRIBE_QUERY,
          onFault: () => {
            session.failComputation();
          },
        });
        break;
    }
  }

  /** Asks the readout along the view's line of sight now, its outcome held for the next publish. */
  #ask(
    session: TerrainSession,
    run: ViewRun,
    body: ViewBody,
    header: NonNullable<TerrainSurvey["header"]>,
  ): void {
    const sight = lineOfSightOf(runPose(run), run.scene, body, header);
    if (sight === null) {
      // No line of sight to ask along (the body's rotation not known): no readout can stand.
      this.#readout = null;
      return;
    }
    const asked: Asked = { session, body: body.id };
    this.#inFlight = asked;
    void this.#query(asked, sight, litTimeOf(run, body)).catch((error: unknown) => {
      console.error("the terrain readout's answer could not be taken:", error);
    });
  }

  /**
   * The query `asked`, its outcome held for the next publish, its refusal among them, which that
   * publish takes through the guard; an outcome of a query let go meanwhile is dropped.
   */
  async #query(asked: Asked, sight: GroundSight, time: UniverseTime): Promise<void> {
    let settled: Settled;
    try {
      const readout = await asked.session.groundReadout(
        this.#view,
        asked.body,
        sight,
        time,
        GROUND_READOUT_OPTIONS,
      );
      settled = { ...asked, kind: "answered", readout };
    } catch (error: unknown) {
      settled = { ...asked, kind: "refused", error };
    }
    if (this.#inFlight === asked) {
      this.#inFlight = null;
      this.#settled = settled;
    }
  }
}
