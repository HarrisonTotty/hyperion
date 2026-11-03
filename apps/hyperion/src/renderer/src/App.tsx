import { galacticPositionFromLy, type RequestClient } from "@hyperion/protocol";
import { Activity, type ReactElement, useCallback, useEffect, useMemo, useState } from "react";

import { ConnectionPanel } from "./components/ConnectionPanel";
import { ConsoleFrame } from "./components/ConsoleFrame";
import { GraphicsPanel } from "./components/GraphicsPanel";
import { UniverseProvider } from "./components/UniverseProvider";
import { GalaxyDisplay } from "./displays/galaxy/GalaxyDisplay";
import { SystemDisplay } from "./displays/system/SystemDisplay";
import {
  type InspectionOpening,
  inspectionOption,
  ViewSceneProvider,
} from "./displays/view/ViewSceneProvider";
import { ViewDisplay } from "./displays/view/ViewDisplay";
import { type CheckedRun, ViewsCheckRunner } from "./displays/view/check/ViewsCheckRunner";
import { placedAtmosphere } from "./displays/view/check/atmospherePlaces";
import { handoverApproachFor } from "./displays/view/check/handoverGeometry";
import {
  handoverRangeAtM,
  HandoverScenes,
  type HandoverWorld,
  terrainTestHandoverWorld,
} from "./displays/view/check/handoverScenes";
import { readServedHandoverWorld } from "./displays/view/check/servedHandover";
import { terrainProbeOptions } from "./displays/view/check/terrainCheckRun";
import { TerrainProbe } from "./displays/view/check/terrainProbe";
import { ViewsProbe } from "./displays/view/check/viewsProbe";
import { wholeLookSceneOptions } from "./displays/view/check/wholeLooks";
import type { ViewEngineSource } from "./displays/view/useViewEngine";
import type { SceneOption } from "./displays/view/viewRun";
import type { BodyInspection } from "./view/scenes/inspectionScene";
import { surveyInspected } from "./view/scenes/inspectionSurvey";
import type { TerrainSession } from "./view/terrain/session";
import type { WorkerLike } from "./view/terrain/workers/pool";
import type { SystemOpening, SystemTarget } from "./displays/system/systemTarget";
import { type ConnectionState, useServerConnection } from "./lib/connection";
import type { SystemPlace } from "./lib/scene/model";
import { DISPLAYS, type DisplayId } from "./lib/displays";
import { ServerLinkContext, useServerLinkValue } from "./lib/serverLink";
import { useDisplayKeys } from "./lib/useDisplayKeys";
import { GraphicsStatusProvider } from "./view/engine/GraphicsStatusProvider";
import { navigatorGpu } from "./view/engine/status";
import type { QualitySetting } from "./view/quality/qualitySetting";
import { selectionViewport } from "./view/terrain/handover";
import type {
  GeometryOverride,
  GraphicsLaunchMode,
  ServedWorldNames,
  ViewsCheckLaunch,
} from "../../preload/api";
import { useSurfaceModuleCheck } from "./wasm/useSurfaceModuleCheck";

/** What `App` hands the displays besides the server link and the universe, which are contexts. */
interface DisplayInputs {
  readonly connection: ConnectionState;
  readonly serverUrl: string;
  readonly systemOpening: SystemOpening | null;
  readonly openSystem: (target: SystemTarget) => void;
  /** Opens a body in `VIEW`'s body inspection, from `SYSTEM`'s `VIEW BODY` (the owner's request). */
  readonly viewBody: (inspection: BodyInspection) => void;
  /** `VIEW`'s engine on a `--views-check` launch (R07.T20), and its terrain run's seam, else `null`. */
  readonly checkedView: CheckedView | null;
  /** The quality setting `VIEW` draws at, the launch's `--setting` (R07.T17). */
  readonly setting: QualitySetting;
  /** The platform the client runs on, whose primary modifier `VIEW`'s rate chord takes (R07.T19.f). */
  readonly platform: string;
  /**
   * The machine's performance cores' threads with a third of its efficiency cores', which `VIEW`'s
   * height workers' count reads.
   */
  readonly performanceThreads: number | null;
  /** The launch's geometry override (`--geometry`, plan R14), which `VIEW`'s terrain draws with. */
  readonly geometry: GeometryOverride | undefined;
}

