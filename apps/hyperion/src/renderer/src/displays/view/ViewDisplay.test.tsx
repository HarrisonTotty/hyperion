import type { ResponseBody, SystemIdHex } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Activity } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { norm, vec3 } from "../../geometry/vec3";
import { readTokens } from "../../spatial/paint";
import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import type { FakeView } from "../../test/fakeRenderEngine";
import { FakeResizeObserver } from "../../test/FakeResizeObserver";
import { binaryFrame } from "../../test/binaryFrames";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { InThreadSkyWorker, skyPayload, skyResponse } from "../../test/skyFixtures";
import { FIXTURE_SYSTEM } from "../../test/planetaryFixture";
import {
  anOpenedUniverse,
  aStellarBrief,
  aSystemsInRange,
  aUniverseList,
} from "../../test/galaxyFixtures";
import {
  SCENE_DESIGNATION,
  SCENE_TIDAL_RADIUS_M,
  sceneClock,
  scenePlace,
  shipInSystem,
  sliceSceneSystem,
} from "../../test/sceneFixture";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { FULL_VIEW_PX, renderViewDisplay, stubViewLayout } from "../../test/viewDisplayHarness";
import { UniverseProvider } from "../../components/UniverseProvider";
import { UniversePanel } from "../galaxy/UniversePanel";
import { rotate } from "../../view/camera/quaternion";
import { type FakeMeteredImage, fakeViewEngineSource } from "../../test/fakeViewEngine";
import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { requestAdapterOutcome } from "../../view/engine/platform";
import { stubMatchMedia } from "../../test/stubMatchMedia";
import { DEFAULT_FOV_DEG } from "../../view/camera/projection";
import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../../view/engine/status";
import type { FrameSubmission } from "../../view/engine/types";
import { METER_CLASS } from "../../view/post/meter";
import { precisionScene } from "../../view/scenes/precision";
import STYLES from "../../styles.css?raw";
import { lineScale } from "../../lib/strokes";
import { buildWireframeDrawList, CASING_PX, viewStrokesAt } from "../../view/wireframe/drawList";
import { linearColour } from "../../view/wireframe/submit";
import type { ViewEngineSource } from "./useViewEngine";
import { ViewDisplay } from "./ViewDisplay";
import { NO_VIEW_DRAWN, PHOTOREAL_NOT_CREATED } from "./styleRefusals";
import { ViewSceneProvider } from "./ViewSceneProvider";
import { runPose, STAR_SOURCE, STARS_WITHOUT_POSITION, startRun, stepRun } from "./viewRun";

const WIDTH_PX = 640;
const HEIGHT_PX = 360;

afterEach(() => {
  vi.useRealTimers();
});

/** Lays the stage out at {@link WIDTH_PX} by {@link HEIGHT_PX}, in VIEW's full layout. */
function stubLayout(): void {
  stubViewLayout({ widthPx: WIDTH_PX, heightPx: HEIGHT_PX }, () => FULL_VIEW_PX);
}

interface Setup {
  readonly user: ReturnType<typeof userEvent.setup>;
  readonly advance: (ms: number) => void;
  readonly views: () => FakeView[];
  readonly lastFrame: () => FrameSubmission | undefined;
  readonly engines: ReturnType<typeof fakeViewEngineSource>["engines"];
  /** What the fake engines' photorealistic image holds, as the meter weighs it. */
  readonly image: FakeMeteredImage;
  readonly socket: FakeWebSocket;
  readonly rerender: (mode: "visible" | "hidden") => void;
  readonly unmount: () => void;
}

/** Renders the display in a provider, with fake frames, layout and engine. */
function setup(
  options: { readonly store?: GraphicsStatusStore; readonly source?: ViewEngineSource } = {},
): Setup {
  const advanceTimers = fakeFramesAndTimeouts();
  stubLayout();
  const fake = fakeViewEngineSource();
  const store = options.store ?? new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
  const source = options.source ?? fake.source;
  vi.stubGlobal("WebSocket", FakeWebSocket);
  const tree = (mode: "visible" | "hidden") => (
    <ServerLinkHarness>
      <UniverseProvider>
        <GraphicsStatusContext value={store}>
          <UniversePanel expanded onToggle={() => undefined} />
          <ViewSceneProvider active={mode === "visible"} knownSystem={null}>
            {() => (
              <Activity mode={mode}>
                <ViewDisplay engineSource={source} />
              </Activity>
            )}
          </ViewSceneProvider>
        </GraphicsStatusContext>
      </UniverseProvider>
    </ServerLinkHarness>
  );
  const user = userEvent.setup({ advanceTimers });
  const view = render(tree("visible"));
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  const views = (): FakeView[] => fake.engines.flatMap((engine) => engine.views);
  return {
    user,
    advance: (ms) => {
      act(() => {
        FakeResizeObserver.resizeAll();
        vi.advanceTimersByTime(ms);
      });
    },
    views,
    lastFrame: () => views().at(-1)?.frames.at(-1),
    engines: fake.engines,
    image: fake.image,
    socket,
    rerender: (mode) => {
      view.rerender(tree(mode));
    },
    unmount: view.unmount,
  };
}

/** Lets the engine's promises settle. */
async function settle(): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** The distance from the camera to the own ship's hull, by its occluder draw, m. */
function hullDistanceM(frame: FrameSubmission | undefined): number {
  const hull = frame?.draws.find((draw) => draw.material.name === "wireframe:occluderHull");
  const o = hull?.offsetFromCameraM;
  return o === undefined ? Number.NaN : norm(vec3(o[0] ?? 0, o[1] ?? 0, o[2] ?? 0));
}

/** The materials of a frame's draws, in their order. */
function materialNames(frame: FrameSubmission | undefined): string[] {
  return (frame?.draws ?? []).map((draw) => draw.material.name);
}

/** A frame's draws of hull faces, each its material and its `fill` uniform (empty for none). */
function hullFaceDraws(frame: FrameSubmission | undefined): Array<[string, number[]]> {
  return (frame?.draws ?? [])
    .filter((draw) => /^wireframe:(occluderHull|hullSilhouette)$/.test(draw.material.name))
    .map((draw) => [draw.material.name, [...(draw.uniforms["fill"] ?? [])]]);
}

/** The number of line draws in a frame. */
function lineDraws(frame: FrameSubmission | undefined): number {
  return frame?.draws.filter((draw) => draw.material.name === "wireframe:lines").length ?? 0;
}

/** The style panel's buttons: each one's name, whether it is held back and whether it is pressed. */
function styleButtons(): Array<[string, string | null, string | null]> {
  return within(screen.getByRole("region", { name: "Style PRIMARY" }))
    .getAllByRole("button")
    .map((button) => [
      button.textContent,
      button.getAttribute("aria-disabled"),
      button.getAttribute("aria-pressed"),
    ]);
}

