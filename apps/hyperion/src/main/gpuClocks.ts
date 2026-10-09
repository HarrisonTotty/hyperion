/**
 * The GPU's clocks for the descent spike (plan R05, T14.k; decision-r05-trace-windows-2.md,
 * addendum B, ruling 2), read by the 1 Hz memory sampler beside the memory, from what an
 * unprivileged process can read on the run's GPU.
 *
 * @remarks
 * A GPU row measures each pass at the clocks the driver chose for the spike's load, so the clocks
 * are recorded beside the rows and never pinned or normalised. The sources:
 *
 * - NVIDIA, under Linux and Windows: the `nvidia-smi -q -x` reading the sampler already takes each
 *   second (`fdinfo.ts`), with its graphics and memory clocks, its maximum graphics clock and its
 *   performance state.
 * - Intel under Linux (i915, the owner's UHD 620): the run's DRM card's `gt_act_freq_mhz`, the
 *   clock the GPU runs at, against `gt_RP0_freq_mhz`, the highest the hardware runs at, or
 *   `gt_boost_freq_mhz` where RP0's file is absent (the kernel's i915 sysfs ABI). The GPU shares
 *   the system's memory, so there is no memory clock, and i915 has no performance states. On a
 *   15 W part the clock also moves with the package's shared power budget, so the CPU's load
 *   shows in it.
 * - AMD under Linux (amdgpu): the card's `device/pp_dpm_sclk` and `device/pp_dpm_mclk`, each a
 *   list of clock levels with the current one marked `*` (the kernel's amdgpu documentation), the
 *   highest graphics level being the maximum. Its levels are not performance states.
 * - Elsewhere there is no unprivileged reading: on macOS `powermetrics` needs root, and Windows
 *   has none for a GPU other than NVIDIA's. The clocks are then null with the reason.
 *
 * The run's DRM card is the one whose PCI vendor and device (`device/vendor`, `device/device`)
 * are those of the GPU Chromium reports as active. A missing reading never fails a run: a file
 * that is absent or unreadable makes its reading null with the path in the reason.
 */

import { readdir, readFile } from "node:fs/promises";

import type { NvidiaReading } from "./fdinfo";
import { type Measured, measured, missing } from "./measured";

/** Where a run's GPU clocks are read. */
export type GpuClockSource = "nvidia-smi" | "i915-sysfs" | "amdgpu-sysfs";

/** The GPU's clocks at one sample, each reading present or missing with its reason. */
export interface GpuClockReadings {
  readonly kind: "clocks";
  readonly source: GpuClockSource;
  /** The GPU's maximum graphics clock, whole MHz. */
  readonly maxGraphicsMHz: Measured<number>;
  /** The graphics clock now, whole MHz. */
  readonly graphicsMHz: Measured<number>;
  /** The memory clock now, whole MHz. */
  readonly memoryMHz: Measured<number>;
  /** The performance state now, its number (P0, the highest, is 0). */
  readonly performanceState: Measured<number>;
}

/** One sample of the GPU's clocks, or why the run's GPU has no clock reading. */
export type GpuClockSample =
  GpuClockReadings | { readonly kind: "unavailable"; readonly reason: string };

/** The run's GPU by its PCI IDs, as Chromium reports the active one. */
export interface GpuIdentity {
  readonly vendorId: number;
  readonly deviceId: number;
}

/** The file system calls the sysfs readers make, for tests. */
export interface SysfsFiles {
  readdir(path: string): Promise<ReadonlyArray<string>>;
  readFile(path: string): Promise<string>;
}

const NODE_FILES: SysfsFiles = {
  readdir: (path) => readdir(path),
  readFile: (path) => readFile(path, "utf8"),
};

/** The PCI vendor IDs of the GPUs with a clock reader. */
const PCI_VENDOR = { nvidia: 0x10_de, intel: 0x80_86, amd: 0x10_02 } as const;

/** Why macOS has no clock reading (addendum B, ruling 2). */
export const DARWIN_CLOCKS_REASON =
  "no unprivileged GPU clock reading on darwin; powermetrics needs root";

/** Why Windows has no clock reading for a GPU other than NVIDIA's. */
export const WIN32_CLOCKS_REASON =
  "no unprivileged GPU clock reading on win32 for a non-NVIDIA GPU";

