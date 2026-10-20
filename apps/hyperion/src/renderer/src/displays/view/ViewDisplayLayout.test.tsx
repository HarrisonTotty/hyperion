/**
 * `VIEW` at both sizes (plan R07, R07.T19.b; decision-r07-t19-layout): its full and compact
 * layouts, the label block's additions, the instruments' statements and anchors, and the list's
 * head. jsdom lays nothing out, so the harness gives each box its size; the fit, the breaks and
 * the clipping are measured by the hidden captures recorded in R07's Risks.
 */
import { act, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { fakeViewEngineSource } from "../../test/fakeViewEngine";
import { stubMatchMedia } from "../../test/stubMatchMedia";
import {
  COMPACT_VIEW_PX,
  type LaidOutBoxPx,
  type LaidOutPx,
  nominalStore,
  renderViewDisplay,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import {
  DEVICE_LOSS_LIMIT,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../../view/engine/status";
import { NO_VIEW_DRAWN, PHOTOREAL_NOT_CREATED } from "./styleRefusals";
import type { ViewEngineSource } from "./useViewEngine";
import { STUB_REASON } from "./viewRoom";
import { describedText, statusLine } from "../../test/statusLine";
import STYLES from "../../styles.css?raw";

afterEach(() => {
  vi.useRealTimers();
});

async function setup(
  options: {
    readonly viewPx?: LaidOutPx;
    readonly stagePx?: LaidOutPx;
    readonly blockPx?: () => LaidOutBoxPx;
    readonly store?: GraphicsStatusStore;
    readonly source?: ViewEngineSource;
  } = {},
): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: options.store ?? new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
    source: options.source ?? fake.source,
    engines: fake.engines,
    ...(options.viewPx === undefined ? {} : { viewPx: options.viewPx }),
    ...(options.stagePx === undefined ? {} : { stagePx: options.stagePx }),
    ...(options.blockPx === undefined ? {} : { blockPx: options.blockPx }),
  });
  await settle();
  view.advance(100);
  return view;
}

/** A compact `VIEW`. */
function compact(store?: GraphicsStatusStore): Promise<ViewDisplayHarness> {
  return setup({ viewPx: COMPACT_VIEW_PX, ...(store === undefined ? {} : { store }) });
}

/** A side panel's region, by its title and designator, or `null` where it is not on show. */
function region(name: string): HTMLElement | null {
  return screen.queryByRole("region", { name });
}

/** A disclosure button, by its name. */
function disclosure(name: string): HTMLElement {
  return screen.getByRole("button", { name });
}

/** VIEW's own disclosure buttons on show and whether each is expanded, in their order. */
function disclosures(): Array<[string, string | null]> {
  const display = document.querySelector<HTMLElement>(".view");
  if (display === null) {
    throw new Error("VIEW is not laid out");
  }
  return within(display)
    .getAllByRole("button")
    .filter((button) => button.hasAttribute("aria-expanded"))
    .map((button) => [button.textContent, button.getAttribute("aria-expanded")]);
}

/** A view's canvas, by its slot's name, which its label block and statements describe. */
function canvasOf(slot: string): HTMLElement {
  return screen.getByRole("application", { name: new RegExp(`, ${slot}, `) });
}