/**
 * The terrain run's seam a `--terrain` check launch hands `VIEW` (R10.T16.d): its counted height
 * workers and its watch of the terrain session, together, beside its kept scenes with
 * `TERRAIN TEST`'s paced fixture (`CheckedView.sceneOptions`).
 */
interface TerrainSeam {
  readonly heightWorkers: () => WorkerLike;
  readonly watchTerrain: (session: TerrainSession) => () => void;
}

/**
 * What a `--views-check` launch hands `VIEW`: its probe's engine source (R07.T20), the terrain
 * run's seam on a `--terrain` launch, `null` on T20's (the batch review's D-C), and the kept scenes
 * it offers: the terrain probe's on a `--terrain` launch, and `VIEW`'s own with the places' on a
 * software `--atmosphere` one (`atmospherePlaces.ts`), `undefined` for `VIEW`'s own alone.
 */
interface CheckedView {
  readonly engineSource: ViewEngineSource;
  readonly terrain: TerrainSeam | null;
  readonly sceneOptions: ReadonlyArray<SceneOption> | undefined;
}

/**
 * The probes of a `--views-check` launch: the views probe, and its run's own (`CheckedRun`): the
 * terrain run's probe on a `--terrain` one, and on a `--handover` one also the hand-over check's
 * looks (R10.T10.g), offered among the terrain probe's kept scenes on that launch alone; none of its
 * own on an `--atmosphere` one (R08.T10.b); on an `--inspection` one, the link's request client,
 * through which its run asks what it opens (the owner's request).
 */
interface CheckProbes {
  readonly views: ViewsProbe;
  readonly run: CheckedRun;
}

/**
 * The served world a `--handover` launch names (`--universe`, `--body`; plan R10, R10.T15.b), or
 * `null` for `TERRAIN TEST`'s.
 */
function servedHandoverOf(launch: ViewsCheckLaunch): ServedWorldNames | null {
  return launch.run === "handover" ? (launch.served ?? null) : null;
}

/**
 * A `--views-check` launch's probes, by its options and graphics mode (an `--atmosphere` run's
 * places in the software mode alone); a `--handover` launch's looks on `served`, the served world
 * read from the server, where it names one, else on `TERRAIN TEST`'s.
 */
function checkProbesOf(
  launch: ViewsCheckLaunch,
  launchMode: GraphicsLaunchMode,
  requests: RequestClient,
  served: HandoverWorld | null = null,
): CheckProbes {
  // An inspection profile's `--composite-probe` makes VIEW's composite under it (the owner's
  // priority, 2026-11-01).
  const views = new ViewsProbe(undefined, undefined, launch.compositeProbe ?? null);
  // A `--weathered` launch's probe serves `TEST MOON` weathered's field in TERRAIN TEST's place.
  const terrainProbe = (checkScenes: ReadonlyArray<SceneOption>): TerrainProbe =>
    new TerrainProbe(views, {
      ...terrainProbeOptions(launch.smoke),
      checkScenes,
      fixture: launch.fixture,
    });
  let run: CheckedRun;
  switch (launch.run) {
    case "several-views":
      run = { kind: "several-views" };
      break;
    case "atmosphere":
      run = {
        kind: "atmosphere",
        placed: launchMode === "software" ? placedAtmosphere(launch.smoke) : null,
      };
      break;
    case "inspection":
      run = { kind: "inspection", requests, served: launch.served ?? null };
      break;
    case "terrain":
      // A `--whole` launch's looks at the whole body (plan R10, R10.T14.c), offered after the kept
      // scenes; none on any other.
      run = {
        kind: "terrain",
        terrain: terrainProbe(
          launch.fixture === "terrain-test-whole" ? wholeLookSceneOptions() : [],
        ),
      };
      break;
    case "handover": {
      const world = served ?? terrainTestHandoverWorld(launch.fixture);
      // A look chosen by hand before the run measures the primary is drawn at the range of a
      // canvas the window's width, at its selection's CSS px, and measures nothing
      // (`HandoverScenes`).
      const windowPx = Math.max(1, Math.round(window.innerWidth * window.devicePixelRatio));
      const selection = selectionViewport(
        { widthPx: windowPx, heightPx: windowPx },
        window.devicePixelRatio,
      );
      const scenes = new HandoverScenes(
        world,
        handoverApproachFor(launch.look, launch.smoke),
        handoverRangeAtM(world, launch.setting, selection.widthPx).rangeM,
      );
      run = { kind: "handover", terrain: terrainProbe(scenes.sceneOptions), scenes };
      break;
    }
  }
  return { views, run };
}

