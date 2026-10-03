import { act, render, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import {
  bakePatch,
  initSync,
  levelTable,
  surfaceHeightM,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../engine/status";
import type { TerrainPool } from "../terrain/terrainPass";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import { FakeAdapter, FakeGpu, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import { requestAdapterOutcome } from "../engine/platform";
import type { RenderView } from "../engine/types";
import { DescentSpike, TEST_PLANET_STATEMENT, VIEWS_NOT_MADE } from "./DescentSpike";
import type { SpikeWorkers } from "./spikeRun";
import {
  answerSurfaceQuery,
  type SurfaceQueryReply,
  type SurfaceQueryRequest,
  type SurfaceQueryWorker,
} from "./surfaceQuery";

function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

beforeAll(() => {
  initSync({ module: wasmBytes() });
});

/** The query's worker, answering with the real module on a microtask, as a message would arrive. */
class InlineQueryWorker implements SurfaceQueryWorker {
  readonly #listeners: ((event: MessageEvent<SurfaceQueryReply>) => void)[] = [];

  postMessage(message: SurfaceQueryRequest, _transfer: Transferable[]): void {
    const reply = answerSurfaceQuery({ bakePatch, levelTable, surfaceHeightM }, message);
    queueMicrotask(() => {
      for (const cb of this.#listeners) {
        cb(new MessageEvent("message", { data: reply }));
      }
    });
  }

  addEventListener(type: "message" | "error", cb: never): void {
    if (type === "message") {
      this.#listeners.push(cb);
    }
  }

  terminate(): void {}
}

/** A pool that bakes nothing: the views below never draw. */
const IDLE_POOL: TerrainPool = {
  reprioritise: () => undefined,
  onBaked: () => () => undefined,
  terminate: () => undefined,
};

const WORKERS: SpikeWorkers = {
  query: () => new InlineQueryWorker(),
  pool: () => () => IDLE_POOL,
};

/** An engine source whose adapter never answers, so the spike stays acquiring its adapter. */
const PENDING_ENGINE: ViewEngineSource = {
  requestAdapter: () => new Promise(() => undefined),
  load: () => Promise.reject(new Error("never asked")),
};

/** A query worker that fails, as a module that does not load does. */
class FailingQueryWorker implements SurfaceQueryWorker {
  readonly #errors: ((event: ErrorEvent) => void)[] = [];
  terminated = false;

  postMessage(_message: SurfaceQueryRequest, _transfer: Transferable[]): void {
    queueMicrotask(() => {
      for (const cb of this.#errors) {
        cb(new ErrorEvent("error", { message: "the module did not load" }));
      }
    });
  }

  addEventListener(type: "message" | "error", cb: never): void {
    if (type === "error") {
      this.#errors.push(cb);
    }
  }

  terminate(): void {
    this.terminated = true;
  }
}

/** An engine source whose engine refuses every view, as one without a canvas context does. */
function refusingEngine(): ViewEngineSource {
  return {
    requestAdapter: () =>
      requestAdapterOutcome(
        new FakeGpu([new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] })]),
      ),
    load: async () => {
      const engine = await countingRenderEngine();
      engine.createView = () => {
        throw new Error("no context");
      };
      return engine;
    },
  };
}

/** One graphics status for every render. */
const STORE = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));

/** Lays every element out at 640 × 360, so that the canvases have a size to draw at. */
function stubLayout(): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 640, height: 360 }),
  );
}

/** A ready counting engine, its views refusing every frame if asked, and the views it made. */
function readyEngine(options: { readonly refuseFrames: boolean }): {
  readonly source: ViewEngineSource;
  readonly views: () => ReadonlyArray<RenderView>;
} {
  const views: RenderView[] = [];
  return {
    views: () => views,
    source: {
      requestAdapter: () =>
        requestAdapterOutcome(
          new FakeGpu([new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] })]),
        ),
      load: async () => {
        const engine = await countingRenderEngine();
        const make = engine.createView.bind(engine);
        engine.createView = (canvas, name) => {
          const view = make(canvas, name);
          views.push(view);
          if (options.refuseFrames) {
            view.render = () => {
              throw new Error("refused");
            };
          }
          return view;
        };
        return engine;
      },
    },
  };
}

afterEach(() => {
  vi.useRealTimers();
});

