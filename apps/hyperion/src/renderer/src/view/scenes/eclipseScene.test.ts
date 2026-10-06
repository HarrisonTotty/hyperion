import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { photorealFrame } from "../../displays/view/photorealFrame";
import { runPose, SCENE_OPTIONS, startRun, stepRun } from "../../displays/view/viewRun";
import { add, cross, dot, norm, normalise, scale, sub, type Vec3 } from "../../geometry/vec3";
import { SPEED_OF_LIGHT_M_PER_S } from "../../lib/scene/lightTime";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { type DiscPixel, type DiscRecord, rasteriseDisc, viewRay } from "../bodies/discShading";
import { type BodyFramePlan, planLitBodies } from "../bodies/draw";
import { compositeBodyFrame } from "../bodies/frameTwin";
import { sphereFootprint } from "../bodies/regime";
import type { CameraPose } from "../camera/pose";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import { circleOverlapArea, DISC_ANNULI_HIGH, DISC_ANNULI_LOW } from "../lighting/annuli";
import { penumbraRadius } from "../lighting/occluders";
import { eclipseIntegralVisibleFraction } from "../lighting/oracle";
import {
  litNeighbours,
  PLANETSHINE_SOURCES_HIGH,
  planetshineSources,
} from "../lighting/planetshine";
import { PSF_QUAD_PX } from "../photometry/magnitude";
import type { PhotorealFrame } from "../photoreal/renderer";
import { METER_CLASS } from "../post/meter";
import { HostDiscLayer, type HostDiscRecord, hostPlacements } from "../sky/disc";
import { emptyDrawList, viewStrokesAt } from "../wireframe/drawList";
import {
  ECLIPSE_CAMERA_OFFSET_M,
  ECLIPSE_CAMERA_POSE,
  ECLIPSE_CONJUNCTION_S,
  ECLIPSE_DURATION_S,
  ECLIPSE_GIANT,
  ECLIPSE_GIANT_RADIUS_M,
  ECLIPSE_MID_S,
  ECLIPSE_MOON,
  ECLIPSE_MOON_RADIUS_M,
  ECLIPSE_PLANET,
  ECLIPSE_PLANET_RADIUS_M,
  ECLIPSE_SCENE_NAME,
  ECLIPSE_TIME_RATE,
  eclipsePlaceAt,
  eclipsePoseAtStar,
  eclipseScene,
  eclipseSceneAt,
} from "./eclipseScene";

/** The scene's host disc: the Sun-like star. */
const DISC = (() => {
  const disc = eclipseSceneAt(0).hostDiscs?.[0];
  if (disc === undefined) {
    throw new Error("the eclipse scene has no host disc");
  }
  return disc;
})();

/** The pre-exposure the frames are drawn with: the day side near 1, never clamped. */
const EXPOSURE = 1e-4;

/** No wireframe draw list: the frames' stars are not under test. */
const NO_LIST = emptyDrawList(viewStrokesAt(1));

/** One frame of the scene from a camera, as the view's photorealistic frame assembles it. */
interface Drawn {
  readonly frame: PhotorealFrame;
  readonly plan: BodyFramePlan;
  readonly camera: ProjectionCamera;
  readonly viewport: Viewport;
  /** The camera's place, m from the barycentre. */
  readonly cameraM: Vec3;
}

/**
 * The scene at scene time `sceneS` drawn from `pose` (in the planet's frame) across `fovDeg`, with
 * `annuli` annuli, through `photorealFrame` and `planLitBodies` as the view draws it.
 */
