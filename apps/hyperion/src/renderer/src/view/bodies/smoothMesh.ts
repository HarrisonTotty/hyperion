/**
 * The mesh regime's smooth figure (plan R07, T9; Design notes 2 and 3): R05's selection and patch
 * geometry on the body's reference spheroid at zero height, built over R05's public geometry, and
 * the `f64` twin of its draw.
 *
 * @remarks
 * A smooth figure is R05's cube sphere at zero height, its unit directions scaled by (a, a, c) in
 * the body-fixed axes, selected by R05's `selectPatches` on `planetGeometry(figure, null)` (no
 * level table, so selection refines on the chords' sag alone, R05.T7.b as built) and drawn through
 * R05's `FaceDifferences` arithmetic with its per-patch `f64` origins, so that nothing reaches the
 * GPU in world coordinates; the zero height needs no worker. R05's CDLOD morph bands
 * (`morphRangeM`) and skirts come with it.
 *
 * The figure decides where the body is opaque and its depth; its light is the disc's. Its draw
 * (`shaders/smoothMesh.wgsl`) shades each pixel it covers as the disc's first draw does, from the
 * pixel's rays against the analytic spheroid, where the spheroid covers the pixel wholly, and the
 * limb is the disc's second draw, its rectangle on the limb's plane ({@link limbDepths}). So the
 * mesh must cover every pixel the spheroid covers wholly, whose centre lies at least half a pixel
 * inside the limb: it is selected to {@link SMOOTH_MESH_TAU_PX} at the view's corner
 * ({@link smoothMeshTauPx}), and R05's bound holds a selected figure, morph included, within its
 * tolerance of the spheroid.
 *
 * A smooth figure is symmetric about its pole, so it is built in axes about the pole
 * ({@link poleAxes}), not the body's rotation: the patches do not turn with the body, and selection
 * does not change while it spins. A class map's texels are read by the record's axes, as the
 * disc's are.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  NEAR_PLANE_M,
  type ProjectionCamera,
  project,
  toViewAxes,
  type Viewport,
} from "../camera/projection";
import { conjugate, multiply, quaternionFromRows } from "../camera/quaternion";
import {
  type Rotation3,
  rotateToBody,
  rotateToBodyFixed,
  rotation3FromRows,
} from "../coords/rotation";
import type { QualitySetting } from "../quality/qualitySetting";
import { stToUv } from "../terrain/cube";
import { patchMeshData } from "../terrain/gpu/resources";
import {
  INSTANCE_RECORD_BYTES,
  InstanceRecords,
  patchTerms,
  type PatchTerms,
  SLOT_RECORD_BYTES,
  writeSlotRecord,
} from "../terrain/gpu/uniforms";
import type { PatchKey } from "../terrain/patchKey";
import { type BodyFigure, planetGeometry, type PlanetGeometry } from "../terrain/planet";
import { chordSagittaM, selectPatches, type ViewSelectionInput } from "../terrain/select";
import { effectiveTauPx, morphRangeM } from "../terrain/terrainPass";
import type { ScreenRect } from "../wireframe/submit";
import { perpendicularPair } from "../wireframe/bodies";

/**
 * The most a smooth figure's outline may lie inside its spheroid's limb, px, at the view's corner:
 * a quarter of a pixel, half the half pixel by which a wholly covered pixel's centre lies inside
 * the limb, so that the figure covers every such pixel.
 */
export const SMOOTH_MESH_TAU_PX = 0.25;

/**
 * The most patches one smooth figure selects: R05's `maxPatches`. 50 patches serve a camera 2 m
 * above an Earth at 1080p (tested). Where it binds, {@link SmoothMesh.limited} says so; no caller
 * acts on it, and the figure may then leave a wholly covered pixel by the limb undrawn.
 */
export const MAX_SMOOTH_MESH_PATCHES = 1024;

