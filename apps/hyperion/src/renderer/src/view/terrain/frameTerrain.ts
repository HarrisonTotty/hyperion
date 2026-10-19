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
 * - **The air** (D9). The terrain takes the tables of no air: `TerrainFrameLighting.atmosphere` is
 *   absent, since R08.T10.b, which draws a body's atmosphere in the view, is not built. The binding
 *   point is {@link terrainLightingOf}: whichever of R10.T16.c and R08.T10.b lands second gives it
 *   the record's `atmosphere`, R08.T10.a's medium's tables, and tests the composite over the
 *   terrain.
 * - **The label block.** Each view's frame keeps the annunciation of the bodies it hands over
 *   ({@link LiveFrameTerrain.annunciation}: `TERRAIN: STREAMING` before `TERRAIN: DETAIL LIMITED`),
 *   which its label block states through `labelStatements(run, terrain)`; the fault goes through
 *   the label block's fault lines (decision-r10-t16-contracts §7).
 * - **The survey edge**, in both styles ({@link surveyEdgeViewsOf}): each wanted body's coverage in
 *   view, its rim at max(drawn height, 0) from its pass's resident bakes that hold the edge, the
 *   datum where none does (`ResidentHeights`). Over the image the terrain's depth is drawn into the
 *   symbology's pass before its lines ({@link LiveFrameTerrain.symbologyDepth}), so that nearer
 *   ground hides the edge as it does in the image, and the note's range counts only the points
 *   that ground leaves in sight (`SurveyEdgeView.ground`); in the wireframe, which draws no terrain
 *   until R10.T11 (D10), the edge is drawn against the occluder spheres alone, and every drawn point
 *   counts.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import { scale, type Vec3, vec3 } from "../../geometry/vec3";
import type { BodyFramePlan, LitBodyInput } from "../bodies/draw";
import type { DiscRecord } from "../bodies/discShading";
import { discSurfaceLaws } from "../bodies/discSurface";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import type { RenderStyle, ViewId } from "../camera/state";
import { relativeToCamera } from "../coords/relative";
import { type Rotation3, rotateToBodyFixed } from "../coords/rotation";
import type { DrawItem, MaterialHandle, TextureHandle, ViewSize } from "../engine/types";
import type { FrameTerrain } from "../photoreal/renderer";
import { sceneOrigins, type ViewScene } from "../scene/model";
import { sphereInFrustum } from "../wireframe/cull";
import type { DrawCamera } from "../wireframe/drawList";
import type { TerrainAnnunciation } from "./annunciation";
import type { ResidentHeightsView } from "./residentHeights";
import type { TerrainSurvey } from "./session";
import type { SurveyEdgeView } from "./surveyEdge";
import type {
  TerrainFrameLighting,
  TerrainView,
  TerrainViewFrame,
  TerrainViewFrameInput,
} from "./terrainPass";

/** A body's lights for its terrain's draw, from its disc record ({@link terrainLightingOf}). */
export interface TerrainLightingFrame {
  /**
   * What lights the terrain: the record's stars, occluders and planetshine sources along the
   * body-fixed axes, its uniform law (`elsewhere`) and the frame's pre-exposure; no atmosphere's
   * tables, so the tables of no air (R08.T10.b's binding point).
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
 * class map's unsurveyed ground's), whose A the terrain scales for the figure as the disc does; and
 * the frame's pre-exposure. The sun is the brightest light's, the record's first (`lightsAt` puts
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
    // R08.T10.b's binding point: the record's `atmosphere` joins here, with its composite's test.
    lighting: { lights, occluders, secondaries, elsewhere, exposureScale },
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

/** What a view's survey edges read of its source beyond its frame terrain's. */
export interface ViewTerrainSource extends FrameTerrainSource {
  /** The bodies whose fields are wanted (`TerrainSession.wanted`). */
  readonly wanted: ReadonlySet<BodyIdHex>;
  /** A wanted body's survey: its coverage and header (`TerrainSession.survey`). */
  survey(body: BodyIdHex): Pick<TerrainSurvey, "coverage" | "header"> | null;
}

/** A view's terrain for one frame, as its loop gives its drawer (`ViewFrameInputs.terrain`). */
export interface ViewTerrainInput {
  /** The stage's terrain session. */
  readonly source: ViewTerrainSource;
  /** The view, as the session knows it: `PRIMARY`'s or an instrument's ID. */
  readonly view: ViewId;
  /** The frame's monotonic time, ms, for the annunciation's debounce. */
  readonly nowMs: number;
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
   * @param bodies - The lit bodies the plan was made from, its `LitBodyInput`s.
   */
  draws(plan: BodyFramePlan, bodies: ReadonlyArray<LitBodyInput>): ReadonlyArray<DrawItem> {
    const { view, camera, presentedPx, exposureScale, occluderSlopePx, nowMs } = this.#input;
    const byId = new Map(bodies.map((body) => [body.id, body]));
    const draws: DrawItem[] = [];
    const depth: DrawItem[] = [];
    let annunciation: TerrainAnnunciation | null = null;
    for (const mesh of plan.meshes) {
      if (mesh.role.kind !== "handed-over") {
        continue;
      }
      const record = plan.discs[mesh.index];
      const body = record === undefined ? undefined : byId.get(record.body);
      const terrainView = body === undefined ? null : terrainViewOf(body, camera, presentedPx);
      if (record === undefined || terrainView === null) {
        continue;
      }
      const frame = this.#source.drawTerrain(view, record.body, {
        view: terrainView,
        ...terrainLightingOf(record, terrainView.rotation, exposureScale),
        exposureScale,
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
 * surveyed ground, whose sphere of its equatorial radius and relief meets the view, its rim from
 * its pass's resident bakes that hold the edge (`ResidentHeightsView.rimHeightAtM`, the datum
 * where none does or it has no pass), its centre from the camera as the draw list places a body
 * (`relativeToCamera` of its frame's origin).
 *
 * @remarks
 * Only in the photorealistic style, and only for a body whose pass this view's last step drew,
 * does the drawn ground hide a point from the note's range (`SurveyEdgeView.ground`): the
 * wireframe draws no terrain until R10.T11, and its edge is drawn against the occluder spheres
 * alone.
 *
 * @param style - The style the view draws (`ViewBudget.style`).
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
    const pass = source.pass(body.id);
    const heights = pass?.heights ?? null;
    const drawsGround = style === "photorealistic" && pass !== null && pass.viewIds.has(view);
    views.push({
      body: body.id,
      coverage,
      figure: {
        equatorialRadiusM: header.equatorial_radius_m,
        polarRadiusM: header.polar_radius_m,
      },
      rotation,
      centreM,
      heightAtM: heights === null ? NO_HEIGHT : (direction) => heights.rimHeightAtM(direction),
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
