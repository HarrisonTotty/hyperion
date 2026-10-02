/**
 * The interim star field (plan R02, R02.T16; Design note 19): the stars of four range queries about
 * the scene's system, with their absolute V magnitudes, until R06's sky replaces them.
 */

import type {
  GalacticPosition,
  MassLayer,
  RequestOf,
  SystemIdHex,
  SystemRecord,
  SystemsInRange,
  UniverseIdHex,
  UniverseTime,
} from "@hyperion/protocol";

import { formatNumber } from "../../lib/format";
import { norm, scale } from "../../geometry/vec3";
import { galacticDeltaM } from "../coords/position";
import type { ViewStar } from "../scene/model";

/**
 * The server's `MAX_CENSUS_LIMIT` (`crates/hyperion-server/src/limits.rs`), which every interim
 * request sets: the field is required on the wire, and the census's own default of 4,096 would
 * void the radii below.
 */
export const INTERIM_LIMIT = 20_000;

/** One of the four queries: its floor layer and its radius, ly. */
export interface InterimQuery {
  readonly layer: Extract<MassLayer, "a" | "b" | "c" | "d" | "e">;
  readonly radiusLy: number;
}

/**
 * The four queries, about a tenth under the largest complete radii measured near the Sun at
 * generator version 15 (Design note 19): `e` to 620 ly, `d` to 360, `c` to 210 and `a` to 60, the
 * last reaching layer B's K dwarfs and young T Tauri stars.
 */
export const INTERIM_QUERIES: ReadonlyArray<InterimQuery> = [
  { layer: "e", radiusLy: 620 },
  { layer: "d", radiusLy: 360 },
  { layer: "c", radiusLy: 210 },
  { layer: "a", radiusLy: 60 },
];

/** The share of the limit a retried query aims its expected count at: 0.9 (Design note 19). */
export const RETRY_FILL = 0.9;

/** A query's request: its floor, its radius, the briefs, and the server's limit. */
export function interimRequest(
  universe: UniverseIdHex,
  centre: GalacticPosition,
  time: UniverseTime,
  query: InterimQuery,
): RequestOf<"systems_in_range"> {
  return {
    kind: "systems_in_range",
    universe,
    centre,
    radius_ly: query.radiusLy,
    time,
    min_layer: query.layer,
    limit: INTERIM_LIMIT,
    include_stellar: true,
  };
}

/** The stellar layers from the heaviest, as the census adds them. */
const HEAVIEST_FIRST: ReadonlyArray<MassLayer> = ["e", "d", "c", "b", "a"];

/**
 * The radius to ask again at where the answer left its own floor out over the limit, or `null`
 * where the floor is in: r × (0.9 × limit ÷ Σ expected)^⅓, the expected counts summed from E down
 * to the floor, since the census adds layers from E and drops the one that would pass the limit
 * together with every lighter one (Design note 19).
 */
export function retryRadiusLy(answer: SystemsInRange, query: InterimQuery): number | null {
  const floor = answer.census.layers.find((layer) => layer.layer === query.layer);
  if (floor?.status !== "over_limit") {
    return null;
  }
  const down = new Set(HEAVIEST_FIRST.slice(0, HEAVIEST_FIRST.indexOf(query.layer) + 1));
  const expected = answer.census.layers
    .filter((layer) => down.has(layer.layer))
    .reduce((sum, layer) => sum + layer.expected, 0);
  if (!(expected > 0)) {
    return null;
  }
  return query.radiusLy * Math.cbrt((RETRY_FILL * answer.census.limit) / expected);
}

/** The merged field: the stars drawn, and the rows left out for want of a magnitude. */
export interface InterimField {
  readonly stars: ReadonlyArray<ViewStar>;
  readonly withoutV: number;
}

/**
 * The answers merged by system ID, one row per system, the scene's own system left out, and each
 * row with an absolute V magnitude made a star by its direction and distance from the scene's
 * barycentre; a row without one (white dwarfs and other remnants, most of layer E) is counted, not
 * drawn. Positions are present, not retarded, and no companion's light is added (Design note 19).
 */
export function interimField(
  answers: ReadonlyArray<SystemsInRange>,
  barycentre: GalacticPosition,
  ownSystem: SystemIdHex | null,
): InterimField {
  const rows = new Map<SystemIdHex, SystemRecord>();
  for (const answer of answers) {
    for (const row of answer.systems) {
      if (row.id !== ownSystem && !rows.has(row.id)) {
        rows.set(row.id, row);
      }
    }
  }
  const stars: ViewStar[] = [];
  let withoutV = 0;
  for (const row of rows.values()) {
    const absoluteV = row.stellar?.absolute_v_mag;
    const offsetM = galacticDeltaM(barycentre, row.position);
    const distanceM = norm(offsetM);
    if (absoluteV === undefined) {
      withoutV += 1;
      continue;
    }
    if (!(distanceM > 0)) {
      continue;
    }
    stars.push({
      id: row.id,
      direction: scale(offsetM, 1 / distanceM),
      distanceM,
      absoluteV,
      tEffK: row.stellar?.teff_k ?? null,
    });
  }
  return { stars, withoutV };
}

/**
 * The count line (R02.T16.b): `STARS 1,234 DRAWN · 567 WITHOUT V · RADII 620/360/210/60 ly`, the
 * radii those used, in the order E, D, C, A.
 */
export function interimCountLine(field: InterimField, radiiLy: ReadonlyArray<number>): string {
  const radii = radiiLy.map((radius) => formatNumber(radius, 0)).join("/");
  return `STARS ${formatNumber(field.stars.length, 0)} DRAWN · ${formatNumber(field.withoutV, 0)} WITHOUT V · RADII ${radii} ly`;
}
