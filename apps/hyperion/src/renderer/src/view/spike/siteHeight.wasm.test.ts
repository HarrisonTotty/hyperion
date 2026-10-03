import { beforeAll, describe, expect, it } from "vitest";

import { norm, sub } from "../../geometry/vec3";
import {
  bakePatch,
  initSync,
  levelTable,
  NormalScale,
  omittedSigmaM,
  Ridges,
  surfaceHeightM,
  VertexPath,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import type { PatchKey } from "../terrain/patchKey";
import { hardPlanet, patchKeyAt, recordProfile, siteHeightOf } from "./demandRecord";
import { datumDirection } from "./descentProfile";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

/** A real bake's lowest and highest height, ridges off, metres. */
function rangeOf(key: PatchKey): readonly [number, number] {
  const patch = bakePatch(
    key.face,
    key.level,
    key.i,
    key.j,
    VertexPath.FaceDifferences,
    NormalScale.Mesh,
    Ridges.Off,
    0,
  );
  try {
    const [low, high] = patch.heightRangeM();
    return [low ?? NaN, high ?? NaN];
  } finally {
    patch.free();
  }
}

describe("the record's measured site height", () => {
  beforeAll(() => {
    initSync({ module: wasmBytes() });
  });

  it("lies within the finest bake's range under the site, about 1.85 km below the datum", () => {
    const table = levelTable(Ridges.Off);
    const siteHeightM = siteHeightOf(recordProfile(), {
      surfaceHeightM: ([x, y, z]) => surfaceHeightM(x, y, z, Ridges.Off),
    });
    const site = recordProfile().siteDir;
    const finest = hardPlanet(table).finestLevel;
    const [low, high] = rangeOf(patchKeyAt([site.x, site.y, site.z], finest));
    expect(siteHeightM).toBeGreaterThanOrEqual(low);
    expect(siteHeightM).toBeLessThanOrEqual(high);
    // Seed 7's site (lane B's `belowDatum.wasm.test.ts`); the geocentric direction gave −1,953.2 m.
    expect(siteHeightM).toBeLessThan(-1_800);
    expect(siteHeightM).toBeGreaterThan(-1_900);
    expect(omittedSigmaM(finest, Ridges.Off)).toBe(0);
  });

  it("takes the site's direction from the spheroid point, not the geocentric one", () => {
    const flat = recordProfile();
    const ground = flat.poseAt(flat.durationS).groundPointM;
    expect(norm(sub(datumDirection(TEST_PLANET_FIGURE, ground), flat.siteDir))).toBeLessThan(1e-15);
  });
});
