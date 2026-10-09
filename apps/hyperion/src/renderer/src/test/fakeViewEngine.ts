/**
 * A fake engine for the `VIEW` display's tests: R01's `FakeRenderEngine`, which makes views and
 * records their frames, with the creations the wireframe's and the photorealistic style's
 * renderers need answered by handles (R07.T8.a: async materials, render targets and textures), and
 * the exposure histogram answered from a {@link FakeMeteredImage} under the meter it was
 * dispatched with (R07.T16.b).
 */

import type { ViewEngineSource } from "../displays/view/useViewEngine";
import type { BufferSpec, TextureSpec } from "../view/engine/memory";
import { requestAdapterOutcome } from "../view/engine/platform";
import type { KernelPair } from "../view/engine/kernels";
import { HISTOGRAM_BINS, HISTOGRAM_KERNEL } from "../view/post/histogram";
import { METER_CLASS, type MeterClass } from "../view/post/meter";
import type {
  BufferHandle,
  ComputeBindings,
  ComputeHandle,
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

/** A run of a drawn image's pixels of one meter class, in one histogram bin. */
export interface FakePixels {
  readonly meterClass: MeterClass;
  readonly bin: number;
  readonly count: number;
}

/**
 * What the photorealistic image holds, as the exposure histogram weighs it; a test may change its
 * `pixels` to bring a lit body into view.
 */
export interface FakeMeteredImage {
  pixels: ReadonlyArray<FakePixels>;
}

/**
 * The image every fake engine draws unless a test says otherwise: a thousand pixels in the middle
 * bin, none of a body (`other`), so that `AVG` meters it and `LIT` and `DARK` weigh nothing.
 */
export function aFakeMeteredImage(): FakeMeteredImage {
  return { pixels: [{ meterClass: METER_CLASS.other, bin: HISTOGRAM_BINS >> 1, count: 1000 }] };
}

/**
 * An engine on `device` that answers the renderer's creations and drops its writes; its histogram
 * read-back weighs `image` by the weights its dispatch carried.
 */
export function fakeViewEngine(
  device: GPUDevice,
  image: FakeMeteredImage = aFakeMeteredImage(),
): FakeViewEngine {
  // Each histogram buffer's weights, as its last dispatch wrote them (`histogramParams`).
  const weightsOf = new Map<string, Uint32Array>();
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
    createComputeAsync: (pair: KernelPair): Promise<ComputeHandle> =>
      Promise.resolve({ kind: "compute", name: pair.name, path: "reference" }),
    dispatch: (kernel: ComputeHandle, bindings: ComputeBindings): void => {
      const bins = bindings.buffers["bins"];
      const params = bindings.uniforms["params"];
      if (
        kernel.name === HISTOGRAM_KERNEL.name &&
        bins !== undefined &&
        params instanceof Uint32Array
      ) {
        weightsOf.set(bins.name, params.slice(0, 4));
      }
    },
    // The image's histogram under the meter it was dispatched with.
    readBuffer: (buffer: BufferHandle): Promise<ArrayBuffer> => {
      const bins = new Uint32Array(buffer.bytes / 4);
      const weights = weightsOf.get(buffer.name);
      for (const { meterClass, bin, count } of image.pixels) {
        bins[bin] = (bins[bin] ?? 0) + count * (weights?.[meterClass] ?? 0);
      }
      return Promise.resolve(bins.buffer);
    },
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
  /** The image every engine of the source draws, which a test may change. */
  readonly image: FakeMeteredImage;
}

/** A source whose adapter is a fake SwiftShader and whose engines are {@link fakeViewEngine}s. */
export function fakeViewEngineSource(): FakeViewEngineSource {
  const engines: FakeViewEngine[] = [];
  const image = aFakeMeteredImage();
  return {
    engines,
    image,
    source: {
      requestAdapter: () =>
        requestAdapterOutcome(
          new FakeGpu([new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] })]),
        ),
      load: async (outcome) => {
        const engine = fakeViewEngine(await outcome.adapter.requestDevice(), image);
        engines.push(engine);
        return engine;
      },
    },
  };
}
