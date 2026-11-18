import { describe, expect, it } from "vitest";

import { parseFdinfo, parseNvidiaSmi } from "../fdinfo";
import type { GpuIdentity } from "../gpuClocks";
import { type Measured, measured, missing } from "../measured";
import {
  commandFixture,
  DESKTOP_SYS,
  fdinfoFixture,
  fixtureCommands,
  fixtureFiles,
  procFixture,
  X1_YOGA_SYS,
} from "./fixtures/sysfs";
import {
  appMemoryOf,
  displayServerFiguresOf,
  type DrmMemorySample,
  drmDriversOf,
  type Every,
  findDisplayServerPid,
  gpuMemoryHeadline,
  LARGE_RELEASE_BYTES,
  type MemorySamplerSources,
  nvidiaBaseline,
  nvidiaMemoryOf,
  RELEASE_SETTLE_MS,
  sampleDisplayServer,
  sampleGpuMemory,
  sampleNvidia,
  summariseDrm,
  summariseNvidia,
  type GpuClockReading,
  GpuClockSampler,
  NVIDIA_CLOCK_FIELDS,
  NvidiaClockStream,
  nvidiaClockArgs,
  parseNvidiaClockLine,
  parsePmsetTherm,
  type PerfSample,
  PerfSampler,
  readNumber,
  NVIDIA_MAX_REASON,
  type SamplerSources,
  sampleThermal,
  SPAWN_LINES,
  sampleThrottle,
  type SpawnLines,
  summariseSamples,
  type ThermalSample,
  type ThrottleSample,
} from "./samplers";

const UHD_620: Measured<GpuIdentity> = measured({ vendorId: 0x80_86, deviceId: 0x3e_a0 });
const RTX_3080: Measured<GpuIdentity> = measured({ vendorId: 0x10_de, deviceId: 0x22_06 });

function sources(
  sysRoot: string,
  platform: NodeJS.Platform = "linux",
  files = fixtureFiles(sysRoot),
): SamplerSources {
  return {
    platform,
    files,
    sysRoot,
    run: fixtureCommands({ "pmset -g therm": commandFixture("pmset-therm-nominal.txt") }),
  };
}

/** A callback that does nothing, before the fake's child is started. */
function ignore(): void {
  // Nothing to do.
}

/** A spawner that hands each started child's line and end callbacks to the test. */
function fakeSpawn(): {
  readonly spawn: SpawnLines;
  readonly started: { file: string; args: ReadonlyArray<string> }[];
  line(text: string): void;
  end(reason: string): void;
  readonly stopped: () => number;
} {
  const started: { file: string; args: ReadonlyArray<string> }[] = [];
  let onLine: (line: string) => void = ignore;
  let onEnd: (reason: string) => void = ignore;
  let stops = 0;
  return {
    started,
    spawn: (file, args, line, end) => {
      started.push({ file, args });
      onLine = line;
      onEnd = end;
      return {
        stop: () => {
          stops += 1;
        },
      };
    },
    line: (text) => {
      onLine(text);
    },
    end: (reason) => {
      onEnd(reason);
    },
    stopped: () => stops,
  };
}