/** A text the canvas's description holds. */
function describing(text: string): unknown {
  return expect.stringContaining(text);
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

/** The style's fault standing under the row, or `null`. */
function standing(): HTMLElement | null {
  return screen.queryByText(statusLine(PHOTOREAL_NOT_CREATED), {
    selector: ".view-folds__standing",
  });
}

/**
 * How many of the lines reading `text` are on show, in no folded panel: each the innermost element
 * whose whole text it is, so that a line holding a phrase unbroken in a run of its own counts.
 */
function onShow(text: string): number {
  return screen
    .queryAllByText(
      (_, element) =>
        element !== null &&
        element.textContent === text &&
        [...element.children].every((child) => child.textContent !== text),
    )
    .filter((line) => line.closest("[hidden]") === null).length;
}

async function chooseScene(view: ViewDisplayHarness, name: string): Promise<void> {
  await view.user.click(screen.getByRole("button", { name }));
  view.advance(100);
}

/**
 * Advances the frames by `ms` in steps of 50 ms, letting the histograms' reads settle after each,
 * as they would between real frames.
 */
async function advanceReading(view: ViewDisplayHarness, ms: number): Promise<void> {
  for (let elapsed = 0; elapsed < ms; elapsed += 50) {
    view.advance(50);
    // Each step's reads settle before the next step's frames.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
}

/** Presses the key `4` off a canvas and lets the photorealistic pipelines be made. */
async function toggleStyle(view: ViewDisplayHarness): Promise<void> {
  await view.user.keyboard("4");
  view.advance(100);
  await settle();
  view.advance(300);
}

async function openInstrument(view: ViewDisplayHarness, name: string): Promise<void> {
  await view.user.click(
    within(screen.getByRole("group", { name })).getByRole("button", { name: "OPEN" }),
  );
  view.advance(300);
}

/** Whether the region named `name` is the first panel of the full layout's second column. */
function headsSecondColumn(name: string): boolean {
  const panel = screen.getByRole("region", { name });
  const column = panel.closest(".view__column--b");
  return column instanceof HTMLElement && within(column).getAllByRole("region")[0] === panel;
}

/** Points CONTROLS at a view. */
async function controlsAt(view: ViewDisplayHarness, name: string): Promise<void> {
  await view.user.click(
    within(screen.getByRole("group", { name: "CONTROLS" })).getByRole("button", { name }),
  );
  view.advance(100);
}

/** The names of the controls Tab reaches from the primary canvas, in order, up to `steps`. */
async function tabOrder(view: ViewDisplayHarness, steps: number): Promise<string[]> {
  screen.getByRole("application", { name: /PRIMARY/ }).focus();
  const names: string[] = [];
  for (let step = 0; step < steps; step += 1) {
    // Each Tab moves on from where the last left the focus.
    // oxlint-disable-next-line no-await-in-loop
    await view.user.tab();
    const focused = document.activeElement;
    names.push(focused?.getAttribute("aria-label") ?? focused?.textContent ?? "");
  }
  // Tab runs on round the document; each control is taken where it is first reached.
  return [...new Set(names)];
}

describe("VIEW's full layout (R07.T19.b)", () => {
  it("lays its six panels out in two columns, the first's before the second's", async () => {
    const view = await setup({ store: await nominalStore() });
    await chooseScene(view, "PHASE TEST");
    await toggleStyle(view);
    const panels = [
      "Instruments",
      "Targets PRIMARY",
      "Camera PRIMARY",
      "Style PRIMARY",
      "Exposure PRIMARY",
      "Exposure meter PRIMARY",
    ].map((name) => screen.getByRole("region", { name }));
    const inOrder = panels.every(
      (panel, index) =>
        index === 0 ||
        ((panels[index - 1]?.compareDocumentPosition(panel) ?? 0) &
          Node.DOCUMENT_POSITION_FOLLOWING) !==
          0,
    );
    expect([
      inOrder,
      panels.map((panel) => panel.closest(".view__column")?.classList.contains("view__column--a")),
    ]).toEqual([true, [true, true, true, false, false, false]]);
  });

  it("heads the second column with the CONTROLS view's style, the primary's or an instrument's", async () => {
    const view = await setup({ store: await nominalStore() });
    await openInstrument(view, "INSTRUMENT 1");
    const primary = headsSecondColumn("Style PRIMARY");
    await controlsAt(view, "INSTRUMENT 1");
    expect([
      primary,
      headsSecondColumn("Style INSTRUMENT 1"),
      region("Style PRIMARY"),
      region("Camera INSTRUMENT 1")
        ?.closest(".view__column")
        ?.classList.contains("view__column--a"),
    ]).toEqual([true, true, null, true]);
  });

  it("shows every panel, with no disclosure", async () => {
    await setup();
    expect([
      disclosures(),
      ["Camera PRIMARY", "Style PRIMARY", "Exposure PRIMARY"].every(
        (name) => region(name) !== null,
      ),
    ]).toEqual([[], true]);
  });

  it("gives the focus to INHIBIT, the control before the meter, when the meter goes", async () => {
    // The orchestrator's ruling on R07.T16.b: the compact layout gives it to CAMERA instead.
    const view = await setup({ store: await nominalStore() });
    await chooseScene(view, "PHASE TEST");
    await toggleStyle(view);
    await view.user.click(
      within(screen.getByRole("region", { name: "Exposure meter PRIMARY" })).getByRole("button", {
        name: "LIT",
      }),
    );
    const focusedOnMeter = document.activeElement?.textContent;
    // Back to the wireframe: the meter's control, and its panel, go.
    await toggleStyle(view);
    expect([
      focusedOnMeter,
      region("Exposure meter PRIMARY"),
      document.activeElement ===
        within(screen.getByRole("region", { name: "Exposure PRIMARY" })).getByRole("button", {
          name: "INHIBIT",
        }),
    ]).toEqual(["LIT", null, true]);
  });

  it("leaves the focus where it is when the meter goes from under a control outside it", async () => {
    const view = await setup({ store: await nominalStore() });
    await chooseScene(view, "PHASE TEST");
    await toggleStyle(view);
    const wireframe = within(screen.getByRole("region", { name: "Style PRIMARY" })).getByRole(
      "button",
      { name: "WIREFRAME" },
    );
    await view.user.click(wireframe);
    view.advance(100);
    await settle();
    view.advance(300);
    expect([region("Exposure meter PRIMARY"), document.activeElement === wireframe]).toEqual([
      null,
      true,
    ]);
  });
});

describe("VIEW's compact layout (R07.T19.b)", () => {
  it("folds INSTRUMENTS to its title row and opens CAMERA alone when it mounts", async () => {
    await compact();
    expect([
      disclosures(),
      region("Targets PRIMARY") !== null,
      region("Camera PRIMARY") !== null,
      region("Style PRIMARY"),
      region("Exposure PRIMARY"),
      screen.queryByRole("group", { name: "CONTROLS" }),
    ]).toEqual([
      [
        ["Instruments", "false"],
        ["CAMERA", "true"],
        ["STYLE", "false"],
        ["EXPOSURE", "false"],
      ],
      true,
      true,
      null,
      null,
      null,
    ]);
  });

  it("folds the open panel when another opens, and opens CAMERA again when that one folds", async () => {
    const view = await compact();
    await view.user.click(disclosure("STYLE"));
    const styleOpen = [region("Style PRIMARY") !== null, region("Camera PRIMARY")];
    await view.user.click(disclosure("STYLE"));
    expect([
      styleOpen,
      region("Camera PRIMARY") !== null,
      region("Style PRIMARY"),
      disclosure("CAMERA").getAttribute("aria-expanded"),
    ]).toEqual([[true, null], true, null, "true"]);
  });

  it("folds the open panel when INSTRUMENTS opens, and opens CAMERA when it folds", async () => {
    const view = await compact();
    await view.user.click(disclosure("EXPOSURE"));
    await view.user.click(disclosure("Instruments"));
    const instrumentsOpen = [
      screen.queryByRole("group", { name: "CONTROLS" }) !== null,
      region("Exposure PRIMARY"),
    ];
    await view.user.click(disclosure("Instruments"));
    expect([
      instrumentsOpen,
      screen.queryByRole("group", { name: "CONTROLS" }),
      region("Camera PRIMARY") !== null,
    ]).toEqual([[true, null], null, true]);
  });

  it("offers EXPOSURE METER, last, only beside a drawn photorealistic image", async () => {
    const view = await compact(await nominalStore());
    const wireframe = disclosures().map(([name]) => name);
    await chooseScene(view, "PHASE TEST");
    await toggleStyle(view);
    expect([wireframe, disclosures().map(([name]) => name)]).toEqual([
      ["Instruments", "CAMERA", "STYLE", "EXPOSURE"],
      ["Instruments", "CAMERA", "STYLE", "EXPOSURE", "EXPOSURE METER"],
    ]);
  });

  it("gives the focus and the place of the meter's panel to CAMERA when the meter goes", async () => {
    const view = await compact(await nominalStore());
    await chooseScene(view, "PHASE TEST");
    await toggleStyle(view);
    await view.user.click(disclosure("EXPOSURE METER"));
    await view.user.click(
      within(screen.getByRole("region", { name: "Exposure meter PRIMARY" })).getByRole("button", {
        name: "LIT",
      }),
    );
    // Back to the wireframe: the meter's control, and its panel, go.
    await toggleStyle(view);
    expect([
      region("Exposure meter PRIMARY"),
      region("Camera PRIMARY") !== null,
      document.activeElement === disclosure("CAMERA"),
    ]).toEqual([null, true, true]);
  });

  it("opens the panel that holds the focus on a switch from the full layout", async () => {
    const view = await setup();
    const wireframe = within(screen.getByRole("region", { name: "Style PRIMARY" })).getByRole(
      "button",
      { name: "WIREFRAME" },
    );
    wireframe.focus();
    view.resizeView(COMPACT_VIEW_PX);
    expect([
      disclosure("STYLE").getAttribute("aria-expanded"),
      region("Camera PRIMARY"),
      document.activeElement === wireframe,
    ]).toEqual(["true", null, true]);
  });

  it("keeps the full layout's tab order among the parts both show", async () => {
    const shared = [
      "Marks in view",
      "1 SEAT",
      "2 CHASE",
      "3 FREE",
      "[ PREVIOUS TARGET",
      "] NEXT TARGET",
      "Narrower field of view",
      "Wider field of view",
      "EASED CAMERA MOVES",
    ];
    const full = await setup();
    const fullOrder = (await tabOrder(full, 24)).filter((name) => shared.includes(name));
    full.resizeView(COMPACT_VIEW_PX);
    const compactOrder = (await tabOrder(full, 24)).filter((name) => shared.includes(name));
    expect([fullOrder, compactOrder]).toEqual([shared, shared]);
  });

  it("stands a folded panel's fault or status under the row, and folds its limit reasons", async () => {
    const view = await compact();
    await chooseScene(view, "PHASE TEST");
    // The field of view at its narrowest: a limit reason, which folds with the camera panel.
    for (let step = 0; step < 10; step += 1) {
      // Each press is one step.
      // oxlint-disable-next-line no-await-in-loop
      await view.user.keyboard("+");
    }
    view.advance(300);
    const noShip = "NO OWN SHIP: SEAT and CHASE need one";
    const noImage = "AUTO NOT AVAILABLE: NO IMAGE TO METER";
    const narrowest = "NOT AVAILABLE: FOV at its narrowest step";
    // CAMERA open: its own lines in it; EXPOSURE folded, its status under the row.
    const cameraOpen = [onShow(noShip), onShow(narrowest), onShow(noImage)];
    await view.user.click(disclosure("EXPOSURE"));
    // EXPOSURE open: its status in it; CAMERA folded, its status under the row, its limit folded.
    expect([cameraOpen, [onShow(noShip), onShow(narrowest), onShow(noImage)]]).toEqual([
      [1, 1, 1],
      [1, 0, 1],
    ]);
  });

  it("holds NO IMAGE TO METER unbroken in the folded exposure's status under the row (R07.T19.d)", async () => {
    await compact();
    const line = screen.getByText(
      (_, element) =>
        element?.classList.contains("view-folds__standing") === true &&
        element.textContent === "AUTO NOT AVAILABLE: NO IMAGE TO METER",
    );
    expect([...line.querySelectorAll(".view-label__run")].map((run) => run.textContent)).toEqual([
      "NO IMAGE TO METER",
    ]);
  });

  it("stands the meter's own status under the row while the exposure is folded, after its window (R07.T16.b)", async () => {
    const view = await compact(await nominalStore());
    await chooseScene(view, "PHASE TEST");
    await toggleStyle(view);
    await view.user.click(disclosure("EXPOSURE METER"));
    // The fake image holds no body's lit side: LIT weighs nothing in it.
    await view.user.click(
      within(screen.getByRole("region", { name: "Exposure meter PRIMARY" })).getByRole("button", {
        name: "LIT",
      }),
    );
    const folded = "AUTO NOT AVAILABLE: NO LIT SIDE";
    await advanceReading(view, 300);
    const before = onShow(folded);
    await advanceReading(view, 700);
    const line = screen.getByText(
      (_, element) =>
        element?.classList.contains("view-folds__standing") === true &&
        element.textContent === folded,
    );
    expect([
      before,
      onShow(folded),
      [...line.querySelectorAll(".view-label__run")].map((run) => run.textContent),
    ]).toEqual([0, 1, ["NO LIT SIDE"]]);
  });

  it("moves no scroll position of its column or the work area as Tab passes every control", async () => {
    // jsdom lays nothing out and never scrolls on focus, so this guards against code that
    // scrolls; the hidden captures measure the column's fit and the work area's scrollTop.
    const view = await compact();
    await tabOrder(view, 40);
    const side = document.querySelector(".view__side");
    const scrolled: Array<[string, number]> = [];
    for (let node: Element | null = side; node !== null; node = node.parentElement) {
      scrolled.push([node.className, node.scrollTop]);
    }
    expect([
      scrolled.some(([name]) => name.includes("console__work")),
      scrolled.every(([, top]) => top === 0),
    ]).toEqual([true, true]);
  });

  it("opens CAMERA, not the panel the focus last left, on a switch from the full layout", async () => {
    const view = await setup();
    within(screen.getByRole("region", { name: "Style PRIMARY" }))
      .getByRole("button", { name: "WIREFRAME" })
      .focus();
    canvasOf("PRIMARY").focus();
    view.resizeView(COMPACT_VIEW_PX);
    expect(disclosure("CAMERA")).toHaveAttribute("aria-expanded", "true");
  });

  it("stands a style fault under the row while STYLE is folded", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const view = await setup({
      viewPx: COMPACT_VIEW_PX,
      store: await nominalStore(),
      source: refusingMaterialsSource(),
    });
    await toggleStyle(view);
    // Folded, the fault stands under the row, once; open, it is the panel's own line, once.
    const folded = [onShow(PHOTOREAL_NOT_CREATED), standing() !== null];
    await view.user.click(disclosure("STYLE"));
    expect([folded, [onShow(PHOTOREAL_NOT_CREATED), standing()]]).toEqual([
      [1, true],
      [1, null],
    ]);
  });

  it("states no view drawn, not a style fault, once the graphics are ruled out after one", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const store = await nominalStore();
    const view = await setup({ viewPx: COMPACT_VIEW_PX, store, source: refusingMaterialsSource() });
    // The photorealistic pipelines refused: a style fault, standing under the row.
    await toggleStyle(view);
    const faulted = standing() !== null;
    // Then the device is lost until the graphics are disabled: no view can be drawn.
    act(() => {
      for (let loss = 0; loss < DEVICE_LOSS_LIMIT; loss += 1) {
        store.dispatch({ kind: "device-lost", reason: "unknown", message: "lost" });
      }
    });
    view.advance(100);
    const foldedLines = [onShow(PHOTOREAL_NOT_CREATED), onShow(NO_VIEW_DRAWN)];
    await view.user.click(disclosure("STYLE"));
    const reason = within(screen.getByRole("region", { name: "Style PRIMARY" })).getByText(
      statusLine(NO_VIEW_DRAWN),
    );
    expect([faulted, foldedLines, reason.classList.contains("view-style__reason--fault")]).toEqual([
      true,
      [0, 0],
      false,
    ]);
  });

  it("offers STYLE while no view can be drawn too, so that the row changes by the meter alone", async () => {
    await compact(new GraphicsStatusStore(initialGraphicsStatus("safe", false)));
    expect(disclosures().map(([name]) => name)).toEqual([
      "Instruments",
      "CAMERA",
      "STYLE",
      "EXPOSURE",
    ]);
  });
});

