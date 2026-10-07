import { describe, expect, it } from "vitest";

import { qualityFromArgv, qualitySwitch } from "./qualityLaunch";

describe("the quality setting's hand-off (R07.T17)", () => {
  it.each(["high", "low"] as const)("carries the setting %s the launch gave", (setting) => {
    const argv = ["/opt/hyperion/hyperion", "--type=renderer", qualitySwitch(setting)];

    expect(qualityFromArgv(argv)).toBe(setting);
  });

  it("throws when the window was created without it", () => {
    expect(() => qualityFromArgv(["/opt/hyperion/hyperion", "--type=renderer"])).toThrow(
      "--hyperion-quality=",
    );
  });

  it("throws on a setting that is neither high nor low", () => {
    expect(() => qualityFromArgv(["--hyperion-quality=medium"])).toThrow("--hyperion-quality=");
  });
});