/** An engine source whose engine refuses the stage's canvas: the views cannot be made. */
function refusingViewSource(): ViewEngineSource {
  const fake = fakeViewEngineSource();
  return {
    ...fake.source,
    load: async (outcome, status) => {
      const engine = await fake.source.load(outcome, status);
      engine.createView = () => {
        throw new Error("the canvas gave no context");
      };
      return engine;
    },
  };
}

/** The side column's panels, each by its region's name, in the ruled order. */
const SIDE_PANELS = [
  "Instruments",
  "Targets PRIMARY",
  "Camera PRIMARY",
  "Style PRIMARY",
  "Exposure PRIMARY",
];

/** Whether each of {@link SIDE_PANELS} stands, and after the one before it in the document. */
function sidePanelsInOrder(): boolean {
  const panels = SIDE_PANELS.map((name) => screen.getByRole("region", { name }));
  return panels.every(
    (panel, index) =>
      index === 0 ||
      ((panels[index - 1]?.compareDocumentPosition(panel) ?? 0) &
        Node.DOCUMENT_POSITION_FOLLOWING) !==
        0,
  );
}

const HELD_BACK = [
  ["WIREFRAME", "true", "true"],
  ["PHOTOREALISTIC", "true", "false"],
];

/** Whether both style buttons are described by why no view is drawn. */
function bothHeldBackByNoView(): boolean {
  return within(screen.getByRole("region", { name: "Style PRIMARY" }))
    .getAllByRole("button")
    .every((button) => {
      const ids = button.getAttribute("aria-describedby")?.split(" ") ?? [];
      return ids.some((id) => document.getElementById(id)?.textContent === NO_VIEW_DRAWN);
    });
}

describe("VIEW's style panel while no view is drawn (R07.T19.b's follow-up)", () => {
  it("stands while the engine is made, the camera's style pressed and both held back", () => {
    setup();
    expect([styleButtons(), bothHeldBackByNoView()]).toEqual([HELD_BACK, true]);
  });

  it("stands where the views could not be made, held back with no view drawn", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    setup({
      source: refusingViewSource(),
    });
    await settle();
    expect([screen.queryByRole("application"), styleButtons(), bothHeldBackByNoView()]).toEqual([
      null,
      HELD_BACK,
      true,
    ]);
  });

  it("refuses the key 4 where the views could not be made, as its panel says", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { user, advance } = setup({
      store: await nominalStore(),
      source: refusingViewSource(),
    });
    await settle();
    await user.keyboard("4");
    advance(100);
    expect(styleButtons()).toEqual(HELD_BACK);
  });

  it("moves no panel when the engine is made", async () => {
    const { advance } = setup();
    const pending = sidePanelsInOrder();
    await settle();
    advance(100);
    expect(screen.getByRole("application")).toBeInTheDocument();
    expect([pending, sidePanelsInOrder()]).toEqual([true, true]);
    expect(
      within(screen.getByRole("region", { name: "Style PRIMARY" })).queryByText(NO_VIEW_DRAWN),
    ).toBeNull();
  });
});

