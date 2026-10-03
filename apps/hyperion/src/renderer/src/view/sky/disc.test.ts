import type { HostDiscDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import fixture from "../../../../../../../packages/protocol/fixtures/eye_observer.json" with { type: "json" };
import { cross, dot, norm, normalise, vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aBody, aCameraPose, aViewScene, FIXTURE_SYSTEM } from "../../test/viewFixtures";
import { pixelSolidAngle } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import { DISC_MIN_DIAMETER_PX, HostDiscLayer, hostPlacements } from "./disc";
import {
  angularRadiusRad,
  discExcessLuminanceRgb,
  discIlluminanceRgbLx,
  discLuminanceRgb,
} from "./discFlux";
import { DEFAULT_EYE_OBSERVER } from "./eye";

const AU_M = 149_597_870_700;
const SUN_RADIUS_M = 6.957e8;

/** The Sun as a host: the power-2 law of Design note 16 (c 0.7837, α 0.6893) on every channel. */
function sunHost(meanCdM2 = 2e9): HostDiscDto {
  const c = 0.7837;
  const alpha = 0.6893;
  const central = meanCdM2 / (1 - (c * alpha) / (alpha + 2));
  return {
    star: 0,
    radius_m: SUN_RADIUS_M,
    teff_k: 5_772,
    log_g: 4.438,
    mean_luminance_cd_m2: [meanCdM2, meanCdM2, meanCdM2],
    central_luminance_cd_m2: [central, central, central],
    limb: [
      { c, alpha },
      { c, alpha },
      { c, alpha },
    ],
    chroma: [1, 1],
    lux_per_v0: 1,
    bake_spectrum: [
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
    ],
  };
}

const LOOK = { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 3 };
const VIEWPORT = { widthPx: 1_920, heightPx: 1_080 };

describe("the host discs", () => {
  it("make the Sun from 1 au 0.533° across", () => {
    const diameterDeg = (2 * angularRadiusRad(SUN_RADIUS_M, AU_M) * 180) / Math.PI;
    expect(Math.abs(diameterDeg - 0.533)).toBeLessThan(0.001);
  });

  it("draw a disc whose pixels sum to π L̄ sin²ρ within 1%, as disc.wgsl lights them", () => {
    const host = sunHost();
    // 2° across, about 64 px wide at 1080p and 60°.
    const rho = (1 * Math.PI) / 180;
    const camera = { orientation: LOOK.orientation, fovXRad: LOOK.fovXRad };
    const axis = vec3(0, 0, -1);
    let flux = 0;
    const s = 1 / Math.tan(camera.fovXRad / 2);
    const aspect = VIEWPORT.widthPx / VIEWPORT.heightPx;
    for (let py = 470; py < 610; py += 1) {
      for (let px = 890; px < 1_030; px += 1) {
        const ndcX = ((px + 0.5) / VIEWPORT.widthPx) * 2 - 1;
        const ndcY = 1 - ((py + 0.5) / VIEWPORT.heightPx) * 2;
        const ray = normalise(vec3(ndcX / s, ndcY / (s * aspect), -1));
        const sinTheta = norm(cross(ray, axis));
        if (dot(ray, axis) <= 0 || sinTheta >= Math.sin(rho)) {
          continue;
        }
        const q = sinTheta / Math.sin(rho);
        const [, g] = discLuminanceRgb(host, Math.sqrt(1 - q * q));
        flux += g * pixelSolidAngle(ray, camera, VIEWPORT);
      }
    }
    const expected = discIlluminanceRgbLx(host, rho)[1];
    expect(Math.abs(flux / expected - 1)).toBeLessThan(0.01);
  });

  it("draw a disc under three pixels as a sprite of the same illuminance", async () => {
    const engine = await countingRenderEngine();
    const layer = new HostDiscLayer(engine);
    const host = sunHost();
    // The Sun from 50 au is 38″ across, a third of a 1080p, 60° pixel.
    const far = layer.frame(
      [{ host, direction: vec3(0, 0, -1), distanceM: 50 * AU_M }],
      LOOK,
      VIEWPORT,
      1,
    );
    const near = layer.frame(
      [{ host, direction: vec3(0, 0, -1), distanceM: AU_M }],
      LOOK,
      VIEWPORT,
      1,
    );
    expect([far.draws.length, far.sprites.length, near.draws.length, near.sprites.length]).toEqual([
      0, 1, 1, 0,
    ]);
    expect(far.sprites[0]?.illuminanceRgbLx).toEqual(
      discIlluminanceRgbLx(host, angularRadiusRad(SUN_RADIUS_M, 50 * AU_M)),
    );
    expect(DISC_MIN_DIAMETER_PX).toBe(3);
  });

  it("hand R07 the light above a half's 65,504, and none while the disc fits", () => {
    const host = sunHost();
    expect(discExcessLuminanceRgb(host, 1e-6)).toEqual([0, 0, 0]);
    const [, excess] = discExcessLuminanceRgb(host, 1);
    // Almost all of the mean is above 65,504 cd/m² at a scale of 1.
    expect(Math.abs(excess / (2e9 - 65_504) - 1)).toBeLessThan(0.01);
  });

  it("give an eye view glare from a disc 30° past an edge, but not 50°, and a camera none outside", async () => {
    const engine = await countingRenderEngine();
    const layer = new HostDiscLayer(engine);
    const halfX = Math.PI / 6;
    const halfY = Math.atan(Math.tan(halfX) * (1_080 / 1_920));
    /** A direction `deg` past the top edge (about x) or the right edge (about y). */
    const past = (edge: "top" | "side", deg: number) => {
      const angle = (edge === "top" ? halfY : halfX) + (deg * Math.PI) / 180;
      const turn =
        edge === "top"
          ? { w: Math.cos(angle / 2), x: Math.sin(angle / 2), y: 0, z: 0 }
          : { w: Math.cos(angle / 2), x: 0, y: -Math.sin(angle / 2), z: 0 };
      return rotate(turn, vec3(0, 0, -1));
    };
    const host = sunHost();
    const sources = (edge: "top" | "side", deg: number, role: "eye" | "camera") => {
      layer.frame([{ host, direction: past(edge, deg), distanceM: AU_M }], LOOK, VIEWPORT, 1);
      return layer.glareSources(LOOK, VIEWPORT, role).length;
    };
    expect([
      sources("top", 30, "eye"),
      sources("top", 50, "eye"),
      sources("side", 30, "eye"),
      sources("side", 50, "eye"),
      sources("top", 2, "camera"),
      sources("top", -2, "camera"),
    ]).toEqual([1, 0, 1, 0, 0, 1]);
  });

  it("cast no glare source from a host drawn as a sprite", async () => {
    const engine = await countingRenderEngine();
    const layer = new HostDiscLayer(engine);
    layer.frame(
      [{ host: sunHost(), direction: vec3(0, 0, -1), distanceM: 50 * AU_M }],
      LOOK,
      VIEWPORT,
      1,
    );
    expect(layer.glareSources(LOOK, VIEWPORT, "eye")).toEqual([]);
  });

  it("hand R07 the excess of a partly clamped disc as a fine integration gives it", () => {
    const host = sunHost();
    const centralG = host.central_luminance_cd_m2[1];
    // The limit falls inside the disc: 65,504 ÷ scale = 0.6 I(1).
    const scale = 65_504 / (0.6 * centralG);
    let reference = 0;
    const rings = 100_000;
    for (let ring = 0; ring < rings; ring += 1) {
      const r = (ring + 0.5) / rings;
      const [, g] = discLuminanceRgb(host, Math.sqrt(1 - r * r));
      reference += Math.max(0, g - 0.6 * centralG) * ((2 * r) / rings);
    }
    const [, excess] = discExcessLuminanceRgb(host, scale);
    expect(Math.abs(excess / reference - 1)).toBeLessThan(0.01);
  });
});

describe("hostPlacements", () => {
  it("joins each host to the scene's star body of its index, and places it from the camera", () => {
    const star = aBody({
      id: `${FIXTURE_SYSTEM}.0000`,
      parent: null,
      kind: "star",
      designation: "TEST STAR",
      radiusM: SUN_RADIUS_M,
      centreM: vec3(0, 0, 0),
    });
    const scene = aViewScene({ bodies: [star, ...aViewScene().bodies] });
    const pose = aCameraPose({
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: vec3(AU_M, 0, 0),
    });
    const placed = hostPlacements(scene, [sunHost(), { ...sunHost(), star: 7 }], pose);
    expect(placed).toHaveLength(1);
    expect(placed[0]?.host.star).toBe(0);
    expect(placed[0]?.distanceM).toBeCloseTo(AU_M, -2);
    expect(placed[0]?.direction.x).toBeCloseTo(-1, 9);
  });
});

describe("DEFAULT_EYE_OBSERVER", () => {
  it("equals the sim's defaults, by the fixture both read", () => {
    expect(DEFAULT_EYE_OBSERVER).toEqual({
      fieldFactor: fixture.field_factor,
      ageYears: fixture.age_years,
      pigmentation: fixture.pigmentation,
    });
  });
});
