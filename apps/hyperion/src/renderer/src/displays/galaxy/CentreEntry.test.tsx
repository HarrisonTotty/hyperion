import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { CentreLy } from "../../lib/galaxy/model";
import { CentreEntry } from "./CentreEntry";

interface HarnessProps {
  readonly onCentre: (centreLy: CentreLy) => void;
  readonly onCursor: (cursorLy: CentreLy) => void;
  readonly heldBack?: string | null;
}

/** Holds the cursor as the display does, starting off the axis, beside a button to take focus. */
function Harness({ onCentre, onCursor, heldBack = null }: HarnessProps) {
  const [cursorLy, setCursorLy] = useState<CentreLy>([26_000, 0, 12]);
  return (
    <>
      <button type="button">ELSEWHERE</button>
      <CentreEntry
        cursorLy={cursorLy}
        onCursor={(next) => {
          onCursor(next);
          setCursorLy(next);
        }}
        onCentre={onCentre}
        heldBack={heldBack}
      />
    </>
  );
}

function renderEntry(heldBack: string | null = null) {
  const user = userEvent.setup();
  const onCentre = vi.fn<(centreLy: CentreLy) => void>();
  const onCursor = vi.fn<(cursorLy: CentreLy) => void>();
  render(<Harness onCentre={onCentre} onCursor={onCursor} heldBack={heldBack} />);
  return { user, onCentre, onCursor };
}

function field(label: "X" | "Y" | "Z"): HTMLInputElement {
  const element = screen.getByRole("textbox", { name: label });
  if (!(element instanceof HTMLInputElement)) {
    throw new Error(`the ${label} field is not an input`);
  }
  return element;
}

/** The readout's value for `label`, as shown. */
function reading(label: string): string | null | undefined {
  return screen.getByText(label, { exact: true }).nextElementSibling?.textContent;
}