describe("the VIEW display", () => {
  it("reads GRAPHICS ACQUIRING ADAPTER in the view's place until its engine is made", () => {
    setup();
    expect([
      screen.getByText("GRAPHICS ACQUIRING ADAPTER"),
      screen.queryByRole("application"),
    ]).toEqual([expect.anything(), null]);
  });

  it("draws the wireframe into its canvas every frame once its engine is made", async () => {
    const { advance, views } = setup();
    await settle();
    advance(100);
    expect((views()[0]?.frames.length ?? 0) > 3).toBe(true);
  });

  it("labels its frames for the pass timer", async () => {
    const { advance, views } = setup();
    await settle();
    advance(100);
    expect(views()[0]?.frames[0]?.label).toBe("view:wireframe");
  });

  it("stops drawing while it is hidden", async () => {
    const { advance, views, rerender } = setup();
    await settle();
    advance(100);
    rerender("hidden");
    const drawn = views().reduce((n, view) => n + view.frames.length, 0);
    advance(500);
    expect(views().reduce((n, view) => n + view.frames.length, 0)).toBe(drawn);
  });

  it("disposes its engine when it goes", async () => {
    const { engines, unmount } = setup();
    await settle();
    unmount();
    expect(engines.map((engine) => engine.disposed)).toEqual([true]);
  });

  it("shows the graphics annunciation in place of the view in safe mode", async () => {
    setup({ store: new GraphicsStatusStore(initialGraphicsStatus("safe", false)) });
    await settle();
    expect([screen.getByText(/^GRAPHICS SAFE MODE/), screen.queryByRole("application")]).toEqual([
      expect.anything(),
      null,
    ]);
  });

  it("says the view could not be made where no adapter is granted", async () => {
    const fake = fakeViewEngineSource();
    setup({
      source: { ...fake.source, requestAdapter: () => Promise.resolve({ kind: "no-adapter" }) },
    });
    await settle();
    expect(screen.getByText(/^GRAPHICS NOT AVAILABLE/)).toBeInTheDocument();
  });

  it("says the view could not be made where the engine refuses its canvas", async () => {
    const logged = vi.spyOn(console, "error").mockImplementation(() => undefined);
    setup({
      source: refusingViewSource(),
    });
    await settle();
    expect([
      screen.getByText("GRAPHICS NOT AVAILABLE: views could not be made, relaunch to retry"),
      screen.queryByRole("application"),
    ]).toEqual([expect.anything(), null]);
    expect(logged).toHaveBeenCalledWith("view view could not be made:", expect.any(Error));
  });

  it("shows a refused view's fault on its own view's plate", async () => {
    const store = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    const { advance } = setup({ store });
    await settle();
    act(() => {
      store.dispatch({ kind: "view-refused", viewName: "view" });
    });
    advance(16);
    expect(
      screen.getByText(
        "GRAPHICS VIEW REFUSED: not re-created after device loss, not drawn, relaunch to retry",
      ),
    ).toHaveClass("request-status__text--fault");
    expect(screen.getByRole("application")).toBeInTheDocument();
  });

  it("does not show another view's refusal on its plate", async () => {
    const store = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    const { advance } = setup({ store });
    await settle();
    act(() => {
      store.dispatch({ kind: "view-refused", viewName: "cockpit" });
    });
    advance(16);
    expect(store.getSnapshot().fault).toEqual({ kind: "view-refused", viewName: "cockpit" });
    expect(screen.queryByText(/^GRAPHICS VIEW REFUSED/)).toBeNull();
  });

  it("names its canvas for its style and camera", async () => {
    const { advance } = setup();
    await settle();
    advance(100);
    expect(
      screen.getByRole("application", { name: "VIEW, WIREFRAME, PRIMARY, SEAT" }).tagName,
    ).toBe("CANVAS");
  });

  it("draws no text into its canvas", async () => {
    const getContext = vi.spyOn(HTMLCanvasElement.prototype, "getContext");
    const { advance } = setup();
    await settle();
    advance(100);
    expect(getContext.mock.calls.filter(([kind]) => kind === "2d")).toEqual([]);
  });

  it("shows each line of the label block", async () => {
    const { advance } = setup();
    await settle();
    advance(300);
    const block = screen.getByText("VIEW", { selector: "p" }).parentElement;
    expect(block === null ? "" : block.textContent).toMatch(
      /FRAME.*TIME.*UT .*STYLE.*WIREFRAME.*CAMERA.*SEAT.*FOV.*60°.*EXPOSURE.*EV100 -1\.0 MAN.*STARS.*RANGE QUERY.*SCENE.*PRECISION TEST/,
    );
  });

  it("selects in its list the mark a click on the canvas picks", async () => {
    const { user, advance } = setup();
    await settle();
    advance(16);
    // The display's first frame is the scene at its start; its marks are where the draw list puts
    // them for the same camera and viewport.
    const run = stepRun(startRun(precisionScene()), {
      serverScene: null,
      dtS: 0,
      held: new Set(),
      reducedMotion: false,
    });
    const list = buildWireframeDrawList(
      run.scene,
      { pose: runPose(run), fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180 },
      { widthPx: WIDTH_PX, heightPx: HEIGHT_PX },
      readTokens(document.documentElement),
      {
        lowSetting: false,
        ev100: -1,
        selection: null,
        destination: null,
        remPx: 16,
        ...viewStrokesAt(1),
      },
    );
    const anchor = list.anchors[0];
    if (anchor === undefined) {
      throw new Error("the precision scene's first frame has no mark in view");
    }
    advance(300);
    await user.pointer({
      keys: "[MouseLeft]",
      target: screen.getByRole("application"),
      coords: { clientX: anchor.xPx, clientY: anchor.yPx },
    });
    const name = run.scene.bodies.find(
      (body) => anchor.target.kind === "body" && body.id === anchor.target.body,
    )?.designation;
    expect(
      within(screen.getByRole("listbox", { name: "Marks in view" })).getByRole("option", {
        selected: true,
      }),
    ).toHaveAccessibleName(new RegExp(`^${name ?? "(a craft)"}`));
  });

  it("draws the bracket reticle about the mark its list selects", async () => {
    const { user, advance, lastFrame } = setup();
    await settle();
    advance(300);
    const before = lineDraws(lastFrame());
    await user.click(screen.getAllByRole("option")[0] ?? document.body);
    advance(16);
    // The reticle is one cased batch: its casing and its stroke.
    expect(lineDraws(lastFrame())).toBe(before + 2);
  });

  it("moves the selection through its list by keyboard", async () => {
    const { user, advance } = setup();
    await settle();
    advance(300);
    const list = screen.getByRole("listbox", { name: "Marks in view" });
    await user.click(list);
    await user.keyboard("{ArrowDown}");
    expect(within(list).getAllByRole("option", { selected: true })).toHaveLength(1);
  });

  it("reaches every camera control from the keyboard", async () => {
    const { user, advance } = setup();
    await settle();
    advance(300);
    const names = [
      "1 SEAT",
      "2 CHASE",
      "3 FREE",
      "[ PREVIOUS TARGET",
      "] NEXT TARGET",
      "Narrower field of view",
      "Wider field of view",
      "EASED CAMERA MOVES",
    ];
    const reached = new Set<string>();
    for (let i = 0; i < 40 && reached.size < names.length; i += 1) {
      // Each Tab moves on from where the last left the focus, so they cannot run together.
      // oxlint-disable-next-line no-await-in-loop
      await user.tab();
      const focused = document.activeElement;
      const name = focused?.getAttribute("aria-label") ?? focused?.textContent ?? "";
      if (names.includes(name)) {
        reached.add(name);
      }
    }
    expect([...reached]).toEqual(names);
  });

  it("changes its preset on the preset's key", async () => {
    const { user, advance } = setup();
    await settle();
    advance(300);
    await user.keyboard("2");
    advance(300);
    expect(screen.getByRole("button", { name: "2 CHASE" })).toHaveAttribute("aria-pressed", "true");
  });

  it("flies a free camera by the keys held on its canvas", async () => {
    stubMatchMedia(true);
    const { user, advance, lastFrame } = setup();
    await settle();
    advance(300);
    await user.keyboard("3");
    advance(100);
    // The ship moves on its script meanwhile, so the camera's own motion is the change beyond it.
    const drift = (): number => {
      const from = hullDistanceM(lastFrame());
      advance(500);
      return hullDistanceM(lastFrame()) - from;
    };
    const still = drift();
    await user.click(screen.getByRole("application"));
    await user.keyboard("{s>}");
    expect(Math.abs(drift() - still) > 100).toBe(true);
  });

  it("shows the free camera's rate, and says so at the lowest step", async () => {
    const { user, advance } = setup();
    await settle();
    advance(300);
    const rate = screen.getByRole("status", { name: "Free camera rate" });
    expect(rate).toHaveTextContent("RATE 1.00 km/s");
    await user.click(screen.getByRole("application"));
    await user.keyboard("3");
    await user.keyboard("{PageUp}");
    advance(300);
    expect(rate).toHaveTextContent("RATE 3.16 km/s");
    expect(screen.queryByText(/NOT AVAILABLE: PAGE/)).not.toBeInTheDocument();
    for (let i = 0; i < 8; i += 1) {
      // Each press is one step; they are sequential by nature.
      // oxlint-disable-next-line no-await-in-loop
      await user.keyboard("{PageDown}");
    }
    advance(300);
    expect(rate).toHaveTextContent("RATE 1.00 m/s");
    expect(
      screen.getByText("NOT AVAILABLE: PAGE DOWN, RATE at its lowest step"),
    ).toBeInTheDocument();
  });

  it("moves a mark's label with its mark every frame, between readouts", async () => {
    stubMatchMedia(true);
    const { user, advance } = setup();
    await settle();
    advance(300);
    await user.keyboard("3");
    advance(300);
    // The labels are hidden from assistive technology; the list names the same targets.
    const name = screen.getAllByRole("option")[0]?.querySelector(".view-list__name")?.textContent;
    const label = screen
      .getAllByText(name ?? "", { exact: false })
      .find((each) => each.classList.contains("view-marks__label"));
    const before = label?.style.transform;
    await user.click(screen.getByRole("application"));
    await user.keyboard("{ArrowLeft>}");
    // Three frames, short of the next 4 Hz readout.
    advance(50);
    expect([label?.isConnected, before !== undefined && label?.style.transform !== before]).toEqual(
      [true, true],
    );
  });

  it("stops flying when its canvas loses focus", async () => {
    stubMatchMedia(true);
    const { user, advance, lastFrame } = setup();
    await settle();
    advance(300);
    await user.keyboard("3");
    advance(100);
    const drift = (): number => {
      const from = hullDistanceM(lastFrame());
      advance(500);
      return hullDistanceM(lastFrame()) - from;
    };
    const still = drift();
    await user.click(screen.getByRole("application"));
    await user.keyboard("{s>}");
    advance(100);
    await user.tab();
    advance(16);
    expect(Math.abs(drift() - still) < 10).toBe(true);
  });

  it("switches its scene from the SCENE selector", async () => {
    const { user, advance } = setup();
    await settle();
    await user.click(screen.getByRole("button", { name: "FRAME CHANGE TEST" }));
    await settle();
    advance(300);
    expect(screen.getByText("VIEW", { selector: "p" }).parentElement?.textContent).toMatch(
      /FRAME CHANGE TEST/,
    );
  });

  it("keeps its engine across a change of scene, asking for no new adapter", async () => {
    const { user, advance, engines } = setup();
    await settle();
    advance(300);
    await user.click(screen.getByRole("button", { name: "FRAME CHANGE TEST" }));
    await settle();
    advance(300);
    expect(engines.length).toBe(1);
    // The view stands in its place (the style control may still say the adapter is acquired).
    expect(screen.getByRole("application", { name: /^VIEW,/ }).tagName).toBe("CANVAS");
  });

  it("says AUTO is not available while there is no image to meter", async () => {
    setup();
    await settle();
    expect(
      screen.getByText(
        (_, element) =>
          element?.tagName === "P" &&
          element.textContent === "AUTO NOT AVAILABLE: NO IMAGE TO METER",
      ),
    ).toBeInTheDocument();
  });

  it("offers its exposure's congruent pair in the guide's order, ENABLE then INHIBIT", async () => {
    setup();
    await settle();
    const names = screen
      .getAllByRole("button")
      .map((button) => button.textContent)
      .filter((name) => name === "ENABLE" || name === "INHIBIT");
    expect(names).toEqual(["ENABLE", "INHIBIT"]);
  });
});

