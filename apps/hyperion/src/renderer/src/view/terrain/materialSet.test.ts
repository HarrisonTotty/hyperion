/**
 * The terrain session's materials (plan R10, R10.T14.d; decision-r10-t14a §1): compiled as the set
 * is made on its setting's vertex paths, against a mesh of the patches' layout, each one handle
 * however often it is asked, compiled again after a restore, and released when the set is let go,
 * over R01's counting engine.
 */

import { describe, expect, it, vi } from "vitest";

import {
  type CountingLimits,
  CountingRenderEngine,
  countingRenderEngine,
  fakeDevice,
} from "../../test/countingRenderEngine";
import { EngineUnavailable } from "../engine/resilientEngine";
import { assertOutputsFit } from "../engine/webgpu/materials";
import { pipelineOutputs } from "../engine/webgpu/pipelineOutputs";
import type { MaterialHandle, MeshHandle, MeshSpec } from "../engine/types";
import { TERRAIN_MIXES_OFF, TERRAIN_MIXES_ON } from "./gpu/material";
import { terrainMeshSpec } from "./gpu/resources";
import {
  COMPILE_MESH_NAME,
  sessionMaterials,
  type TerrainMaterialKey,
  TERRAIN_COMPILES_IN_FLIGHT,
  type TerrainMaterialKind,
  TerrainMaterialSet,
  terrainMaterialOf,
} from "./materialSet";

/** Every kind of terrain material, each as a key, so that a kind left out fails to compile. */
const KINDS: Readonly<Record<TerrainMaterialKind, true>> = {
  lit: true,
  "lit-emissive": true,
  "lit-interior": true,
  "lit-emissive-interior": true,
  "lit-cluster": true,
  "lit-emissive-cluster": true,
  "lit-fin": true,
  "lit-emissive-fin": true,
  depth: true,
  "depth-cluster": true,
  "depth-fin": true,
  wire: true,
  "wire-cluster": true,
  shadow: true,
  "shadow-cluster": true,
  "shadow-fin": true,
};
/** The kinds of {@link KINDS}. */
const TERRAIN_MATERIAL_KINDS = Object.keys(KINDS).filter(
  (kind): kind is TerrainMaterialKind => kind in KINDS,
);

/** Each compile asked of `engine` from the `from`-th, as `name targets meshes`. */
function compiles(engine: CountingRenderEngine, from = 0): string[] {
  return engine.compiles
    .slice(from)
    .map(({ name, targets, meshes }) => `${name} ${targets.join(",")} ${meshes.join(",")}`);
}

/** Lets the set's promise chains settle, its compiles two at a time among them. */
async function settle(): Promise<void> {
  for (let hop = 0; hop < 200; hop += 1) {
    // Each hop waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await Promise.resolve();
  }
}

/**
 * The high setting's twelve, on its own path, in its order of first use: the shadow cascades' caster (R10.T9.b), the interior
 * patches' lit form (plan R14, R14.T1.a), their cluster caster (R14.T1.b), their cluster depth
 * (R14.T1.e), their lit form on the cluster path (R14.T2.a), the wireframe's interior patches' on
 * it (R14.T4.c) and the edge patches' fin forms (R14.T1.g) among them: what every compile below is
 * held to.
 */
const HIGH = [
  `terrain-lit-baked-offsets rgba16float ${COMPILE_MESH_NAME}`,
  `terrain-lit-cluster-baked-offsets rgba16float ${COMPILE_MESH_NAME}`,
  `terrain-lit-fin-baked-offsets rgba16float ${COMPILE_MESH_NAME}`,
  `terrain-cluster-shadow-baked-offsets depth-only ${COMPILE_MESH_NAME}`,
  `terrain-fin-shadow-baked-offsets depth-only ${COMPILE_MESH_NAME}`,
  `terrain-cluster-depth-baked-offsets canvas ${COMPILE_MESH_NAME}`,
  `terrain-fin-depth-baked-offsets canvas ${COMPILE_MESH_NAME}`,
  `terrain-lit-interior-baked-offsets rgba16float ${COMPILE_MESH_NAME}`,
  `terrain-shadow-baked-offsets depth-only ${COMPILE_MESH_NAME}`,
  `terrain-depth-baked-offsets canvas ${COMPILE_MESH_NAME}`,
  `terrain-wire-baked-offsets canvas ${COMPILE_MESH_NAME}`,
  `terrain-wire-cluster-baked-offsets canvas ${COMPILE_MESH_NAME}`,
];

