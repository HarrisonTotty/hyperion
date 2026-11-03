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

  it("carries a terrain run's whole-body field and its file's path (R10.T14.c)", () => {
    const launch: ViewsCheckLaunch = {
      ...LAUNCH,
      run: "terrain",
      fixture: "terrain-test-whole",
      wholeField: "/repo/target/views-check/terrain/whole.golden",
    };
    expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toEqual(launch);
  });

  it.each([
    [{ run: "terrain", fixture: "terrain-test-whole" }],
    [{ run: "terrain", fixture: "terrain-test-whole", wholeField: "" }],
    [{ run: "handover", fixture: "terrain-test-whole", wholeField: "/whole.golden" }],
    [{ run: "terrain", fixture: "terrain-test", wholeField: "/whole.golden" }],
    [
      {
        run: "terrain",
        fixture: "terrain-test-whole",
        wholeField: "/whole.golden",
        served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
      },
    ],
  ])("refuses the whole-body field but on a terrain run that names its file: %j", (change) => {
    expect(readViewsCheckLaunch({ ...LAUNCH, ...change })).toBeNull();
  });

  it("carries a hand-over check's served world, its universe and body (R10.T15.b)", () => {
    const launch: ViewsCheckLaunch = {
      ...LAUNCH,
      run: "handover",
      served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
    };
    expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toEqual(launch);
  });

  it.each([
    [{ run: "terrain" }],
    [{ fixture: "terrain-test-weathered" }],
    [{ served: { universe: "3504954ff4feabf4" } }],
    [{ served: { universe: "not hex", body: "61ffd967fe000023.0100" } }],
  ])("refuses a served world on a launch with %j", (change) => {
    const served = {
      ...LAUNCH,
      run: "handover",
      served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
    };
    expect(readViewsCheckLaunch({ ...served, ...change })).toBeNull();
  });

  it("carries the inspection run's body, which it always names (the owner's request)", () => {
    const launch: ViewsCheckLaunch = {
      ...LAUNCH,
      run: "inspection",
      served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
    };
    expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toEqual(launch);
    expect(readViewsCheckLaunch({ ...LAUNCH, run: "inspection" })).toBeNull();
    expect(readViewsCheckLaunch({ ...launch, fixture: "terrain-test-weathered" })).toBeNull();
  });

  it("carries the inspection run's frame profile, on the inspection run alone (the owner's report)", () => {
    const launch: ViewsCheckLaunch = {
      ...LAUNCH,
      run: "inspection",
      served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
      timing: true,
    };
    expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toEqual(launch);
    expect(readViewsCheckLaunch({ ...launch, timing: false })).toBeNull();
    expect(
      readViewsCheckLaunch({ ...launch, run: "handover", fixture: "terrain-test" }),
    ).toBeNull();
    expect(readViewsCheckLaunch({ ...LAUNCH, timing: true })).toBeNull();
  });

  it("carries the frame profile's composite probe, on that profile alone (the owner's priority)", () => {
    const profile: ViewsCheckLaunch = {
      ...LAUNCH,
      run: "inspection",
      served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
      timing: true,
    };
    for (const compositeProbe of [
      "sky-off",
      "transmittance-off",
      "per-pixel",
      "surface-grey",
    ] as const) {
      const launch: ViewsCheckLaunch = { ...profile, compositeProbe };
      expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toEqual(launch);
    }
    expect(readViewsCheckLaunch({ ...profile, compositeProbe: "march-off" })).toBeNull();
    const { timing: _, ...inspection } = profile;
    expect(readViewsCheckLaunch({ ...inspection, compositeProbe: "sky-off" })).toBeNull();
    expect(readViewsCheckLaunch({ ...LAUNCH, compositeProbe: "sky-off" })).toBeNull();
  });

  it("carries the hand-over check's one look, on a hand-over check alone (R10.T15.b.2)", () => {
    const launch: ViewsCheckLaunch = {
      ...LAUNCH,
      run: "handover",
      served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
      look: "handover-90",
    };
    expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toEqual(launch);
    const fixtureLook: ViewsCheckLaunch = { ...LAUNCH, run: "handover", look: "handover-30" };
    expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(fixtureLook)])).toEqual(fixtureLook);
    expect(readViewsCheckLaunch({ ...launch, look: "handover-45" })).toBeNull();
    expect(readViewsCheckLaunch({ ...LAUNCH, run: "terrain", look: "handover-30" })).toBeNull();
    expect(readViewsCheckLaunch({ ...LAUNCH, look: "handover-30" })).toBeNull();
  });

  it("carries the terrain run's GPU time by pass, on a terrain run alone (R14's Mac measurements)", () => {
    const launch: ViewsCheckLaunch = { ...LAUNCH, run: "terrain", passTimes: true };
    expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(launch)])).toEqual(launch);
    const whole: ViewsCheckLaunch = {
      ...launch,
      fixture: "terrain-test-whole",
      wholeField: "/data/whole.golden",
    };
    expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(whole)])).toEqual(whole);
    expect([
      readViewsCheckLaunch({ ...launch, passTimes: false }),
      readViewsCheckLaunch({ ...LAUNCH, passTimes: true }),
      readViewsCheckLaunch({ ...LAUNCH, run: "handover", passTimes: true }),
    ]).toEqual([null, null, null]);
  });

  it("carries the halo's look on a hand-over check of the weathered fixture (R10.T1.c)", () => {
    const halo: ViewsCheckLaunch = {
      ...LAUNCH,
      run: "handover",
      fixture: "terrain-test-weathered",
      look: "handover-halo",
    };
    expect(viewsCheckLaunchFromArgv([viewsCheckSwitch(halo)])).toEqual(halo);
    expect(readViewsCheckLaunch({ ...LAUNCH, run: "terrain", look: "handover-halo" })).toBeNull();
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