describe("the VIEW display's interim stars", () => {
  it("draws the stars of the open universe's range queries, with their count line", async () => {
    const { user, advance, lastFrame, socket } = setup();
    await act(async () => {
      socket.serverAnswers("list_universes", () => aUniverseList());
      await Promise.resolve();
    });
    await settle();
    advance(300);
    await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
    await act(async () => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse());
      await Promise.resolve();
    });
    // One star 100 ly straight ahead of the seat, in every answer (merged to one).
    const run = startRun(precisionScene());
    const ahead = rotate(runPose(run).orientation, vec3(0, 0, -1));
    // The kept scenes' barycentre (`KEPT_BARYCENTRE`).
    const centre = [0, 26_000, 0] as const;
    for (let i = 0; i < 4; i += 1) {
      // Each answer goes to the latest request not yet answered, so they are played in turn.
      // oxlint-disable-next-line no-await-in-loop
      await act(async () => {
        socket.serverAnswers("systems_in_range", (body) =>
          aSystemsInRange({
            centreLy: centre,
            radiusLy: body.radius_ly,
            minLayer: body.min_layer,
            limit: body.limit,
            systems: [
              {
                relLy: [ahead.x * 100, ahead.y * 100, ahead.z * 100],
                layer: "c",
                stellar: { ...aStellarBrief("c"), absolute_v_mag: 1 },
              },
            ],
          }),
        );
        await Promise.resolve();
      });
    }
    advance(300);
    expect([
      lastFrame()?.draws.some((draw) => draw.material.name === "wireframe:starSprite"),
      // A reading of numbers, so an `output` (B612 Mono), not a statement.
      screen.getByText(/^STARS 1 DRAWN · 0 WITHOUT V · RADII 620\/360\/210\/60 ly$/).tagName,
    ]).toEqual([true, "OUTPUT"]);
  });
});

describe("the VIEW display's interim queries", () => {
  it("are not asked again on a change of scene in the same system", async () => {
    const { user, advance, socket } = setup();
    await act(async () => {
      socket.serverAnswers("list_universes", () => aUniverseList());
      await Promise.resolve();
    });
    await settle();
    advance(300);
    await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
    await act(async () => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse());
      await Promise.resolve();
    });
    const asked = socket.requestsOfKind("systems_in_range").length;
    await user.click(screen.getByRole("button", { name: "FRAME CHANGE TEST" }));
    await settle();
    expect([asked, socket.requestsOfKind("systems_in_range").length]).toEqual([4, 4]);
  });
});

describe("the VIEW display's eased moves", () => {
  it("ease a preset change when asked for", async () => {
    const { user, advance, lastFrame } = setup();
    await settle();
    advance(300);
    const seat = hullDistanceM(lastFrame());
    await user.click(screen.getByRole("button", { name: "EASED CAMERA MOVES" }));
    await user.keyboard("2");
    advance(16);
    expect(hullDistanceM(lastFrame()) - seat < 10).toBe(true);
  });

  it("are cuts under reduced motion", async () => {
    stubMatchMedia(true);
    const { user, advance, lastFrame } = setup();
    await settle();
    advance(300);
    const seat = hullDistanceM(lastFrame());
    await user.click(screen.getByRole("button", { name: "EASED CAMERA MOVES" }));
    await user.keyboard("2");
    advance(16);
    expect(hullDistanceM(lastFrame()) - seat > 30).toBe(true);
  });
});

/** Opens a universe, whose scene subscription the display then sends. */
async function openUniverse(view: Setup): Promise<void> {
  await act(async () => {
    view.socket.serverAnswers("list_universes", () => aUniverseList());
    await Promise.resolve();
  });
  await settle();
  view.advance(300);
  await view.user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
  await act(async () => {
    view.socket.serverAnswers("open_universe", () => anOpenedUniverse());
    await Promise.resolve();
  });
}

/** A system ID other than the kept scenes' (`KEPT_SYSTEM`), which the slice's fixture shares. */
const ELSEWHERE: SystemIdHex = "0200080020000005";

/**
 * Answers the scene subscription with the ship 1 AU out in the slice's system, or in the same
 * system under another ID.
 *
 * @remarks
 * Under another ID the answer is sent as text with every mention of the fixture's ID rewritten,
 * as a server would write it, so that no parsed value needs a type assertion.
 */
