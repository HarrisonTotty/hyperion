import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { DrawAnchor } from "../../view/wireframe/drawList";
import type { MarkRow } from "./viewRun";
import { ViewMarkLabels } from "./ViewMarkLabels";

const ANCHOR: DrawAnchor = {
  target: { kind: "craft", craft: "other" },
  xPx: 200,
  yPx: 100,
  distanceM: 5_000,
  label: { kind: "target", rangeM: 5_000, closureMPerS: null },
};

function aRow(overrides: Partial<MarkRow> = {}): MarkRow {
  return {
    key: "craft:other",
    target: { kind: "craft", craft: "other" },
    name: "OTHER · TEST HULL",
    kind: "CRAFT",
    range: "5.00 km",
    unit: "km",
    fromCamera: false,
    closure: { kind: "known", text: "+3.40 m/s" },
    ...overrides,
  };
}

/** The label of `row`, found by its name: the labels are hidden from assistive technology. */
function labelOf(row: MarkRow): HTMLElement {
  render(<ViewMarkLabels anchors={[ANCHOR]} devicePixelRatio={1} rows={[row]} />);
  return screen.getByText(row.name, { exact: false });
}

describe("a mark's label", () => {
  it("reads the range and the closure rate", () => {
    expect(labelOf(aRow())).toHaveTextContent(/^OTHER · TEST HULL 5\.00 km \+3\.40 m\/s$/);
  });

  it("says FROM CAMERA, as the list does, where there is no own ship", () => {
    expect(labelOf(aRow({ fromCamera: true, closure: { kind: "none" } }))).toHaveTextContent(
      /^OTHER · TEST HULL 5\.00 km FROM CAMERA$/,
    );
  });

  it("shows a closure rate it does not have as a muted em dash", () => {
    const label = labelOf(aRow({ closure: { kind: "unknown" } }));
    expect([label.textContent, screen.getByText("—")]).toEqual([
      "OTHER · TEST HULL 5.00 km —",
      expect.objectContaining({ className: "readout__missing" }),
    ]);
  });

  it("hands each label to the drawing loop by its target's key, placed beside its mark", () => {
    const labelRef = vi.fn<(key: string, node: HTMLElement | null) => void>();
    const { unmount } = render(
      <ViewMarkLabels
        anchors={[ANCHOR]}
        devicePixelRatio={2}
        rows={[aRow()]}
        labelRef={labelRef}
      />,
    );
    expect([labelRef.mock.calls[0]?.[0], labelRef.mock.calls[0]?.[1]?.style.transform]).toEqual([
      "craft:other",
      "translate(calc(100px + 0.75rem), 50px)",
    ]);
    unmount();
    expect(labelRef).toHaveBeenLastCalledWith("craft:other", null);
  });
});
