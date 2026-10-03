import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import {
  type FdinfoFiles,
  parseFdinfo,
  parseNvidiaSmi,
  readDrmMemory,
  readNvidiaSmi,
  sumDrmClients,
} from "./fdinfo";

/**
 * The fdinfo fixtures: `nvidia-renderD128.txt` recorded on the development machine (RTX 3080,
 * driver 615.71.09, 2026-10-02); the i915 and amdgpu files in the kernel's documented layout
 * (Documentation/gpu/drm-usage-stats.rst, i915's and amdgpu's `show_fdinfo`), amdgpu's client 34 in
 * the older `drm-memory-*` form and client 35 in the newer `drm-total-*` one, since neither device
 * is in the development machine.
 */
const FIXTURES = join(__dirname, "fixtures");

function fixture(name: string): string {
  return readFileSync(join(FIXTURES, name), "utf8");
}

const KIB = 1024;
const MIB = 1024 ** 2;

/** A `/proc/<pid>/fdinfo` holding the named fixtures, under file-descriptor numbers. */
function fakeProc(byFd: Readonly<Record<string, string>>): FdinfoFiles {
  return {
    readdir: (path) =>
      path === "/proc/4242/fdinfo"
        ? Promise.resolve(Object.keys(byFd))
        : Promise.reject(new Error(`ENOENT: ${path}`)),
    readFile: (path) => {
      const name = byFd[path.slice("/proc/4242/fdinfo/".length)];
      return name === undefined
        ? Promise.reject(new Error(`ENOENT: ${path}`))
        : Promise.resolve(fixture(`fdinfo/${name}`));
    },
  };
}

describe("an fdinfo file", () => {
  it("reads i915's client, device and memory by region", () => {
    expect(parseFdinfo(fixture("fdinfo/i915-client7.txt"))).toEqual({
      driver: "i915",
      pdev: "0000:00:02.0",
      clientId: "7",
      totalBytes: { system0: 231_424 * KIB, "stolen-system0": 0 },
      residentBytes: { system0: 198_656 * KIB, "stolen-system0": 0 },
    });
  });

  it("reads amdgpu's older drm-memory keys as the resident amount", () => {
    expect(parseFdinfo(fixture("fdinfo/amdgpu-client34.txt"))).toEqual({
      driver: "amdgpu",
      pdev: "0000:0b:00.0",
      clientId: "34",
      totalBytes: {},
      residentBytes: { vram: 30_348 * KIB, gtt: 2048 * KIB, cpu: 0 },
    });
  });

  it("prefers drm-resident to drm-memory where a driver writes both", () => {
    expect(parseFdinfo(fixture("fdinfo/amdgpu-client35.txt"))?.residentBytes).toEqual({
      vram: 524_288 * KIB,
      gtt: 8 * MIB,
      cpu: 0,
    });
  });

  it("is no client where the driver writes no DRM keys, as NVIDIA's", () => {
    expect(parseFdinfo(fixture("fdinfo/nvidia-renderD128.txt"))).toBeNull();
  });
});

