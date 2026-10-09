import { describe, expect, it } from "vitest";

import { QUIET_RULE_UNCHECKED } from "../main/machineLoad";
import { measured } from "../main/measured";
import {
  CHILD_FRAME_NAME,
  CHILD_SIZE,
  type ChildWindowRun,
  childDisplayOf,
  childOpenHandler,
  childOpenResponse,
  childWindowMarkdown,
  type DisplayInfo,
  recordName,
  refusalLine,
  withGpuExits,
} from "./childWindow";

function display(id: number, x: number, hz: number): DisplayInfo {
  return {
    id,
    label: id === 1 ? "VX2450 SERIES" : "",
    bounds: { x, y: 0, width: 1920, height: 1080 },
    workArea: { x, y: 0, width: 1920, height: 1040 },
    displayFrequency: hz,
    scaleFactor: 1,
  };
}

const MAIN = display(1, 0, 60);
const SECOND = display(2, 1920, 75);

describe("the child window's display", () => {
  it("is the first display that is not the opener's", () => {
    expect(childDisplayOf([MAIN, SECOND], MAIN.id)).toBe(SECOND);
    expect(childDisplayOf([SECOND, MAIN], MAIN.id)).toBe(SECOND);
  });

  it("is none with one display, which the run refuses naming what it sees", () => {
    expect(childDisplayOf([MAIN], MAIN.id)).toBeNull();
    expect(refusalLine([MAIN])).toMatch(
      /^SETUP R07\.T21 needs a second display, and Electron sees 1: display 1 VX2450 SERIES 1920 × 1080 at \(0, 0\), 60\.00 Hz/,
    );
  });
});

describe("the opener's window-open handler", () => {
  it("opens only the child, centred on its display, sandboxed", () => {
    const response = childOpenResponse(
      { url: "about:blank", frameName: CHILD_FRAME_NAME },
      SECOND,
      false,
    );
    expect(response).toMatchObject({
      action: "allow",
      overrideBrowserWindowOptions: {
        x: 1920 + (1920 - CHILD_SIZE.width) / 2,
        y: (1040 - CHILD_SIZE.height) / 2,
        show: true,
        webPreferences: {
          sandbox: true,
          contextIsolation: true,
          nodeIntegration: false,
          offscreen: false,
        },
      },
    });
  });

  it("keeps a hidden child offscreen", () => {
    const response = childOpenResponse(
      { url: "about:blank", frameName: CHILD_FRAME_NAME },
      MAIN,
      true,
    );
    expect(response).toMatchObject({
      overrideBrowserWindowOptions: { show: false, webPreferences: { offscreen: true } },
    });
  });

  it.each([
    [{ url: "https://example.com/", frameName: CHILD_FRAME_NAME }],
    [{ url: "about:blank", frameName: "another" }],
  ])("denies %j", (request) => {
    expect(childOpenResponse(request, SECOND, false)).toEqual({ action: "deny" });
  });
});

describe("the opener's handler", () => {
  const request = { url: "about:blank", frameName: CHILD_FRAME_NAME };

  it("allows the child once", () => {
    const handle = childOpenHandler(SECOND, false);
    expect(handle(request).action).toBe("allow");
  });

  it("denies a second child after the first", () => {
    const handle = childOpenHandler(SECOND, false);
    handle(request);
    expect(handle(request)).toEqual({ action: "deny" });
  });
});

describe("the GPU process's exits", () => {
  const codes = { pass: 0, failed: 1 };

  it("fail a run that passed otherwise", () => {
    expect(withGpuExits(0, 1, codes)).toEqual({
      line: "FAIL T21 no GPU-process exit: 1",
      exitCode: 1,
    });
  });

  it("leave a run without one as it was", () => {
    expect(withGpuExits(2, 0, codes)).toEqual({
      line: "PASS T21 no GPU-process exit: 0",
      exitCode: 2,
    });
  });
});

describe("the child window's record", () => {
  const run: ChildWindowRun = {
    startedAt: new Date("2026-10-05T12:00:00Z"),
    hidden: false,
    displays: [MAIN, SECOND],
    mainDisplayId: MAIN.id,
    childDisplayId: SECOND.id,
    versions: { electron: "44.4.3", chromium: "152" },
    switches: ["--use-angle=vulkan"],
    platform: "linux",
    machine: {
      name: "effect",
      cpu: "AMD Ryzen 7 3700X",
      logicalCores: 16,
      memoryBytes: 32e9,
      governor: measured("performance"),
      loadAverage: [0.2, 0.3, 0.4],
      gpu: measured({ vendorId: 4318, deviceId: 8710, driverVersion: "615", description: null }),
    },
    exitCode: 0,
  };

  it("is named by date and machine, a hidden run apart", () => {
    expect(recordName(run, "effect")).toBe("2026-10-05-effect-child-window.md");
    expect(recordName({ ...run, hidden: true }, "effect")).toBe(
      "2026-10-05-effect-child-window-hidden.md",
    );
  });

  it("names each display's role and keeps the harness's lines", () => {
    const text = childWindowMarkdown(run, "effect", ["PASS T21 the child window opened: yes"]);
    expect(text).toContain("(the opener's)");
    expect(text).toContain("(the child's)");
    expect(text).toContain("PASS T21 the child window opened: yes");
    expect(text).toContain("governor performance");
  });

  it("marks its load average provisional at a load of 1 or more, and not under it", () => {
    const loaded = { ...run, machine: { ...run.machine, loadAverage: [1.5, 1.2, 0.9] as const } };
    expect([
      childWindowMarkdown(run, "effect", []).includes("0.20, 0.30, 0.40\n"),
      childWindowMarkdown(loaded, "effect", []).includes(
        "1.50, 1.20, 0.90 (provisional: under 1 asked)",
      ),
    ]).toEqual([true, true]);
  });

  it("marks a Windows run provisional, its quiet-machine rule unchecked (R05.T20)", () => {
    const windows = {
      ...run,
      platform: "win32" as const,
      machine: { ...run.machine, loadAverage: [0, 0, 0] as const },
    };
    expect(childWindowMarkdown(windows, "effect", [])).toContain(
      `- **Load average at the start:** none (${QUIET_RULE_UNCHECKED}): provisional`,
    );
  });
});
