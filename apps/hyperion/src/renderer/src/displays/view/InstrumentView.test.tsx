import { act, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { binaryFrame } from "../../test/binaryFrames";
import { InThreadSkyWorker, skyPayload, skyResponse } from "../../test/skyFixtures";
import {
  nominalStore,
  openUniverse,
  renderViewDisplay,
  sceneArrives,
  settle,
  STAGE_HEIGHT_PX,
  STAGE_WIDTH_PX,
  submittedBy,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { ONE_PHOTOREALISTIC_VIEW } from "../../view/budget/viewBudget";
import { EngineUnavailable } from "../../view/engine/resilientEngine";
import { GraphicsStatusStore, initialGraphicsStatus } from "../../view/engine/status";
import type {
  MaterialHandle,
  RenderView,
  ViewSize,
  WgslMaterialSpec,
} from "../../view/engine/types";
import { SKY_PASS_LABEL } from "../../view/photoreal/passes";
import { HISTOGRAM_PASS } from "../../view/post/histogram";
import { TONEMAP_PASS } from "../../view/post/tonemap";
import type { QualitySetting } from "../../view/quality/qualitySetting";
import type { ViewEngineSource } from "./useViewEngine";

afterEach(() => {
  vi.useRealTimers();
});

const FULL: ViewSize = { widthPx: STAGE_WIDTH_PX, heightPx: STAGE_HEIGHT_PX };

/** The camera controls a view's `Camera` panel offers, by their accessible names. */
const CAMERA_CONTROLS = [
  "1 SEAT",
  "2 CHASE",
  "3 FREE",
  "[ PREVIOUS TARGET",
  "] NEXT TARGET",
  "Narrower field of view",
  "Wider field of view",
  "EASED CAMERA MOVES",
];

interface Setup extends ViewDisplayHarness {
  readonly fake: ReturnType<typeof timedEngineSource>;
}

async function setup(
  options: {
    readonly store?: GraphicsStatusStore;
    readonly setting?: QualitySetting;
    readonly stagePx?: { readonly widthPx: number; readonly heightPx: number };
  } = {},
): Promise<Setup> {
  const fake = timedEngineSource();
  const harness = renderViewDisplay({
    store: options.store ?? new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
    source: fake.source,
    engines: fake.engines,
    ...(options.setting === undefined ? {} : { setting: options.setting }),
    ...(options.stagePx === undefined ? {} : { stagePx: options.stagePx }),
  });
  await settle();
  harness.advance(100);
  return { ...harness, fake };
}

async function openInstrument({ user, advance }: Setup, name: string): Promise<void> {
  await user.click(
    within(screen.getByRole("group", { name })).getByRole("button", { name: "OPEN" }),
  );
  advance(300);
}

async function closeInstrument({ user, advance }: Setup, name: string): Promise<void> {
  await user.click(
    within(screen.getByRole("group", { name })).getByRole("button", { name: "CLOSE" }),
  );
  advance(300);
}

function controlsButton(name: string): HTMLElement {
  return within(screen.getByRole("group", { name: "CONTROLS" })).getByRole("button", { name });
}

/**
 * Switches the `CONTROLS` view, the primary until another is chosen, to the photorealistic style by
 * its key and lets its pipelines be made.
 */
async function photorealControls({ user, advance }: Setup): Promise<void> {
  await user.keyboard("4");
  advance(100);
  await settle();
  advance(300);
}

/** The last size the primary's scene target was given. */
function sceneTargetSize(view: Setup): ViewSize | undefined {
  return view.fake.targets.findLast((target) => target.spec.name === "view:hdr")?.sizes.at(-1);
}

/** Runs frames, reporting each resolve's pass times at `costMs` of its view or target. */
function runFrames(view: Setup, frames: number, costMs: (name: string) => number): void {
  for (let frame = 0; frame < frames; frame += 1) {
    view.advance(16);
    view.fake.deliver(costMs);
  }
}

/** The frames a view of `name` has drawn so far. */
function framesOf(view: Setup, name: string): number {
  return view.views().find((each) => each.name === name)?.frames.length ?? 0;
}

/** The names of the controls Tab reaches from where the focus is, among `names`. */
async function tabbedTo(view: Setup, names: ReadonlyArray<string>): Promise<string[]> {
  const reached = new Set<string>();
  for (let i = 0; i < 40 && reached.size < names.length; i += 1) {
    // Each Tab moves on from where the last left the focus, so they cannot run together.
    // oxlint-disable-next-line no-await-in-loop
    await view.user.tab();
    const focused = document.activeElement;
    const name = focused?.getAttribute("aria-label") ?? focused?.textContent ?? "";
    if (names.includes(name)) {
      reached.add(name);
    }
  }
  return [...reached];
}

/** The `CONTROLS` view, by its pressed button's name. */
function controlsView(): string {
  return (
    within(screen.getByRole("group", { name: "CONTROLS" }))
      .getAllByRole("button")
      .find((button) => button.getAttribute("aria-pressed") === "true")?.textContent ?? ""
  );
}

/**
 * A source of the timed engines whose material creations refuse, as during a device loss, while
 * `lose(true)` holds.
 */
function lossySource(fake: ReturnType<typeof timedEngineSource>): {
  readonly source: ViewEngineSource;
  readonly lose: (lost: boolean) => void;
} {
  let lost = false;
  return {
    source: {
      ...fake.source,
      load: async (outcome, status) => {
        const engine = await fake.source.load(outcome, status);
        const createMaterial = engine.createMaterial.bind(engine);
        return Object.assign(engine, {
          createMaterial: (spec: WgslMaterialSpec): MaterialHandle => {
            if (lost) {
              throw new EngineUnavailable("createMaterial");
            }
            return createMaterial(spec);
          },
        });
      },
    },
    lose: (next) => {
      lost = next;
    },
  };
}

/**
 * The passes of the last frame the view `name` submitted, from its scene target's sky pass on:
 * each pass's label and its draws' materials.
 */
function lastFramePasses(view: Setup, name: string): Array<[string, ReadonlyArray<string>]> {
  const passes = view.fake.submissions.filter((each) => submittedBy(each) === name);
  const start = passes.findLastIndex((each) => each.label === SKY_PASS_LABEL);
  return passes
    .slice(Math.max(0, start))
    .map((each): [string, ReadonlyArray<string>] => [each.label, each.materials]);
}

/** Each canvas's accessible name, the primary's first. */
function canvasNames(): Array<string | null> {
  return screen.getAllByRole("application").map((canvas) => canvas.getAttribute("aria-label"));
}

function canvasNamed(name: RegExp): HTMLElement {
  return screen.getByRole("application", { name });
}

describe("VIEW's instrument views (R07.T19)", () => {
  it("names each view's canvas for its style, its slot and its camera", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    await openInstrument(view, "INSTRUMENT 2");
    expect(canvasNames()).toEqual([
      "VIEW, WIREFRAME, PRIMARY, SEAT",
      "VIEW, WIREFRAME, INSTRUMENT 1, CHASE",
      "VIEW, WIREFRAME, INSTRUMENT 2, CHASE",
    ]);
  });

  it("reaches each open instrument's canvas by Tab from the primary's", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    await openInstrument(view, "INSTRUMENT 2");
    canvasNamed(/PRIMARY/).focus();
    await view.user.tab();
    const first = document.activeElement?.getAttribute("aria-label");
    await view.user.tab();
    expect([first, document.activeElement?.getAttribute("aria-label")]).toEqual([
      "VIEW, WIREFRAME, INSTRUMENT 1, CHASE",
      "VIEW, WIREFRAME, INSTRUMENT 2, CHASE",
    ]);
  });

  it("reaches every camera control of every view from the keyboard", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    await openInstrument(view, "INSTRUMENT 2");
    const reached: string[][] = [];
    for (const name of ["PRIMARY", "INSTRUMENT 1", "INSTRUMENT 2"]) {
      controlsButton(name).focus();
      // Each view is chosen, then its controls reached, in turn.
      // oxlint-disable-next-line no-await-in-loop
      await view.user.keyboard("{Enter}");
      view.advance(100);
      // oxlint-disable-next-line no-await-in-loop
      reached.push(await tabbedTo(view, CAMERA_CONTROLS));
    }
    expect(reached).toEqual([CAMERA_CONTROLS, CAMERA_CONTROLS, CAMERA_CONTROLS]);
  });

  it("holds a second view's photorealistic style back on low with its reason", async () => {
    const view = await setup({ store: await nominalStore(), setting: "low" });
    await photorealControls(view);
    await openInstrument(view, "INSTRUMENT 1");
    await view.user.click(controlsButton("INSTRUMENT 1"));
    view.advance(100);
    expect(
      within(screen.getByRole("region", { name: "Style INSTRUMENT 1" })).getByRole("button", {
        name: "PHOTOREALISTIC",
      }),
    ).toHaveAccessibleDescription(ONE_PHOTOREALISTIC_VIEW);
  });

  it("refuses a second view's key 4 on low, the CONTROLS instrument staying a wireframe", async () => {
    const view = await setup({ store: await nominalStore(), setting: "low" });
    await photorealControls(view);
    await openInstrument(view, "INSTRUMENT 1");
    await view.user.click(controlsButton("INSTRUMENT 1"));
    view.advance(100);
    // Off a canvas the key acts on the CONTROLS view, the instrument.
    await view.user.keyboard("4");
    view.advance(300);
    expect(canvasNames()).toEqual([
      "VIEW, PHOTOREALISTIC, PRIMARY, SEAT",
      "VIEW, WIREFRAME, INSTRUMENT 1, CHASE",
    ]);
  });

  it("shows the primary's exposure on a wireframe instrument, with its source and its stars", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    expect(screen.getByRole("region", { name: "INSTRUMENT 1" }).textContent).toMatch(
      /STYLE\s*WIREFRAME.*EXPOSURE\s*EV100 -1\.0 MAN\s*SOURCE\s*PRIMARY\s*STARS\s*RANGE QUERY/,
    );
  });

  it("makes a focused instrument the CONTROLS view when a key is pressed on it, and acts on it", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    canvasNamed(/INSTRUMENT 1/).focus();
    await view.user.keyboard("3");
    view.advance(300);
    expect([controlsView(), canvasNames()]).toEqual([
      "INSTRUMENT 1",
      ["VIEW, WIREFRAME, PRIMARY, SEAT", "VIEW, WIREFRAME, INSTRUMENT 1, FREE"],
    ]);
  });

  it("makes an instrument the CONTROLS view when its canvas is pressed", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    await view.user.click(canvasNamed(/INSTRUMENT 1/));
    expect(controlsView()).toBe("INSTRUMENT 1");
  });

  it("leaves CONTROLS as it was when Tab passes the canvases", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    await openInstrument(view, "INSTRUMENT 2");
    canvasNamed(/PRIMARY/).focus();
    for (let step = 0; step < 4; step += 1) {
      // Each Tab moves on from where the last left the focus.
      // oxlint-disable-next-line no-await-in-loop
      await view.user.tab();
    }
    expect(controlsView()).toBe("PRIMARY");
  });

  it("holds a closed instrument's CONTROLS button back with its reason", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    expect(controlsButton("INSTRUMENT 2")).toHaveAccessibleDescription(
      "NOT AVAILABLE: INSTRUMENT 2 is not open",
    );
  });

  it("gives both closed instruments' CONTROLS buttons one reason", async () => {
    await setup();
    expect(controlsButton("INSTRUMENT 1")).toHaveAccessibleDescription(
      "NOT AVAILABLE: INSTRUMENT 1 and INSTRUMENT 2 are not open",
    );
  });

  it("designates the target, camera and style panels with the CONTROLS view", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    await view.user.click(controlsButton("INSTRUMENT 1"));
    view.advance(100);
    expect([
      screen.getByRole("heading", { name: "Targets INSTRUMENT 1" }),
      screen.getByRole("region", { name: "Camera INSTRUMENT 1" }),
      screen.getByRole("region", { name: "Style INSTRUMENT 1" }),
    ]).toEqual([expect.anything(), expect.anything(), expect.anything()]);
  });

  it("designates the exposure with PRIMARY whichever view CONTROLS points at", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    await view.user.click(controlsButton("INSTRUMENT 1"));
    view.advance(100);
    expect(screen.getByRole("region", { name: "Exposure PRIMARY" })).toBeInTheDocument();
  });

  it("returns CONTROLS to PRIMARY when the controlled instrument closes", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    await view.user.click(controlsButton("INSTRUMENT 1"));
    await closeInstrument(view, "INSTRUMENT 1");
    expect(controlsView()).toBe("PRIMARY");
  });

  it("holds every OPEN back while no view can be drawn, saying so", async () => {
    await setup({ store: new GraphicsStatusStore(initialGraphicsStatus("safe", false)) });
    const open = within(screen.getByRole("group", { name: "INSTRUMENT 1" })).getByRole("button", {
      name: "OPEN",
    });
    expect(open).toHaveAccessibleDescription("NOT AVAILABLE: no view can be drawn");
  });

  it("draws an instrument opened during a device loss once the engine is restored", async () => {
    const fake = timedEngineSource();
    const lossy = lossySource(fake);
    const view = renderViewDisplay({
      store: new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
      source: lossy.source,
      engines: fake.engines,
    });
    await settle();
    view.advance(100);
    // The device is lost as the instrument opens: its view is made, its renderers are not.
    lossy.lose(true);
    await view.user.click(
      within(screen.getByRole("group", { name: "INSTRUMENT 1" })).getByRole("button", {
        name: "OPEN",
      }),
    );
    view.advance(100);
    lossy.lose(false);
    act(() => {
      fake.engines.at(-1)?.raiseRestored();
    });
    await settle();
    view.advance(300);
    const instruments = view.views().filter((each) => each.name === "instrument-1");
    expect([instruments.length, (instruments[0]?.frames.length ?? 0) > 0]).toEqual([1, true]);
  });

  it("says where the engine refuses an instrument's canvas", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const fake = timedEngineSource();
    const source: ViewEngineSource = {
      ...fake.source,
      load: async (outcome, status) => {
        const engine = await fake.source.load(outcome, status);
        const createView = engine.createView.bind(engine);
        return Object.assign(engine, {
          createView: (canvas: HTMLCanvasElement, name: string): RenderView => {
            if (name === "instrument-1") {
              throw new Error("no context");
            }
            return createView(canvas, name);
          },
        });
      },
    };
    const view = renderViewDisplay({
      store: new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
      source,
      engines: fake.engines,
    });
    await settle();
    view.advance(100);
    await view.user.click(
      within(screen.getByRole("group", { name: "INSTRUMENT 1" })).getByRole("button", {
        name: "OPEN",
      }),
    );
    view.advance(100);
    expect(
      within(screen.getByRole("region", { name: "INSTRUMENT 1" })).getByText(
        /^GRAPHICS NOT AVAILABLE: views could not be made/,
      ),
    ).toBeInTheDocument();
  });

  it("holds a slot's OPEN back where the stage has no room for it", async () => {
    const view = await setup({ stagePx: { widthPx: 1280, heightPx: 400 } });
    await openInstrument(view, "INSTRUMENT 1");
    const open = within(screen.getByRole("group", { name: "INSTRUMENT 2" })).getByRole("button", {
      name: "OPEN",
    });
    expect(open).toHaveAccessibleDescription(
      "NOT AVAILABLE: no room for INSTRUMENT 2 at this window size",
    );
  });
});

