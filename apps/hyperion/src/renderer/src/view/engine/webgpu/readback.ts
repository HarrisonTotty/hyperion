/**
 * CPU readback, the one guarded path (R01 Design note 20).
 *
 * @remarks
 * `readBuffer` and `readTexture` copy through `copyBufferToBuffer` or `copyTextureToBuffer` into a
 * `MAP_READ` staging buffer and map it. Each buffer and texture records the kernel that last wrote
 * it, so the refusal of a `presentation-only` result (Design note 16) holds at the one place a
 * result reaches the CPU. A request the GPU would reject is refused here first, with an error that
 * names it: an invalid copy is dropped by the queue, and the read would otherwise give zeros.
 */

import { BUFFER_USAGE, MAP_MODE, TEXTURE_USAGE } from "../gpuFlags";
import type { KernelPair } from "../kernels";
import { bytesPerTexel, extentOf, type BufferSpec, type TextureSpec } from "../memory";
import { PresentationOnlyReadback, type TexelRect } from "../types";

/** The kernel that last wrote each resource, by its handle. */
export class WriterRecord {
  readonly #writers = new WeakMap<object, KernelPair>();

  /** Records that `kernel` has written `resource`. */
  wroteBy(resource: object, kernel: KernelPair): void {
    this.#writers.set(resource, kernel);
  }

  /**
   * Records that something other than a kernel has replaced the whole of `resource`: a draw that
   * clears it, or an upload that covers every byte or texel.
   *
   * @remarks
   * A write of part of a resource, or one that adds to what is there (a splat), goes through
   * {@link WriterRecord.wrotePart} instead: the rest still holds what its last kernel wrote, so a
   * `presentation-only` mark stays.
   */
  wroteOtherwise(resource: object): void {
    this.#writers.delete(resource);
  }

  /**
   * Records a write of part of `resource`, by `kernel` or (`null`) by something else: it marks the
   * resource `presentation-only` if that kernel is, and otherwise leaves the record as it was.
   */
  wrotePart(resource: object, kernel: KernelPair | null): void {
    if (kernel?.readback === "presentation-only") {
      this.#writers.set(resource, kernel);
    }
  }

  /**
   * Records that `to` was copied from `from`: the whole of `to`, so that it carries `from`'s
   * writer, or (`whole` false) a part of it, as {@link WriterRecord.wrotePart} records.
   */
  copied(from: object, to: object, whole: boolean): void {
    const writer = this.#writers.get(from) ?? null;
    if (!whole) {
      this.wrotePart(to, writer);
    } else if (writer === null) {
      this.#writers.delete(to);
    } else {
      this.#writers.set(to, writer);
    }
  }

  /**
   * Refuses to read `resource` back if a `presentation-only` kernel wrote it last.
   *
   * @param access - `tolerance` lifts the refusal, for the smoke harness's tolerance checks alone.
   * @throws {@link PresentationOnlyReadback} naming the kernel.
   */
  assertReadable(resource: object, access: "cpu" | "tolerance"): void {
    const writer = this.#writers.get(resource);
    if (writer?.readback === "presentation-only" && access !== "tolerance") {
      throw new PresentationOnlyReadback(writer.name);
    }
  }
}

/** Whether a write of `bytes` at `offsetBytes` covers every byte of a buffer of `spec`. */
export function coversBuffer(spec: BufferSpec, offsetBytes: number, bytes: number): boolean {
  return offsetBytes === 0 && bytes >= spec.bytes;
}

/** A `GPUOrigin3D` as its three numbers, the absent ones 0. */
function originOf(origin: GPUOrigin3D): readonly [number, number, number] {
  if (Symbol.iterator in origin) {
    const [x = 0, y = 0, z = 0] = [...origin];
    return [x, y, z];
  }
  return [origin.x ?? 0, origin.y ?? 0, origin.z ?? 0];
}

/**
 * Whether a write of `size` texels at `origin` of `level` covers every texel of a texture of
 * `spec`: every layer of its one level.
 */
