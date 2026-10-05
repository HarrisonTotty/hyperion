import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { cross, dot, norm } from "../../geometry/vec3";
import { toViewAxes } from "../camera/projection";
import { type DiscRecord, viewRay } from "../bodies/discShading";
import { type BodyFramePlan, planLitBodies } from "../bodies/draw";
import { compositeBodyFrame, firstHitShares, type OracleBody } from "../bodies/frameTwin";
import type { LitRegime } from "../bodies/regime";
import { DISC_ANNULI_HIGH } from "../lighting/annuli";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import {
  OCCULTATION_CAMERA,
  OCCULTATION_MOON,
  OCCULTATION_PLANET,
  OCCULTATION_STEPS,
  OCCULTATION_VIEWPORT,
  occultationFrame,
} from "./occultationScene";

const CAMERA = OCCULTATION_CAMERA;
const VIEW = OCCULTATION_VIEWPORT;

/** The oracle's rays a pixel along each axis. */
const ORACLE_SAMPLES = 16;

/** The script's plans, each step's regimes the next one's previous. */
function plans(promoted: boolean): BodyFramePlan[] {
  const out: BodyFramePlan[] = [];
  let previous: ReadonlyMap<BodyIdHex, LitRegime> = new Map();
  for (let step = 0; step < OCCULTATION_STEPS; step += 1) {
    const frame = occultationFrame(step, promoted);
    const plan = planLitBodies(
      frame.bodies,
      frame.lights,
      {
        camera: CAMERA,
        viewport: VIEW,
        exposureScale: 1e-4,
        annuli: DISC_ANNULI_HIGH,
        planetshine: PLANETSHINE_SOURCES_HIGH,
        setting: "high" as const,
        depthWriters: frame.depthWriters,
      },
      previous,
    );
    out.push(plan);
    previous = plan.regimes;
  }
  return out;
}

/** The record of `body` in a plan. */
function recordOf(plan: BodyFramePlan, body: BodyIdHex): DiscRecord {
  const record = plan.discs.find((each) => each.body === body);
  if (record === undefined) {
    throw new Error(`${body} has no record`);
  }
  return record;
}

/** The signed distance of a pixel's centre from a record's limb, px, positive outside. */
function limbDistancePx(record: DiscRecord, index: number): number {
  const x = (index % VIEW.widthPx) + 0.5;
  const y = Math.floor(index / VIEW.widthPx) + 0.5;
  const centre = toViewAxes(record.direction, CAMERA.orientation);
  const angle = (px: number, py: number): number => {
    const r = viewRay(px, py, CAMERA, VIEW);
    return (
      Math.atan2(norm(cross(r, centre)), dot(r, centre)) - Math.asin(record.radiusOverDistance)
    );
  };
  const h = 1 / 64;
  const a0 = angle(x, y);
  return a0 / (Math.hypot(angle(x + h, y) - a0, angle(x, y + h) - a0) / h);
}

/** How a step's frame agrees with the oracle. */
interface StepAgreement {
  /** The moon's regime. */
  readonly moon: LitRegime | undefined;
  /** The worst |twin − oracle| share of a body in a pixel on at most one limb. */
  readonly worstShare: number;
  /**
   * The same where both limbs cross a pixel, which the limb's blend over what is beneath takes as
   * uncorrelated (the usual coverage-over error, T8.a as built).
   */
  readonly worstCrossing: number;
  /** Pixels a pixel or more from both limbs where the moon shows other than the oracle says. */
  readonly misplaced: number;
  /** The moon's visible area, px², drawn and by the oracle. */
  readonly drawnMoonPx: number;
  readonly oracleMoonPx: number;
  readonly holes: number;
}

