/**
 * The GPU memory categories and the one creation path's specs and events.
 *
 * @remarks
 * Every GPU buffer and texture is created through the engine's `createBuffer`, `createTexture` or
 * `createPackedCube`, each with a category, and every creation, destruction and upload raises an
 * {@link AllocationEvent}: R05's tally is built on them and R12 itemises them (R01 Design note 18).
 * Later plans add their categories to {@link MemoryCategory} here.
 */

/** What a GPU allocation is for. */
export type MemoryCategory = "render-targets" | "other";

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
