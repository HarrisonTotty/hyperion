/**
 * Perf mode's samplers (plan R12, R12.T2; Design notes 6 and 7): the machine's thermal state,
 * throttling and clocks, sampled at 10 Hz by {@link PerfSampler} and summarised per repetition as
 * the record's `ThrottleFigures`.
 *
 * @remarks
 * Every reading is what an unprivileged process can read, and a missing file is `null` with the
 * path in its reason, never a guess. Under Linux the readings are the kernel's sysfs files, as its
 * ABI documents them (`Documentation/ABI/testing/sysfs-devices-system-cpu`, `sysfs-class-power`,
 * `Documentation/driver-api/thermal/sysfs-api.rst`, `Documentation/hwmon/k10temp.rst`,
 * `Documentation/power/powercap/powercap.rst` and i915's `gt_*_freq_mhz`):
 *
 * - the package temperature: the `temp` (millidegrees Celsius) of the thermal zone whose `type` is
 *   `x86_pkg_temp`, else `k10temp`'s `Tctl` on an AMD machine such as the development desktop
 *   (hardware item 2, decided 2026-09-30), with every zone's temperature beside it;
 * - throttling: each CPU's `thermal_throttle/{package,core}_throttle_total_time_ms`;
 * - the CPUs' clocks: each CPU's `cpufreq/scaling_cur_freq` (kHz);
 * - the GPU's clock: i915's `gt_act_freq_mhz` against `gt_RP0_freq_mhz` (`gpuClocks.ts`), or on
 *   NVIDIA a long-lived `nvidia-smi` child ({@link NvidiaClockStream}), whose clock event reasons
 *   stand in for i915's files (hardware item 2);
 * - package power: RAPL's `energy_uj`, root-only since the kernel's fix for CVE-2020-8694, so
 *   `null` unless the owner makes it group-readable. That is optional, never required, and is one
 *   udev rule, such as
 *   `SUBSYSTEM=="powercap", ACTION=="add", RUN+="/bin/chmod g+r /sys%p/energy_uj", RUN+="/bin/chgrp <group> /sys%p/energy_uj"`.
 *
 * Off Linux each `/sys` reading is missing with its reason rather than failing (R12.T2's platform
 * audit, 2026-11-13); on macOS {@link sampleThermal} reads the thermal pressure from `pmset -g
 * therm`, which needs no root, as `scripts/macProfile.mjs` does, and nothing needs `powermetrics`.
 * The platform is one switch in {@link sampleThermal} and one in {@link sampleThrottle} (the M5
 * Mac's lean, pending the owner). Nothing here runs outside a perf run: the runner (R12.T4.a)
 * starts the sampler.
 *
 * The memory samplers (R12.T3.b; Design note 6), read at 1 Hz by the runner: the GPU process's
 * DRM fdinfo ({@link sampleGpuMemory}, over R05's `fdinfo.ts`), summed per distinct client with
 * the lower bound Σresident − ½·Σshared beside it; the display server's ({@link
 * sampleDisplayServer}, Xorg's fdinfo where it is readable); `nvidia-smi -q -x`'s `process_info`
 * of the GPU process and the device's `memory.used` less a baseline taken before launch
 * ({@link sampleNvidia}), the headline on NVIDIA, whose driver writes no DRM keys, where the
 * fdinfo readings are `null` with their reason (hardware item 2); and `app.getAppMetrics()`'s
 * memory per process ({@link appMemoryOf}). The kernel's peak leaves out the samples within
 * {@link RELEASE_SETTLE_MS} of a large release in the tally ({@link summariseDrm}), since Dawn
 * frees a destroyed resource on a later device tick.
 */

import { execFile, spawn } from "node:child_process";
import { readdir, readFile } from "node:fs/promises";
import { createInterface } from "node:readline";

import {
  type DisplayServerFigures,
  type GpuMemoryFigures,
  median,
  type ThrottleFigures,
} from "@hyperion/perf";

import {
  type DrmClient,
  type FdinfoFiles,
  type NvidiaReading,
  readDrmClients,
  readNvidiaSmi,
  type RunCommand,
} from "../fdinfo";
import { GpuClockReader, type GpuIdentity, type SysfsFiles } from "../gpuClocks";
import { type Measured, measured, missing } from "../measured";

/** What the samplers read, injected so that a test reads a fixture tree on any platform. */
export interface SamplerSources {
  /** `process.platform` outside a test. */
  readonly platform: NodeJS.Platform;
  readonly files: SysfsFiles;
  /** Where sysfs is mounted: `/sys`, or a fixture tree in a test. */
  readonly sysRoot: string;
  /** Runs a command and resolves its standard output (`pmset`, `tlp-stat`). */
  readonly run: RunCommand;
}

/** Node's file system. */
export const NODE_FILES: SysfsFiles = {
  readdir: (path) => readdir(path),
  readFile: (path) => readFile(path, "utf8"),
};

/** How long a sampled command (`pmset`, `tlp-stat`) may take before it is given up, ms. */
const COMMAND_TIMEOUT_MS = 5000;

/** Runs a command with {@link COMMAND_TIMEOUT_MS}'s timeout and resolves its standard output. */
export const RUN_COMMAND: RunCommand = (file, args) =>
  new Promise((resolve, reject) => {
    execFile(file, [...args], { timeout: COMMAND_TIMEOUT_MS }, (error, stdout) => {
      if (error === null) {
        resolve(stdout);
      } else {
        reject(error);
      }
    });
  });

/** This process's sources: Node's file system, `/sys` and `execFile`. */
export function nodeSamplerSources(): SamplerSources {
  return { platform: process.platform, files: NODE_FILES, sysRoot: "/sys", run: RUN_COMMAND };
}

/** An error's code (`ENOENT`, `EACCES`) where it has one, else its message. */
export function failureOf(error: unknown): string {
  if (error instanceof Error && "code" in error && typeof error.code === "string") {
    return error.code;
  }
  return error instanceof Error ? error.message : String(error);
}

/** A file's text, trimmed, or why it could not be read, its path named. */
export async function readText(files: SysfsFiles, path: string): Promise<Measured<string>> {
  try {
    return measured((await files.readFile(path)).trim());
  } catch (error: unknown) {
    return missing(`${path} could not be read (${failureOf(error)})`);
  }
}

/** A file holding one finite number, or why it holds none. */
export async function readNumber(files: SysfsFiles, path: string): Promise<Measured<number>> {
  const text = await readText(files, path);
  if (text.value === null) {
    return missing(text.reason);
  }
  const value = Number(text.value);
  return text.value.length > 0 && Number.isFinite(value)
    ? measured(value)
    : missing(`${path} holds no number: "${text.value}"`);
}

/** A directory's entries, or why it could not be listed. */
export async function listDir(
  files: SysfsFiles,
  path: string,
): Promise<Measured<ReadonlyArray<string>>> {
  try {
    return measured(await files.readdir(path));
  } catch (error: unknown) {
    return missing(`${path} could not be listed (${failureOf(error)})`);
  }
}

/** The platforms the samplers tell apart: Linux's `/sys`, macOS's `pmset`, and the rest. */
export type SamplerPlatform = "linux" | "darwin" | "other";

