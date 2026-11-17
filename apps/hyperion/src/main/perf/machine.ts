/**
 * What a perf record says about the machine (plan R12, R12.T2; Design note 5): the record's
 * `MachineRecord`, read in the main process at a run's start.
 *
 * @remarks
 * The CPU from `os.cpus()`, the memory from `os.totalmem()`, the kernel from `os.release()`, the
 * active GPU from `app.getGPUInfo("complete")` (its device and vendor IDs, and the driver's vendor
 * and version, `Mesa 26.2.3` or NVIDIA's `615.71.09`), the display from Electron's `screen`, the
 * versions from `process.versions` and `app.getVersion()`, and R01's `AdapterSummary`,
 * capabilities and kernel paths from the renderer over `window.hyperion.perf` (R12.T4.b), passed in
 * as {@link MachineInputs}. The window manager, compositor and window system come from the
 * environment: under Linux `XDG_SESSION_TYPE` and the Ozone platform the launch chose give X11,
 * XWayland or Wayland; `XDG_CURRENT_DESKTOP`, `XDG_SESSION_DESKTOP`, `DESKTOP_SESSION`, or i3's and
 * sway's own variables (`I3SOCK`, `SWAYSOCK`) name the window manager; under Wayland the
 * compositor is the window manager, and under X11 it is the first known compositor among the
 * processes' `/proc/<pid>/comm`, `none` where none runs. macOS's WindowServer and Windows's DWM
 * always composite. The function works on every platform Electron runs on (the M5 Mac's lean,
 * pending the owner).
 */

import { cpus, release, totalmem } from "node:os";

import type {
  AdapterRecord,
  CardRecord,
  DisplayRecord,
  GpuRecord,
  MachineRecord,
  VersionsRecord,
} from "@hyperion/perf";

import type { SysfsFiles } from "../gpuClocks";
import { activeGpu } from "../results";
import { listDir, NODE_FILES, readText } from "./samplers";

/** Electron's `Display`, as much of it as the record reads. */
export interface DisplayLike {
  /** Device-independent pixels. */
  readonly size: { readonly width: number; readonly height: number };
  readonly scaleFactor: number;
  /** Hz; 0 where Chromium does not know it. */
  readonly displayFrequency: number;
}

/** What {@link describeMachine} reads, injected so that a test describes any machine. */
export interface MachineSources {
  /** `process.platform` outside a test. */
  readonly platform: NodeJS.Platform;
  /** `os.release()`. */
  release(): string;
  cpus(): ReadonlyArray<{ readonly model: string }>;
  totalmem(): number;
  /** `app.getGPUInfo("complete")`. */
  gpuInfo(): Promise<unknown>;
  /** The run window's display (`screen.getDisplayMatching`), or `null` with none. */
  display(): DisplayLike | null;
  readonly env: Readonly<Record<string, string | undefined>>;
  readonly versions: VersionsRecord;
  /** The file system, for `/proc`'s process names. */
  readonly files: SysfsFiles;
  /** Where procfs is mounted: `/proc`, or a fixture tree in a test. */
  readonly procRoot: string;
}

/** What the run knows that the main process cannot read itself. */
export interface MachineInputs {
  /**
   * The machine's slug, the record ID's middle part: a Target hardware entry of the rendering
   * plans' README (`uhd620`, `rtx3080`, `m5`), never a host name.
   */
  readonly name: string;
  /** R01's `AdapterSummary` from the renderer, or `null` before it reports. */
  readonly adapter: AdapterRecord | null;
  /** R01's `GpuCapabilities`, by member name. */
  readonly capabilities: Readonly<Record<string, boolean>> | null;
  /** `selectKernel`'s path for each kernel pair. */
  readonly kernelPaths: Readonly<Record<string, string>> | null;
  /** A discrete card's published figures (R12.T9.a); `null` elsewhere. */
  readonly card: CardRecord | null;
  /** The Ozone platform the launch chose (`--ozone-platform`), or `null` where it chose none. */
  readonly ozonePlatform: string | null;
}

