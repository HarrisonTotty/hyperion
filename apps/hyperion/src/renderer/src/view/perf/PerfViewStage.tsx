import { type ReactNode, use, useEffect, useMemo, useRef } from "react";

import { UniverseProvider } from "../../components/UniverseProvider";
import { ViewDisplay } from "../../displays/view/ViewDisplay";
import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import { ViewSceneContext, ViewSceneProvider } from "../../displays/view/ViewSceneProvider";
import { useServerConnection } from "../../lib/connection";
import { openLinkSocket } from "../../lib/linkSocket";
import { ServerLinkContext, useServerLinkValue } from "../../lib/serverLink";
import { UniverseContext, useUniverse } from "../../lib/universe";
import { SERVED_LINK_INCOMPATIBLE_REMEDY } from "../spike/ServedDescentSpike";
import { SPIKE_RETRY } from "../spike/spikeFaults";
import { SpikeHeader } from "../spike/SpikeHeader";
import type { PerfFailure } from "./runPerfScene";
import { VIEW_TITLE, type ViewStage } from "./viewDriver";

/** Props of {@link PerfViewStage}: a mount's stage, and what the page gives every mount. */
export interface PerfViewStageProps extends Omit<ViewStage, "key"> {
  /** The run's measured engine (`PerfEngine.source`), kept across the mounts. */
  readonly engineSource: ViewEngineSource;
  /** `window.hyperion.performanceThreads`, which `VIEW`'s height and optics workers' counts read. */
  readonly performanceThreads: number | null;
  /** `window.hyperion.platform`. */
  readonly platform: string;
}

/** Props of {@link SceneWatch}, and of {@link ArrivalUniverse} with its own. */
interface StageFailedProps {
  readonly onFailed: (failure: PerfFailure) => void;
}

/** Tells the run once of `failure`, the first that is not `null`. */
function useToldOnce(failure: PerfFailure | null, onFailed: (failure: PerfFailure) => void): void {
  const toldRef = useRef(false);
  useEffect(() => {
    if (failure !== null && !toldRef.current) {
      toldRef.current = true;
      onFailed(failure);
    }
  }, [failure, onFailed]);
}

/** Props of {@link ArrivalUniverse}. */
interface ArrivalUniverseProps extends StageFailedProps {
  readonly universe: string;
  readonly children: ReactNode;
}

/**
 * The arrival's universe as the one open, taken from the server's list of universes, which the
 * session asks once the link is up, without opening it, as the descent spike takes its universe
 * from the arrival: requests each name their universe, and an opening would warm the galaxy's sky
 * and tube tables on the scene's two server workers, a session's start, not the arrival Design note
 * 13 lands in each repetition. A list the server refuses, does not answer, or that lacks the
 * arrival's universe ends the run once.
 */
function ArrivalUniverse({ universe, onFailed, children }: ArrivalUniverseProps) {
  const session = useUniverse();
  const { list } = session;
  const listed =
    list.kind === "ok" ? (list.response.universes.find(({ id }) => id === universe) ?? null) : null;
  const value = useMemo(() => ({ ...session, open: listed }), [session, listed]);
  let failure: PerfFailure | null = null;
  if (list.kind === "rejected") {
    failure = {
      reason: `the server refused its universe list: ${list.reason}`,
      shown: `the server refused its universe list (${list.reason}), ${SPIKE_RETRY}`,
    };
  } else if (list.kind === "timed_out") {
    failure = {
      reason: "the server did not answer its universe list",
      shown: `the server did not answer its universe list, ${SPIKE_RETRY}`,
    };
  } else if (list.kind === "ok" && listed === null) {
    failure = {
      reason: `the server lists no universe ${universe}, the arrival's`,
      shown: `the server lists no arrival's universe, ${SPIKE_RETRY}`,
    };
  }
  useToldOnce(failure, onFailed);
  return <UniverseContext value={value}>{children}</UniverseContext>;
}

/**
 * How many times a perf run asks again for a scene its server left unanswered before the run ends:
 * the arrival's work can hold a loaded machine's two server workers past a request's 30 s (a lean,
 * 2026-11-18), and the span starts only at the scene's first frame.
 */
