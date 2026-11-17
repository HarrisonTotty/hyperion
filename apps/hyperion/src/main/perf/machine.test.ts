import { describe, expect, it } from "vitest";

import type { SysfsFiles } from "../gpuClocks";
import {
  describeMachine,
  type DisplayLike,
  displayRecordOf,
  gpuRecordOf,
  type MachineInputs,
  type MachineSources,
  windowManagerOf,
  windowSystemOf,
} from "./machine";

/** `app.getGPUInfo("complete")` on the UHD 620 laptop, trimmed to the device list. */
const UHD_620_INFO = {
  gpuDevice: [
    {
      active: true,
      vendorId: 0x80_86,
      deviceId: 0x3e_a0,
      driverVendor: "Mesa",
      driverVersion: "26.2.3",
      deviceString: "Mesa Intel(R) UHD Graphics 620 (WHL GT2)",
    },
  ],
  auxAttributes: { glRenderer: "Mesa Intel(R) UHD Graphics 620 (WHL GT2)" },
};

/** The same on the development desktop's RTX 3080. */
const RTX_3080_INFO = {
  gpuDevice: [{ active: true, vendorId: 0x10_de, deviceId: 0x22_06, driverVersion: "615.71.09" }],
};

const DISPLAY: DisplayLike = {
  size: { width: 1920, height: 1080 },
  scaleFactor: 1,
  displayFrequency: 59.94,
};

/** `/proc` as a map of each process's `comm`. */
function procFiles(comm: Readonly<Record<string, string>>): SysfsFiles {
  return {
    readdir: (path) =>
      path === "/proc"
        ? Promise.resolve(["self", ...Object.keys(comm)])
        : Promise.reject(new Error(`ENOENT: ${path}`)),
    readFile: (path) => {
      const pid = /^\/proc\/(\d+)\/comm$/.exec(path)?.[1];
      const name = pid === undefined ? undefined : comm[pid];
      return name === undefined
        ? Promise.reject(new Error(`ENOENT: ${path}`))
        : Promise.resolve(`${name}\n`);
    },
  };
}

function sources(overrides: Partial<MachineSources> = {}): MachineSources {
  return {
    platform: "linux",
    release: () => "6.18.0-arch1-1",
    cpus: () =>
      Array.from({ length: 8 }, () => ({ model: "Intel(R) Core(TM) i7-8665U CPU @ 1.90GHz " })),
    totalmem: () => 16_000_000_000,
    gpuInfo: () => Promise.resolve(UHD_620_INFO),
    display: () => DISPLAY,
    env: { XDG_SESSION_TYPE: "x11", DISPLAY: ":0", I3SOCK: "/run/user/1000/i3/ipc-socket.1" },
    versions: { app: "0.1.0", electron: "45.1.0", chromium: "156.0.0.0", node: "24.22.1" },
    files: procFiles({ "1": "systemd", "812": "Xorg", "901": "i3" }),
    procRoot: "/proc",
    ...overrides,
  };
}

const INPUTS: MachineInputs = {
  name: "uhd620",
  adapter: { vendor: "intel", architecture: "gen-9", description: "", fallback: false },
  capabilities: { timestampQuery: true, subgroups: true },
  kernelPaths: { "atmosphere-lut": "subgroups" },
  card: null,
  ozonePlatform: "x11",
};

