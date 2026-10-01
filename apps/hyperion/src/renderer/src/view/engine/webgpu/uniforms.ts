/**
 * The `Frame` and `Draw` uniform blocks of the shader convention (R01 Design note 23): their
 * layouts by WGSL's uniform rules, their packing, and the per-frame ring that holds every draw's
 * `Draw` at a dynamic offset.
 */

import { BUFFER_USAGE } from "../gpuFlags";
import type { BufferSpec } from "../memory";
import type { BufferHandle, FrameSubmission, UniformSpec, ViewSize } from "../types";

/** One member of a uniform struct, with its offset in bytes. */
export interface StructMember {
  readonly name: string;
  readonly type: string;
  readonly offsetBytes: number;
  readonly sizeBytes: number;
}

/** A uniform struct's members and its size, by WGSL's rules for a uniform buffer. */
export interface StructLayout {
  readonly members: ReadonlyArray<StructMember>;
  readonly sizeBytes: number;
}

/** The size and alignment of each type a uniform struct here may hold. */
const LAYOUT: Readonly<Record<string, { readonly size: number; readonly align: number }>> = {
  f32: { size: 4, align: 4 },
  u32: { size: 4, align: 4 },
  vec2f: { size: 8, align: 8 },
  vec3f: { size: 12, align: 16 },
  vec4f: { size: 16, align: 16 },
  vec2u: { size: 8, align: 8 },
  vec3u: { size: 12, align: 16 },
  vec4u: { size: 16, align: 16 },
  mat3x3f: { size: 48, align: 16 },
  mat4x4f: { size: 64, align: 16 },
};

/** The `Frame` block's size: two `mat4x4f`, then a `vec4f` (`view/shaders/frame.wgsl`). */
export const FRAME_BYTES = 144;

/** The member a material's `Draw` begins with, the draw's offset from the camera. */
export const OFFSET_MEMBER: UniformSpec = { name: "offsetFromCameraM", type: "vec3f" };

/**
 * The layout of a struct of `members`, in order.
 *
 * @throws Error naming the member when its type has no layout here.
 */
export function uniformLayout(
  members: ReadonlyArray<{ readonly name: string; readonly type: string }>,
): StructLayout {
  const laid: StructMember[] = [];
  let offset = 0;
  let structAlign = 16;
  for (const member of members) {
    const layout = LAYOUT[member.type];
    if (layout === undefined) {
      throw new Error(`uniform ${member.name} has a type, ${member.type}, with no layout`);
    }
    offset = Math.ceil(offset / layout.align) * layout.align;
    laid.push({
      name: member.name,
      type: member.type,
      offsetBytes: offset,
      sizeBytes: layout.size,
    });
    offset += layout.size;
    structAlign = Math.max(structAlign, layout.align);
  }
  return { members: laid, sizeBytes: Math.max(16, Math.ceil(offset / structAlign) * structAlign) };
}

/** A WGSL type in its short form: `vec3<f32>` as `vec3f`. */
function shortType(type: string): string {
  return type
    .replaceAll(/\s+/gu, "")
    .replace(/^(vec[234]|mat[234]x[234])<f32>$/u, "$1f")
    .replace(/^(vec[234])<u32>$/u, "$1u");
}

/**
 * The members of the struct `name` declares in `wgsl`, laid out by WGSL's uniform rules, or
 * `null` when it declares none.
 *
 * @throws Error naming the struct and member when a member's type has no layout here.
 */
export function declaredStructLayout(wgsl: string, name: string): StructLayout | null {
  const withoutComments = wgsl.replaceAll(/\/\/[^\n]*/gu, "");
  const match = new RegExp(String.raw`struct\s+${name}\s*\{([^}]*)\}`, "u").exec(withoutComments);
  if (match === null) {
    return null;
  }
  const members: Array<{ readonly name: string; readonly type: string }> = [];
  for (const declaration of (match[1] ?? "").split(",")) {
    const parts = /^\s*(?:@\w+(?:\([^)]*\))?\s*)*(\w+)\s*:\s*(.+?)\s*$/u.exec(declaration);
    if (parts === null) {
      continue;
    }
    const [, member = "", rawType = ""] = parts;
    const type = shortType(rawType);
    if (LAYOUT[type] === undefined) {
      throw new Error(`struct ${name}'s member ${member} has a type, ${rawType}, with no layout`);
    }
    members.push({ name: member, type });
  }
  return uniformLayout(members);
}

