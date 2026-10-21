import { describe, expect, it } from "vitest";

import { norm, normalise, scale, sub } from "../../../geometry/vec3";
import { rotate } from "../../../view/camera/quaternion";
import { rotateToBody } from "../../../view/coords/rotation";
import { limbDistanceM } from "../../../view/terrain/handover";
import { terrainMoon, terrainTestGolden } from "../../../view/scenes/terrainScene";
import { SCENE_OPTIONS } from "../viewRun";
import { HANDOVER_APPROACH, HANDOVER_LOOKS, type HandoverLook } from "./handoverGeometry";
import { handoverRangeAtM, HandoverScenes, terrainTestHandoverWorld } from "./handoverScenes";
import { TerrainProbe } from "./terrainProbe";
import { ViewsProbe } from "./viewsProbe";

const LOOK_SCENES = ["HAND-OVER 30°", "HAND-OVER 90°", "HAND-OVER TERMINATOR"];

/** The range of the high setting's primary, 1126 px across. */
const RANGE_M = 1.5419e7;

/** The look `index`. */
function lookAt(index: number): HandoverLook {
  const look = HANDOVER_LOOKS[index];
  if (look === undefined) {
    throw new Error(`no look ${String(index)}`);
  }
  return look;
}

/** A provisional range, m: other than the run's. */
const PROVISIONAL_M = 9.0e6;

/** The scenes over `TERRAIN TEST`'s world, their range set. */
function scenes(): HandoverScenes {
  const made = new HandoverScenes(
    terrainTestHandoverWorld(),
    HANDOVER_APPROACH.full,
    PROVISIONAL_M,
  );
  made.setRangeM(RANGE_M);
  return made;
}

/** The own ship's distance to the moon's limb in the first look's scene at script time `tS`, m. */
function shipLimbM(made: HandoverScenes, tS: number): number {
  const look = HANDOVER_LOOKS[0];
  const position = made.sceneOptions[0]?.make().sceneAt(tS).craft[0]?.pose.position;
  if (look === undefined || position?.kind !== "body_fixed") {
    throw new Error("the first look's ship is not placed in the moon's axes");
  }
  const galactic = rotateToBody(made.geometry(look).rotation, position.m);
  return limbDistanceM(norm(galactic), terrainTestGolden().header.equatorial_radius_m);
}

describe("TERRAIN TEST's hand-over world", () => {
  it.each([
    ["high", 1126, 15_419],
    ["low", 822, 5_628],
  ] as const)("hands over on %s at %i px from %i km", (setting, widthPx, rangeKm) => {
    const range = handoverRangeAtM(terrainTestHandoverWorld(), setting, widthPx);
    expect(Math.round(range.rangeM / 1000)).toBe(rangeKm);
  });

  it("is the terrain golden's moon, its relief, its level and its whole survey's 48 cells", () => {
    const world = terrainTestHandoverWorld();
    const { header } = terrainTestGolden();
    expect([world.body, world.reliefM, world.cellLevel, world.surveyedCells]).toEqual([
      terrainMoon(),
      header.realised_relief_m,
      6,
      48,
    ]);
  });
});

describe("the hand-over check's scenes", () => {
  it("are one a look, in the looks' order", () => {
    expect(scenes().sceneOptions.map((option) => option.name)).toEqual(LOOK_SCENES);
  });

  it("are no kept scene of an ordinary launch", () => {
    expect(SCENE_OPTIONS.filter((option) => LOOK_SCENES.includes(option.name))).toEqual([]);
  });

  it("are offered by the terrain probe after VIEW's kept scenes only when given", () => {
    const plain = new TerrainProbe(new ViewsProbe(), { paceMs: 0, readBackMs: 1_000 });
    const checked = new TerrainProbe(new ViewsProbe(), {
      paceMs: 0,
      readBackMs: 1_000,
      checkScenes: scenes().sceneOptions,
    });
    expect([
      plain.sceneOptions.map((option) => option.name),
      checked.sceneOptions.map((option) => option.name),
    ]).toEqual([
      SCENE_OPTIONS.map((option) => option.name),
      [...SCENE_OPTIONS.map((option) => option.name), ...LOOK_SCENES],
    ]);
  });

  it("draw a look chosen before the run sets its range at the provisional range", () => {
    const unset = new HandoverScenes(
      terrainTestHandoverWorld(),
      HANDOVER_APPROACH.full,
      PROVISIONAL_M,
    );
    expect(shipLimbM(unset, 0) / PROVISIONAL_M).toBeCloseTo(3, 9);
  });

  it("draw a look at the run's range once it is set, in the provisional's place", () => {
    const made = new HandoverScenes(
      terrainTestHandoverWorld(),
      HANDOVER_APPROACH.full,
      PROVISIONAL_M,
    );
    made.sceneOptions[0]?.make();
    made.setRangeM(RANGE_M);
    expect(shipLimbM(made, 0) / RANGE_M).toBeCloseTo(3, 9);
  });

  it("refuse a provisional range that is not positive", () => {
    expect(() => new HandoverScenes(terrainTestHandoverWorld(), HANDOVER_APPROACH.full, 0)).toThrow(
      /provisional hand-over range must be positive/,
    );
  });

  it("hold the moon still at the look's rotation, the clock at one second a second", () => {
    const made = scenes();
    const scene = made.sceneOptions[1]?.make().sceneAt(40);
    const moon = scene?.bodies.find((body) => body.id === terrainMoon());
    expect([scene?.timeRate, moon?.rotation]).toEqual([1, made.geometry(lookAt(1)).rotation]);
  });

  it("start the own ship at 3 times the range from the limb, on the line to the moon's centre", () => {
    const made = scenes();
    const geometry = made.geometry(lookAt(0));
    const ship = made.sceneOptions[0]?.make().sceneAt(0).craft[0];
    const position = ship?.pose.position;
    if (position?.kind !== "body_fixed") {
      throw new Error("the ship is not placed in the moon's axes");
    }
    const galactic = rotateToBody(geometry.rotation, position.m);
    const { header } = terrainTestGolden();
    expect([
      limbDistanceM(norm(galactic), header.equatorial_radius_m) / RANGE_M,
      norm(sub(normalise(galactic), geometry.towardCamera)),
    ]).toEqual([expect.closeTo(3, 9), expect.closeTo(0, 12)]);
  });

  it("look the seat at the moon's centre", () => {
    const made = scenes();
    const geometry = made.geometry(lookAt(2));
    const camera = made.sceneOptions[2]?.make().cameraAt(0);
    const forward = camera === undefined ? null : rotate(camera.orientation, { x: 0, y: 0, z: -1 });
    expect(
      forward === null ? null : norm(sub(forward, scale(geometry.towardCamera, -1))),
    ).toBeCloseTo(0, 12);
  });

  it("follow each look's script as its stage plays it, afresh at each choice", () => {
    const made = scenes();
    const option = made.sceneOptions[0];
    option?.terrain?.notePlayed?.(12.5);
    const played = made.scriptS(lookAt(0));
    option?.make();
    expect([played, made.scriptS(lookAt(0)), made.scriptS(lookAt(1))]).toEqual([12.5, null, null]);
  });
});
