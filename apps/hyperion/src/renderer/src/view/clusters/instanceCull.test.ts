/**
 * The instance cull (plan R14, R14.T5.d): its parameters packed as `instanceCull.wgsl`,
 * `instanceCompact.wgsl` and `instanceFold.wgsl` lay them out, its kernels composed of the cluster
 * cull's pass tests, and its twin, which runs the kernel's fold, workgroups and appends, against a
 * brute-force oracle over 10⁶ instances: each instance tested against each pass on its own, by the
 * predicates written out again here, its template by its projected size.
 */

import { beforeAll, describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import { CountingRenderEngine, fakeDevice } from "../../test/countingRenderEngine";
import { seededRandom } from "../../test/seededRandom";
import { declaredStructLayout } from "../engine/webgpu/uniforms";
import { frustumOf } from "../terrain/cull";
import { quaternionFromAxisAngle } from "../camera/quaternion";
import { ClusterBuckets } from "./buckets";
import {
  CLUSTER_CULL_KERNEL,
  CLUSTER_CULL_OCCLUSION_KERNEL,
  CULL_PASS_TESTS_WGSL,
  CULL_ROUNDING_FRACTION,
  CULL_ROUNDING_M,
  HORIZON_ROUNDING,
  RECEIVER_ROUNDING_M,
} from "./cull";
import {
  type CameraCullPass,
  type CascadeCullPass,
  type CullPass,
  cullHorizonOf,
  MAX_CULL_PASSES,
} from "./cullPass";
import { FOLDED_GROUP_WGSL } from "./fold";
import instanceCompactWgsl from "./instanceCompact.wgsl?raw";
import {
  INSTANCE_COMPACT_KERNEL,
  INSTANCE_COMPACT_PARAMS_BYTES,
  INSTANCE_CULL_KERNEL,
  INSTANCE_CULL_PARAMS_BYTES,
  INSTANCE_CULL_WORKGROUP,
  INSTANCE_FOLD_PARAMS_BYTES,
  INSTANCE_ROUNDING_M,
  type InstanceBounds,
  instanceBucketEntries,
  instanceCompactTwin,
  instanceCullTwin,
  type InstanceCullTwinInput,
  instanceTemplateNear,
  instanceTemplateOf,
  instanceWorkgroups,
  packInstanceCompactParams,
  packInstanceCullParams,
} from "./instanceCull";
import instanceCullWgsl from "./instanceCull.wgsl?raw";
import instanceFoldWgsl from "./instanceFold.wgsl?raw";
import { MAX_PARTICLE_TEMPLATES, sphereParticleTemplates } from "./particles";

/** A struct's members as (name, offset), and its size, of `source`. */
function layoutOf(
  source: string,
  name: string,
): readonly [Array<readonly [string, number]>, number] {
  const layout = declaredStructLayout(source, name);
  return [
    layout?.members.map((m) => [m.name, m.offsetBytes] as const) ?? [],
    layout?.sizeBytes ?? 0,
  ];
}

/**
 * The test's capacity: its live instances, at least 10⁶ of a radius, about the camera, directions
 * uniform, distances and radii log-uniform; within four passes' four templates' lists of 2²⁴.
 */
const CAPACITY = 1_020_000;

/** The live ones: the rest past the producer's count. */
const LIVE = 1_015_000;

/** The sphere templates' bounds over the radius. */
const BOUNDS = sphereParticleTemplates().boundsScale;

function instancesOf(seed: number): InstanceBounds[] {
  const next = seededRandom(seed);
  const out: InstanceBounds[] = [];
  for (let k = 0; k < CAPACITY; k += 1) {
    const z = 2 * next() - 1;
    const phi = 2 * Math.PI * next();
    const s = Math.sqrt(1 - z * z);
    const d = 1 * 5000 ** next();
    // One in a hundred is no instance: a negative radius.
    const radius = next() < 0.01 ? -1 : 0.001 * 50_000 ** next();
    out.push({
      centreM: { x: d * s * Math.cos(phi), y: d * s * Math.sin(phi), z: d * z },
      radiusM: radius,
    });
  }
  return out;
}

/** A 60° camera at 1920 × 1080 looking along −z turned 20° about x, above a body's occluder. */
function cameraPass(): CameraCullPass {
  const frustum = frustumOf({
    orientation: quaternionFromAxisAngle({ x: 1, y: 0, z: 0 }, -0.35),
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 1920, heightPx: 1080 },
  });
  const pxPerRad = 1920 / (2 * Math.tan(Math.PI / 6));
  return {
    kind: "camera",
    planes: frustum.planes,
    horizon: cullHorizonOf({ x: 0, y: 1200, z: 0 }, 1000),
    order: "appended",
    toleranceRad: 1 / pxPerRad,
    strideCap: 1,
  };
}

