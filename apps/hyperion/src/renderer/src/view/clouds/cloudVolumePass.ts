/**
 * The cloud volume's passes (plan R11, R11.T7.c; Design notes 10 and 18): the high setting's clouds
 * of one body marched at a quarter of the view's pixels with temporal reprojection, then laid over
 * the body's air at the view's own resolution, as the two-dimensional layer is laid.
 *
 * @remarks
 * - **Three passes a frame** (`cloudVolumeMarch.wgsl`, `cloudVolumeResolve.wgsl`,
 *   `cloudVolumeComposite.wgsl`): the march of each pixel's ray through the deck into the volume's
 *   target, at half the view's width and height ({@link volumeTargetSize}), its steps offset by the
 *   blue noise moved by the frame's jitter ({@link volumeJitter}); the resolve, which blends it with
 *   the last frame's resolved clouds where the same cloud stood then ({@link reprojectedUv}), held to
 *   the march's 3 × 3 range, into the other of two histories; and the composite, bilinear, over the
 *   target the body's composite drew, where the scene stands no nearer than the deck.
 * - **Which body.** The nearest of the frame's bodies whose clouds the layer would draw, on a
 *   setting whose `clouds` is `volume` while the frame-rate floor has not stepped the air down;
 *   every other body, and that one until its condensate's correction is made, draws the layer
 *   (lean, 2026-11-18: one body's history a view).
 * - **The correction** of the octaves to δ-Eddington (`cloudVolume.ts`' `octaveCorrectionOf`) is
 *   made a μ₀ row a step over frames within the frame's budget ({@link OctaveCorrectionBuilder}),
 *   per condensate's optics, into an `r32float` texture of 2 KiB in the `clouds` category, beside the
 *   volume's three quarter-resolution `rgba16float` targets.
 * - **The history** holds where the last frame resolved the same body at the same size; a cut, a
 *   new body or a new size begins again from the march alone.
 * - **Its density** is the layer's field (`cloud_field_at`), the deck's column spread uniformly
 *   through its depth (`cloudVolume.wgsl`); Design note 10's 128³, 32³ and 128² noises are not built
 *   (plan R11's T7.c records).
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { dot, norm, sub, type Vec3 } from "../../geometry/vec3";
import { tan } from "../../lib/pinnedMath";
import {
  SURFACE_LIGHTING_WGSL,
  type SurfaceLightingTables,
  surfaceLightingDrawTextures,
  surfaceLightingTextures,
} from "../atmosphere/surfaceLighting";
import { NEAR_PLANE_M } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type {
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  RenderTarget,
  TexelRect,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
} from "../engine/types";
import { HDR_COLOUR_FORMAT } from "../photometry/toneCurve";
import frameWgsl from "../shaders/frame.wgsl?raw";
import {
  CLOUD_LAYER_PASS,
  CLOUD_TEXTURES,
  type CloudBodyGpu,
  type CloudLayerView,
  cloudLayerUniforms,
} from "./cloudLayer";
import { CORRECTION_MUS, CORRECTION_TAUS, octaveCorrectionRow } from "./cloudVolume";
import type { CloudCondensate, CloudOptics } from "./condensates";
import cloudFieldWgsl from "./shaders/cloudField.wgsl?raw";
import cloudOpticsWgsl from "./shaders/cloudOptics.wgsl?raw";
import cloudVolumeWgsl from "./shaders/cloudVolume.wgsl?raw";
import cloudVolumeCompositeWgsl from "./shaders/cloudVolumeComposite.wgsl?raw";
import cloudVolumeMarchWgsl from "./shaders/cloudVolumeMarch.wgsl?raw";
import cloudVolumeResolveWgsl from "./shaders/cloudVolumeResolve.wgsl?raw";

/** The label of the volume's march in `PassTimes`. */
export const CLOUD_VOLUME_PASS = "clouds volume";

/** The label of its temporal resolve. */
export const CLOUD_RESOLVE_PASS = "clouds resolve";

/**
 * The march's steps through a ray's stretch of the deck (lean, 2026-11-18): 32, the march's twin's
 * deck cases' count, which the frames' jitter spreads over the stretch.
 */
export const VOLUME_STEPS = 32;

/**
 * The weight of a frame's march in its resolve, the rest the reprojected history's (lean,
 * 2026-11-18): an eighth, some 16 frames to settle within 12%.
 */
