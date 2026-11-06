import { spawnSync, type SpawnSyncReturns } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import {
  CAPTURES_WATCHDOG_MS,
  GPU_TIMING_WATCHDOG_MS,
  isOfflineUrl,
  isUncapturedGpuError,
  judgeSmokeRun,
  readSmokeImages,
  readSmokeResult,
  readWatchdogScale,
  SMOKE_EXIT,
  SMOKE_WATCHDOG_BASE_MS,
  SMOKE_WATCHDOG_SCALE_MAX,
  type SmokeResult,
  smokeWatchdogMs,
  THICK_READ_WATCHDOG_MS,
  TRANSFER_WATCHDOG_MS,
  transferSocketUrl,
  UNCAPTURED_GPU_ERROR,
} from "./result";

const PASSING: SmokeResult = {
  variant: "default",
  adapter: "google/swiftshader, fallback true",
  capabilities: "subgroups yes",
  checks: [{ name: "T9.a", pass: true, detail: "ok" }],
  setupError: null,
};

describe("the smoke run's verdict", () => {
  it("passes when every check passes and nothing left the page", () => {
    const { lines, exitCode } = judgeSmokeRun(PASSING, [], []);
    expect(exitCode).toBe(SMOKE_EXIT.pass);
    expect(lines).toContain("PASS T9.a: ok");
    expect(lines).toContain("cancelled requests []");
  });

  it("fails on a failed check", () => {
    const failed = { ...PASSING, checks: [{ name: "T9.d", pass: false, detail: "upside down" }] };
    expect(judgeSmokeRun(failed, [], [])).toMatchObject({ exitCode: SMOKE_EXIT.failed });
  });

  it("fails on a cancelled request, naming its URL and resource type", () => {
    const { lines, exitCode } = judgeSmokeRun(
      PASSING,
      [{ url: "https://example.invalid/a.json", resourceType: "xhr" }],
      [],
    );
    expect(exitCode).toBe(SMOKE_EXIT.failed);
    expect(lines).toContain("cancelled requests [xhr https://example.invalid/a.json]");
  });

  it("fails when the page ran no checks", () => {
    const { lines, exitCode } = judgeSmokeRun({ ...PASSING, checks: [] }, [], []);
    expect(exitCode).toBe(SMOKE_EXIT.failed);
    expect(lines).toContain("FAIL the page ran no checks");
  });

  it("fails on an uncaptured GPU error, printing it", () => {
    const message = `${UNCAPTURED_GPU_ERROR}: Invalid QuerySet`;
    const { lines, exitCode } = judgeSmokeRun(PASSING, [], [message]);
    expect(exitCode).toBe(SMOKE_EXIT.failed);
    expect(lines).toContain("uncaptured GPU errors 1");
    expect(lines).toContain(`GPU ERROR ${message}`);
  });

  it("knows the engine's log of an uncaptured GPU error from other page errors", () => {
    expect(isUncapturedGpuError(`${UNCAPTURED_GPU_ERROR}: Invalid QuerySet`)).toBe(true);
    expect(isUncapturedGpuError("material broken failed to compile")).toBe(false);
  });

  it("is a setup error, exit 2, with no adapter", () => {
    const none = { ...PASSING, adapter: null, checks: [], setupError: "no adapter: no-adapter" };
    expect(judgeSmokeRun(none, [], [])).toMatchObject({ exitCode: SMOKE_EXIT.setup });
  });
});

describe("the page's report", () => {
  it("is read when well formed and refused otherwise", () => {
    expect(readSmokeResult(PASSING)).toEqual(PASSING);
    expect(readSmokeResult({ ...PASSING, checks: [{ name: "x" }] })).toBeNull();
    expect(readSmokeResult("report")).toBeNull();
  });
});

describe("the offline rule", () => {
  it("lets only file: and data: URLs load", () => {
    expect(isOfflineUrl("file:///out/renderer/smoke.html")).toBe(true);
    expect(isOfflineUrl("data:text/plain,x")).toBe(true);
    expect(isOfflineUrl("https://example.invalid/")).toBe(false);
    expect(isOfflineUrl("devtools://devtools")).toBe(false);
  });
});

