import { act, getDefaultNormalizer, render, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import { ViewFrameDrawer } from "../../displays/view/viewFrameDrawer";
import { vec3 } from "../../geometry/vec3";
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
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import type { RenderStyle } from "../camera/state";
import { requestAdapterOutcome } from "../engine/platform";
import type { RenderView } from "../engine/types";
import { terrainTestGolden } from "../scenes/terrainScene";
import { CoverageView } from "../terrain/coverage";
import { finestLevel } from "../terrain/cube";
import { planetGeometry } from "../terrain/planet";
import { NO_BREAK_SPACE, SURVEY_PENDING } from "../terrain/readouts";
import { MAX_CONSECUTIVE_WORKER_FAILURES } from "../terrain/workers/pool";
import { bodySiteOf, type MeasureDescent, NoLandingSite, siteCandidates } from "./bodyDescent";
import { DescentProfile, landingSiteOf, trackStretches } from "./descentProfile";
import {
  DESCENT_REFUSED,
  DescentSpike,
  groundSpeedReading,
  heightReading,
  heightUnitFor,
  patchesReading,
  siteElevationReading,
  siteSurveyReading,
  TEST_PLANET_STATEMENT,
  VIEWS_NOT_MADE,
} from "./DescentSpike";
import {
  DescentRefused,
  type PreparedDescent,
  type prepareDescent,
  type SiteReading,
  type SpikeListeners,
  type SpikeWorkers,
} from "./spikeRun";
import { contactRule } from "./spikeScene";
import { spikeWorldNamed } from "./spikeWorld";
import type { TerrainVariant } from "../quality/qualitySetting";
import { type BakeSettings, PALETTE_NONE } from "../terrain/workers/messages";
import {
  answerSurfaceQuery,
  type SurfaceQueryModule,
  type SurfaceQueryReply,
  type SurfaceQueryRequest,
  type SurfaceQueryWorker,
} from "./surfaceQuery";
import { statusLine } from "../../test/statusLine";

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
  height: () => new ScriptedHeightWorker(),
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

/**
 * A ready counting engine, its views refusing every frame if asked and every material failing if
 * asked, and the views it made.
 */
function readyEngine(options: {
  readonly refuseFrames: boolean;
  readonly failMaterials?: boolean;
}): {
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
        if (options.failMaterials === true) {
          engine.failMaterials(() => true);
        }
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
    readonly listeners?: SpikeListeners;
    readonly variant?: TerrainVariant;
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
        listeners={options.listeners}
        variant={options.variant}
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
    expect(screen.getByText(statusLine(TEST_PLANET_STATEMENT))).toBeDefined();
    expect(TEST_PLANET_STATEMENT).toMatch(/provisional, dry and hand-parameterised/);
  });

  it("stands under the measurement banner, flown by script and recording", () => {
    renderSpike();
    expect(screen.getByRole("status", { name: "Mode" }).textContent).toBe("MEASUREMENT");
  });

  it("says it is measuring the terrain, then shows the site's elevation", async () => {
    renderSpike();
    expect(
      screen.getByText(statusLine("MEASURING TERRAIN: landing site and ground track")),
    ).toBeDefined();
    // ELEVATION's form: 0.1 m, `-` below the datum and no `+`.
    const reading = await screen.findByText(/^-?[\d,]+\.\d m$/, undefined, { timeout: 20_000 });
    expect(reading.closest("dd")?.previousElementSibling?.textContent).toBe("Site Elevation");
  }, 30_000);

  it("says it is acquiring its adapter while none has answered", async () => {
    renderSpike();
    expect(await screen.findByText("GRAPHICS ACQUIRING ADAPTER")).toBeDefined();
  });

  it("says the terrain was not measured when the query fails", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    renderSpike({ workers: { ...WORKERS, query: () => new FailingQueryWorker() } });
    expect(
      await screen.findByText(statusLine(/^TERRAIN NOT MEASURED: .*relaunch to retry$/)),
    ).toBeDefined();
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
    expect(
      await screen.findByText(statusLine(VIEWS_NOT_MADE), undefined, { timeout: 20_000 }),
    ).toBeDefined();
  }, 30_000);

  it("stops and says so when the engine refuses a frame", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    fakeFramesAndTimeouts();
    stubLayout();
    const made = readyEngine({ refuseFrames: true });
    renderSpike({ engine: made.source });
    expect(
      await screen.findByText(statusLine(VIEWS_NOT_MADE), undefined, { timeout: 20_000 }),
    ).toBeDefined();
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
    expect(
      await screen.findByText(statusLine(DESCENT_REFUSED), undefined, { timeout: 20_000 }),
    ).toBeDefined();
  }, 30_000);

  it("tells its listeners each failure that ends the run, with the status it shows", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const failed: string[] = [];
    const listeners: SpikeListeners = { onFailed: (status) => failed.push(status) };
    const refused = renderSpike({
      listeners,
      prepare: () =>
        Promise.reject(
          new DescentRefused(5n, new RangeError("the descent cannot clear its floors")),
        ),
    });
    await screen.findByText(statusLine(DESCENT_REFUSED), undefined, { timeout: 20_000 });
    refused.unmount();
    const unmeasured = renderSpike({
      listeners,
      workers: { ...WORKERS, query: () => new FailingQueryWorker() },
    });
    await screen.findByText(/^TERRAIN NOT MEASURED/);
    unmeasured.unmount();
    renderSpike({ listeners, engine: refusingEngine() });
    await screen.findByText(statusLine(VIEWS_NOT_MADE), undefined, { timeout: 20_000 });
    expect(failed).toEqual([
      DESCENT_REFUSED,
      expect.stringMatching(/^TERRAIN NOT MEASURED/),
      VIEWS_NOT_MADE,
    ]);
  }, 60_000);

  it("hands its listeners the measured descent and each selection, and the pool the variant", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    const made = readyEngine({ refuseFrames: false });
    const prepared: number[] = [];
    const selected: number[] = [];
    const bakes: BakeSettings[] = [];
    renderSpike({
      engine: made.source,
      workers: {
        ...WORKERS,
        pool: () => (bake) => {
          bakes.push(bake);
          return IDLE_POOL;
        },
      },
      variant: { normals: "double" },
      listeners: {
        onPrepared: (descent) => prepared.push(descent.omittedSigmaM.length),
        onSelect: (_input, selection) => selected.push(selection.patches.size),
      },
    });
    await screen.findByText(/SELECTED/, undefined, { timeout: 20_000 });
    expect(prepared).toEqual([25]);
    expect(selected.length).toBeGreaterThan(0);
    expect(bakes[0]).toMatchObject({ vertexPath: "face-differences", normals: "double" });
  }, 30_000);

  it("states no survey line, and notes no survey edge, over the test planet", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    renderSpike({ engine: readyEngine({ refuseFrames: false }).source });
    await screen.findByText(/SELECTED/, undefined, { timeout: 20_000 });
    expect([
      screen.queryByText(/SURVEY COVERAGE/u),
      screen.queryByText(/^TERRAIN: NOT YET MODELLED$/u),
      screen.queryByRole("note"),
    ]).toEqual([null, null, null]);
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
      "Site Elevation",
      "Finest Terrain Held",
      "Patches",
    ]) {
      expect(screen.getByText(label, { selector: "dt" })).toBeDefined();
    }
  });
});

