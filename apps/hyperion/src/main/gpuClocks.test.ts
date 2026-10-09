import { readFileSync } from "node:fs";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { type NvidiaReading, parseNvidiaSmi } from "./fdinfo";
import {
  AMDGPU_STATE_REASON,
  DARWIN_CLOCKS_REASON,
  GpuClockReader,
  type GpuClockReaderOptions,
  type GpuIdentity,
  I915_MEMORY_REASON,
  I915_STATE_REASON,
  nvidiaClocks,
  parseDpmLevels,
  type SysfsFiles,
  WIN32_CLOCKS_REASON,
} from "./gpuClocks";
import { type Measured, measured, missing } from "./measured";

/**
 * The fixtures: `nvidia-smi.xml` recorded unprivileged on the development machine's RTX 3080
 * (2026-10-06), and two sysfs trees written in the kernel's documented layout, since neither GPU
 * is in that machine: `sysfs/i915`, a UHD 620 (`0x8086:0x5917`) as card0 with i915's frequency
 * files, and `sysfs/amdgpu`, an RX 6800 XT (`0x1002:0x73bf`) as card1 beside an Intel GPU as card0,
 * with amdgpu's `pp_dpm_sclk` and `pp_dpm_mclk`.
 */
const FIXTURES = join(__dirname, "fixtures");

const I915_ROOT = join(FIXTURES, "sysfs/i915");
const AMDGPU_ROOT = join(FIXTURES, "sysfs/amdgpu");

const RTX_3080: GpuIdentity = { vendorId: 0x10_de, deviceId: 0x22_06 };
const UHD_620: GpuIdentity = { vendorId: 0x80_86, deviceId: 0x59_17 };
const RX_6800_XT: GpuIdentity = { vendorId: 0x10_02, deviceId: 0x73_bf };

const NO_NVIDIA: NvidiaReading = { kind: "unavailable", reason: "no nvidia-smi on this machine" };

function fixture(name: string): string {
  return readFileSync(join(FIXTURES, name), "utf8");
}

function nvidiaFixture(): NvidiaReading {
  return parseNvidiaSmi(fixture("nvidia-smi.xml"));
}

/** The disk's files, `hidden` ones absent as the kernel leaves an absent file, every read listed. */
function diskFiles(hidden: ReadonlyArray<string> = []): SysfsFiles & { readonly read: string[] } {
  const read: string[] = [];
  return {
    read,
    readdir: (path) => readdir(path),
    readFile: (path) => {
      read.push(path);
      return hidden.some((name) => path.endsWith(name))
        ? Promise.reject(
            Object.assign(new Error(`ENOENT: no such file or directory, open '${path}'`), {
              code: "ENOENT",
            }),
          )
        : readFile(path, "utf8");
    },
  };
}

function readerOf(
  platform: NodeJS.Platform,
  gpu: Measured<GpuIdentity>,
  options: Partial<GpuClockReaderOptions> = {},
): GpuClockReader {
  return new GpuClockReader({ platform, gpu: () => Promise.resolve(gpu), ...options });
}

describe("the GPU's clocks from nvidia-smi", () => {
  it("reads the recorded report's graphics, memory and maximum clocks and its performance state", async () => {
    const sample = await readerOf("linux", measured(RTX_3080)).read(nvidiaFixture());
    expect(sample).toEqual({
      kind: "clocks",
      source: "nvidia-smi",
      maxGraphicsMHz: measured(2115),
      graphicsMHz: measured(210),
      memoryMHz: measured(405),
      performanceState: measured(8),
    });
  });

  it("reads them on Windows too", async () => {
    const sample = await readerOf("win32", measured(RTX_3080)).read(nvidiaFixture());
    expect(sample).toMatchObject({ kind: "clocks", source: "nvidia-smi" });
  });

  it("gives each reading null with nvidia-smi's reason when it has none", () => {
    const none = missing("no nvidia-smi on this machine");
    expect(nvidiaClocks(NO_NVIDIA)).toEqual({
      kind: "clocks",
      source: "nvidia-smi",
      maxGraphicsMHz: none,
      graphicsMHz: none,
      memoryMHz: none,
      performanceState: none,
    });
  });

  it("gives a clock nvidia-smi writes as N/A null with the element", () => {
    const xml = fixture("nvidia-smi.xml").replace(
      "<mem_clock>405 MHz</mem_clock>",
      "<mem_clock>N/A</mem_clock>",
    );
    expect(nvidiaClocks(parseNvidiaSmi(xml)).memoryMHz).toEqual(
      missing("nvidia-smi gives no clocks/mem_clock (N/A)"),
    );
  });
});

