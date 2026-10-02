import { describe, expect, it } from "vitest";

import FRAME_WGSL from "../../shaders/frame.wgsl?raw";
import { FakeBuffer } from "../../../test/fakeGpu";
import type { BufferSpec } from "../memory";
import type { BufferHandle } from "../types";
import {
  assertDrawStruct,
  declaredStructLayout,
  FRAME_BYTES,
  OFFSET_MEMBER,
  packFrame,
  packUniforms,
  type RingHost,
  UniformRing,
  uniformLayout,
} from "./uniforms";

describe("a uniform struct's layout", () => {
  it("follows WGSL's uniform rules: a scalar packs after a vec3f", () => {
    const layout = uniformLayout([
      OFFSET_MEMBER,
      { name: "brightness", type: "f32" },
      { name: "tint", type: "vec4f" },
      { name: "count", type: "u32" },
    ]);
    expect(layout.members.map(({ name, offsetBytes }) => [name, offsetBytes])).toEqual([
      ["offsetFromCameraM", 0],
      ["brightness", 12],
      ["tint", 16],
      ["count", 32],
    ]);
    expect(layout.sizeBytes).toBe(48);
  });

  it("matches the Frame that frame.wgsl declares", () => {
    expect(declaredStructLayout(FRAME_WGSL, "Frame")?.sizeBytes).toBe(FRAME_BYTES);
  });

  it("is checked against a source's own struct Draw", () => {
    const expected = uniformLayout([OFFSET_MEMBER, { name: "tint", type: "vec4f" }]);
    const good = "struct Draw {\n  offsetFromCameraM : vec3f,\n  tint : vec4<f32>,\n}";
    expect(() => {
      assertDrawStruct("material m", expected, good, "no struct here");
    }).not.toThrow();
    const swapped = "struct Draw { tint : vec4f, offsetFromCameraM : vec3f }";
    expect(() => {
      assertDrawStruct("material m", expected, swapped);
    }).toThrow(
      "material m's struct Draw has tint : vec4f where the engine packs offsetFromCameraM",
    );
  });
});

describe("packed uniforms", () => {
  it("write floats, unsigned integers as integers, and zero what has no value", () => {
    const layout = uniformLayout([
      { name: "scale", type: "f32" },
      { name: "count", type: "u32" },
      { name: "absent", type: "vec2f" },
    ]);
    const bytes = new ArrayBuffer(32);
    new Float32Array(bytes).fill(9);
    const values: Record<string, ArrayLike<number>> = { scale: [0.5], count: [7] };
    packUniforms(layout, (name) => values[name], bytes, 16);
    expect([...new Float32Array(bytes, 16, 1)]).toEqual([0.5]);
    expect([...new Uint32Array(bytes, 20, 1)]).toEqual([7]);
    expect([...new Float32Array(bytes, 24, 2)]).toEqual([0, 0]);
    expect(new Float32Array(bytes, 0, 1)[0]).toBe(9);
  });

  it("put the frame's matrices unchanged, then the viewport and its reciprocals", () => {
    const viewRotation = new Float32Array(16).map((_, index) => index);
    const projection = new Float32Array(16).map((_, index) => 100 + index);
    const block = packFrame(
      { label: "f", viewRotation, projection, draws: [], postProcesses: [] },
      { widthPx: 4, heightPx: 2 },
    );
    expect([...block.subarray(0, 16)]).toEqual([...viewRotation]);
    expect([...block.subarray(16, 32)]).toEqual([...projection]);
    expect([...block.subarray(32)]).toEqual([4, 2, 0.25, 0.5]);
  });
});

const BIND_GROUP_LAYOUT: GPUBindGroupLayout = { label: "draw" };

/** A ring host that records buffers and writes. */
function recordingHost(): RingHost & {
  readonly made: BufferSpec[];
  readonly destroyed: string[];
  readonly writes: Array<{ readonly bytes: number }>;
} {
  const made: BufferSpec[] = [];
  const destroyed: string[] = [];
  const writes: Array<{ readonly bytes: number }> = [];
  return {
    made,
    destroyed,
    writes,
    device: {
      limits: { minUniformBufferOffsetAlignment: 256 },
      createBindGroup: (descriptor: GPUBindGroupDescriptor) => ({
        label: descriptor.label ?? "",
        __brand: "GPUBindGroup" as const,
      }),
    },
    createBuffer: (spec: BufferSpec): BufferHandle => {
      made.push(spec);
      return { kind: "buffer", name: `${spec.name} ${made.length}`, bytes: spec.bytes };
    },
    gpuBufferOf: () => new FakeBuffer({ size: 4, usage: 0 }),
    destroyBuffer: (handle: BufferHandle) => {
      destroyed.push(handle.name);
    },
    writeBuffer: (_handle: BufferHandle, _offset: number, data: ArrayBufferView) => {
      writes.push({ bytes: data.byteLength });
    },
  };
}

describe("the uniform ring", () => {
  const layout = uniformLayout([OFFSET_MEMBER, { name: "tint", type: "vec4f" }]);

  it("gives each block an offset aligned to the device's, and uploads the frame at once", () => {
    const host = recordingHost();
    const ring = new UniformRing(host, BIND_GROUP_LAYOUT);
    ring.begin();
    const first = ring.push(layout, () => undefined);
    const second = ring.push(layout, () => undefined);
    expect([first.offsetBytes, second.offsetBytes]).toEqual([0, 256]);
    expect(second.sizeBytes).toBe(32);
    ring.upload();
    expect(host.writes).toEqual([{ bytes: 512 }]);
    ring.begin();
    expect(ring.push(layout, () => undefined).offsetBytes).toBe(0);
  });

  it("grows by remaking its buffer through the host, the old one destroyed", () => {
    const host = recordingHost();
    const ring = new UniformRing(host, BIND_GROUP_LAYOUT);
    ring.begin();
    for (let index = 0; index < 257; index += 1) {
      ring.push(layout, () => undefined);
    }
    expect(host.made.map(({ bytes }) => bytes)).toEqual([65_536, 131_072]);
    expect(host.destroyed).toEqual(["draw uniforms 1"]);
    expect(host.made[0]?.category).toBe("other");
  });
});