/** The view a check launch hands `VIEW`, from its probes. */
function checkedViewOf(probes: CheckProbes): CheckedView {
  const { run } = probes;
  // T20's run, the atmosphere run (R08.T10.b) and the inspection run draw through the views probe
  // alone.
  if (run.kind === "several-views" || run.kind === "inspection") {
    return { engineSource: probes.views.source, terrain: null, sceneOptions: undefined };
  }
  if (run.kind === "atmosphere") {
    return {
      engineSource: probes.views.source,
      terrain: null,
      sceneOptions: run.placed?.sceneOptions,
    };
  }
  const { terrain } = run;
  return {
    engineSource: terrain.source,
    terrain: { heightWorkers: terrain.heightWorkers, watchTerrain: terrain.watchTerrain },
    sceneOptions: terrain.sceneOptions,
  };
}

function displayContent(id: DisplayId, inputs: DisplayInputs): ReactElement {
  let content: ReactElement;
  switch (id) {
    case "link":
      content = (
        <>
          <ConnectionPanel
            url={inputs.serverUrl}
            clientVersion={__APP_VERSION__}
            connection={inputs.connection}
          />
          <GraphicsPanel setting={inputs.setting} />
        </>
      );
      break;
    case "galaxy":
      content = <GalaxyDisplay onOpenSystem={inputs.openSystem} />;
      break;
    case "system":
      content = <SystemDisplay opening={inputs.systemOpening} onViewBody={inputs.viewBody} />;
      break;
    case "view":
      content = (
        <ViewDisplay
          engineSource={inputs.checkedView?.engineSource}
          heightWorkers={inputs.checkedView?.terrain?.heightWorkers}
          sceneOptions={inputs.checkedView?.sceneOptions}
          watchTerrain={inputs.checkedView?.terrain?.watchTerrain}
          setting={inputs.setting}
          platform={inputs.platform}
          performanceThreads={inputs.performanceThreads}
          geometry={inputs.geometry}
        />
      );
      break;
  }
  return content;
}

