import { describe, expect, it } from "vitest";

import { DEFAULT_FRAME_PACING, framePacingFromArgv, framePacingSwitch } from "./framePacingLaunch";

describe("the frame pacing's hand-off (R07.T19)", () => {
  it.each(["rate", "display"] as const)("carries the pacing %s the launch gave", (pacing) => {
    const argv = ["/opt/hyperion/hyperion", "--type=renderer", framePacingSwitch(pacing)];

    expect(framePacingFromArgv(argv)).toBe(pacing);
  });

  it("throws when the window was created without it", () => {
    expect(() => framePacingFromArgv(["/opt/hyperion/hyperion", "--type=renderer"])).toThrow(
      "--hyperion-frame-pacing=",
    );
  });

  it("throws on a pacing that is neither rate nor display", () => {
    expect(() => framePacingFromArgv(["--hyperion-frame-pacing=vsync"])).toThrow(
      "--hyperion-frame-pacing=",
    );
  });

  it("keeps today's frame counting as the default until the owner rules", () => {
    expect(DEFAULT_FRAME_PACING).toBe("display");
  });
});