describe("readSmokeImages", () => {
  /** A 2 × 1 image's eight bytes, base64. */
  const TWO_PIXELS = Buffer.from([1, 2, 3, 255, 4, 5, 6, 255]).toString("base64");
  const image = { name: "hillaire-orbit", width: 2, height: 1, rgba: TWO_PIXELS };

  it("reads well-formed images", () => {
    expect(readSmokeImages({ images: [image] })).toEqual({ images: [image], rejected: 0 });
  });

  it("reads none from a report without images", () => {
    expect(readSmokeImages({ variant: "default" })).toEqual({ images: [], rejected: 0 });
  });

  it("rejects a name that could leave the captures' directory", () => {
    expect(readSmokeImages({ images: [{ ...image, name: "../escape" }] }).rejected).toBe(1);
  });

  it("rejects bytes that do not fill width × height × 4", () => {
    expect(readSmokeImages({ images: [{ ...image, width: 3 }] }).rejected).toBe(1);
    expect(
      readSmokeImages({ images: [{ ...image, width: 0, height: 0, rgba: "" }] }).rejected,
    ).toBe(1);
  });

  it("keeps the good images beside a bad one and counts the bad", () => {
    const { images, rejected } = readSmokeImages({ images: [image, { ...image, rgba: "!!" }] });
    expect(images).toEqual([image]);
    expect(rejected).toBe(1);
  });
});

/** `scripts/testRender.sh`, which sets each run's outer limit. */
const script = (): string => readFileSync(join(__dirname, "../../scripts/testRender.sh"), "utf8");

/** The parts after the first, 2 to 21, which take neither part 1's captures nor its timings. */
const LATER_PARTS = Array.from({ length: 20 }, (_, k) => String(k + 2));

