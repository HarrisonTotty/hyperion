import { describe, expect, it } from "vitest";

import { FakeAdapter, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { FakeRenderEngine } from "../../test/fakeRenderEngine";
import { decodedSky, skyPayload } from "../../test/skyFixtures";
import type { BufferHandle, RenderEngine, TextureHandle } from "../engine/types";
import type { BufferSpec } from "../engine/memory";
import { type CubeRequest, SkyCubeCache } from "./cache";

/** An engine without float32-blendable that bakes on the CPU and records cubes made and released. */
async function countingCubes(): Promise<{
  engine: RenderEngine;
  made: string[];
  released: string[];
  restore: () => void;
}> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
  await adapter.requestDevice();
  const device = adapter.devices.at(0);
  if (device === undefined) {
    throw new Error("the fake adapter made no device");
  }
  const made: string[] = [];
  const released: string[] = [];
  let next = 0;
  const restored: Array<() => void> = [];
  const engine = Object.assign(new FakeRenderEngine(device), {
    onRestored: (listener: () => void): (() => void) => {
      restored.push(listener);
      return () => undefined;
    },
    createPackedCube: (): TextureHandle => {
      next += 1;
      const name = `cube ${String(next)}`;
      made.push(name);
      return { kind: "texture", name };
    },
    writePackedCubeLevel: (): void => undefined,
    createBuffer: (spec: BufferSpec): BufferHandle => ({
      kind: "buffer",
      name: spec.name,
      bytes: spec.bytes,
    }),
    writeBuffer: (): void => undefined,
    releaseBuffer: (): void => undefined,
    releaseTexture: (texture: TextureHandle): void => {
      released.push(texture.name);
    },
  });
  return {
    engine,
    made,
    released,
    restore: () => {
      for (const listener of restored) {
        listener();
      }
    },
  };
}

/** A decoded sky of two stars, both baked; each call is another sky, as another arrival's is. */
function aSky() {
  return decodedSky(
    skyPayload(
      [
        { direction: [1, 0, 0], distanceLy: 0.05, vMag: 9 },
        { direction: [0, 1, 0], distanceLy: 400, vMag: 9 },
      ],
      2,
      null,
    ),
    2,
  ).stars;
}

/** The sky the views near one another share. */
const STARS = aSky();

/** A view's request for a sky's two stars. */
function requestOf(stars = STARS): CubeRequest {
  return {
    stars,
    baked: Uint32Array.from([0, 1]),
    bakeInput: () => ({
      directions: stars.directions,
      illuminanceLx: new Float32Array(6).fill(1e-9),
      faceSizePx: 4,
      name: "sky cube",
    }),
  };
}

describe("SkyCubeCache", () => {
  it("shares one cube between two views near one another", async () => {
    const { engine, made } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    const a = cache.acquire("cockpit", requestOf());
    const b = cache.acquire("instrument", requestOf());
    expect(a).toBe(b);
    expect([made.length, cache.size]).toEqual([1, 1]);
  });

  it("gives two views far apart, each with a sky of its own place, a cube each", async () => {
    const { engine, made } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("cockpit", requestOf());
    cache.acquire("far camera", requestOf(aSky()));
    expect([made.length, cache.size]).toEqual([2, 2]);
  });

  it("releases a view's cube with its last view", async () => {
    const { engine, released } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("cockpit", requestOf());
    cache.acquire("instrument", requestOf());
    cache.release("cockpit");
    expect(released).toEqual([]);
    cache.release("instrument");
    expect([released, cache.size]).toEqual([["cube 1"], 0]);
  });

  it("re-bakes a view's cube when its new sky arrives, letting the old one go", async () => {
    const { engine, made, released } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("cockpit", requestOf());
    cache.acquire("cockpit", requestOf(aSky()));
    expect([made, released]).toEqual([["cube 1", "cube 2"], ["cube 1"]]);
  });

  it("bakes nothing for a view with no baked stars", async () => {
    const { engine, made } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    expect(cache.acquire("cockpit", { ...requestOf(), baked: new Uint32Array(0) })).toBeNull();
    expect(made).toEqual([]);
  });

  it("bakes again after a device loss's restore, the old cubes having died with the device", async () => {
    const { engine, made, released, restore } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("cockpit", requestOf());
    restore();
    cache.acquire("cockpit", requestOf());
    expect([made, released, cache.size]).toEqual([["cube 1", "cube 2"], [], 1]);
  });

  it("keeps a view's cube across frames that hand it the same stars again", async () => {
    const { engine, made } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    const request = requestOf();
    for (let frame = 0; frame < 10; frame += 1) {
      cache.acquire("cockpit", { ...request, baked: Uint32Array.from(request.baked) });
    }
    expect(made).toEqual(["cube 1"]);
  });
});