describe("the thermal sampler", () => {
  it("reads the laptop's x86_pkg_temp zone and every zone beside it", async () => {
    const sample = await sampleThermal(sources(X1_YOGA_SYS), 0);
    expect(sample.packageDegC).toEqual(measured(46));
    expect(sample.packageSource).toBe("x86_pkg_temp");
    expect(sample.zonesDegC).toEqual({
      acpitz: 38,
      SEN1: 34.05,
      "INT3400 Thermal": 20,
      pch_cannonlake: 39,
      B0D4: 38.05,
      x86_pkg_temp: 46,
      iwlwifi_1: 44,
    });
  });

  it("leaves out a zone whose temperature cannot be read", async () => {
    const files = fixtureFiles(X1_YOGA_SYS, {
      denied: [`${X1_YOGA_SYS}/class/thermal/thermal_zone6/temp`],
    });
    const sample = await sampleThermal(sources(X1_YOGA_SYS, "linux", files), 0);
    expect(sample.zonesDegC).not.toHaveProperty("iwlwifi_1");
    expect(sample.zonesDegC).toHaveProperty("x86_pkg_temp", 46);
  });

  it("falls back to k10temp's Tctl on the desktop, which has no thermal zone", async () => {
    const sample = await sampleThermal(sources(DESKTOP_SYS), 0);
    expect(sample.packageDegC).toEqual(measured(45.25));
    expect(sample.packageSource).toBe("k10temp Tctl");
    expect(sample.zonesDegC).toEqual({});
  });

  it("gives the energy counter as null while it is root-only, and reads it once it is not", async () => {
    const closed = await sampleThermal(sources(X1_YOGA_SYS), 0);
    expect(closed.energy.value).toBeNull();
    expect(closed.energy.reason).toMatch(/intel-rapl-mmio:0\/energy_uj .*EACCES.*root-only/u);
    const open = await sampleThermal(
      sources(X1_YOGA_SYS, "linux", fixtureFiles(X1_YOGA_SYS, { denied: [] })),
      0,
    );
    expect(open.energy).toEqual(measured({ energyUj: 41_927_318_402, rangeUj: 262_143_328_850 }));
  });

  it("reports the /sys readings unavailable on macOS and reads its thermal pressure", async () => {
    const sample = await sampleThermal(sources(X1_YOGA_SYS, "darwin"), 0);
    expect(sample.packageDegC).toEqual(missing("/sys is Linux's; this is darwin"));
    expect(sample.energy.value).toBeNull();
    expect(sample.zonesDegC).toEqual({});
    expect(sample.pressure).toEqual(
      measured({ nominal: true, detail: "", cpuSpeedLimitPercent: null }),
    );
  });

  it("reads nothing on Windows", async () => {
    const sample = await sampleThermal(sources(X1_YOGA_SYS, "win32"), 0);
    expect(sample.packageDegC.reason).toMatch(/win32/u);
    expect(sample.pressure.value).toBeNull();
  });
});

describe("a missing file", () => {
  it("is null with its path, never a guess", async () => {
    const read = await readNumber(fixtureFiles(DESKTOP_SYS), `${DESKTOP_SYS}/class/thermal/x/temp`);
    expect(read.value).toBeNull();
    expect(read.reason).toMatch(/class\/thermal\/x\/temp could not be read \(ENOENT\)/u);
  });
});

describe("pmset -g therm", () => {
  it("is nominal where nothing has been recorded", () => {
    expect(parsePmsetTherm(commandFixture("pmset-therm-nominal.txt")).nominal).toBe(true);
  });

  it("is pressure where the CPU's speed is limited below 100%", () => {
    const pressure = parsePmsetTherm(commandFixture("pmset-therm-pressure.txt"));
    expect(pressure.nominal).toBe(false);
    expect(pressure.cpuSpeedLimitPercent).toBe(71);
    expect(pressure.detail).toMatch(/CPU_Speed_Limit = 71/u);
  });

  it("is pressure at a warning level above 0", () => {
    expect(parsePmsetTherm("Thermal warning level set to 2.").nominal).toBe(false);
    expect(parsePmsetTherm("Thermal warning level set to 0.").nominal).toBe(true);
  });
});

describe("the throttle sampler", () => {
  it("reads the laptop's throttle counters, CPU clocks and i915's clock against RP0", async () => {
    const gpu = new GpuClockSampler({ sources: sources(X1_YOGA_SYS), gpu: UHD_620, nvidia: null });
    const sample = await sampleThrottle(sources(X1_YOGA_SYS), gpu, 5);
    expect(sample.atMs).toBe(5);
    expect(Object.keys(sample.packageThrottleMs)).toHaveLength(8);
    expect(sample.packageThrottleMs["cpu0"]).toBe(524_127);
    expect(sample.coreThrottleMs["cpu4"]).toBe(286_008);
    expect(sample.cpuMHz[0]).toBe(3400.055);
    // Idle when captured: i915 at 317 MHz, below its RP0 of 1,150.
    expect(sample.gpu).toEqual({
      source: "i915-sysfs",
      mhz: measured(317),
      maxMHz: measured(1150),
      slowdown: missing("i915-sysfs reports no slowdown reason"),
    });
  });

  it("reads no throttle counter on the desktop, which has none", async () => {
    const stream = new NvidiaClockStream({
      spawn: fakeSpawn().spawn,
      intervalMs: 100,
      gpuIndex: 0,
      now: () => 0,
    });
    const gpu = new GpuClockSampler({
      sources: sources(DESKTOP_SYS),
      gpu: RTX_3080,
      nvidia: stream,
    });
    const sample = await sampleThrottle(sources(DESKTOP_SYS), gpu, 0);
    expect(sample.packageThrottleMs).toEqual({});
    expect(sample.cpuMHz).toEqual([3599.382, 2199.945, 4391.204, 2200]);
    expect(sample.gpu).toEqual({ source: "none", reason: "nvidia-smi was not started" });
  });

  it("reports the CPUs' files unavailable off Linux", async () => {
    const gpu = new GpuClockSampler({
      sources: sources(X1_YOGA_SYS, "darwin"),
      gpu: UHD_620,
      nvidia: null,
    });
    const sample = await sampleThrottle(sources(X1_YOGA_SYS, "darwin"), gpu, 0);
    expect(sample.cpuReason).toBe("/sys is Linux's; this is darwin");
    expect(sample.cpuMHz).toEqual([]);
    expect(sample.gpu.source).toBe("none");
  });
});