/** One patch of a smooth figure as a frame draws it. */
export interface SmoothPatch {
  readonly key: PatchKey;
  /** R05's terms of the patch at zero height (`patchTerms(key, figure, 0)`). */
  readonly terms: PatchTerms;
  /** The patch origin M d₀ less the camera, m along the galactic axes, in `f64`. */
  readonly originFromCameraM: Vec3;
  /** Where the morph to the parent's mesh begins and ends, m from the camera (R05's band). */
  readonly morphStartM: number;
  readonly morphEndM: number;
  /** How far the skirts hang below the edges along the spheroid's normal, m. */
  readonly skirtDepthM: number;
}

/** A body's smooth figure on one view this frame. */
export interface SmoothMesh {
  readonly body: BodyIdHex;
  /** The axes the figure is built in, body-fixed to galactic ({@link poleAxes}). */
  readonly axes: Rotation3;
  readonly figure: BodyFigure;
  readonly patches: ReadonlyArray<SmoothPatch>;
  /**
   * Whether {@link MAX_SMOOTH_MESH_PATCHES} stopped the selection, when the figure may lie further
   * than its tolerance inside the limb.
   */
  readonly limited: boolean;
}

/**
 * Body-fixed axes with z along `pole`, as a rotation to the galactic axes: x and y the pole's
 * `perpendicularPair`.
 */
export function poleAxes(pole: Vec3): Rotation3 {
  const z = normalise(pole);
  const [x, y] = perpendicularPair(z);
  return rotation3FromRows([vec3(x.x, y.x, z.x), vec3(x.y, y.y, z.y), vec3(x.z, y.z, z.z)]);
}

/** The geometries made, by figure: selection memoises each patch's bounds on the object. */
const GEOMETRIES = new Map<string, PlanetGeometry>();
const GEOMETRIES_MAX = 64;

/** R05's zero-height geometry of a figure, one object per figure's radii. */
function smoothFigureOf(figure: BodyFigure): PlanetGeometry {
  const key = `${String(figure.equatorialRadiusM)} ${String(figure.polarRadiusM)}`;
  let planet = GEOMETRIES.get(key);
  if (planet === undefined) {
    if (GEOMETRIES.size >= GEOMETRIES_MAX) {
      GEOMETRIES.clear();
    }
    planet = planetGeometry(figure, null);
    GEOMETRIES.set(key, planet);
  }
  return planet;
}

/**
 * The tolerance a view's smooth figures are selected at, px at its centre pixel's scale (R05's ρ):
 * {@link SMOOTH_MESH_TAU_PX} over sec²θ at the view's corner, since a gnomonic view's radial scale
 * at θ off its axis is sec²θ its centre's, the tangential secθ (Snyder 1987, _Map Projections — A
 * Working Manual_, USGS Professional Paper 1395, p. 165, eqs. 22-2 and 22-3).
 */
export function smoothMeshTauPx(camera: ProjectionCamera, viewport: Viewport): number {
  const tanX = Math.tan(camera.fovXRad / 2);
  const tanY = (tanX * viewport.heightPx) / viewport.widthPx;
  return SMOOTH_MESH_TAU_PX / (1 + tanX * tanX + tanY * tanY);
}

/** The unit direction d of vertex (x, y) of a patch from its terms, in `f64` (R05's formula). */
function directionAt(terms: PatchTerms, x: number, y: number): Vec3 {
  const [a, e1, e2] = terms.axes;
  const u = stToUv(terms.st0[0] + (x - 32) * terms.step);
  const v = stToUv(terms.st0[1] + (y - 32) * terms.step);
  return normalise(
    vec3(a.x + u * e1.x + v * e2.x, a.y + u * e1.y + v * e2.y, a.z + u * e1.z + v * e2.z),
  );
}

/** The patch centre's unit direction d₀, in `patchTerms`' operations. */
function centreDirection(terms: PatchTerms): Vec3 {
  const [a, e1, e2] = terms.axes;
  const [u0, v0] = terms.uv0;
  const n = vec3(
    a.x + u0 * e1.x + v0 * e2.x,
    a.y + u0 * e1.y + v0 * e2.y,
    a.z + u0 * e1.z + v0 * e2.z,
  );
  const length = Math.sqrt(n.x * n.x + n.y * n.y + n.z * n.z);
  return vec3(n.x / length, n.y / length, n.z / length);
}

