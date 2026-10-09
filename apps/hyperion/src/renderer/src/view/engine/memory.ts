/**
 * The GPU memory categories and the one creation path's specs and events.
 *
 * @remarks
 * Every GPU buffer and texture is created through the engine's `createBuffer`, `createTexture` or
 * `createPackedCube`, each with a category, and released by `releaseBuffer` and `releaseTexture`
 * or at disposal; every creation, destruction and upload raises an
 * {@link AllocationEvent}: R05's tally is built on them and R12 itemises them (R01 Design note 18).
 * Later plans add their categories to {@link MemoryCategory} here.
 */

/**
 * What a GPU allocation is for, by the names R12.T3.a reports (R12 Design note 6).
 *
 * - `atmosphere-tables`: the per-planet transmittance and multiple-scattering tables (R05.T12.b).
 * - `atmosphere-view`: the per-frame sky-view, aerial-perspective and ray-march tables (R05.T12.c).
 * - `height-cache`: the terrain's patch cache, its slot buffers and normals atlas (R05.T11.a).
 * - `sky-cube`: R06's baked star cube and the buffer of its scale (R06.T13.g).
 * - `sky-scratch`: the sky bake's transients, released when it ends (R06.T13.g).
 */
export type MemoryCategory =
  | "render-targets"
  | "other"
  | "atmosphere-tables"
  | "atmosphere-view"
  | "height-cache"
  | "sky-cube"
  | "sky-scratch";

/** A GPU buffer to create. */
export interface BufferSpec {
  readonly name: string;
  readonly bytes: number;
  /** `GPUBufferUsage` flags. */
  readonly usage: GPUBufferUsageFlags;
  readonly category: MemoryCategory;
}

/** A GPU texture to create: 2D, 3D or a cube, sampled and/or storage. */
export interface TextureSpec {
  readonly name: string;
  /** Texels; `GPUExtent3D`, since TypeScript 7's `lib.dom` has no `GPUExtent3DStrict`. */
  readonly size: GPUExtent3D;
  readonly dimension: "2d" | "3d" | "cube";
  readonly format: GPUTextureFormat;
  readonly mips: number;
  /** `GPUTextureUsage` flags. */
  readonly usage: GPUTextureUsageFlags;
  readonly category: MemoryCategory;
}

/** A creation, destruction or upload, with its size in bytes. */
export type AllocationEvent =
  | {
      readonly kind: "created" | "destroyed";
      readonly name: string;
      readonly bytes: number;
      readonly category: MemoryCategory;
    }
  | { readonly kind: "uploaded"; readonly name: string; readonly bytes: number };

/** Texels on each axis, from a `GPUExtent3D` in either of its forms. */
export interface Extent {
  readonly width: number;
  readonly height: number;
  /** Depth of a 3D texture, or array layers of a 2D one. */
  readonly depthOrArrayLayers: number;
}

/** A `GPUExtent3D` as its three numbers, the absent ones 1. */
export function extentOf(size: GPUExtent3D): Extent {
  if (Symbol.iterator in size) {
    const [width = 1, height = 1, depthOrArrayLayers = 1] = [...size];
    return { width, height, depthOrArrayLayers };
  }
  return {
    width: size.width,
    height: size.height ?? 1,
    depthOrArrayLayers: size.depthOrArrayLayers ?? 1,
  };
}

/**
 * Bytes per texel of each uncompressed format a texture here may have.
 *
 * @remarks
 * From the WebGPU specification's texel-format tables (§26.1). `rg11b10ufloat` and
 * `rgb9e5ufloat` pack three channels into 32 bits. The depth formats count their aspects' sizes
 * as the specification gives them: `depth24plus` as 4 bytes and `depth24plus-stencil8` as 4
 * plus 1, though a driver may allocate more.
 */
const BYTES_PER_TEXEL: Readonly<Partial<Record<GPUTextureFormat, number>>> = {
  r8unorm: 1,
  r8snorm: 1,
  r8uint: 1,
  r8sint: 1,
  r16float: 2,
  r16uint: 2,
  r16sint: 2,
  rg8unorm: 2,
  rg8snorm: 2,
  rg8uint: 2,
  rg8sint: 2,
  r32float: 4,
  r32uint: 4,
  r32sint: 4,
  rg16float: 4,
  rg16uint: 4,
  rg16sint: 4,
  rgba8unorm: 4,
  "rgba8unorm-srgb": 4,
  rgba8snorm: 4,
  rgba8uint: 4,
  rgba8sint: 4,
  bgra8unorm: 4,
  "bgra8unorm-srgb": 4,
  rgb10a2unorm: 4,
  rgb10a2uint: 4,
  rg11b10ufloat: 4,
  rgb9e5ufloat: 4,
  rg32float: 8,
  rg32uint: 8,
  rg32sint: 8,
  rgba16float: 8,
  rgba16uint: 8,
  rgba16sint: 8,
  rgba32float: 16,
  rgba32uint: 16,
  rgba32sint: 16,
  stencil8: 1,
  depth16unorm: 2,
  depth24plus: 4,
  "depth24plus-stencil8": 5,
  depth32float: 4,
  "depth32float-stencil8": 5,
};

/**
 * Bytes per texel of `format`.
 *
 * @throws Error for a compressed or unknown format, which no creation here makes.
 */
export function bytesPerTexel(format: GPUTextureFormat): number {
  const bytes = BYTES_PER_TEXEL[format];
  if (bytes === undefined) {
    throw new Error(`no texel size is known for ${format}`);
  }
  return bytes;
}

/**
 * The bytes a texture occupies: every mip of every layer or slice.
 *
 * @remarks
 * A mip halves each axis, rounding down, to no less than 1; a 3D texture's depth halves too, while
 * a 2D texture's layers and a cube's six faces do not. Drivers pad and align beyond this; the
 * figure is the texels' own size, which R05's tally and R12's itemisation compare across runs.
 */
export function textureBytes(spec: TextureSpec): number {
  const { width, height, depthOrArrayLayers } = extentOf(spec.size);
  const layers = spec.dimension === "cube" ? 6 : depthOrArrayLayers;
  const texel = bytesPerTexel(spec.format);
  let total = 0;
  for (let level = 0; level < spec.mips; level += 1) {
    const w = Math.max(1, width >> level);
    const h = Math.max(1, height >> level);
    const d = spec.dimension === "3d" ? Math.max(1, layers >> level) : layers;
    total += w * h * d * texel;
  }
  return total;
}
