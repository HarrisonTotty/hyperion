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
import { type MarkLabelSide, PLATE_SIDE_PADDING_REM } from "../displays/view/ViewMarkLabels";
import type { OffsetPx } from "../spatial/symbols";
import { ViewSceneProvider } from "../displays/view/ViewSceneProvider";
import { UniversePanel } from "../displays/galaxy/UniversePanel";
import { requestAdapterOutcome } from "../view/engine/platform";
import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../view/engine/status";
import type {
  ComputeBindings,
  ComputeHandle,
  FrameSubmission,
  RenderEngine,
  RenderTarget,
  RenderTargetSpec,
  RenderView,
  ViewSize,
} from "../view/engine/types";
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

/**
 * The `.view` box's laid-out size in the tests unless a test gives another, CSS px: about the box
 * of a 1920 × 1080 window at 100%, which takes VIEW's full layout (R07.T19.b).
 */
export const FULL_VIEW_PX = { widthPx: 1888, heightPx: 923 } as const;

/** The `.view` box of a 1280 × 720 window at 100%, CSS px, which takes VIEW's compact layout. */
export const COMPACT_VIEW_PX = { widthPx: 1248, heightPx: 563 } as const;

/** A laid-out size, CSS px. */
export interface LaidOutPx {
  readonly widthPx: number;
  readonly heightPx: number;
}

/**
 * A mark's label's plate as the tests lay it out, CSS px: `TEST PLANET`'s at 100%, as measured on
 * the development machine in the compact layout (R07.T16.f's hidden captures).
 */
export const MARK_LABEL_PX = { widthPx: 108, heightPx: 18 } as const;

/** A laid-out box, CSS px from the page's top left, at which the stage stands. */
export interface LaidOutBoxPx extends LaidOutPx {
  readonly leftPx: number;
  readonly topPx: number;
}

/**
 * A shown mark's label as the harness lays it out, read back from its transform (R07.T16.i's
 * follow-up).
 */
export interface ShownMarkLabelPx {
  /** The side of its mark at which it stands. */
  readonly side: MarkLabelSide;
  /** Its plate's box, {@link MARK_LABEL_PX}, CSS px from the stage's top left. */
  readonly box: LaidOutBoxPx;
  /** Its mark's centre, CSS px from the stage's top left. */
  readonly centrePx: OffsetPx;
}

/** A CSS length in px as `sideLabelTransform` writes it, a number's `String`. */
const PX = String.raw`(-?\d+(?:\.\d+)?(?:e[-+]?\d+)?)px`;

/** Each side's transform as `sideLabelTransform` writes it, its two lengths captured. */
const SIDE_TRANSFORMS: ReadonlyArray<readonly [MarkLabelSide, RegExp]> = [
  ["right", new RegExp(String.raw`^translate\(${PX}, ${PX}\)$`, "u")],
  ["left", new RegExp(String.raw`^translate\(calc\(${PX} - 100%\), ${PX}\)$`, "u")],
  [
    "below",
    new RegExp(
      String.raw`^translate\(calc\(${PX} - 50%\), calc\(${PX} \+ [\d.]+rem \+ 50%\)\)$`,
      "u",
    ),
  ],
  [
    "above",
    new RegExp(
      String.raw`^translate\(calc\(${PX} - 50%\), calc\(${PX} - [\d.]+rem - 50%\)\)$`,
      "u",
    ),
  ],
];

/**
 * Where a mark's label stands, read back from the transform the drawing loop gave it at a ratio
 * of 1, with its plate laid out at {@link MARK_LABEL_PX} as {@link stubViewLayout} lays it out;
 * `null` where it is hidden.
 *
 * @param labelOffsetPx - Its mark's `DrawAnchor.labelOffsetPx`, CSS px at a ratio of 1.
 * @param remPx - The stage's rem, CSS px.
 * @throws Error where a shown label's transform is none of the four sides'.
 */
export function shownMarkLabelPx(
  label: HTMLElement,
  labelOffsetPx: number,
  remPx: number,
): ShownMarkLabelPx | null {
  if (label.style.visibility === "hidden") {
    return null;
  }
  const { transform } = label.style;
  const found = SIDE_TRANSFORMS.map(([side, form]) => ({ side, match: form.exec(transform) })).find(
    (each) => each.match !== null,
  );
  const [, xText, yText] = found?.match ?? [];
  if (found === undefined || xText === undefined || yText === undefined) {
    throw new Error(`a shown mark label's transform is none of its sides': ${transform}`);
  }
  const { side } = found;
  // The two lengths are the plate's near edge or centre across, and its mark's line or its near
  // edge's line beyond the offset down; the plate's own `translate: 0 -50%` centres it on the line.
  const xPx = Number(xText);
  const yPx = Number(yText);
  const { widthPx, heightPx } = MARK_LABEL_PX;
  const paddingPx = PLATE_SIDE_PADDING_REM * remPx;
  let leftPx: number;
  let topPx: number;
  let centrePx: OffsetPx;
  switch (side) {
    case "right":
      [leftPx, topPx] = [xPx, yPx - heightPx / 2];
      centrePx = { xPx: xPx - labelOffsetPx, yPx };
      break;
    case "left":
      [leftPx, topPx] = [xPx - widthPx, yPx - heightPx / 2];
      centrePx = { xPx: xPx + labelOffsetPx, yPx };
      break;
    case "below":
      [leftPx, topPx] = [xPx - widthPx / 2, yPx + paddingPx];
      centrePx = { xPx, yPx: yPx - labelOffsetPx };
      break;
    case "above":
      [leftPx, topPx] = [xPx - widthPx / 2, yPx - paddingPx - heightPx];
      centrePx = { xPx, yPx: yPx + labelOffsetPx };
      break;
  }
  return { side, box: { leftPx, topPx, widthPx, heightPx }, centrePx };
}