/** M d: the spheroid's point over the unit direction d, body-fixed m. */
function spheroidAt(figure: BodyFigure, d: Vec3): Vec3 {
  return vec3(
    figure.equatorialRadiusM * d.x,
    figure.equatorialRadiusM * d.y,
    figure.polarRadiusM * d.z,
  );
}

/**
 * A body's smooth figure on a view: R05's selection of its zero-height spheroid at
 * {@link smoothMeshTauPx}, each selected patch's terms, origin and morph band.
 *
 * @param centreM - The body's centre from the camera, m along the galactic axes, `f64`.
 * @param pole - The pole the body is drawn about (its disc record's).
 * @param setting - The view's quality setting, which selection takes.
 */
export function smoothMeshOf(
  id: BodyIdHex,
  centreM: Vec3,
  figure: BodyFigure,
  pole: Vec3,
  camera: ProjectionCamera,
  viewport: Viewport,
  setting: QualitySetting,
): SmoothMesh {
  const planet = smoothFigureOf(figure);
  const axes = poleAxes(pole);
  const tauPx = smoothMeshTauPx(camera, viewport);
  const view: ViewSelectionInput = {
    camera: {
      positionM: rotateToBodyFixed(axes, scale(centreM, -1)),
      orientation: multiply(conjugate(quaternionFromRows(axes.rows)), camera.orientation),
    },
    fovXRad: camera.fovXRad,
    viewport,
    weight: 1,
    tauPx,
  };
  const selection = selectPatches({
    planet,
    views: [view],
    setting,
    grounded: [],
    maxPatches: MAX_SMOOTH_MESH_PATCHES,
  });
  // The bands at the tolerance selected at, raised where the budget binds (R05's F2).
  const morphView = { ...view, tauPx: effectiveTauPx(tauPx, selection.limitExcess, 1) };
  const patches: SmoothPatch[] = [];
  for (const selected of selection.patches.values()) {
    if (!selected.seen) {
      continue;
    }
    const { key } = selected;
    const terms = patchTerms(key, figure, 0);
    const origin = rotateToBody(axes, spheroidAt(figure, centreDirection(terms)));
    const originFromCameraM = vec3(
      centreM.x + origin.x,
      centreM.y + origin.y,
      centreM.z + origin.z,
    );
    const [morphStartM, morphEndM] = morphRangeM(planet, key.level, morphView);
    patches.push({
      key,
      terms,
      originFromCameraM,
      morphStartM,
      morphEndM,
      // Below the coarser neighbour's chords, which an edge not yet fully morphed meets: an edge
      // chord sags 0.49 of the coarser level's bound, and twice the bound also covers a coarser
      // neighbour that is itself morphing towards its parent (1.94 of it); and the f32 steps of
      // two patches' origins narrowed apart (about 2⁻²² of their distance).
      skirtDepthM:
        2 * chordSagittaM(planet, Math.max(key.level - 1, 0)) + 2 ** -20 * norm(originFromCameraM),
    });
  }
  return { body: id, axes, figure, patches, limited: selection.limited };
}

/** The records of a frame's smooth figures, for one draw each. */
export interface SmoothMeshRecords {
  /** R05's slot records (`SLOT_RECORD_BYTES` each), one per patch in the frame's order. */
  readonly slots: Float32Array;
  /** R05's instance records (`INSTANCE_RECORD_BYTES` each), patch i reading slot i. */
  readonly instances: Float32Array;
  /** Each figure's first instance. */
  readonly firstInstance: ReadonlyArray<number>;
  /** The patches in all. */
  readonly count: number;
}

/**
 * Packs a frame's smooth figures: each patch's terms and skirt into its slot record, its origin
 * less the camera (narrowed once) and its morph band into its instance record.
 */