/** The parts of this process's sources that are not Electron's, which the caller supplies. */
export interface ElectronMachineParts {
  gpuInfo(): Promise<unknown>;
  display(): DisplayLike | null;
  /** `app.getVersion()`. */
  readonly appVersion: string;
}

/** This process's sources, with Electron's parts from the caller (the runner, R12.T4.a). */
export function nodeMachineSources(electron: ElectronMachineParts): MachineSources {
  return {
    platform: process.platform,
    release,
    cpus,
    totalmem,
    gpuInfo: () => electron.gpuInfo(),
    display: () => electron.display(),
    env: process.env,
    versions: {
      app: electron.appVersion,
      electron: process.versions.electron,
      chromium: process.versions.chrome,
      node: process.versions.node,
    },
    files: NODE_FILES,
    procRoot: "/proc",
  };
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * The active GPU of Chromium's complete GPU information: R05's narrowing (`results.ts`'s
 * `activeGpu`), with the driver's vendor before its version where Chromium gives one apart
 * (`Mesa` and `26.2.3`), or `null` where Chromium reports no device.
 */
export function gpuRecordOf(info: unknown): GpuRecord | null {
  const gpu = activeGpu(info);
  if (gpu.value === null) {
    return null;
  }
  const devices = isRecord(info) ? info["gpuDevice"] : undefined;
  const list = Array.isArray(devices) ? devices.filter(isRecord) : [];
  const device = list.find((entry) => entry["active"] === true) ?? list[0];
  const vendor = device?.["driverVendor"];
  const { driverVersion } = gpu.value;
  const prefixed =
    typeof vendor === "string" &&
    vendor.length > 0 &&
    driverVersion !== null &&
    !driverVersion.startsWith(vendor)
      ? `${vendor} ${driverVersion}`
      : driverVersion;
  return { ...gpu.value, driverVersion: prefixed };
}

/** The display in pixels, from Electron's device-independent size and scale. */
export function displayRecordOf(display: DisplayLike | null): DisplayRecord | null {
  if (display === null) {
    return null;
  }
  const { size, scaleFactor, displayFrequency } = display;
  return {
    widthPx: Math.round(size.width * scaleFactor),
    heightPx: Math.round(size.height * scaleFactor),
    refreshHz: displayFrequency > 0 ? displayFrequency : null,
    devicePixelRatio: scaleFactor,
  };
}

/** The record's window system: Linux's from the session and the Ozone platform. */
export function windowSystemOf(
  platform: MachineRecord["platform"],
  env: Readonly<Record<string, string | undefined>>,
  ozonePlatform: string | null,
): MachineRecord["windowSystem"] {
  let windowSystem: MachineRecord["windowSystem"];
  switch (platform) {
    case "darwin":
      windowSystem = "macos";
      break;
    case "win32":
      windowSystem = "windows";
      break;
    case "linux": {
      const session = env["XDG_SESSION_TYPE"];
      const wayland =
        session === "wayland" || (session === undefined && env["WAYLAND_DISPLAY"] !== undefined);
      if (wayland) {
        windowSystem = ozonePlatform === "wayland" ? "wayland" : "xwayland";
      } else {
        windowSystem = session === "x11" || env["DISPLAY"] !== undefined ? "x11" : null;
      }
      break;
    }
  }
  return windowSystem;
}

/** Linux's window manager as the environment names it, lower-cased, or `null` where it names none. */
function linuxWindowManagerOf(env: Readonly<Record<string, string | undefined>>): string | null {
  const named = [env["XDG_CURRENT_DESKTOP"], env["XDG_SESSION_DESKTOP"], env["DESKTOP_SESSION"]]
    .map((value) => value?.trim() ?? "")
    .find((value) => value.length > 0);
  if (named !== undefined) {
    // `XDG_CURRENT_DESKTOP` is a colon-separated list, most specific first (`ubuntu:GNOME`).
    return named.split(":").at(-1)?.toLowerCase() ?? null;
  }
  if (env["I3SOCK"] !== undefined) {
    return "i3";
  }
  return env["SWAYSOCK"] === undefined ? null : "sway";
}

/** The window manager: macOS's WindowServer, Windows's DWM, or the one Linux's environment names. */
export function windowManagerOf(
  platform: MachineRecord["platform"],
  env: Readonly<Record<string, string | undefined>>,
): string | null {
  let windowManager: string | null;
  switch (platform) {
    case "darwin":
      windowManager = "WindowServer";
      break;
    case "win32":
      windowManager = "dwm";
      break;
    case "linux":
      windowManager = linuxWindowManagerOf(env);
      break;
  }
  return windowManager;
}

/**
 * The X11 compositors looked for among the processes: the standalone ones and the window managers
 * that composite as they run.
 */
export const X11_COMPOSITORS = [
  "picom",
  "compton",
  "xcompmgr",
  "compiz",
  "gnome-shell",
  "kwin_x11",
  "muffin",
  "cinnamon",
  "marco",
] as const;

/** The first of {@link X11_COMPOSITORS} among `/proc`'s processes, `none`, or `null` where unread. */
async function x11CompositorOf(files: SysfsFiles, procRoot: string): Promise<string | null> {
  const entries = await listDir(files, procRoot);
  if (entries.value === null) {
    return null;
  }
  const names = await Promise.all(
    entries.value
      .filter((name) => /^\d+$/.test(name))
      .map(async (pid) => (await readText(files, `${procRoot}/${pid}/comm`)).value),
  );
  const running = new Set(names.filter((name): name is string => name !== null));
  return X11_COMPOSITORS.find((name) => running.has(name)) ?? "none";
}

/** The compositor, by window system (the module's remarks). */
async function compositorOf(
  sources: MachineSources,
  windowSystem: MachineRecord["windowSystem"],
  windowManager: string | null,
): Promise<string | null> {
  if (windowSystem === "x11") {
    return x11CompositorOf(sources.files, sources.procRoot);
  }
  // macOS's WindowServer, Windows's DWM and a Wayland session's window manager composite.
  return windowSystem === null ? null : windowManager;
}

/** The record's platform, Electron's three. */
function platformOf(platform: NodeJS.Platform): MachineRecord["platform"] {
  if (platform === "linux" || platform === "darwin" || platform === "win32") {
    return platform;
  }
  throw new Error(`a perf record names linux, darwin or win32; this is ${platform}`);
}

/**
 * The machine a run is on (Design note 5; the module's remarks).
 *
 * @throws Error on a platform other than Linux, macOS and Windows, which Electron does not run on.
 */
export async function describeMachine(
  inputs: MachineInputs,
  sources: MachineSources,
): Promise<MachineRecord> {
  const platform = platformOf(sources.platform);
  const gpu = await sources.gpuInfo().then(gpuRecordOf, () => null);
  const cpuList = sources.cpus();
  const windowSystem = windowSystemOf(platform, sources.env, inputs.ozonePlatform);
  const windowManager = windowManagerOf(platform, sources.env);
  return {
    name: inputs.name,
    platform,
    kernel: sources.release(),
    cpu: { model: cpuList[0]?.model.trim() ?? "unknown", logicalCores: cpuList.length },
    memoryBytes: sources.totalmem(),
    gpu,
    adapter: inputs.adapter,
    capabilities: inputs.capabilities,
    kernelPaths: inputs.kernelPaths,
    versions: sources.versions,
    display: displayRecordOf(sources.display()),
    windowManager,
    compositor: await compositorOf(sources, windowSystem, windowManager),
    windowSystem,
    card: inputs.card,
  };
}
