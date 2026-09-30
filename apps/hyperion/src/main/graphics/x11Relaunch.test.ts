import { describe, expect, it } from "vitest";

import { OZONE_X11_FLAG, x11RelaunchArgs } from "./x11Relaunch";

const ARGS: readonly string[] = ["/opt/hyperion/resources/app", "--port", "9000"];

describe("the X11 relaunch", () => {
  it("relaunches a Wayland session with the X11 flag appended", () => {
    expect(x11RelaunchArgs(ARGS, { XDG_SESSION_TYPE: "wayland" }, "linux")).toEqual([
      ...ARGS,
      OZONE_X11_FLAG,
    ]);
  });

  it("puts no executable path at the head of the arguments", () => {
    const relaunch = x11RelaunchArgs(["--port", "9000"], { XDG_SESSION_TYPE: "wayland" }, "linux");
    expect(relaunch?.[0]).toBe("--port");
  });

  it("does nothing when the flag is already on the command line", () => {
    expect(
      x11RelaunchArgs([...ARGS, OZONE_X11_FLAG], { XDG_SESSION_TYPE: "wayland" }, "linux"),
    ).toBeUndefined();
  });

  it("does nothing under X11 or with no session type", () => {
    expect(x11RelaunchArgs(ARGS, { XDG_SESSION_TYPE: "x11" }, "linux")).toBeUndefined();
    expect(x11RelaunchArgs(ARGS, {}, "linux")).toBeUndefined();
  });

  it("does nothing on other platforms", () => {
    for (const platform of ["win32", "darwin", "freebsd"] as const) {
      expect(x11RelaunchArgs(ARGS, { XDG_SESSION_TYPE: "wayland" }, platform)).toBeUndefined();
    }
  });

  it("never duplicates the flag", () => {
    const once = x11RelaunchArgs(ARGS, { XDG_SESSION_TYPE: "wayland" }, "linux") ?? [];
    expect(x11RelaunchArgs(once, { XDG_SESSION_TYPE: "wayland" }, "linux")).toBeUndefined();
    expect(once.filter((argument) => argument === OZONE_X11_FLAG)).toHaveLength(1);
  });
});
