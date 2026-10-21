/**
 * The terrain session's shadow cascades (plan R10, R10.T9.b; decision-r10-t9b-design D2 and D9):
 * the one set, made at its first need on the high setting alone, under `shadows`; the placeholder
 * bound where no set exists; both let go at a restore and made again at their next need, and
 * released when let go; and the registry of other casters, over R01's counting engine.
 */

import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../../test/countingRenderEngine";
import { viewId } from "../../camera/state";
import { IDENTITY_ROTATION } from "../../coords/rotation";
import type { DrawItem, MaterialHandle, MeshHandle } from "../../engine/types";
import { allocationTally } from "../gpu/allocationTally";
import { CASCADE_BYTES, type CascadeSet, fitCascades } from "./cascades";
import {
  SHADOW_PLACEHOLDER,
  SHADOW_PLACEHOLDER_NAME,
  ShadowCasters,
  TerrainShadows,
} from "./terrainShadows";

/** A draw that stands for a non-terrain caster's, told apart by its offset. */
function casterDraw(offsetM: number): DrawItem {
  const handle = { kind: "mesh", name: "caster" } as const satisfies MeshHandle;
  const material = { kind: "material", name: "caster" } as const satisfies MaterialHandle;
  return {
    mesh: handle,
    material,
    offsetFromCameraM: Float32Array.of(offsetM, 0, 0),
    uniforms: {},
    textures: {},
  };
}

/** The view the casters are asked for. */
const VIEW = viewId("primary");

/** The offsets that tell `draws` apart ({@link casterDraw}). */
function offsets(draws: ReadonlyArray<DrawItem>): number[] {
  return draws.map((draw) => draw.offsetFromCameraM[0] ?? Number.NaN);
}

/** A set, 2 m over a Moon-sized body: the providers here read none of it. */
const SET: CascadeSet = fitCascades({
  cameraBodyFixedM: { x: 0, y: 0, z: 1_737_402 },
  orientationBodyFixed: { w: 1, x: 0, y: 0, z: 0 },
  rotation: IDENTITY_ROTATION,
  fovXRad: Math.PI / 3,
  aspect: 16 / 9,
  heightAboveGroundM: 2,
  sunDirectionBodyFixed: { x: 1, y: 0, z: 0.3 },
  datumNormalBodyFixed: { x: 0, y: 0, z: 1 },
  bodyRadiusM: 1_737_400,
  reliefRangeM: 14_900,
  horizonTexelM: 1,
  depthClipControl: false,
});

describe("the session's set of cascades (R10.T9.b)", () => {
  it("is made at its first need on the high setting, once, its 67,108,864 B under shadows", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    const shadows = new TerrainShadows(engine, { cascades: true });
    expect([shadows.target, tally.liveBytes("shadows")]).toEqual([null, 0]);
    const first = shadows.cascadeTarget();
    const second = shadows.cascadeTarget();
    expect([
      first?.name,
      first?.sizePx,
      first?.layers,
      second === first,
      engine.counts.depthLayerTargets,
      tally.liveBytes("shadows"),
      shadows.bytesByCategory().get("shadows"),
    ]).toEqual(["terrain shadow cascades", 2048, 4, true, 1, CASCADE_BYTES, CASCADE_BYTES]);
  });

  it("is never made on the low setting", async () => {
    const engine = await countingRenderEngine();
    const shadows = new TerrainShadows(engine, { cascades: false });
    expect([shadows.cascadeTarget(), engine.counts.depthLayerTargets]).toEqual([null, 0]);
  });

  it("binds the placeholder, one 1 × 1 × 4 depth array under other, until the set is made, and the set after", async () => {
    const engine = await countingRenderEngine();
    const shadows = new TerrainShadows(engine, { cascades: true });
    const placeholder = shadows.bound();
    expect([placeholder.name, shadows.bound() === placeholder]).toEqual([
      SHADOW_PLACEHOLDER_NAME,
      true,
    ]);
    expect(engine.textureSpecs.find((spec) => spec.name === SHADOW_PLACEHOLDER_NAME)).toEqual({
      ...SHADOW_PLACEHOLDER,
      size: { width: 1, height: 1, depthOrArrayLayers: 4 },
      format: "depth32float",
      category: "other",
    });
    const target = shadows.cascadeTarget();
    expect(shadows.bound()).toBe(target?.depth);
    expect(Object.fromEntries(shadows.bytesByCategory())).toEqual({
      shadows: CASCADE_BYTES,
      other: 16,
    });
  });

  it("lets both go at a restore, and makes each again at its next need on the new device", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    const shadows = new TerrainShadows(engine, { cascades: true });
    const placeholder = shadows.bound();
    const first = shadows.cascadeTarget();
    engine.loseDevice();
    engine.restore();
    expect([shadows.target, tally.liveBytes("shadows"), engine.live.textures]).toEqual([
      null,
      0,
      0,
    ]);
    const again = shadows.bound();
    const remade = shadows.cascadeTarget();
    expect([
      again === placeholder,
      again.name,
      remade === first,
      engine.counts.depthLayerTargets,
      tally.liveBytes("shadows"),
    ]).toEqual([false, SHADOW_PLACEHOLDER_NAME, false, 2, CASCADE_BYTES]);
  });

  it("releases a living set and placeholder at a restore that took no handle", async () => {
    const engine = await countingRenderEngine();
    const shadows = new TerrainShadows(engine, { cascades: true });
    const before = engine.live;
    shadows.bound();
    shadows.cascadeTarget();
    engine.restore();
    expect(engine.live).toEqual(before);
  });

  it("releases both when let go, and makes nothing more", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    const before = engine.live;
    const shadows = new TerrainShadows(engine, { cascades: true });
    shadows.bound();
    shadows.cascadeTarget();
    shadows.dispose();
    shadows.dispose();
    expect([engine.live, tally.liveBytes("shadows"), shadows.cascadeTarget()]).toEqual([
      before,
      0,
      null,
    ]);
    expect(() => shadows.bound()).toThrow(/let go/u);
    engine.restore();
    expect(engine.counts.depthLayerTargets).toBe(1);
  });
});

describe("the other casters' registry (R10.T9.b; decision-r10-t9b-design D9)", () => {
  it("gives each provider's draws of a cascade in the order they were added, and drops one removed", () => {
    const casters = new ShadowCasters();
    const asked: Array<readonly [number, string]> = [];
    const removeFirst = casters.add((_set, index, view) => {
      asked.push([index, view]);
      return [casterDraw(1)];
    });
    casters.add(() => [casterDraw(2), casterDraw(3)]);
    expect([offsets(casters.draws(SET, 2, VIEW)), casters.size, asked]).toEqual([
      [1, 2, 3],
      2,
      [[2, VIEW]],
    ]);
    removeFirst();
    removeFirst();
    expect([offsets(casters.draws(SET, 0, VIEW)), casters.size]).toEqual([[2, 3], 1]);
  });

  it("gives none while it holds no provider", () => {
    expect(new ShadowCasters().draws(SET, 0, VIEW)).toEqual([]);
  });
});
