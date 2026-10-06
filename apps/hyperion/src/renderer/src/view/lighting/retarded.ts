/**
 * Lighting's geometry: where a lit body's stars, occluders and planetshine neighbours were when the
 * light that reaches it, as the camera sees it, left them (plan R07, T10.a; decision-r07-t8a,
 * follow-up (a), as ruled for T10.a).
 *
 * @remarks
 * Drawing keeps the apparent places: where a disc is drawn, the painter's order, one disc covering
 * another as the camera sees it, and R06's host discs (`hostPlacements`). Lighting takes retarded
 * geometric positions in the system frame, with straight photons and no aberration:
 *
 * - the lit body B at its retarded time t_B = T − τ_B, when the light the camera sees left it
 *   ({@link RetardedCentre}), the ship's local body included, though it is drawn at the present;
 * - each star, occluder or neighbour X at t_B − |r_X − r_B| ÷ c ({@link retardedFrom});
 * - all of it taken from B and translated to B's drawn centre ({@link lightingFrameOf}), so that a
 *   body-fixed shadow depends neither on the camera's velocity nor on its position.
 *
 * Every τ is a light time to the camera, so the extrapolation's span
 * δ = τ_B + |r_X − r_B| ÷ c − τ_X lies in [0, 2 |r_X − r_B| ÷ c] by the triangle inequality.
 *
 * A planetshine neighbour N stands where B's frame puts it, and its own stars and shadows are its
 * own frame's: at N's retarded time, where t_B − |r_N − r_B| ÷ c would be exact, δ earlier. That
 * turns the direction of N's star by at most |v_N − v★| δ ÷ d★ (6 × 10⁻⁷ rad for the Earth lighting
 * the Moon) and moves N's own eclipse by up to δ (2.8 s of Io's 254 s ingress), and it keeps each
 * neighbour's starlight one evaluation a frame (T11).
 *
 * Neglected, and stated in the tests (decision-r07-t8a, follow-up (a)):
 *
 * - B's own motion: B's rest frame sees its star's light turned by up to v_B ÷ c, 1.0 × 10⁻⁴ rad
 *   for the Earth (0.64 km at its limb);
 * - the star's reflex motion, about 4 × 10⁻⁸ rad for a Sun pulled by a Jupiter (`illuminance.ts`),
 *   which the system's placements leave out: they hold a lone star at the barycentre;
 * - the linear retardation's ½ a δ², with a the source's barycentric acceleration: 1–3 cm for the
 *   Moon, about 3 m for Io.
 */
import type { BodyIdHex, HostDiscDto } from "@hyperion/protocol";

import { add, norm, scale, sub, type Vec3 } from "../../geometry/vec3";
import { SPEED_OF_LIGHT_M_PER_S } from "../../lib/scene/lightTime";
import type { CameraPose } from "../camera/pose";
import { relativeToCamera } from "../coords/relative";
import {
  isLitKind,
  type RetardedCentre,
  sceneOrigins,
  type ViewBody,
  type ViewScene,
} from "../scene/model";
import { type HostLight, hostLights, placeLights, type PlacedLight } from "./hostLights";
import type { LightingBody } from "./occluders";

/**
 * The fixed-point steps {@link retardedFrom} takes on the light time. One step from the source's
 * retarded time leaves the light time wrong by (v ÷ c) δ, which puts the Moon v² δ ÷ c, up to
 * 8 m, off for a camera past it (v 30 km/s, δ 2.6 s); the second leaves v³ δ ÷ c², under a
 * millimetre.
 */
export const RETARDATION_STEPS = 2;

/**
 * Where `source` was when the light reaching `lit` at its retarded time left it, m from the
 * barycentre along the galactic axes: centre − v δ, δ = τ_lit + |c_s − c_lit| ÷ c − τ_s, the
 * distance taken from the source's place at the previous step ({@link RETARDATION_STEPS}).
 *
 * @remarks
 * δ is the time from that light leaving the source back to the source's own retarded time: the
 * lit body's is the scene's less τ_lit, the source's less τ_s. A source at rest is where it is,
 * exactly, so that a kept scene's lighting is its drawing's.
 */
export function retardedFrom(lit: RetardedCentre, source: RetardedCentre): Vec3 {
  const { centreM, velocityMPerS } = source;
  if (velocityMPerS.x === 0 && velocityMPerS.y === 0 && velocityMPerS.z === 0) {
    return centreM;
  }
  let at = centreM;
  for (let step = 0; step < RETARDATION_STEPS; step += 1) {
    const deltaS =
      lit.lightTimeS + norm(sub(at, lit.centreM)) / SPEED_OF_LIGHT_M_PER_S - source.lightTimeS;
    at = sub(centreM, scale(velocityMPerS, deltaS));
  }
  return at;
}

/** A lit body's lighting geometry in one frame, m from the camera along the galactic axes. */
export interface LightingFrame {
  /** Its stars (`hostLights`), each where it was when the light reaching the body left it. */
  readonly lights: ReadonlyArray<PlacedLight>;
  /**
   * The scene's other lit bodies, each where it was when the light, or the shadow, reaching the
   * body left it: the places of its occluders and of its planetshine neighbours.
   */
  readonly occluders: ReadonlyArray<LightingBody>;
}

/**
 * Whether a scene body is lit: a planet, dwarf planet or moon (`isLitKind`) with a radius. The
 * photorealistic frame lights these (`litBodiesOf`), each with its {@link lightingFramesOf} entry.
 */
