import { describe, expect, it, vi } from "vitest";

import { add, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aViewRing } from "../../test/ringFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { LitBodyInput } from "../bodies/draw";
import type { LitRegime } from "../bodies/regime";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { AllocationEvent } from "../engine/memory";
import { sunLikeHostDisc } from "../lighting/hostDisc";
import { lightsAt } from "../lighting/hostLights";
import type { ViewRing } from "../scene/model";
import { PROFILE_KEPT_FRAMES, type RingSlabFrameInput, RingSlabs } from "./ringSlabs";

const A_M = 6e7;
const AU_M = 1.495978707e11;

/** A ringed planet `index` ahead of the camera, a sphere of radius {@link A_M}. */
function planetAt(index: number, centreM: Vec3): LitBodyInput {
  return {
    id: `0300080020000000.${String(index).padStart(4, "0")}`,
    centreM,
    figure: { equatorialRadiusM: A_M, polarRadiusM: A_M, pole: null },
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
  };
}

/** A frame of `count` ringed planets, each a disc, side by side ahead of the camera. */
function frameOf(count: number, regime: LitRegime = "disc"): RingSlabFrameInput {
  const planets = Array.from({ length: count }, (_, i) =>
    planetAt(i + 1, vec3((i - count / 2) * 3 * A_M, 0, -40 * A_M)),
  );
  const rings: ViewRing[] = planets.map((planet) =>
    aViewRing(planet.id, 1.24 * A_M, 2.27 * A_M, vec3(0, 0.96, 0.28)),
  );
  const hosts = [
    { disc: sunLikeHostDisc(), centreM: add(vec3(0, 0, 0), scale(vec3(1, 0, 0), AU_M)) },
  ];
  return {
    rings,
    bodies: planets,
    plan: {
      regimes: new Map(planets.map((planet) => [planet.id, regime])),
      sources: new Map(
        planets.map((planet) => [
          planet.id,
          { stars: lightsAt(planet.centreM, hosts, 2), planetshine: [] },
        ]),
      ),
    },
    camera: { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
    viewport: { widthPx: 640, heightPx: 360 },
    exposureScale: 1e-3,
    annuli: 4,
  };
}

/** The frame with no ring in its scene. */
function bareFrame(): RingSlabFrameInput {
  return { ...frameOf(1), rings: [] };
}

/** Each event's kind and name, as an engine's allocations raise them. */
function eventsOf(engine: Awaited<ReturnType<typeof countingRenderEngine>>): AllocationEvent[] {
  const events: AllocationEvent[] = [];
  engine.onAllocation((event) => {
    events.push(event);
  });
  return events;
}

describe("the rings' slabs on a view (R11.T9.c)", () => {
  it("draws no ring until made, and starts its making at the first frame with one", async () => {
    const engine = await countingRenderEngine();
    const slabs = new RingSlabs(engine, "test");
    expect(slabs.frame(frameOf(1))).toBeNull();
    expect(engine.compiles.map((asked) => asked.name)).toEqual(["rings:slab", "rings:slabOverAir"]);
    slabs.dispose();
  });

  it("draws each ring of a planet larger than a point, by the form its target asks", async () => {
    const engine = await countingRenderEngine();
    const slabs = new RingSlabs(engine, "test");
    await slabs.prepare();
    const frame = slabs.frame(frameOf(2));
    expect(frame?.rings.map((ring) => ring.index)).toEqual([0, 1]);
    const depth = { kind: "texture", name: "scene depth" } as const;
    expect([
      frame?.draw(0, "all", 0, null).material.name,
      frame?.draw(1, "before-air", 1.1, depth).material.name,
      frame?.draw(1, "before-air", 1.1, depth).textures["sceneDepth"],
    ]).toEqual(["rings:slab", "rings:slabOverAir", depth]);
    expect(slabs.frame(frameOf(2, "point"))).toBeNull();
    slabs.dispose();
  });

  it("lets a profile go once no frame has drawn it for PROFILE_KEPT_FRAMES frames, frames without rings counted", async () => {
    const engine = await countingRenderEngine();
    const events = eventsOf(engine);
    const slabs = new RingSlabs(engine, "test");
    await slabs.prepare();
    slabs.frame(frameOf(1));
    const gone = (): number =>
      events.filter((e) => e.kind === "destroyed" && e.name === "test:ring profile").length;
    for (let frame = 0; frame < PROFILE_KEPT_FRAMES; frame += 1) {
      slabs.frame(bareFrame());
    }
    expect(gone()).toBe(0);
    slabs.frame(bareFrame());
    expect(gone()).toBe(1);
    slabs.dispose();
  });

  it("grows its records' buffer for a frame of more rings than it holds", async () => {
    const engine = await countingRenderEngine();
    const events = eventsOf(engine);
    const slabs = new RingSlabs(engine, "test");
    await slabs.prepare();
    slabs.frame(frameOf(12));
    const records = events.filter((e) => e.name === "test:ring records" && e.kind !== "uploaded");
    expect(records.map((e) => e.kind)).toEqual(["created", "destroyed", "created"]);
    slabs.dispose();
  });

  it("makes its slabs again after a restore", async () => {
    const engine = await countingRenderEngine();
    const slabs = new RingSlabs(engine, "test");
    await slabs.prepare();
    slabs.restored();
    expect(slabs.frame(frameOf(1))).toBeNull();
    await slabs.prepare();
    expect(slabs.frame(frameOf(1))?.rings).toHaveLength(1);
    expect(engine.compiles.filter((asked) => asked.name === "rings:slab")).toHaveLength(2);
    slabs.dispose();
  });

  it("reports a refused making once, releases what it made, and asks no more until a restore", async () => {
    const engine = await countingRenderEngine();
    engine.failMaterials((name) => name === "rings:slabOverAir");
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const released = vi.spyOn(engine, "releaseMaterial");
    const slabs = new RingSlabs(engine, "test");
    await slabs.prepare();
    expect(errors).toHaveBeenCalledTimes(1);
    expect(released.mock.calls.map(([material]) => material.name)).toEqual(["rings:slab"]);
    expect(slabs.frame(frameOf(1))).toBeNull();
    expect(engine.compiles).toHaveLength(2);
    slabs.dispose();
  });

  it("releases its materials, mesh, tables, records and profiles when disposed", async () => {
    const engine = await countingRenderEngine();
    const events = eventsOf(engine);
    const slabs = new RingSlabs(engine, "test");
    await slabs.prepare();
    slabs.frame(frameOf(1));
    slabs.dispose();
    const live = new Map<string, number>();
    for (const event of events) {
      if (event.kind !== "uploaded") {
        live.set(event.name, (live.get(event.name) ?? 0) + (event.kind === "created" ? 1 : -1));
      }
    }
    expect([...live.values()].every((count) => count === 0)).toBe(true);
  });
});