/** {@link HIGH}'s still forms (plan R10, R10.T16.i; `stillMaterialSpec`), in the same order. */
const HIGH_STILL = HIGH.map((compile) => compile.replace(/^(\S+)/u, "$1-still"));

/** What the set compiles on the high setting: {@link HIGH}, then their still forms. */
const SETTING = [...HIGH, ...HIGH_STILL];

/** `keys`, then each one's still form, as {@link sessionMaterials} gives them (R10.T16.i). */
function withStills(keys: ReadonlyArray<TerrainMaterialKey>): TerrainMaterialKey[] {
  return [...keys, ...keys.map((key): TerrainMaterialKey => ({ ...key, still: true }))];
}

/** A still form's source with `TERRAIN_MIXES` declared true again. */
function back(source: string): string {
  return source.replace(TERRAIN_MIXES_OFF, TERRAIN_MIXES_ON);
}

/**
 * A binding under the high lit cache's offsets (1,208 slots × 101,400 B), and over its fallback's
 * horizon map at the same 1,208 slots (decision-r10-t14b-constants §4) and the wireframe's offsets
 * (520 slots).
 */
const NARROW: CountingLimits = { maxStorageBufferBindingSize: 1e8, maxBufferSize: 1e8 };

/**
 * A binding for four lit slots on `face-differences` (68,896 B of horizon map each), fewer than the
 * six roots', and eight of the wireframe's (33,800 B of heights each).
 */
const TINY: CountingLimits = { maxStorageBufferBindingSize: 3e5, maxBufferSize: 3e5 };

/** What a mesh's layout and size are made of, to compare two meshes by. */
function layout(spec: MeshSpec): unknown {
  return [spec.topology, spec.positions.length, spec.indices?.length ?? null, spec.attributes];
}

/** A counting engine whose release of the depth material throws, as a stale handle's would. */
class RefusingEngine extends CountingRenderEngine {
  override releaseMaterial(material: MaterialHandle): void {
    if (material.name === "terrain-depth-baked-offsets") {
      throw new Error(`material ${material.name} was released`);
    }
    super.releaseMaterial(material);
  }
}

/** A counting engine that records each mesh made, with its specification. */
class MeshRecordingEngine extends CountingRenderEngine {
  readonly meshSpecs: MeshSpec[] = [];

  override createMesh(spec: MeshSpec): MeshHandle {
    this.meshSpecs.push(spec);
    return super.createMesh(spec);
  }
}

/** A counting engine whose compiles end only when a test ends them, the oldest first. */
class DeferredEngine extends CountingRenderEngine {
  readonly #held: Array<() => void> = [];

  override createMaterialAsync(
    ...args: Parameters<CountingRenderEngine["createMaterialAsync"]>
  ): Promise<MaterialHandle> {
    const compiled = super.createMaterialAsync(...args);
    return new Promise((resolve, reject) => {
      this.#held.push(() => {
        compiled.then(resolve, reject);
      });
    });
  }

  /** The compiles started and not yet ended. */
  get running(): number {
    return this.#held.length;
  }

  /** Ends the oldest compile still running, and lets the set's chains settle. */
  async endOne(): Promise<void> {
    this.#held.shift()?.();
    await settle();
  }
}

