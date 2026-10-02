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
import { FakeWebSocket } from "../../test/FakeWebSocket";
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
import { UniverseProvider } from "../../components/UniverseProvider";
import { UniversePanel } from "../galaxy/UniversePanel";
import { rotate } from "../../view/camera/quaternion";
import { fakeViewEngineSource } from "../../test/fakeViewEngine";
import { stubMatchMedia } from "../../test/stubMatchMedia";
import { DEFAULT_FOV_DEG } from "../../view/camera/projection";
import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../../view/engine/status";
import type { FrameSubmission } from "../../view/engine/types";
import { precisionScene } from "../../view/scenes/precision";
import { buildWireframeDrawList } from "../../view/wireframe/drawList";
import type { ViewEngineSource } from "./useViewEngine";
import { ViewDisplay } from "./ViewDisplay";
import { ViewSceneProvider } from "./ViewSceneProvider";
import { runPose, STAR_SOURCE, STARS_WITHOUT_POSITION, startRun, stepRun } from "./viewRun";

const WIDTH_PX = 640;
const HEIGHT_PX = 360;

afterEach(() => {
  vi.useRealTimers();
});

function stubLayout(): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: WIDTH_PX, height: HEIGHT_PX }),
  );
}

interface Setup {
  readonly user: ReturnType<typeof userEvent.setup>;
  readonly advance: (ms: number) => void;
  readonly views: () => FakeView[];
  readonly lastFrame: () => FrameSubmission | undefined;
  readonly engines: ReturnType<typeof fakeViewEngineSource>["engines"];
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

/** The number of line draws in a frame. */
function lineDraws(frame: FrameSubmission | undefined): number {
  return frame?.draws.filter((draw) => draw.material.name === "wireframe:lines").length ?? 0;
}

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

  it("names its canvas for its style and camera", async () => {
    const { advance } = setup();
    await settle();
    advance(100);
    expect(screen.getByRole("application", { name: "VIEW, WIREFRAME, SEAT" }).tagName).toBe(
      "CANVAS",
    );
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
    const block = screen.getByText("VIEW").parentElement;
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
      { lowSetting: false, ev100: -1, selection: null, destination: null, remPx: 16 },
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
    await user.keyboard("{PageUp}");
    advance(300);
    expect(rate).toHaveTextContent("RATE 3.16 km/s");
    expect(screen.queryByText(/NOT AVAILABLE: RATE/)).not.toBeInTheDocument();
    for (let i = 0; i < 8; i += 1) {
      // Each press is one step; they are sequential by nature.
      // oxlint-disable-next-line no-await-in-loop
      await user.keyboard("{PageDown}");
    }
    advance(300);
    expect(rate).toHaveTextContent("RATE 1.00 m/s");
    expect(screen.getByText("NOT AVAILABLE: RATE at its lowest step")).toBeInTheDocument();
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
    expect(screen.getByText("VIEW").parentElement?.textContent).toMatch(/FRAME CHANGE TEST/);
  });

  it("says AUTO is not available while there is no image to meter", async () => {
    setup();
    await settle();
    expect(screen.getByText("AUTO NOT AVAILABLE: NO IMAGE TO METER")).toBeInTheDocument();
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
      screen.getByText(/^STARS 1 DRAWN · 0 WITHOUT V · RADII 620\/360\/210\/60 ly$/),
    ]).toEqual([true, expect.anything()]);
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
  return screen.getByText("VIEW").parentElement?.textContent ?? "";
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
    ]).toEqual([true, true, 0]);
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
      screen.getByText(/^UT /).classList.contains("stale"),
      screen.getByRole("heading", { name: "Targets stale" }),
      labelBlock().includes("PRECISION TEST"),
      screen
        .getAllByRole("option")
        .every((row) => row.getAttribute("aria-label")?.endsWith(", stale") === true),
    ]).toEqual([expect.anything(), true, expect.anything(), false, true]);
  });

  it("says a camera report was refused, with the server's reason", async () => {
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
      screen.getByText("CAMERA REPORT REFUSED: a camera is outside the scene's reach"),
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
