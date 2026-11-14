/**
 * The smoke harness's checks of plan R14's instance path (R14.T5.d), test-render's part 29.
 *
 * @remarks
 * - **The particles' cull against its twin, over 10⁶ instances** ({@link checkInstanceCull}): the
 *   test scene (`clusters/instanceScene.ts`) places its field of 10⁶ particles on the device, in a
 *   60° view at 256 × 144 px, culls it by the instance cull and draws its buckets, the SwiftShader
 *   frame. Its instances, their count and its buckets are read back, and each bucket held between
 *   its twin's exact test and its twin at twice the kernel's rounding allowance, entry by entry as a
 *   set (`instanceCull.ts`): every instance the exact test keeps is in one bucket, by its twin's
 *   template but where an error falls within the `f32` rounding of its tolerance, and every entry is
 *   one the doubly widened test keeps, none twice. The frame's fingerprint, its covered share and
 *   the wall clock of its submission and read-back are in the details (SwiftShader's, relative).
 * - **A particle's near faces** ({@link checkParticleFaces}): one particle of 1 m 5 m ahead, drawn
 *   by the template its twin takes, its centre pixel's depth its near surface's, between that
 *   template's vertices' and faces' reach (`particles.ts`' `extents`), not its far side's: the
 *   templates wound as the engine culls.
 * - **Instances above the cluster cull, folded past 65,535** ({@link checkInstancesAboveClusters}):
 *   70,001 mesh instances, a one-level DAG of 8 clusters each, culled first by their bounds against
 *   a camera's frustum and a camera pass of no planes, so that every instance survives and the
 *   cluster cull's indirect dispatch folds them into two rows (`instanceFold.wgsl`). The survivors'
 *   count and fold, each survivor's passes and the zeroed tail are read back against the twin, and
 *   the cluster cull's buckets, each entry's instance its survivor's tag, held between the twin's
 *   exact and doubly widened tests as the cluster cull's own check holds its (`smoke/clusterCull.ts`).
 */

import type { Vec3 } from "../geometry/vec3";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { RenderEngine } from "../view/engine/types";
import { ClusterConsumerPath } from "../view/clusters/consumer";
import { CLUSTER_CULL_KERNEL, type CullInstance, cullTwin, passKeeps } from "../view/clusters/cull";
import type { CameraCullPass, CullPass } from "../view/clusters/cullPass";
import { foldedWorkgroups } from "../view/clusters/fold";
import {
  INSTANCE_COMPACT_KERNEL,
  INSTANCE_CULL_KERNEL,
  INSTANCE_FOLD_KERNEL,
  INSTANCE_ROUNDING_M,
  type InstanceBounds,
  instanceCompactTwin,
  instanceTemplateNear,
  instanceTemplateOf,
} from "../view/clusters/instanceCull";
import {
  cameraCullPassOf,
  InstanceScene,
  PARTICLE_TEST_MATERIAL,
  TEST_FIELD_DISTANCE_M,
  TEST_FIELD_INSTANCES,
  TEST_FIELD_RADIUS_M,
  TEST_FIELD_TOLERANCE_PX,
  viewAxisOf,
} from "../view/clusters/instanceScene";
import { ParticlePath } from "../view/clusters/particlePath";
import {
  PARTICLE_INSTANCE_BYTES,
  packParticleInstance,
  sphereParticleTemplates,
} from "../view/clusters/particles";
import {
  CLUSTER_LOD_BYTES,
  CLUSTER_RECORD_BYTES,
  type ClusterLod,
  type ClusterRecord,
  meshTemplateRef,
  packClusterLod,
  packClusterRecord,
} from "../view/clusters/record";
import { meshTemplate } from "../view/clusters/templates";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import { bytesDigest, type Checks, halfTexels, IDENTITY, NEAR_M } from "./harness";

/** The frame's size: 16:9, small enough for SwiftShader's raster of 10⁶ instances. */
const FRAME = { widthPx: 256, heightPx: 144 } as const;

/** A reversed-Z infinite perspective of a 60° horizontal field at `aspect`, column-major. */
function projectionOf(aspect: number): Float32Array {
  const fx = 1 / Math.tan(Math.PI / 6);
  return new Float32Array([fx, 0, 0, 0, 0, fx * aspect, 0, 0, 0, 0, 0, -1, 0, 0, NEAR_M, 0]);
}