/** `TERRAIN TEST`'s world, and a descent over it at the bare datum, as a measurement would give. */
const MOON = spikeWorldNamed("terrain-test");
const MOON_PROFILE = new DescentProfile(MOON.figure, landingSiteOf(7n));
const MOON_STRETCHES = trackStretches(MOON_PROFILE);
const MOON_CONTACT = contactRule(MOON_PROFILE);

/** The descent over `TERRAIN TEST`'s moon with `siteReading` at its site. */
function moonDescent(siteReading: SiteReading | null): PreparedDescent {
  return {
    planet: planetGeometry(MOON.figure, null),
    profile: MOON_PROFILE,
    contact: MOON_CONTACT,
    siteHeightM: 0,
    stretches: MOON_STRETCHES,
    stretchMaxHeightsM: MOON_STRETCHES.map(() => 0),
    trackMaxHeightM: 0,
    omittedSigmaM: new Float64Array(25).fill(Infinity),
    siteReading,
  };
}

/** The spike over `TERRAIN TEST`'s moon, its measurement `measure`, and its height workers. */
interface MoonSpike {
  readonly view: ReturnType<typeof render>;
  /** The session's height workers, scripted, as it started them. */
  readonly heightWorkers: ReadonlyArray<ScriptedHeightWorker>;
}