/**
 * Root of the bridge client. Owns the server connection and the choice of display, and lays out
 * the consoles.
 *
 * @remarks
 * The server it links to is the one the main process resolved from the command line, handed over by
 * the preload, as are the launch's graphics mode and the GPU process's crashes, which feed the one
 * graphics status store beside the adapter's answer (R01.T4).
 *
 * Every display stays mounted under React's `Activity`: a hidden one keeps its state (a chart's
 * centre, its camera, its selection) while its effects are torn down and it leaves the
 * accessibility tree, and switching back is instant (plan 05, design note D1). Displays reach the
 * server through `ServerLinkContext` and the open universe through `UniverseContext`; both values
 * are memoised, so that a latency update re-renders the `LINK` display and the header but no
 * display that only makes requests.
 *
 * Once the server has said its generator version, the client's surface module is loaded in its
 * worker and checked against it (`useSurfaceModuleCheck`, R04.T10.c).
 *
 * `OPEN SYSTEM` on the `GALAXY` display opens the `SYSTEM` display on its selected system at the
 * chart's time (plan 14, P14.T41.a): `App` keeps the latest opening, counted so that each is a new
 * display state, and switches to the display. The callback is stable, so the memoised `GALAXY`
 * display is not rendered again for it.
 *
 * `VIEW BODY` on the `SYSTEM` display's orbit map opens its body in `VIEW`'s body inspection (the
 * owner's request, 2026-11-01): `App` makes its kept scene as it is pressed and keeps the latest
 * opening, counted as a system's opening is, hands it to `ViewSceneProvider`, which shows it, and
 * switches to `VIEW`; the callback is stable.
 *
 * `VIEW` and the `LINK` display's `Graphics` panel take the launch's quality setting (`--setting`,
 * R07.T17), which holds for the run.
 *
 * The header strip's `TRAINING` banner stands while `VIEW` is shown drawing a kept test scene, and
 * not over the server's scene (R02.T17; the guide's training banner): `ViewSceneProvider` holds the
 * display's scene and its `SCENE` choice above the frame, so the banner is computed during render
 * from what `VIEW` draws. The system last opened on `SYSTEM` is handed to it with its designation
 * and position, which the server's scene does not carry. A `--views-check` launch, T20's or the
 * terrain run's, is a measurement run, flown by script and wanting no input: its banner reads
 * `MEASUREMENT` in place of `TRAINING` on every display for the whole launch (plan R10, R10.T16.e;
 * decision-r10-t16-leans §11).
 */
