import type { PaletteEntryDto, SurfaceHeaderDto } from "@hyperion/protocol";
import { render, type RenderResult, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import { stylesheetRule } from "../../test/stylesheet";
import { FIXTURE_MOON } from "../../test/viewFixtures";
import {
  ELEVATION_FIELD_CH,
  ELEVATION_UNCERTAINTY_CH,
  groundReadoutLines,
  NO_BREAK_SPACE,
  NO_READOUT_UNITS,
  type ReadoutContext,
  type ReadoutField,
  type ReadoutLine,
  readoutLineText,
  STALE_MARK_CH,
  tallestTerrainLines,
} from "./readouts";
import { TerrainReadout } from "./TerrainReadout";
import type {
  SurfaceGroundProperties,
  SurfaceGroundReadout,
  SurfaceReading,
  SurfaceSlantRange,
} from "./workers/messages";

/** `±` with its no-break spaces, as every Estimated reading writes it. */
const PM = `${NO_BREAK_SPACE}±${NO_BREAK_SPACE}`;

/** Radians from degrees. */
function rad(deg: number): number {
  return (deg * Math.PI) / 180;
}

/** A palette entry of `substance` as `role`, its photometry stand-ins, since no test reads them. */
function entry(substance: string, role: PaletteEntryDto["role"]): PaletteEntryDto {
  return { substance, role, normal_albedo_bvr: [0.1, 0.12, 0.14], phase_row: 1, liquid: null };
}

/** The Moon-like test header with a palette of two: an Fe-rich basalt crust and water ice. */
const HEADER: SurfaceHeaderDto = {
  ...moonLikeHeader(FIXTURE_MOON),
  palette: [entry("basalt_fe", "primary_crust"), entry("H2O", "ice")],
};

/** What {@link groundAt} may vary of the guide's Moon-like example. */
interface GroundOptions {
  readonly reading?: Partial<SurfaceReading>;
  readonly sight?: Partial<SurfaceSlantRange>;
  readonly entry?: number | null;
  readonly properties?: Partial<SurfaceGroundProperties>;
}

/**
 * A readout where the line of sight meets surveyed regolith of the Fe-rich basalt: the guide's
 * Moon-like example, Estimated under a 410 m survey, `~41.2 km ± 0.3 km` away, by default.
 */
function groundAt(options: GroundOptions = {}): SurfaceGroundReadout {
  return {
    kind: "ground",
    sight: {
      rangeM: 41_234,
      sigmaM: 290,
      resolved: false,
      hitM: [1_738_000, 0, 0],
      at: { face: 0, u: 0, v: 0 },
      ...options.sight,
    },
    reading: {
      elevationM: 2_143.7,
      elevationSigmaM: 180,
      slopeRmsRad: rad(12),
      slopeSigmaRad: rad(4),
      slopeBaselineM: 2,
      resolutionM: 410,
      resolved: false,
      ...options.reading,
    },
    material: { classCode: 1, entry: options.entry === undefined ? 0 : options.entry },
    properties: {
      bearingCapacityPa: 3_432_000,
      capacitySlopeRad: rad(12),
      padFriction: 0.2,
      frictionOnTemperature: false,
      temperatureK: 250,
      ...options.properties,
    },
  };
}

/** The readout's lines for `readout`: live, from the hull, under the reference pad, by default. */
function linesOf(
  readout: SurfaceGroundReadout,
  overrides: Partial<ReadoutContext> = {},
): ReadonlyArray<ReadoutLine> {
  const context: ReadoutContext = {
    stale: false,
    slantFromCamera: false,
    padWidthM: 1,
    ...overrides,
  };
  return groundReadoutLines(readout, HEADER, context, NO_READOUT_UNITS).lines;
}

/** The readout of `readout` rendered as the lower plate sets it. */
function renderReadout(
  readout: SurfaceGroundReadout,
  overrides: Partial<ReadoutContext> = {},
): RenderResult {
  return render(<TerrainReadout lines={linesOf(readout, overrides)} />);
}

/** The parts of the reading labelled `label` in `container`: the definition after its term. */
function partsIn(container: HTMLElement, label: string): HTMLElement {
  const parts = within(container).getByText(label, { selector: "dt" }).nextElementSibling;
  if (!(parts instanceof HTMLElement) || parts.tagName !== "DD") {
    throw new Error(`the reading ${label} has no parts`);
  }
  return parts;
}

/** The parts of the reading labelled `label` on the page. */
function partsOf(label: string): HTMLElement {
  return partsIn(document.body, label);
}

/** The place kept for an `S` in `parts`, which must hold one. */
function markIn(parts: HTMLElement): HTMLElement {
  const mark = parts.querySelector(".terrain-mark");
  if (!(mark instanceof HTMLElement)) {
    throw new Error("the parts keep no place for an S");
  }
  return mark;
}

/** The fields of `lines`' readings, in the order they are set. */
function fieldsOf(lines: ReadonlyArray<ReadoutLine>): ReadoutField[] {
  return lines.flatMap((line) =>
    line.kind === "reading"
      ? line.segments.flat().filter((piece): piece is ReadoutField => piece.kind === "field")
      : [],
  );
}

/** How many places for an `S` `lines`' readings keep. */
function marksOf(lines: ReadonlyArray<ReadoutLine>): number {
  return lines
    .flatMap((line) => (line.kind === "reading" ? line.segments.flat() : []))
    .filter((piece) => piece.kind === "mark").length;
}

/** The text `node` shows, the words read to assistive technology alone left out. */
function visibleText(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) {
    return node.textContent ?? "";
  }
  if (node instanceof HTMLElement && node.classList.contains("visually-hidden")) {
    return "";
  }
  return [...node.childNodes].map(visibleText).join("");
}

