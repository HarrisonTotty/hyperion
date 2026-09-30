import { Constants } from "@babylonjs/core/Engines/constants";
import { NullEngine } from "@babylonjs/core/Engines/nullEngine.pure";
import { ShaderMaterial } from "@babylonjs/core/Materials/shaderMaterial.pure";
import { Scene } from "@babylonjs/core/scene.pure";
import { afterEach, describe, expect, it } from "vitest";

import type { WgslMaterialSpec } from "../types";
import {
  applyMaterialState,
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