/**
 * The spike over `TERRAIN TEST`'s moon, on a ready engine unless given another, its measurement
 * `measure`.
 */
function renderMoon(
  measure: MeasureDescent,
  options: {
    readonly style?: RenderStyle;
    readonly listeners?: SpikeListeners;
    readonly engine?: ViewEngineSource;
  } = {},
): MoonSpike {
  const heightWorkers: ScriptedHeightWorker[] = [];
  const spikeWorkers: SpikeWorkers = {
    ...WORKERS,
    height: () => {
      const worker = new ScriptedHeightWorker();
      heightWorkers.push(worker);
      return worker;
    },
  };
  const view = render(
    <GraphicsStatusContext value={STORE}>
      <DescentSpike
        seed={7n}
        setting="low"
        ridges="off"
        workers={1}
        world="terrain-test"
        style={options.style}
        engineSource={options.engine ?? readyEngine({ refuseFrames: false }).source}
        spikeWorkers={spikeWorkers}
        measure={measure}
        listeners={options.listeners}
      />
    </GraphicsStatusContext>,
  );
  return { view, heightWorkers };
}

/** Plays `steps` × 300 ms of frames, letting each step's work settle. */
async function play(steps: number): Promise<void> {
  for (let step = 0; step < steps; step += 1) {
    // Each step waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
  }
}

/** The main view's label block: the `VIEW` block on its stage, not an instrument's. */
function mainLabelBlock(): HTMLElement {
  const block = screen
    .getAllByText("VIEW", { selector: "p" })
    .map((title) => title.parentElement)
    .find((each) => each !== null && each.closest(".spike-instrument") === null);
  if (block === undefined || block === null) {
    throw new Error("the spike shows no main label block");
  }
  return block;
}

/** A label block's or plate's reading of `label`: its value's text, or `null` where it has none. */
function readingIn(block: HTMLElement, label: string): string | null {
  return (
    within(block).queryByText(label, { selector: "dt" })?.nextElementSibling?.textContent ?? null
  );
}

/** The main view's statements, in order. */
function statementsOf(block: HTMLElement): ReadonlyArray<string> {
  return Array.from(block.querySelectorAll(".view-label__statement"), (each) => each.textContent);
}

/**
 * The main view's fault lines, its status lines that stand in `--status-caution` (`StatusLine`'s
 * fault), by their words.
 */
function cautionLines(): ReadonlyArray<string> {
  return within(mainLabelBlock())
    .queryAllByRole("status")
    .filter((each) => each.classList.contains("request-status__text--fault"))
    .map((each) => each.textContent);
}

/** An engine source whose adapter request finds none, as a machine with no adapter does. */
const NO_ADAPTER_ENGINE: ViewEngineSource = {
  requestAdapter: () => Promise.resolve({ kind: "no-adapter" }),
  load: () => Promise.reject(new Error("never asked")),
};

/** The `Site Elevation` reading's text. */
function siteElevationText(): string | null {
  return (
    screen.getByText("Site Elevation", { selector: "dt" }).nextElementSibling?.textContent ?? null
  );
}

/** The measuring status's line. */
const MEASURING_LINE = statusLine("MEASURING TERRAIN: landing site and ground track");