/** `platform` as the samplers tell it apart: the one switch on the platform (the module's remarks). */
export function samplerPlatformOf(platform: NodeJS.Platform): SamplerPlatform {
  return platform === "linux" || platform === "darwin" ? platform : "other";
}

/** Why a `/sys` reading is missing off Linux. */
export function notLinuxReason(platform: NodeJS.Platform): string {
  return `/sys is Linux's; this is ${platform}`;
}

/** The name of a numbered directory's number (`cpu12` is 12), for sorting. */
function numberOf(name: string): number {
  return Number(/\d+$/.exec(name)?.[0] ?? Number.NaN);
}

/** The entries of `names` that `pattern` matches, by their number. */
function numbered(names: ReadonlyArray<string>, pattern: RegExp): ReadonlyArray<string> {
  return names.filter((name) => pattern.test(name)).toSorted((a, b) => numberOf(a) - numberOf(b));
}

// --- Thermal ---

/** One thermal zone's temperature. */
export interface ZoneReading {
  /** The zone's directory, `thermal_zone<N>`. */
  readonly zone: string;
  /** Its `type`, such as `x86_pkg_temp` or `acpitz`. */
  readonly type: string;
  readonly degC: number;
}

/**
 * Every thermal zone's temperature under `<sysRoot>/class/thermal`; a zone whose `temp` cannot be
 * read (a sensor that is down) is left out.
 */
export async function readThermalZones(
  files: SysfsFiles,
  sysRoot: string,
): Promise<Measured<ReadonlyArray<ZoneReading>>> {
  const dir = `${sysRoot}/class/thermal`;
  const entries = await listDir(files, dir);
  if (entries.value === null) {
    return missing(entries.reason);
  }
  const zones = await Promise.all(
    numbered(entries.value, /^thermal_zone\d+$/).map(async (zone) => {
      const [type, milli] = await Promise.all([
        readText(files, `${dir}/${zone}/type`),
        readNumber(files, `${dir}/${zone}/temp`),
      ]);
      return type.value === null || milli.value === null
        ? null
        : { zone, type: type.value, degC: milli.value / 1000 };
    }),
  );
  return measured(zones.filter((zone): zone is ZoneReading => zone !== null));
}

/**
 * `k10temp`'s `Tctl`, °C: the `temp<N>_input` (millidegrees) whose `temp<N>_label` is `Tctl`, of the
 * hwmon device whose `name` is `k10temp`.
 */
export async function readK10Tctl(files: SysfsFiles, sysRoot: string): Promise<Measured<number>> {
  const dir = `${sysRoot}/class/hwmon`;
  const entries = await listDir(files, dir);
  if (entries.value === null) {
    return missing(entries.reason);
  }
  const hwmons = numbered(entries.value, /^hwmon\d+$/);
  const names = await Promise.all(hwmons.map((hwmon) => readText(files, `${dir}/${hwmon}/name`)));
  const hwmon = hwmons.find((_, i) => names[i]?.value === "k10temp");
  if (hwmon === undefined) {
    return missing(`no k10temp device under ${dir}`);
  }
  const sensors = numbered(
    ((await listDir(files, `${dir}/${hwmon}`)).value ?? [])
      .filter((file) => file.endsWith("_label"))
      .map((file) => file.slice(0, -"_label".length)),
    /^temp\d+$/,
  );
  const labels = await Promise.all(
    sensors.map((sensor) => readText(files, `${dir}/${hwmon}/${sensor}_label`)),
  );
  const tctl = sensors.find((_, i) => labels[i]?.value === "Tctl");
  if (tctl === undefined) {
    return missing(`${dir}/${hwmon} (k10temp) has no Tctl sensor`);
  }
  const milli = await readNumber(files, `${dir}/${hwmon}/${tctl}_input`);
  return milli.value === null ? missing(milli.reason) : measured(milli.value / 1000);
}

/** The package temperature's source: the zone or sensor read (`PowerThermalRecord`). */
export type PackageTemperatureSource = "x86_pkg_temp" | "k10temp Tctl";

/** The package temperature and where it was read. */
export interface PackageTemperature {
  readonly degC: number;
  readonly source: PackageTemperatureSource;
}

/**
 * The package temperature: the hottest `x86_pkg_temp` zone (one a package), else `k10temp`'s
 * `Tctl` (hardware item 2), else missing with both reasons.
 */
export async function readPackageTemperature(
  files: SysfsFiles,
  sysRoot: string,
  zones: Measured<ReadonlyArray<ZoneReading>>,
): Promise<Measured<PackageTemperature>> {
  const packages = (zones.value ?? []).filter((zone) => zone.type === "x86_pkg_temp");
  if (packages.length > 0) {
    return measured({
      degC: Math.max(...packages.map((zone) => zone.degC)),
      source: "x86_pkg_temp",
    });
  }
  const tctl = await readK10Tctl(files, sysRoot);
  if (tctl.value !== null) {
    return measured({ degC: tctl.value, source: "k10temp Tctl" });
  }
  const zoneReason = zones.value === null ? zones.reason : "no x86_pkg_temp thermal zone";
  return missing(`${zoneReason}; ${tctl.reason}`);
}

/** The RAPL zones read, the first present winning (Design note 7). */
export const RAPL_ZONES = ["intel-rapl-mmio:0", "intel-rapl:0"] as const;

/** A RAPL zone's name, as {@link RAPL_ZONES} lists them. */
export type RaplZone = (typeof RAPL_ZONES)[number];

/** The first of {@link RAPL_ZONES} under `<sysRoot>/class/powercap`, or why there is none. */
export async function findRaplZone(
  files: SysfsFiles,
  sysRoot: string,
): Promise<Measured<{ readonly zone: RaplZone; readonly dir: string }>> {
  const found = await Promise.all(
    RAPL_ZONES.map(async (zone) => {
      const dir = `${sysRoot}/class/powercap/${zone}`;
      return { zone, dir, entries: await listDir(files, dir) };
    }),
  );
  const first = found.find(({ entries }) => entries.value !== null);
  return first === undefined
    ? missing(found.map(({ entries }) => entries.reason).join("; "))
    : measured({ zone: first.zone, dir: first.dir });
}

/** The package's energy counter now, µJ, and its range, for a power over two readings. */
export interface EnergyReading {
  readonly energyUj: number;
  /** `max_energy_range_uj`, the counter's wrap; `null` where unread. */
  readonly rangeUj: number | null;
}

/** RAPL's package energy counter, or why it is unread: root-only unless the owner opened it. */
async function readPackageEnergy(
  files: SysfsFiles,
  sysRoot: string,
): Promise<Measured<EnergyReading>> {
  const zone = await findRaplZone(files, sysRoot);
  if (zone.value === null) {
    return missing(zone.reason);
  }
  const [energy, range] = await Promise.all([
    readNumber(files, `${zone.value.dir}/energy_uj`),
    readNumber(files, `${zone.value.dir}/max_energy_range_uj`),
  ]);
  if (energy.value === null) {
    return missing(
      `${energy.reason}: the package energy counter is root-only (the module's remarks)`,
    );
  }
  return measured({ energyUj: energy.value, rangeUj: range.value });
}

/** macOS's thermal state from `pmset -g therm` (no root). */
export interface ThermalPressure {
  /** No warning level above 0 and no CPU limit below 100%. */
  readonly nominal: boolean;
  /** The report's lines other than its "Note: No … has been recorded", joined; empty when none. */
  readonly detail: string;
  /** `CPU_Speed_Limit`, percent, where the report has one. */
  readonly cpuSpeedLimitPercent: number | null;
}

