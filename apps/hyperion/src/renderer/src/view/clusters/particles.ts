/**
 * The cluster path's particles (plan R14, R14.T5.d): the instance record R11's scatter and ring
 * particles place on the GPU, the templates a particle is drawn by, one a size, and the vertex
 * library that places a template's vertex for an instance of a bucket's list.
 *
 * @remarks
 * - **The record**, {@link PARTICLE_INSTANCE_BYTES} an instance, as `particleInstance.wgsl`'s
 *   `ParticleInstance` lays it out: its centre less the camera, three `f32`, m, in the frame the
 *   passes are in (the instance cull's words 0 to 2), its radius, m (word 3: the sphere a template
 *   draws, its scale, whose bounds the cull tests widened by the templates' bounds scale; negative
 *   for no instance), its rotation, a
 *   unit quaternion (x, y, z, w) as four `snorm16` over two words (`pack2x16snorm`), the template's
 *   axes into the passes' frame, then the consumer's two words: a colour, `rgba8unorm`, and a tag.
 *   R11's placement writes it on the GPU (R11.T6.a's pebbles, R11.T9.d's ring cells), and the
 *   count of instances placed into the path's count word.
 * - **The templates**, coarsest first (`sphereParticleTemplates`): polyhedra of the unit sphere, the
 *   octahedron (8 triangles), the icosahedron (20) and the icosahedron divided once and twice (80,
 *   320), each scaled by k = √(4π ÷ S), S its surface's area, so that its mean projected area over
 *   every direction, S k² ÷ 4 (Cauchy's surface-area formula: a convex body's mean projected area is
 *   a quarter of its surface's; Cauchy 1841, C. R. Acad. Sci. Paris 13, 1060–1065), is the sphere's
 *   π: a particle a few pixels across covers on average what its sphere would, which an inscribed
 *   octahedron would draw at 0.55 of (R11.T9.d's photometry, R11 Design note 16). Each lies between
 *   its least face's distance from the centre and its vertices' k ({@link ParticleTemplates.extents}),
 *   each face's nearest point to the centre inside it (`particles.test.ts`), so that its error
 *   fraction ε ÷ r, the farthest its surface lies from the sphere, is the larger of k − 1 and one
 *   less that distance, and the instance's bounds are its sphere widened by the largest k
 *   ({@link ParticleTemplates.boundsScale}). The cull draws an instance by the coarsest template whose
 *   ε r is within its pass's tolerance at the bounds (`instanceCull.wgsl`'s `instance_template`), the
 *   finest where none is. A consumer may bring templates of its own, with their error fractions and
 *   bounds scale, at most {@link MAX_PARTICLE_TEMPLATES}.
 * - **The vertex library** (`particleInstance.wgsl`, {@link PARTICLE_INSTANCE_WGSL}): a bucket
 *   draw's instance `entry` reads its list's entry from its `Draw`'s `clusterBase` (an instance's
 *   index), the instance's record, and places the template's vertex at its centre plus its radius
 *   times the vertex turned by its rotation, its normal the turned vertex's direction. A consumer's
 *   material composes it after `frame.wgsl` and its own `Draw`, which declares `clusterBase`.
 * - **The low setting** has no instance pipeline: {@link instancePathOn} is false where R11's
 *   ladder draws no decorative scatter (decoration off, or a scatter density of 0) and no ring
 *   particles (the annulus), as low has it (R11 Design note 18), so that no particle's kernel or
 *   material is made there.
 */