/** A camera pass of no planes, every sphere in it, its τ wide so that each template is taken. */
const OPEN: CameraCullPass = {
  kind: "camera",
  planes: [],
  horizon: null,
  order: "appended",
  toleranceRad: 0.002,
  strideCap: 1,
};

/** A cascade's box of 600 m about a point ahead, its light 30° up, its receivers culled. */
const CASCADE: CascadeCullPass = (() => {
  const up = { x: 0, y: Math.sin(Math.PI / 6), z: Math.cos(Math.PI / 6) };
  const x = { x: 1, y: 0, z: 0 };
  const y = { x: 0, y: up.z, z: -up.y };
  return {
    kind: "cascade",
    axes: [x, y, up],
    centreFromCameraLightM: { x: 50, y: -120, z: 40 },
    radiusM: 600,
    depthFarM: 700,
    toleranceM: 0.5,
    strideCap: 1,
    receivers: {
      depthM: [150, 1500],
      reachM: [80, 30],
      coveredBelow: { centreM: [10, 20], halfSideM: 140, reachM: 25 },
    },
  };
})();

const VIEW_AXIS: Vec3 = { x: 0, y: -Math.sin(0.35), z: -Math.cos(0.35) };

// The oracle: each predicate written out again from its definition (`clusterCull.wgsl`'s head),
// each instance tested against each pass on its own.

const len = (v: Vec3): number => Math.hypot(v.x, v.y, v.z);
const dotOf = (a: Vec3, b: Vec3): number => a.x * b.x + a.y * b.y + a.z * b.z;

function oracleCamera(pass: CameraCullPass, c: Vec3, r: number, slack: number): boolean {
  const reach = r + slack * (CULL_ROUNDING_FRACTION * len(c) + INSTANCE_ROUNDING_M);
  if (len(c) <= reach) {
    return true;
  }
  for (const plane of pass.planes) {
    if (dotOf(plane.normal, c) + plane.offsetM < -reach) {
      return false;
    }
  }
  const h = pass.horizon;
  if (h === null) {
    return true;
  }
  const along = dotOf(c, h.axis);
  const allowance = slack * (CULL_ROUNDING_FRACTION * (len(c) + h.planeM) + INSTANCE_ROUNDING_M);
  if (along - r - allowance <= h.planeM) {
    return true;
  }
  const off = Math.sqrt(
    Math.max(dotOf(c, c) - along * along, 0) + slack * HORIZON_ROUNDING * dotOf(c, c),
  );
  return !(along * h.sin - off * h.cos > r + allowance);
}

