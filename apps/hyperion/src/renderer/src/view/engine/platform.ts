/**
 * Acquiring and vetting a WebGPU adapter, with no engine types.
 *
 * @remarks
 * The client requests its own adapter and hands it to the engine (R01 Design note 24), so that
 * every adapter the engine sees has been summarised and, if it is a software one, refused the
 * photorealistic style (Design note 8). An adapter is consumed by its first `requestDevice`, so a
 * rebuild after a device loss asks for a fresh one here and never reuses the old one.
 */

/** Who made the adapter and whether it is a software one. */
export interface AdapterSummary {
  readonly vendor: string;
  readonly architecture: string;
  readonly description: string;
  /**
   * `info.isFallbackAdapter`, or an `architecture` of `swiftshader`: the probe of 2026-09-29 found
   * both on SwiftShader and neither on the UHD 620 (R01 Design note 8).
   */
  readonly fallback: boolean;
}

/** The optional features and limits the views and kernels branch on. */
export interface GpuCapabilities {
  readonly subgroups: boolean;
  readonly shaderF16: boolean;
  readonly timestampQuery: boolean;
  readonly float32Filterable: boolean;
  readonly float32Blendable: boolean;
  readonly rg11b10Renderable: boolean;
  /** R10's shadow cascades. */
  readonly depthClipControl: boolean;
  /** Pixels on a side. */
  readonly maxTextureDimension2D: number;
  /** Invocations in the smallest subgroup, or `null` without the `subgroups` feature. */
  readonly subgroupMinSize: number | null;
  /** Bytes a storage-buffer binding may span (R05.T11.a; decisions-r06-r07.md item 7). */
  readonly maxStorageBufferBindingSize: number;
  /** Bytes a buffer may hold. */
  readonly maxBufferSize: number;
}

/** Which view styles the adapter may draw. */
export interface StyleAvailability {
  readonly wireframe: boolean;
  readonly photorealistic: boolean;
}

/** What asking for an adapter gave. */
export type AdapterOutcome =
  /** `navigator.gpu` is absent. */
  | { readonly kind: "no-webgpu" }
  /** `requestAdapter` resolved to null. */
  | { readonly kind: "no-adapter" }
  | {
      readonly kind: "adapter";
      readonly adapter: GPUAdapter;
      readonly summary: AdapterSummary;
      readonly capabilities: GpuCapabilities;
      readonly styles: StyleAvailability;
    };

/**
 * The optional features requested whenever the adapter has them, and never required (R01 Design
 * note 24).
 */
export const WANTED_FEATURES: ReadonlyArray<GPUFeatureName> = [
  "subgroups",
  "shader-f16",
  "timestamp-query",
  "float32-filterable",
  "float32-blendable",
  "rg11b10ufloat-renderable",
  "depth-clip-control",
];

/**
 * Features the smoke harness withholds from the device, so that each capability path runs on one
 * adapter.
 *
 * @remarks
 * Used only by the harness page (R01.T9): the bridge client never withholds a feature.
 */
export interface CapabilityOverrides {
  readonly withholdSubgroups: boolean;
  readonly withholdShaderF16: boolean;
  readonly withholdFloat32Blendable?: boolean;
  /**
   * Requests WebGPU's default limits rather than {@link requiredLimits}' raised ones, so that the
   * harness runs R05's `FaceDifferences` fallback for the high setting (decisions-r06-r07.md
   * item 7).
   */
  readonly defaultLimits?: boolean;
}

/** The adapter's identity and capabilities. */
export function summariseAdapter(adapter: GPUAdapter): {
  readonly summary: AdapterSummary;
  readonly capabilities: GpuCapabilities;
} {
  const { info, features, limits } = adapter;
  const subgroups = features.has("subgroups");
  return {
    summary: {
      vendor: info.vendor,
      architecture: info.architecture,
      description: info.description,
      fallback: info.isFallbackAdapter || info.architecture === "swiftshader",
    },
    capabilities: {
      subgroups,
      shaderF16: features.has("shader-f16"),
      timestampQuery: features.has("timestamp-query"),
      float32Filterable: features.has("float32-filterable"),
      float32Blendable: features.has("float32-blendable"),
      rg11b10Renderable: features.has("rg11b10ufloat-renderable"),
      depthClipControl: features.has("depth-clip-control"),
      maxTextureDimension2D: limits.maxTextureDimension2D,
      subgroupMinSize: subgroups ? info.subgroupMinSize : null,
      maxStorageBufferBindingSize: limits.maxStorageBufferBindingSize,
      maxBufferSize: limits.maxBufferSize,
    },
  };
}