/** A box laid out at the origin. */
function rect({ widthPx, heightPx }: LaidOutPx): DOMRect {
  return DOMRect.fromRect({ x: 0, y: 0, width: widthPx, height: heightPx });
}

/**
 * Lays every element out at `stagePx`, but for the `.view` box, at `viewPx()`; each open
 * instrument slot, at {@link SLOT_WIDTH_PX} by {@link SLOT_HEIGHT_PX}; each mark's label, at
 * {@link MARK_LABEL_PX}; and the primary's label block, at `blockPx()`, by default nothing at the
 * stage's top left, so that a label is placed clear of it wherever its mark stands (R07.T16.i).
 */
export function stubViewLayout(
  stagePx: LaidOutPx,
  viewPx: () => LaidOutPx,
  blockPx: () => LaidOutBoxPx = () => ({ leftPx: 0, topPx: 0, widthPx: 0, heightPx: 0 }),
): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function laidOut(
    this: HTMLElement,
  ) {
    if (this.classList.contains("view")) {
      return rect(viewPx());
    }
    if (this.classList.contains("view-marks__label")) {
      return rect(MARK_LABEL_PX);
    }
    if (this.matches(".view__overlay > .view-label")) {
      const block = blockPx();
      return DOMRect.fromRect({
        x: block.leftPx,
        y: block.topPx,
        width: block.widthPx,
        height: block.heightPx,
      });
    }
    return this.classList.contains("view-instrument")
      ? rect({ widthPx: SLOT_WIDTH_PX, heightPx: SLOT_HEIGHT_PX })
      : rect(stagePx);
  });
}

/** A render target the timed engine made: its spec and every size it was given, first first. */
export interface RecordedTarget {
  readonly spec: RenderTargetSpec;
  readonly sizes: ViewSize[];
}

/**
 * A submission the timed engine recorded: a canvas pass, a render target's pass or a dispatch,
 * in the order made (R07.T19.c).
 */
export interface Submission {
  /**
   * What made it: the view's or the target's name, or a dispatch's first buffer's (the histogram's
   * `<view> histogram <i>`).
   */
  readonly by: string;
  /** The pass's label (`FrameSubmission.label`, or a dispatch's pass). */
  readonly label: string;
  /** Its draws' materials, in order; none for a dispatch. */
  readonly materials: ReadonlyArray<string>;
  /** Its draws' instance counts, in the materials' order (R07.T17). */
  readonly instances: ReadonlyArray<number>;
  /** A dispatch's workgroups (R07.T17), absent for a pass. */
  readonly workgroups?: ComputeWorkgroups;
}

/** A dispatch's workgroup counts, or GPU-written ones. */
type ComputeWorkgroups = Parameters<RenderEngine["dispatch"]>[2];

/** A submission's view: the name `by` begins with, up to a `:` or a space. */
export function submittedBy(submission: Submission): string {
  return submission.by.split(/[: ]/)[0] ?? submission.by;
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
  /** Every submission, first first; a dispatch is recorded but numbered as no resolve. */
  readonly submissions: ReadonlyArray<Submission>;
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
  const submissions: Submission[] = [];
  const pending: Resolve[] = [];
  const engines: FakeViewEngine[] = [];
  const submitted = (by: string, frame: FrameSubmission): void => {
    submissions.push({
      by,
      label: frame.label,
      materials: frame.draws.map((draw) => draw.material.name),
      instances: frame.draws.map((draw) => draw.instanceCount ?? 1),
    });
  };
  return {
    engines,
    targets,
    submissions,
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
              submitted(name, frame);
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
                submitted(spec.name, frame);
                resolved(spec.name);
              },
            };
          },
          dispatch: (
            kernel: ComputeHandle,
            bindings: ComputeBindings,
            workgroups: ComputeWorkgroups,
            pass?: string,
          ): void => {
            submissions.push({
              by: Object.values(bindings.buffers)[0]?.name ?? kernel.name,
              label: pass ?? "compute",
              materials: [],
              instances: [],
              workgroups,
            });
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
  /** Lays the `.view` box out at another size, as a window resize does, and lays out again. */
  readonly resizeView: (viewPx: LaidOutPx) => void;
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
  readonly stagePx?: LaidOutPx;
  /** The `.view` box's laid-out size, CSS px: {@link FULL_VIEW_PX}, the full layout. */
  readonly viewPx?: LaidOutPx;
  /** The primary's label block's laid-out box, each time it is measured: nothing by default. */
  readonly blockPx?: () => LaidOutBoxPx;
}): ViewDisplayHarness {
  const advanceTimers = fakeFramesAndTimeouts();
  let viewPx: LaidOutPx = options.viewPx ?? FULL_VIEW_PX;
  stubViewLayout(
    options.stagePx ?? { widthPx: STAGE_WIDTH_PX, heightPx: STAGE_HEIGHT_PX },
    () => viewPx,
    options.blockPx,
  );
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
                {/* In the console frame's work area, as `ConsoleFrame` lays VIEW out. */}
                <main className="console__work console__work--view">
                  <ViewDisplay engineSource={options.source} setting={options.setting} />
                </main>
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
    resizeView: (next) => {
      viewPx = next;
      act(() => {
        FakeResizeObserver.resizeAll();
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
