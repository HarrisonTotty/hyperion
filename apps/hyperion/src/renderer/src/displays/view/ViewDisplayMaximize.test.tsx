/**
 * `VIEW`'s `MAXIMIZE VIEW` (the owner's request, 2026-11-01): the side columns hidden and restored,
 * the keys reaching their controls while they are, the primary's statements folded, and the setting
 * kept through a change of scene, in both layouts. jsdom lays nothing out, so the harness gives each
 * box its size; the stage's place is measured by the hidden captures in the lane's outbox.
 */
import { screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  COMPACT_VIEW_PX,
  FULL_VIEW_PX,
  type LaidOutPx,
  nominalStore,
  primaryLabelBlock,
  renderViewDisplay,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { statusLine } from "../../test/statusLine";
import { stylesheetRule } from "../../test/stylesheet";
import { fakeViewEngineSource } from "../../test/fakeViewEngine";
import { NO_OWN_SHIP } from "./CameraControls";
import type { KeyPress } from "../../view/camera/keys";
import { GRID_KEY, MAXIMIZE_KEY, MAXIMIZED_KEYS, viewDisplayKey } from "./displayKeys";
import { PHOTOREAL_NOT_CREATED } from "./styleRefusals";
import type { ViewEngineSource } from "./useViewEngine";
import { POSITIONS_FROM_SHIP } from "./viewRun";

afterEach(() => {
  vi.useRealTimers();
});

/**
 * `VIEW` over a nominal adapter, both styles offered, its `.view` box at `viewPx`, its engine from
 * `source` where given.
 */
async function setup(
  viewPx: LaidOutPx = FULL_VIEW_PX,
  source?: ViewEngineSource,
): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: source ?? fake.source,
    engines: fake.engines,
    viewPx,
  });
  await settle();
  view.advance(300);
  return view;
}