export function coversTexture(
  spec: TextureSpec,
  origin: GPUOrigin3D,
  size: GPUExtent3D,
  level: number,
): boolean {
  if (spec.mips !== 1 || level !== 0 || originOf(origin).some((axis) => axis !== 0)) {
    return false;
  }
  const whole = extentOf(spec.size);
  const written = extentOf(size);
  const layers = spec.dimension === "cube" ? 6 : whole.depthOrArrayLayers;
  return (
    written.width >= whole.width &&
    written.height >= whole.height &&
    written.depthOrArrayLayers >= layers
  );
}

/** Rows of a texel copy are 256-byte aligned in the buffer (WebGPU's `bytesPerRow` rule). */
export const COPY_ROW_ALIGNMENT = 256;

/** The padded row pitch of a copy of `widthTexels` texels of `texelBytes` each. */
export function paddedBytesPerRow(widthTexels: number, texelBytes: number): number {
  return Math.ceil((widthTexels * texelBytes) / COPY_ROW_ALIGNMENT) * COPY_ROW_ALIGNMENT;
}

/** How a texel region lies in a staging buffer: rows padded, layer after layer. */
export interface StagingLayout {
  readonly bytesPerRow: number;
  /** Bytes of one row's texels, without the padding. */
  readonly rowBytes: number;
  /** Rows of one layer. */
  readonly rows: number;
  readonly layers: number;
  readonly bytes: number;
}

/** The staging layout of `layers` regions of `widthTexels × heightTexels` texels. */
export function stagingLayout(
  widthTexels: number,
  heightTexels: number,
  texelBytes: number,
  layers = 1,
): StagingLayout {
  const bytesPerRow = paddedBytesPerRow(widthTexels, texelBytes);
  return {
    bytesPerRow,
    rowBytes: widthTexels * texelBytes,
    rows: heightTexels,
    layers,
    bytes: bytesPerRow * heightTexels * layers,
  };
}

/** The texels of a padded copy, rows packed tightly, layer after layer. */
export function unpad(padded: Uint8Array, layout: StagingLayout): ArrayBuffer {
  const texels = new Uint8Array(layout.rowBytes * layout.rows * layout.layers);
  for (let row = 0; row < layout.rows * layout.layers; row += 1) {
    const start = row * layout.bytesPerRow;
    texels.set(padded.subarray(start, start + layout.rowBytes), row * layout.rowBytes);
  }
  return texels.buffer;
}

/** What a texture read copies: a region of one level, every layer or slice of it. */
export interface TextureRead {
  readonly level: number;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  /** Faces of a cube, layers of a 2D array, or slices of a 3D texture at that level. */
  readonly layers: number;
  readonly bytesPerTexel: number;
  readonly aspect: GPUTextureAspect;
}

/** Depth formats a copy can read: their depth aspect is 32-bit float. */
const COPYABLE_DEPTH: ReadonlySet<GPUTextureFormat> = new Set([
  "depth32float",
  "depth32float-stencil8",
]);

/**
 * What reading `rect` of `level` of a texture of `spec` copies.
 *
 * @throws Error naming the texture when the read is one the GPU would reject: no `COPY_SRC`, a
 * level it lacks, a rect outside the level, a part of a depth level (a depth copy covers the whole
 * subresource), or a depth format whose depth cannot be copied.
 */
export function textureRead(spec: TextureSpec, level: number, rect?: TexelRect): TextureRead {
  if ((spec.usage & TEXTURE_USAGE.COPY_SRC) === 0) {
    throw new Error(`${spec.name} was not made with COPY_SRC`);
  }
  if (!Number.isInteger(level) || level < 0 || level >= spec.mips) {
    throw new Error(`${spec.name} has no level ${level}`);
  }
  const extent = extentOf(spec.size);
  const width = Math.max(1, extent.width >> level);
  const height = Math.max(1, extent.height >> level);
  const layers =
    spec.dimension === "cube"
      ? 6
      : spec.dimension === "3d"
        ? Math.max(1, extent.depthOrArrayLayers >> level)
        : extent.depthOrArrayLayers;
  const region = rect ?? { x: 0, y: 0, width, height };
  if (
    region.x < 0 ||
    region.y < 0 ||
    region.width < 1 ||
    region.height < 1 ||
    region.x + region.width > width ||
    region.y + region.height > height
  ) {
    throw new Error(`the region is outside level ${level} of ${spec.name}, ${width} × ${height}`);
  }
  const isDepth = spec.format.startsWith("depth");
  if (isDepth && !COPYABLE_DEPTH.has(spec.format)) {
    throw new Error(`the depth of ${spec.name}, ${spec.format}, cannot be copied`);
  }
  if (isDepth && (region.width !== width || region.height !== height)) {
    throw new Error(`a depth copy of ${spec.name} covers the whole level`);
  }
  return {
    level,
    ...region,
    layers,
    bytesPerTexel: isDepth ? 4 : bytesPerTexel(spec.format),
    aspect: isDepth ? "depth-only" : "all",
  };
}

