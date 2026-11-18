import { readFileSync } from "node:fs";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

import type { RunCommand } from "../../fdinfo";
import type { SysfsFiles } from "../../gpuClocks";

/**
 * The perf samplers' fixtures (plan R12, R12.T2): two sysfs trees and the commands' output.
 *
 * @remarks
 * The UHD 620 laptop's files are captured from it (2026-11-18, R12.T2 and R12.T3.b); the rest are
 * written from the kernel's documented layout and the machines as the plan records them (2026-09-29
 * and 2026-09-30), not captured: the owner's capture of the development machine's `/sys` replaces
 * them by hand (R12.T2, NOT RUN in a pod).
 *
 * - `x1-yoga/sys`: the ThinkPad X1 Yoga 4th generation, an i7-8665U with its UHD 620 (`8086:3ea0`),
 *   read at idle on AC after the day's descent runs (the load average 0.03): a `Mains` supply `AC`
 *   beside two UCSI USB-C sources and `BAT0`; seven thermal zones, one `x86_pkg_temp` at 46 °C;
 *   i915's frequency files under `card1`, at 317 MHz against RP0's 1,150; eight CPUs with
 *   `thermal_throttle` (524 s of package throttling since boot); `intel_pstate`. The powercap
 *   files are as read earlier the same day (21:54 UTC), when `intel-rapl-mmio:0` held PL1 15 W,
 *   τ 27.98 s and PL2 51 W beside `intel-rapl:0`'s PL1 of 51 W; by 23:08 UTC the MMIO zone's
 *   PL1 read 51 W, raised at run time (not traced), so PL1 is read per run, never assumed.
 *   `energy_uj` is root-only there, so its two files are written, not captured.
 * - `desktop/sys`: the Ryzen 7 3700X and RTX 3080 (`10de:2206`), trimmed to four of its sixteen
 *   CPUs: no `Mains` supply and no system battery, only a mouse's (`scope` `Device`); no thermal
 *   zone, `k10temp`'s `Tctl`; `acpi-cpufreq` with `schedutil` and no `thermal_throttle`;
 *   `intel-rapl:0` with no constraint files.
 * - `commands/`: `tlp-stat -s` (TLP 1.11's, captured on the laptop, its `System` and
 *   `/proc/cmdline` lines trimmed; 1.10's form and 1.6's, written), `pmset -g batt`, `-g therm` and
 *   `-g`, the long-lived `nvidia-smi` child's CSV, and `nvidia-smi -q -x` on the RTX 3080 idle and
 *   with the GPU process listed as `G`, as `C+G`, and under Windows's WDDM (`N/A`) (R12.T3.b).
 * - `fdinfo/`: captured on the laptop five minutes into a low descent (R12.T3.b; Design note 6):
 *   the GPU process's six i915 clients on nine descriptors (client 17 behind four), two of them
 *   resident (61,008 KiB with 14,696 KiB shared, and 186,168 KiB), and Xorg's one client, readable
 *   since Xorg runs as the user.
 *
 * A path's `:` is stored as `%3A`, since Windows refuses it in a file name (`intel-rapl-mmio:0`).
 */
export const PERF_FIXTURES = __dirname;

/** The UHD 620 laptop's tree, as `/sys`. */
export const X1_YOGA_SYS = join(PERF_FIXTURES, "x1-yoga/sys");

/** The development desktop's tree, as `/sys`. */
export const DESKTOP_SYS = join(PERF_FIXTURES, "desktop/sys");

/** A command fixture's text. */
export function commandFixture(name: string): string {
  return readFileSync(join(PERF_FIXTURES, "commands", name), "utf8");
}

/** How {@link fixtureFiles} departs from the tree. */
export interface FixtureChanges {
  /** Paths (as read, under the tree's root) that fail with `EACCES`, as a root-only file does. */
  readonly denied?: ReadonlyArray<string>;
  /** Paths whose text is replaced, or added. */
  readonly replaced?: Readonly<Record<string, string>>;
}