describe("DescentSpike over a generated world (R10.T14.c)", () => {
  it("says it is measuring the terrain until the session's measurement answers", async () => {
    fakeFramesAndTimeouts();
    const asked: Array<readonly [string, bigint]> = [];
    renderMoon((_session, world, seed) => {
      asked.push([world.bodyDesignation, seed]);
      return new Promise(() => undefined);
    });
    await vi.waitFor(() => {
      expect(asked).toEqual([["TEST MOON", 7n]]);
    });
    expect(screen.getByText(MEASURING_LINE)).toBeDefined();
  });

  it("stops the measurement when it goes before the answer", async () => {
    fakeFramesAndTimeouts();
    const asked: AbortSignal[] = [];
    const view = renderMoon((_session, _world, _seed, signal) => {
      asked.push(signal);
      return new Promise(() => undefined);
    });
    await vi.waitFor(() => {
      expect(asked).toHaveLength(1);
    });
    view.view.unmount();
    expect(asked.map((signal) => signal.aborted)).toEqual([true]);
  });

  it("says it is acquiring its adapter, not measuring, while the engine is pending", () => {
    renderMoon(() => Promise.resolve(moonDescent(null)), { engine: PENDING_ENGINE });
    expect([
      screen.queryByText(statusLine("GRAPHICS ACQUIRING ADAPTER")) !== null,
      screen.queryByText(MEASURING_LINE),
    ]).toEqual([true, null]);
  });

  it("says no adapter answered, not measuring, when the engine is unavailable", async () => {
    renderMoon(() => Promise.resolve(moonDescent(null)), { engine: NO_ADAPTER_ENGINE });
    expect(
      await screen.findByText(
        statusLine("GRAPHICS NO ADAPTER: views not available, relaunch to retry"),
      ),
    ).toBeDefined();
    expect(screen.queryByText(MEASURING_LINE)).toBeNull();
  });

  it("states no test planet, and the survey's reading at the site, Estimated with its uncertainty", async () => {
    fakeFramesAndTimeouts();
    const reading: SiteReading = {
      kind: "ground",
      elevationM: -410.2,
      sigmaM: 18,
      resolved: false,
    };
    renderMoon(() => Promise.resolve(moonDescent(reading)));
    const expected = `~-410 m${NO_BREAK_SPACE}±${NO_BREAK_SPACE}18 m`;
    const site = await screen.findByText(expected, {
      normalizer: getDefaultNormalizer({ trim: false, collapseWhitespace: false }),
    });
    expect(site.closest("dd")?.previousElementSibling?.textContent).toBe("Site Elevation");
    expect(siteSurveyReading(reading)).toBe(expected);
    expect(screen.queryByText(statusLine(TEST_PLANET_STATEMENT))).toBeNull();
    expect(screen.queryByText(MEASURING_LINE)).toBeNull();
  });

  it("states the site's elevation resolved where the survey resolves it", async () => {
    fakeFramesAndTimeouts();
    renderMoon(() =>
      Promise.resolve(
        moonDescent({ kind: "ground", elevationM: -410.2, sigmaM: 0, resolved: true }),
      ),
    );
    await vi.waitFor(() => {
      expect(siteElevationText()).toBe("-410.2 m");
    });
  });

  it.each([
    ["not-surveyed", "NOT SURVEYED"],
    ["no-ground", "NO GROUND"],
  ] as const)(
    "states the readout's own %s at the site where it met no surveyed ground: %s",
    async (kind, words) => {
      fakeFramesAndTimeouts();
      renderMoon(() => Promise.resolve(moonDescent({ kind })));
      await vi.waitFor(() => {
        expect(siteElevationText()).toBe(words);
      });
    },
  );

  it("reads the site's elevation as missing while the survey gave no reading there", async () => {
    fakeFramesAndTimeouts();
    const prepared: number[] = [];
    renderMoon(() => Promise.resolve(moonDescent(null)), {
      listeners: { onPrepared: () => prepared.push(1) },
    });
    await vi.waitFor(() => {
      expect(prepared).toEqual([1]);
    });
    // `onPrepared` is told before React re-renders the measured state, so the line's going and the
    // site's reading are waited for too, not read at once.
    await vi.waitFor(() => {
      expect(screen.queryByText(MEASURING_LINE)).toBeNull();
    });
    expect(siteElevationText()).toBe("—");
  });

  it("refuses to fly when the script cannot clear the world's terrain", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    fakeFramesAndTimeouts();
    renderMoon(() =>
      Promise.reject(new DescentRefused(7n, new RangeError("the descent cannot clear its floors"))),
    );
    expect(await screen.findByText(statusLine(DESCENT_REFUSED))).toBeDefined();
  });

  it("says the terrain was not measured where the survey holds no landing site", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    fakeFramesAndTimeouts();
    const failed: string[] = [];
    renderMoon(() => Promise.reject(new NoLandingSite(MOON.body, "nothing is surveyed")), {
      listeners: { onFailed: (status) => failed.push(status) },
    });
    expect(
      await screen.findByText(statusLine(/^TERRAIN NOT MEASURED: .*relaunch to retry$/)),
    ).toBeDefined();
    expect(failed).toEqual([expect.stringMatching(/^TERRAIN NOT MEASURED/)]);
  });

  it.each([
    ["photorealistic, the default", undefined, "VIEW, PHOTOREALISTIC, SCRIPTED"],
    ["wireframe, as asked", "wireframe", "VIEW, WIREFRAME, SCRIPTED"],
  ] as const)("names its main view in its style, %s, before it draws", (_how, style, name) => {
    render(
      <GraphicsStatusContext value={STORE}>
        <DescentSpike
          seed={7n}
          setting="low"
          ridges="off"
          workers={1}
          world="terrain-test"
          style={style}
          engineSource={PENDING_ENGINE}
          spikeWorkers={WORKERS}
        />
      </GraphicsStatusContext>,
    );
    expect(screen.getByRole("img", { name })).toBeDefined();
    expect(screen.queryByRole("img", { name: /SPIKE LIT/ })).toBeNull();
  });
});

