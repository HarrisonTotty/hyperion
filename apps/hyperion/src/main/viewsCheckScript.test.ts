import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

/**
 * `scripts/viewsCheck.sh`'s launches (plan R10, R10.T16.d; decision-r10-t16-contracts §6), run
 * with a stand-in for Electron (`HYPERION_VIEWS_CHECK_ELECTRON`) that records its arguments and
 * exits as the test asks. The script needs `setsid` and GNU `timeout`, which only Linux has
 * (R07.T20's pre-existing limit), so the test runs there alone.
 */

/** The script, from this file. */
const SCRIPT = join(__dirname, "../../scripts/viewsCheck.sh");

/** Each test's own directory, removed after it. */
let scratch: string | null = null;

afterEach(() => {
  if (scratch !== null) {
    rmSync(scratch, { recursive: true, force: true });
    scratch = null;
  }
});

/** What a run of the script did: each launch's arguments, its output and its exit. */
interface ScriptRun {
  readonly launches: ReadonlyArray<ReadonlyArray<string>>;
  readonly output: string;
  readonly status: number | null;
  /** The profiles' directory, which should be empty again. */
  readonly profiles: string;
}

/**
 * Runs the script with `args`, the stand-in exiting with `exits[setting]` for a launch at that
 * setting (`default` for one with none), 0 otherwise; a launch at `terminateAt` sends the script
 * TERM and waits to be stopped.
 */
function runScript(
  args: ReadonlyArray<string>,
  exits: Readonly<Record<string, number>> = {},
  terminateAt: string | null = null,
): ScriptRun {
  const dir = mkdtempSync(join(tmpdir(), "hyperion-views-check-script-"));
  scratch = dir;
  const log = join(dir, "launches.log");
  const pidFile = join(dir, "script.pid");
  const profiles = join(dir, "profiles");
  mkdirSync(profiles);
  const stub = join(dir, "electron");
  writeFileSync(
    stub,
    [
      "#!/usr/bin/env bash",
      `printf '%s\\n' "$*" >> "$STUB_LOG"`,
      'setting=default; prev=""',
      'for arg in "$@"; do if [[ "$prev" == --setting ]]; then setting="$arg"; fi; prev="$arg"; done',
      // The launch the test stops the script at: TERM to the script, then a wait to be killed.
      'if [[ "$setting" == "${STUB_TERMINATE_AT:-}" ]]; then kill -TERM "$(cat "$STUB_PID_FILE")"; sleep 10; fi',
      'var="STUB_EXIT_$setting"',
      'exit "${!var:-0}"',
      "",
    ].join("\n"),
  );
  chmodSync(stub, 0o755);
  const env: Record<string, string> = {
    PATH: process.env["PATH"] ?? "/usr/bin:/bin",
    HOME: dir,
    TMPDIR: profiles,
    STUB_LOG: log,
    STUB_PID_FILE: pidFile,
    HYPERION_VIEWS_CHECK_ELECTRON: stub,
    ...(terminateAt === null ? {} : { STUB_TERMINATE_AT: terminateAt }),
  };
  for (const [setting, code] of Object.entries(exits)) {
    env[`STUB_EXIT_${setting}`] = String(code);
  }
  // The script's process ID, for a stand-in to signal: the shell's own, which it becomes.
  const result = spawnSync(
    "bash",
    ["-c", 'echo "$$" > "$STUB_PID_FILE"; exec bash "$0" "$@"', SCRIPT, ...args],
    { env, encoding: "utf8", timeout: 60_000 },
  );
  const launches = existsSync(log)
    ? readFileSync(log, "utf8")
        .split("\n")
        .filter((line) => line.length > 0)
        .map((line) => line.split(" "))
    : [];
  return { launches, output: result.stdout, status: result.status, profiles };
}

/** The value after `flag` in `launch`, or `null`. */
function valueOf(launch: ReadonlyArray<string>, flag: string): string | null {
  const at = launch.indexOf(flag);
  return at < 0 ? null : (launch[at + 1] ?? null);
}