/** A stream over `fake`, its clock `clock.nowMs`. */
function streamOf(
  fake: ReturnType<typeof fakeSpawn>,
  clock: { nowMs: number } = { nowMs: 0 },
): NvidiaClockStream {
  return new NvidiaClockStream({
    spawn: fake.spawn,
    intervalMs: 100,
    gpuIndex: 0,
    now: () => clock.nowMs,
  });
}

describe("the long-lived nvidia-smi child", () => {
  it("asks for its fields as CSV every interval", () => {
    expect(nvidiaClockArgs(100)).toEqual([
      `--query-gpu=${NVIDIA_CLOCK_FIELDS.join(",")}`,
      "--format=csv,noheader,nounits",
      "-lms",
      "100",
    ]);
  });

  it("parses its lines, N/A as null and any active slowdown reason as slowed", () => {
    const [idle, capped, unsupported, error] = commandFixture("nvidia-smi-clocks.csv").split("\n");
    expect(parseNvidiaClockLine(idle ?? "")).toEqual({
      index: 0,
      graphicsMHz: 1965,
      maxGraphicsMHz: 2100,
      slowdown: false,
      temperatureDegC: 64,
    });
    expect(parseNvidiaClockLine(capped ?? "")?.slowdown).toBe(true);
    expect(parseNvidiaClockLine(unsupported ?? "")).toMatchObject({
      graphicsMHz: null,
      slowdown: null,
    });
    expect(parseNvidiaClockLine(error ?? "")).toBeNull();
  });

  it("starts one child however often it is started", () => {
    const fake = fakeSpawn();
    const stream = streamOf(fake);
    stream.start();
    stream.start();
    expect(fake.started).toEqual([{ file: "nvidia-smi", args: nvidiaClockArgs(100) }]);
    expect(stream.latest()).toEqual(missing("no line from nvidia-smi yet"));
  });

  it("gives its GPU's latest line to the GPU sampler, with no maximum to share time at", async () => {
    const fake = fakeSpawn();
    const stream = streamOf(fake);
    stream.start();
    fake.line("1, 300, 2100, Not Active, Not Active, Not Active, Not Active, Not Active, 40");
    fake.line(commandFixture("nvidia-smi-clocks.csv").split("\n")[1] ?? "");
    const gpu = new GpuClockSampler({
      sources: sources(DESKTOP_SYS),
      gpu: RTX_3080,
      nvidia: stream,
    });
    expect(await gpu.read()).toEqual({
      source: "nvidia-smi",
      mhz: measured(1890),
      maxMHz: missing(NVIDIA_MAX_REASON),
      slowdown: measured(true),
    });
  });

  it("reads a line older than ten intervals as stale", () => {
    const fake = fakeSpawn();
    const clock = { nowMs: 0 };
    const stream = streamOf(fake, clock);
    stream.start();
    fake.line(commandFixture("nvidia-smi-clocks.csv").split("\n")[0] ?? "");
    clock.nowMs = 1001;
    expect(stream.latest().reason).toBe("nvidia-smi's latest line is 1001 ms old");
  });

  it("ends its child when stopped", () => {
    const fake = fakeSpawn();
    const stream = streamOf(fake);
    stream.start();
    stream.stop();
    expect(fake.stopped()).toBe(1);
    expect(stream.latest()).toEqual(missing("stopped"));
  });

  it("gives the reason it ended, such as no nvidia-smi on the machine", () => {
    const fake = fakeSpawn();
    const stream = new NvidiaClockStream({
      spawn: fake.spawn,
      intervalMs: 100,
      gpuIndex: 0,
      now: () => 0,
    });
    stream.start();
    fake.end("no nvidia-smi on this machine");
    expect(stream.latest()).toEqual(missing("no nvidia-smi on this machine"));
  });
});

