/**
 * A perf run's preconditions (plan R12, R12.T2; Design note 7): the machine on AC, a one-minute
 * load average below 1.0 and the package below 60 °C at the start, and the power profile recorded
 * as found. The runner (R12.T4.a) refuses a run whose report is not quiet unless `--force` marks
 * the record, its refusals the record's `forcedReasons`.
 *
 * @remarks
 * Under Linux, AC is the `online` file of the power supply whose `type` is `Mains`
 * (`Documentation/ABI/testing/sysfs-class-power`: `type`, `online` and a battery's `status`); a
 * USB-C source's `online` is ignored, and a supply whose `scope` is `Device`
 * (`drivers/power/supply/power_supply_sysfs.c`, `POWER_SUPPLY_SCOPE_TEXT`), such as a mouse's
 * battery, is ignored too, so that a machine with no `Mains` supply and no system battery reads as
 * `desktop`, which is always on AC (hardware item 2, decided 2026-09-30; this settles R12.T9's open
 * point: a desktop passes). Without a `Mains` supply, a system battery that is not discharging
 * reads as on AC, and a kernel with no power-supply class at all as a desktop. The profile is
 * TLP's (`tlp-stat -s`, which runs unprivileged), the governor and energy preference of `cpu0`,
 * `intel_pstate/no_turbo`, and RAPL's PL1, PL2 and τ from `intel-rapl-mmio:0`, else
 * `intel-rapl:0` (`Documentation/power/powercap/powercap.rst`: `constraint_<n>_name`, with
 * `power_limit_uw` and `time_window_us`; `drivers/powercap/intel_rapl_common.c`'s `pl_names`:
 * `long_term` is PL1 and `short_term` PL2).
 *
 * The load average takes R05's rule (`machineLoad.ts`'s `quietOf`), so that Windows, which keeps
 * none, is never quiet on `os.loadavg()`'s zeros: its record is forced, its quiet rule unchecked.
 * On macOS AC and the thermal state come from `pmset -g batt` and `pmset -g therm`, and Low Power
 * Mode from `pmset -g`, all unprivileged (Apple's `pmset.m` prints them; the M5 Mac's lean,
 * pending the owner): the thermal pressure stands in for the package temperature, which macOS
 * gives no unprivileged reading of, and the start waits for it to clear as for the package to cool.
 * The platform is one switch, in {@link checkQuiet}.
 */

import type { PowerProfileRecord, PowerThermalRecord } from "@hyperion/perf";

import type { RunCommand } from "../fdinfo";
import type { SysfsFiles } from "../gpuClocks";
import { type LoadAverage, quietOf, readLoadAverage } from "../machineLoad";
import { type Measured, measured, missing } from "../measured";
import {
  findRaplZone,
  listDir,
  notLinuxReason,
  readNumber,
  readPackageTemperature,
  readText,
  readThermalPressure,
  readThermalZones,
  samplerPlatformOf,
  type SamplerSources,
  type ThermalPressure,
} from "./samplers";

/** Design note 7's limits. */
export const QUIET_LIMITS = {
  /** The package temperature at the start must be below this, °C. */
  startPackageDegC: 60,
  /** How long the start waits for the package to cool below it, ms: five minutes. */
  coolWaitMs: 5 * 60 * 1000,
  /** How often the wait reads the temperature, ms. */
  coolPollMs: 5000,
} as const;

/** What {@link checkQuiet} reads, beyond the samplers' sources. */
export interface PreconditionSources extends SamplerSources {
  /** `os.loadavg` outside a test. */
  loadavg(): ReadonlyArray<number>;
  /** Waits `ms`; a test's resolves at once. */
  sleep(ms: number): Promise<void>;
}

/** One power supply under `/sys/class/power_supply`. */
export interface PowerSupply {
  readonly name: string;
  /** `Battery`, `UPS`, `Mains`, `USB` or `Wireless`. */
  readonly type: string;
  /** `System`, `Device` or `Unknown`; `null` where the driver writes no `scope`. */
  readonly scope: string | null;
  /** `online`'s value for a source; `null` where it has none. */
  readonly online: number | null;
  /** A battery's `status`: `Charging`, `Discharging`, `Full`, `Not charging` or `Unknown`. */
  readonly status: string | null;
}

