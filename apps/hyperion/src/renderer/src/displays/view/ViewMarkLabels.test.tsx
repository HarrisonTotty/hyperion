import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { markStrokeDevicePx, markShiftDevicePx } from "../../lib/strokes";
import { SYMBOL_STROKE_PX } from "../../spatial/symbols";
import type { DrawAnchor } from "../../view/wireframe/drawList";
import {
  BRACKET_MARGIN_REM,
  contactRadiusPx,
  reticleGrowthPx,
} from "../../view/wireframe/symbology";
import type { MarkRow } from "./viewRun";
import {
  LABEL_OFFSET_REM,
  markLabelShiftPx,
  markLabelTransform,
  ViewMarkLabels,
} from "./ViewMarkLabels";

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

/** The ratios and interface scales the label's place is tested at. */
const PLACES = [0.78125, 1, 2].flatMap((ratio) =>
  [0.8, 1, 1.5].map((scale) => [ratio, scale] as const),
);

describe("a label's place beside its mark (R07.T16.d)", () => {
  it("stands off by the selection's reticle's growth below a ratio of 4/3, and no more from it", () => {
    expect([
      [0.78125, 1, 2].map(markLabelShiftPx),
      markLabelTransform(ANCHOR, 1),
      markLabelTransform(ANCHOR, 2),
    ]).toEqual([
      [2.65, 1.25, 0],
      "translate(calc(200px + 0.75rem + 1.25px), 100px)",
      "translate(calc(100px + 0.75rem), 50px)",
    ]);
  });

  it.each(PLACES)(
    "keeps a selected craft's bracket clear of the plate, as a 1.5 px outline was, at %s and %s",
    (ratio, scale) => {
      const remPx = 16 * scale * ratio;
      const shift = markShiftDevicePx(ratio);
      // The bracket's half-size, device px, as built and as drawn now; and its outer reach.
      const asBuilt = contactRadiusPx(remPx) + BRACKET_MARGIN_REM * remPx;
      const nowReach =
        asBuilt + reticleGrowthPx(shift) - shift + markStrokeDevicePx(ratio) / 2 + 0.5;
      const thenReach = asBuilt + (SYMBOL_STROKE_PX * ratio) / 2 + 0.5;
      const plate = (LABEL_OFFSET_REM * 16 * scale + markLabelShiftPx(ratio)) * ratio;
      const clearance = plate - nowReach;
      expect([
        clearance >= 0,
        Math.abs(clearance - (LABEL_OFFSET_REM * remPx - thenReach)) < 1e-9,
      ]).toEqual([true, true]);
    },
  );
});
