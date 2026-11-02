/**
 * The cluster path's counting harness (plan R14, Design note 18; built by R14.T1.f): triangles a
 * frame by pass over R05's descent on R10.T4.b's sample worlds, today's path against each of the
 * levers as built, the research's lost `tools/triCount.ts` rebuilt on `levelBoundsCounts.ts`.
 *
 * @remarks
 * - **The poses and the view** are `levelBoundsCounts.ts`': R05's descent at its record seed, every
 *   {@link COUNT_POSE_STEP_S}, one 1,920 × 1,080 view of 60°, each setting's τ, selected under the
 *   hard bound at the setting's budget (half its slots: 604 high, 187 low) with the boxes baked
 *   flat at ±1 m, as the research counted (`research-terrain-gpu-driven`'s measurements, §1).
 * - **Each selected patch** is a bake on the datum: its origin the datum point at its grid's
 *   centre, its heights ±1 m, its skirts {@link COUNT_SKIRT_DEPTH_M} deep, and its 96 cluster records
 *   written as a slot's are (`writeTerrainClusterRecords`); its bounding radius about its origin the
 *   largest reach of its blocks' spheres. The sample worlds are wholly surveyed, so every patch is
 *   interior; edge patches are counted apart, and are none here (a served world mid-survey is the
 *   owner's run).
 * - **The cascades** (high only, as the low setting has none): `fitCascades` of the pose, the sun
 *   {@link COUNT_SUN_ELEVATION_DEG} above the horizon at the camera's nadir at each of
 *   {@link COUNT_SUN_AZIMUTHS_DEG} from the view's heading, the relief {@link COUNT_RELIEF_M}, the
 *   horizon texel the nadir patch's mean spacing; a pose whose cascades take w = 0 casts nothing.
 *   A patch casts into a cascade where `castsInto` admits it, by the lit mesh today (10,752
 *   triangles), and on the cluster path each of its clusters the cull's twin keeps (`cullTwin`, at
 *   its kernel's allowance), at stride 1 (T1.b), with its receivers' cull (T1.c,
 *   `cascadeCullPasses`) and at its cascade's texel up to the setting's cap (T1.d).
 * - **The lit pass**: today every patch's lit mesh (10,752); T1.a an interior patch's 8,704; T2.a
 *   the interior patches' clusters its ordered camera pass keeps at stride 1 (`cullMarksTwin`): the
 *   view's frustum opened by the cull's margin and R05's horizon occluder (`horizonCone`).
 * - **The overlay's depth**: today the lit draws redrawn; T1.e the interior patches' clusters the
 *   camera's frustum keeps at stride 1 (opened by the overlay cull's margin), and the edge patches'
 *   draws.
 *
 * Renderer code reads no files: `scripts/clusterCounts.mjs` reads the fixture, runs the cells
 * through Vite's module runner and writes the record this module makes, by hand, not in `ci`.
 */

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { SETTINGS, type QualitySetting } from "../quality/qualitySetting";
import { vertexDir, vertexSpacing } from "../terrain/cube";
import { frustumOf, horizonCone } from "../terrain/cull";
import { LIT_INTERIOR_INDICES, LIT_PATCH_INDICES } from "../terrain/gpu/resources";
import {
  COUNT_FOV_X_RAD,
  COUNT_POSE_STEP_S,
  COUNT_STYLES,
  COUNT_VIEWPORT,
  countProfile,
  countTimesS,
  type SampleWorld,
} from "../terrain/levelBoundsCounts";
import type { PatchKey } from "../terrain/patchKey";
import { type PlanetGeometry, planetGeometry, surfacePoint } from "../terrain/planet";
import { type HeightRangeLookup, selectPatches } from "../terrain/select";
import { type CascadeIndex } from "../terrain/shadows/cascadeShadow";
import { type CascadeSet, fitCascades } from "../terrain/shadows/cascades";
import { castsInto, meanRadiusM } from "../terrain/terrainPass";
import { cullMarksTwin, cullTwin, type CullInstance, type CullTwinBuckets, NO_MARK } from "./cull";
import { type CameraCullPass, type CullPass, type CullPassPlace, cullHorizonOf } from "./cullPass";
import { cascadeCullPasses } from "./receivers";
import { CLUSTER_RECORD_BYTES, type ClusterRecord } from "./record";
import { TERRAIN_CLUSTERS_PER_PAGE, writeTerrainClusterRecords } from "./terrain";
import { terrainClusterMesh } from "./templates";

