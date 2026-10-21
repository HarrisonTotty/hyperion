/**
 * The high setting's shadow cascades as the terrain session holds them (plan R10, R10.T9.b;
 * Design note 11; decision-r10-t9b-design D2 and D9): the one set of four layers for the engine,
 * the placeholder a lit draw binds where no set exists, and the registry of casters that are not
 * terrain.
 *
 * @remarks
 * - **The set** ({@link TerrainShadows.cascadeTarget}) is one `DepthLayerTarget` of
 *   {@link CASCADE_COUNT} {@link CASCADE_SIZE_PX}² `depth32float` layers under `shadows`
 *   ({@link CASCADE_BYTES}, 64 MiB), named {@link CASCADE_TEXTURE}'s, made at the first view-frame
 *   that draws cascades (its weight above 0 on the high setting) and kept until the owner lets it
 *   go. Every view draws its own cascades into it again, immediately before its own bodies pass,
 *   so that one set serves every view (decision-r10-t9-split §3). A low setting never makes it.
 * - **The placeholder** ({@link SHADOW_PLACEHOLDER_NAME}) is a 1 × 1 × 4-layer `depth32float`
 *   texture under `other`, which every lit terrain draw binds as `shadowCascades` while no set
 *   exists: the lit material declares the binding on both settings, and the receiver never samples
 *   it at w = 0.
 * - **A device loss.** Both died with the lost device. At the restore each is let go, as
 *   `TerrainResources` lets its handles go: the resilient engine drops the placeholder's release, and
 *   the lost engine disposed of the set's target with itself, so its disposal does nothing; and each
 *   is made again at its first need after it: the placeholder at the next lit draw, the set at the
 *   next frame that draws cascades.
 * - **The other casters** ({@link ShadowCasters}): craft and R11's rocks give each cascade's
 *   draws through a provider; nothing in the app fills it yet, and the cascades' `casterHeightM`
 *   stays 0 until one does (D9). The cascades are fitted to the top edge of star 0's disc
 *   (decision-r10-t9b-penumbra), so their shadow begins where the horizon map's share m reaches 0,
 *   and min(m, c′) is m across the penumbra; non-terrain casters' shadows, which the map does not
 *   see, are short by about half a penumbra, ρL (4.7 cm at 10 m).
 */

import { depthLayersTexture } from "../../engine/depthLayers";
import { TEXTURE_USAGE } from "../../engine/gpuFlags";
import { type MemoryCategory, textureBytes, type TextureSpec } from "../../engine/memory";
import type {
  DepthLayersSpec,
  DepthLayerTarget,
  DrawItem,
  RenderEngine,
  TextureHandle,
} from "../../engine/types";
import type { TerrainViewId } from "../terrainPass";
import type { CascadeIndex } from "./cascadeShadow";
import {
  CASCADE_COUNT,
  CASCADE_MEMORY_CATEGORY,
  CASCADE_SIZE_PX,
  CASCADE_TEXTURE,
  type CascadeSet,
} from "./cascades";

/** The name of the placeholder a lit draw binds where no set of cascades exists. */
export const SHADOW_PLACEHOLDER_NAME = "terrain shadow placeholder";

/**
 * The placeholder's texture: 1 × 1 × {@link CASCADE_COUNT} layers of `depth32float`, so that it
 * binds as the lit material's `texture_depth_2d_array`, sampled alone, under `other`; 16 B.
 */
export const SHADOW_PLACEHOLDER: TextureSpec = {
  name: SHADOW_PLACEHOLDER_NAME,
  size: { width: 1, height: 1, depthOrArrayLayers: CASCADE_COUNT },
  dimension: "2d",
  format: "depth32float",
  mips: 1,
  usage: TEXTURE_USAGE.TEXTURE_BINDING,
  category: "other",
};

/**
 * The set of cascades as the engine makes it: {@link CASCADE_TEXTURE}'s name, {@link CASCADE_SIZE_PX}
 * a side, {@link CASCADE_COUNT} layers, under {@link CASCADE_MEMORY_CATEGORY}.
 */
export const CASCADE_LAYERS: DepthLayersSpec = {
  name: CASCADE_TEXTURE.name,
  sizePx: CASCADE_SIZE_PX,
  layers: CASCADE_COUNT,
  category: CASCADE_MEMORY_CATEGORY,
};

/**
 * Makes a placeholder for a lit terrain draw that has no cascades to bind (R10.T9.b), for a caller
 * that draws the lit material by hand, as the smoke harness does; the caller releases it.
 */
export function createShadowPlaceholder(
  engine: Pick<RenderEngine, "createTexture">,
): TextureHandle {
  return engine.createTexture(SHADOW_PLACEHOLDER);
}

/**
 * A source of one cascade's draws of casters that are not terrain (decision-r10-t9b-design D9):
 * craft, R11's rocks. Its draws are camera-relative, as the eye's, through a material made for
 * `depth-only`; they read the cascade's own `Frame`, its light rotation and projection.
 *
 * @param set - The view's cascades this frame, with their light axes (z_L the top edge of star 0's
 *   disc, not its centre), centres and reach.
 * @param index - The cascade, nearest first.
 * @param view - The view whose cascades they are.
 */
