import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Activity } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { norm, vec3 } from "../../geometry/vec3";
import { readTokens } from "../../spatial/paint";
import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import type { FakeView } from "../../test/fakeRenderEngine";
import { FakeResizeObserver } from "../../test/FakeResizeObserver";
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
import { runPose, startRun, stepRun } from "./viewRun";

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
  const tree = (mode: "visible" | "hidden") => (
    <GraphicsStatusContext value={store}>
      <Activity mode={mode}>
        <ViewDisplay engineSource={source} />
      </Activity>
    </GraphicsStatusContext>
  );
  const user = userEvent.setup({ advanceTimers });
  const view = render(tree("visible"));
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