/**
 * Parses `pmset -g therm`: "Note: No thermal warning level has been recorded" and its siblings
 * where nothing was, else a warning level ("… warning level set to N") and the CPU power status
 * (`CPU_Scheduler_Limit`, `CPU_Available_CPUs`, `CPU_Speed_Limit`). A warning level above 0 or a
 * scheduler or speed limit below 100 is pressure; a line of another form is kept in the detail and
 * judges nothing.
 */
export function parsePmsetTherm(text: string): ThermalPressure {
  const lines = text
    .split("\n")
    .map((line) => line.replaceAll(/\s+/g, " ").trim())
    .filter((line) => line.length > 0 && !/^Note: No .* has been recorded\.?$/i.test(line));
  let nominal = true;
  let cpuSpeedLimitPercent: number | null = null;
  for (const line of lines) {
    const level = /warning level\D*(\d+)/i.exec(line);
    if (level !== null && Number(level[1]) > 0) {
      nominal = false;
    }
    const limit = /^CPU_(Scheduler|Speed)_Limit = (\d+)/.exec(line);
    if (limit !== null) {
      const percent = Number(limit[2]);
      if (limit[1] === "Speed") {
        cpuSpeedLimitPercent = percent;
      }
      if (percent < 100) {
        nominal = false;
      }
    }
  }
  return { nominal, detail: lines.join("; "), cpuSpeedLimitPercent };
}

/** macOS's thermal pressure now, or why it is unread. */
export async function readThermalPressure(run: RunCommand): Promise<Measured<ThermalPressure>> {
  try {
    return measured(parsePmsetTherm(await run("pmset", ["-g", "therm"])));
  } catch (error: unknown) {
    return missing(`pmset -g therm failed (${failureOf(error)})`);
  }
}

/** One 10 Hz sample of the machine's thermal state. */
export interface ThermalSample {
  /** The sample's time, ms on the sampler's clock. */
  readonly atMs: number;
  readonly packageDegC: Measured<number>;
  readonly packageSource: PackageTemperatureSource | null;
  /** Every thermal zone's temperature, by type, the hottest where a type repeats. */
  readonly zonesDegC: Readonly<Record<string, number>>;
  readonly energy: Measured<EnergyReading>;
  /** macOS's thermal pressure; missing elsewhere. */
  readonly pressure: Measured<ThermalPressure>;
}

/** Each zone's temperature by type, the hottest of a type that repeats. */
function zonesByType(zones: ReadonlyArray<ZoneReading>): Readonly<Record<string, number>> {
  const byType: Record<string, number> = {};
  for (const { type, degC } of zones) {
    byType[type] = Math.max(byType[type] ?? Number.NEGATIVE_INFINITY, degC);
  }
  return byType;
}

/** Why the thermal pressure is missing off macOS. */
const PRESSURE_REASON = "the thermal pressure is macOS's (pmset -g therm)";

/**
 * One sample of the thermal state: on Linux the zones, the package temperature and the energy
 * counter; on macOS the thermal pressure, the `/sys` readings missing; elsewhere nothing, each
 * reading missing with its reason. Never rejects.
 */
export async function sampleThermal(sources: SamplerSources, atMs: number): Promise<ThermalSample> {
  const { platform, files, sysRoot } = sources;
  const offLinux = missing(notLinuxReason(platform));
  let zones: Measured<ReadonlyArray<ZoneReading>> = offLinux;
  let packageTemperature: Measured<PackageTemperature> = offLinux;
  let energy: Measured<EnergyReading> = offLinux;
  let pressure: Measured<ThermalPressure> = missing(PRESSURE_REASON);
  switch (samplerPlatformOf(platform)) {
    case "linux":
      zones = await readThermalZones(files, sysRoot);
      [packageTemperature, energy] = await Promise.all([
        readPackageTemperature(files, sysRoot, zones),
        readPackageEnergy(files, sysRoot),
      ]);
      break;
    case "darwin":
      pressure = await readThermalPressure(sources.run);
      break;
    case "other":
      break;
  }
  return {
    atMs,
    packageDegC:
      packageTemperature.value === null
        ? missing(packageTemperature.reason)
        : measured(packageTemperature.value.degC),
    packageSource: packageTemperature.value?.source ?? null,
    zonesDegC: zonesByType(zones.value ?? []),
    energy,
    pressure,
  };
}

// --- Throttling and clocks ---

/** The NVIDIA clock event reasons that are a power or thermal slowdown (`nvidia-smi`'s manual). */
export const NVIDIA_SLOWDOWN_REASONS = [
  "clocks_event_reasons.sw_power_cap",
  "clocks_event_reasons.sw_thermal_slowdown",
  "clocks_event_reasons.hw_slowdown",
  "clocks_event_reasons.hw_thermal_slowdown",
  "clocks_event_reasons.hw_power_brake_slowdown",
] as const;

/**
 * The fields the long-lived `nvidia-smi` child queries, in the order of its CSV columns: the GPU's
 * index, its graphics clock and maximum graphics clock (MHz), the slowdown reasons ("Active" or
 * "Not Active", as driver 615.71.09's `-q -x` writes them in `clocks_event_reasons`) and its
 * temperature (°C).
 */
export const NVIDIA_CLOCK_FIELDS = [
  "index",
  "clocks.gr",
  "clocks.max.gr",
  ...NVIDIA_SLOWDOWN_REASONS,
  "temperature.gpu",
] as const;

/** The long-lived child's arguments: {@link NVIDIA_CLOCK_FIELDS} as CSV every `intervalMs`. */
export function nvidiaClockArgs(intervalMs: number): ReadonlyArray<string> {
  return [
    `--query-gpu=${NVIDIA_CLOCK_FIELDS.join(",")}`,
    "--format=csv,noheader,nounits",
    "-lms",
    String(intervalMs),
  ];
}

/** One line of the long-lived child: one GPU at one moment. */
export interface NvidiaClockLine {
  readonly index: number;
  readonly graphicsMHz: number | null;
  readonly maxGraphicsMHz: number | null;
  /** Any of {@link NVIDIA_SLOWDOWN_REASONS} active; `null` where none is reported. */
  readonly slowdown: boolean | null;
  readonly temperatureDegC: number | null;
}

/** A CSV cell as a number, `null` for `[N/A]`, `[Not Supported]` or anything else. */
function cellNumber(cell: string | undefined): number | null {
  const text = cell?.trim() ?? "";
  const value = Number(text);
  return text.length > 0 && Number.isFinite(value) ? value : null;
}

/**
 * Parses one CSV line of {@link nvidiaClockArgs}' output.
 *
 * @returns The line, or `null` for a line of another shape (an error message).
 */
export function parseNvidiaClockLine(line: string): NvidiaClockLine | null {
  const cells = line.split(",").map((cell) => cell.trim());
  const index = cellNumber(cells[0]);
  if (cells.length !== NVIDIA_CLOCK_FIELDS.length || index === null) {
    return null;
  }
  const reasons = cells.slice(3, 3 + NVIDIA_SLOWDOWN_REASONS.length);
  const known = reasons.filter((cell) => cell === "Active" || cell === "Not Active");
  return {
    index,
    graphicsMHz: cellNumber(cells[1]),
    maxGraphicsMHz: cellNumber(cells[2]),
    slowdown: known.length === 0 ? null : known.includes("Active"),
    temperatureDegC: cellNumber(cells.at(-1)),
  };
}

