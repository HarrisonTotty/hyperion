import { describe, expect, it } from "vitest";

import { FakeAdapter, type FakeDevice, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import type { AllocationEvent } from "../memory";
import { packedCubeSpec, ResourceRegistry, viewDimensionOf } from "./resources";

async function device(): Promise<FakeDevice> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
  await adapter.requestDevice();
  const made = adapter.devices.at(0);
  if (made === undefined) {
    throw new Error("the fake adapter made no device");
  }
  return made;
}

async function registry(): Promise<{
  readonly gpu: FakeDevice;
  readonly resources: ResourceRegistry;
  readonly events: AllocationEvent[];
}> {
  const gpu = await device();
  const events: AllocationEvent[] = [];
  return { gpu, resources: new ResourceRegistry(gpu, (event) => events.push(event)), events };
}

describe("the one creation path", () => {
  it("raises one created event for a buffer, with its category", async () => {
    const { resources, events } = await registry();
    resources.createBuffer({
      name: "heights",
      bytes: 1024,
      usage: BUFFER_USAGE.STORAGE,
      category: "render-targets",
    });
    expect(events).toEqual([
      { kind: "created", name: "heights", bytes: 1024, category: "render-targets" },
    ]);
  });

  it("raises one created event for a texture, with its bytes", async () => {
    const { gpu, resources, events } = await registry();
    resources.createTexture({
      name: "lut",
      size: [4, 4, 4],
      dimension: "3d",
      format: "rgba16float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING,
      category: "other",
    });
    expect(events).toEqual([{ kind: "created", name: "lut", bytes: 512, category: "other" }]);
    expect(gpu.textures.at(0)?.descriptor.dimension).toBe("3d");
  });

  it("makes a cube as six 2D layers", async () => {
    const { gpu, resources } = await registry();
    resources.createTexture(packedCubeSpec(8, 4, "other"));
    expect(gpu.textures.at(0)?.descriptor).toMatchObject({
      dimension: "2d",
      format: "rgb9e5ufloat",
      mipLevelCount: 4,
      size: { width: 8, height: 8, depthOrArrayLayers: 6 },
    });
  });

  it("raises one destroyed event for each resource at disposal, and destroys it", async () => {
    const { gpu, resources, events } = await registry();
    resources.createBuffer({
      name: "b",
      bytes: 16,
      usage: BUFFER_USAGE.UNIFORM,
      category: "other",
    });
    resources.createTexture(packedCubeSpec(2, 1, "other"));
    events.length = 0;
    resources.dispose();
    expect(events).toEqual([
      { kind: "destroyed", name: "b", bytes: 16, category: "other" },
      { kind: "destroyed", name: "packed star cube", bytes: 96, category: "other" },
    ]);
    expect(gpu.buffers.every((buffer) => buffer.destroyed)).toBe(true);
    expect(gpu.textures.every((texture) => texture.destroyed)).toBe(true);
  });
});

describe("releases (R06.T13.h)", () => {
  it("raise one destroyed event with the bytes a buffer was made with, and destroy it", async () => {
    const { gpu, resources, events } = await registry();
    const buffer = resources.createBuffer({
      name: "bake scratch",
      bytes: 4096,
      usage: BUFFER_USAGE.STORAGE,
      category: "other",
    });
    events.length = 0;
    resources.destroyBuffer(buffer);
    expect(events).toEqual([
      { kind: "destroyed", name: "bake scratch", bytes: 4096, category: "other" },
    ]);
    expect(gpu.buffers.at(0)?.destroyed).toBe(true);
  });

  it("refuse a released handle on every later use, naming the release", async () => {
    const { resources } = await registry();
    const cube = resources.createTexture(packedCubeSpec(4, 1, "other"));
    resources.destroyTexture(cube);
    expect(() => resources.textureOf(cube)).toThrow(/packed star cube was released/u);
    expect(() => {
      resources.destroyTexture(cube);
    }).toThrow(/was released/u);
  });

  it("leave a released texture out of the disposal's events", async () => {
    const { resources, events } = await registry();
    const cube = resources.createTexture(packedCubeSpec(2, 1, "other"));
    resources.destroyTexture(cube);
    events.length = 0;
    resources.dispose();
    expect(events).toEqual([]);
  });
});

describe("the packed cube's name (R06.T13.h)", () => {
  it("is the caller's, so that two views' cubes report their own", async () => {
    const { resources, events } = await registry();
    resources.createTexture(packedCubeSpec(2, 1, "other", "sky cube: cockpit"));
    resources.createTexture(packedCubeSpec(2, 1, "other", "sky cube: main"));
    expect(events.map((event) => event.name)).toEqual(["sky cube: cockpit", "sky cube: main"]);
  });
});

