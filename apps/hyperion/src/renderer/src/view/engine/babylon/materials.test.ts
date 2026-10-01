import { Constants } from "@babylonjs/core/Engines/constants";
import { NullEngine } from "@babylonjs/core/Engines/nullEngine.pure";
import { ShaderMaterial } from "@babylonjs/core/Materials/shaderMaterial.pure";
import { Scene } from "@babylonjs/core/scene.pure";
import { afterEach, describe, expect, it } from "vitest";

import type { WgslMaterialSpec } from "../types";
import {
  applyMaterialState,
  BLEND_STATES,
  declaredAttributes,
  declaredTextures,
  materialState,
} from "./materials";

const SPEC: WgslMaterialSpec = {
  name: "hull",
  vertexWgsl: "attribute position : vec3f;\n// attribute ghost : vec2f;\nattribute normal:vec3f;",
  fragmentWgsl: "var depthTexture : texture_depth_2d;\nvar lut: texture_3d<f32>;",
  uniforms: [],
  samplers: [],
  transparent: false,
  cullMode: "back",
  depthWrite: true,
  colourWrites: true,
  blend: "none",
};

describe("a material's state", () => {
  let scene: Scene | null = null;

  afterEach(() => {
    scene?.getEngine().dispose();
    scene = null;
  });

  it("sets each flag on Babylon's own property", () => {
    scene = new Scene(new NullEngine());
    const material = new ShaderMaterial("flags", scene, { vertexSource: "", fragmentSource: "" });
    applyMaterialState(
      material,
      materialState({ ...SPEC, depthWrite: false, colourWrites: false, blend: "additive" }),
    );
    expect(material.disableDepthWrite).toBe(true);
    expect(material.disableColorWrite).toBe(true);
    expect(material.alphaMode).toBe(Constants.ALPHA_ADD);
    expect(material.backFaceCulling).toBe(true);

    applyMaterialState(material, materialState({ ...SPEC, cullMode: "none" }));
    expect(material.disableDepthWrite).toBe(false);
    expect(material.disableColorWrite).toBe(false);
    expect(material.alphaMode).toBe(Constants.ALPHA_DISABLE);
    expect(material.backFaceCulling).toBe(false);
  });

  it("hands depthBiasAway to Babylon as the same positive values, unflipped", () => {
    scene = new Scene(new NullEngine());
    const material = new ShaderMaterial("bias", scene, { vertexSource: "", fragmentSource: "" });
    applyMaterialState(
      material,
      materialState({ ...SPEC, depthBiasAway: { constant: 4, slopeScale: 1.5 } }),
    );
    expect(material.zOffsetUnits).toBe(4);
    expect(material.zOffset).toBe(1.5);
  });

  it("winds front faces counter-clockwise", () => {
    scene = new Scene(new NullEngine());
    const material = new ShaderMaterial("winding", scene, { vertexSource: "", fragmentSource: "" });
    applyMaterialState(material, materialState(SPEC));
    expect(material.sideOrientation).toBe(Constants.MATERIAL_CounterClockWiseSideOrientation);
  });

  it("puts a blending material in the transparent queue", () => {
    expect(materialState({ ...SPEC, blend: "additive" }).needAlphaBlending).toBe(true);
    expect(materialState({ ...SPEC, transparent: true }).needAlphaBlending).toBe(true);
    expect(materialState(SPEC).needAlphaBlending).toBe(false);
  });
});

describe("a material's declarations", () => {
  it("give the attributes and textures a source declares, comments aside", () => {
    expect(declaredAttributes(SPEC.vertexWgsl)).toEqual(["position", "normal"]);
    expect(declaredTextures(SPEC.fragmentWgsl)).toEqual(["depthTexture", "lut"]);
  });
});

describe("the blend table", () => {
  it("keeps the destination alpha in every blending mode: (zero, one)", () => {
    for (const mode of ["additive", "premultiplied"] as const) {
      expect(BLEND_STATES[mode]?.alpha).toEqual({
        srcFactor: "zero",
        dstFactor: "one",
        operation: "add",
      });
    }
    expect(BLEND_STATES.none).toBeUndefined();
  });

  it("adds source-alpha-weighted colour, or composites premultiplied colour", () => {
    expect(BLEND_STATES.additive?.color).toEqual({
      srcFactor: "src-alpha",
      dstFactor: "one",
      operation: "add",
    });
    expect(BLEND_STATES.premultiplied?.color).toEqual({
      srcFactor: "one",
      dstFactor: "one-minus-src-alpha",
      operation: "add",
    });
  });
});
