/**
 * The procedural builder's rocks (plan R14, R14.T5.c, with R11.T4): a finest patch's rock layer
 * tessellated within R11's chordal bound (Design note 6; `rockMesh.ts`) and clustered into mesh
 * pages, the patch's own mesh, which the page pool holds and the cluster path draws.
 *
 * @remarks
 * - **The mesh's frame** is the patch's: body-fixed axes about the patch's origin, the rock's base
 *   centre less the origin taken in `f64`, so that the mesh is placed as the patch's vertices are,
 *   its pages' grid a few micrometres at a 32 m patch.
 * - **Clusters**: each rock at its own level ({@link rockLevel}), each of its six cube faces cut
 *   into tiles of at most {@link ROCK_TILE_QUADS} quads a side, 128 triangles, a tile a cluster,
 *   its seam vertices the same bits as its neighbours'. Every cluster of a rock takes the rock's
 *   sphere and the errors (0, its largest diameter), so that the cut keeps or refuses the rock
 *   whole: drawn wherever it subtends more than the camera pass's tolerance, and always at a
 *   tolerance of 0 (the could-touch region's finest cut, R11 Design note 7). A vertex's fourth word
 *   is its rock's index in the layer.
 * - **The rocks' records** ({@link ROCK_RECORD_WORDS} words a rock, {@link RockPages.records}): what
 *   the shading reads of each rock beside its vertices: its base centre less the patch's origin,
 *   body-fixed m, `f32`; the ground's height at its axis, m; the datum's normal there; and its
 *   place in the patch, (x, y) ÷ 64 of its axis, as two unorm16 in one word, at which the patch's
 *   own layers (its class weights and its horizon map) are read for it.
 * - Every byte is a function of the layer, the origin and the key: the same in the mesh page worker
 *   (`meshPageWork.ts`) and on the render thread. It imports nothing that needs the engine.
 */

import { type Vec3, vec3 } from "../../../geometry/vec3";
import { type ProceduralCluster, writeProceduralMesh } from "../../clusters/meshWrite";
import { uvToSt, xyzToFaceUv } from "../../terrain/cube";
import type { PatchKey } from "../../terrain/patchKey";
import { type Rock, type RockLayer, rocksOf } from "./rockLayer";
import { rockLevel, tessellateRock } from "./rockMesh";

/** The quads along a cluster's tile's side: 8 × 8 quads, 128 triangles over 81 vertices. */
export const ROCK_TILE_QUADS = 8;

/** The words a rock's record takes (see the module's remarks). */
export const ROCK_RECORD_WORDS = 8;

/** A patch's rock mesh as the builder makes it. */
export interface RockPages {
  /** The mesh file (`pageFormat.ts`' format, version 2), a one-level DAG of root pages. */
  readonly file: ArrayBuffer;
  /** The rocks' records, {@link ROCK_RECORD_WORDS} 32-bit words a rock, in the layer's order. */
  readonly records: ArrayBuffer;
  /** How many rocks, and their triangles, for the counts. */
  readonly rocks: number;
  readonly triangles: number;
}

/** What the builder takes of a patch: its key, its origin and its rock layer's words. */
export interface RockPagesInput {
  readonly key: PatchKey;
  /** The patch's origin, body-fixed m. */
  readonly originM: Vec3;
  readonly layer: RockLayer;
}

/** A unorm16 of `x` in [0, 1]. */
function unorm16(x: number): number {
  return Math.round(Math.min(1, Math.max(0, x)) * 65_535);
}

/** The place of the unit direction `dir` in `key`'s patch, (x, y) ÷ 64, each in [0, 1]. */
export function placeInPatch(key: PatchKey, dir: Vec3): readonly [number, number] {
  const { u, v } = xyzToFaceUv([dir.x, dir.y, dir.z]);
  const side = 2 ** key.level;
  return [uvToSt(u) * side - key.i, uvToSt(v) * side - key.j];
}

