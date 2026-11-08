import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

/**
 * `scripts/descentSpike.sh`'s launch in the check-only software mode (plan R01, R01.T16; main's
 * ruling of 2026-11-01 for the spike), run with a stand-in for Electron (`HYPERION_SPIKE_ELECTRON`)
 * that records its arguments and environment, a stand-in `cargo` for the server the script starts,
 * and the runs' directory its own (`HYPERION_SPIKE_RUNS`). The test runs on Linux alone; one of
 * its runs takes away `setsid` and `timeout`, as macOS lacks them, and the script then finds
 * `tools/portable`'s.
 */

/** The script, from this file. */
const SCRIPT = join(__dirname, "../../scripts/descentSpike.sh");

/** The smoke harness's runner, whose SwiftShader switches the software mode takes (R01.T16). */
const TEST_RENDER = join(__dirname, "../../scripts/testRender.sh");

/** The environment the stand-in reports, `<unset>` for a variable it lacks. */
const REPORTED_ENV = [
  "DISPLAY",
  "WAYLAND_DISPLAY",
  "XDG_SESSION_TYPE",
  "DBUS_SESSION_BUS_ADDRESS",
  "HYPERION_SPIKE_HIDDEN",
] as const;

/** The software mode's switches after test-render's own (R01.T16). */
const SOFTWARE_SWITCHES = [
  "--hyperion-graphics-software",
  "--ozone-override-screen-size=1920,1080",
  "--disable-gpu-watchdog",
];

/** A desktop session's display variables, which a software launch must not see. */
const DESKTOP_ENV = {
  DISPLAY: ":0",
  WAYLAND_DISPLAY: "wayland-0",
  XDG_SESSION_TYPE: "wayland",
  DBUS_SESSION_BUS_ADDRESS: "unix:path=/run/user/1000/bus",
};

/** Each run's own directory, all removed after the test. */
const scratch: string[] = [];

afterEach(() => {
  for (const dir of scratch.splice(0)) {
    rmSync(dir, { recursive: true, force: true });
  }
});

/** What a run of the script did. */
interface ScriptRun {
  /** The client's launch's arguments, or `null` where the script launched none. */
  readonly launch: ReadonlyArray<string> | null;
  /** The launch's {@link REPORTED_ENV}, by name. */
  readonly env: Readonly<Record<string, string>>;
  readonly output: string;
  readonly status: number | null;
  /** The runs' directory: its profiles, removed again, and its kept shader caches. */
  readonly runs: string;
}

/** What a run of the script is given beyond its arguments. */
interface ScriptSetting {
  /** Variables the script is run with, beside its own. */
  readonly env?: Readonly<Record<string, string>>;
  /** What a stand-in `uname -s` answers; absent, the machine's own answers. */
  readonly uname?: string;
  /** Commands the script's PATH lacks, as macOS lacks setsid(1) and timeout(1). */
  readonly without?: ReadonlyArray<string>;
}

/**
 * A PATH as a machine without `tools` has it, macOS's without setsid(1) and timeout(1): a
 * directory under `dir` linking every other command on this PATH, the first of each name.
 */
function pathWithout(dir: string, tools: ReadonlyArray<string>): string {
  const linked = join(dir, "path");
  mkdirSync(linked);
  for (const each of (process.env["PATH"] ?? "/usr/bin:/bin").split(delimiter)) {
    const names = existsSync(each) && statSync(each).isDirectory() ? readdirSync(each) : [];
    for (const name of names.filter((one) => !tools.includes(one))) {
      if (!existsSync(join(linked, name))) {
        symlinkSync(join(each, name), join(linked, name));
      }
    }
  }
  return linked;
}

/** Writes an executable stand-in at `path`. */
function stub(path: string, body: ReadonlyArray<string>): void {
  writeFileSync(path, ["#!/usr/bin/env bash", ...body, ""].join("\n"));
  chmodSync(path, 0o755);
}

/** Runs the script with `args` as {@link ScriptSetting} says. */
function runScript(args: ReadonlyArray<string>, given: ScriptSetting = {}): ScriptRun {
  const dir = mkdtempSync(join(tmpdir(), "hyperion-descent-spike-script-"));
  scratch.push(dir);
  const bin = join(dir, "bin");
  mkdirSync(bin);
  const runs = join(dir, "runs");
  const log = join(dir, "launch.log");
  const envLog = join(dir, "env.log");
  // The server the script starts: a process that waits to be killed with its group.
  stub(join(bin, "cargo"), ["exec sleep 60"]);
  // No NVIDIA driver, whatever the machine has.
  stub(join(bin, "nvidia-smi"), ["exit 1"]);
  if (given.uname !== undefined) {
    stub(join(bin, "uname"), [`echo ${given.uname}`]);
  }
  // The client: it records its arguments and environment, and leaves a shader cache in its profile.
  const electron = join(dir, "electron");
  stub(electron, [
    `printf '%s\\n' "$@" > "$STUB_LOG"`,
    `printf '%s\\n' ${REPORTED_ENV.map((name) => `"${name}=\${${name}-<unset>}"`).join(" ")} > "$STUB_ENV_LOG"`,
    'for arg in "$@"; do case "$arg" in --user-data-dir=*) mkdir -p "${arg#--user-data-dir=}/GPUCache" ;; esac; done',
    "exit 0",
  ]);
  const env: Record<string, string> = {
    ...given.env,
    PATH: `${bin}${delimiter}${
      given.without === undefined
        ? (process.env["PATH"] ?? "/usr/bin:/bin")
        : pathWithout(dir, given.without)
    }`,
    HOME: dir,
    STUB_LOG: log,
    STUB_ENV_LOG: envLog,
    HYPERION_SPIKE_ELECTRON: electron,
    HYPERION_SPIKE_RUNS: runs,
    HYPERION_SPIKE_PORT: "7899",
  };
  const result = spawnSync("bash", [SCRIPT, ...args], {
    env,
    encoding: "utf8",
    timeout: 60_000,
  });
  const launch = existsSync(log) ? lines(log) : null;
  const reported = Object.fromEntries(
    lines(envLog).map((pair) => {
      const at = pair.indexOf("=");
      return [pair.slice(0, at), pair.slice(at + 1)];
    }),
  );
  return {
    launch,
    env: reported,
    output: `${result.stdout}${result.stderr}`,
    status: result.status,
    runs,
  };
}