export function App() {
  const serverUrl = window.hyperion.serverUrl;
  const connection = useServerConnection(serverUrl, __APP_VERSION__);
  useSurfaceModuleCheck(connection.serverGeneratorVersion);
  const link = useServerLinkValue(connection);
  const viewsCheck = window.hyperion.viewsCheck;
  // A `--views-check` launch opens on `VIEW`, whose engine its probe watches (R07.T20); a
  // `--terrain` one also its terrain run's (R10.T16.d). A hand-over check of a served world
  // (R10.T15.b) makes them once the world is read from the server.
  const [probes, setProbes] = useState((): CheckProbes | null =>
    viewsCheck === undefined || servedHandoverOf(viewsCheck.launch) !== null
      ? null
      : checkProbesOf(viewsCheck.launch, window.hyperion.graphics.launchMode, connection.requests),
  );
  const awaitingServed =
    probes === null && viewsCheck !== undefined && servedHandoverOf(viewsCheck.launch) !== null;
  const connected = connection.status === "connected";
  const requests = connection.requests;
  useEffect(() => {
    // `awaitingServed` is true only on a views check's launch, by which TypeScript narrows `viewsCheck`.
    if (!awaitingServed || !connected) {
      return undefined;
    }
    const target = servedHandoverOf(viewsCheck.launch);
    if (target === null) {
      return undefined;
    }
    const abort = new AbortController();
    /** Ends the run failed with `reason`, which the main process writes. */
    const fail = (reason: string): void => {
      viewsCheck.end({ status: "fail", reason }).catch((error: unknown) => {
        console.error("views check: the run could not be ended:", error);
      });
    };
    readServedHandoverWorld(requests, target.universe, target.body, abort.signal)
      .then((read) => {
        if (abort.signal.aborted) {
          return undefined;
        }
        switch (read.kind) {
          case "read":
            setProbes(
              checkProbesOf(
                viewsCheck.launch,
                window.hyperion.graphics.launchMode,
                requests,
                read.world,
              ),
            );
            break;
          case "not served":
            fail(
              `the server serves no surface for ${target.body}: its field is not modelled (${read.section})`,
            );
            break;
          case "failed":
            fail(`the hand-over check's served world could not be read: ${read.message}`);
            break;
        }
        return undefined;
      })
      .catch((error: unknown) => {
        // A bug in reading the world: the run ends failed, the error in the log.
        console.error("views check: the hand-over check's served world could not be read:", error);
        if (!abort.signal.aborted) {
          fail(error instanceof Error ? error.message : String(error));
        }
      });
    return () => {
      abort.abort();
    };
  }, [awaitingServed, connected, requests, viewsCheck]);
  const [activeDisplay, setActiveDisplay] = useState<DisplayId>(
    viewsCheck === undefined ? "link" : "view",
  );
  const [systemOpening, setSystemOpening] = useState<SystemOpening | null>(null);
  useDisplayKeys(DISPLAYS, setActiveDisplay);
  const openSystem = useCallback((target: SystemTarget): void => {
    setSystemOpening((previous) => ({ target, sequence: (previous?.sequence ?? 0) + 1 }));
    setActiveDisplay("system");
  }, []);
  const [inspection, setInspection] = useState<InspectionOpening | null>(null);
  const viewBody = useCallback(
    (body: BodyInspection): void => {
      // Made here, as the operator opens it, not in a render.
      const option = inspectionOption(body);
      // The body surveyed whole as it opens, so that its ground is drawn at once.
      const survey = surveyInspected(requests, body).then((outcome) => {
        if (outcome.kind === "refused") {
          console.warn(`the inspection's survey of ${outcome.body} was refused: ${outcome.reason}`);
        }
        return outcome;
      });
      setInspection((previous) => ({ option, sequence: (previous?.sequence ?? 0) + 1, survey }));
      setActiveDisplay("view");
    },
    [requests],
  );
  const target = systemOpening?.target ?? null;
  const knownSystem = useMemo(
    (): SystemPlace | null =>
      target === null
        ? null
        : {
            kind: "charted",
            system: target.system,
            designation: target.designation,
            barycentre: galacticPositionFromLy([
              target.positionLy.x,
              target.positionLy.y,
              target.positionLy.z,
            ]),
          },
    [target],
  );
  const inputs: DisplayInputs = {
    connection,
    serverUrl,
    systemOpening,
    openSystem,
    viewBody,
    checkedView: probes === null ? null : checkedViewOf(probes),
    setting: window.hyperion.setting,
    platform: window.hyperion.platform,
    performanceThreads: window.hyperion.performanceThreads,
    geometry: window.hyperion.geometry,
  };
  return (
    <GraphicsStatusProvider graphics={window.hyperion.graphics} gpu={navigatorGpu()}>
      {viewsCheck === undefined || probes === null ? null : (
        <ViewsCheckRunner
          api={viewsCheck}
          probe={probes.views}
          run={probes.run}
          launchMode={window.hyperion.graphics.launchMode}
          openSystem={openSystem}
        />
      )}
      <ServerLinkContext value={link}>
        <UniverseProvider>
          <ViewSceneProvider
            active={activeDisplay === "view"}
            knownSystem={knownSystem}
            inspection={inspection}
          >
            {(sceneProvenance) => (
              <ConsoleFrame
                displays={DISPLAYS}
                activeDisplay={activeDisplay}
                onSelectDisplay={setActiveDisplay}
                linkStatus={connection.status}
                // A views check measures for the whole launch, on every display; training is a
                // kept test scene on VIEW; the server's scene is live operation.
                modeBanner={
                  viewsCheck !== undefined
                    ? "MEASUREMENT"
                    : activeDisplay === "view" && sceneProvenance === "kept"
                      ? "TRAINING"
                      : null
                }
              >
                {DISPLAYS.map(({ id }) => (
                  <Activity key={id} mode={id === activeDisplay ? "visible" : "hidden"}>
                    {displayContent(id, inputs)}
                  </Activity>
                ))}
              </ConsoleFrame>
            )}
          </ViewSceneProvider>
        </UniverseProvider>
      </ServerLinkContext>
    </GraphicsStatusProvider>
  );
}
