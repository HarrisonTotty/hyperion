/**
 * The `VIEW` display under test (plan R07, T19): rendered in its providers with fake animation
 * frames, layout and engine, and an engine whose timer numbers each submission as R01's resolves
 * do, so that a test reports each one's pass times.
 */
import type { ResponseBody } from "@hyperion/protocol";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Activity } from "react";
import { vi } from "vitest";

import { UniverseProvider } from "../components/UniverseProvider";
import type { ViewEngineSource } from "../displays/view/useViewEngine";
import { ViewDisplay } from "../displays/view/ViewDisplay";
import { ViewSceneProvider } from "../displays/view/ViewSceneProvider";
import { UniversePanel } from "../displays/galaxy/UniversePanel";
import { requestAdapterOutcome } from "../view/engine/platform";
import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../view/engine/status";
import type { RenderTarget, RenderTargetSpec, RenderView, ViewSize } from "../view/engine/types";
import type { QualitySetting } from "../view/quality/qualitySetting";
import { fakeFramesAndTimeouts } from "./fakeFramesAndTimeouts";
import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO } from "./fakeGpu";
import type { FakeView } from "./fakeRenderEngine";
import { FakeResizeObserver } from "./FakeResizeObserver";
import { type FakeViewEngine, fakeViewEngineSource } from "./fakeViewEngine";
import { FakeWebSocket } from "./FakeWebSocket";
import { anOpenedUniverse, aUniverseList } from "./galaxyFixtures";
import { SCENE_TIDAL_RADIUS_M, sceneClock, shipInSystem, sliceSceneSystem } from "./sceneFixture";
import { ServerLinkHarness } from "./ServerLinkHarness";

/**
 * The stage's laid-out width in the tests, CSS px (and device px, at a ratio of 1): 80 rem, room
 * for an instrument slot beside the primary's label block.
 */
export const STAGE_WIDTH_PX = 1280;

/** The stage's laid-out height in the tests, CSS px: 45 rem, room for both instrument slots. */
export const STAGE_HEIGHT_PX = 720;

/**
 * An open instrument slot's laid-out size in the tests, CSS px: as measured on the development
 * machine at 1920 × 1080 (R07.T19).
 */
export const SLOT_WIDTH_PX = 555;
export const SLOT_HEIGHT_PX = 254;

/** A render target the timed engine made: its spec and every size it was given, first first. */
export interface RecordedTarget {
  readonly spec: RenderTargetSpec;
  readonly sizes: ViewSize[];
}

/** A submission the timed engine numbered as a resolve, by the view or target that made it. */
interface Resolve {
  readonly frame: number;
  readonly name: string;
}

/** An engine source whose engines number each view's and target's submission as a resolve. */
export interface TimedEngineSource {
  readonly source: ViewEngineSource;
  readonly engines: FakeViewEngine[];
  /** The render targets made, first first. */
  readonly targets: RecordedTarget[];
  /**
   * Reports the times of every resolve not yet reported, one pass each of `costMs(name)`, `name`
   * the view's or target's that submitted it, as R01's timer does once their reads settle.
   */
  readonly deliver: (costMs: (name: string) => number) => void;
}

/** A source of fake engines whose views and targets count their submissions as resolves. */
export function timedEngineSource(): TimedEngineSource {
  const fake = fakeViewEngineSource();
  const targets: RecordedTarget[] = [];
  const pending: Resolve[] = [];
  const engines: FakeViewEngine[] = [];
  return {
    engines,
    targets,
    deliver: (costMs) => {
      const engine = engines.at(-1);
      for (const { frame, name } of pending.splice(0)) {
        engine?.reportPassTimes({
          frame,
          timer: "quantized",
          passes: [{ label: name, ns: costMs(name) * 1e6, bracketed: false }],
        });
      }
    },
    source: {
      ...fake.source,
      load: async (outcome, status) => {
        await fake.source.load(outcome, status);
        const engine = fake.engines.at(-1);
        if (engine === undefined) {
          throw new Error("the fake source made no engine");
        }
        engines.push(engine);
        const resolved = (name: string): void => {
          engine.passTimesFrame += 1;
          pending.push({ frame: engine.passTimesFrame, name });
        };
        const createView = engine.createView.bind(engine);
        const createRenderTarget = engine.createRenderTarget.bind(engine);
        return Object.assign(engine, {
          createView: (canvas: HTMLCanvasElement, name: string): RenderView => {
            const view = createView(canvas, name);
            const draw = view.render.bind(view);
            view.render = (frame) => {
              draw(frame);
              resolved(name);
            };
            return view;
          },
          createRenderTarget: (spec: RenderTargetSpec): RenderTarget => {
            const target = createRenderTarget(spec);
            const record: RecordedTarget = { spec, sizes: [spec.size] };
            targets.push(record);
            return {
              ...target,
              resize: (size: ViewSize): void => {
                record.sizes.push(size);
                target.resize(size);
              },
              render: (frame): void => {
                target.render(frame);
                resolved(spec.name);
              },
            };
          },
        });
      },
    },
  };
}

