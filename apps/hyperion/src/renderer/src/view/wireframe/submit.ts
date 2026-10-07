/**
 * The wireframe's submission through R01's engine adapter (plan R02, R02.T14): the one file in
 * `view/` outside `view/engine/` that calls the engine.
 *
 * @remarks
 * The draw list (R02.T13) is packed into four storage buffers, one per shader, and drawn in this
 * order: the bodies' occluder spheres and the hulls' opaque faces (each pushed away in its
 * fragment by the list's `occluderSlopePx`, R07.T16.d; depth only, or over the photorealistic image
 * a silhouette filled opaque in its mesh's `fill`, R07.T16.e), the star sprites (additive), then
 * each line batch, its `--surface-0` casing first and its stroke over it, both premultiplied over
 * what is beneath. Sprites go before the lines, where the draw list lists them
 * after, so that a mark's casing covers a star beneath it, as the guide's casing rule wants of every
 * mark over the image. The shaders are standard WGSL in R01's convention (its Design note 23):
 * `frame.wgsl`'s `Frame` at `@group(0)`, each material's `Draw` at `@group(1)` (the draw's offset
 * from the camera, then the spec's uniforms in order), its storage buffer at `@group(2)`, and the
 * entry points `vertexMain` and `fragmentMain`. Every mesh's positions are the corners of the
 * instanced primitive, read as `@location(0)`.
 */