export const TEMPORAL_WEIGHT = 0.125;

/** The golden ratio's fraction, by which the frames' jitter steps (an additive recurrence). */
const JITTER_STEP = 0.6180339887498949;

/** The march's bindings in `@group(2)` (`cloudVolumeMarch.wgsl`'s). */
export const CLOUD_VOLUME_BINDINGS = {
  sceneDepth: 0,
  coverage: 1,
  noise: 2,
  sampler: 3,
  layer: 4,
  /** The first of R08's surface lighting's nine tables. */
  atmosphere: 5,
  correction: 14,
  blueNoise: 15,
} as const;

/** The names a volume's draws bind their own textures under. */
export const CLOUD_VOLUME_TEXTURES = {
  correction: "cloudOctaveCorrection",
  blueNoise: "blueNoise",
  march: "cloudMarch",
  history: "cloudHistory",
  historySampler: "cloudHistorySampler",
  resolved: "cloudResolved",
  resolvedSampler: "cloudResolvedSampler",
} as const;

/** The march's `Draw` members after `offsetFromCameraM`, each a `vec4f`. */
const MARCH_MEMBERS = [
  "camera",
  "cameraNormal",
  "right",
  "upRay",
  "forward",
  "shell",
  "view",
  "source0",
  "sourceLight0",
  "source1",
  "sourceLight1",
  "levers",
  "deck",
] as const;

/** The march's WGSL: `frame.wgsl`, R08's surface lighting, the field, the optics, the volume. */
export const CLOUD_VOLUME_WGSL = `${frameWgsl}\n${SURFACE_LIGHTING_WGSL}\n${cloudFieldWgsl}\n${cloudOpticsWgsl}\n${cloudVolumeWgsl}\n${cloudVolumeMarchWgsl}`;