/** The sun's elevation at the camera's nadir, degrees: the research's 30°. */
export const COUNT_SUN_ELEVATION_DEG = 30;

/** The sun's azimuths from the view's heading, degrees: ahead, abeam and behind. */
export const COUNT_SUN_AZIMUTHS_DEG: ReadonlyArray<number> = [0, 90, 180];

/** The body's relief the cascades' caster reach takes, m: the research's 20 km. */
export const COUNT_RELIEF_M = 20_000;

/** A counted bake's skirt depth, m: the boxes' own span, ±1 m. */
export const COUNT_SKIRT_DEPTH_M = 2;

/** The overlay cull's frustum margin, rad, as `terrainPass.ts`' `OVERLAY_CULL_MARGIN_RAD`. */
const OVERLAY_MARGIN_RAD = 0.01;

/** Boxes baked flat at ±1 m, as `levelBoundsCounts.ts`' flat boxes. */
const FLAT: HeightRangeLookup = { heightRangeM: () => [-1, 1] };

const TEMPLATES = terrainClusterMesh().templates;

/** The bands the counts are summarised in, by the camera's altitude, m. */
export const COUNT_BANDS: ReadonlyArray<{
  readonly name: string;
  readonly fromM: number;
  readonly toM: number;
}> = [
  { name: "orbit 100-1000 km", fromM: 1e5, toM: Infinity },
  { name: "10-100 km", fromM: 1e4, toM: 1e5 },
  { name: "1-10 km", fromM: 1e3, toM: 1e4 },
  { name: "below 1 km", fromM: -Infinity, toM: 1e3 },
];

/** One pose's triangles by pass and lever (see the module's remarks). */
export interface PoseTriangles {
  readonly tS: number;
  readonly altitudeM: number;
  readonly interiorPatches: number;
  readonly edgePatches: number;
  /** The lit pass: today's lit mesh, T1.a's interior draw, and T2.a's clusters. */
  readonly litToday: number;
  readonly litInterior: number;
  readonly litClusters: number;
  /**
   * The casters, all four cascades, at each sun azimuth in {@link COUNT_SUN_AZIMUTHS_DEG}'s order:
   * today's, T1.b's, T1.c's and T1.d's; `null` on low and where the cascades take w = 0.
   */
  readonly casters: ReadonlyArray<{
    readonly today: number;
    readonly clusters: number;
    readonly receivers: number;
    readonly strided: number;
  } | null>;
  /** The overlay's depth: today's redraw, and T1.e's at stride 1. */
  readonly overlayToday: number;
  readonly overlayClusters: number;
}

/** One world and setting over the descent's poses. */
export interface ClusterCountCell {
  readonly world: string;
  readonly setting: QualitySetting;
  readonly maxPatches: number;
  readonly poses: ReadonlyArray<PoseTriangles>;
}

/** A patch as the counts bake it: its records, its origin less the camera and its reach. */
interface CountedPatch {
  readonly key: PatchKey;
  readonly fromCameraM: Vec3;
  readonly radiusM: number;
  readonly records: ReadonlyArray<ClusterRecord>;
}

/** `key`'s bake on the datum (see the module's remarks): its records and its reach. */
function bakeOf(planet: PlanetGeometry, key: PatchKey, cameraM: Vec3): CountedPatch {
  const [ox, oy, oz] = surfacePoint(planet.figure, vertexDir(key, 32, 32), 0);
  const originM = vec3(ox, oy, oz);
  const out = new DataView(new ArrayBuffer(TERRAIN_CLUSTERS_PER_PAGE * CLUSTER_RECORD_BYTES));
  writeTerrainClusterRecords(
    planet,
    { key, originM, heightRangeM: [-1, 1], skirtDepthM: COUNT_SKIRT_DEPTH_M },
    out,
    0,
  );
  const records: ClusterRecord[] = [];
  let radiusM = 0;
  for (let c = 0; c < TERRAIN_CLUSTERS_PER_PAGE; c += 1) {
    const at = c * CLUSTER_RECORD_BYTES;
    const centreM = vec3(
      out.getFloat32(at, true),
      out.getFloat32(at + 4, true),
      out.getFloat32(at + 8, true),
    );
    const r = out.getFloat32(at + 12, true);
    radiusM = Math.max(radiusM, norm(centreM) + r);
    records.push({
      centreM,
      radiusM: r,
      mode: "strided",
      strideErrorBits: null,
      templateRef: 0,
      shape: c < 64 ? "terrain-block" : "terrain-skirt",
      mayDiscard: false,
      material: 0,
    });
  }
  return { key, fromCameraM: sub(originM, cameraM), radiusM, records };
}

