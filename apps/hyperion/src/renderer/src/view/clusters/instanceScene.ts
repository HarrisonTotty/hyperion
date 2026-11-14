/**
 * The instance path's test scene (plan R14, R14.T5.d): a field of particles placed on the GPU in a
 * view's frustum, culled by the instance cull and drawn by bucket, as R11's scatter will be, with no
 * consumer of R11's yet. The smoke harness's part 29 checks it against its twin and draws its
 * SwiftShader frame; the views check's `--instance-probe` adds it to `VIEW`'s photorealistic frame
 * (the owner's Mac pair `instances`).
 *
 * @remarks
 * - **The field** (`instanceTestPlace.wgsl`): each frame `count` instances, a direction uniform in
 *   the view's rectangle of tangents widened by {@link TEST_FIELD_WIDENING}, so that some lie outside
 *   the frustum and the cull refuses them, a distance log-uniform over
 *   {@link TEST_FIELD_DISTANCE_M}, a radius log-uniform over {@link TEST_FIELD_RADIUS_M} (R11's
 *   decorative scatter lies below 0.4 m across, R11 Design note 2), a uniform rotation and a rock's
 *   grey. Placed in the view's own axes each frame, so that every frame of a fixed view draws the
 *   same field.
 * - **Its passes**: one camera pass, the submission's frustum ({@link cameraCullPassOf}) at τ of
 *   {@link TEST_FIELD_TOLERANCE_PX}, the high setting's terrain τ, with no horizon.
 * - **Its picture**: `particleTest.wgsl`, lit from a fixed direction with an ambient share, into the
 *   frame's target with depth; check-only, not the picture a player sees.
 */

import type { Vec3 } from "../../geometry/vec3";
import type { KernelPair } from "../engine/kernels";
import type {
  ComputeHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  WgslMaterialSpec,
} from "../engine/types";
import frameWgsl from "../shaders/frame.wgsl?raw";
import type { BucketDrawBase } from "./buckets";
import type { CameraCullPass, CullPlane } from "./cullPass";
import { FOLDED_GROUP_WGSL } from "./fold";
import { INSTANCE_CULL_KERNEL, instanceWorkgroups } from "./instanceCull";
import instanceTestPlaceWgsl from "./instanceTestPlace.wgsl?raw";
import { ParticlePath } from "./particlePath";
import {
  PARTICLE_INSTANCE_WGSL,
  PARTICLE_STORAGE_BUFFERS,
  sphereParticleTemplates,
} from "./particles";
import particleTestWgsl from "./particleTest.wgsl?raw";

/** The test field's placement: `presentation-only`, its `f32` arithmetic the device's. */
export const INSTANCE_TEST_PLACE_KERNEL: KernelPair = {
  name: "instance test place",
  reference: FOLDED_GROUP_WGSL + instanceTestPlaceWgsl,
  subgroup: null,
  readback: "presentation-only",
};

/** The test particles' `Draw`, after `offsetFromCameraM`: the light and the bucket's base. */
const PARTICLE_TEST_DRAW = `
struct Draw {
  offsetFromCameraM : vec3f,
  // The light's direction towards it, a unit vector in the passes' frame, and the ambient share.
  light : vec4f,
  // The bucket's first entry in the lists (\`clusters/buckets.ts\`' \`clusterBase\`).
  clusterBase : u32,
}

@group(1) @binding(0) var<uniform> draw : Draw;
`;

/** The test particles' material (see the module's remarks): opaque, depth written, back faces culled. */
export const PARTICLE_TEST_MATERIAL: WgslMaterialSpec = {
  name: "clusters:particleTest",
  displayName: "TEST PARTICLES",
  vertexWgsl: frameWgsl + PARTICLE_TEST_DRAW + PARTICLE_INSTANCE_WGSL + particleTestWgsl,
  fragmentWgsl: frameWgsl + PARTICLE_TEST_DRAW + PARTICLE_INSTANCE_WGSL + particleTestWgsl,
  uniforms: [
    { name: "light", type: "vec4f" },
    { name: "clusterBase", type: "u32" },
  ],
  samplers: [],
  cullMode: "back",
  depthWrite: true,
  colourWrites: true,
  blend: "none",
  storageBuffers: PARTICLE_STORAGE_BUFFERS,
};

/** The test field's instances: the acceptance's 10⁶. */
export const TEST_FIELD_INSTANCES = 1_000_000;

/** The field's widening of the view's tangents, so that about a third of it lies outside the view. */
export const TEST_FIELD_WIDENING = 1.25;

/** The field's distances, least and most, m. */
export const TEST_FIELD_DISTANCE_M = [2, 200] as const;

