import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { GraphicsLaunchMode } from "../../../preload/api";
import { DISPLAYS, type DisplayId } from "../lib/displays";
import {
  type GraphicsEvent,
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../view/engine/status";
import { ConsoleFrame } from "./ConsoleFrame";

function graphicsStore(
  mode: GraphicsLaunchMode,
  events: ReadonlyArray<GraphicsEvent>,
): GraphicsStatusStore {
  const store = new GraphicsStatusStore(initialGraphicsStatus(mode, false));
  for (const event of events) {
    store.dispatch(event);
  }
  return store;
}

function renderFrame(
  activeDisplay: DisplayId,
  onSelectDisplay: (id: DisplayId) => void,
  store: GraphicsStatusStore = graphicsStore("vulkan", []),
): ReturnType<typeof render> {
  return render(
    <GraphicsStatusContext value={store}>
      <ConsoleFrame
        displays={DISPLAYS}
        activeDisplay={activeDisplay}
        onSelectDisplay={onSelectDisplay}
        linkStatus="connected"
      >
        <p>Work area</p>
      </ConsoleFrame>
    </GraphicsStatusContext>,
  );
}

const LOST: GraphicsEvent = { kind: "device-lost", reason: "unknown", message: "gpu gone" };

/** The header strip's graphics banner, which must be there. */
function banner(): HTMLElement {
  return within(screen.getByRole("banner")).getByRole("status", { name: "Graphics mode" });
}

function tabs(): HTMLElement[] {
  return within(screen.getByRole("navigation", { name: "Displays" })).getAllByRole("button");
}

describe("ConsoleFrame", () => {
  it("offers every display as a button showing its key, in order", () => {
    renderFrame("link", () => {});

    expect(tabs().map((tab) => tab.textContent)).toEqual([
      "F1 Link",
      "F2 Galaxy",
      "F3 System",
      "F4 View",
    ]);
    expect(screen.getByRole("button", { name: "F1 Link" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "F2 Galaxy" })).toBeInTheDocument();
  });

  it("titles the header and marks the tab of the active display", () => {
    renderFrame("galaxy", () => {});

    expect(screen.getByRole("heading", { level: 1, name: "Galaxy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "F2 Galaxy" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByRole("button", { name: "F1 Link" })).not.toHaveAttribute("aria-current");
  });

  it("reports a clicked tab", async () => {
    const user = userEvent.setup();
    const onSelectDisplay = vi.fn<(id: DisplayId) => void>();
    renderFrame("link", onSelectDisplay);

    await user.click(screen.getByRole("button", { name: "F2 Galaxy" }));

    expect(onSelectDisplay).toHaveBeenCalledExactlyOnceWith("galaxy");
  });

  it("reaches the tabs with Tab and activates one with Enter", async () => {
    const user = userEvent.setup();
    const onSelectDisplay = vi.fn<(id: DisplayId) => void>();
    renderFrame("link", onSelectDisplay);

    await user.tab();
    expect(screen.getByRole("button", { name: "F1 Link" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "F2 Galaxy" })).toHaveFocus();
    await user.keyboard("{Enter}");

    expect(onSelectDisplay).toHaveBeenCalledExactlyOnceWith("galaxy");
  });

  it("shows the safe mode in the header strip", () => {
    renderFrame("link", () => {}, graphicsStore("safe", []));
    expect(banner()).toHaveTextContent(/^GRAPHICS SAFE MODE$/);
  });

  it("shows WebGPU disabled by device losses in the header strip", () => {
    renderFrame("link", () => {}, graphicsStore("vulkan", [LOST, LOST, LOST]));
    expect(banner()).toHaveTextContent(/^GRAPHICS DISABLED$/);
  });

  it("shows WebGPU disabled by a withdrawn adapter in the header strip", () => {
    renderFrame("link", () => {}, graphicsStore("vulkan", [LOST, { kind: "adapter-withdrawn" }]));
    expect(banner()).toHaveTextContent(/^GRAPHICS DISABLED$/);
  });

  it("shows no graphics banner otherwise, a fault included", () => {
    renderFrame(
      "link",
      () => {},
      graphicsStore("vulkan", [LOST, { kind: "gpu-process-gone", count: 1 }]),
    );
    expect(
      within(screen.getByRole("banner")).queryByRole("status", { name: "Graphics mode" }),
    ).not.toBeInTheDocument();
  });

  it("shows the same banner on every display", () => {
    const store = graphicsStore("safe", []);
    const texts = DISPLAYS.map(({ id }) => {
      const { unmount } = renderFrame(id, () => {}, store);
      const text = banner().textContent;
      unmount();
      return text;
    });
    expect(new Set(texts).size).toBe(1);
    expect(texts[0]).toContain("GRAPHICS SAFE MODE");
  });

  it("refuses an active display that is not offered", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});

    expect(() => {
      render(
        <ConsoleFrame
          displays={DISPLAYS.slice(0, 1)}
          activeDisplay="galaxy"
          onSelectDisplay={() => {}}
          linkStatus="connected"
        >
          <p>Work area</p>
        </ConsoleFrame>,
      );
    }).toThrow(/galaxy/);
  });
});
