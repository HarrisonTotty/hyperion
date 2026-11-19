import { describe, expect, it, vi } from "vitest";

import { add, scale, vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aViewRing } from "../../test/ringFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { LitBodyInput } from "../bodies/draw";
import {
  perspectiveReversedInfinite,
  type ProjectionCamera,
  viewRotation4,
} from "../camera/projection";
import { IDENTITY_QUATERNION, lookAlong } from "../camera/quaternion";
import { cameraCullPassOf } from "../clusters/instanceScene";
import { sunLikeHostDisc } from "../lighting/hostDisc";
import { lightsAt } from "../lighting/hostLights";
import type { ViewRing } from "../scene/model";
import { fieldParticles, ringFieldOf } from "./ringField";
import {
  expectedFieldParticles,
  fieldBudgetScale,
  fieldRingOf,
  lambertSpherePhase,
  particlePhaseRatio,
  particlesOn,
  RING_PARTICLE_CAPACITY,
  RING_PARTICLE_CULL_LABEL,
  RING_PARTICLE_PLACE_KERNEL,
  RingParticles,
  type RingParticlesFrameInput,
} from "./ringParticles";
import { ringProfile } from "./ringProfile";
import { handOverDistance, pixelAngleRad } from "./ringSplit";

const A_M = 6e7;
const AU_M = 1.495978707e11;
const PLANET = "0300080020000000.0301";
const VIEWPORT = { widthPx: 64, heightPx: 36 };
const FOV = Math.PI / 3;
const PIXEL = pixelAngleRad(FOV, VIEWPORT.widthPx);

/** The planet at `centreM` from the camera. */
function planetAt(centreM: ReturnType<typeof vec3>): LitBodyInput {
  return {
    id: PLANET,
    centreM,
    figure: { equatorialRadiusM: A_M, polarRadiusM: A_M, pole: vec3(0, 0, 1) },
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
  };
}

const RING: ViewRing = {
  ...aViewRing(PLANET, 1.24 * A_M, 2.27 * A_M, vec3(0, 0, 1)),
  gravitationalParameterM3S2: 3.79e16,
};

/** The ring's gap's radius, m (`aViewRing`'s, 0.8 of the way out). */
const GAP_M = 1.24 * A_M + 0.8 * (2.27 - 1.24) * A_M;

/** Looking along the orbit, level with the plane. */
const ALONG_ORBIT = lookAlong(vec3(0, 1, 0), vec3(0, 0, 1));

/** Looking towards the planet, 6° below the plane's level. */
const INWARDS = lookAlong(vec3(-Math.cos(0.1), 0, -Math.sin(0.1)), vec3(0, 0, 1));

/** A frame `heightM` over the ring at 1.8 a from its planet, looking down at it. */
function frameOver(heightM: number): RingParticlesFrameInput {
  const planet = planetAt(vec3(-1.8 * A_M, 0, -heightM));
  const hosts = [
    { disc: sunLikeHostDisc(), centreM: add(planet.centreM, scale(vec3(0.6, 0, 0.8), AU_M)) },
  ];
  return {
    rings: [RING],
    bodies: [planet],
    plan: {
      regimes: new Map([[PLANET, "disc"]]),
      sources: new Map([[PLANET, { stars: lightsAt(planet.centreM, hosts, 2), planetshine: [] }]]),
    },
    camera: { orientation: IDENTITY_QUATERNION, fovXRad: FOV },
    viewport: VIEWPORT,
    viewRotation: viewRotation4(IDENTITY_QUATERNION),
    projection: perspectiveReversedInfinite(FOV, VIEWPORT.widthPx / VIEWPORT.heightPx, 0.1),
    time: { seconds: 1_000_000, nanos: 0 },
    exposureScale: 1e-3,
    annuli: 4,
  };
}

/** A field's particles four times the capacity at the plain pixel, falling as its angle's square. */
function estimate(scaleBy: number): number {
  return (4 * RING_PARTICLE_CAPACITY) / scaleBy ** 2;
}

