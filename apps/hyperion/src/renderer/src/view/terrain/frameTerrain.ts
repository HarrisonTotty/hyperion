/**
 * A view's live terrain in its frame (plan R10, R10.T16.c; decision-r10-live-path, D5, D7, D9 and
 * D10): the `FrameTerrain` the terrain session gives each view's photorealistic frame, the lights
 * its body's disc record lights it with, and the survey edges every view's draw list marks.
 *
 * @remarks
 * - **One rotation and one record** (D5). A body handed over (T10.e's `terrain`, a `MeshBodyPlan`
 *   whose `role` is `handed-over`) is drawn from the plan's body: its `TerrainView` is the
 *   `LitBodyInput` the plan drew the figure with ({@link terrainViewOf}: its rotation, its centre
 *   negated, the camera's orientation and field of view, the presented size), so that the
 *   terrain, its coverage mask, its class map and the reference surface share R07.T10.a's
 *   `rotationAt`; and its lights are its disc record's ({@link terrainLightingOf}), rotated into the
 *   body-fixed axes by the inverse of that rotation, so that the disc, the reference surface and
 *   the terrain are lit by one record. A body whose rotation is not known is never handed over:
 *   its figure and disc stand.
 * - **The mask** (D7). {@link LiveFrameTerrain.surveyCoverage} gives a body's coverage mask only
 *   while the view's draw set of the body leaves nothing missing (`TerrainPass.maskReadyFor`, read
 *   before the plan takes it), so that the reference surface stays whole until the terrain draws
 *   all the surveyed ground the mask would discard.
 * - **The air** (D9; plan R08, R08.T10.b). The terrain takes its body's atmosphere tables, the
 *   record's `atmosphere` (`TerrainFrameLighting.atmosphere`, R08.T10.a's medium's, through
 *   `surfaceLightingTablesOf`), in {@link terrainLightingOf}: each light crosses the air to the
 *   ground and the sky lights it, and the photorealistic frame's composite lays the air over it
 *   after. A body with no air takes the tables of no air.
 * - **The shadow cascades** (plan R10, R10.T9.b; decision-r10-t9b-design D3). One set serves one
 *   body a view-frame: of the bodies handed over, the one over whose datum the camera stands lowest
 *   takes the view's cascades ({@link cascadeBodyIndex}), ties going to the first in the plan's
 *   order; the others are drawn at w = 0, under the horizon map alone. On the high setting its
 *   pass draws the cascades' four caster passes as {@link LiveFrameTerrain.draws} asks for its
 *   draw, so they are submitted before the bodies pass the draws are drawn in.
 * - **The label block.** Each view's frame keeps the annunciation of the bodies it hands over
 *   ({@link LiveFrameTerrain.annunciation}: `TERRAIN: STREAMING` before `TERRAIN: DETAIL LIMITED`),
 *   which its label block states through `labelStatements(run, terrain)`; the fault goes through
 *   the label block's fault lines (decision-r10-t16-contracts §7).
 * - **The survey edge**, in both styles ({@link surveyEdgeViewsOf}): each wanted body's coverage in
 *   view, its rim at max(drawn height, 0) from its pass's resident bakes that hold the edge, the
 *   datum where none does (`ResidentHeights`). Over the image the terrain's depth is drawn into the
 *   symbology's pass before its lines ({@link LiveFrameTerrain.symbologyDepth}), so that nearer
 *   ground hides the edge as it does in the image, and the note's range counts only the points
 *   that ground leaves in sight (`SurveyEdgeView.ground`); in the wireframe the rim stands on the
 *   wireframe pass's resident bakes, and the terrain the wireframe draws gives the edge its depth
 *   (R10.T11).
 * - **The wireframe's terrain** (R10.T11; D10; {@link wireframeTerrainOf}): each body a wireframe
 *   view hands over to its wireframe pass, drawn as depth with its contours and graticule at one
 *   interval for the view, and the bodies whose datum, their occluder sphere and own lines, gives
 *   way to it where their coverage mask may be given.
 */

import type { BodyIdHex, SurfaceHeaderDto } from "@hyperion/protocol";

