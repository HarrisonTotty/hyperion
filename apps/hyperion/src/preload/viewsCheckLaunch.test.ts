import { describe, expect, it } from "vitest";

import {
  readViewsCheckLaunch,
  type ViewsCheckLaunch,
  viewsCheckLaunchFromArgv,
  viewsCheckSwitch,
} from "./viewsCheckLaunch";

const LAUNCH: ViewsCheckLaunch = {
  setting: "low",
  smoke: false,
  out: "/data/out",
  run: "several-views",
  fixture: "terrain-test",
};

describe("the views check's launch switch", () => {
  it("carries the options to the renderer and back", () => {
    expect(viewsCheckLaunchFromArgv(["--other", viewsCheckSwitch(LAUNCH)])).toEqual(LAUNCH);
  });

  it.each([["terrain"], ["handover"], ["atmosphere"]] as const)(
    "carries a %s run's launch (R10.T16.d, R10.T10.g, R08.T10.b)",
    (run) => {
      const launch = { ...LAUNCH, run };
      expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toEqual(launch);
    },
  );

  it.each([["terrain"], ["handover"]] as const)(
    "carries a %s run's weathered fixture, TEST MOON weathered (R10.T1.c)",
    (run) => {
      const launch = { ...LAUNCH, run, fixture: "terrain-test-weathered" } as const;
      expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toEqual(launch);
    },
  );

  it("refuses the weathered fixture on T20's run, which serves no field (R10.T1.c)", () => {
    expect(readViewsCheckLaunch({ ...LAUNCH, fixture: "terrain-test-weathered" })).toBeNull();
    expect(() =>
      viewsCheckLaunchFromArgv([
        viewsCheckSwitch({ ...LAUNCH, fixture: "terrain-test-weathered" }),
      ]),
    ).toThrow(/does not hold/);
  });

  it("refuses the weathered fixture on the atmosphere run, which serves no field (R08.T10.b)", () => {
    const launch = { ...LAUNCH, run: "atmosphere", fixture: "terrain-test-weathered" } as const;
    expect(readViewsCheckLaunch(launch)).toBeNull();
    expect(() => viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toThrow(/does not hold/);
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
    [{ ...LAUNCH, run: "sunset" }],
    [{ ...LAUNCH, run: true }],
    [{ ...LAUNCH, run: "terrain", fixture: "weathered" }],
    [{ ...LAUNCH, run: "terrain", fixture: null }],
    [{ setting: "low", smoke: false, out: "/data/out", run: "terrain" }],
    [{ setting: "high", smoke: false }],
    [{ setting: "high", smoke: false, out: null }],
    [{ setting: "high", smoke: false, out: null, terrain: true }],
    [null],
  ])("reads %j as no options", (value) => {
    expect(readViewsCheckLaunch(value)).toBeNull();
  });
});
