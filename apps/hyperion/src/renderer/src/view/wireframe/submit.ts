/**
 * The wireframe's submission through R01's engine adapter (plan R02, R02.T14): the one file in
 * `view/` outside `view/engine/` that calls the engine.
 *
 * @remarks
 * The draw list (R02.T13) is packed into four storage buffers, one per shader, and drawn in this
 * order: the bodies' occluder spheres and the hulls' opaque faces (each pushed away in its
 * fragment by the list's `occluderSlopePx`, R07.T16.d; depth only, or over the photorealistic image
 * a silhouette filled opaque in its mesh's `fill`, R07.T16.e), the bodies' surveyed terrain where a
 * view hands it over (plan R10, R10.T11: its depth, black, with its contours and graticule), the
 * star sprites (additive), then each line batch, its `--surface-0` casing first and its stroke over
 * it, both premultiplied over what is beneath. A body whose terrain is drawn has its occluder sphere
 * and its own lines discarded over its surveyed cells, its terrain standing there in their place. Sprites go before the lines, where the draw list lists them
 * after, so that a mark's casing covers a star beneath it, as the guide's casing rule wants of every
 * mark over the image. The shaders are standard WGSL in R01's convention (its Design note 23):
 * `frame.wgsl`'s `Frame` at `@group(0)`, each material's `Draw` at `@group(1)` (the draw's offset
 * from the camera, then the spec's uniforms in order), its storage buffer at `@group(2)`, and the
 * entry points `vertexMain` and `fragmentMain`. Every mesh's positions are the corners of the
 * instanced primitive, read as `@location(0)`.
 */

