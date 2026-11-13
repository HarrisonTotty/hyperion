/**
 * Plan R14, R14.T5.f's check on the run's adapter: the DAG consumers' occluder depth in the
 * wireframe's hidden-line frames.
 *
 * @remarks
 * Every frame of the wireframe group that draws a craft's hull, and `TEST_HULL` seen from outside
 * at three distances in three attitudes ({@link testHullFrames}), is drawn again with each hull's
 * faces as a mesh of a page pool, quantised to a grid of {@link GRID_STEP_M} about their least
 * corner and written as a two-level DAG (`test/meshPages.ts`): its leaves the faces in two
 * clusters, its root all of them in one, each corner moved {@link offsetM} from their centroid,
 * outward in one file and inward in another, nearly the whole error the cut draws at
 * `meshTolerancePx.wireframeOccluder` from the frame's camera, and that τ 0 does not. The
 * quantised faces drawn as the hulls' occluders are the reference; for each file, the meshes at
 * τ 0, the finest cut, must draw the reference's colour and depth to the byte, so that
 * `meshOccluderCluster.wgsl` decodes each page's corners to the bits the hull draw places; and the
 * meshes at the setting's tolerance, the root drawn where its error projects within it, its faces
 * nearer the camera or farther, must flip no hull edge between shown and hidden: an edge shown at
 * every point sampled on the view ({@link EDGE_SAMPLES}) at one cut and hidden at every one at
 * the other. A point that moves alone, an edge's end by a silhouette moved within the tolerance, is
 * reported, not failed.
 */

import type { ColourTokens } from "../spatial/paint";
import { readMeshFile } from "../test/meshFlyby";
import { aViewCraft, aViewScene, FIXTURE_SYSTEM } from "../test/viewFixtures";
import { type SyntheticCluster, writeMeshFile } from "../test/meshPages";
import { type Vec3, vec3 } from "../geometry/vec3";
import { project, type Viewport } from "../view/camera/projection";
import { IDENTITY_QUATERNION, quaternionFromAxisAngle } from "../view/camera/quaternion";
import { TEST_HULL } from "../view/scene/hull";
import type { PageSphere } from "../view/clusters/pageFormat";
import { GeometryPagePool, pagePoolSlots } from "../view/clusters/pages";
import type { RenderEngine } from "../view/engine/types";
import type { GeometrySettings } from "../view/quality/qualitySetting";
import {
  buildWireframeDrawList,
  type DrawCamera,
  type OccluderMesh,
  viewStrokesAt,
  type WireframeDrawList,
} from "../view/wireframe/drawList";
import { type MeshOccluder, wireframeToleranceRad } from "../view/wireframe/meshOccluders";
import {
  linearColour,
  MESH_OCCLUDER_CONSUMER,
  type WireframeRenderer,
} from "../view/wireframe/submit";
import { ClusterStats } from "../view/clusters/stats";
import { bytesDigest, type Checks, halfTexels, pause, texel } from "./harness";

/** The grid the hulls' corners are quantised to, m: a power of two, as the builder's is. */
const GRID_STEP_M = 2 ** -12;

/** How far below the faces' least corner the grid starts, m, room for a root moved outward. */
const GRID_MARGIN_M = 0.5;

/** The share of the cut's allowance a root's corners move, short of it by the `f32` rounding. */
const ALLOWANCE_SHARE = 0.9;

/** Which way a root's corners move from their centroid. */
type Offset = "outward" | "inward";

/** Where along each hull edge its visibility is sampled, as fractions of its length. */
const EDGE_SAMPLES = [0.2, 0.35, 0.5, 0.65, 0.8] as const;

/** The view of {@link testHullFrames}: 320 × 240 px, 60° across. */
const HULL_VIEW: Viewport = { widthPx: 320, heightPx: 240 };

/** Where {@link testHullFrames} sets `TEST_HULL` in front of the camera, m. */
const HULL_DISTANCES_M = [25, 60, 160] as const;

/** `TEST_HULL`'s attitudes in {@link testHullFrames}: square on, yawed, and pitched and yawed. */
const HULL_ATTITUDES = [
  IDENTITY_QUATERNION,
  quaternionFromAxisAngle(vec3(0, 1, 0), Math.PI / 3),
  quaternionFromAxisAngle(vec3(0.6, 0.8, 0), 2.4),
] as const;

