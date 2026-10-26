import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import STYLES from "../../styles.css?raw";
import { statusLine } from "../../test/statusLine";
import { TERRAIN_NOT_AVAILABLE } from "../../view/terrain/annunciation";
import { ViewLabelBlock, type ViewLabelBlockProps } from "./ViewLabelBlock";
import type { UpperRoom } from "./viewRoom";

/** The classes of the parts the status line reading `text` is set in. */
function partsOf(text: string): string[] {
  return [...screen.getByText(statusLine(text)).children].map((part) => part.className);
}

/** The label block with a graphics fault and the terrain's. */
function renderFaults(): void {
  render(
    <ViewLabelBlock
      lines={[{ label: "STYLE", value: "PHOTOREALISTIC" }]}
      statements={[]}
      countLine={null}
      faults={["GRAPHICS DEVICE LOST: re-creating", TERRAIN_NOT_AVAILABLE]}
    />,
  );
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

  it("sets ATMOSPHERE REJECTED among the statements as every request's REJECTED (R08.T10.b)", () => {
    const failed = "ATMOSPHERE REJECTED: for TEST GIANT, internal error, reopen VIEW to retry";
    const refused = "ATMOSPHERE REJECTED: for TEST MOON, no such body, reopen VIEW to retry";
    render(
      <ViewLabelBlock
        lines={[{ label: "STYLE", value: "PHOTOREALISTIC" }]}
        statements={[
          "ATMOSPHERE: NOT RESOLVED",
          { text: refused, standing: "refused" },
          { text: failed, standing: "fault" },
        ]}
        countLine={null}
        faults={[]}
      />,
    );
    const statuses = screen.getAllByRole("status");
    const of = (text: string): HTMLElement | undefined =>
      statuses.find((output) => output.textContent === text);
    // The server's own failure in --status-caution, as a fault; a refusal of what was asked plain.
    expect([
      of(failed)?.classList.contains("request-status__text--fault"),
      of(refused)?.classList.contains("request-status__text--fault"),
    ]).toEqual([true, false]);
    expect(screen.queryByRole("alert")).toBeNull();
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

  describe("with a graphics fault and the terrain's (R10.T16.b)", () => {
    it("sets each fault on a fault line of its own, the graphics fault first", () => {
      renderFaults();
      const faults = screen
        .getAllByRole("status")
        .filter((output) => output.classList.contains("request-status__text--fault"));
      expect(faults.map((output) => output.textContent)).toEqual([
        "GRAPHICS DEVICE LOST: re-creating",
        "TERRAIN NOT AVAILABLE: height computation failed, reopen VIEW to retry",
      ]);
    });

    it("sets the fault line in --status-caution, the guide's colour for a failed system", () => {
      expect(STYLES).toMatch(
        /\.request-status__text--fault,[^{]*\{\s*color: var\(--status-caution\);\s*\}/u,
      );
    });

    it("tracks the terrain's state and leaves its sentence after the colon untracked (Typography)", () => {
      renderFaults();
      expect(partsOf(TERRAIN_NOT_AVAILABLE)).toEqual([
        "status-text__state",
        "status-text__sentence",
      ]);
    });

    it("raises no alert", () => {
      renderFaults();
      expect(screen.queryByRole("alert")).toBeNull();
    });
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

/** A block's scrolling part named `name`, a region while it scrolls. */
function scrolling(name: string): HTMLElement {
  return screen.getByRole("region", { name });
}

/** The labels of the lines a part of a block holds, in order. */
function labelsIn(part: HTMLElement): Array<string | null> {
  return within(part)
    .getAllByRole("term")
    .map((term) => term.textContent);
}

describe("the PRIMARY view's upper plate in its room (R10.T12.b)", () => {
  /** Its lines before STARS, which scroll: FRAME to METER, in `labelLines`' order. */
  const MIDDLE_LABELS = [
    "FRAME",
    "TIME",
    "STYLE",
    "QUALITY",
    "CAMERA",
    "FOV",
    "CONTOUR INTERVAL",
    "EXPOSURE",
    "METER",
  ];
  /** STARS and the lines after it, SCENE and SCENE CLOCK as `labelLines` sets them, never scrolled. */
  const REST_LABELS = ["STARS", "SCENE", "SCENE CLOCK"];
  const LABELS = [...MIDDLE_LABELS, ...REST_LABELS];
  const LINES = LABELS.map((label) => ({ label, value: "NOMINAL" }));
  const STATEMENT = "POSITIONS AS SEEN FROM SHIP";
  const COUNT = "STARS 1,204 DRAWN";
  const FAULT = "GRAPHICS DEVICE LOST: re-creating";
  const LINES_NAME = "PRIMARY lines";
  const BODY_NAME = "PRIMARY label block";

  /** What the plate's scrolls count as one item each, as the block measures them. */
  const ITEMS = ".view-label__line, .view-label__statement, .view-label__count, .request-status";
  /** Each item's laid-out height, and each scroll's box, CSS px. */
  const ITEM_PX = 20;
  const BOX_PX = 100;

  const MIDDLE: UpperRoom = { kind: "middle", heightPx: 140, lines: 6 };
  const BODY: UpperRoom = { kind: "body", heightPx: BOX_PX };

  /**
   * Lays the plate out as jsdom does not: each item of a scroll 20 px tall, one under another, in
   * a 100 px box whose content is its items.
   */
  function layOut(): void {
    vi.spyOn(HTMLElement.prototype, "offsetTop", "get").mockImplementation(function top(
      this: HTMLElement,
    ) {
      const scroll = this.closest(".view-label__scroll");
      const index = scroll === null ? -1 : [...scroll.querySelectorAll(ITEMS)].indexOf(this);
      return index < 0 ? 0 : index * ITEM_PX;
    });
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function tall(
      this: HTMLElement,
    ) {
      return this.matches(ITEMS) ? ITEM_PX : 0;
    });
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function box(
      this: HTMLElement,
    ) {
      return this.classList.contains("view-label__scroll") ? BOX_PX : 0;
    });
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockImplementation(function content(
      this: HTMLElement,
    ) {
      return this.classList.contains("view-label__scroll")
        ? this.querySelectorAll(ITEMS).length * ITEM_PX
        : 0;
    });
  }

  /** The plate's props in `room`, its lines scrolling from STARS. */
  function plate(
    room: UpperRoom,
    statements: ReadonlyArray<string> = [STATEMENT],
  ): ViewLabelBlockProps {
    return {
      lines: LINES,
      statements,
      countLine: COUNT,
      faults: [FAULT],
      scroll: { from: "STARS", room, view: "PRIMARY" },
    };
  }

  /** The plate laid out in `room`. */
  function renderIn(room: UpperRoom): ReturnType<typeof render> {
    layOut();
    return render(<ViewLabelBlock {...plate(room)} />);
  }

  describe("while its lines before STARS scroll", () => {
    it("scrolls FRAME to METER in a region named for the view", () => {
      renderIn(MIDDLE);
      expect(labelsIn(scrolling(LINES_NAME))).toEqual(MIDDLE_LABELS);
    });

    it("sets the region as a named section, with no role of its own", () => {
      renderIn(MIDDLE);
      const region = scrolling(LINES_NAME);
      expect([region.tagName, region.getAttribute("role")]).toEqual(["SECTION", null]);
    });

    it("names no region for the whole plate", () => {
      renderIn(MIDDLE);
      expect(screen.queryByRole("region", { name: BODY_NAME })).toBeNull();
    });

    it("gives the region the focus from the keyboard, so that the keys scroll it", async () => {
      const user = userEvent.setup();
      renderIn(MIDDLE);
      await user.tab();
      expect(scrolling(LINES_NAME)).toHaveFocus();
    });

    it("sets the region's box as tall as the room gives it", () => {
      renderIn(MIDDLE);
      expect(scrolling(LINES_NAME)).toHaveStyle({ maxHeight: "140px" });
    });

    it("states the lines in view and their total under the region", () => {
      renderIn(MIDDLE);
      expect(scrolling(LINES_NAME).nextElementSibling?.textContent).toBe("1-5 of 9");
    });

    it("keeps STARS, the statements, the count line and the faults out of the region", () => {
      renderIn(MIDDLE);
      const region = scrolling(LINES_NAME);
      expect([
        within(region).queryByText("STARS"),
        within(region).queryByText(statusLine(STATEMENT)),
        within(region).queryByText(COUNT),
        within(region).queryByText(statusLine(FAULT)),
      ]).toEqual([null, null, null, null]);
    });

    it("never scrolls SCENE, which stands after STARS", () => {
      renderIn(MIDDLE);
      expect(within(scrolling(LINES_NAME)).queryByText("SCENE")).toBeNull();
    });

    it("stands the lines from STARS on under the position line", () => {
      renderIn(MIDDLE);
      const lines = scrolling(LINES_NAME).nextElementSibling?.nextElementSibling;
      expect(lines instanceof HTMLElement ? labelsIn(lines) : null).toEqual(REST_LABELS);
    });

    it("drops the region and its position line once the plate stands whole again", () => {
      const { rerender } = renderIn(MIDDLE);
      rerender(<ViewLabelBlock {...plate({ kind: "whole" })} />);
      expect([
        screen.queryByRole("region"),
        document.querySelector(".view-label__position"),
      ]).toEqual([null, null]);
    });
  });

  describe("while it stands whole", () => {
    it("sets no region", () => {
      renderIn({ kind: "whole" });
      expect(screen.queryByRole("region")).toBeNull();
    });

    it("gives nothing of it the focus", async () => {
      const user = userEvent.setup();
      renderIn({ kind: "whole" });
      await user.tab();
      expect(document.body).toHaveFocus();
    });

    it("states no position line", () => {
      renderIn({ kind: "whole" });
      expect(screen.queryByText(/ of \d+$/u)).toBeNull();
    });
  });

  describe("while everything under its VIEW line scrolls", () => {
    it("scrolls every line in a region named for the view's label block", () => {
      renderIn(BODY);
      expect(labelsIn(scrolling(BODY_NAME))).toEqual(LABELS);
    });

    it("sets the region as a named section, with no role of its own", () => {
      renderIn(BODY);
      const region = scrolling(BODY_NAME);
      expect([region.tagName, region.getAttribute("role")]).toEqual(["SECTION", null]);
    });

    it("names no region for the lines before STARS within it", () => {
      renderIn(BODY);
      expect(screen.queryByRole("region", { name: LINES_NAME })).toBeNull();
    });

    it("gives the region the focus from the keyboard", async () => {
      const user = userEvent.setup();
      renderIn(BODY);
      await user.tab();
      expect(scrolling(BODY_NAME)).toHaveFocus();
    });

    it("scrolls the statements, the count line and the faults with the lines", () => {
      renderIn(BODY);
      const region = scrolling(BODY_NAME);
      expect([
        within(region).queryByText(statusLine(STATEMENT)) !== null,
        within(region).queryByText(COUNT) !== null,
        within(region).queryByText(statusLine(FAULT)) !== null,
      ]).toEqual([true, true, true]);
    });

    it("keeps the VIEW line above the region", () => {
      renderIn(BODY);
      expect(within(scrolling(BODY_NAME)).queryByText("VIEW")).toBeNull();
    });

    it("states its items' position and total under the region, out of it", () => {
      renderIn(BODY);
      const position = scrolling(BODY_NAME).nextElementSibling;
      expect([
        position?.classList.contains("view-label__position"),
        position?.textContent.endsWith(" of 15"),
      ]).toEqual([true, true]);
    });

    it("holds the scroll at its foot as its content grows", () => {
      const { rerender } = renderIn(BODY);
      rerender(<ViewLabelBlock {...plate(BODY, [STATEMENT, "EASED CAMERA MOVES"])} />);
      expect(scrolling(BODY_NAME).scrollTop).toBe(16 * ITEM_PX);
    });

    it("leaves the scroll where the operator scrolled it from the foot", () => {
      const { rerender } = renderIn(BODY);
      const region = scrolling(BODY_NAME);
      region.scrollTop = 0;
      fireEvent.scroll(region);
      rerender(<ViewLabelBlock {...plate(BODY, [STATEMENT, "EASED CAMERA MOVES"])} />);
      expect(region.scrollTop).toBe(0);
    });

    it("takes the region's tab stop away once the plate stands whole again", () => {
      const { rerender } = renderIn(BODY);
      const region = scrolling(BODY_NAME);
      rerender(<ViewLabelBlock {...plate({ kind: "whole" })} />);
      expect(region.tabIndex).toBe(-1);
    });
  });

  describe("on a block that never scrolls, an instrument's", () => {
    it("sets no region", () => {
      layOut();
      render(
        <ViewLabelBlock lines={LINES} statements={[STATEMENT]} countLine={COUNT} faults={[]} />,
      );
      expect(screen.queryByRole("region")).toBeNull();
    });

    it("keeps its lines in one list", () => {
      const { container } = render(
        <ViewLabelBlock lines={LINES} statements={[]} countLine={null} faults={[]} />,
      );
      expect(container.querySelectorAll("dl")).toHaveLength(1);
    });
  });
});