async function drawnAt(
  sceneS: number,
  pose: CameraPose,
  fovDeg: number,
  viewport: Viewport,
  annuli = DISC_ANNULI_HIGH,
): Promise<Drawn> {
  const engine = await countingRenderEngine();
  const start = startRun(eclipseScene());
  const run = {
    ...start,
    scene: eclipseSceneAt(sceneS),
    camera: { ...start.camera, pose, fovDeg },
  };
  const frame = photorealFrame({
    run,
    pose,
    viewport,
    setting: "high",
    exposureScale: EXPOSURE,
    list: NO_LIST,
    sky: null,
    band: null,
    discs: new HostDiscLayer(engine),
    cube: null,
    previousRegimes: new Map(),
    overlay: null,
    meter: null,
  });
  const plan = planLitBodies(
    frame.bodies,
    frame.lights,
    {
      camera: frame.camera,
      viewport,
      exposureScale: EXPOSURE,
      annuli,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high",
    },
    new Map(),
  );
  const cameraM = add(eclipsePlaceAt(ECLIPSE_PLANET, sceneS).centreM, pose.positionM);
  return { frame, plan, camera: frame.camera, viewport, cameraM };
}

/** A body's disc record in a plan. */
function recordOf(plan: BodyFramePlan, body: BodyIdHex): DiscRecord {
  const record = plan.discs.find((each) => each.body === body);
  if (record === undefined) {
    throw new Error(`the plan draws no disc of ${body}`);
  }
  return record;
}

/** The pixel's ray, unit, along the galactic axes. */
function rayOf(x: number, y: number, camera: ProjectionCamera, viewport: Viewport): Vec3 {
  return rotate(camera.orientation, viewRay(x + 0.5, y + 0.5, camera, viewport));
}

/** The first point where a ray from `fromM` meets the sphere about `centreM`, or `null`. */
function hitSphere(fromM: Vec3, ray: Vec3, centreM: Vec3, radiusM: number): Vec3 | null {
  const toCentre = sub(centreM, fromM);
  const along = dot(toCentre, ray);
  const across2 = dot(toCentre, toCentre) - along * along;
  const h2 = radiusM * radiusM - across2;
  if (h2 < 0 || along <= 0) {
    return null;
  }
  return add(fromM, scale(ray, along - Math.sqrt(h2)));
}

/**
 * Where the moon was when the light reaching `pointM` at scene time `sceneS` passed it: its exact
 * track at s − δ, δ = |moon(s − δ) − point| ÷ c, by fixed-point steps to 10⁻¹² s.
 */
function retardedMoonM(pointM: Vec3, sceneS: number): Vec3 {
  let delay = 0;
  for (let step = 0; step < 8; step += 1) {
    delay =
      norm(sub(eclipsePlaceAt(ECLIPSE_MOON, sceneS - delay).centreM, pointM)) /
      SPEED_OF_LIGHT_M_PER_S;
  }
  return eclipsePlaceAt(ECLIPSE_MOON, sceneS - delay).centreM;
}

/** The angle between two directions, rad. */
function angleBetween(a: Vec3, b: Vec3): number {
  return Math.atan2(norm(cross(a, b)), dot(a, b));
}

/** The star and the moon as a point sees them: their angular radii and separation, rad. */
interface SkyAt {
  readonly starRad: number;
  readonly moonRad: number;
  readonly separationRad: number;
}

/** The star (at rest at the barycentre) and a moon at `moonM` as seen from `pointM`. */
function skyAt(pointM: Vec3, moonM: Vec3): SkyAt {
  const toStar = scale(pointM, -1);
  const toMoon = sub(moonM, pointM);
  return {
    starRad: Math.asin(DISC.radius_m / norm(toStar)),
    moonRad: Math.asin(ECLIPSE_MOON_RADIUS_M / norm(toMoon)),
    separationRad: angleBetween(toStar, toMoon),
  };
}

/** The oracle's shadow class of a point: clear 0, penumbra 1, umbra 2 (the exact tangent cones). */
function shadowClass(sky: SkyAt): 0 | 1 | 2 {
  if (sky.separationRad >= sky.starRad + sky.moonRad) {
    return 0;
  }
  return sky.moonRad >= sky.starRad && sky.separationRad <= sky.moonRad - sky.starRad ? 2 : 1;
}