/**
 * The moon's descent at the bare datum to the site the measurement draws for seed 7 inside
 * `TERRAIN TEST`'s larger survey (`bodySiteOf` of its third payload's coverage), its site's reading
 * not asked.
 */
const SURVEYED_DESCENT: PreparedDescent = (() => {
  const golden = terrainTestGolden();
  const payload = golden.payloads[2];
  if (payload === undefined) {
    throw new Error("the terrain golden has its third payload");
  }
  const merged = CoverageView.empty(golden.header.level).merged(payload.coverage, payload.revision);
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  const site = bodySiteOf(
    siteCandidates(merged.coverage),
    7n,
    0,
    finestLevel(MOON.figure.equatorialRadiusM),
  );
  if (site === null) {
    throw new Error("the larger survey holds a site");
  }
  const profile = new DescentProfile(MOON.figure, site.site);
  const stretches = trackStretches(profile);
  return {
    ...moonDescent(null),
    profile,
    contact: contactRule(profile),
    stretches,
    stretchMaxHeightsM: stretches.map(() => 0),
  };
})();

/** The moon's descent into its survey, measured at once. */
function measured(): Promise<PreparedDescent> {
  return Promise.resolve(SURVEYED_DESCENT);
}

/** Answers each bake `worker` was asked for, flat at the datum and surveyed throughout. */
function answerEveryBake(worker: ScriptedHeightWorker): void {
  for (const request of worker.pendingBakes) {
    const { key, settings } = request;
    const n = settings.normals === "double" ? 129 : 65;
    worker.reply({
      kind: "baked",
      id: request.id,
      bake: {
        key,
        generation: request.generation,
        originM: vec3(0, 0, MOON.figure.polarRadiusM),
        heights: new Float32Array(65 * 65 * 2),
        offsets: settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
        normals: new Float16Array(n * n * 2),
        heightRangeM: [0, 0],
        boundingRadiusM: 1,
        originHeightM: 0,
        skirtDepthM: 1,
        layers: {
          palette: new Uint8Array(16).fill(PALETTE_NONE),
          weights: new Uint8Array(),
          surveyMask: new Uint8Array(65 * 65).fill(255),
          fieldLevel: 5,
          boundM: 1,
          sigmaM: 0,
          complete: true,
          unsurveyedCells: new Uint32Array(),
          photometry: new Float32Array(40),
          horizonNear: new Uint32Array(),
          horizonFar: new Uint32Array(),
          horizonComplete: true,
          horizonRefusedCells: new Uint32Array(),
        },
      },
    });
  }
}

