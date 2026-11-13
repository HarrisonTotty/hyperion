/**
 * The whole-body run's check-only looks (plan R10, R10.T14.c, the owner's request;
 * research-leans-20 §2): `WHOLE ORBIT`, `WHOLE CUBE CORNER` and `WHOLE ANTIPODE`, `TEST MOON`'s whole
 * body seen at full phase from three fixed places, which a `--terrain --whole` launch alone offers
 * in `VIEW`, and how a look's frame is read.
 *
 * @remarks
 * - **Each look** holds `TEST MOON` still, turned so that the body-fixed direction the look stands
 *   over faces the star, and holds the own ship, `TEST HULL`, on that line, its seat looking at the
 *   moon's centre. The star stands behind the camera, so the disc in view is lit wholly, and a
 *   pixel of it that draws no ground reads as the sky, which `TERRAIN TEST` leaves black (no star
 *   field, the star itself behind the camera).
 * - `WHOLE ORBIT` stands 4 radii over the old patch's first survey, the whole disc and its limb in
 *   view; `WHOLE CUBE CORNER` 1.5 radii over (1, 1, 1), where faces +x, +y and +z meet, the disc
 *   past the frame; `WHOLE ANTIPODE` 2.5 radii over the first survey's antipode.
 * - **Each look has its own fixture**, the whole body's, unpaced, so each choice makes a fresh
 *   session served the whole field at its first ask.
 * - **The read** ({@link readWholeDisc}): over a region of the primary's photorealistic scene
 *   target, the pixels well inside the disc, short of the datum's spheroid by the field's relief and
 *   a pixel, that read the sky's black; and the pixels well beyond its limb, past the spheroid by
 *   the relief and two pixels, that read lit.
 * - **Never on an ordinary launch**: `App` makes these scenes on a `--whole` launch alone and hands
 *   them to the terrain probe's kept scenes (`TerrainProbeOptions.checkScenes`).
 */

