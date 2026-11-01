/**
 * The smoke page's geometry override (`just test-render --geometry=<field>=<value>[,…]`, plan R14,
 * Design note 17): the fields the harness's main process read and checked from
 * `--smoke-geometry=`, each in the page's query as `geometry-<field>`, which the page's lit terrain
 * passes draw with over their setting's geometry, so that a run of the terrain's groups shows each
 * lever, the way back among them (`clusterPath=false`), from one build. The checks that draw a
 * frame both ways set each side's geometry themselves.
 */

import type { GeometryOverride } from "../../../preload/api";
import {
  geometryOf,
  type GeometrySettings,
  type QualitySetting,
} from "../view/quality/qualitySetting";

/** The query's `geometry-<field>` values as an override, each read as its field's values are. */
function queryOverride(parameters: URLSearchParams): GeometryOverride {
  const flag = (name: string): boolean | undefined => {
    const value = parameters.get(`geometry-${name}`);
    return value === "true" ? true : value === "false" ? false : undefined;
  };
  const clusterPath = flag("clusterPath");
  const casterDepthCull = flag("casterDepthCull");
  const cap = parameters.get("geometry-casterStrideCap");
  const casterStrideCap =
    cap === "1" ? 1 : cap === "2" ? 2 : cap === "4" ? 4 : cap === "8" ? 8 : undefined;
  const depth = parameters.get("geometry-overlayDepth");
  const overlayDepth =
    depth === "redraw" || depth === "scene-depth" || depth === "cluster" ? depth : undefined;
  return {
    ...(clusterPath === undefined ? {} : { clusterPath }),
    ...(casterDepthCull === undefined ? {} : { casterDepthCull }),
    ...(casterStrideCap === undefined ? {} : { casterStrideCap }),
    ...(overlayDepth === undefined ? {} : { overlayDepth }),
  };
}

/** The page's override, read once from its query. */
const OVERRIDE = queryOverride(new URLSearchParams(window.location.search));

/** `setting`'s geometry with the page's override over it: the setting's own without one. */
export function smokeGeometry(setting: QualitySetting): GeometrySettings {
  return geometryOf(setting, OVERRIDE);
}
