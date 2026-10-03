import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { add, dot, norm, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { seededRandom } from "../../test/seededRandom";
import {
  type HostSphere,
  litSphereOf,
  painterOrder,
  type PainterEntry,
  spherePower,
} from "./painter";
import type { LitRegime, LitSphere } from "./regime";

const A: BodyIdHex = "0200080020000000.0300";
const B: BodyIdHex = "0200080020000000.0301";

/** The distance along a unit ray from the origin to its first hit on a sphere, or null. */
function entry(direction: Vec3, centre: Vec3, radius: number): number | null {
  const b = dot(direction, centre);
  const disc = b * b - (dot(centre, centre) - radius * radius);
  if (disc < 0) {
    return null;
  }
  const t = b - Math.sqrt(disc);
  return t > 0 ? t : null;
}

/** A unit vector within `spread` rad of `axis`. */
function jitter(axis: Vec3, spread: number, random: () => number): Vec3 {
  const offset = vec3(random() - 0.5, random() - 0.5, random() - 0.5);
  return normalise(add(normalise(axis), scale(offset, 2 * spread)));
}

function order(
  bodies: ReadonlyArray<LitSphere>,
  hosts: ReadonlyArray<HostSphere>,
  regime: LitRegime = "disc",
): PainterEntry[] {
  return painterOrder(bodies, new Map(bodies.map((body) => [body.id, regime])), hosts);
}

describe("painterOrder", () => {
  it("matches the first hit along shared rays for 10⁴ random disjoint pairs, hosts and points included", () => {
    const random = seededRandom(7);
    let pairs = 0;
    let rays = 0;
    let disagreements = 0;
    while (pairs < 10_000) {
      const scaleM = 10 ** (3 + 9 * random());
      const radiusA = scaleM * (0.01 + random());
      const radiusB = scaleM * (0.01 + random());
      const centreA = scale(jitter(vec3(0, 0, -1), 0.5, random), radiusA * (1.5 + 20 * random()));
      // B lies somewhere about A's line of sight, nearer or farther.
      const centreB = add(
        scale(normalise(centreA), norm(centreA) * (0.2 + 2 * random())),
        scale(vec3(random() - 0.5, random() - 0.5, random() - 0.5), 2 * radiusA),
      );
      const separation = norm(add(centreA, scale(centreB, -1)));
      if (separation <= radiusA + radiusB || norm(centreB) <= radiusB) {
        continue;
      }
      pairs += 1;
      // Either sphere may be a disc body, a point body or a host star's disc.
      const roleOf = (): "disc" | "point" | "host" => {
        const draw = random();
        return draw < 1 / 3 ? "disc" : draw < 2 / 3 ? "point" : "host";
      };
      const roleA = roleOf();
      const roleB = roleOf();
      const bodies: LitSphere[] = [];
      const hosts: HostSphere[] = [];
      const regimes = new Map<BodyIdHex, LitRegime>();
      for (const [id, star, role, centreM, radiusM] of [
        [A, 0, roleA, centreA, radiusA],
        [B, 1, roleB, centreB, radiusB],
      ] as const) {
        if (role === "host") {
          hosts.push({ star, centreM, radiusM });
        } else {
          bodies.push({ id, centreM, radiusM });
          regimes.set(id, role);
        }
      }
      const front = painterOrder(bodies, regimes, hosts).at(-1);
      const isA = (last: PainterEntry | undefined): boolean =>
        roleA === "host"
          ? last?.kind === "host" && last.star === 0
          : last?.kind === "body" && last.body === A;
      for (let k = 0; k < 8; k += 1) {
        const direction = jitter(centreA, Math.asin(radiusA / norm(centreA)), random);
        const hitA = entry(direction, centreA, radiusA);
        const hitB = entry(direction, centreB, radiusB);
        if (hitA === null || hitB === null) {
          continue;
        }
        rays += 1;
        if (hitA < hitB !== isA(front)) {
          disagreements += 1;
        }
      }
    }
    expect(rays).toBeGreaterThan(5_000);
    expect(disagreements).toBe(0);
  });

  it("orders host stars and points with the discs", () => {
    const star: HostSphere = { star: 0, centreM: vec3(0, 0, -1.5e11), radiusM: 7e8 };
    const point: LitSphere = { id: A, centreM: vec3(0, 0, -1e11), radiusM: 1e6 };
    const sequence = painterOrder([point], new Map([[A, "point"]]), [star]);
    expect(sequence).toEqual([
      { kind: "host", star: 0 },
      { kind: "body", body: A },
    ]);
  });

  it("orders a planet behind its star behind it", () => {
    const star: HostSphere = { star: 0, centreM: vec3(0, 0, -2.3e11), radiusM: 6.957e8 };
    const planet: LitSphere = { id: A, centreM: vec3(0, 0, -3.8e11), radiusM: 3.4e6 };
    expect(order([planet], [star])[0]).toEqual({ kind: "body", body: A });
  });

  it("orders an oblate body on its equatorial sphere", () => {
    // Saturn (equatorial 60,268 km, polar 54,364 km, mean 58,232 km) and a small moon whose power
    // lies between Saturn's on its equatorial sphere and on its mean sphere: only the equatorial
    // sphere orders Saturn in front.
    const figure = { equatorialRadiusM: 60_268e3, polarRadiusM: 54_364e3, pole: null };
    const saturn = litSphereOf(A, vec3(0, 0, -1e9), figure);
    expect(saturn.radiusM).toBe(60_268e3);
    const saturnPower = spherePower(saturn.centreM, saturn.radiusM);
    const meanPower = spherePower(saturn.centreM, 58_232e3);
    const moonRadiusM = 1e3;
    const moonDistance = Math.sqrt((saturnPower + meanPower) / 2 + moonRadiusM ** 2);
    const moon: LitSphere = {
      id: B,
      centreM: vec3(0, 7e7, -Math.sqrt(moonDistance ** 2 - 7e7 ** 2)),
      radiusM: moonRadiusM,
    };
    expect(order([moon, saturn], []).at(-1)).toEqual({ kind: "body", body: A });
    expect(order([moon, { ...saturn, radiusM: 58_232e3 }], []).at(-1)).toEqual({
      kind: "body",
      body: B,
    });
  });

  it("leaves meshes out", () => {
    const body: LitSphere = { id: A, centreM: vec3(0, 0, -1e8), radiusM: 1e6 };
    expect(order([body], [], "mesh")).toEqual([]);
  });

  it("does not depend on the input order", () => {
    const bodies: LitSphere[] = [
      { id: A, centreM: vec3(0, 0, -1e8), radiusM: 1e6 },
      { id: B, centreM: vec3(1e6, 0, -2e8), radiusM: 3e6 },
    ];
    expect(order(bodies.toReversed(), [])).toEqual(order(bodies, []));
  });
});
