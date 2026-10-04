/**
 * The host stars that light a view's scene: R06's discs joined to the scene's stars (plan R07, T8.a;
 * decision-r07-t8a, item 1).
 *
 * @remarks
 * `HostDiscDto.star` is the star's body index in its system (R06's `StarIndex::from_body`), so the
 * star's scene body is `formatBodyId({ system, bodyIndex: disc.star })`. A light carries the body,
 * not a centre: each frame takes the star's centre from that frame's scene, at the lit body's
 * emitted time (Design note 4), never from R06's apparent placement of the disc, which is
 * aberrated. A server scene's discs are its held sky's `hosts`; a kept scene's its own
 * `ViewScene.hostDiscs`.
 */
import { type BodyIdHex, formatBodyId, type HostDiscDto } from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import type { ViewScene } from "../scene/model";

/** A host star that lights the scene: R06's disc, and the star's body in the scene. */
export interface HostLight {
  readonly disc: HostDiscDto;
  /** `formatBodyId({ system: scene.system, bodyIndex: disc.star })`. */
  readonly body: BodyIdHex;
}

/** Whether the view's bodies are lit, or why not (the label block's `LIGHTING:` line). */
export type LightingState = "lit" | "pending" | "hosts-not-received";

/** The scene's lights: each disc whose star the scene has, in `disc.star` order. */
export function hostLights(scene: ViewScene, discs: ReadonlyArray<HostDiscDto>): HostLight[] {
  const stars = new Set(scene.bodies.filter((b) => b.kind === "star").map((b) => b.id));
  return discs
    .flatMap((disc) => {
      if (!(Number.isInteger(disc.star) && disc.star >= 0 && disc.star <= 0xffff)) {
        return [];
      }
      const body = formatBodyId({ system: scene.system, bodyIndex: disc.star });
      return stars.has(body) ? [{ disc, body }] : [];
    })
    .toSorted((a, b) => a.disc.star - b.disc.star);
}

/**
 * The discs a view lights its scene with: a kept scene's own, a server scene's held sky's.
 *
 * @param skyHosts - The view's sky's `response.hosts`, or `null` where it has none.
 */
export function sceneHostDiscs(
  scene: ViewScene,
  skyHosts: ReadonlyArray<HostDiscDto> | null,
): ReadonlyArray<HostDiscDto> {
  return scene.provenance.kind === "kept" ? (scene.hostDiscs ?? []) : (skyHosts ?? []);
}

/**
 * The lighting state the label block reads: `lit` with any light; else `pending` while the sky is
 * asked and not answered, and `hosts-not-received` otherwise.
 *
 * @param skyPending - Whether the view's sky has been asked for and not yet answered.
 */
export function lightingState(
  lights: ReadonlyArray<HostLight>,
  skyPending: boolean,
): LightingState {
  if (lights.length > 0) {
    return "lit";
  }
  return skyPending ? "pending" : "hosts-not-received";
}

/** The label block's line for a lighting state, `null` when lit (drafted for the owner). */
export function lightingStatement(state: LightingState): string | null {
  let line: string | null;
  switch (state) {
    case "lit":
      line = null;
      break;
    case "pending":
      line = "LIGHTING: PENDING";
      break;
    case "hosts-not-received":
      line = "LIGHTING: STAR DISCS NOT RECEIVED";
      break;
  }
  return line;
}

/** A light placed in one frame: its disc and its star's centre from the camera. */
export interface PlacedLight {
  readonly disc: HostDiscDto;
  readonly centreM: Vec3;
}

/**
 * The lights placed at their stars in a frame: each light whose star the frame places.
 *
 * @param centreOf - The star's centre from the camera, m, or `null` where the frame lacks it.
 */
export function placeLights(
  lights: ReadonlyArray<HostLight>,
  centreOf: (body: BodyIdHex) => Vec3 | null,
): PlacedLight[] {
  return lights.flatMap((light) => {
    const centreM = centreOf(light.body);
    return centreM === null ? [] : [{ disc: light.disc, centreM }];
  });
}
