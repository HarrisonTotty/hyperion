import { describe, expect, it } from "vitest";

import { add, normalise, scale, type Vec3 } from "../../geometry/vec3";
import { bindingStages } from "../../test/wgslStages";
import { collapseWhitespace, wgslFunction } from "../../test/wgslText";
import { CLOUD_TEXTURES } from "../clouds/cloudLayer";
import { cloudMapFrameOf } from "../clouds/cloudMap";
import { CLEAR_GROUND, type CloudGroundLight } from "../clouds/cloudShadow";
import {
  type CloudShadowFrame,
  cloudGroundLightAt,
  type ShadowLevelRead,
} from "../clouds/cloudShadowMap";
import { WGSL_CATALOGUE } from "../engine/catalogue";
import type { WgslMaterialSpec } from "../engine/types";
import { OCEAN_CLOUD_BINDINGS, oceanCloudedLight, withOceanCloudLight } from "./oceanClouds";
import { OCEAN_NEAR_CLOUD_PASS_MATERIAL, OCEAN_NEAR_PASS_MATERIAL } from "./oceanNearPass";
import { OCEAN_FAR_CLOUD_PASS_MATERIAL, OCEAN_FAR_PASS_MATERIAL } from "./oceanPass";

/** Each cloudy form with its plain form and the sea's point it reads its clouds at. */
const FORMS: ReadonlyArray<{
  readonly plain: WgslMaterialSpec;
  readonly cloudy: WgslMaterialSpec;
  readonly place: string;
}> = [
  { plain: OCEAN_FAR_PASS_MATERIAL, cloudy: OCEAN_FAR_CLOUD_PASS_MATERIAL, place: "hit.point" },
  { plain: OCEAN_NEAR_PASS_MATERIAL, cloudy: OCEAN_NEAR_CLOUD_PASS_MATERIAL, place: "world" },
];

/** The `@group(2)` bindings a material's entry points read that its layout does not declare. */
function undeclaredBindings(spec: WgslMaterialSpec): number[] {
  const declared = new Set([
    ...(spec.storageBuffers ?? []).map((buffer) => buffer.binding),
    ...(spec.textures ?? []).map((texture) => texture.binding),
    ...spec.samplers.map((sampler) => sampler.binding),
  ]);
  return [...bindingStages(spec.fragmentWgsl).keys()].filter((binding) => !declared.has(binding));
}

/** The storage buffers a stage of `spec` reads. */
function storageIn(spec: WgslMaterialSpec, stage: "vertex" | "fragment"): number {
  return (spec.storageBuffers ?? []).filter((buffer) =>
    (buffer.stages ?? ["vertex", "fragment"]).includes(stage),
  ).length;
}

