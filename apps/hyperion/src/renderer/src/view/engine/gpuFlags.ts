/**
 * WebGPU's usage and mode flags, as the specification numbers them.
 *
 * @remarks
 * TypeScript 7's `lib.dom` declares the flag types (`GPUBufferUsageFlags` and the rest) but not the
 * `GPUBufferUsage`, `GPUTextureUsage` and `GPUMapMode` namespaces that hold their values, so the
 * values are written here, from the WebGPU specification (W3C, §5.1 "GPUBufferUsage", §6.1
 * "GPUTextureUsage", §5.2 "GPUMapMode"). Every `BufferSpec.usage` and `TextureSpec.usage` is built
 * from them.
 */

/** `GPUBufferUsage`'s flags. */
export const BUFFER_USAGE = {
  MAP_READ: 0x0001,
  MAP_WRITE: 0x0002,
  COPY_SRC: 0x0004,
  COPY_DST: 0x0008,
  INDEX: 0x0010,
  VERTEX: 0x0020,
  UNIFORM: 0x0040,
  STORAGE: 0x0080,
  INDIRECT: 0x0100,
  QUERY_RESOLVE: 0x0200,
} as const;

/** `GPUTextureUsage`'s flags. */
export const TEXTURE_USAGE = {
  COPY_SRC: 0x01,
  COPY_DST: 0x02,
  TEXTURE_BINDING: 0x04,
  STORAGE_BINDING: 0x08,
  RENDER_ATTACHMENT: 0x10,
} as const;

/** `GPUMapMode`'s flags. */
export const MAP_MODE = {
  READ: 0x0001,
  WRITE: 0x0002,
} as const;
