import { describe, expect, it } from "vitest";

import { WGSL_CATALOGUE } from "../engine/catalogue";
import type { WgslMaterialSpec } from "../engine/types";
import { bindingStages } from "../../test/wgslStages";
import { collapseWhitespace, wgslFunction } from "../../test/wgslText";
import {
  cloudLitMaterialSpec,
  TERRAIN_LIT_CLOUD_MATERIALS,
  terrainLitCloudMaterialSpec,
  withCloudLight,
} from "./cloudLight";
import {
  TERRAIN_LIT_BINDINGS,
  TERRAIN_LIT_CLUSTER_MATERIALS,
  TERRAIN_LIT_EMISSIVE_MATERIALS,
  TERRAIN_LIT_FIN_MATERIALS,
  TERRAIN_LIT_INTERIOR_MATERIALS,
  TERRAIN_LIT_MATERIALS,
  terrainLitMaterialSpec,
} from "./terrainMaterial";

/** The plain forms each cloudy form is made from, in {@link TERRAIN_LIT_CLOUD_MATERIALS}' order. */
const PLAIN: ReadonlyArray<WgslMaterialSpec> = [0, 1].flatMap((path) =>
  [
    TERRAIN_LIT_MATERIALS,
    TERRAIN_LIT_INTERIOR_MATERIALS,
    TERRAIN_LIT_CLUSTER_MATERIALS,
    TERRAIN_LIT_FIN_MATERIALS,
  ].flatMap((forms) => {
    const form = forms[path];
    return form === undefined ? [] : [form];
  }),
);

/** The `@group(2)` bindings a material's entry points read that its layout does not declare. */
function undeclaredBindings(spec: WgslMaterialSpec): number[] {
  const declared = new Set([
    ...(spec.storageBuffers ?? []).map((buffer) => buffer.binding),
    ...(spec.textures ?? []).map((texture) => texture.binding),
    ...spec.samplers.map((sampler) => sampler.binding),
  ]);
  return [...bindingStages(spec.vertexWgsl).keys()].filter((binding) => !declared.has(binding));
}

/** The storage buffers a stage of `spec` reads. */
function storageIn(spec: WgslMaterialSpec, stage: "vertex" | "fragment"): number {
  return (spec.storageBuffers ?? []).filter((buffer) =>
    (buffer.stages ?? ["vertex", "fragment"]).includes(stage),
  ).length;
}

describe("the lit terrain's cloudy forms (R11.T7.b)", () => {
  it("leaves the plain forms without either of the clouds' hooks written", () => {
    for (const plain of [...PLAIN, ...TERRAIN_LIT_EMISSIVE_MATERIALS]) {
      const shade = collapseWhitespace(wgslFunction(plain.fragmentWgsl, "shade_terrain"));
      expect(shade).toContain(
        "var shadowed = terrain_shadow_factor(q) * terrain_ring_shadow(q, towards);",
      );
      expect(plain.fragmentWgsl).not.toContain("terrain_cloud_light");
      expect(plain.fragmentWgsl).not.toContain("cloud_field_at");
    }
  });

  it("takes each source's light on the ground through the one read of its clouds", () => {
    const shade = collapseWhitespace(
      wgslFunction(terrainLitCloudMaterialSpec("baked-offsets").fragmentWgsl, "shade_terrain"),
    );
    // The volume's shadow map where it holds the place, the layer's tap where not (R11.T7.c).
    expect(shade).toContain("let cloud = cloud_ground_light(q, towards, star_radius);");
    expect(shade).toContain(
      "let clouded_sky = sky * cloud.y + cloud.z * max(mu_datum, 0.0) * transmitted * sky_factor;",
    );
    expect(shade).toContain(
      "out.radiance = out.radiance + light.xyz * misc.y * mean_a * clouded_sky * visible;",
    );
    expect(shade).toContain("var shadowed = cloud.x * terrain_ring_shadow(q, towards);");
    expect(shade).not.toContain("terrain_shadow_factor(q)");
    // The direct term takes the one lookup of the source's transmittance.
    expect(shade.split("atmosphere_sun_transmittance(").length - 1).toBe(1);
  });

  it("differs from its plain form only in the three lines and the clouds composed after it", () => {
    const plain = terrainLitMaterialSpec("face-differences").fragmentWgsl;
    const cloudy = withCloudLight(plain);
    const [head] = cloudy.split("\n// R11's clouds over the terrain");
    expect(head).toBeDefined();
    const restored = (head ?? "")
      .replace(
        /    let cloud = cloud_ground_light[\s\S]*?clouded_sky \* visible;\n/u,
        "    out.radiance = out.radiance + light.xyz * misc.y * mean_a * sky * visible;\n",
      )
      .replace(
        "    var shadowed = cloud.x * terrain_ring_shadow(q, towards);\n",
        "    var shadowed = terrain_shadow_factor(q) * terrain_ring_shadow(q, towards);\n",
      );
    const withoutDirectLookup = plain.replace(
      "      let transmitted = atmosphere_sun_transmittance(\n        height, mu_datum, local.latitude, local_azimuth(local, towards), j\n      );\n",
      "",
    );
    expect(restored.trimEnd()).toBe(withoutDirectLookup.trimEnd());
  });

  it("refuses a source whose shade_terrain is not the lit terrain's", () => {
    expect(() => withCloudLight("fn shade_terrain() {}")).toThrow(/sky light once, found 0/u);
  });

  it("declares every binding its entry points read, the clouds' to the fragment stage alone", () => {
    for (const spec of TERRAIN_LIT_CLOUD_MATERIALS) {
      expect(undeclaredBindings(spec)).toEqual([]);
      const read = bindingStages(spec.vertexWgsl);
      expect([...(read.get(TERRAIN_LIT_BINDINGS.cloudLayer) ?? [])]).toEqual(["fragment"]);
    }
  });

  it("stays inside WebGPU's default limits: 16 sampled textures and 8 storage buffers a stage", () => {
    for (const spec of TERRAIN_LIT_CLOUD_MATERIALS) {
      expect(spec.textures?.length ?? 0).toBeLessThanOrEqual(16);
      expect(storageIn(spec, "vertex")).toBeLessThanOrEqual(8);
      expect(storageIn(spec, "fragment")).toBeLessThanOrEqual(8);
    }
  });

  it("names each form on the console in three words at most, every name its own", () => {
    const names = TERRAIN_LIT_CLOUD_MATERIALS.map((spec) => spec.displayName);
    expect(new Set(names).size).toBe(names.length);
    for (const name of names) {
      expect(name.split(" ").length).toBeLessThanOrEqual(3);
      expect(name).toBe(name.toUpperCase());
    }
    expect(TERRAIN_LIT_CLOUD_MATERIALS.map((spec) => spec.name)).toEqual(
      PLAIN.map((spec) => `${spec.name}-clouds`),
    );
  });

  it("makes no cloudy form of an emissive form", () => {
    for (const emissive of TERRAIN_LIT_EMISSIVE_MATERIALS) {
      expect(() => cloudLitMaterialSpec(emissive)).toThrow(/has no cloudy form/u);
    }
  });

  it("is registered in the catalogue", () => {
    for (const spec of TERRAIN_LIT_CLOUD_MATERIALS) {
      expect(WGSL_CATALOGUE.some((entry) => entry.kind === "material" && entry.spec === spec)).toBe(
        true,
      );
    }
  });
});
