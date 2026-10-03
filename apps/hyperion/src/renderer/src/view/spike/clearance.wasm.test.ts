import { beforeAll, describe, expect, it } from "vitest";

import { add, cross, normalise, scale, type Vec3 } from "../../geometry/vec3";
import {
  bakePatch,
  initSync,
  levelTable,
  omittedSigmaM,
  surfaceHeightM,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { PATCH_QUADS, vertexSpacing } from "../terrain/cube";
import { WASM_RIDGES } from "../terrain/workers/heightBake";
import type { TestPlanetRidges } from "../terrain/workers/messages";
import {
  DescentProfile,
  FLOOR_TOLERANCE_M,
  landingSiteOf,
  type TrackStretch,
  trackStretches,
} from "./descentProfile";
import { stretchKeys } from "./demandRecord";
import { TEST_PLANET_FIGURE } from "./spikeScene";
import { answerSurfaceQuery, type SurfaceQueryModule } from "./surfaceQuery";

function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

const MODULE: SurfaceQueryModule = { bakePatch, levelTable, omittedSigmaM, surfaceHeightM };

beforeAll(() => {
  initSync({ module: wasmBytes() });
});

/**
 * The runs the ruling checks (decision-r05-descent-clearance.md, lane C's tests): seeds 0 and 1
 * and seed 7, the rough site 1.85 km below the datum, each with ridges off and on. Each run bakes
 * some 600 patches, about 30 s on a loaded machine, so the suite runs the roughest one; the others
 * were run by hand (R05's Risks, "the clearance follow-up").
 */
const RUNS: ReadonlyArray<{ readonly seed: bigint; readonly ridges: TestPlanetRidges }> = [
  { seed: 7n, ridges: "on" },
];

/** The profile `prepareDescent` flies, measured as it measures it, without the worker. */
function measured(
  seed: bigint,
  ridges: TestPlanetRidges,
): {
  readonly profile: DescentProfile;
  readonly stretches: ReadonlyArray<TrackStretch>;
  readonly floors: Float64Array;
  readonly siteHeightM: number;
} {
  const site = landingSiteOf(seed);
  const datum = new DescentProfile(TEST_PLANET_FIGURE, site);
  const stretches = trackStretches(datum);
  const reply = answerSurfaceQuery(MODULE, {
    kind: "max-heights",
    id: 1,
    ridges,
    groups: stretches.map((stretch) => stretchKeys(datum, stretch)),
  });
  if (reply.kind !== "max-heights") {
    throw new Error(`the floors were not measured: ${reply.kind}`);
  }
  const d = datum.siteDir;
  const siteHeightM = surfaceHeightM(d.x, d.y, d.z, WASM_RIDGES[ridges]);
  const profile = new DescentProfile(TEST_PLANET_FIGURE, site, {
    siteHeightM,
    stretchMaxHeightsM: Array.from(reply.maxesM),
  });
  return { profile, stretches, floors: reply.maxesM, siteHeightM };
}

/** The finest mesh's height at the unit direction `d`, metres above the datum. */
function heightAt(d: Vec3, ridges: TestPlanetRidges): number {
  return surfaceHeightM(d.x, d.y, d.z, WASM_RIDGES[ridges]);
}

describe.each(RUNS)("the descent of seed $seed, ridges $ridges", ({ seed, ridges }) => {
  // Measured once for the block's tests: the bakes are its cost.
  let run: ReturnType<typeof measured>;
  beforeAll(() => {
    run = measured(seed, ridges);
  }, 300_000);

  it("finds the finest mesh under each stretch, and half an edge to either side, below its floor", () => {
    const { profile, stretches, floors } = run;
    const over: string[] = [];
    // The track is a great circle through the site, so across it is the circle's own normal.
    const across = normalise(cross(profile.groundDirAt(0), profile.siteDir));
    stretches.forEach((stretch, k) => {
      const floorM = floors[k] ?? Number.NaN;
      const edgeM =
        PATCH_QUADS * vertexSpacing(TEST_PLANET_FIGURE.polarRadiusM, stretch.level).minM;
      const offRad = edgeM / 2 / TEST_PLANET_FIGURE.polarRadiusM;
      for (let n = stretch.startS * 64; n <= stretch.endS * 64; n += 1) {
        const t = n / 64;
        const d = profile.groundDirAt(t);
        for (const side of [0, -1, 1]) {
          const at = normalise(add(d, scale(across, side * offRad)));
          const h = heightAt(at, ridges);
          if (h > floorM) {
            over.push(`${stretch.piece} at ${t} s, side ${side}: ${h} m over ${floorM} m`);
          }
        }
      }
    });
    expect(over).toEqual([]);
  });

  it("flies at least each piece's clearance above the finest mesh under the camera", () => {
    const { profile, stretches } = run;
    const short: string[] = [];
    for (let n = 0; n <= profile.durationS * 64; n += 1) {
      const t = n / 64;
      const stretch = stretches.find((each) => each.startS <= t && t < each.endS);
      if (stretch === undefined) {
        continue;
      }
      const pose = profile.poseAt(t);
      const above = pose.altitudeM - heightAt(pose.groundDir, ridges);
      if (above < stretch.clearanceM - FLOOR_TOLERANCE_M) {
        short.push(`${stretch.piece} at ${t} s: ${above} m, under ${stretch.clearanceM} m`);
      }
    }
    expect(short).toEqual([]);
    // T13.a solves the lifts to its own tolerance, rounding included.
    expect(profile.minFloorMarginM).toBeGreaterThanOrEqual(-FLOOR_TOLERANCE_M);
  });

  it("touches down a metre above the site, as the table has it", () => {
    const { profile, siteHeightM } = run;
    const end = profile.poseAt(profile.durationS);
    const above = end.altitudeM - heightAt(end.groundDir, ridges);
    expect(above).toBeGreaterThanOrEqual(1 - FLOOR_TOLERANCE_M);
    expect(Math.abs(above - end.clearanceM)).toBeLessThan(1e-6);
    expect(end.floorM).toBe(siteHeightM);
  });
});
