import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { brdf, brdfFromTable } from "../appearance/brdf";
import {
  mcewenLunarProfile,
  PHASE_TABLE_SAMPLES,
  PHASE_TABLE_STEP_RAD,
  type PhaseFactorTable,
  phaseFactorTableOf,
  type PhotometricLaw,
} from "../appearance/law";
import { lawPhaseIntegral, withTemplateCurve } from "../appearance/phase";
import { oblateAlbedoScale } from "../bodies/oblate";
import { WGSL_CATALOGUE } from "../engine/catalogue";
import type { WgslMaterialSpec } from "../engine/types";
import {
  declaredStructLayout,
  OFFSET_MEMBER,
  type StructLayout,
  uniformLayout,
} from "../engine/webgpu/uniforms";
import { annulusEdges } from "../lighting/annuli";
import { bindingStages } from "../../test/wgslStages";
import maturityNodesWgsl from "../shaders/maturityNodes.wgsl?raw";
import occluderWgsl from "../shaders/occluder.wgsl?raw";
import terrainDepthWgsl from "./shaders/terrainDepth.wgsl?raw";
import terrainLitWgsl from "./shaders/terrainLit.wgsl?raw";
import terrainLitShadeWgsl from "./shaders/terrainLitShade.wgsl?raw";
import terrainShadowWgsl from "./shaders/terrainShadow.wgsl?raw";
import terrainSurveyWgsl from "./shaders/terrainSurvey.wgsl?raw";
import cascadeShadowWgsl from "./shadows/cascadeShadow.wgsl?raw";
import { CASCADE_DEPTH_BIAS } from "./shadows/cascades";
import horizonWgsl from "./shadows/horizon.wgsl?raw";
import { LIT_VERTEX_CODE } from "./gpu/resources";
import { PATH_SOURCES } from "./gpu/material";
import {
  MATURITY_NODES,
  maturityCodeRatio,
  maturityNodeAlbedo,
  maturitySegment,
} from "./maturityNodes";
import {
  CLASS_LAWS_BYTES,
  CLASS_LAYERS_BYTES,
  CLASS_MATURITY_BYTES,
  CLASS_NODES_BYTES,
  CLASS_WEIGHTS_BYTES,
  SURVEY_CELL_CODE_BYTE,
  SURVEY_MASK_BYTES,
} from "./slotLayout";
import {
  AIRLESS_ICE_EXPONENT,
  CLASS_LAW_ROWS,
  CLASS_LAW_TEMPLATES,
  classPhotometryOf,
  classRowLaw,
  classTexelLaw,
  dominantSlot,
  packClassLaws,
  packClassNodes,
  packTerrainLighting,
  SHADOW_CASCADES_TEXTURE,
  SHADOW_COMPARE_SAMPLER,
  TERRAIN_CLASSES,
  TERRAIN_LIGHT_ROWS,
  TERRAIN_DEPTH_MATERIALS,
  TERRAIN_LIT_BINDINGS,
  TERRAIN_LIT_MATERIALS,
  TERRAIN_SHADOW_MATERIALS,
  PHASE_LAW,
  type TerrainLighting,
  TerrainPhaseRows,
  terrainDepthMaterialSpec,
  terrainLitMaterialSpec,
  terrainReflectance,
  terrainShadowMaterialSpec,
  terrainShares,
  terrainTexelLaws,
  texelClassAlbedo,
  texelLawOf,
  texelWeights,
} from "./terrainMaterial";

const RAD = Math.PI / 180;

/** A struct layout's members as `name : type @ offset`, for comparison. */
function membersOf(layout: StructLayout | null): string[] | undefined {
  return layout?.members.map((m) => `${m.name} : ${m.type} @ ${String(m.offsetBytes)}`);
}

/** The `Draw` the engine packs from a material's uniforms, its offset member first. */
function packedDraw(spec: WgslMaterialSpec): string[] | undefined {
  return membersOf(uniformLayout([OFFSET_MEMBER, ...spec.uniforms]));
}

/** Each storage buffer's name, its declared stages and the stages its entry points read it in. */
function storageStages(spec: WgslMaterialSpec): string[][] {
  const read = bindingStages(spec.vertexWgsl);
  return (spec.storageBuffers ?? []).map((buffer) => [
    buffer.name,
    [...(buffer.stages ?? [])].toSorted().join(" "),
    [...(read.get(buffer.binding) ?? [])].toSorted().join(" "),
  ]);
}

/** The `@group(2)` bindings a material's entry points read that its layout does not declare. */
function undeclaredBindings(spec: WgslMaterialSpec): number[] {
  const declared = new Set([
    ...(spec.storageBuffers ?? []).map((buffer) => buffer.binding),
    ...(spec.textures ?? []).map((texture) => texture.binding),
    ...spec.samplers.map((sampler) => sampler.binding),
  ]);
  return [...bindingStages(spec.vertexWgsl).keys()].filter((binding) => !declared.has(binding));
}

/** A bake's photometry: each slot `[B, V, R, law, L(0)]`, the rest empty. */
function photometryOf(slots: ReadonlyArray<readonly [number, number, number, number, number]>) {
  const out = new Float32Array(TERRAIN_CLASSES * 5);
  slots.forEach((slot, k) => {
    out.set(slot, 5 * k);
  });
  return out;
}

/** A material's storage buffers' names. */
function storageNames(spec: WgslMaterialSpec): string[] {
  return (spec.storageBuffers ?? []).map((buffer) => buffer.name);
}

describe("the lit terrain's depth material for the symbology (R10.T16.c)", () => {
  it("registers both vertex paths' depth materials in the catalogue", () => {
    for (const spec of TERRAIN_DEPTH_MATERIALS) {
      expect(WGSL_CATALOGUE.some((entry) => entry.kind === "material" && entry.spec === spec)).toBe(
        true,
      );
    }
    expect(TERRAIN_DEPTH_MATERIALS.map((spec) => spec.displayName)).toEqual([
      "TERRAIN DEPTH OFFSETS",
      "TERRAIN DEPTH",
    ]);
  });

  it("writes depth alone, and binds what its entry points read: R05's vertex storage and the survey mask", () => {
    const offsets = terrainDepthMaterialSpec("baked-offsets");
    const read = bindingStages(offsets.vertexWgsl);
    expect([offsets.colourWrites, offsets.depthWrite, offsets.textures]).toEqual([false, true, []]);
    expect(
      (offsets.storageBuffers ?? []).map((buffer) => [
        buffer.name,
        [...(buffer.stages ?? [])].toSorted().join(" "),
        [...(read.get(buffer.binding) ?? [])].toSorted().join(" "),
      ]),
    ).toEqual([
      ["heights", "vertex", "vertex"],
      ["slots", "vertex", "vertex"],
      ["instances", "vertex", "vertex"],
      ["contacts", "vertex", "vertex"],
      ["offsets", "vertex", "vertex"],
      ["surveyMask", "fragment vertex", "fragment vertex"],
    ]);
    // No binding its entry points read is left out of the layout.
    const declared = new Set((offsets.storageBuffers ?? []).map((buffer) => buffer.binding));
    expect([...read.keys()].filter((binding) => !declared.has(binding))).toEqual([]);
  });

  it("takes the lit material's own draw fields, without the cascades', with the overlay's slope term after them", () => {
    expect(terrainDepthMaterialSpec("face-differences").uniforms.map((u) => u.name)).toEqual([
      ...terrainLitMaterialSpec("face-differences")
        .uniforms.slice(0, 3)
        .map((u) => u.name),
      "occluderSlopePx",
    ]);
    expect(terrainDepthWgsl).toContain("occluderSlopePx : f32,");
    const spec = terrainDepthMaterialSpec("baked-offsets");
    expect(membersOf(declaredStructLayout(spec.vertexWgsl, "Draw"))).toEqual(packedDraw(spec));
  });

  it("pushes its depth away by the occluders' constant fraction", () => {
    const fraction = /DEPTH_FRACTION\s*=\s*([0-9.]+)/u;
    const depthFraction = /TERRAIN_DEPTH_FRACTION : f32 = ([0-9.]+)/u;
    expect(terrainDepthWgsl.match(depthFraction)?.[1]).toBe(occluderWgsl.match(fraction)?.[1]);
  });
});

