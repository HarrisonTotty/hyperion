import { describe, expect, it } from "vitest";

import type { GpuIdentity } from "../gpuClocks";
import { type Measured, measured, missing } from "../measured";
import {
  commandFixture,
  DESKTOP_SYS,
  fixtureCommands,
  fixtureFiles,
  X1_YOGA_SYS,
} from "./fixtures/sysfs";
import {
  type Every,
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
  type SamplerSources,
  sampleThermal,
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
    expect(sample.packageDegC).toEqual(measured(52));
    expect(sample.packageSource).toBe("x86_pkg_temp");
    // iwlwifi_1 has no reading, and is left out.
    expect(sample.zonesDegC).toEqual({
      acpitz: 48,
      "INT3400 Thermal": 20,
      pch_cannonlake: 46,
      B0D4: 52,
      x86_pkg_temp: 52,
    });
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
    expect(sample.packageThrottleMs["cpu0"]).toBe(17_010_000);
    expect(sample.coreThrottleMs["cpu4"]).toBe(6_410_000);
    expect(sample.cpuMHz[0]).toBe(3900);
    expect(sample.gpu).toEqual({
      source: "i915-sysfs",
      mhz: measured(1150),
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

  it("starts one child and gives its latest line to the GPU sampler", async () => {
    const fake = fakeSpawn();
    let nowMs = 0;
    const stream = new NvidiaClockStream({
      spawn: fake.spawn,
      intervalMs: 100,
      gpuIndex: 0,
      now: () => nowMs,
    });
    stream.start();
    stream.start();
    expect(fake.started).toEqual([{ file: "nvidia-smi", args: nvidiaClockArgs(100) }]);
    expect(stream.latest()).toEqual(missing("no line from nvidia-smi yet"));
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
      maxMHz: measured(2100),
      slowdown: measured(true),
    });
    nowMs = 1001;
    expect(stream.latest().reason).toBe("nvidia-smi's latest line is 1001 ms old");
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

describe("the 10 Hz sampler", () => {
  it("samples every interval, skips a tick while one reads, and summarises a span", async () => {
    const ticks: (() => void)[] = [];
    let cancelled = 0;
    const every: Every = (callback, ms) => {
      expect(ms).toBe(100);
      ticks.push(callback);
      return () => {
        cancelled += 1;
      };
    };
    let nowMs = 0;
    const laptop = sources(X1_YOGA_SYS);
    const sampler = new PerfSampler({
      sources: laptop,
      gpu: new GpuClockSampler({ sources: laptop, gpu: UHD_620, nvidia: null }),
      intervalMs: 100,
      now: () => nowMs,
      every,
    });
    sampler.start();
    sampler.start();
    expect(ticks).toHaveLength(1);
    const tick = ticks[0] ?? ((): void => undefined);
    tick();
    // The first tick is still reading: this one is skipped.
    tick();
    await expect.poll(() => sampler.samples.length).toBe(1);
    nowMs = 100;
    tick();
    await expect.poll(() => sampler.samples.length).toBe(2);
    sampler.stop();
    expect(cancelled).toBe(1);
    expect(sampler.failure).toBeNull();
    const figures = sampler.summarise(0, 100);
    expect(figures.packageThrottleMs).toBe(0);
    expect(figures.throttleFraction).toBe(0);
    expect(figures.rp0Share).toBe(1);
    expect(figures.maxPackageDegC).toBe(52);
    expect(sampler.summarise(200, 300).gpuMedianMHz).toBeNull();
    sampler.discardBefore(100);
    expect(sampler.samples.map((s) => s.thermal.atMs)).toEqual([100]);
  });
});
