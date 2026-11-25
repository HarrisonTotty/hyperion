import { afterEach, describe, expect, it, vi } from "vitest";

import { EngineUnavailable } from "../../engine/resilientEngine";
import type { ComputeHandle, MaterialHandle, MeshHandle } from "../../engine/types";
import { type ScatterKernelEngine, ScatterKernelSet } from "./scatterKernels";

/** One compile the fake engine was asked for, which the test settles. */
interface Pending {
  readonly name: string;
  readonly resolve: () => void;
  readonly reject: (reason: unknown) => void;
}

/** A fake engine whose compiles the test settles, counting what it made and released. */
class SettledEngine implements ScatterKernelEngine {
  readonly pending: Pending[] = [];
  readonly made: string[] = [];
  readonly released: string[] = [];
  readonly #restored = new Set<() => void>();

  #settle<T>(name: string, handle: T): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      this.pending.push({
        name,
        resolve: () => {
          this.made.push(name);
          resolve(handle);
        },
        reject,
      });
    });
  }

  createComputeAsync(pair: { readonly name: string }): Promise<ComputeHandle> {
    return this.#settle(pair.name, {
      kind: "compute",
      name: pair.name,
      path: "reference",
    } as const);
  }

  createMaterialAsync(spec: { readonly name: string }): Promise<MaterialHandle> {
    return this.#settle(spec.name, { kind: "material", name: spec.name } as const);
  }

  createMesh(spec: { readonly name: string }): MeshHandle {
    this.made.push(spec.name);
    return { kind: "mesh", name: spec.name };
  }

  releaseCompute(kernel: ComputeHandle): void {
    this.released.push(kernel.name);
  }

  releaseMaterial(material: MaterialHandle): void {
    this.released.push(material.name);
  }

  releaseMesh(mesh: MeshHandle): void {
    this.released.push(mesh.name);
  }

  onRestored(listener: () => void): () => void {
    this.#restored.add(listener);
    return () => {
      this.#restored.delete(listener);
    };
  }

  /** Tells the listeners of a restore, as `ResilientEngine` does. */
  restore(): void {
    for (const listener of this.#restored) {
      listener();
    }
  }

  /** Settles every compile asked so far, in order. */
  settleAll(): void {
    for (const p of this.pending.splice(0)) {
      p.resolve();
    }
  }
}

/** Lets the microtasks run that a settled compile schedules. */
async function settle(): Promise<void> {
  for (let n = 0; n < 5; n += 1) {
    // oxlint-disable-next-line no-await-in-loop -- each turn lets the next settle.
    await Promise.resolve();
  }
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("the decorative scatter's handles", () => {
  it("compiles once on the restored device where a restore comes before its first compile begins", async () => {
    const engine = new SettledEngine();
    const { promise: after, resolve: release } = withResolvers();
    const set = new ScatterKernelSet(engine, "baked-offsets", after);
    engine.restore();
    // The restore's own compile asks its three.
    expect(engine.pending.map((p) => p.name)).toEqual([
      "scatter pebbles baked-offsets",
      "instance cull",
      "scatter-pebbles-baked-offsets",
    ]);
    release();
    await settle();
    // The first device's, scheduled before the restore, makes nothing.
    expect(engine.pending).toHaveLength(3);
    engine.settleAll();
    expect(await set.ready).toBe(true);
    expect(set.kernels).not.toBeNull();
    set.dispose();
    expect(engine.released.toSorted()).toEqual(
      [
        "instance cull",
        "pebble templates",
        "scatter pebbles baked-offsets",
        "scatter-pebbles-baked-offsets",
      ].toSorted(),
    );
  });

  it("releases a compile that settles after it is let go", async () => {
    const engine = new SettledEngine();
    const set = new ScatterKernelSet(engine, "face-differences");
    await settle();
    expect(engine.pending).toHaveLength(3);
    set.dispose();
    engine.settleAll();
    expect(await set.ready).toBe(false);
    expect(set.kernels).toBeNull();
    expect(engine.released).toHaveLength(4);
  });

  it("keeps nothing of a compile that settles after a restore: the lost device's handles", async () => {
    const engine = new SettledEngine();
    const set = new ScatterKernelSet(engine, "face-differences");
    await settle();
    const first = engine.pending.splice(0);
    engine.restore();
    for (const p of first) {
      p.resolve();
    }
    await settle();
    // None of the lost device's is released, `ResilientEngine` dropping them; none is kept.
    expect(set.kernels).toBeNull();
    expect(engine.released).toEqual([]);
    engine.settleAll();
    expect(await set.ready).toBe(true);
    set.dispose();
  });

  it("releases what compiled where one compile fails, reports the failure and settles not ready", async () => {
    const engine = new SettledEngine();
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const set = new ScatterKernelSet(engine, "baked-offsets");
    await settle();
    const [place, cull, material] = engine.pending.splice(0);
    place?.resolve();
    cull?.resolve();
    material?.reject(new Error("refused"));
    expect(await set.ready).toBe(false);
    expect(set.kernels).toBeNull();
    expect(engine.released.toSorted()).toEqual(
      ["instance cull", "pebble templates", "scatter pebbles baked-offsets"].toSorted(),
    );
    expect(errors).toHaveBeenCalledTimes(1);
    set.dispose();
  });

  it("reports no failure of a lost device", async () => {
    const engine = new SettledEngine();
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const set = new ScatterKernelSet(engine, "baked-offsets");
    await settle();
    for (const p of engine.pending.splice(0)) {
      p.reject(new EngineUnavailable("createComputeAsync"));
    }
    expect(await set.ready).toBe(false);
    expect(errors).not.toHaveBeenCalled();
    set.dispose();
  });
});

/** A promise and its resolver. */
function withResolvers(): { readonly promise: Promise<void>; readonly resolve: () => void } {
  const resolvers: Array<() => void> = [];
  const promise = new Promise<void>((resolve) => {
    resolvers.push(resolve);
  });
  return {
    promise,
    resolve: () => {
      for (const resolve of resolvers) {
        resolve();
      }
    },
  };
}
