/**
 * A fake engine for the `VIEW` display's tests: R01's `FakeRenderEngine`, which makes views and
 * records their frames, with the creations the wireframe's renderer needs answered by handles.
 */

import type { ViewEngineSource } from "../displays/view/useViewEngine";
import type { BufferSpec } from "../view/engine/memory";
import { requestAdapterOutcome } from "../view/engine/platform";
import type {
  BufferHandle,
  MaterialHandle,
  MeshHandle,
  MeshSpec,
  RenderEngine,
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