/** The triangles of a twin's buckets, each bucket's entries times its template's. */
function bucketTriangles(buckets: CullTwinBuckets, templates: number): number {
  let sum = 0;
  for (const [bucket, entries] of buckets) {
    sum += entries.length * (TEMPLATES[bucket % templates]?.triangles ?? 0);
  }
  return sum;
}

/** The sun's direction at the nadir, `elevationDeg` up at `azimuthDeg` from `forward`'s heading. */
function sunOver(cameraM: Vec3, forward: Vec3, azimuthDeg: number, elevationDeg: number): Vec3 {
  const up = normalise(cameraM);
  const level = sub(forward, scale(up, dot(forward, up)));
  // A view straight down takes the north for its heading.
  const heading =
    norm(level) > 1e-9 ? normalise(level) : normalise(sub(vec3(0, 0, 1), scale(up, up.z)));
  const right = cross(heading, up);
  const az = (azimuthDeg * Math.PI) / 180;
  const el = (elevationDeg * Math.PI) / 180;
  return add(
    scale(add(scale(heading, Math.cos(az)), scale(right, Math.sin(az))), Math.cos(el)),
    scale(up, Math.sin(el)),
  );
}

/** The casters of `patches` into `set`'s four cascades, by the lever (see the module's remarks). */
function castersOf(
  set: CascadeSet,
  patches: ReadonlyArray<CountedPatch>,
  forward: Vec3,
  radiusM: number,
  strideCap: CullPass["strideCap"],
): {
  readonly today: number;
  readonly clusters: number;
  readonly receivers: number;
  readonly strided: number;
} {
  const instances: CullInstance[] = [];
  let today = 0;
  patches.forEach((patch, page) => {
    let mask = 0;
    for (const index of [0, 1, 2, 3] as const satisfies ReadonlyArray<CascadeIndex>) {
      if (castsInto(set, index, patch.fromCameraM, patch.radiusM)) {
        mask |= 1 << index;
        today += LIT_PATCH_INDICES / 3;
      }
    }
    instances.push({
      originM: patch.fromCameraM,
      page,
      passMask: mask,
      unitM: vertexSpacing(radiusM, patch.key.level).maxM,
    });
  });
  const axes = [set.basis.x, set.basis.y, set.basis.z] as const;
  const run = (cap: CullPass["strideCap"], receivers: boolean): number => {
    const passes = cascadeCullPasses(set, axes, forward, cap, receivers);
    const places: CullPassPlace[] = passes.map((_, p) => ({
      bucketBase: p * TEMPLATES.length,
      listBase: 0,
    }));
    return bucketTriangles(
      cullTwin({
        instances,
        records: (page, cluster) => {
          const record = patches[page]?.records[cluster];
          if (record === undefined) {
            throw new Error(`no record ${String(cluster)} of page ${String(page)}`);
          }
          return record;
        },
        clustersPerPage: TERRAIN_CLUSTERS_PER_PAGE,
        rotationColumns: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)],
        passes,
        places,
        templates: TEMPLATES.length,
        viewAxis: forward,
      }),
      TEMPLATES.length,
    );
  };
  return {
    today,
    clusters: run(1, false),
    receivers: run(1, true),
    strided: run(strideCap, true),
  };
}

/** What a cell is: its world and setting, and its poses' times. */
export interface ClusterCountInput {
  readonly world: SampleWorld;
  readonly setting: QualitySetting;
  /** The poses' times, s; every {@link COUNT_POSE_STEP_S} of the descent where absent. */
  readonly timesS?: ReadonlyArray<number>;
}