/** Every power supply under `<sysRoot>/class/power_supply`, or why the directory is unread. */
export async function readPowerSupplies(
  files: SysfsFiles,
  sysRoot: string,
): Promise<Measured<ReadonlyArray<PowerSupply>>> {
  const dir = `${sysRoot}/class/power_supply`;
  const entries = await listDir(files, dir);
  if (entries.value === null) {
    // No power supply class at all is a machine without one, as a desktop may be.
    return entries.reason.includes("ENOENT") ? measured([]) : missing(entries.reason);
  }
  const supplies = await Promise.all(
    entries.value.toSorted().map(async (name) => {
      const [type, scope, online, status] = await Promise.all([
        readText(files, `${dir}/${name}/type`),
        readText(files, `${dir}/${name}/scope`),
        readNumber(files, `${dir}/${name}/online`),
        readText(files, `${dir}/${name}/status`),
      ]);
      return type.value === null
        ? null
        : {
            name,
            type: type.value,
            scope: scope.value,
            online: online.value,
            status: status.value,
          };
    }),
  );
  return measured(supplies.filter((supply): supply is PowerSupply => supply !== null));
}

/** Which supply the machine runs on, and the words that say why. */
export interface SupplyReading {
  readonly supply: NonNullable<PowerThermalRecord["supply"]>;
  readonly why: string;
}

/**
 * The machine's supply from its power supplies (the module's remarks): a `Mains` supply's
 * `online`; without one, a system battery's `status` (discharging is on battery); without either,
 * `desktop`. Supplies of `scope` `Device` and every `USB` source are ignored.
 */
export function supplyOf(supplies: ReadonlyArray<PowerSupply>): SupplyReading {
  const system = supplies.filter((supply) => supply.scope !== "Device");
  const mains = system.filter((supply) => supply.type === "Mains");
  if (mains.length > 0) {
    // `online` is 1, or 2 for a programmable source (`sysfs-class-power`).
    const online = mains.filter((supply) => supply.online !== null && supply.online > 0);
    return online.length > 0
      ? { supply: "mains", why: `${online.map((s) => s.name).join(", ")} online` }
      : { supply: "battery", why: `${mains.map((s) => s.name).join(", ")} offline` };
  }
  const batteries = system.filter((supply) => supply.type === "Battery");
  if (batteries.length > 0) {
    const discharging = batteries.filter((supply) => supply.status === "Discharging");
    return discharging.length > 0
      ? { supply: "battery", why: `${discharging.map((s) => s.name).join(", ")} discharging` }
      : {
          supply: "mains",
          why: `no Mains supply; ${batteries.map((s) => s.name).join(", ")} not discharging`,
        };
  }
  return { supply: "desktop", why: "no Mains supply and no system battery" };
}

/** macOS's supply from `pmset -g batt`'s first line, "Now drawing from 'AC Power'". */
export function parsePmsetBatt(text: string): SupplyReading | null {
  const source = /Now drawing from '([^']+)'/.exec(text)?.[1];
  if (source === undefined) {
    return null;
  }
  return source === "AC Power"
    ? { supply: "mains", why: "pmset: AC Power" }
    : { supply: "battery", why: `pmset: ${source}` };
}

/**
 * macOS's Low Power Mode from `pmset -g`: its `lowpowermode` setting, else `powermode`, whose 1 is
 * low power (0 automatic, 2 high power); `null` with neither.
 */
export function parsePmsetLowPower(text: string): boolean | null {
  const low = /^\s*lowpowermode\s+(\d+)/m.exec(text)?.[1];
  if (low !== undefined) {
    return low === "1";
  }
  const mode = /^\s*powermode\s+(\d+)/m.exec(text)?.[1];
  return mode === undefined ? null : mode === "1";
}

/**
 * TLP's mode from `tlp-stat -s`: the value of its `Mode` line (TLP 1.6), `Power profile` (1.9) or
 * `TLP profile` (later), such as `performance/AC`, without the `(default)` or `(manual)` after it.
 */