/** A running child whose output is read a line at a time. */
export interface LineChild {
  /** Ends the child; no line or end is reported after it. */
  stop(): void;
}

/**
 * Starts `file` with `args`, calls `onLine` with each line of its standard output, and `onEnd` once
 * with the reason it ended (it could not start, or it exited), unless it was stopped.
 */
export type SpawnLines = (
  file: string,
  args: ReadonlyArray<string>,
  onLine: (line: string) => void,
  onEnd: (reason: string) => void,
) => LineChild;

/** Node's {@link SpawnLines}: a child process, its standard error kept for the reason. */
export const SPAWN_LINES: SpawnLines = (file, args, onLine, onEnd) => {
  const child = spawn(file, [...args], { stdio: ["ignore", "pipe", "pipe"] });
  let stopped = false;
  let ended = false;
  let stderr = "";
  const end = (reason: string): void => {
    if (!ended && !stopped) {
      ended = true;
      onEnd(reason);
    }
  };
  child.on("error", (error) => {
    end(
      failureOf(error) === "ENOENT"
        ? `no ${file} on this machine`
        : `${file} failed: ${failureOf(error)}`,
    );
  });
  child.on("exit", (code, signal) => {
    end(`${file} exited (${String(code ?? signal)}): ${stderr.trim().slice(-200)}`);
  });
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk: string) => {
    stderr = (stderr + chunk).slice(-2000);
  });
  createInterface({ input: child.stdout }).on("line", (line) => {
    if (!stopped) {
      onLine(line);
    }
  });
  return {
    stop: () => {
      stopped = true;
      child.kill();
    },
  };
};

/** How old the child's latest line may be before it reads as stale, ms: ten of its intervals. */
const NVIDIA_STALE_INTERVALS = 10;

/** What a {@link NvidiaClockStream} is given. */
export interface NvidiaClockStreamOptions {
  /** {@link SPAWN_LINES} outside a test. */
  readonly spawn: SpawnLines;
  /** The child's interval, ms: 100 for the 10 Hz samplers. */
  readonly intervalMs: number;
  /** The GPU read, by `nvidia-smi`'s index: the first, as `nvidiaClocks` reads its first. */
  readonly gpuIndex: number;
  /** The sampler's clock, ms. */
  readonly now: () => number;
}

/**
 * NVIDIA's clocks and slowdown reasons from one long-lived `nvidia-smi` child (hardware item 2),
 * rather than a process a sample: its CSV every {@link NvidiaClockStreamOptions.intervalMs}.
 */
export class NvidiaClockStream {
  readonly #options: NvidiaClockStreamOptions;
  #child: LineChild | null = null;
  #latest: { readonly line: NvidiaClockLine; readonly atMs: number } | null = null;
  #ended: string | null = null;

  constructor(options: NvidiaClockStreamOptions) {
    this.#options = options;
  }

  /** Starts the child, once. */
  start(): void {
    if (this.#child !== null || this.#ended !== null) {
      return;
    }
    const { spawn: start, intervalMs, gpuIndex, now } = this.#options;
    this.#child = start(
      "nvidia-smi",
      nvidiaClockArgs(intervalMs),
      (text) => {
        const line = parseNvidiaClockLine(text);
        if (line?.index === gpuIndex) {
          this.#latest = { line, atMs: now() };
        }
      },
      (reason) => {
        this.#ended = reason;
        this.#child = null;
      },
    );
  }

  /** The latest line, or why there is none: not started, ended, or stale. */
  latest(): Measured<NvidiaClockLine> {
    const { intervalMs, now } = this.#options;
    if (this.#ended !== null) {
      return missing(this.#ended);
    }
    if (this.#latest === null) {
      return missing(
        this.#child === null ? "nvidia-smi was not started" : "no line from nvidia-smi yet",
      );
    }
    const ageMs = now() - this.#latest.atMs;
    return ageMs > NVIDIA_STALE_INTERVALS * intervalMs
      ? missing(`nvidia-smi's latest line is ${Math.round(ageMs)} ms old`)
      : measured(this.#latest.line);
  }

  /** Ends the child. */
  stop(): void {
    this.#child?.stop();
    this.#child = null;
    this.#ended ??= "stopped";
  }
}

/** The GPU's clock at one sample. */
export interface GpuClockNow {
  readonly source: "i915-sysfs" | "amdgpu-sysfs" | "nvidia-smi";
  readonly mhz: Measured<number>;
  /** RP0 on i915, the highest level on amdgpu, the maximum graphics clock on NVIDIA. */
  readonly maxMHz: Measured<number>;
  /** NVIDIA's slowdown reasons; missing elsewhere. */
  readonly slowdown: Measured<boolean>;
}

/** The GPU's clock at one sample, or why the run's GPU has none. */
export type GpuClockReading = GpuClockNow | { readonly source: "none"; readonly reason: string };

/** No `nvidia-smi -q -x` reading, for `GpuClockReader`, which reads NVIDIA from the stream here. */
const NO_NVIDIA_XML: NvidiaReading = {
  kind: "unavailable",
  reason: "NVIDIA's clocks are read from the long-lived nvidia-smi child",
};

const NVIDIA_VENDOR_ID = 0x10_de;

/** What a {@link GpuClockSampler} is given. */
export interface GpuClockSamplerOptions {
  readonly sources: SamplerSources;
  /** The run's GPU, as Chromium reports the active one, or why it is not known. */
  readonly gpu: Measured<GpuIdentity>;
  /** The long-lived child, for an NVIDIA GPU; `null` elsewhere. */
  readonly nvidia: NvidiaClockStream | null;
}

/**
 * The run's GPU clock: from the long-lived `nvidia-smi` child on NVIDIA, from the DRM card's sysfs
 * files on Intel and AMD under Linux (`gpuClocks.ts`'s reader), and nothing elsewhere.
 */
export class GpuClockSampler {
  readonly #options: GpuClockSamplerOptions;
  readonly #sysfs: GpuClockReader;

  constructor(options: GpuClockSamplerOptions) {
    this.#options = options;
    const { sources, gpu } = options;
    this.#sysfs = new GpuClockReader({
      platform: sources.platform,
      gpu: () => Promise.resolve(gpu),
      files: sources.files,
      sysRoot: sources.sysRoot,
    });
  }

  /** One reading; never rejects. */
  async read(): Promise<GpuClockReading> {
    const { gpu, nvidia } = this.#options;
    if (gpu.value?.vendorId === NVIDIA_VENDOR_ID) {
      if (nvidia === null) {
        return { source: "none", reason: "no nvidia-smi child was started for the NVIDIA GPU" };
      }
      const line = nvidia.latest();
      if (line.value === null) {
        return { source: "none", reason: line.reason };
      }
      const { graphicsMHz, maxGraphicsMHz, slowdown } = line.value;
      return {
        source: "nvidia-smi",
        mhz:
          graphicsMHz === null ? missing("nvidia-smi gives no clocks.gr") : measured(graphicsMHz),
        maxMHz:
          maxGraphicsMHz === null
            ? missing("nvidia-smi gives no clocks.max.gr")
            : measured(maxGraphicsMHz),
        slowdown:
          slowdown === null
            ? missing("nvidia-smi gives no clock event reason")
            : measured(slowdown),
      };
    }
    const sample = await this.#sysfs.read(NO_NVIDIA_XML);
    if (sample.kind === "unavailable") {
      return { source: "none", reason: sample.reason };
    }
    return {
      source: sample.source === "amdgpu-sysfs" ? "amdgpu-sysfs" : "i915-sysfs",
      mhz: sample.graphicsMHz,
      maxMHz: sample.maxGraphicsMHz,
      slowdown: missing(`${sample.source} reports no slowdown reason`),
    };
  }
}

