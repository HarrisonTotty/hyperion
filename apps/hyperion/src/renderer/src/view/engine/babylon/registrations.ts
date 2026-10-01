/**
 * Babylon's side-effect registrations, made explicitly and only for what the adapter uses.
 *
 * @remarks
 * `Engines/webgpuEngine.js` registers everything a WebGPU engine may use, the audio engine and the
 * texture loaders included, so the adapter imports the `.pure` modules and calls their register
 * functions itself (R01 Design note 14). Shader sources are registered in the shader store by the
 * modules that define them, which the adapter imports by name and checks, so that no import is left
 * for its side effect alone. The list is what the adapter's own creation paths reach in 9.28.0; an
 * upgrade that moves a feature behind another registration shows in `just test-render`.
 */

import { RegisterBufferAlign } from "@babylonjs/core/Buffers/buffer.align.pure";
import { RegisterAbstractEngineDom } from "@babylonjs/core/Engines/AbstractEngine/abstractEngine.dom.pure";
import { RegisterAbstractEngineRenderPass } from "@babylonjs/core/Engines/AbstractEngine/abstractEngine.renderPass.pure";
import { RegisterAbstractEngineStates } from "@babylonjs/core/Engines/AbstractEngine/abstractEngine.states.pure";
import { RegisterAbstractEngineStencil } from "@babylonjs/core/Engines/AbstractEngine/abstractEngine.stencil.pure";
import { RegisterAbstractEngineTexture } from "@babylonjs/core/Engines/AbstractEngine/abstractEngine.texture.pure";
import { ShaderStore } from "@babylonjs/core/Engines/shaderStore";
import { RegisterEnginesWebGPUExtensionsEngineAlpha } from "@babylonjs/core/Engines/WebGPU/Extensions/engine.alpha.pure";
import { RegisterEnginesWebGPUExtensionsEngineCubeTexture } from "@babylonjs/core/Engines/WebGPU/Extensions/engine.cubeTexture.pure";
import { RegisterEnginesWebGPUExtensionsEngineRawTexture } from "@babylonjs/core/Engines/WebGPU/Extensions/engine.rawTexture.pure";
import { RegisterEnginesWebGPUExtensionsEngineReadTexture } from "@babylonjs/core/Engines/WebGPU/Extensions/engine.readTexture.pure";
import { RegisterEnginesWebGPUExtensionsEngineRenderTarget } from "@babylonjs/core/Engines/WebGPU/Extensions/engine.renderTarget.pure";
import { RegisterEnginesWebGPUExtensionsEngineRenderTargetTexture } from "@babylonjs/core/Engines/WebGPU/Extensions/engine.renderTargetTexture.pure";
import { clearQuadPixelShaderWGSL } from "@babylonjs/core/ShadersWGSL/clearQuad.fragment";
import { clearQuadVertexShaderWGSL } from "@babylonjs/core/ShadersWGSL/clearQuad.vertex";
import { postprocessVertexShaderWGSL } from "@babylonjs/core/ShadersWGSL/postprocess.vertex";

/** The WGSL shaders Babylon itself compiles for the adapter: its clears and post-process quads. */
const ENGINE_SHADERS: ReadonlyArray<{ readonly name: string; readonly shader: string }> = [
  clearQuadVertexShaderWGSL,
  clearQuadPixelShaderWGSL,
  postprocessVertexShaderWGSL,
];

let registered = false;

/**
 * Registers the engine extensions and shaders the adapter uses, once per page.
 *
 * @remarks
 * Called before the first engine is made. Every register function is idempotent in Babylon, and
 * the flag keeps a rebuild from repeating them.
 */
export function registerBabylonModules(): void {
  if (registered) {
    return;
  }
  RegisterBufferAlign();
  RegisterAbstractEngineDom();
  RegisterAbstractEngineRenderPass();
  RegisterAbstractEngineStates();
  RegisterAbstractEngineStencil();
  RegisterAbstractEngineTexture();
  RegisterEnginesWebGPUExtensionsEngineAlpha();
  RegisterEnginesWebGPUExtensionsEngineCubeTexture();
  RegisterEnginesWebGPUExtensionsEngineRawTexture();
  RegisterEnginesWebGPUExtensionsEngineReadTexture();
  RegisterEnginesWebGPUExtensionsEngineRenderTarget();
  RegisterEnginesWebGPUExtensionsEngineRenderTargetTexture();
  for (const { name, shader } of ENGINE_SHADERS) {
    ShaderStore.ShadersStoreWGSL[name] ??= shader;
  }
  registered = true;
}