import { scale, type Vec3, vec3 } from "../../geometry/vec3";
import { CASING_PX } from "../../lib/strokes";
import type { ColourTokens } from "../../spatial/paint";
import type { BodyFramePlan, LitBodyInput } from "../bodies/draw";
import type { DiscRecord } from "../bodies/discShading";
import { discSurfaceLaws } from "../bodies/discSurface";
import { rotationColumns } from "../bodies/smoothMesh";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import type { RenderStyle, ViewId } from "../camera/state";
import { relativeToCamera } from "../coords/relative";
import { type Rotation3, rotateToBodyFixed } from "../coords/rotation";
import type { DrawItem, MaterialHandle, TextureHandle, ViewSize } from "../engine/types";
import type { FrameTerrain } from "../photoreal/renderer";
import { sceneOrigins, type ViewBody, type ViewScene } from "../scene/model";
import { angularDiameterPx, bodyRegime, graticuleStepDeg } from "../wireframe/bodies";
import { sphereInFrustum } from "../wireframe/cull";
import { type DrawCamera, STROKE_PX } from "../wireframe/drawList";
import { linearColour, type SurveyedDatumFrame, type WireframeTerrain } from "../wireframe/submit";
import type { TerrainAnnunciation } from "./annunciation";
import { datumGroundUnder } from "./grounded";
import type { BodyFigure } from "./planet";
import type { ResidentHeightsView } from "./residentHeights";
import type { TerrainSurvey } from "./session";
import type { SurveyEdgeView } from "./surveyEdge";
import type {
  TerrainFrameLighting,
  TerrainView,
  TerrainViewFrame,
  TerrainViewFrameInput,
} from "./terrainPass";
import { contourInterval, heightAboveGroundM } from "./terrainWire";
import { poseInterval, type PoseIntervals } from "./poseInterval";

/** A body's lights for its terrain's draw, from its disc record ({@link terrainLightingOf}). */
export interface TerrainLightingFrame {
  /**
   * What lights the terrain: the record's stars, occluders and planetshine sources along the
   * body-fixed axes, its uniform law (`elsewhere`), the frame's pre-exposure, and the record's
   * atmosphere tables where its body's air is drawn (R08.T10.b), else none: the tables of no air.
   */
  readonly lighting: TerrainFrameLighting;
  /** The brightest star's unit direction from the body's centre, body-fixed; +z with no star. */
  readonly sunDirectionBodyFixed: Vec3;
  /**
   * The brightest star's illuminance face-on at the body per Rec. 709 channel, lx; zeros with no
   * star. R05's spike shading reads it and the direction; the lit material reads `lighting`.
   */
  readonly sunIlluminanceLx: readonly [number, number, number];
}

/** No star: the direction the spike's shading takes, unlit. */
const NO_SUN = vec3(0, 0, 1);

/**
 * A body's terrain lights from its disc record (plan R10, R10.T16.c; decision D5), pure: the
 * record's lights, occluders and planetshine sources rotated from the galactic axes into the
 * body-fixed ones by the inverse of `rotation` (Rᵀ, `rotateToBodyFixed`), each light's and source's
 * distance, radius, illuminance and annuli and each occluder's radius as they stand; its uniform
 * law, `elsewhere` (the record's surface's first law, `discSurfaceLaws`: its uniform law, or its
 * class map's unsurveyed ground's), whose A the terrain scales for the figure as the disc does;
 * the frame's pre-exposure; and the record's atmosphere tables, where it has them (R08.T10.b). The sun is the brightest light's, the record's first (`lightsAt` puts
 * the brightest first).
 *
 * @param rotation - The rotation the plan drew the body's figure with, body-fixed to galactic.
 * @param exposureScale - The frame's pre-exposure (R02's `exposureScale` of its EV100).
 */