/** The drawn class of a disc pixel: umbra where no star lights it, penumbra where it is dimmed. */
function drawnClass(shadowed: DiscPixel, clear: DiscPixel): 0 | 1 | 2 {
  if (shadowed.meterClass === METER_CLASS.unlitBody) {
    return 2;
  }
  return shadowed.rgb[1] < clear.rgb[1] ? 1 : 0;
}

/** A record lit by its stars alone, and the same with no occluders. */
function starlitPair(record: DiscRecord): readonly [DiscRecord, DiscRecord] {
  const starlit = { ...record, secondaries: [] };
  return [starlit, { ...starlit, occluders: [] }];
}

/** The interior pixels of a raster by index (y × width + x). */
function interiorByIndex(pixels: ReadonlyArray<DiscPixel>, width: number): Map<number, DiscPixel> {
  const byIndex = new Map<number, DiscPixel>();
  for (const pixel of pixels) {
    if (pixel.draw === "interior") {
      byIndex.set(pixel.yPx * width + pixel.xPx, pixel);
    }
  }
  return byIndex;
}

/** How the drawn shadow on the planet agrees with an oracle's, pixel by pixel. */
interface ShadowAgreement {
  /** Pixels of each drawn class: clear, penumbra, umbra. */
  readonly drawn: readonly [number, number, number];
  /** Pixels whose drawn class differs from the oracle's though no neighbour's oracle class differs. */
  readonly beyondAPixel: number;
  /** Pixels whose drawn class differs from the oracle's at the oracle's boundary. */
  readonly atTheEdge: number;
}

/**
 * The planet's drawn shadow against an oracle's classes over its wholly covered pixels: a pixel may
 * differ only where one of its eight neighbours is of another class in the oracle, which is the
 * boundary to a pixel.
 *
 * @param moonAt - The moon's place the oracle takes for a point of the planet's surface.
 */
function shadowAgreement(
  drawn: Drawn,
  sceneS: number,
  moonAt: (pointM: Vec3) => Vec3,
): ShadowAgreement {
  const { camera, viewport, cameraM, plan } = drawn;
  const [starlit, unshadowed] = starlitPair(recordOf(plan, ECLIPSE_PLANET));
  const width = viewport.widthPx;
  const shadowed = interiorByIndex(rasteriseDisc(starlit, camera, viewport), width);
  const clear = interiorByIndex(rasteriseDisc(unshadowed, camera, viewport), width);
  const planetM = eclipsePlaceAt(ECLIPSE_PLANET, sceneS).centreM;
  const oracle = new Map<number, 0 | 1 | 2>();
  for (let y = 0; y < viewport.heightPx; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const point = hitSphere(
        cameraM,
        rayOf(x, y, camera, viewport),
        planetM,
        ECLIPSE_PLANET_RADIUS_M,
      );
      if (point !== null) {
        oracle.set(y * width + x, shadowClass(skyAt(point, moonAt(point))));
      }
    }
  }
  const counts: [number, number, number] = [0, 0, 0];
  let beyondAPixel = 0;
  let atTheEdge = 0;
  for (const [index, pixel] of shadowed) {
    const unlit = clear.get(index);
    const truth = oracle.get(index);
    if (unlit === undefined || truth === undefined) {
      throw new Error(`pixel ${String(index)} is drawn but not in the clear raster or the oracle`);
    }
    const seen = drawnClass(pixel, unlit);
    counts[seen] += 1;
    if (seen === truth) {
      continue;
    }
    const x = index % width;
    const y = Math.floor(index / width);
    let edge = false;
    for (let dy = -1; dy <= 1; dy += 1) {
      for (let dx = -1; dx <= 1; dx += 1) {
        const neighbour = oracle.get((y + dy) * width + (x + dx));
        edge ||= neighbour !== undefined && neighbour !== truth;
      }
    }
    if (edge) {
      atTheEdge += 1;
    } else {
      beyondAPixel += 1;
    }
  }
  return { drawn: counts, beyondAPixel, atTheEdge };
}