describe("VIEW's label block at both sizes (R07.T19.b)", () => {
  it("states the free camera's rate on the CAMERA line in FREE", async () => {
    const view = await setup();
    await view.user.keyboard("3");
    view.advance(300);
    expect(canvasOf("PRIMARY")).toHaveAccessibleDescription(
      describing("CAMERA FREE · RATE 1.00 km/s"),
    );
  });

  it("refuses PAGE UP outside FREE, the rate unchanged", async () => {
    const view = await setup();
    await view.user.click(canvasOf("PRIMARY"));
    await view.user.keyboard("{PageUp}");
    view.advance(300);
    expect(screen.getByRole("status", { name: "Free camera rate" })).toHaveTextContent(
      "RATE 1.00 km/s",
    );
    expect(canvasOf("PRIMARY")).toHaveAccessibleDescription(describing("CAMERA SEAT"));
  });

  it("states the meter on the primary's block while the meter's control stands", async () => {
    const view = await setup({ store: await nominalStore() });
    expect(canvasOf("PRIMARY")).not.toHaveAccessibleDescription(describing("METER"));
    await chooseScene(view, "PHASE TEST");
    await toggleStyle(view);
    expect(canvasOf("PRIMARY")).toHaveAccessibleDescription(
      describing("EXPOSURE EV100 -1.0 MAN METER AVG"),
    );
  });

  it("states EASED CAMERA MOVES on the primary's block while the setting is on", async () => {
    const view = await setup();
    await view.user.click(screen.getByRole("button", { name: "EASED CAMERA MOVES" }));
    view.advance(300);
    expect(canvasOf("PRIMARY")).toHaveAccessibleDescription(describing("EASED CAMERA MOVES"));
  });

  it("does not state EASED CAMERA MOVES under reduced motion, where it is not applied", async () => {
    stubMatchMedia(true);
    const view = await setup();
    await view.user.click(screen.getByRole("button", { name: "EASED CAMERA MOVES" }));
    view.advance(300);
    expect(canvasOf("PRIMARY")).not.toHaveAccessibleDescription(describing("EASED CAMERA MOVES"));
  });
});