async function sceneArrives(
  socket: FakeWebSocket,
  system: SystemIdHex = FIXTURE_SYSTEM,
  stated: "place" | "no_place" = "place",
): Promise<void> {
  const { place: _place, ...withoutPlace } = sliceSceneSystem();
  const answer: ResponseBody = {
    kind: "subscribe",
    subscription: 5,
    state: {
      topic: "scene",
      sequence: 0,
      clock: sceneClock(3_000),
      ship: shipInSystem(3_000),
      system: stated === "place" ? sliceSceneSystem() : withoutPlace,
      tidal_radius_m: SCENE_TIDAL_RADIUS_M,
      craft: [],
    },
  };
  await act(async () => {
    if (system === FIXTURE_SYSTEM) {
      socket.serverAnswers("subscribe", () => answer);
    } else {
      const id = socket.requestsOfKind("subscribe").at(-1)?.id;
      const text = JSON.stringify({ type: "response", id, body: answer });
      socket.dispatchEvent(
        new MessageEvent("message", { data: text.replaceAll(FIXTURE_SYSTEM, system) }),
      );
    }
    await vi.advanceTimersByTimeAsync(0);
  });
}

function labelBlock(): string {
  return screen.getByText("VIEW", { selector: "p" }).parentElement?.textContent ?? "";
}

describe("the VIEW display's server scene", () => {
  it("draws a kept scene in its place, saying why, while no universe is open", async () => {
    const { advance } = setup();
    await settle();
    advance(300);
    expect([
      screen.getByRole("button", { name: "SERVER" }).getAttribute("aria-pressed"),
      screen.getByText("SCENE NOT AVAILABLE: no universe open"),
      labelBlock(),
    ]).toEqual(["true", expect.anything(), expect.stringMatching(/SCENE.*PRECISION TEST/)]);
  });

  it("draws the server's scene once it arrives, and reports its camera", async () => {
    const view = setup();
    await openUniverse(view);
    await sceneArrives(view.socket);
    await settle();
    view.advance(300);
    await settle();
    expect([
      labelBlock().includes("PRECISION TEST"),
      screen.queryByText(/^SCENE /),
      // The seat is on the stand-in's test hull, whose origin is metres from the eye.
      hullDistanceM(view.lastFrame()) < 20,
      view.socket.requestsOfKind("scene_cameras").at(-1)?.body.cameras.length,
    ]).toEqual([false, null, true, 1]);
  });

  it("names a system never opened on SYSTEM by the place the scene states, and asks its stars", async () => {
    // The provider is told of no system (`knownSystem` is `null`): only the scene names it.
    const view = setup();
    await openUniverse(view);
    const keptQueries = view.socket.requestsOfKind("systems_in_range").length;
    // The fixture shares the kept scenes' system ID, whose stars are asked already; the scene
    // here is in another system, as a real server's always is.
    await sceneArrives(view.socket, ELSEWHERE);
    await settle();
    view.advance(300);
    await settle();
    const place = scenePlace();
    const asked = view.socket.requestsOfKind("systems_in_range").slice(keptQueries);
    expect([
      screen.getAllByText(new RegExp(`^${SCENE_DESIGNATION} /`)).length > 0,
      labelBlock().includes(STAR_SOURCE),
      labelBlock().includes(STARS_WITHOUT_POSITION),
      asked.length,
      asked.every(
        (request) =>
          JSON.stringify(request.body.centre) === JSON.stringify(place.barycentre) &&
          JSON.stringify(request.body.time) === JSON.stringify(place.time),
      ),
    ]).toEqual([true, true, false, 4, true]);
  });

  it("labels the sky once it arrives, in the interim field's place", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const view = setup();
    await openUniverse(view);
    await sceneArrives(view.socket, ELSEWHERE);
    await settle();
    view.advance(300);
    await settle();
    const before = labelBlock();
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
    const after = labelBlock();
    expect([
      before.includes(STAR_SOURCE),
      sky.body.eye?.field_factor,
      sky.body.exclude_system,
      after.includes("V 7.4 mag EYE · CLUSTERS: NOT YET MODELLED"),
      after.includes(STAR_SOURCE),
    ]).toEqual([true, 1.4, ELSEWHERE, true, false]);
  });

  it("names a system from a server that states no place by its ID, with no stars", async () => {
    const view = setup();
    await openUniverse(view);
    const keptQueries = view.socket.requestsOfKind("systems_in_range").length;
    await sceneArrives(view.socket, ELSEWHERE, "no_place");
    await settle();
    view.advance(300);
    await settle();
    expect([
      screen.getAllByText(new RegExp(`^${ELSEWHERE} /`)).length > 0,
      labelBlock().includes(STARS_WITHOUT_POSITION),
      view.socket.requestsOfKind("systems_in_range").length - keptQueries,
      view.socket.requestsOfKind("sky").length,
    ]).toEqual([true, true, 0, 0]);
  });

  it("shows a scene the server ended as stale while it is reopened", async () => {
    const view = setup();
    await openUniverse(view);
    await sceneArrives(view.socket);
    await settle();
    view.advance(300);
    act(() => {
      view.socket.serverSends({
        type: "subscription_ended",
        subscription: 5,
        error: { code: "internal", message: "the scene could not be advanced", field: null },
      });
    });
    await settle();
    view.advance(300);
    expect([
      screen.getByText("SCENE STALE: reopening the scene"),
      // The time reading, its runs each in a span of its own (R07.T19.b).
      screen
        .getByText(
          (_, element) => element?.tagName === "OUTPUT" && element.textContent.startsWith("UT "),
        )
        .classList.contains("stale"),
      screen.getByRole("heading", { name: "Targets stale PRIMARY" }),
      labelBlock().includes("PRECISION TEST"),
      screen
        .getAllByRole("option")
        .every((row) => row.getAttribute("aria-label")?.endsWith(", stale") === true),
    ]).toEqual([expect.anything(), true, expect.anything(), false, true]);
  });

  it("says a camera report was rejected, with the server's reason", async () => {
    const view = setup();
    await openUniverse(view);
    await sceneArrives(view.socket);
    await settle();
    view.advance(300);
    const report = view.socket.requestsOfKind("scene_cameras").at(-1);
    if (report === undefined) {
      throw new Error("the view's camera is reported");
    }
    act(() => {
      view.socket.serverRejects(report.id, {
        code: "bad_request",
        message: "a camera is outside the scene's reach",
        field: "cameras",
      });
    });
    await settle();
    view.advance(300);
    expect(
      screen.getByText("CAMERA REPORT REJECTED: a camera is outside the scene's reach"),
    ).toBeInTheDocument();
  });

  it("offers RETRY on a refused scene, which asks for it again", async () => {
    const view = setup();
    await openUniverse(view);
    const subscribe = view.socket.requestsOfKind("subscribe").at(-1);
    if (subscribe === undefined) {
      throw new Error("the scene is asked for");
    }
    act(() => {
      view.socket.serverRejects(subscribe.id, {
        code: "internal",
        message: "the scene could not be made",
        field: null,
      });
    });
    await settle();
    view.advance(300);
    expect(screen.getByText("SCENE REJECTED: the scene could not be made")).toBeInTheDocument();
    await view.user.click(screen.getByRole("button", { name: "RETRY" }));
    expect([
      view.socket.requestsOfKind("subscribe").length,
      screen.getByText("SCENE PENDING"),
    ]).toEqual([2, expect.anything()]);
  });
});