export function terrainLightingOf(
  record: DiscRecord,
  rotation: Rotation3,
  exposureScale: number,
): TerrainLightingFrame {
  const lights = record.lights.map((light) => ({
    ...light,
    direction: rotateToBodyFixed(rotation, light.direction),
  }));
  const occluders = record.occluders.map((occluder) => ({
    ...occluder,
    centre: rotateToBodyFixed(rotation, occluder.centre),
  }));
  const secondaries = record.secondaries.map((source) => ({
    ...source,
    direction: rotateToBodyFixed(rotation, source.direction),
  }));
  const [elsewhere] = discSurfaceLaws(record.surface);
  if (elsewhere === undefined) {
    throw new Error(`disc ${record.body} shades with no law`);
  }
  const brightest = lights[0];
  return {
    // The record's air, where the frame draws its atmosphere (R08.T10.b).
    lighting: {
      lights,
      occluders,
      secondaries,
      elsewhere,
      exposureScale,
      ...(record.atmosphere === undefined ? {} : { atmosphere: record.atmosphere }),
    },
    sunDirectionBodyFixed: brightest?.direction ?? NO_SUN,
    sunIlluminanceLx: brightest === undefined ? [0, 0, 0] : [...brightest.illuminance],
  };
}

/**
 * The view of a body's terrain from the plan's body (plan R10, R10.T16.c): `rotation` the body's
 * `LitBodyInput.rotation`, the one the plan drew its figure with; `cameraM` its `centreM` negated,
 * the camera from the body's centre along the galactic axes, the body frame's; the camera's
 * orientation and field of view; and the presented size, at which its pass selects. `null` for a
 * body whose rotation is not known, which is never handed over.
 *
 * @param presentedPx - The view's presented size, device px (its canvas's, R05 Design note 23).
 */
export function terrainViewOf(
  body: LitBodyInput,
  camera: ProjectionCamera,
  presentedPx: ViewSize,
): TerrainView | null {
  const rotation = body.rotation;
  if (rotation === undefined) {
    return null;
  }
  return {
    rotation,
    cameraM: scale(body.centreM, -1),
    orientation: camera.orientation,
    fovXRad: camera.fovXRad,
    viewport: presentedPx,
  };
}

/**
 * Which of a view-frame's handed-over bodies takes the view's shadow cascades (plan R10,
 * R10.T9.b; decision-r10-t9b-design D3): the one over whose datum the camera stands lowest, the
 * height along the spheroid's normal (`datumGroundUnder`), ties going to the first in their
 * order; −1 for none.
 *
 * @param bodies - Each body's figure and its terrain's view, in the plan's order:
 *   {@link LiveFrameTerrain.draws} gives those a pass draws, so that a body with no pass takes
 *   none another could take.
 */
export function cascadeBodyIndex(
  bodies: ReadonlyArray<{
    readonly figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">;
    readonly view: Pick<TerrainView, "rotation" | "cameraM">;
  }>,
): number {
  let lowest = -1;
  let lowestM = Number.POSITIVE_INFINITY;
  bodies.forEach(({ figure, view }, index) => {
    const heightM = datumGroundUnder(figure)(
      rotateToBodyFixed(view.rotation, view.cameraM),
    ).heightM;
    // Strictly lower, so that a tie keeps the earlier body.
    if (lowest < 0 || heightM < lowestM) {
      lowest = index;
      lowestM = heightM;
    }
  });
  return lowest;
}

/** The more pressing of two terrain annunciations: `STREAMING`, then `DETAIL LIMITED`. */
export function morePressingAnnunciation(
  a: TerrainAnnunciation | null,
  b: TerrainAnnunciation | null,
): TerrainAnnunciation | null {
  if (a === "TERRAIN: STREAMING" || b === "TERRAIN: STREAMING") {
    return "TERRAIN: STREAMING";
  }
  return a ?? b;
}

/** What a view's frame terrain reads of a body's pass (`TerrainPass`'s). */
export interface FrameTerrainPass {
  /** Whether the view's frame may give the body's mask (`TerrainPass.maskReadyFor`). */
  maskReadyFor(view: string): boolean;
  /** The terrain as depth alone, for the symbology's pass (`TerrainPass.symbologyDepthMaterial`). */
  readonly symbologyDepthMaterial: MaterialHandle | null;
  /** The resident bakes' heights (`TerrainPass.heights`). */
  readonly heights: ResidentHeightsView;
  /** The views its last step took (`TerrainPass.viewIds`). */
  readonly viewIds: ReadonlySet<string>;
  /**
   * Whether the pass is faulted (`TerrainPass.faulted`): it draws nothing more, whatever its last
   * step's views (R10.T16.b's lost workers, R10.T16.e's computation fault).
   */
  readonly faulted: boolean;
}