/** Runs `SPAWN_LINES` on Node itself with `script`, resolving its lines and its end. */
function run(
  script: string,
  stopAfter: number | null = null,
): Promise<{ readonly lines: string[]; readonly end: string | null }> {
  return new Promise((resolve) => {
    const lines: string[] = [];
    const child = SPAWN_LINES(
      process.execPath,
      ["-e", script],
      (line) => {
        lines.push(line);
        if (lines.length === stopAfter) {
          child.stop();
          // A stopped child reports no end; give its exit a moment to prove it.
          setTimeout(() => {
            resolve({ lines, end: null });
          }, 200);
        }
      },
      (end) => {
        resolve({ lines, end });
      },
    );
  });
}

describe("a child read a line at a time", () => {
  it("gives each line of the child's output", async () => {
    const { lines } = await run('console.log("0, 1965"); console.log("0, 1890");');
    expect(lines).toEqual(["0, 1965", "0, 1890"]);
  });

  it("ends with the child's exit code and the tail of its standard error", async () => {
    const { end } = await run(
      'console.error("Field is not a valid field to query."); process.exit(2);',
    );
    expect(end).toMatch(/exited \(2\): Field is not a valid field to query\.$/u);
  });

  it("ends with no such program where the file is missing", async () => {
    const result = await new Promise<string>((resolve) => {
      SPAWN_LINES("hyperion-no-such-program", [], () => undefined, resolve);
    });
    expect(result).toBe("no hyperion-no-such-program on this machine");
  });

  it("reports neither a line nor an end once stopped", async () => {
    const { lines, end } = await run(
      'console.log("a"); setTimeout(() => console.log("b"), 100); setTimeout(() => {}, 2000);',
      1,
    );
    expect(lines).toEqual(["a"]);
    expect(end).toBeNull();
  });
});

/** A synthetic sample at `atMs`. */
function synthetic(
  atMs: number,
  fields: {
    readonly pkgMs?: number;
    readonly coreMs?: Readonly<Record<string, number>>;
    readonly gpuMHz?: number;
    readonly maxMHz?: number;
    readonly slowdown?: boolean;
    readonly degC?: number;
    readonly zones?: Readonly<Record<string, number>>;
    readonly energyUj?: number;
    readonly cpuMHz?: ReadonlyArray<number>;
  },
): PerfSample {
  const gpu: GpuClockReading =
    fields.gpuMHz === undefined
      ? { source: "none", reason: "none" }
      : {
          source: "i915-sysfs",
          mhz: measured(fields.gpuMHz),
          maxMHz: measured(fields.maxMHz ?? 1150),
          slowdown: fields.slowdown === undefined ? missing("none") : measured(fields.slowdown),
        };
  const thermal: ThermalSample = {
    atMs,
    packageDegC: fields.degC === undefined ? missing("none") : measured(fields.degC),
    packageSource: fields.degC === undefined ? null : "x86_pkg_temp",
    zonesDegC: fields.zones ?? {},
    energy:
      fields.energyUj === undefined
        ? missing("root-only")
        : measured({ energyUj: fields.energyUj, rangeUj: 1_000_000 }),
    pressure: missing("none"),
  };
  const throttle: ThrottleSample = {
    atMs,
    packageThrottleMs: fields.pkgMs === undefined ? {} : { cpu0: fields.pkgMs, cpu1: fields.pkgMs },
    coreThrottleMs: fields.coreMs ?? {},
    cpuMHz: fields.cpuMHz ?? [],
    gpu,
    cpuReason: null,
  };
  return { thermal, throttle };
}

