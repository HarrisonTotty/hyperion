/**
 * Bind groups kept by their layout and their resources (R01 Design note 23's draw path, and plan
 * R14's dispatches): the same layout over the same buffers, views and samplers takes the bind group
 * made for them the first time, rather than a new one a draw or a dispatch.
 *
 * @remarks
 * A bind group is immutable, and binds the very objects it was made over, so one made earlier
 * binds exactly what a new one of the same entries would. Its key is the layout's and each entry's
 * object's number, so that a new texture, buffer or view, never the same object as a destroyed
 * one, takes a bind group of its own. The cache is cleared whole whenever a resource it may hold is
 * destroyed, and once it passes {@link MAX_BIND_GROUPS}.
 */

/** The bind groups a cache holds before it is cleared and starts again. */
export const MAX_BIND_GROUPS = 4096;

/** One entry of a kept bind group: its binding, its resource, and the object it is known by. */
export interface CachedBindGroupEntry {
  readonly binding: number;
  readonly resource: GPUBindingResource;
  /** The view, sampler or buffer the resource binds, whose identity keys the bind group. */
  readonly key: object;
}

/** Bind groups by their layout and resources, made on `device` at their first use. */
export class BindGroupCache {
  readonly #device: Pick<GPUDevice, "createBindGroup">;
  /** Each object's number, for cache keys. */
  readonly #ids = new WeakMap<object, number>();
  #nextId = 1;
  readonly #bindGroups = new Map<string, GPUBindGroup>();

  constructor(device: Pick<GPUDevice, "createBindGroup">) {
    this.#device = device;
  }

  /** The bind group of `layout` over `entries`, made and labelled `label` at its first use. */
  bindGroup(
    layout: GPUBindGroupLayout,
    label: string,
    entries: ReadonlyArray<CachedBindGroupEntry>,
  ): GPUBindGroup {
    const key = [
      this.#id(layout),
      ...entries.map((entry) => `${entry.binding}:${this.#id(entry.key)}`),
    ].join(",");
    let bindGroup = this.#bindGroups.get(key);
    if (bindGroup === undefined) {
      if (this.#bindGroups.size > MAX_BIND_GROUPS) {
        this.#bindGroups.clear();
      }
      bindGroup = this.#device.createBindGroup({
        label,
        layout,
        entries: entries.map(({ binding, resource }) => ({ binding, resource })),
      });
      this.#bindGroups.set(key, bindGroup);
    }
    return bindGroup;
  }

  /** Forgets every bind group, after a texture or buffer they may hold was destroyed. */
  clear(): void {
    this.#bindGroups.clear();
  }

  #id(object: object): number {
    let id = this.#ids.get(object);
    if (id === undefined) {
      id = this.#nextId;
      this.#nextId += 1;
      this.#ids.set(object, id);
    }
    return id;
  }
}
