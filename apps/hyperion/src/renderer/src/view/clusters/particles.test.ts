/**
 * The cluster path's particles (plan R14, R14.T5.d): the instance record as `particleInstance.wgsl`
 * lays it out, the sphere templates, each of its sphere's mean projected area, their extents and
 * error fractions, and the low setting's want of an instance pipeline.
 */

import { describe, expect, it } from "vitest";

import { normalise, type Vec3 } from "../../geometry/vec3";
import { declaredStructLayout } from "../engine/webgpu/uniforms";
import { SETTINGS } from "../quality/qualitySetting";
import particleInstanceWgsl from "./particleInstance.wgsl?raw";
import {
  instancePathOn,
  MAX_PARTICLE_TEMPLATES,
  PARTICLE_INSTANCE_BYTES,
  PARTICLE_RADIUS_WORD,
  extentErrorFraction,
  packParticleInstance,
  polyhedronExtent,
  sphereParticleTemplates,
} from "./particles";

/** A template's triangles, each its three vertices, from the templates' mesh. */
function trianglesOf(index: number): Array<readonly [Vec3, Vec3, Vec3]> {
  const { templates, mesh } = sphereParticleTemplates();
  const template = templates[index];
  if (template === undefined || mesh.indices === null) {
    throw new Error(`no template ${String(index)}`);
  }
  const vertex = (i: number): Vec3 => {
    const k = mesh.indices?.[i] ?? 0;
    return {
      x: mesh.positions[3 * k] ?? Number.NaN,
      y: mesh.positions[3 * k + 1] ?? Number.NaN,
      z: mesh.positions[3 * k + 2] ?? Number.NaN,
    };
  };
  const out: Array<readonly [Vec3, Vec3, Vec3]> = [];
  for (let i = template.firstIndex; i < template.firstIndex + template.indexCount; i += 3) {
    out.push([vertex(i), vertex(i + 1), vertex(i + 2)]);
  }
  return out;
}

const sub = (a: Vec3, b: Vec3): Vec3 => ({ x: a.x - b.x, y: a.y - b.y, z: a.z - b.z });
const cross = (a: Vec3, b: Vec3): Vec3 => ({
  x: a.y * b.z - a.z * b.y,
  y: a.z * b.x - a.x * b.z,
  z: a.x * b.y - a.y * b.x,
});
const dot = (a: Vec3, b: Vec3): number => a.x * b.x + a.y * b.y + a.z * b.z;

/** A vertex's place as a key, to a micrometre. */
const key = (p: Vec3): string => [p.x, p.y, p.z].map((v) => v.toFixed(6)).join(",");

