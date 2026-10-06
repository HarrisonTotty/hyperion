import type { HostDiscDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import fixture from "../../../../../../../packages/protocol/fixtures/eye_observer.json" with { type: "json" };
import { cross, dot, norm, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aBody, aCameraPose, aViewScene, FIXTURE_SYSTEM } from "../../test/viewFixtures";
import { viewRay } from "../bodies/discShading";
import { limbDepthAt, limbDepths } from "../bodies/smoothMesh";
import { NEAR_PLANE_M, pixelSolidAngle, type Viewport } from "../camera/projection";
import { quaternionFromAxisAngle, rotate } from "../camera/quaternion";
import { HALF_FLOAT_MAX } from "../photometry/toneCurve";
import {
  DISC_MIN_DIAMETER_PX,
  HostDiscLayer,
  hostDiscRecord,
  hostPlacements,
  rasteriseHostDisc,
} from "./disc";
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
    // 2° across, about 64 px wide at 1080p and 60°, pre-exposed below the clamp.
    const rho = (1 * Math.PI) / 180;
    const exposureScale = 1e-6;
    const placement = { host, direction: vec3(0, 0, -1), distanceM: SUN_RADIUS_M / Math.sin(rho) };
    let flux = 0;
    for (const p of rasteriseHostDisc(hostDiscRecord(placement, exposureScale), LOOK, VIEWPORT)) {
      const ray = rotate(LOOK.orientation, viewRay(p.xPx + 0.5, p.yPx + 0.5, LOOK, VIEWPORT));
      flux += (p.rgb[1] / exposureScale) * pixelSolidAngle(ray, LOOK, VIEWPORT);
    }
    const expected = discIlluminanceRgbLx(host, rho)[1];
    expect(Math.abs(flux / expected - 1)).toBeLessThan(0.01);
  });

  it("light each pixel by the law on the central luminance, as discLuminanceRgb gives it", () => {
    const host = sunHost();
    const placement = { host, direction: vec3(0, 0, -1), distanceM: 30 * SUN_RADIUS_M };
    const sinRho = 1 / 30;
    const worst = Math.max(
      ...rasteriseHostDisc(hostDiscRecord(placement, 1e-6), LOOK, {
        widthPx: 96,
        heightPx: 54,
      }).map((p) => {
        const ray = viewRay(p.xPx + 0.5, p.yPx + 0.5, LOOK, { widthPx: 96, heightPx: 54 });
        const q = norm(cross(ray, vec3(0, 0, -1))) / sinRho;
        const [, g] = discLuminanceRgb(host, Math.sqrt(1 - q * q));
        return Math.abs(p.rgb[1] / (g * 1e-6) - 1);
      }),
    );
    expect(worst).toBeLessThan(1e-12);
  });

  it("clamp a pixel pre-exposed above a half float's range at 65,504", () => {
    const placement = { host: sunHost(), direction: vec3(0, 0, -1), distanceM: 30 * SUN_RADIUS_M };
    const pixels = rasteriseHostDisc(hostDiscRecord(placement, 1), LOOK, {
      widthPx: 48,
      heightPx: 27,
    });
    expect(pixels.length).toBeGreaterThan(0);
    expect(pixels.every((p) => p.rgb.every((c) => c === HALF_FLOAT_MAX))).toBe(true);
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
    // The limit falls inside the disc: 65,504 ÷ exposureScale = 0.6 I(1).
    const exposureScale = 65_504 / (0.6 * centralG);
    let reference = 0;
    const rings = 100_000;
    for (let ring = 0; ring < rings; ring += 1) {
      const r = (ring + 0.5) / rings;
      const [, g] = discLuminanceRgb(host, Math.sqrt(1 - r * r));
      reference += Math.max(0, g - 0.6 * centralG) * ((2 * r) / rings);
    }
    const [, excess] = discExcessLuminanceRgb(host, exposureScale);
    expect(Math.abs(excess / reference - 1)).toBeLessThan(0.01);
  });
});

describe("a host disc's depth (the follow-up to R07.T9)", () => {
  // 30° across 128 px, the camera turned, the Sun from 30 radii, 3.8° across: 16 px.
  const VIEW: Viewport = { widthPx: 128, heightPx: 96 };
  const TURNED = {
    orientation: quaternionFromAxisAngle(normalise(vec3(1, 2, 0.5)), 0.7),
    fovXRad: Math.PI / 6,
  };
  const distanceM = 30 * SUN_RADIUS_M;
  const centreM = scale(rotate(TURNED.orientation, normalise(vec3(0.05, -0.04, -1))), distanceM);
  const placement = { host: sunHost(), direction: normalise(centreM), distanceM };
  const pixels = rasteriseHostDisc(hostDiscRecord(placement, 1e-6), TURNED, VIEW);

  /** The reversed depth n ÷ w of the point `t` m along a pixel's unit ray. */
  function depthAt(xPx: number, yPx: number, t: (ray: Vec3) => number): number {
    const ray = viewRay(xPx + 0.5, yPx + 0.5, TURNED, VIEW);
    return NEAR_PLANE_M / (t(rotate(TURNED.orientation, ray)) * -ray.z);
  }

  /** How far along a unit ray it meets the star's sphere: its near or its far side, m. */
  function hitM(ray: Vec3, side: -1 | 1): number {
    const along = dot(ray, centreM);
    const across = norm(cross(ray, centreM));
    return along + side * Math.sqrt(SUN_RADIUS_M ** 2 - across ** 2);
  }

  it("lies inside the star at every pixel it lights: behind its near side, before its far", () => {
    const outside = pixels.filter(
      (p) =>
        !(
          p.depth < depthAt(p.xPx, p.yPx, (ray) => hitM(ray, -1)) &&
          p.depth > depthAt(p.xPx, p.yPx, (ray) => hitM(ray, 1))
        ),
    );
    expect(pixels.length).toBeGreaterThan(150);
    expect(outside).toEqual([]);
  });

  it("is the limb's plane, as R07.T9 draws a mesh body's limb", () => {
    const whole = { leftPx: 0, topPx: 0, rightPx: VIEW.widthPx, bottomPx: VIEW.heightPx };
    const sphere = { equatorialRadiusM: SUN_RADIUS_M, polarRadiusM: SUN_RADIUS_M, pole: null };
    const depths = limbDepths(centreM, sphere, vec3(0, 0, 1), whole, TURNED, VIEW);
    const worst = Math.max(
      ...pixels.map((p) =>
        Math.abs(p.depth / limbDepthAt(whole, depths, p.xPx + 0.5, p.yPx + 0.5) - 1),
      ),
    );
    expect(worst).toBeLessThan(1e-9);
  });

  it("is carried to the shader as the reciprocal of the plane's distance, d cos²ρ", async () => {
    const layer = new HostDiscLayer(await countingRenderEngine());
    const [draw] = layer.frame([placement], TURNED, VIEW, 1e-6).draws;
    const rho = angularRadiusRad(SUN_RADIUS_M, distanceM);
    const carried = draw?.item.uniforms["inverseLimbDistance"]?.[0] ?? Number.NaN;
    // To f32's rounding.
    expect(Math.abs(carried * distanceM * Math.cos(rho) ** 2 - 1)).toBeLessThan(1e-7);
  });

  it("hands its twin the record it draws", async () => {
    const layer = new HostDiscLayer(await countingRenderEngine());
    const [draw] = layer.frame([placement], TURNED, VIEW, 1e-6).draws;
    expect(draw?.record).toEqual(hostDiscRecord(placement, 1e-6));
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