/** Plays frames 16 ms at a time until the first publish, the panel's script time standing. */
async function untilFirstPublish(): Promise<void> {
  for (let step = 0; step < 200; step += 1) {
    const time = screen.getByText("Script Time", { selector: "dt" }).nextElementSibling;
    if (time !== null && time.textContent !== "—") {
      return;
    }
    // Each step waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await act(async () => {
      await vi.advanceTimersByTimeAsync(16);
    });
  }
  throw new Error("the spike published no frame");
}

/** A survey-edge note of `TEST MOON` by its accessible name, whatever its range. */
const TEST_MOON_EDGE = /^TEST MOON, survey edge, range [\d.,]+ (?:m|km|Mm)$/u;

describe("DescentSpike's main view over a generated world, drawn (R10.T14.c)", () => {
  it("states SURVEY COVERAGE: PENDING on its lower plate until the fixture answers, then the line's values", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    renderMoon(measured, { style: "wireframe" });
    await untilFirstPublish();
    const pending = screen.queryByText(SURVEY_PENDING) !== null;
    await play(8);
    expect([
      pending,
      screen.queryByText(SURVEY_PENDING),
      screen.getByText("SURVEY COVERAGE", { selector: "dt" }).nextElementSibling?.textContent,
    ]).toEqual([true, null, expect.stringMatching(/^[\d.]+ % · FINEST [\d.]+ k?m$/u)]);
  });

  it("states no DATUM and no GROUND AT LINE OF SIGHT", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    renderMoon(measured);
    await play(9);
    expect([
      screen.queryByText("SURVEY COVERAGE", { selector: "dt" }) !== null,
      screen.queryByText("DATUM", { selector: "dt" }),
      screen.queryByText("GROUND AT LINE OF SIGHT"),
    ]).toEqual([true, null, null]);
  });

  it("asks the height workers no ground readout after its measurement", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    const asks: number[] = [];
    const spike = renderMoon((session) => {
      vi.spyOn(session, "groundReadout").mockImplementation(() => {
        asks.push(1);
        return Promise.resolve(null);
      });
      return measured();
    });
    await play(9);
    expect([
      screen.queryByText("SURVEY COVERAGE", { selector: "dt" }) !== null,
      asks,
      spike.heightWorkers.flatMap((worker) =>
        worker.received.filter((message) => message.kind === "ground-readout"),
      ),
    ]).toEqual([true, [], []]);
  });

  it("notes TEST MOON's survey edge in its list while the drawn frame marks it, and none after", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    // The drawn frame marks the moon's edge 4.21 km off, wherever the script has the craft.
    const edge = { marked: true };
    // The drawer's own draw, kept before the spy takes its place and called with its own `this`.
    // oxlint-disable-next-line typescript/unbound-method
    const draw = ViewFrameDrawer.prototype.draw;
    vi.spyOn(ViewFrameDrawer.prototype, "draw").mockImplementation(function (
      this: ViewFrameDrawer,
      inputs,
    ) {
      const drawn = draw.call(this, inputs);
      return drawn === null || !edge.marked
        ? drawn
        : { ...drawn, surveyEdges: [{ body: MOON.body, nearestRangeM: 4_210 }] };
    });
    renderMoon(measured, { style: "wireframe" });
    await play(2);
    const note = screen.getByRole("note", { name: TEST_MOON_EDGE });
    edge.marked = false;
    await play(2);
    // The spike's scene has its craft for the own ship, so that the range is the ship's, as the
    // list's are, and not FROM CAMERA.
    expect([note.textContent, screen.queryByRole("note")]).toEqual([
      "SURVEY EDGE TEST MOON 4.21 km",
      null,
    ]);
  });

  it("reads the style drawn on its STYLE line, the photorealistic image's statements after the others", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    renderMoon(measured, { style: "photorealistic" });
    await untilFirstPublish();
    const preparing = [readingIn(mainLabelBlock(), "STYLE"), statementsOf(mainLabelBlock())];
    await play(8);
    expect([
      preparing,
      readingIn(mainLabelBlock(), "STYLE"),
      statementsOf(mainLabelBlock()),
    ]).toEqual([
      ["WIREFRAME", ["POSITIONS AS SEEN FROM SHIP", "PHOTOREALISTIC: PREPARING"]],
      "PHOTOREALISTIC",
      ["POSITIONS AS SEEN FROM SHIP", "TERRAIN: STREAMING", "CRAFT PHOTOMETRY: NOT YET MODELLED"],
    ]);
  });

  it("states its CONTOUR INTERVAL in the wireframe once its terrain draws", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    const spike = renderMoon(measured, { style: "wireframe" });
    for (let step = 0; step < 12; step += 1) {
      // Each step answers what the one before asked.
      // oxlint-disable-next-line no-await-in-loop
      await play(1);
      for (const worker of spike.heightWorkers) {
        worker.answerFields();
        answerEveryBake(worker);
      }
    }
    expect(readingIn(mainLabelBlock(), "CONTOUR INTERVAL")).toMatch(
      /^[\d.]+ k?m · INDEX [\d.]+ k?m$/u,
    );
  });
});

