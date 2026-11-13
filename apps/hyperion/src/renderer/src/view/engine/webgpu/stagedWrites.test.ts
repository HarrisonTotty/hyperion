import { describe, expect, it } from "vitest";

import {
  FakeAdapter,
  FakeBuffer,
  FakeCommandBuffer,
  type FakeDevice,
  FakeTexture,
  INTEL_UHD_620_INFO,
} from "../../../test/fakeGpu";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import { bytesPerTexel, extentOf } from "../memory";
import type { BufferHandle, TextureHandle } from "../types";
import { ResourceRegistry } from "./resources";
import { StagedWrites } from "./stagedWrites";

async function device(): Promise<FakeDevice> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
  await adapter.requestDevice();
  const made = adapter.devices.at(0);
  if (made === undefined) {
    throw new Error("the fake adapter made no device");
  }
  return made;
}

function isObject(value: unknown): value is object {
  return typeof value === "object" && value !== null;
}

function numberOr(value: unknown, otherwise: number): number {
  return typeof value === "number" ? value : otherwise;
}

/** An origin, as the registry or its copies give it, as three numbers. */
function originOf(origin: unknown): [number, number, number] {
  if (Array.isArray(origin)) {
    return [numberOr(origin[0], 0), numberOr(origin[1], 0), numberOr(origin[2], 0)];
  }
  if (isObject(origin)) {
    return [
      numberOr(Reflect.get(origin, "x"), 0),
      numberOr(Reflect.get(origin, "y"), 0),
      numberOr(Reflect.get(origin, "z"), 0),
    ];
  }
  return [0, 0, 0];
}

/**
 * A fake device's memory as its queue's writes and the copies of its submissions leave it, byte
 * for byte: buffers whole, each texture level's texels rows tightly packed, layer after layer.
 */
class SimulatedMemory {
  readonly #buffers = new Map<GPUBuffer, Uint8Array>();
  readonly #levels = new Map<GPUTexture, Uint8Array[]>();
  /** Every queue operation, in order: `write`, or `submit <label>`. */
  readonly operations: string[] = [];

