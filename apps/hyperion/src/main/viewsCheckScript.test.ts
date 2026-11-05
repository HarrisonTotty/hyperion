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

/** The smoke harness's runner, whose SwiftShader switches the software mode takes (R01.T16). */
const TEST_RENDER = join(__dirname, "../../scripts/testRender.sh");

/** The environment the stand-in reports for each launch, `<unset>` for a variable it lacks. */
const REPORTED_ENV = [
  "DISPLAY",
  "WAYLAND_DISPLAY",
  "XDG_SESSION_TYPE",
  "DBUS_SESSION_BUS_ADDRESS",
  "HYPERION_VIEWS_CHECK_HIDDEN",
] as const;

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
  /** Each launch's {@link REPORTED_ENV}, by name. */
  readonly environments: ReadonlyArray<Readonly<Record<string, string>>>;
  readonly output: string;
  readonly status: number | null;
  /** The profiles' directory, which should be empty again. */
  readonly profiles: string;
}

/** What a run of the script is given beyond its arguments. */
interface ScriptSetting {
  /** The stand-in's exit for a launch at each setting (`default` for one with none), 0 otherwise. */
  readonly exits?: Readonly<Record<string, number>>;
  /** The setting of the launch at which the stand-in sends the script TERM, then waits. */
  readonly terminateAt?: string;
  /** Variables the script is run with, beside its own. */
  readonly env?: Readonly<Record<string, string>>;
  /** What a stand-in `uname -s` answers; absent, the machine's own answers. */
  readonly uname?: string;
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
  return runScriptWith(args, { exits, ...(terminateAt === null ? {} : { terminateAt }) });
}

/** A log's lines, each split into its words; none for a log never written. */
function logLines(path: string): ReadonlyArray<ReadonlyArray<string>> {
  return existsSync(path)
    ? readFileSync(path, "utf8")
        .split("\n")
        .filter((line) => line.length > 0)
        .map((line) => line.split(" "))
    : [];
}

