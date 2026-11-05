/**
 * The terrain's geometry in the descent spike's results file (plan R14, R14.T1.f; results versions
 * 7 and 8): the renderer's `geometry` block, a generated world's main view's lit pass by segment,
 * read from the report, checked in the file and summarised.
 *
 * @remarks
 * The block states how the pass drew its geometry, the setting's `GeometrySettings` or a launch's
 * `--geometry` over them, and for each of the descent's segments the median and the largest of its
 * frames' interior and edge patches, the edge patches the cluster path drew (R14.T1.g; `null` in a
 * file written before it), drawn triangles (the interior mesh's, and the lit pass's
 * clusters' as drawn, R14.T2.a), four cascades' caster triangles summed and
 * overlay-depth triangles (`TerrainPass.counts`, its cluster counters on), each `null` where no
 * frame of the segment had it. A version 6 file, R05's runs' and the native replay's
 * (`tools/gpu-replay`), holds none and reads as before; a version 7 or 8 file holds one only over
 * a generated world. Version 8's settings add R14.T6.b's `farCascadeRate`; version 7's, from builds
 * that rendered every cascade every frame, read as rate 1. A file written before a consumer's lever
 * (plan R14's T4), or before the edge patches' (R14.T1.g), lacks its field, which reads as
 * `false`: that run drew the consumer, or the edge patches, by its own draw.
 */

import type { SpikeCountFigures, SpikeGeometryReport, SpikeGeometrySegment } from "../preload/api";

/** The block's counts by segment, in the report's and the summary's order. */
const COUNTS = [
  "interiorPatches",
  "edgePatches",
  "finPatches",
  "drawnTriangles",
  "litClusterTriangles",
  "casterTriangles",
  "overlayTriangles",
] as const;

/** Each count's column in the summary. */
const COUNT_HEADS: Readonly<Record<(typeof COUNTS)[number], string>> = {
  interiorPatches: "Interior patches",
  edgePatches: "Edge patches",
  finPatches: "Edge patches by clusters",
  drawnTriangles: "Drawn triangles",
  litClusterTriangles: "Drawn by clusters",
  casterTriangles: "Caster triangles (4 cascades)",
  overlayTriangles: "Overlay triangles",
};

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isCount(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}

/** `value` as one count's figures, `null` for the `null` of no frame, `undefined` if neither. */
function countFigures(value: unknown): SpikeCountFigures | null | undefined {
  if (value === null) {
    return null;
  }
  if (!isRecord(value) || Object.keys(value).length !== 3) {
    return undefined;
  }
  const { frames, p50, max } = value;
  return isCount(frames) &&
    frames > 0 &&
    typeof p50 === "number" &&
    Number.isFinite(p50) &&
    p50 >= 0 &&
    isCount(max) &&
    p50 <= max
    ? { frames, p50, max }
    : undefined;
}

/**
 * The fields a file written before each lacks: the consumers' levers (plan R14's T4) and the edge
 * patches' (R14.T1.g), each absent one read as off, the mesh page pool's bytes (T5.b), absent read
 * as 0, no pool, the meshes' tolerance (T5.f), absent read as 0 px, the finest cut, the camera
 * passes' strides (T3.c), absent read as `off`, the two-pass occlusion (T3.d), absent read as
 * `off`, and the lit pass's shading (T2), absent read as `forward`.
 */
const LATER_FIELDS = [
  "figureClusters",
  "hullClusters",
  "wireClusters",
  "pageBytes",
  "meshTolerancePx",
  "finClusters",
  "strides",
  "occlusion",
  "shading",
] as const;

/** `value` as a mesh tolerance, two numbers of at least 0, px, or `null`. */
function meshToleranceOf(
  value: unknown,
): SpikeGeometryReport["settings"]["meshTolerancePx"] | null {
  if (!isRecord(value) || Object.keys(value).length !== 2) {
    return null;
  }
  const { camera, wireframeOccluder } = value;
  return typeof camera === "number" &&
    typeof wireframeOccluder === "number" &&
    Number.isFinite(camera) &&
    Number.isFinite(wireframeOccluder) &&
    camera >= 0 &&
    wireframeOccluder >= 0
    ? { camera, wireframeOccluder }
    : null;
}

/**
 * `value` as the geometry's settings at results `version`, every field of its kind, or `null`:
 * version 8's six fields, or version 7's five, its far cascades read at rate 1, and the later
 * fields it holds ({@link LATER_FIELDS}).
 */