  constructor(gpu: FakeDevice) {
    const queue = gpu.queue;
    const writeBuffer = queue.writeBuffer.bind(queue);
    queue.writeBuffer = (buffer, offset, data, dataOffset, size) => {
      this.operations.push("write");
      if (!(data instanceof ArrayBuffer)) {
        throw new Error("the registry writes an ArrayBuffer with its offset and size in bytes");
      }
      this.#bufferBytes(buffer).set(
        new Uint8Array(data, dataOffset ?? 0, size ?? data.byteLength - (dataOffset ?? 0)),
        offset,
      );
      return writeBuffer(buffer, offset, data, dataOffset, size);
    };
    const writeTexture = queue.writeTexture.bind(queue);
    // As WebGPU's queue takes it, with its size, which the fake's own leaves out.
    const asQueue: GPUQueue = queue;
    asQueue.writeTexture = (destination, data, layout, size) => {
      this.operations.push("write");
      if (!ArrayBuffer.isView(data)) {
        throw new Error("the registry writes a view");
      }
      this.#toTexture(
        new Uint8Array(data.buffer, data.byteOffset, data.byteLength),
        layout,
        destination,
        size,
      );
      return writeTexture(destination, data, layout);
    };
    const submit = queue.submit.bind(queue);
    queue.submit = (buffers) => {
      for (const buffer of buffers) {
        if (!(buffer instanceof FakeCommandBuffer)) {
          throw new Error("the fake device submits its own command buffers");
        }
        this.operations.push(`submit ${buffer.label}`);
        for (const { op, args } of buffer.commands) {
          this.#run(op, args);
        }
      }
      return submit(buffers);
    };
  }

  /** A buffer's bytes as they stand. */
  buffer(buffer: GPUBuffer): Uint8Array {
    return this.#bufferBytes(buffer);
  }

  /** A texture level's texels as they stand. */
  level(texture: GPUTexture, mipLevel = 0): Uint8Array {
    return this.#levelBytes(texture, mipLevel);
  }

  #bufferBytes(buffer: GPUBuffer): Uint8Array {
    let bytes = this.#buffers.get(buffer);
    if (bytes === undefined) {
      bytes = new Uint8Array(buffer.size);
      this.#buffers.set(buffer, bytes);
    }
    return bytes;
  }

  #levelBytes(texture: GPUTexture, mipLevel: number): Uint8Array {
    let levels = this.#levels.get(texture);
    if (levels === undefined) {
      levels = [];
      this.#levels.set(texture, levels);
    }
    let bytes = levels[mipLevel];
    if (bytes === undefined) {
      const width = Math.max(1, texture.width >> mipLevel);
      const height = Math.max(1, texture.height >> mipLevel);
      bytes = new Uint8Array(
        width * height * texture.depthOrArrayLayers * bytesPerTexel(texture.format),
      );
      levels[mipLevel] = bytes;
    }
    return bytes;
  }

  #toTexture(
    source: Uint8Array,
    layout: GPUTexelCopyBufferLayout,
    destination: GPUTexelCopyTextureInfo,
    size: GPUExtent3D | Iterable<number>,
  ): void {
    const { texture } = destination;
    const mipLevel = destination.mipLevel ?? 0;
    const level = this.#levelBytes(texture, mipLevel);
    const texel = bytesPerTexel(texture.format);
    const width = Math.max(1, texture.width >> mipLevel);
    const height = Math.max(1, texture.height >> mipLevel);
    const extent = extentOf(Symbol.iterator in size ? [...size] : size);
    const [x, y, z] = originOf(destination.origin);
    const offset = layout.offset ?? 0;
    const rowBytes = extent.width * texel;
    const bytesPerRow = layout.bytesPerRow ?? rowBytes;
    const rowsPerImage = layout.rowsPerImage ?? extent.height;
    for (let layer = 0; layer < extent.depthOrArrayLayers; layer += 1) {
      for (let row = 0; row < extent.height; row += 1) {
        const from = offset + (layer * rowsPerImage + row) * bytesPerRow;
        const to = (((z + layer) * height + y + row) * width + x) * texel;
        level.set(source.subarray(from, from + rowBytes), to);
      }
    }
  }

  #run(op: string, args: ReadonlyArray<unknown>): void {
    if (op === "copyBufferToBuffer") {
      const [source, sourceOffset, destination, destinationOffset, size] = args;
      if (
        !(source instanceof FakeBuffer) ||
        !(destination instanceof FakeBuffer) ||
        typeof sourceOffset !== "number" ||
        typeof destinationOffset !== "number" ||
        typeof size !== "number"
      ) {
        throw new Error("a buffer copy of an unexpected shape");
      }
      this.#bufferBytes(destination).set(
        this.#sourceBytes(source).subarray(sourceOffset, sourceOffset + size),
        destinationOffset,
      );
    } else if (op === "copyBufferToTexture") {
      const [source, destination, size] = args;
      const buffer: unknown = isObject(source) ? Reflect.get(source, "buffer") : undefined;
      const texture: unknown = isObject(destination)
        ? Reflect.get(destination, "texture")
        : undefined;
      if (
        !(buffer instanceof FakeBuffer) ||
        !(texture instanceof FakeTexture) ||
        !isObject(source) ||
        !isObject(destination) ||
        !isObject(size)
      ) {
        throw new Error("a texture copy of an unexpected shape");
      }
      this.#toTexture(
        this.#sourceBytes(buffer),
        {
          offset: numberOr(Reflect.get(source, "offset"), 0),
          bytesPerRow: numberOr(Reflect.get(source, "bytesPerRow"), 0),
          rowsPerImage: numberOr(Reflect.get(source, "rowsPerImage"), 0),
        },
        {
          texture,
          origin: originOf(Reflect.get(destination, "origin") ?? undefined),
          mipLevel: numberOr(Reflect.get(destination, "mipLevel"), 0),
        },
        {
          width: numberOr(Reflect.get(size, "width"), 1),
          height: numberOr(Reflect.get(size, "height"), 1),
          depthOrArrayLayers: numberOr(Reflect.get(size, "depthOrArrayLayers"), 1),
        },
      );
    } else {
      throw new Error(`the simulation does not run ${op}`);
    }
  }

  /** A copy's source: a buffer mapped at its creation holds what was written into its range. */
  #sourceBytes(buffer: FakeBuffer): Uint8Array {
    return (this.#buffers.get(buffer) ?? buffer.mappedBytes).slice();
  }
}

