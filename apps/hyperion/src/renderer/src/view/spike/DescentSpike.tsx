import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";

import { StatusLine } from "../../components/StatusLine";
import {
  DEFAULT_ENGINE_SOURCE,
  useViewEngine,
  type ViewEngineSource,
} from "../../displays/view/useViewEngine";
import { ViewLabelBlock } from "../../displays/view/ViewLabelBlock";
import { ViewMarkList } from "../../displays/view/ViewMarkList";
import {
  type LabelLine,
  labelLines,
  labelStatements,
  markRows,
  PRESET_NAMES,
  type ViewRun,
} from "../../displays/view/viewRun";
import { formatNumber, formatSigned } from "../../lib/format";
import { type ElementSize, useElementSize } from "../../lib/useElementSize";
import { type ColourTokens, readTokens } from "../../spatial/paint";
import { type CameraTarget, newCameraState } from "../camera/state";
import type { ViewSize } from "../engine/types";
import type { QualitySetting, TerrainVariant } from "../quality/qualitySetting";
import { cameraSceneOf } from "../scene/model";
import type { PatchKey } from "../terrain/patchKey";
import type { Selection, SelectionInput } from "../terrain/select";
import type { TestPlanetRidges } from "../terrain/workers/messages";
import { spikeExposure } from "./litView";
import {
  DEFAULT_SPIKE_WORKERS,
  DescentRefused,
  type PreparedDescent,
  prepareDescent,
  type SpikeFrame,
  type SpikeFrameSample,
  type SpikeListeners,
  type SpikePatchEvent,
  SpikeRun,
  type SpikeWorkers,
} from "./spikeRun";
import { spikeScene } from "./spikeScene";
import { SurfaceQuery } from "./surfaceQuery";

/** The shortest time between two changes of the readouts: 4 Hz, as the `VIEW`'s. */
const READOUT_INTERVAL_MS = 250;

/** The label block's statement of what the terrain is (Design note 12). */
export const TEST_PLANET_STATEMENT = "TEST PLANET: provisional, dry and hand-parameterised";

/** The main view's accessible name: its class, its style and its camera. */
export const MAIN_VIEW_NAME = "VIEW, SPIKE LIT, SCRIPTED";

/** The status where the terrain could not be measured; the cause goes to the log. */
const NOT_MEASURED = "TERRAIN NOT MEASURED: surface query failed, relaunch to retry";

/** The status where the script cannot clear the measured terrain (decision-r05-spike-ux.md). */
export const DESCENT_REFUSED =
  "DESCENT REFUSED: terrain cannot be cleared on this seed, relaunch with another seed";

/** The status where the views or their materials could not be made, as `ViewDisplay` words it. */
export const VIEWS_NOT_MADE = "GRAPHICS NOT AVAILABLE: views could not be made, relaunch to retry";

/** The status where no adapter answered (the guide's nomenclature). */
const NO_ADAPTER = "GRAPHICS NO ADAPTER: views not available, relaunch to retry";

/** Props of {@link DescentSpike}. */
export interface DescentSpikeProps {
  /** The spike's seed, which draws the landing site (T13.a). */
  readonly seed: bigint;
  readonly setting: QualitySetting;
  readonly ridges: TestPlanetRidges;
  /** How many height workers the pool runs (Design note 11; `--workers`). */
  readonly workers: number;
  /** The terrain variant (`--vertex-path`, `--normals`; T13.c); the setting's own if absent. */
  readonly variant?: TerrainVariant | undefined;
  /** Where the engine comes from: R01's, or T13.c's measured one, or a fake in a test. */
  readonly engineSource?: ViewEngineSource | undefined;
  readonly spikeWorkers?: SpikeWorkers | undefined;
  /** How the descent is measured before it flies: `prepareDescent`, or a test's. */
  readonly prepare?: typeof prepareDescent | undefined;
  /**
   * T13.c's metrics: each frame's sample and each patch's progress. Read at each call, so a new
   * object does not restart the run.
   */
  readonly listeners?: SpikeListeners | undefined;
}

type Preparation =
  | { readonly kind: "measuring" }
  | { readonly kind: "ready"; readonly prepared: PreparedDescent }
  | { readonly kind: "failed" }
  /** The terrain was measured, but the script cannot clear it (`DescentRefused`). */
  | { readonly kind: "refused" };