describe("VIEW's instruments at both sizes (R07.T19.b)", () => {
  it("states BODY PHOTOMETRY on a photorealistic instrument beside a wireframe primary, not beside a photorealistic one", async () => {
    const photometry = "BODY PHOTOMETRY: NOT YET MODELLED";
    const view = await setup({ store: await nominalStore() });
    await chooseScene(view, "PHASE TEST");
    await openInstrument(view, "INSTRUMENT 1");
    await controlsAt(view, "INSTRUMENT 1");
    await toggleStyle(view);
    expect(canvasOf("INSTRUMENT 1")).toHaveAccessibleDescription(describing(photometry));
    await controlsAt(view, "PRIMARY");
    await toggleStyle(view);
    expect(canvasOf("PRIMARY")).toHaveAccessibleDescription(describing(photometry));
    expect(canvasOf("INSTRUMENT 1")).not.toHaveAccessibleDescription(describing(photometry));
  });

  it("states BODY AND CRAFT PHOTOMETRY on a photorealistic instrument beside a wireframe primary, not beside a photorealistic one (R07.T16.e)", async () => {
    // PRECISION TEST: a planet and its moon, and the own ship.
    const photometry = "BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED";
    const view = await setup({ store: await nominalStore() });
    await chooseScene(view, "PRECISION TEST");
    await openInstrument(view, "INSTRUMENT 1");
    await controlsAt(view, "INSTRUMENT 1");
    await toggleStyle(view);
    expect(canvasOf("INSTRUMENT 1")).toHaveAccessibleDescription(describing(photometry));
    await controlsAt(view, "PRIMARY");
    await toggleStyle(view);
    expect(canvasOf("PRIMARY")).toHaveAccessibleDescription(describing(photometry));
    expect(canvasOf("INSTRUMENT 1")).not.toHaveAccessibleDescription(describing(photometry));
  });

  it("states the lighting on a photorealistic instrument beside a wireframe primary, not beside a photorealistic one", async () => {
    const lighting = "LIGHTING: NOT RECEIVED";
    const view = await setup({ store: await nominalStore() });
    await chooseScene(view, "PRECISION TEST");
    await openInstrument(view, "INSTRUMENT 1");
    await controlsAt(view, "INSTRUMENT 1");
    await toggleStyle(view);
    expect(canvasOf("INSTRUMENT 1")).toHaveAccessibleDescription(describing(lighting));
    await controlsAt(view, "PRIMARY");
    await toggleStyle(view);
    expect(canvasOf("PRIMARY")).toHaveAccessibleDescription(describing(lighting));
    expect(canvasOf("INSTRUMENT 1")).not.toHaveAccessibleDescription(describing(lighting));
  });

  it("keeps the slots in their reading order, INSTRUMENT 2 in its own slot opened alone", async () => {
    // The corners themselves are the stylesheet's (the slot's class); the captures measure them.
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 2");
    const alone = screen
      .getByRole("region", { name: "INSTRUMENT 2" })
      .classList.contains("view-instrument--slot-2");
    await openInstrument(view, "INSTRUMENT 1");
    expect([
      alone,
      screen
        .getAllByRole("region", { name: /^INSTRUMENT \d$/ })
        .map((slot) => within(slot).getByRole("heading", { level: 2 }).textContent),
    ]).toEqual([true, ["INSTRUMENT 1", "INSTRUMENT 2"]]);
  });
});