/** One 10 Hz sample of throttling and clocks. */
export interface ThrottleSample {
  /** The sample's time, ms on the sampler's clock. */
  readonly atMs: number;
  /** Each CPU's `package_throttle_total_time_ms`, by its directory (`cpu0`). */
  readonly packageThrottleMs: Readonly<Record<string, number>>;
  /** Each CPU's `core_throttle_total_time_ms`, by its directory. */
  readonly coreThrottleMs: Readonly<Record<string, number>>;
  /** Each CPU's `scaling_cur_freq`, MHz, in CPU order. */
  readonly cpuMHz: ReadonlyArray<number>;
  readonly gpu: GpuClockReading;
  /** Why the CPUs' files are missing, where the CPU directory could not be listed. */
  readonly cpuReason: string | null;
}

/** Each CPU's throttle counters and clock under `<sysRoot>/devices/system/cpu`. */
async function readCpus(
  files: SysfsFiles,
  sysRoot: string,
): Promise<Omit<ThrottleSample, "atMs" | "gpu">> {
  const dir = `${sysRoot}/devices/system/cpu`;
  const entries = await listDir(files, dir);
  if (entries.value === null) {
    return { packageThrottleMs: {}, coreThrottleMs: {}, cpuMHz: [], cpuReason: entries.reason };
  }
  const cpus = numbered(entries.value, /^cpu\d+$/);
  const read = await Promise.all(
    cpus.map((cpu) =>
      Promise.all([
        readNumber(files, `${dir}/${cpu}/thermal_throttle/package_throttle_total_time_ms`),
        readNumber(files, `${dir}/${cpu}/thermal_throttle/core_throttle_total_time_ms`),
        readNumber(files, `${dir}/${cpu}/cpufreq/scaling_cur_freq`),
      ]),
    ),
  );
  const packageThrottleMs: Record<string, number> = {};
  const coreThrottleMs: Record<string, number> = {};
  const cpuMHz: number[] = [];
  read.forEach(([pkg, core, kHz], i) => {
    const cpu = cpus[i] ?? "";
    if (pkg.value !== null) {
      packageThrottleMs[cpu] = pkg.value;
    }
    if (core.value !== null) {
      coreThrottleMs[cpu] = core.value;
    }
    if (kHz.value !== null) {
      cpuMHz.push(kHz.value / 1000);
    }
  });
  return { packageThrottleMs, coreThrottleMs, cpuMHz, cpuReason: null };
}

/**
 * One sample of throttling and clocks: on Linux each CPU's throttle counters and clock and the
 * GPU's clock; elsewhere the GPU's clock alone (NVIDIA's, on Windows), the CPUs' files missing.
 * Never rejects.
 */
export async function sampleThrottle(
  sources: SamplerSources,
  gpu: GpuClockSampler,
  atMs: number,
): Promise<ThrottleSample> {
  let cpus: Promise<Omit<ThrottleSample, "atMs" | "gpu">>;
  switch (samplerPlatformOf(sources.platform)) {
    case "linux":
      cpus = readCpus(sources.files, sources.sysRoot);
      break;
    case "darwin":
    case "other":
      cpus = Promise.resolve({
        packageThrottleMs: {},
        coreThrottleMs: {},
        cpuMHz: [],
        cpuReason: notLinuxReason(sources.platform),
      });
      break;
  }
  const [read, clock] = await Promise.all([cpus, gpu.read()]);
  return { atMs, ...read, gpu: clock };
}

// --- The 10 Hz loop and its summary ---

/** One tick of {@link PerfSampler}. */
export interface PerfSample {
  readonly thermal: ThermalSample;
  readonly throttle: ThrottleSample;
}

/** The 10 Hz interval of Design note 14's thermal, throttle and frequency samplers, ms. */
export const PERF_SAMPLE_INTERVAL_MS = 100;

/**
 * Calls `callback` every `ms` until the returned function is called: Node's `setInterval`, or a
 * test's fake timers.
 */
export type Every = (callback: () => void, ms: number) => () => void;

/** Node's {@link Every}. */
export const NODE_EVERY: Every = (callback, ms) => {
  const handle = setInterval(callback, ms);
  return () => {
    clearInterval(handle);
  };
};

/** What a {@link PerfSampler} is given. */
export interface PerfSamplerOptions {
  readonly sources: SamplerSources;
  readonly gpu: GpuClockSampler;
  /** {@link PERF_SAMPLE_INTERVAL_MS} outside a test. */
  readonly intervalMs: number;
  /** The clock the samples are stamped with, ms: the runner's, which the spans share. */
  readonly now: () => number;
  /** {@link NODE_EVERY} outside a test. */
  readonly every: Every;
}

/**
 * The 10 Hz thermal, throttle and frequency samplers of a run (Design note 14), each sample
 * timestamped so that the runner joins them to a repetition's span by time.
 *
 * @remarks
 * A tick that finds the previous one still reading is skipped, so a slow file system lowers the
 * rate rather than queueing reads; the summary's shares are of samples, which the even interval
 * makes shares of time.
 */
export class PerfSampler {
  readonly #options: PerfSamplerOptions;
  readonly #samples: PerfSample[] = [];
  #cancel: (() => void) | null = null;
  #reading = false;
  #failure: string | null = null;

  constructor(options: PerfSamplerOptions) {
    this.#options = options;
  }

  /** Starts sampling every {@link PerfSamplerOptions.intervalMs}, once. */
  start(): void {
    if (this.#cancel !== null) {
      return;
    }
    this.#cancel = this.#options.every(() => {
      if (this.#reading) {
        return;
      }
      this.#reading = true;
      this.sampleOnce()
        .catch((error: unknown) => {
          this.#failure = failureOf(error);
        })
        .finally(() => {
          this.#reading = false;
        });
    }, this.#options.intervalMs);
  }

  /** Stops sampling; the samples taken are kept. */
  stop(): void {
    this.#cancel?.();
    this.#cancel = null;
  }

  /** Takes and keeps one sample now. */
  async sampleOnce(): Promise<PerfSample> {
    const { sources, gpu, now } = this.#options;
    const atMs = now();
    const [thermal, throttle] = await Promise.all([
      sampleThermal(sources, atMs),
      sampleThrottle(sources, gpu, atMs),
    ]);
    const sample = { thermal, throttle };
    this.#samples.push(sample);
    return sample;
  }

  /** Every sample kept, in time order. */
  get samples(): ReadonlyArray<PerfSample> {
    return this.#samples;
  }

  /** Why a tick failed, which a sample never should; `null` when none did. */
  get failure(): string | null {
    return this.#failure;
  }

  /** The summary of the samples in `[fromMs, toMs]`: a repetition's `ThrottleFigures`. */
  summarise(fromMs: number, toMs: number): ThrottleFigures {
    return summariseSamples(
      this.#samples.filter((s) => s.thermal.atMs >= fromMs && s.thermal.atMs <= toMs),
    );
  }

  /** Drops the samples before `ms`, once their repetition is summarised. */
  discardBefore(ms: number): void {
    const keep = this.#samples.findIndex((sample) => sample.thermal.atMs >= ms);
    this.#samples.splice(0, keep < 0 ? this.#samples.length : keep);
  }
}

