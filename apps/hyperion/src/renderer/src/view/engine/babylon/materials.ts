/**
 * WGSL materials over Babylon's `ShaderMaterial`, and their per-draw bindings.
 *
 * @remarks
 * Every material is WGSL in Babylon's dialect (R01 Design note 12): Babylon collects its
 * `attribute`, `varying` and `uniform` declarations and gives every texture, sampler and storage
 * buffer its group and binding, so the specification's sources declare them without either. The
 * attributes and textures a material binds are read from those declarations, since Babylon's effect
 * needs their names up front. Material state goes through Babylon's public properties (Design note
 * 21): `depthWrite` and `colourWrites` through `disableDepthWrite` and `disableColorWrite`, the blend
 * through `alphaMode`, the cull mode through `backFaceCulling`, and `depthBiasAway` through
 * `zOffset` and `zOffsetUnits`, unflipped, since Babylon negates both itself under reversed depth
 * (Design note 18).
 */

import { Constants } from "@babylonjs/core/Engines/constants";
import type { Effect } from "@babylonjs/core/Materials/effect.pure";
import type { Material } from "@babylonjs/core/Materials/material.pure";
import { ShaderLanguage } from "@babylonjs/core/Materials/shaderLanguage";
import { ShaderMaterial } from "@babylonjs/core/Materials/shaderMaterial.pure";
import { TextureSampler } from "@babylonjs/core/Materials/Textures/textureSampler";
import { Matrix } from "@babylonjs/core/Maths/math.vector.pure";
import type { Scene } from "@babylonjs/core/scene.pure";

import {
  FRAME_UNIFORMS,
  type SamplerSpec,
  type UniformSpec,
  type WgslMaterialSpec,
} from "../types";

/** The Babylon state a material's specification sets, by Babylon's own property names. */
export interface MaterialState {
  readonly disableDepthWrite: boolean;
  readonly disableColorWrite: boolean;
  readonly alphaMode: number;
  readonly backFaceCulling: boolean;
  /** Babylon's slope-scale bias, positive away from the camera before Babylon's own flip. */
  readonly zOffset: number;
  /** Babylon's constant bias, likewise. */
  readonly zOffsetUnits: number;
  /** Whether the draw goes in Babylon's transparent queue, where alone it blends. */
  readonly needAlphaBlending: boolean;
}

/** Babylon's alpha mode for each blend (Design note 21). */
const ALPHA_MODES: Readonly<Record<WgslMaterialSpec["blend"], number>> = {
  none: Constants.ALPHA_DISABLE,
  additive: Constants.ALPHA_ADD,
  premultiplied: Constants.ALPHA_PREMULTIPLIED,
};

/**
 * The Babylon state of a material.
 *
 * @remarks
 * `depthBiasAway` is handed over as the same positive values: Babylon negates `zOffset` and
 * `zOffsetUnits` itself when `useReverseDepthBuffer` is set (`abstractEngine.pure.js` lines 523,
 * 531, 538 and 546 in 9.28.0), so positive always moves a fragment away (Design note 18). Babylon
 * blends only in its transparent queue, so a blending material goes there whatever `transparent`
 * says.
 */
export function materialState(spec: WgslMaterialSpec): MaterialState {
  return {
    disableDepthWrite: !spec.depthWrite,
    disableColorWrite: !spec.colourWrites,
    alphaMode: ALPHA_MODES[spec.blend],
    backFaceCulling: spec.cullMode === "back",
    zOffset: spec.depthBiasAway?.slopeScale ?? 0,
    zOffsetUnits: spec.depthBiasAway?.constant ?? 0,
    needAlphaBlending: spec.transparent || spec.blend !== "none",
  };
}

/** Sets `state` on a Babylon material. */
export function applyMaterialState(material: Material, state: MaterialState): void {
  material.disableDepthWrite = state.disableDepthWrite;
  material.disableColorWrite = state.disableColorWrite;
  material.alphaMode = state.alphaMode;
  material.backFaceCulling = state.backFaceCulling;
  material.cullBackFaces = true;
  // Front faces wind counter-clockwise in WebGPU's framebuffer, as R02's right-handed matrices
  // expect; left to the mesh, Babylon's default orientation reverses it (checked on SwiftShader).
  material.sideOrientation = Constants.MATERIAL_CounterClockWiseSideOrientation;
  material.zOffset = state.zOffset;
  material.zOffsetUnits = state.zOffsetUnits;
}