describe("a repetition's summary", () => {
  it("is the package's throttle fraction over a synthetic trace", () => {
    const figures = summariseSamples([
      synthetic(0, { pkgMs: 1000, coreMs: { cpu0: 50, cpu1: 70 } }),
      synthetic(500, { pkgMs: 1100 }),
      synthetic(1000, { pkgMs: 1300, coreMs: { cpu0: 150, cpu1: 90 } }),
    ]);
    expect(figures.packageThrottleMs).toBe(300);
    expect(figures.throttleFraction).toBeCloseTo(0.3, 12);
    // The most any core rose: cpu0's 100 ms.
    expect(figures.coreThrottleMs).toBe(100);
  });

  it("is the GPU's share at RP0, its minimum and median clock, over a synthetic trace", () => {
    const figures = summariseSamples([
      synthetic(0, { gpuMHz: 1150 }),
      synthetic(100, { gpuMHz: 1150 }),
      synthetic(200, { gpuMHz: 900 }),
      synthetic(300, { gpuMHz: 1150 }),
    ]);
    expect(figures.rp0Share).toBe(0.75);
    expect(figures.gpuMinMHz).toBe(900);
    expect(figures.gpuMedianMHz).toBe(1150);
  });

  it("is NVIDIA's share slowed, the CPUs' median, and the hottest package and zones", () => {
    const figures = summariseSamples([
      synthetic(0, {
        gpuMHz: 1900,
        maxMHz: 2100,
        slowdown: false,
        degC: 50,
        zones: { acpitz: 40 },
        cpuMHz: [3900, 3800],
      }),
      synthetic(100, {
        gpuMHz: 1800,
        maxMHz: 2100,
        slowdown: true,
        degC: 61,
        zones: { acpitz: 44 },
        cpuMHz: [3700],
      }),
    ]);
    expect(figures.gpuThrottleShare).toBe(0.5);
    expect(figures.cpuMedianMHz).toBe(3800);
    expect(figures.maxPackageDegC).toBe(61);
    expect(figures.zoneMaxDegC).toEqual({ acpitz: 44 });
  });

  it("is the package's mean power, a wrap of the energy counter undone", () => {
    // 1,000,000 µJ over 200 ms is 5 W; the counter wraps at 1,000,000 µJ after the first sample.
    const figures = summariseSamples([
      synthetic(0, { energyUj: 900_000 }),
      synthetic(100, { energyUj: 400_000 }),
      synthetic(200, { energyUj: 900_000 }),
    ]);
    expect(figures.packageWatts).toBeCloseTo(5, 12);
  });

  it("ignores a counter that fell, which was reset", () => {
    const figures = summariseSamples([
      synthetic(0, { pkgMs: 5000 }),
      synthetic(1000, { pkgMs: 10 }),
    ]);
    expect(figures.packageThrottleMs).toBeNull();
    expect(figures.throttleFraction).toBeNull();
  });

  it("is null for every figure no sample read", () => {
    expect(summariseSamples([synthetic(0, {})])).toEqual({
      packageThrottleMs: null,
      coreThrottleMs: null,
      throttleFraction: null,
      rp0Share: null,
      gpuMinMHz: null,
      gpuMedianMHz: null,
      gpuThrottleShare: null,
      cpuMedianMHz: null,
      maxPackageDegC: null,
      zoneMaxDegC: {},
      packageWatts: null,
    });
  });
});

function tickOf(ticks: ReadonlyArray<() => void>): () => void {
  const tick = ticks[0];
  if (tick === undefined) {
    throw new Error("the sampler set no interval");
  }
  return tick;
}

describe("the 10 Hz sampler", () => {
  /** A sampler over the laptop's tree whose ticks the test fires, and its clock. */
  function laptopSampler(): {
    readonly sampler: PerfSampler;
    readonly ticks: (() => void)[];
    readonly clock: { nowMs: number };
    readonly cancelled: () => number;
  } {
    const ticks: (() => void)[] = [];
    let cancels = 0;
    const every: Every = (callback, ms) => {
      expect(ms).toBe(100);
      ticks.push(callback);
      return () => {
        cancels += 1;
      };
    };
    const clock = { nowMs: 0 };
    const laptop = sources(X1_YOGA_SYS);
    const sampler = new PerfSampler({
      sources: laptop,
      gpu: new GpuClockSampler({ sources: laptop, gpu: UHD_620, nvidia: null }),
      intervalMs: 100,
      now: () => clock.nowMs,
      every,
    });
    return { sampler, ticks, clock, cancelled: () => cancels };
  }

  it("sets one interval however often it is started, and clears it when stopped", () => {
    const { sampler, ticks, cancelled } = laptopSampler();
    sampler.start();
    sampler.start();
    expect(ticks).toHaveLength(1);
    sampler.stop();
    expect(cancelled()).toBe(1);
  });

  it("skips a tick while the one before it is still reading", async () => {
    const { sampler, ticks } = laptopSampler();
    sampler.start();
    const tick = tickOf(ticks);
    tick();
    tick();
    await expect.poll(() => sampler.samples.length).toBe(1);
    expect(sampler.failure).toBeNull();
  });

  it("summarises the samples of a span", async () => {
    const { sampler, clock } = laptopSampler();
    await sampler.sampleOnce();
    clock.nowMs = 100;
    await sampler.sampleOnce();
    const figures = sampler.summarise(0, 100);
    expect(figures).toMatchObject({
      packageThrottleMs: 0,
      throttleFraction: 0,
      rp0Share: 0,
      maxPackageDegC: 46,
    });
    expect(sampler.summarise(200, 300).gpuMedianMHz).toBeNull();
  });

  it("drops the samples before a time", async () => {
    const { sampler, clock } = laptopSampler();
    await sampler.sampleOnce();
    clock.nowMs = 100;
    await sampler.sampleOnce();
    sampler.discardBefore(100);
    expect(sampler.samples.map((s) => s.thermal.atMs)).toEqual([100]);
  });
});

