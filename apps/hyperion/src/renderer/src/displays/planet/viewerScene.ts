/**
 * The `PLANET VIEWER`'s scene (plan R15, T4): `VIEW`'s body inspection of its planet, opened in
 * `ORBIT`, with its sun where its orbit puts it or held over a point of the planet's ground.
 *
 * @remarks
 * - **`ORBITAL`**, as the scene opens: every star stands where the system's records put it, so the
 *   planet turns under its sun as it does.
 * - **`HELD`**: every star is turned about the planet's centre, at its own distance, to stand over
 *   the subsolar point at `LATITUDE` and `LONGITUDE` on the planet's ground (body-fixed, its pole
 *   +z, longitude east from its prime meridian +x), so that the day side holds still as the planet
 *   turns. A planet whose rotation is not given has its galactic axes for its body-fixed ones.
 * - **The setting is read at every frame** from the holder the panel writes, so that a change of
 *   the sun moves the light at the next frame and never makes a new scene, which would frame the
 *   camera afresh. Nothing else of the scene moves.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { add, norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { rotateToBody } from "../../view/coords/rotation";
import type { ViewBody, ViewScene } from "../../view/scene/model";
import type { KeptScene } from "../../view/scenes/kept";
import { type BodyInspection, inspectionScene } from "../../view/scenes/inspectionScene";

/** Where the viewer's sun stands. */
export type SunSetting =
  | { readonly kind: "orbital" }
  | {
      readonly kind: "held";
      /** The subsolar point's latitude on the planet, degrees, -90 to +90. */
      readonly latitudeDeg: number;
      /** Its longitude east of the prime meridian, degrees, 0 to 360. */
      readonly longitudeDeg: number;
    };

/** The sun as the scene opens: where its orbit puts it. */
export const ORBITAL_SUN: SunSetting = { kind: "orbital" };

/** An angle of the sun's setting as the panel and the label block show it: to a tenth, degrees. */
export function formatSunDegrees(valueDeg: number): string {
  return (valueDeg + 0).toFixed(1);
}

/**
 * What the view's label block states of `sun`, where the sun is held off where the system's records
 * put it (`SUN HELD: LATITUDE 30.0° · LONGITUDE 120.0°`), or `null` where it stands where they do.
 */
export function heldSunStatement(sun: SunSetting): string | null {
  return sun.kind === "held"
    ? `SUN HELD: LATITUDE ${formatSunDegrees(sun.latitudeDeg)}° · LONGITUDE ${formatSunDegrees(sun.longitudeDeg)}°`
    : null;
}

/** The holder of the sun's setting, which the panel writes and the scene reads at every frame. */
export interface SunHolder {
  current: SunSetting;
}

/** The unit vector from a body's centre to the point at `latitudeDeg`, `longitudeDeg`, body-fixed. */
function subsolarFixed(latitudeDeg: number, longitudeDeg: number): Vec3 {
  const lat = (latitudeDeg * Math.PI) / 180;
  const lon = (longitudeDeg * Math.PI) / 180;
  return vec3(Math.cos(lat) * Math.cos(lon), Math.cos(lat) * Math.sin(lon), Math.sin(lat));
}

/** `star` turned about `planet`'s centre, at its own distance, to stand over the held point. */
function heldStar(
  star: ViewBody,
  planet: ViewBody,
  sun: Extract<SunSetting, { readonly kind: "held" }>,
): ViewBody {
  const distanceM = norm(sub(star.centreM, planet.centreM));
  const fixed = subsolarFixed(sun.latitudeDeg, sun.longitudeDeg);
  const direction = planet.rotation === null ? fixed : rotateToBody(planet.rotation, fixed);
  const centreM = add(planet.centreM, scale(direction, distanceM));
  return {
    ...star,
    centreM,
    retarded: star.retarded === null ? null : { ...star.retarded, centreM },
  };
}

/** `scene` with its stars where `sun` puts them about `planet`. */
export function sunnedScene(scene: ViewScene, planet: BodyIdHex, sun: SunSetting): ViewScene {
  if (sun.kind === "orbital") {
    return scene;
  }
  const body = scene.bodies.find((each) => each.id === planet);
  if (body === undefined) {
    return scene;
  }
  return {
    ...scene,
    bodies: scene.bodies.map((each) => (each.kind === "star" ? heldStar(each, body, sun) : each)),
  };
}

/**
 * The viewer's kept scene of `inspection` (the module's remarks): the body inspection's, offering
 * and opening in `ORBIT` about its planet, its stars where `sun`'s setting puts them at each frame.
 *
 * @throws Error from `inspectionScene` for a body it cannot draw.
 */
export function viewerScene(inspection: BodyInspection, sun: SunHolder): KeptScene {
  const kept = inspectionScene(inspection);
  return {
    ...kept,
    startPreset: "orbit",
    sceneAt: (tS) => ({
      ...sunnedScene(kept.sceneAt(tS), inspection.body, sun.current),
      orbitHome: inspection.body,
    }),
  };
}