export const SCENE_RETRIES = 3;

/**
 * Asks again, as `VIEW`'s `RETRY` would, for a scene the server left unanswered, up to
 * {@link SCENE_RETRIES} times, and tells the run once if the server refuses the scene or leaves it
 * unanswered after them: a measurement waits on no operator's retry, as the descent spike's
 * `SCENE REJECTED` and `SCENE TIMED OUT` end its run.
 */
function SceneWatch({ onFailed }: StageFailedProps) {
  const scene = use(ViewSceneContext)?.scene ?? null;
  const status = scene?.status ?? null;
  const retry = scene?.retry;
  const retriedRef = useRef(0);
  const toldRef = useRef(false);
  useEffect(() => {
    if (status === null || retry === undefined || toldRef.current) {
      return;
    }
    if (status.kind === "timed_out" && retriedRef.current < SCENE_RETRIES) {
      retriedRef.current += 1;
      console.warn(
        `perf run: the scene's subscription went unanswered; asked again (${String(retriedRef.current)} of ${String(SCENE_RETRIES)})`,
      );
      retry();
      return;
    }
    let failure: PerfFailure | null = null;
    if (status.kind === "rejected") {
      failure = {
        reason: `the server refused the scene's subscription (${status.code}): ${status.reason}`,
        shown: `the server refused the scene (${status.reason}), ${SPIKE_RETRY}`,
      };
    } else if (status.kind === "timed_out") {
      failure = {
        reason: `the server did not answer the scene's subscription, asked ${String(SCENE_RETRIES + 1)} times`,
        shown: `the server did not answer the scene, ${SPIKE_RETRY}`,
      };
    }
    if (failure !== null) {
      toldRef.current = true;
      onFailed(failure);
    }
  }, [status, retry, onFailed]);
  return null;
}

/**
 * A perf run's mount of `VIEW` (plan R12, R12.T4.c): the display over the stage's server, its own
 * link made to it, the arrival's universe the one open and the server's scene drawn, its primary on
 * the run's free scripted camera (`ViewDisplayProps.perf`), under the `MEASUREMENT` banner.
 *
 * @remarks
 * Every mount is a new link, a new universe session and a new `VIEW`, whose terrain session,
 * atmosphere cache and sky go with it (Design note 13's resets); the engine is the page's. A server
 * of another protocol version, a universe or a scene the server refuses or leaves unanswered ends
 * the run, as they end the descent spike's. `VIEW` is laid out in the console's frame, its views
 * paced by counting the display's frames (`display`), so that a 30 Hz primary draws at every
 * second vsync, as the scene's pacing says.
 */
export function PerfViewStage({
  serverUrl,
  universe,
  setting,
  perf,
  watchTerrain,
  onFailed,
  engineSource,
  performanceThreads,
  platform,
}: PerfViewStageProps) {
  const connection = useServerConnection(serverUrl, __APP_VERSION__, openLinkSocket);
  const link = useServerLinkValue(connection);
  const incompatible = connection.status === "incompatible";
  useToldOnce(
    incompatible
      ? {
          reason: "the scene's server speaks another protocol version",
          shown: SERVED_LINK_INCOMPATIBLE_REMEDY,
        }
      : null,
    onFailed,
  );
  return (
    <ServerLinkContext value={link}>
      <UniverseProvider>
        <ArrivalUniverse universe={universe} onFailed={onFailed}>
          <ViewSceneProvider active knownSystem={null} inspection={null}>
            {() => (
              // The console's own frame, as `App` lays `VIEW` out: header, work area, no tabs.
              <div className="console">
                <SceneWatch onFailed={onFailed} />
                <SpikeHeader title={VIEW_TITLE} link={connection.status} />
                <main className="console__work console__work--view">
                  <ViewDisplay
                    engineSource={engineSource}
                    setting={setting}
                    framePacing="display"
                    platform={platform}
                    performanceThreads={performanceThreads}
                    watchTerrain={watchTerrain}
                    perf={perf}
                  />
                </main>
              </div>
            )}
          </ViewSceneProvider>
        </ArrivalUniverse>
      </UniverseProvider>
    </ServerLinkContext>
  );
}