function agreement(plan: BodyFramePlan, step: number): StepAgreement {
  const frame = occultationFrame(step);
  const drawn = compositeBodyFrame(plan, CAMERA, VIEW);
  const planet = recordOf(plan, OCCULTATION_PLANET);
  const moon = recordOf(plan, OCCULTATION_MOON);
  // The moon's rectangle, and every pixel the frame gives any of the moon's light.
  const region = new Set<number>();
  for (const [index, pixel] of drawn.pixels) {
    if ((pixel.shares.get(OCCULTATION_MOON) ?? 0) > 0) {
      region.add(index);
    }
  }
  const { rect } = moon;
  for (let y = Math.floor(rect.topPx); y < Math.ceil(rect.bottomPx); y += 1) {
    for (let x = Math.floor(rect.leftPx); x < Math.ceil(rect.rightPx); x += 1) {
      region.add(y * VIEW.widthPx + x);
    }
  }
  const bodies: OracleBody[] = frame.bodies.map((body) => ({
    id: body.id,
    centreM: body.centreM,
    figure: body.figure,
    pole: recordOf(plan, body.id).pole,
  }));
  const oracle = firstHitShares(bodies, CAMERA, VIEW, ORACLE_SAMPLES, region);
  let worstShare = 0;
  let worstCrossing = 0;
  let misplaced = 0;
  let drawnMoonPx = 0;
  let oracleMoonPx = 0;
  for (const index of region) {
    const truth = oracle.get(index) ?? new Map<BodyIdHex, number>();
    const shares = drawn.pixels.get(index)?.shares ?? new Map<BodyIdHex, number>();
    const nearPlanet = Math.abs(limbDistancePx(planet, index)) < 1;
    const nearMoon = Math.abs(limbDistancePx(moon, index)) < 1;
    for (const id of [OCCULTATION_PLANET, OCCULTATION_MOON]) {
      const error = Math.abs((shares.get(id) ?? 0) - (truth.get(id) ?? 0));
      if (nearPlanet && nearMoon) {
        worstCrossing = Math.max(worstCrossing, error);
      } else {
        worstShare = Math.max(worstShare, error);
      }
    }
    const moonDrawn = shares.get(OCCULTATION_MOON) ?? 0;
    const moonTrue = truth.get(OCCULTATION_MOON) ?? 0;
    drawnMoonPx += moonDrawn;
    oracleMoonPx += moonTrue;
    if (!nearPlanet && !nearMoon && (moonTrue === 0 || moonTrue === 1) && moonDrawn !== moonTrue) {
      misplaced += 1;
    }
  }
  return {
    moon: plan.regimes.get(OCCULTATION_MOON),
    worstShare,
    worstCrossing,
    misplaced,
    drawnMoonPx,
    oracleMoonPx,
    holes: drawn.holes,
  };
}

describe("the scripted occultation (R07.T9)", () => {
  const promoted = plans(true);

  it("draws the planet as a mesh throughout and promotes the moon once their footprints meet", () => {
    expect(promoted.every((plan) => plan.regimes.get(OCCULTATION_PLANET) === "mesh")).toBe(true);
    const moon = promoted.map((plan) => plan.regimes.get(OCCULTATION_MOON));
    expect(moon[0]).toBe("disc");
    expect(moon.at(-1)).toBe("mesh");
    // One switch, from disc to mesh, and none back.
    const switches = moon.filter((regime, i) => i > 0 && regime !== moon[i - 1]);
    expect(switches).toEqual(["mesh"]);
  });

  it("hides the moon behind the planet's limb where the oracle says, on every step", () => {
    const steps = promoted.map((plan, step) => agreement(plan, step));
    for (const each of steps) {
      expect(each.holes).toBe(0);
      expect(each.misplaced).toBe(0);
      // A pixel's shares within the oracle's resolution, 1 ÷ 16 for a straight edge across its
      // 16 × 16 rays, and where both limbs cross it within the coverage-over blend's error.
      expect(each.worstShare).toBeLessThan(1 / ORACLE_SAMPLES);
      expect(each.worstCrossing).toBeLessThan(0.25);
      expect(Math.abs(each.drawnMoonPx - each.oracleMoonPx)).toBeLessThan(
        0.5 + 0.01 * each.oracleMoonPx,
      );
    }
    // The moon starts wholly in view, passes behind the limb, and ends wholly hidden.
    const first = steps[0];
    const last = steps.at(-1);
    expect(first?.drawnMoonPx).toBeGreaterThan(100);
    expect(last?.drawnMoonPx).toBe(0);
    expect(last?.oracleMoonPx).toBe(0);
    expect(steps.some((each) => each.oracleMoonPx > 10 && each.oracleMoonPx < 90)).toBe(true);
  });

  it("draws the same light with the planet as a disc, where nothing writes depth", () => {
    const discs = plans(false);
    expect(
      discs.every((plan) => [...plan.regimes.values()].every((regime) => regime === "disc")),
    ).toBe(true);
    let worst = 0;
    for (const [step, plan] of discs.entries()) {
      const mesh = promoted[step];
      if (mesh === undefined) {
        continue;
      }
      const asDiscs = compositeBodyFrame(plan, CAMERA, VIEW);
      const asMeshes = compositeBodyFrame(mesh, CAMERA, VIEW);
      // Where the moon is behind the planet's interior, the two orders agree pixel for pixel; on
      // the planet's limb the painter blends the moon beneath as the depth test does.
      for (const [index, pixel] of asDiscs.pixels) {
        const other = asMeshes.pixels.get(index);
        const scaleOf = Math.max(pixel.rgb[1], other?.rgb[1] ?? 0, 1e-12);
        worst = Math.max(worst, Math.abs(pixel.rgb[1] - (other?.rgb[1] ?? 0)) / scaleOf);
      }
    }
    expect(worst).toBeLessThan(1e-9);
  });
});
