/**
 * Perf mode's graphics switches (plan R12, R12.T4.a; Design note 4): R01's own, with their timing
 * toggle, and nothing more.
 *
 * @remarks
 * Chromium quantises WebGPU timestamps to 65,536 ns unless Dawn's `timestamp_quantization` toggle
 * is disabled, which R01's `gpuTiming` option does through `--disable-dawn-features`, changing no
 * feature, limit, WGSL language feature or adapter (Design note 4's probe of Chromium 152 on the
 * UHD 620). `--enable-webgpu-developer-features` would lift the mask too, but unmasks the adapter's
 * information, and `--enable-unsafe-webgpu` adds features, so perf mode passes neither: the
 * machine's identity comes from `app.getGPUInfo("complete")`. A control run (`--perf-control`)
 * omits the toggle, to confirm that the frame-interval median and the kernel's busy time do not
 * move with it.
 */

import type { GraphicsLaunchMode } from "../../preload/api";
import { type ChromiumSwitch, graphicsSwitches } from "../graphics/switches";

/** What decides a perf launch's switches. */
export interface PerfGraphicsOptions {
  readonly platform: NodeJS.Platform;
  readonly mode: GraphicsLaunchMode;
  /** `--perf-control`: the control run, without the timing toggle. */
  readonly control: boolean;
}

/**
 * Whether a perf launch lifts the timestamps' quantization: in every mode that draws with WebGPU
 * (R01's `graphicsSwitches` applies the toggle in all but `safe`), unless it is the control run.
 */
export function perfGpuTiming(options: Pick<PerfGraphicsOptions, "mode" | "control">): boolean {
  return options.mode !== "safe" && !options.control;
}

/**
 * A perf launch's switches: R01's `graphicsSwitches` with `gpuTiming` on, or off for the control
 * run, and no switch of perf mode's own.
 */
export function perfSwitches(options: PerfGraphicsOptions): ReadonlyArray<ChromiumSwitch> {
  return graphicsSwitches({
    platform: options.platform,
    mode: options.mode,
    gpuTiming: perfGpuTiming(options),
  });
}

/** The machines the rendering plans' README names (Target hardware), by what identifies them. */
const KNOWN_MACHINES: ReadonlyArray<{
  readonly slug: string;
  readonly matches: (gpu: GpuIds | null, platform: NodeJS.Platform, cpu: string) => boolean;
}> = [
  { slug: "rtx3080", matches: (gpu) => gpu?.vendorId === 0x10_de && gpu.deviceId === 0x22_06 },
  { slug: "uhd620", matches: (gpu) => gpu?.vendorId === 0x80_86 && gpu.deviceId === 0x3e_a0 },
  {
    slug: "m5",
    matches: (_gpu, platform, cpu) => platform === "darwin" && cpu.includes("Apple M5"),
  },
];

/** A PCI ID as four lowercase hexadecimal digits. */
function hexId(id: number): string {
  return id.toString(16).padStart(4, "0");
}

/** Google's PCI vendor ID, which Chromium's SwiftShader adapter reports. */
const SWIFTSHADER_VENDOR_ID = 0x1a_e0;

/** A GPU's PCI identity. */
export interface GpuIds {
  readonly vendorId: number;
  readonly deviceId: number;
}

/**
 * The machine's slug, a perf record ID's middle part (Design note 5): a Target hardware entry of
 * the rendering plans' README by its GPU's PCI IDs (`rtx3080`, `uhd620`) or, for the owner's Mac,
 * its platform and CPU (`m5`); `swiftshader` for Chromium's software adapter; otherwise `gpu-` and
 * the vendor's and device's IDs in hexadecimal, never a host name.
 */
export function machineSlugOf(
  gpu: GpuIds | null,
  platform: NodeJS.Platform,
  cpuModel: string,
): string {
  const known = KNOWN_MACHINES.find(({ matches }) => matches(gpu, platform, cpuModel));
  if (known !== undefined) {
    return known.slug;
  }
  if (gpu === null) {
    return `unknown-${platform}`;
  }
  if (gpu.vendorId === SWIFTSHADER_VENDOR_ID) {
    return "swiftshader";
  }
  return `gpu-${hexId(gpu.vendorId)}-${hexId(gpu.deviceId)}`;
}
