import { ShaderLanguage } from "@babylonjs/core/Materials/shaderLanguage";
import { describe, expect, it, vi } from "vitest";

import { GraphicsStatusStore, initialGraphicsStatus } from "../status";
import {
  effectNameOf,
  GlslShaderRefused,
  GLSLANG_STUB,
  type GuardedEffect,
  guardCreateEffect,
  listenForUnhandledRefusals,
  TWGSL_STUB,
} from "./wgslGuard";

function store(): GraphicsStatusStore {
  return new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
}

/** An engine whose `createEffect` hands back an effect in the language it was asked for. */
class FakeEffectEngine {
  createEffect(name: string, shaderLanguage: ShaderLanguage): GuardedEffect {
    return { name, shaderLanguage };
  }
}

/** An `unhandledrejection` event as Chromium raises it, without a DOM. */
function rejection(reason: unknown): Event {
  return Object.assign(new Event("unhandledrejection"), { reason });
}

/** Calls `compiler[method]()`, as Babylon would. */
function compileWith(compiler: unknown, method: string): () => unknown {
  const compile: unknown = Reflect.get(Object(compiler), method);
  if (typeof compile !== "function") {
    throw new Error(`the stub has no ${method}`);
  }
  return () => Reflect.apply(compile, compiler, []);
}

const RULE = /every shader is WGSL \(R01 Design note 12\)/u;

describe("the stub compilers", () => {
  it("glslang refuses GLSL, naming the rule", async () => {
    const glslang: unknown = await GLSLANG_STUB.glslang;
    expect(compileWith(glslang, "compileGLSL")).toThrow(RULE);
  });

  it("twgsl refuses SPIR-V, naming the rule", () => {
    expect(compileWith(TWGSL_STUB.twgsl, "convertSpirV2WGSL")).toThrow(RULE);
  });
});

describe("the createEffect wrapper", () => {
  it("passes a WGSL effect through", () => {
    const engine = new FakeEffectEngine();
    const status = store();
    guardCreateEffect(engine, status);
    expect(engine.createEffect("lines", ShaderLanguage.WGSL)).toEqual({
      name: "lines",
      shaderLanguage: ShaderLanguage.WGSL,
    });
    expect(status.getSnapshot().fault).toBeNull();
  });

  it("throws on a GLSL effect with its name, and reports it", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = new FakeEffectEngine();
    const status = store();
    guardCreateEffect(engine, status);
    expect(() => engine.createEffect("standard", ShaderLanguage.GLSL)).toThrow(
      new GlslShaderRefused("standard"),
    );
    expect(status.getSnapshot().fault).toEqual({ kind: "shader-refused", effectName: "standard" });
    expect(console.error).toHaveBeenCalledOnce();
  });

  it("names an effect given by its shader paths", () => {
    expect(effectNameOf({ vertex: "postprocess", fragment: "bloom" })).toBe("postprocess");
    expect(effectNameOf({ spectorName: "tonemap", vertexSource: "…" })).toBe("tonemap");
    expect(effectNameOf({ vertexSource: "…", fragmentSource: "…" })).toBe("(inline source)");
  });
});

describe("the unhandled-rejection backstop", () => {
  it("reports a refusal no caller caught", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const target = new EventTarget();
    const status = store();
    const stop = listenForUnhandledRefusals(target, status);
    target.dispatchEvent(rejection(new GlslShaderRefused("ssao")));
    expect(status.getSnapshot().fault).toEqual({ kind: "shader-refused", effectName: "ssao" });
    stop();
  });

  it("ignores other rejections, and stops listening when asked", () => {
    const target = new EventTarget();
    const status = store();
    const stop = listenForUnhandledRefusals(target, status);
    target.dispatchEvent(rejection(new Error("the network is down")));
    expect(status.getSnapshot().fault).toBeNull();
    stop();
    target.dispatchEvent(rejection(new GlslShaderRefused("ssao")));
    expect(status.getSnapshot().fault).toBeNull();
  });
});
