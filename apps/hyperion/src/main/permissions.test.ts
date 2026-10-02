import { describe, expect, it, vi } from "vitest";

import { denyPermissionRequests, refusePermission, refusePermissionCheck } from "./permissions";

describe("the permission handlers", () => {
  it.each(["media", "notifications", "geolocation", "midiSysex"])("refuse %s", (permission) => {
    const decide = vi.fn<(granted: boolean) => void>();
    refusePermission(null, permission, decide);
    expect(decide).toHaveBeenCalledExactlyOnceWith(false);
  });

  it("answer no to a permission check", () => {
    expect(refusePermissionCheck()).toBe(false);
  });

  it("are both installed on the session", () => {
    const installed: unknown[] = [];
    denyPermissionRequests({
      setPermissionRequestHandler: (handler) => {
        installed.push(handler);
      },
      setPermissionCheckHandler: (handler) => {
        installed.push(handler);
      },
    });
    expect(installed).toEqual([refusePermission, refusePermissionCheck]);
  });
});