/**
 * `TEST_HULL` seen from outside, at each of {@link HULL_DISTANCES_M} in each of
 * {@link HULL_ATTITUDES}, as the wireframe's list draws a craft: its edges and its faces, from a
 * camera at the system's origin looking down −z.
 */
export function testHullFrames(tokens: ColourTokens): HullOccluderFrame[] {
  const camera: DrawCamera = {
    pose: {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: vec3(0, 0, 0),
      orientation: IDENTITY_QUATERNION,
    },
    fovXRad: Math.PI / 3,
  };
  return HULL_DISTANCES_M.flatMap((distanceM) =>
    HULL_ATTITUDES.map((attitude) => {
      const scene = aViewScene({
        bodies: [],
        stars: [],
        ownShip: null,
        craft: [
          aViewCraft({
            id: "T5.f hull",
            designation: TEST_HULL.name,
            hull: TEST_HULL,
            pose: {
              position: { kind: "system", system: FIXTURE_SYSTEM, m: vec3(0, 0, -distanceM) },
              attitude,
            },
          }),
        ],
      });
      const list = buildWireframeDrawList(scene, camera, HULL_VIEW, tokens, {
        lowSetting: false,
        ev100: 0,
        selection: null,
        destination: null,
        remPx: 16,
        ...viewStrokesAt(1),
      });
      return { list, camera, viewport: HULL_VIEW };
    }),
  );
}

/** A frame the wireframe group drew with a hull's faces in it. */
export interface HullOccluderFrame {
  readonly list: WireframeDrawList;
  readonly camera: DrawCamera;
  readonly viewport: Viewport;
}

/** A hull's faces quantised: their least corner, m from the craft's origin, and grid corners. */
interface Quantised {
  readonly leastM: Vec3;
  readonly grid: Uint32Array;
}

/** `mesh`'s corners on the grid, from {@link GRID_MARGIN_M} below their least corner. */
function quantised(mesh: OccluderMesh): Quantised {
  const least = [0, 1, 2].map((k) => {
    let low = Infinity;
    for (let n = k; n < mesh.triangles.length; n += 3) {
      low = Math.min(low, mesh.triangles[n] ?? Infinity);
    }
    return low - GRID_MARGIN_M;
  });
  const grid = Uint32Array.from(mesh.triangles, (v, n) =>
    Math.round((v - (least[n % 3] ?? 0)) / GRID_STEP_M),
  );
  return { leastM: { x: least[0] ?? 0, y: least[1] ?? 0, z: least[2] ?? 0 }, grid };
}

/** The grid corners' origin from the camera, as both draws take it: narrowed once to `f32`. */
function originOf(mesh: OccluderMesh, q: Quantised): Float32Array {
  return Float32Array.from(
    [q.leastM.x, q.leastM.y, q.leastM.z],
    (m, k) => (mesh.originF32[k] ?? 0) + m,
  );
}

/** The quantised faces as the hulls' occluder: corners the grid's times its step, exactly. */
function referenceOf(mesh: OccluderMesh, q: Quantised): OccluderMesh {
  return {
    ...mesh,
    originF32: originOf(mesh, q),
    triangles: Float32Array.from(q.grid, (g) => g * GRID_STEP_M),
  };
}

/** A sphere about its corners' box's middle holding `corners` (grid units, nine a triangle), m. */
function sphereOf(corners: ArrayLike<number>): PageSphere {
  const low = [Infinity, Infinity, Infinity];
  const high = [-Infinity, -Infinity, -Infinity];
  for (let n = 0; n < corners.length; n += 1) {
    const v = (corners[n] ?? 0) * GRID_STEP_M;
    low[n % 3] = Math.min(low[n % 3] ?? Infinity, v);
    high[n % 3] = Math.max(high[n % 3] ?? -Infinity, v);
  }
  const centre = [0, 1, 2].map((k) => ((low[k] ?? 0) + (high[k] ?? 0)) / 2);
  let radiusM = 0;
  for (let n = 0; n < corners.length; n += 3) {
    radiusM = Math.max(
      radiusM,
      Math.hypot(
        (corners[n] ?? 0) * GRID_STEP_M - (centre[0] ?? 0),
        (corners[n + 1] ?? 0) * GRID_STEP_M - (centre[1] ?? 0),
        (corners[n + 2] ?? 0) * GRID_STEP_M - (centre[2] ?? 0),
      ),
    );
  }
  return {
    centreM: { x: centre[0] ?? 0, y: centre[1] ?? 0, z: centre[2] ?? 0 },
    radiusM: radiusM + 2 * GRID_STEP_M,
  };
}