import { normalise, type Vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import type { MeshSpec } from "../engine/types";
import type { R11Ladder } from "../surface/decoration/decorationSetting";
import particleInstanceWgsl from "./particleInstance.wgsl?raw";
import type { ClusterTemplate } from "./templates";

/** Bytes of a particle's instance record (see the module's remarks). */
export const PARTICLE_INSTANCE_BYTES = 32;

/** `u32` words of a particle's instance record. */
export const PARTICLE_INSTANCE_WORDS = PARTICLE_INSTANCE_BYTES / 4;

/** The record's word that holds its radius, the instance cull's bounds' (`InstanceCullParams`). */
export const PARTICLE_RADIUS_WORD = 3;

/** The most templates a particle has: the instance cull's error fractions, one `vec4f`. */
export const MAX_PARTICLE_TEMPLATES = 4;

/** `particleInstance.wgsl`, the particles' vertex library (see the module's remarks). */
export const PARTICLE_INSTANCE_WGSL: string = particleInstanceWgsl;

/** The `@group(2)` bindings the vertex library declares: the instances and the buckets' lists. */
export const PARTICLE_BINDINGS = { instances: 0, lists: 1 } as const;

/** The storage buffers a particle material binds, by the names its draws give them. */
export const PARTICLE_STORAGE_BUFFERS = [
  { name: "particleInstances", binding: PARTICLE_BINDINGS.instances, stages: ["vertex"] },
  { name: "particleLists", binding: PARTICLE_BINDINGS.lists, stages: ["vertex"] },
] as const;

/** One particle as the CPU packs it (see the module's remarks). */
export interface ParticleInstance {
  /** Its centre less the camera, in the passes' frame, m. */
  readonly centreM: Vec3;
  /** Its bounding sphere's radius, m, its template's scale. */
  readonly radiusM: number;
  /** Its template's axes into the passes' frame, a unit quaternion. */
  readonly rotation: Quaternion;
  /** The consumer's colour, `rgba8unorm`: red in the low byte. */
  readonly colour: number;
  /** The consumer's tag. */
  readonly tag: number;
}

/**
 * A component of a unit quaternion as WGSL's `pack2x16snorm` packs it: round(clamp(v, −1, 1) ×
 * 32,767), WGSL's `round` taking a half to the even neighbour.
 */
function snorm16(value: number): number {
  const scaled = Math.min(Math.max(value, -1), 1) * 32_767;
  const floor = Math.floor(scaled);
  const fraction = scaled - floor;
  const rounded = fraction > 0.5 || (fraction === 0.5 && floor % 2 !== 0) ? floor + 1 : floor;
  return rounded & 0xffff;
}

/**
 * Packs `particle` at `offsetBytes` of `out`, as `particleInstance.wgsl`'s `ParticleInstance`.
 *
 * @throws RangeError for a colour or a tag that is not a `u32`.
 */
export function packParticleInstance(
  out: DataView,
  offsetBytes: number,
  particle: ParticleInstance,
): void {
  const { centreM: c, rotation: q, colour, tag } = particle;
  for (const [name, value] of [
    ["colour", colour],
    ["tag", tag],
  ] as const) {
    if (!Number.isInteger(value) || value < 0 || value > 0xffff_ffff) {
      throw new RangeError(`a particle's ${name} is a u32, not ${String(value)}`);
    }
  }
  out.setFloat32(offsetBytes, c.x, true);
  out.setFloat32(offsetBytes + 4, c.y, true);
  out.setFloat32(offsetBytes + 8, c.z, true);
  out.setFloat32(offsetBytes + 12, particle.radiusM, true);
  out.setUint32(offsetBytes + 16, (snorm16(q.x) | (snorm16(q.y) << 16)) >>> 0, true);
  out.setUint32(offsetBytes + 20, (snorm16(q.z) | (snorm16(q.w) << 16)) >>> 0, true);
  out.setUint32(offsetBytes + 24, colour, true);
  out.setUint32(offsetBytes + 28, tag, true);
}

/** A particle's templates: their mesh, each template's range of it, its extent and its error. */
export interface ParticleTemplates {
  /** Coarsest first, each `particle` at stride 1, its range of {@link ParticleTemplates.mesh}. */
  readonly templates: ReadonlyArray<ClusterTemplate>;
  /**
   * Each template's least and most distances of its surface from the centre, over the radius: its
   * least face's and its vertices' (see the module's remarks).
   */
  readonly extents: ReadonlyArray<readonly [inner: number, outer: number]>;
  /**
   * Each template's error fraction, ε ÷ r, the farthest its surface lies from the unit sphere, the
   * larger of its outer extent less one and one less its inner (see the module's remarks),
   * decreasing.
   */
  readonly errorFractions: ReadonlyArray<number>;
  /** The largest outer extent: an instance's bounds over its radius, which the cull tests. */
  readonly boundsScale: number;
  /** One mesh of every template, `position` each vertex at its template's scale, indices in turn. */
  readonly mesh: MeshSpec;
}

/** A polyhedron of the unit sphere: its vertices and its triangles, wound outward. */
interface Polyhedron {
  readonly vertices: ReadonlyArray<Vec3>;
  readonly triangles: ReadonlyArray<readonly [number, number, number]>;
}

/** The octahedron on the unit axes. */
function octahedron(): Polyhedron {
  const vertices: Vec3[] = [
    { x: 1, y: 0, z: 0 },
    { x: -1, y: 0, z: 0 },
    { x: 0, y: 1, z: 0 },
    { x: 0, y: -1, z: 0 },
    { x: 0, y: 0, z: 1 },
    { x: 0, y: 0, z: -1 },
  ];
  const triangles: Array<readonly [number, number, number]> = [];
  for (const x of [0, 1]) {
    for (const y of [2, 3]) {
      for (const z of [4, 5]) {
        triangles.push([x, y, z]);
      }
    }
  }
  return outward({ vertices, triangles });
}

/** The icosahedron, its vertices the cyclic (0, ±1, ±φ) on the unit sphere. */
function icosahedron(): Polyhedron {
  const phi = (1 + Math.sqrt(5)) / 2;
  const raw: Vec3[] = [];
  for (const a of [-1, 1]) {
    for (const b of [-phi, phi]) {
      raw.push({ x: 0, y: a, z: b }, { x: a, y: b, z: 0 }, { x: b, y: 0, z: a });
    }
  }
  const vertices = raw.map(normalise);
  // Its faces: the triples of vertices each two an edge apart, the edge the least distance.
  const edge = Math.min(...vertices.slice(1).map((v) => distance(v, vertices[0] ?? v)));
  const adjacent = (i: number, j: number): boolean =>
    Math.abs(distance(vertices[i] ?? ORIGIN, vertices[j] ?? ORIGIN) - edge) < 1e-9;
  const triangles: Array<readonly [number, number, number]> = [];
  for (let i = 0; i < vertices.length; i += 1) {
    for (let j = i + 1; j < vertices.length; j += 1) {
      for (let k = j + 1; k < vertices.length; k += 1) {
        if (adjacent(i, j) && adjacent(j, k) && adjacent(i, k)) {
          triangles.push([i, j, k]);
        }
      }
    }
  }
  return outward({ vertices, triangles });
}

const ORIGIN: Vec3 = { x: 0, y: 0, z: 0 };

function distance(a: Vec3, b: Vec3): number {
  return Math.hypot(a.x - b.x, a.y - b.y, a.z - b.z);
}

/** `p`, each triangle wound so that its normal by the right hand points away from the centre. */
function outward(p: Polyhedron): Polyhedron {
  return {
    vertices: p.vertices,
    triangles: p.triangles.map(([a, b, c]) => {
      const n = faceNormal(p, [a, b, c]);
      const va = p.vertices[a] ?? ORIGIN;
      return n.x * va.x + n.y * va.y + n.z * va.z >= 0 ? [a, b, c] : [a, c, b];
    }),
  };
}

/** A triangle's normal by the right hand, (b − a) × (c − a), unnormalised. */
function faceNormal(p: Polyhedron, [a, b, c]: readonly [number, number, number]): Vec3 {
  const va = p.vertices[a] ?? ORIGIN;
  const vb = p.vertices[b] ?? ORIGIN;
  const vc = p.vertices[c] ?? ORIGIN;
  const u = { x: vb.x - va.x, y: vb.y - va.y, z: vb.z - va.z };
  const v = { x: vc.x - va.x, y: vc.y - va.y, z: vc.z - va.z };
  return { x: u.y * v.z - u.z * v.y, y: u.z * v.x - u.x * v.z, z: u.x * v.y - u.y * v.x };
}

/** `p` divided once: each triangle into four, its edges' midpoints pushed onto the sphere. */
function divided(p: Polyhedron): Polyhedron {
  const vertices = [...p.vertices];
  const midpoints = new Map<string, number>();
  const midpoint = (i: number, j: number): number => {
    const key = i < j ? `${String(i)} ${String(j)}` : `${String(j)} ${String(i)}`;
    const held = midpoints.get(key);
    if (held !== undefined) {
      return held;
    }
    const a = vertices[i] ?? ORIGIN;
    const b = vertices[j] ?? ORIGIN;
    vertices.push(normalise({ x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }));
    midpoints.set(key, vertices.length - 1);
    return vertices.length - 1;
  };
  const triangles: Array<readonly [number, number, number]> = [];
  for (const [a, b, c] of p.triangles) {
    const ab = midpoint(a, b);
    const bc = midpoint(b, c);
    const ca = midpoint(c, a);
    triangles.push([a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]);
  }
  return { vertices, triangles };
}

/**
 * A polyhedron's extent about its centre, as {@link ParticleTemplates.extents} holds it: its least
 * face plane's distance and its farthest vertex's, each face's nearest point inside it.
 */
export function polyhedronExtent(
  vertices: ReadonlyArray<Vec3>,
  triangles: ReadonlyArray<readonly [number, number, number]>,
): readonly [inner: number, outer: number] {
  let inner = Infinity;
  for (const triangle of triangles) {
    const n = normalise(faceNormal({ vertices, triangles }, triangle));
    const a = vertices[triangle[0]] ?? ORIGIN;
    inner = Math.min(inner, n.x * a.x + n.y * a.y + n.z * a.z);
  }
  const outer = Math.max(...vertices.map((v) => Math.hypot(v.x, v.y, v.z)));
  return [inner, outer];
}

/** The error fraction of a template of `extent`: the larger of its outer less one and one less its inner. */
export function extentErrorFraction(extent: readonly [number, number]): number {
  return Math.max(extent[1] - 1, 1 - extent[0]);
}

/** A polyhedron's surface's area. */
function surfaceArea(p: Polyhedron): number {
  let area = 0;
  for (const triangle of p.triangles) {
    const n = faceNormal(p, triangle);
    area += Math.hypot(n.x, n.y, n.z) / 2;
  }
  return area;
}

/**
 * `p` scaled about its centre so that its mean projected area is the unit sphere's, π (see the
 * module's remarks): by √(4π ÷ S).
 */
function areaMatched(p: Polyhedron): Polyhedron {
  const k = Math.sqrt((4 * Math.PI) / surfaceArea(p));
  return {
    vertices: p.vertices.map((v) => ({ x: k * v.x, y: k * v.y, z: k * v.z })),
    triangles: p.triangles,
  };
}

/**
 * The polyhedra the particles are drawn by, coarsest first, each of the unit sphere's mean projected
 * area (see the module's remarks).
 */
function spherePolyhedra(): ReadonlyArray<Polyhedron> {
  const ico = icosahedron();
  const ico80 = divided(ico);
  return [octahedron(), ico, ico80, divided(ico80)].map(areaMatched);
}

/**
 * The particles' sphere templates (see the module's remarks): the octahedron, the icosahedron and
 * the icosahedron divided once and twice, 8, 20, 80 and 320 triangles, each of its sphere's mean
 * projected area, in one mesh.
 */
export function sphereParticleTemplates(): ParticleTemplates {
  const polyhedra = spherePolyhedra();
  const positions: number[] = [];
  const indices: number[] = [];
  const templates: ClusterTemplate[] = [];
  const extents: Array<readonly [number, number]> = [];
  for (const p of polyhedra) {
    const base = positions.length / 3;
    for (const v of p.vertices) {
      positions.push(v.x, v.y, v.z);
    }
    const firstIndex = indices.length;
    for (const [a, b, c] of p.triangles) {
      indices.push(base + a, base + b, base + c);
    }
    const extent = polyhedronExtent(p.vertices, p.triangles);
    extents.push(extent);
    templates.push({
      shape: "particle",
      stride: 1,
      triangles: p.triangles.length,
      firstIndex,
      indexCount: 3 * p.triangles.length,
      errorScale: extentErrorFraction(extent),
      mayDiscard: false,
    });
  }
  return {
    templates,
    extents,
    errorFractions: extents.map(extentErrorFraction),
    boundsScale: Math.max(...extents.map(([, outer]) => outer)),
    mesh: {
      name: "particle templates",
      positions: Float32Array.from(positions),
      indices: Uint32Array.from(indices),
      topology: "triangle-list",
      attributes: {},
    },
  };
}

/**
 * Whether a view draws any instanced particle, and so makes the instance path's kernel and
 * materials (see the module's remarks): decorative scatter (decoration on and a scatter density
 * above 0) or ring particles; false on the low setting.
 */
export function instancePathOn(
  ladder: Pick<R11Ladder, "decoration" | "scatterDensity" | "rings">,
): boolean {
  return (ladder.decoration && ladder.scatterDensity > 0) || ladder.rings === "particles";
}