describe("the cluster path's particles (plan R14, R14.T5.d)", () => {
  it("lays an instance out in 32 B as WGSL's ParticleInstance, its radius in word 3", () => {
    const layout = declaredStructLayout(particleInstanceWgsl, "ParticleInstance");
    expect([
      layout?.members.map((m) => [m.name, m.offsetBytes]),
      layout?.sizeBytes,
      PARTICLE_RADIUS_WORD,
    ]).toEqual([
      [
        ["centre", 0],
        ["radius", 12],
        ["rotation", 16],
        ["colour", 24],
        ["tag", 28],
      ],
      PARTICLE_INSTANCE_BYTES,
      3,
    ]);
  });

  it("packs an instance's rotation as four snorm16 and its colour and tag as given", () => {
    const out = new DataView(new ArrayBuffer(2 * PARTICLE_INSTANCE_BYTES));
    packParticleInstance(out, PARTICLE_INSTANCE_BYTES, {
      centreM: { x: 1.5, y: -2, z: 3 },
      radiusM: 0.25,
      rotation: { w: 0.5, x: -0.5, y: 0.5, z: -1 },
      colour: 0xff20_4080,
      tag: 7,
    });
    const at = PARTICLE_INSTANCE_BYTES;
    expect([
      out.getFloat32(at, true),
      out.getFloat32(at + 4, true),
      out.getFloat32(at + 8, true),
      out.getFloat32(at + 12, true),
      out.getInt16(at + 16, true),
      out.getInt16(at + 18, true),
      out.getInt16(at + 20, true),
      out.getInt16(at + 22, true),
      out.getUint32(at + 24, true),
      out.getUint32(at + 28, true),
      out.getUint32(0, true),
    ]).toEqual([1.5, -2, 3, 0.25, -16_384, 16_384, -32_767, 16_384, 0xff20_4080, 7, 0]);
    expect(() => {
      packParticleInstance(out, 0, {
        centreM: { x: 0, y: 0, z: 0 },
        radiusM: 1,
        rotation: { w: 1, x: 0, y: 0, z: 0 },
        colour: -1,
        tag: 0,
      });
    }).toThrow(RangeError);
  });

  it("gives four templates of 8, 20, 80 and 320 triangles, at most the cull's four, coarsest first", () => {
    const { templates, errorFractions, mesh } = sphereParticleTemplates();
    expect([
      templates.map((t) => [t.shape, t.stride, t.triangles, t.indexCount, t.mayDiscard]),
      templates.length <= MAX_PARTICLE_TEMPLATES,
      errorFractions.every((e, k) => k === 0 || e < (errorFractions[k - 1] ?? 0)),
      mesh.indices?.length,
    ]).toEqual([
      [
        ["particle", 1, 8, 24, false],
        ["particle", 1, 20, 60, false],
        ["particle", 1, 80, 240, false],
        ["particle", 1, 320, 960, false],
      ],
      true,
      true,
      3 * (8 + 20 + 80 + 320),
    ]);
  });

  it("gives each template its sphere's mean projected area, a quarter of its surface's (Cauchy's formula)", () => {
    const quarters = [0, 1, 2, 3].map(
      (t) =>
        trianglesOf(t).reduce(
          (area, [a, b, c]) => area + Math.hypot(...Object.values(cross(sub(b, a), sub(c, a)))) / 2,
          0,
        ) / 4,
    );
    expect(quarters).toEqual([0, 1, 2, 3].map(() => expect.closeTo(Math.PI, 5)));
  });

  it("places each template's vertices on one sphere, its scale, and bounds every template by the largest", () => {
    const { mesh, templates, extents, boundsScale } = sphereParticleTemplates();
    const off: string[] = [];
    templates.forEach((template, t) => {
      const outer = extents[t]?.[1] ?? Number.NaN;
      for (let i = template.firstIndex; i < template.firstIndex + template.indexCount; i += 1) {
        const k = mesh.indices?.[i] ?? 0;
        const r = Math.hypot(
          mesh.positions[3 * k] ?? 0,
          mesh.positions[3 * k + 1] ?? 0,
          mesh.positions[3 * k + 2] ?? 0,
        );
        if (Math.abs(r - outer) > 1e-6) {
          off.push(`${String(t)}: ${String(r)}`);
        }
      }
    });
    expect(off).toEqual([]);
    // The octahedron's scale, √(4π ÷ 4√3), its unit surface 4√3: the largest.
    expect(boundsScale).toBeCloseTo(Math.sqrt(Math.PI / Math.sqrt(3)), 12);
    expect(Math.max(...extents.map(([, outer]) => outer))).toBe(boundsScale);
  });

  it("winds every triangle outward, front-facing from outside under the engine's ccw", () => {
    const inward = [0, 1, 2, 3].flatMap((t) =>
      trianglesOf(t).filter(([a, b, c]) => dot(cross(sub(b, a), sub(c, a)), a) <= 0),
    );
    expect(inward).toEqual([]);
  });

  it("closes each template's surface: every edge shared by two triangles, once each way", () => {
    const open = [0, 1, 2, 3].map((t) => {
      const edges = new Map<string, number>();
      for (const [a, b, c] of trianglesOf(t)) {
        for (const [p, q] of [
          [a, b],
          [b, c],
          [c, a],
        ] as const) {
          const e = `${key(p)}>${key(q)}`;
          edges.set(e, (edges.get(e) ?? 0) + 1);
        }
      }
      return [...edges].filter(([e, n]) => {
        const [p, q] = e.split(">");
        return n !== 1 || edges.get(`${q ?? ""}>${p ?? ""}`) !== 1;
      }).length;
    });
    expect(open).toEqual([0, 0, 0, 0]);
  });

  it("states each template's error as the larger of its vertices' reach past the sphere and its least face's inside it, each face's nearest point within it", () => {
    const { errorFractions, extents } = sphereParticleTemplates();
    // The octahedron's scale k = √(π ÷ √3), its faces at k ÷ √3; the icosahedron's
    // k = √(4π ÷ 5√3 a²), a = 4 ÷ √(10 + 2√5) its edge on the unit sphere, its faces at
    // k √((5 + 2√5) ÷ 15): both reach farther past the sphere than they fall inside it.
    const octahedron = Math.sqrt(Math.PI / Math.sqrt(3));
    const edge = 4 / Math.sqrt(10 + 2 * Math.sqrt(5));
    const icosahedron = Math.sqrt((4 * Math.PI) / (5 * Math.sqrt(3) * edge * edge));
    expect([errorFractions[0], errorFractions[1], extents[0]?.[0], extents[1]?.[0]]).toEqual([
      expect.closeTo(octahedron - 1, 12),
      expect.closeTo(icosahedron - 1, 12),
      expect.closeTo(octahedron / Math.sqrt(3), 12),
      expect.closeTo(icosahedron * Math.sqrt((5 + 2 * Math.sqrt(5)) / 15), 12),
    ]);
    const outside = [0, 1, 2, 3].flatMap((t) =>
      trianglesOf(t).filter(([a, b, c]) => {
        const n = normalise(cross(sub(b, a), sub(c, a)));
        const foot = { x: n.x * dot(n, a), y: n.y * dot(n, a), z: n.z * dot(n, a) };
        // The foot's barycentric weights, each its sub-triangle's share of the face's area.
        const area = dot(cross(sub(b, a), sub(c, a)), n);
        return [
          [b, c],
          [c, a],
          [a, b],
        ].some(([p, q]) => dot(cross(sub(q ?? a, p ?? a), sub(foot, p ?? a)), n) / area < 0);
      }),
    );
    expect(outside).toEqual([]);
    // The same from the mesh's `f32` vertices, to their rounding.
    expect(
      [0, 1, 2, 3].map((t) => {
        const triangles = trianglesOf(t);
        return extentErrorFraction(
          polyhedronExtent(
            triangles.flat(),
            triangles.map((_, k) => [3 * k, 3 * k + 1, 3 * k + 2] as const),
          ),
        );
      }),
    ).toEqual(errorFractions.map((e) => expect.closeTo(e, 6)));
  });

  it("makes no instance pipeline on the low setting, which draws no scatter and no ring particles, and one on high", () => {
    expect([instancePathOn(SETTINGS.low), instancePathOn(SETTINGS.high)]).toEqual([false, true]);
    expect(
      instancePathOn({ decoration: true, scatterDensity: 0, rings: "annulus" }) ||
        instancePathOn({ decoration: false, scatterDensity: 1, rings: "annulus" }),
    ).toBe(false);
    expect(instancePathOn({ decoration: false, scatterDensity: 0, rings: "particles" })).toBe(true);
  });
});