describe("CentreEntry", () => {
  it("shows the cursor in its fields, with the range and unit they take", () => {
    renderEntry();

    expect(field("X")).toHaveValue("26,000.0");
    expect(field("Y")).toHaveValue("0.0");
    expect(field("Z")).toHaveValue("12.0");
    expect(field("X")).toHaveAccessibleDescription("±65,536 ly");
  });

  it("reads the cursor's radius, angle and height in the GALACTIC frame", () => {
    renderEntry();

    // The guide's § Voice: the three stand under the heading GALACTIC.
    expect(screen.getByRole("heading", { name: "GALACTIC" }).nextElementSibling).toContainElement(
      screen.getByText("RADIUS", { exact: true }),
    );
    expect(reading("RADIUS")).toBe("26,000.0 ly");
    expect(reading("ANGLE")).toBe("000.0°");
    expect(reading("HEIGHT")).toBe("+12.0 ly");
  });

  it("moves the cursor when a coordinate in range is entered", async () => {
    const { user } = renderEntry();

    await user.clear(field("X"));
    await user.type(field("X"), "-30000");
    await user.tab();

    expect(reading("RADIUS")).toBe("30,000.0 ly");
    expect(reading("ANGLE")).toBe("180.0°");
  });

  it("leaves the cursor and says nothing while a value is still being typed", async () => {
    const { user } = renderEntry();

    await user.clear(field("X"));
    await user.type(field("X"), "-");

    expect(field("X")).toHaveValue("-");
    expect(field("X")).not.toHaveAttribute("aria-invalid");
    expect(reading("RADIUS")).toBe("26,000.0 ly");
  });

  it("shows an accepted coordinate in the cursor's format once the field is left", async () => {
    const { user } = renderEntry();

    await user.clear(field("Y"));
    await user.type(field("Y"), "50000");
    await user.tab();

    expect(field("Y")).toHaveValue("50,000.0");
  });

  it("refuses a coordinate outside the root cube, and keeps the cursor", async () => {
    const { user } = renderEntry();

    await user.clear(field("X"));
    await user.type(field("X"), "70000");
    await user.tab();

    expect(field("X")).toHaveValue("70000");
    expect(field("X")).toHaveAttribute("aria-invalid", "true");
    expect(field("X")).toHaveAccessibleDescription(
      "±65,536 ly X INVALID: enter -65,536 to 65,535.9999 ly",
    );
    expect(reading("RADIUS")).toBe("26,000.0 ly");
  });

  it("refuses text that is not a number, and keeps the cursor", async () => {
    const { user } = renderEntry();

    await user.clear(field("Z"));
    await user.type(field("Z"), "north");
    await user.tab();

    expect(screen.getByText("Z INVALID: enter -65,536 to 65,535.9999 ly")).toBeInTheDocument();
    expect(reading("HEIGHT")).toBe("+12.0 ly");
  });

  it("centres the chart on the cursor when C is pressed outside a field", async () => {
    const { user, onCentre } = renderEntry();
    await user.click(screen.getByRole("button", { name: "ELSEWHERE" }));

    await user.keyboard("c");

    expect(onCentre).toHaveBeenCalledWith([26_000, 0, 12]);
  });

  it("types C into a field instead of centring the chart", async () => {
    const { user, onCentre } = renderEntry();
    await user.clear(field("Y"));

    await user.type(field("Y"), "c");

    expect(field("Y")).toHaveValue("c");
    expect(onCentre).not.toHaveBeenCalled();
  });

  it("ignores C with a modifier", async () => {
    const { user, onCentre } = renderEntry();
    await user.click(screen.getByRole("button", { name: "ELSEWHERE" }));

    await user.keyboard("{Shift>}c{/Shift}{Control>}c{/Control}");

    expect(onCentre).not.toHaveBeenCalled();
  });

  it("centres the chart from CENTRE CHART, which shows its key", async () => {
    const { user, onCentre } = renderEntry();

    await user.click(screen.getByRole("button", { name: "C CENTRE CHART" }));

    expect(onCentre).toHaveBeenCalledWith([26_000, 0, 12]);
  });

  it("enters a coordinate with Enter, without centring the chart", async () => {
    const { user, onCentre } = renderEntry();
    await user.clear(field("Z"));

    await user.type(field("Z"), "-40{Enter}");

    expect(reading("HEIGHT")).toBe("-40.0 ly");
    expect(onCentre).not.toHaveBeenCalled();
  });

  it("centres the chart on a coordinate typed and not yet entered", async () => {
    const { user, onCentre } = renderEntry();
    await user.clear(field("Y"));
    await user.type(field("Y"), "300");

    await user.click(screen.getByRole("button", { name: "C CENTRE CHART" }));

    expect(onCentre).toHaveBeenCalledWith([26_000, 300, 12]);
  });

  it("centres the chart once for a held key", async () => {
    const { user, onCentre } = renderEntry();
    await user.click(screen.getByRole("button", { name: "ELSEWHERE" }));

    await user.keyboard("{c>3/}");

    expect(onCentre).toHaveBeenCalledTimes(1);
  });

  it("shows an entered coordinate with the decimals it was typed with", async () => {
    const { user } = renderEntry();
    await user.clear(field("Z"));

    await user.type(field("Z"), "0.05{Enter}");

    expect(field("Z")).toHaveValue("0.05");
    expect(reading("HEIGHT")).toBe("+0.1 ly");
  });

  it("refuses a value that rounds, at the four decimals an entry keeps, onto the cube's upper face", async () => {
    const { user } = renderEntry();
    await user.clear(field("X"));

    await user.type(field("X"), "65535.99996{Enter}");

    expect(field("X")).toHaveAttribute("aria-invalid", "true");
    expect(reading("RADIUS")).toBe("26,000.0 ly");
  });

  it("takes the largest value an entry can hold in the cube, and shows its unit", async () => {
    const { user } = renderEntry();
    await user.clear(field("Y"));

    await user.type(field("Y"), "65535.9999{Enter}");

    expect(field("Y")).toHaveValue("65,535.9999");
    expect(field("Y")).not.toHaveAttribute("aria-invalid");
    expect(field("Y").nextElementSibling).toHaveTextContent("ly");
  });

  it("takes -65,536 ly, the root cube's lower face, and refuses 65,536 ly, outside it", async () => {
    const { user } = renderEntry();
    await user.clear(field("X"));
    await user.type(field("X"), "-65536{Enter}");
    await user.clear(field("Y"));

    await user.type(field("Y"), "65536{Enter}");

    expect(field("X")).toHaveValue("-65,536.0");
    expect(field("X")).not.toHaveAttribute("aria-invalid");
    expect(field("Y")).toHaveAttribute("aria-invalid", "true");
  });

  it("holds CENTRE CHART back while an entry is refused, saying why", async () => {
    const { user, onCentre } = renderEntry();
    await user.clear(field("X"));
    await user.type(field("X"), "70000{Enter}");
    const centre = screen.getByRole("button", { name: "C CENTRE CHART" });

    await user.click(centre);

    expect(centre).toHaveAttribute("aria-disabled", "true");
    expect(centre).toHaveAccessibleDescription("X INVALID: enter -65,536 to 65,535.9999 ly");
    expect(onCentre).not.toHaveBeenCalled();
  });

  it("drops a refused coordinate on Escape, showing the cursor's and offering CENTRE CHART", async () => {
    const { user, onCursor } = renderEntry();
    await user.clear(field("X"));
    await user.type(field("X"), "70000{Enter}");
    const refused = field("X").getAttribute("aria-invalid");

    await user.keyboard("{Escape}");
    await user.tab();

    expect([refused, field("X").value, field("X").getAttribute("aria-invalid")]).toEqual([
      "true",
      "26,000.0",
      null,
    ]);
    expect(screen.queryByText(/INVALID/u)).toBeNull();
    expect(screen.getByRole("button", { name: "C CENTRE CHART" })).not.toHaveAttribute(
      "aria-disabled",
    );
    expect(onCursor).not.toHaveBeenCalled();
  });

  it("keeps Escape's focus in the field", async () => {
    const { user } = renderEntry();
    await user.clear(field("Z"));
    await user.type(field("Z"), "north{Enter}");

    await user.keyboard("{Escape}");

    expect(field("Z")).toHaveFocus();
    expect(field("Z")).toHaveValue("12.0");
  });

  it("enters nothing for a field emptied and left, and refuses nothing", async () => {
    const { user, onCursor } = renderEntry();
    await user.clear(field("Y"));

    await user.tab();

    expect([field("Y").value, field("Y").getAttribute("aria-invalid")]).toEqual(["0.0", null]);
    expect(screen.queryByText(/INVALID/u)).toBeNull();
    expect(onCursor).not.toHaveBeenCalled();
  });

  it("enters nothing for a field of spaces only, and refuses nothing", async () => {
    const { user, onCursor } = renderEntry();
    await user.clear(field("X"));

    await user.type(field("X"), "   {Enter}");

    expect([field("X").value, field("X").getAttribute("aria-invalid")]).toEqual(["26,000.0", null]);
    expect(onCursor).not.toHaveBeenCalled();
  });

  it("selects the coordinate a refusal's Escape restores, so that typing replaces it", async () => {
    const { user, onCursor } = renderEntry();
    await user.clear(field("X"));
    await user.type(field("X"), "70000{Enter}");

    await user.keyboard("{Escape}");
    const selection = [field("X").selectionStart, field("X").selectionEnd];
    await user.keyboard("5");

    expect([selection, field("X").value]).toEqual([[0, "26,000.0".length], "5"]);
    expect(onCursor).not.toHaveBeenCalled();
  });

  it("selects the coordinate on Escape with nothing typed, wherever the caret was", async () => {
    const { user } = renderEntry();
    await user.pointer({ keys: "[MouseLeft]", target: field("Z"), offset: 1 });

    await user.keyboard("{Escape}");

    expect([field("Z").selectionStart, field("Z").selectionEnd]).toEqual([0, "12.0".length]);
  });

  it("selects the coordinate again when an emptied field is entered with Enter", async () => {
    const { user, onCursor } = renderEntry();
    await user.clear(field("Y"));

    await user.keyboard("{Enter}");
    const selection = [field("Y").selectionStart, field("Y").selectionEnd];
    await user.keyboard("5");

    expect([selection, field("Y").value]).toEqual([[0, "0.0".length], "5"]);
    expect(onCursor).not.toHaveBeenCalled();
  });

  it("enters what is typed after Escape on leaving, not the coordinate with it appended", async () => {
    const { user, onCursor } = renderEntry();
    await user.click(field("Z"));
    await user.keyboard("9{Escape}3");

    await user.tab();

    expect(onCursor.mock.calls).toEqual([[[26_000, 0, 3]]]);
  });

  it("announces the whole position as the cursor moves", async () => {
    const { user } = renderEntry();

    await user.clear(field("Y"));
    await user.type(field("Y"), "-300{Enter}");

    const summary = screen.getByText(/^CURSOR X/);
    expect(summary).toHaveAttribute("aria-live", "polite");
    expect(summary).toHaveAttribute("aria-atomic", "true");
    expect(summary).toHaveTextContent(
      "CURSOR X 26,000.0 ly, Y -300.0 ly, Z 12.0 ly, GALACTIC RADIUS 26,001.7 ly, ANGLE 359.3°, " +
        "HEIGHT +12.0 ly",
    );
  });

  it("holds CENTRE CHART and its key back, saying why in the panel", async () => {
    const { user, onCentre } = renderEntry("NO CARRIER");
    const centre = screen.getByRole("button", { name: "C CENTRE CHART" });

    await user.click(centre);
    await user.keyboard("c");

    expect(centre).toHaveAttribute("aria-disabled", "true");
    expect(centre).toHaveAccessibleDescription("NO CARRIER");
    expect(
      within(screen.getByRole("region", { name: "Cursor" })).getByText("NO CARRIER"),
    ).toBeInTheDocument();
    expect(onCentre).not.toHaveBeenCalled();
  });

  it("is a panel titled CURSOR", () => {
    renderEntry();

    expect(
      within(screen.getByRole("region", { name: "Cursor" })).getByRole("heading", { level: 2 }),
    ).toHaveTextContent("Cursor");
  });
});
