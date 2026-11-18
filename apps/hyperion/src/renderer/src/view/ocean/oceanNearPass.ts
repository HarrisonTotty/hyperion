/**
 * The `ocean` pass's near sea (plan R11, R11.T8.c; Design notes 11, 12 and 18): its materials, the
 * uniforms and draw of a sea seen from under 20 km, and the copies of the scene it reads.
 *
 * @remarks
 * - **When.** A sea is drawn near where the camera stands above its sea-level spheroid and under
 *   `OCEAN_NEAR_BELOW_M` (`oceanNear.ts`' `drawsNearSea`); the far draw then draws its closed basins
 *   alone (`oceanPassUniforms`' `basinsOnly`), and above the switch the far draw draws all of it.
 * - **The copies.** Before its draws the pass copies the scene's depth, the opaque passes', into an
 *   `r32uint` target as its bits ({@link OCEAN_DEPTH_COPY_MATERIAL}), so that the near sea reads the
 *   bed's depth while it draws into the scene's own; and, with refraction (the high setting's
 *   `oceanRefraction`), the scene's colour into an `rgba16float` target by `colourFrom`, the bed's
 *   light. Both are under `ocean`, made at the first near frame and let go at a frame with none.
 * - **The grid** is `oceanNear.ts`' mesh at the setting's cell (`OCEAN_GRID_CELL_PX`) over the
 *   frame's size, made again when either changes.
 * - **The shore** (R11.T8.d, `shoreline.ts`): its amplitudes limited by the water's depth, its
 *   surf's foam, and its premultiplied alpha, the depth fade to the bed.
 */
import type { UniverseTime } from "@hyperion/protocol";