describe.runIf(process.platform === "linux")("the views check's script", () => {
  it("makes two launches, high then low, for --terrain with no --setting", () => {
    const run = runScript(["--hidden", "--smoke", "--terrain"]);
    expect(run.status).toBe(0);
    expect(run.launches.map((launch) => valueOf(launch, "--setting"))).toEqual(["high", "low"]);
    for (const launch of run.launches) {
      expect(launch).toEqual(expect.arrayContaining(["--views-check", "--smoke", "--terrain"]));
    }
    const profileDirs = run.launches.map((launch) =>
      launch.find((arg) => arg.startsWith("--user-data-dir=")),
    );
    expect(new Set(profileDirs).size).toBe(2);
    expect(run.output).toContain("views check: setting high, exit 0");
    expect(run.output).toContain("views check: setting low, exit 0");
    expect(readdirSync(run.profiles)).toEqual([]);
  });

  it("makes two launches of the hand-over check for --terrain --handover (R10.T10.g)", () => {
    const run = runScript(["--hidden", "--terrain", "--handover"]);
    expect([
      run.launches.map((launch) => valueOf(launch, "--setting")),
      run.launches.every((launch) =>
        ["--views-check", "--terrain", "--handover"].every((flag) => launch.includes(flag)),
      ),
    ]).toEqual([["high", "low"], true]);
  });

  it("passes --weathered on to both launches of the hand-over check (R10.T1.c)", () => {
    const run = runScript(["--hidden", "--terrain", "--handover", "--weathered"]);
    expect([
      run.status,
      run.launches.map((launch) => valueOf(launch, "--setting")),
      run.launches.every((launch) =>
        ["--views-check", "--terrain", "--handover", "--weathered"].every((flag) =>
          launch.includes(flag),
        ),
      ),
    ]).toEqual([0, ["high", "low"], true]);
  });

  it("makes two launches of the atmosphere run, high then low, for --atmosphere (R08.T10.b)", () => {
    const run = runScript(["--hidden", "--smoke", "--atmosphere"]);
    expect([
      run.status,
      run.launches.map((launch) => valueOf(launch, "--setting")),
      run.launches.every((launch) =>
        ["--views-check", "--smoke", "--atmosphere"].every((flag) => launch.includes(flag)),
      ),
      run.launches.some((launch) => launch.includes("--terrain")),
      readdirSync(run.profiles),
    ]).toEqual([0, ["high", "low"], true, false, []]);
  });

  it("makes one launch of the atmosphere run at the setting given", () => {
    const run = runScript(["--atmosphere", "--setting=low"]);
    expect([
      run.launches.map((launch) => valueOf(launch, "--setting")),
      run.launches[0]?.includes("--atmosphere"),
    ]).toEqual([["low"], true]);
  });

  it("makes one launch at the setting given", () => {
    const run = runScript(["--terrain", "--setting", "low"]);
    expect(run.launches.map((launch) => valueOf(launch, "--setting"))).toEqual(["low"]);
  });

  it("exits with the first launch's failure, after running both", () => {
    const run = runScript(["--terrain"], { high: 1, low: 3 });
    expect(run.launches).toHaveLength(2);
    expect(run.status).toBe(1);
    expect(run.output).toContain("views check: setting low, exit 3");
  });

  it("exits with the low launch's failure when high passes", () => {
    expect(runScript(["--terrain"], { low: 3 }).status).toBe(3);
  });

  it("makes no second launch once it is terminated during the first, and exits 143", () => {
    const run = runScript(["--terrain"], {}, "high");
    expect([run.launches.length, run.status, readdirSync(run.profiles)]).toEqual([1, 143, []]);
  });

  it("makes T20's one launch, at the client's default, with no --setting", () => {
    const run = runScript(["--smoke"]);
    expect(run.launches).toHaveLength(1);
    expect(valueOf(run.launches[0] ?? [], "--setting")).toBeNull();
    expect(run.launches[0]).not.toContain("--terrain");
  });
});