describe("describeMachine", () => {
  it("describes the UHD 620 laptop under i3 and X11 with no compositor", async () => {
    expect(await describeMachine(INPUTS, sources())).toEqual({
      name: "uhd620",
      platform: "linux",
      kernel: "6.18.0-arch1-1",
      cpu: { model: "Intel(R) Core(TM) i7-8665U CPU @ 1.90GHz", logicalCores: 8 },
      memoryBytes: 16_000_000_000,
      gpu: {
        vendorId: 0x80_86,
        deviceId: 0x3e_a0,
        driverVersion: "Mesa 26.2.3",
        description: "Mesa Intel(R) UHD Graphics 620 (WHL GT2)",
      },
      adapter: INPUTS.adapter,
      capabilities: INPUTS.capabilities,
      kernelPaths: INPUTS.kernelPaths,
      versions: { app: "0.1.0", electron: "45.1.0", chromium: "156.0.0.0", node: "24.22.1" },
      display: { widthPx: 1920, heightPx: 1080, refreshHz: 59.94, devicePixelRatio: 1 },
      windowManager: "i3",
      compositor: "none",
      windowSystem: "x11",
      card: null,
    });
  });

  it("finds an X11 compositor among the processes", async () => {
    const machine = await describeMachine(
      INPUTS,
      sources({ files: procFiles({ "812": "Xorg", "901": "i3", "933": "picom" }) }),
    );
    expect(machine.compositor).toBe("picom");
  });

  it("names the NVIDIA driver's version as Chromium gives it", async () => {
    const machine = await describeMachine(
      { ...INPUTS, name: "rtx3080" },
      sources({ gpuInfo: () => Promise.resolve(RTX_3080_INFO) }),
    );
    expect(machine.gpu).toEqual({
      vendorId: 0x10_de,
      deviceId: 0x22_06,
      driverVersion: "615.71.09",
      description: null,
    });
  });

  it("works on macOS, from os, getGPUInfo and screen", async () => {
    const machine = await describeMachine(
      { ...INPUTS, name: "m5", ozonePlatform: null },
      sources({
        platform: "darwin",
        release: () => "25.1.0",
        cpus: () => Array.from({ length: 10 }, () => ({ model: "Apple M5" })),
        gpuInfo: () =>
          Promise.resolve({ gpuDevice: [{ active: true, vendorId: 0x10_6b, deviceId: 0 }] }),
        display: () => ({
          size: { width: 1470, height: 956 },
          scaleFactor: 2,
          displayFrequency: 60,
        }),
        env: {},
      }),
    );
    expect(machine).toMatchObject({
      platform: "darwin",
      cpu: { model: "Apple M5", logicalCores: 10 },
      gpu: { vendorId: 0x10_6b },
      display: { widthPx: 2940, heightPx: 1912, refreshHz: 60, devicePixelRatio: 2 },
      windowManager: "WindowServer",
      compositor: "WindowServer",
      windowSystem: "macos",
    });
  });

  it("gives the GPU as null where Chromium's information fails", async () => {
    const machine = await describeMachine(
      INPUTS,
      sources({ gpuInfo: () => Promise.reject(new Error("GPU process gone")) }),
    );
    expect(machine.gpu).toBeNull();
  });

  it("refuses a platform Electron does not run on", async () => {
    await expect(describeMachine(INPUTS, sources({ platform: "freebsd" }))).rejects.toThrow(
      /freebsd/u,
    );
  });
});

describe("the window system", () => {
  it("is XWayland under a Wayland session on X11's Ozone platform, and Wayland on its own", () => {
    const env = { XDG_SESSION_TYPE: "wayland", WAYLAND_DISPLAY: "wayland-0" };
    expect(windowSystemOf("linux", env, "x11")).toBe("xwayland");
    expect(windowSystemOf("linux", env, "wayland")).toBe("wayland");
    expect(windowSystemOf("linux", {}, null)).toBeNull();
    expect(windowSystemOf("win32", {}, null)).toBe("windows");
  });

  it("names the window manager from the session's variables", () => {
    expect(windowManagerOf("linux", { XDG_CURRENT_DESKTOP: "ubuntu:GNOME" })).toBe("gnome");
    expect(windowManagerOf("linux", { SWAYSOCK: "/run/sway.sock" })).toBe("sway");
    expect(windowManagerOf("linux", {})).toBeNull();
    expect(windowManagerOf("win32", {})).toBe("dwm");
  });
});

describe("the parts", () => {
  it("are null where Electron gives none", () => {
    expect(gpuRecordOf({})).toBeNull();
    expect(displayRecordOf(null)).toBeNull();
    expect(displayRecordOf({ ...DISPLAY, displayFrequency: 0 })?.refreshHz).toBeNull();
  });
});