/** Why i915 has no memory clock. */
export const I915_MEMORY_REASON = "i915 gives no memory clock: the GPU shares the system's memory";

/** Why i915 has no performance state. */
export const I915_STATE_REASON = "i915 has no performance states";

/** Why amdgpu has no performance state. */
export const AMDGPU_STATE_REASON = "amdgpu gives clock levels, not performance states";

/** A PCI ID as sysfs and `lspci` write it, `0x` and four hex digits. */
function hex(id: number): string {
  return `0x${id.toString(16).padStart(4, "0")}`;
}

function failureOf(error: unknown): string {
  if (error instanceof Error && "code" in error && typeof error.code === "string") {
    return error.code;
  }
  return error instanceof Error ? error.message : String(error);
}

/** A sysfs file's text, or why it could not be read, its path named. */
async function readSysfs(files: SysfsFiles, path: string): Promise<Measured<string>> {
  try {
    return measured(await files.readFile(path));
  } catch (error: unknown) {
    return missing(`${path} could not be read (${failureOf(error)})`);
  }
}

/** A sysfs file holding one whole number of MHz, as i915's frequency files do. */
async function readMHz(files: SysfsFiles, path: string): Promise<Measured<number>> {
  const text = await readSysfs(files, path);
  if (text.value === null) {
    return missing(text.reason);
  }
  const trimmed = text.value.trim();
  return /^\d+$/.test(trimmed)
    ? measured(Number(trimmed))
    : missing(`${path} holds no whole MHz: "${trimmed}"`);
}

/** One of amdgpu's `pp_dpm_*` lists: its levels' clocks in its order, and the current one's. */
export interface DpmLevels {
  /** Each level's clock, whole MHz, in the file's order (lowest first). */
  readonly levelsMHz: ReadonlyArray<number>;
  /** The level marked `*`, MHz, or `null` when none is. */
  readonly currentMHz: number | null;
}

/**
 * Parses an amdgpu `pp_dpm_sclk` or `pp_dpm_mclk`: one level a line, `<index>: <clock>Mhz`, the
 * current one followed by `*` (an APU's sleep level is indexed `S`).
 */
export function parseDpmLevels(text: string): DpmLevels {
  const levelsMHz: number[] = [];
  let currentMHz: number | null = null;
  for (const line of text.split("\n")) {
    const match = /^\s*\w+:\s*(\d+)\s*mhz\s*(\*)?\s*$/i.exec(line);
    if (match === null) {
      continue;
    }
    const mhz = Number(match[1]);
    levelsMHz.push(mhz);
    if (match[2] !== undefined) {
      currentMHz = mhz;
    }
  }
  return { levelsMHz, currentMHz };
}

/** A `pp_dpm_*` file's levels, or why it has none. */
async function readDpm(files: SysfsFiles, path: string): Promise<Measured<DpmLevels>> {
  const text = await readSysfs(files, path);
  if (text.value === null) {
    return missing(text.reason);
  }
  const levels = parseDpmLevels(text.value);
  return levels.levelsMHz.length === 0 ? missing(`${path} lists no clock level`) : measured(levels);
}

function currentOf(levels: Measured<DpmLevels>, path: string): Measured<number> {
  if (levels.value === null) {
    return missing(levels.reason);
  }
  return levels.value.currentMHz === null
    ? missing(`no level of ${path} is marked current`)
    : measured(levels.value.currentMHz);
}

/** One of `nvidia-smi`'s readings, `element` naming it in the reason when it is `N/A`. */
function nvidiaReading(value: number | null, element: string): Measured<number> {
  return value === null ? missing(`nvidia-smi gives no ${element} (N/A)`) : measured(value);
}

/**
 * The clocks in a `nvidia-smi -q -x` reading: its first GPU's, as the memory sampler reads its
 * memory; each missing with the reason where the reading is.
 */
