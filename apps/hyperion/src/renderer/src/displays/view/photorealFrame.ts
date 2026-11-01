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

import { atmosphereToBodyFixed, bodyAtmosphereCamera } from "../../view/atmosphere/bodyCamera";
import { tablesFigureOf } from "../../view/atmosphere/hillaire";
import type { AtmosphereTables } from "../../view/atmosphere/tables";
import type { SurfaceTemperatureField } from "../../view/atmosphere/thermal";

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
import type { DrawItem } from "../../view/engine/types";
import {
  hostLights,
  lightsAt,
  MAX_BODY_LIGHTS,
  type PlacedLight,
  placeLights,
  sceneHostDiscs,
} from "../../view/lighting/hostLights";
import { isLitBody, type LitBodyLighting, lightingFramesOf } from "../../view/lighting/retarded";
import type {
  FrameAtmosphere,
  FrameTerrain,
  PhotorealFrame,
  PhotorealOverlay,
} from "../../view/photoreal/renderer";
import type { MeterMode } from "../../view/post/meter";
import { type QualitySetting, SETTINGS } from "../../view/quality/qualitySetting";
import { sceneOrigins, type ViewScene } from "../../view/scene/model";
import type { BandLayer } from "../../view/sky/band";
import type { ClassMapSource } from "../../view/terrain/classMap";
import type { HandoverReliefSource } from "../../view/terrain/handover";
import { type GlareAirOf, type HostDiscLayer, hostPlacements, NO_AIR } from "../../view/sky/disc";
import { type GlareAir, glareAirTransmittance } from "../../view/post/glareAir";
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
  /** The frame's own viewport, device px: the scene target's, at its internal scale. */
  readonly viewport: Viewport;
  /**
   * The size the view is presented at, device px, its canvas's (`PhotorealFrame.presentedPx`),
   * whose centre pixel the lit bodies' hand-over to terrain measures (plan R10, R10.T10.g).
   */
  readonly presentedPx: Viewport;
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
  /**
   * The bodies drawn by their disc tables on the previous frame (plan R08, R08.T16.b), for the 10⁹ m
   * boundary's hysteresis; absent, none.
   */
  readonly previousTabled?: ReadonlySet<BodyIdHex> | undefined;
  /**
   * The symbology's canvas pass, which the renderer builds once it has the frame's plan (R10.T16.f),
   * or `null`.
   */
  readonly overlay: PhotorealOverlay | null;
  /** The operator's meter where the view is the exposure's source, or `null` (no histogram). */
  readonly meter: MeterMode | null;
  /** Whether the frame is presented (`PhotorealFrame.present`): presented where absent. */
  readonly present?: boolean | undefined;
  /**
   * The class maps the lit bodies' discs shade with (plan R10, R10.T10.d; `ClassMapStore`), the
   * terrain session's (R10.T16.a), or absent, every disc then its photometry's uniform law.
   */
  readonly classMaps?: ClassMapSource | undefined;
  /**
   * The lit bodies' reliefs, by which a body hands over to terrain (plan R10, R10.T10.e;
   * `storeReliefSource` over the terrain session's `CoarseFieldStore`, R10.T16.a), or absent, when
   * none hands over.
   */
  readonly reliefs?: HandoverReliefSource | undefined;
  /**
   * The terrain of the bodies handed over (plan R10, R10.T10.e), with their surveys' coverage
   * masks: the view's own from the terrain session (R10.T16.c, `LiveFrameTerrain`), or absent,
   * when a body handed over is drawn as a mesh body is, its figure whole.
   */
  readonly terrain?: FrameTerrain | undefined;
  /**
   * A lit body's atmosphere tables as the view's atmosphere cache holds them this frame (plan R08,
   * R08.T10.b; `AtmosphereCache.entryOf(id).tables`), `null` for one it draws no air for; absent,
   * the frame draws no atmosphere.
   */
  readonly atmospheres?: ((body: BodyIdHex) => AtmosphereTables | null) | undefined;
  /**
   * A lit body's ground temperature field as its record gives it (plan R08, R08.T18.a;
   * `AtmosphereCache.groundFieldOf`), `null` for one with a medium or no record held; absent, no
   * ground is drawn emitting.
   */
  readonly groundFields?: GroundFieldSource | undefined;
  /**
   * Takes a throw from the drawing of a body's atmosphere (`PhotorealFrame.onAtmosphereFailed`):
   * the view's handle on the cache (`AtmosphereView.reportDrawFailure`).
   */
  readonly atmosphereFailed?:
    ((body: BodyIdHex, error: unknown, viewName: string) => void) | undefined;
}