export function packSmoothMeshes(meshes: ReadonlyArray<SmoothMesh>): SmoothMeshRecords {
  const count = meshes.reduce((sum, mesh) => sum + mesh.patches.length, 0);
  const slots = new ArrayBuffer(Math.max(count, 1) * SLOT_RECORD_BYTES);
  const instances = new InstanceRecords(Math.max(count, 1));
  const firstInstance: number[] = [];
  for (const mesh of meshes) {
    firstInstance.push(instances.count);
    for (const patch of mesh.patches) {
      const slot = instances.count;
      writeSlotRecord(slots, slot, patch.terms, patch.skirtDepthM);
      const o = patch.originFromCameraM;
      instances.pushXyz(slot, o.x, o.y, o.z, patch.morphStartM, patch.morphEndM);
    }
  }
  return {
    slots: new Float32Array(slots, 0, (count * SLOT_RECORD_BYTES) / 4),
    instances: new Float32Array(instances.buffer, 0, (count * INSTANCE_RECORD_BYTES) / 4),
    firstInstance,
    count,
  };
}

/**
 * A rotation as the shader's `mat4x4f`, column-major: R05's `bodyRotation`.
 */
export function rotationColumns(rotation: Rotation3): Float32Array {
  const [r0, r1, r2] = rotation.rows;
  return new Float32Array([
    r0.x,
    r1.x,
    r2.x,
    0,
    r0.y,
    r1.y,
    r2.y,
    0,
    r0.z,
    r1.z,
    r2.z,
    0,
    0,
    0,
    0,
    1,
  ]);
}

/** The reversed-Z depth of a limb draw's corners: (left, top), (right, top), (left, bottom), (right, bottom). */
export type LimbDepths = readonly [number, number, number, number];

/** A disc body's limb draw lies at infinity. */
export const DISC_LIMB_DEPTHS: LimbDepths = [0, 0, 0, 0];

/**
 * The reversed-Z depths at a mesh body's limb rectangle's corners, on the limb's plane, so that
 * the rectangle's depth is the plane's at every pixel.
 *
 * @remarks
 * A spheroid's silhouette from a point e outside it is its contour on the polar plane of e,
 * x · (A e) = 1 for x from the centre and A = diag(a⁻², a⁻², c⁻²) about the pole. Along the ray
 * r = (x_ndc ÷ s, y_ndc ÷ (s · aspect), −1) of a point of the view, the plane lies at
 * w = (1 + C · n) ÷ (r · n), for C the centre from the camera and n = A e, so its reversed depth
 * `NEAR_PLANE_M` ÷ w = `NEAR_PLANE_M` (r · n) ÷ (1 + C · n) is affine on the view, and the four
 * corners carry it unclamped. Where part of the rectangle sees the plane behind the camera or nearer
 * than the near plane (a camera near the body looking at its horizon), the values there leave
 * [0, 1] and the depth clip removes that part, which holds no limb pixel: the limb lies on the
 * plane ahead.
 *
 * @param centreM - The body's centre from the camera, m along the galactic axes.
 * @param pole - The pole the body is drawn about.
 */
export function limbDepths(
  centreM: Vec3,
  figure: BodyFigure,
  pole: Vec3,
  rect: ScreenRect,
  camera: ProjectionCamera,
  viewport: Viewport,
): LimbDepths {
  const a2 = figure.equatorialRadiusM ** 2;
  const c2 = figure.polarRadiusM ** 2;
  const centre = toViewAxes(centreM, camera.orientation);
  const p = normalise(toViewAxes(pole, camera.orientation));
  // n = A e with e = −centre, the camera from the body's centre.
  const along = -dot(centre, p);
  const perpendicular = sub(scale(centre, -1), scale(p, along));
  const n = vec3(
    perpendicular.x / a2 + (along * p.x) / c2,
    perpendicular.y / a2 + (along * p.y) / c2,
    perpendicular.z / a2 + (along * p.z) / c2,
  );
  const k = 1 + dot(centre, n);
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const aspect = viewport.widthPx / viewport.heightPx;
  const depthAt = (xPx: number, yPx: number): number => {
    const ray = vec3(
      ((xPx / viewport.widthPx) * 2 - 1) / s,
      (1 - (yPx / viewport.heightPx) * 2) / (s * aspect),
      -1,
    );
    return (NEAR_PLANE_M * dot(ray, n)) / k;
  };
  return [
    depthAt(rect.leftPx, rect.topPx),
    depthAt(rect.rightPx, rect.topPx),
    depthAt(rect.leftPx, rect.bottomPx),
    depthAt(rect.rightPx, rect.bottomPx),
  ];
}

