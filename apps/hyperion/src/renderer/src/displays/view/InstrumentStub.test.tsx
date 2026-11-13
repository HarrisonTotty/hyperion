/**
 * A held `INSTRUMENT 2`'s stub (plan R10, R10.T12.b; decision-r10-view-block-room §3): its title
 * row with `CLOSE`, and why the instrument is not drawn.
 */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { describedText, statusLine } from "../../test/statusLine";
import { InstrumentStub } from "./InstrumentStub";

describe("INSTRUMENT 2's stub (R10.T12.b)", () => {
  it("stands as a region named for its slot", () => {
    render(<InstrumentStub name="INSTRUMENT 2" onClose={() => undefined} />);
    expect(screen.getByRole("region", { name: "INSTRUMENT 2" })).toBeInTheDocument();
  });

  it("closes its slot once from its CLOSE", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn<() => void>();
    render(<InstrumentStub name="INSTRUMENT 2" onClose={onClose} />);
    await user.click(
      within(screen.getByRole("region", { name: "INSTRUMENT 2" })).getByRole("button", {
        name: "CLOSE",
      }),
    );
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("states why the instrument is not drawn", () => {
    render(<InstrumentStub name="INSTRUMENT 2" onClose={() => undefined} />);
    expect(
      within(screen.getByRole("region", { name: "INSTRUMENT 2" })).getByText(
        statusLine("NOT AVAILABLE: no room at this window size"),
      ),
    ).toBeInTheDocument();
  });

  it("is described by its reason", () => {
    render(<InstrumentStub name="INSTRUMENT 2" onClose={() => undefined} />);
    expect(describedText(screen.getByRole("region", { name: "INSTRUMENT 2" }))).toBe(
      "NOT AVAILABLE: no room at this window size",
    );
  });
});
