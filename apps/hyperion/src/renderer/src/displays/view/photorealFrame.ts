/**
 * What a `VIEW` stage's photorealistic frame draws, assembled from its run, its sky and R06's
 * layers (plan R07, T8.a; Design note 8). Pure but for the layers' own frame state: the drawing
 * loop calls it each photorealistic frame.
 *
 * @remarks
 * The lights are the scene's host discs, a kept scene's own `hostDiscs` or the held sky's `hosts`,
 * placed at their stars' drawn centres in the frame for the painter's order (decision-r07-t8a,
 * item 1); the host discs are drawn by R06's `HostDiscLayer` where the scene draws them (its
 * apparent placement), the lit bodies are the scene's planets, dwarf planets and moons, with their
 * sections' figure and photometry (R07.T2.b) or, without them, spheres of their radius with the
 * provisional photometry (R07.T2.a), each lit by its own lighting frame, its
 * stars and the other bodies retarded to the light that reaches it (`lightingFramesOf`, T10.a),
 * and the stars are the wireframe draw list's sprites, the sky's or the interim field's, with the
 * host discs under three pixels.
 */
import type { BodyIdHex, HostDiscDto } from "@hyperion/protocol";

import { rotateToBody } from "../../view/coords/rotation";
import { relativeToCamera } from "../../view/coords/relative";
import type { Vec3 } from "../../geometry/vec3";
import {
  type AppearanceLabel,
  PROVISIONAL_LABELS,
  PROVISIONAL_PHOTOMETRY,
} from "../../view/appearance/fromWire";
import type { LitBodyInput } from "../../view/bodies/draw";
import type { LitRegime } from "../../view/bodies/regime";
import { type ProjectionCamera, project, type Viewport } from "../../view/camera/projection";
import type { CameraPose } from "../../view/camera/pose";
import type { DrawItem, FrameSubmission } from "../../view/engine/types";
import { hostLights, placeLights, sceneHostDiscs } from "../../view/lighting/hostLights";
import { isLitBody, type LitBodyLighting, lightingFramesOf } from "../../view/lighting/retarded";
import type { PhotorealFrame } from "../../view/photoreal/renderer";
import type { MeterMode } from "../../view/post/meter";
import type { QualitySetting } from "../../view/quality/qualitySetting";
import { sceneOrigins, type ViewScene } from "../../view/scene/model";
import type { BandLayer } from "../../view/sky/band";
import { type HostDiscLayer, hostPlacements } from "../../view/sky/disc";
import {
  exposedSpriteRecord,
  type SpriteRecord,
  spriteRecord,
  type WireframeDrawList,
} from "../../view/wireframe/drawList";
import type { DrawnSky } from "./useViewSky";
import type { ViewRun } from "./viewRun";

/** What one photorealistic frame is made from. */
export interface PhotorealInputs {
  readonly run: ViewRun;
  /** The frame's pose (`runPose`). */
  readonly pose: CameraPose;
  readonly viewport: Viewport;
  readonly setting: QualitySetting;
  /** The exposure scale (`exposureScale`), the target's pre-exposure. */
  readonly exposureScale: number;
  /** The frame's wireframe draw list, whose sprites are the stars. */
  readonly list: WireframeDrawList;
  /** The view's sky, or `null` while the interim field stands in. */
  readonly sky: DrawnSky | null;
  /** R06's band layer, uploaded with the sky's band, or `null` with no sky. */
  readonly band: BandLayer | null;
  readonly discs: HostDiscLayer;
  /** R06's baked cube's HDR draw, or `null` before it is baked. */
  readonly cube: DrawItem | null;
  readonly previousRegimes: ReadonlyMap<BodyIdHex, LitRegime>;
  /** The symbology's canvas pass, or `null`. */
  readonly overlay: FrameSubmission | null;
  /** The operator's meter where the view is the exposure's source, or `null` (no histogram). */
  readonly meter: MeterMode | null;
}

/** A wireframe sprite as a record at infinity. */
function starRecord(sprite: WireframeDrawList["sprites"][number]): SpriteRecord {
  return exposedSpriteRecord({ xPx: sprite.xPx, yPx: sprite.yPx, depth: 0 }, sprite.exposedRgb);
}

