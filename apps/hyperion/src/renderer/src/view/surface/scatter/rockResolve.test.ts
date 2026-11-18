/**
 * The rocks by the visibility buffer (plan R14, R14.T5.e; `rockResolve.wgsl`, `rockId.wgsl`): the
 * twins of what the resolve reconstructs of a pixel, its triangle's corners from the page, its
 * barycentrics from their snapped clip positions and its geometric normal from their edges, against
 * the forward draw's own, and the materials' composition and bindings.
 */

import { describe, expect, it } from "vitest";

import { cross, dot, normalise, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { meshPagesOf, rockLayerFrom, rocksAbout } from "../../../test/rockFixtures";
import {
  CLUSTER_TABLE_BYTES,
  type MeshPage,
  PAGE_RECORD_BYTES,
  PAGE_TRIANGLE_BYTES,
  PAGE_VERTEX_BYTES,
  pageClusterCorners,
} from "../../clusters/pageFormat";
import { type ClipPosition, clipBarycentrics, snappedClip } from "../../clusters/resolve";
import type { WgslMaterialSpec } from "../../engine/types";
import { finestLevel, uvToSt, vertexDir, xyzToFaceUv } from "../../terrain/cube";
import type { PatchKey } from "../../terrain/patchKey";
import {
  ROCK_BINDINGS,
  rockIdMaterialSpec,
  rockLitMaterialSpec,
  rockResolveMaterialSpec,
} from "./rockMaterial";
import rockWgsl from "./rock.wgsl?raw";
import rockPageWgsl from "./rockPage.wgsl?raw";
import { buildRockPages } from "./rockPages";

const RADIUS_M = 1_737_400;

/** The frame the twins project into: 256 × 144 px, 60° across, on a 1/256 px sub-pixel grid. */
const SIZE = { widthPx: 256, heightPx: 144 } as const;
const FX = 1 / Math.tan(Math.PI / 6);
const FY = FX * (SIZE.widthPx / SIZE.heightPx);
const CELLS = 256;

/** A patch of 16 rocks about the middle of a finest patch of a Moon-sized sphere, and its pages. */
function rockPages(): { readonly pages: MeshPage[]; readonly originM: Vec3 } {
  const level = finestLevel(RADIUS_M);
  const dir: readonly [number, number, number] = [0.3, 1, 0.2];
  const { face, u, v } = xyzToFaceUv(dir);
  const side = 2 ** level;
  const key: PatchKey = {
    face,
    level,
    i: Math.floor(uvToSt(u) * side),
    j: Math.floor(uvToSt(v) * side),
  };
  const [x, y, z] = vertexDir(key, 32, 32);
  const centre = normalise(vec3(x, y, z));
  const originM = vec3(centre.x * RADIUS_M, centre.y * RADIUS_M, centre.z * RADIUS_M);
  const built = buildRockPages({
    key,
    originM,
    layer: rockLayerFrom(rocksAbout(5, 16, centre, 16)),
  });
  if (built === null) {
    throw new Error("sixteen rocks make a mesh");
  }
  return { pages: meshPagesOf(built.file), originM };
}

/** The header word `const <name> : u32 = <n>u;` of a WGSL source. */
function wgslWord(source: string, name: string): number {
  const found = new RegExp(String.raw`const ${name} : u32 = (\d+)u;`, "u").exec(source);
  if (found?.[1] === undefined) {
    throw new Error(`the source states no ${name}`);
  }
  return Number(found[1]);
}

/**
 * `rockPage.wgsl`'s `rock_corner` grid coordinates, word for word over the page's bytes as the pool
 * holds them: the cluster's template from the page's own records, its triangle's indices, its
 * vertex's 21-bit coordinates.
 */
function rockCornerGrid(
  page: MeshPage,
  cluster: number,
  triangle: number,
  corner: number,
): number[] {
  const words = new Uint32Array(page.body);
  const at = (n: number): number => words[n] ?? Number.NaN;
  const records = at(wgslWord(rockPageWgsl, "ROCK_PAGE_RECORDS_WORD")) / 4;
  const table = at(wgslWord(rockPageWgsl, "ROCK_PAGE_TABLE_WORD")) / 4;
  const vertices = at(wgslWord(rockPageWgsl, "ROCK_PAGE_VERTICES_WORD")) / 4;
  const triangles = at(wgslWord(rockPageWgsl, "ROCK_PAGE_TRIANGLES_WORD")) / 4;
  const recordWords = wgslWord(rockPageWgsl, "ROCK_RECORD_WORDS");
  const template = at(
    records + cluster * recordWords + wgslWord(rockPageWgsl, "ROCK_RECORD_TEMPLATE_WORD"),
  );
  const [first, count] = [template & 0xff_ffff, template >>> 24];
  const indices = at(triangles + first + Math.min(triangle, Math.max(count, 1) - 1));
  const vertex = at(table + cluster * 4) + ((indices >>> (8 * corner)) & 0xff);
  const [w0, w1] = [at(vertices + vertex * 4), at(vertices + vertex * 4 + 1)];
  const mask = 0x1f_ffff;
  return [w0 & mask, ((w0 >>> 21) | (w1 << 11)) & mask, w1 >>> 10];
}

/** The clip position of the view-space point `p` (the camera at the origin looking down −z). */
function clipOf(p: Vec3): ClipPosition {
  return [FX * p.x, FY * p.y, 0.01, -p.z];
}

/** The view-space point where the ray through the pixel at normalised device coordinates `ndc` meets the plane of (`a`, `b`, `c`). */
function rayOnPlane(a: Vec3, b: Vec3, c: Vec3, ndc: readonly [number, number]): Vec3 {
  const ray = vec3(ndc[0] / FX, ndc[1] / FY, -1);
  const n = cross(sub(b, a), sub(c, a));
  const t = dot(n, a) / dot(n, ray);
  return vec3(ray.x * t, ray.y * t, ray.z * t);
}

/** `cascadeShadow.wgsl`'s `cascade_geometric_normal`: the unit normal of `dx` × `dy`, to the datum's side. */
function geometricNormal(dx: Vec3, dy: Vec3, datum: Vec3): Vec3 {
  const unit = normalise(cross(dx, dy));
  return dot(unit, datum) < 0 ? vec3(-unit.x, -unit.y, -unit.z) : unit;
}

/** The storage buffers `spec` binds in `stage`, by name, sorted. */
function stageStorage(spec: WgslMaterialSpec, stage: "vertex" | "fragment"): string[] {
  return (spec.storageBuffers ?? [])
    .filter((buffer) => buffer.stages?.includes(stage) === true)
    .map((buffer) => buffer.name)
    .toSorted();
}

describe("the rocks' resolve's twins (R14.T5.e)", () => {
  it("decodes each corner of every triangle from its page's own records, as the pool's copy has it", () => {
    const { pages } = rockPages();
    expect(wgslWord(rockPageWgsl, "ROCK_PAGE_RECORDS_WORD") * 4).toBe(72);
    expect(wgslWord(rockPageWgsl, "ROCK_RECORD_WORDS") * 4).toBe(PAGE_RECORD_BYTES);
    let corners = 0;
    for (const page of pages) {
      const view = new DataView(page.body);
      for (let c = 0; c < page.clusters; c += 1) {
        const expected = pageClusterCorners(page, c);
        const count =
          view.getUint32(page.sections.records + PAGE_RECORD_BYTES * c + 24, true) >>> 24;
        for (let t = 0; t < count; t += 1) {
          for (let k = 0; k < 3; k += 1) {
            expect(rockCornerGrid(page, c, t, k)).toEqual(
              Array.from(expected.subarray(9 * t + 3 * k, 9 * t + 3 * k + 3)),
            );
            corners += 1;
          }
        }
      }
    }
    expect(corners).toBeGreaterThan(1_000);
    expect([CLUSTER_TABLE_BYTES, PAGE_VERTEX_BYTES, PAGE_TRIANGLE_BYTES]).toEqual([16, 16, 4]);
  });

  it("weighs a pixel's three corners to the place on its rock triangle that projects to the pixel's centre, to the sub-pixel grid, its normal from its edges the forward draw's", () => {
    const { pages, originM } = rockPages();
    // The camera 6 m from the rocks' patch's middle, looking at it along its normal.
    const up = normalise(originM);
    const east = normalise(cross(vec3(0, 0, 1), up));
    const north = cross(up, east);
    const eye = vec3(originM.x + 6 * up.x, originM.y + 6 * up.y, originM.z + 6 * up.z);
    // View space: x east, y north, z up, so that the camera looks down −z at the ground.
    const toView = (p: Vec3): Vec3 => {
      const d = sub(p, eye);
      return vec3(dot(d, east), dot(d, north), dot(d, up));
    };
    let compared = 0;
    let largestPx = 0;
    let largestNormal = 0;
    for (const page of pages) {
      for (let c = 0; c < page.clusters; c += 1) {
        const grid = pageClusterCorners(page, c);
        for (let t = 0; 9 * t < grid.length; t += 1) {
          const corners = [0, 1, 2].map((k) => {
            const local = vec3(
              page.originM.x + Math.fround((grid[9 * t + 3 * k] ?? 0) * page.stepM),
              page.originM.y + Math.fround((grid[9 * t + 3 * k + 1] ?? 0) * page.stepM),
              page.originM.z + Math.fround((grid[9 * t + 3 * k + 2] ?? 0) * page.stepM),
            );
            return toView(vec3(originM.x + local.x, originM.y + local.y, originM.z + local.z));
          });
          const [a, b, cc] = corners;
          if (a === undefined || b === undefined || cc === undefined) {
            continue;
          }
          const clips = corners.map(clipOf);
          const [c0, c1, c2] = clips;
          if (c0 === undefined || c1 === undefined || c2 === undefined) {
            continue;
          }
          // The pixel at the triangle's centroid's projection.
          const centroid = vec3(
            (a.x + b.x + cc.x) / 3,
            (a.y + b.y + cc.y) / 3,
            (a.z + b.z + cc.z) / 3,
          );
          const [cx, cy, , cw] = clipOf(centroid);
          const px = Math.floor((0.5 + (0.5 * cx) / cw) * SIZE.widthPx) + 0.5;
          const py = Math.floor((0.5 - (0.5 * cy) / cw) * SIZE.heightPx) + 0.5;
          const ndc = [(2 * px) / SIZE.widthPx - 1, 1 - (2 * py) / SIZE.heightPx] as const;
          const exact = rayOnPlane(a, b, cc, ndc);
          const w = clipBarycentrics(
            snappedClip(c0, SIZE, CELLS),
            snappedClip(c1, SIZE, CELLS),
            snappedClip(c2, SIZE, CELLS),
            ndc,
          );
          const place = vec3(
            w[0] * a.x + w[1] * b.x + w[2] * cc.x,
            w[0] * a.y + w[1] * b.y + w[2] * cc.y,
            w[0] * a.z + w[1] * b.z + w[2] * cc.z,
          );
          // Only where the pixel's centre lies inside the triangle: elsewhere the weights are clamped.
          if (w.some((x) => x <= 0)) {
            continue;
          }
          // The place projects to the pixel's centre but for the corners' snap, at most half a
          // cell of the grid each, which moves the triangle's image by as much.
          const [qx, qy, , qw] = clipOf(place);
          const apartPx = Math.hypot(
            (0.5 + (0.5 * qx) / qw) * SIZE.widthPx - px,
            (0.5 - (0.5 * qy) / qw) * SIZE.heightPx - py,
          );
          largestPx = Math.max(largestPx, apartPx);
          // Its geometric normal from its edges is the one the forward draw's derivatives span.
          const datum = vec3(0, 0, 1);
          const fromEdges = geometricNormal(sub(b, a), sub(cc, a), datum);
          const dx = sub(rayOnPlane(a, b, cc, [ndc[0] + 2 / SIZE.widthPx, ndc[1]]), exact);
          const dy = sub(rayOnPlane(a, b, cc, [ndc[0], ndc[1] - 2 / SIZE.heightPx]), exact);
          const fromDerivatives = geometricNormal(dx, dy, datum);
          largestNormal = Math.max(largestNormal, 1 - dot(fromEdges, fromDerivatives));
          compared += 1;
        }
      }
    }
    expect(compared).toBeGreaterThan(200);
    expect(largestPx).toBeLessThanOrEqual(1 / CELLS);
    expect(largestNormal).toBeLessThan(1e-9);
  });
});

describe("the rocks' visibility forms (R14.T5.e)", () => {
  const lit = rockLitMaterialSpec();
  const raster = rockIdMaterialSpec();
  const resolve = rockResolveMaterialSpec();

  it("place and shade a rock by the forward draw's own functions, its source to the byte", () => {
    for (const spec of [lit, raster, resolve]) {
      expect(spec.vertexWgsl).toContain(rockWgsl);
      expect(spec.vertexWgsl).toContain(rockPageWgsl);
    }
    // The resolve reads the lights from the resolve frame's rows, in the terrain resolve's place.
    expect(resolve.fragmentWgsl).toContain(
      "return textureLoad(rock_resolve_frame, vec2u(i, 0u), 0);",
    );
    expect(resolve.fragmentWgsl).not.toContain("var<storage, read> terrain_lights");
  });

  it("raster IDs into r32uint with the vertex stage's storage alone, and resolve within WebGPU's limits a stage", () => {
    expect([stageStorage(raster, "fragment"), raster.textures]).toEqual([[], []]);
    expect(stageStorage(resolve, "vertex")).toEqual([]);
    expect(stageStorage(resolve, "fragment")).toEqual([
      "classLayers",
      "clusterLists",
      "horizonMap",
      "rockInstances",
      "rockPages",
      "rockRecords",
      "slots",
      "surveyMask",
    ]);
    expect((resolve.textures ?? []).length).toBeLessThanOrEqual(16);
    expect(
      (resolve.textures ?? []).filter(
        (t) =>
          t.binding === ROCK_BINDINGS.visibilityIds || t.binding === ROCK_BINDINGS.resolveFrame,
      ),
    ).toHaveLength(2);
    expect([resolve.depthWrite, resolve.cullMode, raster.depthWrite, raster.cullMode]).toEqual([
      false,
      "none",
      true,
      "back",
    ]);
  });
});