/** A path as the fixture stores it. */
function onDisk(path: string): string {
  return path.replaceAll(":", "%3A");
}

function failure(code: string, path: string): Error {
  return Object.assign(new Error(`${code}: ${path}`), { code });
}

/**
 * A fixture tree's files, `:` mapped to `%3A` on the way to the disk and back, with
 * `energy_uj` denied unless `denied` says otherwise, as an unprivileged process finds it.
 */
export function fixtureFiles(root: string, changes: FixtureChanges = {}): SysfsFiles {
  const denied = changes.denied ?? [
    `${root}/class/powercap/intel-rapl-mmio:0/energy_uj`,
    `${root}/class/powercap/intel-rapl:0/energy_uj`,
  ];
  const replaced = changes.replaced ?? {};
  return {
    readdir: async (path) => {
      const names = (await readdir(onDisk(path))).map((name) => name.replaceAll("%3A", ":"));
      const added = Object.keys(replaced)
        .filter((file) => file.startsWith(`${path}/`) && !file.slice(path.length + 1).includes("/"))
        .map((file) => file.slice(path.length + 1));
      return [...new Set([...names, ...added])];
    },
    readFile: async (path) => {
      const text = replaced[path];
      if (text !== undefined) {
        return text;
      }
      if (denied.includes(path)) {
        throw failure("EACCES", path);
      }
      return readFile(onDisk(path), "utf8");
    },
  };
}

/** A {@link RunCommand} answering each `file args…` from `outputs`, failing with `ENOENT` else. */
export function fixtureCommands(outputs: Readonly<Record<string, string>>): RunCommand {
  return (file, args) => {
    const output = outputs[[file, ...args].join(" ")];
    return output === undefined ? Promise.reject(failure("ENOENT", file)) : Promise.resolve(output);
  };
}

/** An fdinfo fixture's text. */
export function fdinfoFixture(name: string): string {
  return readFileSync(join(PERF_FIXTURES, "fdinfo", name), "utf8");
}

/**
 * `/proc` with each process's open descriptors as fdinfo fixtures (`fds`, by process ID, then by
 * descriptor, its file under `fixtures/fdinfo` or R05's `../fixtures/fdinfo`) and its `comm`;
 * a process in `unreadable` refuses its fdinfo with `EACCES`, as a root process's does.
 */
export function procFixture(
  fds: Readonly<Record<string, Readonly<Record<string, string>>>>,
  comm: Readonly<Record<string, string>> = {},
  unreadable: ReadonlyArray<string> = [],
): SysfsFiles {
  const text = (name: string): string =>
    name.startsWith("r05:")
      ? readFileSync(join(PERF_FIXTURES, "../../fixtures/fdinfo", name.slice(4)), "utf8")
      : fdinfoFixture(name);
  return {
    readdir: (path) => {
      if (path === "/proc") {
        return Promise.resolve([...new Set(["self", ...Object.keys(fds), ...Object.keys(comm)])]);
      }
      const pid = /^\/proc\/(\d+)\/fdinfo$/.exec(path)?.[1];
      if (pid !== undefined && unreadable.includes(pid)) {
        return Promise.reject(failure("EACCES", path));
      }
      const open = pid === undefined ? undefined : fds[pid];
      return open === undefined
        ? Promise.reject(failure("ENOENT", path))
        : Promise.resolve(Object.keys(open));
    },
    readFile: (path) => {
      const name = /^\/proc\/(\d+)\/comm$/.exec(path)?.[1];
      if (name !== undefined) {
        const process = comm[name];
        return process === undefined
          ? Promise.reject(failure("ENOENT", path))
          : Promise.resolve(`${process}\n`);
      }
      const [, pid = "", fd = ""] = /^\/proc\/(\d+)\/fdinfo\/(\d+)$/.exec(path) ?? [];
      const file = fds[pid]?.[fd];
      return file === undefined
        ? Promise.reject(failure("ENOENT", path))
        : Promise.resolve(text(file));
    },
  };
}
