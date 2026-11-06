import { describe, expect, it } from "vitest";

import { add, norm, normalise, scale, sub, vec3 } from "../../geometry/vec3";
import {
  AIR_SWITCH_FIXTURES,
  type AirSwitchBody,
  airSwitchBodyOf,
  airSwitchFixtureOf,
} from "../../test/airSwitchCases";
import { aHostDisc } from "../../test/litFixtures";
import type { DiscAir } from "../atmosphere/discReflectance";
import { DISC_LAYER_VALUES } from "../atmosphere/discTable";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import { rotation3FromRows } from "../coords/rotation";
import { DISC_ANNULI_HIGH } from "../lighting/annuli";
import { discEclipseVisible } from "../lighting/discEclipse";
import { hostAnnuli, type PlacedLight } from "../lighting/hostLights";
import { starIlluminance } from "../lighting/illuminance";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import { limbShareAt } from "../appearance/law";
import type { Rgb } from "../photometry/toneCurve";
import { AU_M } from "../scenes/kept";
import {
  type AirPointBody,
  airPointRatio,
  airPointWork,
  AIR_POINT_NEAR_SLICES,
  cancelAirPointsOf,
  stepAirPoints,
} from "./airPoint";
import { makeClassMapPointNow, stepClassMapPoints } from "./classMapPoint";
import { type LitBodyInput, planLitBodies, pointFlux } from "./draw";
import { bodyReflection } from "./oblate";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
const RAD = Math.PI / 180;
const OPTIONS = {
  camera: CAMERA,
  viewport: VIEWPORT,
  presentedPx: VIEWPORT,
  exposureScale: 1e-3,
  annuli: DISC_ANNULI_HIGH,
  planetshine: PLANETSHINE_SOURCES_HIGH,
  setting: "high" as const,
  drawsAir: true,
};

/** A rotation whose body-fixed z, a class map's pole, is the view's +y. */
const POLE_UP = rotation3FromRows([vec3(1, 0, 0), vec3(0, 0, 1), vec3(0, -1, 0)]);

/** A case's body, its table decoded anew, so that no other test's held ratios are its own. */
function caseOf(name: keyof typeof AIR_SWITCH_FIXTURES): AirSwitchBody {
  return airSwitchBodyOf(airSwitchFixtureOf(AIR_SWITCH_FIXTURES[name].json));
}

/** Runs every queued ratio to its end. */
function drain(): void {
  while (stepAirPoints(Number.POSITIVE_INFINITY, () => 0, "all")) {
    // Every queued ratio found.
  }
}

/** The case's body `px` across ahead, lit by a Sun 1 au off at `phaseDeg`, turned by `turnRad`. */
function sceneOf(
  item: AirSwitchBody,
  px: number,
  phaseDeg: number,
  turnRad = 0,
): { readonly body: LitBodyInput; readonly hosts: PlacedLight[] } {
  const distanceM = item.figure.equatorialRadiusM / Math.sin(px / 2 / PX_PER_RAD);
  const centreM = vec3(0, 0, -distanceM);
  const phase = phaseDeg * RAD + turnRad;
  const towards = vec3(Math.sin(phase), 0, Math.cos(phase));
  return {
    body: {
      id: "0200080020000000.0300",
      centreM,
      figure: { ...item.figure, pole: vec3(0, 1, 0) },
      photometry: item.photometry,
      lighting: undefined,
      air: item.air,
      ...(item.surface === undefined ? {} : { surface: item.surface, rotation: POLE_UP }),
    },
    hosts: [{ disc: aHostDisc(), centreM: add(centreM, scale(towards, AU_M)) }],
  };
}

/** The body's point flux as before its air: E (a ÷ Δ)² × its law over its figure, eclipsed. */
function airlessFlux(body: LitBodyInput, hosts: ReadonlyArray<PlacedLight>): Rgb {
  const distanceM = norm(body.centreM);
  const toCamera = scale(body.centreM, -1 / distanceM);
  const solid = (body.figure.equatorialRadiusM / distanceM) ** 2;
  const flux: [number, number, number] = [0, 0, 0];
  for (const host of hosts) {
    const toStar = sub(host.centreM, body.centreM);
    const e = starIlluminance(host.disc, norm(toStar));
    const reflected = bodyReflection(body.figure, body.photometry.law, normalise(toStar), toCamera);
    const visible = discEclipseVisible(
      host,
      body,
      [],
      toCamera,
      (alpha) => limbShareAt(body.photometry.law, alpha),
      DISC_ANNULI_HIGH,
    );
    for (const c of [0, 1, 2] as const) {
      flux[c] += e[c] * solid * reflected[c] * visible[c];
    }
  }
  return flux;
}