/** The field's radii, least and most, m: 1 cm to 0.2 m, R11's scatter below 0.4 m across. */
export const TEST_FIELD_RADIUS_M = [0.01, 0.2] as const;

/** The test pass's τ, px: the high setting's terrain τ (plan R05). */
export const TEST_FIELD_TOLERANCE_PX = 1;

/** The field's seed. */
export const TEST_FIELD_SEED = 0x5eed_0d05;

/** The test light: towards it, a unit vector, and an ambient share of 0.25. */
const TEST_LIGHT = (() => {
  const l = { x: 0.4, y: 0.8, z: 0.45 };
  const n = Math.hypot(l.x, l.y, l.z);
  return Float32Array.of(l.x / n, l.y / n, l.z / n, 0.25);
})();

/** Bytes of `instanceTestPlace.wgsl`'s `PlaceParams`. */
export const TEST_PLACE_PARAMS_BYTES = 80;

/** A 4 × 4 column-major matrix's row `i`, its four elements. */
function row(m: Float32Array, i: number): readonly [number, number, number, number] {
  return [m[i] ?? 0, m[4 + i] ?? 0, m[8 + i] ?? 0, m[12 + i] ?? 0];
}

/** The view's axes in the passes' frame from a submission's `viewRotation`: its first three rows. */
export function viewAxesOf(viewRotation: Float32Array): readonly [Vec3, Vec3, Vec3] {
  const axis = (i: number): Vec3 => {
    const [x, y, z] = row(viewRotation, i);
    return { x, y, z };
  };
  return [axis(0), axis(1), axis(2)];
}

/**
 * A camera pass of a submission's frustum (see the module's remarks): the planes of its clip
 * volume, `projection × viewRotation`, about the camera (Gribb and Hartmann 2001, "Fast Extraction
 * of Viewing Frustum Planes from the World-View-Projection Matrix"): its four sides, w ± x and
 * w ± y, and its near plane, w − z under reversed Z, each normalised; no far plane for an infinite
 * projection, whose z row holds no direction; τ of `tolerancePx` over its px per radian,
 * `widthPx × P₀₀ ÷ 2`; no horizon, stride 1, appended.
 */
export function cameraCullPassOf(
  viewRotation: Float32Array,
  projection: Float32Array,
  widthPx: number,
  tolerancePx: number,
): CameraCullPass {
  const clip = new Float64Array(16);
  for (let c = 0; c < 4; c += 1) {
    for (let r = 0; r < 4; r += 1) {
      let sum = 0;
      for (let k = 0; k < 4; k += 1) {
        sum += (projection[4 * k + r] ?? 0) * (viewRotation[4 * c + k] ?? 0);
      }
      clip[4 * c + r] = sum;
    }
  }
  const at = (r: number): readonly [number, number, number, number] => [
    clip[r] ?? 0,
    clip[4 + r] ?? 0,
    clip[8 + r] ?? 0,
    clip[12 + r] ?? 0,
  ];
  const w = at(3);
  const combos = [
    [at(0), 1],
    [at(0), -1],
    [at(1), 1],
    [at(1), -1],
    [at(2), -1],
  ] as const;
  const planes: CullPlane[] = [];
  for (const [r, sign] of combos) {
    const n = { x: w[0] + sign * r[0], y: w[1] + sign * r[1], z: w[2] + sign * r[2] };
    const length = Math.hypot(n.x, n.y, n.z);
    if (length > 1e-12) {
      planes.push({
        normal: { x: n.x / length, y: n.y / length, z: n.z / length },
        offsetM: (w[3] + sign * r[3]) / length,
      });
    }
  }
  const pxPerRad = (widthPx * (projection[0] ?? 0)) / 2;
  return {
    kind: "camera",
    planes,
    horizon: null,
    order: "appended",
    toleranceRad: tolerancePx / pxPerRad,
    strideCap: 1,
  };
}

/** The view axis, the view's −z, in the passes' frame. */
export function viewAxisOf(viewRotation: Float32Array): Vec3 {
  const [, , z] = viewAxesOf(viewRotation);
  return { x: -z.x, y: -z.y, z: -z.z };
}

/**
 * Packs the field's placement for a view (see the module's remarks) into `out`
 * ({@link TEST_PLACE_PARAMS_BYTES} at least): its axes, its tangents widened, the ranges, the
 * count, the capacity and the seed.
 */