/** The rock's tessellation's clusters about `originM`, its index `n` in their fourth word. */
function rockClusters(rock: Rock, n: number, originM: Vec3): ProceduralCluster[] {
  const level = rockLevel(rock);
  const mesh = tessellateRock(rock, level);
  const base = vec3(
    rock.baseCentreM.x - originM.x,
    rock.baseCentreM.y - originM.y,
    rock.baseCentreM.z - originM.z,
  );
  const [a, b, c] = rock.semiAxesM;
  const sphere = { centreM: base, radiusM: Math.hypot(a, b, c) };
  const diameterM = 2 * Math.max(a, b, c);
  const side = level + 1;
  const tiles = Math.ceil(level / ROCK_TILE_QUADS);
  const clusters: ProceduralCluster[] = [];
  for (let face = 0; face < 6; face += 1) {
    const first = face * side * side;
    for (let tj = 0; tj < tiles; tj += 1) {
      for (let ti = 0; ti < tiles; ti += 1) {
        const [i0, j0] = [ti * ROCK_TILE_QUADS, tj * ROCK_TILE_QUADS];
        const [i1, j1] = [
          Math.min(level, i0 + ROCK_TILE_QUADS),
          Math.min(level, j0 + ROCK_TILE_QUADS),
        ];
        const width = i1 - i0 + 1;
        const vertices = width * (j1 - j0 + 1);
        const positionsM = new Float64Array(3 * vertices);
        const normals = new Float64Array(3 * vertices);
        let v = 0;
        for (let j = j0; j <= j1; j += 1) {
          for (let i = i0; i <= i1; i += 1) {
            const at = 3 * (first + j * side + i);
            positionsM[3 * v] = base.x + (mesh.positionsM[at] ?? 0);
            positionsM[3 * v + 1] = base.y + (mesh.positionsM[at + 1] ?? 0);
            positionsM[3 * v + 2] = base.z + (mesh.positionsM[at + 2] ?? 0);
            normals.set(mesh.normals.subarray(at, at + 3), 3 * v);
            v += 1;
          }
        }
        const triangles = new Uint8Array(6 * (i1 - i0) * (j1 - j0));
        let t = 0;
        for (let j = 0; j < j1 - j0; j += 1) {
          for (let i = 0; i < i1 - i0; i += 1) {
            const v00 = j * width + i;
            const [v10, v01, v11] = [v00 + 1, v00 + width, v00 + width + 1];
            triangles.set([v00, v10, v11, v00, v11, v01], t);
            t += 6;
          }
        }
        clusters.push({
          positionsM,
          normals,
          words: new Uint32Array(vertices).fill(n),
          triangles,
          sphere,
          errorsM: [0, diameterM],
          material: 0,
        });
      }
    }
  }
  return clusters;
}

/**
 * The patch's rock mesh and the rocks' records (see the module's remarks), or `null` for a layer
 * of no rocks.
 */
export function buildRockPages(input: RockPagesInput): RockPages | null {
  const rocks = rocksOf(input.layer);
  if (rocks.length === 0) {
    return null;
  }
  const records = new ArrayBuffer(4 * ROCK_RECORD_WORDS * rocks.length);
  const view = new DataView(records);
  const clusters = rocks.flatMap((rock, n) => {
    const at = 4 * ROCK_RECORD_WORDS * n;
    const base = [
      rock.baseCentreM.x - input.originM.x,
      rock.baseCentreM.y - input.originM.y,
      rock.baseCentreM.z - input.originM.z,
    ];
    base.forEach((x, k) => {
      view.setFloat32(at + 4 * k, x, true);
    });
    view.setFloat32(at + 12, rock.groundHeightM, true);
    const up = rock.axes[2];
    [up.x, up.y, up.z].forEach((x, k) => {
      view.setFloat32(at + 16 + 4 * k, x, true);
    });
    const [px, py] = placeInPatch(input.key, rock.dir);
    view.setUint32(at + 28, (unorm16(px) | (unorm16(py) << 16)) >>> 0, true);
    return rockClusters(rock, n, input.originM);
  });
  const triangles = clusters.reduce((sum, c) => sum + c.triangles.length / 3, 0);
  return { file: writeProceduralMesh(clusters), records, rocks: rocks.length, triangles };
}