import type { TerrainWholeLookName } from "../../../../../preload/api";
import { cross, dot, normalise, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import type { CameraPose } from "../../../view/camera/pose";
import { lookAlong, rotate } from "../../../view/camera/quaternion";
import { type Rotation3, rotation3FromRows } from "../../../view/coords/rotation";
import { TEST_HULL } from "../../../view/scene/hull";
import type { ViewScene } from "../../../view/scene/model";
import { type KeptScene, keptTime } from "../../../view/scenes/kept";
import {
  TERRAIN_TEST_SHIP,
  terrainSceneAt,
  terrainSceneFixture,
  terrainTestGolden,
} from "../../../view/scenes/terrainScene";
import type { SceneOption } from "../viewRun";
import type { PixelRect } from "./handoverReading";

/** One of the whole-body run's looks. */
export interface WholeLook {
  readonly name: TerrainWholeLookName;
  /** Its kept scene's name in `VIEW`'s `SCENE` selector. */
  readonly scene: string;
  /** The body-fixed direction the camera stands over, not normalised. */
  readonly over: Vec3;
  /** The camera's distance from the moon's centre, equatorial radii. */
  readonly radii: number;
}

/** The old patch's first survey's centre, body-fixed, not normalised (`terrain_test.rs`'). */
const FIRST_SURVEY_CENTRE = vec3(1, 1, 0.8);

/** The looks, in the order the run makes them. */
export const WHOLE_LOOKS: ReadonlyArray<WholeLook> = [
  { name: "orbit", scene: "WHOLE ORBIT", over: FIRST_SURVEY_CENTRE, radii: 4 },
  { name: "cube-corner", scene: "WHOLE CUBE CORNER", over: vec3(1, 1, 1), radii: 1.5 },
  {
    name: "antipode",
    scene: "WHOLE ANTIPODE",
    over: scale(FIRST_SURVEY_CENTRE, -1),
    radii: 2.5,
  },
];

/** The star's direction from `TEST MOON`'s centre, galactic: the moon stands 1 au out along +x. */
const TOWARD_STAR = vec3(-1, 0, 0);

/** The galactic up the looks are framed by. */
const UP = vec3(0, 0, 1);

/**
 * The moon's rotation for a look over `over`: body-fixed to its non-rotating axes, taking `over`
 * to the star's direction and the body-fixed pole, made square to it, towards galactic +z (+x
 * where `over` lies along the pole).
 */
export function wholeLookRotation(over: Vec3): Rotation3 {
  const d1 = normalise(over);
  const pole = Math.abs(d1.z) > 0.999 ? vec3(1, 0, 0) : vec3(0, 0, 1);
  const d2 = normalise(sub(pole, scale(d1, dot(pole, d1))));
  const d3 = cross(d1, d2);
  const e1 = TOWARD_STAR;
  const e2 = UP;
  const e3 = cross(e1, e2);
  // R = e1 d1ᵀ + e2 d2ᵀ + e3 d3ᵀ, row by row.
  const row = (i: "x" | "y" | "z"): Vec3 =>
    vec3(
      e1[i] * d1.x + e2[i] * d2.x + e3[i] * d3.x,
      e1[i] * d1.y + e2[i] * d2.y + e3[i] * d3.y,
      e1[i] * d1.z + e2[i] * d2.z + e3[i] * d3.z,
    );
  return rotation3FromRows([row("x"), row("y"), row("z")]);
}

/** The camera's distance from the moon's centre in `look`, m. */
export function wholeLookDistanceM(look: WholeLook): number {
  return look.radii * terrainTestGolden("terrain-test-whole").header.equatorial_radius_m;
}

/**
 * `look`'s kept scene: `TERRAIN TEST`'s bodies on the whole body's field, the moon held at
 * {@link wholeLookRotation} and the own ship held over the look's direction, its attitude looking
 * along +x at the moon's centre with +z up; the clock at the kept epoch, still.
 */
export function wholeLookScene(look: WholeLook): KeptScene {
  const base = terrainSceneAt(0, "terrain-test-whole");
  const moon = terrainTestGolden("terrain-test-whole").body;
  const rotation = wholeLookRotation(look.over);
  const shipM = scale(normalise(look.over), wholeLookDistanceM(look));
  const attitude = lookAlong(scale(TOWARD_STAR, -1), UP);
  const scene: ViewScene = {
    ...base,
    provenance: { kind: "kept", name: look.scene },
    time: keptTime(0),
    timeRate: 1,
    bodies: base.bodies.map((body) =>
      body.id === moon ? Object.assign({}, body, { rotation }) : body,
    ),
    craft: base.craft.map((craft) =>
      Object.assign({}, craft, {
        pose: { position: { kind: "body_fixed" as const, body: moon, m: shipM }, attitude },
        predictedPath: null,
        velocityMPerS: vec3(0, 0, 0),
      }),
    ),
  };
  // The seat, as `terrainScene`'s camera takes it: the hull's eye point turned by the attitude.
  const camera: CameraPose = {
    frame: { kind: "craft", craft: TERRAIN_TEST_SHIP },
    positionM: rotate(attitude, TEST_HULL.eyePointM),
    orientation: attitude,
  };
  return { name: look.scene, durationS: 60, sceneAt: () => scene, cameraAt: () => camera };
}

/**
 * The looks' kept scenes for `VIEW` on a `--whole` launch, each with a fixture of its own, the
 * whole body's served at the first ask.
 */
export function wholeLookSceneOptions(): ReadonlyArray<SceneOption> {
  return WHOLE_LOOKS.map((look): SceneOption => ({
    name: look.scene,
    make: () => wholeLookScene(look),
    terrain: terrainSceneFixture(0, "terrain-test-whole"),
  }));
}

/** What {@link readWholeDisc} found in a look's frame. */
export interface WholeDiscReading {
  readonly discPixels: number;
  readonly skyPixels: number;
  readonly outsidePixels: number;
  readonly litOutsidePixels: number;
  readonly finite: boolean;
}

/** What {@link readWholeDisc} reads a frame with. */
export interface WholeDiscFrame {
  /** The scene target's size, px. */
  readonly widthPx: number;
  readonly heightPx: number;
  /** The horizontal field of view, rad. */
  readonly fovXRad: number;
  /** The camera's distance from the moon's centre, m. */
  readonly distanceM: number;
  /** The datum's equatorial radius and the field's realised relief, m. */
  readonly radiusM: number;
  readonly reliefM: number;
}

/**
 * The share of the disc's median luminance below which a pixel reads as the sky's black, and above
 * which one beyond the limb reads lit.
 */
const SKY_SHARE = 0.01;

/**
 * Reads a look's frame, `halves` the rgba16float texels of `rect` of its scene target, row after
 * row: the pixels a pixel inside the disc of the datum's equatorial radius less the field's relief,
 * and of them those darker than {@link SKY_SHARE} of their median luminance; and the pixels two
 * pixels beyond the disc of the radius and the relief, and of them those brighter than it.
 */
export function readWholeDisc(
  halves: Float16Array,
  rect: PixelRect,
  frame: WholeDiscFrame,
): WholeDiscReading {
  const focalPx = frame.widthPx / 2 / Math.tan(frame.fovXRad / 2);
  const radiusPx = (radiusM: number): number =>
    radiusM < frame.distanceM ? focalPx * Math.tan(Math.asin(radiusM / frame.distanceM)) : Infinity;
  // Ground stands below the datum by up to the relief, so the disc's own limb may lie that far in.
  const insidePx = radiusPx(frame.radiusM - frame.reliefM) - 1;
  const outsidePx = radiusPx(frame.radiusM + frame.reliefM) + 2;
  const cx = frame.widthPx / 2;
  const cy = frame.heightPx / 2;
  const inside: number[] = [];
  const outside: number[] = [];
  let finite = true;
  for (let y = 0; y < rect.height; y += 1) {
    for (let x = 0; x < rect.width; x += 1) {
      const n = 4 * (y * rect.width + x);
      const [r = 0, g = 0, b = 0] = [halves[n], halves[n + 1], halves[n + 2]];
      if (!(Number.isFinite(r) && Number.isFinite(g) && Number.isFinite(b))) {
        finite = false;
        continue;
      }
      const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
      const distancePx = Math.hypot(rect.x + x + 0.5 - cx, rect.y + y + 0.5 - cy);
      if (distancePx <= insidePx) {
        inside.push(luminance);
      } else if (distancePx >= outsidePx) {
        outside.push(luminance);
      }
    }
  }
  const sorted = inside.toSorted((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)] ?? 0;
  const floor = SKY_SHARE * median;
  return {
    discPixels: inside.length,
    skyPixels: inside.filter((luminance) => !(luminance > floor)).length,
    outsidePixels: outside.length,
    litOutsidePixels: outside.filter((luminance) => luminance > floor).length,
    finite,
  };
}
