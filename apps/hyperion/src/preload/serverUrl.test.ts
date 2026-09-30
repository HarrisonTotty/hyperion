import { describe, expect, it } from "vitest";

import { serverUrlFromArgv, serverUrlSwitch } from "./serverUrl";

describe("the server URL hand-off", () => {
  it("carries the URL the main process resolved", () => {
    const argv = ["/opt/hyperion/hyperion", serverUrlSwitch("ws://10.0.0.5:9100/ws")];

    expect(serverUrlFromArgv(argv)).toBe("ws://10.0.0.5:9100/ws");
  });

  it("finds it among the switches Chromium adds", () => {
    const argv = [
      "/opt/hyperion/hyperion",
      "--type=renderer",
      serverUrlSwitch("ws://[::1]:7878/ws"),
      "--enable-sandbox",
    ];

    expect(serverUrlFromArgv(argv)).toBe("ws://[::1]:7878/ws");
  });

  it("throws when the window was created without it", () => {
    expect(() => serverUrlFromArgv(["/opt/hyperion/hyperion", "--type=renderer"])).toThrow(
      "--hyperion-server-url=",
    );
  });
});