/** A sphere holding `spheres`, about `about`, its radius `padM` more. */
function holding(spheres: ReadonlyArray<PageSphere>, about: Vec3, padM: number): PageSphere {
  const radiusM = Math.max(
    ...spheres.map(
      (s) =>
        Math.hypot(s.centreM.x - about.x, s.centreM.y - about.y, s.centreM.z - about.z) + s.radiusM,
    ),
  );
  return { centreM: about, radiusM: radiusM + padM + GRID_STEP_M };
}

/**
 * How far a root's corners move, m: {@link ALLOWANCE_SHARE} of the tolerance at the hull's group
 * sphere's nearest point, less two grid steps for their rounding, 0 where the camera is inside it.
 */
function offsetM(origin: Float32Array, group: PageSphere, toleranceRad: number): number {
  const distanceM =
    Math.hypot(
      (origin[0] ?? 0) + group.centreM.x,
      (origin[1] ?? 0) + group.centreM.y,
      (origin[2] ?? 0) + group.centreM.z,
    ) - group.radiusM;
  return Math.max(0, ALLOWANCE_SHARE * toleranceRad * distanceM - 2 * GRID_STEP_M);
}

/** The hull's two-level DAG's file (see the module's remarks), and how far its root moves, m. */
function hullFile(
  q: Quantised,
  origin: Float32Array,
  toleranceRad: number,
  offset: Offset,
): { readonly bytes: ArrayBuffer; readonly offsetM: number } {
  const triangles = q.grid.length / 9;
  const half = Math.ceil(triangles / 2);
  const leaves = [q.grid.subarray(0, 9 * half), q.grid.subarray(9 * half)];
  const leafSpheres = leaves.map(sphereOf);
  const whole = sphereOf(q.grid);
  // Padded by the margin, which holds every root's corners however far they move.
  const group = holding(leafSpheres, whole.centreM, GRID_MARGIN_M);
  const moved = offsetM(origin, group, toleranceRad);
  const sign = offset === "outward" ? -1 : 1;
  const centroid = [0, 1, 2].map((k) => {
    let sum = 0;
    for (let n = k; n < q.grid.length; n += 3) {
      sum += q.grid[n] ?? 0;
    }
    return sum / (q.grid.length / 3);
  });
  const root = Array.from({ length: q.grid.length }, (_, n) => {
    const base = n - (n % 3);
    const toward = [0, 1, 2].map((k) => (centroid[k] ?? 0) - (q.grid[base + k] ?? 0));
    const length = Math.hypot(...toward);
    const move = length === 0 ? 0 : sign * (moved / GRID_STEP_M) * ((toward[n % 3] ?? 0) / length);
    return Math.round((q.grid[n] ?? 0) + move);
  });
  const errorM = moved === 0 ? 0 : moved + GRID_STEP_M;
  const rootCluster: SyntheticCluster = {
    level: 1,
    generatingGroup: 0,
    consumingGroup: null,
    ownErrorM: errorM,
    parentErrorM: Infinity,
    ownSphere: group,
    parentSphere: group,
    corners: root,
  };
  const leafClusters = leaves.map((corners, k): SyntheticCluster => ({
    level: 0,
    generatingGroup: null,
    consumingGroup: 0,
    ownErrorM: 0,
    parentErrorM: errorM,
    ownSphere: leafSpheres[k] ?? whole,
    parentSphere: group,
    corners: [...corners],
  }));
  return {
    bytes: writeMeshFile({
      clusters: [rootCluster, ...leafClusters],
      groups: 1,
      pages: [[0], [1, 2]],
      rootPages: 1,
      originM: q.leastM,
      stepM: GRID_STEP_M,
    }),
    offsetM: moved,
  };
}