import { cross, norm, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { type ClusterConsumer, ClusterConsumerPath } from "../clusters/consumer";
import { CLUSTER_CULL_KERNEL } from "../clusters/cull";
import { CLUSTER_SHAPE_CODE } from "../clusters/record";
import { meshClusterMesh } from "../clusters/templates";
import { BUFFER_USAGE } from "../engine/gpuFlags";
import { EngineUnavailable } from "../engine/resilientEngine";
import { type GeometrySettings, SETTINGS } from "../quality/qualitySetting";
import { frustumOf } from "../terrain/cull";
import type {
  BufferHandle,
  ComputeHandle,
  DrawItem,
  FrameSubmission,
  MaterialHandle,
  MeshHandle,
  MeshSpec,
  RenderEngine,
  RenderView,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type ProjectionCamera,
  project,
  toViewAxes,
  type Viewport,
  viewRotation4,
} from "../camera/projection";
import cubeTexelWgsl from "../shaders/cubeTexel.wgsl?raw";
import frameWgsl from "../shaders/frame.wgsl?raw";
import lineStrokeWgsl from "../shaders/lineStroke.wgsl?raw";
import linesWgsl from "../shaders/lines.wgsl?raw";
import linesSurveyedWgsl from "../shaders/linesSurveyed.wgsl?raw";
import occluderWgsl from "../shaders/occluder.wgsl?raw";
import occluderClusterWgsl from "../shaders/occluderCluster.wgsl?raw";
import occluderSphereWgsl from "../shaders/occluderSphere.wgsl?raw";
import occluderSphereHitWgsl from "../shaders/occluderSphereHit.wgsl?raw";
import occluderSphereSurveyedWgsl from "../shaders/occluderSphereSurveyed.wgsl?raw";
import starSpriteWgsl from "../shaders/starSprite.wgsl?raw";
import toneCurveWgsl from "../shaders/toneCurve.wgsl?raw";
import wireCoverageWgsl from "../shaders/wireCoverage.wgsl?raw";
import type {
  DrawCamera,
  LineBatch,
  OccluderMesh,
  OccluderSphere,
  WireframeDrawList,
} from "./drawList";
import { HULL_CLUSTERS_PER_PAGE, HULL_INSTANCE_BYTES, hullClusterFrame } from "./hullClusters";
import type { DatumSpheroid } from "./spheroid";

/**
 * The wireframe's materials, by name: `occluderHull` a hull's faces as depth alone, and
 * `hullSilhouette` the same faces filled opaque over the photorealistic image (R07.T16.e); and,
 * for a body whose surveyed terrain the wireframe draws (plan R10, R10.T11), its occluder sphere and
 * its own lines, its limb and graticule at the datum, each discarded over the cells its coverage
 * mask holds surveyed (`occluderSphereSurveyed`, `linesSurveyed`).
 */
export type WireframeMaterial =
  | "lines"
  | "occluderSphere"
  | "occluderHull"
  | "hullSilhouette"
  | "starSprite"
  | "linesSurveyed"
  | "occluderSphereSurveyed";

/** The meshes the wireframe instances: a unit quad's two triangles, and one triangle. */
export type WireframeMesh = "quad" | "triangle";

/** The storage buffers the wireframe writes each frame, one per shader. */
export type WireframeBuffer = "segments" | "spheres" | "corners" | "sprites";

/** The pass's label in `PassTimes`, stable across frames (R12 keys its records on it). */
export const WIREFRAME_PASS_LABEL = "view:wireframe";

/** The sources, each composed after `frame.wgsl` by plain concatenation; no preprocessor. */
const SOURCES: Readonly<Record<WireframeMaterial, string>> = {
  lines: frameWgsl + lineStrokeWgsl + linesWgsl,
  occluderSphere: frameWgsl + occluderSphereHitWgsl + occluderSphereWgsl,
  occluderHull: frameWgsl + occluderWgsl,
  hullSilhouette: frameWgsl + occluderWgsl,
  starSprite: frameWgsl + toneCurveWgsl + starSpriteWgsl,
  linesSurveyed: frameWgsl + lineStrokeWgsl + cubeTexelWgsl + wireCoverageWgsl + linesSurveyedWgsl,
  occluderSphereSurveyed:
    frameWgsl +
    occluderSphereHitWgsl +
    cubeTexelWgsl +
    wireCoverageWgsl +
    occluderSphereSurveyedWgsl,
};

/** Which storage buffer each material reads, at `@group(2) @binding(0)`. */
export const MATERIAL_BUFFER: Readonly<Record<WireframeMaterial, WireframeBuffer>> = {
  lines: "segments",
  occluderSphere: "spheres",
  occluderHull: "corners",
  hullSilhouette: "corners",
  starSprite: "sprites",
  linesSurveyed: "segments",
  occluderSphereSurveyed: "spheres",
};

/**
 * Each material's display name, the effect's name on the console (decided 2026-10-02): what it
 * draws, in the guide's words.
 */
const DISPLAY_NAMES: Readonly<Record<WireframeMaterial, string>> = {
  lines: "WIREFRAME LINES",
  occluderSphere: "BODY OCCLUDER",
  occluderHull: "HULL OCCLUDER",
  hullSilhouette: "HULL SILHOUETTE",
  starSprite: "STAR SPRITES",
  linesSurveyed: "DATUM LINES",
  occluderSphereSurveyed: "DATUM OCCLUDER",
};

/**
 * The binding of a body's coverage mask in `@group(2)` (`wireCoverage.wgsl`), beside the
 * material's storage buffer at 0 (R10.T11).
 */
export const COVERAGE_MASK_BINDING = 1;

function spec(
  name: WireframeMaterial,
  state: Pick<WgslMaterialSpec, "uniforms" | "depthWrite" | "colourWrites" | "blend"> &
    Pick<Partial<WgslMaterialSpec>, "textures">,
): WgslMaterialSpec {
  return {
    name: `wireframe:${name}`,
    displayName: DISPLAY_NAMES[name],
    vertexWgsl: SOURCES[name],
    fragmentWgsl: SOURCES[name],
    samplers: [],
    // Two-sided everywhere: a winding flip between our right-handed matrices and an engine's
    // convention must not unhide every hidden line (Design note 5).
    cullMode: "none",
    storageBuffers: [{ name: MATERIAL_BUFFER[name], binding: 0 }],
    ...state,
  };
}

/** A surveyed body's mask, as both of its materials bind it (R10.T11). */
const COVERAGE_TEXTURES: NonNullable<WgslMaterialSpec["textures"]> = [
  { name: "coverageMask", binding: COVERAGE_MASK_BINDING, viewDimension: "2d-array" },
];

/**
 * Whether each material writes the depth that hides what follows it, so that its draws go before
 * the background and every line (R06.T13.g; R07.T16.e): the occluders and the hulls' silhouettes.
 */
const IS_OCCLUDER: Readonly<Record<WireframeMaterial, boolean>> = {
  lines: false,
  occluderSphere: true,
  occluderHull: true,
  hullSilhouette: true,
  starSprite: false,
  linesSurveyed: false,
  occluderSphereSurveyed: true,
};

/** The uniforms of a line's stroke, for both of its materials, in its `Draw` struct's order. */
const LINE_UNIFORMS: WgslMaterialSpec["uniforms"] = [
  { name: "colour", type: "vec4f" },
  { name: "widthPx", type: "f32" },
  { name: "firstSegment", type: "f32" },
  { name: "space", type: "f32" },
  { name: "dashOnPx", type: "f32" },
  { name: "dashOffPx", type: "f32" },
  { name: "ground", type: "vec4f" },
];

/** A hull face's uniforms, for both of its materials, which share one source. */
const HULL_UNIFORMS: WgslMaterialSpec["uniforms"] = [
  { name: "firstTriangle", type: "f32" },
  { name: "occluderSlopePx", type: "f32" },
  { name: "fill", type: "vec4f" },
];

/**
 * The wireframe's seven materials (Design notes 5, 9 and 12; the two over a body's surveyed
 * terrain, plan R10, R10.T11).
 *
 * @remarks
 * Lines and sprites are depth-tested and write no depth; the occluders write depth and no colour,
 * each its own depth from its fragment, pushed away by `occluderSlopePx` pixels of the depth's
 * screen slope (Design note 5; R07.T16.d). The hull's silhouette is its occluder with colour
 * writes on: opaque, its colour the draw's `fill` (R07.T16.e; decision-r07-t16a, item 3). No
 * material sets a hardware depth bias: it is pipeline state, which could not follow the display's
 * ratio. The uniforms are listed in the order of each shader's `Draw` struct, after its
 * `offsetFromCameraM`.
 */
export const WIREFRAME_MATERIALS: Readonly<Record<WireframeMaterial, WgslMaterialSpec>> = {
  lines: spec("lines", {
    uniforms: LINE_UNIFORMS,
    depthWrite: false,
    colourWrites: true,
    blend: "premultiplied",
  }),
  linesSurveyed: spec("linesSurveyed", {
    uniforms: [
      ...LINE_UNIFORMS,
      { name: "coverageAxes", type: "mat4x4f" },
      { name: "coverageStretch", type: "vec4f" },
      { name: "coverageCentreM", type: "vec4f" },
    ],
    textures: COVERAGE_TEXTURES,
    depthWrite: false,
    colourWrites: true,
    blend: "premultiplied",
  }),
  occluderSphereSurveyed: spec("occluderSphereSurveyed", {
    uniforms: [
      { name: "occluderSlopePx", type: "f32" },
      { name: "firstSphere", type: "f32" },
      { name: "coverageAxes", type: "mat4x4f" },
      { name: "coverageStretch", type: "vec4f" },
    ],
    textures: COVERAGE_TEXTURES,
    depthWrite: true,
    colourWrites: false,
    blend: "none",
  }),
  occluderSphere: spec("occluderSphere", {
    uniforms: [{ name: "occluderSlopePx", type: "f32" }],
    depthWrite: true,
    colourWrites: false,
    blend: "none",
  }),
  occluderHull: spec("occluderHull", {
    uniforms: HULL_UNIFORMS,
    depthWrite: true,
    colourWrites: false,
    blend: "none",
  }),
  hullSilhouette: spec("hullSilhouette", {
    uniforms: HULL_UNIFORMS,
    depthWrite: true,
    colourWrites: true,
    blend: "none",
  }),
  starSprite: spec("starSprite", {
    uniforms: [],
    depthWrite: false,
    colourWrites: true,
    blend: "additive",
  }),
};

/** The meshes: corner parameters, not positions, which the shaders read as `@location(0)`. */
export const WIREFRAME_MESHES: Readonly<Record<WireframeMesh, MeshSpec>> = {
  quad: {
    name: "wireframe:quad",
    // (0, 0) to (1, 1): a line's end and side, a sprite's or a sphere's rectangle corner.
    positions: new Float32Array([0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0]),
    indices: null,
    topology: "triangle-list",
    attributes: {},
  },
  triangle: {
    name: "wireframe:triangle",
    // The corner's index in its triangle.
    positions: new Float32Array([0, 0, 0, 1, 0, 0, 2, 0, 0]),
    indices: null,
    topology: "triangle-list",
    attributes: {},
  },
};

/** occluder.wgsl's instanced vertex stage's head, which the hulls' cluster forms take out. */
const OCCLUDER_VERTEX_HEAD = "@vertex\nfn vertexMain(";

/** occluder.wgsl's `Draw`'s last field and its end, after which a cluster form takes its base. */
const OCCLUDER_DRAW_END = "  fill: vec4f,\n}";

/**
 * A hull material's form on the cluster path (plan R14, R14.T4.b): `instanced`'s states, uniforms
 * and fragment stage, its `Draw` taking `clusterBase` after its own fields, its instanced vertex
 * stage taken out for `occluderCluster.wgsl`'s, which reads a bucket's list and the hulls'
 * instances.
 *
 * @throws Error unless occluder.wgsl's vertex stage and its `Draw`'s end each occur once in
 *   `instanced`.
 */
export function hullClusterMaterial(
  instanced: WgslMaterialSpec,
  displayName: string,
): WgslMaterialSpec {
  const source = instanced.vertexWgsl;
  const start = source.indexOf(OCCLUDER_VERTEX_HEAD);
  const end = start < 0 ? -1 : source.indexOf("\n}\n", start);
  const draws = source.split(OCCLUDER_DRAW_END);
  if (
    start < 0 ||
    end < 0 ||
    source.indexOf(OCCLUDER_VERTEX_HEAD, start + 1) >= 0 ||
    draws.length !== 2
  ) {
    throw new Error(
      `a hull's cluster form needs occluder.wgsl's vertex stage and Draw once each in ${instanced.name}`,
    );
  }
  const withoutVertex = source.slice(0, start) + source.slice(end + 3);
  const withBase = withoutVertex.replace(
    OCCLUDER_DRAW_END,
    "  fill: vec4f,\n  // The bucket's first entry in the lists (`clusters/buckets.ts`' `clusterBase`).\n  clusterBase: u32,\n}",
  );
  const wgsl = `${withBase}\n${occluderClusterWgsl}`;
  return {
    ...instanced,
    name: `${instanced.name}Cluster`,
    displayName,
    vertexWgsl: wgsl,
    fragmentWgsl: wgsl,
    uniforms: [...instanced.uniforms, { name: "clusterBase", type: "u32" }],
    storageBuffers: [
      ...(instanced.storageBuffers ?? []),
      { name: "hullInstances", binding: 1, stages: ["vertex"] },
      { name: "clusterLists", binding: 2, stages: ["vertex"] },
    ],
  };
}

/** The hulls' two materials on the cluster path (plan R14, R14.T4.b): depth alone, and filled. */
export const HULL_CLUSTER_MATERIALS: Readonly<
  Record<"occluderHull" | "hullSilhouette", WgslMaterialSpec>
> = {
  occluderHull: hullClusterMaterial(WIREFRAME_MATERIALS.occluderHull, "HULL OCCLUDER CLUSTERS"),
  hullSilhouette: hullClusterMaterial(WIREFRAME_MATERIALS.hullSilhouette, "SILHOUETTE CLUSTERS"),
};

/** The mesh shape's cluster mesh and its one template, which the hulls' clusters draw by. */
export const HULL_CLUSTER_MESH = meshClusterMesh();

/**
 * The hulls' occluder faces as a consumer of the cluster path (plan R14, R14.T4.b;
 * `hullClusters.ts`): pages of {@link HULL_CLUSTERS_PER_PAGE} mesh clusters, the DAG-cut mode, one
 * template.
 */
export const HULL_CLUSTER_CONSUMER: ClusterConsumer = {
  name: "wireframe:hull clusters",
  templates: HULL_CLUSTER_MESH.templates,
  clustersPerPage: HULL_CLUSTERS_PER_PAGE,
  firstShape: CLUSTER_SHAPE_CODE.mesh,
  templateClusters: [HULL_CLUSTERS_PER_PAGE],
  instanceBytes: HULL_INSTANCE_BYTES,
  vertexWgsl: occluderClusterWgsl,
};

/** The hulls' cull's timed label (plan R14, R14.T4.b): one dispatch a frame with a hull drawn. */
export const HULL_CLUSTER_CULL_LABEL = "hull cluster cull";

/**
 * How far each side of the hulls' frustum is opened for their cull, rad: past the `f32` rounding of
 * the planes, as the terrain's overlay's is, so that no cluster that draws is refused.
 */
const HULL_CULL_MARGIN_RAD = 0.01;

/** One draw of the packed frame, before it is bound to the engine's handles. */
export interface PackedDraw {
  readonly material: WireframeMaterial;
  readonly mesh: WireframeMesh;
  readonly instanceCount: number;
  readonly offsetFromCameraM: Float32Array;
  readonly uniforms: Readonly<Record<string, Float32Array>>;
  /**
   * The body whose coverage mask the draw binds (R10.T11): a surveyed body's occluder sphere or
   * one of its own lines; absent for every other draw.
   */
  readonly surveyedBody?: string;
}

/**
 * A body whose datum the wireframe draws beside its surveyed terrain (plan R10, R10.T11), as the
 * pack takes it: its occluder sphere and its own lines, its limb and graticule, are discarded where
 * its coverage mask holds a cell surveyed.
 */
export interface SurveyedDatum {
  /** Its body-fixed axes into the camera frame's, as a column-major `mat4x4f` (`rotationColumns`). */
  readonly axes: Float32Array;
  /** Its equatorial radius over its polar, a ÷ c, by which a direction is stretched along its pole. */
  readonly stretch: number;
}

/** A surveyed body's datum with its coverage mask, as the frame binds it (R10.T11). */
export interface SurveyedDatumFrame extends SurveyedDatum {
  /** Its GPU coverage mask (`TerrainSession.coverageMask`), given only while it may discard. */
  readonly mask: TextureHandle;
}

/**
 * What the wireframe draws of the bodies' terrain in one frame (plan R10, R10.T11): each handed-over
 * body's depth with its contours, drawn after the occluders and before the background and the
 * lines, so that it hides what lies behind it; and the bodies whose datum gives way to it.
 */
export interface WireframeTerrain {
  readonly draws: ReadonlyArray<DrawItem>;
  readonly surveyed: ReadonlyMap<string, SurveyedDatumFrame>;
}

/** No terrain: the wireframe as R02 built it. */
export const NO_WIREFRAME_TERRAIN: WireframeTerrain = { draws: [], surveyed: new Map() };

/** One frame of the wireframe, packed for the GPU: the buffers' contents and the draws. */
export interface PackedWireframe {
  /** Two `vec4f` per segment: one end and the dash's phase there, px; the other end and 0. */
  readonly segments: Float32Array;
  /**
   * {@link OCCLUDER_SPHERE_ROWS} `vec4f` per sphere: its rectangle, px; its centre and radius, m;
   * its altitude, m, and {@link spheroidAxis} of its spheroid along the camera frame's axes, 0 for
   * a sphere (R02.T19).
   */
  readonly spheres: Float32Array;
  /** One `vec4f` per corner of each hull triangle, m from the craft. */
  readonly corners: Float32Array;
  /** Two `vec4f` per sprite: its position, px; its pre-exposed linear colour. */
  readonly sprites: Float32Array;
  readonly draws: ReadonlyArray<PackedDraw>;
}

/** The sRGB transfer function's inverse (IEC 61966-2-1), from an encoded value in [0, 1]. */
function srgbToLinear(encoded: number): number {
  return encoded <= 0.040_45 ? encoded / 12.92 : ((encoded + 0.055) / 1.055) ** 2.4;
}

/**
 * A colour token's value as a linear RGBA, alpha 1, so that the canvas's sRGB view encodes it back
 * to the token's own value.
 *
 * @param css - `#rgb`, `#rrggbb` or `rgb(r, g, b)`, as `readTokens` reads the guide's tokens.
 * @throws Error on any other form.
 */
export function linearColour(css: string): Float32Array {
  const text = css.trim();
  let channels: number[] | null = null;
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(text)?.[1];
  if (hex !== undefined) {
    const full = hex.length === 3 ? hex.replace(/./g, (c) => c + c) : hex;
    channels = [0, 2, 4].map((i) => Number.parseInt(full.slice(i, i + 2), 16));
  } else {
    const rgb = /^rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)$/i.exec(text);
    if (rgb !== null) {
      channels = [rgb[1], rgb[2], rgb[3]].map((c) => Number(c));
    }
  }
  if (channels === null || channels.some((c) => !(c >= 0 && c <= 255))) {
    throw new Error(`the colour ${css} is not #rgb, #rrggbb or rgb(r, g, b)`);
  }
  return new Float32Array([...channels.map((c) => srgbToLinear(c / 255)), 1]);
}