describe("VIEW's list without an own ship (R07.T19.b)", () => {
  it("heads its range column RANGE FROM CAMERA, its rows reading the bare range", async () => {
    const view = await setup();
    await chooseScene(view, "PHASE TEST");
    const targets = screen.getByRole("region", { name: "Targets PRIMARY" });
    const rows = within(targets).getAllByRole("option");
    expect(within(targets).getByText("FROM CAMERA").previousElementSibling).toHaveTextContent(
      "RANGE",
    );
    expect([
      rows.some((row) => row.textContent.includes("FROM CAMERA")),
      rows.every((row) => row.getAttribute("aria-label")?.includes("FROM CAMERA") === true),
    ]).toEqual([false, true]);
  });

  it("heads it RANGE alone where there is an own ship", async () => {
    await setup();
    const targets = screen.getByRole("region", { name: "Targets PRIMARY" });
    expect(within(targets).getByText("RANGE")).toBeInTheDocument();
    expect(within(targets).queryByText("FROM CAMERA")).not.toBeInTheDocument();
  });
});

describe("VIEW's SCENE selector at 150% interface scale (R10.T16's batch review, D-D)", () => {
  it("wraps its row between its buttons", () => {
    expect(STYLES).toMatch(/\.view-display__bar \.preset-buttons \{[^}]*flex-wrap: wrap;/u);
  });

  it("keeps each button's label on one line", () => {
    expect(STYLES).toMatch(
      /\.view-display__bar \.preset-buttons__button \{[^}]*white-space: nowrap;/u,
    );
  });
});