function renderSpike(
  options: { readonly engine?: ViewEngineSource; readonly workers?: SpikeWorkers } = {},
): ReturnType<typeof render> {
  return render(
    <GraphicsStatusContext value={new GraphicsStatusStore(initialGraphicsStatus("vulkan", false))}>
      <DescentSpike
        seed={5n}
        setting="low"
        ridges="off"
        workers={2}
        engineSource={options.engine ?? PENDING_ENGINE}
        spikeWorkers={options.workers ?? WORKERS}
      />
    </GraphicsStatusContext>,
  );
}

describe("DescentSpike", () => {
  it("renders its three canvases, each focusable, named and paired with its list", () => {
    renderSpike();
    const names = [
      /^VIEW, SPIKE LIT, SCRIPTED$/,
      /^VIEW, WIREFRAME, ORBIT/,
      /^VIEW, WIREFRAME, CRAFT/,
    ];
    for (const name of names) {
      const canvas = screen.getByRole("img", { name });
      expect(canvas.tabIndex).toBe(0);
      const listId = canvas.getAttribute("aria-details");
      const list = listId === null ? null : document.getElementById(listId);
      if (list === null) {
        throw new Error(`the canvas ${String(name)} names no list`);
      }
      expect(within(list).getByRole("listbox")).toBeDefined();
    }
  });

  it("states the test planet as provisional, dry and hand-parameterised", () => {
    renderSpike();
    expect(screen.getByText(TEST_PLANET_STATEMENT)).toBeDefined();
    expect(TEST_PLANET_STATEMENT).toMatch(/provisional, dry and hand-parameterised/);
  });

  it("stands under the training banner, since it draws a kept test scene", () => {
    renderSpike();
    expect(screen.getByRole("status", { name: "Mode" }).textContent).toBe("TRAINING");
  });

  it("says it is measuring the terrain, then shows the site's height", async () => {
    renderSpike();
    expect(screen.getByText(/TERRAIN MEASURING/)).toBeDefined();
    const reading = await screen.findByText(/^-?[\d,]+ m$/, undefined, { timeout: 20_000 });
    expect(reading.closest("dd")?.previousElementSibling?.textContent).toBe("Site Height");
  }, 30_000);

  it("says it is acquiring its adapter while none has answered", async () => {
    renderSpike();
    expect(await screen.findByText("GRAPHICS ACQUIRING ADAPTER")).toBeDefined();
  });

  it("says the terrain was not measured when the query fails", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    renderSpike({ workers: { ...WORKERS, query: () => new FailingQueryWorker() } });
    expect(await screen.findByText(/^TERRAIN NOT MEASURED: .*relaunch to retry$/)).toBeDefined();
  });

  it("stops the query's worker when it goes before the answer", () => {
    const worker = new FailingQueryWorker();
    const view = renderSpike({ workers: { ...WORKERS, query: () => worker } });
    view.unmount();
    expect(worker.terminated).toBe(true);
  });

  it("says the views could not be made when the engine refuses them", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    renderSpike({ engine: refusingEngine() });
    expect(await screen.findByText(VIEWS_NOT_MADE, undefined, { timeout: 20_000 })).toBeDefined();
  }, 30_000);

  it("stops and says so when the engine refuses a frame", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    fakeFramesAndTimeouts();
    stubLayout();
    const made = readyEngine({ refuseFrames: true });
    renderSpike({ engine: made.source });
    expect(await screen.findByText(VIEWS_NOT_MADE, undefined, { timeout: 20_000 })).toBeDefined();
    expect(made.views().length).toBe(3);
  }, 30_000);

  it("keeps its run when given new listeners", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    const made = readyEngine({ refuseFrames: false });
    const samples: number[] = [];
    const tree = (n: number) => (
      <GraphicsStatusContext value={STORE}>
        <DescentSpike
          seed={5n}
          setting="low"
          ridges="off"
          workers={2}
          engineSource={made.source}
          spikeWorkers={WORKERS}
          listeners={{ onFrame: () => samples.push(n) }}
        />
      </GraphicsStatusContext>
    );
    const view = render(tree(1));
    await screen.findByText(/SELECTED/, undefined, { timeout: 20_000 });
    view.rerender(tree(2));
    await act(async () => {
      vi.advanceTimersByTime(100);
      await Promise.resolve();
    });
    expect(made.views().length).toBe(3);
    expect(samples.at(-1)).toBe(2);
  }, 30_000);
});