/**
 * The capabilities of a device, read from its own features and limits.
 *
 * @remarks
 * A device has only the features it was asked for, so a feature the smoke harness withholds reads
 * as absent here, and so everywhere downstream, although the adapter has it (R01 Design note 24).
 */
export function deviceCapabilities(device: GPUDevice): GpuCapabilities {
  const { features, limits, adapterInfo } = device;
  const subgroups = features.has("subgroups");
  return {
    subgroups,
    shaderF16: features.has("shader-f16"),
    timestampQuery: features.has("timestamp-query"),
    float32Filterable: features.has("float32-filterable"),
    float32Blendable: features.has("float32-blendable"),
    rg11b10Renderable: features.has("rg11b10ufloat-renderable"),
    depthClipControl: features.has("depth-clip-control"),
    maxTextureDimension2D: limits.maxTextureDimension2D,
    subgroupMinSize: subgroups ? adapterInfo.subgroupMinSize : null,
    maxStorageBufferBindingSize: limits.maxStorageBufferBindingSize,
    maxBufferSize: limits.maxBufferSize,
  };
}

/**
 * The features asked of a device that it does not have.
 *
 * @remarks
 * The engine compares what it asked for with what the device enabled and reports the
 * difference (R01 Design note 24), so that a feature quietly missing is never mistaken for one
 * present.
 */
export function featuresNotEnabled(
  requested: ReadonlyArray<GPUFeatureName>,
  enabled: ReadonlySet<string>,
): ReadonlyArray<GPUFeatureName> {
  return requested.filter((feature) => !enabled.has(feature));
}

/**
 * The styles an adapter may draw.
 *
 * @returns The wireframe always; the photorealistic style only on a hardware adapter, since the
 * brainstorm confines the software adapter's refusal to that style (R01 Design note 8).
 */
export function styleAvailability(summary: AdapterSummary): StyleAvailability {
  return { wireframe: true, photorealistic: !summary.fallback };
}

/**
 * Asks for a high-performance adapter and vets it.
 *
 * @param gpu - `navigator.gpu`, or `undefined` where WebGPU is absent.
 * @remarks
 * `powerPreference: "high-performance"` changed nothing on the one-GPU probe machine but picks the
 * discrete part on a machine with two.
 */
export async function requestAdapterOutcome(gpu: GPU | undefined): Promise<AdapterOutcome> {
  if (gpu === undefined) {
    return { kind: "no-webgpu" };
  }
  const adapter = await gpu.requestAdapter({ powerPreference: "high-performance" });
  if (adapter === null) {
    return { kind: "no-adapter" };
  }
  const { summary, capabilities } = summariseAdapter(adapter);
  return { kind: "adapter", adapter, summary, capabilities, styles: styleAvailability(summary) };
}

/** The most bytes a raised buffer limit asks for: 1 GiB (decisions-r06-r07.md item 7). */
export const MAX_REQUESTED_BUFFER_BYTES = 2 ** 30;

/**
 * The limits to require of the device: the adapter's storage-binding and buffer sizes, never more
 * than it reports and at most {@link MAX_REQUESTED_BUFFER_BYTES}, so that R05's high-setting
 * `BakedOffsets` layout fits one binding where the adapter allows (decisions-r06-r07.md item 7).
 *
 * @remarks
 * A pure function of the adapter, so a rebuild after a device loss asks the same of the same
 * hardware and less of a lesser adapter, and `requestDevice` cannot reject on limits. With the
 * harness's `defaultLimits` override, nothing is raised.
 */
export function requiredLimits(
  adapter: GPUAdapter,
  overrides: CapabilityOverrides | undefined,
): Record<string, number> {
  if (overrides?.defaultLimits === true) {
    return {};
  }
  return {
    maxStorageBufferBindingSize: Math.min(
      adapter.limits.maxStorageBufferBindingSize,
      MAX_REQUESTED_BUFFER_BYTES,
    ),
    maxBufferSize: Math.min(adapter.limits.maxBufferSize, MAX_REQUESTED_BUFFER_BYTES),
  };
}

/**
 * The features to require of the device.
 *
 * @returns {@link WANTED_FEATURES} that the adapter has, less those the overrides withhold.
 */
export function requiredFeatures(
  adapter: GPUAdapter,
  overrides: CapabilityOverrides | undefined,
): ReadonlyArray<GPUFeatureName> {
  const withheld = new Set<GPUFeatureName>();
  if (overrides?.withholdSubgroups === true) {
    withheld.add("subgroups");
  }
  if (overrides?.withholdShaderF16 === true) {
    withheld.add("shader-f16");
  }
  if (overrides?.withholdFloat32Blendable === true) {
    withheld.add("float32-blendable");
  }
  return WANTED_FEATURES.filter(
    (feature) => adapter.features.has(feature) && !withheld.has(feature),
  );
}