describe("the eclipse scene", () => {
  it("is offered by the SCENE selector", () => {
    const option = SCENE_OPTIONS.find((each) => each.name === ECLIPSE_SCENE_NAME);
    expect(option?.make().name).toBe(ECLIPSE_SCENE_NAME);
  });

  it("runs its clock a hundred times the script's", () => {
    const at = eclipseScene().sceneAt(ECLIPSE_MID_S / ECLIPSE_TIME_RATE);
    expect([at.time.seconds, at.timeRate, at.provenance]).toEqual([
      ECLIPSE_MID_S,
      ECLIPSE_TIME_RATE,
      { kind: "kept", name: ECLIPSE_SCENE_NAME },
    ]);
  });

  it("keeps its free camera 20,000 km above the planet through the script, as the view steps it", () => {
    let run = startRun(eclipseScene());
    expect(run.camera.preset).toBe("free");
    const input = { serverScene: null, dtS: 0.25, held: new Set<string>(), reducedMotion: true };
    for (let step = 0; step < (ECLIPSE_DURATION_S - 1) / 0.25; step += 1) {
      run = stepRun(run, input);
    }
    const pose = runPose(run);
    expect(pose.frame).toEqual({ kind: "body", body: ECLIPSE_PLANET });
    expect(norm(sub(pose.positionM, ECLIPSE_CAMERA_OFFSET_M))).toBeLessThan(1e-6);
    expect(norm(pose.positionM) - ECLIPSE_PLANET_RADIUS_M).toBeCloseTo(2e7, 0);
  });

  it("starts and ends its script with the moon and the giant clear of the star and the moon's shadow off the planet", () => {
    const end = ECLIPSE_DURATION_S * ECLIPSE_TIME_RATE;
    for (const sceneS of [0, end]) {
      const planetM = eclipsePlaceAt(ECLIPSE_PLANET, sceneS).centreM;
      const cameraM = add(planetM, ECLIPSE_CAMERA_OFFSET_M);
      const sky = skyAt(cameraM, retardedMoonM(cameraM, sceneS));
      expect(sky.separationRad).toBeGreaterThan(sky.starRad + sky.moonRad);
      // The giant as drawn, at the scene's time.
      const toGiant = sub(eclipsePlaceAt(ECLIPSE_GIANT, sceneS).centreM, cameraM);
      const giantRad = Math.asin(ECLIPSE_GIANT_RADIUS_M / norm(toGiant));
      expect(angleBetween(scale(cameraM, -1), toGiant)).toBeGreaterThan(sky.starRad + giantRad);
      // The shadow the planet is lit by, the moon's where its light finds the planet, passes
      // farther from the planet's centre than the planet's radius and the penumbra's radius there.
      const moonM = retardedMoonM(planetM, sceneS);
      const axis = normalise(moonM);
      const offset = sub(planetM, moonM);
      const behindM = dot(offset, axis);
      const across = norm(sub(offset, scale(axis, behindM)));
      const penumbraM = penumbraRadius(ECLIPSE_MOON_RADIUS_M, DISC.radius_m, norm(moonM), behindM);
      expect(across - penumbraM).toBeGreaterThan(ECLIPSE_PLANET_RADIUS_M);
    }
  });
});