export function nvidiaClocks(reading: NvidiaReading): GpuClockReadings {
  const gpu = reading.kind === "nvidia" ? reading.gpus[0] : undefined;
  if (gpu === undefined) {
    const none = missing(reading.kind === "nvidia" ? "nvidia-smi lists no GPU" : reading.reason);
    return {
      kind: "clocks",
      source: "nvidia-smi",
      maxGraphicsMHz: none,
      graphicsMHz: none,
      memoryMHz: none,
      performanceState: none,
    };
  }
  const { clocks } = gpu;
  return {
    kind: "clocks",
    source: "nvidia-smi",
    maxGraphicsMHz: nvidiaReading(clocks.maxGraphicsMHz, "max_clocks/graphics_clock"),
    graphicsMHz: nvidiaReading(clocks.graphicsMHz, "clocks/graphics_clock"),
    memoryMHz: nvidiaReading(clocks.memoryMHz, "clocks/mem_clock"),
    performanceState: nvidiaReading(clocks.performanceState, "performance_state"),
  };
}

/** An i915 card's clocks: the actual graphics clock against RP0, or boost without RP0's file. */
async function i915Clocks(files: SysfsFiles, card: string): Promise<GpuClockReadings> {
  const [graphicsMHz, rp0MHz] = await Promise.all([
    readMHz(files, `${card}/gt_act_freq_mhz`),
    readMHz(files, `${card}/gt_RP0_freq_mhz`),
  ]);
  let maxGraphicsMHz = rp0MHz;
  if (rp0MHz.value === null) {
    const boostMHz = await readMHz(files, `${card}/gt_boost_freq_mhz`);
    maxGraphicsMHz =
      boostMHz.value === null ? missing(`${rp0MHz.reason}; ${boostMHz.reason}`) : boostMHz;
  }
  return {
    kind: "clocks",
    source: "i915-sysfs",
    maxGraphicsMHz,
    graphicsMHz,
    memoryMHz: missing(I915_MEMORY_REASON),
    performanceState: missing(I915_STATE_REASON),
  };
}

/** An amdgpu card's clocks: the current graphics and memory levels, against the highest. */
async function amdgpuClocks(files: SysfsFiles, card: string): Promise<GpuClockReadings> {
  const sclkPath = `${card}/device/pp_dpm_sclk`;
  const mclkPath = `${card}/device/pp_dpm_mclk`;
  const [sclk, mclk] = await Promise.all([readDpm(files, sclkPath), readDpm(files, mclkPath)]);
  return {
    kind: "clocks",
    source: "amdgpu-sysfs",
    maxGraphicsMHz:
      sclk.value === null ? missing(sclk.reason) : measured(Math.max(...sclk.value.levelsMHz)),
    graphicsMHz: currentOf(sclk, sclkPath),
    memoryMHz: currentOf(mclk, mclkPath),
    performanceState: missing(AMDGPU_STATE_REASON),
  };
}

/** A sysfs file holding a hex ID (`0x8086`), or `undefined`. */
async function readHexId(files: SysfsFiles, path: string): Promise<number | undefined> {
  const text = await readSysfs(files, path);
  const match = /^0x([0-9a-f]+)$/i.exec(text.value?.trim() ?? "");
  return match === null ? undefined : Number.parseInt(match[1] ?? "", 16);
}

/**
 * The DRM card of the run's GPU: the `card<N>` under `<sysRoot>/class/drm` whose PCI vendor and
 * device are `gpu`'s, the lowest-numbered if several are.
 */
async function findCard(
  files: SysfsFiles,
  sysRoot: string,
  gpu: GpuIdentity,
): Promise<Measured<string>> {
  const drm = `${sysRoot}/class/drm`;
  let entries: ReadonlyArray<string>;
  try {
    entries = await files.readdir(drm);
  } catch (error: unknown) {
    return missing(`${drm} could not be read (${failureOf(error)})`);
  }
  const cards = entries
    .filter((name) => /^card\d+$/.test(name))
    .toSorted((a, b) => Number(a.slice(4)) - Number(b.slice(4)));
  const ids = await Promise.all(
    cards.map(async (name) => {
      const dir = `${drm}/${name}`;
      const [vendorId, deviceId] = await Promise.all([
        readHexId(files, `${dir}/device/vendor`),
        readHexId(files, `${dir}/device/device`),
      ]);
      return { dir, vendorId, deviceId };
    }),
  );
  const card = ids.find(
    ({ vendorId, deviceId }) => vendorId === gpu.vendorId && deviceId === gpu.deviceId,
  );
  return card === undefined
    ? missing(
        `no DRM card under ${drm} is the run's GPU (${hex(gpu.vendorId)}:${hex(gpu.deviceId)})`,
      )
    : measured(card.dir);
}

