/**
 * Mip generation for an offscreen target's colour, after it is rendered.
 *
 * @remarks
 * A `RenderTargetSpec` names its mips, and the adapter generates them itself: each level is a full-screen triangle sampling the level above through a linear sampler,
 * which averages the four texels a texel of the smaller level covers when the size is even.
 */

/** The WGSL of the downsampling pass: a full-screen triangle and one linear sample. */
export const DOWNSAMPLE_WGSL = `
@group(0) @binding(0) var source : texture_2d<f32>;
@group(0) @binding(1) var linear : sampler;

struct Varyings {
  @builtin(position) position : vec4f,
  @location(0) uv : vec2f,
};

@vertex fn vertexMain(@builtin(vertex_index) index : u32) -> Varyings {
  let corner = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
  var out : Varyings;
  out.position = vec4f(corner * 2.0 - 1.0, 0.0, 1.0);
  out.uv = vec2f(corner.x, 1.0 - corner.y);
  return out;
}

@fragment fn fragmentMain(in : Varyings) -> @location(0) vec4f {
  return textureSample(source, linear, in.uv);
}
`;

/** Generates the mips of 2D textures, one pipeline per format. */
export class MipGenerator {
  readonly #device: GPUDevice;
  readonly #pipelines = new Map<GPUTextureFormat, GPURenderPipeline>();
  #module: GPUShaderModule | null = null;
  #sampler: GPUSampler | null = null;

  constructor(device: GPUDevice) {
    this.#device = device;
  }

  /**
   * Encodes levels 1 to `mips - 1` of `texture`, each from the one above.
   *
   * @param writes - The timestamps of the whole chain: the first level's pass writes the
   * beginning, the last level's the end.
   */
  encode(
    encoder: GPUCommandEncoder,
    texture: GPUTexture,
    mips: number,
    writes?: {
      readonly querySet: GPUQuerySet;
      readonly beginningOfPassWriteIndex: number;
      readonly endOfPassWriteIndex: number;
    },
  ): void {
    const pipeline = this.#pipeline(texture.format);
    const sampler = this.#linearSampler();
    for (let level = 1; level < mips; level += 1) {
      const source = texture.createView({ baseMipLevel: level - 1, mipLevelCount: 1 });
      const destination = texture.createView({ baseMipLevel: level, mipLevelCount: 1 });
      const timestampWrites =
        writes === undefined
          ? undefined
          : {
              querySet: writes.querySet,
              ...(level === 1
                ? { beginningOfPassWriteIndex: writes.beginningOfPassWriteIndex }
                : {}),
              ...(level === mips - 1 ? { endOfPassWriteIndex: writes.endOfPassWriteIndex } : {}),
            };
      const pass = encoder.beginRenderPass({
        label: `${texture.label} mip ${level}`,
        colorAttachments: [{ view: destination, loadOp: "clear", storeOp: "store" }],
        ...(timestampWrites === undefined ? {} : { timestampWrites }),
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(
        0,
        this.#device.createBindGroup({
          layout: pipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: source },
            { binding: 1, resource: sampler },
          ],
        }),
      );
      pass.draw(3);
      pass.end();
    }
  }

  #pipeline(format: GPUTextureFormat): GPURenderPipeline {
    const existing = this.#pipelines.get(format);
    if (existing !== undefined) {
      return existing;
    }
    this.#module ??= this.#device.createShaderModule({
      label: "downsample",
      code: DOWNSAMPLE_WGSL,
    });
    const pipeline = this.#device.createRenderPipeline({
      label: `downsample ${format}`,
      layout: "auto",
      vertex: { module: this.#module, entryPoint: "vertexMain" },
      fragment: { module: this.#module, entryPoint: "fragmentMain", targets: [{ format }] },
      primitive: { topology: "triangle-list" },
    });
    this.#pipelines.set(format, pipeline);
    return pipeline;
  }

  #linearSampler(): GPUSampler {
    this.#sampler ??= this.#device.createSampler({
      label: "downsample",
      magFilter: "linear",
      minFilter: "linear",
    });
    return this.#sampler;
  }
}