/** WGSL without its comments, so that a commented-out declaration is not read. */
function withoutComments(wgsl: string): string {
  return wgsl.replaceAll(/\/\*[\s\S]*?\*\//gu, " ").replaceAll(/\/\/[^\n]*/gu, " ");
}

/** The names of the vertex attributes a vertex source declares, in Babylon's `attribute` form. */
export function declaredAttributes(vertexWgsl: string): ReadonlyArray<string> {
  return [...withoutComments(vertexWgsl).matchAll(/\battribute\s+(\w+)\s*:/gu)].flatMap((match) =>
    match[1] === undefined ? [] : [match[1]],
  );
}

/** The names of the textures a source declares (`var name : texture_…`). */
export function declaredTextures(wgsl: string): ReadonlyArray<string> {
  return [...withoutComments(wgsl).matchAll(/\bvar\s+(\w+)\s*:\s*texture_/gu)].flatMap((match) =>
    match[1] === undefined ? [] : [match[1]],
  );
}

/**
 * The blend state of each mode in a raw pass, colour and alpha factors: every mode keeps the
 * destination's alpha, R07's meter class (Design note 21).
 */
export const BLEND_STATES: Readonly<Record<WgslMaterialSpec["blend"], GPUBlendState | undefined>> =
  {
    none: undefined,
    additive: {
      color: { srcFactor: "src-alpha", dstFactor: "one", operation: "add" },
      alpha: { srcFactor: "zero", dstFactor: "one", operation: "add" },
    },
    premultiplied: {
      color: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
      alpha: { srcFactor: "zero", dstFactor: "one", operation: "add" },
    },
  };

/** The sampler a texture's automatic `<name>Sampler` gets in a raw pass: linear, clamped. */
export const DEFAULT_SAMPLER: SamplerSpec = {
  name: "default",
  filter: "linear",
  address: "clamp-to-edge",
};

/**
 * A Babylon sampler with the specification's filter and address mode.
 *
 * @remarks
 * The filter applies between mips too: Babylon's plain bilinear and nearest modes clamp the level
 * of detail to 0, so a mipped texture would only ever be read at its first level.
 */
export function textureSamplerOf(spec: SamplerSpec): TextureSampler {
  const address =
    spec.address === "repeat"
      ? Constants.TEXTURE_WRAP_ADDRESSMODE
      : Constants.TEXTURE_CLAMP_ADDRESSMODE;
  const filter =
    spec.filter === "linear"
      ? Constants.TEXTURE_LINEAR_LINEAR_MIPLINEAR
      : Constants.TEXTURE_NEAREST_NEAREST_MIPNEAREST;
  return new TextureSampler().setParameters(address, address, address, 1, filter);
}

/**
 * A `ShaderMaterial` for `spec`, in WGSL, with its state and samplers set.
 *
 * @remarks
 * `ShaderMaterial` defaults to GLSL (`effect.pure.js:154`), so the language is always passed
 * (Design note 12). The frame's uniforms are listed so that the adapter can set them whether or not
 * the source declares them.
 */
export function createShaderMaterial(scene: Scene, spec: WgslMaterialSpec): ShaderMaterial {
  const state = materialState(spec);
  const material = new ShaderMaterial(
    spec.name,
    scene,
    { vertexSource: spec.vertexWgsl, fragmentSource: spec.fragmentWgsl },
    {
      attributes: [...declaredAttributes(spec.vertexWgsl)],
      uniforms: [
        FRAME_UNIFORMS.viewRotation,
        FRAME_UNIFORMS.projection,
        FRAME_UNIFORMS.offsetFromCamera,
        ...spec.uniforms.map(({ name }) => name),
      ],
      samplers: [
        ...new Set([...declaredTextures(spec.vertexWgsl), ...declaredTextures(spec.fragmentWgsl)]),
      ],
      storageBuffers: (spec.storageBuffers ?? []).map(({ name }) => name),
      shaderLanguage: ShaderLanguage.WGSL,
      needAlphaBlending: state.needAlphaBlending,
      useClipPlane: false,
    },
  );
  applyMaterialState(material, state);
  for (const sampler of spec.samplers) {
    material.setTextureSampler(sampler.name, textureSamplerOf(sampler));
  }
  return material;
}

/** The components each uniform type takes. */
const COMPONENTS: Readonly<Record<UniformSpec["type"], number>> = {
  f32: 1,
  vec2f: 2,
  vec3f: 3,
  vec4f: 4,
  mat4x4f: 16,
  u32: 1,
};

/** Reused for every matrix a draw sets; `setMatrix` copies it into the uniform buffer. */
const scratchMatrix = new Matrix();

/**
 * Sets one uniform on `effect` from its value.
 *
 * @param value - Its components, column-major for a matrix; a `u32` is read from the first.
 * @throws Error naming the uniform when `value` has too few components for its type.
 */
export function setUniform(effect: Effect, uniform: UniformSpec, value: Float32Array): void {
  const { name, type } = uniform;
  if (value.length < COMPONENTS[type]) {
    throw new Error(`uniform ${name} is a ${type} and was given ${value.length} components`);
  }
  const [x = 0, y = 0, z = 0, w = 0] = value;
  switch (type) {
    case "f32":
      effect.setFloat(name, x);
      break;
    case "vec2f":
      effect.setFloat2(name, x, y);
      break;
    case "vec3f":
      effect.setFloat3(name, x, y, z);
      break;
    case "vec4f":
      effect.setFloat4(name, x, y, z, w);
      break;
    case "mat4x4f":
      Matrix.FromArrayToRef(value, 0, scratchMatrix);
      effect.setMatrix(name, scratchMatrix);
      break;
    case "u32":
      effect.setUInt(name, x);
      break;
  }
}