function settingsOf(value: unknown, version: 7 | 8): SpikeGeometryReport["settings"] | null {
  if (!isRecord(value)) {
    return null;
  }
  const later = LATER_FIELDS.filter((field) => field in value).length;
  if (Object.keys(value).length !== (version === 7 ? 5 : 6) + later) {
    return null;
  }
  const { clusterPath, casterDepthCull, casterStrideCap, overlayDepth, litClusters } = value;
  const farCascadeRate = version === 7 ? 1 : value["farCascadeRate"];
  const {
    figureClusters = false,
    hullClusters = false,
    wireClusters = false,
    pageBytes = 0,
    meshTolerancePx: tolerance = { camera: 0, wireframeOccluder: 0 },
    finClusters = false,
    strides = "off",
    occlusion = "off",
    shading = "forward",
  } = value;
  const meshTolerancePx = meshToleranceOf(tolerance);
  if (
    (farCascadeRate !== 1 && farCascadeRate !== 2 && farCascadeRate !== 4) ||
    typeof clusterPath !== "boolean" ||
    typeof casterDepthCull !== "boolean" ||
    typeof litClusters !== "boolean" ||
    typeof figureClusters !== "boolean" ||
    typeof hullClusters !== "boolean" ||
    typeof wireClusters !== "boolean" ||
    !isCount(pageBytes) ||
    meshTolerancePx === null ||
    typeof finClusters !== "boolean" ||
    (strides !== "off" && strides !== "measured") ||
    (occlusion !== "off" && occlusion !== "hzb") ||
    (shading !== "forward" && shading !== "visibility") ||
    (casterStrideCap !== 1 &&
      casterStrideCap !== 2 &&
      casterStrideCap !== 4 &&
      casterStrideCap !== 8) ||
    (overlayDepth !== "redraw" && overlayDepth !== "scene-depth" && overlayDepth !== "cluster")
  ) {
    return null;
  }
  return {
    clusterPath,
    casterDepthCull,
    casterStrideCap,
    overlayDepth,
    litClusters,
    farCascadeRate,
    figureClusters,
    hullClusters,
    wireClusters,
    pageBytes,
    meshTolerancePx,
    finClusters,
    strides,
    occlusion,
    shading,
  };
}

/**
 * `value` as one segment's counts, or `null`: every count, but the edge patches by clusters (plan
 * R14, R14.T1.g), which a file written before them lacks and which then reads as `null`.
 */
function segmentOf(value: unknown): SpikeGeometrySegment | null {
  if (
    !isRecord(value) ||
    Object.keys(value).length !== COUNTS.length + ("finPatches" in value ? 1 : 0)
  ) {
    return null;
  }
  const segment = value["segment"];
  if (typeof segment !== "string" || segment.length === 0) {
    return null;
  }
  const read = COUNTS.map((name) =>
    name === "finPatches" && !(name in value) ? null : countFigures(value[name]),
  );
  const [
    interiorPatches,
    edgePatches,
    finPatches,
    drawnTriangles,
    litClusterTriangles,
    casterTriangles,
    overlayTriangles,
  ] = read;
  if (
    interiorPatches === undefined ||
    edgePatches === undefined ||
    finPatches === undefined ||
    drawnTriangles === undefined ||
    litClusterTriangles === undefined ||
    casterTriangles === undefined ||
    overlayTriangles === undefined
  ) {
    return null;
  }
  return {
    segment,
    interiorPatches,
    edgePatches,
    finPatches,
    drawnTriangles,
    litClusterTriangles,
    casterTriangles,
    overlayTriangles,
  };
}

/**
 * `value` as the renderer's `geometry` block (the report's, or a file's at results `version`), or
 * `null` where it is not one: exactly its fields, every figure of its kind.
 */
export function readGeometryReport(value: unknown, version: 7 | 8 = 8): SpikeGeometryReport | null {
  if (!isRecord(value) || Object.keys(value).length !== 2) {
    return null;
  }
  const settings = settingsOf(value["settings"], version);
  const segments = value["segments"];
  if (settings === null || !Array.isArray(segments)) {
    return null;
  }
  const read = segments.map(segmentOf);
  const kept = read.filter((each) => each !== null);
  return kept.length === read.length ? { settings, segments: kept } : null;
}

/**
 * The problems of a results file's `geometry` block at `version` (`validateResults`): none where a
 * version 7 or 8 file holds a well-formed one of its version or none, or a version 6 file none.
 */
export function geometryResultsProblems(value: unknown, version: unknown): string[] {
  if (value === undefined) {
    return [];
  }
  if (version !== 7 && version !== 8) {
    return ["geometry is a version 7 or 8 file's, not this version's"];
  }
  return readGeometryReport(value, version) === null
    ? ["geometry is not the settings and each segment's counts, every figure of its kind"]
    : [];
}

/** A whole number with thousands separated by commas. */
function whole(n: number): string {
  return Math.round(n).toLocaleString("en-US");
}

