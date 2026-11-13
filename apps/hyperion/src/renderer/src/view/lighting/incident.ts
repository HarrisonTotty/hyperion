/**
 * The `INCIDENT` meter's reading: the illuminance the scene's host discs give a flat receptor at a
 * body, facing each disc, through the scene's shadowing (ux-calls-view, item 3b, the owner's
 * delegated call, 2026-11-01).
 *
 * @remarks
 * Each star lights the receptor face-on with `starIlluminance`'s π L̄ sin²ρ per channel, taken
 * where the light reaching the body left it (`lightingFrameOf`, R07.T10.a), times the share of
 * the channel's flux the scene's other lit bodies leave the body's centre (`eclipseVisible`, by
 * the channel's limb law, as the body's shading takes it). Only a shadow at least as wide as the
 * body counts, its penumbra's radius there at least the body's (a lean of this change,
 * 2026-11-01): a moon in its planet's shadow reads dark, while a moon's narrow shadow crossing a
 * planet, whose lit face stays almost whole, does not swing the planet's exposure. The stars are
 * summed face-on, a receptor turned to face each in turn: an upper bound on what one plane takes,
 * |Σ Eᵢ ŝᵢ|, met where the stars stand together on the sky and within 3% for a circumbinary
 * planet's pair. The body's own sphere hides none of it, the receptor standing on its lit side.
 * The reading is the sum's photopic illuminance, which `ev100FromIlluminance` exposes for.
 */
import type { BodyIdHex, HostDiscDto } from "@hyperion/protocol";

import { dot, norm, scale, sub, type Vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import { relativeToCamera } from "../coords/relative";
import { sceneOrigins, type ViewScene } from "../scene/model";
import { DISC_ANNULI_HIGH, eclipseVisible } from "./annuli";
import { photopicIlluminance, starIlluminance } from "./illuminance";
import { asOccluders, type LightingBody, type LightingSphere, penumbraRadius } from "./occluders";
import { lightingFrameOf } from "./retarded";

/**
 * The photopic illuminance on a flat receptor at `body`'s centre facing each of the scene's host
 * discs, summed, lx: 0 where no disc reaches it, every star shadowed whole or the scene lit by
 * none; `null` where `body` is not a lit body of the scene (`isLitBody`).
 *
 * @param pose - The camera's pose, from which the scene's places are taken; the reading does not
 *   depend on it.
 * @param discs - The scene's host discs (`sceneHostDiscs`).
 */
export function incidentIlluminanceLx(
  scene: ViewScene,
  body: BodyIdHex,
  pose: CameraPose,
  discs: ReadonlyArray<HostDiscDto>,
): number | null {
  const frame = lightingFrameOf(scene, body, pose, discs);
  const lit = scene.bodies.find((each) => each.id === body);
  if (frame === null || lit === undefined) {
    return null;
  }
  const centreM = relativeToCamera(
    { kind: "system", system: scene.system, m: lit.centreM },
    pose,
    sceneOrigins(scene),
  );
  const receptor: LightingSphere = { centreM, radiusM: lit.radiusM };
  let illuminanceLx = 0;
  for (const { disc, centreM: starM } of frame.lights) {
    const star: LightingSphere = { centreM: starM, radiusM: disc.radius_m };
    const occluders = asOccluders(
      frame.occluders.filter((occluder) => shadowsWhole(receptor, occluder, star)),
    );
    const [r, g, b] = starIlluminance(disc, norm(sub(starM, centreM)));
    // Each channel by its own limb law, the disc's B, V and R laws for the display's b, g and r.
    const [lawB, lawV, lawR] = disc.limb;
    const visible = (law: HostDiscDto["limb"][number]): number =>
      eclipseVisible(
        { centreM: starM, radiusM: disc.radius_m, limbC: law.c, limbAlpha: law.alpha },
        centreM,
        occluders,
        DISC_ANNULI_HIGH,
      );
    illuminanceLx += photopicIlluminance([r * visible(lawR), g * visible(lawV), b * visible(lawB)]);
  }
  return illuminanceLx;
}

/**
 * Whether `occluder`'s shadow from `star` is at least as wide as `body` where it stands: the body
 * behind the occluder and the penumbra's radius there (`penumbraRadius`) at least the body's.
 */
function shadowsWhole(body: LightingSphere, occluder: LightingBody, star: LightingSphere): boolean {
  const axis: Vec3 = sub(occluder.centreM, star.centreM);
  const distanceM = norm(axis);
  const behindM = dot(sub(body.centreM, occluder.centreM), scale(axis, 1 / distanceM));
  return (
    behindM > 0 &&
    penumbraRadius(occluder.radiusM, star.radiusM, distanceM, behindM) >= body.radiusM
  );
}
