import { useEffect, useState } from "react";

import type { GraphicsApi, SpikeApi } from "../../../../preload/api";
import { GraphicsStatusProvider } from "../engine/GraphicsStatusProvider";
import { navigatorGpu } from "../engine/status";
import type { TerrainVariant } from "../quality/qualitySetting";
import { GpuCapture } from "./capture";
import { DescentSpike, MAIN_VIEW_NAME } from "./DescentSpike";
import { SpikeController, variantOf } from "./spikeController";
import { SPIKE_PASS_ROWS, spikeGpu } from "./spikeHarness";
import { defaultSpikeWorkers, type SpikeListeners, type SpikeWorkers } from "./spikeRun";

/** Props of {@link SpikeApp}. */
export interface SpikeAppProps {
  /** `window.hyperion.spike`: the launch's options and the main process's functions. */
  readonly spike: SpikeApi;
  /** `window.hyperion.graphics`. */
  readonly graphics: GraphicsApi;
  /** The spike's workers: the browser's by default, fakes in a test. */
  readonly spikeWorkers?: SpikeWorkers | undefined;
}

/** The run's measured engine, its control and its listeners, made once. */
interface Harness {
  readonly gpu: ReturnType<typeof spikeGpu>;
  readonly capture: GpuCapture | null;
  readonly controller: SpikeController;
  readonly listeners: SpikeListeners;
  readonly variant: TerrainVariant;
}

function makeHarness(spike: SpikeApi): Harness {
  const { launch } = spike;
  const clock = { scriptTimeS: 0 };
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
      const main = document.querySelector<HTMLCanvasElement>(
        `canvas[aria-label="${MAIN_VIEW_NAME}"]`,
      );
      return { widthPx: main?.width ?? 0, heightPx: main?.height ?? 0 };
    },
    nowMs: () => performance.now(),
    log: (message, error) => {
      console.error(`descent spike: ${message}`, error);
    },
  });
  const load = gpu.source.load;
  return {
    gpu: {
      ...gpu,
      source: {
        requestAdapter: async () => {
          const outcome = await gpu.source.requestAdapter();
          if (outcome.kind !== "adapter") {
            controller.fail(`no GPU adapter (${outcome.kind})`);
          }
          return outcome;
        },
        load: async (outcome, status) => {
          try {
            const engine = await load(outcome, status);
            controller.engine(engine);
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
 */
export function SpikeApp({ spike, graphics, spikeWorkers }: SpikeAppProps) {
  const [harness] = useState(() => makeHarness(spike));
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
        engineSource={harness.gpu.source}
        spikeWorkers={spikeWorkers}
        listeners={harness.listeners}
      />
    </GraphicsStatusProvider>
  );
}
