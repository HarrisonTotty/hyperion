/**
 * The order in which analytic spheres are drawn: back to front by power (plan R07, Design note 2).
 *
 * @remarks
 * A disc quad has no depth of its own, so bodies are ordered analytically. Between two spheres that
 * do not intersect, the one on the camera's side of their radical plane is in front along every ray
 * that meets both, and the camera is on sphere A's side when its power |c_A|² − r_A² (the squared
 * tangent length) is the smaller. Every analytic sphere — a disc body, a point body and each host
 * star's disc, as a sphere of the star's radius — is drawn in one sequence of falling power, in
 * `f64`, exact for disjoint spheres, with no depth write; an oblate body is ordered on its
 * equatorial bounding sphere. Mesh bodies are left out: the depth test orders them.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { dot, type Vec3 } from "../../geometry/vec3";
import type { BodyFigure } from "../terrain/planet";
import type { LitRegime, LitSphere } from "./regime";

/** One analytic sphere in the painter's sequence. */
export type PainterEntry =
  | { readonly kind: "body"; readonly body: BodyIdHex }
  | { readonly kind: "host"; readonly star: number };

/** A host star's disc as a sphere for the order: its index, centre from the camera and radius. */
export interface HostSphere {
  readonly star: number;
  /** Its centre from the camera, m. */
  readonly centreM: Vec3;
  readonly radiusM: number;
}

/**
 * A body as a sphere for the order and the regime: its equatorial bounding sphere (Design note 2).
 *
 * @param centreM - Its centre from the camera, m.
 */
export function litSphereOf(id: BodyIdHex, centreM: Vec3, figure: BodyFigure): LitSphere {
  return { id, centreM, radiusM: figure.equatorialRadiusM };
}

/** A sphere's power with respect to the camera, |c|² − r², m². */
export function spherePower(centreM: Vec3, radiusM: number): number {
  return dot(centreM, centreM) - radiusM * radiusM;
}

/**
 * The analytic spheres back to front: falling power, meshes excluded; equal powers keep bodies
 * before hosts and then their identifiers' order, so that the sequence does not depend on input
 * order.
 */
export function painterOrder(
  bodies: ReadonlyArray<LitSphere>,
  regimes: ReadonlyMap<BodyIdHex, LitRegime>,
  hosts: ReadonlyArray<HostSphere>,
): PainterEntry[] {
  const entries: Array<{
    readonly entry: PainterEntry;
    readonly power: number;
    /** Bodies before hosts on a tie, then by identifier or index. */
    readonly rank: readonly [number, string, number];
  }> = [];
  for (const body of bodies) {
    const regime = regimes.get(body.id);
    if (regime === "point" || regime === "disc") {
      entries.push({
        entry: { kind: "body", body: body.id },
        power: spherePower(body.centreM, body.radiusM),
        rank: [0, body.id, 0],
      });
    }
  }
  for (const host of hosts) {
    entries.push({
      entry: { kind: "host", star: host.star },
      power: spherePower(host.centreM, host.radiusM),
      rank: [1, "", host.star],
    });
  }
  entries.sort((a, b) => {
    const byPower = b.power - a.power;
    if (byPower !== 0) {
      return byPower;
    }
    const [kindA, idA, starA] = a.rank;
    const [kindB, idB, starB] = b.rank;
    if (kindA !== kindB) {
      return kindA - kindB;
    }
    return idA < idB ? -1 : idA > idB ? 1 : starA - starB;
  });
  return entries.map(({ entry }) => entry);
}