/** A status store whose adapter has answered: a hardware adapter, both styles offered. */
export async function nominalStore(
  features: ReadonlyArray<GPUFeatureName> = [],
): Promise<GraphicsStatusStore> {
  const store = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
  const outcome = await requestAdapterOutcome(
    new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [...features] })]),
  );
  store.dispatch({ kind: "adapter-outcome", outcome });
  return store;
}

/** The display under test. */
export interface ViewDisplayHarness {
  readonly user: ReturnType<typeof userEvent.setup>;
  /** Advances the fake clock, animation frames included, after laying out again. */
  readonly advance: (ms: number) => void;
  /** Every view the engines made, first first. */
  readonly views: () => FakeView[];
  /** The server link's socket, which the test answers as the server. */
  readonly socket: FakeWebSocket;
}

/** Renders `VIEW` in its providers with fake frames, layout and engine. */
export function renderViewDisplay(options: {
  readonly store: GraphicsStatusStore;
  readonly source: ViewEngineSource;
  readonly engines: ReadonlyArray<FakeViewEngine>;
  readonly setting?: QualitySetting;
  /** Every element's laid-out size, CSS px: {@link STAGE_WIDTH_PX} by {@link STAGE_HEIGHT_PX}. */
  readonly stagePx?: { readonly widthPx: number; readonly heightPx: number };
}): ViewDisplayHarness {
  const advanceTimers = fakeFramesAndTimeouts();
  const stage = DOMRect.fromRect({
    x: 0,
    y: 0,
    width: options.stagePx?.widthPx ?? STAGE_WIDTH_PX,
    height: options.stagePx?.heightPx ?? STAGE_HEIGHT_PX,
  });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function rect(
    this: HTMLElement,
  ) {
    // The open slots' column is as tall as its slots and the gaps between them; all else is the
    // stage.
    const slots = this.classList.contains("view-instruments") ? this.children.length : null;
    return slots === null
      ? stage
      : DOMRect.fromRect({
          x: 0,
          y: 0,
          width: slots === 0 ? 0 : SLOT_WIDTH_PX,
          height: slots === 0 ? 0 : slots * SLOT_HEIGHT_PX + (slots - 1) * 8,
        });
  });
  vi.stubGlobal("WebSocket", FakeWebSocket);
  const user = userEvent.setup({ advanceTimers });
  render(
    <ServerLinkHarness>
      <UniverseProvider>
        <GraphicsStatusContext value={options.store}>
          <UniversePanel expanded onToggle={() => undefined} />
          <ViewSceneProvider active knownSystem={null}>
            {() => (
              <Activity mode="visible">
                <ViewDisplay engineSource={options.source} setting={options.setting} />
              </Activity>
            )}
          </ViewSceneProvider>
        </GraphicsStatusContext>
      </UniverseProvider>
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return {
    user,
    advance: (ms) => {
      act(() => {
        FakeResizeObserver.resizeAll();
        vi.advanceTimersByTime(ms);
      });
    },
    views: () => options.engines.flatMap((engine) => engine.views),
    socket,
  };
}

/** Opens a universe, whose scene subscription the display then sends. */
export async function openUniverse(view: ViewDisplayHarness): Promise<void> {
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

/** Answers the scene subscription with the ship 1 AU out in the slice's system. */
export async function sceneArrives(view: ViewDisplayHarness): Promise<void> {
  const answer: ResponseBody = {
    kind: "subscribe",
    subscription: 5,
    state: {
      topic: "scene",
      sequence: 0,
      clock: sceneClock(3_000),
      ship: shipInSystem(3_000),
      system: sliceSceneSystem(),
      tidal_radius_m: SCENE_TIDAL_RADIUS_M,
      craft: [],
    },
  };
  await act(async () => {
    view.socket.serverAnswers("subscribe", () => answer);
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** Lets the engine's promises settle. */
export async function settle(): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}