/** Counts one cell: at each pose, each pass's triangles today and by each lever. */
export function countClusterCell(input: ClusterCountInput): ClusterCountCell {
  const { world, setting } = input;
  const style = COUNT_STYLES.find((each) => each.name === setting);
  if (style === undefined) {
    throw new Error(`no count style for ${setting}`);
  }
  const planet = planetGeometry(world.figure, world.table, world.sigmaM, "hard");
  const profile = countProfile(world);
  const radiusM = world.figure.equatorialRadiusM;
  const geometry = SETTINGS[setting].geometry;
  const poses = (input.timesS ?? countTimesS(profile)).map((tS): PoseTriangles => {
    const pose = profile.poseAt(tS);
    const camera = { positionM: pose.positionM, orientation: pose.orientation };
    const selection = selectPatches({
      planet,
      views: [
        {
          camera,
          fovXRad: COUNT_FOV_X_RAD,
          viewport: COUNT_VIEWPORT,
          weight: 1,
          tauPx: style.tauPx,
        },
      ],
      setting,
      grounded: [],
      heightRanges: FLAT,
      maxPatches: style.maxPatches,
    });
    const keys = [...selection.patches.values()].filter((p) => p.seen).map((p) => p.key);
    const patches = keys.map((key) => bakeOf(planet, key, pose.positionM));
    const n = patches.length;
    const forward = rotate(pose.orientation, vec3(0, 0, -1));
    // The overlay's camera pass: R05's frustum, opened by the overlay cull's margin.
    const frustum = frustumOf(
      { orientation: pose.orientation, fovXRad: COUNT_FOV_X_RAD, viewport: COUNT_VIEWPORT },
      OVERLAY_MARGIN_RAD,
    );
    const overlayPass: CameraCullPass = {
      kind: "camera",
      planes: frustum.planes,
      toleranceM: 0,
      strideCap: 1,
      horizon: null,
      order: "appended",
    };
    // The lit pass's ordered camera pass (T2.a): the frustum and R05's horizon occluder.
    const litPass: CameraCullPass = {
      ...overlayPass,
      horizon: cullHorizonOf(pose.positionM, horizonCone(planet, pose.positionM).occluderRadiusM),
      order: "ordered",
    };
    const marks = cullMarksTwin({
      instances: patches.map((patch, page) => ({
        originM: patch.fromCameraM,
        page,
        passMask: 1,
        unitM: 0,
      })),
      records: (page, cluster) => {
        const record = patches[page]?.records[cluster];
        if (record === undefined) {
          throw new Error(`no record ${String(cluster)} of page ${String(page)}`);
        }
        return record;
      },
      clustersPerPage: TERRAIN_CLUSTERS_PER_PAGE,
      rotationColumns: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)],
      passes: [litPass],
      places: [{ bucketBase: 0, listBase: 0 }],
      templates: TEMPLATES.length,
      viewAxis: forward,
    });
    let litClusters = 0;
    for (const mark of marks) {
      litClusters += mark === NO_MARK ? 0 : (TEMPLATES[mark]?.triangles ?? 0);
    }
    const overlayClusters = bucketTriangles(
      cullTwin({
        instances: patches.map((patch, page) => ({
          originM: patch.fromCameraM,
          page,
          passMask: 1,
          unitM: 0,
        })),
        records: (page, cluster) => {
          const record = patches[page]?.records[cluster];
          if (record === undefined) {
            throw new Error(`no record ${String(cluster)} of page ${String(page)}`);
          }
          return record;
        },
        clustersPerPage: TERRAIN_CLUSTERS_PER_PAGE,
        rotationColumns: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)],
        passes: [overlayPass],
        places: [{ bucketBase: 0, listBase: 0 }],
        templates: TEMPLATES.length,
        viewAxis: forward,
      }),
      TEMPLATES.length,
    );
    let casters: PoseTriangles["casters"] = COUNT_SUN_AZIMUTHS_DEG.map(() => null);
    if (setting === "high" && n > 0) {
      // The nadir patch: the selected patch nearest the camera's foot, whose spacing the horizon
      // map's texel there is.
      const foot = scale(normalise(pose.positionM), radiusM);
      const nadir = patches.reduce((best, patch) =>
        norm(sub(add(patch.fromCameraM, pose.positionM), foot)) <
        norm(sub(add(best.fromCameraM, pose.positionM), foot))
          ? patch
          : best,
      );
      const up = normalise(pose.positionM);
      casters = COUNT_SUN_AZIMUTHS_DEG.map((azimuthDeg) => {
        const set = fitCascades({
          cameraBodyFixedM: pose.positionM,
          orientationBodyFixed: pose.orientation,
          rotation: IDENTITY_ROTATION,
          fovXRad: COUNT_FOV_X_RAD,
          aspect: COUNT_VIEWPORT.widthPx / COUNT_VIEWPORT.heightPx,
          nearM: NEAR_PLANE_M,
          heightAboveGroundM: Math.max(pose.altitudeM, 0),
          sunDirectionBodyFixed: sunOver(
            pose.positionM,
            forward,
            azimuthDeg,
            COUNT_SUN_ELEVATION_DEG,
          ),
          datumNormalBodyFixed: up,
          bodyRadiusM: meanRadiusM(world.figure),
          reliefRangeM: COUNT_RELIEF_M,
          horizonTexelM: vertexSpacing(radiusM, nadir.key.level).meanM,
          depthClipControl: true,
        });
        return set.weight > 0
          ? castersOf(set, patches, forward, radiusM, geometry.casterStrideCap)
          : null;
      });
    }
    return {
      tS,
      altitudeM: pose.altitudeM,
      interiorPatches: n,
      edgePatches: 0,
      litToday: (n * LIT_PATCH_INDICES) / 3,
      litInterior: (n * LIT_INTERIOR_INDICES) / 3,
      litClusters,
      casters,
      overlayToday: (n * LIT_PATCH_INDICES) / 3,
      overlayClusters,
    };
  });
  return { world: world.name, setting, maxPatches: style.maxPatches, poses };
}

