/**
 * The host stars of the camera's system as limb-darkened discs (plan R06, Design note 16, T13.e).
 *
 * @remarks
 * Each host from the `sky` response's `hosts` is placed where the scene draws it (its apparent
 * position, light time and aberration, R03), at angular radius asin(R ÷ d). A disc whose diameter
 * is three pixels or more is drawn analytically by `disc.wgsl` into the HDR scene target, writing
 * R07's meter class `hostDisc`; a smaller one is a point sprite of the same illuminance, π L̄ sin²ρ.
 * The light the target's 65,504 cannot hold is handed to R07's glare pass as one `GlareSource` per
 * disc, which in an eye view is kept for a disc up to 45° outside the frame (R07 Design note 12).
 * The discs are drawn only in the photorealistic style (decision record item 1).
 */

import { formatBodyId, type HostDiscDto } from "@hyperion/protocol";

import { add, norm, normalise, sub, type Vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import { toViewAxes } from "../camera/projection";
import type { ViewRole } from "../camera/state";
import { expressIn, type ViewPosition } from "../coords/position";
import { frameOrigin } from "../coords/relative";
import type {
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  WgslMaterialSpec,
} from "../engine/types";
import type { GlareSource } from "../post/glare";
import { sceneOrigins, type ViewScene } from "../scene/model";
import frameWgsl from "../shaders/frame.wgsl?raw";
import type { SpriteStar } from "../wireframe/drawList";
import {
  angularRadiusRad,
  discExcessLuminanceRgb,
  discIlluminanceRgbLx,
  rgbOfBvr,
} from "./discFlux";
import discWgsl from "./shaders/disc.wgsl?raw";

/** The disc's full-screen draw into the HDR scene target. */
export const DISC_MATERIAL: WgslMaterialSpec = {
  name: "sky:hostDisc",
  displayName: "STAR DISCS",
  vertexWgsl: frameWgsl + discWgsl,
  fragmentWgsl: frameWgsl + discWgsl,
  uniforms: [
    { name: "axis", type: "vec4f" },
    { name: "central", type: "vec4f" },
    { name: "limbC", type: "vec4f" },
    { name: "limbAlpha", type: "vec4f" },
    { name: "exposure", type: "vec4f" },
  ],
  samplers: [],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The diameter, px, below which a disc is drawn as a point sprite: three pixels. */
export const DISC_MIN_DIAMETER_PX = 3;

/** How far outside an eye view's frame a disc still casts glare into it, rad: 45°. */
export const EYE_GLARE_REACH_RAD = Math.PI / 4;

/** A host star where the scene draws it. */
export interface HostPlacement {
  readonly host: HostDiscDto;
  /** Its direction from the camera, unit, on the galactic axes. */
  readonly direction: Vec3;
  /** Its distance from the camera, m. */
  readonly distanceM: number;
}

/** One disc's draw, keyed by its star's body index, so that R07's painter order can place it. */
export interface DiscDraw {
  /** The host's `HostDiscDto.star`, its star's body index in the scene's system. */
  readonly star: number;
  readonly item: DrawItem;
}

/** The discs of a frame: their draws, and the sprites of those too small to be discs. */
export interface DiscFrame {
  readonly draws: ReadonlyArray<DiscDraw>;
  readonly sprites: ReadonlyArray<SpriteStar>;
}

/**
 * The hosts the scene has, placed where it draws them from the camera.
 *
 * @remarks
 * Joined as R07's lights are (decision record R07.T8.a, R06 coordination (c)): a host's
 * `HostDiscDto.star` is its star's body index, so its scene body is
 * `formatBodyId({ system: scene.system, bodyIndex: host.star })`, and the disc is drawn at that
 * body's drawn centre (its apparent position). A host the scene does not have is left out. A
 * camera inside a star has no placement for it.
 */
export function hostPlacements(
  scene: ViewScene,
  hosts: ReadonlyArray<HostDiscDto>,
  pose: CameraPose,
): HostPlacement[] {
  const origins = sceneOrigins(scene);
  const origin = frameOrigin(pose.frame, origins);
  const camera: ViewPosition =
    origin.kind === "galactic" ? origin : { ...origin, m: add(origin.m, pose.positionM) };
  const inSystem = expressIn(camera, { kind: "system", system: scene.system }, origins);
  if (inSystem.kind !== "system") {
    throw new Error("a position expressed in the system frame came back in another frame");
  }
  const bodies = new Map(scene.bodies.map((body) => [body.id, body]));
  const placements: HostPlacement[] = [];
  for (const host of hosts) {
    const body = bodies.get(formatBodyId({ system: scene.system, bodyIndex: host.star }));
    if (body === undefined) {
      continue;
    }
    const fromCamera = sub(body.centreM, inSystem.m);
    const distanceM = norm(fromCamera);
    if (distanceM > host.radius_m) {
      placements.push({ host, direction: normalise(fromCamera), distanceM });
    }
  }
  return placements;
}

/**
 * How far a direction lies outside a camera's frame, rad: the larger of its angles past the left
 * or right edge and past the top or bottom edge, each measured in its own plane; zero or less
 * inside.
 */
export function angleOutsideFrameRad(
  direction: Vec3,
  camera: ProjectionCamera,
  viewport: Viewport,
): number {
  const view = toViewAxes(normalise(direction), camera.orientation);
  const tanX = Math.tan(camera.fovXRad / 2);
  const tanY = (tanX * viewport.heightPx) / viewport.widthPx;
  const pastSide = Math.atan2(Math.abs(view.x), -view.z) - Math.atan(tanX);
  const pastTop = Math.atan2(Math.abs(view.y), -view.z) - Math.atan(tanY);
  return Math.max(pastSide, pastTop);
}

/** The pixel's angle at the frame's centre, rad. */
function centrePixelRad(camera: ProjectionCamera, viewport: Viewport): number {
  return (2 * Math.tan(camera.fovXRad / 2)) / viewport.widthPx;
}

/** The host stars' discs on the GPU. */
export class HostDiscLayer {
  readonly #material: MaterialHandle;
  readonly #triangle: MeshHandle;
  /** The last frame's hosts drawn as discs; those drawn as sprites cast no glare source. */
  #discs: ReadonlyArray<HostPlacement> = [];
  #exposureScale = 1;

  constructor(engine: RenderEngine) {
    this.#material = engine.createMaterial(DISC_MATERIAL);
    this.#triangle = engine.createMesh({
      name: "sky disc triangle",
      positions: new Float32Array([-1, -1, 0, 3, -1, 0, -1, 3, 0]),
      indices: null,
      topology: "triangle-list",
      attributes: {},
    });
  }

  /**
   * The frame's discs: a draw for each disc of three pixels or more across, a sprite for each
   * smaller one.
   *
   * @param exposureScale - The frame's pre-exposure scale (R02's `exposureScale`).
   */
  frame(
    placements: ReadonlyArray<HostPlacement>,
    camera: ProjectionCamera,
    viewport: Viewport,
    exposureScale: number,
  ): DiscFrame {
    this.#exposureScale = exposureScale;
    const discs: HostPlacement[] = [];
    const pixelRad = centrePixelRad(camera, viewport);
    const draws: DiscDraw[] = [];
    const sprites: SpriteStar[] = [];
    for (const { host, direction, distanceM } of placements) {
      const rho = angularRadiusRad(host.radius_m, distanceM);
      const axis = normalise(direction);
      if ((2 * rho) / pixelRad < DISC_MIN_DIAMETER_PX) {
        sprites.push({
          id: `host ${String(host.star)}`,
          direction: axis,
          illuminanceRgbLx: discIlluminanceRgbLx(host, rho),
        });
        continue;
      }
      discs.push({ host, direction, distanceM });
      const central = rgbOfBvr(host.central_luminance_cd_m2);
      draws.push({
        star: host.star,
        item: {
          mesh: this.#triangle,
          material: this.#material,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {
            axis: new Float32Array([axis.x, axis.y, axis.z, Math.sin(rho)]),
            central: new Float32Array([central[0], central[1], central[2], 0]),
            limbC: new Float32Array([host.limb[2].c, host.limb[1].c, host.limb[0].c, 0]),
            limbAlpha: new Float32Array([
              host.limb[2].alpha,
              host.limb[1].alpha,
              host.limb[0].alpha,
              0,
            ]),
            exposure: new Float32Array([exposureScale, 0, 0, 0]),
          },
          textures: {},
        },
      });
    }
    this.#discs = discs;
    return { draws, sprites };
  }

  /**
   * One R07 `GlareSource` per disc of the last frame that can glare into the view: one touching
   * the frame for a camera, and one up to 45° outside it for the eye (R07's asks).
   *
   * @remarks
   * A host drawn as a sprite casts none: its light is in the sprite, which the target holds, and
   * R07's bloom chain spreads it.
   */
  glareSources(camera: ProjectionCamera, viewport: Viewport, role: ViewRole): GlareSource[] {
    const reach = role === "eye" ? EYE_GLARE_REACH_RAD : 0;
    const sources: GlareSource[] = [];
    for (const { host, direction, distanceM } of this.#discs) {
      const axis = normalise(direction);
      const rho = angularRadiusRad(host.radius_m, distanceM);
      if (angleOutsideFrameRad(axis, camera, viewport) - rho > reach) {
        continue;
      }
      sources.push({
        direction: axis,
        angularRadiusRad: rho,
        excessLuminance: discExcessLuminanceRgb(host, this.#exposureScale),
      });
    }
    return sources;
  }
}
