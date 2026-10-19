import { cleanup, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import STYLES from "../../styles.css?raw";
import { statusLine } from "../../test/statusLine";
import { TERRAIN_NOT_AVAILABLE } from "../../view/terrain/annunciation";
import { ViewLabelBlock } from "./ViewLabelBlock";

/** The classes of the parts the status line reading `text` is set in. */
function partsOf(text: string): string[] {
  return [...screen.getByText(statusLine(text)).children].map((part) => part.className);
}

describe("the view's label block", () => {
  it("shows a terrain annunciation as a steady statement, not an alert or a fault", () => {
    render(
      <ViewLabelBlock
        lines={[{ label: "STYLE", value: "PHOTOREALISTIC" }]}
        statements={["TERRAIN: DETAIL LIMITED"]}
        countLine={null}
        faults={[]}
      />,
    );
    const line = screen.getByText(statusLine("TERRAIN: DETAIL LIMITED"));
    // The statement class carries no status colour: the plate's `--text`.
    expect(line.className).toBe("view-label__statement");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryAllByRole("status").some((s) => s.contains(line))).toBe(false);
  });

  it("tracks a statement upper case whole as one state, and splits one that gives a sentence", () => {
    render(
      <ViewLabelBlock
        lines={[{ label: "STYLE", value: "WIREFRAME" }]}
        statements={["TERRAIN: STREAMING", "TEST PLANET: provisional, dry and hand-parameterised"]}
        countLine={null}
        faults={[]}
      />,
    );
    expect([
      partsOf("TERRAIN: STREAMING"),
      partsOf("TEST PLANET: provisional, dry and hand-parameterised"),
    ]).toEqual([["status-text__state"], ["status-text__state", "status-text__sentence"]]);
  });

  it("sets each fault on a fault line of its own in --status-caution, the graphics fault first (R10.T16.b)", () => {
    render(
      <ViewLabelBlock
        lines={[{ label: "STYLE", value: "PHOTOREALISTIC" }]}
        statements={[]}
        countLine={null}
        faults={["GRAPHICS DEVICE LOST: re-creating", TERRAIN_NOT_AVAILABLE]}
      />,
    );
    const faults = screen
      .getAllByRole("status")
      .filter((output) => output.classList.contains("request-status__text--fault"));
    expect(faults.map((output) => output.textContent)).toEqual([
      "GRAPHICS DEVICE LOST: re-creating",
      "TERRAIN NOT AVAILABLE: height computation failed, reopen VIEW to retry",
    ]);
    // The fault line's class is set in the caution colour, the guide's for a failed system.
    expect(STYLES).toMatch(
      /\.request-status__text--fault,[^{]*\{\s*color: var\(--status-caution\);\s*\}/u,
    );
    // The terrain's state is tracked and its sentence after the colon is not (Typography).
    expect(partsOf(TERRAIN_NOT_AVAILABLE)).toEqual(["status-text__state", "status-text__sentence"]);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("sets no fault line with no fault", () => {
    render(
      <ViewLabelBlock
        lines={[{ label: "STYLE", value: "PHOTOREALISTIC" }]}
        statements={["TERRAIN: STREAMING"]}
        countLine={null}
        faults={[]}
      />,
    );
    expect(
      screen
        .getAllByRole("status")
        .filter((output) => output.classList.contains("request-status__text--fault")),
    ).toEqual([]);
  });
});

describe("the view's label block's line breaks (R07.T19.b)", () => {
  const LINES = [
    { label: "TIME", value: "UT +0 yr 000/00:00:01" },
    { label: "CAMERA", value: "FREE · RATE 1.00 km/s" },
    { label: "FOV", value: "60°" },
    { label: "EXPOSURE", value: "EV100 -1.0 INHIBITED · OPERATOR" },
    { label: "STARS", value: "V 9.5 mag CAM · CLUSTERS AND WHITE DWARFS: NOT YET MODELLED" },
  ];

  function block(): HTMLElement {
    const { container } = render(
      <ViewLabelBlock lines={LINES} statements={[]} countLine={null} faults={[]} />,
    );
    return container;
  }

  it("keeps each reading's text as it reads", () => {
    block();
    expect(screen.getAllByRole("status").map((output) => output.textContent)).toEqual(
      LINES.map((line) => line.value),
    );
  });

  it("sets each part between middle dots on a line of its own where it fits", () => {
    const parts = [...block().querySelectorAll(".view-label__part")].map(
      (part) => part.textContent,
    );
    expect(parts).toEqual([
      "UT +0 yr 000/00:00:01",
      "FREE",
      "RATE 1.00 km/s",
      "60°",
      "EV100 -1.0 INHIBITED",
      "OPERATOR",
      "V 9.5 mag CAM",
      "CLUSTERS AND WHITE DWARFS: NOT YET MODELLED",
    ]);
  });

  it("never breaks inside a quantity, a star limit and its kind, after UT or in a clock", () => {
    const runs = [...block().querySelectorAll(".view-label__run")].map((run) => run.textContent);
    expect(runs).toEqual(["UT +0 yr", "000/00:00:01", "1.00 km/s", "EV100 -1.0", "V 9.5 mag CAM"]);
  });

  it("never breaks a kept scene's clock rate between its number and its unit, s/s", () => {
    const { container } = render(
      <ViewLabelBlock
        lines={[{ label: "SCENE CLOCK", value: "86,400 s/s" }]}
        statements={[]}
        countLine={null}
        faults={[]}
      />,
    );
    const runs = [...container.querySelectorAll(".view-label__run")].map((run) => run.textContent);
    expect(runs).toEqual(["86,400 s/s"]);
  });
});

describe("the view's label block's field (R06.T11.f)", () => {
  const VALUE = "V 10.0 mag CAM · BEYOND 2000 ly: STREAMING";

  /** The block with the sky's edge in its field, and the field's element. */
  function fieldOf(stale: boolean): HTMLElement {
    const { container } = render(
      <ViewLabelBlock
        lines={[
          { label: "STARS", value: VALUE, fields: [{ text: "2000 ly", widthCh: 12, stale }] },
        ]}
        statements={[]}
        countLine={null}
        faults={[]}
      />,
    );
    const field = container.querySelector<HTMLElement>(".view-label__field");
    if (field === null) {
      throw new Error("the block sets no field");
    }
    return field;
  }

  it("sets the edge as a run in its fixed width, the reading's text unchanged", () => {
    const field = fieldOf(false);
    expect([
      field.textContent,
      field.classList.contains("view-label__run"),
      field.style.minWidth,
      screen.getByRole("status").textContent,
    ]).toEqual(["2000 ly", true, "12ch", VALUE]);
  });

  it("mutes the edge alone while it is stale, its S inside the field", () => {
    const field = fieldOf(true);
    expect([
      field.querySelector(".stale")?.textContent,
      field.querySelector(".stale-mark")?.textContent,
      screen.getByRole("status").classList.contains("stale"),
    ]).toEqual(["2000 ly", "S", false]);
  });
});

/** A block's first field, the limit's. */
function limitOf(block: HTMLElement): HTMLElement {
  const field = block.querySelector<HTMLElement>(".view-label__field");
  if (field === null) {
    throw new Error("the block sets no field");
  }
  return field;
}

/** A block's parts, as the count of runs in each, and each field's width. */
function shapeOf(block: HTMLElement): ReadonlyArray<ReadonlyArray<number | string>> {
  return [
    [...block.querySelectorAll(".view-label__part")].map(
      (part) => part.querySelectorAll(".view-label__run").length,
    ),
    [...block.querySelectorAll<HTMLElement>(".view-label__field")].map(
      (field) => field.style.minWidth,
    ),
  ];
}

describe("the view's label block's limit field (R06.T13.j)", () => {
  const VALUE = "V 9.5 mag CAM · BEYOND 2000 ly: STREAMING";

  /** The block with the sky's limit and edge in their fields, live or stale together. */
  function blockOf(stale: boolean): HTMLElement {
    const { container } = render(
      <ViewLabelBlock
        lines={[
          {
            label: "STARS",
            value: VALUE,
            fields: [
              { text: "V 9.5 mag", widthCh: 13, stale },
              { text: "2000 ly", widthCh: 12, stale },
            ],
          },
        ]}
        statements={[]}
        countLine={null}
        faults={[]}
      />,
    );
    return container;
  }

  it("sets the limit in its fixed width at the head of its run, its kind after it", () => {
    const field = limitOf(blockOf(false));
    expect([
      field.textContent,
      field.style.minWidth,
      field.classList.contains("view-label__run"),
      field.parentElement?.classList.contains("view-label__run"),
      field.parentElement?.textContent,
      screen.getByRole("status").textContent,
    ]).toEqual(["V 9.5 mag", "13ch", false, true, "V 9.5 mag CAM", VALUE]);
  });

  it("mutes the limit while it is stale, its S after its unit and before its kind", () => {
    const field = limitOf(blockOf(true));
    expect([
      field.querySelector(".stale")?.textContent,
      field.querySelector(".stale-mark")?.textContent,
      field.nextSibling?.textContent,
      screen.getByRole("status").textContent,
      screen.getByRole("status").classList.contains("stale"),
    ]).toEqual([
      "V 9.5 mag",
      "S",
      " CAM",
      "V 9.5 mag Sstale CAM · BEYOND 2000 ly Sstale: STREAMING",
      false,
    ]);
  });

  it("keeps the line's parts, its runs and its fields' widths when it goes stale", () => {
    const live = shapeOf(blockOf(false));
    cleanup();
    expect(shapeOf(blockOf(true))).toEqual(live);
  });
});

/** The text standing directly in the reading shown, between its parts. */
function textBetweenParts(): ReadonlyArray<string | null> {
  return [...screen.getByRole("status").childNodes]
    .filter((node) => node.nodeType === Node.TEXT_NODE)
    .map((node) => node.textContent);
}

describe("the sky's STARS reading's leading dots (R06.T13.j)", () => {
  const VALUE = "V 9.5 mag CAM · BEYOND 2000 ly: STREAMING · CLUSTERS: NOT YET MODELLED";
  const FIELDS = [
    { text: "V 9.5 mag", widthCh: 13, stale: false },
    { text: "2000 ly", widthCh: 12, stale: false },
  ];

  /** The block with one `STARS` line of `value`, its dots leading or not. */
  function linesOf(value: string, dotsLead: boolean): HTMLElement {
    const { container } = render(
      <ViewLabelBlock
        lines={[
          dotsLead
            ? { label: "STARS", value, fields: FIELDS, dotsLead: true }
            : { label: "STARS", value, fields: FIELDS },
        ]}
        statements={[]}
        countLine={null}
        faults={[]}
      />,
    );
    return container;
  }

  it("sets every dot at the head of the part after it, none between the parts, the text unchanged", () => {
    const block = linesOf(VALUE, true);
    expect([
      [...block.querySelectorAll(".view-label__part")].map((part) => part.textContent),
      textBetweenParts(),
      screen.getByRole("status").textContent,
    ]).toEqual([
      ["V 9.5 mag CAM", "· BEYOND 2000 ly: STREAMING", "· CLUSTERS: NOT YET MODELLED"],
      [" ", " "],
      VALUE,
    ]);
  });

  it("keeps each dot on one line with its part's first word, and the limit with its kind", () => {
    const block = linesOf(VALUE, true);
    const parts = [...block.querySelectorAll(".view-label__part")];
    expect([
      parts.slice(1).map((part) => part.firstElementChild?.className),
      parts.slice(1).map((part) => part.firstElementChild?.textContent),
      parts[0]?.querySelector(".view-label__run")?.textContent,
    ]).toEqual([
      ["view-label__run", "view-label__run"],
      ["· BEYOND", "· CLUSTERS:"],
      "V 9.5 mag CAM",
    ]);
  });

  it("keeps a dot with a part's first run where one heads it", () => {
    linesOf("V 9.5 mag CAM · 2000 ly AWAY", true);
    const field = screen.getByText("2000 ly");
    expect([field.parentElement?.textContent, field.parentElement?.className]).toEqual([
      "· 2000 ly",
      "view-label__run",
    ]);
  });

  it("keeps every other reading's dots between its parts", () => {
    linesOf(VALUE, false);
    expect(textBetweenParts()).toEqual([" · ", " · "]);
  });

  it("sets each field once, in the reading's order", () => {
    const { container } = render(
      <ViewLabelBlock
        lines={[
          {
            label: "STARS",
            value: "2000 ly · 2000 ly",
            fields: [{ text: "2000 ly", widthCh: 12, stale: true }],
          },
        ]}
        statements={[]}
        countLine={null}
        faults={[]}
      />,
    );
    expect(
      [...container.querySelectorAll(".view-label__part")].map(
        (part) => part.querySelector(".view-label__field") !== null,
      ),
    ).toEqual([true, false]);
  });
});
