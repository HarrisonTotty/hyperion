/**
 * R06's point splat: an additive `point-list` pass into a 2D `rgba32float` bake target (R01 Design
 * note 21).
 *
 * @remarks
 * Encoded by the adapter like its other raw passes, on the engine's device. The sources are plain
 * WGSL with `main` entry points, not Babylon's dialect: the vertex stage reads the points from a
 * read-only storage buffer the source declares as `@group(0) @binding(0) var<storage, read>
 * points`, indexed by `@builtin(vertex_index)`, and each point adds its colour to the target, alpha
 * included. The pass loads the target, so successive draws accumulate; the caller clears it by
 * writing zeros when a bake begins. Blending an `rgba32float` target needs `float32-blendable`;
 * without it the splat cannot be made, and R06 answers with a compute splat. The brainstorm keeps
 * `rgba32float` out of colour targets for a frame's bandwidth; a splat target is a per-arrival bake
 * scratch, never drawn into each frame, so that reason does not reach it.
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import { type BufferSpec, extentOf, type MemoryCategory, type TextureSpec } from "../memory";
import type { GpuCapabilities } from "../platform";
import { Float32BlendUnavailable, type PointSplatSpec } from "../types";

/** The blend of a splat: every channel added, alpha too. */
export const SPLAT_BLEND: Readonly<GPUBlendState> = Object.freeze({
  color: Object.freeze({ srcFactor: "one", dstFactor: "one", operation: "add" } as const),
  alpha: Object.freeze({ srcFactor: "one", dstFactor: "one", operation: "add" } as const),
});

/**
 * Checks a splat's target and points before a draw.
 *
 * @throws Error naming what is wrong: a target that is not a single-layer 2D texture of the
 * splat's format made with `RENDER_ATTACHMENT`, or points not made with `STORAGE`.
 */
export function assertSplatInputs(
  spec: PointSplatSpec,
  target: TextureSpec,
  points: BufferSpec,
): void {
  if (target.format !== spec.format || target.dimension !== "2d") {
    throw new Error(`splat ${spec.name} draws into a 2D ${spec.format}, not ${target.name}`);
  }
  if (extentOf(target.size).depthOrArrayLayers !== 1) {
    throw new Error(`splat ${spec.name} draws into one layer; ${target.name} has more`);
  }
  if ((target.usage & TEXTURE_USAGE.RENDER_ATTACHMENT) === 0) {
    throw new Error(`${target.name} was not made with RENDER_ATTACHMENT`);
  }
  if ((points.usage & BUFFER_USAGE.STORAGE) === 0) {
    throw new Error(`${points.name} was not made with STORAGE`);
  }
}

/**
 * Refuses a splat on a device that cannot blend `rgba32float`.
 *
 * @throws {@link Float32BlendUnavailable} without `float32-blendable`.
 */
export function assertSplatSupported(capabilities: GpuCapabilities): void {
  if (!capabilities.float32Blendable) {
    throw new Float32BlendUnavailable();
  }
}

/** The render pipeline of a splat, from its two compiled stages. */
export function pointSplatPipelineDescriptor(
  spec: PointSplatSpec,
  vertex: GPUShaderModule,
  fragment: GPUShaderModule,
): GPURenderPipelineDescriptor {
  return {
    label: `${spec.name} splat`,
    layout: "auto",
    vertex: { module: vertex, entryPoint: "main" },
    fragment: {
      module: fragment,
      entryPoint: "main",
      targets: [{ format: spec.format, blend: SPLAT_BLEND }],
    },
    primitive: { topology: "point-list" },
  };
}

/** Encodes one splat of `count` points into `target`, accumulating onto what it holds. */
export function encodeSplat(
  device: GPUDevice,
  encoder: GPUCommandEncoder,
  pipeline: GPURenderPipeline,
  target: GPUTexture,
  points: GPUBuffer,
  count: number,
  label: string,
  timestampWrites: GPURenderPassTimestampWrites | undefined,
): void {
  const pass = encoder.beginRenderPass({
    label,
    colorAttachments: [
      {
        view: target.createView({
          dimension: "2d",
          baseMipLevel: 0,
          mipLevelCount: 1,
          baseArrayLayer: 0,
          arrayLayerCount: 1,
        }),
        loadOp: "load",
        storeOp: "store",
      },
    ],
    ...(timestampWrites === undefined ? {} : { timestampWrites }),
  });
  pass.setPipeline(pipeline);
  pass.setBindGroup(
    0,
    device.createBindGroup({
      label: `${label} points`,
      layout: pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: points } }],
    }),
  );
  pass.draw(count);
  pass.end();
}

/**
 * The specification of a splat's bake target, a square face of `sizePx`: made with
 * `createTexture`, so its bytes are counted, sampleable and readable, and cleared by a write.
 */
export function splatTargetSpec(
  name: string,
  sizePx: number,
  category: MemoryCategory,
): TextureSpec {
  return {
    name,
    size: [sizePx, sizePx],
    dimension: "2d",
    format: "rgba32float",
    mips: 1,
    usage:
      TEXTURE_USAGE.RENDER_ATTACHMENT |
      TEXTURE_USAGE.TEXTURE_BINDING |
      TEXTURE_USAGE.COPY_SRC |
      TEXTURE_USAGE.COPY_DST,
    category,
  };
}