/** The record's schema name. */
export const CLUSTER_COUNTS_SCHEMA = "hyperion.clusters.counts";

/** The record's schema version, bumped with any change to its shape or to what a field means. */
export const CLUSTER_COUNTS_VERSION = 1;

/** The counts' record: what was counted, how, and every cell. */
export interface ClusterCountsRecord {
  readonly schema: typeof CLUSTER_COUNTS_SCHEMA;
  readonly version: typeof CLUSTER_COUNTS_VERSION;
  readonly startedAt: string;
  readonly generatorVersion: number;
  readonly poseStepS: number;
  readonly sunAzimuthsDeg: ReadonlyArray<number>;
  readonly notes: ReadonlyArray<string>;
  readonly cells: ReadonlyArray<ClusterCountCell>;
}

/** The record of `cells`. */
export function clusterCountsRecord(input: {
  readonly startedAt: string;
  readonly generatorVersion: number;
  readonly cells: ReadonlyArray<ClusterCountCell>;
  readonly notes?: ReadonlyArray<string>;
}): ClusterCountsRecord {
  return {
    schema: CLUSTER_COUNTS_SCHEMA,
    version: CLUSTER_COUNTS_VERSION,
    startedAt: input.startedAt,
    generatorVersion: input.generatorVersion,
    poseStepS: COUNT_POSE_STEP_S,
    sunAzimuthsDeg: COUNT_SUN_AZIMUTHS_DEG,
    notes: input.notes ?? [],
    cells: input.cells,
  };
}

