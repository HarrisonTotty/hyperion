import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { stubSurfaceWorker } from "../test/FakeSurfaceWorker";
import { useSurfaceModuleCheck } from "./useSurfaceModuleCheck";

function outcome(): string | undefined {
  return document.documentElement.dataset["surfaceModule"];
}

describe("the surface module check", () => {
  it("writes the ready line and logs nothing when the versions match", async () => {
    const workers = stubSurfaceWorker({ kind: "generator-version", version: 16 });
    const error = vi.spyOn(console, "error");
    const { unmount } = renderHook(() => {
      useSurfaceModuleCheck(16);
    });
    await waitFor(() => {
      expect(outcome()).toBe("surface module ready: generator version 16");
    });
    expect(error).not.toHaveBeenCalled();
    expect(workers).toHaveLength(1);
    unmount();
  });

  it("writes and logs the fault when the module's version is not the server's", async () => {
    stubSurfaceWorker({ kind: "generator-version", version: 15 });
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { unmount } = renderHook(() => {
      useSurfaceModuleCheck(16);
    });
    const line =
      "surface module fault: the module is for generator version 15, the server's is 16: " +
      "run just gen-surface";
    await waitFor(() => {
      expect(outcome()).toBe(line);
    });
    expect(error).toHaveBeenCalledWith(line);
    unmount();
  });

  it("starts no worker until the server has said its version", () => {
    const workers = stubSurfaceWorker({ kind: "generator-version", version: 16 });
    const { unmount } = renderHook(() => {
      useSurfaceModuleCheck(null);
    });
    expect(workers).toHaveLength(0);
    expect(outcome()).toBeUndefined();
    unmount();
  });

  it("terminates the worker and leaves no line when unmounted before the answer", async () => {
    const workers = stubSurfaceWorker("silence");
    const { unmount } = renderHook(() => {
      useSurfaceModuleCheck(16);
    });
    expect(workers).toHaveLength(1);
    unmount();
    await Promise.resolve();
    expect(workers[0]?.terminated).toBe(true);
    expect(outcome()).toBeUndefined();
  });

  it("removes its line when torn down", async () => {
    stubSurfaceWorker({ kind: "generator-version", version: 16 });
    const { unmount } = renderHook(() => {
      useSurfaceModuleCheck(16);
    });
    await waitFor(() => {
      expect(outcome()).toBeDefined();
    });
    unmount();
    expect(outcome()).toBeUndefined();
  });
});