describe("the GPU's clocks from i915's sysfs", () => {
  const card = `${I915_ROOT}/class/drm/card0`;

  it("reads gt_act_freq_mhz against gt_RP0_freq_mhz, with no memory clock or state", async () => {
    const files = diskFiles();
    const sample = await readerOf("linux", measured(UHD_620), {
      files,
      sysRoot: I915_ROOT,
    }).read(NO_NVIDIA);
    expect(sample).toEqual({
      kind: "clocks",
      source: "i915-sysfs",
      maxGraphicsMHz: measured(1150),
      graphicsMHz: measured(600),
      memoryMHz: missing(I915_MEMORY_REASON),
      performanceState: missing(I915_STATE_REASON),
    });
    expect(files.read).not.toContain(`${card}/gt_boost_freq_mhz`);
  });

  it("falls back to gt_boost_freq_mhz where gt_RP0_freq_mhz is absent", async () => {
    const files = diskFiles(["gt_RP0_freq_mhz"]);
    const sample = await readerOf("linux", measured(UHD_620), {
      files,
      sysRoot: I915_ROOT,
    }).read(NO_NVIDIA);
    expect(sample).toMatchObject({ maxGraphicsMHz: measured(1150) });
    expect(files.read).toContain(`${card}/gt_boost_freq_mhz`);
  });

  it("gives a reading whose file is absent null with the path", async () => {
    const files = diskFiles(["gt_act_freq_mhz", "gt_RP0_freq_mhz", "gt_boost_freq_mhz"]);
    const sample = await readerOf("linux", measured(UHD_620), {
      files,
      sysRoot: I915_ROOT,
    }).read(NO_NVIDIA);
    expect(sample).toMatchObject({
      graphicsMHz: missing(`${card}/gt_act_freq_mhz could not be read (ENOENT)`),
      maxGraphicsMHz: missing(
        `${card}/gt_RP0_freq_mhz could not be read (ENOENT); ${card}/gt_boost_freq_mhz could not be read (ENOENT)`,
      ),
    });
  });

  it("reads the card each sample, choosing it once", async () => {
    let asked = 0;
    const files = diskFiles();
    const reader = new GpuClockReader({
      platform: "linux",
      gpu: () => {
        asked += 1;
        return Promise.resolve(measured(UHD_620));
      },
      files,
      sysRoot: I915_ROOT,
    });
    await reader.read(NO_NVIDIA);
    await reader.read(NO_NVIDIA);
    expect(asked).toBe(1);
    expect(files.read.filter((path) => path.endsWith("gt_act_freq_mhz"))).toHaveLength(2);
  });
});