/** A status store whose adapter has answered: a hardware adapter, both styles offered. */
async function nominalStore(): Promise<GraphicsStatusStore> {
  const store = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
  const outcome = await requestAdapterOutcome(
    new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] })]),
  );
  store.dispatch({ kind: "adapter-outcome", outcome });
  return store;
}

describe("the VIEW display's style (R07.T8.a)", () => {
  it("shows the style control, the photorealistic style held back while the adapter offers only the wireframe", async () => {
    const { advance } = setup();
    await settle();
    advance(100);
    const panel = screen.getByRole("region", { name: "Style PRIMARY" });
    expect([
      within(panel).getByRole("button", { name: "WIREFRAME" }).getAttribute("aria-pressed"),
      within(panel).getByRole("button", { name: "PHOTOREALISTIC" }).getAttribute("aria-disabled"),
    ]).toEqual(["true", "true"]);
  });

  it("switches to the photorealistic style on 4, and states the provisional albedo", async () => {
    const { user, advance, lastFrame } = setup({ store: await nominalStore() });
    await settle();
    advance(100);
    await user.click(screen.getByRole("button", { name: "PHASE TEST" }));
    advance(100);
    await user.keyboard("4");
    // The first photorealistic frame starts the pipelines' compile; once made, the view draws it.
    advance(100);
    await settle();
    advance(300);
    expect([
      screen.getByRole("application", { name: /^VIEW, PHOTOREALISTIC/ }).tagName,
      labelBlock().includes("BODY PHOTOMETRY: NOT YET MODELLED"),
      labelBlock().includes("LIGHTING:"),
      lastFrame()?.label,
    ]).toEqual(["CANVAS", true, false, "symbology"]);
  });

  it("switches to the photorealistic style from its control", async () => {
    const { user, advance } = setup({ store: await nominalStore() });
    await settle();
    advance(100);
    await user.click(screen.getByRole("button", { name: "PHOTOREALISTIC" }));
    advance(100);
    await settle();
    advance(300);
    expect(screen.getByRole("application", { name: /^VIEW, PHOTOREALISTIC/ }).tagName).toBe(
      "CANVAS",
    );
  });

  it("states the lighting is not received for a kept scene without host discs", async () => {
    const { user, advance } = setup({ store: await nominalStore() });
    await settle();
    advance(100);
    await user.click(screen.getByRole("button", { name: "PRECISION TEST" }));
    advance(100);
    await user.keyboard("4");
    // The first photorealistic frame starts the pipelines' compile; once made, the view draws it.
    advance(100);
    await settle();
    advance(300);
    expect(labelBlock().includes("LIGHTING: NOT RECEIVED")).toBe(true);
  });
});

/** The widths of the line draws of a frame that stroke a hull's edges, at its non-zero origin. */
function hullLineWidths(frame: FrameSubmission | undefined): ReadonlyArray<number> {
  return (frame?.draws ?? [])
    .filter(
      (draw) =>
        draw.material.name === "wireframe:lines" &&
        Array.from(draw.offsetFromCameraM).some((value) => value !== 0),
    )
    .map((draw) => draw.uniforms["widthPx"]?.[0] ?? Number.NaN);
}

/**
 * Whether a frame's line draws come in pairs, each a `--surface-0` casing two casings wider than
 * the stroke drawn after it: every batch cased, its casing beneath it, at the display's ratio.
 */
function casedInPairs(frame: FrameSubmission | undefined): boolean {
  const casingPx = CASING_PX * lineScale(window.devicePixelRatio);
  const lines = (frame?.draws ?? []).filter((draw) => draw.material.name === "wireframe:lines");
  const casing = [...linearColour(readTokens(document.documentElement).surface0)];
  const width = (index: number): number => lines[index]?.uniforms["widthPx"]?.[0] ?? Number.NaN;
  return (
    lines.length > 0 &&
    lines.length % 2 === 0 &&
    lines.every((draw, index) =>
      index % 2 === 1
        ? width(index - 1) - width(index) === 2 * casingPx
        : [...(draw.uniforms["colour"] ?? [])].every((value, c) => value === casing[c]),
    )
  );
}

/** The classes of the `--surface-0` plates that DOM text over a view's image sits on. */
const PLATE_CLASSES = ["view-label", "view-marks__label"] as const;

/** The declarations of `.<name> { … }`, the stylesheet's rule for exactly that class. */
function ruleOf(css: string, name: string): string {
  return new RegExp(`(?:^|\\n)\\.${name} \\{([^}]*)\\}`, "u").exec(css)?.[1] ?? "";
}

/** Switches the default `PRECISION TEST` to the photorealistic style and lets it draw. */
async function drawPrecisionPhotoreal(view: Setup): Promise<void> {
  await settle();
  view.advance(100);
  await view.user.keyboard("4");
  // The first photorealistic frame starts the pipelines' compile; once made, the view draws it.
  view.advance(100);
  await settle();
  view.advance(300);
}

