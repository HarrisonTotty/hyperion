import { describe, expect, it } from "vitest";

import { planetViewerFromArgv, planetViewerSwitch } from "./planetViewerLaunch";

describe("the planet viewer's launch switch", () => {
  it("offers the viewer where its switch is on, and only there", () => {
    expect(planetViewerFromArgv(["electron", planetViewerSwitch(true)])).toBe(true);
    expect(planetViewerFromArgv(["electron", planetViewerSwitch(false)])).toBe(false);
    expect(planetViewerFromArgv(["electron"])).toBe(false);
  });
});
