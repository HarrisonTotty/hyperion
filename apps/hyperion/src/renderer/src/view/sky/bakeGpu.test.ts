import { describe, expect, it } from "vitest";

import { FakeAdapter, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { FakeRenderEngine } from "../../test/fakeRenderEngine";
import type { BufferSpec, MemoryCategory, TextureSpec } from "../engine/memory";
import type {
  BufferHandle,
  ComputeHandle,
  PointSplatHandle,
  RenderEngine,
  TextureHandle,
} from "../engine/types";
import { bakeSkyCube } from "./bake";

/** What the recording engine saw of a GPU bake. */
interface Seen {
  readonly made: Array<{ name: string; category: MemoryCategory }>;
  readonly released: string[];
  readonly copies: Array<{ level: number; face: number | undefined }>;
  readonly dispatches: string[];
  splats: number;
}

/**
 * An engine on a device with float32-blendable that records a GPU bake's calls; `failOn` makes the
 * texture of that name throw, as a device out of memory would.
 */
async function gpuEngine(
  failOn: string | null = null,
): Promise<{ engine: RenderEngine; seen: Seen }> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: ["float32-blendable"] });
  await adapter.requestDevice({ requiredFeatures: ["float32-blendable"] });
  const device = adapter.devices.at(0);
  if (device === undefined) {
    throw new Error("the fake adapter made no device");
  }
  const seen: Seen = { made: [], released: [], copies: [], dispatches: [], splats: 0 };
  const engine = Object.assign(new FakeRenderEngine(device), {
    createCompute: (pair: { readonly name: string }): ComputeHandle => ({
      kind: "compute",
      name: pair.name,
      path: "reference",
    }),
    createPointSplat: (): PointSplatHandle => ({
      draw: (): void => {
        seen.splats += 1;
      },
      dispose: (): void => undefined,
    }),
    createBuffer: (spec: BufferSpec): BufferHandle => {
      seen.made.push({ name: spec.name, category: spec.category });
      return { kind: "buffer", name: spec.name, bytes: spec.bytes };
    },
    createTexture: (spec: TextureSpec): TextureHandle => {
      if (spec.name === failOn) {
        throw new Error(`out of memory for ${spec.name}`);
      }
      seen.made.push({ name: spec.name, category: spec.category });
      return { kind: "texture", name: spec.name };
    },
    createPackedCube: (
      _size: number,
      _mips: number,
      category: MemoryCategory,
      name = "packed star cube",
    ): TextureHandle => {
      seen.made.push({ name, category });
      return { kind: "texture", name };
    },
    writeBuffer: (): void => undefined,
    dispatch: (kernel: ComputeHandle): void => {
      seen.dispatches.push(kernel.name);
    },
    writePackedCubeLevelFromBuffer: (
      _cube: TextureHandle,
      level: number,
      _packed: BufferHandle,
      face?: number,
    ): void => {
      seen.copies.push({ level, face });
    },
    releaseBuffer: (buffer: BufferHandle): void => {
      seen.released.push(buffer.name);
    },
    releaseTexture: (texture: TextureHandle): void => {
      seen.released.push(texture.name);
    },
  });
  return { engine, seen };
}

const INPUT = {
  directions: Float32Array.from([1, 0, 0, 0, -1, 0]),
  illuminanceLx: Float32Array.from([1e-6, 1e-6, 1e-6, 2e-6, 2e-6, 2e-6]),
  faceSizePx: 8,
  name: "sky cube: test",
} as const;

describe("bakeSkyCube on the GPU", () => {
  it("splats each face twice and copies every level of each face, one face at a time", async () => {
    const { engine, seen } = await gpuEngine();
    const baked = bakeSkyCube(engine, INPUT);
    expect(baked.path).toBe("gpu");
    // Six faces in the peak pass and six again; 8² has four levels.
    expect(seen.splats).toBe(12);
    expect(seen.copies).toHaveLength(6 * 4);
    expect(seen.copies.slice(0, 4)).toEqual([0, 1, 2, 3].map((level) => ({ level, face: 0 })));
    expect(seen.copies.at(-1)).toEqual({ level: 3, face: 5 });
    expect(seen.dispatches.filter((name) => name === "sky bake peak")).toHaveLength(6);
  });

  it("releases every transient and keeps the cube and its peak, each in its category", async () => {
    const { engine, seen } = await gpuEngine();
    bakeSkyCube(engine, INPUT);
    const kept = seen.made.filter((made) => !seen.released.includes(made.name));
    expect(kept).toEqual([
      { name: "sky cube: test peak", category: "sky-cube" },
      { name: "sky cube: test", category: "sky-cube" },
    ]);
    expect(
      seen.made.filter((made) => seen.released.includes(made.name)).map((m) => m.category),
    ).toEqual(["sky-scratch", "sky-scratch", "sky-scratch", "sky-scratch"]);
  });

  it("releases everything it made when it fails partway", async () => {
    const { engine, seen } = await gpuEngine("sky cube: test face levels");
    expect(() => bakeSkyCube(engine, INPUT)).toThrow(/out of memory/);
    expect(seen.made.map((made) => made.name).toSorted()).toEqual(seen.released.toSorted());
  });
});