/** The volume's march: a full-screen triangle into the volume's quarter-resolution target. */
export const CLOUD_VOLUME_MATERIAL: WgslMaterialSpec = {
  name: "cloud volume",
  displayName: "CLOUD VOLUME",
  vertexWgsl: CLOUD_VOLUME_WGSL,
  fragmentWgsl: CLOUD_VOLUME_WGSL,
  uniforms: MARCH_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [
    {
      name: CLOUD_TEXTURES.sampler,
      filter: "linear",
      address: "repeat",
      binding: CLOUD_VOLUME_BINDINGS.sampler,
    },
  ],
  textures: [
    { name: "sceneDepth", binding: CLOUD_VOLUME_BINDINGS.sceneDepth, sampleType: "depth" },
    { name: CLOUD_TEXTURES.coverage, binding: CLOUD_VOLUME_BINDINGS.coverage },
    { name: CLOUD_TEXTURES.noise, binding: CLOUD_VOLUME_BINDINGS.noise, viewDimension: "3d" },
    ...surfaceLightingTextures(CLOUD_VOLUME_BINDINGS.atmosphere),
    {
      name: CLOUD_VOLUME_TEXTURES.correction,
      binding: CLOUD_VOLUME_BINDINGS.correction,
      sampleType: "unfilterable-float",
    },
    { name: CLOUD_VOLUME_TEXTURES.blueNoise, binding: CLOUD_VOLUME_BINDINGS.blueNoise },
  ],
  storageBuffers: [{ name: CLOUD_TEXTURES.layer, binding: CLOUD_VOLUME_BINDINGS.layer }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The resolve's `Draw` members after `offsetFromCameraM`. */
const RESOLVE_MEMBERS = [
  "camera",
  "right",
  "upRay",
  "forward",
  "shell",
  "moved",
  "lastRight",
  "lastUp",
  "lastForward",
  "view",
] as const;

/** The volume's temporal resolve: the march blended with the reprojected history. */
export const CLOUD_RESOLVE_MATERIAL: WgslMaterialSpec = {
  name: "cloud volume resolve",
  displayName: "CLOUD VOLUME RESOLVE",
  vertexWgsl: `${frameWgsl}\n${cloudVolumeResolveWgsl}`,
  fragmentWgsl: `${frameWgsl}\n${cloudVolumeResolveWgsl}`,
  uniforms: RESOLVE_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [
    {
      name: CLOUD_VOLUME_TEXTURES.historySampler,
      filter: "linear",
      address: "clamp-to-edge",
      binding: 2,
    },
  ],
  textures: [
    { name: CLOUD_VOLUME_TEXTURES.march, binding: 0 },
    { name: CLOUD_VOLUME_TEXTURES.history, binding: 1 },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The composite's `Draw` members after `offsetFromCameraM`. */
const COMPOSITE_MEMBERS = ["camera", "right", "upRay", "forward", "deck", "view"] as const;

/** The volume's composite over the body's air, premultiplied, keeping the meter class. */
export const CLOUD_COMPOSITE_MATERIAL: WgslMaterialSpec = {
  name: "cloud volume composite",
  displayName: "CLOUD VOLUME COMPOSITE",
  vertexWgsl: `${frameWgsl}\n${cloudVolumeCompositeWgsl}`,
  fragmentWgsl: `${frameWgsl}\n${cloudVolumeCompositeWgsl}`,
  uniforms: COMPOSITE_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [
    {
      name: CLOUD_VOLUME_TEXTURES.resolvedSampler,
      filter: "linear",
      address: "clamp-to-edge",
      binding: 2,
    },
  ],
  textures: [
    { name: "sceneDepth", binding: 0, sampleType: "depth" },
    { name: CLOUD_VOLUME_TEXTURES.resolved, binding: 1 },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "premultiplied",
};

/** The volume's three materials, for the catalogue (compiled in the smoke's part 26). */
export const CLOUD_VOLUME_MATERIALS: ReadonlyArray<WgslMaterialSpec> = [
  CLOUD_VOLUME_MATERIAL,
  CLOUD_RESOLVE_MATERIAL,
  CLOUD_COMPOSITE_MATERIAL,
];

/** The volume's targets' size for a view of `size`: half its width and height, rounded up. */
export function volumeTargetSize(size: ViewSize): ViewSize {
  return {
    widthPx: Math.max(1, Math.ceil(size.widthPx / 2)),
    heightPx: Math.max(1, Math.ceil(size.heightPx / 2)),
  };
}

/** Frame `index`'s jitter of the march's steps, in [0, 1): the golden ratio's recurrence. */
export function volumeJitter(index: number): number {
  const at = index * JITTER_STEP;
  return at - Math.floor(at);
}

/**
 * A condensate's correction table made a μ₀ row a step (`cloudVolume.ts`' `octaveCorrectionRow`),
 * so that its making, some tens of milliseconds, is spread over frames; the table at its last
 * step is `octaveCorrectionOf`'s, bit for bit.
 */
export class OctaveCorrectionBuilder {
  readonly #optics: CloudOptics;
  readonly #factors = new Float32Array(CORRECTION_TAUS * CORRECTION_MUS);
  #row = 0;

  constructor(optics: CloudOptics) {
    this.#optics = optics;
  }

  /** Does one row; the finished factors once the last is done, `null` before. */
  step(): Float32Array | null {
    if (this.#row < CORRECTION_MUS) {
      octaveCorrectionRow(this.#optics, this.#row, this.#factors);
      this.#row += 1;
    }
    return this.#row === CORRECTION_MUS ? this.#factors : null;
  }
}

/** A camera as the resolve reprojects into it: its place and basis, body-fixed. */
export interface VolumeCamera {
  /** Its place from the body's centre, body-fixed, m. */
  readonly positionM: Vec3;
  /** Its rays' basis: right × tan(fovX ÷ 2), up × tan(fovY ÷ 2), forward, body-fixed. */
  readonly right: Vec3;
  readonly up: Vec3;
  readonly forward: Vec3;
}

/** A camera's place and basis from the layer's view of it. */
export function volumeCameraOf(view: Pick<CloudLayerView, "camera">): VolumeCamera {
  const { camera } = view;
  const tanX = tan(camera.fovXRad / 2);
  const tanY = tanX / (camera.viewport.widthPx / camera.viewport.heightPx);
  const right = rotate(camera.orientation, { x: 1, y: 0, z: 0 });
  const up = rotate(camera.orientation, { x: 0, y: 1, z: 0 });
  return {
    positionM: camera.positionM,
    right: { x: right.x * tanX, y: right.y * tanX, z: right.z * tanX },
    up: { x: up.x * tanY, y: up.y * tanY, z: up.z * tanY },
    forward: rotate(camera.orientation, { x: 0, y: 0, z: -1 }),
  };
}

/**
 * Where a place, body-fixed, m, stood in `last`'s view, as `cloudVolumeResolve.wgsl` reprojects
 * it: its texture coordinates from the top left, or `null` behind that camera or outside its view.
 */
export function reprojectedUv(
  placeM: Vec3,
  last: VolumeCamera,
): { readonly u: number; readonly v: number } | null {
  const v = sub(placeM, last.positionM);
  const z = dot(v, last.forward);
  if (!(z > 0)) {
    return null;
  }
  const rightSq = dot(last.right, last.right);
  const upSq = dot(last.up, last.up);
  const x = dot(v, last.right) / rightSq / z;
  const y = dot(v, last.up) / upSq / z;
  if (Math.abs(x) > 1 || Math.abs(y) > 1) {
    return null;
  }
  return { u: 0.5 + 0.5 * x, v: 0.5 - 0.5 * y };
}

/** A body's volume this frame: what its march, resolve and composite draw from. */
export interface CloudVolumeDraw {
  readonly id: BodyIdHex;
  readonly gpu: CloudBodyGpu;
  readonly condensate: CloudCondensate;
  /** The layer's view of the body this frame, at the frame's own viewport. */
  readonly view: CloudLayerView;
  readonly lighting: SurfaceLightingTables;
  readonly sceneDepth: TextureHandle;
  readonly blueNoise: TextureHandle;
  readonly correction: TextureHandle;
}

/** The volume's materials and the full-screen triangle they draw. */
export interface CloudVolumeMaterials {
  readonly march: MaterialHandle;
  readonly resolve: MaterialHandle;
  readonly composite: MaterialHandle;
  readonly triangle: MeshHandle;
}

/** A pass's rotation and projection, which its full-screen draws read none of. */
interface PassMatrices {
  readonly viewRotation: Float32Array;
  readonly projection: Float32Array;
}

/** A view's volume targets: the march's and the two histories, at a quarter of the view's pixels. */
interface VolumeTargets {
  readonly march: RenderTarget;
  readonly histories: readonly [RenderTarget, RenderTarget];
  readonly size: ViewSize;
}

/** The time a correction table's making may take a frame, ms. */
export const CORRECTION_STEP_BUDGET_MS = 1;

/**
 * A view's cloud volume (the module's remarks): its targets and history, made at its first frame
 * and at a new size, and each condensate's correction texture, made over frames; it owns them and
 * none of the materials.
 */
export class CloudVolumes {
  readonly #engine: Pick<
    RenderEngine,
    "createRenderTarget" | "createTexture" | "writeTexture" | "releaseTexture"
  >;
  readonly #name: string;
  readonly #corrections = new Map<
    string,
    { builder: OctaveCorrectionBuilder | null; texture: TextureHandle | null }
  >();
  #targets: VolumeTargets | null = null;
  /** The history the last frame resolved into, and what it resolved; `null` where none holds. */
  #last: {
    readonly at: 0 | 1;
    readonly id: BodyIdHex;
    readonly camera: VolumeCamera;
  } | null = null;
  #frames = 0;

  constructor(
    engine: Pick<
      RenderEngine,
      "createRenderTarget" | "createTexture" | "writeTexture" | "releaseTexture"
    >,
    name: string,
  ) {
    this.#engine = engine;
    this.#name = name;
  }

  /**
   * The correction texture of a condensate's optics, made a row a step for up to `budgetMs` of
   * `nowMs`'s clock at each call until it is made; `null` before.
   */
  correction(
    optics: CloudOptics,
    nowMs: () => number,
    budgetMs = CORRECTION_STEP_BUDGET_MS,
  ): TextureHandle | null {
    const key = `${String(optics.asymmetry)}/${String(optics.coAlbedo)}`;
    let held = this.#corrections.get(key);
    if (held === undefined) {
      held = { builder: new OctaveCorrectionBuilder(optics), texture: null };
      this.#corrections.set(key, held);
    }
    const { builder } = held;
    if (builder !== null) {
      const start = nowMs();
      let factors = builder.step();
      while (factors === null && nowMs() - start < budgetMs) {
        factors = builder.step();
      }
      if (factors !== null) {
        const size = { width: CORRECTION_TAUS, height: CORRECTION_MUS };
        const texture = this.#engine.createTexture({
          name: `${this.#name} cloud correction ${key}`,
          size,
          dimension: "2d",
          format: "r32float",
          mips: 1,
          usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
          category: "clouds",
        });
        this.#engine.writeTexture(texture, { x: 0, y: 0 }, size, factors);
        held.texture = texture;
        held.builder = null;
      }
    }
    return held.texture;
  }

  /** Drops the history, so that the next frame resolves from its march alone (a cut). */
  cut(): void {
    this.#last = null;
  }

  /**
   * Draws a body's volume: its march and its resolve into the volume's own targets, then its
   * composite into `target`, within `scissor` where given, over what the body's composite drew.
   *
   * @param size - The frame's own size, the scene depth's.
   */
  draw(
    volume: CloudVolumeDraw,
    materials: CloudVolumeMaterials,
    target: RenderTarget,
    size: ViewSize,
    pass: PassMatrices,
    scissor: TexelRect | null,
  ): void {
    const targets = this.#targetsOf(size);
    const quarter = targets.size;
    const { view, gpu, condensate } = volume;
    const marchView: CloudLayerView = {
      ...view,
      camera: { ...view.camera, viewport: quarter },
    };
    const layer = cloudLayerUniforms(marchView);
    const jitter = volumeJitter(this.#frames);
    this.#frames += 1;
    targets.march.render({
      label: CLOUD_VOLUME_PASS,
      viewRotation: pass.viewRotation,
      projection: pass.projection,
      draws: [
        {
          mesh: materials.triangle,
          material: materials.march,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {
            ...layer,
            levers: new Float32Array([view.octaves, VOLUME_STEPS, jitter, 0]),
            deck: new Float32Array([
              condensate.baseAltitudeM,
              condensate.topAltitudeM,
              size.widthPx,
              size.heightPx,
            ]),
          },
          textures: {
            sceneDepth: volume.sceneDepth,
            [CLOUD_TEXTURES.coverage]: gpu.coverage,
            [CLOUD_TEXTURES.noise]: gpu.noise,
            ...surfaceLightingDrawTextures(volume.lighting),
            [CLOUD_VOLUME_TEXTURES.correction]: volume.correction,
            [CLOUD_VOLUME_TEXTURES.blueNoise]: volume.blueNoise,
          },
          storageBuffers: { [CLOUD_TEXTURES.layer]: gpu.layer },
        },
      ],
      postProcesses: [],
    });
    const camera = volumeCameraOf(marchView);
    const last = this.#last?.id === volume.id ? this.#last : null;
    const into: 0 | 1 = last === null ? 0 : last.at === 0 ? 1 : 0;
    const from = targets.histories[into === 0 ? 1 : 0];
    const moved =
      last === null ? { x: 0, y: 0, z: 0 } : sub(camera.positionM, last.camera.positionM);
    const lastCamera = last?.camera ?? camera;
    const shell = memberOf(layer, "shell");
    targets.histories[into].render({
      label: CLOUD_RESOLVE_PASS,
      viewRotation: pass.viewRotation,
      projection: pass.projection,
      draws: [
        {
          mesh: materials.triangle,
          material: materials.resolve,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {
            camera: new Float32Array([
              camera.positionM.x,
              camera.positionM.y,
              camera.positionM.z,
              last === null ? 0 : 1,
            ]),
            right: memberOf(layer, "right"),
            upRay: memberOf(layer, "upRay"),
            forward: memberOf(layer, "forward"),
            shell: new Float32Array([shell[0] ?? 0, shell[1] ?? 1, 0, TEMPORAL_WEIGHT]),
            moved: new Float32Array([moved.x, moved.y, moved.z, 0]),
            lastRight: overSquare(lastCamera.right),
            lastUp: overSquare(lastCamera.up),
            lastForward: new Float32Array([
              lastCamera.forward.x,
              lastCamera.forward.y,
              lastCamera.forward.z,
              0,
            ]),
            view: new Float32Array([quarter.widthPx, quarter.heightPx, 0, 0]),
          },
          textures: {
            [CLOUD_VOLUME_TEXTURES.march]: targets.march.colour,
            [CLOUD_VOLUME_TEXTURES.history]: from.colour,
          },
        },
      ],
      postProcesses: [],
    });
    this.#last = { at: into, id: volume.id, camera };
    target.render({
      label: CLOUD_LAYER_PASS,
      viewRotation: pass.viewRotation,
      projection: pass.projection,
      draws: [
        {
          mesh: materials.triangle,
          material: materials.composite,
          offsetFromCameraM: new Float32Array(3),
          uniforms: compositeUniforms(view, condensate),
          textures: {
            sceneDepth: volume.sceneDepth,
            [CLOUD_VOLUME_TEXTURES.resolved]: targets.histories[into].colour,
          },
        },
      ],
      postProcesses: [],
      colourLoad: "load",
      ...(scissor === null ? {} : { scissor }),
    });
  }

  /** Lets go of every condensate's correction but those of `kept`'s optics, by their keys. */
  keepOnly(kept: ReadonlyArray<CloudOptics>): void {
    const keys = new Set(
      kept.map((optics) => `${String(optics.asymmetry)}/${String(optics.coAlbedo)}`),
    );
    for (const [key, held] of this.#corrections) {
      if (!keys.has(key)) {
        if (held.texture !== null) {
          this.#engine.releaseTexture(held.texture);
        }
        this.#corrections.delete(key);
      }
    }
  }

  /** Lets go of everything. */
  dispose(): void {
    this.keepOnly([]);
    this.#releaseTargets();
    this.#last = null;
  }

  #targetsOf(size: ViewSize): VolumeTargets {
    const quarter = volumeTargetSize(size);
    const held = this.#targets;
    if (
      held !== null &&
      held.size.widthPx === quarter.widthPx &&
      held.size.heightPx === quarter.heightPx
    ) {
      return held;
    }
    this.#releaseTargets();
    this.#last = null;
    const made = (what: string): RenderTarget =>
      this.#engine.createRenderTarget({
        name: `${this.#name}:cloud ${what}`,
        size: quarter,
        format: HDR_COLOUR_FORMAT,
        mips: 1,
        depth: false,
        category: "clouds",
      });
    const targets: VolumeTargets = {
      march: made("march"),
      histories: [made("history 0"), made("history 1")],
      size: quarter,
    };
    this.#targets = targets;
    return targets;
  }

  #releaseTargets(): void {
    const held = this.#targets;
    if (held !== null) {
      held.march.dispose();
      for (const history of held.histories) {
        history.dispose();
      }
      this.#targets = null;
    }
  }
}