describe("sessionMaterials (R10.T14.d)", () => {
  it("gives the high setting's lit, caster, depth and wireframe materials on baked offsets, in their order of first use", async () => {
    const engine = await countingRenderEngine();
    expect(sessionMaterials("high", engine.capabilities)).toEqual(
      withStills([
        { kind: "lit", path: "baked-offsets" },
        { kind: "lit-cluster", path: "baked-offsets" },
        { kind: "lit-fin", path: "baked-offsets" },
        { kind: "shadow-cluster", path: "baked-offsets" },
        { kind: "shadow-fin", path: "baked-offsets" },
        { kind: "depth-cluster", path: "baked-offsets" },
        { kind: "depth-fin", path: "baked-offsets" },
        { kind: "lit-interior", path: "baked-offsets" },
        { kind: "shadow", path: "baked-offsets" },
        { kind: "depth", path: "baked-offsets" },
        { kind: "wire", path: "baked-offsets" },
        { kind: "wire-cluster", path: "baked-offsets" },
      ]),
    );
  });

  it("gives the low setting's on face differences, with no caster", async () => {
    const engine = await countingRenderEngine();
    const low = [
      "lit face-differences",
      "lit-cluster face-differences",
      "lit-fin face-differences",
      "depth-cluster face-differences",
      "depth-fin face-differences",
      "lit-interior face-differences",
      "depth face-differences",
      "wire face-differences",
      "wire-cluster face-differences",
    ];
    expect(
      sessionMaterials("low", engine.capabilities).map(
        ({ kind, path, still }) => `${kind} ${path}${still === true ? " still" : ""}`,
      ),
    ).toEqual([...low, ...low.map((key) => `${key} still`)]);
  });

  it("gives the lit layout's on face differences where the device's binding holds the wireframe's offsets alone", async () => {
    const engine = await countingRenderEngine(NARROW);
    expect(sessionMaterials("high", engine.capabilities)).toEqual(
      withStills([
        { kind: "lit", path: "face-differences" },
        { kind: "lit-cluster", path: "face-differences" },
        { kind: "lit-fin", path: "face-differences" },
        { kind: "shadow-cluster", path: "face-differences" },
        { kind: "shadow-fin", path: "face-differences" },
        { kind: "depth-cluster", path: "face-differences" },
        { kind: "depth-fin", path: "face-differences" },
        { kind: "lit-interior", path: "face-differences" },
        { kind: "shadow", path: "face-differences" },
        { kind: "depth", path: "face-differences" },
        { kind: "wire", path: "baked-offsets" },
        { kind: "wire-cluster", path: "baked-offsets" },
      ]),
    );
  });

  it("gives only the wireframe's where the device's binding holds the wireframe's layout alone", async () => {
    const engine = await countingRenderEngine(TINY);
    expect(sessionMaterials("high", engine.capabilities)).toEqual(
      withStills([
        { kind: "wire", path: "face-differences" },
        { kind: "wire-cluster", path: "face-differences" },
      ]),
    );
  });

  it("gives none where the device holds no layout of the setting", async () => {
    // Under six of the wireframe's 33,800 B of heights.
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: 2e5,
      maxBufferSize: 2e5,
    });
    expect(sessionMaterials("high", engine.capabilities)).toEqual([]);
  });
});