/** The most any counter of `key` rose from `first` to `last`; `null` with none in both. */
function counterRise(
  first: Readonly<Record<string, number>>,
  last: Readonly<Record<string, number>>,
): number | null {
  let rise: number | null = null;
  for (const [key, end] of Object.entries(last)) {
    const start = first[key];
    // A counter that fell was reset (a CPU taken offline and back), and says nothing of the span.
    if (start !== undefined && end >= start) {
      rise = Math.max(rise ?? 0, end - start);
    }
  }
  return rise;
}

/** The share of `flags` that are true, or `null` for none. */
function shareOf(flags: ReadonlyArray<boolean>): number | null {
  return flags.length === 0 ? null : flags.filter(Boolean).length / flags.length;
}

/**
 * The package's mean power over `samples`, W: the energy counter's rise over each pair of
 * successive readings, a wrap at `max_energy_range_uj` undone, over the time they span.
 */
function packageWattsOf(samples: ReadonlyArray<ThermalSample>): number | null {
  let energyUj = 0;
  let spanMs = 0;
  for (let i = 1; i < samples.length; i += 1) {
    const before = samples[i - 1];
    const after = samples[i];
    const from = before?.energy.value ?? null;
    const to = after?.energy.value ?? null;
    if (before === undefined || after === undefined || from === null || to === null) {
      continue;
    }
    let rise = to.energyUj - from.energyUj;
    if (rise < 0) {
      if (to.rangeUj === null) {
        continue;
      }
      rise += to.rangeUj;
    }
    energyUj += rise;
    spanMs += after.atMs - before.atMs;
  }
  // µJ over ms is mW; a thousandth of it is W.
  return spanMs > 0 ? energyUj / spanMs / 1000 : null;
}

/**
 * A span's samples as the record's `ThrottleFigures` (Design note 7): the throttle counters' rises
 * and the package's share throttled, the GPU's minimum and median clock and its share at its
 * maximum (RP0), NVIDIA's share slowed, the CPUs' median clock, the hottest package and zone
 * temperatures, and the package's mean power. A figure no sample read is `null`.
 */
export function summariseSamples(samples: ReadonlyArray<PerfSample>): ThrottleFigures {
  const first = samples[0];
  const last = samples.at(-1);
  const spanMs =
    first === undefined || last === undefined ? 0 : last.thermal.atMs - first.thermal.atMs;
  const packageThrottleMs =
    first === undefined || last === undefined || spanMs <= 0
      ? null
      : counterRise(first.throttle.packageThrottleMs, last.throttle.packageThrottleMs);
  const coreThrottleMs =
    first === undefined || last === undefined || spanMs <= 0
      ? null
      : counterRise(first.throttle.coreThrottleMs, last.throttle.coreThrottleMs);
  const gpuMHz: number[] = [];
  const atMax: boolean[] = [];
  const slowed: boolean[] = [];
  const cpuMHz: number[] = [];
  const packageDegC: number[] = [];
  const zoneMaxDegC: Record<string, number> = {};
  for (const { thermal, throttle } of samples) {
    const { gpu } = throttle;
    if (gpu.source !== "none") {
      if (gpu.mhz.value !== null) {
        gpuMHz.push(gpu.mhz.value);
        if (gpu.maxMHz.value !== null) {
          atMax.push(gpu.mhz.value >= gpu.maxMHz.value);
        }
      }
      if (gpu.slowdown.value !== null) {
        slowed.push(gpu.slowdown.value);
      }
    }
    cpuMHz.push(...throttle.cpuMHz);
    if (thermal.packageDegC.value !== null) {
      packageDegC.push(thermal.packageDegC.value);
    }
    for (const [type, degC] of Object.entries(thermal.zonesDegC)) {
      zoneMaxDegC[type] = Math.max(zoneMaxDegC[type] ?? Number.NEGATIVE_INFINITY, degC);
    }
  }
  return {
    packageThrottleMs,
    coreThrottleMs,
    throttleFraction: packageThrottleMs === null ? null : Math.min(1, packageThrottleMs / spanMs),
    rp0Share: shareOf(atMax),
    gpuMinMHz: gpuMHz.length === 0 ? null : Math.min(...gpuMHz),
    gpuMedianMHz: median(gpuMHz),
    gpuThrottleShare: shareOf(slowed),
    cpuMedianMHz: median(cpuMHz),
    maxPackageDegC: packageDegC.length === 0 ? null : Math.max(...packageDegC),
    zoneMaxDegC,
    packageWatts: packageWattsOf(samples.map(({ thermal }) => thermal)),
  };
}

// --- GPU memory, the kernel's and NVIDIA's (R12.T3.b) ---

/** The DRM drivers whose clients count for a GPU vendor, or `null` to count every driver's. */
export function drmDriversOf(vendorId: number): ReadonlyArray<string> | null {
  switch (vendorId) {
    case 0x80_86:
      return ["i915", "xe"];
    case 0x10_02:
      return ["amdgpu"];
    default:
      return null;
  }
}

/** One sample of a process's GPU memory from its DRM fdinfo (Design note 6). */
export interface DrmMemorySample {
  /** The sample's time, ms on the sampler's clock. */
  readonly atMs: number;
  /**
   * Σ `drm-resident-<region>` over the distinct clients, bytes: the headline against the ceiling,
   * an upper bound since a buffer the clients share is counted in each.
   */
  readonly residentBytes: number;
  /** Σ `drm-total-<region>`, `stolen-system0` among them; `null` unless every client gives one. */
  readonly totalBytes: number | null;
  /** Σ `drm-shared-<region>`; `null` where no client gives one. */
  readonly sharedBytes: number | null;
  /** Σresident − ½·Σshared: Design note 6's lower bound, the resident figure where none is shared. */
  readonly residentLowerBoundBytes: number;
  /**
   * Σ of each client's render engine time, ns, cumulative (`drm-engine-render`, amdgpu's
   * `drm-engine-gfx`); `null` where no client gives one.
   */
  readonly busyNs: number | null;
  readonly clients: number;
  readonly drivers: ReadonlyArray<string>;
}

function sumOf(record: Readonly<Record<string, number>>): number {
  let sum = 0;
  for (const value of Object.values(record)) {
    sum += value;
  }
  return sum;
}

/**
 * The memory of `clients` at `atMs`: those of `drivers` (every driver's where `null`; an fdinfo of
 * another driver, such as a second GPU's, is skipped), each distinct client (device and
 * `drm-client-id`) counted once, as the kernel's `drm-usage-stats.rst` says duplicated descriptors
 * share an ID.
 */