/** An output's least width, ch, from its inline style; 0 where it sets none. */
function minWidthCh(output: HTMLElement): number {
  if (output.style.minWidth === "") {
    return 0;
  }
  const ch = /^(\d+(?:\.\d+)?)ch$/u.exec(output.style.minWidth)?.[1];
  if (ch === undefined) {
    throw new Error(`a field's width is ${output.style.minWidth}, not in ch`);
  }
  return Number(ch);
}

/**
 * The set width of `node`, ch of B612 Mono, as the stylesheet lays it out: a field (an `output`,
 * or a place kept blank) its least width or, where longer, the text it shows; a place kept for an
 * `S` 2 ch; any other text its length; the words read to assistive technology alone none.
 */
function renderedWidthCh(node: Node): number {
  if (node.nodeType === Node.TEXT_NODE) {
    return node.textContent?.length ?? 0;
  }
  if (!(node instanceof HTMLElement) || node.classList.contains("visually-hidden")) {
    return 0;
  }
  if (node.classList.contains("terrain-mark")) {
    return 2;
  }
  const innerCh = [...node.childNodes].reduce((sum, child) => sum + renderedWidthCh(child), 0);
  return node.classList.contains("terrain-field") ? Math.max(minWidthCh(node), innerCh) : innerCh;
}