function oracleCascade(pass: CascadeCullPass, c: Vec3, r: number, slack: number): boolean {
  const [ax, ay, az] = pass.axes;
  const o = pass.centreFromCameraLightM;
  const v = { x: dotOf(c, ax) - o.x, y: dotOf(c, ay) - o.y, z: dotOf(c, az) - o.z };
  const allowance =
    slack * (CULL_ROUNDING_FRACTION * (len(c) + len(o) + pass.radiusM) + INSTANCE_ROUNDING_M);
  const reach = pass.radiusM + r + allowance;
  if (!(
    Math.abs(v.x) <= reach &&
    Math.abs(v.y) <= reach &&
    v.z >= -(pass.depthFarM + r + allowance)
  )) {
    return false;
  }
  const rc = pass.receivers;
  if (rc === null) {
    return true;
  }
  const below = rc.coveredBelow;
  const d = dotOf(c, VIEW_AXIS);
  const narrowing =
    slack *
    (CULL_ROUNDING_FRACTION *
      (len(c) +
        Math.abs(rc.depthM[0]) +
        Math.abs(rc.depthM[1]) +
        (below === null ? 0 : Math.hypot(below.centreM[0], below.centreM[1]) + below.halfSideM)) +
      RECEIVER_ROUNDING_M);
  if (d - r - rc.reachM[0] - narrowing > rc.depthM[1]) {
    return false;
  }
  if (below === null || d + r + rc.reachM[1] + narrowing > rc.depthM[0]) {
    return true;
  }
  const x = dotOf(c, ax) - below.centreM[0];
  const y = dotOf(c, ay) - below.centreM[1];
  const across = r + below.reachM + narrowing;
  return !(Math.abs(x) + across <= below.halfSideM && Math.abs(y) + across <= below.halfSideM);
}

function oracleKeeps(pass: CullPass, c: Vec3, r: number, slack: number): boolean {
  let keeps: boolean;
  switch (pass.kind) {
    case "camera":
      keeps = oracleCamera(pass, c, r, slack);
      break;
    case "cascade":
      keeps = oracleCascade(pass, c, r, slack);
      break;
  }
  return keeps;
}

/**
 * The oracle's template: the coarsest whose error, ε r, projects within the pass's tolerance at the
 * instance's bounds.
 */
function oracleTemplate(pass: CullPass, errors: ReadonlyArray<number>, c: Vec3, r: number): number {
  const tolerance =
    pass.kind === "cascade"
      ? pass.toleranceM
      : pass.toleranceRad * Math.max(len(c) - r * BOUNDS, 0);
  const t = errors.findIndex((e, k) => k < errors.length - 1 && e * r <= tolerance);
  return t < 0 ? errors.length - 1 : t;
}

/** The oracle's buckets: by (pass, template), the instances each pass keeps at `slack`. */
function oracle(
  instances: ReadonlyArray<InstanceBounds>,
  passes: ReadonlyArray<CullPass | null>,
  passMask: number,
  slack: number,
): Map<string, Set<number>> {
  const { errorFractions } = sphereParticleTemplates();
  const out = new Map<string, Set<number>>();
  for (let k = 0; k < LIVE; k += 1) {
    const instance = instances[k];
    if (instance === undefined || instance.radiusM < 0) {
      continue;
    }
    passes.forEach((pass, p) => {
      if (pass === null || (passMask & (1 << p)) === 0) {
        return;
      }
      if (oracleKeeps(pass, instance.centreM, instance.radiusM * BOUNDS, slack)) {
        const key = `${String(p)} ${String(oracleTemplate(pass, errorFractions, instance.centreM, instance.radiusM))}`;
        const set = out.get(key) ?? new Set<number>();
        set.add(k);
        out.set(key, set);
      }
    });
  }
  return out;
}

/** The passes of the large check: the camera, none, the cascade and the open camera. */
const PASSES: ReadonlyArray<CullPass | null> = [cameraPass(), null, CASCADE, OPEN];

/** The twin's input over `instances` with buckets of the four passes' and the sphere templates. */
async function twinInput(
  instances: ReadonlyArray<InstanceBounds>,
  passMask: number,
): Promise<InstanceCullTwinInput> {
  const engine = new CountingRenderEngine(await fakeDevice());
  const { templates, errorFractions, boundsScale } = sphereParticleTemplates();
  const buckets = new ClusterBuckets(
    engine,
    "instances",
    PASSES.length,
    templates,
    templates.map(() => CAPACITY),
  );
  return {
    instances,
    count: LIVE,
    capacity: CAPACITY,
    passes: PASSES,
    places: PASSES.map((_, p) => buckets.place(p)),
    viewAxis: VIEW_AXIS,
    errorFractions,
    boundsScale,
    templateOffsets: buckets.templateOffsets,
    templateCapacities: buckets.capacities,
    passMask,
    listEntries: PASSES.length * buckets.passEntries,
  };
}