/** The same buffers and textures on a registry of their own device, with its memory simulated. */
async function registry(partBytes?: number): Promise<{
  readonly gpu: FakeDevice;
  readonly memory: SimulatedMemory;
  readonly resources: ResourceRegistry;
  readonly a: BufferHandle;
  readonly b: BufferHandle;
  readonly mask: TextureHandle;
  readonly normals: TextureHandle;
}> {
  const gpu = await device();
  const memory = new SimulatedMemory(gpu);
  const resources = new ResourceRegistry(gpu, () => undefined, partBytes);
  const usage = BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST;
  return {
    gpu,
    memory,
    resources,
    a: resources.createBuffer({ name: "a", bytes: 4096, usage, category: "other" }),
    b: resources.createBuffer({ name: "b", bytes: 256, usage, category: "other" }),
    // A cube's mask of 20-byte rows, and a two-level tile of 8-byte texels, each row shorter than
    // a copy's 256-byte alignment.
    mask: resources.createTexture({
      name: "mask",
      size: [20, 20, 6],
      dimension: "2d",
      format: "r8unorm",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "other",
    }),
    normals: resources.createTexture({
      name: "normals",
      size: [8, 8, 1],
      dimension: "2d",
      format: "rgba16float",
      mips: 2,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "other",
    }),
  };
}

/** Bytes counting up from `start`. */
function counting(bytes: number, start: number): Uint8Array {
  return Uint8Array.from({ length: bytes }, (_, i) => (start + 7 * i) % 251);
}

/**
 * A run of writes as an upload makes them: overlapping writes, the later one standing; a scratch
 * array written, refilled and written again; and texture regions with narrow rows.
 */
function writeAll(made: Awaited<ReturnType<typeof registry>>): void {
  const { resources, a, b, mask, normals } = made;
  resources.writeBuffer(a, 0, counting(1000, 1));
  resources.writeBuffer(a, 512, counting(256, 99));
  const scratch = new Float32Array([1, 2, 3, 4]);
  resources.writeBuffer(b, 16, scratch);
  scratch.set([5, 6, 7, 8]);
  resources.writeBuffer(b, 64, scratch);
  resources.writeTexture(mask, [0, 0, 0], [20, 20, 6], counting(2400, 3));
  resources.writeTexture(normals, [1, 2, 0], [3, 2, 1], counting(48, 5));
  resources.writeTexture(normals, [0, 0, 0], [4, 4, 1], counting(128, 11), 1);
}

/** Every buffer's and texture level's bytes of a registry's device. */
function contents(made: Awaited<ReturnType<typeof registry>>): ReadonlyArray<Uint8Array> {
  const { resources, memory, a, b, mask, normals } = made;
  return [
    memory.buffer(resources.bufferOf(a).buffer),
    memory.buffer(resources.bufferOf(b).buffer),
    memory.level(resources.textureOf(mask).texture),
    memory.level(resources.textureOf(normals).texture),
    memory.level(resources.textureOf(normals).texture, 1),
  ];
}

