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
  };
}