/**
 * The reversed-Z depth of a limb rectangle at a point of the view, px, as the rasteriser
 * interpolates its two triangles' corners: (0, 0), (1, 0), (1, 1) and (0, 0), (1, 1), (0, 1) of
 * the rectangle (R02's quad).
 */
export function limbDepthAt(
  rect: ScreenRect,
  depths: LimbDepths,
  xPx: number,
  yPx: number,
): number {
  const u = (xPx - rect.leftPx) / (rect.rightPx - rect.leftPx);
  const v = (yPx - rect.topPx) / (rect.bottomPx - rect.topPx);
  const [d00, d10, d01, d11] = depths;
  // Below the diagonal (u ≥ v) the first triangle, above it the second.
  return u >= v ? d00 + u * (d10 - d00) + v * (d11 - d10) : d00 + u * (d11 - d01) + v * (d01 - d00);
}

/** The shared patch mesh: R05's 65 × 65 grid with skirts, made once. */
let PATCH_MESH: ReturnType<typeof patchMeshData> | null = null;

function patchMesh(): ReturnType<typeof patchMeshData> {
  PATCH_MESH ??= patchMeshData();
  return PATCH_MESH;
}

/** One vertex of a patch, projected: its pixel coordinates and reversed depth. */
interface ProjectedVertex {
  readonly xPx: number;
  readonly yPx: number;
  readonly depth: number;
  readonly inFront: boolean;
}

/** A patch's vertices from the camera, m along the galactic axes, as its draw places them, in `f64`. */
export function smoothPatchVertices(mesh: SmoothMesh, patch: SmoothPatch): Vec3[] {
  const { positions } = patchMesh();
  const { terms, originFromCameraM: origin } = patch;
  const { figure } = mesh;
  const p0 = spheroidAt(figure, centreDirection(terms));
  const own = (x: number, y: number): Vec3 => sub(spheroidAt(figure, directionAt(terms, x, y)), p0);
  const morphTarget = (x: number, y: number): Vec3 => {
    const oddX = x % 2 === 1;
    const oddY = y % 2 === 1;
    if (terms.straddles || (!oddX && !oddY)) {
      return own(x, y);
    }
    const [a, b] =
      oddX && !oddY
        ? [own(x - 1, y), own(x + 1, y)]
        : !oddX
          ? [own(x, y - 1), own(x, y + 1)]
          : [own(x - 1, y - 1), own(x + 1, y + 1)];
    return scale(vec3(a.x + b.x, a.y + b.y, a.z + b.z), 0.5);
  };
  const toCamera = (q: Vec3): Vec3 => {
    const r = rotateToBody(mesh.axes, q);
    return vec3(origin.x + r.x, origin.y + r.y, origin.z + r.z);
  };
  const vertices: Vec3[] = [];
  for (let i = 0; i < positions.length; i += 3) {
    const x = positions[i] ?? 0;
    const y = positions[i + 1] ?? 0;
    const skirt = (positions[i + 2] ?? 0) > 0.5;
    const unmorphed = own(x, y);
    const span = patch.morphEndM - patch.morphStartM;
    const k =
      span > 0
        ? Math.min(1, Math.max(0, (norm(toCamera(unmorphed)) - patch.morphStartM) / span))
        : 0;
    const target = morphTarget(x, y);
    let q = vec3(
      unmorphed.x + k * (target.x - unmorphed.x),
      unmorphed.y + k * (target.y - unmorphed.y),
      unmorphed.z + k * (target.z - unmorphed.z),
    );
    if (skirt) {
      const d = directionAt(terms, x, y);
      const nu = normalise(
        vec3(
          d.x / figure.equatorialRadiusM,
          d.y / figure.equatorialRadiusM,
          d.z / figure.polarRadiusM,
        ),
      );
      q = sub(q, scale(nu, patch.skirtDepthM));
    }
    vertices.push(toCamera(q));
  }
  return vertices;
}