export function parseTlpMode(text: string): string | null {
  const value = /^(?:Mode|Power profile|TLP profile)\s*=\s*(\S+)/m.exec(text)?.[1];
  return value ?? null;
}

/** RAPL's limits as found: PL1 and PL2, W, and τ, s. */
interface RaplLimits {
  readonly zone: string | null;
  readonly pl1Watts: number | null;
  readonly pl2Watts: number | null;
  readonly tauSeconds: number | null;
}

/** A zone's constraint named `name`'s power limit, W, and time window, s. */
async function readConstraint(
  files: SysfsFiles,
  dir: string,
  names: ReadonlyArray<string>,
  name: string,
): Promise<{ readonly watts: number | null; readonly seconds: number | null }> {
  const indices = names
    .map((file) => /^constraint_(\d+)_name$/.exec(file)?.[1])
    .filter((index): index is string => index !== undefined);
  const named = await Promise.all(
    indices.map((index) => readText(files, `${dir}/constraint_${index}_name`)),
  );
  const index = indices.find((_, i) => named[i]?.value === name);
  if (index === undefined) {
    return { watts: null, seconds: null };
  }
  const [uw, us] = await Promise.all([
    readNumber(files, `${dir}/constraint_${index}_power_limit_uw`),
    readNumber(files, `${dir}/constraint_${index}_time_window_us`),
  ]);
  return {
    watts: uw.value === null ? null : uw.value / 1e6,
    seconds: us.value === null ? null : us.value / 1e6,
  };
}

/** RAPL's PL1 (`long_term`), PL2 (`short_term`) and τ (PL1's window) of the first zone found. */
async function readRaplLimits(files: SysfsFiles, sysRoot: string): Promise<RaplLimits> {
  const zone = await findRaplZone(files, sysRoot);
  if (zone.value === null) {
    return { zone: null, pl1Watts: null, pl2Watts: null, tauSeconds: null };
  }
  const { dir } = zone.value;
  const names = (await listDir(files, dir)).value ?? [];
  const [pl1, pl2] = await Promise.all([
    readConstraint(files, dir, names, "long_term"),
    readConstraint(files, dir, names, "short_term"),
  ]);
  return {
    zone: zone.value.zone,
    pl1Watts: pl1.watts,
    pl2Watts: pl2.watts,
    tauSeconds: pl1.seconds,
  };
}

/** A command's standard output, or `null` where it does not run. */
async function outputOf(
  run: RunCommand,
  file: string,
  args: ReadonlyArray<string>,
): Promise<string | null> {
  return run(file, args).then(
    (text) => text,
    () => null,
  );
}

/** The power profile as found on Linux (Design note 7). */
async function linuxProfile(sources: SamplerSources): Promise<PowerProfileRecord> {
  const { files, sysRoot, run } = sources;
  const cpufreq = `${sysRoot}/devices/system/cpu/cpu0/cpufreq`;
  const [tlp, governor, epp, noTurbo, rapl] = await Promise.all([
    outputOf(run, "tlp-stat", ["-s"]),
    readText(files, `${cpufreq}/scaling_governor`),
    readText(files, `${cpufreq}/energy_performance_preference`),
    readText(files, `${sysRoot}/devices/system/cpu/intel_pstate/no_turbo`),
    readRaplLimits(files, sysRoot),
  ]);
  return {
    tlpMode: tlp === null ? null : parseTlpMode(tlp),
    governor: governor.value,
    epp: epp.value,
    noTurbo: noTurbo.value === null ? null : noTurbo.value === "1",
    pl1Watts: rapl.pl1Watts,
    pl2Watts: rapl.pl2Watts,
    tauSeconds: rapl.tauSeconds,
    raplZone: rapl.zone,
    lowPowerMode: null,
  };
}

/** No profile reading: every field `null`. */
const NO_PROFILE: PowerProfileRecord = {
  tlpMode: null,
  governor: null,
  epp: null,
  noTurbo: null,
  pl1Watts: null,
  pl2Watts: null,
  tauSeconds: null,
  raplZone: null,
  lowPowerMode: null,
};