/** A log's lines; none for a log never written. */
function lines(path: string): ReadonlyArray<string> {
  return existsSync(path)
    ? readFileSync(path, "utf8")
        .split("\n")
        .filter((line) => line.length > 0)
    : [];
}

/** A bash array's words as `script` assigns it, `name=(…)`, over one line or more. */
function bashArray(script: string, name: string): ReadonlyArray<string> {
  const match = new RegExp(`^${name}=\\(([^)]*)\\)`, "mu").exec(readFileSync(script, "utf8"));
  if (match?.[1] === undefined) {
    throw new Error(`${script} assigns no ${name}`);
  }
  return match[1].split(/\s+/u).filter((word) => word.length > 0);
}

describe.runIf(process.platform === "linux")("the descent spike's script", () => {
  it("takes test-render's adapter and headless switches word for word", () => {
    for (const name of ["adapter_switches", "headless_switches"]) {
      expect(bashArray(SCRIPT, name)).toEqual(bashArray(TEST_RENDER, name));
    }
  });

  it("launches --software hidden on SwiftShader, out of the desktop session's reach", () => {
    const run = runScript(["--software", "--smoke"], { env: DESKTOP_ENV });
    expect(run.status).toBe(0);
    const launch = run.launch ?? [];
    const switches = [
      ...bashArray(TEST_RENDER, "adapter_switches"),
      ...bashArray(TEST_RENDER, "headless_switches"),
      ...SOFTWARE_SWITCHES,
    ];
    expect(launch.slice(0, switches.length)).toEqual(switches);
    expect(launch).toContain("--descent-spike");
    expect(launch).not.toContain("--ozone-platform=x11");
    expect(run.env).toEqual({
      DISPLAY: "<unset>",
      WAYLAND_DISPLAY: "<unset>",
      XDG_SESSION_TYPE: "<unset>",
      DBUS_SESSION_BUS_ADDRESS: "unix:path=/nonexistent",
      HYPERION_SPIKE_HIDDEN: "1",
    });
  });

  it("gives a software run five times the limit", () => {
    expect(runScript(["--software", "--smoke"]).output).toContain(
      "descent spike: software mode, on SwiftShader; the run's limit 1500 s",
    );
    expect(runScript(["--software"]).output).toContain("the run's limit 18000 s");
  });

  it("keeps a software run's shader caches apart from a GPU run's", () => {
    const run = runScript(["--software", "--smoke"]);
    expect(run.status).toBe(0);
    expect(readdirSync(join(run.runs, "gpu-cache-software"))).toEqual(["GPUCache"]);
    expect(existsSync(join(run.runs, "gpu-cache"))).toBe(false);
  });

  it("starts the server and the client on a PATH without setsid and timeout, as macOS's is", () => {
    const run = runScript(["--smoke"], { without: ["setsid", "timeout"] });
    expect([run.status, run.launch?.includes("--descent-spike")]).toEqual([0, true]);
  });

  it("refuses --software off Linux, launching nothing", () => {
    const run = runScript(["--software", "--smoke"], { uname: "Darwin" });
    expect(run.status).toBe(2);
    expect(run.output).toContain("descentSpike.sh: --software runs on Linux only");
    expect(run.launch).toBeNull();
  });

  it("leaves a GPU launch as it was: the session's own display, no software switch", () => {
    const run = runScript(["--smoke"], { env: DESKTOP_ENV });
    expect(run.status).toBe(0);
    const launch = run.launch ?? [];
    expect(launch[0]).toBe("--ozone-platform=x11");
    for (const word of [...SOFTWARE_SWITCHES, "--use-webgpu-adapter=swiftshader"]) {
      expect(launch).not.toContain(word);
    }
    expect(run.env).toEqual({ ...DESKTOP_ENV, HYPERION_SPIKE_HIDDEN: "<unset>" });
    expect(run.output).not.toContain("software mode");
    expect(readdirSync(join(run.runs, "gpu-cache"))).toEqual(["GPUCache"]);
  });

  it("launches a hidden run with no display on headless Ozone at the software mode's screen size, on the adapter it finds", () => {
    for (const args of [["--smoke"], ["--hidden"]]) {
      const run = runScript(args, { env: { DISPLAY: "", WAYLAND_DISPLAY: "" } });
      expect(run.status).toBe(0);
      const launch = run.launch ?? [];
      expect(launch.slice(0, 2)).toEqual([
        "--ozone-platform=headless",
        "--ozone-override-screen-size=1920,1080",
      ]);
      for (const word of ["--hyperion-graphics-software", "--use-webgpu-adapter=swiftshader"]) {
        expect(launch).not.toContain(word);
      }
    }
    // A shown run with no display is left to fail as it did: nothing to show it on.
    expect(runScript([], { env: { DISPLAY: "", WAYLAND_DISPLAY: "" } }).launch?.[0]).not.toBe(
      "--ozone-platform=headless",
    );
  });
});