describe("smokeWatchdogMs", () => {
  const plain = {
    part: "1",
    captures: false,
    gpuTiming: false,
    thickRead: false,
    soakSeconds: 0,
    childSeconds: 0,
    scale: 1,
    transfer: false,
  };

  it("is 180 s for a run without the captures", () => {
    expect(SMOKE_WATCHDOG_BASE_MS).toBe(180_000);
    for (const part of ["1", "4", "7", "all"]) {
      expect(smokeWatchdogMs({ ...plain, part })).toBe(180_000);
    }
  });

  it("adds the captures' allowance to part 1 and to every part at once", () => {
    for (const part of ["1", "all"]) {
      expect(smokeWatchdogMs({ ...plain, part, captures: true })).toBe(
        180_000 + CAPTURES_WATCHDOG_MS,
      );
    }
  });

  it("adds nothing for the captures to parts 2 to 21, which take none", () => {
    for (const part of LATER_PARTS) {
      expect(smokeWatchdogMs({ ...plain, part, captures: true })).toBe(180_000);
    }
  });

  it("adds the GPU timings' allowance to part 1 and to every part at once", () => {
    for (const part of ["1", "all"]) {
      expect(smokeWatchdogMs({ ...plain, part, gpuTiming: true })).toBe(
        180_000 + GPU_TIMING_WATCHDOG_MS,
      );
      expect(smokeWatchdogMs({ ...plain, part, gpuTiming: true, captures: true })).toBe(
        180_000 + GPU_TIMING_WATCHDOG_MS + CAPTURES_WATCHDOG_MS,
      );
    }
  });

  it("adds nothing for the GPU timings to parts 2 to 21, which take none", () => {
    for (const part of LATER_PARTS) {
      expect(smokeWatchdogMs({ ...plain, part, gpuTiming: true })).toBe(180_000);
    }
  });

  it("adds the thick read's rows' allowance to part 1's GPU timings, and nothing without them", () => {
    for (const part of ["1", "all"]) {
      expect(smokeWatchdogMs({ ...plain, part, gpuTiming: true, thickRead: true })).toBe(
        180_000 + GPU_TIMING_WATCHDOG_MS + THICK_READ_WATCHDOG_MS,
      );
      expect(smokeWatchdogMs({ ...plain, part, thickRead: true })).toBe(180_000);
    }
    for (const part of LATER_PARTS) {
      expect(smokeWatchdogMs({ ...plain, part, gpuTiming: true, thickRead: true })).toBe(180_000);
    }
    expect(smokeWatchdogMs({ ...plain, gpuTiming: true, thickRead: true, scale: 2 })).toBe(
      2 * (180_000 + GPU_TIMING_WATCHDOG_MS + THICK_READ_WATCHDOG_MS),
    );
  });

  it("adds the soak's and the child window's seconds as before", () => {
    expect(smokeWatchdogMs({ ...plain, soakSeconds: 60, childSeconds: 30 })).toBe(270_000);
    expect(smokeWatchdogMs({ ...plain, captures: true, soakSeconds: 60 })).toBe(
      240_000 + CAPTURES_WATCHDOG_MS,
    );
  });

  it("stretches the work's allowances by the scale", () => {
    expect(smokeWatchdogMs({ ...plain, part: "7", scale: 2 })).toBe(360_000);
    expect(smokeWatchdogMs({ ...plain, captures: true, scale: 2 })).toBe(
      2 * (180_000 + CAPTURES_WATCHDOG_MS),
    );
    expect(smokeWatchdogMs({ ...plain, gpuTiming: true, scale: 3 })).toBe(
      3 * (180_000 + GPU_TIMING_WATCHDOG_MS),
    );
  });

  it("leaves the soak's and the child window's seconds unscaled", () => {
    expect(smokeWatchdogMs({ ...plain, soakSeconds: 60, childSeconds: 30, scale: 2 })).toBe(
      450_000,
    );
  });

  it("adds the transfer check's waits to its run (R10.T15.b)", () => {
    expect(smokeWatchdogMs({ ...plain, part: "all", transfer: true })).toBe(
      180_000 + TRANSFER_WATCHDOG_MS,
    );
    // The page's waits, 60 s for the welcome and 540 s for the answer, and a minute.
    expect(TRANSFER_WATCHDOG_MS).toBe(660_000);
  });

  it("is the allowance testRender.sh adds to part 1's outer limit", () => {
    const allowance = /^captures_allowance_s=(\d+)$/mu.exec(script())?.[1];
    expect(Number(allowance) * 1000).toBe(CAPTURES_WATCHDOG_MS);
  });

  it("leaves testRender.sh's outer limit 120 s over the base watchdog", () => {
    const limit = /^outer_limit_s=(\d+)$/mu.exec(script())?.[1];
    expect(Number(limit) * 1000).toBe(SMOKE_WATCHDOG_BASE_MS + 120_000);
  });

  it("stretches testRender.sh's outer limit with the watchdog, keeping its margin", () => {
    const margin = /^outer_margin_s=(\d+)$/mu.exec(script())?.[1];
    expect(Number(margin) * 1000).toBe(120_000);
    expect(script()).toMatch(
      /^ {4}limit_s=\$\(\( \(outer_limit_s - outer_margin_s\) \* scale \+ outer_margin_s \)\)$/mu,
    );
    expect(script()).toMatch(/^ {8}limit_s=\$\(\(limit_s \+ captures_allowance_s \* scale\)\)$/mu);
  });

  it("keeps testRender.sh's outer limits 120 s over the watchdog at every scale", () => {
    const outerS = Number(/^outer_limit_s=(\d+)$/mu.exec(script())?.[1]);
    const marginS = Number(/^outer_margin_s=(\d+)$/mu.exec(script())?.[1]);
    const allowanceS = Number(/^captures_allowance_s=(\d+)$/mu.exec(script())?.[1]);
    for (let scale = 1; scale <= SMOKE_WATCHDOG_SCALE_MAX; scale++) {
      // The script's two `limit_s` lines, evaluated.
      const limitS = (outerS - marginS) * scale + marginS;
      expect(limitS * 1000).toBe(smokeWatchdogMs({ ...plain, part: "2", scale }) + 120_000);
      expect((limitS + allowanceS * scale) * 1000).toBe(
        smokeWatchdogMs({ ...plain, captures: true, scale }) + 120_000,
      );
    }
  });

  it("passes testRender.sh's scale to every run", () => {
    expect(script()).toMatch(/ --smoke-watchdog-scale="\$scale" /u);
  });

  it("allows testRender.sh the same scales as the harness", () => {
    const largest = /^scale_max=(\d)$/mu.exec(script())?.[1];
    expect(Number(largest)).toBe(SMOKE_WATCHDOG_SCALE_MAX);
    expect(script()).toMatch(/^ {4}\[1-\$\{scale_max\}\]\) ;;$/mu);
  });
});

