/**
 * The cloud map (plan R11, R11.T7.c; Design note 10): the layer's field's coarse octaves about the
 * camera, made each frame at five scales into one texture, from which the volume's march reads
 * each sample's column at one or two bilinear taps and adds only the octaves finer than the map's
 * texel down to its own footprint (`cloudShape.ts`' `columnFromMap`), where the field itself takes
 * some 25 taps (`cloudField.wgsl`), as Nubis's weather map serves its march (Schneider and Vos
 * 2015). The CPU reference of `cloudMap.wgsl` and of `cloudShape.wgsl`'s reading.
 *
 * @remarks
 * - **Its plane.** The gnomonic projection onto the plane tangent to a sphere of the shell's
 *   equatorial radius a_s at an anchor direction n: a place's direction q goes to a_s (q·e, q·f) ÷
 *   (q·n), e and f the anchor's east and north. The anchor is the camera's nadir, kept until the
 *   nadir strays {@link CLOUD_MAP_REANCHOR_RAD} from it, so that the texels hold still as the
 *   camera moves; within that the plane's scale departs from the sphere's by under 1%.
 * - **Its levels.** Level k's {@link CLOUD_MAP_SIDE}² texels are {@link CLOUD_MAP_FINEST_M} ×
 *   4^k across, 160 m to 41 km, so that its five levels reach 20 km, 82 km, 330 km, 1,300 km and
 *   5,200 km either side of the camera; each level's texels are aligned to the plane's own grid of
 *   their size, its origin snapped to it about the camera, so that a texel's place is the same from
 *   frame to frame.
 * - **A texel** holds the field's parts at its centre's place on the shell over a footprint of the
 *   texel's size: the resolved sum Σ w_k ρᵏ n_k of the octaves that footprint resolves, and the
 *   coverage's threshold z_c and wind there, eastward and northward (`cloudShape.ts`'
 *   `cloudMapTexelAt`), `rgba16float`, filterable as `rgba32float` is not in core WebGPU, the
 *   levels stacked in rows. A finer footprint's octaves are
 *   the reader's: so the field the volume marches is the layer's at each sample's own footprint,
 *   its coarse part interpolated between texels whose octaves span four of them.
 * - **A read** at a place and footprint F takes the level whose texel is F, or the first whose
 *   extent holds the place within {@link CLOUD_MAP_INNER} of its half-width, whichever is coarser,
 *   as a continuous level blended between its two nearest; a place beyond the last level has no
 *   clouds.
 * - Memory: 256 × 1,280 `rgba16float`, 2.5 MiB, in the `clouds` category. The resolved sum, of up
 *   to twelve octaves of unit noise, about ±4, takes `f16`'s 2⁻⁸ steps there, 0.003 in z after its
 *   normalisation, under the field's own GPU tolerance of 0.02 (`smoke/clouds.ts`).
 */
import { cross, dot, norm, scale, type Vec3 } from "../../geometry/vec3";
import { log2 } from "../../lib/pinnedMath";
import { fromHalfBits, toHalfBits } from "../sky/half";
import type { CloudShell } from "./cloudLayer";

/** Texels along a level's edge. */
export const CLOUD_MAP_SIDE = 256;

/** The map's levels. */
export const CLOUD_MAP_LEVELS = 5;

/** Level 0's texel, m. */
export const CLOUD_MAP_FINEST_M = 160;

/** Each level's texel over the last's. */
export const CLOUD_MAP_RATIO = 4;

/** The angle from the anchor at which the camera's nadir moves it, rad. */
export const CLOUD_MAP_REANCHOR_RAD = 0.08;

/** The share of a level's half-width within which it is read before the next blends in. */
export const CLOUD_MAP_INNER = 0.7;

/** The least cosine from the anchor at which a place is projected. */
export const CLOUD_MAP_LEAST_COSINE = 0.05;

/** A frame's map: its plane and each level's origin. */
export interface CloudMapFrame {
  /** n, the anchor direction, unit, body-fixed. */
  readonly anchor: Vec3;
  /** e and f, the plane's east and north, unit, body-fixed. */
  readonly east: Vec3;
  readonly north: Vec3;
  /** a_s, the plane's distance from the centre, m. */
  readonly radiusM: number;
  /** Each level's corner on the plane, m: its texel (0, 0)'s lower corner. */
  readonly origins: ReadonlyArray<{ readonly x: number; readonly y: number }>;
}

