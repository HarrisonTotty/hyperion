/**
 * The two `rgba16float` textures a view's or target's post-process chain ping-pongs between, made
 * when a frame first has post-processes and again at each new size.
 */

import { TEXTURE_USAGE } from "../gpuFlags";
import type { MemoryCategory, TextureSpec } from "../memory";
import type { TextureHandle, ViewSize } from "../types";
import { INTERMEDIATE_FORMAT } from "./drawing";

/** What the pair needs of the engine: its one creation path for textures. */
export interface IntermediateHost {
  createTexture(spec: TextureSpec): TextureHandle;
  gpuTextureOf(handle: TextureHandle): GPUTexture;
  destroyTexture(handle: TextureHandle): void;
}

/** The specification of one intermediate of `owner` at `size`. */
export function intermediateSpec(
  owner: string,
  index: number,
  size: ViewSize,
  category: MemoryCategory,
): TextureSpec {
  return {
    name: `${owner}:post-process ${index}`,
    size: [size.widthPx, size.heightPx],
    dimension: "2d",
    format: INTERMEDIATE_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING,
    category,
  };
}

/** A view's or target's intermediates, at its current size. */
export class Intermediates {
  readonly #host: IntermediateHost;
  readonly #owner: string;
  readonly #category: MemoryCategory;
  #pair: {
    readonly size: ViewSize;
    readonly handles: readonly [TextureHandle, TextureHandle];
  } | null = null;

  constructor(host: IntermediateHost, owner: string, category: MemoryCategory) {
    this.#host = host;
    this.#owner = owner;
    this.#category = category;
  }

  /** The pair at `size`, made or re-made as needed. */
  at(size: ViewSize): readonly [GPUTexture, GPUTexture] {
    const pair = this.#pair;
    if (
      pair === null ||
      pair.size.widthPx !== size.widthPx ||
      pair.size.heightPx !== size.heightPx
    ) {
      this.release();
      const handles = [
        this.#host.createTexture(intermediateSpec(this.#owner, 0, size, this.#category)),
        this.#host.createTexture(intermediateSpec(this.#owner, 1, size, this.#category)),
      ] as const;
      this.#pair = { size, handles };
      return [this.#host.gpuTextureOf(handles[0]), this.#host.gpuTextureOf(handles[1])];
    }
    return [this.#host.gpuTextureOf(pair.handles[0]), this.#host.gpuTextureOf(pair.handles[1])];
  }

  /** Destroys the pair. */
  release(): void {
    if (this.#pair !== null) {
      for (const handle of this.#pair.handles) {
        this.#host.destroyTexture(handle);
      }
      this.#pair = null;
    }
  }
}