// --- The memory samplers (R12.T3.b) ---

const KIB = 1024;
const MIB = 1024 ** 2;
/** The GPU process's ID, as the desktop's `nvidia-smi` fixtures list it. */
const GPU_PID = 52_144;
const XORG_PID = 1909;
const I915 = drmDriversOf(0x80_86);

function memorySources(
  files = procFixture({}),
  platform: NodeJS.Platform = "linux",
  commands: Readonly<Record<string, string>> = {},
): MemorySamplerSources {
  return { platform, files, procRoot: "/proc", run: fixtureCommands(commands) };
}

/**
 * The UHD 620's GPU process five minutes into a low descent (2026-11-18): six i915 clients on nine
 * descriptors, client 17 behind four of them, and two clients holding resident memory.
 */
const UHD_620_GPU = {
  [String(GPU_PID)]: {
    "22": "uhd620-gpu-client11.txt",
    "23": "uhd620-gpu-client15.txt",
    "30": "uhd620-gpu-client16.txt",
    "43": "uhd620-gpu-client17.txt",
    "47": "uhd620-gpu-client17.txt",
    "48": "uhd620-gpu-client17.txt",
    "49": "uhd620-gpu-client17.txt",
    "74": "uhd620-gpu-client19.txt",
    "77": "uhd620-gpu-client23.txt",
  },
};

describe("the GPU process's kernel memory", () => {
  it("parses the UHD 620's i915 text: system0's total, shared and resident, and stolen-system0", () => {
    expect(parseFdinfo(fdinfoFixture("uhd620-gpu-client15.txt"))).toMatchObject({
      driver: "i915",
      clientId: "15",
      totalBytes: { system0: 67_152 * KIB, "stolen-system0": 0 },
      sharedBytes: { system0: 14_696 * KIB, "stolen-system0": 0 },
      residentBytes: { system0: 61_008 * KIB, "stolen-system0": 0 },
      engineNs: { render: 54_327_119_215 },
    });
  });

  it("counts the descriptors of one client once, and states the lower bound beside the sum", async () => {
    const sample = await sampleGpuMemory(GPU_PID, I915, memorySources(procFixture(UHD_620_GPU)), 7);
    expect(sample).toEqual(
      measured({
        atMs: 7,
        residentBytes: (61_008 + 186_168) * KIB,
        // Client 17's 268 KiB once, not four times.
        totalBytes: (67_152 + 268 + 199_320) * KIB,
        sharedBytes: (14_696 + 60) * KIB,
        // Σresident − ½·Σshared: memory shared between two clients counted once at most.
        residentLowerBoundBytes: (61_008 + 186_168 - (14_696 + 60) / 2) * KIB,
        busyNs: 54_327_119_215 + 56_437_356_533,
        clients: 6,
        drivers: ["i915"],
      }),
    );
  });

  it("skips an fdinfo of another driver", async () => {
    const files = procFixture({
      [String(GPU_PID)]: { ...UHD_620_GPU[String(GPU_PID)], "13": "r05:amdgpu-client35.txt" },
    });
    const sample = await sampleGpuMemory(GPU_PID, I915, memorySources(files), 0);
    expect(sample.value).toMatchObject({ clients: 6, drivers: ["i915"] });
    const every = await sampleGpuMemory(GPU_PID, null, memorySources(files), 0);
    expect(every.value).toMatchObject({ clients: 7, drivers: ["amdgpu", "i915"] });
  });

  it("is null with its reason on NVIDIA, whose driver writes no DRM keys", async () => {
    const files = procFixture({ [String(GPU_PID)]: { "10": "r05:nvidia-renderD128.txt" } });
    expect(await sampleGpuMemory(GPU_PID, null, memorySources(files), 0)).toEqual(
      missing("no DRM client in the process's fdinfo (NVIDIA's driver writes none)"),
    );
  });

  it("is null for an unreadable fdinfo, and off Linux", async () => {
    const files = procFixture(UHD_620_GPU, {}, [String(GPU_PID)]);
    const unreadable = await sampleGpuMemory(GPU_PID, I915, memorySources(files), 0);
    expect(unreadable.reason).toMatch(/\/proc\/52144\/fdinfo could not be read: EACCES/u);
    const mac = await sampleGpuMemory(
      GPU_PID,
      I915,
      memorySources(procFixture(UHD_620_GPU), "darwin"),
      0,
    );
    expect(mac).toEqual(missing("DRM fdinfo is Linux's; this is darwin"));
  });
});

