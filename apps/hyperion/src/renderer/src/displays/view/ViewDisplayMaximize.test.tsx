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
import { type KeyPress, VIEW_SINGLE_KEYS } from "../../view/camera/keys";
import { STYLE_TOGGLE_KEY } from "../../view/photoreal/style";
import {
  GRID_KEY,
  HIDDEN_CONTROL_KEYS,
  LABEL_BLOCK_FOLDS,
  LABEL_BLOCK_KEY,
  LABEL_BLOCK_KEYS,
  MAXIMIZE_KEY,
  MAXIMIZED_KEYS,
  viewDisplayKey,
} from "./displayKeys";
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

/** The view's foot's lines: the key legend, and while MAXIMIZE VIEW is on what runs on after it. */
function maximizedPart(): HTMLElement {
  const part = document.querySelector<HTMLElement>(".view__foot-lines");
  if (part === null) {
    throw new Error("the view has no foot");
  }
  return part;
}

/** MAXIMIZE VIEW's line after the key legend, its parts joined as they read, or `null` while off. */
function maximizedLine(): string | null {
  const parts = [...maximizedPart().querySelectorAll(".view__maximized-keys")].map(
    (part) => part.textContent,
  );
  return parts.length === 0 ? null : parts.join(" · ");
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

  it("names the key that shows the panels after the key legend while on, and not while off", async () => {
    const view = await setup();
    const off = maximizedLine();
    await pressMaximize(view);
    expect([off, maximizedLine()?.startsWith(MAXIMIZED_KEYS)]).toEqual([null, true]);
  });

  it("names every key of the controls it hides on its line", async () => {
    const view = await setup();
    await pressMaximize(view);
    expect(maximizedLine()).toBe(`${MAXIMIZED_KEYS} · ${HIDDEN_CONTROL_KEYS}`);
  });

  it("runs its line on from the key legend's, in one row that wraps between parts", async () => {
    const view = await setup();
    await pressMaximize(view);
    const legend = maximizedPart().querySelector(".view__keys");
    if (legend === null) {
      throw new Error("the view's foot has no key legend");
    }
    const after = legend.nextElementSibling;
    expect([
      legend.textContent.startsWith("FOCUSED VIEW"),
      after !== null && after.querySelector(".view__maximized-keys") !== null,
      stylesheetRule(".view__foot-lines"),
    ]).toEqual([true, true, expect.stringContaining("flex-wrap: wrap;")]);
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
      await pressMaximize(view);
      expect([before !== null, readingIn(primaryLabelBlock(), "FOV") !== before]).toEqual([
        true,
        true,
      ]);
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
        `${MAXIMIZED_KEYS} · ${HIDDEN_CONTROL_KEYS} · SELECTED ${String(name)}`,
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
      expect(maximizedLine()).toBe(
        `${MAXIMIZED_KEYS} · ${HIDDEN_CONTROL_KEYS} · CONTROLS INSTRUMENT 1`,
      );
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
    it("folds them to one line while it is on, after the label block's key", async () => {
      const view = await setup();
      await view.user.keyboard("3");
      view.advance(300);
      await pressMaximize(view);
      expect(primaryStatements()).toEqual([
        expect.stringMatching(new RegExp(`^${LABEL_BLOCK_KEYS} · STATEMENTS: \\d+ FOLDED$`, "u")),
      ]);
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

/** The labels of the primary's label block's lines, in order. */
function primaryLabels(): ReadonlyArray<string | null> {
  return [...primaryLabelBlock().querySelectorAll("dt")].map((term) => term.textContent);
}

/** Presses the label block's key, as from the page. */
async function pressLabelBlock(view: ViewDisplayHarness): Promise<void> {
  await view.user.keyboard(LABEL_BLOCK_KEY);
  view.advance(300);
}

describe("the primary's label block under MAXIMIZE VIEW (the owner's request, 2026-11-01)", () => {
  it("folds to its summary once on: its style, camera and exposure", async () => {
    const view = await setup();
    await pressMaximize(view);
    expect(primaryLabels()).toEqual(["STYLE", "CAMERA", "EXPOSURE"]);
  });

  it("names on its summary the key that shows the whole block", async () => {
    const view = await setup();
    await pressMaximize(view);
    expect(primaryStatements()[0]).toMatch(new RegExp(`^${LABEL_BLOCK_KEYS}`, "u"));
  });

  it("keeps the camera's rate in the summary in FREE", async () => {
    const view = await setup();
    await view.user.keyboard("3");
    view.advance(300);
    await pressMaximize(view);
    expect(readingIn(primaryLabelBlock(), "CAMERA")).toMatch(/^FREE · RATE /u);
  });

  it("keeps GRID OFF in the summary while the grid is off", async () => {
    const view = await setup();
    await pressMaximize(view);
    await view.user.keyboard("4");
    await playFor(view, 4);
    await view.user.keyboard(GRID_KEY);
    view.advance(300);
    expect(primaryLabels()).toEqual(["STYLE", "GRID", "CAMERA", "EXPOSURE"]);
  });

  it("shows the whole block by its key, its statements still folded after the key that folds it", async () => {
    const view = await setup();
    const whole = primaryLabels();
    await pressMaximize(view);
    await pressLabelBlock(view);
    expect([primaryLabels(), primaryStatements()[0]?.startsWith(LABEL_BLOCK_FOLDS)]).toEqual([
      whole,
      true,
    ]);
  });

  it("folds the whole block again by its key", async () => {
    const view = await setup();
    await pressMaximize(view);
    await pressLabelBlock(view);
    await pressLabelBlock(view);
    expect(primaryLabels()).toEqual(["STYLE", "CAMERA", "EXPOSURE"]);
  });

  it("folds it again each time MAXIMIZE VIEW is turned on", async () => {
    const view = await setup();
    await pressMaximize(view);
    await pressLabelBlock(view);
    await pressMaximize(view);
    await pressMaximize(view);
    expect(primaryLabels()).toEqual(["STYLE", "CAMERA", "EXPOSURE"]);
  });

  it("restores the whole block exactly once off", async () => {
    const view = await setup();
    await view.user.keyboard("3");
    view.advance(300);
    const before = [primaryLabels(), primaryStatements()];
    await pressMaximize(view);
    await pressMaximize(view);
    expect([primaryLabels(), primaryStatements()]).toEqual(before);
  });

  it("does nothing by its key while MAXIMIZE VIEW is off", async () => {
    const view = await setup();
    const before = primaryLabels();
    await pressLabelBlock(view);
    expect([before.length > 3, primaryLabels()]).toEqual([true, before]);
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
      stylesheetRule(".view__maximize"),
      stylesheetRule('.view__maximize[aria-pressed="true"]'),
    ]).toEqual([
      expect.stringContaining("border-bottom: 2px solid transparent;"),
      expect.stringContaining("border-bottom-color: var(--accent);"),
    ]);
  });

  it.each([
    ["1920 × 1080", FULL_VIEW_PX],
    ["1280 × 720", COMPACT_VIEW_PX],
  ] as const)(
    "stands its toggle at the view's foot at %s, not in the SCENE bar",
    async (_size, px) => {
      const view = await setup(px);
      const off = maximizeButton().closest(".view__foot") !== null;
      await pressMaximize(view);
      expect([
        off,
        maximizeButton().closest(".view__foot") !== null,
        maximizeButton().closest(".view-display__bar"),
      ]).toEqual([true, true, null]);
    },
  );

  it("lets the key legend wrap beside the toggle, which keeps its width", () => {
    expect([stylesheetRule(".view__foot-lines"), stylesheetRule(".view__maximize")]).toEqual([
      expect.stringContaining("min-width: 0;"),
      expect.stringContaining("flex: none;"),
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
  it("names on MAXIMIZE VIEW's line each hidden control's key as VIEW binds it", () => {
    expect([
      VIEW_SINGLE_KEYS["1"],
      VIEW_SINGLE_KEYS["2"],
      VIEW_SINGLE_KEYS["3"],
      VIEW_SINGLE_KEYS[STYLE_TOGGLE_KEY],
      VIEW_SINGLE_KEYS["["],
      VIEW_SINGLE_KEYS["]"],
      VIEW_SINGLE_KEYS["+"],
      VIEW_SINGLE_KEYS["-"],
      viewDisplayKey(press(GRID_KEY)),
      viewDisplayKey(press(LABEL_BLOCK_KEY)),
      HIDDEN_CONTROL_KEYS,
    ]).toEqual([
      { kind: "preset", preset: "seat" },
      { kind: "preset", preset: "chase" },
      { kind: "preset", preset: "free" },
      { kind: "style", style: "toggle" },
      { kind: "target", step: -1 },
      { kind: "target", step: 1 },
      { kind: "fov", step: -1 },
      { kind: "fov", step: 1 },
      "grid",
      "label-block",
      `1 SEAT, 2 CHASE, 3 FREE, ${STYLE_TOGGLE_KEY} STYLE, ${GRID_KEY} GRID, 7 ENABLE, 8 INHIBIT, ${LABEL_BLOCK_KEY} LABEL BLOCK, [/] TARGET, +/- FOV`,
    ]);
  });

  it("reads 5 as the GRID row's, 6 as MAXIMIZE VIEW's, 7 and 8 as ENABLE and INHIBIT and 9 as the label block's", () => {
    expect(["5", "6", "7", "8", "9"].map((key) => viewDisplayKey(press(key)))).toEqual([
      "grid",
      "maximize",
      "enable",
      "inhibit",
      "label-block",
    ]);
  });

  it("reads no other key", () => {
    expect(["4", "0", "m", "g"].map((key) => viewDisplayKey(press(key)))).toEqual([
      null,
      null,
      null,
      null,
    ]);
  });

  /** VIEW's display keys, 5 to 9. */
  const DISPLAY_KEYS = ["5", "6", "7", "8", "9"];

  it("passes a key repeat of each through", () => {
    expect(DISPLAY_KEYS.map((key) => viewDisplayKey(press(key, { repeat: true })))).toEqual(
      DISPLAY_KEYS.map(() => null),
    );
  });

  it.each(["ctrlKey", "altKey", "metaKey", "shiftKey"] as const)(
    "passes a press of each with %s through",
    (modifier) => {
      expect(DISPLAY_KEYS.map((key) => viewDisplayKey(press(key, { [modifier]: true })))).toEqual(
        DISPLAY_KEYS.map(() => null),
      );
    },
  );

  it("passes a press of each in a text field through", () => {
    const field = document.createElement("input");
    expect(DISPLAY_KEYS.map((key) => viewDisplayKey(press(key, { target: field })))).toEqual(
      DISPLAY_KEYS.map(() => null),
    );
  });
});