/** Runs the script with `args` as {@link ScriptSetting} says. */
function runScriptWith(args: ReadonlyArray<string>, given: ScriptSetting): ScriptRun {
  const exits = given.exits ?? {};
  const terminateAt = given.terminateAt ?? null;
  const dir = mkdtempSync(join(tmpdir(), "hyperion-views-check-script-"));
  scratch = dir;
  const log = join(dir, "launches.log");
  const envLog = join(dir, "environments.log");
  const bin = join(dir, "bin");
  mkdirSync(bin);
  if (given.uname !== undefined) {
    const uname = join(bin, "uname");
    writeFileSync(uname, `#!/usr/bin/env bash\necho ${given.uname}\n`);
    chmodSync(uname, 0o755);
  }
  const pidFile = join(dir, "script.pid");
  const profiles = join(dir, "profiles");
  mkdirSync(profiles);
  const stub = join(dir, "electron");
  writeFileSync(
    stub,
    [
      "#!/usr/bin/env bash",
      `printf '%s\\n' "$*" >> "$STUB_LOG"`,
      `printf '%s\\n' "${REPORTED_ENV.map((name) => `${name}=\${${name}-<unset>}`).join(" ")}" >> "$STUB_ENV_LOG"`,
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
    ...given.env,
    PATH: `${bin}:${process.env["PATH"] ?? "/usr/bin:/bin"}`,
    HOME: dir,
    TMPDIR: profiles,
    STUB_LOG: log,
    STUB_ENV_LOG: envLog,
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
  const environments = logLines(envLog).map((line) =>
    Object.fromEntries(
      line.map((pair) => {
        const at = pair.indexOf("=");
        return [pair.slice(0, at), pair.slice(at + 1)];
      }),
    ),
  );
  return {
    launches: logLines(log),
    environments,
    output: result.stdout,
    status: result.status,
    profiles,
  };
}

/** A bash array's words as `script` assigns it on its own lines, `name=(…)`. */
function bashArray(script: string, name: string): ReadonlyArray<string> {
  const match = new RegExp(`^${name}=\\(([^)]*)\\)`, "mu").exec(readFileSync(script, "utf8"));
  if (match?.[1] === undefined) {
    throw new Error(`${script} assigns no ${name}`);
  }
  return match[1].split(/\s+/u).filter((word) => word.length > 0);
}

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

  it("writes the whole body's field with the native tool first, then gives each launch --whole (R10.T14.c)", () => {
    const dir = mkdtempSync(join(tmpdir(), "hyperion-views-check-cargo-"));
    try {
      const toolLog = join(dir, "cargo.log");
      const cargo = join(dir, "cargo");
      writeFileSync(cargo, `#!/usr/bin/env bash\nprintf '%s\\n' "$*" >> "${toolLog}"\n`);
      chmodSync(cargo, 0o755);
      const run = runScriptWith(["--hidden", "--terrain", "--whole", "--setting", "low"], {
        env: { HYPERION_VIEWS_CHECK_CARGO: cargo },
      });
      const field = join(__dirname, "../../../../target/views-check/terrain/whole.golden");
      expect([
        run.status,
        logLines(toolLog).map((words) => words.join(" ")),
        run.launches.map((launch) => valueOf(launch, "--whole")),
      ]).toEqual([
        0,
        [
          `run -q -p hyperion-surface --release --features testing --example whole_terrain_test -- ${field}`,
        ],
        [field],
      ]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("makes no launch, and exits 2, where the native tool fails to write the whole body's field", () => {
    const run = runScriptWith(["--hidden", "--terrain", "--whole"], {
      env: { HYPERION_VIEWS_CHECK_CARGO: "false" },
    });
    expect([run.status, run.launches.length]).toEqual([2, 0]);
    expect(run.output).toContain("views check: writing the whole body's field");
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

  it("makes two launches of the inspection's frame profile, high then low, for --timing (the owner's report)", () => {
    const run = runScriptWith(
      ["--software", "--inspection", "--universe", "u", "--body", "b", "--timing"],
      {},
    );
    expect([
      run.status,
      run.launches.map((launch) => valueOf(launch, "--setting")),
      run.launches.every((launch) =>
        ["--views-check", "--inspection", "--timing"].every((flag) => launch.includes(flag)),
      ),
      /each launch's limit (\d+) s/u.exec(run.output)?.[1],
    ]).toEqual([0, ["high", "low"], true, "14100"]);
  });

  it("keeps the inspection run's one launch, at the client's default, without --timing", () => {
    const run = runScript(["--inspection", "--universe", "u", "--body", "b"]);
    expect(run.launches.map((launch) => valueOf(launch, "--setting"))).toEqual([null]);
  });

  it("passes --planet-viewer on in one launch, at the inspection run's limit (plan R15, T6)", () => {
    const run = runScriptWith(["--software", "--planet-viewer"], {});
    expect([
      run.launches.map((launch) => launch.includes("--planet-viewer")),
      /each launch's limit (\d+) s/u.exec(run.output)?.[1],
    ]).toEqual([[true], "5100"]);
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

  it("holds its SwiftShader switches equal to testRender.sh's (R01.T16)", () => {
    for (const name of ["adapter_switches", "headless_switches"]) {
      expect(bashArray(SCRIPT, name)).toEqual(bashArray(TEST_RENDER, name));
    }
  });

  it("launches --software hidden, in test-render's environment and switches (R01.T16)", () => {
    const run = runScriptWith(["--software", "--smoke", "--atmosphere"], { env: DESKTOP_ENV });
    const switches = [
      ...bashArray(TEST_RENDER, "adapter_switches"),
      ...bashArray(TEST_RENDER, "headless_switches"),
      ...SOFTWARE_SWITCHES,
    ];
    expect([
      run.status,
      run.launches.map((launch) => launch.slice(0, switches.length)),
      run.launches.some((launch) => launch.includes("--ozone-platform=x11")),
      run.environments,
    ]).toEqual([
      0,
      [switches, switches],
      false,
      [0, 1].map(() => ({
        DISPLAY: "<unset>",
        WAYLAND_DISPLAY: "<unset>",
        XDG_SESSION_TYPE: "<unset>",
        DBUS_SESSION_BUS_ADDRESS: "unix:path=/nonexistent",
        HYPERION_VIEWS_CHECK_HIDDEN: "1",
      })),
    ]);
  });

  it("gives each software launch five times its limit", () => {
    const limits = [
      ["--smoke"],
      ["--smoke", "--atmosphere"],
      ["--terrain", "--setting=high"],
      ["--terrain", "--handover", "--universe", "u", "--body", "b", "--setting=low"],
      ["--atmosphere", "--setting=high"],
    ].map((args) => {
      const run = runScriptWith(["--software", ...args], {});
      return /each launch's limit (\d+) s/u.exec(run.output)?.[1];
    });
    // The atmosphere run's full run takes its places a stage each in the software mode (R08.T17):
    // 40 minutes, five times, above its watchdog's 150.
    expect(limits).toEqual(["1500", "3000", "12000", "18000", "12000"]);
  });

  it("passes --device-pixel-ratio on, and sizes a software launch's headless screen by it", () => {
    const run = runScriptWith(["--software", "--smoke", "--device-pixel-ratio", "2"], {});
    const launch = run.launches[0] ?? [];
    expect([
      run.status,
      valueOf(launch, "--device-pixel-ratio"),
      launch.filter((word) => word.startsWith("--ozone-override-screen-size=")),
    ]).toEqual([0, "2", ["--ozone-override-screen-size=3840,2160"]]);
  });

  it("passes no ratio on at 1, the default", () => {
    const run = runScriptWith(["--hidden", "--smoke", "--device-pixel-ratio=1"], {});
    expect([run.status, run.launches[0]?.includes("--device-pixel-ratio")]).toEqual([0, false]);
  });

  it("refuses a ratio but 1, 2 or 3, with exit 2 and no launch", () => {
    const run = runScriptWith(["--hidden", "--smoke", "--device-pixel-ratio", "1.5"], {});
    expect([run.status, run.launches]).toEqual([2, []]);
  });

  it("refuses --software off Linux, with exit 2 and no launch", () => {
    const run = runScriptWith(["--software", "--smoke"], { uname: "Darwin" });
    expect([run.status, run.launches]).toEqual([2, []]);
  });

  it("keeps a hardware launch's switches and environment as before", () => {
    const run = runScriptWith(["--hidden", "--smoke"], { env: DESKTOP_ENV });
    expect([
      run.launches[0]?.[0],
      run.launches[0]?.includes("--hyperion-graphics-software"),
    ]).toEqual(["--ozone-platform=x11", false]);
    expect(run.environments).toEqual([{ ...DESKTOP_ENV, HYPERION_VIEWS_CHECK_HIDDEN: "1" }]);
    expect(run.output).not.toContain("software mode");
  });

  it("makes T20's one launch, at the client's default, with no --setting", () => {
    const run = runScript(["--smoke"]);
    expect(run.launches).toHaveLength(1);
    expect(valueOf(run.launches[0] ?? [], "--setting")).toBeNull();
    expect(run.launches[0]).not.toContain("--terrain");
  });
});