/** Level k's texel, m. */
export function mapTexelM(level: number): number {
  return CLOUD_MAP_FINEST_M * CLOUD_MAP_RATIO ** level;
}

/** A direction's east and north: east along ẑ × n, or ŷ's where n is the pole. */
function basisOf(n: Vec3): { readonly east: Vec3; readonly north: Vec3 } {
  let east = cross({ x: 0, y: 0, z: 1 }, n);
  if (norm(east) < 1e-6) {
    east = { x: 0, y: 1, z: 0 };
  }
  east = scale(east, 1 / norm(east));
  return { east, north: cross(n, east) };
}

/** A place's gnomonic coordinates on a frame's plane, m; `null` too far from its anchor. */
export function onMapPlane(
  frame: Pick<CloudMapFrame, "anchor" | "east" | "north" | "radiusM">,
  placeM: Vec3,
): { readonly x: number; readonly y: number } | null {
  const r = norm(placeM);
  const c = dot(placeM, frame.anchor) / r;
  if (!(c > CLOUD_MAP_LEAST_COSINE)) {
    return null;
  }
  const k = frame.radiusM / (c * r);
  return { x: dot(placeM, frame.east) * k, y: dot(placeM, frame.north) * k };
}

/**
 * A frame's map about a camera, from an anchor direction (unit, body-fixed): each level's origin
 * snapped to its texels about the camera's nadir.
 */
export function cloudMapFrameOf(anchor: Vec3, cameraM: Vec3, radiusM: number): CloudMapFrame {
  const { east, north } = basisOf(anchor);
  const plane = { anchor, east, north, radiusM };
  const nadir = onMapPlane(plane, cameraM) ?? { x: 0, y: 0 };
  const origins = Array.from({ length: CLOUD_MAP_LEVELS }, (_, level) => {
    const texel = mapTexelM(level);
    const half = (CLOUD_MAP_SIDE / 2) * texel;
    return {
      x: Math.round((nadir.x - half) / texel) * texel,
      y: Math.round((nadir.y - half) / texel) * texel,
    };
  });
  return { ...plane, origins };
}

/**
 * A view's map's anchor across frames (the module's remarks): the camera's nadir, moved when the
 * nadir strays {@link CLOUD_MAP_REANCHOR_RAD} from it.
 */
export class CloudMapAnchor {
  #anchor: Vec3 | null = null;