/** What a view's frame terrain reads of its source: `TerrainSession`'s, structurally. */
export interface FrameTerrainSource {
  /** View `view`'s draw of `body`'s terrain this frame (`TerrainSession.drawTerrain`). */
  drawTerrain(view: ViewId, body: BodyIdHex, input: TerrainViewFrameInput): TerrainViewFrame | null;
  /** `body`'s GPU coverage mask, or `null` (`TerrainSession.coverageMask`). */
  coverageMask(body: BodyIdHex): TextureHandle | null;
  /** `body`'s pass, or `null` (`TerrainSession.pass`). */
  pass(body: BodyIdHex): FrameTerrainPass | null;
}

/** What a wireframe view's frame terrain reads of its source (R10.T11): `TerrainSession`'s. */
export interface WireTerrainSource {
  /** `body`'s wireframe pass, or `null` (`TerrainSession.wirePass`). */
  wirePass(body: BodyIdHex): FrameTerrainPass | null;
  /** View `view`'s wireframe draw of `body`'s terrain this frame (`TerrainSession.drawWireTerrain`). */
  drawWireTerrain(
    view: ViewId,
    body: BodyIdHex,
    input: TerrainViewFrameInput,
  ): TerrainViewFrame | null;
  /** `body`'s GPU coverage mask, or `null` (`TerrainSession.coverageMask`). */
  coverageMask(body: BodyIdHex): TextureHandle | null;
  /** A wanted body's survey: its coverage and header (`TerrainSession.survey`). */
  survey(body: BodyIdHex): Pick<TerrainSurvey, "coverage" | "header"> | null;
}

/** What a view's survey edges and its wireframe's terrain read of its source beyond its frame terrain's. */
export interface ViewTerrainSource extends FrameTerrainSource, WireTerrainSource {
  /** The bodies whose fields are wanted (`TerrainSession.wanted`). */
  readonly wanted: ReadonlySet<BodyIdHex>;
}

/**
 * What a view's drawer reads of its terrain session: its survey edges' source, and the fault that a
 * lasting failure of them raises (plan R10, R10.T16.e).
 */
export interface ViewTerrainSession extends ViewTerrainSource {
  /**
   * Faults the session's terrain for a computation that keeps failing
   * (`TerrainSession.failComputation`; decision-r10-t16-leans §8).
   */
  failComputation(): void;
}

/** A view's terrain for one frame, as its loop gives its drawer (`ViewFrameInputs.terrain`). */
export interface ViewTerrainInput {
  /** The stage's terrain session. */
  readonly source: ViewTerrainSession;
  /** The view, as the session knows it: `PRIMARY`'s or an instrument's ID. */
  readonly view: ViewId;
  /**
   * The frame's monotonic time, ms, for the annunciation's debounce and for how long its survey
   * edges have failed (R10.T16.e).
   */
  readonly nowMs: number;
  /**
   * The stage's holder of the contour interval each pose states (plan R10, R10.T12.b), through
   * which the views at one point state one; absent, the view's hysteresis is its own.
   */
  readonly intervals?: PoseIntervals | undefined;
}

/** What a view's photorealistic frame's terrain is made with, beyond its source. */
export interface LiveFrameTerrainInput {
  readonly view: ViewId;
  /** The frame's camera: its orientation and horizontal field of view. */
  readonly camera: ProjectionCamera;
  /** The size the view is presented at, device px: its canvas's. */
  readonly presentedPx: ViewSize;
  /** The frame's pre-exposure. */
  readonly exposureScale: number;
  /**
   * The symbology's slope term, device px (the overlay's draw list's `occluderSlopePx`), by which
   * the terrain's depth there is pushed away, as the occluders' is.
   */
  readonly occluderSlopePx: number;
  /** The frame's monotonic time, ms. */
  readonly nowMs: number;
}

/**
 * One view's photorealistic frame's terrain (plan R10, R10.T16.c): the session's draws of each
 * body the frame's plan hands over, for this view, and its coverage masks (see the module's
 * remarks). Made for one frame.
 */