/** What {@link checkQuiet} found: whether the run may start unforced, and the state as found. */
export interface PreconditionReport {
  /** Every precondition held. */
  readonly quiet: boolean;
  /** Each precondition that failed or went unchecked, in words: the record's `forcedReasons`. */
  readonly refusals: ReadonlyArray<string>;
  /** The record's `state`. */
  readonly state: PowerThermalRecord;
  /** How long the start waited for the package to cool, or macOS's pressure to clear, ms. */
  readonly cooledMs: number;
}

/** The supply's refusal, or `null` where it passes: on battery, or unread. */
function supplyRefusal(reading: Measured<SupplyReading>): string | null {
  if (reading.value === null) {
    return `the power supply is unread (${reading.reason}): on AC is unchecked`;
  }
  return reading.value.supply === "battery"
    ? `on battery (${reading.value.why}); Design note 7 asks for AC`
    : null;
}

/** Why a platform that keeps no load average is never quiet. */
export const LOAD_UNCHECKED =
  "no load average on this platform: Design note 7's quiet machine is unchecked";

/** The load average's refusal by R05's rule (`quietOf`), in this plan's words, or `null`. */
function loadRefusal(platform: NodeJS.Platform, load: Measured<LoadAverage>): string | null {
  if (!quietOf(platform, load.value ?? [0, 0, 0]).provisional) {
    return null;
  }
  return load.value === null
    ? LOAD_UNCHECKED
    : `load average ${load.value[0].toFixed(2)} at the start (Design note 7 asks below 1.0)`;
}

/**
 * The package temperature at the start, waiting for it to fall below the limit for at most
 * {@link QUIET_LIMITS.coolWaitMs}, one reading every {@link QUIET_LIMITS.coolPollMs}.
 */
async function cooledTemperature(
  sources: PreconditionSources,
  waitedMs: number,
): Promise<{
  readonly reading: Measured<{ readonly degC: number; readonly source: string }>;
  readonly waitedMs: number;
}> {
  const { files, sysRoot } = sources;
  const reading = await readPackageTemperature(
    files,
    sysRoot,
    await readThermalZones(files, sysRoot),
  );
  if (
    reading.value === null ||
    reading.value.degC < QUIET_LIMITS.startPackageDegC ||
    waitedMs >= QUIET_LIMITS.coolWaitMs
  ) {
    return { reading, waitedMs };
  }
  await sources.sleep(QUIET_LIMITS.coolPollMs);
  return cooledTemperature(sources, waitedMs + QUIET_LIMITS.coolPollMs);
}

/** The temperature's refusal, or `null` where it passes: too hot after the wait, or unread. */
function temperatureRefusal(reading: Measured<{ degC: number }>, waitedMs: number): string | null {
  if (reading.value === null) {
    return `the package temperature is unread (${reading.reason}): below ${QUIET_LIMITS.startPackageDegC} °C is unchecked`;
  }
  return reading.value.degC >= QUIET_LIMITS.startPackageDegC
    ? `package at ${reading.value.degC.toFixed(1)} °C after ${Math.round(waitedMs / 1000)} s (Design note 7 asks below ${QUIET_LIMITS.startPackageDegC} °C)`
    : null;
}

/** The report from what was found, its refusals those that are not `null`. */
function reportOf(
  refusals: ReadonlyArray<string | null>,
  state: PowerThermalRecord,
  cooledMs: number,
): PreconditionReport {
  const named = refusals.filter((refusal): refusal is string => refusal !== null);
  return { quiet: named.length === 0, refusals: named, state, cooledMs };
}

/** The load average's reading and its first figure, for a report. */
interface LoadReading {
  readonly load: Measured<LoadAverage>;
  readonly loadAverage1m: number | null;
}

/** Linux's preconditions: the supply, the load, the cooled package and the profile. */
async function linuxQuiet(
  sources: PreconditionSources,
  { load, loadAverage1m }: LoadReading,
): Promise<PreconditionReport> {
  const { platform } = sources;
  const supplies = await readPowerSupplies(sources.files, sources.sysRoot);
  const supply =
    supplies.value === null ? missing(supplies.reason) : measured(supplyOf(supplies.value));
  const [profile, cooled] = await Promise.all([
    linuxProfile(sources),
    cooledTemperature(sources, 0),
  ]);
  return reportOf(
    [
      supplyRefusal(supply),
      loadRefusal(platform, load),
      temperatureRefusal(cooled.reading, cooled.waitedMs),
    ],
    {
      supply: supply.value?.supply ?? null,
      profile,
      loadAverage1m,
      startPackageDegC: cooled.reading.value?.degC ?? null,
      temperatureSource: cooled.reading.value?.source ?? null,
    },
    cooled.waitedMs,
  );
}

