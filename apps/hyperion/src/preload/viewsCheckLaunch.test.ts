import { describe, expect, it } from "vitest";

import {
  readViewsCheckLaunch,
  type ViewsCheckLaunch,
  viewsCheckLaunchFromArgv,
  viewsCheckSwitch,
} from "./viewsCheckLaunch";

const LAUNCH: ViewsCheckLaunch = { setting: "low", smoke: false, out: "/data/out" };

describe("the views check's launch switch", () => {
  it("carries the options to the renderer and back", () => {
    expect(viewsCheckLaunchFromArgv(["--other", viewsCheckSwitch(LAUNCH)])).toEqual(LAUNCH);
  });

  it("is absent on any other launch", () => {
    expect(viewsCheckLaunchFromArgv(["--hyperion-server-url=ws://127.0.0.1:7878/ws"])).toBeNull();
  });

  it("refuses a switch that holds no options", () => {
    expect(() => viewsCheckLaunchFromArgv(["--hyperion-views-check=%7B"])).toThrow(/not JSON/);
    expect(() =>
      viewsCheckLaunchFromArgv([`--hyperion-views-check=${encodeURIComponent("{}")}`]),
    ).toThrow(/does not hold/);
  });

  it.each([
    [{ ...LAUNCH, setting: "medium" }],
    [{ ...LAUNCH, smoke: "yes" }],
    [{ ...LAUNCH, out: "" }],
    [{ setting: "high", smoke: false }],
    [null],
  ])("reads %j as no options", (value) => {
    expect(readViewsCheckLaunch(value)).toBeNull();
  });
});