/** The median of `values`, `NaN` for none. */
function median(values: ReadonlyArray<number>): number {
  if (values.length === 0) {
    return Number.NaN;
  }
  const sorted = values.toSorted((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1
    ? (sorted[mid] ?? Number.NaN)
    : ((sorted[mid - 1] ?? Number.NaN) + (sorted[mid] ?? Number.NaN)) / 2;
}

/** A band's medians, in millions of triangles a frame, and its poses. */
export interface BandFigures {
  readonly band: string;
  readonly poses: number;
  readonly litToday: number;
  readonly litInterior: number;
  readonly litClusters: number;
  /** By sun azimuth, `null` where no pose of the band cast. */
  readonly casters: ReadonlyArray<{
    readonly today: number;
    readonly clusters: number;
    readonly receivers: number;
    readonly strided: number;
  } | null>;
  readonly overlayToday: number;
  readonly overlayClusters: number;
  /**
   * A frame's total on the cluster path, the lit pass's (T2.a's), the casters' (the azimuths'
   * median, each band's median over its poses) and the overlay's; and today's.
   */
  readonly totalToday: number;
  readonly totalClusterPath: number;
}

/** `cell`'s figures by band (see {@link BandFigures}). */
export function bandFigures(cell: ClusterCountCell): BandFigures[] {
  const m = 1e-6;
  return COUNT_BANDS.flatMap(({ name, fromM, toM }) => {
    const poses = cell.poses.filter((p) => p.altitudeM >= fromM && p.altitudeM < toM);
    if (poses.length === 0) {
      return [];
    }
    const of = (pick: (p: PoseTriangles) => number): number => median(poses.map(pick)) * m;
    const casters = COUNT_SUN_AZIMUTHS_DEG.map((_, a) => {
      const cast = poses.flatMap((p) => {
        const each = p.casters[a];
        return each === null || each === undefined ? [] : [each];
      });
      return cast.length === 0
        ? null
        : {
            today: median(cast.map((c) => c.today)) * m,
            clusters: median(cast.map((c) => c.clusters)) * m,
            receivers: median(cast.map((c) => c.receivers)) * m,
            strided: median(cast.map((c) => c.strided)) * m,
          };
    });
    const cast = casters.filter((each) => each !== null);
    const castToday = cast.length === 0 ? 0 : median(cast.map((c) => c.today));
    const castStrided = cast.length === 0 ? 0 : median(cast.map((c) => c.strided));
    const litToday = of((p) => p.litToday);
    const litInterior = of((p) => p.litInterior);
    const litClusters = of((p) => p.litClusters);
    const overlayToday = of((p) => p.overlayToday);
    const overlayClusters = of((p) => p.overlayClusters);
    return [
      {
        band: name,
        poses: poses.length,
        litToday,
        litInterior,
        litClusters,
        casters,
        overlayToday,
        overlayClusters,
        totalToday: litToday + castToday + overlayToday,
        totalClusterPath: litClusters + castStrided + overlayClusters,
      },
    ];
  });
}

/** A figure to two decimals, `—` where it is not finite. */
function m2(value: number): string {
  return Number.isFinite(value) ? value.toFixed(2) : "—";
}

/** A ratio to three decimals, `—` where it is not finite. */
function r3(value: number): string {
  return Number.isFinite(value) ? value.toFixed(3) : "—";
}

/** The record's summary in Markdown: each cell's bands, the counts and each lever's ratio. */
export function clusterCountsMarkdown(record: ClusterCountsRecord): string {
  const lines = [
    `# Cluster counts, ${record.startedAt}`,
    "",
    `Plan R14's counting harness (Design note 18) on fixtures/level-bounds.json at generator version ${String(record.generatorVersion)}: R05's descent at its record seed every ${String(record.poseStepS)} s, one 1920 × 1080 view of 60° on each setting, the hard bound at the setting's budget, the boxes flat at ±1 m; the cascades on high at a sun ${String(COUNT_SUN_ELEVATION_DEG)}° up at ${record.sunAzimuthsDeg.join("°, ")}° from the heading, a relief of ${String(COUNT_RELIEF_M / 1000)} km. Medians a band, millions of triangles a frame. Every sample world is wholly surveyed: every patch interior, no edge patch.`,
    ...record.notes.map((note) => `- ${note}`),
    "",
    "| World | Setting | Band | Poses | Lit | Lit, T1.a (ratio) | Lit, T2.a (ratio to T1.a) | Casters by azimuth: today, T1.b, T1.c, T1.d | Overlay, T1.e (ratio) | Total today | Total on the path (ratio) |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
  ];
  for (const cell of record.cells) {
    for (const band of bandFigures(cell)) {
      const casters = band.casters
        .map((c) =>
          c === null
            ? "—"
            : `${m2(c.today)}, ${m2(c.clusters)} (${r3(c.clusters / c.today)}), ${m2(c.receivers)} (${r3(c.receivers / c.clusters)}), ${m2(c.strided)} (${r3(c.strided / c.receivers)})`,
        )
        .join("; ");
      lines.push(
        `| ${cell.world} | ${cell.setting} | ${band.band} | ${String(band.poses)} | ${m2(band.litToday)} | ${m2(band.litInterior)} (${r3(band.litInterior / band.litToday)}) | ${m2(band.litClusters)} (${r3(band.litClusters / band.litInterior)}) | ${casters} | ${m2(band.overlayClusters)} (${r3(band.overlayClusters / band.overlayToday)}) | ${m2(band.totalToday)} | ${m2(band.totalClusterPath)} (${r3(band.totalClusterPath / band.totalToday)}) |`,
      );
    }
  }
  lines.push("");
  return lines.join("\n");
}