describe("DescentSpike's photorealistic style refused over a generated world", () => {
  it("ends the run once with GRAPHICS STYLE REFUSED on its fault line, and draws no more", async () => {
    // The renderer says why its pipelines could not be made.
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    fakeFramesAndTimeouts();
    stubLayout();
    const failed: string[] = [];
    const frames: number[] = [];
    renderMoon(measured, {
      style: "photorealistic",
      engine: readyEngine({ refuseFrames: false, failMaterials: true }).source,
      listeners: {
        onFailed: (status) => failed.push(status),
        onFrame: (sample) => frames.push(sample.scriptTimeS),
      },
    });
    await play(4);
    const drawnBefore = frames.length;
    await play(2);
    const refused = "GRAPHICS STYLE REFUSED: photorealistic style not created, relaunch to retry";
    expect([cautionLines(), failed, frames.length]).toEqual([[refused], [refused], drawnBefore]);
  });
});

describe("DescentSpike's terrain fault over a generated world (decision-r10-t14c-guide Q2)", () => {
  it("ends the run once, in the spike's words, when every height worker is lost", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    const failed: string[] = [];
    const spike = renderMoon(measured, {
      listeners: { onFailed: (status) => failed.push(status) },
    });
    await play(3);
    const workers = spike.heightWorkers.length;
    for (let round = 0; round < MAX_CONSECUTIVE_WORKER_FAILURES; round += 1) {
      for (const worker of spike.heightWorkers.slice(-workers)) {
        worker.fail();
      }
    }
    await play(3);
    const fault = "TERRAIN NOT AVAILABLE: height computation failed, relaunch to retry";
    expect([
      cautionLines(),
      failed,
      screen.queryByText(/reopen VIEW/u),
      screen.queryByText("TERRAIN: STREAMING"),
    ]).toEqual([[fault], [fault], null, null]);
  });

  it("ends the run once, in the spike's words, when its terrain computation keeps failing", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    fakeFramesAndTimeouts();
    stubLayout();
    const failed: string[] = [];
    renderMoon(
      (session) => {
        vi.spyOn(session, "step").mockImplementation(() => {
          throw new Error("a bug of the session's step");
        });
        return measured();
      },
      { listeners: { onFailed: (status) => failed.push(status) } },
    );
    await play(4);
    const fault = "TERRAIN NOT AVAILABLE: terrain computation failed, relaunch to retry";
    expect([
      cautionLines(),
      failed,
      screen.queryByText(/reopen VIEW/u),
      screen.queryByText("TERRAIN: STREAMING"),
    ]).toEqual([[fault], [fault], null, null]);
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

  it("writes the site's elevation in ELEVATION's form: 0.1 m, no plus, grouped from five", () => {
    expect([
      siteElevationReading(412.34),
      siteElevationReading(-410),
      siteElevationReading(12_480),
      siteElevationReading(-0.04),
    ]).toEqual(["412.3 m", "-410.0 m", "12,480.0 m", "0.0 m"]);
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
