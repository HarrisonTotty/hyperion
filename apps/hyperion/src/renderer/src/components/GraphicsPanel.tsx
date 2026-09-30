import type { GraphicsLaunchMode } from "../../../preload/api";
import type { AdapterSummary, GpuCapabilities } from "../view/engine/platform";
import {
  type GpuTimer,
  type GraphicsCondition,
  graphicsAnnunciation,
  useGraphicsStatus,
} from "../view/engine/status";
import { StatusLine } from "./StatusLine";

interface ReadingProps {
  /** The formatted value, or `null` when the value is missing. */
  readonly value: string | null;
}

function Reading({ value }: ReadingProps) {
  return value === null ? <dd className="readout__missing">—</dd> : <dd>{value}</dd>;
}

/** The launch modes' words, drafted for the owner (R01.T5.c). */
const MODE_WORDS = {
  default: "DEFAULT",
  vulkan: "VULKAN",
  safe: "SAFE",
} as const satisfies Record<GraphicsLaunchMode, string>;

/** The timer's words, drafted for the owner (R01.T5.c). */
const TIMER_WORDS = {
  quantized: "QUANTIZED",
  full: "FULL",
  absent: "ABSENT",
} as const satisfies Record<GpuTimer, string>;

/**
 * The capabilities the panel lists, by their WebGPU feature names, in `WANTED_FEATURES`' order.
 *
 * @remarks
 * The names are the API's identifiers, kept verbatim in lower case as the adapter's vendor and
 * architecture are, since they are names and not the console's words.
 */
const FEATURE_NAMES: ReadonlyArray<
  readonly [
    keyof Omit<GpuCapabilities, "maxTextureDimension2D" | "subgroupMinSize">,
    GPUFeatureName,
  ]
> = [
  ["subgroups", "subgroups"],
  ["shaderF16", "shader-f16"],
  ["timestampQuery", "timestamp-query"],
  ["float32Filterable", "float32-filterable"],
  ["float32Blendable", "float32-blendable"],
  ["rg11b10Renderable", "rg11b10ufloat-renderable"],
  ["depthClipControl", "depth-clip-control"],
];

function summaryOf(condition: GraphicsCondition): AdapterSummary | null {
  let summary: AdapterSummary | null;
  switch (condition.kind) {
    case "nominal":
    case "software-adapter":
      summary = condition.summary;
      break;
    case "acquiring":
    case "no-webgpu":
    case "no-adapter":
    case "safe-mode":
    case "disabled":
      summary = null;
      break;
  }
  return summary;
}

/** The styles the console may draw, or `null` while the adapter has not answered. */
function stylesOf(condition: GraphicsCondition): string | null {
  let styles: string | null;
  switch (condition.kind) {
    case "acquiring":
      styles = null;
      break;
    case "nominal":
      styles = condition.styles.photorealistic ? "WIREFRAME, PHOTOREALISTIC" : "WIREFRAME";
      break;
    case "software-adapter":
      styles = "WIREFRAME";
      break;
    case "no-webgpu":
    case "no-adapter":
    case "safe-mode":
    case "disabled":
      styles = "NONE";
      break;
  }
  return styles;
}

function featuresOf(capabilities: GpuCapabilities | null): string | null {
  if (capabilities === null) {
    return null;
  }
  const present = FEATURE_NAMES.filter(([key]) => capabilities[key]).map(([, name]) => name);
  return present.length > 0 ? present.join(", ") : "NONE";
}

/**
 * The client's graphics on the `LINK` display: the adapter, its features, the styles it may draw,
 * the launch mode, the GPU timer, the losses and restarts, and the current annunciation.
 *
 * @remarks
 * The annunciation is the console's report on itself, never an alert (R01 Design note 10): a fault
 * in `StatusLine`'s `fault` standing, a statement of condition in plain text.
 */
export function GraphicsPanel() {
  const status = useGraphicsStatus();
  const summary = summaryOf(status.condition);
  const annunciation = graphicsAnnunciation(status);
  return (
    <section className="panel" aria-labelledby="graphics-title">
      <h2 className="panel__title" id="graphics-title">
        Graphics
      </h2>
      <dl className="readout">
        <dt>Adapter</dt>
        <Reading value={summary === null ? null : `${summary.vendor} · ${summary.architecture}`} />
        <dt>Software Adapter</dt>
        <Reading value={summary === null ? null : summary.fallback ? "YES" : "NO"} />
        <dt>Features</dt>
        <Reading value={featuresOf(status.capabilities)} />
        <dt>Styles</dt>
        <Reading value={stylesOf(status.condition)} />
        <dt>Mode</dt>
        <Reading value={MODE_WORDS[status.launchMode]} />
        <dt>GPU Timer</dt>
        <Reading value={status.capabilities === null ? null : TIMER_WORDS[status.timer]} />
        <dt>Device Losses</dt>
        <dd>
          <output>{status.deviceLosses}</output>
        </dd>
        <dt>Process Restarts</dt>
        <dd>
          <output>{status.gpuProcessCrashes}</output>
        </dd>
      </dl>
      {annunciation === null ? null : (
        <StatusLine text={annunciation.text} standing={annunciation.standing} />
      )}
    </section>
  );
}
