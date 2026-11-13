import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import { normalsAtlasOf } from "./gpu/resources";
import { type PatchKey, patchKeyString } from "./patchKey";
import { planetGeometry } from "./planet";
import type { PatchRequest, Selection, SelectionInput } from "./select";
import {
  type TerrainFrameInput,
  TerrainPass,
  type TerrainPassObserver,
  type TerrainPassOptions,
  type TerrainPool,
} from "./terrainPass";
import type { BakedPatch, BakeSettings } from "./workers/messages";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/** A pool that records the demand and bakes it flat on the test's command. */
class FlatPool implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  readonly #listeners = new Set<(bake: BakedPatch) => void>();

  constructor(readonly settings: BakeSettings) {}

  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.demand = demand;
  }

  onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.#listeners.add(cb);
    return () => {
      this.#listeners.delete(cb);
    };
  }

  terminate(): void {}

  /** Bakes every patch demanded, flat, for `pass`; returns their keys. */
  bakeDemand(pass: TerrainPass): string[] {
    const n = normalsAtlasOf(pass.layout).samplesPerSide;
    return this.demand.map(({ key }) => {
      const bake: BakedPatch = {
        key,
        generation: 1,
        originM: { x: 1_000, y: 2_000, z: PLANET.figure.polarRadiusM },
        heights: new Float32Array(65 * 65 * 2),
        offsets:
          this.settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
        normals: new Float16Array(n * n * 2),
        heightRangeM: [0, 0],
        boundingRadiusM: 1,
        originHeightM: 0,
        skirtDepthM: 1,
        layers: null,
      };
      for (const listener of this.#listeners) {
        listener(bake);
      }
      return patchKeyString(key);
    });
  }
}

/**
 * A frame 1,000 km above the north pole at `nowMs`, looking straight down, or tilted from it by
 * `tiltRad` about the y axis.
 */
function aboveThePole(nowMs: number, tiltRad = 0): TerrainFrameInput {
  return {
    view: {
      rotation: IDENTITY_ROTATION,
      cameraM: { x: 0, y: 0, z: PLANET.figure.polarRadiusM + 1_000_000 },
      orientation: lookAlong(
        { x: Math.sin(tiltRad), y: 0, z: -Math.cos(tiltRad) },
        { x: Math.cos(tiltRad), y: 0, z: Math.sin(tiltRad) },
      ),
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 640, heightPx: 360 },
    },
    grounded: [],
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-5,
    nowMs,
  };
}

/** The test planet's pass on the low setting, with `options`, and its pool. */
async function passWith(
  options: Pick<TerrainPassOptions, "onResident" | "onSelect"> = {},
): Promise<{ readonly pass: TerrainPass; readonly pool: () => FlatPool }> {
  const engine = await countingRenderEngine({
    maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
    maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
  });
  let made: FlatPool | null = null;
  const pass = new TerrainPass({
    engine,
    setting: "low",
    planet: PLANET,
    source: { kind: "test-planet", ridges: "off" },
    createPool: (bake) => {
      made = new FlatPool(bake);
      return made;
    },
    ...options,
  });
  await pass.ready();
  return {
    pass,
    pool: () => {
      if (made === null) {
        throw new Error("no pool was made");
      }
      return made;
    },
  };
}

/** An observer that records what it is told. */
function recording(): TerrainPassObserver & {
  readonly selections: Array<readonly [SelectionInput, Selection]>;
  readonly resident: string[];
} {
  const selections: Array<readonly [SelectionInput, Selection]> = [];
  const resident: string[] = [];
  return {
    selections,
    resident,
    onSelect: (input, selection) => selections.push([input, selection]),
    onResident: (key: PatchKey) => resident.push(patchKeyString(key)),
  };
}

describe("a terrain pass's observers (R10.T14.c)", () => {
  it("tells each observer of each selection and each patch made resident, as its options do", async () => {
    const options = recording();
    const { pass, pool } = await passWith(options);
    const observers = [recording(), recording()];
    for (const observer of observers) {
      pass.observe(observer);
    }
    const first = pass.frame(aboveThePole(0));
    pass.frame(aboveThePole(16));
    const baked = pool().bakeDemand(pass);
    expect(baked.length).toBeGreaterThan(0);
    for (const observer of observers) {
      expect(observer.selections).toEqual(options.selections);
      expect(observer.selections).toHaveLength(1);
      expect(observer.selections[0]?.[1]).toBe(first.selection);
      expect(observer.resident).toEqual(baked);
    }
    expect(options.resident).toEqual(baked);
  });

  it("tells an observer nothing once it is removed, and the others on", async () => {
    const { pass, pool } = await passWith();
    const kept = recording();
    const removed = recording();
    pass.observe(kept);
    const remove = pass.observe(removed);
    remove();
    pass.frame(aboveThePole(0));
    const baked = pool().bakeDemand(pass);
    expect(removed.selections).toEqual([]);
    expect(removed.resident).toEqual([]);
    expect(kept.selections).toHaveLength(1);
    expect(kept.resident).toEqual(baked);
  });

  it("tells an observer added later only of what follows", async () => {
    const { pass, pool } = await passWith();
    pass.frame(aboveThePole(0));
    const earlier = pool().bakeDemand(pass);
    const late = recording();
    pass.observe(late);
    const before = { selections: [...late.selections], resident: [...late.resident] };
    // A turned camera selects afresh, and its demand is made resident after the observer came.
    const following = pass.frame(aboveThePole(16, 0.4));
    const later = pool().bakeDemand(pass);
    expect([earlier.length > 0, later.length > 0, before]).toEqual([
      true,
      true,
      { selections: [], resident: [] },
    ]);
    expect(late.selections.map(([, selection]) => selection)).toEqual([following.selection]);
    expect(late.resident).toEqual(later);
  });
});