/**
 * Checks that every source of `owner` that declares a `Draw` struct declares the layout the engine
 * packs.
 *
 * @throws Error naming the owner, and the first member that differs, when one does not.
 */
export function assertDrawStruct(
  owner: string,
  expected: StructLayout,
  ...sources: ReadonlyArray<string>
): void {
  for (const source of sources) {
    const declared = declaredStructLayout(source, "Draw");
    if (declared === null) {
      continue;
    }
    const count = Math.max(declared.members.length, expected.members.length);
    for (let index = 0; index < count; index += 1) {
      const has = declared.members[index];
      const wants = expected.members[index];
      if (
        has?.name !== wants?.name ||
        has?.type !== wants?.type ||
        has?.offsetBytes !== wants?.offsetBytes
      ) {
        throw new Error(
          `${owner}'s struct Draw has ${has === undefined ? "nothing" : `${has.name} : ${has.type}`} ` +
            `where the engine packs ${wants === undefined ? "nothing" : `${wants.name} : ${wants.type}`}`,
        );
      }
    }
  }
}

/**
 * Writes `values` into `into` at `offsetBytes` by `layout`: an unsigned member as integers, every
 * other as `f32`s; a member with no value stays zero.
 */
export function packUniforms(
  layout: StructLayout,
  values: (name: string) => ArrayLike<number> | undefined,
  into: ArrayBuffer,
  offsetBytes: number,
): void {
  const floats = new Float32Array(into, offsetBytes, layout.sizeBytes / 4);
  const uints = new Uint32Array(into, offsetBytes, layout.sizeBytes / 4);
  floats.fill(0);
  for (const member of layout.members) {
    const value = values(member.name);
    if (value === undefined) {
      continue;
    }
    const count = Math.min(value.length, member.sizeBytes / 4);
    const start = member.offsetBytes / 4;
    const integral = member.type === "u32" || member.type.endsWith("u");
    for (let index = 0; index < count; index += 1) {
      const component = value[index] ?? 0;
      if (integral) {
        uints[start + index] = component;
      } else {
        floats[start + index] = component;
      }
    }
  }
}

/** The `Frame` block of `frame` drawn at `size`. */
export function packFrame(frame: FrameSubmission, size: ViewSize): Float32Array<ArrayBuffer> {
  const block = new Float32Array(FRAME_BYTES / 4);
  block.set(frame.viewRotation.subarray(0, 16), 0);
  block.set(frame.projection.subarray(0, 16), 16);
  block.set([size.widthPx, size.heightPx, 1 / size.widthPx, 1 / size.heightPx], 32);
  return block;
}

/** What the ring needs of the engine: its one creation path for buffers. */
export interface RingHost {
  readonly device: Pick<GPUDevice, "createBindGroup"> & {
    readonly limits: Pick<GPUSupportedLimits, "minUniformBufferOffsetAlignment">;
  };
  createBuffer(spec: BufferSpec): BufferHandle;
  gpuBufferOf(handle: BufferHandle): GPUBuffer;
  destroyBuffer(handle: BufferHandle): void;
  writeBuffer(handle: BufferHandle, offsetBytes: number, data: ArrayBufferView): void;
}

/** A slot the ring gave a block: its dynamic offset and its size. */
export interface RingSlot {
  readonly offsetBytes: number;
  readonly sizeBytes: number;
}