export function drmSampleOf(
  clients: ReadonlyArray<DrmClient>,
  drivers: ReadonlyArray<string> | null,
  atMs: number,
): Measured<DrmMemorySample> {
  const distinct = new Map<string, DrmClient>();
  for (const client of clients) {
    if (drivers === null || (client.driver !== null && drivers.includes(client.driver))) {
      distinct.set(`${client.pdev ?? ""}/${client.clientId}`, client);
    }
  }
  const withMemory = [...distinct.values()].filter(
    (client) =>
      Object.keys(client.totalBytes).length > 0 || Object.keys(client.residentBytes).length > 0,
  );
  if (withMemory.length === 0) {
    return missing(
      clients.length === 0
        ? "no DRM client in the process's fdinfo (NVIDIA's driver writes none)"
        : `no DRM client of ${drivers?.join(" or ") ?? "any driver"} reports drm-* memory keys`,
    );
  }
  const residentBytes = withMemory.reduce((sum, client) => sum + sumOf(client.residentBytes), 0);
  const totalBytes = withMemory.every((client) => Object.keys(client.totalBytes).length > 0)
    ? withMemory.reduce((sum, client) => sum + sumOf(client.totalBytes), 0)
    : null;
  const sharing = withMemory.filter((client) => Object.keys(client.sharedBytes).length > 0);
  const sharedBytes =
    sharing.length === 0
      ? null
      : sharing.reduce((sum, client) => sum + sumOf(client.sharedBytes), 0);
  const busy = withMemory
    .map((client) => client.engineNs["render"] ?? client.engineNs["gfx"])
    .filter((ns): ns is number => ns !== undefined);
  return measured({
    atMs,
    residentBytes,
    totalBytes,
    sharedBytes,
    residentLowerBoundBytes: residentBytes - (sharedBytes ?? 0) / 2,
    busyNs: busy.length === 0 ? null : busy.reduce((sum, ns) => sum + ns, 0),
    clients: withMemory.length,
    drivers: [...new Set(withMemory.map(({ driver }) => driver ?? "unknown"))].toSorted(),
  });
}

/** What the memory samplers read. */
export interface MemorySamplerSources {
  readonly platform: NodeJS.Platform;
  readonly files: FdinfoFiles;
  /** Where procfs is mounted, for {@link findDisplayServerPid}: `/proc`, or a test's. */
  readonly procRoot: string;
  /** Runs `nvidia-smi`. */
  readonly run: RunCommand;
}

/** This process's memory sources. */
export function nodeMemorySources(): MemorySamplerSources {
  return { platform: process.platform, files: NODE_FILES, procRoot: "/proc", run: RUN_COMMAND };
}

/**
 * The GPU process's memory from its DRM fdinfo, the clients of `drivers` alone (the GPU's,
 * {@link drmDriversOf}).
 *
 * @returns The sample, or why there is none: off Linux, an unreadable fdinfo, or no client with
 * memory keys, as on NVIDIA.
 */
export async function sampleGpuMemory(
  pid: number,
  drivers: ReadonlyArray<string> | null,
  sources: MemorySamplerSources,
  atMs: number,
): Promise<Measured<DrmMemorySample>> {
  const read = await readDrmClients(pid, sources.platform, sources.files);
  return read.kind === "unavailable"
    ? missing(read.reason)
    : drmSampleOf(read.clients, drivers, atMs);
}

/** The X server's process ID: the first process whose `comm` is `Xorg`, or `null` with none. */
export async function findDisplayServerPid(sources: MemorySamplerSources): Promise<number | null> {
  if (sources.platform !== "linux") {
    return null;
  }
  const entries = await listDir(sources.files, sources.procRoot);
  const pids = numbered(
    (entries.value ?? []).filter((name) => /^\d+$/.test(name)),
    /^\d+$/,
  );
  const names = await Promise.all(
    pids.map((pid) => readText(sources.files, `${sources.procRoot}/${pid}/comm`)),
  );
  const found = pids.find((_, i) => names[i]?.value === "Xorg");
  return found === undefined ? null : Number(found);
}

/**
 * The display server's memory and render time from Xorg's fdinfo, readable under X11 where Xorg
 * runs as the user (Design note 6's "display server" line); `null` where there is no X server or
 * its fdinfo cannot be read, as when it runs as root.
 */
export async function sampleDisplayServer(
  pid: number | null,
  drivers: ReadonlyArray<string> | null,
  sources: MemorySamplerSources,
  atMs: number,
): Promise<DrmMemorySample | null> {
  if (pid === null) {
    return null;
  }
  return (await sampleGpuMemory(pid, drivers, sources, atMs)).value;
}

/** A release the tally saw, which the kernel's figure follows a device tick later. */
export interface TallyRelease {
  /** When the renderer's tally saw it, ms on the sampler's clock. */
  readonly atMs: number;
  readonly bytes: number;
}

/**
 * How long after a large release the kernel's peak ignores its samples, ms: Dawn frees a destroyed
 * resource on a later device tick (Design note 6).
 */
export const RELEASE_SETTLE_MS = 2000;

/**
 * The smallest release that opens a settle window, bytes (lean, 2026-11-17): a 16 MiB texture's
 * pages lingering a tick are a few percent of the UHD 620's 150–300 MB budget, and the patch
 * cache's steady churn of smaller buffers never closes every window.
 */
export const LARGE_RELEASE_BYTES = 16 * 1024 ** 2;

/** Whether a sample at `atMs` falls within the settle window of a large release. */
export function settling(atMs: number, releases: ReadonlyArray<TallyRelease>): boolean {
  return releases.some(
    (release) =>
      release.bytes >= LARGE_RELEASE_BYTES &&
      atMs >= release.atMs &&
      atMs < release.atMs + RELEASE_SETTLE_MS,
  );
}

/** A span's DRM samples summarised (Design note 6). */
export interface DrmSpanSummary {
  /** The settled samples' peak resident bytes. */
  readonly residentPeakBytes: number;
  /** The settled samples' peak total, a finding above the ceiling; `null` where none gave one. */
  readonly totalPeakBytes: number | null;
  /** The settled samples' peak lower bound, which the peak resident is at least. */
  readonly residentLowerBoundPeakBytes: number;
  /** The render engine's time over the span, ns: the last sample's less the first's. */
  readonly busyNs: number | null;
  /** The samples within a settle window, which the peaks leave out. */
  readonly settlingSamples: number;
}

/**
 * The span's DRM samples as its peaks, the samples within {@link RELEASE_SETTLE_MS} of a large
 * release left out of them; `null` with no settled sample.
 */
export function summariseDrm(
  samples: ReadonlyArray<DrmMemorySample>,
  releases: ReadonlyArray<TallyRelease>,
): DrmSpanSummary | null {
  const settled = samples.filter((sample) => !settling(sample.atMs, releases));
  if (settled.length === 0) {
    return null;
  }
  const totals = settled
    .map((sample) => sample.totalBytes)
    .filter((bytes): bytes is number => bytes !== null);
  const first = samples[0]?.busyNs ?? null;
  const last = samples.at(-1)?.busyNs ?? null;
  return {
    residentPeakBytes: Math.max(...settled.map((sample) => sample.residentBytes)),
    totalPeakBytes: totals.length === 0 ? null : Math.max(...totals),
    residentLowerBoundPeakBytes: Math.max(...settled.map((s) => s.residentLowerBoundBytes)),
    busyNs: first === null || last === null || last < first ? null : last - first,
    settlingSamples: samples.length - settled.length,
  };
}

