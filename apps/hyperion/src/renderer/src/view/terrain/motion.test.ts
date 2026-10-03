import { describe, expect, it } from "vitest";

import { add, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { lookAlong } from "../camera/quaternion";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { DrawSetResolver, PatchCache } from "./cache";
import { vertexDir } from "./cube";
import type { PatchKey } from "./patchKey";
import { levelBoundM, planetGeometry, surfacePoint } from "./planet";
import { selectPatches } from "./select";
import { slotLayout } from "./slotLayout";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const SITE: PatchKey = { face: 0, level: 19, i: 300_001, j: 200_003 };
const RATE_HZ = 64;
const BUDGET = 981;

/**
 * A stand-in for a bake's height range: ±(100 m + 2 ε_n), within the level's own range and tight
 * enough that a baked patch's bounds tighten as a real bake's do.
 */
function syntheticRange(key: PatchKey): readonly [number, number] {
  const half = 100 + 2 * levelBoundM(PLANET, key.level);
  return [-half, half];
}

/** One frame of a flight. */
interface FlightFrame {
  readonly patches: number;
  /** Keys selected this frame that the last frame did not select. */
  readonly entered: number;
}

/**
 * Flies a camera 300 m above the site at 300 m/s, looking 20° below the horizon ahead, through
 * selection (high setting, 1080p, τ = 1 px, the patch budget), the real patch cache and its draw
 * set at 64 Hz, baking up to `bakesPerFrame` demanded patches a frame, for `seconds`.
 */
function fly(slots: number, bakesPerFrame: number, seconds: number): FlightFrame[] {
  const cache = new PatchCache(
    slotLayout([{ name: "heights", storage: "storage-buffer", bytes: 100 }], slots * 100),
  );
  const resolver = new DrawSetResolver(cache);
  const start = vec3(...surfacePoint(WGS84_FIGURE, vertexDir(SITE, 32, 32), 0));
  const up = normalise(start);
  const east = normalise(vec3(-up.y, up.x, 0));
  const forward = normalise(add(scale(east, Math.cos(0.35)), scale(up, -Math.sin(0.35))));
  const frames: FlightFrame[] = [];
  let last = new Set<string>();
  for (let step = 0; step < seconds * RATE_HZ; step += 1) {
    const ground: Vec3 = add(start, scale(east, (300 * step) / RATE_HZ));
    const selection = selectPatches({
      planet: PLANET,
      views: [
        {
          camera: { positionM: add(ground, scale(up, 300)), orientation: lookAlong(forward, up) },
          fovXRad: Math.PI / 3,
          viewport: { widthPx: 1920, heightPx: 1080 },
          weight: 1,
          tauPx: 1,
        },
      ],
      setting: "high",
      grounded: [],
      heightRanges: cache,
      maxPatches: BUDGET,
    });
    cache.retain(selection, resolver.resolve(selection));
    for (const request of selection.demand.slice(0, bakesPerFrame)) {
      cache.insert({
        key: request.key,
        generation: step,
        originM: vec3(0, 0, 0),
        heightRangeM: syntheticRange(request.key),
        boundingRadiusM: 1,
      });
    }
    const now = new Set(selection.patches.keys());
    let entered = 0;
    for (const k of now) {
      if (!last.has(k)) {
        entered += 1;
      }
    }
    last = now;
    frames.push({ patches: now.size, entered });
  }
  return frames;
}

describe("selection in steady flight", () => {
  it("changes a few patches a frame once streamed in, not the whole set", () => {
    // Eight bakes a frame, 512 a second, as a pool of a few workers might; the first two seconds
    // stream the view in.
    const steady = fly(2 * BUDGET, 8, 3.5).slice(2 * RATE_HZ);
    const entered = steady.map((f) => f.entered);
    const mean = entered.reduce((s, n) => s + n, 0) / entered.length;
    // At 300 m/s a frame moves the camera 4.7 m, a quarter of a finest patch.
    expect(mean).toBeLessThan(0.03 * BUDGET);
    expect(Math.max(...entered)).toBeLessThan(0.1 * BUDGET);
    expect(Math.min(...steady.map((f) => f.patches))).toBeGreaterThan(0.9 * BUDGET);
  });

  it("does not collapse when the slots barely hold the selection and its ancestors", () => {
    const steady = fly(1100, 200, 2.5).slice(Math.round(1.5 * RATE_HZ));
    expect(Math.min(...steady.map((f) => f.patches))).toBeGreaterThan(0.9 * BUDGET);
  });
});