/** The twin's buckets, by (pass, template), as sets. */
function twinBuckets(
  input: InstanceCullTwinInput,
  writes: ReturnType<typeof instanceCullTwin>,
): Map<string, Set<number>> {
  const out = new Map<string, Set<number>>();
  input.passes.forEach((_, p) => {
    for (let t = 0; t < input.errorFractions.length; t += 1) {
      const place = input.places[p];
      if (place === undefined) {
        continue;
      }
      const entries = instanceBucketEntries(
        writes,
        place,
        t,
        input.templateOffsets[t] ?? 0,
        input.templateCapacities[t] ?? 0,
      );
      if (entries.length > 0) {
        out.set(`${String(p)} ${String(t)}`, new Set(entries));
      }
    }
  });
  return out;
}

/** The instances of `a` not in `b`, bucket by bucket, at most five of each. */
function missing(a: Map<string, Set<number>>, b: Map<string, Set<number>>): string[] {
  const out: string[] = [];
  for (const [key, set] of a) {
    const other = b.get(key) ?? new Set<number>();
    const gone = [...set].filter((k) => !other.has(k));
    if (gone.length > 0) {
      out.push(`${key}: ${String(gone.length)} (${gone.slice(0, 5).join(", ")})`);
    }
  }
  return out;
}

/** The union of a bucket map's instances by pass, as sets. */
function byPass(buckets: Map<string, Set<number>>): Map<number, Set<number>> {
  const out = new Map<number, Set<number>>();
  for (const [key, set] of buckets) {
    const p = Number(key.split(" ")[0]);
    const all = out.get(p) ?? new Set<number>();
    for (const k of set) {
      all.add(k);
    }
    out.set(p, all);
  }
  return out;
}