/**
 * Checks that a buffer can be read back whole.
 *
 * @throws Error naming the buffer when it lacks `COPY_SRC` or its size is not a multiple of 4,
 * which a buffer copy requires.
 */
export function assertBufferReadable(spec: BufferSpec): void {
  if ((spec.usage & BUFFER_USAGE.COPY_SRC) === 0) {
    throw new Error(`${spec.name} was not made with COPY_SRC`);
  }
  if (spec.bytes % 4 !== 0) {
    throw new Error(`${spec.name} holds ${spec.bytes} bytes, not a multiple of 4`);
  }
}

/** Copies a staging buffer's contents to the CPU. */
async function mapAndCopy(staging: GPUBuffer): Promise<Uint8Array<ArrayBuffer>> {
  await staging.mapAsync(MAP_MODE.READ);
  return new Uint8Array(staging.getMappedRange().slice(0));
}

/**
 * Encodes a copy into a staging buffer of `bytes`, submits it and maps the result, destroying the
 * staging buffer however that ends.
 */
async function readThroughStaging(
  device: GPUDevice,
  label: string,
  bytes: number,
  submit: (encode: (encoder: GPUCommandEncoder) => void) => void,
  copy: (encoder: GPUCommandEncoder, staging: GPUBuffer) => void,
): Promise<Uint8Array<ArrayBuffer>> {
  const staging = device.createBuffer({
    label: `${label} readback`,
    size: bytes,
    usage: BUFFER_USAGE.COPY_DST | BUFFER_USAGE.MAP_READ,
  });
  try {
    submit((encoder) => {
      copy(encoder, staging);
    });
    return await mapAndCopy(staging);
  } finally {
    staging.destroy();
  }
}

/**
 * Reads a whole buffer back.
 *
 * @param submit - Submits the encoded copy after what the engine has recorded so far.
 */
export async function readGpuBuffer(
  device: GPUDevice,
  buffer: GPUBuffer,
  bytes: number,
  submit: (encode: (encoder: GPUCommandEncoder) => void) => void,
): Promise<ArrayBuffer> {
  const copied = await readThroughStaging(device, buffer.label, bytes, submit, (encoder, to) => {
    encoder.copyBufferToBuffer(buffer, 0, to, 0, bytes);
  });
  return copied.buffer;
}

/**
 * Reads a region of one level of a texture back, every layer of it, rows tightly packed.
 *
 * @param submit - Submits the encoded copy after what the engine has recorded so far.
 */
export async function readGpuTexture(
  device: GPUDevice,
  texture: GPUTexture,
  read: TextureRead,
  submit: (encode: (encoder: GPUCommandEncoder) => void) => void,
): Promise<ArrayBuffer> {
  const layout = stagingLayout(read.width, read.height, read.bytesPerTexel, read.layers);
  const copied = await readThroughStaging(
    device,
    texture.label,
    layout.bytes,
    submit,
    (encoder, to) => {
      encoder.copyTextureToBuffer(
        {
          texture,
          mipLevel: read.level,
          origin: { x: read.x, y: read.y, z: 0 },
          aspect: read.aspect,
        },
        { buffer: to, bytesPerRow: layout.bytesPerRow, rowsPerImage: read.height },
        { width: read.width, height: read.height, depthOrArrayLayers: read.layers },
      );
    },
  );
  return unpad(copied, layout);
}
