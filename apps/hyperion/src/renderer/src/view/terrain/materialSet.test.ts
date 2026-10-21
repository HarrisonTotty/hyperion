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
import type { MaterialHandle, MeshHandle, MeshSpec } from "../engine/types";
import { terrainMeshSpec } from "./gpu/resources";
import {
  COMPILE_MESH_NAME,
  sessionMaterials,
  TerrainMaterialSet,
  terrainMaterialOf,
} from "./materialSet";

/** Each compile asked of `engine` from the `from`-th, as `name targets meshes`. */
function compiles(engine: CountingRenderEngine, from = 0): string[] {
  return engine.compiles
    .slice(from)
    .map(({ name, targets, meshes }) => `${name} ${targets.join(",")} ${meshes.join(",")}`);
}

/** Lets the set's promise chains settle. */
async function settle(): Promise<void> {
  for (let hop = 0; hop < 10; hop += 1) {
    // Each hop waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await Promise.resolve();
  }
}

/**
 * The high setting's four, on its own path, the shadow cascades' caster among them (R10.T9.b):
 * what every compile below is held to.
 */
const HIGH = [
  `terrain-lit-baked-offsets rgba16float ${COMPILE_MESH_NAME}`,
  `terrain-depth-baked-offsets canvas ${COMPILE_MESH_NAME}`,
  `terrain-shadow-baked-offsets depth-only ${COMPILE_MESH_NAME}`,
  `terrain-wire-baked-offsets canvas ${COMPILE_MESH_NAME}`,
];

/**
 * A binding under the high lit cache's offsets (1,286 slots × 101,400 B) and its fallback's horizon
 * map (1,908 slots), and over the wireframe's offsets (520 slots).
 */
const NARROW: CountingLimits = { maxStorageBufferBindingSize: 1e8, maxBufferSize: 1e8 };

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

describe("sessionMaterials (R10.T14.d)", () => {
  it("gives the high setting's lit, depth, caster and wireframe materials on baked offsets", async () => {
    const engine = await countingRenderEngine();
    expect(sessionMaterials("high", engine.capabilities)).toEqual([
      { kind: "lit", path: "baked-offsets" },
      { kind: "depth", path: "baked-offsets" },
      { kind: "shadow", path: "baked-offsets" },
      { kind: "wire", path: "baked-offsets" },
    ]);
  });

  it("gives the low setting's on face differences, with no caster", async () => {
    const engine = await countingRenderEngine();
    expect(
      sessionMaterials("low", engine.capabilities).map(({ kind, path }) => `${kind} ${path}`),
    ).toEqual(["lit face-differences", "depth face-differences", "wire face-differences"]);
  });

  it("gives only the wireframe's where the device's binding holds the wireframe's layout alone", async () => {
    const engine = await countingRenderEngine(NARROW);
    expect(sessionMaterials("high", engine.capabilities)).toEqual([
      { kind: "wire", path: "baked-offsets" },
    ]);
  });

  it("gives none where the device holds no layout of the setting", async () => {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: 1e6,
      maxBufferSize: 1e6,
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
    expect([await set.compiled, compiles(engine)]).toEqual([4, HIGH]);
  });

  it("compiles each against a mesh of its pass's layout, released once compiled", async () => {
    const engine = new MeshRecordingEngine(await fakeDevice());
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    expect([engine.meshSpecs.map(layout), engine.live.meshes]).toEqual([
      [
        layout(terrainMeshSpec("lit")),
        layout(terrainMeshSpec("lit")),
        layout(terrainMeshSpec("lit")),
        layout(terrainMeshSpec("wire")),
      ],
      0,
    ]);
  });

  it("gives the one handle however often a material is asked, compiling it once", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    const first = await set.material("lit", "baked-offsets");
    const second = await set.material("lit", "baked-offsets");
    expect([second, compiles(engine).length]).toEqual([first, 4]);
  });

  it("compiles a material of another path at its first ask, once", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    await set.material("wire", "face-differences");
    await set.material("wire", "face-differences");
    expect(compiles(engine, 4)).toEqual([
      `terrain-wire-face-differences canvas ${COMPILE_MESH_NAME}`,
    ]);
  });

  it("forgets a compile the device refused, so that the next ask tries again", async () => {
    const engine = await countingRenderEngine();
    engine.failMaterials((name) => name === "terrain-depth-baked-offsets");
    const set = new TerrainMaterialSet(engine, "high");
    expect(await set.compiled).toBe(3);
    engine.failMaterials(() => false);
    const depth = await set.material("depth", "baked-offsets");
    expect([depth.name, compiles(engine).length]).toEqual(["terrain-depth-baked-offsets", 5]);
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
      HIGH,
      4,
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
    expect([after === before, compiles(engine, 4), engine.live.materials]).toEqual([
      false,
      HIGH,
      4,
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
    expect([after === before, engine.live.materials]).toEqual([false, 4]);
    set.dispose();
    expect(engine.live.materials).toBe(0);
  });

  it("refuses a compile a restore overtook, releasing what it made", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    // The counting engine resolves a compile a microtask on: the restore comes first.
    const overtaken = set.material("wire", "face-differences");
    engine.restore();
    await expect(overtaken).rejects.toBeInstanceOf(EngineUnavailable);
    await settle();
    // The setting's four compiled again on the restored device, and nothing else held.
    expect(engine.live.materials).toBe(4);
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
    expect([compiles(engine, 4), engine.live.materials]).toEqual([HIGH, 4]);
  });

  it("releases a compile that ends after it is let go", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    set.dispose();
    await settle();
    expect(engine.live.materials).toBe(0);
  });

  it("follows no restore once let go", async () => {
    const engine = await countingRenderEngine();
    const set = new TerrainMaterialSet(engine, "high");
    await set.compiled;
    set.dispose();
    engine.restore();
    await settle();
    expect(compiles(engine).length).toBe(4);
  });
});