describe("VIEW's budgets against a fake engine (R07.T19)", () => {
  it("sizes a photorealistic primary's scene target from its controller's scale once an instrument opens", async () => {
    const view = await setup({ store: await nominalStore(["timestamp-query"]) });
    await photorealControls(view);
    const before = sceneTargetSize(view);
    await openInstrument(view, "INSTRUMENT 1");
    // Every frame twice its budget: the controller lowers the scale.
    runFrames(view, 40, () => 5);
    const after = sceneTargetSize(view);
    expect([
      before,
      (after?.widthPx ?? 0) < FULL.widthPx,
      (after?.widthPx ?? 0) >= FULL.widthPx / 2,
    ]).toEqual([FULL, true, true]);
  });

  it("returns the scene target to the bounds' max once the instruments close", async () => {
    const view = await setup({ store: await nominalStore(["timestamp-query"]) });
    await photorealControls(view);
    await openInstrument(view, "INSTRUMENT 1");
    runFrames(view, 40, () => 5);
    const lowered = sceneTargetSize(view);
    await closeInstrument(view, "INSTRUMENT 1");
    runFrames(view, 10, () => 5);
    expect([(lowered?.widthPx ?? 0) < FULL.widthPx, sceneTargetSize(view)]).toEqual([true, FULL]);
  });

  it("counts an instrument's pass times in the primary's frame", async () => {
    const view = await setup({ store: await nominalStore(["timestamp-query"]) });
    await photorealControls(view);
    await openInstrument(view, "INSTRUMENT 1");
    // The primary's own passes are cheap; the instrument's alone take the frame over its budget.
    runFrames(view, 40, (name) => (name.startsWith("instrument-") ? 30 : 0.01));
    expect((sceneTargetSize(view)?.widthPx ?? FULL.widthPx) < FULL.widthPx).toBe(true);
  });

  it("feeds the controller the frame interval, not pass times, while the timer is absent", async () => {
    const view = await setup({ store: await nominalStore() });
    await photorealControls(view);
    await openInstrument(view, "INSTRUMENT 1");
    // Times arriving under an absent timer are not read; the fake frames meet every vsync.
    runFrames(view, 40, () => 30);
    expect(sceneTargetSize(view)).toEqual(FULL);
  });

  it("draws a 30 Hz instrument in every second frame of a 60 Hz primary", async () => {
    const view = await setup();
    await openInstrument(view, "INSTRUMENT 1");
    const primary = framesOf(view, "view");
    const instrument = framesOf(view, "instrument-1");
    view.advance(16 * 20);
    expect([framesOf(view, "view") - primary, framesOf(view, "instrument-1") - instrument]).toEqual(
      [20, 10],
    );
  });

  it("draws a 30 Hz photorealistic primary in every second animation frame on low", async () => {
    const view = await setup({ store: await nominalStore(), setting: "low" });
    await photorealControls(view);
    const tonemapped = (): number =>
      view
        .views()
        .find((each) => each.name === "view")
        ?.frames.filter((frame) => frame.label === TONEMAP_PASS).length ?? 0;
    const before = tonemapped();
    view.advance(16 * 20);
    expect(tonemapped() - before).toBe(10);
  });
});

