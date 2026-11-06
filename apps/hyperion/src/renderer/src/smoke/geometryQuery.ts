/**
 * The smoke page's geometry override read from its query (plan R14, Design note 17; `geometry.ts`):
 * each `geometry-<field>` the harness's main process checked from `--smoke-geometry=`, read as its
 * field's values are, and any other value left out.
 */

import type { GeometryOverride } from "../../../preload/api";

/** The query's `geometry-<field>` values as an override, each read as its field's values are. */
export function queryOverride(parameters: URLSearchParams): GeometryOverride {
  const flag = (name: string): boolean | undefined => {
    const value = parameters.get(`geometry-${name}`);
    return value === "true" ? true : value === "false" ? false : undefined;
  };
  const clusterPath = flag("clusterPath");
  const casterDepthCull = flag("casterDepthCull");
  const litClusters = flag("litClusters");
  const figureClusters = flag("figureClusters");
  const hullClusters = flag("hullClusters");
  const wireClusters = flag("wireClusters");
  const finClusters = flag("finClusters");
  const cap = parameters.get("geometry-casterStrideCap");
  const casterStrideCap =
    cap === "1" ? 1 : cap === "2" ? 2 : cap === "4" ? 4 : cap === "8" ? 8 : undefined;
  const depth = parameters.get("geometry-overlayDepth");
  const overlayDepth =
    depth === "redraw" || depth === "scene-depth" || depth === "cluster" ? depth : undefined;
  const strideText = parameters.get("geometry-strides");
  const strides = strideText === "off" || strideText === "measured" ? strideText : undefined;
  const occlusionText = parameters.get("geometry-occlusion");
  const occlusion = occlusionText === "off" || occlusionText === "hzb" ? occlusionText : undefined;
  const shadingText = parameters.get("geometry-shading");
  const shading =
    shadingText === "forward" || shadingText === "visibility" ? shadingText : undefined;
  const budgetText = parameters.get("geometry-terrainBudget");
  const terrainBudget =
    budgetText === "patches" || budgetText === "triangles" ? budgetText : undefined;
  const standInText = parameters.get("geometry-standInStrides");
  const standInStrides =
    standInText === "off" || standInText === "own-grids" || standInText === "all"
      ? standInText
      : undefined;
  const hiddenText = parameters.get("geometry-hiddenMarks");
  const hiddenMarks = hiddenText === "drawn" || hiddenText === "culled" ? hiddenText : undefined;
  const edgeText = parameters.get("geometry-edgeStandIns");
  const edgeStandIns =
    edgeText === "off" || edgeText === "surveyed" || edgeText === "masks" ? edgeText : undefined;
  const cellsText = parameters.get("geometry-discCells");
  const discCells = cellsText === "per-cell" || cellsText === "packed" ? cellsText : undefined;
  const figureText = parameters.get("geometry-figureShading");
  const figureShading =
    figureText === "forward" || figureText === "screen" ? figureText : undefined;
  const rate = parameters.get("geometry-farCascadeRate");
  const farCascadeRate = rate === "1" ? 1 : rate === "2" ? 2 : rate === "4" ? 4 : undefined;
  const toleranceText = /^(\d+(?:\.\d+)?):(\d+(?:\.\d+)?)$/.exec(
    parameters.get("geometry-meshTolerancePx") ?? "",
  );
  const tolerance =
    toleranceText === null
      ? null
      : { camera: Number(toleranceText[1]), wireframeOccluder: Number(toleranceText[2]) };
  const pool = parameters.get("geometry-pageBytes");
  const pageBytes = pool !== null && /^\d+$/.test(pool) ? Number(pool) : undefined;
  return {
    ...(clusterPath === undefined ? {} : { clusterPath }),
    ...(casterDepthCull === undefined ? {} : { casterDepthCull }),
    ...(casterStrideCap === undefined ? {} : { casterStrideCap }),
    ...(overlayDepth === undefined ? {} : { overlayDepth }),
    ...(litClusters === undefined ? {} : { litClusters }),
    ...(farCascadeRate === undefined ? {} : { farCascadeRate }),
    ...(figureClusters === undefined ? {} : { figureClusters }),
    ...(hullClusters === undefined ? {} : { hullClusters }),
    ...(wireClusters === undefined ? {} : { wireClusters }),
    ...(pageBytes === undefined || !Number.isSafeInteger(pageBytes) ? {} : { pageBytes }),
    ...(tolerance === null ? {} : { meshTolerancePx: tolerance }),
    ...(finClusters === undefined ? {} : { finClusters }),
    ...(strides === undefined ? {} : { strides }),
    ...(occlusion === undefined ? {} : { occlusion }),
    ...(shading === undefined ? {} : { shading }),
    ...(terrainBudget === undefined ? {} : { terrainBudget }),
    ...(standInStrides === undefined ? {} : { standInStrides }),
    ...(hiddenMarks === undefined ? {} : { hiddenMarks }),
    ...(edgeStandIns === undefined ? {} : { edgeStandIns }),
    ...(discCells === undefined ? {} : { discCells }),
    ...(figureShading === undefined ? {} : { figureShading }),
  };
}
