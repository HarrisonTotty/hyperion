/**
 * Meshes over Babylon's geometry, drawn through one Babylon mesh per use in a frame.
 *
 * @remarks
 * A `MeshHandle` is geometry: its vertex and index buffers are made once, in a Babylon `Geometry`.
 * A frame may draw it several times, with several materials and offsets, so each draw is given a
 * Babylon mesh of its own that shares the geometry, pooled by material: the n-th draw of a mesh
 * with a material in a frame reuses the n-th mesh of that pair, and keeps its effect and bindings
 * from frame to frame. Every such mesh has `alwaysSelectAsActiveMesh`, since culling is R02's and
 * R05's, not the engine's (R01 Design note 18). Per-instance attributes are instanced vertex
 * buffers on the shared geometry, and every draw goes through Babylon's instanced path with its
 * `forcedInstanceCount`, so that `@builtin(instance_index)` counts the draw's instances (Design
 * note 21).
 */

import { VertexBuffer } from "@babylonjs/core/Buffers/buffer.pure";
import { Constants } from "@babylonjs/core/Engines/constants";
import type { WebGPUEngine } from "@babylonjs/core/Engines/webgpuEngine.pure";
import type { Material } from "@babylonjs/core/Materials/material.pure";
import { Geometry } from "@babylonjs/core/Meshes/geometry";
import { Mesh } from "@babylonjs/core/Meshes/mesh.pure";
import type { Scene } from "@babylonjs/core/scene.pure";

import type { MeshSpec, VertexAttribute } from "../types";

/** Babylon's fill mode for each topology. */
const FILL_MODES: Readonly<Record<MeshSpec["topology"], number>> = {
  "triangle-list": Constants.MATERIAL_TriangleFillMode,
  "line-list": Constants.MATERIAL_LineListDrawMode,
  "point-list": Constants.MATERIAL_PointListDrawMode,
};

/** The Babylon fill mode that draws `topology`. */
export function fillModeOf(topology: MeshSpec["topology"]): number {
  return FILL_MODES[topology];
}

/** The name of Babylon's position attribute, which a source declares as `attribute position`. */
export const POSITION_ATTRIBUTE = VertexBuffer.PositionKind;

function vertexBuffer(
  engine: WebGPUEngine,
  kind: string,
  attribute: VertexAttribute,
  instanced: boolean,
): VertexBuffer {
  return new VertexBuffer(engine, attribute.data, kind, {
    updatable: false,
    stride: attribute.size,
    size: attribute.size,
    instanced,
    divisor: instanced ? 1 : 0,
  });
}

/** One mesh's geometry and the Babylon meshes that draw it. */
export class MeshRecord {
  readonly name: string;
  readonly fillMode: number;
  readonly #geometry: Geometry;
  readonly #unindexed: boolean;
  readonly #scene: Scene;
  readonly #pools = new Map<Material, Mesh[]>();

  constructor(engine: WebGPUEngine, scene: Scene, spec: MeshSpec) {
    if (spec.positions.length % 3 !== 0) {
      throw new Error(`mesh ${spec.name} has ${spec.positions.length} position components`);
    }
    const vertices = spec.positions.length / 3;
    this.name = spec.name;
    this.fillMode = fillModeOf(spec.topology);
    this.#scene = scene;
    this.#unindexed = spec.indices === null;
    const geometry = new Geometry(`${spec.name}:geometry`, scene);
    geometry.setVerticesBuffer(
      vertexBuffer(engine, POSITION_ATTRIBUTE, { data: spec.positions, size: 3 }, false),
      vertices,
    );
    for (const [kind, attribute] of Object.entries(spec.attributes)) {
      if (attribute.data.length !== vertices * attribute.size) {
        throw new Error(`mesh ${spec.name}'s ${kind} does not have one value per vertex`);
      }
      geometry.setVerticesBuffer(vertexBuffer(engine, kind, attribute, false), vertices);
    }
    for (const [kind, attribute] of Object.entries(spec.instanceAttributes ?? {})) {
      geometry.setVerticesBuffer(vertexBuffer(engine, kind, attribute, true), vertices);
    }
    if (spec.indices !== null) {
      geometry.setIndices(spec.indices, vertices);
    }
    this.#geometry = geometry;
  }

  /**
   * The Babylon mesh for the `use`-th draw of this mesh with `material` in a frame.
   *
   * @param use - Counted from 0 within the frame, per material.
   */
  meshFor(material: Material, use: number): Mesh {
    let pool = this.#pools.get(material);
    if (pool === undefined) {
      pool = [];
      this.#pools.set(material, pool);
    }
    let mesh = pool[use];
    if (mesh === undefined) {
      mesh = new Mesh(`${this.name}:${material.name}:${use}`, this.#scene);
      this.#geometry.applyToMesh(mesh);
      mesh.isUnIndexed = this.#unindexed;
      mesh.overrideRenderingFillMode = this.fillMode;
      mesh.alwaysSelectAsActiveMesh = true;
      mesh.doNotSyncBoundingInfo = true;
      mesh.material = material;
      pool.push(mesh);
    }
    return mesh;
  }

  /** Releases every Babylon mesh and the geometry. */
  dispose(): void {
    for (const pool of this.#pools.values()) {
      for (const mesh of pool) {
        mesh.dispose(true, false);
      }
    }
    this.#pools.clear();
    this.#geometry.dispose();
  }
}