describe("the terrain readout on the lower plate (R10.T12)", () => {
  it("sets its heading GROUND AT LINE OF SIGHT as the label block's heading", () => {
    renderReadout(groundAt());
    const heading = screen.getByRole("heading", { name: "GROUND AT LINE OF SIGHT" });
    expect(heading.className).toBe("view-label__heading");
  });

  it("sets its seven readings in the guide's order as one list of the label block's lines", () => {
    renderReadout(groundAt());
    const terms = screen.getAllByRole("term");
    expect({
      labels: terms.map((term) => term.textContent),
      lists: [...new Set(terms.map((term) => term.closest("dl")))].map((list) => list?.className),
    }).toEqual({
      labels: [
        "ELEVATION",
        "SLOPE",
        "SLANT RANGE",
        "MATERIAL",
        "ULTIMATE BEARING CAPACITY",
        "PAD FRICTION",
        "RESOLUTION",
      ],
      lists: ["view-label__lines"],
    });
  });

  it("reads each reading as the model writes it", () => {
    const lines = linesOf(groundAt({ sight: { sigmaM: null } }), { stale: true });
    render(<TerrainReadout lines={lines} />);
    const shown = screen
      .getAllByRole("term")
      .map((term) => `${term.textContent} ${visibleText(partsOf(term.textContent))}`);
    expect(shown).toEqual(lines.filter((line) => line.kind === "reading").map(readoutLineText));
  });

  it("states each value in an output that is not announced as it changes", () => {
    const lines = linesOf(groundAt());
    render(<TerrainReadout lines={lines} />);
    const outputs = screen.getAllByRole("status");
    expect(
      outputs.map((output) => `${output.tagName} ${output.getAttribute("aria-live") ?? ""}`),
    ).toEqual(fieldsOf(lines).map(() => "OUTPUT off"));
  });

  it("sets each value at least as wide as its fixed field, in ch, and a name at its own width", () => {
    const lines = linesOf(groundAt(), { stale: true });
    render(<TerrainReadout lines={lines} />);
    expect(screen.getAllByRole("status").map((output) => output.style.minWidth)).toEqual(
      fieldsOf(lines).map((field) => (field.widthCh > 0 ? `${String(field.widthCh)}ch` : "")),
    );
  });

  it("holds an Estimated ELEVATION's value and its uncertainty with its S in their fields' widths", () => {
    renderReadout(groundAt());
    expect(within(partsOf("ELEVATION")).getAllByRole("status").map(minWidthCh)).toEqual([
      ELEVATION_FIELD_CH,
      ELEVATION_UNCERTAINTY_CH + STALE_MARK_CH,
    ]);
  });

  it("sets a value right-aligned in its field and an uncertainty left-aligned after its ±", () => {
    renderReadout(groundAt());
    expect(
      within(partsOf("ELEVATION"))
        .getAllByRole("status")
        .map((output) => output.className),
    ).toEqual(["terrain-field", "terrain-field terrain-field--start"]);
  });

  it("aligns a field to its right, an uncertainty to its left, never broken, and an S's place 2 ch wide", () => {
    const field = stylesheetRule(".terrain-field");
    expect([
      /text-align: right;/u.test(field),
      /white-space: nowrap;/u.test(field),
      /display: inline-block;/u.test(field),
      /text-align: left;/u.test(stylesheetRule(".terrain-field--start")),
      /width: 2ch;/u.test(stylesheetRule(".terrain-mark")),
    ]).toEqual([true, true, true, true, true]);
  });

  it("mutes a stale value and keeps its S in the place after it, read as the word stale", () => {
    renderReadout(groundAt(), { stale: true });
    const parts = partsOf("RESOLUTION");
    const mark = markIn(parts);
    expect([
      within(parts).getByRole("status").classList.contains("stale"),
      within(mark).getByText("S").className,
      within(mark).getByText("stale").className,
    ]).toEqual([true, "stale-mark", "visually-hidden"]);
  });

  it("sets a stale uncertainty's S inside its field, straight after it, for the whole reading", () => {
    renderReadout(groundAt(), { stale: true });
    const [value, uncertainty] = within(partsOf("ELEVATION")).getAllByRole("status");
    expect([
      value?.className,
      uncertainty?.className,
      uncertainty === undefined ? null : visibleText(uncertainty),
      uncertainty === undefined ? null : within(uncertainty).getByText("stale").className,
    ]).toEqual([
      "terrain-field stale",
      "terrain-field terrain-field--start stale",
      `${PM}180 m S`,
      "visually-hidden",
    ]);
  });

  it("keeps a value's place for its S whether it is stale or not, blank while live", () => {
    const shown = [false, true].map((stale) => {
      const view = renderReadout(groundAt(), { stale });
      const marks = [...partsIn(view.container, "RESOLUTION").querySelectorAll(".terrain-mark")];
      view.unmount();
      return marks.map((mark) => visibleText(mark));
    });
    expect(shown).toEqual([[""], [" S"]]);
  });

  it.each([false, true])("keeps every place for an S the model keeps, stale %s", (stale) => {
    const lines = linesOf(groundAt({ reading: { resolved: true } }), { stale });
    const { container } = render(<TerrainReadout lines={lines} />);
    expect(container.querySelectorAll(".terrain-mark")).toHaveLength(marksOf(lines));
  });

  it("mutes the em dash as Missing, with no S", () => {
    renderReadout(groundAt({ entry: null, properties: { bearingCapacityPa: null } }), {
      stale: true,
    });
    const dashes = screen.getAllByText("—");
    expect({
      dashes: dashes.map((dash) => [dash.tagName, dash.className, visibleText(dash)]),
      capacityMark: visibleText(markIn(partsOf("ULTIMATE BEARING CAPACITY"))),
    }).toEqual({
      dashes: [
        ["OUTPUT", "terrain-field readout__missing", "—"],
        ["OUTPUT", "terrain-field readout__missing", "—"],
      ],
      capacityMark: "",
    });
  });

  it("joins a reading's parts with the middle dot, each part in a span of its own", () => {
    renderReadout(groundAt());
    const parts = [...partsOf("SLOPE").childNodes].map((node) =>
      node instanceof HTMLElement ? `${node.className}: ${visibleText(node)}` : node.textContent,
    );
    expect(parts).toEqual([
      `view-label__part: ~12°${PM}4°`,
      " · ",
      "view-label__part: BASELINE 2 m",
    ]);
  });

  it("sets GRAZING as the console's own word in its part, neither a value nor muted", () => {
    renderReadout(groundAt({ sight: { sigmaM: null } }), { stale: true });
    const grazing = within(partsOf("SLANT RANGE")).getByText("GRAZING");
    expect([grazing.className, grazing.closest("output, .stale")]).toEqual([
      "view-label__part",
      null,
    ]);
  });

  it.each<readonly [string, SurfaceGroundReadout]>([
    ["NOT SURVEYED", { kind: "not-surveyed" }],
    ["NO GROUND", { kind: "no-ground" }],
  ])("states %s after the heading in place of the readings", (text, readout) => {
    const { container } = renderReadout(readout, { stale: true });
    expect({
      lines: [...container.children].map((child) => `${child.className}: ${child.textContent}`),
      readings: screen.queryAllByRole("term"),
      values: screen.queryAllByRole("status"),
    }).toEqual({
      lines: ["view-label__heading: GROUND AT LINE OF SIGHT", `view-label__statement: ${text}`],
      readings: [],
      values: [],
    });
  });

  it.each<readonly [string, ReadonlyArray<ReadoutLine>]>([
    ["Estimated and stale", linesOf(groundAt({ entry: null }), { stale: true })],
    ["at NOT SURVEYED", linesOf({ kind: "not-surveyed" })],
    ["in its tallest form", tallestTerrainLines(HEADER, 1).readout],
  ])("carries no id %s, since a copy of it stands in the hidden sizer", (_, lines) => {
    const { container } = render(<TerrainReadout lines={lines} />);
    expect(container.querySelectorAll("[id]")).toHaveLength(0);
  });
});

