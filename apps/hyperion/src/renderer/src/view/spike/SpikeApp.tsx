import { useEffect, useState } from "react";

import type { GraphicsApi, SpikeApi } from "../../../../preload/api";
import { GraphicsStatusProvider } from "../engine/GraphicsStatusProvider";
import { navigatorGpu } from "../engine/status";
import type { TerrainVariant } from "../quality/qualitySetting";
import { GpuCapture } from "./capture";
import { DescentSpike } from "./DescentSpike";
import { SpikeController, variantOf } from "./spikeController";
import { SPIKE_PASS_ROWS, spikeGpu } from "./spikeHarness";
import { defaultSpikeWorkers, type SpikeListeners } from "./spikeRun";

/** Props of {@link SpikeApp}. */
export interface SpikeAppProps {
  /** `window.hyperion.spike`: the launch's options and the main process's functions. */
  readonly spike: SpikeApi;
  /** `window.hyperion.graphics`. */
  readonly graphics: GraphicsApi;
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
    canvas: () => ({
      widthPx: Math.round(window.innerWidth * window.devicePixelRatio),
      heightPx: Math.round(window.innerHeight * window.devicePixelRatio),
    }),
    log: (message, error) => {
      console.error(`descent spike: ${message}`, error);
    },
  });
  const load = gpu.source.load;
  return {
    gpu: {
      ...gpu,
      source: {
        requestAdapter: gpu.source.requestAdapter,
        load: async (outcome, status) => {
          const engine = await load(outcome, status);
          controller.engine(engine);
          return engine;
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
    },
  };
}

/**
 * The client's root on a `--descent-spike` launch (plan R05, T13.c): T13.b's `DescentSpike` on the
 * measured engine, with T14.a's metrics fed from its listeners, the main process measuring
 * between the descent's start and end, and the run ended with its results file or, for `--smoke`,
 * its status.
 */
export function SpikeApp({ spike, graphics }: SpikeAppProps) {
  const [harness] = useState(() => makeHarness(spike));
  const { launch } = spike;
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
        listeners={harness.listeners}
      />
    </GraphicsStatusProvider>
  );
}
