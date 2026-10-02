import { describe, expect, it } from "vitest";

import { FakeBuffer } from "../../../test/fakeGpu";
import type { BufferSpec } from "../memory";
import type { BufferHandle, MeshSpec } from "../types";
import { assertMeshData, MeshRecord, meshAttributes, vertexBufferLayouts } from "./meshes";

const SPEC: MeshSpec = {
  name: "segment",
  positions: new Float32Array([0, 0, 0, 1, 0, 0]),
  indices: null,
  topology: "line-list",
  attributes: { colour: { data: new Float32Array(8), size: 4 } },
  instanceAttributes: { offset: { data: new Float32Array(6), size: 3 } },
};

describe("a mesh's vertex layout", () => {
  it("puts position at 0, then its attributes, then its instance attributes", () => {
    expect(
      meshAttributes(SPEC).map(({ name, location, instanced }) => [name, location, instanced]),
    ).toEqual([
      ["position", 0, false],
      ["colour", 1, false],
      ["offset", 2, true],
    ]);
    expect(vertexBufferLayouts(meshAttributes(SPEC))).toEqual([
      {
        arrayStride: 12,
        stepMode: "vertex",
        attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }],
      },
      {
        arrayStride: 16,
        stepMode: "vertex",
        attributes: [{ shaderLocation: 1, offset: 0, format: "float32x4" }],
      },
      {
        arrayStride: 12,
        stepMode: "instance",
        attributes: [{ shaderLocation: 2, offset: 0, format: "float32x3" }],
      },
    ]);
  });
});

describe("a mesh's data", () => {
  it("is refused when ragged, naming the mesh", () => {
    expect(() => {
      assertMeshData({ ...SPEC, positions: new Float32Array(4) });
    }).toThrow("mesh segment's positions are not whole xyz triples");
    expect(() => {
      assertMeshData({ ...SPEC, attributes: { colour: { data: new Float32Array(4), size: 4 } } });
    }).toThrow("attribute colour does not give each vertex 4");
    expect(() => {
      assertMeshData({ ...SPEC, indices: new Uint32Array([0, 2]) });
    }).toThrow("an index, 2, past its 2 vertices");
  });

  it("is uploaded once a buffer an attribute, through the one creation path", () => {
    const made: BufferSpec[] = [];
    const written: number[] = [];
    const record = new MeshRecord(
      {
        createBuffer: (spec: BufferSpec): BufferHandle => {
          made.push(spec);
          return { kind: "buffer", name: spec.name, bytes: spec.bytes };
        },
        writeBuffer: (_handle, _offset, data) => {
          written.push(data.byteLength);
        },
        gpuBufferOf: () => new FakeBuffer({ size: 4, usage: 0 }),
      },
      { ...SPEC, indices: new Uint32Array([0, 1]) },
    );
    expect(made.map(({ name, bytes, category }) => [name, bytes, category])).toEqual([
      ["segment:position", 24, "other"],
      ["segment:colour", 32, "other"],
      ["segment:offset", 24, "other"],
      ["segment:indices", 8, "other"],
    ]);
    expect(written).toEqual([24, 32, 24, 8]);
    expect(record.vertexCount).toBe(2);
    expect(record.index?.count).toBe(2);
  });

  it("keys its layout by topology and attribute shapes, not by names or data", () => {
    const host = {
      createBuffer: (spec: BufferSpec): BufferHandle => ({
        kind: "buffer",
        name: spec.name,
        bytes: spec.bytes,
      }),
      writeBuffer: () => undefined,
      gpuBufferOf: () => new FakeBuffer({ size: 4, usage: 0 }),
    };
    const first = new MeshRecord(host, SPEC);
    const renamed = new MeshRecord(host, {
      ...SPEC,
      name: "other",
      attributes: { tint: { data: new Float32Array(8), size: 4 } },
    });
    const triangles = new MeshRecord(host, { ...SPEC, topology: "point-list" });
    expect(renamed.layoutKey).toBe(first.layoutKey);
    expect(triangles.layoutKey).not.toBe(first.layoutKey);
  });
});