/** The canvas's drawing size, device pixels, from its stage's laid-out size. */
function deviceSize(size: ElementSize | null): ViewSize | null {
  if (size === null || size.widthPx <= 0 || size.heightPx <= 0) {
    return null;
  }
  return {
    widthPx: Math.max(1, Math.round(size.widthPx * size.devicePixelRatio)),
    heightPx: Math.max(1, Math.round(size.heightPx * size.devicePixelRatio)),
  };
}

/** A length in whole metres, in one unit at every altitude, so that it never switches unit. */
function metres(m: number): string {
  return `${formatNumber(m, 0)} m`;
}

/** A speed, m/s, to the centimetre a second that the hover's 0.05 m/s needs. */
function speed(mps: number, signed: boolean): string {
  return `${signed ? formatSigned(mps, 2) : formatNumber(mps, 2)} m/s`;
}

/** The spike's seed as every seed is written: 16 upper-case hexadecimal digits. */
function seedReading(seed: bigint): string {
  return seed.toString(16).toUpperCase().padStart(16, "0");
}

/** The main view's label lines: R02's, its style the spike's lit one and its camera the script. */
function mainLabelLines(run: ViewRun): ReadonlyArray<LabelLine> {
  return labelLines(run, spikeExposure()).map((line) => {
    if (line.label === "STYLE") {
      return { label: line.label, value: "SPIKE LIT" };
    }
    if (line.label === "CAMERA") {
      return { label: line.label, value: "SCRIPTED" };
    }
    return line;
  });
}

interface InstrumentProps {
  readonly title: string;
  readonly canvasRef: (canvas: HTMLCanvasElement | null) => void;
  readonly stageRef: (stage: HTMLElement | null) => void;
  readonly run: ViewRun | null;
  /** The ID of the list its marks are selected from. */
  readonly listId: string;
}

/** A wireframe instrument: its canvas and its label block beside it. */
function Instrument({ title, canvasRef, stageRef, run, listId }: InstrumentProps) {
  const id = useId();
  const preset = run === null ? "" : `, ${PRESET_NAMES[run.camera.preset]}`;
  return (
    <section className="panel spike-instrument" aria-labelledby={`${id}-title`}>
      <h2 className="panel__title" id={`${id}-title`}>
        {title}
      </h2>
      <div className="spike-instrument__body">
        <div className="view__stage spike-instrument__stage" ref={stageRef}>
          <canvas
            ref={canvasRef}
            className="view__canvas"
            // A picture the run draws, not a control; a WebGPU canvas cannot be an <img>, and its
            // marks are the list it points to (aria-details).
            // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
            role="img"
            tabIndex={0}
            aria-label={`VIEW, WIREFRAME, ${title.toUpperCase()}${preset}`}
            aria-details={listId}
          />
        </div>
        <div className="spike-instrument__label">
          <ViewLabelBlock
            lines={run === null ? [] : labelLines(run, spikeExposure())}
            statements={run === null ? [] : labelStatements(run)}
            countLine={null}
            fault={null}
          />
        </div>
      </div>
    </section>
  );
}

/**
 * The descent spike (plan R05, T13.b): the test planet's terrain under Earth's atmosphere in the
 * main view, flown down T13.a's scripted descent, with two small wireframe instruments of R02's
 * style, the orbit (the planet's graticule, the craft and its path) and the craft (its hull, from
 * astern), and one console panel of the descent's readings, all on one engine through R01's
 * per-view contexts, under the `TRAINING` banner, since it draws a kept test scene.
 *
 * @remarks
 * Before it draws, it measures the landing site's height and the terrain under the low pass's track
 * through the surface query, once, so that the script stays a function of the seed. Each canvas is
 * focusable, named, and points to the list of the scene's bodies and craft (`aria-details`), from
 * which a mark is selected; the instruments draw its bracket reticle. The label block states the
 * test planet as provisional, dry and hand-parameterised, and carries the terrain's annunciation
 * (T9). Readings change at 4 Hz, and show `—` while no run draws.
 */