describe("the moon's shadow on the planet (T10.c)", () => {
  const WIDE: Viewport = { widthPx: 480, heightPx: 270 };
  const NARROW: Viewport = { widthPx: 320, heightPx: 180 };

  it.each([
    ["entering", ECLIPSE_MID_S - 4_000],
    ["leaving", ECLIPSE_MID_S + 2_500],
  ])(
    "draws the umbra and the penumbra where the exact tangent cones put them, to a pixel, as the shadow is %s",
    async (_, sceneS) => {
      // 45° across 480 px: the planet's disc 288 px across, 34.5 km a pixel at the camera's foot.
      const drawn = await drawnAt(sceneS, ECLIPSE_CAMERA_POSE, 45, WIDE);
      const agreement = shadowAgreement(drawn, sceneS, (point) => retardedMoonM(point, sceneS));
      // The penumbra's whole edge is on the disc (18,052 and 24,488 px), and its umbra (12 and
      // 14 px); 8 and 44 pixels differ, each beside the oracle's boundary.
      expect(agreement.drawn[0]).toBeGreaterThan(30_000);
      expect(agreement.drawn[1]).toBeGreaterThan(15_000);
      expect(agreement.drawn[2]).toBeGreaterThan(5);
      expect(agreement.beyondAPixel).toBe(0);
    },
  );

  it("draws the umbra where the moon was when the light left it, 3 px from where it is drawn", async () => {
    // 10° across 320 px: 10.9 km a pixel at the camera's foot, the umbra 155 km across.
    const sceneS = ECLIPSE_MID_S;
    const drawn = await drawnAt(sceneS, ECLIPSE_CAMERA_POSE, 10, NARROW);
    const retarded = shadowAgreement(drawn, sceneS, (point) => retardedMoonM(point, sceneS));
    // The umbra's 160 px, every one of them the oracle's.
    expect(retarded.drawn[2]).toBeGreaterThan(150);
    expect(retarded.beyondAPixel).toBe(0);
    // The moon where it is drawn, at the scene's time: the moon's 28.7 km/s over its 1.21 s light
    // time to the planet puts that shadow 34.8 km, 3.2 px, off, which the comparison sees (40
    // pixels beyond a pixel of its boundary).
    const drawnMoon = eclipsePlaceAt(ECLIPSE_MOON, sceneS).centreM;
    const instantaneous = shadowAgreement(drawn, sceneS, () => drawnMoon);
    expect(instantaneous.beyondAPixel).toBeGreaterThan(20);
  });
});

/** The probe's pixel, the centre of a 3 × 3 view, as its disc's interior draw writes it. */
function probePixel(pixels: ReadonlyArray<DiscPixel>): DiscPixel {
  const centre = pixels.find((p) => p.xPx === 1 && p.yPx === 1 && p.draw === "interior");
  if (centre === undefined) {
    throw new Error("the probe's pixel is not drawn");
  }
  return centre;
}

describe("a probe point's light through the moon's shadow (T10.c)", () => {
  /** A 3 × 3 view a milliradian across, its centre pixel's ray on the camera's axis. */
  const PROBE_VIEW: Viewport = { widthPx: 3, heightPx: 3 };
  const PROBE_FOV_DEG = 0.06;

  /**
   * The eclipse term's worst error at the setting's annuli for the scene's star's law (Claret and
   * Southworth 2022's V-band power-2 law in every channel, `sunLikeHostDisc`): T6.b's bounds for
   * the Sun's V, 0.58% at K = 4 and 1.02% at K = 3, under which this law's worst on T6.b's grid
   * (0.56% and 0.98%) lies.
   */
  const SETTING_ERROR = [
    ["high", DISC_ANNULI_HIGH, 5.8e-3],
    ["low", DISC_ANNULI_LOW, 1.02e-2],
  ] as const;

  it.each(SETTING_ERROR)(
    "keeps, on the %s setting, the share of its clear light the oracle gives over the crossing, to the setting's error",
    async (_, annuli, error) => {
      let worst = 0;
      const shares: number[] = [];
      for (let sceneS = ECLIPSE_MID_S - 3_700; sceneS <= ECLIPSE_MID_S + 3_700; sceneS += 100) {
        // Each step is a frame of its own, drawn in turn.
        // oxlint-disable-next-line no-await-in-loop
        const drawn = await drawnAt(sceneS, ECLIPSE_CAMERA_POSE, PROBE_FOV_DEG, PROBE_VIEW, annuli);
        const [starlit, unshadowed] = starlitPair(recordOf(drawn.plan, ECLIPSE_PLANET));
        const shadowed = probePixel(rasteriseDisc(starlit, drawn.camera, PROBE_VIEW));
        const clear = probePixel(rasteriseDisc(unshadowed, drawn.camera, PROBE_VIEW));
        const planetM = eclipsePlaceAt(ECLIPSE_PLANET, sceneS).centreM;
        const point = hitSphere(
          drawn.cameraM,
          rayOf(1, 1, drawn.camera, PROBE_VIEW),
          planetM,
          ECLIPSE_PLANET_RADIUS_M,
        );
        if (point === null) {
          throw new Error("the probe's ray misses the planet");
        }
        const sky = skyAt(point, retardedMoonM(point, sceneS));
        // The display's r, g and b take R06's R, V and B laws.
        for (const [c, law] of [
          [0, DISC.limb[2]],
          [1, DISC.limb[1]],
          [2, DISC.limb[0]],
        ] as const) {
          const truth = eclipseIntegralVisibleFraction(
            law.c,
            law.alpha,
            sky.moonRad / sky.starRad,
            sky.separationRad / sky.starRad,
          );
          const share = shadowed.rgb[c] / clear.rgb[c];
          worst = Math.max(worst, Math.abs(share - truth));
          if (c === 1) {
            shares.push(share);
          }
        }
      }
      // 0.29% at worst on the high setting, 0.48% on the low.
      expect(worst).toBeLessThan(error);
      // Through the whole crossing: clear at both ends, and total at its middle (the probe's
      // umbra lasts 158 s).
      expect([shares[0], shares.at(-1)]).toEqual([1, 1]);
      expect(Math.min(...shares)).toBe(0);
    },
  );
});