describe("the lit terrain's material (R10.T10.b)", () => {
  it("registers both vertex paths' lit materials in the catalogue", () => {
    for (const spec of TERRAIN_LIT_MATERIALS) {
      expect(WGSL_CATALOGUE.some((entry) => entry.kind === "material" && entry.spec === spec)).toBe(
        true,
      );
    }
    expect(TERRAIN_LIT_MATERIALS.map((spec) => spec.displayName)).toEqual([
      "LIT TERRAIN OFFSETS",
      "LIT TERRAIN",
    ]);
  });

  it("declares each binding its WGSL reads, the offsets on the baked-offsets path alone", () => {
    const offsets = terrainLitMaterialSpec("baked-offsets");
    const differences = terrainLitMaterialSpec("face-differences");
    expect(storageNames(offsets)).toContain("offsets");
    expect(storageNames(differences)).not.toContain("offsets");
    expect(storageNames(differences)).toEqual([
      "heights",
      "slots",
      "instances",
      "contacts",
      "classLayers",
      "terrainLights",
      "horizonMap",
      "surveyMask",
    ]);
    // Nine storage buffers on the baked-offsets path, five read by the vertex stage alone, three by
    // the fragment stage alone, and the survey mask by both (decision-r10-t10b-r09-t2e §5;
    // R10.T10.c).
    expect(offsets.storageBuffers).toHaveLength(9);
    expect(
      (offsets.storageBuffers ?? []).map((buffer) => [buffer.name, buffer.stages?.join(" ")]),
    ).toEqual([
      ["heights", "vertex"],
      ["slots", "vertex"],
      ["instances", "vertex"],
      ["contacts", "vertex"],
      ["offsets", "vertex"],
      ["classLayers", "fragment"],
      ["terrainLights", "fragment"],
      ["horizonMap", "fragment"],
      ["surveyMask", "vertex fragment"],
    ]);
    const textures = differences.textures ?? [];
    expect(textures.map((t) => t.binding)).toEqual([4, 7, 9, 10, 11, 12, 13, 14, 15, 19]);
    expect(differences.samplers.map((s) => s.binding)).toEqual([20]);
    for (const [name, binding] of [
      ["class_layers", TERRAIN_LIT_BINDINGS.classLayers],
      ["phase_factor_table", TERRAIN_LIT_BINDINGS.phaseFactorTable],
      ["terrain_lights", TERRAIN_LIT_BINDINGS.terrainLights],
      ["atmosphere_body", TERRAIN_LIT_BINDINGS.atmosphere],
      ["horizon_map", TERRAIN_LIT_BINDINGS.horizonMap],
      ["survey_mask", TERRAIN_LIT_BINDINGS.surveyMask],
      ["shadow_cascades", TERRAIN_LIT_BINDINGS.shadowCascades],
      ["shadow_compare", TERRAIN_LIT_BINDINGS.shadowCompare],
    ] as const) {
      expect(terrainLitWgsl + horizonWgsl + terrainSurveyWgsl + cascadeShadowWgsl).toMatch(
        new RegExp(`@binding\\(${String(binding)}\\) var[^\\n]* ${name}`, "u"),
      );
    }
    // No binding its entry points read is left out of the layout, on either path.
    expect([offsets, differences].map(undeclaredBindings)).toEqual([[], []]);
  });

  it("states the hooks R11 fills, each with no effect", () => {
    for (const hook of [
      "fn terrain_decoration(p : vec3f, normal : vec3f) -> TerrainDecoration",
      "fn terrain_shadow_factor(p : vec3f) -> f32",
      "fn terrain_ring_shadow(p : vec3f, sun : vec3f) -> vec3f",
      "fn terrain_sky_factor(p : vec3f) -> vec3f",
    ]) {
      expect(terrainLitWgsl).toContain(hook);
    }
  });

  it("carries the layout's sizes: two vec4f a class after the weights, 25 rows of lights", () => {
    expect(CLASS_LAW_ROWS * 16).toBe(CLASS_LAWS_BYTES);
    expect(terrainLitWgsl).toContain(
      `const CLASS_LAW_WORD : u32 = ${String(CLASS_WEIGHTS_BYTES / 4)}u;`,
    );
    expect(terrainLitWgsl).toContain(
      `const CLASS_LAYER_WORDS : u32 = ${String(CLASS_LAYERS_BYTES / 4)}u;`,
    );
    expect(terrainLitWgsl).toContain("const TERRAIN_CLASSES : u32 = 8u;");
    expect(terrainLitWgsl).toContain("const FIRST_TERRAIN_SECONDARY_ROW : u32 = 21u;");
    expect(TERRAIN_LIGHT_ROWS).toBe(21 + 2 * 2);
  });

  it("carries the survey mask's layout: its words a slot, and the byte of its cell's code (R10.T10.c)", () => {
    expect([
      terrainSurveyWgsl.includes(
        `const SURVEY_MASK_WORDS : u32 = ${String(SURVEY_MASK_BYTES / 4)}u;`,
      ),
      terrainSurveyWgsl.includes(
        `const SURVEY_CELL_CODE_BYTE : u32 = ${String(SURVEY_CELL_CODE_BYTE)}u;`,
      ),
    ]).toEqual([true, true]);
  });

  it.each(Object.entries(LIT_VERTEX_CODE))(
    "carries the lit mesh's vertex code %s as resources.ts gives it (R10.T10.c)",
    (name, code) => {
      const constant = `LIT_${name.replace(/[A-Z]/gu, (letter) => `_${letter}`).toUpperCase()}`;
      expect(terrainSurveyWgsl).toContain(`const ${constant} : u32 = ${String(code)}u;`);
    },
  );
});

