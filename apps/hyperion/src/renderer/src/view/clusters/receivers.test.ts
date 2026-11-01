/**
 * T1.c's receivers' cull against the receivers' own twin (plan R14, R14.T1.c): over random cascade
 * sets and casters, no caster the rule refuses from a cascade has a receiver that
 * `cascadeShadow.ts`' `cascadeReceiverLit` sends to that cascade's texels under it; and the rule
 * without its fall-through's cover, its blend band or its margins would refuse some.
 */

import { describe, expect, it } from "vitest";

import { add, cross, dot, normalise, scale, type Vec3 } from "../../geometry/vec3";
import { lookAlong, quaternionFromAxisAngle, rotationRows } from "../camera/quaternion";
import { rotateToBody, rotation3FromRows } from "../coords/rotation";
import {
  type CascadeIndex,
  cascadeReceiverLit,
  cascadeUniforms,
  packCascadeUniforms,
} from "../terrain/shadows/cascadeShadow";
import { type CascadeSet, fitCascades } from "../terrain/shadows/cascades";
import { cascadeKeeps, receiversRefuse } from "./cull";
import type { CascadeCullPass, CullReceivers } from "./cullPass";
import { cascadeCullPasses, cascadeCullReceivers, RECEIVER_READ_TEXELS } from "./receivers";

const RAD = Math.PI / 180;

/** A seeded uniform generator in [0, 1) (mulberry32). */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

/** A random unit vector of `next`. */
function unit(next: () => number): Vec3 {
  for (;;) {
    const v = { x: 2 * next() - 1, y: 2 * next() - 1, z: 2 * next() - 1 };
    const n = Math.hypot(v.x, v.y, v.z);
    if (n > 0.1 && n <= 1) {
      return scale(v, 1 / n);
    }
  }
}

/** A random point of the unit ball. */
function inBall(next: () => number): Vec3 {
  for (;;) {
    const v = { x: 2 * next() - 1, y: 2 * next() - 1, z: 2 * next() - 1 };
    if (Math.hypot(v.x, v.y, v.z) <= 1) {
      return v;
    }
  }
}

/** One random view's cascades, everything in the body's non-rotating axes, camera-relative. */
interface Scene {
  readonly set: CascadeSet;
  readonly axes: readonly [Vec3, Vec3, Vec3];
  readonly viewAxis: Vec3;
  readonly passes: ReadonlyArray<CascadeCullPass>;
  readonly uniforms: ReturnType<typeof cascadeUniforms>;
}

/** A random camera over a Moon or an Earth, looking about the horizon, a random sun up. */
function scene(next: () => number): Scene {
  const bodyRadiusM = next() < 0.5 ? 1_737_400 : 6_371_000;
  const up = unit(next);
  const heightM = 10 ** (6 * next() - 1.5);
  const tangent = normalise(cross(up, unit(next)));
  const pitch = (-80 + 100 * next()) * RAD;
  const forward = add(scale(tangent, Math.cos(pitch)), scale(up, Math.sin(pitch)));
  const towards = normalise(cross(up, unit(next)));
  const elevation = (1 + 84 * next()) * RAD;
  const sun = add(scale(towards, Math.cos(elevation)), scale(up, Math.sin(elevation)));
  const rotation = rotation3FromRows(rotationRows(quaternionFromAxisAngle(unit(next), 6 * next())));
  const set = fitCascades({
    cameraBodyFixedM: scale(up, bodyRadiusM + heightM),
    orientationBodyFixed: lookAlong(forward, up),
    rotation,
    fovXRad: 60 * RAD,
    aspect: 16 / 9,
    heightAboveGroundM: heightM,
    sunDirectionBodyFixed: sun,
    datumNormalBodyFixed: up,
    bodyRadiusM,
    reliefRangeM: 20_000 * next(),
    horizonTexelM: 1,
    depthClipControl: next() < 0.5,
  });
  const axes = [
    rotateToBody(rotation, set.basis.x),
    rotateToBody(rotation, set.basis.y),
    rotateToBody(rotation, set.basis.z),
  ] as const;
  const viewAxis = rotateToBody(rotation, forward);
  const uniforms = cascadeUniforms();
  packCascadeUniforms(set, viewAxis, uniforms);
  // The receivers sample at any weight above 0; at 1 every sample counts.
  uniforms.cascadeViewAxis[3] = 1;
  return {
    set,
    axes,
    viewAxis,
    passes: cascadeCullPasses(set, axes, viewAxis, 1, true),
    uniforms,
  };
}