describe("VIEW's one frame path (R07.T19.c)", () => {
  it("draws an instrument's frame in the primary's passes, but for the histogram", async () => {
    const view = await setup({ store: await nominalStore() });
    // Both views at CHASE, photorealistic, on stages of one size.
    await view.user.keyboard("2");
    await photorealControls(view);
    await openInstrument(view, "INSTRUMENT 1");
    await view.user.click(controlsButton("INSTRUMENT 1"));
    await photorealControls(view);
    // The histograms in flight are read back, so that the primary's next frames take theirs.
    await settle();
    view.advance(16 * 2);
    const primary = lastFramePasses(view, "view");
    expect([
      primary.some(([label]) => label === HISTOGRAM_PASS),
      lastFramePasses(view, "instrument-1"),
    ]).toEqual([true, primary.filter(([label]) => label !== HISTOGRAM_PASS)]);
  });

  it("takes no histogram on a photorealistic instrument", async () => {
    const view = await setup({ store: await nominalStore() });
    await openInstrument(view, "INSTRUMENT 1");
    await view.user.click(controlsButton("INSTRUMENT 1"));
    await photorealControls(view);
    view.advance(16 * 10);
    const labels = new Set(
      view.fake.submissions
        .filter((each) => submittedBy(each) === "instrument-1")
        .map((each) => each.label),
    );
    expect([labels.has(TONEMAP_PASS), labels.has(HISTOGRAM_PASS)]).toEqual([true, false]);
  });

  it("disposes of a primary's view waiting for a restore when its stage goes", async () => {
    const fake = timedEngineSource();
    const lossy = lossySource(fake);
    const view = renderViewDisplay({
      store: new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
      source: lossy.source,
      engines: fake.engines,
    });
    await settle();
    view.advance(100);
    lossy.lose(true);
    await view.user.click(screen.getByRole("button", { name: "PHASE TEST" }));
    view.advance(100);
    const waiting = view.views().filter((each) => each.name === "view")[1];
    // Another scene remounts the stage before the restore.
    await view.user.click(screen.getByRole("button", { name: "FRAME CHANGE TEST" }));
    view.advance(100);
    expect(waiting?.disposed).toBe(true);
  });

  it("makes the primary's renderers at the restore where its stage mounts during a device loss", async () => {
    const fake = timedEngineSource();
    const lossy = lossySource(fake);
    const view = renderViewDisplay({
      store: new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
      source: lossy.source,
      engines: fake.engines,
    });
    await settle();
    view.advance(100);
    // A new scene remounts the stage while the device is lost: its view is made, its renderers
    // are not.
    lossy.lose(true);
    await view.user.click(screen.getByRole("button", { name: "PHASE TEST" }));
    view.advance(100);
    const remounted = view.views().filter((each) => each.name === "view");
    const waiting = remounted[1]?.frames.length;
    lossy.lose(false);
    act(() => {
      fake.engines.at(-1)?.raiseRestored();
    });
    await settle();
    view.advance(300);
    expect([remounted.length, waiting, (remounted[1]?.frames.length ?? 0) > 0]).toEqual([
      2,
      0,
      true,
    ]);
  });
});