import { cross, norm, normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { BUFFER_USAGE } from "../engine/gpuFlags";
import type {
  BufferHandle,
  DrawItem,
  FrameSubmission,
  MaterialHandle,
  MeshHandle,
  MeshSpec,
  RenderEngine,
  RenderView,
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
import frameWgsl from "../shaders/frame.wgsl?raw";
import linesWgsl from "../shaders/lines.wgsl?raw";
import occluderWgsl from "../shaders/occluder.wgsl?raw";
import occluderSphereWgsl from "../shaders/occluderSphere.wgsl?raw";
import starSpriteWgsl from "../shaders/starSprite.wgsl?raw";
import toneCurveWgsl from "../shaders/toneCurve.wgsl?raw";
import type { DrawCamera, LineBatch, OccluderSphere, WireframeDrawList } from "./drawList";

/**
 * The wireframe's materials, by name: `occluderHull` a hull's faces as depth alone, and
 * `hullSilhouette` the same faces filled opaque over the photorealistic image (R07.T16.e).
 */
export type WireframeMaterial =
  "lines" | "occluderSphere" | "occluderHull" | "hullSilhouette" | "starSprite";

/** The meshes the wireframe instances: a unit quad's two triangles, and one triangle. */
export type WireframeMesh = "quad" | "triangle";

/** The storage buffers the wireframe writes each frame, one per shader. */
export type WireframeBuffer = "segments" | "spheres" | "corners" | "sprites";

/** The pass's label in `PassTimes`, stable across frames (R12 keys its records on it). */
export const WIREFRAME_PASS_LABEL = "view:wireframe";

/** The sources, each composed after `frame.wgsl` by plain concatenation; no preprocessor. */
const SOURCES: Readonly<Record<WireframeMaterial, string>> = {
  lines: frameWgsl + linesWgsl,
  occluderSphere: frameWgsl + occluderSphereWgsl,
  occluderHull: frameWgsl + occluderWgsl,
  hullSilhouette: frameWgsl + occluderWgsl,
  starSprite: frameWgsl + toneCurveWgsl + starSpriteWgsl,
};

/** Which storage buffer each material reads, at `@group(2) @binding(0)`. */
export const MATERIAL_BUFFER: Readonly<Record<WireframeMaterial, WireframeBuffer>> = {
  lines: "segments",
  occluderSphere: "spheres",
  occluderHull: "corners",
  hullSilhouette: "corners",
  starSprite: "sprites",
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
};

function spec(
  name: WireframeMaterial,
  state: Pick<WgslMaterialSpec, "uniforms" | "depthWrite" | "colourWrites" | "blend">,
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
};

/** A hull face's uniforms, for both of its materials, which share one source. */
const HULL_UNIFORMS: WgslMaterialSpec["uniforms"] = [
  { name: "firstTriangle", type: "f32" },
  { name: "occluderSlopePx", type: "f32" },
  { name: "fill", type: "vec4f" },
];

/**
 * The wireframe's five materials (Design notes 5, 9 and 12).
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
    uniforms: [
      { name: "colour", type: "vec4f" },
      { name: "widthPx", type: "f32" },
      { name: "firstSegment", type: "f32" },
      { name: "space", type: "f32" },
      { name: "dashOnPx", type: "f32" },
      { name: "dashOffPx", type: "f32" },
    ],
    depthWrite: false,
    colourWrites: true,
    blend: "premultiplied",
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

/** One draw of the packed frame, before it is bound to the engine's handles. */
export interface PackedDraw {
  readonly material: WireframeMaterial;
  readonly mesh: WireframeMesh;
  readonly instanceCount: number;
  readonly offsetFromCameraM: Float32Array;
  readonly uniforms: Readonly<Record<string, Float32Array>>;
}

/** One frame of the wireframe, packed for the GPU: the buffers' contents and the draws. */
export interface PackedWireframe {
  /** Two `vec4f` per segment: one end and the dash's phase there, px; the other end and 0. */
  readonly segments: Float32Array;
  /** Three `vec4f` per sphere: its rectangle, px; its centre and radius, m; its altitude, m. */
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

/**
 * Packs one frame of the draw list for the GPU (plan R02, R02.T14): a function of its arguments
 * alone, so that what reaches the engine is tested without a GPU.
 */
export function packWireframe(
  list: WireframeDrawList,
  camera: DrawCamera,
  viewport: Viewport,
): PackedWireframe {
  const projection: ProjectionCamera = {
    orientation: camera.pose.orientation,
    fovXRad: camera.fovXRad,
  };
  const draws: PackedDraw[] = [];
  const occluderSlopePx = new Float32Array([list.occluderSlopePx]);

  const sphereRows: number[] = [];
  let sphereCount = 0;
  for (const sphere of list.occluderSpheres) {
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
    sphereRows.push(rect.leftPx, rect.topPx, rect.rightPx, rect.bottomPx);
    sphereRows.push(c[0] ?? 0, c[1] ?? 0, c[2] ?? 0, sphere.radiusM);
    sphereRows.push(sphere.altitudeM, 0, 0, 0);
    sphereCount += 1;
  }
  if (sphereCount > 0) {
    draws.push({
      material: "occluderSphere",
      mesh: "quad",
      instanceCount: sphereCount,
      offsetFromCameraM: ZERO_OFFSET,
      uniforms: { occluderSlopePx },
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
    const stroke = (colour: string, widthPx: number): PackedDraw => ({
      material: "lines",
      mesh: "quad",
      instanceCount: count,
      offsetFromCameraM: batch.space === "view" ? batch.originF32 : ZERO_OFFSET,
      uniforms: {
        colour: linearColour(colour),
        widthPx: new Float32Array([widthPx]),
        firstSegment: new Float32Array([first]),
        space: new Float32Array([batch.space === "screen" ? 1 : 0]),
        dashOnPx: new Float32Array([batch.dash?.onPx ?? 0]),
        dashOffPx: new Float32Array([batch.dash?.offPx ?? 0]),
      },
    });
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

function centreOf(sphere: OccluderSphere): Vec3 {
  const c = sphere.centreF32;
  return vec3(c[0] ?? 0, c[1] ?? 0, c[2] ?? 0);
}

/** The smallest storage buffer made, bytes; each grows by doubling. */
const MIN_BUFFER_BYTES = 4_096;

/** What the renderer holds on the engine's device, made again after a device loss. */
interface DeviceResources {
  readonly materials: Readonly<Record<WireframeMaterial, MaterialHandle>>;
  readonly meshes: Readonly<Record<WireframeMesh, MeshHandle>>;
  readonly buffers: Record<WireframeBuffer, BufferHandle>;
}

/** The part of the engine the renderer uses. */
export type WireframeEngine = Pick<
  RenderEngine,
  "createMaterial" | "createMesh" | "createBuffer" | "writeBuffer" | "onRestored"
>;

/**
 * Draws wireframe draw lists into a view through the engine (plan R02, R02.T14).
 *
 * @remarks
 * It makes its materials, meshes and storage buffers on construction and again after the engine
 * restores a lost device (`RenderEngine.onRestored`), and grows a buffer by doubling when a frame
 * outgrows it. The engine has no way to free a buffer, so an outgrown one is released only with the
 * engine; doubling bounds the waste to the largest frame's size.
 */
export class WireframeRenderer {
  readonly #engine: WireframeEngine;
  #resources: DeviceResources;
  readonly #unsubscribe: () => void;

  constructor(engine: WireframeEngine) {
    this.#engine = engine;
    this.#resources = this.#create();
    this.#unsubscribe = engine.onRestored(() => {
      this.#resources = this.#create();
    });
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
    };
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
   */
  frame(
    list: WireframeDrawList,
    camera: DrawCamera,
    viewport: Viewport,
    background: ReadonlyArray<DrawItem> = [],
  ): FrameSubmission {
    const packed = packWireframe(list, camera, viewport);
    const buffers: Record<WireframeBuffer, BufferHandle> = {
      segments: this.#write("segments", packed.segments),
      spheres: this.#write("spheres", packed.spheres),
      corners: this.#write("corners", packed.corners),
      sprites: this.#write("sprites", packed.sprites),
    };
    const { materials, meshes } = this.#resources;
    const bound: DrawItem[] = packed.draws.map((draw) => {
      const bufferName = MATERIAL_BUFFER[draw.material];
      return {
        mesh: meshes[draw.mesh],
        material: materials[draw.material],
        offsetFromCameraM: draw.offsetFromCameraM,
        uniforms: draw.uniforms,
        textures: {},
        instanceCount: draw.instanceCount,
        storageBuffers: { [bufferName]: buffers[bufferName] },
      };
    });
    // The occluders, then the background they hide, then everything else in its packed order: the
    // spheres before the hulls' faces, a silhouette's included, and every line after them.
    const isOccluder = (index: number): boolean => {
      const material = packed.draws[index]?.material;
      return material !== undefined && IS_OCCLUDER[material];
    };
    const draws = [
      ...bound.filter((_, index) => isOccluder(index)),
      ...background,
      ...bound.filter((_, index) => !isOccluder(index)),
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

  /** Packs, writes and renders one frame into `view`. */
  render(
    view: RenderView,
    list: WireframeDrawList,
    camera: DrawCamera,
    viewport: Viewport,
    background: ReadonlyArray<DrawItem> = [],
  ): void {
    view.render(this.frame(list, camera, viewport, background));
  }

  /** Stops following the engine's restores; the engine owns and frees the handles. */
  dispose(): void {
    this.#unsubscribe();
  }
}
