/**
 * The WGSL-only guard: GLSL never compiles, and never makes Babylon fetch a compiler.
 *
 * @remarks
 * GLSL reaches Babylon's WebGPU engine at `_preparePipelineContextAsync`, which fetches glslang and
 * twgsl from `cdn.babylonjs.com` when first needed; the compile is handed only the source, with no
 * name; and the effect's preparation is not awaited, so a throw there is an unhandled rejection
 * that Babylon's error observables never see (R01 Design note 12, researched in 9.28.0). So the
 * guard has three layers: (a) `initAsync` is given a glslang and a twgsl whose compile methods
 * throw, so no fetch can happen; (b) `createEffect` is wrapped, and throws {@link GlslShaderRefused}
 * naming an effect whose language is GLSL; (c) a `window` `unhandledrejection` listener reports
 * what the first two miss. Each reports to `console.error` and to the status store.
 */

import type { GlslangOptions } from "@babylonjs/core/Engines/webgpuEngine.pure";
import type { TwgslOptions } from "@babylonjs/core/Engines/WebGPU/webgpuTintWASM";
import type { IShaderPath } from "@babylonjs/core/Materials/effect.pure";
import { ShaderLanguage } from "@babylonjs/core/Materials/shaderLanguage";

import type { GraphicsStatusStore } from "../status";

/** The rule every refusal names. */
const RULE = "every shader is WGSL (R01 Design note 12)";

/** Thrown by the WGSL-only guard for a GLSL effect, naming it (R01 Design note 12). */
export class GlslShaderRefused extends Error {
  readonly effectName: string;

  constructor(effectName: string) {
    super(`GLSL effect ${effectName} is refused: ${RULE}`);
    this.name = "GlslShaderRefused";
    this.effectName = effectName;
  }
}

/** The error a stub compiler throws: the source reached it unnamed. */
function stubRefusal(): GlslShaderRefused {
  return new GlslShaderRefused("(unnamed, at the compiler)");
}

/**
 * A glslang whose compile throws, handed to `initAsync` so that Babylon never fetches the real one.
 *
 * @remarks
 * Babylon calls `.then` on what `glslangOptions.glslang` holds, so it is a promise.
 */
export const GLSLANG_STUB: GlslangOptions = {
  glslang: Promise.resolve({
    compileGLSL(): never {
      throw stubRefusal();
    },
  }),
};

/** A twgsl whose conversion throws, handed to `initAsync` for the same reason. */
export const TWGSL_STUB: TwgslOptions = {
  twgsl: {
    convertSpirV2WGSL(): never {
      throw stubRefusal();
    },
  },
};

/** A readable name for an effect's `baseName`, which is a string or a shader path. */
export function effectNameOf(name: IShaderPath | string): string {
  if (typeof name === "string") {
    return name;
  }
  const named = [name.spectorName, name.vertex, name.fragment, name.vertexElement];
  return named.find((part): part is string => typeof part === "string") ?? "(inline source)";
}

/** What of an effect the guard reads. */
export interface GuardedEffect {
  readonly name: IShaderPath | string;
  readonly shaderLanguage: ShaderLanguage;
}

/** What of an engine the guard wraps: its `createEffect`, whatever its arguments. */
export interface EffectFactory {
  createEffect(...args: never[]): GuardedEffect;
}

/** Reports a refusal to the console and the status store. */
export function reportRefusal(status: GraphicsStatusStore, refusal: GlslShaderRefused): void {
  console.error(refusal.message);
  status.dispatch({ kind: "shader-refused", effectName: refusal.effectName });
}

/**
 * Wraps `engine.createEffect` so that a GLSL effect throws, naming itself.
 *
 * @remarks
 * `ShaderMaterial` defaults to GLSL, so every material the adapter makes passes WGSL; the wrapper
 * catches any other path, the engine's own effects included.
 *
 * @throws {@link GlslShaderRefused} from the wrapped `createEffect`, naming a GLSL effect, after
 * reporting it.
 */
export function guardCreateEffect(engine: EffectFactory, status: GraphicsStatusStore): void {
  const createEffect = engine.createEffect.bind(engine);
  engine.createEffect = (...args: never[]): GuardedEffect => {
    const effect = createEffect(...args);
    if (effect.shaderLanguage === ShaderLanguage.GLSL) {
      const refusal = new GlslShaderRefused(effectNameOf(effect.name));
      reportRefusal(status, refusal);
      throw refusal;
    }
    return effect;
  };
}

/**
 * Listens for a {@link GlslShaderRefused} no caller caught, which Babylon's unawaited preparation
 * of an effect leaves as an unhandled rejection.
 *
 * @returns The listener's removal.
 */
export function listenForUnhandledRefusals(
  target: EventTarget,
  status: GraphicsStatusStore,
): () => void {
  const listener = (event: Event): void => {
    const reason: unknown = "reason" in event ? event.reason : undefined;
    if (reason instanceof GlslShaderRefused) {
      // Reported here, so Chromium need not log it again as uncaught.
      event.preventDefault();
      reportRefusal(status, reason);
    }
  };
  target.addEventListener("unhandledrejection", listener);
  return () => {
    target.removeEventListener("unhandledrejection", listener);
  };
}