/** A record's bounds read back: its centre and radius, words 0 to 3. */
function boundsOf(words: Float32Array, k: number, stride: number): InstanceBounds {
  return {
    centreM: {
      x: words[k * stride] ?? 0,
      y: words[k * stride + 1] ?? 0,
      z: words[k * stride + 2] ?? 0,
    },
    radiusM: words[k * stride + 3] ?? -1,
  };
}

/** The particles' cull against its twin over 10⁶ instances, and its frame (see the module's remarks). */
export async function checkInstanceCull(engine: RenderEngine, checks: Checks): Promise<void> {
  const target = engine.createRenderTarget({
    name: "R14 instances",
    size: FRAME,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const scene = new InstanceScene(engine, "R14 instances", TEST_FIELD_INSTANCES);
  try {
    const projection = projectionOf(FRAME.widthPx / FRAME.heightPx);
    const startMs = performance.now();
    const draws = scene.frame({ viewRotation: IDENTITY, projection, widthPx: FRAME.widthPx });
    target.render({
      label: "R14 instances",
      viewRotation: IDENTITY,
      projection,
      draws,
      postProcesses: [],
    });
    const path = scene.path;
    const [instanceBytes, countBytes, argBytes, listBytes, colourBytes] = await Promise.all([
      engine.readBuffer(path.instances, "tolerance"),
      engine.readBuffer(path.count, "tolerance"),
      engine.readBuffer(path.buckets.args, "tolerance"),
      engine.readBuffer(path.buckets.lists, "tolerance"),
      engine.readTexture(target.colour),
    ]);
    const frameMs = performance.now() - startMs;
    const words = new Float32Array(instanceBytes);
    const count = new Uint32Array(countBytes)[0] ?? 0;
    const stride = PARTICLE_INSTANCE_BYTES / 4;
    // The field as placed: every instance within its distances and radii, to their f32 rounding.
    let misplaced = 0;
    const instances: InstanceBounds[] = [];
    for (let k = 0; k < count; k += 1) {
      const bounds = boundsOf(words, k, stride);
      const d = Math.hypot(bounds.centreM.x, bounds.centreM.y, bounds.centreM.z);
      if (
        d < TEST_FIELD_DISTANCE_M[0] * (1 - 1e-5) ||
        d > TEST_FIELD_DISTANCE_M[1] * (1 + 1e-5) ||
        bounds.radiusM < TEST_FIELD_RADIUS_M[0] * (1 - 1e-5) ||
        bounds.radiusM > TEST_FIELD_RADIUS_M[1] * (1 + 1e-5)
      ) {
        misplaced += 1;
      }
      instances.push(bounds);
    }
    checks.check(
      "R14.T5.d the test field places its 10⁶ instances on the device, each within its distances and radii",
      count === TEST_FIELD_INSTANCES && misplaced === 0,
      `count ${String(count)} (want ${String(TEST_FIELD_INSTANCES)}); ${String(misplaced)} outside [${TEST_FIELD_DISTANCE_M.join(", ")}] m and radii [${TEST_FIELD_RADIUS_M.join(", ")}] m`,
    );
    const pass = cameraCullPassOf(IDENTITY, projection, FRAME.widthPx, TEST_FIELD_TOLERANCE_PX);
    const viewAxis = viewAxisOf(IDENTITY);
    const { errorFractions, boundsScale } = path.templates;
    const args = new Uint32Array(argBytes);
    const lists = new Uint32Array(listBytes);
    const place = path.buckets.place(0);
    // Each instance the device lists, and the template it took.
    const taken = new Int8Array(count).fill(-1);
    let twice = 0;
    let listed = 0;
    const perTemplate: number[] = [];
    errorFractions.forEach((_, t) => {
      const n = Math.min(
        args[(place.bucketBase + t) * 5 + 1] ?? 0,
        path.buckets.capacities[t] ?? 0,
      );
      perTemplate.push(n);
      const start = place.listBase + (path.buckets.templateOffsets[t] ?? 0);
      for (let a = 0; a < n; a += 1) {
        const k = lists[start + a] ?? 0;
        if (k >= count) {
          twice += 1;
        } else if ((taken[k] ?? -1) >= 0) {
          twice += 1;
        } else {
          taken[k] = t;
        }
        listed += 1;
      }
    });
    let refusedKept = 0;
    let keptRefused = 0;
    let wrongTemplate = 0;
    let exactKept = 0;
    for (let k = 0; k < count; k += 1) {
      const bounds = instances[k];
      if (bounds === undefined) {
        continue;
      }
      const t = taken[k] ?? -1;
      const exact = cameraKeepsAt(pass, viewAxis, bounds, boundsScale, 0);
      exactKept += exact ? 1 : 0;
      if (exact && t < 0) {
        refusedKept += 1;
      }
      if (t >= 0 && !cameraKeepsAt(pass, viewAxis, bounds, boundsScale, 2)) {
        keptRefused += 1;
      }
      if (
        t >= 0 &&
        t !==
          instanceTemplateOf(pass, errorFractions, bounds.centreM, bounds.radiusM, boundsScale) &&
        !instanceTemplateNear(pass, errorFractions, bounds.centreM, bounds.radiusM, boundsScale)
      ) {
        wrongTemplate += 1;
      }
    }
    checks.check(
      "R14.T5.d the instance cull's buckets over 10⁶ instances hold every instance the exact test keeps, by its twin's template, and none the doubly widened test refuses, each once",
      refusedKept === 0 &&
        keptRefused === 0 &&
        wrongTemplate === 0 &&
        twice === 0 &&
        exactKept > 0 &&
        exactKept < count,
      `listed ${String(listed)} of ${String(count)} (exact test keeps ${String(exactKept)}), by template ${perTemplate.join(" / ")}; refused but kept exactly ${String(refusedKept)}, kept past twice the allowance ${String(keptRefused)}, another template ${String(wrongTemplate)}, listed twice ${String(twice)}`,
    );
    const texels = halfTexels(colourBytes);
    let covered = 0;
    for (let i = 0; i < FRAME.widthPx * FRAME.heightPx; i += 1) {
      if ((texels[4 * i] ?? 0) > 0 || (texels[4 * i + 1] ?? 0) > 0) {
        covered += 1;
      }
    }
    checks.check(
      "R14.T5.d the instance path draws its 10⁶ test instances by bucket, the SwiftShader frame",
      // Its largest particle needs no finer template than the third at 256 px; the faces' check
      // draws one by it.
      covered > 0 && perTemplate.slice(0, 3).every((n) => n > 0),
      `frame ${bytesDigest(colourBytes)}, ${(covered / (FRAME.widthPx * FRAME.heightPx)).toFixed(3)} of its ${String(FRAME.widthPx)} × ${String(FRAME.heightPx)} px covered; placed, culled, drawn and read back in ${frameMs.toFixed(0)} ms (SwiftShader's wall clock, relative only)`,
    );
  } finally {
    scene.dispose();
    target.dispose();
  }
}

/**
 * The camera pass's keep of an instance, its bounds `boundsScale` times its radius, at `slack`
 * times the kernel's allowance.
 */
function cameraKeepsAt(
  pass: CullPass,
  viewAxis: Vec3,
  bounds: InstanceBounds,
  boundsScale: number,
  slack: number,
): boolean {
  return (
    bounds.radiusM >= 0 &&
    passKeeps(
      pass,
      viewAxis,
      bounds.centreM,
      bounds.radiusM * boundsScale,
      slack,
      INSTANCE_ROUNDING_M,
    )
  );
}

/** A particle's near faces drawn, its far faces culled (see the module's remarks). */
export async function checkParticleFaces(engine: RenderEngine, checks: Checks): Promise<void> {
  const side = 64;
  const target = engine.createRenderTarget({
    name: "R14 particle faces",
    size: { widthPx: side, heightPx: side },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const templates = sphereParticleTemplates();
  const path = new ParticlePath(engine, "R14 particle faces", templates, 1, 1);
  const mesh = engine.createMesh(templates.mesh);
  const material = engine.createMaterial(PARTICLE_TEST_MATERIAL);
  const kernel = engine.createCompute(INSTANCE_CULL_KERNEL);
  try {
    const data = new ArrayBuffer(PARTICLE_INSTANCE_BYTES);
    packParticleInstance(new DataView(data), 0, {
      centreM: { x: 0, y: 0, z: -5 },
      radiusM: 1,
      rotation: IDENTITY_QUATERNION,
      colour: 0xffff_ffff,
      tag: 0,
    });
    path.writeInstances(data, 1);
    const projection = projectionOf(1);
    const pass = cameraCullPassOf(IDENTITY, projection, side, TEST_FIELD_TOLERANCE_PX);
    path.cull(kernel, { passes: [pass], viewAxis: viewAxisOf(IDENTITY) });
    // The template the cull draws it by, the twin's: its error within τ at the bounds.
    const drawn = instanceTemplateOf(
      pass,
      templates.errorFractions,
      { x: 0, y: 0, z: -5 },
      1,
      templates.boundsScale,
    );
    target.render({
      label: "R14 particle faces",
      viewRotation: IDENTITY,
      projection,
      draws: path.drawItems(0, mesh, {
        material,
        offsetFromCameraM: new Float32Array(3),
        uniforms: { light: Float32Array.of(0, 0, 1, 0.25) },
        textures: {},
      }),
      postProcesses: [],
    });
    if (target.depth === null) {
      throw new Error(`${target.name} was made with depth and has none`);
    }
    const depth =
      new Float32Array(await engine.readTexture(target.depth))[(side / 2) * side + side / 2] ?? 0;
    const [inner, outer] = templates.extents[drawn] ?? [1, 1];
    // The pixel's ray, half a pixel off the axis each way, meets that template's near surface
    // between the spheres of its faces' and its vertices' reach, at a depth of
    // NEAR ÷ (t cos θ) for the near meeting t of a sphere of radius ρ 5 m out.
    const offAxis = Math.atan(Math.SQRT2 * (0.5 / side) * 2 * Math.tan(Math.PI / 6));
    const depthAt = (rho: number): number => {
      const t = 5 * Math.cos(offAxis) - Math.sqrt(rho * rho - 25 * Math.sin(offAxis) ** 2);
      return NEAR_M / (t * Math.cos(offAxis));
    };
    const nearest = depthAt(outer);
    const farthest = depthAt(inner);
    checks.check(
      "R14.T5.d a particle's template draws its near faces and culls its far ones: its centre's depth its near surface's",
      depth <= nearest * (1 + 1e-5) && depth >= farthest * (1 - 1e-5),
      `depth ${depth.toExponential(5)} (want ${farthest.toExponential(5)} to ${nearest.toExponential(5)}, its template ${String(drawn)}'s; the far side's ${(NEAR_M / 6).toExponential(5)})`,
    );
  } finally {
    path.dispose();
    engine.releaseMesh(mesh);
    engine.releaseMaterial(material);
    engine.releaseCompute(kernel);
    target.dispose();
  }
}

/**
 * The mesh instances of the fold's check: past 65,535, so that the survivors fold into two rows,
 * and odd, so that the fold leaves a place past them.
 */
const MESH_INSTANCES = 70_001;

/** Their clusters a page, a one-level DAG. */
const MESH_CLUSTERS = 8;

/** Words of a mesh instance: its origin, page, radius (word 4), tag (word 5) and two spare. */
const MESH_INSTANCE_WORDS = 8;

/** Mulberry32's next number in [0, 1). */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b_79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

/** Instances above the cluster cull, folded past 65,535 (see the module's remarks). */
export async function checkInstancesAboveClusters(
  engine: RenderEngine,
  checks: Checks,
): Promise<void> {
  const next = random(0x0f01_d5ed);
  // One page of 8 clusters about the mesh's origin, each 0.3 m across, a one-level DAG.
  const records: ClusterRecord[] = [];
  const lods: ClusterLod[] = [];
  const recordBytes = new ArrayBuffer(MESH_CLUSTERS * CLUSTER_RECORD_BYTES);
  const lodBytes = new ArrayBuffer(MESH_CLUSTERS * CLUSTER_LOD_BYTES);
  for (let c = 0; c < MESH_CLUSTERS; c += 1) {
    const centreM = {
      x: (c % 2) - 0.5,
      y: (Math.floor(c / 2) % 2) - 0.5,
      z: Math.floor(c / 4) - 0.5,
    };
    const record: ClusterRecord = {
      centreM,
      radiusM: 0.3,
      mode: "dag",
      errorsM: [0, Infinity],
      templateRef: meshTemplateRef(c * 16, 16),
      shape: "mesh",
      mayDiscard: false,
      material: 0,
    };
    records.push(record);
    lods.push({ parentCentreM: centreM, parentRadiusM: 0.3 });
    packClusterRecord(new DataView(recordBytes), c * CLUSTER_RECORD_BYTES, record);
    packClusterLod(new DataView(lodBytes), c * CLUSTER_LOD_BYTES, {
      parentCentreM: centreM,
      parentRadiusM: 0.3,
    });
  }
  const recordAt = (c: number): ClusterRecord => {
    const record = records[c];
    if (record === undefined) {
      throw new Error(`the instance fold's page has no cluster ${String(c)}`);
    }
    return record;
  };
  // A sphere about the origin holding every cluster: √0.75 + 0.3, padded.
  const boundingM = Math.sqrt(0.75) + 0.3 + 0.01;
  const instanceBytes = new ArrayBuffer(MESH_INSTANCES * MESH_INSTANCE_WORDS * 4);
  const view = new DataView(instanceBytes);
  const origins: Vec3[] = [];
  for (let k = 0; k < MESH_INSTANCES; k += 1) {
    // Ahead and behind, so that the frustum refuses some.
    const originM = { x: 200 * next() - 100, y: 200 * next() - 100, z: 300 * next() - 250 };
    origins.push(originM);
    const at = k * MESH_INSTANCE_WORDS * 4;
    view.setFloat32(at, originM.x, true);
    view.setFloat32(at + 4, originM.y, true);
    view.setFloat32(at + 8, originM.z, true);
    view.setUint32(at + 12, 0, true);
    view.setFloat32(at + 16, boundingM, true);
    view.setUint32(at + 20, k, true);
  }
  const buffer = (name: string, data: ArrayBuffer): ReturnType<RenderEngine["createBuffer"]> => {
    const made = engine.createBuffer({
      name,
      bytes: data.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "geometry-clusters",
    });
    engine.writeBuffer(made, 0, new Uint8Array(data));
    return made;
  };
  const recordBuffer = buffer("R14 instance fold records", recordBytes);
  const lodBuffer = buffer("R14 instance fold lods", lodBytes);
  const instanceBuffer = buffer("R14 instance fold instances", instanceBytes);
  const countBuffer = buffer(
    "R14 instance fold count",
    Uint32Array.of(MESH_INSTANCES, 0, 0, 0).buffer,
  );
  const consumer = new ClusterConsumerPath(engine, {
    name: "R14 instance fold",
    templates: [meshTemplate()],
    clustersPerPage: MESH_CLUSTERS,
    firstShape: 2,
    templateClusters: [MESH_CLUSTERS],
    instanceBytes: MESH_INSTANCE_WORDS * 4,
    vertexWgsl: "",
  });
  const kernels = {
    compact: engine.createCompute(INSTANCE_COMPACT_KERNEL),
    fold: engine.createCompute(INSTANCE_FOLD_KERNEL),
    cull: engine.createCompute(CLUSTER_CULL_KERNEL),
  };
  try {
    const camera = cameraCullPassOf(IDENTITY, projectionOf(1), 512, 1);
    const open: CameraCullPass = { ...camera, planes: [] };
    const passes: ReadonlyArray<CullPass> = [camera, open];
    const viewAxis = viewAxisOf(IDENTITY);
    const rotationColumns = [
      { x: 1, y: 0, z: 0 },
      { x: 0, y: 1, z: 0 },
      { x: 0, y: 0, z: 1 },
    ] as const;
    consumer.cullInstances(
      kernels,
      {
        records: recordBuffer,
        lods: lodBuffer,
        instances: instanceBuffer,
        count: countBuffer,
        capacity: MESH_INSTANCES,
        radiusWord: 4,
        passes,
        passMask: 0b11,
        rotationColumns,
        viewAxis,
      },
      "R14 instance fold",
    );
    const buckets = consumer.buckets;
    const compacted = consumer.compactedInstances;
    const survivors = consumer.survivors;
    const inputs = consumer.compactedInputs;
    if (buckets === null || compacted === null || survivors === null || inputs === null) {
      throw new Error("the instance fold's cull made no buckets or survivors");
    }
    const [survivorBytes, compactedBytes, inputBytes, argBytes, listBytes] = await Promise.all([
      engine.readBuffer(survivors, "tolerance"),
      engine.readBuffer(compacted, "tolerance"),
      engine.readBuffer(inputs, "tolerance"),
      engine.readBuffer(buckets.args, "tolerance"),
      engine.readBuffer(buckets.lists, "tolerance"),
    ]);
    const surviving = new Uint32Array(survivorBytes);
    const n = surviving[0] ?? 0;
    const fold = foldedWorkgroups(n);
    const tags = new Uint32Array(compactedBytes);
    const bounds = origins.map((originM): InstanceBounds => ({
      centreM: originM,
      radiusM: boundingM,
    }));
    const exactSurvivors = instanceCompactTwin(
      { instances: bounds, passes, viewAxis, passMask: 0b11 },
      0,
    );
    const boundSurvivors = instanceCompactTwin(
      { instances: bounds, passes, viewAxis, passMask: 0b11 },
      2,
    );
    const passWords = new Uint32Array(inputBytes);
    let wrongPasses = 0;
    for (let k = 0; k < n; k += 1) {
      const tag = tags[k * MESH_INSTANCE_WORDS + 5] ?? 0;
      const bits = passWords[2 * k] ?? 0;
      const least = exactSurvivors.get(tag) ?? 0;
      const most = boundSurvivors.get(tag) ?? 0;
      if ((bits & least) !== least || (bits & ~most) !== 0 || passWords[2 * k + 1] !== 0) {
        wrongPasses += 1;
      }
    }
    let tailSet = 0;
    for (let k = n; k < fold[0] * fold[1]; k += 1) {
      tailSet += (passWords[2 * k] ?? 0) !== 0 || (passWords[2 * k + 1] ?? 0) !== 0 ? 1 : 0;
    }
    checks.check(
      "R14.T5.d each survivor's cull input the passes its bounds' test keeps, and every place of the fold past the survivors none",
      wrongPasses === 0 && tailSet === 0 && fold[0] * fold[1] > n,
      `${String(wrongPasses)} survivors' passes outside their twins'; ${String(tailSet)} of the fold's ${String(fold[0] * fold[1] - n)} places past them not zeroed`,
    );
    checks.check(
      "R14.T5.d the instances above the cluster cull: every one a pass keeps survives, its count folded into two rows for the cluster cull, as the CPU folds it",
      n === exactSurvivors.size &&
        n > 65_535 &&
        surviving[1] === fold[0] &&
        surviving[2] === fold[1] &&
        surviving[3] === 1 &&
        fold[1] === 2,
      `${String(n)} survivors (twin ${String(exactSurvivors.size)}); fold ${surviving.slice(1, 4).join(" × ")} (want ${fold.join(" × ")})`,
    );
    // The twin's buckets: the cluster cull's twin over every instance, each entering the passes
    // its bounds' test keeps, at the exact test and at twice the allowance.
    const twinOf = (slack: number): Map<number, Set<number>> => {
      const kept = instanceCompactTwin(
        { instances: bounds, passes, viewAxis, passMask: 0b11 },
        slack,
      );
      const instances: CullInstance[] = origins.map((originM, k) => ({
        originM,
        page: 0,
        passMask: kept.get(k) ?? 0,
        unitM: 0,
      }));
      const twin = cullTwin(
        {
          instances,
          records: (_, c) => recordAt(c),
          lods: (_, c) => lods[c] ?? { parentCentreM: { x: 0, y: 0, z: 0 }, parentRadiusM: 0 },
          clustersPerPage: MESH_CLUSTERS,
          rotationColumns,
          passes,
          places: passes.map((_, p) => buckets.place(p)),
          templates: 1,
          viewAxis,
          firstShape: 2,
        },
        slack,
      );
      const out = new Map<number, Set<number>>();
      for (const [bucket, entries] of twin) {
        out.set(bucket, new Set(entries));
      }
      return out;
    };
    const exact = twinOf(0);
    const bound = twinOf(2);
    const args = new Uint32Array(argBytes);
    const lists = new Uint32Array(listBytes);
    let missing = 0;
    let extra = 0;
    let entries = 0;
    for (let p = 0; p < passes.length; p += 1) {
      const place = buckets.place(p);
      const count = Math.min(args[place.bucketBase * 5 + 1] ?? 0, buckets.capacities[0] ?? 0);
      const device = new Set<number>();
      for (let a = 0; a < count; a += 1) {
        const entry = lists[place.listBase + a] ?? 0;
        const k = entry >>> 8;
        const tag = tags[k * MESH_INSTANCE_WORDS + 5] ?? 0;
        device.add(((tag << 8) | (entry & 0xff)) >>> 0);
      }
      entries += device.size;
      for (const e of exact.get(place.bucketBase) ?? []) {
        missing += device.has(e) ? 0 : 1;
      }
      const widened = bound.get(place.bucketBase) ?? new Set<number>();
      for (const e of device) {
        extra += widened.has(e) ? 0 : 1;
      }
    }
    checks.check(
      "R14.T5.d the cluster cull over the survivors, folded past 65,535: every cluster the exact test keeps, none the doubly widened test refuses",
      missing === 0 && extra === 0 && entries > 0,
      `${String(entries)} entries over ${String(passes.length)} passes; missing ${String(missing)}, past twice the allowance ${String(extra)}`,
    );
  } finally {
    consumer.dispose();
    for (const each of [recordBuffer, lodBuffer, instanceBuffer, countBuffer]) {
      engine.releaseBuffer(each);
    }
    engine.releaseCompute(kernels.compact);
    engine.releaseCompute(kernels.fold);
    engine.releaseCompute(kernels.cull);
  }
}