/** Where a smooth figure's draw covers the view: per sample, its reversed depth, or −1. */
export interface MeshRaster {
  /** Samples per pixel along each axis. */
  readonly samples: number;
  readonly widthSamples: number;
  readonly heightSamples: number;
  /** Row by row from the top; sample (i, j) sits at ((i + 0.5) ÷ samples, (j + 0.5) ÷ samples) px. */
  readonly depth: Float64Array;
}

/**
 * Rasterises a smooth figure's front faces, skirts included, at points of the view, in `f64`, as
 * its draw does (counter-clockwise front faces, back faces culled, the nearest depth kept under
 * reversed-Z): the twin the coverage tests read. A triangle with a vertex behind the near plane is
 * left out, which the tests' views never meet.
 *
 * @param samples - Samples per pixel along each axis: 1 for the pixels' centres.
 */
export function rasteriseSmoothMesh(
  mesh: SmoothMesh,
  camera: ProjectionCamera,
  viewport: Viewport,
  samples = 1,
): MeshRaster {
  const widthSamples = viewport.widthPx * samples;
  const heightSamples = viewport.heightPx * samples;
  const depth = new Float64Array(widthSamples * heightSamples).fill(-1);
  const { indices } = patchMesh();
  for (const patch of mesh.patches) {
    const projected: ProjectedVertex[] = smoothPatchVertices(mesh, patch).map((v) =>
      project(v, camera, viewport),
    );
    for (let t = 0; t < indices.length; t += 3) {
      const a = projected[indices[t] ?? 0];
      const b = projected[indices[t + 1] ?? 0];
      const c = projected[indices[t + 2] ?? 0];
      if (a === undefined || b === undefined || c === undefined) {
        continue;
      }
      if (!(a.inFront && b.inFront && c.inFront)) {
        continue;
      }
      rasteriseTriangle(a, b, c, samples, widthSamples, heightSamples, depth);
    }
  }
  return { samples, widthSamples, heightSamples, depth };
}

/** One triangle into the raster, if it faces the camera. */
function rasteriseTriangle(
  a: ProjectedVertex,
  b: ProjectedVertex,
  c: ProjectedVertex,
  samples: number,
  widthSamples: number,
  heightSamples: number,
  depth: Float64Array,
): void {
  // On the view, y down: a counter-clockwise front face (y up) has a negative signed area.
  const area = (b.xPx - a.xPx) * (c.yPx - a.yPx) - (c.xPx - a.xPx) * (b.yPx - a.yPx);
  if (!(area < 0)) {
    return;
  }
  const minI = Math.max(0, Math.ceil(Math.min(a.xPx, b.xPx, c.xPx) * samples - 0.5));
  const maxI = Math.min(
    widthSamples - 1,
    Math.floor(Math.max(a.xPx, b.xPx, c.xPx) * samples - 0.5),
  );
  const minJ = Math.max(0, Math.ceil(Math.min(a.yPx, b.yPx, c.yPx) * samples - 0.5));
  const maxJ = Math.min(
    heightSamples - 1,
    Math.floor(Math.max(a.yPx, b.yPx, c.yPx) * samples - 0.5),
  );
  for (let j = minJ; j <= maxJ; j += 1) {
    const y = (j + 0.5) / samples;
    for (let i = minI; i <= maxI; i += 1) {
      const x = (i + 0.5) / samples;
      // Barycentric weights from the edge functions, each ≥ 0 inside.
      const wa = ((b.xPx - x) * (c.yPx - y) - (c.xPx - x) * (b.yPx - y)) / area;
      const wb = ((c.xPx - x) * (a.yPx - y) - (a.xPx - x) * (c.yPx - y)) / area;
      const wc = 1 - wa - wb;
      if (wa < 0 || wb < 0 || wc < 0) {
        continue;
      }
      const z = wa * a.depth + wb * b.depth + wc * c.depth;
      const at = j * widthSamples + i;
      if (z >= (depth[at] ?? -1)) {
        depth[at] = z;
      }
    }
  }
}
