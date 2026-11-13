/**
 * Normal maps for the disc's tests and its smoke run (plan R10, R10.T10.g; `bodies/discNormals.ts`):
 * the octahedral pair of a unit vector as the surface crate encodes it, and maps over a field's cells
 * of the datum's normals, of one vector, or of cells tilted in a checkerboard; and the engine stub
 * the twins' surfaces are made through.
 */
import { add, cross, dot, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import type { DiscNormalMap } from "../view/appearance/bodyAppearance";
import { type NormalMapTexels, packNormalMap } from "../view/bodies/discNormals";
import type { RenderEngine } from "../view/engine/types";
import { fieldCellIndex } from "../view/terrain/coverage";
import { faceUvToXyz, stToUv } from "../view/terrain/cube";
import type { Face } from "../view/terrain/patchKey";

const FACES: ReadonlyArray<Face> = [0, 1, 2, 3, 4, 5];

/**
 * An engine whose textures nothing reads, for surfaces only the CPU twins and the point read: it
 * names each texture after its specification and writes nothing.
 */
export const UNREAD_TEXTURES: Pick<RenderEngine, "createTexture" | "writeTexture"> = {
  createTexture: (spec) => ({ kind: "texture", name: spec.name }),
  writeTexture: () => undefined,
};

/** `texels` as a surface carries them, beside a texture of `name` that nothing reads. */
export function unreadNormalMap(texels: NormalMapTexels, name = "test normals"): DiscNormalMap {
  return { texture: { kind: "texture", name }, texels };
}

/**
 * The body-fixed direction of the point (s, t) of `face`'s cell coordinates, not normalised:
 * `faceUvToXyz` through the cube's warp.
 */
export function faceDirection(face: Face, s: number, t: number): readonly [number, number, number] {
  return faceUvToXyz(face, stToUv(s), stToUv(t));
}

/** The octahedral pair of the unit vector `n`: the surface crate's `encode_octahedral`. */
export function octahedralPair(n: Vec3): [number, number] {
  const l1 = Math.abs(n.x) + Math.abs(n.y) + Math.abs(n.z);
  const x = n.x / l1;
  const y = n.y / l1;
  if (n.z >= 0) {
    return [x, y];
  }
  return [(1 - Math.abs(y)) * (x < 0 ? -1 : 1), (1 - Math.abs(x)) * (y < 0 ? -1 : 1)];
}

/** The unit direction of the point (s, t) of `face`'s cell coordinates, body-fixed. */
export function directionAt(face: Face, s: number, t: number): Vec3 {
  const [x, y, z] = faceDirection(face, s, t);
  return normalise(vec3(x, y, z));
}

/**
 * The datum's outward unit normal at the unit direction `d` of a spheroid of c ÷ a `cOverA` about
 * the body-fixed z axis: R05's point of d is (d_x, d_y, d_z c ÷ a), its normal along
 * (d_x, d_y, d_z a ÷ c).
 */
export function datumNormalAt(d: Vec3, cOverA: number): Vec3 {
  return normalise(vec3(d.x, d.y, d.z / cOverA));
}

/** One cell of a field's level: its face, its place and its centre's unit direction. */
export interface FieldCell {
  readonly face: Face;
  readonly i: number;
  readonly j: number;
  readonly centre: Vec3;
}

/**
 * A normal map at field level `level` (5 to 8) holding `normalOf(cell)` at each cell, or no normal
 * where it gives `null`, packed as a worker's answer is (`packNormalMap`, its cells rising).
 */
export function normalMapOf(
  level: number,
  normalOf: (cell: FieldCell) => Vec3 | null,
): NormalMapTexels {
  const n = 2 ** level;
  const cells: number[] = [];
  const pairs: number[] = [];
  for (const face of FACES) {
    for (let j = 0; j < n; j += 1) {
      for (let i = 0; i < n; i += 1) {
        const normal = normalOf({
          face,
          i,
          j,
          centre: directionAt(face, (i + 0.5) / n, (j + 0.5) / n),
        });
        if (normal !== null) {
          cells.push(fieldCellIndex({ face, level, i, j }));
          pairs.push(...octahedralPair(normal));
        }
      }
    }
  }
  const order = cells.map((_, k) => k).toSorted((a, b) => (cells[a] ?? 0) - (cells[b] ?? 0));
  return packNormalMap(
    level,
    Uint32Array.from(order.map((k) => cells[k] ?? 0)),
    Float16Array.from(order.flatMap((k) => [pairs[2 * k] ?? 0, pairs[2 * k + 1] ?? 0])),
  );
}

/**
 * The unit tangent along +s of the cell about `cell`'s centre, perpendicular to `normal`: the
 * direction the checkerboard's cells tilt in.
 */
function tangentAlongS(cell: FieldCell, n: number, normal: Vec3): Vec3 {
  const across = sub(
    directionAt(cell.face, (cell.i + 0.75) / n, (cell.j + 0.5) / n),
    directionAt(cell.face, (cell.i + 0.25) / n, (cell.j + 0.5) / n),
  );
  return normalise(sub(across, scale(normal, dot(across, normal))));
}

/** `normal` tilted by `tiltRad` towards the unit tangent `towards`. */
export function tilted(normal: Vec3, towards: Vec3, tiltRad: number): Vec3 {
  return normalise(add(scale(normal, Math.cos(tiltRad)), scale(towards, Math.sin(tiltRad))));
}

/**
 * A normal map of each cell's datum normal tilted along its face's +s by `tiltOf(cell)`, rad
 * (negative for −s), or no normal where it gives `null`.
 *
 * @param cOverA - The figure's c ÷ a, whose datum normals are tilted; 1 for a sphere.
 */
export function slopedNormalMap(
  level: number,
  tiltOf: (cell: FieldCell) => number | null,
  cOverA = 1,
): NormalMapTexels {
  const n = 2 ** level;
  return normalMapOf(level, (cell) => {
    const tiltRad = tiltOf(cell);
    if (tiltRad === null) {
      return null;
    }
    const datum = datumNormalAt(cell.centre, cOverA);
    return tilted(datum, tangentAlongS(cell, n, datum), tiltRad);
  });
}

/**
 * A normal map of each cell's datum normal tilted by `tiltRad` along its face's +s where i + j is
 * even and by −`tiltRad` where it is odd: the slopes of a ground of ridges a cell apart, at the
 * cells `surveyed` keeps (all by default).
 *
 * @param cOverA - The figure's c ÷ a, whose datum normals are tilted; 1 for a sphere.
 */
export function checkerNormalMap(
  level: number,
  tiltRad: number,
  cOverA = 1,
  surveyed: (cell: FieldCell) => boolean = () => true,
): NormalMapTexels {
  return slopedNormalMap(
    level,
    (cell) => (surveyed(cell) ? ((cell.i + cell.j) % 2 === 0 ? tiltRad : -tiltRad) : null),
    cOverA,
  );
}

/** A normal map of each cell's datum normal at its centre, at the cells `surveyed` keeps. */
export function datumNormalMap(
  level: number,
  cOverA = 1,
  surveyed: (cell: FieldCell) => boolean = () => true,
): NormalMapTexels {
  return normalMapOf(level, (cell) => (surveyed(cell) ? datumNormalAt(cell.centre, cOverA) : null));
}

/**
 * A normal map holding the one body-fixed unit vector `normal` at every cell within `radiusRad` of
 * the unit direction `about`: wherever a point's four nearest cells hold it, its reconstruction is
 * `normal` itself.
 */
export function constantNormalMap(
  level: number,
  normal: Vec3,
  about: Vec3,
  radiusRad: number,
): NormalMapTexels {
  const cosine = Math.cos(radiusRad);
  return normalMapOf(level, (cell) => (dot(cell.centre, about) >= cosine ? normal : null));
}

/** A unit vector perpendicular to the unit vector `d`, turned `turnRad` about it from a fixed seed. */
export function perpendicularTo(d: Vec3, turnRad = 0): Vec3 {
  const seed = Math.abs(d.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  const u = normalise(cross(seed, d));
  const w = cross(d, u);
  return add(scale(u, Math.cos(turnRad)), scale(w, Math.sin(turnRad)));
}