/** A rule the test holds the receivers' twin to: whether it refuses a sphere from a pass. */
type Rule = (pass: CascadeCullPass, centreM: Vec3, radiusM: number) => boolean;

/** `pass` with its receivers changed by `change`. */
function withReceivers(
  pass: CascadeCullPass,
  change: (receivers: CullReceivers) => CullReceivers,
): CascadeCullPass {
  return pass.receivers === null ? pass : { ...pass, receivers: change(pass.receivers) };
}

/** The rule and the rule without each of its parts, by name, for `scene`. */
function rules(s: Scene): Readonly<Record<string, Rule>> {
  const index = (pass: CascadeCullPass): number => s.passes.indexOf(pass);
  const lean = (pass: CascadeCullPass, change: (r: CullReceivers, k: number) => CullReceivers) =>
    withReceivers(pass, (r) => change(r, index(pass)));
  return {
    rule: (pass, c, r) => receiversRefuse(pass, s.viewAxis, c, r, 0),
    "no cover": (pass, c, r) =>
      receiversRefuse(
        lean(pass, (rec) => ({
          ...rec,
          coveredBelow: rec.coveredBelow === null ? null : { ...rec.coveredBelow, halfSideM: 1e12 },
        })),
        s.viewAxis,
        c,
        r,
        0,
      ),
    "no blend band": (pass, c, r) =>
      receiversRefuse(
        lean(pass, (rec, k) => ({
          ...rec,
          depthM: [k === 0 ? 0 : (s.set.cascades[k - 1]?.farM ?? 0), rec.depthM[1]],
        })),
        s.viewAxis,
        c,
        r,
        0,
      ),
    "no margins": (pass, c, r) =>
      receiversRefuse(
        lean(pass, (rec, k) => {
          const cascade = s.set.cascades[k] ?? s.set.cascades[0];
          const marginM = 2 * cascade.normalOffsetM + RECEIVER_READ_TEXELS * cascade.texelM;
          return {
            ...rec,
            reachM: [rec.reachM[0] - marginM, rec.reachM[1] - marginM],
            coveredBelow: rec.coveredBelow === null ? null : { ...rec.coveredBelow, reachM: 0 },
          };
        }),
        s.viewAxis,
        c,
        r,
        0,
      ),
  };
}

/**
 * Whether some receiver under `castsInto`'s premise, of a caster at `centreM` of radius `radiusM`,
 * reads cascade `k`'s map where the caster may have written it, as `cascadeReceiverLit` sends it.
 */
function receiverReads(
  s: Scene,
  k: CascadeIndex,
  centreM: Vec3,
  radiusM: number,
  next: () => number,
): boolean {
  const cascade = s.set.cascades[k];
  const [xL, yL, zL] = s.axes;
  const o = cascade.centreFromCameraLightM;
  const cx = dot(centreM, xL);
  const cy = dot(centreM, yL);
  const touchM = radiusM + Math.SQRT2 * cascade.texelM;
  const acrossM = (1 + Math.SQRT2) * cascade.texelM + cascade.normalOffsetM;
  const seen = { read: false };
  const compare = (j: CascadeIndex, u: number, v: number): number => {
    if (j === k) {
      const x = o.x + cascade.radiusM * (2 * u - 1) - cx;
      const y = o.y + cascade.radiusM * (1 - 2 * v) - cy;
      seen.read ||= Math.hypot(x, y) <= touchM;
    }
    return 1;
  };
  for (let n = 0; n < 48 && !seen.read; n += 1) {
    // A caster's point, a receiver up to ℓ farther from the light, and across it the reads' reach.
    const p = add(centreM, scale(inBall(next), radiusM));
    const t = -cascade.normalOffsetM + (s.set.casterReachM + cascade.normalOffsetM) * next();
    const a = 2 * Math.PI * next();
    const across = acrossM * Math.sqrt(next());
    const receiver = add(
      add(p, scale(zL, -t)),
      add(scale(xL, across * Math.cos(a)), scale(yL, across * Math.sin(a))),
    );
    cascadeReceiverLit(s.uniforms, receiver, unit(next), unit(next), compare);
  }
  return seen.read;
}

