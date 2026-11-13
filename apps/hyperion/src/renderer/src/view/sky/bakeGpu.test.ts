import { describe, expect, it } from "vitest";

import { FakeAdapter, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { FakeRenderEngine } from "../../test/fakeRenderEngine";
import type { BufferSpec, MemoryCategory, TextureSpec } from "../engine/memory";
import type {
  BufferHandle,
  ComputeHandle,
  PointSplatHandle,
  RenderEngine,
  TexelRect,
  TextureHandle,
} from "../engine/types";
import { BAKE_TILE_PX, bakeSkyCube, bakeTiling } from "./bake";

/** What the recording engine saw of a GPU bake. */
interface Seen {
  readonly made: Array<{ name: string; category: MemoryCategory }>;
  /** Each scratch texture's side and levels, and each buffer's bytes, by name. */
  readonly sizes: Map<string, string>;
  readonly released: string[];
  readonly copies: Array<{ level: number; face: number | undefined; region?: TexelRect }>;
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
  const seen: Seen = {
    made: [],
    sizes: new Map(),
    released: [],
    copies: [],
    dispatches: [],
    splats: 0,
  };
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
      seen.sizes.set(spec.name, `${spec.bytes} B`);
      return { kind: "buffer", name: spec.name, bytes: spec.bytes };
    },
    createTexture: (spec: TextureSpec): TextureHandle => {
      if (spec.name === failOn) {
        throw new Error(`out of memory for ${spec.name}`);
      }
      seen.made.push({ name: spec.name, category: spec.category });
      seen.sizes.set(spec.name, `${JSON.stringify(spec.size)} × ${spec.mips}`);
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
      region?: TexelRect,
    ): void => {
      seen.copies.push(region === undefined ? { level, face } : { level, face, region });
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

/** A 48² face, 3 × 2⁴, baked in nine tiles of 16². */
const TILED = { ...INPUT, faceSizePx: 48, tilePx: 16 } as const;

/** The square of side `side` at (x, y). */
function square(x: number, y: number, side: number): TexelRect {
  return { x, y, width: side, height: side };
}

describe("bakeSkyCube on the GPU, a tile at a time (the owner's priority)", () => {
  it("tiles the high setting's face in nine and bakes the low setting's whole", () => {
    expect(bakeTiling(3_072)).toEqual({ tilePx: 1_024, tilesPerSide: 3, upperLevel: 3 });
    expect(bakeTiling(1_024)).toEqual({ tilePx: 1_024, tilesPerSide: 1, upperLevel: 0 });
    expect(BAKE_TILE_PX).toBe(1_024);
  });

  it("refuses a tiling whose tiles shrink below a texel before the levels baked whole", () => {
    expect(() => bakeTiling(48, 4)).toThrow(/no tiling/);
  });

  it("copies each tile's first levels into their places, then the face's last levels whole", async () => {
    const { engine, seen } = await gpuEngine();
    bakeSkyCube(engine, TILED);
    // Each face's nine tiles, splatted in the peak pass and again.
    expect(seen.splats).toBe(2 * 6 * 9);
    expect(seen.dispatches.filter((name) => name === "sky bake peak")).toHaveLength(6 * 9);
    // Each tile's levels 0–2, then the face's 6², 3² and 1² whole.
    expect(seen.copies).toHaveLength(6 * (9 * 3 + 3));
    expect(seen.copies.slice(0, 6)).toEqual([
      { level: 0, face: 0, region: square(0, 0, 16) },
      { level: 1, face: 0, region: square(0, 0, 8) },
      { level: 2, face: 0, region: square(0, 0, 4) },
      { level: 0, face: 0, region: square(16, 0, 16) },
      { level: 1, face: 0, region: square(8, 0, 8) },
      { level: 2, face: 0, region: square(4, 0, 4) },
    ]);
    expect(seen.copies.slice(24, 30)).toEqual([
      { level: 0, face: 0, region: square(32, 32, 16) },
      { level: 1, face: 0, region: square(16, 16, 8) },
      { level: 2, face: 0, region: square(8, 8, 4) },
      { level: 3, face: 0 },
      { level: 4, face: 0 },
      { level: 5, face: 0 },
    ]);
    expect(seen.copies.at(-1)).toEqual({ level: 5, face: 5 });
  });

  it("makes a tile's scratch and the face's last levels, and solid angles below the diagonal", async () => {
    const { engine, seen } = await gpuEngine();
    bakeSkyCube(engine, TILED);
    expect(seen.sizes.get("sky cube: test tile levels")).toBe("[16,16] × 3");
    expect(seen.sizes.get("sky cube: test face levels")).toBe("[6,6] × 3");
    // A 16-texel row padded to 64 texels, 16 rows.
    expect(seen.sizes.get("sky cube: test staging")).toBe(`${64 * 16 * 4} B`);
    expect(seen.sizes.get("sky cube: test solid angles")).toBe(`${((48 * 49) / 2) * 4} B`);
    const kept = seen.made.filter((made) => !seen.released.includes(made.name));
    expect(kept.map((made) => made.name)).toEqual(["sky cube: test peak", "sky cube: test"]);
  });
});
