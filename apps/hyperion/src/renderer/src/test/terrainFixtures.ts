import levelTableGolden from "../../../../../../crates/hyperion-surface/tests/golden/level_table.golden?raw";
import { vec3 } from "../geometry/vec3";
import type { BodyFigure } from "../view/terrain/planet";
import type { PatchBounds } from "../view/terrain/bounds";
import { type PatchKey, patchKeyString } from "../view/terrain/patchKey";
import type { SelectedPatch, Selection } from "../view/terrain/select";

/** A placeholder bounding volume, a metre round the origin, for tests that never read bounds. */
export const UNIT_BOUNDS: PatchBounds = {
  centre: vec3(0, 0, 0),
  radiusM: 1,
  minHeightM: 0,
  maxHeightM: 0,
  box: {
    centre: vec3(0, 0, 0),
    axes: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)],
    halfExtentsM: [1, 1, 1],
  },
};

/** WGS 84's figure, the test planet's datum (NIMA TR8350.2): a = 6,378,137 m, f = 1 ÷ 298.257223563. */
export const WGS84_FIGURE: BodyFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257_223_563),
  pole: null,
};

/**
 * The test planet's level table as `hyperion-surface`'s `level_table.golden` pins it, without or
 * with ridges: the module's `levelTable()`, bit for bit.
 */
export function goldenLevelTable(ridges: "off" | "on"): Float64Array {
  const view = new DataView(new ArrayBuffer(8));
  const values: number[] = [];
  for (const line of levelTableGolden.split("\n")) {
    const fields = line.split(" ");
    if (fields[0] === "level" && fields[1] === ridges) {
      for (const hex of fields.slice(3)) {
        view.setBigUint64(0, BigInt(hex));
        values.push(view.getFloat64(0));
      }
    }
  }
  return Float64Array.from(values);
}

/** A selection of `keys`, those in `forced` marked forced, with no demand. */
export function selectionOf(
  keys: readonly PatchKey[],
  forced: readonly PatchKey[] = [],
): Selection {
  const forcedStrings = new Set(forced.map(patchKeyString));
  const patches = new Map<string, SelectedPatch>();
  for (const key of keys) {
    const keyString = patchKeyString(key);
    patches.set(keyString, {
      key,
      bounds: UNIT_BOUNDS,
      forced: forcedStrings.has(keyString),
      seen: true,
    });
  }
  return { patches, demand: [], limited: false };
}