export function DescentSpike({
  seed,
  setting,
  ridges,
  workers,
  variant,
  engineSource = DEFAULT_ENGINE_SOURCE,
  spikeWorkers = DEFAULT_SPIKE_WORKERS,
  prepare = prepareDescent,
  listeners,
}: DescentSpikeProps) {
  const id = useId();
  // The variant by its fields, so that a new object of the same variant does not restart the run.
  const vertexPath = variant?.vertexPath;
  const normals = variant?.normals;
  const listId = `${id}-targets`;
  const engineState = useViewEngine(engineSource);
  const [preparation, setPreparation] = useState<Preparation>({ kind: "measuring" });
  const [published, setPublished] = useState<SpikeFrame | null>(null);
  const [drawFault, setDrawFault] = useState<string | null>(null);
  const [selection, setSelection] = useState<CameraTarget | null>(null);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [mainCanvas, setMainCanvas] = useState<HTMLCanvasElement | null>(null);
  const [orbitCanvas, setOrbitCanvas] = useState<HTMLCanvasElement | null>(null);
  const [craftCanvas, setCraftCanvas] = useState<HTMLCanvasElement | null>(null);
  const { ref: mainStageRef, size: mainSize } = useElementSize();
  const { ref: orbitStageRef, size: orbitSize } = useElementSize();
  const { ref: craftStageRef, size: craftSize } = useElementSize();
  const inputsRef = useRef<{
    readonly main: ElementSize | null;
    readonly orbit: ElementSize | null;
    readonly craft: ElementSize | null;
    readonly tokens: ColourTokens | null;
    readonly selection: CameraTarget | null;
    readonly listeners: SpikeListeners | undefined;
  }>({ main: null, orbit: null, craft: null, tokens: null, selection: null, listeners });

  useLayoutEffect(() => {
    inputsRef.current = {
      main: mainSize,
      orbit: orbitSize,
      craft: craftSize,
      tokens: mainCanvas === null || !mainCanvas.isConnected ? null : readTokens(mainCanvas),
      selection,
      listeners,
    };
  }, [mainSize, orbitSize, craftSize, mainCanvas, selection, listeners]);

  // The run's listeners read the latest props through the ref, so that the run is made once.
  const [forwarding] = useState<SpikeListeners>(() => ({
    onFrame: (sample: SpikeFrameSample) => {
      inputsRef.current.listeners?.onFrame?.(sample);
    },
    onPatch: (event: SpikePatchEvent, key: PatchKey) => {
      inputsRef.current.listeners?.onPatch?.(event, key);
    },
    onSelect: (input: SelectionInput, selected: Selection) => {
      inputsRef.current.listeners?.onSelect?.(input, selected);
    },
  }));

  // The terrain is measured once, in the surface query's worker, before the run starts.
  useEffect(() => {
    const query = new SurfaceQuery(spikeWorkers.query(), ridges);
    const life = { ended: false };
    void prepare(query, seed)
      .finally(() => {
        query.dispose();
      })
      .then((prepared) => {
        if (!life.ended) {
          inputsRef.current.listeners?.onPrepared?.(prepared);
          setPreparation({ kind: "ready", prepared });
        }
        return undefined;
      })
      .catch((error: unknown) => {
        if (!life.ended) {
          if (error instanceof DescentRefused) {
            console.error("the descent spike refuses to fly:", error);
            inputsRef.current.listeners?.onFailed?.(DESCENT_REFUSED);
            setPreparation({ kind: "refused" });
          } else {
            console.error("the descent spike's terrain could not be measured:", error);
            inputsRef.current.listeners?.onFailed?.(NOT_MEASURED);
            setPreparation({ kind: "failed" });
          }
        }
      });
    return () => {
      life.ended = true;
      query.dispose();
      setPreparation({ kind: "measuring" });
    };
  }, [spikeWorkers, ridges, seed, prepare]);

  const prepared = preparation.kind === "ready" ? preparation.prepared : null;

  // The run draws every frame once the engine, the measurement and the three canvases are in.
  useEffect(() => {
    if (
      engineState.kind !== "ready" ||
      prepared === null ||
      mainCanvas === null ||
      orbitCanvas === null ||
      craftCanvas === null
    ) {
      return undefined;
    }
    let run: SpikeRun;
    try {
      run = new SpikeRun(
        engineState.engine,
        prepared,
        {
          setting,
          ridges,
          createPool: spikeWorkers.pool(workers),
          variant: {
            ...(vertexPath === undefined ? {} : { vertexPath }),
            ...(normals === undefined ? {} : { normals }),
          },
        },
        { main: mainCanvas, orbit: orbitCanvas, craft: craftCanvas },
        forwarding,
      );
    } catch (error: unknown) {
      console.error("the descent spike's views could not be made:", error);
      inputsRef.current.listeners?.onFailed?.(VIEWS_NOT_MADE);
      // The engine's refusal is the external system's answer, known only once the canvases exist.
      // oxlint-disable-next-line react/set-state-in-effect
      setDrawFault(VIEWS_NOT_MADE);
      return () => {
        setDrawFault(null);
      };
    }
    const life = { ended: false, failed: false };
    // A failure stops the run: the loop draws no more, and the readings go to their missing value.
    const fail = (what: string, error: unknown): void => {
      console.error(`the descent spike's ${what} failed:`, error);
      life.failed = true;
      inputsRef.current.listeners?.onFailed?.(VIEWS_NOT_MADE);
      if (!life.ended) {
        setDrawFault(VIEWS_NOT_MADE);
        setPublished(null);
      }
    };
    void run.ready().catch((error: unknown) => {
      fail("materials", error);
    });
    let frame = 0;
    let publishedMs = Number.NEGATIVE_INFINITY;
    const tick = (nowMs: number): void => {
      if (life.failed) {
        return;
      }
      const inputs = inputsRef.current;
      const sizes = {
        main: deviceSize(inputs.main),
        orbit: deviceSize(inputs.orbit),
        craft: deviceSize(inputs.craft),
      };
      if (
        inputs.tokens !== null &&
        inputs.main !== null &&
        sizes.main !== null &&
        sizes.orbit !== null &&
        sizes.craft !== null
      ) {
        let drawn: SpikeFrame;
        try {
          drawn = run.frame({
            nowMs,
            sizes: { main: sizes.main, orbit: sizes.orbit, craft: sizes.craft },
            tokens: inputs.tokens,
            remPx: inputs.main.remPx * inputs.main.devicePixelRatio,
            selection: inputs.selection,
          });
        } catch (error: unknown) {
          // A frame the engine refused: the run stops and says so, rather than freezing silently.
          fail("frame", error);
          return;
        }
        if (nowMs - publishedMs >= READOUT_INTERVAL_MS) {
          publishedMs = nowMs;
          setPublished(drawn);
        }
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      life.ended = true;
      cancelAnimationFrame(frame);
      run.dispose();
      // Nothing draws now: the readings show their missing value, never the last as live.
      setPublished(null);
      setDrawFault(null);
    };
  }, [
    engineState,
    prepared,
    mainCanvas,
    orbitCanvas,
    craftCanvas,
    setting,
    ridges,
    workers,
    vertexPath,
    normals,
    spikeWorkers,
    forwarding,
  ]);

  // The runs the label blocks and the list read: the published frame's, or the script's start.
  const kept = useMemo(() => (prepared === null ? null : spikeScene(prepared.profile)), [prepared]);
  const runs = useMemo(() => {
    if (kept === null) {
      return null;
    }
    const tS = published?.tS ?? 0;
    const scene = published?.scene ?? kept.sceneAt(tS);
    const of = (key: "main" | "orbit" | "craft"): ViewRun => ({
      source: { kind: "kept", kept },
      tS,
      scene,
      camera: published?.cameras[key] ?? newCameraState(cameraSceneOf(scene), "camera"),
    });
    return { main: of("main"), orbit: of("orbit"), craft: of("craft") };
  }, [kept, published]);

  const status: { readonly text: string; readonly standing: "waiting" | "fault" } | null =
    preparation.kind === "failed"
      ? { text: NOT_MEASURED, standing: "fault" }
      : preparation.kind === "refused"
        ? { text: DESCENT_REFUSED, standing: "fault" }
        : preparation.kind === "measuring"
          ? { text: "TERRAIN MEASURING: the landing site and the low track", standing: "waiting" }
          : engineState.kind === "unavailable"
            ? { text: NO_ADAPTER, standing: "fault" }
            : engineState.kind === "pending"
              ? { text: "GRAPHICS ACQUIRING ADAPTER", standing: "waiting" }
              : null;

  const pose = published?.pose ?? null;
  const terrain = published?.terrain ?? null;
  return (
    <div className="spike">
      <header className="console__header">
        <div className="console__identity">
          <span className="console__ship">HYPERION</span>
          <h1 className="console__title">DESCENT SPIKE</h1>
        </div>
        <div className="console__status">
          <output className="console__banner" aria-label="Mode">
            TRAINING
          </output>
        </div>
      </header>
      <main className="view spike__work">
        <div className="view__main">
          <div className="view__stage" ref={mainStageRef}>
            <canvas
              ref={setMainCanvas}
              className="view__canvas"
              // A picture the run draws, not a control; a WebGPU canvas cannot be an <img>, and its
              // marks are the list it points to (aria-details).
              // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
              role="img"
              tabIndex={0}
              aria-label={MAIN_VIEW_NAME}
              aria-details={listId}
            />
            <div className="view__overlay">
              <ViewLabelBlock
                lines={runs === null ? [] : mainLabelLines(runs.main)}
                statements={[
                  TEST_PLANET_STATEMENT,
                  ...(runs === null
                    ? []
                    : labelStatements(runs.main, terrain?.annunciation ?? null)),
                ]}
                countLine={null}
                fault={drawFault}
              />
            </div>
          </div>
          <div className="spike__instruments">
            <Instrument
              title="Orbit"
              canvasRef={setOrbitCanvas}
              stageRef={orbitStageRef}
              run={runs?.orbit ?? null}
              listId={listId}
            />
            <Instrument
              title="Craft"
              canvasRef={setCraftCanvas}
              stageRef={craftStageRef}
              run={runs?.craft ?? null}
              listId={listId}
            />
          </div>
          {status === null ? null : <StatusLine text={status.text} standing={status.standing} />}
        </div>
        <div className="view__side">
          <section className="panel spike__panel" aria-labelledby={`${id}-descent`}>
            <h2 className="panel__title" id={`${id}-descent`}>
              Descent
            </h2>
            <dl className="readout">
              <dt>Spike Seed</dt>
              <dd>{seedReading(seed)}</dd>
              <dt>Setting</dt>
              <dd>{setting.toUpperCase()}</dd>
              <dt>Segment</dt>
              <Reading value={pose?.segment.toUpperCase() ?? null} />
              <dt>Script Time</dt>
              <Reading value={pose === null ? null : `${formatNumber(pose.tS, 1)} s`} />
              <dt>Clearance</dt>
              <Reading value={pose === null ? null : metres(pose.clearanceM)} />
              <dt>Ground Speed</dt>
              <Reading value={pose === null ? null : speed(pose.horizontalSpeedMps, false)} />
              <dt>Vertical Speed</dt>
              <Reading value={pose === null ? null : speed(pose.verticalSpeedMps, true)} />
              <dt>Site Height</dt>
              <Reading value={prepared === null ? null : metres(prepared.siteHeightM)} />
              <dt>Ground Contact</dt>
              <Reading value={published === null ? null : published.contact ? "YES" : "NO"} />
              <dt>Patches</dt>
              <Reading
                value={
                  terrain === null
                    ? null
                    : `${terrain.selected} SELECTED · ${terrain.drawn} DRAWN · ` +
                      `${terrain.standingIn} STANDING IN · ${terrain.missing} MISSING`
                }
              />
            </dl>
          </section>
          <section className="panel spike__targets" aria-labelledby={`${id}-targets-title`}>
            <h2 className="panel__title" id={`${id}-targets-title`}>
              Targets
            </h2>
            <div id={listId} className="spike__list">
              <ViewMarkList
                rows={runs === null ? [] : markRows(runs.main)}
                selectedKey={selectedKey}
                onSelect={(row) => {
                  setSelectedKey(row.key);
                  setSelection(row.target);
                }}
              />
            </div>
          </section>
        </div>
      </main>
    </div>
  );
}

/** Props of {@link Reading}. */
interface ReadingProps {
  /** The reading's text, or `null` while no run draws. */
  readonly value: string | null;
}

/** A reading, or the missing value's `—` while no run draws. */
function Reading({ value }: ReadingProps) {
  return (
    <dd>
      {value === null ? (
        <span className="readout__missing">—</span>
      ) : (
        <output aria-live="off">{value}</output>
      )}
    </dd>
  );
}
