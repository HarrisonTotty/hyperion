import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { contrastRatio, tokenLuminance } from "../../smoke/strokeContrast";
import { stylesheetRule } from "../../test/stylesheet";
import type { DrawAnchor } from "../../view/wireframe/drawList";
import { type MarkRow, targetKey } from "./viewRun";
import {
  markLabelPlaces,
  markLabelTransform,
  type PlateSizePx,
  ViewMarkLabels,
} from "./ViewMarkLabels";

const ANCHOR: DrawAnchor = {
  target: { kind: "craft", craft: "other" },
  xPx: 200,
  yPx: 100,
  distanceM: 5_000,
  label: { kind: "target", rangeM: 5_000, closureMPerS: null },
  labelOffsetPx: 24,
  labelRisePx: null,
  markReachPx: 12,
};

/** {@link ANCHOR} as the destination: its label stands 20 device px above or below its chevrons. */
const DESTINATION: DrawAnchor = { ...ANCHOR, labelRisePx: 20 };

/** The destination's label's key. */
const KEY = targetKey(DESTINATION.target);

/** Another craft's anchor at a place, its label shown or not, its reach as {@link ANCHOR}'s. */
function aCraftAt(craft: string, xPx: number, yPx: number, labelled = true): DrawAnchor {
  return {
    target: { kind: "craft", craft },
    xPx,
    yPx,
    distanceM: 5_000,
    label: labelled ? { kind: "target", rangeM: 5_000, closureMPerS: null } : null,
    labelOffsetPx: 24,
    labelRisePx: null,
    markReachPx: 12,
  };
}

/** Another craft, well clear of the destination. */
const NEIGHBOUR = aCraftAt("neighbour", 600, 400);

/** A stage of 800 × 600 CSS px at a ratio of 1 and a rem of 16 px: 0.5 rem is 8 px. */
const STAGE = { widthPx: 800, heightPx: 600, devicePixelRatio: 1, remPx: 16 } as const;

/** Each anchor's plate laid out 100 × 18 CSS px, as jsdom, which lays nothing out, cannot. */
function plates(...anchors: ReadonlyArray<DrawAnchor>): ReadonlyMap<string, PlateSizePx> {
  return new Map(
    anchors.map((anchor) => [targetKey(anchor.target), { widthPx: 100, heightPx: 18 }]),
  );
}

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

  it("mounts a destination's label hidden until the drawing loop places it", () => {
    render(<ViewMarkLabels anchors={[DESTINATION]} devicePixelRatio={1} rows={[aRow()]} />);
    const label = screen.getByText("OTHER · TEST HULL", { exact: false });
    expect([label.style.visibility, label.style.transform]).toEqual([
      "hidden",
      "translate(224px, calc(80px - 50%))",
    ]);
  });
});

describe("a label's place beside its mark (R07.T16.g)", () => {
  it("stands its plate the draw list's offset right of its mark, in CSS px", () => {
    expect([markLabelTransform(ANCHOR, 1, "line"), markLabelTransform(ANCHOR, 2, "line")]).toEqual([
      "translate(224px, 100px)",
      "translate(112px, 50px)",
    ]);
  });

  it("centres its plate on its mark's line by the plate's own translate", () => {
    expect(stylesheetRule(".view-marks__label")).toContain("translate: 0 -50%;");
  });

  it("pads its plate 0.25 rem to the left and right alone, its text's clearance from the bracket", () => {
    // The draw list's tests read the text 0.25 rem inside the plate's near edge, and its bottom
    // edge as the text's (R07.T16.h's follow-up).
    expect(stylesheetRule(".view-marks__label")).toContain("padding: 0 0.25rem;");
  });
});