describe("the terrain readout's widths (R10.T12; the guide's Numbers)", () => {
  /** The friction where it depends on the ground's temperature, so that its AT is set. */
  const ON_TEMPERATURE: Partial<SurfaceGroundProperties> = {
    padFriction: 0.08,
    frictionOnTemperature: true,
    temperatureK: 243,
  };

  /** The readout in each state a reading's width must hold through. */
  const FORMS: ReadonlyArray<readonly [SurfaceGroundReadout, Partial<ReadoutContext>]> = [
    [groundAt({ properties: ON_TEMPERATURE }), {}],
    [groundAt({ properties: ON_TEMPERATURE }), { stale: true }],
    [
      groundAt({
        reading: { resolved: true, elevationM: -410, slopeRmsRad: rad(7.2), resolutionM: 9_700 },
        sight: { resolved: true, sigmaM: 0, rangeM: 412.3 },
        properties: { ...ON_TEMPERATURE, bearingCapacityPa: 5_368_600 },
      }),
      {},
    ],
    [
      groundAt({
        reading: { resolved: true, elevationM: 12_480 },
        sight: { resolved: true, sigmaM: 0 },
        properties: ON_TEMPERATURE,
      }),
      { stale: true },
    ],
  ];

  it.each([
    "ELEVATION",
    "SLOPE",
    "SLANT RANGE",
    "MATERIAL",
    "ULTIMATE BEARING CAPACITY",
    "PAD FRICTION",
    "RESOLUTION",
  ])("keeps %s's set width from Estimated to resolved and stale", (label) => {
    // Each form rendered alone, so that the reading's label finds its one line.
    const widths = FORMS.map(([readout, overrides]) => {
      const view = renderReadout(readout, overrides);
      const widthCh = renderedWidthCh(partsIn(view.container, label));
      view.unmount();
      return widthCh;
    });
    expect(widths).toEqual(FORMS.map(() => widths[0]));
  });

  it("sets ELEVATION as wide as its value's field and its uncertainty's with the S's place", () => {
    renderReadout(groundAt(), { stale: true });
    expect(renderedWidthCh(partsOf("ELEVATION"))).toBe(
      ELEVATION_FIELD_CH + ELEVATION_UNCERTAINTY_CH + STALE_MARK_CH,
    );
  });
});

describe("the terrain readout's breaks (R10.T12; the guide's ± row)", () => {
  it("sets an Estimated value, its uncertainty and its S as one run that never breaks", () => {
    renderReadout(groundAt(), { stale: true });
    const runs = partsOf("ELEVATION").querySelectorAll(".view-label__run");
    expect({
      runs: runs.length,
      fields: runs[0]?.querySelectorAll(".terrain-field").length,
      stale: runs[0]?.textContent.endsWith("Sstale"),
    }).toEqual({ runs: 1, fields: 2, stale: true });
  });

  it("keeps the console's words outside a value's run, breakable at their spaces", () => {
    renderReadout(groundAt());
    const parts = partsOf("SLOPE");
    const runs = [...parts.querySelectorAll(".view-label__run")].map((run) => run.textContent);
    expect([runs.some((text) => text.includes("BASELINE")), parts.textContent]).toEqual([
      false,
      expect.stringContaining("BASELINE 2 m"),
    ]);
  });

  it("keeps a resolved reading's blank uncertainty out of what is read, its place kept", () => {
    renderReadout(groundAt({ reading: { resolved: true }, sight: { resolved: true, sigmaM: 0 } }));
    const parts = partsOf("ELEVATION");
    const blank = parts.querySelector("[aria-hidden='true'].terrain-field");
    expect([
      within(parts).getAllByRole("status").length,
      blank?.textContent,
      blank instanceof HTMLElement ? blank.style.minWidth : null,
    ]).toEqual([1, "", "12ch"]);
  });
});