describe("T1.c's receivers' cull (plan R14, R14.T1.c)", () => {
  it("refuses no caster a receiver reads, by cascadeShadow.ts' receiver twin, and refuses some where its parts are left out", () => {
    const next = random(0x14_1c);
    const refused: Record<string, number> = {};
    const reads: Record<string, number> = {};
    let tested = 0;
    for (let n = 0; n < 120; n += 1) {
      const s = scene(next);
      const named = Object.entries(rules(s));
      for (const k of [0, 1, 2, 3] as const) {
        const pass = s.passes[k];
        if (pass === undefined) {
          continue;
        }
        for (let m = 0; m < 160; m += 1) {
          // About this cascade's centre or the next nearer's, across and beyond either's box.
          const around = s.set.cascades[k > 0 && next() < 0.5 ? k - 1 : k] ?? s.set.cascades[k];
          const r = around.radiusM;
          const c = around.centreFromCameraLightM;
          const [xL, yL, zL] = s.axes;
          const centreM = add(
            add(
              scale(xL, c.x + 1.3 * r * (2 * next() - 1)),
              scale(yL, c.y + 1.3 * r * (2 * next() - 1)),
            ),
            scale(zL, c.z + r * (2 * next() - 1)),
          );
          const radiusM = r * 10 ** (-3.5 + 3.5 * next());
          if (!cascadeKeeps(pass, centreM, radiusM, 0)) {
            continue;
          }
          tested += 1;
          const refusing = named.filter(([, rule]) => rule(pass, centreM, radiusM));
          if (refusing.length === 0) {
            continue;
          }
          const isRead = receiverReads(s, k, centreM, radiusM, next);
          for (const [name] of refusing) {
            refused[name] = (refused[name] ?? 0) + 1;
            reads[name] = (reads[name] ?? 0) + (isRead ? 1 : 0);
          }
        }
      }
    }
    expect(tested).toBeGreaterThan(5_000);
    // The rule refuses a share of the casters the boxes keep, none of them read.
    expect((refused["rule"] ?? 0) / tested).toBeGreaterThan(0.15);
    expect(reads["rule"] ?? 0).toBe(0);
    // Each part it leaves out lets it refuse casters some receiver reads.
    for (const name of ["no cover", "no blend band", "no margins"]) {
      expect([name, (reads[name] ?? 0) > 0]).toEqual([name, true]);
    }
  });

  it("takes the next nearer cascade's fade start and cover, and widens each reach by the offsets and the reads", () => {
    const s = scene(random(7));
    const [c0, c1] = s.set.cascades;
    const z = s.axes[2];
    const lightOnView = dot(z, s.viewAxis);
    const first = cascadeCullReceivers(s.set, 0, lightOnView);
    const second = cascadeCullReceivers(s.set, 1, lightOnView);
    const margin = 2 * c1.normalOffsetM + 3 * c1.texelM;
    expect(first.coveredBelow).toBeNull();
    expect(first.depthM).toEqual([0, c0.farM]);
    expect(second.depthM).toEqual([c0.fadeStartM, c1.farM]);
    expect(second.reachM[0]).toBeCloseTo(s.set.casterReachM * Math.max(0, lightOnView) + margin, 9);
    expect(second.reachM[1]).toBeCloseTo(
      s.set.casterReachM * Math.max(0, -lightOnView) + margin,
      9,
    );
    expect(second.coveredBelow).toEqual({
      centreM: [c0.centreFromCameraLightM.x, c0.centreFromCameraLightM.y],
      halfSideM: c0.radiusM - 2 * c0.texelM,
      reachM: 3 * c1.texelM + c1.normalOffsetM + c0.normalOffsetM,
    });
    expect(cascadeCullPasses(s.set, s.axes, s.viewAxis, 2, false).map((p) => p.receivers)).toEqual([
      null,
      null,
      null,
      null,
    ]);
  });
});