export class LiveFrameTerrain implements FrameTerrain {
  readonly #source: FrameTerrainSource;
  readonly #input: LiveFrameTerrainInput;
  #annunciation: TerrainAnnunciation | null = null;
  #depth: DrawItem[] = [];

  constructor(source: FrameTerrainSource, input: LiveFrameTerrainInput) {
    this.#source = source;
    this.#input = input;
  }

  /**
   * The terrain's annunciation in this view, of every body its plan hands over: the more pressing
   * of theirs; `null` before {@link LiveFrameTerrain.draws}, or with none to say.
   */
  get annunciation(): TerrainAnnunciation | null {
    return this.#annunciation;
  }

  /**
   * `body`'s coverage mask, only while this view's draw set of the body leaves nothing missing and
   * its draw is ready wherever it sees a patch (decision D7; `TerrainPass.maskReadyFor`); `null`
   * otherwise, and for a body with no pass, when its figure is drawn whole.
   */
  surveyCoverage(body: BodyIdHex): TextureHandle | null {
    const pass = this.#source.pass(body);
    return pass !== null && pass.maskReadyFor(this.#input.view)
      ? this.#source.coverageMask(body)
      : null;
  }

  /**
   * The session's draw of each body `plan` hands over, for this view: its `TerrainView` from the
   * plan's body ({@link terrainViewOf}) and its lights from its record ({@link terrainLightingOf});
   * none for a body without a rotation or a pass, or with nothing drawn. Notes the frame's
   * annunciation and its terrain's depth for the symbology's pass.
   *
   * @remarks
   * The body over whose datum the camera stands lowest, of those the source has a pass for, takes
   * the view's shadow cascades ({@link cascadeBodyIndex}; R10.T9.b, D3). On the high setting its
   * pass renders their four caster passes into the session's one set before this returns, each a
   * submission of its own: call it immediately before the bodies pass its draws are drawn in,
   * whose resolve times them and whose terrain reads the layers, with no other view's frame
   * between.
   *
   * @param bodies - The lit bodies the plan was made from, its `LitBodyInput`s.
   */
  draws(plan: BodyFramePlan, bodies: ReadonlyArray<LitBodyInput>): ReadonlyArray<DrawItem> {
    const { view, camera, presentedPx, exposureScale, occluderSlopePx, nowMs } = this.#input;
    const byId = new Map(bodies.map((body) => [body.id, body]));
    const draws: DrawItem[] = [];
    const depth: DrawItem[] = [];
    let annunciation: TerrainAnnunciation | null = null;
    const handed: Array<{
      readonly record: DiscRecord;
      readonly figure: BodyFigure;
      readonly view: TerrainView;
    }> = [];
    for (const mesh of plan.meshes) {
      if (mesh.role.kind !== "handed-over") {
        continue;
      }
      const record = plan.discs[mesh.index];
      const body = record === undefined ? undefined : byId.get(record.body);
      const terrainView = body === undefined ? null : terrainViewOf(body, camera, presentedPx);
      if (record === undefined || body === undefined || terrainView === null) {
        continue;
      }
      handed.push({ record, figure: body.figure, view: terrainView });
    }
    // Among the bodies a pass draws, so that one with no pass, whose draw is none, takes no
    // cascades another could take.
    const drawable = handed.flatMap((each, index) =>
      this.#source.pass(each.record.body) === null ? [] : [{ ...each, index }],
    );
    const cascades = drawable[cascadeBodyIndex(drawable)]?.index ?? -1;
    for (const [index, { record, view: terrainView }] of handed.entries()) {
      const frame = this.#source.drawTerrain(view, record.body, {
        view: terrainView,
        ...terrainLightingOf(record, terrainView.rotation, exposureScale),
        exposureScale,
        takesCascades: index === cascades,
        nowMs,
      });
      if (frame === null) {
        continue;
      }
      annunciation = morePressingAnnunciation(annunciation, frame.annunciation);
      if (frame.draw === null) {
        continue;
      }
      draws.push(frame.draw);
      const material = this.#source.pass(record.body)?.symbologyDepthMaterial ?? null;
      if (material !== null) {
        depth.push({
          ...frame.draw,
          material,
          uniforms: {
            ...frame.draw.uniforms,
            occluderSlopePx: new Float32Array([occluderSlopePx]),
          },
        });
      }
    }
    this.#annunciation = annunciation;
    this.#depth = depth;
    return draws;
  }

  /**
   * The terrain the last {@link LiveFrameTerrain.draws} drew, as depth alone with the depth
   * material, for the symbology's pass before its lines: each view draw's own resources, records
   * and indirect count, at the overlay's slope term.
   */
  symbologyDepth(): ReadonlyArray<DrawItem> {
    return this.#depth;
  }
}

