import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import type { WhiteBalance } from "../../view/post/whiteBalance";
import { ExposurePanel } from "./ExposurePanel";

/** The panel with the white balance standing, its choice held as the display holds it. */
function Panel({ teffK }: { readonly teffK: number | null }) {
  const [balance, setBalance] = useState<WhiteBalance>("host");
  return (
    <ExposurePanel
      exposure={{ kind: "auto", ev100: 9.6 }}
      metering={{ kind: "metered", ev100: 9.6 }}
      onChange={() => undefined}
      whiteBalance={{ balance, balancedTeffK: teffK, onBalance: setBalance }}
    />
  );
}

describe("the Exposure panel's WHITE BALANCE", () => {
  it("reads the star it adapts to and offers D65 and HOST under SELECT, HOST pressed", () => {
    render(<Panel teffK={3_444} />);
    expect(screen.getByRole("status", { name: "WHITE BALANCE" }).textContent).toBe(
      "HOST · T EFF 3444 K",
    );
    const select = screen.getByRole("group", { name: "SELECT" });
    expect(
      within(select)
        .getAllByRole("button")
        .map((button) => [button.textContent, button.getAttribute("aria-pressed")]),
    ).toEqual([
      ["D65", "false"],
      ["HOST", "true"],
    ]);
  });

  it("takes D65 at a press, at once", async () => {
    const user = userEvent.setup();
    render(<Panel teffK={3_444} />);
    await user.click(screen.getByRole("button", { name: "D65" }));
    expect(screen.getByRole("status", { name: "WHITE BALANCE" }).textContent).toBe("D65");
    expect(screen.getByRole("button", { name: "D65" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("reads HOST with the em dash while no host light is drawn", () => {
    render(<Panel teffK={null} />);
    expect(screen.getByRole("status", { name: "WHITE BALANCE" }).textContent).toBe("HOST · —");
  });

  it("stands only where the display draws a photorealistic image", () => {
    render(
      <ExposurePanel
        exposure={{ kind: "auto", ev100: 9.6 }}
        metering={{ kind: "no-image" }}
        onChange={() => undefined}
      />,
    );
    expect(screen.queryByRole("group", { name: "SELECT" })).toBeNull();
    expect(screen.queryByText("WHITE BALANCE")).toBeNull();
  });
});