/** The primary's upper plate on the stage. */
function upperPlate(): Element {
  const plate = document.querySelector(".view__overlay > .view-label");
  if (plate === null) {
    throw new Error("the stage shows no upper plate");
  }
  return plate;
}

async function closeInstrument(view: ViewDisplayHarness, name: string): Promise<void> {
  await view.user.click(
    within(screen.getByRole("group", { name })).getByRole("button", { name: "CLOSE" }),
  );
  view.advance(300);
}

function controlsButton(name: string): HTMLElement {
  return within(screen.getByRole("group", { name: "CONTROLS" })).getByRole("button", { name });
}

/** The `CONTROLS` view, by its pressed button's name. */
function controlsView(): string | null {
  return (
    within(screen.getByRole("group", { name: "CONTROLS" }))
      .getAllByRole("button")
      .find((button) => button.getAttribute("aria-pressed") === "true")?.textContent ?? null
  );
}

/** The target selected on the `Targets` list of the view `name`, by its row's name, or `null`. */
function selectedTarget(name: string): string | null {
  const [row] = within(screen.getByRole("region", { name: `Targets ${name}` })).queryAllByRole(
    "option",
    { selected: true },
  );
  return row?.getAttribute("aria-label") ?? null;
}

/** Sets the interface scale's rem, as the scale setting does, and fires the window's resize. */
function scaleTo(view: ViewDisplayHarness, remPx: number): void {
  act(() => {
    document.documentElement.style.fontSize = `${String(remPx)}px`;
    window.dispatchEvent(new Event("resize"));
  });
  view.advance(100);
}