describe("the GPU's clocks from amdgpu's sysfs", () => {
  it("reads the current levels of the run's card against the highest", async () => {
    const sample = await readerOf("linux", measured(RX_6800_XT), {
      files: diskFiles(),
      sysRoot: AMDGPU_ROOT,
    }).read(NO_NVIDIA);
    expect(sample).toEqual({
      kind: "clocks",
      source: "amdgpu-sysfs",
      maxGraphicsMHz: measured(2250),
      graphicsMHz: measured(2045),
      memoryMHz: measured(1000),
      performanceState: missing(AMDGPU_STATE_REASON),
    });
  });

  it("chooses the card by the GPU's PCI vendor and device", async () => {
    // The tree's card0 is an Intel GPU (0x8086:0x3e92), whose i915 files it reads for that GPU.
    const sample = await readerOf("linux", measured({ vendorId: 0x80_86, deviceId: 0x3e_92 }), {
      files: diskFiles(),
      sysRoot: AMDGPU_ROOT,
    }).read(NO_NVIDIA);
    expect(sample).toMatchObject({
      source: "i915-sysfs",
      graphicsMHz: measured(350),
      maxGraphicsMHz: measured(1200),
    });
  });

  it("gives a reading whose file is absent null with the path", async () => {
    const card = `${AMDGPU_ROOT}/class/drm/card1`;
    const sample = await readerOf("linux", measured(RX_6800_XT), {
      files: diskFiles(["pp_dpm_mclk"]),
      sysRoot: AMDGPU_ROOT,
    }).read(NO_NVIDIA);
    expect(sample).toMatchObject({
      graphicsMHz: measured(2045),
      memoryMHz: missing(`${card}/device/pp_dpm_mclk could not be read (ENOENT)`),
    });
  });

  it("parses the kernel's levels, the current one marked, an APU's sleep level among them", () => {
    expect(parseDpmLevels("0: 500Mhz \n1: 2045Mhz *\n2: 2250Mhz \n")).toEqual({
      levelsMHz: [500, 2045, 2250],
      currentMHz: 2045,
    });
    expect(parseDpmLevels("S: 19Mhz *\n0: 400Mhz \n1: 1900Mhz \n")).toEqual({
      levelsMHz: [19, 400, 1900],
      currentMHz: 19,
    });
    expect(parseDpmLevels("0: 300Mhz\n")).toEqual({ levelsMHz: [300], currentMHz: null });
  });
});

describe("the GPU's clocks elsewhere", () => {
  it("are null with the reason on macOS", async () => {
    await expect(readerOf("darwin", measured(UHD_620)).read(NO_NVIDIA)).resolves.toEqual({
      kind: "unavailable",
      reason: DARWIN_CLOCKS_REASON,
    });
    expect(DARWIN_CLOCKS_REASON).toBe(
      "no unprivileged GPU clock reading on darwin; powermetrics needs root",
    );
  });

  it("are null with the reason on Windows for a GPU other than NVIDIA's", async () => {
    await expect(readerOf("win32", measured(UHD_620)).read(NO_NVIDIA)).resolves.toEqual({
      kind: "unavailable",
      reason: WIN32_CLOCKS_REASON,
    });
  });

  it("are null with the reason for a vendor with no reader", async () => {
    await expect(
      readerOf("linux", measured({ vendorId: 0x1a_03, deviceId: 0x20_00 }), {
        files: diskFiles(),
        sysRoot: I915_ROOT,
      }).read(NO_NVIDIA),
    ).resolves.toEqual({
      kind: "unavailable",
      reason: "no GPU clock reader for the vendor 0x1a03 on linux",
    });
  });

  it("are null with the reason for a GPU that no DRM card is", async () => {
    await expect(
      readerOf("linux", measured(RX_6800_XT), { files: diskFiles(), sysRoot: I915_ROOT }).read(
        NO_NVIDIA,
      ),
    ).resolves.toEqual({
      kind: "unavailable",
      reason: `no DRM card under ${I915_ROOT}/class/drm is the run's GPU (0x1002:0x73bf)`,
    });
  });

  it("are null with the reason when Chromium reports no GPU", async () => {
    await expect(
      readerOf("linux", missing("Chromium reports no GPU device")).read(nvidiaFixture()),
    ).resolves.toEqual({
      kind: "unavailable",
      reason: "the run's GPU is not known: Chromium reports no GPU device",
    });
  });

  it("are null with the reason when Chromium's GPU information fails", async () => {
    const failing = new GpuClockReader({
      platform: "linux",
      gpu: () => Promise.reject(new Error("the GPU process is gone")),
    });
    await expect(failing.read(NO_NVIDIA)).resolves.toEqual({
      kind: "unavailable",
      reason:
        "the run's GPU is not known: Chromium's GPU information failed: the GPU process is gone",
    });
  });
});