describe("a ring's particles close to (R11.T9.d)", () => {
  it("are drawn on the high setting alone, which instances them", () => {
    expect([particlesOn("high"), particlesOn("low")]).toEqual([true, false]);
  });

  it("are of the massive ring the camera is within its largest particle's hand-over of", () => {
    const near = frameOver(0.5 * handOverDistance(RING, PIXEL));
    const far = frameOver(2 * handOverDistance(RING, PIXEL));
    const dusty: ViewRing = { ...RING, ringKind: "dusty" };
    const regimes = near.plan.regimes;
    expect(fieldRingOf(near.rings, near.bodies, regimes, PIXEL)?.ring).toBe(RING);
    expect(fieldRingOf(far.rings, far.bodies, regimes, PIXEL)).toBeNull();
    expect(fieldRingOf([dusty], near.bodies, regimes, PIXEL)).toBeNull();
    expect(fieldRingOf(near.rings, near.bodies, new Map([[PLANET, "point"]]), PIXEL)).toBeNull();
  });

  it("widen their pixel until the expected particles fit the capacity's budget", () => {
    const factor = fieldBudgetScale(estimate);
    expect(factor).toBeGreaterThan(2);
    expect(estimate(factor)).toBeLessThan(RING_PARTICLE_CAPACITY);
    expect(fieldBudgetScale(() => 10)).toBe(1);
  });

  it("place and cull their field, then give its bucket draws, once made", async () => {
    const engine = await countingRenderEngine();
    const particles = new RingParticles(engine, "test");
    const input = frameOver(300);
    expect(particles.frame(input)).toBeNull();
    await particles.prepare();
    const frame = particles.frame(input);
    expect(frame?.ring).toBe(RING);
    expect(frame?.pixelRad).toBeGreaterThanOrEqual(PIXEL);
    expect(frame?.draws.length).toBeGreaterThan(0);
    expect(frame?.draws.every((draw) => draw.material.name === "rings:particles")).toBe(true);
    const passes = engine.dispatched.map((each) => each.pass);
    expect(passes).toEqual(
      expect.arrayContaining([RING_PARTICLE_PLACE_KERNEL.name, RING_PARTICLE_CULL_LABEL]),
    );
    particles.dispose();
  });

  it("give a dusty ring none, even within a layer's depth of it", () => {
    const near = frameOver(5);
    const dusty: ViewRing = { ...RING, ringKind: "dusty" };
    expect(fieldRingOf([dusty], near.bodies, near.plan.regimes, PIXEL)).toBeNull();
  });

  it.each([
    ["300 m over the ring, looking down", 1.8 * A_M, 300, IDENTITY_QUATERNION],
    [
      "1 km beyond its outer edge, 100 m over its plane, looking in",
      2.27 * A_M + 1e3,
      100,
      INWARDS,
    ],
    ["at its mid-plane, looking along the orbit", 1.8 * A_M, 0, ALONG_ORBIT],
    ["over its gap, looking down", GAP_M + 400, 200, IDENTITY_QUATERNION],
  ] as const)(
    "expect about the particles their twin places in view, and never far fewer: %s",
    (_, radiusM, heightM, orientation) => {
      const camera: ProjectionCamera = { orientation, fovXRad: FOV };
      const viewport = { widthPx: 160, heightPx: 90 };
      const projection = perspectiveReversedInfinite(FOV, 16 / 9, 0.1);
      const pass = cameraCullPassOf(viewRotation4(orientation), projection, viewport.widthPx, 1);
      const field = ringFieldOf({
        centreM: vec3(-radiusM, 0, -heightM),
        normal: RING.normal,
        profile: ringProfile(RING),
        gravitationalParameterM3S2: null,
        referenceRadiusM: radiusM,
        sinceS: 0,
        pixelRad: pixelAngleRad(FOV, viewport.widthPx),
        planes: pass.planes,
        seed: 3,
        towardsStar: vec3(0.6, 0, 0.8),
      });
      if (field === null) {
        throw new Error("no field");
      }
      const placed = fieldParticles(field).length;
      const expected = expectedFieldParticles(field, camera, viewport);
      expect(placed).toBeGreaterThan(100);
      expect(expected / placed).toBeGreaterThan(0.7);
      expect(expected / placed).toBeLessThan(3);
    },
  );

  it("take the power law's phase over a Lambert sphere's, finite and falling to 0 back-lit", () => {
    expect(lambertSpherePhase(0)).toBeCloseTo(8 / 3, 12);
    // Its series below u = 0.01 against the closed form there, where it keeps its digits.
    const u = 0.0101;
    const closed = (8 / (3 * Math.PI)) * (Math.sin(u) - u * Math.cos(u));
    expect(lambertSpherePhase(Math.PI - u) / closed).toBeCloseTo(1, 9);
    expect(lambertSpherePhase(Math.PI - 1e-3) / ((8 / (9 * Math.PI)) * 1e-9)).toBeCloseTo(1, 6);
    const ratios = [Math.PI - 1e-2, Math.PI - 1e-5, Math.PI].map((a) => particlePhaseRatio(a));
    expect(ratios.every((ratio) => Number.isFinite(ratio) && ratio > 0)).toBe(true);
    expect(ratios.toSorted((a, b) => b - a)).toEqual(ratios);
  });

  it("make their pipelines again after a restore, and let them go for a setting without them", async () => {
    const engine = await countingRenderEngine();
    const releasedKernels = vi.spyOn(engine, "releaseCompute");
    const particles = new RingParticles(engine, "test");
    await particles.prepare();
    particles.restored();
    expect(particles.frame(frameOver(300))).toBeNull();
    await particles.prepare();
    expect(particles.frame(frameOver(300))?.draws.length).toBeGreaterThan(0);
    particles.release();
    expect(releasedKernels).toHaveBeenCalledTimes(2);
    expect(particles.frame(frameOver(300))).toBeNull();
    particles.dispose();
  });

  it("report a refused making once, release what it made, and draw nothing", async () => {
    const engine = await countingRenderEngine();
    engine.failMaterials((name) => name === "rings:particles");
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const releasedKernels = vi.spyOn(engine, "releaseCompute");
    const particles = new RingParticles(engine, "test");
    await particles.prepare();
    expect(errors).toHaveBeenCalledTimes(1);
    expect(releasedKernels).toHaveBeenCalledTimes(2);
    expect(particles.frame(frameOver(300))).toBeNull();
    particles.dispose();
  });

  it("give nothing far from every ring", async () => {
    const engine = await countingRenderEngine();
    const particles = new RingParticles(engine, "test");
    await particles.prepare();
    expect(particles.frame(frameOver(10 * handOverDistance(RING, PIXEL)))).toBeNull();
    particles.dispose();
  });

  it("release their path, kernels, material and mesh when disposed", async () => {
    const engine = await countingRenderEngine();
    const live = new Map<string, number>();
    engine.onAllocation((event) => {
      if (event.kind !== "uploaded") {
        live.set(event.name, (live.get(event.name) ?? 0) + (event.kind === "created" ? 1 : -1));
      }
    });
    const releasedKernels = vi.spyOn(engine, "releaseCompute");
    const particles = new RingParticles(engine, "test");
    await particles.prepare();
    particles.frame(frameOver(300));
    particles.dispose();
    expect([...live.entries()].filter(([, count]) => count !== 0)).toEqual([]);
    expect(releasedKernels).toHaveBeenCalledTimes(2);
  });
});
