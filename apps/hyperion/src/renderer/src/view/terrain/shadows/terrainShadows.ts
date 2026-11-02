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
 * - **The layers' last owner** (plan R14, R14.T6.b): every view's passes clear and draw the layers
 *   they name and leave the rest, so a view that holds a far cascade from an earlier frame samples
 *   it only where it rendered that layer last ({@link TerrainShadows.holds}): the set records the
 *   owner of each layer's last render ({@link TerrainShadows.renderCasters}), and forgets them all
 *   when it is let go, at a restore among others.
 * - **Reuse** (plan R14, R14.T6.a; {@link CascadeReuse}): a view skips a cascade's pass where every
 *   input of its casters is as it was at its last render of the layer, word for word, and it
 *   rendered the layer last. A provider of other casters that gives no revision
 *   ({@link ShadowCasters.add}) re-renders every cascade every frame.
 */

import { depthLayersTexture } from "../../engine/depthLayers";
import { TEXTURE_USAGE } from "../../engine/gpuFlags";
import { type MemoryCategory, textureBytes, type TextureSpec } from "../../engine/memory";
import type {
  DepthLayerPass,
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

/**
 * A provider's revision (plan R14, R14.T6.a): a number that changes whenever any of its draws of
 * any cascade would, so that a cascade whose inputs are otherwise unchanged is reused.
 */
export type ShadowCasterRevision = () => number;

/** A provider and its revision, `null` where it gives none. */
interface ProviderEntry {
  readonly provider: ShadowCasterProvider;
  readonly revision: ShadowCasterRevision | null;
}

/** The registry of the casters that are not terrain, one a terrain session (D9). */
export class ShadowCasters {
  #providers: ReadonlyArray<ProviderEntry> = [];
  #version = 0;

  /** How many times a provider was added or removed (plan R14, R14.T6.a). */
  get version(): number {
    return this.#version;
  }

  /** How many providers it holds. */
  get size(): number {
    return this.#providers.length;
  }

  /**
   * Adds a provider, whose draws each cascade's pass draws after the terrain's, from the next frame
   * on.
   *
   * @param revision - Its revision (plan R14, R14.T6.a); absent, every cascade is rendered again
   *   every frame while it is held, so that a moving craft's shadow never freezes.
   * @returns Its removal, which takes effect at once.
   */
  add(provider: ShadowCasterProvider, revision?: ShadowCasterRevision): () => void {
    const entry: ProviderEntry = { provider, revision: revision ?? null };
    this.#providers = [...this.#providers, entry];
    this.#version += 1;
    return () => {
      const kept = this.#providers.filter((each) => each !== entry);
      if (kept.length !== this.#providers.length) {
        this.#providers = kept;
        this.#version += 1;
      }
    };
  }

  /**
   * Every provider's revision now, in the order they were added (plan R14, R14.T6.a), or `null`
   * where any gives none.
   */
  revisions(): number[] | null {
    const revisions: number[] = [];
    for (const { revision } of this.#providers) {
      if (revision === null) {
        return null;
      }
      revisions.push(revision());
    }
    return revisions;
  }

  /** Every provider's draws of cascade `index` of `view`'s `set`, in the order they were added. */
  draws(set: CascadeSet, index: CascadeIndex, view: TerrainViewId): DrawItem[] {
    const providers = this.#providers;
    const draws: DrawItem[] = [];
    for (let n = 0; n < providers.length; n += 1) {
      const entry = providers[n];
      if (entry !== undefined) {
        draws.push(...entry.provider(set, index, view));
      }
    }
    return draws;
  }
}

/**
 * One view's cascades' caster inputs as each was last rendered (plan R14, R14.T6.a): a frame
 * appends a cascade's inputs as 32-bit words, between {@link CascadeReuse.start} and
 * {@link CascadeReuse.unchanged}, and a cascade whose words equal its last render's, one for one,
 * is reused; {@link CascadeReuse.rendered} takes them as its last render's.
 */
export class CascadeReuse {
  readonly #last: Array<Uint32Array | null> = Array.from({ length: CASCADE_COUNT }, () => null);
  #words = new Uint32Array(1024);
  #length = 0;
  readonly #f64 = new Float64Array(1);
  readonly #f64Words = new Uint32Array(this.#f64.buffer);

  /** Starts a cascade's inputs. */
  start(): void {
    this.#length = 0;
  }

  /** Appends `words`, each a 32-bit word. */
  words(words: Uint32Array): void {
    this.#reserve(words.length);
    this.#words.set(words, this.#length);
    this.#length += words.length;
  }

  /** Appends `values`' `f32` bits. */
  f32(values: Float32Array): void {
    this.words(new Uint32Array(values.buffer, values.byteOffset, values.length));
  }

  /** Appends a byte buffer, a multiple of 4 bytes long, as its words. */
  bytes(bytes: Uint8Array): void {
    this.words(new Uint32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength / 4));
  }

  /** Appends `value`'s `f64` bits, two words. */
  f64(value: number): void {
    this.#f64[0] = value;
    this.word(this.#f64Words[0] ?? 0);
    this.word(this.#f64Words[1] ?? 0);
  }

  /** Appends one 32-bit word. */
  word(value: number): void {
    this.#reserve(1);
    this.#words[this.#length] = value;
    this.#length += 1;
  }

  /** Whether the words appended since {@link CascadeReuse.start} equal cascade `index`'s last render's. */
  unchanged(index: CascadeIndex): boolean {
    const last = this.#last[index] ?? null;
    if (last === null || last.length !== this.#length) {
      return false;
    }
    const words = this.#words;
    for (let n = 0; n < last.length; n += 1) {
      if (last[n] !== words[n]) {
        return false;
      }
    }
    return true;
  }

  /** Takes the words appended since {@link CascadeReuse.start} as cascade `index`'s last render's. */
  rendered(index: CascadeIndex): void {
    this.#last[index] = this.#words.slice(0, this.#length);
  }

  /** Forgets every cascade's last render: the next frame renders each. */
  forget(): void {
    this.#last.fill(null);
  }

  #reserve(more: number): void {
    if (this.#length + more > this.#words.length) {
      const grown = new Uint32Array(Math.max(2 * this.#words.length, this.#length + more));
      grown.set(this.#words.subarray(0, this.#length));
      this.#words = grown;
    }
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
  /** Each layer's last render's owner, `null` before any on the current set (R14.T6.b). */
  readonly #owners: Array<object | null> = Array.from({ length: CASCADE_COUNT }, () => null);

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

  /**
   * Renders a view's caster passes into the set ({@link DepthLayerTarget.render}, under `name`),
   * and records `owner` as the last to render each layer they name (plan R14, R14.T6.b); nothing
   * where the set does not live.
   *
   * @param owner - What holds the view's cascades between frames, compared by identity.
   * @throws as {@link DepthLayerTarget.render} does, before any layer's owner is recorded.
   */
  renderCasters(owner: object, name: string, passes: ReadonlyArray<DepthLayerPass>): void {
    const target = this.#target;
    if (target === null) {
      return;
    }
    target.render(name, passes);
    for (const pass of passes) {
      this.#owners[pass.layer] = owner;
    }
  }

  /**
   * Whether layer `layer` of the set still holds what `owner` last rendered into it through
   * {@link TerrainShadows.renderCasters} (plan R14, R14.T6.b): `owner` rendered it last, and the
   * set has not been let go since.
   */
  holds(owner: object, layer: CascadeIndex): boolean {
    return this.#target !== null && this.#owners[layer] === owner;
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
    this.#owners.fill(null);
    try {
      target?.dispose();
    } finally {
      if (placeholder !== null) {
        this.#engine.releaseTexture(placeholder);
      }
    }
  }
}
