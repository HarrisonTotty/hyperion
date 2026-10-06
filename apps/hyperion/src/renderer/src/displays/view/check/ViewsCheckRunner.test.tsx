import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ViewsCheckApi } from "../../../../../preload/api";
import { ViewsCheckRunner } from "./ViewsCheckRunner";
import { ViewsProbe } from "./viewsProbe";

afterEach(() => {
  vi.useRealTimers();
});

/** A check launch's functions, the ending's two mocks. */
function checkApi(): ViewsCheckApi & {
  readonly end: ReturnType<typeof vi.fn<ViewsCheckApi["end"]>>;
  readonly writeResults: ReturnType<typeof vi.fn<ViewsCheckApi["writeResults"]>>;
} {
  return {
    launch: { setting: "high", smoke: true, out: null },
    startPhase: () => Promise.resolve(),
    endPhase: () => Promise.resolve(),
    askRightWayUp: () => Promise.resolve(),
    writeResults: vi.fn<ViewsCheckApi["writeResults"]>(() =>
      Promise.resolve({ json: "a", markdown: "b" }),
    ),
    end: vi.fn<ViewsCheckApi["end"]>(() => Promise.resolve()),
  };
}

describe("ViewsCheckRunner", () => {
  it("gives the page its own animation frames back on unmount", () => {
    const own = window.requestAnimationFrame;
    const { unmount } = render(<ViewsCheckRunner api={checkApi()} probe={new ViewsProbe()} />);
    expect(window.requestAnimationFrame).not.toBe(own);

    unmount();

    expect(window.requestAnimationFrame).toBe(own);
  });

  it("ends nothing once unmounted", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    const api = checkApi();
    const { unmount } = render(<ViewsCheckRunner api={api} probe={new ViewsProbe()} />);

    unmount();
    // Past the script's first wait, which a run still going would have failed.
    await vi.advanceTimersByTimeAsync(61_000);

    expect(api.end).not.toHaveBeenCalled();
    expect(api.writeResults).not.toHaveBeenCalled();
  });
});