/** A count's figures in a summary's cell: its median and its largest, or `—`. */
function cell(figures: SpikeCountFigures | null): string {
  return figures === null ? "—" : `${whole(figures.p50)} (${whole(figures.max)})`;
}

/** How the pass drew its geometry, in the summary's words, its levers in their order. */
function settingsWords(settings: SpikeGeometryReport["settings"]): string {
  const {
    clusterPath,
    casterDepthCull,
    casterStrideCap,
    overlayDepth,
    litClusters,
    farCascadeRate,
    figureClusters,
    hullClusters,
    wireClusters,
    pageBytes,
    meshTolerancePx,
    finClusters,
    strides,
    occlusion,
    shading,
  } = settings;
  return `cluster path ${clusterPath ? "on" : "off (the way back)"}, casters culled by their receivers' depths ${casterDepthCull ? "on" : "off"}, caster stride cap ${String(casterStrideCap)}, overlay depth ${overlayDepth}, the lit pass's interior patches by ${litClusters ? "clusters" : "the interior mesh"}, far cascades ${farCascadeRate === 1 ? "every frame" : `every ${String(farCascadeRate)} frames`}, the mesh bodies' figures by ${figureClusters ? "clusters" : "their instanced draws"}, the hulls' faces by ${hullClusters ? "clusters" : "their instanced draws"}, the wireframe's interior patches by ${wireClusters ? "clusters" : "the lit mesh"}, ${pageBytes === 0 ? "no mesh page pool" : `the mesh page pool's bytes ${String(pageBytes / 2 ** 20)} MiB`}, meshes cut at ${String(meshTolerancePx.camera)} px in the camera and ${String(meshTolerancePx.wireframeOccluder)} px as the wireframe's occluders, the lit pass's edge patches by ${finClusters ? "clusters" : "the lit mesh"}, the camera's strides ${strides === "measured" ? "by measured error" : "off"}, occlusion ${occlusion === "hzb" ? "two-pass against the depth pyramid" : "off"}, the lit pass's interior patches shaded ${shading === "visibility" ? "by the visibility buffer" : "forward"}`;
}

/** The summary's "Geometry" section of `geometry` (plan R14, R14.T1.f). */
export function geometrySummaryLines(geometry: SpikeGeometryReport): string[] {
  return [
    "",
    "## Geometry (plan R14)",
    "",
    `The main view's lit pass a frame by segment, median (largest): ${settingsWords(geometry.settings)}.`,
    "",
    `| Segment | ${COUNTS.map((name) => COUNT_HEADS[name]).join(" | ")} |`,
    `| ${["---", ...COUNTS.map(() => "---")].join(" | ")} |`,
    ...geometry.segments.map(
      (segment) =>
        `| ${segment.segment} | ${COUNTS.map((name) => cell(segment[name])).join(" | ")} |`,
    ),
  ];
}

/** Whether a frame of `segment` drew a terrain patch. */
function drewTerrain(segment: SpikeGeometrySegment): boolean {
  return (segment.interiorPatches?.max ?? 0) + (segment.edgePatches?.max ?? 0) > 0;
}

/**
 * A smoke's log of its `geometry` block (R05.T18; plan R14, R14.T1.f), which it writes in no file:
 * the summary's section where a segment sampled drawn terrain; where none did, the settings and
 * that no segment sampled drawn terrain, with the frames each segment had, rather than a table of
 * zeros that no draw measured.
 *
 * @remarks
 * A smoke over TEST MOON on SwiftShader ends 85 to 95 s into its script, early in the descent arc,
 * where its lit pass selects no patch: the pass is made at about 82 s, once its photorealistic
 * pipelines are, and selects patches from about 275 km up, 416 s into the script, where the
 * fixture's surveyed cells come into view; its ground elsewhere is bare, never drawn (R10 Design
 * note 12; pod, 2026-11-02).
 */
export function geometrySmokeLines(geometry: SpikeGeometryReport): string[] {
  if (geometry.segments.some(drewTerrain)) {
    return geometrySummaryLines(geometry).filter((line) => line.length > 0);
  }
  const sampled = geometry.segments.flatMap(({ segment, interiorPatches }) =>
    interiorPatches === null ? [] : [`${segment} ${whole(interiorPatches.frames)}`],
  );
  return [
    `Geometry (plan R14): ${settingsWords(geometry.settings)}.`,
    sampled.length === 0
      ? "No segment sampled drawn terrain: no frame had the lit pass's counts."
      : `No segment sampled drawn terrain: the lit pass drew no patch in its frames (${sampled.join(", ")})${sampled.length < geometry.segments.length ? ", and the other segments had none" : ""}.`,
  ];
}