/** Plays `steps` × 300 ms of frames, letting each step's work settle. */
async function playFor(view: ViewDisplayHarness, steps: number): Promise<void> {
  for (let step = 0; step < steps; step += 1) {
    view.advance(300);
    // Each step waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
}

/** Opens `INSTRUMENT 1` from the `Instruments` panel. */
async function openInstrument(view: ViewDisplayHarness): Promise<void> {
  await view.user.click(
    within(screen.getByRole("group", { name: "INSTRUMENT 1" })).getByRole("button", {
      name: "OPEN",
    }),
  );
  view.advance(300);
}

/** `MAXIMIZE VIEW`'s button, by its key and name. */
function maximizeButton(): HTMLElement {
  return screen.getByRole("button", { name: `${MAXIMIZE_KEY} MAXIMIZE VIEW` });
}

/** Presses `MAXIMIZE VIEW`'s key, as from the page. */
async function pressMaximize(view: ViewDisplayHarness): Promise<void> {
  await view.user.keyboard(MAXIMIZE_KEY);
  view.advance(300);
}

/** An engine source whose engine refuses every material: the photorealistic style faults. */
function refusingMaterialsSource(): ViewEngineSource {
  const fake = fakeViewEngineSource();
  return {
    ...fake.source,
    load: async (outcome, status) => {
      const engine = await fake.source.load(outcome, status);
      return Object.assign(engine, {
        createMaterialAsync: (): Promise<never> => Promise.reject(new Error("refused")),
      });
    },
  };
}

/** What stands under the key legend while MAXIMIZE VIEW is on. */
function maximizedPart(): HTMLElement {
  const part = document.querySelector<HTMLElement>(".view__maximized");
  if (part === null) {
    throw new Error("nothing stands under the key legend");
  }
  return part;
}

/** The line under the key legend while MAXIMIZE VIEW is on, as it reads. */
function maximizedLine(): string | null {
  return maximizedPart().querySelector(".view__keys")?.textContent ?? null;
}

/** The side column's panels in the full layout, each by its region's name. */
const SIDE_PANELS = [
  "Instruments",
  "Targets PRIMARY",
  "Camera PRIMARY",
  "Style PRIMARY",
  "Exposure PRIMARY",
];

/** Which of {@link SIDE_PANELS} are on show. */
function panelsShown(): ReadonlyArray<string> {
  return SIDE_PANELS.filter((name) => screen.queryByRole("region", { name }) !== null);
}

/** A label block's reading of `label`, its value's text as it reads, or `null` where it has none. */
function readingIn(block: HTMLElement, label: string): string | null {
  const term = within(block).queryByText(label, { selector: "dt" });
  return term?.nextElementSibling?.textContent ?? null;
}

/** The primary's statements under its lines, as they read. */
function primaryStatements(): ReadonlyArray<string | null> {
  return [...primaryLabelBlock().querySelectorAll(".view-label__statement")].map(
    (each) => each.textContent,
  );
}

/** The `VIEW` box, whose classes name its layout. */
function viewBox(): HTMLElement {
  const box = document.querySelector<HTMLElement>(".view");
  if (box === null) {
    throw new Error("VIEW is not laid out");
  }
  return box;
}

/** The list of the primary's marks, and the name of the one selected, or `null`. */
function selectedMark(): string | null {
  const list = screen.getByRole("listbox", { name: "Marks in view" });
  return (
    within(list).queryAllByRole("option", { selected: true })[0]?.querySelector(".view-list__name")
      ?.textContent ?? null
  );
}

describe("VIEW's MAXIMIZE VIEW (the owner's request, 2026-11-01)", () => {
  it("offers MAXIMIZE VIEW with its key, off as the display mounts", async () => {
    await setup();
    expect([
      maximizeButton().getAttribute("aria-pressed"),
      maximizeButton().getAttribute("aria-keyshortcuts"),
    ]).toEqual(["false", MAXIMIZE_KEY]);
  });

  it("hides every side panel once pressed", async () => {
    const view = await setup();
    await view.user.click(maximizeButton());
    expect([maximizeButton().getAttribute("aria-pressed"), panelsShown()]).toEqual(["true", []]);
  });

  it("shows every side panel again once pressed again", async () => {
    const view = await setup();
    await view.user.click(maximizeButton());
    await view.user.click(maximizeButton());
    expect(panelsShown()).toEqual(SIDE_PANELS);
  });

  it("turns on and off by its key from anywhere on the display", async () => {
    const view = await setup();
    await pressMaximize(view);
    const on = panelsShown();
    await pressMaximize(view);
    expect([on, panelsShown()]).toEqual([[], SIDE_PANELS]);
  });

  it("does not take its key typed in a text field", async () => {
    const view = await setup();
    await view.user.click(screen.getByRole("textbox", { name: /MAN/u }));
    await view.user.keyboard(MAXIMIZE_KEY);
    expect(maximizeButton().getAttribute("aria-pressed")).toBe("false");
  });

  it("keeps the scene bar while on", async () => {
    const view = await setup();
    await pressMaximize(view);
    expect(screen.getByRole("group", { name: "Scene" })).toBeVisible();
  });

  it("names the key that shows the panels under the key legend while on, and not while off", async () => {
    const view = await setup();
    const off = screen.queryByText(MAXIMIZED_KEYS);
    await pressMaximize(view);
    expect([off, screen.getByText(MAXIMIZED_KEYS).tagName]).toEqual([null, "P"]);
  });

  it("stays on through a change of scene", async () => {
    const view = await setup();
    await pressMaximize(view);
    await view.user.click(screen.getByRole("button", { name: "FRAME CHANGE TEST" }));
    view.advance(300);
    expect([maximizeButton().getAttribute("aria-pressed"), panelsShown()]).toEqual(["true", []]);
  });

  it("gives the focus from a panel it hides to the primary's canvas, the CONTROLS view", async () => {
    const view = await setup();
    await view.user.click(screen.getByRole("button", { name: "2 CHASE" }));
    await pressMaximize(view);
    expect(document.activeElement).toBe(
      screen.getByRole("application", { name: /^VIEW, .*, PRIMARY, / }),
    );
  });

  it("gives the focus from a panel it hides to the CONTROLS instrument's canvas", async () => {
    const view = await setup();
    await openInstrument(view);
    await view.user.click(
      within(screen.getByRole("group", { name: "CONTROLS" })).getByRole("button", {
        name: "INSTRUMENT 1",
      }),
    );
    view.advance(300);
    await pressMaximize(view);
    expect(document.activeElement).toBe(
      within(screen.getByRole("region", { name: "INSTRUMENT 1" })).getByRole("application"),
    );
  });

  it("gives the focus from a panel it hides to its own button where no canvas stands", async () => {
    const fake = timedEngineSource();
    const view = renderViewDisplay({
      store: await nominalStore(),
      // An engine that never loads: the view's status stands in its place, with no canvas.
      source: { ...fake.source, load: () => new Promise(() => undefined) },
      engines: fake.engines,
    });
    await settle();
    view.advance(300);
    await view.user.click(screen.getByRole("button", { name: "2 CHASE" }));
    await pressMaximize(view);
    expect(document.activeElement).toBe(maximizeButton());
  });

  it("keeps an open instrument in its slot", async () => {
    const view = await setup();
    await openInstrument(view);
    await pressMaximize(view);
    expect(screen.getByRole("region", { name: "INSTRUMENT 1" })).toBeVisible();
  });

  describe("the keys while it is on", () => {
    it("sets the camera's preset by its key", async () => {
      const view = await setup();
      await pressMaximize(view);
      await view.user.keyboard("3");
      view.advance(300);
      expect(readingIn(primaryLabelBlock(), "CAMERA")).toMatch(/^FREE/u);
    });

    it("narrows the field of view by its key", async () => {
      const view = await setup();
      const before = readingIn(primaryLabelBlock(), "FOV");
      await pressMaximize(view);
      await view.user.keyboard("+");
      view.advance(300);
      expect(readingIn(primaryLabelBlock(), "FOV")).not.toBe(before);
    });

    it("switches the style by its key", async () => {
      const view = await setup();
      await pressMaximize(view);
      await view.user.keyboard("4");
      await playFor(view, 4);
      expect(readingIn(primaryLabelBlock(), "STYLE")).toBe("PHOTOREALISTIC");
    });

    it("sets the GRID OFF by its key over the photorealistic image", async () => {
      const view = await setup();
      await pressMaximize(view);
      await view.user.keyboard("4");
      await playFor(view, 4);
      await view.user.keyboard(GRID_KEY);
      view.advance(300);
      expect(readingIn(primaryLabelBlock(), "GRID")).toBe("OFF");
    });

    it("leaves the GRID as it is by its key in the WIREFRAME, where its row does not stand", async () => {
      const view = await setup();
      await pressMaximize(view);
      await view.user.keyboard(GRID_KEY);
      await view.user.keyboard("4");
      await playFor(view, 4);
      expect([
        readingIn(primaryLabelBlock(), "STYLE"),
        readingIn(primaryLabelBlock(), "GRID"),
      ]).toEqual(["PHOTOREALISTIC", null]);
    });

    it("sets the CONTROLS instrument's GRID OFF by its key, the primary's kept", async () => {
      const view = await setup();
      await openInstrument(view);
      await view.user.click(
        within(screen.getByRole("group", { name: "CONTROLS" })).getByRole("button", {
          name: "INSTRUMENT 1",
        }),
      );
      await view.user.keyboard("4");
      await playFor(view, 4);
      await pressMaximize(view);
      await view.user.keyboard(GRID_KEY);
      view.advance(300);
      expect([
        readingIn(screen.getByRole("region", { name: "INSTRUMENT 1" }), "GRID"),
        readingIn(primaryLabelBlock(), "GRID"),
      ]).toEqual(["OFF", null]);
    });

    it("selects the next target by its key", async () => {
      const view = await setup();
      const before = selectedMark();
      await pressMaximize(view);
      await view.user.keyboard("]");
      view.advance(300);
      await pressMaximize(view);
      expect([before, selectedMark()]).toEqual([null, expect.any(String)]);
    });
  });

  describe("what the hidden panels would show, under its line", () => {
    it("names the primary's selection", async () => {
      const view = await setup();
      await view.user.keyboard("]");
      view.advance(300);
      const name = selectedMark();
      await pressMaximize(view);
      expect([name !== null, maximizedLine()]).toEqual([
        true,
        `${MAXIMIZED_KEYS} · SELECTED ${String(name)}`,
      ]);
    });

    it("names the CONTROLS view where it is an instrument", async () => {
      const view = await setup();
      await openInstrument(view);
      await view.user.click(
        within(screen.getByRole("group", { name: "CONTROLS" })).getByRole("button", {
          name: "INSTRUMENT 1",
        }),
      );
      view.advance(300);
      await pressMaximize(view);
      expect(maximizedLine()).toBe(`${MAXIMIZED_KEYS} · CONTROLS INSTRUMENT 1`);
    });

    it("stands NO OWN SHIP where the scene has none", async () => {
      const view = await setup();
      await view.user.click(screen.getByRole("button", { name: "PHASE TEST" }));
      view.advance(300);
      await pressMaximize(view);
      expect(
        within(maximizedPart()).getByText(statusLine(NO_OWN_SHIP), {
          selector: ".view-folds__standing",
        }),
      ).toBeVisible();
    });

    it("stands the style's fault", async () => {
      vi.spyOn(console, "error").mockImplementation(() => undefined);
      const view = await setup(FULL_VIEW_PX, refusingMaterialsSource());
      await pressMaximize(view);
      await view.user.keyboard("4");
      view.advance(100);
      await settle();
      view.advance(300);
      expect(
        within(maximizedPart()).getByText(statusLine(PHOTOREAL_NOT_CREATED), {
          selector: ".view-folds__standing",
        }),
      ).toBeVisible();
    });
  });

  describe("the primary's statements", () => {
    it("folds them to one line while it is on", async () => {
      const view = await setup();
      await view.user.keyboard("3");
      view.advance(300);
      await pressMaximize(view);
      expect(primaryStatements()).toEqual([expect.stringMatching(/^STATEMENTS: \d+ FOLDED$/u)]);
    });

    it("states them whole again once it is off", async () => {
      const view = await setup();
      await view.user.keyboard("3");
      view.advance(300);
      await pressMaximize(view);
      await pressMaximize(view);
      expect(primaryStatements()).toContain(POSITIONS_FROM_SHIP);
    });
  });
});

describe("VIEW's layouts with MAXIMIZE VIEW (the owner's request, 2026-11-01)", () => {
  it.each([
    ["1920 × 1080", FULL_VIEW_PX, "view--full"],
    ["1280 × 720", COMPACT_VIEW_PX, "view--compact"],
  ] as const)(
    "keeps its layout at %s, the stage taking the body while on",
    async (_size, px, layout) => {
      const view = await setup(px);
      await pressMaximize(view);
      expect([
        viewBox().classList.contains(layout),
        viewBox().classList.contains("view--maximized"),
        document.querySelector(".view__side")?.hasAttribute("hidden"),
      ]).toEqual([true, true, true]);
    },
  );

  it.each([
    ["1920 × 1080", FULL_VIEW_PX],
    ["1280 × 720", COMPACT_VIEW_PX],
  ] as const)("restores the side column at %s as it stood", async (_size, px) => {
    const view = await setup(px);
    await view.user.click(screen.getByRole("button", { name: "2 CHASE" }));
    view.advance(300);
    const before = panelsShown();
    await pressMaximize(view);
    await pressMaximize(view);
    expect([
      viewBox().classList.contains("view--maximized"),
      before.length > 0 && panelsShown().join() === before.join(),
      screen.getByRole("button", { name: "2 CHASE" }).getAttribute("aria-pressed"),
    ]).toEqual([false, true, "true"]);
  });

  it("keeps the compact layout's open panel through on and off", async () => {
    const view = await setup(COMPACT_VIEW_PX);
    await view.user.click(screen.getByRole("button", { name: "STYLE" }));
    await pressMaximize(view);
    await pressMaximize(view);
    expect(screen.getByRole("button", { name: "STYLE" })).toHaveAttribute("aria-expanded", "true");
  });

  it("marks MAXIMIZE VIEW on with the pressed toggle's 2px --accent rule under it", () => {
    expect([
      stylesheetRule(".view-display__maximize"),
      stylesheetRule('.view-display__maximize[aria-pressed="true"]'),
    ]).toEqual([
      expect.stringContaining("border-bottom: 2px solid transparent;"),
      expect.stringContaining("border-bottom-color: var(--accent);"),
    ]);
  });

  it("gives the stage the display's whole width, the side column taking no place", () => {
    expect([
      stylesheetRule(".view--maximized"),
      stylesheetRule(".view--maximized > .view__side"),
    ]).toEqual([
      expect.stringContaining("grid-template-columns: minmax(0, 1fr);"),
      expect.stringContaining("display: none;"),
    ]);
  });
});

/** A press of `key` on the page, with no modifier and no repeat, and `overrides` over it. */
function press(key: string, overrides: Partial<KeyPress> = {}): KeyPress {
  return {
    key,
    ctrlKey: false,
    altKey: false,
    metaKey: false,
    shiftKey: false,
    repeat: false,
    target: document.body,
    ...overrides,
  };
}

describe("VIEW's display keys (the owner's request, 2026-11-01)", () => {
  it("reads 5 as the GRID row's and 6 as MAXIMIZE VIEW's", () => {
    expect([viewDisplayKey(press(GRID_KEY)), viewDisplayKey(press(MAXIMIZE_KEY))]).toEqual([
      "grid",
      "maximize",
    ]);
  });

  it("reads no other key", () => {
    expect(["4", "7", "m", "g"].map((key) => viewDisplayKey(press(key)))).toEqual([
      null,
      null,
      null,
      null,
    ]);
  });

  it("passes a key repeat through", () => {
    expect(viewDisplayKey(press(MAXIMIZE_KEY, { repeat: true }))).toBeNull();
  });

  it.each(["ctrlKey", "altKey", "metaKey", "shiftKey"] as const)(
    "passes a press with %s through",
    (modifier) => {
      expect(viewDisplayKey(press(MAXIMIZE_KEY, { [modifier]: true }))).toBeNull();
    },
  );

  it("passes a press in a text field through", () => {
    const field = document.createElement("input");
    expect(viewDisplayKey(press(GRID_KEY, { target: field }))).toBeNull();
  });
});