/**
 * The appearance labels of the bodies a scene's photorealistic frame lights, each once, in the
 * scene's order: a body's own (none with its photometric section, R07.T2.b), or, with no
 * appearance, the provisional photometry's (Design note 5).
 */
export function litLabelsOf(scene: ViewScene): ReadonlyArray<AppearanceLabel> {
  const labels = scene.bodies
    .filter(isLitBody)
    .flatMap((body) => body.appearance?.labels ?? PROVISIONAL_LABELS);
  return [...new Set(labels)];
}

/** A lit body of the scene as the frame lights it, with its rotation where it has one. */
function litBodyOf({ body, centreM, frame }: LitBodyLighting): LitBodyInput {
  const id = body.id;
  const figure = body.appearance?.figure ?? {
    equatorialRadiusM: body.radiusM,
    polarRadiusM: body.radiusM,
    pole: body.rotation === null ? null : rotateToBody(body.rotation, { x: 0, y: 0, z: 1 }),
  };
  const photometry = body.appearance?.photometry ?? PROVISIONAL_PHOTOMETRY;
  return body.rotation === null
    ? { id, centreM, figure, photometry, lighting: frame }
    : { id, centreM, figure, photometry, rotation: body.rotation, lighting: frame };
}

/**
 * The scene's lit bodies from the camera, each lit by its own lighting frame (`lightingFramesOf`):
 * a server body with its sections' figure and photometry (R07.T2.b), and its rotation, which
 * orients a class map; a body with no appearance, or none with a figure, a sphere of its radius
 * about its rotation's pole with the provisional photometry.
 *
 * @param discs - The scene's host discs (`sceneHostDiscs`).
 */
export function litBodiesOf(
  scene: ViewScene,
  pose: CameraPose,
  discs: ReadonlyArray<HostDiscDto>,
): LitBodyInput[] {
  return lightingFramesOf(scene, pose, discs).map(litBodyOf);
}

/** The photorealistic frame of a stage's run. */
export function photorealFrame(inputs: PhotorealInputs): PhotorealFrame {
  const { run, pose, viewport, sky, exposureScale } = inputs;
  const { scene } = run;
  const origins = sceneOrigins(scene);
  const camera: ProjectionCamera = {
    orientation: pose.orientation,
    fovXRad: (run.camera.fovDeg * Math.PI) / 180,
  };
  const discs = sceneHostDiscs(scene, sky?.model.response.hosts ?? null);
  const bodies = new Map(scene.bodies.map((body) => [body.id, body]));
  const centreOf = (id: BodyIdHex): Vec3 | null => {
    const body = bodies.get(id);
    return body === undefined
      ? null
      : relativeToCamera({ kind: "system", system: scene.system, m: body.centreM }, pose, origins);
  };
  const lights = placeLights(hostLights(scene, discs), centreOf);
  const discFrame = inputs.discs.frame(
    hostPlacements(scene, discs, pose),
    camera,
    viewport,
    exposureScale,
  );
  const hostDraws = new Map<number, DrawItem[]>();
  for (const { star, item } of discFrame.draws) {
    hostDraws.set(star, [...(hostDraws.get(star) ?? []), item]);
  }
  const hostSprites = discFrame.sprites.flatMap((star): SpriteRecord[] => {
    const p = project(star.direction, camera, viewport);
    return p.inFront
      ? [
          spriteRecord(
            { xPx: p.xPx, yPx: p.yPx, depth: 0 },
            star.illuminanceRgbLx,
            exposureScale,
            star.direction,
            camera,
            viewport,
          ),
        ]
      : [];
  });
  const band = inputs.band?.draw(exposureScale) ?? null;
  return {
    camera,
    viewport,
    role: run.camera.role,
    setting: inputs.setting,
    exposureScale,
    sky: [...(band === null ? [] : [band]), ...(inputs.cube === null ? [] : [inputs.cube])],
    starSprites: [...inputs.list.sprites.map(starRecord), ...hostSprites],
    hostDraws,
    glareSources: inputs.discs.glareSources(camera, viewport, run.camera.role),
    lights,
    bodies: litBodiesOf(scene, pose, discs),
    // Nothing in a view writes depth yet; R10's terrain and lit craft add their footprints.
    depthWriters: [],
    previousRegimes: inputs.previousRegimes,
    overlay: inputs.overlay,
    meter: inputs.meter,
  };
}