/** A height function that holds none: the mark stands on the datum. */
const NO_HEIGHT = (): number | null => null;

/**
 * The survey edges a view's draw list marks (plan R10, R10.T16.c; `DrawOptions.surveyEdges`), in
 * both styles: each wanted body of `scene` with a rotation, a held header and a coverage with
 * surveyed ground, whose sphere of its equatorial radius and relief meets the view, its centre
 * from the camera as the draw list places a body (`relativeToCamera` of its frame's origin).
 *
 * @remarks
 * The edge stands on the ground the view draws. Where the view's last step drew a body's terrain,
 * its lit pass in the photorealistic style or its wireframe pass in the wireframe (R10.T11), that
 * is the terrain: there the rim is that pass's resident bakes that hold the edge
 * (`ResidentHeightsView.rimHeightAtM`, the datum where none does), and the drawn ground hides a
 * point from the note's range (`SurveyEdgeView.ground`); in the wireframe the edge then takes the
 * terrain's own depth, which the wireframe's terrain writes. Elsewhere the view draws no terrain of
 * the body (a frame that hands it over to no pass, or a faulted pass, which keeps its last step's
 * views but draws nothing, R10.T16.e), so the rim is the datum, the figure it draws, and the edge
 * is drawn against the occluder spheres alone (decision-r10-t16-review §3; the batch review's
 * S1).
 *
 * @param style - The style the frame is drawn in (`ViewFrameDrawn.drawnStyle`, as the drawer
 *   foresees it before the draw).
 */
export function surveyEdgeViewsOf(
  source: ViewTerrainSource,
  view: ViewId,
  scene: ViewScene,
  camera: DrawCamera,
  viewport: Viewport,
  style: RenderStyle,
): SurveyEdgeView[] {
  const origins = sceneOrigins(scene);
  const projection: ProjectionCamera = {
    orientation: camera.pose.orientation,
    fovXRad: camera.fovXRad,
  };
  const views: SurveyEdgeView[] = [];
  for (const body of scene.bodies) {
    const rotation = body.rotation;
    if (!source.wanted.has(body.id) || rotation === null) {
      continue;
    }
    const survey = source.survey(body.id);
    const coverage = survey?.coverage ?? null;
    const header = survey?.header ?? null;
    if (coverage === null || header === null || !(coverage.surveyedFraction() > 0)) {
      continue;
    }
    const centreM = relativeToCamera(
      { kind: "body", body: body.id, m: vec3(0, 0, 0) },
      camera.pose,
      origins,
    );
    const reachM = header.equatorial_radius_m + Math.max(0, header.realised_relief_m);
    if (!sphereInFrustum(centreM, reachM, projection, viewport)) {
      continue;
    }
    // The pass whose terrain this view draws in its style: the lit one, or the wireframe's.
    const pass = style === "photorealistic" ? source.pass(body.id) : source.wirePass(body.id);
    const heights = pass?.heights ?? null;
    // A faulted pass keeps its last step's views but draws nothing: the view draws the figure.
    const drawsGround = pass !== null && !pass.faulted && pass.viewIds.has(view);
    views.push({
      body: body.id,
      coverage,
      figure: {
        equatorialRadiusM: header.equatorial_radius_m,
        polarRadiusM: header.polar_radius_m,
      },
      rotation,
      centreM,
      heightAtM:
        drawsGround && heights !== null
          ? (direction) => heights.rimHeightAtM(direction)
          : NO_HEIGHT,
      ...(drawsGround && heights !== null
        ? {
            ground: {
              heightAtM: (direction: Vec3) => heights.groundHeightAtM(direction),
              ceilingM: heights.ceilingM,
            },
          }
        : {}),
    });
  }
  return views;
}