  /** The frame's map for a camera, body-fixed, m, on a plane at `radiusM`. */
  frameFor(cameraM: Vec3, radiusM: number): CloudMapFrame {
    const nadir = scale(cameraM, 1 / norm(cameraM));
    const held = this.#anchor;
    if (held === null || dot(held, nadir) < Math.cos(CLOUD_MAP_REANCHOR_RAD)) {
      this.#anchor = nadir;
    }
    return cloudMapFrameOf(this.#anchor ?? nadir, cameraM, radiusM);
  }

  /** Lets the anchor go, so that the next frame takes the nadir afresh. */
  reset(): void {
    this.#anchor = null;
  }
}

/**
 * Level k's texel (i, j)'s centre's place on the shell, body-fixed, m: the plane's point through
 * the spheroid of the shell's semi-axes, along the ray from the centre.
 */
export function texelPlace(
  frame: CloudMapFrame,
  shell: Pick<CloudShell, "equatorialRadiusM" | "polarRadiusM">,
  level: number,
  i: number,
  j: number,
): Vec3 {
  const texel = mapTexelM(level);
  const origin = frame.origins[level] ?? { x: 0, y: 0 };
  const x = origin.x + (i + 0.5) * texel;
  const y = origin.y + (j + 0.5) * texel;
  const v = {
    x: frame.anchor.x * frame.radiusM + x * frame.east.x + y * frame.north.x,
    y: frame.anchor.y * frame.radiusM + x * frame.east.y + y * frame.north.y,
    z: frame.anchor.z * frame.radiusM + x * frame.east.z + y * frame.north.z,
  };
  const k = shell.equatorialRadiusM / shell.polarRadiusM;
  return scale(v, shell.equatorialRadiusM / Math.hypot(v.x, v.y, v.z * k));
}

/**
 * The continuous level a read at a place on the plane and footprint takes (the module's remarks);
 * `null` beyond the last level.
 */
export function mapLevelAt(
  frame: CloudMapFrame,
  plane: { readonly x: number; readonly y: number },
  footprintM: number,
): number | null {
  const origin = frame.origins[0] ?? { x: 0, y: 0 };
  const half0 = (CLOUD_MAP_SIDE / 2) * CLOUD_MAP_FINEST_M;
  const r0 =
    Math.max(Math.abs(plane.x - (origin.x + half0)), Math.abs(plane.y - (origin.y + half0))) /
    half0;
  const last = CLOUD_MAP_LEVELS - 1;
  if (r0 / CLOUD_MAP_RATIO ** last > 1 - 1 / CLOUD_MAP_SIDE) {
    return null;
  }
  const byFootprint = log2(Math.max(footprintM, 1e-3) / CLOUD_MAP_FINEST_M) / 2;
  const byExtent = log2(Math.max(r0, 1e-9) / CLOUD_MAP_INNER) / 2;
  return Math.min(Math.max(byFootprint, byExtent, 0), last);
}

/** A map's texel (the module's remarks). */
export interface CloudMapTexel {
  /** Σ w_k ρᵏ n_k over the octaves the texel's footprint resolves. */
  readonly resolvedSum: number;
  /** z_c, the coverage's threshold there. */
  readonly threshold: number;
  /** The wind there, eastward and northward, m s⁻¹. */
  readonly eastMps: number;
  readonly northMps: number;
}

/** A read of the map at a place: its texel, blended between its two nearest levels. */
export interface CloudMapRead {
  readonly texel: CloudMapTexel;
  /** The finer level's texel and the coarser's, m, and the share of the coarser in the blend. */
  readonly fineM: number;
  readonly coarseM: number;
  readonly blend: number;
}

/** Two texels mixed, `t` of the second. */
function mixed(a: CloudMapTexel, b: CloudMapTexel, t: number): CloudMapTexel {
  return {
    resolvedSum: a.resolvedSum + t * (b.resolvedSum - a.resolvedSum),
    threshold: a.threshold + t * (b.threshold - a.threshold),
    eastMps: a.eastMps + t * (b.eastMps - a.eastMps),
    northMps: a.northMps + t * (b.northMps - a.northMps),
  };
}

/**
 * The map's twin over a frame: its texels as `texelAt` gives them, each read once, and read as
 * `cloudShape.wgsl` reads the texture: bilinear between texel centres, held at a level's edge,
 * blended between the two nearest levels. {@link cloudMapTwinOf} makes its texels from the field;
 * {@link cloudMapTwinFromTexels} reads them from a map's texture read back.
 */
export class CloudMapTwin {
  readonly #frame: CloudMapFrame;
  readonly #texelAt: (level: number, i: number, j: number) => CloudMapTexel;
  readonly #texels = new Map<number, CloudMapTexel>();

  constructor(
    frame: CloudMapFrame,
    texelAt: (level: number, i: number, j: number) => CloudMapTexel,
  ) {
    this.#frame = frame;
    this.#texelAt = texelAt;
  }

  get frame(): CloudMapFrame {
    return this.#frame;
  }

  /** Level k's texel (i, j), read once. */
  texel(level: number, i: number, j: number): CloudMapTexel {
    const key = (level * CLOUD_MAP_SIDE + j) * CLOUD_MAP_SIDE + i;
    let held = this.#texels.get(key);
    if (held === undefined) {
      held = this.#texelAt(level, i, j);
      this.#texels.set(key, held);
    }
    return held;
  }

  /** Level k at a place on the plane, bilinear, held within the level's texel centres. */
  #levelAt(level: number, plane: { readonly x: number; readonly y: number }): CloudMapTexel {
    const texel = mapTexelM(level);
    const origin = this.#frame.origins[level] ?? { x: 0, y: 0 };
    const n = CLOUD_MAP_SIDE;
    const tx = Math.min(Math.max((plane.x - origin.x) / texel - 0.5, 0), n - 1);
    const ty = Math.min(Math.max((plane.y - origin.y) / texel - 0.5, 0), n - 1);
    const i0 = Math.min(Math.floor(tx), n - 2);
    const j0 = Math.min(Math.floor(ty), n - 2);
    const fx = tx - i0;
    const fy = ty - j0;
    const top = mixed(this.texel(level, i0, j0), this.texel(level, i0 + 1, j0), fx);
    const bottom = mixed(this.texel(level, i0, j0 + 1), this.texel(level, i0 + 1, j0 + 1), fx);
    return mixed(top, bottom, fy);
  }