describe("the GPU process's DRM memory", () => {
  it("sums i915's distinct clients, counting a duplicated descriptor once", async () => {
    const reading = await readDrmMemory(
      4242,
      "linux",
      fakeProc({ "21": "i915-client7.txt", "22": "i915-client7.txt", "23": "i915-client8.txt" }),
    );
    expect(reading).toEqual({
      kind: "drm",
      totalBytes: 231_424 * KIB + 12 * MIB,
      residentBytes: 198_656 * KIB + 12 * MIB,
      clients: 2,
      drivers: ["i915"],
    });
  });

  it("sums amdgpu's clients of both forms", async () => {
    const reading = await readDrmMemory(
      4242,
      "linux",
      fakeProc({ "30": "amdgpu-client34.txt", "31": "amdgpu-client35.txt" }),
    );
    expect(reading).toEqual({
      kind: "drm",
      totalBytes: null,
      residentBytes: 30_348 * KIB + 2048 * KIB + 524_288 * KIB + 8 * MIB,
      clients: 2,
      drivers: ["amdgpu"],
    });
  });

  it("is null with its reason on NVIDIA", async () => {
    const reading = await readDrmMemory(4242, "linux", fakeProc({ "34": "nvidia-renderD128.txt" }));
    expect(reading).toEqual({
      kind: "unavailable",
      reason: "no DRM client in the process's fdinfo (NVIDIA's driver writes none)",
    });
  });

  it("is null with its reason off Linux", async () => {
    await expect(readDrmMemory(4242, "win32", fakeProc({}))).resolves.toEqual({
      kind: "unavailable",
      reason: "DRM fdinfo is Linux's; this is win32",
    });
  });

  it("is null with its reason for a process whose fdinfo cannot be read", async () => {
    await expect(readDrmMemory(1, "linux", fakeProc({}))).resolves.toEqual({
      kind: "unavailable",
      reason: "/proc/1/fdinfo could not be read: ENOENT: /proc/1/fdinfo",
    });
  });

  it("gives no total unless every client gives one", async () => {
    const reading = await readDrmMemory(
      4242,
      "linux",
      fakeProc({ "30": "amdgpu-client34.txt", "31": "amdgpu-client35.txt" }),
    );
    expect(reading).toMatchObject({ kind: "drm", totalBytes: null });
  });

  it("is null when the clients carry no memory keys", () => {
    const bare = { driver: "x", pdev: null, clientId: "1", totalBytes: {}, residentBytes: {} };
    expect(sumDrmClients([bare], "the driver reports no drm-* memory keys")).toEqual({
      kind: "unavailable",
      reason: "the driver reports no drm-* memory keys",
    });
  });
});

describe("nvidia-smi", () => {
  it("reads the device's memory and its processes from a recorded report", () => {
    expect(parseNvidiaSmi(fixture("nvidia-smi.xml"))).toEqual({
      kind: "nvidia",
      driverVersion: "615.71.09",
      gpus: [
        {
          busId: "00000000:08:00.0",
          productName: "NVIDIA GeForce RTX 3080",
          totalBytes: 10_240 * MIB,
          reservedBytes: 320 * MIB,
          usedBytes: 168 * MIB,
          processes: [
            { pid: 1303, type: "G", name: "/usr/lib/Xorg", usedBytes: 110 * MIB },
            { pid: 1439, type: "G", name: "alacritty", usedBytes: 10 * MIB },
            { pid: 421_400, type: "G", name: "alacritty", usedBytes: 10 * MIB },
          ],
        },
      ],
    });
  });

  it("reads N/A as null", () => {
    const xml = fixture("nvidia-smi.xml").replace("<used>168 MiB</used>", "<used>N/A</used>");
    const reading = parseNvidiaSmi(xml);
    expect(reading.kind === "nvidia" ? reading.gpus[0]?.usedBytes : "unread").toBeNull();
  });

  it("is null with its reason when it lists no GPU", () => {
    expect(
      parseNvidiaSmi("<nvidia_smi_log><driver_version>1</driver_version></nvidia_smi_log>"),
    ).toEqual({
      kind: "unavailable",
      reason: "nvidia-smi lists no GPU",
    });
  });

  it("is null with its reason when the run fails", async () => {
    await expect(readNvidiaSmi(() => Promise.reject(new Error("timed out")))).resolves.toEqual({
      kind: "unavailable",
      reason: "nvidia-smi failed: timed out",
    });
  });

  it("is null with its reason where there is no nvidia-smi", async () => {
    const missing = Object.assign(new Error("spawn nvidia-smi ENOENT"), { code: "ENOENT" });
    await expect(readNvidiaSmi(() => Promise.reject(missing))).resolves.toEqual({
      kind: "unavailable",
      reason: "no nvidia-smi on this machine",
    });
  });

  it("runs nvidia-smi -q -x", async () => {
    const calls: Array<readonly [string, ReadonlyArray<string>]> = [];
    await readNvidiaSmi((file, args) => {
      calls.push([file, args]);
      return Promise.resolve(fixture("nvidia-smi.xml"));
    });
    expect(calls).toEqual([["nvidia-smi", ["-q", "-x"]]]);
  });
});