describe("the VIEW display's symbology over the image (R07.T16.a)", () => {
  it("cases every mark over the photorealistic image, the hull's edges too, as the wireframe does not", async () => {
    const view = setup({ store: await nominalStore() });
    await settle();
    view.advance(300);
    const wireframe = [hullLineWidths(view.lastFrame()), casedInPairs(view.lastFrame())];
    await drawPrecisionPhotoreal(view);
    expect([
      wireframe,
      view.lastFrame()?.label,
      hullLineWidths(view.lastFrame()),
      casedInPairs(view.lastFrame()),
    ]).toEqual([[[3], false], "symbology", [7, 3], true]);
  });

  it.each([
    [0.78125, [7, 3]],
    [1, [7, 3]],
    [2, [7, 3]],
    [3, [10.5, 4.5]],
  ] as const)(
    "draws the hull's cased edge at a ratio of %s as %j device px, never under 2 (R07.T16.d)",
    async (ratio, widths) => {
      vi.stubGlobal("devicePixelRatio", ratio);
      const view = setup({ store: await nominalStore() });
      await drawPrecisionPhotoreal(view);
      expect([view.lastFrame()?.label, hullLineWidths(view.lastFrame())]).toEqual([
        "symbology",
        widths,
      ]);
    },
  );

  it("draws the bodies' occluders, then the craft's silhouettes, then every line, under symbology (R07.T16.e)", async () => {
    const view = setup({ store: await nominalStore() });
    await drawPrecisionPhotoreal(view);
    const drawn = materialNames(view.lastFrame());
    const lastSphere = drawn.lastIndexOf("wireframe:occluderSphere");
    const firstSilhouette = drawn.indexOf("wireframe:hullSilhouette");
    expect([
      view.lastFrame()?.label,
      lastSphere >= 0 && lastSphere < firstSilhouette,
      drawn.lastIndexOf("wireframe:hullSilhouette") < drawn.indexOf("wireframe:lines"),
    ]).toEqual(["symbology", true, true]);
  });

  it("fills the craft's silhouettes in --surface-0 over the image, and none in the wireframe (R07.T16.e)", async () => {
    const view = setup({ store: await nominalStore() });
    await settle();
    view.advance(300);
    const wireframe = hullFaceDraws(view.lastFrame());
    await drawPrecisionPhotoreal(view);
    const surface = [...linearColour(readTokens(document.documentElement).surface0)];
    expect({
      wireframe: [...new Set(wireframe.map(([name]) => name))],
      overlay: [...new Set(hullFaceDraws(view.lastFrame()).map((draw) => JSON.stringify(draw)))],
    }).toEqual({
      wireframe: ["wireframe:occluderHull"],
      overlay: [JSON.stringify(["wireframe:hullSilhouette", surface])],
    });
  });

  it("says BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED over the image of a scene with a craft (R07.T16.e)", async () => {
    const view = setup({ store: await nominalStore() });
    await drawPrecisionPhotoreal(view);
    expect([
      labelBlock().includes("BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED"),
      labelBlock().includes("BODY PHOTOMETRY: NOT YET MODELLED"),
    ]).toEqual([true, false]);
  });

  it("sets every text over the photorealistic image on a --surface-0 plate, beside an instrument", async () => {
    // On the harness's 1280 × 720 stage, where a slot has room, with INSTRUMENT 1 open over the
    // primary's image: a panel of its own, its label block on it, not text on the image.
    const fake = fakeViewEngineSource();
    const view = renderViewDisplay({
      store: await nominalStore(),
      source: fake.source,
      engines: fake.engines,
    });
    await settle();
    view.advance(100);
    await view.user.click(
      within(screen.getByRole("group", { name: "INSTRUMENT 1" })).getByRole("button", {
        name: "OPEN",
      }),
    );
    view.advance(300);
    await view.user.keyboard("4");
    view.advance(100);
    await settle();
    view.advance(300);
    const overlay =
      screen.getByRole("application", { name: /^VIEW, PHOTOREALISTIC, PRIMARY/ })
        .nextElementSibling ?? document.createElement("div");
    const walker = document.createTreeWalker(overlay, NodeFilter.SHOW_TEXT);
    const unplated: string[] = [];
    const plated = new Set<string>();
    let inSlot = 0;
    for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
      const text = node.textContent?.trim() ?? "";
      const plate = PLATE_CLASSES.find((name) => node.parentElement?.closest(`.${name}`) !== null);
      if (text.length === 0) {
        continue;
      }
      if (node.parentElement?.closest(".view-instrument.panel") !== null) {
        inSlot += 1;
      } else if (plate === undefined) {
        unplated.push(text);
      } else {
        plated.add(plate);
      }
    }
    expect({
      unplated,
      plated: [...plated].toSorted(),
      slotText: inSlot > 0,
      paints: PLATE_CLASSES.map((name) =>
        ruleOf(STYLES, name).includes("background: var(--surface-0)"),
      ),
    }).toEqual({
      unplated: [],
      plated: [...PLATE_CLASSES].toSorted(),
      slotText: true,
      paints: [true, true],
    });
  });
});

describe("the VIEW display's photorealistic style when its pipelines fail (R07.T8.a)", () => {
  it("returns to the wireframe and holds the style back with the fault", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const fake = fakeViewEngineSource();
    const source: ViewEngineSource = {
      ...fake.source,
      load: async (outcome, status) => {
        const engine = await fake.source.load(outcome, status);
        return Object.assign(engine, {
          createMaterialAsync: (): Promise<never> => Promise.reject(new Error("refused")),
        });
      },
    };
    const { user, advance } = setup({ store: await nominalStore(), source });
    await settle();
    advance(100);
    await user.keyboard("4");
    advance(100);
    await settle();
    advance(300);
    expect([
      screen.getByRole("application", { name: /^VIEW, WIREFRAME/ }).tagName,
      screen.getByRole("button", { name: "PHOTOREALISTIC" }).getAttribute("aria-disabled"),
      screen.getByText(PHOTOREAL_NOT_CREATED).tagName,
    ]).toEqual(["CANVAS", "true", "P"]);
  });
});

describe("the VIEW display's exposure meter (R07.T8.a)", () => {
  it("mounts the meter beside the exposure only while the photorealistic image is drawn", async () => {
    const { user, advance } = setup({ store: await nominalStore() });
    await settle();
    advance(100);
    expect(screen.queryByRole("region", { name: "Exposure meter PRIMARY" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "PHASE TEST" }));
    advance(100);
    await user.keyboard("4");
    advance(100);
    await settle();
    advance(300);
    // Beside a drawn image the meter never says there is no image: it reads the exposure.
    const meter = within(screen.getByRole("region", { name: "Exposure meter PRIMARY" }));
    expect([
      meter.queryByText(/NO IMAGE TO METER/u),
      meter.getByRole("status", { name: "SOURCE" }).textContent,
    ]).toEqual([null, "PRIMARY"]);
  });
});

/** The exposure panel's reading. */
function exposureReadout(): string {
  return (
    within(screen.getByRole("region", { name: "Exposure PRIMARY" })).getAllByRole("status")[0]
      ?.textContent ?? ""
  );
}

describe("the VIEW display's AUTO exposure (R07.T8.a)", () => {
  it("enters AUTO on ENABLE once the image is metered, and keeps an operator's INHIBIT", async () => {
    const { user, advance } = setup({ store: await nominalStore() });
    await settle();
    advance(100);
    await user.click(screen.getByRole("button", { name: "PHASE TEST" }));
    advance(100);
    await user.keyboard("4");
    advance(100);
    await settle();
    advance(300);
    await settle();
    advance(300);
    await user.click(screen.getByRole("button", { name: "ENABLE" }));
    advance(600);
    expect(exposureReadout()).toMatch(/^EV100 -?\d+\.\d AUTO$/);
    await user.click(screen.getByRole("button", { name: "INHIBIT" }));
    advance(1000);
    await settle();
    advance(1000);
    expect(exposureReadout()).toMatch(/INHIBITED · OPERATOR$/);
  });
});

/** The exposure meter's panel. */
function meterPanel(): ReturnType<typeof within> {
  return within(screen.getByRole("region", { name: "Exposure meter PRIMARY" }));
}

