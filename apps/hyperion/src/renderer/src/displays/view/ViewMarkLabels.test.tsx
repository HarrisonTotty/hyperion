import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { DrawAnchor } from "../../view/wireframe/drawList";
import { type MarkRow, MISSING_READING } from "./viewRun";
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
    closure: "+3.40 m/s",
    ...overrides,
  };
}

/** The labels' text, which is hidden from assistive technology and so read from the DOM. */
function labelText(row: MarkRow): string {
  const { container } = render(
    <ViewMarkLabels anchors={[ANCHOR]} devicePixelRatio={1} rows={[row]} />,
  );
  return container.textContent;
}

describe("a mark's label", () => {
  it("reads the range and the closure rate", () => {
    expect(labelText(aRow())).toBe("OTHER · TEST HULL 5.00 km +3.40 m/s");
  });

  it("says FROM CAMERA, as the list does, where there is no own ship", () => {
    expect(labelText(aRow({ fromCamera: true, closure: null }))).toBe(
      "OTHER · TEST HULL 5.00 km FROM CAMERA",
    );
  });

  it("shows a closure rate it does not have as a muted em dash", () => {
    const { container } = render(
      <ViewMarkLabels
        anchors={[ANCHOR]}
        devicePixelRatio={1}
        rows={[aRow({ closure: MISSING_READING })]}
      />,
    );
    const missing = container.querySelector(".readout__missing");
    expect([container.textContent, missing?.textContent]).toEqual([
      "OTHER · TEST HULL 5.00 km —",
      "—",
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
    const node: unknown = labelRef.mock.calls[0]?.[1];
    expect([
      labelRef.mock.calls[0]?.[0],
      node instanceof HTMLElement ? node.style.transform : null,
    ]).toEqual(["craft:other", "translate(calc(100px + 0.75rem), 50px)"]);
    unmount();
    expect(labelRef).toHaveBeenLastCalledWith("craft:other", null);
  });
});
