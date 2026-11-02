/**
 * The terrain's geometry in the descent spike's results file (plan R14, R14.T1.f; results versions
 * 7 and 8): the renderer's `geometry` block, a generated world's main view's lit pass by segment,
 * read from the report, checked in the file and summarised.
 *
 * @remarks
 * The block states how the pass drew its geometry, the setting's `GeometrySettings` or a launch's
 * `--geometry` over them, and for each of the descent's segments the median and the largest of its
 * frames' interior and edge patches, drawn triangles (the interior mesh's, and the lit pass's
 * clusters' as drawn, R14.T2.a), four cascades' caster triangles summed and
 * overlay-depth triangles (`TerrainPass.counts`, its cluster counters on), each `null` where no
 * frame of the segment had it. A version 6 file, R05's runs' and the native replay's
 * (`tools/gpu-replay`), holds none and reads as before; a version 7 or 8 file holds one only over
 * a generated world. Version 8's settings add R14.T6.b's `farCascadeRate`; version 7's, from builds
 * that rendered every cascade every frame, read as rate 1. A file written before a consumer's lever
 * (plan R14's T4) lacks its field, which reads as `false`: that run drew the consumer by its own
 * draw.
 */

import type { SpikeCountFigures, SpikeGeometryReport, SpikeGeometrySegment } from "../preload/api";

/** The block's counts by segment, in the report's and the summary's order. */
const COUNTS = [
  "interiorPatches",
  "edgePatches",
  "drawnTriangles",
  "litClusterTriangles",
  "casterTriangles",
  "overlayTriangles",
] as const;

/** Each count's column in the summary. */
const COUNT_HEADS: Readonly<Record<(typeof COUNTS)[number], string>> = {
  interiorPatches: "Interior patches",
  edgePatches: "Edge patches",
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

/** The consumers' levers (plan R14's T4), which a file written before each lacks. */
const CONSUMER_LEVERS = ["figureClusters"] as const;

/**
 * `value` as the geometry's settings at results `version`, every field of its kind, or `null`:
 * version 8's six fields, or version 7's five, its far cascades read at rate 1, and the consumers'
 * levers it holds, each absent one read as off.
 */
function settingsOf(value: unknown, version: 7 | 8): SpikeGeometryReport["settings"] | null {
  if (!isRecord(value)) {
    return null;
  }
  const levers = CONSUMER_LEVERS.filter((lever) => lever in value).length;
  if (Object.keys(value).length !== (version === 7 ? 5 : 6) + levers) {
    return null;
  }
  const { clusterPath, casterDepthCull, casterStrideCap, overlayDepth, litClusters } = value;
  const farCascadeRate = version === 7 ? 1 : value["farCascadeRate"];
  const { figureClusters = false } = value;
  if (
    (farCascadeRate !== 1 && farCascadeRate !== 2 && farCascadeRate !== 4) ||
    typeof clusterPath !== "boolean" ||
    typeof casterDepthCull !== "boolean" ||
    typeof litClusters !== "boolean" ||
    typeof figureClusters !== "boolean" ||
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
  };
}

/** `value` as one segment's counts, or `null`. */
function segmentOf(value: unknown): SpikeGeometrySegment | null {
  if (!isRecord(value) || Object.keys(value).length !== COUNTS.length + 1) {
    return null;
  }
  const segment = value["segment"];
  if (typeof segment !== "string" || segment.length === 0) {
    return null;
  }
  const read = COUNTS.map((name) => countFigures(value[name]));
  const [
    interiorPatches,
    edgePatches,
    drawnTriangles,
    litClusterTriangles,
    casterTriangles,
    overlayTriangles,
  ] = read;
  if (
    interiorPatches === undefined ||
    edgePatches === undefined ||
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

/** The summary's "Geometry" section of `geometry` (plan R14, R14.T1.f). */
export function geometrySummaryLines(geometry: SpikeGeometryReport): string[] {
  const {
    clusterPath,
    casterDepthCull,
    casterStrideCap,
    overlayDepth,
    litClusters,
    farCascadeRate,
    figureClusters,
  } = geometry.settings;
  return [
    "",
    "## Geometry (plan R14)",
    "",
    `The main view's lit pass a frame by segment, median (largest): cluster path ${clusterPath ? "on" : "off (the way back)"}, casters culled by their receivers' depths ${casterDepthCull ? "on" : "off"}, caster stride cap ${String(casterStrideCap)}, overlay depth ${overlayDepth}, the lit pass's interior patches by ${litClusters ? "clusters" : "the interior mesh"}, far cascades ${farCascadeRate === 1 ? "every frame" : `every ${String(farCascadeRate)} frames`}, the mesh bodies' figures by ${figureClusters ? "clusters" : "their instanced draws"}.`,
    "",
    `| Segment | ${COUNTS.map((name) => COUNT_HEADS[name]).join(" | ")} |`,
    `| ${["---", ...COUNTS.map(() => "---")].join(" | ")} |`,
    ...geometry.segments.map(
      (segment) =>
        `| ${segment.segment} | ${COUNTS.map((name) => cell(segment[name])).join(" | ")} |`,
    ),
  ];
}