/** The exposure meter's status with what to do, by its whole text, or `null` where none stands. */
function meterStatusShown(): string | null {
  const line = within(screen.getByRole("region", { name: "Exposure meter PRIMARY" })).queryByText(
    (_, element) =>
      element?.tagName === "P" && /: (?:choose AVG|widen the view)/u.test(element.textContent),
  );
  return line?.textContent ?? null;
}

/** The statuses of a meter, or of the want of an image, that any text on the display names. */
function statusesShown(): string[] {
  return ["NO IMAGE TO METER", "NO LIT SIDE", "NO DARK SIDE", "STAR DISC ONLY"].filter(
    (status) => screen.queryAllByText(new RegExp(status, "u")).length > 0,
  );
}

/**
 * Advances the frames by `ms` in steps of 50 ms, letting the histograms' reads settle after each,
 * as they would between real frames.
 */
async function advanceReading(view: Setup, ms: number): Promise<void> {
  for (let elapsed = 0; elapsed < ms; elapsed += 50) {
    view.advance(50);
    // Each step's reads settle before the next step's frames.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
}

describe("the VIEW display's meter with nothing to weigh (R07.T16.b)", () => {
  /** PHASE TEST drawn photorealistic, its image metered under AVG and the exposure at AUTO. */
  async function meteredAuto(): Promise<Setup> {
    const view = setup({ store: await nominalStore() });
    await settle();
    view.advance(100);
    await view.user.click(screen.getByRole("button", { name: "PHASE TEST" }));
    view.advance(100);
    await view.user.keyboard("4");
    view.advance(100);
    await settle();
    view.advance(300);
    await settle();
    view.advance(300);
    await view.user.click(screen.getByRole("button", { name: "ENABLE" }));
    view.advance(600);
    return view;
  }

  const NO_LIT_SIDE = "NO LIT SIDE: choose AVG, or bring a sunlit body into view";

  it("reads NO LIT SIDE under LIT with no lit body after 0.5 s and not before, never NO IMAGE TO METER", async () => {
    const view = await meteredAuto();
    const auto = exposureReadout();
    await view.user.click(meterPanel().getByRole("button", { name: "LIT" }));
    await advanceReading(view, 300);
    const before = [statusesShown(), exposureReadout()];
    await advanceReading(view, 700);
    expect([auto, before, statusesShown()]).toEqual([
      expect.stringMatching(/AUTO$/u),
      [[], expect.stringMatching(/AUTO$/u)],
      ["NO LIT SIDE"],
    ]);
    expect(meterPanel().getByRole("button", { name: "LIT" })).toHaveAccessibleDescription(
      NO_LIT_SIDE,
    );
    expect(exposureReadout()).toMatch(/INHIBITED · NO LIT SIDE$/u);
    expect(screen.getByRole("button", { name: "ENABLE" })).toHaveAccessibleDescription(
      "NO LIT SIDE",
    );
  });

  it("clears the status at once on a change of meter, and gives the new meter its own after its window", async () => {
    const view = await meteredAuto();
    await view.user.click(meterPanel().getByRole("button", { name: "LIT" }));
    await advanceReading(view, 1000);
    await view.user.click(meterPanel().getByRole("button", { name: "DARK" }));
    // In the click's own render, with no frame since: the status goes with the meter that found
    // nothing, beside ENABLE and under AUTO NOT AVAILABLE too, and the reading stands as it is.
    const atOnce = [
      meterStatusShown(),
      screen.queryByText(/^AUTO NOT AVAILABLE/u),
      exposureReadout(),
    ];
    expect(screen.getByRole("button", { name: "ENABLE" })).toHaveAccessibleDescription(
      "NOT AVAILABLE: not yet metered",
    );
    await advanceReading(view, 300);
    const changed = [meterStatusShown(), exposureReadout()];
    await advanceReading(view, 700);
    expect([atOnce, changed, meterStatusShown()]).toEqual([
      [null, null, expect.stringMatching(/INHIBITED · NO LIT SIDE$/u)],
      [null, expect.stringMatching(/INHIBITED · NO LIT SIDE$/u)],
      "NO DARK SIDE: choose AVG, or bring a night side into view",
    ]);
    expect(meterPanel().getByRole("button", { name: "DARK" })).toHaveAccessibleDescription(
      "NO DARK SIDE: choose AVG, or bring a night side into view",
    );
    expect(exposureReadout()).toMatch(/INHIBITED · NO DARK SIDE$/u);
  });

  it("resumes AUTO by itself when a lit body is metered", async () => {
    const view = await meteredAuto();
    await view.user.click(meterPanel().getByRole("button", { name: "LIT" }));
    await advanceReading(view, 1000);
    const inhibited = exposureReadout();
    view.image.pixels = [
      ...view.image.pixels,
      { meterClass: METER_CLASS.litBody, bin: 150, count: 200 },
    ];
    await advanceReading(view, 600);
    expect([inhibited, exposureReadout(), statusesShown()]).toEqual([
      expect.stringMatching(/INHIBITED · NO LIT SIDE$/u),
      expect.stringMatching(/^EV100 -?\d+\.\d AUTO$/u),
      [],
    ]);
  });
});

describe("the VIEW display's way back to MAN (R07.T13.d)", () => {
  it("sets MAN in the wireframe after the photorealistic style, and ENABLE takes AUTO once metered", async () => {
    const { user, advance } = setup({ store: await nominalStore() });
    await settle();
    advance(100);
    await user.click(screen.getByRole("button", { name: "PHASE TEST" }));
    advance(100);
    await user.click(screen.getByRole("button", { name: "PHOTOREALISTIC" }));
    advance(100);
    await settle();
    advance(300);
    await settle();
    advance(300);
    await user.click(screen.getByRole("button", { name: "ENABLE" }));
    advance(600);
    await user.click(screen.getByRole("button", { name: "WIREFRAME" }));
    // The wireframe draws no image to meter: the system inhibits AUTO once the meter times out.
    advance(1000);
    await settle();
    advance(1000);
    const trapped = exposureReadout();
    await user.click(screen.getByRole("textbox", { name: "MAN" }));
    await user.keyboard("8.6{Enter}");
    advance(300);
    const manual = exposureReadout();
    await user.click(screen.getByRole("button", { name: "PHOTOREALISTIC" }));
    advance(100);
    await settle();
    advance(300);
    await settle();
    advance(300);
    await user.click(screen.getByRole("button", { name: "ENABLE" }));
    advance(600);
    expect([trapped, manual, exposureReadout()]).toEqual([
      expect.stringMatching(/INHIBITED · NO IMAGE TO METER$/),
      "EV100 8.6 MAN",
      expect.stringMatching(/^EV100 -?\d+\.\d AUTO$/),
    ]);
  });
});
