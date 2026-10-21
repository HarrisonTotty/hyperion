import type { SurfaceHeaderDto } from "@hyperion/protocol";
import { render, type RenderResult, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import { FIXTURE_MOON } from "../../test/viewFixtures";
import type { FieldFailure, FieldStatus } from "./coarseFieldStore";
import { CoverageView } from "./coverage";
import {
  COVERAGE_FIELD_CH,
  datumLine,
  formatCoverage,
  NO_READOUT_UNITS,
  type ReadoutLine,
  readoutLineText,
  RESOLUTION_FIELD_CH,
  surveyLines,
  tallestTerrainLines,
} from "./readouts";
import type { TerrainSurvey } from "./session";
import { SurveyLine } from "./SurveyLine";

/** The Moon-like test header, a spheroid with no sea. */
const HEADER: SurfaceHeaderDto = moonLikeHeader(FIXTURE_MOON);

/** The test header on a world with a sea at -2.4 m, its main liquid the palette's second entry. */
const SEA_HEADER: SurfaceHeaderDto = {
  ...HEADER,
  equatorial_radius_m: 6_378_137,
  polar_radius_m: 6_356_752,
  sea_level_m: -2.4,
  main_liquid: 1,
};

/** A coverage of level 5 with `cells` surveyed at code 62, about 2 m, from cell 0, or none. */
function coverageOf(cells: number): CoverageView {
  const ranges = cells === 0 ? [] : [{ start: 0, end: cells, resolution: 62 }];
  const merged = CoverageView.empty(5).merged({ ranges }, 1);
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

/** A field's standing: wanted, answered, nothing failed. */
function standing(overrides: Partial<FieldStatus> = {}): FieldStatus {
  return { reason: "local-body", pending: false, notModelled: null, failure: null, ...overrides };
}

/** A survey of the test body: held, live, its first 77 cells surveyed at about 2 m by default. */
function aSurvey(overrides: Partial<TerrainSurvey> = {}): TerrainSurvey {
  return {
    status: standing(),
    coverage: coverageOf(77),
    header: HEADER,
    stale: false,
    ...overrides,
  };
}

/** The first of the survey's lines: the survey line, or its state in its place. */
function surveyLineOf(survey: TerrainSurvey | null): ReadoutLine {
  const [line] = surveyLines(survey, NO_READOUT_UNITS).lines;
  if (line === undefined) {
    throw new Error("the survey has no line");
  }
  return line;
}

/** `line` rendered as the lower plate sets it. */
function renderLine(line: ReadoutLine): RenderResult {
  return render(<SurveyLine line={line} />);
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

/** The line's parts: the definition after its one term. */
function parts(): HTMLElement {
  const definition = screen.getByRole("term").nextElementSibling;
  if (!(definition instanceof HTMLElement) || definition.tagName !== "DD") {
    throw new Error("the reading has no parts");
  }
  return definition;
}

/** A failure the server refused the field with, or failed it of its own. */
function rejected(refusal: boolean): FieldFailure {
  return { kind: "rejected", code: "internal", message: "no such body", refusal };
}

describe("the survey line on the lower plate (R10.T12; the guide's survey line)", () => {
  it("sets the survey line as a reading in a list of the label block's lines", () => {
    renderLine(surveyLineOf(aSurvey()));
    const term = screen.getByRole("term");
    expect([term.textContent, term.closest("dl")?.className]).toEqual([
      "SURVEY COVERAGE",
      "view-label__lines",
    ]);
  });

  it("joins its coverage and FINEST with the middle dot, each part in a span of its own", () => {
    renderLine(surveyLineOf(aSurvey()));
    const shown = [...parts().childNodes].map((node) =>
      node instanceof HTMLElement ? `${node.className}: ${visibleText(node)}` : node.textContent,
    );
    expect(shown).toEqual([
      `view-label__part: ${formatCoverage(coverageOf(77))}`,
      " · ",
      "view-label__part: FINEST 2.0 m",
    ]);
  });

  it("sets the coverage and FINEST in outputs of their fixed fields, not announced", () => {
    renderLine(surveyLineOf(aSurvey()));
    expect(
      screen
        .getAllByRole("status")
        .map((output) => `${output.style.minWidth} ${output.getAttribute("aria-live") ?? ""}`),
    ).toEqual([`${String(COVERAGE_FIELD_CH)}ch off`, `${String(RESOLUTION_FIELD_CH)}ch off`]);
  });

  it("keeps each value's place for its S, blank while live and holding it while stale", () => {
    const shown = [false, true].map((stale) => {
      const view = renderLine(surveyLineOf(aSurvey({ stale })));
      const marks = [...view.container.querySelectorAll(".terrain-mark")].map(visibleText);
      view.unmount();
      return marks;
    });
    expect(shown).toEqual([
      ["", ""],
      [" S", " S"],
    ]);
  });

  it("mutes a stale survey's values, each S read as the word stale", () => {
    renderLine(surveyLineOf(aSurvey({ stale: true })));
    expect({
      muted: screen.getAllByRole("status").map((output) => output.classList.contains("stale")),
      words: screen.getAllByText("stale").map((word) => word.className),
    }).toEqual({ muted: [true, true], words: ["visually-hidden", "visually-hidden"] });
  });

  it("reads NONE S, its one value stale, where no cell is surveyed", () => {
    const line = surveyLineOf(aSurvey({ coverage: coverageOf(0), stale: true }));
    renderLine(line);
    expect(`${screen.getByRole("term").textContent} ${visibleText(parts())}`).toBe(
      readoutLineText(line),
    );
  });

  it("states SURVEY COVERAGE: PENDING as a statement in --text in the line's place", () => {
    const { container } = renderLine(surveyLineOf(null));
    expect({
      lines: [...container.children].map((child) => `${child.className}: ${child.textContent}`),
      readings: screen.queryAllByRole("term"),
    }).toEqual({
      lines: ["view-label__statement: SURVEY COVERAGE: PENDING"],
      readings: [],
    });
  });

  it.each<readonly [string, FieldFailure, boolean]>([
    ["the server's refusal", rejected(true), false],
    ["the server's own failure", rejected(false), true],
    ["a check that failed", { kind: "invalid", check: "payload cut short" }, true],
  ])(
    "states %s as a request's status, in --status-caution only for a failure",
    (_, failure, caution) => {
      const line = surveyLineOf(
        aSurvey({ coverage: null, header: null, status: standing({ failure }) }),
      );
      renderLine(line);
      const status = screen.getByRole("status");
      expect([
        status.textContent,
        status.classList.contains("request-status__text"),
        status.classList.contains("request-status__text--fault"),
      ]).toEqual([readoutLineText(line), true, caution]);
    },
  );

  it.each([
    ["live", false],
    ["stale", true],
  ])("carries no id %s, since a copy of it stands in the hidden sizer", (_, stale) => {
    const { container } = renderLine(surveyLineOf(aSurvey({ stale })));
    expect(container.querySelectorAll("[id]")).toHaveLength(0);
  });
});

describe("the survey's lines' labels kept whole (R10.T12.a; decision-r10-t12a-scope §4.3)", () => {
  it.each<readonly [string, ReadoutLine]>([
    ["the survey line", surveyLineOf(aSurvey())],
    ["DATUM", datumLine(SEA_HEADER, true)],
    ["the tallest survey line", tallestTerrainLines(HEADER, 1).survey],
    ["the tallest DATUM", tallestTerrainLines(HEADER, 1).datum],
  ])("sets %s's line with the whole-label class", (_, line) => {
    renderLine(line);
    expect(screen.getByRole("term").parentElement?.className).toBe(
      "field view-label__line view-label__line--whole",
    );
  });
});

describe("the DATUM line on the lower plate (R10.T12; the guide's DATUM)", () => {
  it("sets DATUM as a reading in a list of the label block's lines", () => {
    renderLine(datumLine(SEA_HEADER, false));
    const term = screen.getByRole("term");
    expect([term.textContent, term.closest("dl")?.className]).toEqual([
      "DATUM",
      "view-label__lines",
    ]);
  });

  it("reads its figure and SEA LEVEL as the model writes them, each with its S while stale", () => {
    const line = datumLine(SEA_HEADER, true);
    renderLine(line);
    expect(`${screen.getByRole("term").textContent} ${visibleText(parts())}`).toBe(
      readoutLineText(line),
    );
  });

  it("sets each of its values at its own width, a constant of the body's", () => {
    renderLine(datumLine(SEA_HEADER, false));
    expect(
      screen.getAllByRole("status").map((output) => [output.textContent, output.style.minWidth]),
    ).toEqual([
      ["6378.1 km ×", ""],
      ["6356.8 km", ""],
      ["-2.4 m", ""],
    ]);
  });

  it("mutes its stale values, the console's words not", () => {
    renderLine(datumLine(SEA_HEADER, true));
    expect({
      muted: screen.getAllByRole("status").map((output) => output.classList.contains("stale")),
      words: within(parts())
        .getAllByText(/^(?:REFERENCE SPHEROID|SEA LEVEL)/u)
        .map((part) => part.closest(".stale")),
    }).toEqual({ muted: [true, true, true], words: [null, null] });
  });

  it("sets a spheroid's two radii as two runs, a space between them, its one S after the second (R10.T12.a)", () => {
    renderLine(datumLine(SEA_HEADER, true));
    const [figure] = parts().querySelectorAll(".view-label__part");
    const shown = [...(figure?.childNodes ?? [])].map((node) =>
      node instanceof HTMLElement ? `${node.className}: ${visibleText(node)}` : node.textContent,
    );
    expect(shown).toEqual([
      "REFERENCE SPHEROID ",
      "view-label__run: 6378.1 km ×",
      " ",
      "view-label__run: 6356.8 km S",
    ]);
  });

  it("sets a sphere's one radius as one run (R10.T12.a)", () => {
    const sphere = { ...HEADER, equatorial_radius_m: 1_737_400, polar_radius_m: 1_737_400 };
    renderLine(datumLine(sphere, false));
    expect(
      [...parts().querySelectorAll(".view-label__run")].map((run) => visibleText(run).trim()),
    ).toEqual(["1737.4 km"]);
  });

  it("carries no id in its tallest form, since that stands in the hidden sizer", () => {
    const { container } = renderLine(tallestTerrainLines(HEADER, 1).datum);
    expect(container.querySelectorAll("[id]")).toHaveLength(0);
  });
});