describe("the instance cull's parameters and kernels (plan R14, R14.T5.d)", () => {
  it("lays its parameters out as the kernels' structs", () => {
    expect(layoutOf(instanceCullWgsl, "InstanceCullParams")).toEqual([
      [
        ["counts", 0],
        ["viewAxis", 16],
        ["templates", 28],
        ["errors", 32],
        ["listOffsets", 48],
        ["capacities", 64],
        ["passMask", 80],
        ["boundsScale", 84],
        ["spare0", 88],
        ["spare1", 92],
      ],
      INSTANCE_CULL_PARAMS_BYTES,
    ]);
    expect(layoutOf(instanceCompactWgsl, "InstanceCompactParams")).toEqual([
      [
        ["counts", 0],
        ["viewAxis", 16],
        ["mask", 28],
      ],
      INSTANCE_COMPACT_PARAMS_BYTES,
    ]);
    expect(layoutOf(instanceFoldWgsl, "InstanceFoldParams")).toEqual([
      [["counts", 0]],
      INSTANCE_FOLD_PARAMS_BYTES,
    ]);
    for (const [source, constant, value] of [
      [instanceCullWgsl, "MAX_TEMPLATES", MAX_PARTICLE_TEMPLATES],
      [instanceCullWgsl, "GROUP_COUNTERS", MAX_CULL_PASSES * MAX_PARTICLE_TEMPLATES],
      [instanceCullWgsl, "INSTANCE_GROUP", INSTANCE_CULL_WORKGROUP],
      [instanceCompactWgsl, "INSTANCE_GROUP", INSTANCE_CULL_WORKGROUP],
    ] as const) {
      expect(source).toContain(`const ${constant} : u32 = ${String(value)}u;`);
    }
    for (const source of [instanceCullWgsl, instanceCompactWgsl]) {
      expect(source).toContain(`const ROUNDING_M : f32 = ${String(INSTANCE_ROUNDING_M)};`);
    }
    expect(CLUSTER_CULL_KERNEL.reference).toContain(
      `const ROUNDING_M : f32 = ${CULL_ROUNDING_M.toFixed(1)};`,
    );
  });

  it("packs the particles' cull's parameters at their offsets, the rest zero", () => {
    const out = new ArrayBuffer(INSTANCE_CULL_PARAMS_BYTES);
    packInstanceCullParams(
      {
        capacity: 1000,
        words: 8,
        radiusWord: 3,
        passes: 2,
        viewAxis: { x: 0, y: 0.5, z: -1 },
        errorFractions: [0.5, 0.25, 0.125],
        templateOffsets: [0, 1000, 2000],
        templateCapacities: [1000, 1000, 1000],
        passMask: 0b11,
        boundsScale: 1.25,
      },
      out,
    );
    expect([
      [...new Uint32Array(out, 0, 4)],
      [...new Float32Array(out, 16, 3)],
      new Uint32Array(out)[7],
      [...new Float32Array(out, 32, 4)],
      [...new Uint32Array(out, 48, 4)],
      [...new Uint32Array(out, 64, 4)],
      new Uint32Array(out)[20],
      new Float32Array(out)[21],
      [...new Uint32Array(out, 88, 2)],
    ]).toEqual([
      [1000, 8, 3, 2],
      [0, 0.5, -1],
      3,
      [0.5, 0.25, 0.125, 0],
      [0, 1000, 2000, 0],
      [1000, 1000, 1000, 0],
      3,
      1.25,
      [0, 0],
    ]);
    expect(() => {
      packInstanceCullParams(
        {
          capacity: 1,
          words: 8,
          radiusWord: 3,
          passes: 1,
          viewAxis: { x: 0, y: 0, z: -1 },
          errorFractions: [0.5, 0.4, 0.3, 0.2, 0.1],
          templateOffsets: [0, 1, 2, 3, 4],
          templateCapacities: [1, 1, 1, 1, 1],
          passMask: 1,
          boundsScale: 1,
        },
        out,
      );
    }).toThrow(RangeError);
    expect(() => {
      packInstanceCullParams(
        {
          capacity: 1,
          words: 8,
          radiusWord: 3,
          passes: 1,
          viewAxis: { x: 0, y: 0, z: -1 },
          errorFractions: [0.5],
          templateOffsets: [0],
          templateCapacities: [1],
          passMask: 1,
          boundsScale: 0.9,
        },
        out,
      );
    }).toThrow(RangeError);
  });

  it("packs the mesh instances' cull's parameters, and refuses a radius outside the record", () => {
    const out = new ArrayBuffer(INSTANCE_COMPACT_PARAMS_BYTES);
    const params = {
      capacity: 70_000,
      words: 8,
      radiusWord: 4,
      passes: 3,
      viewAxis: { x: 1, y: 0, z: 0 },
      passMask: 0b101,
    };
    packInstanceCompactParams(params, out);
    expect([
      [...new Uint32Array(out, 0, 4)],
      [...new Float32Array(out, 16, 3)],
      new Uint32Array(out)[7],
    ]).toEqual([[70_000, 8, 4, 3], [1, 0, 0], 0b101]);
    expect(() => {
      packInstanceCompactParams({ ...params, radiusWord: 8 }, out);
    }).toThrow(RangeError);
    expect(() => {
      packInstanceCompactParams({ ...params, passes: MAX_CULL_PASSES + 1 }, out);
    }).toThrow(RangeError);
  });

  it("composes each culling kernel of the fold's index and the cluster cull's own pass tests", () => {
    expect(CULL_PASS_TESTS_WGSL).toContain("struct CullPass {");
    expect(CULL_PASS_TESTS_WGSL).toContain("fn pass_keeps(cull_pass : CullPass");
    expect(CULL_PASS_TESTS_WGSL).toContain("fn group_distance(");
    expect(CULL_PASS_TESTS_WGSL).not.toContain("@binding");
    expect(CULL_PASS_TESTS_WGSL).not.toContain("const ROUNDING_M");
    for (const kernel of [INSTANCE_CULL_KERNEL, INSTANCE_COMPACT_KERNEL]) {
      expect([
        kernel.reference.startsWith(FOLDED_GROUP_WGSL + CULL_PASS_TESTS_WGSL),
        kernel.readback,
      ]).toEqual([true, "presentation-only"]);
    }
    for (const kernel of [CLUSTER_CULL_KERNEL, CLUSTER_CULL_OCCLUSION_KERNEL]) {
      expect(kernel.reference.startsWith(FOLDED_GROUP_WGSL)).toBe(true);
      expect(kernel.reference).toContain("let instance = folded_group(group, groups);");
    }
  });

  it("dispatches a workgroup each 64 instances, folded past 65,535 workgroups", () => {
    expect([
      instanceWorkgroups(1),
      instanceWorkgroups(1_000_000),
      instanceWorkgroups(65_535 * 64 + 1),
    ]).toEqual([
      [1, 1, 1],
      [15_625, 1, 1],
      [32_768, 2, 1],
    ]);
  });
});