import type { SurfaceLightingTables } from "../atmosphere/surfaceLighting";
import {
  surfaceLightingDrawTextures,
  surfaceLightingTextures,
} from "../atmosphere/surfaceLighting";
import type {
  BufferHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import frameWgsl from "../shaders/frame.wgsl?raw";
import oceanDepthCopyWgsl from "./oceanDepthCopy.wgsl?raw";
import type { GerstnerSet } from "./gerstner";
import oceanNearWgsl from "./oceanNear.wgsl?raw";
import shorelineWgsl from "./shoreline.wgsl?raw";
import {
  type OceanNearFrame,
  oceanNearFrame,
  oceanNearPeakWavelengthM,
  oceanNearSet,
  type OceanWave,
  type OceanWaveAnchor,
  oceanWavesAt,
} from "./oceanNear";
import {
  OCEAN_ECLIPSE_BUFFER,
  OCEAN_MAP_TEXTURE,
  OCEAN_PASS_BINDINGS,
  OCEAN_SHARED_WGSL,
  type OceanDrawView,
  oceanDrawOptics,
  oceanPassUniforms,
} from "./oceanPass";

/** The label of the pass's copies of the scene, before its draws (R12's `ocean` row). */
export const OCEAN_COPY_PASS_LABEL = "ocean copies";

/** The near sea's WGSL: the shared head, the shoreline's library and the near sea's grid. */
export const OCEAN_NEAR_WGSL = `${OCEAN_SHARED_WGSL}\n${shorelineWgsl}\n${oceanNearWgsl}`;

/** The near sea's bindings in `@group(2)` past the shared ones (`oceanNear.wgsl`'s). */
export const OCEAN_NEAR_BINDINGS = {
  waves: 11,
  sceneDepth: 12,
  sceneColour: 13,
} as const;

/** The names a near draw binds its waves, the scene's depth bits and its colour under. */
export const OCEAN_NEAR_NAMES = {
  waves: "oceanWaves",
  sceneDepth: "oceanSceneDepth",
  sceneColour: "oceanSceneColour",
} as const;

/** The near draw's `Draw` members after `offsetFromCameraM`, each a `vec4f` (`oceanNear.wgsl`'s). */
const NEAR_MEMBERS = [
  "camera",
  "viewX",
  "viewY",
  "viewZ",
  "nearUp",
  "nearUpwind",
  "nearAcross",
  "nearGrid",
  "datum",
  "source0",
  "sourceLight0",
  "source1",
  "sourceLight1",
  "farFigure",
  "farSun",
  "farSunLux",
  "farSkyLux",
  "farIndex",
  "farAttenuation",
  "farDeep",
  "farBed",
] as const;

/**
 * The near sea's material: the projected grid, premultiplied into the scene target keeping its
 * meter class, writing its depth (`oceanNear.wgsl`).
 */
export const OCEAN_NEAR_PASS_MATERIAL: WgslMaterialSpec = {
  name: "ocean near pass",
  displayName: "OCEAN NEAR",
  vertexWgsl: OCEAN_NEAR_WGSL,
  fragmentWgsl: OCEAN_NEAR_WGSL,
  uniforms: NEAR_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [],
  textures: [
    { name: OCEAN_MAP_TEXTURE, binding: OCEAN_PASS_BINDINGS.oceanMap, viewDimension: "2d-array" },
    ...surfaceLightingTextures(OCEAN_PASS_BINDINGS.atmosphere),
    {
      name: OCEAN_NEAR_NAMES.sceneDepth,
      binding: OCEAN_NEAR_BINDINGS.sceneDepth,
      sampleType: "uint",
    },
    { name: OCEAN_NEAR_NAMES.sceneColour, binding: OCEAN_NEAR_BINDINGS.sceneColour },
  ],
  storageBuffers: [
    { name: OCEAN_ECLIPSE_BUFFER, binding: OCEAN_PASS_BINDINGS.eclipse },
    { name: OCEAN_NEAR_NAMES.waves, binding: OCEAN_NEAR_BINDINGS.waves },
  ],
  cullMode: "none",
  depthWrite: true,
  colourWrites: true,
  blend: "premultiplied",
};

const DEPTH_COPY_WGSL = `${frameWgsl}\n${oceanDepthCopyWgsl}`;

/** The scene's depth as its bits, a full-screen draw into an `r32uint` target. */
export const OCEAN_DEPTH_COPY_MATERIAL: WgslMaterialSpec = {
  name: "ocean depth copy",
  displayName: "OCEAN DEPTH COPY",
  vertexWgsl: DEPTH_COPY_WGSL,
  fragmentWgsl: DEPTH_COPY_WGSL,
  uniforms: [],
  samplers: [],
  textures: [{ name: "sceneDepth", binding: 0, sampleType: "depth" }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** No offset: the draw is the screen's. */
const NO_OFFSET = new Float32Array(3);

/** The depth copy's draw of `sceneDepth` on the full-screen `triangle`. */
export function oceanDepthCopyDraw(
  triangle: MeshHandle,
  material: MaterialHandle,
  sceneDepth: TextureHandle,
): DrawItem {
  return {
    mesh: triangle,
    material,
    offsetFromCameraM: NO_OFFSET,
    uniforms: {},
    textures: { sceneDepth },
  };
}

/** What a near sea's draw is formed from, beyond the far draw's view. */
export interface OceanNearView extends OceanDrawView {
  /** The scene's time on the universe clock, at which the waves stand. */
  readonly time: UniverseTime;
  /** The setting's `oceanComponents`. */
  readonly components: number;
  /** The grid's cell's angle, rad: its pixels times a pixel's angle, its displacement's filter. */
  readonly cellRad: number;
  /** Where the waves are laid, and their wind (`nearSeaAnchor`). */
  readonly anchor: OceanWaveAnchor;
  /** The setting's `oceanRefraction`. */
  readonly refraction: boolean;
}

/** A near sea's frame, set and waves this frame, and its peak's deep-water wavelength. */
export interface OceanNearState {
  readonly frame: OceanNearFrame;
  readonly set: GerstnerSet;
  readonly waves: ReadonlyArray<OceanWave>;
  readonly peakWavelengthM: number;
}

/**
 * A near sea's frame over the sea-level spheroid and its waves at the scene's time and the camera's
 * place (`oceanNear.ts`): the set at the anchor's wind, fixed by the sea state.
 */
export function oceanNearState(view: OceanNearView): OceanNearState {
  const optics = oceanDrawOptics(view);
  const frame = oceanNearFrame(optics, view.camera.positionM);
  const [east, north] = view.anchor.windEastNorthMps;
  const wind = Math.hypot(east, north);
  const set = oceanNearSet(view.sea, view.gravityMps2, wind, view.components);
  return {
    frame,
    set,
    waves: oceanWavesAt(set, view.time, view.anchor, view.camera.positionM),
    peakWavelengthM: oceanNearPeakWavelengthM(view.sea, view.gravityMps2, wind),
  };
}

/**
 * A near sea's draw's uniforms (`oceanNear.wgsl`'s `Draw`): the far draw's, its camera's w the
 * refraction, the stretched frame, |o′| and C₀ formed in `f64`, the anchor's wind frame and the
 * camera's height, and the grid's cell's angle.
 */
export function oceanNearUniforms(
  view: OceanNearView,
  frame: OceanNearFrame,
): Readonly<Record<string, Float32Array>> {
  const o = view.camera.positionM;
  const up = frame.stretchedUp;
  const { upwind, across } = view.anchor;
  return {
    ...oceanPassUniforms(view),
    camera: new Float32Array([o.x, o.y, o.z, view.refraction ? 1 : 0]),
    nearUp: new Float32Array([up.x, up.y, up.z, frame.stretchedDistanceM]),
    nearUpwind: new Float32Array([upwind.x, upwind.y, upwind.z, frame.heightM]),
    nearAcross: new Float32Array([across.x, across.y, across.z, frame.excessM2]),
    nearGrid: new Float32Array([view.cellRad, frame.stretch, 0, 0]),
  };
}

/** What a near draw binds beyond its uniforms. */
export interface OceanNearBindings {
  readonly oceanMap: TextureHandle;
  readonly tables: SurfaceLightingTables;
  readonly eclipseRows: BufferHandle;
  readonly waveRows: BufferHandle;
  /** The scene's depth bits, the copy's colour. */
  readonly sceneDepthBits: TextureHandle;
  /** The scene's colour copy, or the one-texel stand-in without refraction. */
  readonly sceneColour: TextureHandle;
}

/** A near sea's draw: the grid with its uniforms over its bindings. */
export function oceanNearDraw(
  grid: MeshHandle,
  material: MaterialHandle,
  bindings: OceanNearBindings,
  uniforms: Readonly<Record<string, Float32Array>>,
): DrawItem {
  return {
    mesh: grid,
    material,
    offsetFromCameraM: NO_OFFSET,
    uniforms,
    textures: {
      [OCEAN_MAP_TEXTURE]: bindings.oceanMap,
      ...surfaceLightingDrawTextures(bindings.tables),
      [OCEAN_NEAR_NAMES.sceneDepth]: bindings.sceneDepthBits,
      [OCEAN_NEAR_NAMES.sceneColour]: bindings.sceneColour,
    },
    storageBuffers: {
      [OCEAN_ECLIPSE_BUFFER]: bindings.eclipseRows,
      [OCEAN_NEAR_NAMES.waves]: bindings.waveRows,
    },
  };
}