/** The display server's line of the record from its span's summary. */
export function displayServerFiguresOf(summary: DrmSpanSummary): DisplayServerFigures {
  return {
    residentPeakBytes: summary.residentPeakBytes,
    renderBusyMs: summary.busyNs === null ? null : summary.busyNs / 1e6,
  };
}

/** One sample of NVIDIA's memory for the GPU process (Design note 6). */
export interface NvidiaMemorySample {
  readonly atMs: number;
  /**
   * The GPU process's `used_memory`, Σ over its `process_info` entries of the first GPU, whatever
   * their type (`G` or `C+G`); `null` where none lists it or the driver writes `N/A`, as WDDM does.
   */
  readonly processUsedBytes: number | null;
  /** The types its entries gave. */
  readonly processTypes: ReadonlyArray<string>;
  /** The device's `fb_memory_usage/used`. */
  readonly deviceUsedBytes: number | null;
  /** The device's use less the baseline taken before launch; `null` without a baseline. */
  readonly deviceLessBaselineBytes: number | null;
}

/** NVIDIA's memory for process `gpuPid` from one `nvidia-smi -q -x` reading. */
export function nvidiaMemoryOf(
  reading: NvidiaReading,
  gpuPid: number | null,
  baselineBytes: number | null,
  atMs: number,
): Measured<NvidiaMemorySample> {
  if (reading.kind === "unavailable") {
    return missing(reading.reason);
  }
  const gpu = reading.gpus[0];
  if (gpu === undefined) {
    return missing("nvidia-smi lists no GPU");
  }
  const entries = gpu.processes.filter((process) => process.pid === gpuPid);
  const used = entries
    .map((process) => process.usedBytes)
    .filter((bytes): bytes is number => bytes !== null);
  return measured({
    atMs,
    processUsedBytes: used.length === 0 ? null : used.reduce((sum, bytes) => sum + bytes, 0),
    processTypes: entries.map((process) => process.type),
    deviceUsedBytes: gpu.usedBytes,
    deviceLessBaselineBytes:
      gpu.usedBytes === null || baselineBytes === null ? null : gpu.usedBytes - baselineBytes,
  });
}

/** One sample of NVIDIA's memory for process `gpuPid`, or why there is none. */
export async function sampleNvidia(
  gpuPid: number | null,
  baselineBytes: number | null,
  sources: MemorySamplerSources,
  atMs: number,
): Promise<Measured<NvidiaMemorySample>> {
  return nvidiaMemoryOf(await readNvidiaSmi(sources.run), gpuPid, baselineBytes, atMs);
}

/** The first GPU's `memory.used` before launch: the baseline its later readings are less. */
export async function nvidiaBaseline(sources: MemorySamplerSources): Promise<Measured<number>> {
  const reading = await readNvidiaSmi(sources.run);
  if (reading.kind === "unavailable") {
    return missing(reading.reason);
  }
  const used = reading.gpus[0]?.usedBytes ?? null;
  return used === null ? missing("nvidia-smi gives no fb_memory_usage/used") : measured(used);
}

/** A span's NVIDIA samples as their peaks. */
export interface NvidiaSpanSummary {
  readonly processPeakBytes: number | null;
  readonly deviceLessBaselinePeakBytes: number | null;
}

/** The largest of `values` that are known, or `null` with none. */
function peak(values: ReadonlyArray<number | null>): number | null {
  const known = values.filter((value): value is number => value !== null);
  return known.length === 0 ? null : Math.max(...known);
}

/** The span's NVIDIA samples as their peaks, each `null` where no sample gave it. */
export function summariseNvidia(samples: ReadonlyArray<NvidiaMemorySample>): NvidiaSpanSummary {
  return {
    processPeakBytes: peak(samples.map((sample) => sample.processUsedBytes)),
    deviceLessBaselinePeakBytes: peak(samples.map((sample) => sample.deviceLessBaselineBytes)),
  };
}

/**
 * The record's GPU-memory headline (Design note 6; R12.T3.b's platform audit): the kernel's DRM
 * figures where there are any; else `nvidia-smi`'s for the GPU process; else its device use less
 * the baseline, as under Windows's WDDM, whose per-process figure is `N/A`; else the tally's, a
 * lower bound; `null` with none of them.
 */
export function gpuMemoryHeadline(
  drm: DrmSpanSummary | null,
  nvidia: NvidiaSpanSummary | null,
  tallyPeakBytes: number | null,
): GpuMemoryFigures | null {
  if (drm !== null) {
    return {
      source: "drm-fdinfo",
      residentPeakBytes: drm.residentPeakBytes,
      totalPeakBytes: drm.totalPeakBytes,
      residentLowerBoundBytes: drm.residentLowerBoundPeakBytes,
    };
  }
  const nvidiaBytes = nvidia?.processPeakBytes ?? nvidia?.deviceLessBaselinePeakBytes ?? null;
  if (nvidiaBytes !== null) {
    return {
      source: "nvidia-smi",
      residentPeakBytes: nvidiaBytes,
      totalPeakBytes: null,
      residentLowerBoundBytes: null,
    };
  }
  return tallyPeakBytes === null
    ? null
    : {
        source: "adapter-tally",
        residentPeakBytes: tallyPeakBytes,
        totalPeakBytes: null,
        residentLowerBoundBytes: null,
      };
}

/** One sample of every process's memory from `app.getAppMetrics()`. */
export interface AppMemorySample {
  readonly atMs: number;
  /** Σ of every process's `workingSetSize`, bytes. */
  readonly totalBytes: number;
  /** The same by Chromium's process type (`Browser`, `GPU`, `Tab`, `Utility`, …). */
  readonly byTypeBytes: Readonly<Record<string, number>>;
  /** The GPU process's ID, for the fdinfo and `nvidia-smi` readings; `null` with none. */
  readonly gpuPid: number | null;
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Every process's memory from `app.getAppMetrics()`, narrowed from what Electron returns: each
 * `ProcessMetric`'s `memory.workingSetSize`, which Electron gives in KiB. It holds no GEM pages
 * (Design note 6: 163 MB before and 166 MB after 256 MiB became resident), so it is reported
 * beside the GPU's figure, never summed with it.
 */
export function appMemoryOf(metrics: unknown, atMs: number): AppMemorySample {
  const byTypeBytes: Record<string, number> = {};
  let totalBytes = 0;
  let gpuPid: number | null = null;
  for (const metric of Array.isArray(metrics) ? metrics.filter(isRecord) : []) {
    const memory = metric["memory"];
    const kib = isRecord(memory) ? memory["workingSetSize"] : undefined;
    const type = typeof metric["type"] === "string" ? metric["type"] : "Unknown";
    if (type === "GPU" && typeof metric["pid"] === "number") {
      gpuPid = metric["pid"];
    }
    if (typeof kib === "number" && Number.isFinite(kib)) {
      byTypeBytes[type] = (byTypeBytes[type] ?? 0) + kib * 1024;
      totalBytes += kib * 1024;
    }
  }
  return { atMs, totalBytes, byTypeBytes, gpuPid };
}
