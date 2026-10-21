import { useEffect, useState } from "react";

import type { GraphicsApi, SpikeApi } from "../../../../preload/api";
import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import { GraphicsStatusProvider } from "../engine/GraphicsStatusProvider";
import { navigatorGpu } from "../engine/status";
import type { TerrainVariant } from "../quality/qualitySetting";
import type { MeasureDescent } from "./bodyDescent";
import { GpuCapture } from "./capture";
import { DescentSpike, MAIN_CANVAS_SELECTOR } from "./DescentSpike";
import { SpikeController, variantOf } from "./spikeController";
import { SPIKE_PASS_ROWS, spikeGpu } from "./spikeHarness";
import {
  defaultSpikeWorkers,
  SPIKE_MAIN_VIEW_ID,
  SPIKE_VIEW_NAMES,
  type SpikeListeners,
  type SpikeWorkers,
} from "./spikeRun";
import { SpikeSizes } from "./spikeSizes";

/** Props of {@link SpikeApp}. */
export interface SpikeAppProps {
  /** `window.hyperion.spike`: the launch's options and the main process's functions. */
  readonly spike: SpikeApi;
  /** `window.hyperion.graphics`. */
  readonly graphics: GraphicsApi;
  /** The spike's workers: the browser's by default, fakes in a test. */
  readonly spikeWorkers?: SpikeWorkers | undefined;
  /**
   * Makes the engine on the adapter the spike's measured source found: R01's WebGPU engine on the
   * measured device by default (`spikeGpu`), a fake engine in a test. Read at the first render.
   */
  readonly loadEngine?: ViewEngineSource["load"] | undefined;
  /** How the terrain world's descent is measured (`DescentSpike`'s `measure`): a fake in a test. */
  readonly measure?: MeasureDescent | undefined;
}

/** The run's measured engine, its control and its listeners, made once. */
interface Harness {
  readonly gpu: ReturnType<typeof spikeGpu>;
  readonly capture: GpuCapture | null;
  readonly controller: SpikeController;
  readonly listeners: SpikeListeners;
  readonly variant: TerrainVariant;
  /** The measured sizes of `--sizes on` over the terrain world (R10.T14.b), or `null`. */
  readonly sizes: SpikeSizes | null;
}

function makeHarness(spike: SpikeApi, loadEngine: ViewEngineSource["load"] | undefined): Harness {
  const { launch } = spike;
  const clock = { scriptTimeS: 0 };
  // The command line refuses `--sizes on` over the test planet; the measurement is the terrain
  // session's.
  const sizes =
    launch.sizes === "on" && launch.world === "terrain-test"
      ? new SpikeSizes({
          setting: launch.setting,
          style: launch.style ?? "photorealistic",
          mainViewId: SPIKE_MAIN_VIEW_ID,
          secondViewName: SPIKE_VIEW_NAMES.craft,
        })
      : null;
  const capture =
    launch.capture === null
      ? null
      : new GpuCapture({
          contexts: GPUCanvasContext.prototype,
          meta: { setting: launch.setting, seed: launch.seed, passRows: SPIKE_PASS_ROWS },
        });
  const gpu = spikeGpu(navigatorGpu(), capture, () => clock.scriptTimeS);
  const controller = new SpikeController({
    spike,
    gpu,
    capture,
    // The main view's own canvas, whose drawing size the engine sets in device pixels.
    canvas: () => {
      const main = document.querySelector<HTMLCanvasElement>(MAIN_CANVAS_SELECTOR);
      return { widthPx: main?.width ?? 0, heightPx: main?.height ?? 0 };
    },
    nowMs: () => performance.now(),
    log: (message, error) => {
      console.error(`descent spike: ${message}`, error);
    },
    ...(sizes === null ? {} : { sizes }),
  });
  const load = loadEngine ?? gpu.source.load;
  return {
    sizes,
    gpu: {
      ...gpu,
      source: {
        requestAdapter: async () => {
          const outcome = await gpu.source.requestAdapter();
          sizes?.noteAdapter(outcome);
          if (outcome.kind !== "adapter") {
            controller.fail(`no GPU adapter (${outcome.kind})`);
          }
          return outcome;
        },
        load: async (outcome, status) => {
          try {
            const engine = await load(outcome, status);
            controller.engine(engine);
            sizes?.noteDevice(engine.capabilities);
            return engine;
          } catch (error: unknown) {
            controller.fail("the engine could not be made", error);
            throw error;
          }
        },
      },
    },
    capture,
    controller,
    variant: variantOf(launch),
    listeners: {
      onPrepared: (prepared) => {
        controller.prepared({
          planet: prepared.planet,
          profile: prepared.profile,
          omittedSigmaM: Array.from(prepared.omittedSigmaM),
        });
      },
      onFrame: (sample) => {
        clock.scriptTimeS = sample.scriptTimeS;
        controller.frame(sample);
      },
      onPatch: (event) => {
        controller.patch(event);
      },
      onSelect: (input) => {
        controller.select(input);
      },
      onFailed: (status) => {
        controller.fail(status);
      },
    },
  };
}

/**
 * The client's root on a `--descent-spike` launch (plan R05, T13.c): T13.b's `DescentSpike` on the
 * measured engine, with T14.a's metrics fed from its listeners, the main process measuring
 * between the descent's start and end, and the run ended with its results file or, for `--smoke`,
 * its status.
 *
 * @remarks
 * With `--sizes on` over the terrain world (plan R10, R10.T14.b) it makes the measured sizes
 * (`SpikeSizes`) once and hands them to `DescentSpike`, which starts the session's height workers
 * on their bake clock; the adapter's and the device's storage-binding limits are noted as the
 * engine is made, and the report's `sizes` block is taken at the run's end.
 */
export function SpikeApp({ spike, graphics, spikeWorkers, loadEngine, measure }: SpikeAppProps) {
  // Made once, with the engine's loader given at the first render: the run's is its launch's.
  const [harness] = useState(() => makeHarness(spike, loadEngine));
  const { launch } = spike;
  // A launch that can have no view asks for no adapter (`useViewEngine`), so nothing else would
  // end its run: the safe mode and a renderer without WebGPU end it as failed at once.
  useEffect(() => {
    if (graphics.launchMode === "safe") {
      harness.controller.fail("the graphics safe mode draws no WebGPU view");
    } else if (navigatorGpu() === undefined) {
      harness.controller.fail("this renderer has no WebGPU");
    }
  }, [harness, graphics]);
  // A run left by a closed window ends its capture's hold on the canvases.
  useEffect(() => () => harness.capture?.dispose(), [harness]);
  return (
    <GraphicsStatusProvider graphics={graphics} gpu={navigatorGpu()}>
      <DescentSpike
        seed={BigInt(launch.seed)}
        setting={launch.setting}
        ridges={launch.ridged}
        workers={launch.workers ?? defaultSpikeWorkers(navigator.hardwareConcurrency)}
        variant={harness.variant}
        world={launch.world}
        // The test planet's launch has no style: its main view is the spike's own lit view.
        style={launch.style ?? undefined}
        engineSource={harness.gpu.source}
        spikeWorkers={spikeWorkers}
        measure={measure}
        listeners={harness.listeners}
        sizes={harness.sizes ?? undefined}
      />
    </GraphicsStatusProvider>
  );
}
