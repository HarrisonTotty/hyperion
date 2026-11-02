/**
 * The smoke page's geometry override (`just test-render --geometry=<field>=<value>[,…]`, plan R14,
 * Design note 17): the fields the harness's main process read and checked from
 * `--smoke-geometry=`, each in the page's query as `geometry-<field>`, which the page's lit terrain
 * passes draw with over their setting's geometry, so that a run of the terrain's groups shows each
 * lever, the way back among them (`clusterPath=false`), from one build. The checks that draw a
 * frame both ways set each side's geometry themselves.
 */

import {
  geometryOf,
  type GeometrySettings,
  type QualitySetting,
} from "../view/quality/qualitySetting";
import { queryOverride } from "./geometryQuery";

/** The page's override, read once from its query. */
const OVERRIDE = queryOverride(new URLSearchParams(window.location.search));

/** `setting`'s geometry with the page's override over it: the setting's own without one. */
export function smokeGeometry(setting: QualitySetting): GeometrySettings {
  return geometryOf(setting, OVERRIDE);
}
