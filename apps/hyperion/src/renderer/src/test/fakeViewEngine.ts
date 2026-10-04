/**
 * A fake engine for the `VIEW` display's tests: R01's `FakeRenderEngine`, which makes views and
 * records their frames, with the creations the wireframe's and the photorealistic style's
 * renderers need answered by handles (R07.T8.a: async materials, render targets and textures).
 */

import type { ViewEngineSource } from "../displays/view/useViewEngine";
import type { BufferSpec, TextureSpec } from "../view/engine/memory";
import { requestAdapterOutcome } from "../view/engine/platform";
import type {
  BufferHandle,
  MaterialHandle,
  MeshHandle,
  MeshSpec,
  RenderEngine,
  RenderTarget,
  RenderTargetSpec,
  TextureHandle,
  WgslMaterialSpec,
} from "../view/engine/types";
import { FakeAdapter, FakeGpu, SWIFTSHADER_INFO } from "./fakeGpu";
import { FakeRenderEngine } from "./fakeRenderEngine";

/** The fake engine, with the views it made. */
export type FakeViewEngine = FakeRenderEngine & RenderEngine;

/** An engine on `device` that answers the renderer's creations and drops its writes. */
export function fakeViewEngine(device: GPUDevice): FakeViewEngine {
  return Object.assign(new FakeRenderEngine(device), {
    createMesh: (spec: MeshSpec): MeshHandle => ({ kind: "mesh", name: spec.name }),
    createMaterial: (spec: WgslMaterialSpec): MaterialHandle => ({
      kind: "material",
      name: spec.name,
    }),
    createBuffer: (spec: BufferSpec): BufferHandle => ({
      kind: "buffer",
      name: spec.name,
      bytes: spec.bytes,
    }),
    writeBuffer: (): void => undefined,
    createMaterialAsync: (spec: WgslMaterialSpec): Promise<MaterialHandle> =>
      Promise.resolve({ kind: "material", name: spec.name }),
    createTexture: (spec: TextureSpec): TextureHandle => ({ kind: "texture", name: spec.name }),
    writeTexture: (): void => undefined,
    releaseBuffer: (): void => undefined,
    releaseTexture: (): void => undefined,
    createRenderTarget: (spec: RenderTargetSpec): RenderTarget => ({
      name: spec.name,
      colour: { kind: "texture", name: `${spec.name} colour` },
      depth: spec.depth ? { kind: "texture", name: `${spec.name} depth` } : null,
      resize: (): void => undefined,
      render: (): void => undefined,
      dispose: (): void => undefined,
    }),
  });
}

/** A source of fake engines, and those it has made, first first. */
export interface FakeViewEngineSource {
  readonly source: ViewEngineSource;
  readonly engines: FakeViewEngine[];
}

/** A source whose adapter is a fake SwiftShader and whose engines are {@link fakeViewEngine}s. */
export function fakeViewEngineSource(): FakeViewEngineSource {
  const engines: FakeViewEngine[] = [];
  return {
    engines,
    source: {
      requestAdapter: () =>
        requestAdapterOutcome(
          new FakeGpu([new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] })]),
        ),
      load: async (outcome) => {
        const engine = fakeViewEngine(await outcome.adapter.requestDevice());
        engines.push(engine);
        return engine;
      },
    },
  };
}