/** Answers the report in flight, so that the next goes out. */
async function answerReport(view: Setup): Promise<void> {
  await act(async () => {
    view.socket.serverAnswers("scene_cameras", () => ({ kind: "scene_cameras" }));
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** The cameras the next report names, once the one in flight is answered (one at a time). */
async function nextReport(view: Setup): Promise<number | undefined> {
  await answerReport(view);
  view.advance(300);
  await settle();
  return view.socket.requestsOfKind("scene_cameras").at(-1)?.body.cameras.length;
}

async function serverScene(): Promise<Setup> {
  const view = await setup();
  await openUniverse(view);
  await sceneArrives(view);
  await settle();
  view.advance(300);
  await settle();
  await answerReport(view);
  return view;
}

/** Answers the primary's sky request with one bright star, and lets the decode settle. */
async function skyArrives(view: Setup): Promise<void> {
  const sky = view.socket.requestsOfKind("sky").at(-1);
  if (sky === undefined) {
    throw new Error("the view asks no sky");
  }
  const payload = skyPayload([{ direction: [0, 0, -1], distanceLy: 100, vMag: 1 }], 2, 7.4);
  await act(async () => {
    view.socket.serverSendsBinary(binaryFrame(sky.id, 0, 1, [...payload]));
    view.socket.serverResponds(sky.id, { kind: "sky", ...skyResponse(sky.body, payload, 1, 2) });
    await vi.advanceTimersByTimeAsync(0);
  });
  view.advance(300);
  await settle();
}

/** An instrument's `STARS` reading on its label block. */
function instrumentStars(name: string): string {
  return (
    /STARS\s*(V -?\d+\.\d mag CAM)/.exec(screen.getByRole("region", { name }).textContent)?.[1] ??
    ""
  );
}

describe("VIEW's instruments in the server's scene (R07.T19)", () => {
  it("reports an open instrument's camera beside the primary's", async () => {
    const view = await serverScene();
    await openInstrument(view, "INSTRUMENT 1");
    expect(await nextReport(view)).toBe(2);
  });

  it("states an instrument's camera limit at the primary's exposure (R07.T13.e)", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const view = await serverScene();
    await skyArrives(view);
    await openInstrument(view, "INSTRUMENT 1");
    const atDefault = instrumentStars("INSTRUMENT 1");
    await view.user.click(screen.getByRole("textbox", { name: "MAN" }));
    await view.user.keyboard("15{Enter}");
    view.advance(300);
    expect([atDefault, instrumentStars("INSTRUMENT 1")]).toEqual([
      "V 10.1 mag CAM",
      "V 2.6 mag CAM",
    ]);
  });

  it("withdraws an instrument's camera when it closes", async () => {
    const view = await serverScene();
    await openInstrument(view, "INSTRUMENT 1");
    await nextReport(view);
    await closeInstrument(view, "INSTRUMENT 1");
    expect(await nextReport(view)).toBe(1);
  });
});