describe("terrainMaterialOf (R10.T14.d)", () => {
  it("draws the lit material into the scene target, the caster into depth alone and the others into the canvas", () => {
    expect(
      (["lit", "depth", "wire", "shadow"] as const).map((kind) => {
        const { spec, targets, shading } = terrainMaterialOf(kind, "face-differences", {
          depthClipControl: false,
        });
        return [spec.name, targets, shading];
      }),
    ).toEqual([
      ["terrain-lit-face-differences", ["rgba16float"], "lit"],
      ["terrain-depth-face-differences", ["canvas"], "lit"],
      ["terrain-wire-face-differences", ["canvas"], "wire"],
      ["terrain-shadow-face-differences", ["depth-only"], "lit"],
    ]);
  });

  it("draws the interior patches' forms into the scene target over the lit mesh's layout (R14.T1.a)", () => {
    expect(
      (["lit-interior", "lit-emissive-interior"] as const).map((kind) => {
        const { spec, targets, shading } = terrainMaterialOf(kind, "baked-offsets", {
          depthClipControl: false,
        });
        return [spec.name, targets, shading];
      }),
    ).toEqual([
      ["terrain-lit-interior-baked-offsets", ["rgba16float"], "lit"],
      ["terrain-lit-emissive-interior-baked-offsets", ["rgba16float"], "lit"],
    ]);
  });

  it("writes into each of its targets an output the target's format takes (plan R14, R14.T2.b's finding)", () => {
    const refused: string[] = [];
    for (const kind of TERRAIN_MATERIAL_KINDS) {
      for (const path of ["face-differences", "baked-offsets"] as const) {
        const { spec, targets } = terrainMaterialOf(kind, path, { depthClipControl: true });
        try {
          assertOutputsFit(
            spec,
            targets
              .flatMap((target) => pipelineOutputs(target, "bgra8unorm"))
              .map(([format]) => format),
          );
        } catch (error: unknown) {
          refused.push(error instanceof Error ? error.message : String(error));
        }
      }
    }
    expect(refused).toEqual([]);
  });

  it("makes each kind's still form from its source with TERRAIN_MIXES declared false, and nothing else (R10.T16.i)", () => {
    const changed = TERRAIN_MATERIAL_KINDS.filter((kind) => {
      const made = terrainMaterialOf(kind, "baked-offsets", { depthClipControl: true });
      const still = terrainMaterialOf(kind, "baked-offsets", { depthClipControl: true }, true);
      return !(
        still.spec.name === `${made.spec.name}-still` &&
        still.spec.vertexWgsl.includes(TERRAIN_MIXES_OFF) &&
        back(still.spec.vertexWgsl) === made.spec.vertexWgsl &&
        back(still.spec.fragmentWgsl) === made.spec.fragmentWgsl &&
        JSON.stringify({ ...still.spec, name: "", vertexWgsl: "", fragmentWgsl: "" }) ===
          JSON.stringify({ ...made.spec, name: "", vertexWgsl: "", fragmentWgsl: "" }) &&
        JSON.stringify([still.targets, still.shading]) ===
          JSON.stringify([made.targets, made.shading])
      );
    });
    expect(changed).toEqual([]);
  });

  it("makes the caster unclipped exactly where the device offers depth-clip-control (R10.T9.b)", () => {
    expect(
      [true, false].map(
        (depthClipControl) =>
          terrainMaterialOf("shadow", "baked-offsets", { depthClipControl }).spec.unclippedDepth,
      ),
    ).toEqual([true, false]);
  });
});

