import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { DISPLAYS, type DisplayId } from "./displays";
import { useDisplayKeys } from "./useDisplayKeys";

/** A console in miniature: a text field, and the title of the display the keys selected. */
function Harness() {
  const [selected, setSelected] = useState<DisplayId>("link");
  useDisplayKeys(DISPLAYS, setSelected);
  const title = DISPLAYS.find((display) => display.id === selected)?.title ?? "—";
  return (
    <>
      <label>
        Seed value
        <input type="text" />
      </label>
      <output aria-label="Display">{title}</output>
    </>
  );
}

function shownDisplay(): string | null {
  return screen.getByRole("status", { name: "Display" }).textContent;
}

describe("useDisplayKeys", () => {
  it("selects a display by its function key even while a text field has focus", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const field = screen.getByRole("textbox", { name: "Seed value" });

    await user.click(field);
    await user.keyboard("{F2}");

    expect(shownDisplay()).toBe("Galaxy");
    expect(field).toHaveFocus();
    expect(field).toHaveValue("");
  });

  it("ignores a function key pressed with a modifier", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.keyboard("{Control>}{F2}{/Control}");
    await user.keyboard("{Alt>}{F2}{/Alt}");
    await user.keyboard("{Shift>}{F2}{/Shift}");

    expect(shownDisplay()).toBe("Link");
  });

  it("ignores key repeats", () => {
    render(<Harness />);

    // user-event cannot send a lone repeated keydown without the press that starts it.
    fireEvent.keyDown(document, { key: "F2", repeat: true });

    expect(shownDisplay()).toBe("Link");
  });

  it("prevents the default action of a key it handles, and only those", () => {
    render(<Harness />);

    expect(fireEvent.keyDown(document, { key: "F2" })).toBe(false);
    expect(fireEvent.keyDown(document, { key: "F5" })).toBe(true);
  });

  it("removes its listener on unmount", () => {
    const add = vi.spyOn(document, "addEventListener");
    const remove = vi.spyOn(document, "removeEventListener");
    const { unmount } = render(<Harness />);
    const added = add.mock.calls.find(([type]) => type === "keydown");

    unmount();

    expect(added).toBeDefined();
    expect(remove).toHaveBeenCalledWith("keydown", added?.[1]);
  });
});