/**
 * macOS's thermal state at the start, waiting for any pressure to clear for at most
 * {@link QUIET_LIMITS.coolWaitMs}, one reading every {@link QUIET_LIMITS.coolPollMs}.
 */
async function clearedPressure(
  sources: PreconditionSources,
  waitedMs: number,
): Promise<{ readonly pressure: Measured<ThermalPressure>; readonly waitedMs: number }> {
  const pressure = await readThermalPressure(sources.run);
  if (pressure.value === null || pressure.value.nominal || waitedMs >= QUIET_LIMITS.coolWaitMs) {
    return { pressure, waitedMs };
  }
  await sources.sleep(QUIET_LIMITS.coolPollMs);
  return clearedPressure(sources, waitedMs + QUIET_LIMITS.coolPollMs);
}

/** macOS's preconditions: `pmset`'s supply, Low Power Mode and thermal pressure, and the load. */
async function darwinQuiet(
  sources: PreconditionSources,
  { load, loadAverage1m }: LoadReading,
): Promise<PreconditionReport> {
  const { platform } = sources;
  const [batt, settings, cleared] = await Promise.all([
    outputOf(sources.run, "pmset", ["-g", "batt"]),
    outputOf(sources.run, "pmset", ["-g"]),
    clearedPressure(sources, 0),
  ]);
  const { pressure } = cleared;
  const parsed = batt === null ? null : parsePmsetBatt(batt);
  const supply = parsed === null ? missing("pmset -g batt gave no power source") : measured(parsed);
  const pressureRefusal =
    pressure.value === null
      ? `the thermal state is unread (${pressure.reason}): a nominal state is unchecked`
      : pressure.value.nominal
        ? null
        : `thermal pressure after ${Math.round(cleared.waitedMs / 1000)} s (pmset -g therm: ${pressure.value.detail})`;
  return reportOf(
    [supplyRefusal(supply), loadRefusal(platform, load), pressureRefusal],
    {
      supply: supply.value?.supply ?? null,
      profile: {
        ...NO_PROFILE,
        lowPowerMode: settings === null ? null : parsePmsetLowPower(settings),
      },
      loadAverage1m,
      startPackageDegC: null,
      temperatureSource: pressure.value === null ? null : "pmset -g therm",
    },
    cleared.waitedMs,
  );
}

/** Elsewhere (Windows): nothing is read, and every precondition is unchecked. */
function otherQuiet(
  platform: NodeJS.Platform,
  { load, loadAverage1m }: LoadReading,
): PreconditionReport {
  return reportOf(
    [
      supplyRefusal(missing(notLinuxReason(platform))),
      loadRefusal(platform, load),
      temperatureRefusal(missing(notLinuxReason(platform)), 0),
    ],
    {
      supply: null,
      profile: NO_PROFILE,
      loadAverage1m,
      startPackageDegC: null,
      temperatureSource: null,
    },
    0,
  );
}

/**
 * Checks Design note 7's preconditions and reads the power profile as found (the module's
 * remarks), waiting up to five minutes for the package to cool below 60 °C. Never rejects: a
 * reading that fails is a refusal naming what is unchecked.
 */
export async function checkQuiet(sources: PreconditionSources): Promise<PreconditionReport> {
  const { platform } = sources;
  const load = readLoadAverage({ platform, loadavg: () => sources.loadavg() });
  const reading: LoadReading = { load, loadAverage1m: load.value?.[0] ?? null };
  let report: Promise<PreconditionReport>;
  switch (samplerPlatformOf(platform)) {
    case "linux":
      report = linuxQuiet(sources, reading);
      break;
    case "darwin":
      report = darwinQuiet(sources, reading);
      break;
    case "other":
      report = Promise.resolve(otherQuiet(platform, reading));
      break;
  }
  return report;
}