/** The large check's fixtures, made once and only read by its tests. */
interface LargeCheck {
  readonly instances: ReadonlyArray<InstanceBounds>;
  readonly input: InstanceCullTwinInput;
  readonly writes: ReturnType<typeof instanceCullTwin>;
  readonly twin: Map<string, Set<number>>;
}

/** A bucket map's instances by pass, keyed by the pass's index as a string. */
function passesOf(buckets: Map<string, Set<number>>): Map<string, Set<number>> {
  return new Map([...byPass(buckets)].map(([p, set]) => [String(p), set]));
}

describe("the instance cull's twin against a brute-force oracle (plan R14, R14.T5.d)", () => {
  let large: LargeCheck | null = null;
  const check = (): LargeCheck => {
    if (large === null) {
      throw new Error("the large check's fixtures were not made");
    }
    return large;
  };
  beforeAll(async () => {
    const instances = instancesOf(0x0d05_17ce);
    const input = await twinInput(instances, 0b1101);
    const writes = instanceCullTwin(input);
    large = { instances, input, writes, twin: twinBuckets(input, writes) };
  }, 120_000);

  it("runs over at least 10⁶ live instances of a radius", () => {
    const { instances } = check();
    expect(instances.slice(0, LIVE).filter((i) => i.radiusM >= 0).length).toBeGreaterThanOrEqual(
      1_000_000,
    );
  });

  it("fills each (pass, template) bucket with the instances the oracle keeps there at the kernel's allowance, but within a thousandth of it", () => {
    const { instances, twin } = check();
    expect(missing(oracle(instances, PASSES, 0b1101, 0.999), twin)).toEqual([]);
    expect(missing(twin, oracle(instances, PASSES, 0b1101, 1.001))).toEqual([]);
  }, 60_000);

  it("never refuses an instance the exact test keeps", () => {
    const { instances, twin } = check();
    expect(missing(passesOf(oracle(instances, PASSES, 0b1101, 0)), passesOf(twin))).toEqual([]);
  }, 60_000);

  it("keeps no instance past twice the kernel's allowance", () => {
    const { instances, twin } = check();
    expect(missing(passesOf(twin), passesOf(oracle(instances, PASSES, 0b1101, 2)))).toEqual([]);
  }, 60_000);

  it("refuses some instances in each pass that culls and takes every template, as the fixture means it to", () => {
    const { twin } = check();
    expect([...byPass(twin)].map(([p, s]) => [p, s.size > 0 && s.size < LIVE])).toEqual([
      [0, true],
      [2, true],
      [3, true],
    ]);
    expect([0, 1, 2, 3].map((t) => (twin.get(`3 ${String(t)}`)?.size ?? 0) > 0)).toEqual([
      true,
      true,
      true,
      true,
    ]);
  });

  it("lists no instance past the live count or of a negative radius, none twice, each bucket's count its entries", () => {
    const { instances, input, writes, twin } = check();
    const listed = [...twin.values()].flatMap((s) => Array.from(s));
    expect(listed.filter((k) => k >= LIVE || (instances[k]?.radiusM ?? -1) < 0)).toEqual([]);
    const total = [...twin].reduce((n, [key]) => {
      const [p, t] = key.split(" ").map(Number);
      return n + (writes.counts[(input.places[p ?? 0]?.bucketBase ?? 0) + (t ?? 0)] ?? 0);
    }, 0);
    expect(total).toBe(listed.length);
  });

  it("fills the same buckets whatever the fold that covers the capacity", () => {
    const { input, twin } = check();
    // 160 rows of 100 workgroups, 16,000 of 64 instances: past the capacity's 15,938.
    const folded = twinBuckets(input, instanceCullTwin({ ...input, fold: [100, 160] }));
    expect([missing(twin, folded), missing(folded, twin)]).toEqual([[], []]);
  }, 60_000);

  it("fills no bucket of a pass the mask leaves out", async () => {
    const input = await twinInput(check().instances, 0b0001);
    const masked = twinBuckets(input, instanceCullTwin(input));
    expect([...masked.keys()].filter((key) => !key.startsWith("0 "))).toEqual([]);
    expect(masked.size).toBeGreaterThan(0);
  }, 60_000);

  it("takes the coarsest template whose error projects within the pass's tolerance at the bounds, the finest past every one", () => {
    const { errorFractions } = sphereParticleTemplates();
    const pass = OPEN;
    // About 1,000 m out, τ 0.002 rad reaches about 2 m: the octahedron's 0.35 r for r up to 5.8 m.
    const at = (r: number): number =>
      instanceTemplateOf(pass, errorFractions, { x: 0, y: 0, z: -1000 - r }, r, BOUNDS);
    expect([at(4), at(9), at(25), at(100), at(1e4)]).toEqual([0, 1, 2, 3, 3]);
    // Inside its bounds every template's error lies past a tolerance of 0: the finest.
    expect(instanceTemplateOf(pass, errorFractions, { x: 0, y: 0, z: -2 }, 2, BOUNDS)).toBe(3);
    // A cascade's tolerance is its texel, whatever the distance.
    expect(instanceTemplateOf(CASCADE, errorFractions, { x: 0, y: 0, z: -1e5 }, 1, BOUNDS)).toBe(0);
    expect(instanceTemplateOf(CASCADE, errorFractions, { x: 0, y: 0, z: -1 }, 30, BOUNDS)).toBe(3);
  });

  it("tells a sphere whose error falls at its tolerance from one well within it", () => {
    const { errorFractions } = sphereParticleTemplates();
    // ε₀ r = 0.002 (1,000 + r − k r) at the edge, k the bounds' scale.
    const edge = 2 / ((errorFractions[0] ?? 1) + 0.002 * (BOUNDS - 1));
    const near = (r: number): boolean =>
      instanceTemplateNear(OPEN, errorFractions, { x: 0, y: 0, z: -1000 - r }, r, BOUNDS);
    expect([near(edge), near(4)]).toEqual([true, false]);
  });

  it("compacts the instances any pass keeps with the passes that keep them, as the oracle tests them", () => {
    const some = check().instances.slice(0, 200_000);
    const twin = instanceCompactTwin(
      { instances: some, passes: PASSES, viewAxis: VIEW_AXIS, passMask: 0b1101 },
      0,
    );
    const wrong: number[] = [];
    some.forEach((instance, k) => {
      let bits = 0;
      PASSES.forEach((pass, p) => {
        if (
          pass !== null &&
          (0b1101 & (1 << p)) !== 0 &&
          instance.radiusM >= 0 &&
          oracleKeeps(pass, instance.centreM, instance.radiusM, 0)
        ) {
          bits |= 1 << p;
        }
      });
      if ((twin.get(k) ?? 0) !== bits) {
        wrong.push(k);
      }
    });
    expect(wrong.slice(0, 10)).toEqual([]);
    expect(twin.size).toBeGreaterThan(0);
  });
});