/**
 * One frame's `Draw` blocks in one uniform buffer, each at a dynamic offset aligned to the
 * device's `minUniformBufferOffsetAlignment`.
 *
 * @remarks
 * A frame calls {@link UniformRing.begin}, takes a slot per block, {@link UniformRing.upload}s,
 * and only then asks each slot's {@link UniformRing.bindGroup} while it encodes, since a slot taken
 * later may grow the buffer. The queue orders the upload before the frame's commands and after the
 * previous frame's, so one buffer serves every frame. It grows by doubling, through the engine's
 * one creation path, so its bytes and uploads are counted.
 */
export class UniformRing {
  readonly #host: RingHost;
  readonly #layout: GPUBindGroupLayout;
  readonly #alignment: number;
  #buffer: BufferHandle | null = null;
  #capacityBytes = 0;
  #staging = new ArrayBuffer(0);
  #usedBytes = 0;
  /** A bind group per binding size, over the current buffer. */
  readonly #bindGroups = new Map<number, GPUBindGroup>();

  constructor(host: RingHost, layout: GPUBindGroupLayout) {
    this.#host = host;
    this.#layout = layout;
    this.#alignment = host.device.limits.minUniformBufferOffsetAlignment;
  }

  /** Starts a frame: its slots are taken from the start of the buffer. */
  begin(): void {
    this.#usedBytes = 0;
  }

  /** A slot for one block of `layout`, packed from `values`. */
  push(layout: StructLayout, values: (name: string) => ArrayLike<number> | undefined): RingSlot {
    const offsetBytes = this.#usedBytes;
    const end = offsetBytes + layout.sizeBytes;
    if (end > this.#staging.byteLength) {
      const grown = new ArrayBuffer(Math.max(4096, 2 ** Math.ceil(Math.log2(end))));
      new Uint8Array(grown).set(new Uint8Array(this.#staging));
      this.#staging = grown;
    }
    packUniforms(layout, values, this.#staging, offsetBytes);
    this.#usedBytes = Math.ceil(end / this.#alignment) * this.#alignment;
    this.#ensureCapacity(this.#usedBytes);
    return { offsetBytes, sizeBytes: layout.sizeBytes };
  }

  /** The bind group of a slot's size over the current buffer, asked after the frame's slots. */
  bindGroup(slot: RingSlot): GPUBindGroup {
    let bindGroup = this.#bindGroups.get(slot.sizeBytes);
    if (bindGroup === undefined) {
      if (this.#buffer === null) {
        throw new Error("the uniform ring has no buffer");
      }
      bindGroup = this.#host.device.createBindGroup({
        label: `draw uniforms ${slot.sizeBytes}`,
        layout: this.#layout,
        entries: [
          {
            binding: 0,
            resource: { buffer: this.#host.gpuBufferOf(this.#buffer), size: slot.sizeBytes },
          },
        ],
      });
      this.#bindGroups.set(slot.sizeBytes, bindGroup);
    }
    return bindGroup;
  }

  /** Writes the frame's blocks to the GPU; called before the frame is submitted. */
  upload(): void {
    if (this.#usedBytes === 0 || this.#buffer === null) {
      return;
    }
    this.#host.writeBuffer(this.#buffer, 0, new Uint8Array(this.#staging, 0, this.#usedBytes));
  }

  /** Destroys the buffer. */
  dispose(): void {
    if (this.#buffer !== null) {
      this.#host.destroyBuffer(this.#buffer);
      this.#buffer = null;
    }
    this.#bindGroups.clear();
  }

  #ensureCapacity(bytes: number): void {
    if (bytes <= this.#capacityBytes) {
      return;
    }
    // A frame's slots taken before the growth keep their offsets in the new, larger buffer, since
    // the whole staging copy is uploaded at once.
    const capacity = Math.max(65_536, 2 ** Math.ceil(Math.log2(bytes)));
    if (this.#buffer !== null) {
      this.#host.destroyBuffer(this.#buffer);
    }
    this.#buffer = this.#host.createBuffer({
      name: "draw uniforms",
      bytes: capacity,
      usage: BUFFER_USAGE.UNIFORM | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    this.#capacityBytes = capacity;
    this.#bindGroups.clear();
  }
}