/** Whether a point of the view falls inside the star's disc as R06's pass draws it: sin θ < sin ρ. */
function inStarDisc(drawn: Drawn, x: number, y: number): boolean {
  const light = drawn.frame.lights[0];
  if (light === undefined) {
    throw new Error("the frame has no light");
  }
  const axis = normalise(light.centreM);
  const sinRho = DISC.radius_m / norm(light.centreM);
  const ray = rotate(drawn.camera.orientation, viewRay(x, y, drawn.camera, drawn.viewport));
  return dot(ray, axis) > 0 && norm(cross(ray, axis)) < sinRho;
}

/**
 * The painter's step that draws `body` (its disc, a mesh body's limb, or the run of points it is
 * in), or −1.
 */
function bodyStep(plan: BodyFramePlan, body: BodyIdHex): number {
  return plan.steps.findIndex((step) => {
    let drawsIt: boolean;
    switch (step.kind) {
      case "disc":
        drawsIt = plan.discs[step.index]?.body === body;
        break;
      case "points":
        drawsIt = step.sprites.some((sprite) => sprite.id === body);
        break;
      case "limb": {
        const mesh = plan.meshes[step.mesh];
        drawsIt = mesh !== undefined && plan.discs[mesh.index]?.body === body;
        break;
      }
      case "host":
        drawsIt = false;
        break;
    }
    return drawsIt;
  });
}

/** The painter's step that places the star's disc. */
function starStep(plan: BodyFramePlan): number {
  return plan.steps.findIndex((step) => step.kind === "host" && step.star === DISC.star);
}