  /** A body-fixed place's coordinates on the map's plane, m; the origin where it has none. */
  planeOf(placeM: Vec3): { readonly x: number; readonly y: number } {
    return onMapPlane(this.#frame, placeM) ?? { x: 0, y: 0 };
  }

  /** The map at a body-fixed place and footprint (the module's remarks); `null` beyond it. */
  readAt(placeM: Vec3, footprintM: number): CloudMapRead | null {
    const plane = onMapPlane(this.#frame, placeM);
    if (plane === null) {
      return null;
    }
    const level = mapLevelAt(this.#frame, plane, footprintM);
    if (level === null) {
      return null;
    }
    const k0 = Math.floor(level);
    const k1 = Math.min(k0 + 1, CLOUD_MAP_LEVELS - 1);
    const fine = this.#levelAt(k0, plane);
    const blend = k1 === k0 ? 0 : level - k0;
    return {
      texel: k1 === k0 ? fine : mixed(fine, this.#levelAt(k1, plane), blend),
      fineM: mapTexelM(k0),
      coarseM: mapTexelM(k1),
      blend,
    };
  }
}

/** The map pass's and a reader's uniforms of a frame's map, each a `vec4f` (`cloudMap.wgsl`'s). */
export function cloudMapUniforms(frame: CloudMapFrame): Readonly<Record<string, Float32Array>> {
  const o = frame.origins;
  const at = (k: number): { readonly x: number; readonly y: number } => o[k] ?? { x: 0, y: 0 };
  return {
    mapAnchor: new Float32Array([frame.anchor.x, frame.anchor.y, frame.anchor.z, frame.radiusM]),
    mapEast: new Float32Array([frame.east.x, frame.east.y, frame.east.z, CLOUD_MAP_FINEST_M]),
    mapNorth: new Float32Array([frame.north.x, frame.north.y, frame.north.z, CLOUD_MAP_LEVELS]),
    mapOrigins01: new Float32Array([at(0).x, at(0).y, at(1).x, at(1).y]),
    mapOrigins23: new Float32Array([at(2).x, at(2).y, at(3).x, at(3).y]),
    mapOrigins4: new Float32Array([at(4).x, at(4).y, CLOUD_MAP_SIDE, CLOUD_MAP_INNER]),
  };
}

/** The map's members in a `Draw`, in order, each a `vec4f`. */
export const CLOUD_MAP_MEMBERS = [
  "mapAnchor",
  "mapEast",
  "mapNorth",
  "mapOrigins01",
  "mapOrigins23",
  "mapOrigins4",
] as const;

/** A value narrowed to `f16` and widened again. */
function narrowedHalf(v: number): number {
  return fromHalfBits(toHalfBits(v));
}

/**
 * The map's twin whose texels are made as they are read, from `texelAt` (`cloudShape.ts`'
 * `cloudMapTexelAt` at a place on the shell and a footprint of the texel's size), narrowed to
 * `f16` as the texture holds them.
 */
export function cloudMapTwinOf(
  frame: CloudMapFrame,
  shell: Pick<CloudShell, "equatorialRadiusM" | "polarRadiusM">,
  texelAt: (pointM: Vec3, footprintM: number) => CloudMapTexel,
): CloudMapTwin {
  return new CloudMapTwin(frame, (level, i, j) => {
    const made = texelAt(texelPlace(frame, shell, level, i, j), mapTexelM(level));
    const h = narrowedHalf;
    return {
      resolvedSum: h(made.resolvedSum),
      threshold: h(made.threshold),
      eastMps: h(made.eastMps),
      northMps: h(made.northMps),
    };
  });
}

/**
 * The map's twin over a map's texture read back: `texels` its {@link CLOUD_MAP_SIDE} ×
 * {@link CLOUD_MAP_SIDE} {@link CLOUD_MAP_LEVELS} texels, four values each, the first row first.
 */
export function cloudMapTwinFromTexels(frame: CloudMapFrame, texels: Float32Array): CloudMapTwin {
  return new CloudMapTwin(frame, (level, i, j) => {
    const at = 4 * ((level * CLOUD_MAP_SIDE + j) * CLOUD_MAP_SIDE + i);
    return {
      resolvedSum: texels[at] ?? 0,
      threshold: texels[at + 1] ?? 0,
      eastMps: texels[at + 2] ?? 0,
      northMps: texels[at + 3] ?? 0,
    };
  });
}