/**
 * Runs a copy of `scripts/testRender.sh` with `HYPERION_SMOKE_WATCHDOG_SCALE` at `scale` and `args`,
 * in a directory of its own where no Electron can be found, so that a refusal lost would launch
 * nothing.
 */
function runScriptAtScale(
  scale: string,
  args: ReadonlyArray<string> = [],
): SpawnSyncReturns<string> {
  const dir = mkdtempSync(join(tmpdir(), "hyperion-test-render-script-"));
  try {
    mkdirSync(join(dir, "scripts"));
    const copy = join(dir, "scripts", "testRender.sh");
    writeFileSync(copy, script());
    return spawnSync("bash", [copy, ...args], {
      env: { PATH: process.env["PATH"] ?? "/usr/bin:/bin", HYPERION_SMOKE_WATCHDOG_SCALE: scale },
      encoding: "utf8",
      timeout: 30_000,
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// The script needs bash, which Windows has only under WSL or Git Bash.
describe.runIf(process.platform !== "win32")("testRender.sh's scale", () => {
  it("refuses a scale that is not a whole number from 1 to the largest, before any run", () => {
    for (const scale of ["0", "2x", String(SMOKE_WATCHDOG_SCALE_MAX + 1)]) {
      const run = runScriptAtScale(scale);
      expect(run.status).toBe(2);
      expect(run.stderr).toContain(
        `HYPERION_SMOKE_WATCHDOG_SCALE must be a whole number from 1 to ${SMOKE_WATCHDOG_SCALE_MAX}, not '${scale}'`,
      );
    }
  });
});

describe.runIf(process.platform !== "win32")("testRender.sh's parts", () => {
  it("takes every part from 1 to 21 as --part= and refuses 0 and 22, before any run", () => {
    for (const part of ["1", ...LATER_PARTS]) {
      expect(runScriptAtScale("1", [`--part=${part}`]).stderr).not.toContain("unknown argument");
    }
    for (const part of ["0", "22"]) {
      const refused = runScriptAtScale("1", [`--part=${part}`]);
      expect(refused.status).toBe(2);
      expect(refused.stderr).toContain(`unknown argument --part=${part}`);
    }
  });

  it("runs every part from 1 to 21 when none is asked for", () => {
    const parts = /^ {4}parts=\(([\d ]+)\)$/mu.exec(script())?.[1];
    expect(parts).toBe(["1", ...LATER_PARTS].join(" "));
  });
});

/** The words of the array `name` that `scripts/testRender.sh` assigns at the start of a line. */
function scriptArray(name: string): ReadonlyArray<string> {
  const match = new RegExp(`^${name}=\\(([^)]*)\\)`, "mu").exec(script());
  if (match?.[1] === undefined) {
    throw new Error(`testRender.sh assigns no ${name}`);
  }
  return match[1].split(/\s+/u).filter((word) => word.length > 0);
}

/**
 * Runs a copy of `scripts/testRender.sh` with `args` on a stand-in Electron, which writes the words
 * it is launched with and exits 0, and gives the script's exit code and each launch's words.
 */
function launchesOf(args: ReadonlyArray<string>): {
  readonly status: number | null;
  readonly launches: ReadonlyArray<ReadonlyArray<string>>;
} {
  const dir = mkdtempSync(join(tmpdir(), "hyperion-test-render-launch-"));
  try {
    const launches = join(dir, "launches");
    for (const sub of ["scripts", "node_modules/electron", "node_modules/.bin", "launches"]) {
      mkdirSync(join(dir, sub), { recursive: true });
    }
    writeFileSync(join(dir, "scripts", "testRender.sh"), script());
    writeFileSync(join(dir, "node_modules", "electron", "index.js"), "module.exports = '';\n");
    writeFileSync(
      join(dir, "node_modules", ".bin", "electron"),
      `#!/usr/bin/env bash\nprintf '%s\\n' "$@" > "${launches}/$$"\n`,
      { mode: 0o755 },
    );
    const run = spawnSync("bash", [join(dir, "scripts", "testRender.sh"), ...args], {
      env: { PATH: process.env["PATH"] ?? "/usr/bin:/bin", HYPERION_SMOKE_WATCHDOG_SCALE: "1" },
      encoding: "utf8",
      timeout: 30_000,
    });
    return {
      status: run.status,
      launches: readdirSync(launches).map((name) =>
        readFileSync(join(launches, name), "utf8")
          .split("\n")
          .filter((word) => word.length > 0),
      ),
    };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// `setsid` is util-linux's, which macOS has only through the justfile's `tools/portable` stand-ins.
describe.runIf(process.platform === "linux")("testRender.sh's launch", () => {
  it("launches Electron with Chromium's GPU watchdog off, after the adapter and headless switches", () => {
    const switches = [
      ...scriptArray("adapter_switches"),
      ...scriptArray("headless_switches"),
      "--disable-gpu-watchdog",
    ];
    const run = launchesOf(["--part=1", "--variant=default"]);
    expect([run.status, run.launches.map((words) => words.slice(0, switches.length))]).toEqual([
      0,
      [switches],
    ]);
  });

  it("keeps Chromium's GPU watchdog off when the adapter's switches are dropped", () => {
    const switches = [...scriptArray("headless_switches"), "--disable-gpu-watchdog"];
    const run = launchesOf(["--part=1", "--variant=default", "--drop-adapter-switches"]);
    expect([run.status, run.launches.map((words) => words.slice(0, switches.length))]).toEqual([
      0,
      [switches],
    ]);
  });
});

describe("readWatchdogScale", () => {
  it("reads a whole number from 1 to the largest scale", () => {
    expect(readWatchdogScale("1")).toBe(1);
    expect(readWatchdogScale("2")).toBe(2);
    expect(readWatchdogScale(String(SMOKE_WATCHDOG_SCALE_MAX))).toBe(SMOKE_WATCHDOG_SCALE_MAX);
  });

  it("refuses anything else", () => {
    for (const text of [
      "",
      "0",
      "01",
      "1.5",
      "-1",
      "2x",
      " 2",
      String(SMOKE_WATCHDOG_SCALE_MAX + 1),
    ]) {
      expect(readWatchdogScale(text)).toBeNull();
    }
  });
});

describe("the transfer check's server (R10.T15.b)", () => {
  it("takes a loopback server's WebSocket at /ws, with its port", () => {
    expect(transferSocketUrl("ws://127.0.0.1:7881/ws")).toBe("ws://127.0.0.1:7881/ws");
    expect(transferSocketUrl("ws://localhost:7881/ws")).toBe("ws://localhost:7881/ws");
    expect(transferSocketUrl("ws://[::1]:7881/ws")).toBe("ws://[::1]:7881/ws");
  });

  it.each([
    "",
    "not a url",
    "wss://127.0.0.1:7881/ws",
    "http://127.0.0.1:7881/ws",
    "ws://10.0.0.2:7881/ws",
    "ws://example.com:7881/ws",
    "ws://127.0.0.1/ws",
    "ws://127.0.0.1:7881/",
    "ws://127.0.0.1:7881/ws?x=1",
    "ws://user@127.0.0.1:7881/ws",
  ])("refuses %j, so that every other request stays cancelled", (value) => {
    expect(transferSocketUrl(value)).toBeNull();
  });
});