describe("the sea's cloudy forms (R11.T7.c)", () => {
  it("leaves the plain forms without the clouds' read", () => {
    for (const { plain } of FORMS) {
      expect(plain.fragmentWgsl).not.toContain("cloud_ground_light");
      expect(plain.fragmentWgsl).not.toContain("cloud_field_at");
      expect(plain.fragmentWgsl).toContain("OceanLight(light * transmitted, light * sky)");
    }
  });

  it("takes each source's direct light and sky through the one read of its clouds at the sea's point", () => {
    for (const { cloudy, place } of FORMS) {
      const main = collapseWhitespace(wgslFunction(cloudy.fragmentWgsl, "fragmentMain"));
      expect(main).toContain(
        `let cloud = ocean_cloud_light(${place}, s, select(draw.sourceLight0.w, draw.sourceLight1.w, j == 1u));`,
      );
      expect(main).toContain(
        "let clouded_sky = sky * cloud.y + cloud.z * max(mu0, 0.0) * transmitted;",
      );
      expect(main).toContain("OceanLight(light * transmitted * cloud.x, light * clouded_sky)");
      expect(main).not.toContain("OceanLight(light * transmitted, light * sky)");
      expect(main.split("ocean_cloud_light(").length - 1).toBe(1);
      // The read is the ground's: the volume's shadow map where it holds the place, else the layer's.
      expect(collapseWhitespace(wgslFunction(cloudy.fragmentWgsl, "ocean_cloud_light"))).toContain(
        "cloud_ground_light(point / cloud_row(CLOUD_ROW_NORM).y, sun, source_radius)",
      );
    }
  });

  it("differs from its plain form only in its two lines and the clouds composed after it", () => {
    for (const { plain, place } of FORMS) {
      const cloudy = withOceanCloudLight(plain.fragmentWgsl, place);
      const [head] = cloudy.split("\n// R11's clouds over the sea");
      const restored = (head ?? "")
        .replace(/    let cloud = ocean_cloud_light\([^\n]*\n    let clouded_sky = [^\n]*\n/u, "")
        .replace(
          "OceanLight(light * transmitted * cloud.x, light * clouded_sky)",
          "OceanLight(light * transmitted, light * sky)",
        );
      expect(restored.trimEnd()).toBe(plain.fragmentWgsl.trimEnd());
    }
  });

  it("refuses a source that is not a sea pass's", () => {
    expect(() => withOceanCloudLight("fn fragmentMain() {}", "p")).toThrow(
      /sky line once, found 0/u,
    );
  });

  it("declares every binding its entry points read, the clouds' rows to the fragment stage alone", () => {
    for (const { cloudy } of FORMS) {
      expect(undeclaredBindings(cloudy)).toEqual([]);
      const read = bindingStages(cloudy.fragmentWgsl);
      expect([...(read.get(OCEAN_CLOUD_BINDINGS.cloudLayer) ?? [])]).toEqual(["fragment"]);
      expect([...(read.get(OCEAN_CLOUD_BINDINGS.cloudShadowRows) ?? [])]).toEqual(["fragment"]);
      const names = [
        ...(cloudy.textures ?? []).map((texture) => texture.name),
        ...(cloudy.storageBuffers ?? []).map((buffer) => buffer.name),
        ...cloudy.samplers.map((sampler) => sampler.name),
      ];
      expect(names).toEqual(expect.arrayContaining(Object.values(CLOUD_TEXTURES)));
    }
  });

  it("stays inside WebGPU's default limits: 16 sampled textures and 8 storage buffers a stage", () => {
    for (const { cloudy } of FORMS) {
      expect(cloudy.textures?.length ?? 0).toBeLessThanOrEqual(16);
      expect(storageIn(cloudy, "vertex")).toBeLessThanOrEqual(8);
      expect(storageIn(cloudy, "fragment")).toBeLessThanOrEqual(8);
    }
  });

  it("is registered in the catalogue, its own name and console name beside its plain form's", () => {
    for (const { plain, cloudy } of FORMS) {
      expect(
        WGSL_CATALOGUE.some((entry) => entry.kind === "material" && entry.spec === cloudy),
      ).toBe(true);
      expect(cloudy.name).toBe(`${plain.name} clouds`);
      expect(cloudy.displayName).not.toBe(plain.displayName);
    }
  });
});

describe("a source's light on the sea beneath its clouds (R11.T7.c)", () => {
  /** A spherical datum's radius. */
  const DATUM_RADIUS_M = 6_371_000;
  const UP: Vec3 = { x: 1, y: 0, z: 0 };
  /** The source 60° up, towards the map's north. */
  const frame: CloudShadowFrame = (() => {
    const map = cloudMapFrameOf(UP, scale(UP, DATUM_RADIUS_M + 2), DATUM_RADIUS_M);
    return {
      map,
      sun: normalise(add(scale(UP, Math.sqrt(3)), map.north)),
      datumRadiusM: DATUM_RADIUS_M,
      axisRatio: 1,
      deck: { baseM: 1_000, topM: 3_000 },
    };
  })();
  /** The volume's shadow over the map's plane west of its anchor's meridian, clear sky east of it. */
  const SHADOWED: CloudGroundLight = { direct: 0.2, sky: 0.45, diffuse: 0.3 };
  const read: ShadowLevelRead = (_, plane) => (plane.x < 0 ? SHADOWED : CLEAR_GROUND);
  /** The sea at sea level, `eastM` along the map's east of its anchor. */
  const seaAt = (eastM: number): Vec3 =>
    scale(normalise(add(scale(UP, DATUM_RADIUS_M), scale(frame.map.east, eastM))), DATUM_RADIUS_M);
  const clear = { sunLux: [1e5, 9e4, 8e4], skyLux: [2e4, 2.2e4, 2.6e4] } as const;
  const mu0 = Math.sqrt(3) / 2;

  it("dims the sun's light where the shadow map holds a cloud's shadow, and turns a share of it into the sky's", () => {
    const cloud = cloudGroundLightAt(frame, seaAt(-2_000), frame.sun, CLEAR_GROUND, read);
    expect(cloud.direct).toBeCloseTo(SHADOWED.direct, 12);
    expect(cloud.sky).toBeCloseTo(SHADOWED.sky, 12);
    expect(cloud.diffuse).toBeCloseTo(SHADOWED.diffuse, 12);
    const light = oceanCloudedLight(clear, cloud, mu0);
    clear.sunLux.forEach((sun, c) => {
      expect(light.sunLux[c]).toBeCloseTo(0.2 * sun, 6);
      expect(light.sunLux[c]).toBeLessThan(sun);
      expect(light.skyLux[c]).toBeCloseTo(0.45 * (clear.skyLux[c] ?? 0) + 0.3 * mu0 * sun, 6);
    });
  });

  it("leaves the clear sea's light where the map's sky is clear, and off the map where the layer is clear", () => {
    for (const eastM of [2_000, 5e6]) {
      const cloud = cloudGroundLightAt(frame, seaAt(eastM), frame.sun, CLEAR_GROUND, read);
      expect(cloud).toEqual(CLEAR_GROUND);
      expect(oceanCloudedLight(clear, cloud, mu0)).toEqual({
        sunLux: [...clear.sunLux],
        skyLux: [...clear.skyLux],
      });
    }
  });

  it("adds none of the cloud's light below the horizon", () => {
    const light = oceanCloudedLight(clear, SHADOWED, -0.1);
    expect(light.skyLux).toEqual(clear.skyLux.map((sky) => 0.45 * sky));
  });
});