/** Where the run's clocks are read, chosen once. */
type ClockSource =
  | { readonly kind: "nvidia-smi" }
  | { readonly kind: "i915-sysfs" | "amdgpu-sysfs"; readonly card: string }
  | { readonly kind: "none"; readonly reason: string };

/** What a {@link GpuClockReader} is given. */
export interface GpuClockReaderOptions {
  /** The platform the run is on, `process.platform` outside a test. */
  readonly platform: NodeJS.Platform;
  /** The run's GPU, as Chromium reports the active one; asked once, at the first reading. */
  readonly gpu: () => Promise<Measured<GpuIdentity>>;
  /** The file operations, Node's by default. */
  readonly files?: SysfsFiles;
  /** Where sysfs is mounted: `/sys`, or a fixture tree in a test. */
  readonly sysRoot?: string;
}

/** The source of the run's clocks: by its platform and its GPU's vendor (the module's remarks). */
async function chooseSource(options: GpuClockReaderOptions): Promise<ClockSource> {
  const { platform } = options;
  if (platform === "darwin") {
    return { kind: "none", reason: DARWIN_CLOCKS_REASON };
  }
  let gpu: Measured<GpuIdentity>;
  try {
    gpu = await options.gpu();
  } catch (error: unknown) {
    gpu = missing(`Chromium's GPU information failed: ${failureOf(error)}`);
  }
  if (gpu.value === null) {
    return { kind: "none", reason: `the run's GPU is not known: ${gpu.reason}` };
  }
  const { vendorId } = gpu.value;
  if (vendorId === PCI_VENDOR.nvidia) {
    return { kind: "nvidia-smi" };
  }
  if (platform === "win32") {
    return { kind: "none", reason: WIN32_CLOCKS_REASON };
  }
  if (platform !== "linux") {
    return { kind: "none", reason: `no unprivileged GPU clock reading on ${platform}` };
  }
  const kind =
    vendorId === PCI_VENDOR.intel
      ? "i915-sysfs"
      : vendorId === PCI_VENDOR.amd
        ? "amdgpu-sysfs"
        : null;
  if (kind === null) {
    return { kind: "none", reason: `no GPU clock reader for the vendor ${hex(vendorId)} on linux` };
  }
  const card = await findCard(options.files ?? NODE_FILES, options.sysRoot ?? "/sys", gpu.value);
  return card.value === null ? { kind: "none", reason: card.reason } : { kind, card: card.value };
}

/**
 * Reads the GPU's clocks for the memory sampler: from its own `nvidia-smi` reading on NVIDIA, from
 * the run's DRM card's sysfs files under Linux, and nothing elsewhere (the module's remarks).
 *
 * @remarks
 * The source is chosen at the first reading, once the run's GPU is known, and kept for the run.
 */
export class GpuClockReader {
  readonly #options: GpuClockReaderOptions;
  #source: Promise<ClockSource> | null = null;

  constructor(options: GpuClockReaderOptions) {
    this.#options = options;
  }

  /**
   * One sample of the clocks; never rejects, a reading it cannot take being missing with the
   * reason.
   *
   * @param nvidia - The sampler's `nvidia-smi -q -x` reading of this sample, which NVIDIA's clocks
   * are read from.
   */
  async read(nvidia: NvidiaReading): Promise<GpuClockSample> {
    this.#source ??= chooseSource(this.#options);
    const source = await this.#source;
    const files = this.#options.files ?? NODE_FILES;
    let sample: GpuClockSample;
    switch (source.kind) {
      case "nvidia-smi":
        sample = nvidiaClocks(nvidia);
        break;
      case "i915-sysfs":
        sample = await i915Clocks(files, source.card);
        break;
      case "amdgpu-sysfs":
        sample = await amdgpuClocks(files, source.card);
        break;
      case "none":
        sample = { kind: "unavailable", reason: source.reason };
        break;
    }
    return sample;
  }
}