/** What a wireframe view's frame terrain is made with, beyond its source (R10.T11). */
export interface WireframeTerrainInput {
  /** The view, as the session knows it. */
  readonly view: ViewId;
  /** The scene the frame draws. */
  readonly scene: ViewScene;
  /** The frame's camera. */
  readonly camera: DrawCamera;
  /** The size the view is presented at, device px: its canvas's, at which its pass selects. */
  readonly viewport: ViewSize;
  /** The device px drawn for each CSS px of a line (`ViewStrokes.strokeScale`). */
  readonly strokeScale: number;
  /** The wireframe's low setting, whose graticules are at 30° only (R02's `lowSetting`). */
  readonly lowSetting: boolean;
  /** The colours the contours and graticule take, as `readTokens` read them. */
  readonly tokens: Pick<ColourTokens, "textMuted" | "text" | "surface0">;
  /** The draw list's slope term, device px (`WireframeDrawList.occluderSlopePx`). */
  readonly occluderSlopePx: number;
  /** The frame's monotonic time, ms, for the annunciation's debounce. */
  readonly nowMs: number;
  /** The contour interval the view stated last, m, its hysteresis's; `null` with none. */
  readonly previousIntervalM: number | null;
  /**
   * The stage's holder of each pose's interval (R10.T12.b): the view takes the interval another
   * view stated this frame at its point, or steps from the one held there; absent, its own.
   */
  readonly intervals?: PoseIntervals | undefined;
}

/** A wireframe view's terrain in one frame (R10.T11): what its renderer draws, and what its label block states. */
export interface WireframeTerrainFrame extends WireframeTerrain {
  /** The more pressing annunciation of the bodies it hands over; `null` with none to say. */
  readonly annunciation: TerrainAnnunciation | null;
  /**
   * The contour interval its label block states, m (`CONTOUR INTERVAL`), while it draws some
   * body's terrain; `null` while it draws none.
   */
  readonly intervalM: number | null;
  /**
   * The interval its hysteresis holds for the next frame: the one this frame's pose sets, or the
   * last where it sets none.
   */
  readonly heldIntervalM: number | null;
}

/** The light the wireframe's terrain takes: none, since it is depth with its lines. */
const NO_LIGHT: readonly [number, number, number] = [0, 0, 0];

/**
 * A wireframe view's terrain in one frame (plan R10, R10.T11; Design note 14), pure but for its
 * source's draws: each body that this view hands over to its wireframe pass (`wirePass`, the view in
 * its last step), drawn from the view's camera with the contours at one interval and the body's
 * graticule at the step and widths R02's draw list draws it with, and, where the view's draw of the
 * body may take the mask (`TerrainViewFrame.maskReady`, the same code-0 rule as the photorealistic
 * style's, decision-r10-t16-review §2), the body's coverage mask, by which its datum gives way.
 *
 * @remarks
 * The interval is one for the view, so that its label block states one: {@link contourInterval} of
 * the camera's height above the ground at its nadir ({@link heightAboveGroundM}, over its pass's
 * resident heights, the datum where the nadir is unsurveyed or not yet drawn) on the body it stands
 * lowest over, from the view's last; and one for a pose (R10.T12.b): through the stage's holder
 * (`poseInterval`), the interval another view stated this frame where the camera stands at its
 * point over that body, or stepped from the one held there. A view drawing no body's terrain
 * leaves the holder.
 */
