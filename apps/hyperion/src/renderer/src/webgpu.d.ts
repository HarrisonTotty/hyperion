/**
 * WebGPU members TypeScript 7's `lib.dom` does not declare yet: the per-stage storage limits of
 * the current specification, and a texture's binding view dimension (compatibility mode). Only the
 * test fakes read them.
 */

interface GPUSupportedLimits {
  readonly maxStorageBuffersInFragmentStage: number;
  readonly maxStorageBuffersInVertexStage: number;
  readonly maxStorageTexturesInFragmentStage: number;
  readonly maxStorageTexturesInVertexStage: number;
}

interface GPUTextureDescriptor {
  textureBindingViewDimension?: GPUTextureViewDimension;
}