describe("the display server", () => {
  // Xorg's one client behind five descriptors, as captured beside the GPU process.
  const xorg = {
    [String(XORG_PID)]: Object.fromEntries(
      ["13", "14", "15", "16", "17"].map((fd) => [fd, "uhd620-xorg-client4.txt"]),
    ),
  };
  const comm = { [String(XORG_PID)]: "Xorg", "901": "i3", [String(GPU_PID)]: "electron" };

  it("is Xorg's fdinfo where it is readable", async () => {
    const x11 = memorySources(procFixture(xorg, comm));
    const pid = await findDisplayServerPid(x11);
    expect(pid).toBe(XORG_PID);
    const sample = await sampleDisplayServer(pid, I915, x11, 0);
    expect(sample).toMatchObject({ residentBytes: 23_024 * KIB, busyNs: 15_193_131_076 });
  });

  it("is null where Xorg runs as root, or there is none", async () => {
    const root = memorySources(procFixture(xorg, comm, [String(XORG_PID)]));
    expect(await sampleDisplayServer(XORG_PID, I915, root, 0)).toBeNull();
    const wayland = memorySources(procFixture({}, { "901": "sway" }));
    expect(await findDisplayServerPid(wayland)).toBeNull();
    expect(await sampleDisplayServer(null, I915, wayland, 0)).toBeNull();
  });
});

function drm(atMs: number, residentMiB: number, busyNs: number | null = null): DrmMemorySample {
  return {
    atMs,
    residentBytes: residentMiB * MIB,
    totalBytes: residentMiB * 2 * MIB,
    sharedBytes: 4 * MIB,
    residentLowerBoundBytes: (residentMiB - 2) * MIB,
    busyNs,
    clients: 2,
    drivers: ["i915"],
  };
}

describe("the kernel's peak", () => {
  it("ignores the samples within 2 s of a large release in the tally", () => {
    // A 64 MiB cube released and remade at 1 s: the kernel shows both until Dawn's next tick.
    const samples = [drm(0, 100), drm(1500, 164), drm(1000 + RELEASE_SETTLE_MS, 100)];
    const summary = summariseDrm(samples, [{ atMs: 1000, bytes: 64 * MIB }]);
    expect(summary).toMatchObject({
      residentPeakBytes: 100 * MIB,
      totalPeakBytes: 200 * MIB,
      residentLowerBoundPeakBytes: 98 * MIB,
      settlingSamples: 1,
    });
  });

  it("keeps the samples after a small release", () => {
    const samples = [drm(0, 100), drm(1500, 164)];
    const summary = summariseDrm(samples, [{ atMs: 1000, bytes: LARGE_RELEASE_BYTES - 1 }]);
    expect(summary?.residentPeakBytes).toBe(164 * MIB);
  });

  it("states the render engine's time over the span, and the display server's line", () => {
    const summary = summariseDrm([drm(0, 30, 5_000_000), drm(1000, 31, 17_500_000)], []);
    expect(summary?.busyNs).toBe(12_500_000);
    expect(summary === null ? null : displayServerFiguresOf(summary)).toEqual({
      residentPeakBytes: 31 * MIB,
      renderBusyMs: 12.5,
    });
    expect(summariseDrm([], [])).toBeNull();
  });
});