describe("VIEW's room for its slots and plates (R10.T12.b)", () => {
  /** A stage with room for INSTRUMENT 1 beside the plates, but not for INSTRUMENT 2 below it. */
  const ONE_SLOT_HIGH = { widthPx: 1000, heightPx: 400 } as const;

  /** A stage too narrow for any slot beside the plates' least width at 16 px to the rem. */
  const NARROW = { widthPx: 800, heightPx: 720 } as const;

  afterEach(() => {
    document.documentElement.style.fontSize = "";
  });

  /** Both slots opened on the full stage, which then has room for INSTRUMENT 1 alone. */
  async function withStub(): Promise<ViewDisplayHarness> {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    await openInstrument(view, "INSTRUMENT 2");
    view.resizeStage(ONE_SLOT_HIGH);
    view.advance(100);
    return view;
  }

  /** INSTRUMENT 1 opened, made the CONTROLS view, and its camera set to SEAT. */
  async function instrumentAtSeat(view: ViewDisplayHarness): Promise<void> {
    await openInstrument(view, "INSTRUMENT 1");
    await controlsAt(view, "INSTRUMENT 1");
    await view.user.keyboard("1");
    view.advance(300);
  }

  describe("INSTRUMENT 2's stub", () => {
    it("stands INSTRUMENT 2 as its stub, with its CLOSE, where the stage holds INSTRUMENT 1 alone", async () => {
      await withStub();
      expect(
        within(screen.getByRole("region", { name: "INSTRUMENT 2" })).getByRole("button", {
          name: "CLOSE",
        }),
      ).toBeInTheDocument();
    });

    it("states in the stub why INSTRUMENT 2 is not drawn", async () => {
      await withStub();
      expect(describedText(screen.getByRole("region", { name: "INSTRUMENT 2" }))).toBe(STUB_REASON);
    });

    it("draws no INSTRUMENT 2 canvas while it stands as its stub", async () => {
      await withStub();
      expect(screen.queryByRole("application", { name: /INSTRUMENT 2/ })).toBeNull();
    });

    it("keeps INSTRUMENT 1 drawn beside the stub", async () => {
      await withStub();
      expect(canvasOf("INSTRUMENT 1")).toBeInTheDocument();
    });

    it("closes INSTRUMENT 2 from its stub's CLOSE", async () => {
      const view = await withStub();
      await view.user.click(
        within(screen.getByRole("region", { name: "INSTRUMENT 2" })).getByRole("button", {
          name: "CLOSE",
        }),
      );
      view.advance(100);
      expect(
        within(screen.getByRole("group", { name: "INSTRUMENT 2" })).getByRole("button", {
          name: "CLOSE",
        }),
      ).toHaveAttribute("aria-pressed", "true");
    });
  });

  describe("a slot held on a change of interface scale", () => {
    /** INSTRUMENT 1 at SEAT on a stage that holds it at 100% but not at 150%, then at 150%. */
    async function heldAt150(): Promise<ViewDisplayHarness> {
      const view = await setup({ stagePx: { widthPx: 1000, heightPx: 720 } });
      await instrumentAtSeat(view);
      // The focus leaves the INSTRUMENTS panel, which the compact layout then folds.
      act(() => {
        canvasOf("PRIMARY").focus();
      });
      scaleTo(view, 24);
      return view;
    }

    it("states the held slot under the folded INSTRUMENTS row at 150%", async () => {
      await heldAt150();
      expect(
        screen.getByText(
          statusLine("NOT AVAILABLE: no room for INSTRUMENT 1 at this window size"),
          { selector: ".view-instruments-panel__standing" },
        ),
      ).toBeInTheDocument();
    });

    it("draws no canvas for the held slot", async () => {
      await heldAt150();
      expect(screen.queryByRole("application", { name: /INSTRUMENT 1/ })).toBeNull();
    });

    it("returns the slot at 100% with its camera at SEAT and its style as they were", async () => {
      const view = await setup({
        store: await nominalStore(),
        stagePx: { widthPx: 1000, heightPx: 720 },
      });
      await instrumentAtSeat(view);
      await toggleStyle(view);
      act(() => {
        canvasOf("PRIMARY").focus();
      });
      scaleTo(view, 24);
      scaleTo(view, 16);
      await settle();
      view.advance(300);
      expect(canvasOf("INSTRUMENT 1")).toHaveAccessibleName(
        "VIEW, PHOTOREALISTIC, INSTRUMENT 1, SEAT",
      );
    });

    it("returns INSTRUMENT 2 whole at 100% where both slots stood with less than a line to spare", async () => {
      // Both slots stand in 532 of the stage's 541 px at 100%; both are held at 150%.
      const view = await setup({ stagePx: { widthPx: 1000, heightPx: 541 } });
      await openInstrument(view, "INSTRUMENT 1");
      await openInstrument(view, "INSTRUMENT 2");
      act(() => {
        canvasOf("PRIMARY").focus();
      });
      scaleTo(view, 24);
      scaleTo(view, 16);
      expect(canvasOf("INSTRUMENT 2")).toBeInTheDocument();
    });

    it("returns the slot at 100% with its target still selected", async () => {
      const view = await setup({ stagePx: { widthPx: 1000, heightPx: 720 } });
      await openInstrument(view, "INSTRUMENT 1");
      await controlsAt(view, "INSTRUMENT 1");
      const rows = within(
        screen.getByRole("region", { name: "Targets INSTRUMENT 1" }),
      ).getAllByRole("option");
      await view.user.click(rows.at(-1) ?? document.body);
      view.advance(100);
      const chosen = selectedTarget("INSTRUMENT 1");
      act(() => {
        canvasOf("PRIMARY").focus();
      });
      scaleTo(view, 24);
      scaleTo(view, 16);
      view.advance(300);
      expect([chosen !== null, selectedTarget("INSTRUMENT 1")]).toEqual([true, chosen]);
    });

    it("lets the upper plate take the stage's full width at 150%", async () => {
      await heldAt150();
      expect(upperPlate()).not.toHaveClass("view-label--beside");
    });

    it("stands the upper plate whole at 150%, with no scroll", async () => {
      await heldAt150();
      expect([
        screen.queryByRole("region", { name: "PRIMARY lines" }),
        screen.queryByRole("region", { name: "PRIMARY label block" }),
      ]).toEqual([null, null]);
    });
  });

  describe("the focus over the stage as the room changes", () => {
    it("gives the PRIMARY canvas the focus once the instrument canvas that held it is held", async () => {
      const view = await setup({ stagePx: { widthPx: 1000, heightPx: 720 } });
      await openInstrument(view, "INSTRUMENT 1");
      act(() => {
        canvasOf("INSTRUMENT 1").focus();
      });
      scaleTo(view, 24);
      expect(canvasOf("PRIMARY")).toHaveFocus();
    });

    it("gives the PRIMARY canvas the focus once the label block scroll that held it stops scrolling", async () => {
      // The plate laid out taller than the 704 px it has: everything under VIEW scrolls.
      const block = { leftPx: 0, topPx: 0, widthPx: 300, heightPx: 800 };
      const view = await setup({ blockPx: () => block });
      view.advance(300);
      act(() => {
        screen.getByRole("region", { name: "PRIMARY label block" }).focus();
      });
      block.heightPx = 100;
      view.advance(300);
      expect(canvasOf("PRIMARY")).toHaveFocus();
    });

    it("leaves the focus on the label block scroll while it still scrolls", async () => {
      const block = { leftPx: 0, topPx: 0, widthPx: 300, heightPx: 800 };
      const view = await setup({ blockPx: () => block });
      view.advance(300);
      const scroll = screen.getByRole("region", { name: "PRIMARY label block" });
      act(() => {
        scroll.focus();
      });
      view.advance(300);
      expect(scroll).toHaveFocus();
    });
  });

  describe("the plates beside the slots", () => {
    it("lets the upper plate take the stage's full width with no slot open", async () => {
      await setup();
      expect(upperPlate()).not.toHaveClass("view-label--beside");
    });

    it("sets the upper plate beside INSTRUMENT 1 while it stands", async () => {
      const view = await setup();
      await openInstrument(view, "INSTRUMENT 1");
      expect(upperPlate()).toHaveClass("view-label--beside");
    });

    it("keeps the upper plate beside the slots while INSTRUMENT 2 stands as its stub", async () => {
      await withStub();
      expect(upperPlate()).toHaveClass("view-label--beside");
    });

    it("lets the upper plate take the full width once INSTRUMENT 1 is held", async () => {
      const view = await setup();
      await openInstrument(view, "INSTRUMENT 1");
      view.resizeStage(NARROW);
      view.advance(100);
      expect(upperPlate()).not.toHaveClass("view-label--beside");
    });

    it("lets the upper plate take the full width once INSTRUMENT 1 closes", async () => {
      const view = await setup();
      await openInstrument(view, "INSTRUMENT 1");
      await closeInstrument(view, "INSTRUMENT 1");
      expect(upperPlate()).not.toHaveClass("view-label--beside");
    });
  });

  describe("CONTROLS while its instrument is held", () => {
    /** INSTRUMENT 1 opened and made the CONTROLS view, then held on a narrow stage. */
    async function heldControls(): Promise<ViewDisplayHarness> {
      const view = await setup();
      await openInstrument(view, "INSTRUMENT 1");
      await controlsAt(view, "INSTRUMENT 1");
      view.resizeStage(NARROW);
      view.advance(100);
      return view;
    }

    it("returns CONTROLS to PRIMARY", async () => {
      await heldControls();
      expect(controlsView()).toBe("PRIMARY");
    });

    it("holds the held instrument's CONTROLS button back", async () => {
      await heldControls();
      expect(controlsButton("INSTRUMENT 1")).toHaveAttribute("aria-disabled", "true");
    });

    it("gives CONTROLS back to the instrument as it returns", async () => {
      const view = await heldControls();
      view.resizeStage({ widthPx: 1280, heightPx: 720 });
      view.advance(100);
      expect(controlsView()).toBe("INSTRUMENT 1");
    });
  });
});

describe("VIEW's room in the stylesheet (R10.T12.b)", () => {
  it("narrows the plates beside the slots by a slot's width and its three insets", () => {
    expect(STYLES).toMatch(/\.view-label--beside \{\s*max-width: calc\(100% - 36\.5rem\);\s*\}/u);
  });

  it("lets a plate's scroll take the pointer, which the overlay passes through", () => {
    expect(STYLES).toMatch(/\.view-label__scroll \{[^}]*pointer-events: auto;/u);
  });
});
