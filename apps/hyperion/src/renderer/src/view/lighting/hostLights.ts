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

import { norm, sub, type Vec3 } from "../../geometry/vec3";
import type { Rgb } from "../photometry/toneCurve";
import type { ViewScene } from "../scene/model";
import { annulusEdges, type AnnulusSet } from "./annuli";
import { photopicIlluminance, shiningStars, starIlluminance } from "./illuminance";

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

/**
 * The label block's line for a lighting state, `null` when lit: `LIGHTING: PENDING` or
 * `LIGHTING: NOT RECEIVED` (decision-r07-owner-ux-signoff).
 */
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
      line = "LIGHTING: NOT RECEIVED";
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

/** The stars that light one body at most: the brightest two (Design note 4's multiple systems). */
export const MAX_BODY_LIGHTS = 2;

/** One placed light as it reaches a point. */
export interface LightAtPoint {
  readonly host: PlacedLight;
  /** From the point to the star's centre, m. */
  readonly toStarM: Vec3;
  /** Its illuminance face-on at the point, lx, per display channel (r, g, b). */
  readonly illuminance: Rgb;
}

/**
 * The stars that light a point: those past `STAR_CUT_RELATIVE`, the brightest `max` of them,
 * brightest first (Design note 4).
 *
 * @param pointM - The point, m from the camera, in the frame of the lights' centres.
 */
export function lightsAt(
  pointM: Vec3,
  hosts: ReadonlyArray<PlacedLight>,
  max: number,
): LightAtPoint[] {
  const all = hosts.map((host) => {
    const toStarM = sub(host.centreM, pointM);
    return { host, toStarM, illuminance: starIlluminance(host.disc, norm(toStarM)) };
  });
  const kept = shiningStars(all.map((light) => light.illuminance)).flatMap((index) => {
    const light = all[index];
    return light === undefined ? [] : [light];
  });
  return kept
    .toSorted((a, b) => photopicIlluminance(b.illuminance) - photopicIlluminance(a.illuminance))
    .slice(0, max);
}

/**
 * The most hosts' annuli kept: a system's few stars at both settings' K, several times over. The
 * least recently used go first, so the laws a scene is lit by stay while those of systems left
 * behind go.
 */
export const HOST_ANNULI_KEPT = 16;

/**
 * Each host's annuli by the values of its limb laws and K, least recently used first, at most
 * {@link HOST_ANNULI_KEPT}: the construction bisects, about 5 ms a host at K = 4 (R07.T17).
 *
 * @remarks
 * Keyed by value, not by the disc object, so that a caller that makes an equal disc anew each
 * frame (a smoke check, a test, any scene that rebuilds its hosts) makes them once, and a disc
 * whose laws change, even in place, gets annuli of its new laws.
 */
const ANNULI = new Map<string, readonly [AnnulusSet, AnnulusSet, AnnulusSet]>();

/**
 * The key of a host's annuli: K, then c and α of its B, V and R laws. Each number is written as
 * its shortest round-trip decimal, so equal laws share a key and unequal ones never do (but for
 * +0 and −0, whose annuli are equal).
 */
function annuliKey(limb: HostDiscDto["limb"], k: number): string {
  const [b, v, r] = limb;
  return [k, b.c, b.alpha, v.c, v.alpha, r.c, r.alpha].join(" ");
}

/**
 * A host's annuli in display order (r, g, b) from R06's B, V, R limb laws, made once for each
 * set of laws and K.
 */
export function hostAnnuli(
  disc: HostDiscDto,
  k: number,
): readonly [AnnulusSet, AnnulusSet, AnnulusSet] {
  const key = annuliKey(disc.limb, k);
  const cached = ANNULI.get(key);
  // A Map iterates in insertion order, so a key set again is the most recently used, and the first
  // key the least.
  if (cached !== undefined) {
    ANNULI.delete(key);
    ANNULI.set(key, cached);
    return cached;
  }
  const [b, v, r] = disc.limb;
  const sets = [
    annulusEdges(r.c, r.alpha, k),
    annulusEdges(v.c, v.alpha, k),
    annulusEdges(b.c, b.alpha, k),
  ] as const;
  if (ANNULI.size >= HOST_ANNULI_KEPT) {
    const unused = ANNULI.keys().next();
    if (unused.done !== true) {
      ANNULI.delete(unused.value);
    }
  }
  ANNULI.set(key, sets);
  return sets;
}