describe("NVIDIA's memory", () => {
  it("reads the GPU process's process_info of type G", () => {
    const reading = parseNvidiaSmi(commandFixture("nvidia-smi-gpu-process-g.xml"));
    expect(nvidiaMemoryOf(reading, GPU_PID, 728 * MIB, 0)).toEqual(
      measured({
        atMs: 0,
        processUsedBytes: 612 * MIB,
        processTypes: ["G"],
        deviceUsedBytes: 1460 * MIB,
        deviceLessBaselineBytes: (1460 - 728) * MIB,
      }),
    );
  });

  it("reads the GPU process's process_info of type C+G, and the device less its baseline", async () => {
    const running = memorySources(procFixture({}), "linux", {
      "nvidia-smi -q -x": commandFixture("nvidia-smi-gpu-process-cg.xml"),
    });
    const idle = memorySources(procFixture({}), "linux", {
      "nvidia-smi -q -x": commandFixture("nvidia-smi-idle.xml"),
    });
    const baseline = await nvidiaBaseline(idle);
    expect(baseline).toEqual(measured(728 * MIB));
    const sample = await sampleNvidia(GPU_PID, baseline.value, running, 0);
    expect(sample.value).toMatchObject({
      processUsedBytes: 640 * MIB,
      processTypes: ["C+G"],
      deviceLessBaselineBytes: (1488 - 728) * MIB,
    });
  });

  it("gives the process's figure as null under WDDM, and where the process is not listed", () => {
    const wddm = nvidiaMemoryOf(
      parseNvidiaSmi(commandFixture("nvidia-smi-wddm.xml")),
      GPU_PID,
      null,
      0,
    );
    expect(wddm.value).toMatchObject({ processUsedBytes: null, processTypes: ["C+G"] });
    const other = nvidiaMemoryOf(
      parseNvidiaSmi(commandFixture("nvidia-smi-idle.xml")),
      GPU_PID,
      null,
      0,
    );
    expect(other.value).toMatchObject({ processUsedBytes: null, processTypes: [] });
  });

  it("is missing with nvidia-smi's reason on a machine without it", async () => {
    expect(await sampleNvidia(GPU_PID, null, memorySources(), 0)).toEqual(
      missing("no nvidia-smi on this machine"),
    );
  });
});

describe("the GPU-memory headline", () => {
  const kernel = summariseDrm([drm(0, 120)], []);
  const nvidia = summariseNvidia([
    {
      atMs: 0,
      processUsedBytes: 600 * MIB,
      processTypes: ["G"],
      deviceUsedBytes: null,
      deviceLessBaselineBytes: 700 * MIB,
    },
  ]);
  const wddm = summariseNvidia([
    {
      atMs: 0,
      processUsedBytes: null,
      processTypes: ["C+G"],
      deviceUsedBytes: null,
      deviceLessBaselineBytes: 700 * MIB,
    },
  ]);

  it("is the kernel's where fdinfo reads, with its bounds", () => {
    expect(gpuMemoryHeadline(kernel, nvidia, 90 * MIB)).toEqual({
      source: "drm-fdinfo",
      residentPeakBytes: 120 * MIB,
      totalPeakBytes: 240 * MIB,
      residentLowerBoundBytes: 118 * MIB,
    });
  });

  it("is nvidia-smi's on NVIDIA: the process's, else the device's less its baseline", () => {
    expect(gpuMemoryHeadline(null, nvidia, 90 * MIB)).toMatchObject({
      source: "nvidia-smi",
      residentPeakBytes: 600 * MIB,
      residentLowerBoundBytes: null,
    });
    expect(gpuMemoryHeadline(null, wddm, 90 * MIB)?.residentPeakBytes).toBe(700 * MIB);
  });

  it("falls to the tally, else nothing", () => {
    expect(gpuMemoryHeadline(null, null, 90 * MIB)?.source).toBe("adapter-tally");
    expect(gpuMemoryHeadline(null, null, null)).toBeNull();
  });
});

describe("app.getAppMetrics()", () => {
  it("sums every process's working set, KiB to bytes, by type, and finds the GPU process", () => {
    const metrics = [
      { pid: 1, type: "Browser", memory: { workingSetSize: 120_000 } },
      { pid: GPU_PID, type: "GPU", memory: { workingSetSize: 163_000 } },
      { pid: 3, type: "Tab", memory: { workingSetSize: 250_000 } },
      { pid: 4, type: "Tab", memory: { workingSetSize: 10_000 } },
    ];
    expect(appMemoryOf(metrics, 3)).toEqual({
      atMs: 3,
      totalBytes: 543_000 * KIB,
      byTypeBytes: { Browser: 120_000 * KIB, GPU: 163_000 * KIB, Tab: 260_000 * KIB },
      gpuPid: GPU_PID,
    });
    expect(appMemoryOf(null, 0)).toEqual({ atMs: 0, totalBytes: 0, byTypeBytes: {}, gpuPid: null });
  });
});
