import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ViewRoomSizers } from "./ViewRoomSizers";

const NOTE = "ATMOSPHERE: APPROXIMATE";
const REASON = "H2O: its absorption estimated, not measured, for KX 4417 /3";

/** The sizers while the slots are held for the plates, the note's reasons opened or not. */
function sizersWith(opened: ReadonlySet<string>): HTMLElement {
  const { container } = render(
    <ViewRoomSizers
      upper={{
        lines: [{ label: "STYLE", value: "PHOTOREALISTIC" }],
        statements: [NOTE, { text: REASON, standing: "sentence" }],
        folds: { opened, toggle: () => undefined },
        countLine: null,
        faults: [],
      }}
      lower={[]}
      liveClass={undefined}
      beside
      scrollsFrom="STARS"
    />,
  );
  const copy = container.querySelector<HTMLElement>('[data-sizer="beside"]');
  if (copy === null) {
    throw new Error("the sizers lay the upper plate out beside the slots while they are held");
  }
  return copy;
}

describe("the room's copy of the upper plate (the owner's request)", () => {
  it("lays its note's reasons out opened while the live plate holds them opened", () => {
    expect(sizersWith(new Set([NOTE])).textContent).toContain(REASON);
  });

  it("lays them out folded while the live plate holds them folded", () => {
    expect(sizersWith(new Set()).textContent).not.toContain(REASON);
  });
});