export type ShadowCasterProvider = (
  set: CascadeSet,
  index: CascadeIndex,
  view: TerrainViewId,
) => ReadonlyArray<DrawItem>;

/** The registry of the casters that are not terrain, one a terrain session (D9). */
export class ShadowCasters {
  #providers: ReadonlyArray<ShadowCasterProvider> = [];

  /** How many providers it holds. */
  get size(): number {
    return this.#providers.length;
  }

  /**
   * Adds a provider, whose draws each cascade's pass draws after the terrain's, from the next frame
   * on.
   *
   * @returns Its removal, which takes effect at once.
   */
  add(provider: ShadowCasterProvider): () => void {
    this.#providers = [...this.#providers, provider];
    return () => {
      this.#providers = this.#providers.filter((each) => each !== provider);
    };
  }

  /** Every provider's draws of cascade `index` of `view`'s `set`, in the order they were added. */
  draws(set: CascadeSet, index: CascadeIndex, view: TerrainViewId): DrawItem[] {
    const providers = this.#providers;
    const draws: DrawItem[] = [];
    for (let n = 0; n < providers.length; n += 1) {
      const provider = providers[n];
      if (provider !== undefined) {
        draws.push(...provider(set, index, view));
      }
    }
    return draws;
  }
}

/** What the cascades make on the engine. */
export type TerrainShadowsEngine = Pick<
  RenderEngine,
  "createTexture" | "releaseTexture" | "createDepthLayers" | "onRestored"
>;

/** What makes a session's cascades. */
export interface TerrainShadowsOptions {
  /** Whether its views draw cascades: on the high setting alone (Design note 11). */
  readonly cascades: boolean;
}

/** One terrain session's shadow cascades, placeholder and other casters (see the module's remarks). */
export class TerrainShadows {
  readonly #engine: TerrainShadowsEngine;
  readonly #offRestored: () => void;
  /** Whether its views draw cascades, the high setting's. */
  readonly cascades: boolean;
  /** The casters that are not terrain (D9). */
  readonly casters = new ShadowCasters();
  #target: DepthLayerTarget | null = null;
  #placeholder: TextureHandle | null = null;
  #disposed = false;

  constructor(engine: TerrainShadowsEngine, options: TerrainShadowsOptions) {
    this.#engine = engine;
    this.cascades = options.cascades;
    this.#offRestored = engine.onRestored(() => {
      // Each is made again at its first need on the new device. A release that throws, a bug, is
      // reported, so that the restore goes on to tell the other listeners.
      try {
        this.#release();
      } catch (error: unknown) {
        console.error("the terrain's shadow cascades could not be let go at a restore:", error);
      }
    });
  }

  /** The set while it lives on the current device; `null` before its first need and after a loss. */
  get target(): DepthLayerTarget | null {
    return this.#target;
  }

  /**
   * The set, made at its first call on the current device (R10.T9.b; D2), or `null` where the
   * views draw no cascades (the low setting) or once let go.
   *
   * @throws EngineUnavailable while the device is lost.
   */
  cascadeTarget(): DepthLayerTarget | null {
    if (!this.cascades || this.#disposed) {
      return null;
    }
    this.#target ??= this.#engine.createDepthLayers(CASCADE_LAYERS);
    return this.#target;
  }

  /**
   * The texture a lit terrain draw binds as `shadowCascades`: the set's layers while it lives, and
   * otherwise the placeholder, made at its first need on the current device.
   *
   * @throws EngineUnavailable while the device is lost and the placeholder is to be made.
   * @throws Error once let go.
   */
  bound(): TextureHandle {
    if (this.#disposed) {
      throw new Error("the terrain's shadow cascades were let go");
    }
    const target = this.#target;
    if (target !== null) {
      return target.depth;
    }
    this.#placeholder ??= createShadowPlaceholder(this.#engine);
    return this.#placeholder;
  }

  /** The GPU bytes it holds, by category: the set under `shadows`, the placeholder under `other`. */
  bytesByCategory(): ReadonlyMap<MemoryCategory, number> {
    const bytes = new Map<MemoryCategory, number>();
    if (this.#target !== null) {
      bytes.set(CASCADE_MEMORY_CATEGORY, textureBytes(depthLayersTexture(CASCADE_LAYERS)));
    }
    if (this.#placeholder !== null) {
      const category = SHADOW_PLACEHOLDER.category;
      bytes.set(category, (bytes.get(category) ?? 0) + textureBytes(SHADOW_PLACEHOLDER));
    }
    return bytes;
  }

  /**
   * Releases the set and the placeholder and stops following restores; nothing more is made. Every
   * pass that binds them is let go first.
   */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#offRestored();
    this.#release();
  }

  /** Lets the set and the placeholder go, whatever the first's disposal does. */
  #release(): void {
    const target = this.#target;
    const placeholder = this.#placeholder;
    this.#target = null;
    this.#placeholder = null;
    try {
      target?.dispose();
    } finally {
      if (placeholder !== null) {
        this.#engine.releaseTexture(placeholder);
      }
    }
  }
}
