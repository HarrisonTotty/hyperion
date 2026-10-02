import { describe, expect, it, vi } from "vitest";

import { denyPermissionRequests, refusePermission } from "./permissions";

describe("the permission handler", () => {
  it.each(["media", "notifications", "geolocation", "midiSysex"])("refuses %s", (permission) => {
    const decide = vi.fn<(granted: boolean) => void>();
    refusePermission(null, permission, decide);
    expect(decide).toHaveBeenCalledExactlyOnceWith(false);
  });

  it("is installed on the session", () => {
    const installed: unknown[] = [];
    denyPermissionRequests({
      setPermissionRequestHandler: (handler) => {
        installed.push(handler);
      },
    });
    expect(installed).toEqual([refusePermission]);
  });
});