/** A rectangle on the view, px. */
export interface ScreenRect {
  readonly leftPx: number;
  readonly topPx: number;
  readonly rightPx: number;
  readonly bottomPx: number;
}

/** The silhouette's bounding polygon's sides. */
const SILHOUETTE_SIDES = 16;

/** The rectangle's margin beyond the silhouette's bound, px. */
const SILHOUETTE_MARGIN_PX = 2;

/**
 * The screen rectangle that holds a sphere's silhouette, clamped to the view; the whole view where
 * the silhouette reaches behind the near plane; `null` where it is off the view.
 *
 * @remarks
 * The silhouette is the circle of tangency, at distance D cos²α along the centre's direction with
 * radius r cos α (sin α = r ÷ D); a regular polygon circumscribing it in its plane contains it, so
 * the bounding box of the polygon's projected corners contains the silhouette's projection.
 *
 * @param centreM - The sphere's centre from the camera, m, along the frame's axes.
 */
export function sphereScreenRect(
  centreM: Vec3,
  radiusM: number,
  camera: ProjectionCamera,
  viewport: Viewport,
): ScreenRect | null {
  const whole: ScreenRect = {
    leftPx: 0,
    topPx: 0,
    rightPx: viewport.widthPx,
    bottomPx: viewport.heightPx,
  };
  const distanceM = norm(centreM);
  if (!(distanceM > radiusM)) {
    return whole;
  }
  const axis = vec3(centreM.x / distanceM, centreM.y / distanceM, centreM.z / distanceM);
  const sinA = radiusM / distanceM;
  const cosA = Math.sqrt(1 - sinA * sinA);
  const along = distanceM * cosA * cosA;
  const circleM = (radiusM * cosA) / Math.cos(Math.PI / SILHOUETTE_SIDES);
  // The seed leaves x for y where the axis lies near x, so that the cross product never vanishes.
  const seed = Math.abs(axis.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  const u = normalise(cross(axis, seed));
  const v = cross(axis, u);
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (let i = 0; i < SILHOUETTE_SIDES; i += 1) {
    const t = (2 * Math.PI * i) / SILHOUETTE_SIDES;
    const c = Math.cos(t) * circleM;
    const s = Math.sin(t) * circleM;
    const corner = vec3(
      axis.x * along + u.x * c + v.x * s,
      axis.y * along + u.y * c + v.y * s,
      axis.z * along + u.z * c + v.z * s,
    );
    const p = project(corner, camera, viewport);
    if (!p.inFront) {
      return whole;
    }
    left = Math.min(left, p.xPx);
    top = Math.min(top, p.yPx);
    right = Math.max(right, p.xPx);
    bottom = Math.max(bottom, p.yPx);
  }
  const rect: ScreenRect = {
    leftPx: Math.max(0, Math.floor(left) - SILHOUETTE_MARGIN_PX),
    topPx: Math.max(0, Math.floor(top) - SILHOUETTE_MARGIN_PX),
    rightPx: Math.min(viewport.widthPx, Math.ceil(right) + SILHOUETTE_MARGIN_PX),
    bottomPx: Math.min(viewport.heightPx, Math.ceil(bottom) + SILHOUETTE_MARGIN_PX),
  };
  return rect.leftPx < rect.rightPx && rect.topPx < rect.bottomPx ? rect : null;
}

/**
 * How far past each side of the view {@link sphereOutsideView} looks, px on the image plane.
 *
 * @remarks
 * A disc draws a pixel only where its corners' mean limb angle is at most 0.75 times their gradient
 * (`OUTSIDE_PX`, `bodyDisc.wgsl`). The gradient is at most √2 times the larger angle a pixel's side
 * subtends there, so a drawn pixel lies within 1.06 of those angles of the limb. A ray through the
 * view stands at least m cos(φ′ ÷ 2) of them off a side plane widened by m pixels to a field φ′ on
 * that axis: 3.35 for 8 px on a 64 px side at 120°. An oblate body's scaled space may shrink the one
 * angle and grow the other by a ÷ c each, 1.56 together at the record's cap of f = 0.2. So on sides
 * of 64 px or more, up to 120° on each axis, 8 px clears the 1.06 three times over for a sphere and
 * twice over at f = 0.2. A side whose field passes 120°, a tall view's height, is outside this
 * bound; the disc's twin draws no pixel there either, with fields to 144° and f = 0.2
 * (`regime.test.ts`).
 *
 * R06's host disc and the occluder sphere light or write a pixel only where the ray through the
 * pixel's centre meets the sphere, and the outermost centres lie half a pixel inside each side, so
 * they need no margin but for their shaders' `f32` (under 10⁻⁶ rad, a pixel at 4K across 10° being
 * 4.5 × 10⁻⁵ rad). This one serves them with room to spare (`disc.test.ts`, `submit.test.ts`).
 */
export const OUTSIDE_VIEW_MARGIN_PX = 8;

/**
 * Whether a sphere stands wholly beyond one of a view's four side planes, each widened by
 * {@link OUTSIDE_VIEW_MARGIN_PX}: no ray through the view then meets it or passes near enough its
 * limb to draw a pixel, wherever it stands. It holds behind the camera and across the camera's
 * plane, where {@link sphereScreenRect} gives the whole view.
 *
 * @remarks
 * Nothing is drawn for such a sphere: no occluder sphere ({@link packWireframe}), no lit body's
 * disc and no footprint for promotion (R07's `bodies/draw.ts` and `bodies/regime.ts`), and no host
 * disc (R06's `sky/disc.ts`; R07.T19.e).
 *
 * @param centreM - The sphere's centre from the camera, m (`f64`).
 */
export function sphereOutsideView(
  centreM: Vec3,
  radiusM: number,
  camera: ProjectionCamera,
  viewport: Viewport,
): boolean {
  const view = toViewAxes(centreM, camera.orientation);
  const tanHalf = Math.tan(camera.fovXRad / 2);
  const marginTan = (OUTSIDE_VIEW_MARGIN_PX * 2 * tanHalf) / viewport.widthPx;
  const tanX = tanHalf + marginTan;
  const tanY = (tanHalf * viewport.heightPx) / viewport.widthPx + marginTan;
  // The view looks down −z: inside the side planes |x| ≤ tanX (−z) and |y| ≤ tanY (−z). Each is the
  // centre's signed distance beyond the plane of its pair on the centre's side, the larger.
  const beyondSideM = (Math.abs(view.x) + tanX * view.z) / Math.hypot(1, tanX);
  const beyondTopM = (Math.abs(view.y) + tanY * view.z) / Math.hypot(1, tanY);
  return Math.max(beyondSideM, beyondTopM) > radiusM;
}

/** Where a batch's point lands, px: projected for a view batch, as given for a screen batch. */
function screenPoint(
  batch: LineBatch,
  p: Float32Array,
  at: number,
  camera: ProjectionCamera,
  viewport: Viewport,
): { readonly xPx: number; readonly yPx: number } | null {
  const x = p[at] ?? 0;
  const y = p[at + 1] ?? 0;
  const z = p[at + 2] ?? 0;
  if (batch.space === "screen") {
    return { xPx: x, yPx: y };
  }
  const o = batch.originF32;
  const projected = project(
    vec3((o[0] ?? 0) + x, (o[1] ?? 0) + y, (o[2] ?? 0) + z),
    camera,
    viewport,
  );
  return projected.inFront ? { xPx: projected.xPx, yPx: projected.yPx } : null;
}

/**
 * The dash's phase at each segment's first end, px: the screen length of the polyline before it,
 * so that a dash runs on across joints and does not crawl; 0 at a polyline's start, or where an
 * end is behind the camera.
 */
function dashPhases(batch: LineBatch, camera: ProjectionCamera, viewport: Viewport): number[] {
  const count = batch.segments.length / 6;
  const phases: number[] = [];
  let phase = 0;
  for (let i = 0; i < count; i += 1) {
    const s = batch.segments;
    const joined =
      i > 0 &&
      s[i * 6] === s[i * 6 - 3] &&
      s[i * 6 + 1] === s[i * 6 - 2] &&
      s[i * 6 + 2] === s[i * 6 - 1];
    if (!joined) {
      phase = 0;
    }
    phases.push(phase);
    const a = screenPoint(batch, s, i * 6, camera, viewport);
    const b = screenPoint(batch, s, i * 6 + 3, camera, viewport);
    phase = a === null || b === null ? 0 : phase + Math.hypot(b.xPx - a.xPx, b.yPx - a.yPx);
  }
  return phases;
}

const ZERO_OFFSET = new Float32Array(3);

/** A hull cluster draw's `firstTriangle`, unread: its entries name their hull and triangles. */
const ZERO_TRIANGLE = new Float32Array(1);

/**
 * How many `vec4f` rows an occluder sphere takes in {@link PackedWireframe.spheres}: 3, as
 * `occluderSphereHit.wgsl`'s `SPHERE_ROWS` reads them (R02.T19). A spheroid's axis rides in its
 * third row's spare lanes ({@link spheroidAxis}), so that the stride is a sphere's.
 */
export const OCCLUDER_SPHERE_ROWS = 3;

/**
 * A spheroid's axis as an occluder sphere's third row carries it (plan R02, R02.T19): its pole times
 * √(s − 1), s its stretch a′ ÷ c′, so that the shader's stretch is L v = v + (v · q) q and
 * L² v = v + (s + 1)(v · q) q, s + 1 = 2 + |q|²; the zero vector for a sphere, which the shader
 * then intersects by the sphere's own sums.
 */
export function spheroidAxis(spheroid: DatumSpheroid | undefined): Vec3 {
  return spheroid === undefined
    ? vec3(0, 0, 0)
    : scale(spheroid.pole, Math.sqrt(spheroid.stretch - 1));
}

/** No body's datum gives way to its terrain. */
const NO_SURVEYED_DATUM: ReadonlyMap<string, SurveyedDatum> = new Map();

/**
 * Packs one frame of the draw list for the GPU (plan R02, R02.T14): a function of its arguments
 * alone, so that what reaches the engine is tested without a GPU.
 *
 * @param surveyed - The bodies whose datum gives way to their surveyed terrain (plan R10, R10.T11):
 *   each one's occluder sphere is its own draw of `occluderSphereSurveyed`, after the others in the
 *   spheres' buffer, and each of its own lines (`LineBatch.body`) is drawn with `linesSurveyed`,
 *   both discarding where its coverage mask holds a cell surveyed.
 */
export function packWireframe(
  list: WireframeDrawList,
  camera: DrawCamera,
  viewport: Viewport,
  surveyed: ReadonlyMap<string, SurveyedDatum> = NO_SURVEYED_DATUM,
): PackedWireframe {
  const projection: ProjectionCamera = {
    orientation: camera.pose.orientation,
    fovXRad: camera.fovXRad,
  };
  const draws: PackedDraw[] = [];
  const occluderSlopePx = new Float32Array([list.occluderSlopePx]);

  const sphereRows: number[] = [];
  let sphereCount = 0;
  // A surveyed body's sphere, packed after the others: its index, and its centre from the camera.
  const surveyedSpheres: Array<{ readonly body: string; readonly index: number }> = [];
  const surveyedCentres = new Map<string, Float32Array>();
  for (const pass of ["plain", "surveyed"] as const) {
    for (const sphere of list.occluderSpheres) {
      if ((pass === "surveyed") !== surveyed.has(sphere.id)) {
        continue;
      }
      if (pass === "surveyed") {
        surveyedCentres.set(sphere.id, sphere.centreF32);
      }
      const centre = centreOf(sphere);
      // Behind the camera or across its plane the rectangle is the whole view, each fragment
      // rejecting itself (R07.T19.e).
      if (sphereOutsideView(centre, sphere.radiusM, projection, viewport)) {
        continue;
      }
      const rect = sphereScreenRect(centre, sphere.radiusM, projection, viewport);
      if (rect === null) {
        continue;
      }
      const c = sphere.centreF32;
      const axis = spheroidAxis(sphere.spheroid);
      sphereRows.push(rect.leftPx, rect.topPx, rect.rightPx, rect.bottomPx);
      sphereRows.push(c[0] ?? 0, c[1] ?? 0, c[2] ?? 0, sphere.radiusM);
      sphereRows.push(sphere.altitudeM, axis.x, axis.y, axis.z);
      if (pass === "surveyed") {
        surveyedSpheres.push({ body: sphere.id, index: sphereCount });
      }
      sphereCount += 1;
    }
  }
  const plainSpheres = sphereCount - surveyedSpheres.length;
  if (plainSpheres > 0) {
    draws.push({
      material: "occluderSphere",
      mesh: "quad",
      instanceCount: plainSpheres,
      offsetFromCameraM: ZERO_OFFSET,
      uniforms: { occluderSlopePx },
    });
  }
  for (const { body, index } of surveyedSpheres) {
    const datum = surveyed.get(body);
    if (datum === undefined) {
      continue;
    }
    draws.push({
      material: "occluderSphereSurveyed",
      mesh: "quad",
      instanceCount: 1,
      offsetFromCameraM: ZERO_OFFSET,
      uniforms: {
        occluderSlopePx,
        firstSphere: new Float32Array([index]),
        coverageAxes: datum.axes,
        coverageStretch: new Float32Array([datum.stretch, 0, 0, 0]),
      },
      surveyedBody: body,
    });
  }

  const cornerRows: number[] = [];
  for (const mesh of list.occluderMeshes) {
    const triangles = mesh.triangles.length / 9;
    if (triangles === 0) {
      continue;
    }
    const firstTriangle = cornerRows.length / 12;
    for (let i = 0; i < triangles * 3; i += 1) {
      cornerRows.push(
        mesh.triangles[i * 3] ?? 0,
        mesh.triangles[i * 3 + 1] ?? 0,
        mesh.triangles[i * 3 + 2] ?? 0,
        0,
      );
    }
    // Each hull is its own draw at its own origin, reading its range of the one buffer: depth
    // alone, or a silhouette filled opaque in its colour (R07.T16.e).
    const first = new Float32Array([firstTriangle]);
    draws.push(
      mesh.fill === null
        ? {
            material: "occluderHull",
            mesh: "triangle",
            instanceCount: triangles,
            offsetFromCameraM: mesh.originF32,
            uniforms: { firstTriangle: first, occluderSlopePx },
          }
        : {
            material: "hullSilhouette",
            mesh: "triangle",
            instanceCount: triangles,
            offsetFromCameraM: mesh.originF32,
            uniforms: { firstTriangle: first, occluderSlopePx, fill: linearColour(mesh.fill) },
          },
    );
  }

  const spriteRows: number[] = [];
  for (const sprite of list.sprites) {
    spriteRows.push(sprite.xPx, sprite.yPx, 0, 0, ...sprite.exposedRgb, 0);
  }
  if (list.sprites.length > 0) {
    draws.push({
      material: "starSprite",
      mesh: "quad",
      instanceCount: list.sprites.length,
      offsetFromCameraM: ZERO_OFFSET,
      uniforms: {},
    });
  }

  const segmentRows: number[] = [];
  for (const batch of list.lines) {
    const count = batch.segments.length / 6;
    if (count === 0) {
      continue;
    }
    const first = segmentRows.length / 8;
    const phases = batch.dash === null ? null : dashPhases(batch, projection, viewport);
    for (let i = 0; i < count; i += 1) {
      const s = batch.segments;
      segmentRows.push(s[i * 6] ?? 0, s[i * 6 + 1] ?? 0, s[i * 6 + 2] ?? 0, phases?.[i] ?? 0);
      segmentRows.push(s[i * 6 + 3] ?? 0, s[i * 6 + 4] ?? 0, s[i * 6 + 5] ?? 0, 0);
    }
    // A surveyed body's own lines give way to its terrain (R10.T11).
    const datum = batch.body === undefined ? undefined : surveyed.get(batch.body);
    const centre = batch.body === undefined ? undefined : surveyedCentres.get(batch.body);
    const stroke = (colour: string, widthPx: number): PackedDraw => {
      const uniforms = {
        colour: linearColour(colour),
        widthPx: new Float32Array([widthPx]),
        firstSegment: new Float32Array([first]),
        space: new Float32Array([batch.space === "screen" ? 1 : 0]),
        dashOnPx: new Float32Array([batch.dash?.onPx ?? 0]),
        dashOffPx: new Float32Array([batch.dash?.offPx ?? 0]),
        // A batch on a body's ground pulls its depth towards the camera (R10.T16.c); w 0 for none.
        ground: groundOf(batch, list.occluderSlopePx),
      };
      const offsetFromCameraM = batch.space === "view" ? batch.originF32 : ZERO_OFFSET;
      if (batch.body === undefined || datum === undefined || centre === undefined) {
        return {
          material: "lines",
          mesh: "quad",
          instanceCount: count,
          offsetFromCameraM,
          uniforms,
        };
      }
      return {
        material: "linesSurveyed",
        mesh: "quad",
        instanceCount: count,
        offsetFromCameraM,
        uniforms: {
          ...uniforms,
          coverageAxes: datum.axes,
          coverageStretch: new Float32Array([datum.stretch, 0, 0, 0]),
          coverageCentreM: new Float32Array([centre[0] ?? 0, centre[1] ?? 0, centre[2] ?? 0, 0]),
        },
        surveyedBody: batch.body,
      };
    };
    if (batch.casingWidthPx > 0) {
      draws.push(stroke(batch.casingColour, batch.widthPx + 2 * batch.casingWidthPx));
    }
    draws.push(stroke(batch.colour, batch.widthPx));
  }

  return {
    segments: new Float32Array(segmentRows),
    spheres: new Float32Array(sphereRows),
    corners: new Float32Array(cornerRows),
    sprites: new Float32Array(spriteRows),
    draws,
  };
}

/**
 * A line batch's `ground` uniform (R10.T16.c; `lineStroke.wgsl`'s `line_depth`): the centre of the
 * body whose ground it is drawn on and the slope term it is pulled by, or zeros for a batch off the
 * ground.
 */
function groundOf(batch: LineBatch, occluderSlopePx: number): Float32Array {
  const centre = batch.groundCentreF32;
  if (centre === undefined || batch.space !== "view") {
    return new Float32Array(4);
  }
  return Float32Array.from([centre[0] ?? 0, centre[1] ?? 0, centre[2] ?? 0, occluderSlopePx]);
}

function centreOf(sphere: OccluderSphere): Vec3 {
  const c = sphere.centreF32;
  return vec3(c[0] ?? 0, c[1] ?? 0, c[2] ?? 0);
}

/** The smallest storage buffer made, bytes; each grows by doubling. */
const MIN_BUFFER_BYTES = 4_096;

/** Whether `material` draws a hull's faces, depth alone or filled. */
function isHull(material: WireframeMaterial): boolean {
  return material === "occluderHull" || material === "hullSilhouette";
}

/** The hulls' cluster path's buffers (plan R14, R14.T4.b), each grown by doubling. */
type HullClusterBuffer = "records" | "lods" | "instances";

/** What the renderer holds on the engine's device, made again after a device loss. */
interface DeviceResources {
  readonly materials: Readonly<Record<WireframeMaterial, MaterialHandle>>;
  readonly meshes: Readonly<Record<WireframeMesh, MeshHandle>>;
  readonly buffers: Record<WireframeBuffer, BufferHandle>;
  /**
   * The hulls' cluster path (plan R14, R14.T4.b): its two materials, its cluster mesh, its path's
   * buckets, its records', LOD spheres' and instances' buffers, and the cull's kernel once made,
   * `null` before and where it could not be, when every hull draws instanced.
   */
  readonly hullMaterials: Readonly<Record<"occluderHull" | "hullSilhouette", MaterialHandle>>;
  readonly hullMesh: MeshHandle;
  readonly hullClusters: ClusterConsumerPath;
  readonly hullBuffers: Record<HullClusterBuffer, BufferHandle>;
  cullKernel: ComputeHandle | null;
}

/** The part of the engine the renderer uses. */
export type WireframeEngine = Pick<
  RenderEngine,
  | "createMaterial"
  | "createMesh"
  | "createBuffer"
  | "writeBuffer"
  | "releaseBuffer"
  | "dispatch"
  | "createComputeAsync"
  | "releaseCompute"
  | "onRestored"
>;

/**
 * Draws wireframe draw lists into a view through the engine (plan R02, R02.T14).
 *
 * @remarks
 * It makes its materials, meshes and storage buffers on construction and again after the engine
 * restores a lost device (`RenderEngine.onRestored`), and grows a buffer by doubling when a frame
 * outgrows it. An outgrown frame buffer is left to the engine; doubling bounds the waste to the
 * largest frame's size. The hulls' faces draw through the cluster path (plan R14, R14.T4.b) where
 * its geometry asks it ({@link WireframeRenderer.setGeometry}) and the cull's kernel is made; a
 * frame's submission must be rendered before the next frame is asked for, since its buckets, as
 * its other buffers, are the renderer's one set.
 */
export class WireframeRenderer {
  readonly #engine: WireframeEngine;
  #resources: DeviceResources;
  /** How the hulls' faces are drawn (plan R14): the high setting's until set. */
  #geometry: GeometrySettings = SETTINGS.high.geometry;
  /** Settles once the current device's cull kernel is made, or could not be (R14.T4.b). */
  #cullReady: Promise<boolean> = Promise.resolve(false);
  /** The last frame's hulls on the cluster path: their clusters and triangles, or `null`. */
  #lastHulls: { readonly clusters: number; readonly triangles: number } | null = null;
  #disposed = false;
  readonly #unsubscribe: () => void;

  constructor(engine: WireframeEngine) {
    this.#engine = engine;
    this.#resources = this.#create();
    this.#readyCull(this.#resources);
    this.#unsubscribe = engine.onRestored(() => {
      this.#resources = this.#create();
      this.#readyCull(this.#resources);
    });
  }

  /** How it draws the hulls' faces (plan R14, R14.T4.b): the high setting's until set. */
  get geometry(): GeometrySettings {
    return this.#geometry;
  }

  /**
   * Draws the hulls' faces by `geometry` from the next frame on: on the cluster path where its
   * `clusterPath` and `hullClusters` are on (plan R14, R14.T4.b), else instanced, as before.
   */
  setGeometry(geometry: GeometrySettings): void {
    this.#geometry = geometry;
  }

  /**
   * Settles once the hulls' cluster cull's kernel is made on the current device (plan R14,
   * R14.T4.b), `true`, or could not be, `false`; until then every hull draws instanced.
   */
  clustersReady(): Promise<boolean> {
    return this.#cullReady;
  }

  /**
   * The hulls' buckets as the last frame's cull left them (plan R14, R14.T4.b), one pass, for the
   * counters' reduction (`clusters/stats.ts`), and that frame's clusters and faces; `null` where
   * its hulls drew instanced.
   */
  get lastHullClusters(): {
    readonly buckets: NonNullable<ClusterConsumerPath["buckets"]>;
    readonly clusters: number;
    readonly triangles: number;
  } | null {
    const buckets = this.#resources.hullClusters.buckets;
    return this.#lastHulls === null || buckets === null ? null : { buckets, ...this.#lastHulls };
  }

  /**
   * Compiles the cluster cull's kernel for `resources`' device (plan R14, R14.T4.b), off the frame;
   * never rejects. Until it is made, or where it cannot be, every hull draws instanced.
   */
  #readyCull(resources: DeviceResources): void {
    // A refusal thrown at the call, as a lost device's, settles as one rejected.
    this.#cullReady = (async () => this.#engine.createComputeAsync(CLUSTER_CULL_KERNEL))().then(
      (kernel) => {
        if (this.#resources === resources && !this.#disposed && resources.cullKernel === null) {
          resources.cullKernel = kernel;
          return true;
        }
        this.#engine.releaseCompute(kernel);
        return false;
      },
      (error: unknown) => {
        if (!(error instanceof EngineUnavailable)) {
          console.error("the hulls' cluster cull failed: every hull draws instanced", error);
        }
        return false;
      },
    );
  }

  #create(): DeviceResources {
    const engine = this.#engine;
    const material = (name: WireframeMaterial): MaterialHandle =>
      engine.createMaterial(WIREFRAME_MATERIALS[name]);
    const buffer = (name: WireframeBuffer): BufferHandle => this.#buffer(name, MIN_BUFFER_BYTES);
    return {
      materials: {
        lines: material("lines"),
        occluderSphere: material("occluderSphere"),
        occluderHull: material("occluderHull"),
        hullSilhouette: material("hullSilhouette"),
        starSprite: material("starSprite"),
        linesSurveyed: material("linesSurveyed"),
        occluderSphereSurveyed: material("occluderSphereSurveyed"),
      },
      meshes: {
        quad: engine.createMesh(WIREFRAME_MESHES.quad),
        triangle: engine.createMesh(WIREFRAME_MESHES.triangle),
      },
      buffers: {
        segments: buffer("segments"),
        spheres: buffer("spheres"),
        corners: buffer("corners"),
        sprites: buffer("sprites"),
      },
      hullMaterials: {
        occluderHull: engine.createMaterial(HULL_CLUSTER_MATERIALS.occluderHull),
        hullSilhouette: engine.createMaterial(HULL_CLUSTER_MATERIALS.hullSilhouette),
      },
      hullMesh: engine.createMesh(HULL_CLUSTER_MESH.mesh),
      hullClusters: new ClusterConsumerPath(engine, HULL_CLUSTER_CONSUMER),
      hullBuffers: {
        records: this.#clusterBuffer("records", MIN_BUFFER_BYTES),
        lods: this.#clusterBuffer("lods", MIN_BUFFER_BYTES),
        instances: this.#clusterBuffer("instances", MIN_BUFFER_BYTES),
      },
      cullKernel: null,
    };
  }

  /** A buffer of the hulls' cluster path (plan R14, R14.T4.b): `geometry-clusters`. */
  #clusterBuffer(name: HullClusterBuffer, bytes: number): BufferHandle {
    return this.#engine.createBuffer({
      name: `wireframe:hull cluster ${name}`,
      bytes,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "geometry-clusters",
    });
  }

  /** Writes `data` into a cluster buffer, replacing it by a doubled one if it is too small. */
  #writeCluster(name: HullClusterBuffer, data: ArrayBuffer): BufferHandle {
    const buffers = this.#resources.hullBuffers;
    let handle = buffers[name];
    if (data.byteLength > handle.bytes) {
      let bytes = handle.bytes;
      while (bytes < data.byteLength) {
        bytes *= 2;
      }
      this.#engine.releaseBuffer(handle);
      handle = this.#clusterBuffer(name, bytes);
      buffers[name] = handle;
    }
    this.#engine.writeBuffer(handle, 0, new Uint8Array(data));
    return handle;
  }

  /**
   * The hulls' draws on the cluster path (plan R14, R14.T4.b), culled now, or `null` where they
   * draw instanced: the geometry's way back, a kernel not yet made, no hull, hulls of more than one
   * fill or kind, or a hull past a page.
   *
   * @remarks
   * Every hull is one instance of the cull's one camera pass, its frustum the frame's camera's in
   * the camera frame's axes, each side opened by {@link HULL_CULL_MARGIN_RAD}, at the finest cut
   * (τ 0: `meshTolerancePx` is plan R14's T5), its clusters appended, and all are drawn by the
   * pass's one bucket draw in the first hull draw's place. The faces write their own depth and one
   * fill, so the order they draw in leaves the depth and the colour as the instanced draws leave
   * them.
   */
  #hullDraw(
    meshes: ReadonlyArray<OccluderMesh>,
    camera: DrawCamera,
    viewport: Viewport,
    corners: BufferHandle,
    uniforms: Readonly<Record<string, Float32Array>>,
  ): DrawItem | null {
    const resources = this.#resources;
    const kernel = resources.cullKernel;
    const drawn = meshes.filter((mesh) => mesh.triangles.length >= 9);
    const fill = drawn[0]?.fill ?? null;
    if (
      !this.#geometry.clusterPath ||
      !this.#geometry.hullClusters ||
      kernel === null ||
      drawn.length === 0 ||
      !drawn.every((mesh) => mesh.fill === fill)
    ) {
      return null;
    }
    const frame = hullClusterFrame(drawn);
    if (frame === null) {
      return null;
    }
    const records = this.#writeCluster("records", frame.records);
    const lods = this.#writeCluster("lods", frame.lods);
    const instances = this.#writeCluster("instances", frame.instances);
    const inputs = new Uint32Array(2 * frame.hulls);
    for (let n = 0; n < frame.hulls; n += 1) {
      inputs[2 * n] = 1;
    }
    resources.hullClusters.cull(
      kernel,
      {
        records,
        lods,
        instances,
        count: frame.hulls,
        inputs,
        passes: [
          {
            kind: "camera",
            planes: frustumOf(
              { orientation: camera.pose.orientation, fovXRad: camera.fovXRad, viewport },
              HULL_CULL_MARGIN_RAD,
            ).planes,
            horizon: null,
            order: "appended",
            toleranceRad: 0,
            strideCap: 1,
          },
        ],
        rotationColumns: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)],
        viewAxis: vec3(0, 0, -1),
      },
      HULL_CLUSTER_CULL_LABEL,
    );
    const lists = resources.hullClusters.buckets?.lists;
    if (lists === undefined) {
      return null;
    }
    this.#lastHulls = { clusters: frame.clusters, triangles: frame.triangles };
    const [item] = resources.hullClusters.drawItems(
      0,
      resources.hullMesh,
      {
        material: resources.hullMaterials[fill === null ? "occluderHull" : "hullSilhouette"],
        offsetFromCameraM: ZERO_OFFSET,
        uniforms,
        textures: {},
        storageBuffers: { corners, hullInstances: instances, clusterLists: lists },
      },
      1,
    );
    return item ?? null;
  }

  #buffer(name: WireframeBuffer, bytes: number): BufferHandle {
    return this.#engine.createBuffer({
      name: `wireframe:${name}`,
      bytes,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
  }

  /** Writes `data` into the named buffer, first growing it by doubling if it is too small. */
  #write(name: WireframeBuffer, data: Float32Array): BufferHandle {
    let handle = this.#resources.buffers[name];
    if (data.byteLength > handle.bytes) {
      let bytes = handle.bytes;
      while (bytes < data.byteLength) {
        bytes *= 2;
      }
      handle = this.#buffer(name, bytes);
      this.#resources.buffers[name] = handle;
    }
    if (data.byteLength > 0) {
      this.#engine.writeBuffer(handle, 0, data);
    }
    return handle;
  }

  /**
   * The frame for a draw list: the list packed, its buffers written, its draws bound.
   *
   * @param viewport - The view's size, px, which the projection's aspect follows.
   * @param background - Draws at infinity, such as R06's baked star cube, encoded after the
   *   occluders (so that they hide them) and before the lines and sprites.
   * @param terrain - The bodies' terrain (plan R10, R10.T11): its draws, encoded after the
   *   occluders, so that the depth they wrote hides the contours, and before the background, which
   *   they hide; and the bodies whose datum gives way to it.
   */
  frame(
    list: WireframeDrawList,
    camera: DrawCamera,
    viewport: Viewport,
    background: ReadonlyArray<DrawItem> = [],
    terrain: WireframeTerrain = NO_WIREFRAME_TERRAIN,
  ): FrameSubmission {
    const packed = packWireframe(list, camera, viewport, terrain.surveyed);
    const buffers: Record<WireframeBuffer, BufferHandle> = {
      segments: this.#write("segments", packed.segments),
      spheres: this.#write("spheres", packed.spheres),
      corners: this.#write("corners", packed.corners),
      sprites: this.#write("sprites", packed.sprites),
    };
    const { materials, meshes } = this.#resources;
    this.#lastHulls = null;
    const bound = packed.draws.map((draw) => {
      const bufferName = MATERIAL_BUFFER[draw.material];
      const mask =
        draw.surveyedBody === undefined ? undefined : terrain.surveyed.get(draw.surveyedBody)?.mask;
      const item: DrawItem = {
        mesh: meshes[draw.mesh],
        material: materials[draw.material],
        offsetFromCameraM: draw.offsetFromCameraM,
        uniforms: draw.uniforms,
        textures: mask === undefined ? {} : { coverageMask: mask },
        instanceCount: draw.instanceCount,
        storageBuffers: { [bufferName]: buffers[bufferName] },
      };
      return { material: draw.material, item };
    });
    // The hulls' faces on the cluster path (plan R14, R14.T4.b): one bucket draw in the first hull
    // draw's place, the others dropped.
    const firstHull = bound.findIndex(({ material }) => isHull(material));
    const hullUniforms = packed.draws[firstHull]?.uniforms;
    const hullDraw =
      hullUniforms === undefined
        ? null
        : this.#hullDraw(list.occluderMeshes, camera, viewport, buffers.corners, {
            ...hullUniforms,
            firstTriangle: ZERO_TRIANGLE,
          });
    const kept =
      hullDraw === null
        ? bound
        : bound.flatMap((entry, index) =>
            index === firstHull
              ? [{ material: entry.material, item: hullDraw }]
              : isHull(entry.material)
                ? []
                : [entry],
          );
    // The occluders, then the terrain, which they hide and which hides the background, then the
    // background, then everything else in its packed order: the spheres before the hulls' faces,
    // a silhouette's included, and every line after them.
    const draws = [
      ...kept.filter(({ material }) => IS_OCCLUDER[material]).map(({ item }) => item),
      ...terrain.draws,
      ...background,
      ...kept.filter(({ material }) => !IS_OCCLUDER[material]).map(({ item }) => item),
    ];
    return {
      label: WIREFRAME_PASS_LABEL,
      viewRotation: viewRotation4(camera.pose.orientation),
      projection: perspectiveReversedInfinite(
        camera.fovXRad,
        viewport.widthPx / viewport.heightPx,
        NEAR_PLANE_M,
      ),
      draws,
      postProcesses: [],
    };
  }

  /** Packs, writes and renders one frame into `view`, with the bodies' terrain (R10.T11). */
  render(
    view: RenderView,
    list: WireframeDrawList,
    camera: DrawCamera,
    viewport: Viewport,
    background: ReadonlyArray<DrawItem> = [],
    terrain: WireframeTerrain = NO_WIREFRAME_TERRAIN,
  ): void {
    view.render(this.frame(list, camera, viewport, background, terrain));
  }

  /**
   * Stops following the engine's restores, and releases the hulls' cluster path's buffers and
   * kernel (plan R14, R14.T4.b); the engine owns and frees its other handles.
   */
  dispose(): void {
    this.#unsubscribe();
    this.#disposed = true;
    const resources = this.#resources;
    resources.hullClusters.dispose();
    for (const buffer of Object.values(resources.hullBuffers)) {
      this.#engine.releaseBuffer(buffer);
    }
    if (resources.cullKernel !== null) {
      this.#engine.releaseCompute(resources.cullKernel);
      resources.cullKernel = null;
    }
  }
}
