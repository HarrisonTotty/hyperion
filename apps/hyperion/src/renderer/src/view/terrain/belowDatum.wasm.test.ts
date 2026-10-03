import { beforeAll, describe, expect, it } from "vitest";

import { add, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import {
  bakePatch,
  initSync,
  levelTable,
  NormalScale,
  Ridges,
  VertexPath,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import type { Quaternion } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { WGS84_FIGURE } from "../../test/terrainFixtures";
import { DrawSetResolver, PatchCache } from "./cache";
import { vertexDir } from "./cube";
import type { GroundContact } from "./grounded";
import type { PatchKey } from "./patchKey";
import { planetGeometry, type PlanetGeometry, surfacePoint } from "./planet";
import { type Selection, selectPatches } from "./select";
import { slotLayout } from "./slotLayout";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

/** A real bake's height range and the height of its centre vertex, metres. */
function bake(key: PatchKey): { range: readonly [number, number]; centreM: number } {
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
    const heights = patch.heights();
    return { range: [low ?? NaN, high ?? NaN], centreM: heights[2 * (65 * 32 + 32)] ?? NaN };
  } finally {
    patch.free();
  }
}

/**
 * The test planet's ground about 1.85 km below the WGS 84 datum, ridges off: the finest patch
 * under the landing site of the descent's seed 7 (R05.T13.a's probe), whose centre vertex is
 * the ground the camera hovers over.
 */
const SITE: PatchKey = { face: 3, level: 19, i: 330_180, j: 300_910 };

describe("selection with the camera below the datum", () => {
  let planet: PlanetGeometry;
  let groundM: number;
  let ground: Vec3;
  let up: Vec3;
  let east: Vec3;

  beforeAll(() => {
    initSync({ module: wasmBytes() });
    planet = planetGeometry(WGS84_FIGURE, levelTable(Ridges.Off));
    groundM = bake(SITE).centreM;
    ground = vec3(...surfacePoint(WGS84_FIGURE, vertexDir(SITE, 32, 32), groundM));
    up = normalise(ground);
    east = normalise(vec3(-up.y, up.x, 0));
  });

  /** A camera 1.6 m above the ground, looking `tiltRad` from straight down towards the east. */
  function camera(tiltRad: number): { positionM: Vec3; orientation: Quaternion } {
    const forward = normalise(add(scale(up, -Math.cos(tiltRad)), scale(east, Math.sin(tiltRad))));
    return { positionM: add(ground, scale(up, 1.6)), orientation: lookAlong(forward, up) };
  }

  /**
   * Selects at a hovering pose for `frames` frames through the real cache and draw set, baking
   * every demanded patch at once with the module, as an ideal pool; returns every frame.
   */
  function hover(
    tiltRad: number,
    grounded: ReadonlyArray<GroundContact>,
    frames: number,
  ): Selection[] {
    const cache = new PatchCache(
      slotLayout([{ name: "heights", storage: "storage-buffer", bytes: 100 }], 1962 * 100),
    );
    const resolver = new DrawSetResolver(cache);
    const out: Selection[] = [];
    for (let n = 0; n < frames; n += 1) {
      const selection = selectPatches({
        planet,
        views: [
          {
            camera: camera(tiltRad),
            fovXRad: Math.PI / 3,
            viewport: { widthPx: 1920, heightPx: 1080 },
            weight: 1,
            tauPx: 1,
          },
        ],
        setting: "high",
        grounded,
        heightRanges: cache,
        maxPatches: 981,
      });
      cache.retain(selection, resolver.resolve(selection));
      for (const request of selection.demand) {
        cache.insert({
          key: request.key,
          generation: n,
          originM: vec3(0, 0, 0),
          heightRangeM: bake(request.key).range,
          boundingRadiusM: 1,
        });
      }
      out.push(selection);
    }
    return out;
  }

  it("is about 1.85 km below the datum", () => {
    expect(groundM).toBeLessThan(-1_800);
    expect(groundM).toBeGreaterThan(-1_900);
  });

  it.each([
    ["straight down", 0],
    ["30° below the horizon", Math.PI / 2 - Math.PI / 6],
  ])("never empties as bakes land, looking %s, and reaches the finest level", (_, tilt) => {
    const frames = hover(tilt, [], 24);
    expect(frames.every((s) => s.patches.size > 0)).toBe(true);
    const last = frames.at(-1);
    const deepest = Math.max(...[...(last?.patches.values() ?? [])].map((p) => p.key.level));
    expect(deepest).toBe(planet.finestLevel);
  });

  it("selects the ground below it without baked ranges", () => {
    const selection = selectPatches({
      planet,
      views: [
        {
          camera: camera(0),
          fovXRad: Math.PI / 3,
          viewport: { widthPx: 1920, heightPx: 1080 },
          weight: 1,
          tauPx: 1,
        },
      ],
      setting: "high",
      grounded: [{ positionM: ground, radiusM: 10 }],
      maxPatches: 981,
    });
    expect(selection.patches.size).toBeGreaterThan(0);
    expect([...selection.patches.values()].some((p) => p.forced)).toBe(true);
  });

  it("forces the ground under a contact at every frame, looking away from it", () => {
    // Looking up at the sky, no view sees the ground; the contact still forces its region.
    const frames = hover(Math.PI, [{ positionM: ground, radiusM: 10 }], 24);
    for (const s of frames) {
      expect([...s.patches.values()].some((p) => p.forced)).toBe(true);
    }
  });
});