export function isLitBody(body: ViewBody): boolean {
  return isLitKind(body.kind) && body.radiusM > 0;
}

/** A scene body with its drawn centre from the camera. */
interface DrawnBody {
  readonly body: ViewBody;
  /** Its drawn centre from the camera, m along the galactic axes. */
  readonly drawnM: Vec3;
}

/** The scene's lights, and its stars and lit bodies with their drawn centres from the camera. */
interface SceneLighting {
  readonly hosts: ReadonlyArray<HostLight>;
  readonly byId: ReadonlyMap<BodyIdHex, DrawnBody>;
  readonly lit: ReadonlyArray<DrawnBody>;
}

function sceneLighting(
  scene: ViewScene,
  pose: CameraPose,
  discs: ReadonlyArray<HostDiscDto>,
): SceneLighting {
  const origins = sceneOrigins(scene);
  const drawn = scene.bodies
    .filter((body) => body.kind === "star" || isLitBody(body))
    .map((body): DrawnBody => ({
      body,
      drawnM: relativeToCamera(
        { kind: "system", system: scene.system, m: body.centreM },
        pose,
        origins,
      ),
    }));
  return {
    hosts: hostLights(scene, discs),
    byId: new Map(drawn.map((entry) => [entry.body.id, entry])),
    lit: drawn.filter((entry) => isLitBody(entry.body)),
  };
}

/**
 * `source` from the camera where `lit`'s light finds it: drawn(lit) + (retardedFrom(lit, source) −
 * lit's retarded centre), m.
 *
 * @remarks
 * Computed as drawn(source) + ((retarded source − drawn source) − (retarded lit − drawn lit)),
 * which is the same sum, the camera's offset being a translation, and is drawn(source) exactly
 * where nothing moves, as in a kept scene.
 */
function placedFor(lit: DrawnBody, litRetarded: RetardedCentre, source: DrawnBody): Vec3 {
  if (source.body.retarded === null) {
    return source.drawnM;
  }
  const shift = sub(
    sub(retardedFrom(litRetarded, source.body.retarded), source.body.centreM),
    sub(litRetarded.centreM, lit.body.centreM),
  );
  return shift.x === 0 && shift.y === 0 && shift.z === 0
    ? source.drawnM
    : add(source.drawnM, shift);
}

function frameFor(lighting: SceneLighting, lit: DrawnBody): LightingFrame {
  const litRetarded = lit.body.retarded;
  const centreOf = (star: BodyIdHex): Vec3 | null => {
    const found = lighting.byId.get(star);
    if (found === undefined) {
      return null;
    }
    return litRetarded === null ? found.drawnM : placedFor(lit, litRetarded, found);
  };
  const lights = placeLights(lighting.hosts, centreOf);
  // A contact, with no retarded centre, never occludes and is never eclipsed.
  const occluders =
    litRetarded === null
      ? []
      : lighting.lit.flatMap((other): LightingBody[] =>
          other.body.id === lit.body.id || other.body.retarded === null
            ? []
            : [
                {
                  id: other.body.id,
                  centreM: placedFor(lit, litRetarded, other),
                  radiusM: other.body.radiusM,
                },
              ],
        );
  return { lights, occluders };
}

/**
 * Lit body `lit`'s stars and occluders, m from the camera along the galactic axes: each where it
 * was when the light, or the shadow, reaching `lit` at its retarded time left it, taken from
 * `lit`'s retarded centre and translated to its drawn centre; `null` where `lit` is not a lit body
 * of the scene ({@link isLitBody}).
 *
 * @remarks
 * The stars are the discs' (`hostLights`). The occluders are the scene's other lit bodies, every
 * one of them, at their equatorial radius: the shadow cones choose among them (`occludersFor`),
 * and planetshine places its neighbours by them. A body with no retarded centre, a contact, takes
 * its stars where they are drawn and nothing as an occluder, and is no other body's occluder.
 *
 * @param discs - The scene's host discs (`sceneHostDiscs`).
 */
export function lightingFrameOf(
  scene: ViewScene,
  lit: BodyIdHex,
  pose: CameraPose,
  discs: ReadonlyArray<HostDiscDto>,
): LightingFrame | null {
  const lighting = sceneLighting(scene, pose, discs);
  const found = lighting.byId.get(lit);
  return found === undefined || !isLitBody(found.body) ? null : frameFor(lighting, found);
}

/** A lit body of the scene, where it is drawn and how it is lit. */
export interface LitBodyLighting {
  readonly body: ViewBody;
  /** Its drawn centre from the camera, m along the galactic axes, in `f64`. */
  readonly centreM: Vec3;
  /** Its {@link lightingFrameOf}. */
  readonly frame: LightingFrame;
}

/**
 * Every lit body of the scene ({@link isLitBody}), in the scene's order, with its drawn centre and
 * its {@link lightingFrameOf}: the drawn centres found once for all of them.
 *
 * @param discs - The scene's host discs (`sceneHostDiscs`).
 */
export function lightingFramesOf(
  scene: ViewScene,
  pose: CameraPose,
  discs: ReadonlyArray<HostDiscDto>,
): ReadonlyArray<LitBodyLighting> {
  const lighting = sceneLighting(scene, pose, discs);
  return lighting.lit.map((entry): LitBodyLighting => ({
    body: entry.body,
    centreM: entry.drawnM,
    frame: frameFor(lighting, entry),
  }));
}