describe("the engine's staged writes (lane perf-page-long-tasks)", () => {
  it("leaves every buffer and texture as the queue's writes do, byte for byte", async () => {
    const direct = await registry();
    writeAll(direct);
    const staged = await registry();
    staged.resources.staged("upload", () => {
      writeAll(staged);
    });
    staged.resources.flushStaged();
    expect(contents(staged)).toEqual(contents(direct));
    expect(contents(direct)[0]?.some((byte) => byte !== 0)).toBe(true);
  });

  it("makes no queue write and submits nothing as the scope ends, then one command buffer with the engine's next submission", async () => {
    const staged = await registry();
    staged.resources.staged("upload", () => {
      writeAll(staged);
    });
    expect(staged.memory.operations).toEqual([]);
    staged.resources.flushStaged();
    expect(staged.memory.operations).toEqual(["submit upload"]);
    const staging = staged.gpu.buffers.at(-1);
    expect([staging?.usage, staging?.destroyed]).toEqual([
      BUFFER_USAGE.MAP_WRITE | BUFFER_USAGE.COPY_SRC,
      true,
    ]);
  });

  it("makes its staging buffers mapped as the writes are staged, and none as it submits (lane perf-long-tasks-check)", async () => {
    const staged = await registry();
    const before = staged.gpu.buffers.length;
    staged.resources.staged("upload", () => {
      writeAll(staged);
    });
    const made = staged.gpu.buffers.slice(before);
    expect(made.map((buffer) => [buffer.mapState, buffer.destroyed])).toEqual([["mapped", false]]);
    staged.resources.flushStaged();
    expect(staged.gpu.buffers.length).toBe(before + 1);
    expect(made.map((buffer) => [buffer.mapState, buffer.destroyed])).toEqual([["unmapped", true]]);
  });

  /** Five buffer writes staged through chunks of 64 bytes, a part of 1024, into one buffer. */
  async function chunked(): Promise<{
    readonly gpu: FakeDevice;
    readonly memory: SimulatedMemory;
    readonly target: GPUBuffer;
    readonly staged: StagedWrites;
    readonly made: number;
    readonly writes: ReadonlyArray<readonly [number, Uint8Array]>;
  }> {
    const gpu = await device();
    const memory = new SimulatedMemory(gpu);
    const usage = BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST;
    const target = gpu.createBuffer({ label: "target", size: 512, usage });
    const staged = new StagedWrites(gpu, 1024, 64);
    const made = gpu.buffers.length;
    const writes: ReadonlyArray<readonly [number, Uint8Array]> = [
      [0, counting(32, 1)],
      [32, counting(32, 2)],
      [64, counting(32, 3)],
      [96, counting(200, 4)],
      [296, counting(16, 5)],
    ];
    staged.run("upload", () => {
      for (const [offset, data] of writes) {
        staged.buffer(target, offset, data);
      }
    });
    return { gpu, memory, target, staged, made, writes };
  }

  it("fills a chunk before it makes the next, and gives a write of more than a chunk a buffer of its own (lane perf-long-tasks-check)", async () => {
    const { gpu, made } = await chunked();
    expect(gpu.buffers.slice(made).map((buffer) => buffer.size)).toEqual([64, 64, 200, 64]);
  });

  it("leaves the same bytes from writes staged across chunks, in one submission (lane perf-long-tasks-check)", async () => {
    const { memory, target, staged, writes } = await chunked();
    staged.flush();
    const expected = new Uint8Array(512);
    for (const [offset, data] of writes) {
      expected.set(data, offset);
    }
    expect([memory.operations, memory.buffer(target)]).toEqual([["submit upload"], expected]);
  });

  it("lays a texture's rows out 256 bytes apart in the staging buffer", async () => {
    const staged = await registry();
    staged.resources.staged("mask", () => {
      staged.resources.writeTexture(staged.mask, [0, 0, 0], [20, 20, 6], counting(2400, 3));
    });
    staged.resources.flushStaged();
    const [command] = staged.gpu.queue.submitted.flatMap((buffer) =>
      buffer instanceof FakeCommandBuffer ? buffer.commands : [],
    );
    expect(command?.op).toBe("copyBufferToTexture");
    expect(command?.args[0]).toMatchObject({ offset: 0, bytesPerRow: 256, rowsPerImage: 20 });
  });

  it("joins a scope inside another and the scopes after it into one submission, named for the first that staged a write", async () => {
    const staged = await registry();
    staged.resources.staged("empty", () => undefined);
    staged.resources.staged("outer", () => {
      staged.resources.writeBuffer(staged.a, 0, counting(16, 1));
      staged.resources.staged("inner", () => {
        staged.resources.writeBuffer(staged.b, 0, counting(16, 2));
      });
    });
    staged.resources.staged("later", () => {
      staged.resources.writeBuffer(staged.b, 16, counting(16, 3));
    });
    expect(staged.memory.operations).toEqual([]);
    staged.resources.flushStaged();
    staged.resources.flushStaged();
    expect(staged.memory.operations).toEqual(["submit outer"]);
  });

  it("submits what is staged before a write outside a scope, which lands after it", async () => {
    const staged = await registry();
    staged.resources.staged("upload", () => {
      staged.resources.writeBuffer(staged.a, 0, counting(16, 1));
    });
    staged.resources.writeBuffer(staged.a, 0, counting(8, 9));
    expect(staged.memory.operations).toEqual(["submit upload", "write"]);
    expect(staged.memory.buffer(staged.resources.bufferOf(staged.a).buffer).slice(0, 16)).toEqual(
      Uint8Array.from([...counting(8, 9), ...counting(16, 1).slice(8)]),
    );
  });

  it("keeps the writes made before a throw for the next submission, and throws on", async () => {
    const staged = await registry();
    expect(() =>
      staged.resources.staged("upload", () => {
        staged.resources.writeBuffer(staged.a, 0, counting(16, 1));
        throw new Error("a bake that did not fit");
      }),
    ).toThrow("a bake that did not fit");
    staged.resources.flushStaged();
    expect(staged.memory.operations).toEqual(["submit upload"]);
    expect(staged.memory.buffer(staged.resources.bufferOf(staged.a).buffer).slice(0, 16)).toEqual(
      counting(16, 1),
    );
  });

  it("submits what is staged before a destruction inside the scope", async () => {
    const staged = await registry();
    const buffer = staged.resources.bufferOf(staged.b).buffer;
    let destroyedAtSubmit: boolean | null = null;
    const submit = staged.gpu.queue.submit.bind(staged.gpu.queue);
    staged.gpu.queue.submit = (buffers) => {
      destroyedAtSubmit = buffer instanceof FakeBuffer ? buffer.destroyed : null;
      return submit(buffers);
    };
    staged.resources.staged("upload", () => {
      staged.resources.writeBuffer(staged.b, 0, counting(16, 1));
      staged.resources.destroyBuffer(staged.b);
    });
    expect([destroyedAtSubmit, staged.gpu.queue.submitted.length]).toEqual([false, 1]);
  });

  it("drops what a disposal inside the scope leaves unsubmitted", async () => {
    const staged = await registry();
    staged.resources.staged("upload", () => {
      staged.resources.writeBuffer(staged.a, 0, counting(16, 1));
      staged.resources.dispose();
    });
    expect(staged.memory.operations).toEqual([]);
  });

  it("destroys the staging buffers a disposal inside the scope leaves unsubmitted (lane perf-long-tasks-check)", async () => {
    const staged = await registry();
    const before = staged.gpu.buffers.length;
    staged.resources.staged("upload", () => {
      staged.resources.writeBuffer(staged.a, 0, counting(16, 1));
      staged.resources.dispose();
    });
    const made = staged.gpu.buffers.slice(before);
    expect(made.map((buffer) => buffer.usage & BUFFER_USAGE.MAP_WRITE)).toEqual([
      BUFFER_USAGE.MAP_WRITE,
    ]);
    expect(made.every((buffer) => buffer.destroyed)).toBe(true);
  });

  it("submits a scope past a part's bytes in parts, in order, with the same bytes", async () => {
    const direct = await registry();
    writeAll(direct);
    const staged = await registry(1024);
    staged.resources.staged("upload", () => {
      writeAll(staged);
    });
    staged.resources.flushStaged();
    expect(contents(staged)).toEqual(contents(direct));
    expect(staged.memory.operations.length).toBeGreaterThan(2);
    expect(new Set(staged.memory.operations)).toEqual(new Set(["submit upload"]));
  });

  it("declines a write the copies cannot make: unaligned, out of bounds, short, outside the level or of a depth format", async () => {
    const gpu = await device();
    const staged = new StagedWrites(gpu);
    const usage = BUFFER_USAGE.COPY_DST;
    const buffer = gpu.createBuffer({ label: "b", size: 16, usage });
    const readOnly = gpu.createBuffer({ label: "r", size: 16, usage: BUFFER_USAGE.STORAGE });
    const texture = (format: GPUTextureFormat): GPUTexture =>
      gpu.createTexture({
        label: format,
        size: [4, 4, 1],
        format,
        mipLevelCount: 2,
        usage: TEXTURE_USAGE.COPY_DST,
      });
    const mask = texture("r8unorm");
    const whole = { width: 4, height: 4, depthOrArrayLayers: 1 };
    const results = staged.run("upload", () => [
      staged.buffer(buffer, 0, new Uint8Array(3)),
      staged.buffer(buffer, 2, new Uint8Array(4)),
      staged.buffer(buffer, 8, new Uint8Array(12)),
      staged.buffer(readOnly, 0, new Uint8Array(4)),
      staged.texture(mask, [0, 0, 0], 0, whole, 4, new Uint8Array(15)),
      staged.texture(mask, [0, 0, 0], 1, whole, 4, new Uint8Array(16)),
      staged.texture(mask, [1, 0, 0], 0, whole, 4, new Uint8Array(16)),
      staged.texture(texture("depth16unorm"), [0, 0, 0], 0, whole, 8, new Uint8Array(32)),
      staged.buffer(buffer, 12, new Uint8Array(4)),
      staged.texture(mask, [0, 0, 0], 0, whole, 4, new Uint8Array(16)),
    ]);
    expect(results).toEqual([false, false, false, false, false, false, false, false, true, true]);
  });

  it("hands a declined write to the queue after what is staged, and stages what follows", async () => {
    const staged = await registry();
    staged.resources.staged("upload", () => {
      staged.resources.writeBuffer(staged.a, 0, counting(16, 1));
      staged.resources.writeBuffer(staged.a, 16, counting(3, 2));
      staged.resources.writeBuffer(staged.a, 20, counting(8, 3));
    });
    staged.resources.flushStaged();
    expect(staged.memory.operations).toEqual(["submit upload", "write", "submit upload"]);
    expect(staged.memory.buffer(staged.resources.bufferOf(staged.a).buffer).slice(0, 28)).toEqual(
      Uint8Array.from([...counting(16, 1), ...counting(3, 2), 0, ...counting(8, 3)]),
    );
  });

  it("copies a texture write's origin as it is made", async () => {
    const direct = await registry();
    const origin = { x: 1, y: 2, z: 0 };
    direct.resources.writeTexture(direct.normals, origin, [3, 2, 1], counting(48, 5));
    const staged = await registry();
    const moving = { x: 1, y: 2, z: 0 };
    staged.resources.staged("upload", () => {
      staged.resources.writeTexture(staged.normals, moving, [3, 2, 1], counting(48, 5));
      moving.x = 4;
    });
    staged.resources.flushStaged();
    expect(contents(staged)).toEqual(contents(direct));
  });
});
