import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { contrastRatio, tokenLuminance } from "../../smoke/strokeContrast";
import { stylesheetRule } from "../../test/stylesheet";
import type { DrawAnchor } from "../../view/wireframe/drawList";
import type { MarkRow } from "./viewRun";
import { markLabelTransform, ViewMarkLabels } from "./ViewMarkLabels";

const ANCHOR: DrawAnchor = {
  target: { kind: "craft", craft: "other" },
  xPx: 200,
  yPx: 100,
  distanceM: 5_000,
  label: { kind: "target", rangeM: 5_000, closureMPerS: null },
  labelOffsetPx: 24,
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
      "translate(112px, 50px)",
    ]);
    unmount();
    expect(labelRef).toHaveBeenLastCalledWith("craft:other", null);
  });
});

describe("a label's place beside its mark (R07.T16.g)", () => {
  it("stands its plate the draw list's offset right of its mark, in CSS px", () => {
    expect([markLabelTransform(ANCHOR, 1), markLabelTransform(ANCHOR, 2)]).toEqual([
      "translate(224px, 100px)",
      "translate(112px, 50px)",
    ]);
  });
});

/** A colour token's value, as the stylesheet's `:root` rule declares it. */
function tokenOf(name: string): string {
  return (
    new RegExp("--" + name + ": (#[0-9a-f]{6});", "u").exec(stylesheetRule(":root"))?.[1] ?? ""
  );
}

// The plate over a neighbouring mark is checked in the stylesheet, not in pixels: the DOM over the
// canvas is not captured by `just test-render`, so what of the covered mark shows beside the plate's
// edge is a stated limit, looked at in the by-hand page captures (R07.T16.g, T16.f).
describe("a label's plate over a neighbouring mark (R07.T16.g; decision-r07-t16d-followups)", () => {
  it("is opaque --surface-0 where it lies over a mark that crowds its own", () => {
    // A second craft 2 rem to the right of the first, at a ratio of 1: beyond the first one's
    // plate's near edge, under the plate, which runs on for the label's text.
    const neighbour: DrawAnchor = {
      ...ANCHOR,
      target: { kind: "craft", craft: "neighbour" },
      xPx: ANCHOR.xPx + 32,
    };
    render(
      <ViewMarkLabels
        anchors={[ANCHOR, neighbour]}
        devicePixelRatio={1}
        rows={[aRow(), aRow({ key: "craft:neighbour", name: "NEIGHBOUR · TEST HULL" })]}
      />,
    );
    const plate = screen.getByText("OTHER · TEST HULL", { exact: false });
    const rules = [stylesheetRule(".view-marks"), stylesheetRule(".view-marks__label")].join("\n");
    expect({
      over: neighbour.xPx > ANCHOR.xPx + ANCHOR.labelOffsetPx,
      plated: plate.classList.contains("view-marks__label"),
      paints: stylesheetRule(".view-marks__label").includes("background: var(--surface-0);"),
      surface: /^#[0-9a-f]{6}$/u.test(tokenOf("surface-0")),
      seeThrough: /opacity|filter|mix-blend-mode|rgba|hsla|transparent|color-mix/u.test(rules),
    }).toEqual({ over: true, plated: true, paints: true, surface: true, seeThrough: false });
  });

  it("holds its text and its stale readings at 6:1 against the plate", () => {
    const surface = tokenLuminance(tokenOf("surface-0"));
    expect(
      ["text", "text-muted"].map(
        (token) => contrastRatio(tokenLuminance(tokenOf(token)), surface) >= 6,
      ),
    ).toEqual([true, true]);
  });

  it("cuts to its place, with no transition or animation", () => {
    // An eased move would carry the plate over a reported destination's reticle for up to 150 ms.
    expect(/transition|animation/u.test(stylesheetRule(".view-marks__label"))).toBe(false);
  });
});