describe("TerrainMaterialSet (R10.T14.d)", () => {
  it("compiles its setting's materials as it is made", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    expect([await set.compiled, compiles(engine)]).toEqual([SETTING.length, SETTING]);
  });

  it("compiles each against a mesh of its pass's layout, released once compiled", async () => {
    const engine = new MeshRecordingEngine(await fakeDevice());
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    // The setting's ten lit meshes and two wireframe meshes, then their still forms' likewise.
    const once = [
      ...Array.from({ length: 10 }, () => layout(terrainMeshSpec("lit"))),
      layout(terrainMeshSpec("wire")),
      layout(terrainMeshSpec("wire")),
    ];
    expect([engine.meshSpecs.map(layout), engine.live.meshes]).toEqual([[...once, ...once], 0]);
  });

  it("gives the one handle however often a material is asked, compiling it once", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    const first = await set.material("lit", "baked-offsets");
    const second = await set.material("lit", "baked-offsets");
    expect([second, compiles(engine).length]).toEqual([first, SETTING.length]);
  });

  it("compiles a material of another path at its first ask, once", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    await set.material("wire", "face-differences");
    await set.material("wire", "face-differences");
    expect(compiles(engine, SETTING.length)).toEqual([
      `terrain-wire-face-differences canvas ${COMPILE_MESH_NAME}`,
    ]);
  });

  it("forgets a compile the device refused, so that the next ask tries again", async () => {
    const engine = await countingRenderEngine();
    engine.failMaterials((name) => name === "terrain-depth-baked-offsets");
    const set = new TerrainMaterialSet(engine, "high");
    expect(await set.compiled).toBe(SETTING.length - 1);
    engine.failMaterials(() => false);
    const depth = await set.material("depth", "baked-offsets");
    expect([depth.name, compiles(engine).length]).toEqual([
      "terrain-depth-baked-offsets",
      SETTING.length + 1,
    ]);
  });

  it("compiles nothing while the device is lost, and its setting once the device is restored", async () => {
    const engine = await countingRenderEngine();
    engine.loseDevice();
    const set = new TerrainMaterialSet(engine, "high");
    expect(await set.compiled).toBe(0);
    engine.restore();
    await settle();
    const lit = await set.material("lit", "baked-offsets");
    expect([lit.name, compiles(engine), engine.live.materials]).toEqual([
      "terrain-lit-baked-offsets",
      SETTING,
      SETTING.length,
    ]);
  });

  it("compiles its setting again after a restore, with handles of the new device", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    const before = await set.material("lit", "baked-offsets");
    engine.loseDevice();
    engine.restore();
    await settle();
    const after = await set.material("lit", "baked-offsets");
    expect([after === before, compiles(engine, SETTING.length), engine.live.materials]).toEqual([
      false,
      SETTING,
      SETTING.length,
    ]);
  });

  it("gives a listener added after it, asking at the restore, the new device's handle", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    const before = await set.material("lit", "baked-offsets");
    // As a pass's resources rebuild it at the restore, after the set's own listener.
    const asked: Array<Promise<MaterialHandle>> = [];
    engine.onRestored(() => {
      asked.push(set.material("lit", "baked-offsets"));
    });
    engine.loseDevice();
    engine.restore();
    const after = await (asked[0] ?? Promise.reject(new Error("never asked")));
    await settle();
    expect([after === before, engine.live.materials]).toEqual([false, SETTING.length]);
    set.dispose();
    expect(engine.live.materials).toBe(0);
  });

  it("refuses a compile a restore overtook, releasing what it made", async () => {
    const engine = new DeferredEngine(await fakeDevice());
    const set = new TerrainMaterialSet(engine, "high");
    // The lit material is running as the restore comes, and ends after it.
    const overtaken = set.material("lit", "baked-offsets");
    engine.restore();
    while (engine.running > 0) {
      // oxlint-disable-next-line no-await-in-loop
      await engine.endOne();
    }
    await expect(overtaken).rejects.toBeInstanceOf(EngineUnavailable);
    // The setting's twelve and their still forms compiled again on the restored device, and
    // nothing else held.
    expect(engine.live.materials).toBe(SETTING.length);
  });

  it("releases every material when let go, and refuses asks after", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    set.dispose();
    expect(engine.live.materials).toBe(0);
    await expect(set.material("lit", "baked-offsets")).rejects.toThrow(/let go/u);
  });

  it("releases every other material when one release throws, and reports it", async () => {
    const engine = new RefusingEngine(await fakeDevice());
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    set.dispose();
    expect([engine.live.materials, errors.mock.calls.length]).toEqual([1, 1]);
  });

  it("compiles its setting again after a restore whose release of one throws", async () => {
    const engine = new RefusingEngine(await fakeDevice());
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    engine.loseDevice();
    engine.restore();
    await settle();
    expect([compiles(engine, SETTING.length), engine.live.materials]).toEqual([
      SETTING,
      SETTING.length,
    ]);
  });

  it("releases a compile that ends after it is let go", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    set.dispose();
    await settle();
    expect(engine.live.materials).toBe(0);
  });

  it("compiles two at a time, in the setting's order of first use (plan R14)", async () => {
    const engine = new DeferredEngine(await fakeDevice());
    const set = new TerrainMaterialSet(engine, "high");
    const running = [engine.running];
    expect(compiles(engine)).toEqual(HIGH.slice(0, TERRAIN_COMPILES_IN_FLIGHT));
    for (let ended = 0; ended < SETTING.length; ended += 1) {
      // Each compile ends before the next is started.
      // oxlint-disable-next-line no-await-in-loop
      await engine.endOne();
      running.push(engine.running);
    }
    expect([compiles(engine), running, await set.compiled]).toEqual([
      SETTING,
      [...Array.from({ length: SETTING.length - 1 }, () => 2), 1, 0],
      SETTING.length,
    ]);
  });

  it("compiles a pass's asks ahead of the setting's waiting materials, its first frame's first", async () => {
    const engine = new DeferredEngine(await fakeDevice());
    const set = new TerrainMaterialSet(engine, "high");
    // A pass's emissive form and the depth of another path, which the set holds neither of, in the
    // order asked; then the wireframe's material on that path, which a wireframe pass's first frame
    // waits for and the set does not hold, ahead of them; then the wireframe's on the setting's
    // path, which the setting's queue holds last, ahead of all.
    const asked = [
      set.material("lit-emissive", "baked-offsets"),
      set.material("depth", "face-differences"),
      set.material("wire", "face-differences"),
      set.material("wire", "baked-offsets"),
    ];
    for (let ended = 0; ended < 6; ended += 1) {
      // oxlint-disable-next-line no-await-in-loop
      await engine.endOne();
    }
    expect(compiles(engine).slice(0, 8)).toEqual([
      ...HIGH.slice(0, 2),
      `terrain-wire-baked-offsets canvas ${COMPILE_MESH_NAME}`,
      `terrain-wire-face-differences canvas ${COMPILE_MESH_NAME}`,
      `terrain-lit-emissive-baked-offsets rgba16float ${COMPILE_MESH_NAME}`,
      `terrain-depth-face-differences canvas ${COMPILE_MESH_NAME}`,
      ...HIGH.slice(2, 4),
    ]);
    expect((await Promise.all(asked)).map(({ name }) => name)).toEqual([
      "terrain-lit-emissive-baked-offsets",
      "terrain-depth-face-differences",
      "terrain-wire-face-differences",
      "terrain-wire-baked-offsets",
    ]);
  });

  it("refuses the compiles still waiting when let go, starting none of them", async () => {
    const engine = new DeferredEngine(await fakeDevice());
    const set = new TerrainMaterialSet(engine, "high");
    const waiting = set.material("depth", "baked-offsets");
    set.dispose();
    await expect(waiting).rejects.toThrow(/let go/u);
    await engine.endOne();
    await engine.endOne();
    expect([compiles(engine).length, await set.compiled, engine.live.materials]).toEqual([
      TERRAIN_COMPILES_IN_FLIGHT,
      0,
      0,
    ]);
  });

  it("refuses the compiles still waiting at a restore, and queues the setting again", async () => {
    const engine = new DeferredEngine(await fakeDevice());
    const set = new TerrainMaterialSet(engine, "high");
    const waiting = set.material("depth", "baked-offsets");
    engine.loseDevice();
    engine.restore();
    await expect(waiting).rejects.toBeInstanceOf(EngineUnavailable);
    while (engine.running > 0) {
      // oxlint-disable-next-line no-await-in-loop
      await engine.endOne();
    }
    // The lost device's two compiles, released as they end, then the setting's twelve and their
    // still forms again.
    expect([compiles(engine), engine.live.materials]).toEqual([
      [...HIGH.slice(0, 2), ...SETTING],
      SETTING.length,
    ]);
  });

  it("follows no restore once let go", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    set.dispose();
    engine.restore();
    await settle();
    expect(compiles(engine).length).toBe(SETTING.length);
  });

  it("hurries no still form a pass asks, the first frame's kinds' among them (R10.T16.i)", async () => {
    const engine = new DeferredEngine(await fakeDevice());
    const set = new TerrainMaterialSet(engine, "high");
    const asked = set.material("wire", "baked-offsets", true);
    for (let ended = 0; ended < 2; ended += 1) {
      // oxlint-disable-next-line no-await-in-loop
      await engine.endOne();
    }
    expect(compiles(engine)).toEqual(HIGH.slice(0, 4));
    while (engine.running > 0) {
      // oxlint-disable-next-line no-await-in-loop
      await engine.endOne();
    }
    expect((await asked).name).toBe("terrain-wire-baked-offsets-still");
  });
});
