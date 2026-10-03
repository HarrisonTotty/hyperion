import { act, render, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import {
  bakePatch,
  initSync,
  levelTable,
  omittedSigmaM,
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
import {
  DESCENT_REFUSED,
  DescentSpike,
  groundSpeedReading,
  heightReading,
  heightUnitFor,
  patchesReading,
  TEST_PLANET_STATEMENT,
  VIEWS_NOT_MADE,
} from "./DescentSpike";
import { DescentRefused, type prepareDescent, type SpikeWorkers } from "./spikeRun";
import {
  answerSurfaceQuery,
  type SurfaceQueryModule,
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
  /**
   * The module's bake, or a stand-in whose every patch spans `flatM` (the floors then sit at
   * `flatM` plus ε): the real floors take some 600 bakes, which these tests do not need.
   */
  readonly bakePatch: SurfaceQueryModule["bakePatch"];

  constructor(flatM: number | null = 0) {
    this.bakePatch =
      flatM === null
        ? bakePatch
        : () => ({ heightRangeM: () => Float32Array.of(flatM, flatM), free: () => undefined });
  }

  readonly #listeners: ((event: MessageEvent<SurfaceQueryReply>) => void)[] = [];

  postMessage(message: SurfaceQueryRequest, _transfer: Transferable[]): void {
    const reply = answerSurfaceQuery(
      { bakePatch: this.bakePatch, levelTable, omittedSigmaM, surfaceHeightM },
      message,
    );
    queueMicrotask(() => {
      this.deliver(reply);
    });
  }

  /** Hands `reply` to the listeners, as a message from the worker. */
  protected deliver(reply: SurfaceQueryReply): void {
    for (const cb of this.#listeners) {
      cb(new MessageEvent("message", { data: reply }));
    }
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
  options: {
    readonly engine?: ViewEngineSource;
    readonly workers?: SpikeWorkers;
    readonly prepare?: typeof prepareDescent;
  } = {},
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
        prepare={options.prepare}
      />
    </GraphicsStatusContext>,
  );
}

describe("DescentSpike", () => {
  it("renders its three canvases, each focusable, named and paired with its list", () => {
    renderSpike();
    const names = [
      /^VIEW, SPIKE LIT, SCRIPTED$/,
      /^VIEW, WIREFRAME, ORBIT, SCRIPTED$/,
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

  it("stands under the measurement banner, flown by script and recording", () => {
    renderSpike();
    expect(screen.getByRole("status", { name: "Mode" }).textContent).toBe("MEASUREMENT");
  });

  it("says it is measuring the terrain, then shows the site's height", async () => {
    renderSpike();
    expect(screen.getByText("MEASURING TERRAIN: landing site and ground track")).toBeDefined();
    const reading = await screen.findByText(/^[+-][\d,]+ m$/, undefined, { timeout: 20_000 });
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

  it("refuses to fly when the script cannot clear the measured terrain", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    // A measurement the script cannot fly, as T13.a's lift limit refuses it.
    renderSpike({
      prepare: () =>
        Promise.reject(
          new DescentRefused(5n, new RangeError("the descent cannot clear its floors")),
        ),
    });
    expect(await screen.findByText(DESCENT_REFUSED, undefined, { timeout: 20_000 })).toBeDefined();
  }, 30_000);

  it("names its readings as the nomenclature does", () => {
    renderSpike();
    for (const label of [
      "Spike Seed",
      "Quality",
      "Segment",
      "Script Time",
      "Height Above Site",
      "Ground Speed",
      "Vertical Speed",
      "Camera ELV",
      "Site Height",
      "Finest Terrain Held",
      "Patches",
    ]) {
      expect(screen.getByText(label, { selector: "dt" })).toBeDefined();
    }
  });
});

describe("the descent panel's readings", () => {
  it("switches the height's unit with hysteresis", () => {
    expect(heightUnitFor(10_000, "m")).toBe("m");
    expect(heightUnitFor(10_600, "m")).toBe("km");
    expect(heightUnitFor(10_000, "km")).toBe("km");
    expect(heightUnitFor(9_400, "km")).toBe("m");
  });

  it("writes the height to the precision the operator can act on", () => {
    expect(heightReading(1.94, "m")).toBe("1.9 m");
    // Grouped from five digits, as `formatNumber` groups every reading.
    expect(heightReading(9_850.4, "m")).toBe("9850 m");
    expect(heightReading(-12_000, "m")).toBe("-12,000 m");
    expect(heightReading(400_000, "km")).toBe("400.0 km");
  });

  it("writes ground speed whole from 100 m/s", () => {
    expect(groundSpeedReading(7_670)).toBe("7670 m/s");
    expect(groundSpeedReading(12.345)).toBe("12.35 m/s");
  });

  it("binds each patch count to its word, breaking only at the separators", () => {
    const text = patchesReading({ selected: 73, drawn: 5, standingIn: 0, missing: 0 });
    expect(text.split("\u00a0· ")).toEqual([
      "73\u00a0SELECTED",
      "5\u00a0DRAWN",
      "0\u00a0STANDING\u00a0IN",
      "0\u00a0MISSING",
    ]);
    expect(text.replaceAll("\u00a0· ", "")).not.toContain(" ");
  });
});