/** The point's samples' body for `body` and its one host, as `pointFlux` forms it. */
function sampledOf(body: LitBodyInput, host: PlacedLight, air: DiscAir): AirPointBody {
  const toStar = sub(host.centreM, body.centreM);
  const law = body.photometry.law;
  const figure = {
    equatorialRadiusM: body.figure.equatorialRadiusM,
    polarRadiusM: body.figure.polarRadiusM,
  };
  return {
    air,
    figure,
    surface: { kind: "uniform", law },
    airedFigure: figure,
    airedSurface: { kind: "uniform", law },
    pole: vec3(0, 1, 0),
    rotation: null,
    classMap: null,
    towardsObserver: scale(body.centreM, -1 / norm(body.centreM)),
    lights: [
      {
        towardsLight: normalise(toStar),
        distanceM: norm(toStar),
        radiusM: host.disc.radius_m,
        annuli: hostAnnuli(host.disc, DISC_ANNULI_HIGH),
        key: `star ${String(host.disc.star)}`,
      },
    ],
    airlessIsDisc: true,
  };
}

describe("an air body's point (R07.T8.a, R08.T16.b; airPoint.ts)", () => {
  it("draws a body given no disc table as before, bit for bit", () => {
    drain();
    const { body, hosts } = sceneOf(caseOf("test-earth"), 2.99, 60);
    const airless: LitBodyInput = {
      id: body.id,
      centreM: body.centreM,
      figure: body.figure,
      photometry: body.photometry,
      lighting: undefined,
    };
    const before = airPointWork().refreshes;
    expect(pointFlux(airless, hosts, [], DISC_ANNULI_HIGH)).toEqual(airlessFlux(airless, hosts));
    expect(airPointWork().refreshes).toBe(before);
  });

  it("finds no ratio on the frame: its first draws the airless point and queues one a star", () => {
    drain();
    const { body, hosts } = sceneOf(caseOf("test-earth"), 2.99, 60);
    expect(pointFlux(body, hosts, [], DISC_ANNULI_HIGH)).toEqual(airlessFlux(body, hosts));
    expect(airPointWork().refreshes).toBe(1);
    // A second frame at the same geometry queues no second.
    pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    expect(airPointWork().refreshes).toBe(1);
  });

  it("draws, once the slices find it, its airless point times the tabled disc's sum over the airless disc's", () => {
    drain();
    const item = caseOf("test-earth");
    const { body, hosts } = sceneOf(item, 2.99, 60);
    const host = hosts[0];
    if (host === undefined) {
      throw new Error("the scene has a host");
    }
    pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    drain();
    const { ratio } = airPointRatio(sampledOf(body, host, item.air), 0);
    const airless = airlessFlux(body, hosts);
    const flux = pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    for (const c of [0, 1, 2] as const) {
      expect(flux[c]).toBeCloseTo(airless[c] * ratio[c], 12);
    }
    // The measured Earth's air is bluer than its grey ground and lights more than it.
    expect([ratio[0] > 1, ratio[2] > ratio[0]]).toEqual([true, true]);
  });

  it("keeps a held ratio within the cache's angle, and queues none", () => {
    drain();
    const item = caseOf("test-mars");
    const at = sceneOf(item, 2.99, 60);
    pointFlux(at.body, at.hosts, [], DISC_ANNULI_HIGH);
    drain();
    const held = pointFlux(at.body, at.hosts, [], DISC_ANNULI_HIGH);
    const turned = sceneOf(item, 2.99, 60, 1e-5);
    const flux = pointFlux(turned.body, turned.hosts, [], DISC_ANNULI_HIGH);
    const ratioOf = (f: Rgb, scene: typeof at): number =>
      f[1] / airlessFlux(scene.body, scene.hosts)[1];
    expect(ratioOf(flux, turned)).toBe(ratioOf(held, at));
    expect(airPointWork().refreshes).toBe(0);
  });

  it("stands a held ratio in past the cache's angle while the slices find a fresh one", () => {
    drain();
    const item = caseOf("test-mars");
    const at = sceneOf(item, 2.99, 60);
    pointFlux(at.body, at.hosts, [], DISC_ANNULI_HIGH);
    drain();
    const held = pointFlux(at.body, at.hosts, [], DISC_ANNULI_HIGH);
    const turned = sceneOf(item, 2.99, 60, 0.05);
    const standIn = pointFlux(turned.body, turned.hosts, [], DISC_ANNULI_HIGH);
    const ratioOf = (f: Rgb, scene: typeof at): number =>
      f[1] / airlessFlux(scene.body, scene.hosts)[1];
    expect([ratioOf(standIn, turned), airPointWork().refreshes]).toEqual([ratioOf(held, at), 1]);
    drain();
    const fresh = pointFlux(turned.body, turned.hosts, [], DISC_ANNULI_HIGH);
    expect(ratioOf(fresh, turned)).not.toBe(ratioOf(held, at));
  });

  it("begins a disc's ratios near the switch, and none for a disc past the band", () => {
    drain();
    const item = caseOf("test-titan");
    const near = sceneOf(item, 6, 30);
    planLitBodies([near.body], near.hosts, OPTIONS, new Map([[near.body.id, "disc"]]));
    expect(airPointWork().refreshes).toBe(1);
    drain();
    // Its first frame as a point finds the ratio held: it queues none.
    const point = sceneOf(item, 2.99, 30);
    pointFlux(point.body, point.hosts, [], DISC_ANNULI_HIGH);
    expect(airPointWork().refreshes).toBe(0);
    const far = sceneOf(caseOf("test-titan"), 40, 30);
    planLitBodies([far.body], far.hosts, OPTIONS, new Map([[far.body.id, "disc"]]));
    expect(airPointWork().refreshes).toBe(0);
  });

  it("steps within its budget, and in the near scope only the tables drawn near lately", () => {
    drain();
    const { body, hosts } = sceneOf(caseOf("test-earth"), 2.99, 45);
    pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    // A clock that moves 1 ms a read: the slice stops at the first step past 2 ms, work left.
    let ms = 0;
    const clock = (): number => {
      ms += 1;
      return ms;
    };
    expect(stepAirPoints(2, clock, "near")).toBe(true);
    // Past the notes' slices with no frame drawing it, the near scope steps none of it.
    for (let i = 0; i < AIR_POINT_NEAR_SLICES; i += 1) {
      stepAirPoints(0, clock, "all");
    }
    expect(stepAirPoints(Number.POSITIVE_INFINITY, clock, "near")).toBe(false);
    expect(airPointWork().refreshes).toBe(1);
    drain();
    expect(airPointWork().refreshes).toBe(0);
  });

  it("draws a class-mapped body's point from the tabled sum, whether or not its map's point is made", () => {
    drain();
    const item = caseOf("served-class-map");
    const map = item.surface?.kind === "class-map" ? item.surface : null;
    if (map === null) {
      throw new Error("the case is drawn under a class map");
    }
    const { body, hosts } = sceneOf(item, 2.99, 60);
    pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    drain();
    const unmade = pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    makeClassMapPointNow(map);
    while (stepClassMapPoints(Number.POSITIVE_INFINITY, () => 0, "all")) {
      // The map's own ratios found.
    }
    expect(pointFlux(body, hosts, [], DISC_ANNULI_HIGH)).toEqual(unmade);
  });

  it("drops a class map's queued and held ratios as the map is dropped, and finds them anew", () => {
    drain();
    const item = caseOf("served-class-map");
    const map = item.surface?.kind === "class-map" ? item.surface : null;
    if (map === null) {
      throw new Error("the case is drawn under a class map");
    }
    const { body, hosts } = sceneOf(item, 2.99, 60);
    pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    cancelAirPointsOf(map.texels);
    expect(airPointWork().refreshes).toBe(0);
    pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    drain();
    // Held: a second frame queues none; dropped with its map, the next queues its ratio again.
    pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    const held = airPointWork().refreshes;
    cancelAirPointsOf(map.texels);
    pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    expect([held, airPointWork().refreshes]).toEqual([0, 1]);
    drain();
  });

  it("draws a black point for a table that holds no light and no transmittance", () => {
    drain();
    const item = caseOf("test-earth");
    const black: DiscAir = {
      ...item.air,
      table: { ...item.air.table, values: new Float32Array(DISC_LAYER_VALUES) },
    };
    const { body, hosts } = sceneOf({ ...item, air: black }, 2.99, 60);
    pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    drain();
    expect(pointFlux(body, hosts, [], DISC_ANNULI_HIGH)).toEqual([0, 0, 0]);
  });
});