export function packTestPlaceParams(
  viewRotation: Float32Array,
  projection: Float32Array,
  count: number,
  capacity: number,
  out: ArrayBuffer,
): void {
  const view = new DataView(out);
  const axes = viewAxesOf(viewRotation);
  // P₀₀ and P₁₁ are 1 ÷ tan of the half-angles.
  const tangents = [
    TEST_FIELD_WIDENING / (projection[0] ?? 1),
    TEST_FIELD_WIDENING / (projection[5] ?? 1),
    0,
  ];
  axes.forEach((axis, k) => {
    view.setFloat32(16 * k, axis.x, true);
    view.setFloat32(16 * k + 4, axis.y, true);
    view.setFloat32(16 * k + 8, axis.z, true);
    view.setFloat32(16 * k + 12, tangents[k] ?? 0, true);
  });
  view.setFloat32(48, TEST_FIELD_DISTANCE_M[0], true);
  view.setFloat32(52, TEST_FIELD_DISTANCE_M[1], true);
  view.setFloat32(56, TEST_FIELD_RADIUS_M[0], true);
  view.setFloat32(60, TEST_FIELD_RADIUS_M[1], true);
  view.setUint32(64, count, true);
  view.setUint32(68, capacity, true);
  view.setUint32(72, TEST_FIELD_SEED, true);
  view.setUint32(76, 0, true);
}

/** The part of the engine the test scene uses. */
export type InstanceSceneEngine = Pick<
  RenderEngine,
  | "createBuffer"
  | "writeBuffer"
  | "releaseBuffer"
  | "dispatch"
  | "createCompute"
  | "releaseCompute"
  | "createMaterial"
  | "releaseMaterial"
  | "createMesh"
  | "releaseMesh"
>;

/** One frame of the test scene: its view's matrices and its target's width. */
export interface InstanceSceneFrame {
  readonly viewRotation: Float32Array;
  readonly projection: Float32Array;
  readonly widthPx: number;
}

/** The test scene (see the module's remarks): its path, kernels, material and mesh, made at once. */
export class InstanceScene {
  readonly path: ParticlePath;
  readonly count: number;
  readonly #engine: InstanceSceneEngine;
  readonly #place: ComputeHandle;
  readonly #cull: ComputeHandle;
  readonly #material: MaterialHandle;
  readonly #mesh: MeshHandle;
  readonly #params = new ArrayBuffer(TEST_PLACE_PARAMS_BYTES);
  readonly #base: BucketDrawBase;
  #disposed = false;

  /** @throws RangeError for a count that is not a whole number from 1. */
  constructor(engine: InstanceSceneEngine, name: string, count: number = TEST_FIELD_INSTANCES) {
    this.#engine = engine;
    this.count = count;
    const templates = sphereParticleTemplates();
    this.path = new ParticlePath(engine, name, templates, count, 1);
    this.#place = engine.createCompute(INSTANCE_TEST_PLACE_KERNEL);
    this.#cull = engine.createCompute(INSTANCE_CULL_KERNEL);
    this.#material = engine.createMaterial(PARTICLE_TEST_MATERIAL);
    this.#mesh = engine.createMesh(templates.mesh);
    this.#base = {
      material: this.#material,
      offsetFromCameraM: new Float32Array(3),
      uniforms: { light: TEST_LIGHT },
      textures: {},
    };
  }

  /**
   * Places the field for `frame`'s view and culls it, two dispatches whose submissions precede
   * every draw encoded after the call; the pass's bucket draws, which the caller draws into its
   * view's target with its matrices.
   */
  frame(frame: InstanceSceneFrame): ReadonlyArray<DrawItem> {
    if (this.#disposed) {
      return [];
    }
    const path = this.path;
    packTestPlaceParams(
      frame.viewRotation,
      frame.projection,
      this.count,
      path.capacity,
      this.#params,
    );
    this.#engine.dispatch(
      this.#place,
      {
        uniforms: { params: new Uint32Array(this.#params) },
        buffers: { instances: path.instances, instance_count: path.count },
        sampled: {},
        storage: {},
      },
      instanceWorkgroups(this.count),
      INSTANCE_TEST_PLACE_KERNEL.name,
    );
    path.cull(this.#cull, {
      passes: [
        cameraCullPassOf(
          frame.viewRotation,
          frame.projection,
          frame.widthPx,
          TEST_FIELD_TOLERANCE_PX,
        ),
      ],
      viewAxis: viewAxisOf(frame.viewRotation),
    });
    return path.drawItems(0, this.#mesh, this.#base);
  }

  /** Releases everything it made. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.path.dispose();
    this.#engine.releaseCompute(this.#place);
    this.#engine.releaseCompute(this.#cull);
    this.#engine.releaseMaterial(this.#material);
    this.#engine.releaseMesh(this.#mesh);
  }
}