describe("uploads", () => {
  it("raise one uploaded event per buffer write, with its bytes", async () => {
    const { gpu, resources, events } = await registry();
    const buffer = resources.createBuffer({
      name: "b",
      bytes: 64,
      usage: BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    events.length = 0;
    resources.writeBuffer(buffer, 16, new Float32Array(4));
    expect(events).toEqual([{ kind: "uploaded", name: "b", bytes: 16 }]);
    expect(gpu.queue.writes).toEqual([{ kind: "buffer", label: "b", offset: 16, bytes: 16 }]);
  });

  it("raise one uploaded event per texture write, rows tightly packed", async () => {
    const { gpu, resources, events } = await registry();
    const texture = resources.createTexture({
      name: "t",
      size: [3, 2],
      dimension: "2d",
      format: "rgba8unorm",
      mips: 1,
      usage: TEXTURE_USAGE.COPY_DST,
      category: "other",
    });
    events.length = 0;
    resources.writeTexture(texture, [0, 0, 0], [3, 2], new Uint8Array(24));
    expect(events).toEqual([{ kind: "uploaded", name: "t", bytes: 24 }]);
    expect(gpu.queue.writes.at(0)).toMatchObject({ kind: "texture", bytesPerRow: 12 });
  });

  it("write a packed cube's level with its six faces", async () => {
    const { gpu, resources } = await registry();
    const cube = resources.createTexture(packedCubeSpec(4, 3, "other"));
    resources.writePackedCubeLevel(cube, 1, new Uint32Array(2 * 2 * 6));
    expect(gpu.queue.writes.at(0)).toMatchObject({ kind: "texture", mipLevel: 1, bytesPerRow: 8 });
  });

  it("refuse a packed cube level of the wrong size", async () => {
    const { resources } = await registry();
    const cube = resources.createTexture(packedCubeSpec(4, 3, "other"));
    expect(() => resources.writePackedCubeLevel(cube, 1, new Uint32Array(5))).toThrow(/24/u);
  });

  it("refuse a packed cube level that does not exist", async () => {
    const { resources } = await registry();
    const cube = resources.createTexture(packedCubeSpec(4, 3, "other"));
    expect(() => resources.writePackedCubeLevel(cube, 3, new Uint32Array(6))).toThrow(/level 3/u);
  });
});

describe("a packed cube level from a kernel's buffer", () => {
  /** A face of 2 texels: rows of 256 bytes, so 256 × 2 × 5 + 256 + 8 bytes for six faces. */
  const NEEDED = 256 * 2 * 5 + 256 * 1 + 2 * 4;

  async function cubeAndBuffer(bytes: number, usage: number) {
    const { resources } = await registry();
    const cube = resources.createTexture(packedCubeSpec(4, 3, "other"));
    const packed = resources.createBuffer({ name: "bake", bytes, usage, category: "other" });
    return { resources, cube, packed };
  }

  it("is copied with rows padded to 256 bytes", async () => {
    const { resources, cube, packed } = await cubeAndBuffer(NEEDED, BUFFER_USAGE.COPY_SRC);
    const copies: unknown[] = [];
    resources.encodePackedCubeLevelFromBuffer(
      { copyBufferToTexture: (...args: unknown[]) => void copies.push(args) },
      cube,
      1,
      packed,
    );
    expect(copies).toEqual([
      [
        expect.objectContaining({ bytesPerRow: 256, rowsPerImage: 2 }),
        expect.objectContaining({ mipLevel: 1 }),
        [2, 2, 6],
      ],
    ]);
  });

  it("copies one face from a buffer that holds it alone (R06.T13.g)", async () => {
    const { resources, cube, packed } = await cubeAndBuffer(256 + 2 * 4, BUFFER_USAGE.COPY_SRC);
    const copies: unknown[] = [];
    resources.encodePackedCubeLevelFromBuffer(
      { copyBufferToTexture: (...args: unknown[]) => void copies.push(args) },
      cube,
      1,
      packed,
      4,
    );
    expect(copies).toEqual([
      [
        expect.objectContaining({ bytesPerRow: 256, rowsPerImage: 2 }),
        expect.objectContaining({ mipLevel: 1, origin: [0, 0, 4] }),
        [2, 2, 1],
      ],
    ]);
    const encoder = { copyBufferToTexture: () => undefined };
    expect(() => resources.encodePackedCubeLevelFromBuffer(encoder, cube, 1, packed, 6)).toThrow(
      /no face 6/u,
    );
  });

  it("is refused from a buffer too small for the padded layout", async () => {
    const { resources, cube, packed } = await cubeAndBuffer(NEEDED - 1, BUFFER_USAGE.COPY_SRC);
    const encoder = { copyBufferToTexture: () => undefined };
    expect(() => resources.encodePackedCubeLevelFromBuffer(encoder, cube, 1, packed)).toThrow(
      /needs 2824/u,
    );
  });

  it("is refused from a buffer without COPY_SRC", async () => {
    const { resources, cube, packed } = await cubeAndBuffer(NEEDED, BUFFER_USAGE.STORAGE);
    const encoder = { copyBufferToTexture: () => undefined };
    expect(() => resources.encodePackedCubeLevelFromBuffer(encoder, cube, 1, packed)).toThrow(
      /COPY_SRC/u,
    );
  });
});

describe("a layered 2D texture", () => {
  it("is sampled through a 2D-array view", () => {
    expect(
      viewDimensionOf({
        name: "layers",
        size: [4, 4, 3],
        dimension: "2d",
        format: "rgba8unorm",
        mips: 1,
        usage: TEXTURE_USAGE.TEXTURE_BINDING,
        category: "other",
      }),
    ).toBe("2d-array");
  });
});

describe("handles", () => {
  it("are refused by a registry that did not make them", async () => {
    const first = await registry();
    const second = await registry();
    const buffer = first.resources.createBuffer({
      name: "b",
      bytes: 4,
      usage: BUFFER_USAGE.UNIFORM,
      category: "other",
    });
    expect(() => second.resources.bufferOf(buffer)).toThrow(/not made by this engine/u);
  });
});
