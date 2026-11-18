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

import type { GraphicsLaunchMode, PerfLaunch } from "../../preload/api";
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

/** A switch as Chromium's command line writes it: `--name`, or `--name=value`. */
function switchText({ name, value }: ChromiumSwitch): string {
  return value === undefined ? `--${name}` : `--${name}=${value}`;
}

/** The profile's directory: the run's own, which says nothing of how it draws. */
const USER_DATA_DIR = "--user-data-dir";

/**
 * The switches in force on a perf launch, for its record (Design note 5): Chromium's that the
 * launch was given on its command line, then perf mode's own appended graphics switches.
 *
 * @remarks
 * The given ones are `argv`'s `--` arguments before the entry script, where the recipe puts them
 * (`perfRun.sh`: `--ozone-platform=…`, and with `--software` test-render's SwiftShader and
 * headless switches, `--enable-unsafe-webgpu` among them), as written; `--user-data-dir`, the
 * run's profile, is left out, and so is everything from the entry on, the client's own options. A
 * packaged launch has no entry to stop at, so its own options cannot be told from Chromium's, and
 * it gives none.
 *
 * @param argv - `process.argv`, its executable first.
 * @param packaged - `app.isPackaged`: a packaged app is launched as `hyperion <args>`, and in
 *   development Electron as `electron [switches] <entry> <args>`.
 * @param appended - Perf mode's switches, which it put on the command line itself (`perfSwitches`).
 */
export function perfSwitchesInForce(
  argv: ReadonlyArray<string>,
  packaged: boolean,
  appended: ReadonlyArray<ChromiumSwitch>,
): ReadonlyArray<string> {
  const given: string[] = [];
  if (!packaged) {
    for (const arg of argv.slice(1)) {
      if (!arg.startsWith("--")) {
        break;
      }
      if (arg !== USER_DATA_DIR && !arg.startsWith(`${USER_DATA_DIR}=`)) {
        given.push(arg);
      }
    }
  }
  return [...given, ...appended.map(switchText)];
}

/**
 * A perf run's window's content size, DIP: the station's 1080p on both settings (Design note 8's
 * station, the wireframe alone at 1080p), and every other scene's R05 Design note 21's 1080p at
 * `high` and 720p at `low`.
 */
export function perfWindowSize(launch: Pick<PerfLaunch, "scene" | "setting">): {
  readonly width: number;
  readonly height: number;
} {
  return launch.scene === "station-wireframe-descent" || launch.setting === "high"
    ? { width: 1920, height: 1080 }
    : { width: 1280, height: 720 };
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
 * its platform and CPU (`m5`); `swiftshader` for Chromium's software adapter, which the software
 * mode draws on and whose PCI IDs Chromium may report as 0 (a pod's run, 2026-11-18); otherwise
 * `gpu-` and the vendor's and device's IDs in hexadecimal, never a host name.
 *
 * @param software - Whether the launch is the check-only software mode (R01.T16).
 */
export function machineSlugOf(
  gpu: GpuIds | null,
  platform: NodeJS.Platform,
  cpuModel: string,
  software = false,
): string {
  if (software) {
    return "swiftshader";
  }
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
