import { useEffect, useState, useSyncExternalStore } from "react";

import type { GraphicsApi, PerfApi, PerfConfig } from "../../../../preload/api";
import { StatusLine, type StatusStanding } from "../../components/StatusLine";
import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import { GraphicsStatusProvider } from "../engine/GraphicsStatusProvider";
import { navigatorGpu } from "../engine/status";
import { ServedDescentSpike } from "../spike/ServedDescentSpike";
import { SpikeHeader } from "../spike/SpikeHeader";
import { defaultSpikeWorkers } from "../spike/spikeRun";
import {
  type DescentDriverPage,
  descentDriverFor,
  DescentStages,
  PerfEngine,
} from "./descentDriver";
import {
  PAGE_CLOCK,
  PERF_COMPLETE,
  PERF_FAILED_LOGGED,
  perfFailedLine,
  type PerfDriverChoice,
  type PerfRunOutcome,
  PerfSceneRun,
} from "./runPerfScene";

/** The status while no scene's views are mounted: before the first, and between repetitions. */
export const PERF_PREPARING = "PERFORMANCE RUN: preparing the scene's views";

/**
 * The run's line while no views are mounted: preparing until it ends, then how it ended (the
 * guide's "Data states": the current state, never the last), complete in plain text, failed as a
 * fault with its cause.
 */
function runLine(ended: PerfRunOutcome | null): {
  readonly text: string;
  readonly standing: StatusStanding;
} {
  if (ended === null) {
    return { text: PERF_PREPARING, standing: "waiting" };
  }
  let line: { readonly text: string; readonly standing: StatusStanding };
  switch (ended.kind) {
    case "complete":
      // Plain: the main process closes the client once the record is written.
      line = { text: PERF_COMPLETE, standing: "waiting" };
      break;
    case "failed":
      line = { text: perfFailedLine(ended.shown ?? PERF_FAILED_LOGGED), standing: "fault" };
      break;
  }
  return line;
}

/** Props of {@link PerfApp}. */
export interface PerfAppProps {
  /** `window.hyperion.perf`: the run's options and the main process's functions. */
  readonly perf: PerfApi;
  /** `window.hyperion.graphics`. */
  readonly graphics: GraphicsApi;
  /**
   * Makes the engine on the adapter the measured source found: R05's measured WebGPU engine by
   * default (`spikeGpu`), a fake engine in a test. Read at the first render.
   */
  readonly loadEngine?: ViewEngineSource["load"] | undefined;
  /**
   * Chooses the scene's driver on the page: the descents' (`descentDriverFor`) by default, a fake
   * in a test. Read at the first render.
   */
  readonly driverFor?:
    ((config: PerfConfig, page: DescentDriverPage) => PerfDriverChoice) | undefined;
}

/**
 * The client's root on a `--perf-run` launch (plan R12, R12.T4.c; Design note 14): the renderer's
 * half of the run (`runPerfScene.ts`), its scene's views mounted for the warm-up and anew for each
 * repetition, each under its own `key` on the server and arrival the main process gives.
 *
 * @remarks
 * The descents (`cockpit-descent`, `station-wireframe-descent`) are R05's spike over the served
 * world (`descentDriver.ts`); a scene the renderer has no driver for yet ends its run at once,
 * saying so. Between mounts the spike's frame stands with {@link PERF_PREPARING}; once the run has
 * ended, with `PERFORMANCE RUN COMPLETE`, or on a failure the spike's own fault line where it
 * showed one, else `PERFORMANCE RUN FAILED` with its cause (`perfFailedLine`).
 */
export function PerfApp({ perf, graphics, loadEngine, driverFor }: PerfAppProps) {
  // Made once: the engine is kept for the run, and the stages are the store the run's driver sets.
  const [page] = useState(() => ({
    engine: new PerfEngine(navigatorGpu(), loadEngine),
    stages: new DescentStages(),
    driverFor: driverFor ?? descentDriverFor,
  }));
  const stage = useSyncExternalStore(page.stages.subscribe, page.stages.current);
  const [ended, setEnded] = useState<PerfRunOutcome | null>(null);
  useEffect(() => {
    const abort = new AbortController();
    const run = new PerfSceneRun({
      api: perf,
      clock: PAGE_CLOCK,
      signal: abort.signal,
      onEnded: (outcome) => {
        if (!abort.signal.aborted) {
          setEnded(outcome);
        }
      },
      driverFor: (config) =>
        page.driverFor(config, {
          engine: page.engine,
          stages: page.stages,
          launchMode: graphics.launchMode,
          webGpu: navigatorGpu() !== undefined,
          defaultWorkers: () =>
            defaultSpikeWorkers(navigator.hardwareConcurrency, window.hyperion.performanceThreads),
        }),
    });
    run.run().catch((error: unknown) => {
      console.error("perf run: the renderer's half failed:", error);
    });
    return () => {
      abort.abort();
    };
  }, [page, perf, graphics]);
  // The engine the run kept goes with the page.
  useEffect(() => () => page.engine.dispose(), [page]);
  const line = runLine(ended);
  return (
    <GraphicsStatusProvider graphics={graphics} gpu={navigatorGpu()}>
      {stage === null ? (
        <div className="spike">
          <SpikeHeader />
          <main className="view spike__work">
            <div className="view__main">
              <StatusLine text={line.text} standing={line.standing} />
            </div>
          </main>
        </div>
      ) : (
        <ServedDescentSpike
          key={stage.key}
          serverUrl={stage.serverUrl}
          universe={stage.universe}
          body={stage.body}
          seed={stage.seed}
          setting={stage.setting}
          ridges="off"
          workers={stage.workers}
          style={stage.style}
          engineSource={page.engine.source}
          measure={stage.measure}
          listeners={stage.listeners}
          perf={stage.perf}
        />
      )}
    </GraphicsStatusProvider>
  );
}