/**
 * Where a frame's lit bodies find their ground's temperature field (plan R08, R08.T18.a): the
 * atmosphere cache's, from the records it holds.
 */
export type GroundFieldSource = (body: BodyIdHex) => SurfaceTemperatureField | null;

/** What the lit bodies take of a view's terrain (plan R10, R10.T10.e). */
export interface LitBodiesTerrain {
  /** Their reliefs, for the hand-over. */
  readonly reliefs?: HandoverReliefSource | undefined;
  /** Their surveys' coverage masks, from the terrain the view draws. */
  readonly terrain?: Pick<FrameTerrain, "surveyCoverage"> | undefined;
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

/**
 * The air between the camera and each host star along its line of sight, through every body's
 * atmosphere the frame draws (fix-view-light, the setting Sun's halo): the product of each one's
 * transmittance (`glareAirTransmittance`) on its tables' figure, read through the star's light slot,
 * its place in that body's brightest-first order of its lights (`lightsAt`, as `skySources` slots
 * them); none where the frame draws no air.
 */
export function frameGlareAir(
  atmospheres: ReadonlyArray<FrameAtmosphere>,
  bodies: ReadonlyArray<LitBodyInput>,
  lights: ReadonlyArray<PlacedLight>,
): GlareAirOf {
  const airs = atmospheres.flatMap((each) => {
    const body = bodies.find((lit) => lit.id === each.id);
    if (body === undefined) {
      return [];
    }
    const figure = tablesFigureOf(each.tables);
    const order = lightsAt(body.centreM, lights, MAX_BODY_LIGHTS).map(
      (light) => light.host.disc.star,
    );
    const air: GlareAir = {
      medium: each.tables.medium,
      equatorialRadiusM: figure.equatorialRadiusM,
      polarRadiusM: figure.polarRadiusM,
      cameraM: each.camera.positionM,
      toBodyFixed: each.toBodyFixed,
    };
    return [{ air, curves: each.tables.curves, order }];
  });
  if (airs.length === 0) {
    return NO_AIR;
  }
  return (star, direction, angularRadiusRad) => {
    const transmitted: [number, number, number] = [1, 1, 1];
    for (const { air, curves, order } of airs) {
      const slot = Math.max(0, order.indexOf(star));
      const [r, g, b] = glareAirTransmittance(air, curves[slot] ?? [], direction, angularRadiusRad);
      transmitted[0] *= r;
      transmitted[1] *= g;
      transmitted[2] *= b;
    }
    return transmitted;
  };
}

/**
 * A lit body of the scene as the frame lights it, with its rotation where it has one, its class
 * map where `classMaps` holds one, and its relief and survey's coverage mask where `terrain` holds
 * them and its rotation is known: a body whose rotation is not known is never handed over, its
 * figure and disc standing (R10.T16.c; decision-r10-live-path, D5). Its ground's temperature field
 * where `groundFields` holds one (R08.T18.a).
 */
function litBodyOf(
  { body, centreM, frame }: LitBodyLighting,
  classMaps: ClassMapSource | undefined,
  terrain: LitBodiesTerrain,
  groundFields: GroundFieldSource | undefined,
): LitBodyInput {
  const id = body.id;
  const figure = body.appearance?.figure ?? {
    equatorialRadiusM: body.radiusM,
    polarRadiusM: body.radiusM,
    pole: body.rotation === null ? null : rotateToBody(body.rotation, { x: 0, y: 0, z: 1 }),
  };
  const photometry = body.appearance?.photometry ?? PROVISIONAL_PHOTOMETRY;
  const surface = classMaps?.classMapOf(id, photometry.law) ?? null;
  // Its terrain, mask and class map are drawn in its body-fixed axes, so it hands over only with them.
  const reliefM = body.rotation === null ? null : (terrain.reliefs?.reliefM(id) ?? null);
  const coverage = terrain.terrain?.surveyCoverage(id) ?? null;
  const thermal = groundFields?.(id) ?? null;
  return {
    id,
    centreM,
    figure,
    photometry,
    ...(body.rotation === null ? {} : { rotation: body.rotation }),
    ...(surface === null ? {} : { surface }),
    ...(reliefM === null ? {} : { handoverReliefM: reliefM }),
    // The mask is read in the body's axes, so it is given only with its rotation.
    ...(coverage === null || body.rotation === null
      ? {}
      : { surveyCoverage: { mask: coverage, rotation: body.rotation } }),
    ...(thermal === null ? {} : { thermal }),
    lighting: frame,
  };
}

/**
 * The scene's lit bodies from the camera, each lit by its own lighting frame (`lightingFramesOf`):
 * a server body with its sections' figure and photometry (R07.T2.b), and its rotation, which
 * orients a class map; a body with no appearance, or none with a figure, a sphere of its radius
 * about its rotation's pole with the provisional photometry.
 *
 * @remarks
 * Each body's disc shades with its class map where `classMaps` holds one (plan R10, R10.T10.d), its
 * unsurveyed ground with its photometry's law (`elsewhere`). The rotation, body-fixed to the body
 * frame, whose axes are the galactic ones, is the scene body's at its drawn time (R07.T10.a's
 * `rotationAt`); a body whose rotation is not known (`null`) is passed none, and R07's disc shades
 * its class map with `elsewhere` alone.
 *
 * Each body takes its relief from `terrain.reliefs`, by which it hands over to terrain, and the
 * coverage mask of its survey from the terrain the view draws (plan R10, R10.T10.e), both only
 * with its rotation (R10.T16.c).
 *
 * @param discs - The scene's host discs (`sceneHostDiscs`).
 * @param classMaps - The bodies' class maps; absent, every disc is uniform.
 * @param terrain - The bodies' reliefs and coverage masks; absent, none hands over.
 * @param groundFields - The bodies' ground temperature fields (R08.T18.a); absent, none emits.
 */
export function litBodiesOf(
  scene: ViewScene,
  pose: CameraPose,
  discs: ReadonlyArray<HostDiscDto>,
  classMaps?: ClassMapSource,
  terrain: LitBodiesTerrain = {},
  groundFields?: GroundFieldSource,
): LitBodyInput[] {
  return lightingFramesOf(scene, pose, discs).map((lit) =>
    litBodyOf(lit, classMaps, terrain, groundFields),
  );
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
  // Each lit body whose tables the cache holds, with its atmosphere's camera from its drawn centre
  // at the frame's own viewport, where its composite is drawn (R08.T10.b).
  const atmospheres: FrameAtmosphere[] = [];
  const tablesOf = inputs.atmospheres;
  if (tablesOf !== undefined) {
    for (const body of scene.bodies.filter(isLitBody)) {
      const tables = tablesOf(body.id);
      const air =
        tables === null
          ? null
          : bodyAtmosphereCamera(scene, body, pose, origins, {
              fovXRad: camera.fovXRad,
              viewport,
            });
      if (tables !== null && air !== null) {
        atmospheres.push({
          id: body.id,
          tables,
          camera: air,
          toBodyFixed: atmosphereToBodyFixed(body),
        });
      }
    }
  }
  const litBodies = litBodiesOf(
    scene,
    pose,
    discs,
    inputs.classMaps,
    { reliefs: inputs.reliefs, terrain: inputs.terrain },
    inputs.groundFields,
  );
  // A disc's glare is of the light that reaches the camera past the frame's bodies.
  const occluders = litBodies.map((body) => ({
    centreM: body.centreM,
    radiusM: Math.sqrt(body.figure.equatorialRadiusM * body.figure.polarRadiusM),
  }));
  return {
    camera,
    viewport,
    presentedPx: inputs.presentedPx,
    role: run.camera.role,
    setting: inputs.setting,
    exposureScale,
    sky: [...(band === null ? [] : [band]), ...(inputs.cube === null ? [] : [inputs.cube])],
    starSprites: [...inputs.list.sprites.map(starRecord), ...hostSprites],
    hostDraws,
    glareSources: inputs.discs.glareSources(
      camera,
      viewport,
      run.camera.role,
      occluders,
      SETTINGS[inputs.setting].photoreal.discAnnuli,
      frameGlareAir(atmospheres, litBodies, lights),
    ),
    lights,
    bodies: litBodies,
    // A body handed over to terrain writes depth as a mesh does (`drawnAsMesh`), its footprint
    // counted by its regime (R10.T16.c); lit craft will add theirs.
    depthWriters: [],
    previousRegimes: inputs.previousRegimes,
    previousTabled: inputs.previousTabled,
    terrain: inputs.terrain,
    overlay: inputs.overlay,
    meter: inputs.meter,
    present: inputs.present,
    atmospheres,
    onAtmosphereFailed: inputs.atmosphereFailed,
  };
}