describe("the lit terrain's shadow receiver (R10.T9.b)", () => {
  const lit = terrainLitMaterialSpec("face-differences");

  it("takes its own draw fields, then the cascades', in the Draw struct's order", () => {
    expect(lit.uniforms.map((u) => `${u.name} : ${u.type}`)).toEqual([
      "bodyRotation : mat4x4f",
      "cameraBodyFixedM : vec4f",
      "atlas : vec4f",
      "cascadeLight0 : mat4x4f",
      "cascadeLight1 : mat4x4f",
      "cascadeLight2 : mat4x4f",
      "cascadeLight3 : mat4x4f",
      "cascadeFarM : vec4f",
      "cascadeFadeStartM : vec4f",
      "cascadeNormalOffsetM : vec4f",
      "cascadeViewAxis : vec4f",
    ]);
    for (const path of ["baked-offsets", "face-differences"] as const) {
      const spec = terrainLitMaterialSpec(path);
      expect(membersOf(declaredStructLayout(spec.vertexWgsl, "Draw"))).toEqual(packedDraw(spec));
    }
  });

  it("binds the cascades' layers as a depth array and reads them through a linear greater-equal comparison sampler", () => {
    expect(lit.textures?.find((t) => t.name === SHADOW_CASCADES_TEXTURE)).toEqual({
      name: "shadowCascades",
      binding: 19,
      sampleType: "depth",
      viewDimension: "2d-array",
    });
    expect(lit.samplers).toEqual([
      {
        name: SHADOW_COMPARE_SAMPLER,
        filter: "linear",
        address: "clamp-to-edge",
        binding: 20,
        compare: "greater-equal",
      },
    ]);
    expect(cascadeShadowWgsl).toContain(
      "@group(2) @binding(19) var shadow_cascades : texture_depth_2d_array;",
    );
    expect(cascadeShadowWgsl).toContain(
      "@group(2) @binding(20) var shadow_compare : sampler_comparison;",
    );
    // Read by the fragment stage alone, where the receiver runs.
    const read = bindingStages(lit.vertexWgsl);
    expect([read.get(19), read.get(20)].map((stages) => Array.from(stages ?? []))).toEqual([
      ["fragment"],
      ["fragment"],
    ]);
  });

  it("composes the receiver into the lit material alone, after its Draw, not into the shared source", () => {
    const shade = lit.vertexWgsl.indexOf(terrainLitShadeWgsl);
    const receiver = lit.vertexWgsl.indexOf(cascadeShadowWgsl);
    expect(shade).toBeGreaterThan(0);
    expect(receiver).toBe(shade + terrainLitShadeWgsl.length);
    for (const spec of [
      terrainDepthMaterialSpec("face-differences"),
      terrainShadowMaterialSpec("face-differences", false),
    ]) {
      const code = spec.vertexWgsl.replaceAll(/\/\/[^\n]*/gu, "");
      expect(code).not.toContain("fn cascade_lit_fraction");
      expect(code).not.toContain("cascadeLight0");
      expect(code).not.toContain("shadow_cascades");
    }
  });

  it("samples with textureSampleCompareLevel, which needs no uniform control flow, never textureSampleCompare", () => {
    const code = cascadeShadowWgsl.replaceAll(/\/\/[^\n]*/gu, "");
    expect(code.match(/textureSampleCompareLevel\(/gu)).toHaveLength(1);
    expect(code).not.toMatch(/textureSampleCompare\(/u);
  });

  it("forms the geometric normal before the discard and hands c′ to shade_terrain, 1 for reference fragments", () => {
    const code = terrainLitShadeWgsl.replaceAll(/\/\/[^\n]*/gu, "");
    const derivatives = code.indexOf("dpdyFine(in.fromCameraBf)");
    expect(derivatives).toBeGreaterThan(0);
    expect(derivatives).toBeLessThan(code.indexOf("discard"));
    expect(code).toContain("var cascade_lit = 1.0;");
    expect(code).toContain("shade_terrain(in, normal, reference, cascade_lit)");
    expect(terrainLitWgsl).toContain("cascade_lit : f32,");
  });

  it("multiplies the cascades' term into star 0's direct light alone, at the map's share of the cut R07 used", () => {
    expect(terrainLitWgsl).toContain("fn terrain_cascade_term(m : f32, cascade_lit : f32) -> f32");
    expect(terrainLitWgsl).toMatch(
      /if \(j == 0u\) \{[^}]*disc_visible_fraction\(star_distance_radii, clamp\(phi \+ max\(local_horizon, 0\.0\), 0\.0, LIT_PI\)\);\s*shadowed = shadowed \* terrain_cascade_term\(m, cascade_lit\);/u,
    );
    // The meter's class keeps the horizon's and the eclipse's tests alone.
    expect(terrainLitWgsl).toContain(
      "out.lit = out.lit || (horizon > TERRAIN_LIT_IRRADIANCE && any(visible > vec3f(TERRAIN_LIT_IRRADIANCE)));",
    );
  });
});

describe("the terrain's shadow casters (R10.T9.b)", () => {
  it("registers both vertex paths' casters in the catalogue, compiled for depth-only", () => {
    for (const spec of TERRAIN_SHADOW_MATERIALS) {
      const entry = WGSL_CATALOGUE.find(
        (candidate) => candidate.kind === "material" && candidate.spec === spec,
      );
      expect(entry?.kind === "material" ? entry.targets : undefined).toEqual(["depth-only"]);
    }
    expect(TERRAIN_SHADOW_MATERIALS.map((spec) => spec.displayName)).toEqual([
      "TERRAIN SHADOWS OFFSETS",
      "TERRAIN SHADOWS",
    ]);
    expect(TERRAIN_SHADOW_MATERIALS.map((spec) => spec.unclippedDepth)).toEqual([false, false]);
  });

  it.each([true, false])(
    "writes depth alone, cull none, biased away by the cascades' bias, unclippedDepth %s as given",
    (unclippedDepth) => {
      const spec = terrainShadowMaterialSpec("face-differences", unclippedDepth);
      expect({
        cullMode: spec.cullMode,
        depthWrite: spec.depthWrite,
        colourWrites: spec.colourWrites,
        blend: spec.blend,
        textures: spec.textures,
        samplers: spec.samplers,
        depthBiasAway: spec.depthBiasAway,
        unclippedDepth: spec.unclippedDepth,
      }).toEqual({
        cullMode: "none",
        depthWrite: true,
        colourWrites: false,
        blend: "none",
        textures: [],
        samplers: [],
        depthBiasAway: {
          constant: CASCADE_DEPTH_BIAS.constant,
          slopeScale: CASCADE_DEPTH_BIAS.slopeScale,
          clamp: CASCADE_DEPTH_BIAS.clamp,
        },
        unclippedDepth,
      });
    },
  );

  it("binds what its entry points read, as the depth material does: R05's vertex storage and the survey mask", () => {
    const offsets = terrainShadowMaterialSpec("baked-offsets", false);
    expect(storageStages(offsets)).toEqual([
      ["heights", "vertex", "vertex"],
      ["slots", "vertex", "vertex"],
      ["instances", "vertex", "vertex"],
      ["contacts", "vertex", "vertex"],
      ["offsets", "vertex", "vertex"],
      ["surveyMask", "fragment vertex", "fragment vertex"],
    ]);
    expect(offsets.storageBuffers).toEqual(
      terrainDepthMaterialSpec("baked-offsets").storageBuffers,
    );
    expect(undeclaredBindings(offsets)).toEqual([]);
    expect(storageNames(terrainShadowMaterialSpec("face-differences", false))).not.toContain(
      "offsets",
    );
  });

  it("takes the lit material's own draw fields alone, its Draw mirroring each the shared source reads", () => {
    for (const path of ["baked-offsets", "face-differences"] as const) {
      const spec = terrainShadowMaterialSpec(path, false);
      expect(spec.uniforms).toEqual(terrainLitMaterialSpec(path).uniforms.slice(0, 3));
      expect(membersOf(declaredStructLayout(spec.vertexWgsl, "Draw"))).toEqual(packedDraw(spec));
    }
  });

  it.each(["baked-offsets", "face-differences"] as const)(
    "draws the lit vertex stage unchanged on %s, with a fragment stage that only discards and writes nothing",
    (path) => {
      const caster = terrainShadowMaterialSpec(path, false).vertexWgsl;
      const lit = terrainLitMaterialSpec(path).vertexWgsl;
      // The shared source up to the material's own Draw, and the path's source after it, are the
      // lit material's own, byte for byte.
      const shared = lit.slice(0, lit.indexOf(terrainLitShadeWgsl));
      expect(caster).toBe(shared + terrainShadowWgsl + PATH_SOURCES[path]);
      expect(shared).toContain("fn vertexMain(");
      const code = terrainShadowWgsl.replaceAll(/\/\/[^\n]*/gu, "");
      expect(code).toMatch(
        /@fragment\s+fn fragmentMain\(in : LitVertexOut\) \{\s*if \(!survey_kept\(in\)\) \{\s*discard;\s*\}\s*\}/u,
      );
      expect(code).not.toContain("@location");
      expect(code).not.toContain("frag_depth");
    },
  );
});

describe("the class laws (decision-r10-limb; decision-r07-t8b)", () => {
  it("takes R10.T1.a's three laws onto R07's moon, airless-ice and lambert templates", () => {
    expect(CLASS_LAW_TEMPLATES).toEqual({
      1: { template: "moon", phaseExponent: 1 },
      2: { template: "airless-ice", phaseExponent: AIRLESS_ICE_EXPONENT },
      3: { template: "lambert", phaseExponent: 1 },
    });
  });

  it("solves the Ice law's exponent to Ganymede's q of 0.80 at a constant L of 1, as the generator does", () => {
    const law: PhotometricLaw = {
      a: [1, 1, 1],
      lommelSeeligerShare: 1,
      template: "airless-ice",
      phaseExponent: [AIRLESS_ICE_EXPONENT, AIRLESS_ICE_EXPONENT, AIRLESS_ICE_EXPONENT],
    };
    expect(lawPhaseIntegral(law)[1]).toBeCloseTo(0.8, 6);
  });

  it("tabulates the particulate rows' L(α) as L(0) times McEwen's profile, and Lambert's f ≡ 1, L ≡ 0", () => {
    const ice = phaseFactorTableOf(classRowLaw(2, 0.38));
    const lambert = phaseFactorTableOf(classRowLaw(3, 0));
    for (const i of [0, 60, 180, 300, 360]) {
      const alpha = i * PHASE_TABLE_STEP_RAD;
      expect(ice.alpha[i]).toBe(Math.fround(0.38 * mcewenLunarProfile(alpha)));
      expect(lambert.alpha[i]).toBe(0);
      expect([lambert.rgb[3 * i], lambert.rgb[3 * i + 1], lambert.rgb[3 * i + 2]]).toEqual([
        1, 1, 1,
      ]);
    }
  });

  it("reads a slot's photometry as a law, R, V and B as r, g and b, and none for law 0", () => {
    const photometry = photometryOf([
      [0.1, 0.12, 0.14, 1, 1],
      [0.9, 0.92, 0.94, 3, 0],
      [0, 0, 0, 0, 0],
    ]);
    expect(classPhotometryOf(photometry, 0)).toEqual({
      a: [Math.fround(0.14), Math.fround(0.12), Math.fround(0.1)],
      l0: 1,
      code: 1,
    });
    expect(classPhotometryOf(photometry, 1)?.code).toBe(3);
    expect(classPhotometryOf(photometry, 2)).toBeNull();
    expect(classPhotometryOf(photometry, 7)).toBeNull();
  });

  it("refuses a slot naming a law no law has", () => {
    expect(() => classPhotometryOf(photometryOf([[0.1, 0.1, 0.1, 9, 1]]), 0)).toThrow(/law 9/u);
  });

  it("keys the rows by law and L(0): every material at L(0) = 1 shares its law's row, a bright frost takes its own", () => {
    const elsewhere = withTemplateCurve({
      a: [0.1, 0.1, 0.1],
      lommelSeeligerShare: 1,
      template: "moon",
      phaseExponent: [1, 1, 1],
    });
    const rows = new TerrainPhaseRows(elsewhere);
    expect(rows.laws).toEqual([elsewhere]);
    const regolith = rows.rowOf(PHASE_LAW.regolith, 1);
    expect(rows.rowOf(PHASE_LAW.regolith, 1)).toBe(regolith);
    const frost = rows.rowOf(PHASE_LAW.ice, 0.57789);
    const ice = rows.rowOf(PHASE_LAW.ice, 1);
    const lambert = rows.rowOf(PHASE_LAW.lambert, 0);
    expect([regolith, frost, ice, lambert]).toEqual([1, 2, 3, 4]);
    expect(rows.laws[frost]?.lommelSeeligerShare).toBe(0.57789);
  });

  it("counts a change of row 0's law as a version, and no change for the same law", () => {
    const elsewhere = classRowLaw(PHASE_LAW.regolith, 1);
    const rows = new TerrainPhaseRows(elsewhere);
    const before = rows.version;
    expect(rows.setElsewhere(elsewhere)).toBe(false);
    expect(rows.version).toBe(before);
    expect(rows.setElsewhere({ ...elsewhere, phaseExponent: [1.1, 1, 1] })).toBe(true);
    expect(rows.version).toBe(before + 1);
  });

  it("counts no change for a law equal to row 0's by value, as each view's frame gives its own (R10.T16.b)", () => {
    const elsewhere = classRowLaw(PHASE_LAW.regolith, 1);
    const rows = new TerrainPhaseRows(elsewhere);
    const before = rows.version;
    expect(rows.setElsewhere({ ...elsewhere, a: [...elsewhere.a] })).toBe(false);
    expect(rows.version).toBe(before);
    expect(rows.setElsewhere({ ...elsewhere, lommelSeeligerShare: 0.5 })).toBe(true);
    expect(rows.version).toBe(before + 1);
  });

  it("packs a slot's class laws: A and L(0), then the row, −1 for a slot with none", () => {
    const rows = new TerrainPhaseRows(classRowLaw(3, 0));
    const packed = packClassLaws(
      photometryOf([
        [0.1, 0.12, 0.14, 1, 1],
        [0, 0, 0, 0, 0],
        [0.9, 0.92, 0.94, 3, 0],
      ]),
      rows,
      1,
    );
    expect(packed).toHaveLength(CLASS_LAW_ROWS * 4);
    expect([...packed.subarray(0, 5)]).toEqual([
      Math.fround(0.14),
      Math.fround(0.12),
      Math.fround(0.1),
      1,
      1,
    ]);
    expect(packed[8 + 4]).toBe(-1);
    expect([...packed.subarray(16, 21)]).toEqual([
      Math.fround(0.94),
      Math.fround(0.92),
      Math.fround(0.9),
      0,
      2,
    ]);
    for (let k = 3; k < TERRAIN_CLASSES; k += 1) {
      expect(packed[8 * k + 4]).toBe(-1);
    }
    expect(rows.laws).toHaveLength(3);
  });

  it("scales each class's A for an oblate figure as law 0's and R07's disc's laws are (R10.T10.e)", () => {
    const cOverA = 0.9;
    const photometry = photometryOf([
      [0.1, 0.12, 0.14, 1, 1],
      [0.8, 0.85, 0.9, 2, 0.6],
      [0.9, 0.92, 0.94, 3, 0],
    ]);
    const packed = packClassLaws(photometry, new TerrainPhaseRows(classRowLaw(3, 0)), cOverA);
    const expected = [
      [0.14, 0.12, 0.1, 1],
      [0.9, 0.85, 0.8, 0.6],
      [0.94, 0.92, 0.9, 0],
    ] as const;
    expected.forEach(([r, g, b, l0], k) => {
      const factor = oblateAlbedoScale(Math.fround(l0), cOverA);
      // A factor of order the flattening wherever the law limb-darkens, 1 under Lommel–Seeliger's
      // (L(0) = 1), whose disc reaches p on any figure.
      expect(factor === 1).toBe(l0 === 1);
      expect([...packed.subarray(8 * k, 8 * k + 4)]).toEqual([
        Math.fround(Math.fround(r) * factor),
        Math.fround(Math.fround(g) * factor),
        Math.fround(Math.fround(b) * factor),
        Math.fround(l0),
      ]);
    });
  });
});

describe("the lit terrain's lights", () => {
  const annuli = annulusEdges(0.6, 0.2, 2);
  const lighting: TerrainLighting = {
    lights: [
      {
        direction: vec3(0, 0.6, 0.8),
        distance: 2.3e4,
        radius: 109,
        illuminance: [1.2e5, 1.1e5, 1.0e5],
        annuli: [annuli, annuli, annuli],
      },
    ],
    occluders: [{ centre: vec3(60, 0, 0), radius: 0.27 }],
    secondaries: [
      { direction: vec3(1, 0, 0), distance: 60, radius: 0.27, illuminance: [0.2, 0.2, 0.2] },
    ],
    elsewhere: withTemplateCurve({
      a: [0.12, 0.12, 0.12],
      lommelSeeligerShare: 1,
      template: "moon",
      phaseExponent: [1, 1, 1],
    }),
    exposureScale: 1e-3,
  };

  it("packs them as terrainLit.wgsl reads them, law 0's A scaled for the figure", () => {
    const rows = packTerrainLighting(lighting, 0.9);
    expect(rows).toHaveLength(TERRAIN_LIGHT_ROWS * 4);
    const row = (i: number) => [...rows.subarray(4 * i, 4 * i + 4)];
    expect(row(0)).toEqual([1, 1, 1, 2]);
    const k = oblateAlbedoScale(1, 0.9);
    expect(row(1)).toEqual([0.12 * k, 0.12 * k, 0.12 * k, 1].map(Math.fround));
    expect(row(2)).toEqual([0, Math.fround(1e-3 / Math.PI), 0, 0]);
    expect(row(3)).toEqual([0, 0.6, 0.8, 2.3e4].map(Math.fround));
    expect(row(4)).toEqual([1.2e5, 1.1e5, 1.0e5, 109].map(Math.fround));
    expect(row(5)).toEqual([...annuli.edges.subarray(1, 3), 1, 1].map(Math.fround));
    expect(row(19)).toEqual([60, 0, 0, 0.27].map(Math.fround));
    expect(row(21)).toEqual([1, 0, 0, 60]);
    expect(row(22)).toEqual([0.2, 0.2, 0.2, 0.27].map(Math.fround));
  });

  it("draws at most two stars, occluders and planetshine sources", () => {
    const star = lighting.lights[0];
    const occluder = lighting.occluders[0];
    const source = lighting.secondaries[0];
    if (star === undefined || occluder === undefined || source === undefined) {
      throw new Error("the fixture has one of each");
    }
    const many = packTerrainLighting(
      {
        ...lighting,
        lights: [star, star, star],
        occluders: [occluder, occluder, occluder],
        secondaries: [source, source, source],
      },
      1,
    );
    expect([...many.subarray(0, 3)]).toEqual([2, 2, 2]);
  });
});

/** An airless body's law of normal albedo `a` and L(0) `l` on a law's row. */
function rowLaw(
  code: 1 | 2 | 3,
  a: number,
  l: number,
): { law: PhotometricLaw; table: PhaseFactorTable } {
  const row = classRowLaw(code, l);
  return { law: { ...row, a: [a, a, a] }, table: phaseFactorTableOf(row) };
}

describe("a texel's blend (R10.T10.b; Design note 8)", () => {
  const regolith = rowLaw(1, 0.12, 1);
  const frost = rowLaw(2, 0.85, 0.65);
  const geometries = [
    [0.9, 0.7, 20],
    [0.5, 0.1, 60],
    [0.2, 0.95, 110],
  ] as const;

  it("reflects w r₁ + (1 − w) r₂ for two classes in weights w and 1 − w, to 10⁻⁶", () => {
    for (const w of [0, 0.25, 0.5, 0.9, 1]) {
      for (const [mu0, mu, deg] of geometries) {
        const ours = terrainReflectance(
          [null, texelLawOf(regolith.law), texelLawOf(frost.law)],
          [0, w, 1 - w],
          mu0,
          mu,
          deg * RAD,
        );
        const one = brdf(regolith.law, mu0, mu, deg * RAD);
        const two = brdf(frost.law, mu0, mu, deg * RAD);
        for (const c of [0, 1, 2] as const) {
          expect(Math.abs(ours[c] - (w * one[c] + (1 - w) * two[c]))).toBeLessThan(1e-6);
        }
      }
    }
  });

  it("sums an even blend of regolith (0.12, L 1) and snow (0.9, L 0) under one f at μ₀ 0.5, μ 0.1, not the 1.7× of the mixed law", () => {
    // The test's own construction: one f for both, each its own L, as Design note 8's example.
    const f = phaseFactorTableOf(classRowLaw(1, 1)).rgb;
    const tableAt = (l: number): PhaseFactorTable => ({
      rgb: f,
      alpha: new Float32Array(PHASE_TABLE_SAMPLES).fill(l),
    });
    const laws = [
      null,
      { a: [0.12, 0.12, 0.12] as const, table: tableAt(1) },
      { a: [0.9, 0.9, 0.9] as const, table: tableAt(0) },
    ];
    const alpha = 30 * RAD;
    const summed = terrainReflectance(laws, [0, 0.5, 0.5], 0.5, 0.1, alpha)[1];
    const parts =
      0.5 * brdfFromTable({ a: [0.12, 0.12, 0.12] }, tableAt(1), 0.5, 0.1, alpha)[1] +
      0.5 * brdfFromTable({ a: [0.9, 0.9, 0.9] }, tableAt(0), 0.5, 0.1, alpha)[1];
    const mixed = brdfFromTable({ a: [0.51, 0.51, 0.51] }, tableAt(0.5), 0.5, 0.1, alpha)[1];
    expect(summed).toBeCloseTo(parts, 12);
    expect(mixed / summed).toBeCloseTo(1.7, 2);
  });

  it("adds nothing for a zero weight, whatever the law", () => {
    const broken = { a: [Number.NaN, Number.NaN, Number.NaN] as const, table: regolith.table };
    const without = terrainReflectance([null, texelLawOf(regolith.law)], [0, 1], 0.6, 0.5, 0.4);
    const with0 = terrainReflectance(
      [null, texelLawOf(regolith.law), broken],
      [0, 1, 0],
      0.6,
      0.5,
      0.4,
    );
    expect(with0).toEqual(without);
  });

  it("names the class of the largest weight, the lowest on a tie, none without weight", () => {
    expect(dominantSlot([0.1, 0.5, 0.4, 0, 0, 0, 0, 0])).toBe(1);
    expect(dominantSlot([0.4, 0.4, 0.2, 0, 0, 0, 0, 0])).toBe(0);
    expect(dominantSlot([0, 0, 0, 0, 0, 0, 0, 0.2])).toBe(7);
    expect(dominantSlot([0, 0, 0, 0, 0, 0, 0, 0])).toBeNull();
  });

  it("draws a one-class palette's texel as the per-body law of that class", () => {
    const photometry = { a: [0.14, 0.12, 0.1] as const, l0: 0.84, code: 2 as const };
    const perBody = withTemplateCurve({
      a: photometry.a,
      lommelSeeligerShare: photometry.l0,
      template: "airless-ice",
      phaseExponent: [AIRLESS_ICE_EXPONENT, AIRLESS_ICE_EXPONENT, AIRLESS_ICE_EXPONENT],
    });
    for (const [mu0, mu, deg] of geometries) {
      expect(
        terrainReflectance([null, classTexelLaw(photometry, 1)], [0, 1], mu0, mu, deg * RAD),
      ).toEqual(brdf(perBody, mu0, mu, deg * RAD));
    }
  });

  it("gives law 0 what the named slots leave, and scales them down past 1", () => {
    const named = [true, false, true, false, false, false, false, false];
    expect(terrainShares([0.5, 0.3, 0.2, 0, 0, 0, 0, 0], named)).toEqual([
      0.30000000000000004, 0.5, 0, 0.2, 0, 0, 0, 0, 0,
    ]);
    const over = terrainShares([0.75, 0, 0.5, 0, 0, 0, 0, 0], named);
    expect(over[0]).toBe(0);
    expect((over[1] ?? 0) + (over[3] ?? 0)).toBeCloseTo(1, 15);
    expect(terrainShares([0, 0, 0, 0, 0, 0, 0, 0], named)[0]).toBe(1);
  });

  it("reads a texel's weights bilinearly between its quad's vertices, each byte ÷ 255", () => {
    const weights = new Uint8Array(65 * 65 * 8);
    // Vertex (3, 4) all slot 0, vertex (4, 4) all slot 1.
    weights[8 * (4 * 65 + 3)] = 255;
    weights[8 * (4 * 65 + 4) + 1] = 255;
    const at = (x: number, y: number) => texelWeights(weights, x / 64, y / 64);
    expect(at(3, 4).slice(0, 2)).toEqual([1, 0]);
    expect(at(4, 4).slice(0, 2)).toEqual([0, 1]);
    const middle = at(3.25, 4);
    expect(middle[0]).toBeCloseTo(0.75, 12);
    expect(middle[1]).toBeCloseTo(0.25, 12);
    expect(texelWeights(new Uint8Array(), 0.5, 0.5)).toEqual([0, 0, 0, 0, 0, 0, 0, 0]);
  });
});

/** The text of WGSL function `name` in `source`, from its `fn` to its closing brace. */
function wgslFunction(source: string, name: string): string {
  const start = source.indexOf(`fn ${name}(`);
  const end = source.indexOf("\n}\n", start);
  if (start === -1 || end === -1) {
    throw new Error(`the source has no function ${name}`);
  }
  return source.slice(start, end + 2);
}

/** Vertices along a patch's side. */
const SIDE = 65;

/**
 * A maturity layer's codes for the twin's tests: each vertex's code at n and at n − 1 from a
 * pattern of their own, spread over unorm16's range.
 */
const CODES = Uint16Array.from({ length: 2 * SIDE * SIDE }, (_, n) => {
  const v = Math.floor(n / 2);
  const x = v % SIDE;
  const y = Math.floor(v / SIDE);
  return n % 2 === 0 ? (x * 997 + y * 6_151) % 65_536 : (x * 3_001 + y * 211 + 17) % 65_536;
});

/**
 * A patch's maturity nodes as the module lays them out, each node's B, V and R: slot k's level ℓ's
 * node j at `3 (K (2k + ℓ) + j)`, rising with j, and distinct by band, slot and level.
 */
const MODULE_NODES = Float32Array.from({ length: CLASS_NODES_BYTES / 4 }, (_, n) => {
  const band = n % 3;
  const node = Math.floor(n / 3) % MATURITY_NODES;
  const level = Math.floor(n / (3 * MATURITY_NODES)) % 2;
  const slot = Math.floor(n / (6 * MATURITY_NODES));
  return 0.05 + 0.01 * node + 0.002 * band + 0.1 * slot + 0.03 * level;
});

/** Slots 0 and 2 named, a limb-darkening slot 2; slot 1 empty. */
const MATURITY_PHOTOMETRY = photometryOf([
  [0.1, 0.12, 0.14, 1, 1],
  [0, 0, 0, 0, 0],
  [0.8, 0.85, 0.9, 2, 0.6],
]);

/** Vertex (x, y)'s albedo at `level` (0 n, 1 n − 1) over packed nodes, its own code's. */
function vertexAlbedo(
  packed: Float32Array,
  slot: number,
  level: 0 | 1,
  x: number,
  y: number,
): [number, number, number] {
  const code = CODES[2 * (SIDE * y + x) + level] ?? Number.NaN;
  const first = 3 * MATURITY_NODES * (2 * slot + level);
  return maturityNodeAlbedo(packed, first, maturitySegment(maturityCodeRatio(code)));
}

/** The largest difference between two colours' channels. */
function largestDifference(a: ReadonlyArray<number>, b: ReadonlyArray<number>): number {
  return Math.max(...a.map((value, c) => Math.abs(value - (b[c] ?? Number.NaN))));
}

/** A phase table's rows of their own, row 0 a Lambert law. */
function freshRows(): TerrainPhaseRows {
  return new TerrainPhaseRows(classRowLaw(3, 0));
}

/** Each slot's "drawn from its nodes" flag in packed class laws, word 5 of its 8. */
function nodeFlags(laws: Float32Array): number[] {
  return Array.from({ length: TERRAIN_CLASSES }, (_, k) => laws[8 * k + 5] ?? Number.NaN);
}

/** The calls of WGSL function `name` in `source`, its definition aside. */
function wgslCalls(source: string, name: string): number {
  return source.split(`${name}(`).length - 2;
}

describe("a class's albedo drawn from its maturity nodes (R10.T1.c's client half)", () => {
  const cOverA = 0.9;
  const packed = packClassNodes(MODULE_NODES, MATURITY_PHOTOMETRY, cOverA);

  it("carries the maturity layer's words after the laws: a word of codes a vertex, then the nodes", () => {
    expect([CLASS_MATURITY_BYTES, CLASS_NODES_BYTES]).toEqual([
      SIDE * SIDE * 4,
      8 * 2 * MATURITY_NODES * 3 * 4,
    ]);
    expect(CLASS_WEIGHTS_BYTES + CLASS_LAWS_BYTES + CLASS_MATURITY_BYTES + CLASS_NODES_BYTES).toBe(
      CLASS_LAYERS_BYTES,
    );
    expect(terrainLitWgsl).toContain(
      `const CLASS_MATURITY_WORD : u32 = ${String((CLASS_WEIGHTS_BYTES + CLASS_LAWS_BYTES) / 4)}u;`,
    );
    expect(terrainLitWgsl).toContain(
      `const CLASS_NODE_WORD : u32 = ${String(
        (CLASS_WEIGHTS_BYTES + CLASS_LAWS_BYTES + CLASS_MATURITY_BYTES) / 4,
      )}u;`,
    );
    for (const read of [
      // Both codes of a vertex from its word, n's in the low half (`unpack2x16unorm`'s x).
      "unpack2x16unorm(class_layers[slot * CLASS_LAYER_WORDS + CLASS_MATURITY_WORD + y * VERTICES_PER_SIDE + x])",
      "bitcast<f32>(class_layers[slot * CLASS_LAYER_WORDS + CLASS_LAW_WORD + 8u * k + 5u]) > 0.5",
      "slot * CLASS_LAYER_WORDS + CLASS_NODE_WORD + 3u * (MATURITY_NODES * (2u * k + level) + j)",
    ]) {
      expect(terrainLitWgsl).toContain(read);
    }
  });

  it("composes the nodes' library before terrainLit.wgsl in every terrain module", () => {
    const specs = [
      ...TERRAIN_LIT_MATERIALS,
      ...TERRAIN_DEPTH_MATERIALS,
      ...TERRAIN_SHADOW_MATERIALS,
    ];
    const misplaced = specs.filter(({ vertexWgsl }) => {
      const library = vertexWgsl.indexOf(maturityNodesWgsl);
      return !(library >= 0 && library < vertexWgsl.indexOf(terrainLitWgsl));
    });
    expect(misplaced.map(({ name }) => name)).toEqual([]);
  });

  it("flags each named slot drawn from its nodes in word 5 of its law, and packs as before without a layer", () => {
    const without = packClassLaws(MATURITY_PHOTOMETRY, freshRows(), cOverA);
    const flagged = packClassLaws(MATURITY_PHOTOMETRY, freshRows(), cOverA, true);
    expect([nodeFlags(without), nodeFlags(flagged)]).toEqual([
      [0, 0, 0, 0, 0, 0, 0, 0],
      [1, 0, 1, 0, 0, 0, 0, 0],
    ]);
    const unflagged = flagged.slice();
    for (let k = 0; k < TERRAIN_CLASSES; k += 1) {
      unflagged[8 * k + 5] = 0;
    }
    expect(unflagged).toEqual(without);
    expect(packClassLaws(MATURITY_PHOTOMETRY, freshRows(), cOverA, false)).toEqual(without);
  });

  it("packs the nodes as r, g, b, each scaled for the figure as its law's A is, zeros for a slot with none", () => {
    expect(packed).toHaveLength(CLASS_NODES_BYTES / 4);
    const slotValues = 6 * MATURITY_NODES;
    const factors = [oblateAlbedoScale(1, cOverA), 0, oblateAlbedoScale(Math.fround(0.6), cOverA)];
    expect(factors[2]).not.toBe(1);
    for (let k = 0; k < TERRAIN_CLASSES; k += 1) {
      const factor = factors[k] ?? 0;
      const expected = Array.from({ length: slotValues }, (_, n) => {
        const node = slotValues * k + n - (n % 3);
        return Math.fround((MODULE_NODES[node + 2 - (n % 3)] ?? Number.NaN) * factor);
      });
      expect([...packed.subarray(slotValues * k, slotValues * (k + 1))]).toEqual(expected);
    }
    expect(() => packClassNodes(new Float32Array(10), MATURITY_PHOTOMETRY, cOverA)).toThrow(
      RangeError,
    );
  });

  it("packs nodes that repeat the photometry's albedo as its law's packed A, to the bit", () => {
    const repeating = Float32Array.from({ length: CLASS_NODES_BYTES / 4 }, (_, n) => {
      const slot = Math.floor(n / (6 * MATURITY_NODES));
      return MATURITY_PHOTOMETRY[5 * slot + (n % 3)] ?? Number.NaN;
    });
    const nodes = packClassNodes(repeating, MATURITY_PHOTOMETRY, cOverA);
    const laws = packClassLaws(
      MATURITY_PHOTOMETRY,
      new TerrainPhaseRows(classRowLaw(3, 0)),
      cOverA,
    );
    for (const k of [0, 2]) {
      const a = [...laws.subarray(8 * k, 8 * k + 3)];
      for (let at = 6 * MATURITY_NODES * k; at < 6 * MATURITY_NODES * (k + 1); at += 3) {
        expect([...nodes.subarray(at, at + 3)]).toEqual(a);
      }
    }
  });

  it("draws a vertex's own albedo at its code at n where the morph is 0", () => {
    for (const [x, y] of [
      [0, 0],
      [3, 4],
      [63, 17],
      [64, 64],
    ] as const) {
      for (const slot of [0, 2]) {
        expect(texelClassAlbedo(CODES, packed, slot, x / 64, y / 64, 0)).toEqual(
          vertexAlbedo(packed, slot, 0, x, y),
        );
      }
    }
    // Below 0 it is the patch's own field as well.
    expect(texelClassAlbedo(CODES, packed, 0, 0.5, 0.5, -0.25)).toEqual(
      vertexAlbedo(packed, 0, 0, 32, 32),
    );
  });

  it("draws the parent's field at a morph of 1, bilinear on its even vertices, at every child vertex", () => {
    const parentAt = (x: number, y: number): [number, number, number] => {
      // The even vertices about (x, y), each corner its own code at n − 1, and their mean.
      const xs = x % 2 === 0 ? [x] : [x - 1, x + 1];
      const ys = y % 2 === 0 ? [y] : [y - 1, y + 1];
      const corners = ys.flatMap((cy) => xs.map((cx) => vertexAlbedo(packed, 2, 1, cx, cy)));
      const mean = (c: 0 | 1 | 2): number =>
        corners.reduce((sum, corner) => sum + corner[c], 0) / corners.length;
      return [mean(0), mean(1), mean(2)];
    };
    let worst = 0;
    for (let y = 0; y < SIDE; y += 3) {
      for (let x = 0; x < SIDE; x += 1) {
        const drawn = texelClassAlbedo(CODES, packed, 2, x / 64, y / 64, 1);
        worst = Math.max(worst, largestDifference(drawn, parentAt(x, y)));
      }
    }
    expect(worst).toBeLessThan(1e-15);
    // Past 1 it is the parent's field as well.
    expect(texelClassAlbedo(CODES, packed, 2, 0.3, 0.7, 1.5)).toEqual(
      texelClassAlbedo(CODES, packed, 2, 0.3, 0.7, 1),
    );
  });

  it("mixes the two levels' fields as (1 − μ) T_n + μ T_(n−1) between the ends", () => {
    const [u, v] = [0.371, 0.629];
    const own = texelClassAlbedo(CODES, packed, 0, u, v, 0);
    const parent = texelClassAlbedo(CODES, packed, 0, u, v, 1);
    for (const morph of [0.25, 0.5, 0.9]) {
      expect(texelClassAlbedo(CODES, packed, 0, u, v, morph)).toEqual(
        own.map((value, c) => (1 - morph) * value + morph * (parent[c] ?? Number.NaN)),
      );
    }
  });

  it("draws uniform codes as the nodes there, wherever the texel and whatever the morph", () => {
    const code = 20_000;
    const uniform = new Uint16Array(2 * SIDE * SIDE).fill(code);
    const segment = maturitySegment(maturityCodeRatio(code));
    const levelA = (level: 0 | 1): [number, number, number] =>
      maturityNodeAlbedo(packed, 3 * MATURITY_NODES * (2 * 2 + level), segment);
    for (const [u, v] of [
      [0, 0],
      [0.123, 0.987],
      [1, 0.5],
    ] as const) {
      expect(
        largestDifference(texelClassAlbedo(uniform, packed, 2, u, v, 0), levelA(0)),
      ).toBeLessThan(1e-15);
      expect(
        largestDifference(texelClassAlbedo(uniform, packed, 2, u, v, 1), levelA(1)),
      ).toBeLessThan(1e-15);
    }
  });

  it("refuses codes too short for the texel's quad", () => {
    expect(() => texelClassAlbedo(new Uint16Array(8), packed, 0, 0.5, 0.5, 0)).toThrow(RangeError);
  });

  it("replaces each named slot's A alone where a texel has a maturity layer, as the twin draws it", () => {
    const elsewhere: PhotometricLaw = { ...classRowLaw(1, 1), a: [0.11, 0.12, 0.13] };
    const maturity = { codes: CODES, nodes: MODULE_NODES };
    const at = { maturity, u: 0.4, v: 0.6, morph: 0.35 };
    const plain = terrainTexelLaws(MATURITY_PHOTOMETRY, elsewhere, cOverA);
    const drawn = terrainTexelLaws(MATURITY_PHOTOMETRY, elsewhere, cOverA, at);
    expect([drawn[0], drawn[2], drawn.slice(4)]).toEqual([plain[0], null, plain.slice(4)]);
    for (const slot of [0, 2]) {
      expect(drawn[slot + 1]).toEqual({
        a: texelClassAlbedo(CODES, packed, slot, at.u, at.v, at.morph),
        table: plain[slot + 1]?.table,
      });
    }
    // Without a layer each slot's A is its law's, scaled for the figure.
    const law = classPhotometryOf(MATURITY_PHOTOMETRY, 2);
    expect(plain[3]?.a).toEqual(law === null ? null : classTexelLaw(law, cOverA).a);
  });
});

describe("the lit terrain's term of a class drawn from its maturity nodes (R10.T1.c's client half)", () => {
  it("draws μ ≤ 0 from the patch's quad alone and μ ≥ 1 from the parent's, mixing between", () => {
    const albedo = wgslFunction(terrainLitWgsl, "terrain_class_albedo");
    for (const line of [
      "if (!(morph >= 1.0)) {\n    own = maturity_quad_albedo(slot, k, 0u, maturity.own);",
      "if (morph > 0.0) {\n    parent = maturity_quad_albedo(slot, k, 1u, maturity.parent);",
      "if (!(morph > 0.0)) {\n    return own;\n  }",
      "if (morph >= 1.0) {\n    return parent;\n  }",
      "return (1.0 - morph) * own + morph * parent;",
    ]) {
      expect(albedo).toContain(line);
    }
  });

  it("takes the patch's quad as the weights' and the parent's on the even vertices, as the twin does", () => {
    const quads = wgslFunction(terrainLitWgsl, "texel_maturity");
    for (const line of [
      "let base = min(vec2u(floor(max(s, vec2f(0.0)))), vec2u(63u));",
      "let f = clamp(s - vec2f(base), vec2f(0.0), vec2f(1.0));",
      "out.own = maturity_quad(slot, base, 1u, 0u, f);",
      "let base = min(2u * vec2u(floor(max(s, vec2f(0.0)) / 2.0)), vec2u(62u));",
      "let f = clamp((s - vec2f(base)) / 2.0, vec2f(0.0), vec2f(1.0));",
      "out.parent = maturity_quad(slot, base, 2u, 1u, f);",
    ]) {
      expect(quads).toContain(line);
    }
    expect(wgslFunction(terrainLitWgsl, "maturity_quad_albedo")).toContain(
      "return mix(mix(corners[0], corners[1], quad.at.x), mix(corners[2], corners[3], quad.at.x), quad.at.y);",
    );
  });

  it("skips a slot of no share before its flag or nodes are read, and gathers the texel's segments once", () => {
    const albedos = wgslFunction(terrainLitWgsl, "terrain_albedos");
    const skip = albedos.indexOf("if (!(shares[k + 1u] > 0.0)) {\n      continue;\n    }");
    expect(skip).toBeGreaterThan(0);
    expect(skip).toBeLessThan(albedos.indexOf("class_drawn_from_nodes(in.slot, k)"));
    expect(albedos).toContain("albedos[0] = terrain_row(1u).xyz;");
    expect(albedos).toContain("albedos[k + 1u] = class_law(in.slot, k).law.a;");
    // One call each in the source, so that a compiler inlining every call takes one copy.
    expect([
      wgslCalls(terrainLitWgsl, "texel_maturity"),
      wgslCalls(terrainLitWgsl, "terrain_class_albedo"),
      wgslCalls(terrainLitWgsl, "maturity_quad"),
      wgslCalls(terrainLitWgsl, "maturity_quad_albedo"),
    ]).toEqual([1, 1, 2, 2]);
  });

  it("weighs each law's A from the texel's albedos in the reflectance and the sky light's mean", () => {
    expect(wgslFunction(terrainLitWgsl, "terrain_reflectance")).toContain(
      "reflectance = reflectance + share * albedos[m] * row.rgb * disc_term;",
    );
    const shade = wgslFunction(terrainLitWgsl, "shade_terrain");
    expect(shade).toContain("let albedos = terrain_albedos(in, shares);");
    expect(shade).toContain("mean_a = mean_a + shares[m] * albedos[m];");
    expect(shade).not.toContain("terrain_law(in.slot, m).a");
  });
});

/** decision-r10-limb's particulate rule L_p(x) (`material/properties.rs`' `limb_parameter`). */
function particulateLimb(x: number): number {
  const onset = 0.6;
  const enceladus = 1.35;
  const limb = 0.38;
  if (!(x > onset)) {
    return 1;
  }
  if (x >= enceladus) {
    return limb;
  }
  return (limb * enceladus * (x - onset) + onset * (enceladus - x)) / (x * (enceladus - onset));
}

/** Eight-point Gauss–Legendre on [0, 1]. */
const GL_NODES = [
  0.019855071751231856, 0.10166676129318664, 0.2372337950418355, 0.4082826787521751,
  0.5917173212478249, 0.7627662049581645, 0.8983332387068134, 0.9801449282487681,
];
const GL_WEIGHTS = [
  0.05061426814518813, 0.11119051722668724, 0.15685332293894363, 0.181341891689181,
  0.181341891689181, 0.15685332293894363, 0.11119051722668724, 0.05061426814518813,
];

/** The emission cosines' subintervals: [0, 10⁻⁶], then 40 geometric steps to 1. */
const MU_EDGES = [0, ...Array.from({ length: 41 }, (_, k) => 10 ** (-6 + (6 * k) / 40))];

/** Midpoints in the relative azimuth over [0, π], doubled for its other half. */
const AZIMUTHS = 180;

/**
 * η(μ₀) = A_h ÷ A_N of a row: (1 ÷ π μ₀) ∫ (I/F at A = 1) μ dΩ over the hemisphere, the I/F the
 * twin's (`brdfFromTable`, the row's f and L(α)), cos α = μ₀ μ + sin i sin e cos φ.
 */
function eta(table: PhaseFactorTable, mu0: number): number {
  const sinI = Math.sqrt(1 - mu0 * mu0);
  const dPhi = Math.PI / AZIMUTHS;
  const one = { a: [1, 1, 1] as const };
  let sum = 0;
  for (let s = 0; s + 1 < MU_EDGES.length; s += 1) {
    const lo = MU_EDGES[s] ?? 0;
    const hi = MU_EDGES[s + 1] ?? 1;
    for (let g = 0; g < GL_NODES.length; g += 1) {
      const mu = lo + (hi - lo) * (GL_NODES[g] ?? 0);
      const w = (hi - lo) * (GL_WEIGHTS[g] ?? 0);
      const sinE = Math.sqrt(Math.max(0, 1 - mu * mu));
      let ring = 0;
      for (let p = 0; p < AZIMUTHS; p += 1) {
        const cosAlpha = mu0 * mu + sinI * sinE * Math.cos((p + 0.5) * dPhi);
        const alpha = Math.acos(Math.min(1, Math.max(-1, cosAlpha)));
        ring += brdfFromTable(one, table, mu0, mu, alpha)[1];
      }
      sum += w * mu * ring * 2 * dPhi;
    }
  }
  return sum / (Math.PI * mu0);
}

/** The incidences of the bound's check (decision-r10-limb §3). */
const MU0S = [1, 0.5, 0.2, 0.1, 0.05, 0.02, 0.01, 0.005];

describe("the rows' energy bound, with their f and L(α) (decision-r10-limb §3)", () => {
  it("keeps every A_h at most 1 on each particulate row, on an A_N grid up to its ceiling of 1.35", () => {
    let worst = { code: 0, normalAlbedo: 0, mu0: 0, ah: 0 };
    for (const code of [1, 2] as const) {
      // L(0) is 1 up to A_N 0.60, where A_h = A_N η rises with A_N: its top, 0.60, stands for the
      // grid below it; above it each A_N has an L(0) of its own.
      for (let k = 12; k <= 27; k += 1) {
        const normalAlbedo = 0.05 * k;
        const table = phaseFactorTableOf(classRowLaw(code, particulateLimb(normalAlbedo)));
        for (const mu0 of MU0S) {
          const ah = normalAlbedo * eta(table, mu0);
          if (ah > worst.ah) {
            worst = { code, normalAlbedo, mu0, ah };
          }
        }
      }
    }
    // The Ice row at its ceiling, 1.35 × 0.720 = 0.97 by the ruling's figures.
    expect(worst).toMatchObject({ code: 2, normalAlbedo: expect.closeTo(1.35, 9) });
    expect(worst.ah).toBeLessThanOrEqual(1);
    expect(worst.ah).toBeCloseTo(0.97, 1);
  });

  it("gives the Ice row at L(0) 0.38 its sup η of 0.720 and its η(1) of 0.636, to 0.01", () => {
    const table = phaseFactorTableOf(classRowLaw(2, 0.38));
    const etas = MU0S.map((mu0) => eta(table, mu0));
    expect(Math.abs((etas[0] ?? 0) - 0.636)).toBeLessThan(0.01);
    expect(Math.abs(Math.max(...etas) - 0.72)).toBeLessThan(0.01);
  });

  it("holds the Lambert row's η at 1 at every incidence, to 10⁻⁶", () => {
    const table = phaseFactorTableOf(classRowLaw(3, 0));
    for (const mu0 of MU0S) {
      expect(Math.abs(eta(table, mu0) - 1)).toBeLessThan(1e-6);
    }
  });
});