/**
 * The layer's uniform `name` (`cloudLayerUniforms`'), which the volume's passes share.
 *
 * @throws Error where the layer's uniforms hold no such member, a renamed one.
 */
function memberOf(uniforms: Readonly<Record<string, Float32Array>>, name: string): Float32Array {
  const member = uniforms[name];
  if (member === undefined) {
    throw new Error(`the cloud layer's uniforms hold no ${name}`);
  }
  return member;
}

/** `v` over its squared length, as a `vec4f`: a ray basis's axis ÷ its tangent. */
function overSquare(v: Vec3): Float32Array {
  const sq = dot(v, v);
  return new Float32Array([v.x / sq, v.y / sq, v.z / sq, 0]);
}

/**
 * The composite's uniforms: the camera's place and rays at the frame's own viewport, and the deck's
 * flattening and its top's and base's quadric constants, |S o|² − r², formed in `f64`.
 */
export function compositeUniforms(
  view: CloudLayerView,
  condensate: Pick<CloudCondensate, "baseAltitudeM" | "topAltitudeM">,
): Readonly<Record<string, Float32Array>> {
  const layer = cloudLayerUniforms(view);
  const o = view.camera.positionM;
  const k = view.shell.equatorialRadiusM / view.shell.polarRadiusM;
  const scaled = o.x * o.x + o.y * o.y + o.z * k * (o.z * k);
  const a = view.figure.equatorialRadiusM;
  const top = a + condensate.topAltitudeM;
  const base = a + condensate.baseAltitudeM;
  return {
    camera: new Float32Array([o.x, o.y, o.z, norm(o)]),
    right: memberOf(layer, "right"),
    upRay: memberOf(layer, "upRay"),
    forward: memberOf(layer, "forward"),
    deck: new Float32Array([k, scaled - top * top, scaled - base * base, NEAR_PLANE_M]),
    view: new Float32Array([view.camera.viewport.widthPx, view.camera.viewport.heightPx, 0, 0]),
  };
}