// The plate's own `translate: 0 -50%` and the transform's −50% lift it by its height, so that its
// bottom edge stands at the transform's y; the transform's +50% undoes the plate's, so that its top
// edge does; and the transform's −100% moves a left-hand plate back by its width, so that its right
// edge stands at the transform's x.
describe("a destination's label above its chevron set (R07.T16.h's follow-up; decision-r07-quality-and-destination, addenda A and B)", () => {
  it.each([
    ["upper-right", "translate(224px, calc(80px - 50%))", "translate(112px, calc(40px - 50%))"],
    [
      "upper-left",
      "translate(calc(176px - 100%), calc(80px - 50%))",
      "translate(calc(88px - 100%), calc(40px - 50%))",
    ],
    ["lower-right", "translate(224px, calc(120px + 50%))", "translate(112px, calc(60px + 50%))"],
    [
      "lower-left",
      "translate(calc(176px - 100%), calc(120px + 50%))",
      "translate(calc(88px - 100%), calc(60px + 50%))",
    ],
  ] as const)(
    "stands its plate at the %s, its near edges the draw list's offset and rise from its mark",
    (place, atOne, atTwo) => {
      expect([
        markLabelTransform(DESTINATION, 1, place),
        markLabelTransform(DESTINATION, 2, place),
      ]).toEqual([atOne, atTwo]);
    },
  );

  it("stands any other mark's label on its line, whatever place is asked", () => {
    expect(markLabelTransform(ANCHOR, 1, "upper-left")).toBe("translate(224px, 100px)");
  });

  it("takes the upper right where it is clear", () => {
    const places = markLabelPlaces([DESTINATION, NEIGHBOUR], plates(DESTINATION, NEIGHBOUR), STAGE);
    expect(places.get(KEY)).toBe("upper-right");
  });

  it("leaves every other label on its line", () => {
    const places = markLabelPlaces([DESTINATION, NEIGHBOUR], plates(DESTINATION, NEIGHBOUR), STAGE);
    expect(places.get(targetKey(NEIGHBOUR.target))).toBe("line");
  });

  it("goes to the upper left where a neighbour's mark stands within 0.5 rem of the upper right", () => {
    // Its square reaches 62 px down, the upper-right plate's top; the upper left is 112 px off.
    const near = { ...NEIGHBOUR, xPx: 300, yPx: 50, label: null };
    expect(markLabelPlaces([DESTINATION, near], plates(DESTINATION), STAGE).get(KEY)).toBe(
      "upper-left",
    );
  });

  it("goes to the upper left where a neighbour's label stands within 0.5 rem of the upper right", () => {
    // Its plate on its line, 216 to 316 px across and 41 to 59 px down, 3 px above the upper right.
    const near = { ...NEIGHBOUR, xPx: 192, yPx: 50, markReachPx: 0 };
    expect(markLabelPlaces([DESTINATION, near], plates(DESTINATION, near), STAGE).get(KEY)).toBe(
      "upper-left",
    );
  });

  it("stays at the upper right where every place is blocked", () => {
    const around = [
      [274, 50],
      [126, 50],
      [274, 150],
      [126, 150],
    ].map(([xPx = 0, yPx = 0], i) => aCraftAt(`around-${String(i)}`, xPx, yPx, false));
    expect(markLabelPlaces([DESTINATION, ...around], plates(DESTINATION), STAGE).get(KEY)).toBe(
      "upper-right",
    );
  });

  it("takes the upper left at the stage's right edge", () => {
    const right = { ...DESTINATION, xPx: 750 };
    expect(markLabelPlaces([right], plates(right), STAGE).get(KEY)).toBe("upper-left");
  });

  it("takes the lower right at the stage's top edge", () => {
    const top = { ...DESTINATION, yPx: 30 };
    expect(markLabelPlaces([top], plates(top), STAGE).get(KEY)).toBe("lower-right");
  });

  it("goes to the lower right where the chrome stands within 0.5 rem of both upper places", () => {
    // A label block down to 60 px, 2 px above the upper places' plates.
    const block = { leftPx: 0, topPx: 0, widthPx: 800, heightPx: 60 };
    expect(markLabelPlaces([DESTINATION], plates(DESTINATION), STAGE, [block]).get(KEY)).toBe(
      "lower-right",
    );
  });

  it("takes the upper right until its plate is laid out", () => {
    expect(markLabelPlaces([DESTINATION], new Map(), STAGE).get(KEY)).toBe("upper-right");
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