describe("the bodies before and behind the star (T10.c)", () => {
  const ZOOMED: Viewport = { widthPx: 320, heightPx: 180 };
  const FULL: Viewport = { widthPx: 1920, heightPx: 1080 };

  it("covers the giant behind the star with the star's disc, drawn after it", async () => {
    // 2° across 320 px: the star 85 px across, the giant 8 px, a disc.
    const drawn = await drawnAt(
      ECLIPSE_CONJUNCTION_S,
      eclipsePoseAtStar(ECLIPSE_CONJUNCTION_S),
      2,
      ZOOMED,
    );
    expect(drawn.plan.regimes.get(ECLIPSE_GIANT)).toBe("disc");
    expect(bodyStep(drawn.plan, ECLIPSE_GIANT)).toBeLessThan(starStep(drawn.plan));
    const giant = rasteriseDisc(recordOf(drawn.plan, ECLIPSE_GIANT), drawn.camera, ZOOMED);
    // Its 68 pixels, each drawn over by the star's disc, opaque, at its later step.
    expect(giant.length).toBeGreaterThan(40);
    expect(giant.every((p) => inStarDisc(drawn, p.xPx + 0.5, p.yPx + 0.5))).toBe(true);
  });

  it("hides the giant behind the star as a mesh too, the star's disc on its limb's plane", async () => {
    const sceneS = ECLIPSE_CONJUNCTION_S;
    const pose = eclipsePoseAtStar(sceneS);
    const drawn = await drawnAt(sceneS, pose, 2, ZOOMED);
    const giant = drawn.frame.bodies.find((body) => body.id === ECLIPSE_GIANT);
    // R07.T9's synthetic depth writer over the giant promotes it: its figure writes its depth.
    const writer =
      giant === undefined
        ? null
        : sphereFootprint(giant.centreM, giant.figure.equatorialRadiusM, drawn.camera, ZOOMED);
    if (writer === null) {
      throw new Error("the giant is not on the view");
    }
    const plan = planLitBodies(
      drawn.frame.bodies,
      drawn.frame.lights,
      {
        camera: drawn.camera,
        viewport: ZOOMED,
        exposureScale: EXPOSURE,
        annuli: DISC_ANNULI_HIGH,
        planetshine: PLANETSHINE_SOURCES_HIGH,
        depthWriters: [writer],
        setting: "high",
      },
      new Map(),
    );
    // The record the layer draws.
    const layer = new HostDiscLayer(await countingRenderEngine());
    const placements = hostPlacements(eclipseSceneAt(sceneS), [DISC], pose);
    const record = layer.frame(placements, drawn.camera, ZOOMED, EXPOSURE).draws[0]?.record;
    if (record === undefined) {
      throw new Error("the eclipse scene draws no star's disc");
    }
    /** The pixels the giant keeps a share of, with the star's disc drawn as `star`. */
    const giantPixels = (star: HostDiscRecord): number =>
      [
        ...compositeBodyFrame(
          plan,
          drawn.camera,
          ZOOMED,
          new Map([[DISC.star, [star]]]),
        ).pixels.values(),
      ].filter((pixel) => (pixel.shares.get(ECLIPSE_GIANT) ?? 0) > 0).length;
    expect(plan.regimes.get(ECLIPSE_GIANT)).toBe("mesh");
    expect(bodyStep(plan, ECLIPSE_GIANT)).toBeLessThan(starStep(plan));
    expect(giantPixels(record)).toBe(0);
    // At infinity, as R06 drew the disc until this follow-up, the figure's depth hid the disc: the
    // giant then shows in its 32 wholly covered pixels.
    expect(giantPixels({ ...record, inverseLimbDistancePerM: 0 })).toBeGreaterThan(20);
  });

  it("leaves the part of the giant that is not yet behind the star uncovered", async () => {
    // 5,600 s before: the giant's centre 4.7 mrad off the star's, at its limb.
    const sceneS = ECLIPSE_CONJUNCTION_S - 5_600;
    const drawn = await drawnAt(sceneS, eclipsePoseAtStar(sceneS), 2, ZOOMED);
    const giant = rasteriseDisc(recordOf(drawn.plan, ECLIPSE_GIANT), drawn.camera, ZOOMED);
    const outside = giant.filter((p) => !inStarDisc(drawn, p.xPx + 0.5, p.yPx + 0.5));
    // 38 of its 68 pixels.
    expect(outside.length).toBeGreaterThan(10);
    expect(outside.length).toBeLessThan(giant.length - 10);
  });

  it("covers the giant as a point at 60° behind the star's disc, its sprite drawn first", async () => {
    const drawn = await drawnAt(
      ECLIPSE_CONJUNCTION_S,
      eclipsePoseAtStar(ECLIPSE_CONJUNCTION_S),
      60,
      FULL,
    );
    expect(drawn.plan.regimes.get(ECLIPSE_GIANT)).toBe("point");
    expect(bodyStep(drawn.plan, ECLIPSE_GIANT)).toBeLessThan(starStep(drawn.plan));
    const giant = drawn.frame.bodies.find((body) => body.id === ECLIPSE_GIANT);
    if (giant === undefined) {
      throw new Error("the frame has no giant");
    }
    const at = project(giant.centreM, drawn.camera, FULL);
    // Its point-spread sprite's quad, every pixel of it inside the star's 15.5 px disc.
    const half = (PSF_QUAD_PX - 1) / 2;
    const quad: Array<readonly [number, number]> = [];
    for (let dy = -half; dy <= half; dy += 1) {
      for (let dx = -half; dx <= half; dx += 1) {
        quad.push([Math.floor(at.xPx) + dx + 0.5, Math.floor(at.yPx) + dy + 0.5]);
      }
    }
    expect(quad.every(([x, y]) => inStarDisc(drawn, x, y))).toBe(true);
  });

  it("covers the star's disc with the moon's as the overlap of the two circles, the moon drawn after it", async () => {
    // From inside the penumbra, 2,000 s before mid-eclipse: the moon 5.9 mrad off the star.
    const sceneS = ECLIPSE_MID_S - 2_000;
    const drawn = await drawnAt(sceneS, eclipsePoseAtStar(sceneS), 2, ZOOMED);
    expect(bodyStep(drawn.plan, ECLIPSE_MOON)).toBeGreaterThan(starStep(drawn.plan));
    const coverage = new Map<number, number>();
    for (const p of rasteriseDisc(recordOf(drawn.plan, ECLIPSE_MOON), drawn.camera, ZOOMED)) {
      const index = p.yPx * ZOOMED.widthPx + p.xPx;
      coverage.set(index, Math.min(1, (coverage.get(index) ?? 0) + p.coverage));
    }
    let starPixels = 0;
    let covered = 0;
    for (let y = 0; y < ZOOMED.heightPx; y += 1) {
      for (let x = 0; x < ZOOMED.widthPx; x += 1) {
        if (inStarDisc(drawn, x + 0.5, y + 0.5)) {
          starPixels += 1;
          covered += coverage.get(y * ZOOMED.widthPx + x) ?? 0;
        }
      }
    }
    const sky = skyAt(drawn.cameraM, eclipsePlaceAt(ECLIPSE_MOON, sceneS).centreM);
    const overlap =
      circleOverlapArea(1, sky.moonRad / sky.starRad, sky.separationRad / sky.starRad) / Math.PI;
    // 0.3192 of the star's 5,712 pixels against the circles' 0.3189.
    expect(overlap).toBeGreaterThan(0.2);
    expect(overlap).toBeLessThan(0.8);
    expect(Math.abs(covered / starPixels - overlap)).toBeLessThan(0.01);
  });

  it("dims the moon's planetshine by the planet's eclipse as the moon sees it (T10.b)", async () => {
    const drawn = await drawnAt(ECLIPSE_MID_S, eclipsePoseAtStar(ECLIPSE_MID_S), 2, ZOOMED);
    const moon = drawn.frame.bodies.find((body) => body.id === ECLIPSE_MOON);
    if (moon === undefined) {
      throw new Error("the frame has no moon");
    }
    const neighbours = litNeighbours(drawn.frame.bodies, drawn.frame.lights, DISC_ANNULI_HIGH);
    const unshadowed = neighbours.map((neighbour) => ({ ...neighbour, shadowing: [] }));
    const shine = (list: typeof neighbours): number =>
      planetshineSources(moon, list, PLANETSHINE_SOURCES_HIGH).find(
        (source) => source.body === ECLIPSE_PLANET,
      )?.illuminance[1] ?? 0;
    const kept = shine(neighbours) / shine(unshadowed);
    // A central solar eclipse keeps about 0.89 of a Lambert planet's light towards its moon
    // (0.892 here; T10.b's 0.892 at 384,400 km under a uniform Sun).
    expect(kept).toBeGreaterThan(0.87);
    expect(kept).toBeLessThan(0.91);
  });
});