/** A frame of `list` from `frame`'s camera onto a fresh target of its size, colour and depth. */
async function drawn(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  frame: HullOccluderFrame,
  list: WireframeDrawList,
): Promise<{ readonly colour: ArrayBuffer; readonly depth: ArrayBuffer }> {
  const target = engine.createRenderTarget({
    name: "R14 mesh occluders",
    size: frame.viewport,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  try {
    target.render(renderer.frame(list, frame.camera, frame.viewport));
    const [colour, depth] = await Promise.all([
      engine.readTexture(target.colour),
      engine.readTexture(target.depth ?? target.colour),
    ]);
    return { colour, depth };
  } finally {
    target.dispose();
  }
}

/** The clusters the mesh occluders' last cull drew, once its reduction answers, or `null`. */
async function meshStats(
  engine: RenderEngine,
  renderer: WireframeRenderer,
): Promise<number | null> {
  const drawnMeshes = renderer.lastMeshOccluders;
  if (drawnMeshes === null) {
    return null;
  }
  // A ring of its own, so that its answer is this cull's.
  const stats = new ClusterStats(engine, "smoke mesh occluders");
  try {
    stats.sample(drawnMeshes.buckets, MESH_OCCLUDER_CONSUMER.templates, ["meshes"]);
    let answer = stats.latest();
    for (let round = 0; round < 100 && answer === null; round += 1) {
      // The reduction's read answers some time after its submission.
      // oxlint-disable-next-line no-await-in-loop
      await pause(10);
      answer = stats.latest();
    }
    return answer?.[0]?.clusters ?? null;
  } finally {
    stats.dispose();
  }
}

/** A sampled point as the details write it: `+` shown, `-` hidden, `.` off the view. */
function mark(seen: boolean | null | undefined): string {
  return seen === null || seen === undefined ? "." : seen ? "+" : "-";
}

/** An edge's state from its sampled points: shown or hidden wholly, else partly; `null` off view. */
function stateOf(points: ReadonlyArray<boolean | null>): "shown" | "hidden" | "partly" | null {
  const seen = points.filter((point) => point !== null);
  if (seen.length === 0) {
    return null;
  }
  return seen.every(Boolean) ? "shown" : seen.some(Boolean) ? "partly" : "hidden";
}

/** Whether `seen` is `want`'s colour, within half-float rounding of a token. */
function sameColour(seen: ReadonlyArray<number>, want: Float32Array): boolean {
  return [0, 1, 2].every((c) => Math.abs((seen[c] ?? Number.NaN) - (want[c] ?? 0)) < 2e-3);
}

/**
 * Each hull edge's sampled points shown in `colour`, in order, `null` for a point off the view:
 * the edges of `list`'s hull batches at {@link EDGE_SAMPLES}.
 */
function edgeSamples(
  list: WireframeDrawList,
  frame: HullOccluderFrame,
  colour: ArrayBuffer,
): Array<boolean | null> {
  const texels = halfTexels(colour);
  const out: Array<boolean | null> = [];
  for (const batch of list.lines.filter((b) => b.id.startsWith("hull:") && b.space === "view")) {
    const want = linearColour(batch.colour);
    const at = (n: number): number => batch.segments[n] ?? 0;
    for (let s = 0; s < batch.segments.length; s += 6) {
      for (const f of EDGE_SAMPLES) {
        const point = [0, 1, 2].map(
          (k) => (batch.originF32[k] ?? 0) + at(s + k) + f * (at(s + 3 + k) - at(s + k)),
        );
        const p = project(
          { x: point[0] ?? 0, y: point[1] ?? 0, z: point[2] ?? 0 },
          { orientation: frame.camera.pose.orientation, fovXRad: frame.camera.fovXRad },
          frame.viewport,
        );
        const [x, y] = [Math.floor(p.xPx), Math.floor(p.yPx)];
        const inside =
          p.inFront &&
          x >= 0 &&
          y >= 0 &&
          x < frame.viewport.widthPx &&
          y < frame.viewport.heightPx;
        out.push(inside ? sameColour(texel(texels, frame.viewport.widthPx, x, y), want) : null);
      }
    }
  }
  return out;
}

/** What one file's two draws of a frame showed (see the module's remarks). */
interface Compared {
  /** Whether the finest cut drew the reference's colour and depth to the byte. */
  readonly same: boolean;
  /** The clusters each cut drew, `null` where the counters did not answer. */
  readonly finestClusters: number | null;
  readonly tolerantClusters: number | null;
  /** The hull edges' points sampled on the view, shown at the finest cut, and flipped. */
  readonly samples: number;
  readonly shown: number;
  /** The hull edges shown at one cut and hidden at the other, wholly. */
  readonly flips: number;
  /**
   * Each edge whose sampled points differ between the cuts, where none flips wholly: its points
   * before and after, `+` shown, `-` hidden, `.` off the view.
   */
  readonly moved: ReadonlyArray<string>;
  /** How far each hull's root moved, m. */
  readonly offsetsM: ReadonlyArray<number>;
}

/**
 * `frame`'s hulls as `offset` files drawn at the finest cut and at the tolerance, against
 * `reference`.
 */
async function compared(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  frame: HullOccluderFrame,
  geometry: GeometrySettings,
  reference: { readonly colour: ArrayBuffer; readonly depth: ArrayBuffer },
  offset: Offset,
): Promise<Compared> {
  const hulls = frame.list.occluderMeshes.filter((mesh) => mesh.triangles.length >= 9);
  const pool = new GeometryPagePool(engine, pagePoolSlots(hulls.length, hulls.length));
  const toleranceRad = wireframeToleranceRad(
    geometry.meshTolerancePx.wireframeOccluder,
    frame.camera.fovXRad,
    frame.viewport,
  );
  const occluders: MeshOccluder[] = [];
  const offsetsM: number[] = [];
  for (const mesh of hulls) {
    const q = quantised(mesh);
    const file = hullFile(q, originOf(mesh, q), toleranceRad, offset);
    const read = readMeshFile(file.bytes);
    if (read.kind === "refused") {
      throw new Error(`hull ${mesh.id}'s mesh file is refused: ${read.reason}`);
    }
    const handle = pool.addMesh(read.value.index);
    for (const page of read.value.pages) {
      if (pool.insert(handle, page).kind !== "stored") {
        throw new Error(`hull ${mesh.id}'s page ${String(page.index)} is refused`);
      }
    }
    occluders.push({ id: mesh.id, mesh: handle, originF32: mesh.originF32 });
    offsetsM.push(file.offsetM);
  }
  const asMeshes: WireframeDrawList = {
    ...frame.list,
    occluderMeshes: frame.list.occluderMeshes.filter((mesh) => mesh.triangles.length < 9),
    meshOccluders: occluders,
  };
  renderer.setMeshPages(pool);
  try {
    renderer.setGeometry({
      ...geometry,
      meshTolerancePx: { ...geometry.meshTolerancePx, wireframeOccluder: 0 },
    });
    const atFinest = await drawn(engine, renderer, frame, asMeshes);
    // Its counters answer before the next draw's cull writes the buckets again.
    const finestClusters = await meshStats(engine, renderer);
    renderer.setGeometry(geometry);
    const atTolerance = await drawn(engine, renderer, frame, asMeshes);
    const tolerantClusters = await meshStats(engine, renderer);
    const before = edgeSamples(frame.list, frame, atFinest.colour);
    const after = edgeSamples(frame.list, frame, atTolerance.colour);
    let [samples, shown, flips] = [0, 0, 0];
    const moved: string[] = [];
    for (let edge = 0; edge < before.length; edge += EDGE_SAMPLES.length) {
      const was = before.slice(edge, edge + EDGE_SAMPLES.length);
      const now = after.slice(edge, edge + EDGE_SAMPLES.length);
      const seen = was.filter((point) => point !== null);
      samples += seen.length;
      shown += seen.filter(Boolean).length;
      const [stateWas, stateNow] = [stateOf(was), stateOf(now)];
      // An edge flips where it is shown at one cut and hidden at the other, wholly.
      flips += stateWas !== stateNow && stateWas !== "partly" && stateNow !== "partly" ? 1 : 0;
      if (was.some((point, k) => point !== null && point !== now[k])) {
        moved.push(
          `${offset} edge ${String(edge / EDGE_SAMPLES.length)} ${was.map(mark).join("")}→${now.map(mark).join("")}`,
        );
      }
    }
    return {
      same:
        bytesDigest(reference.colour) === bytesDigest(atFinest.colour) &&
        bytesDigest(reference.depth) === bytesDigest(atFinest.depth),
      finestClusters,
      tolerantClusters,
      samples,
      shown,
      flips,
      moved,
      offsetsM,
    };
  } finally {
    renderer.setMeshPages(null);
    renderer.setGeometry(geometry);
    pool.dispose();
  }
}

/** R14.T5.f's check (see the module's remarks) over the group's hull frames. */
export async function checkMeshOccluders(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  ready: boolean,
  frames: ReadonlyArray<HullOccluderFrame>,
  geometry: GeometrySettings,
  checks: Checks,
): Promise<void> {
  // The wireframe's own frames of a craft: its hull's edges drawn, its faces depth alone.
  const drawnFrames = frames.filter(
    (frame) =>
      frame.list.lines.some((batch) => batch.id.startsWith("hull:")) &&
      frame.list.occluderMeshes.every((mesh) => mesh.fill === null),
  );
  const runs: Array<Compared & { readonly frame: number; readonly offset: Offset }> = [];
  renderer.setGeometry(geometry);
  for (const [n, frame] of drawnFrames.entries()) {
    const references = frame.list.occluderMeshes.map((mesh) =>
      mesh.triangles.length < 9 ? mesh : referenceOf(mesh, quantised(mesh)),
    );
    // Each frame's draws are read back before the next frame's: the renderer's buffers are one set.
    // oxlint-disable-next-line no-await-in-loop
    const reference = await drawn(engine, renderer, frame, {
      ...frame.list,
      occluderMeshes: references,
    });
    for (const offset of ["outward", "inward"] as const) {
      // Each file's draws are read back before the next file's pool is made.
      // oxlint-disable-next-line no-await-in-loop
      const run = await compared(engine, renderer, frame, geometry, reference, offset);
      runs.push({ ...run, frame: n, offset });
    }
  }
  const differing = runs
    .filter((run) => !run.same)
    .map((run) => `frame ${String(run.frame)} ${run.offset}`);
  const counted = runs.filter(
    (run) => run.finestClusters !== null && run.tolerantClusters !== null,
  );
  const coarser = counted.filter((run) => (run.tolerantClusters ?? 0) < (run.finestClusters ?? 0));
  const offsets = runs.flatMap((run) => run.offsetsM).filter((m) => m > 0);
  const sum = (pick: (run: Compared) => number): number =>
    runs.reduce((total, run) => total + pick(run), 0);
  const samples = sum((run) => run.samples);
  const shown = sum((run) => run.shown);
  const flips = sum((run) => run.flips);
  const movedEdges = runs.flatMap((run) => run.moved.map((m) => `frame ${String(run.frame)} ${m}`));
  checks.check(
    "R14.T5.f the DAG consumers' occluders draw the hidden-line frames: at the finest cut to the byte as the hulls' faces, and at meshTolerancePx.wireframeOccluder, their coarser faces nearer or farther, with no hull edge flipping between shown and hidden",
    ready &&
      drawnFrames.length > 0 &&
      counted.length === runs.length &&
      differing.length === 0 &&
      coarser.length > 0 &&
      shown > 0 &&
      shown < samples &&
      flips === 0,
    `cull kernel made ${String(ready)}; ${String(drawnFrames.length)} hull frames, each with its hulls' roots moved outward and inward; at τ 0 ${differing.length === 0 ? "every draw the hulls' to the byte" : `differing ${differing.join(", ")}`}; at ${String(geometry.meshTolerancePx.wireframeOccluder)} px ${String(coarser.length)} of ${String(runs.length)} draws cut coarser (${String(counted.length)} counted), roots moved ${offsets.length === 0 ? "nowhere" : `${(100 * Math.min(...offsets)).toFixed(2)} to ${(100 * Math.max(...offsets)).toFixed(2)} cm`}; ${String(samples)} hull-edge points sampled, ${String(shown)} shown; ${String(flips)} edges flipped between shown and hidden; ${String(movedEdges.length)} edges' points moved${movedEdges.length === 0 ? "" : ` (${movedEdges.join("; ")})`}`,
  );
}
