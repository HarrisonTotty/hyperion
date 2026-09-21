import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { DISPLAYS, type DisplayId } from "../lib/displays";
import { ConsoleFrame } from "./ConsoleFrame";

function renderFrame(activeDisplay: DisplayId, onSelectDisplay: (id: DisplayId) => void): void {
  render(
    <ConsoleFrame
      displays={DISPLAYS}
      activeDisplay={activeDisplay}
      onSelectDisplay={onSelectDisplay}
      linkStatus="connected"
    >
      <p>Work area</p>
    </ConsoleFrame>,
  );
}

function tabs(): HTMLElement[] {
  return within(screen.getByRole("navigation", { name: "Displays" })).getAllByRole("button");
}

describe("ConsoleFrame", () => {
  it("offers every display as a button showing its key, in order", () => {
    renderFrame("link", () => {});

    expect(tabs().map((tab) => tab.textContent)).toEqual(["F1 Link", "F2 Galaxy"]);
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