export function wireframeTerrainOf(
  source: WireTerrainSource,
  input: WireframeTerrainInput,
): WireframeTerrainFrame {
  const { view, scene, camera, viewport, strokeScale } = input;
  const origins = sceneOrigins(scene);
  const projection: ProjectionCamera = {
    orientation: camera.pose.orientation,
    fovXRad: camera.fovXRad,
  };
  const drawn: Array<{
    readonly body: ViewBody;
    readonly rotation: Rotation3;
    readonly header: SurfaceHeaderDto;
    readonly centreM: Vec3;
    readonly aglM: number;
  }> = [];
  for (const body of scene.bodies) {
    const rotation = body.rotation;
    const pass = source.wirePass(body.id);
    if (rotation === null || pass === null || !pass.viewIds.has(view)) {
      continue;
    }
    const header = source.survey(body.id)?.header ?? null;
    if (header === null) {
      continue;
    }
    const centreM = relativeToCamera(
      { kind: "body", body: body.id, m: vec3(0, 0, 0) },
      camera.pose,
      origins,
    );
    const figure = {
      equatorialRadiusM: header.equatorial_radius_m,
      polarRadiusM: header.polar_radius_m,
    };
    const heights = pass.heights;
    const aglM = heightAboveGroundM(
      rotateToBodyFixed(rotation, scale(centreM, -1)),
      figure,
      (direction) => heights.groundHeightAtM(direction),
    );
    drawn.push({ body, rotation, header, centreM, aglM });
  }
  const nadir = drawn.reduce<(typeof drawn)[number] | null>(
    (lowest, each) => (lowest === null || each.aglM < lowest.aglM ? each : lowest),
    null,
  );
  if (nadir === null) {
    input.intervals?.forget(view);
    return {
      draws: [],
      surveyed: new Map(),
      annunciation: null,
      intervalM: null,
      heldIntervalM: input.previousIntervalM,
    };
  }
  const step = (previousM: number | null): number => contourInterval(nadir.aglM, previousM);
  const intervalM =
    input.intervals === undefined
      ? step(input.previousIntervalM)
      : poseInterval(
          input.intervals,
          view,
          {
            body: nadir.body.id,
            cameraBodyFixedM: rotateToBodyFixed(nadir.rotation, scale(nadir.centreM, -1)),
          },
          input.nowMs,
          input.previousIntervalM,
          step,
        );
  const colours = {
    muted: linearColour(input.tokens.textMuted),
    text: linearColour(input.tokens.text),
    casing: linearColour(input.tokens.surface0),
  };
  const draws: DrawItem[] = [];
  const surveyed = new Map<string, SurveyedDatumFrame>();
  let annunciation: TerrainAnnunciation | null = null;
  for (const { body, rotation, header, centreM } of drawn) {
    // The graticule R02's draw list draws on this body, at its step and widths, or none.
    const diameterPx = angularDiameterPx(centreM, body.radiusM, projection, viewport);
    const graticule =
      bodyRegime(diameterPx, strokeScale) === "graticule"
        ? {
            stepRad: (graticuleStepDeg(diameterPx, strokeScale, input.lowSetting) * Math.PI) / 180,
            thinPx: STROKE_PX.thin * strokeScale,
            heavyPx: STROKE_PX.heavy * strokeScale,
            casingPx: CASING_PX * strokeScale,
          }
        : null;
    const frame = source.drawWireTerrain(view, body.id, {
      view: {
        rotation,
        cameraM: scale(centreM, -1),
        orientation: camera.pose.orientation,
        fovXRad: camera.fovXRad,
        viewport,
      },
      sunDirectionBodyFixed: NO_SUN,
      sunIlluminanceLx: NO_LIGHT,
      exposureScale: 1,
      wire: {
        intervalM,
        // A world with a sea has its main liquid's entry; on one with none, the header's sea level
        // is its lowest cell's, no coastline.
        seaLevelM: header.main_liquid === null ? null : header.sea_level_m,
        strokeScale,
        graticule,
        colours,
        occluderSlopePx: input.occluderSlopePx,
      },
      nowMs: input.nowMs,
    });
    if (frame === null) {
      continue;
    }
    annunciation = morePressingAnnunciation(annunciation, frame.annunciation);
    if (frame.draw !== null) {
      draws.push(frame.draw);
    }
    const mask = frame.maskReady ? source.coverageMask(body.id) : null;
    if (mask !== null) {
      surveyed.set(body.id, {
        mask,
        axes: rotationColumns(rotation),
        stretch: header.equatorial_radius_m / header.polar_radius_m,
      });
    }
  }
  return {
    draws,
    surveyed,
    annunciation,
    intervalM: draws.length > 0 ? intervalM : null,
    heldIntervalM: intervalM,
  };
}
