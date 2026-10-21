import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";

import { StatusLine } from "../../components/StatusLine";
import type { GroundSnapshot } from "../../displays/view/groundReadout";
import { litLabelsOf } from "../../displays/view/photorealFrame";
import { terrainLowerParts } from "../../displays/view/terrainPlate";
import { useTerrainLines } from "../../displays/view/useTerrainLines";
import { useTerrainFaults, useTerrainSession } from "../../displays/view/useTerrainSession";
import {
  DEFAULT_ENGINE_SOURCE,
  useViewEngine,
  type ViewEngineSource,
} from "../../displays/view/useViewEngine";
import { terrainStatementOf } from "../../displays/view/viewFaults";
import { ViewLabelBlock } from "../../displays/view/ViewLabelBlock";
import { ViewLowerPlate } from "../../displays/view/ViewLowerPlate";
import { ViewMarkList } from "../../displays/view/ViewMarkList";
import {
  type LabelLine,
  labelLines,
  labelStatements,
  markRows,
  photorealStatements,
  rangesFromCamera,
  PRESET_NAMES,
  type SurveyEdgeListNote,
  surveyEdgeNotes,
  type ViewRun,
} from "../../displays/view/viewRun";
import { formatNumber, formatSigned, formatSignedDeg } from "../../lib/format";
import { type ElementSize, useElementSize } from "../../lib/useElementSize";
import { type ColourTokens, readTokens } from "../../spatial/paint";
import { type CameraTarget, newCameraState, type RenderStyle } from "../camera/state";
import type { ViewSize } from "../engine/types";
import { hostLights, lightingState, sceneHostDiscs } from "../lighting/hostLights";
import { styleName } from "../photoreal/style";
import type { QualitySetting, TerrainVariant } from "../quality/qualitySetting";
import { cameraSceneOf, type ViewScene } from "../scene/model";
import { TERRAIN_TEST_SCRIPT, terrainSceneFixture } from "../scenes/terrainScene";
import type { PatchKey } from "../terrain/patchKey";
import {
  formatElevationReading,
  NO_BREAK_SPACE,
  NO_GROUND,
  NOT_SURVEYED,
} from "../terrain/readouts";
import type { Selection, SelectionInput } from "../terrain/select";
import type { TestPlanetRidges } from "../terrain/workers/messages";
import { type DrawnSurveyEdge, viewStrokesAt } from "../wireframe/drawList";
import { type MeasureDescent, measureWorldDescent } from "./bodyDescent";
import { spikeExposure } from "./litView";
import {
  DEFAULT_SPIKE_WORKERS,
  DescentRefused,
  type PreparedDescent,
  prepareDescent,
  type SiteReading,
  type SpikeFrame,
  type SpikeFrameSample,
  type SpikeListeners,
  type SpikePatchEvent,
  SpikeRun,
  SPIKE_MAIN_VIEW_ID,
  type SpikeWorkers,
} from "./spikeRun";
import { SPIKE_RETRY, spikeFaultLine } from "./spikeFaults";
import { cameraElevationDeg, spikeScene } from "./spikeScene";
import { surveySnapshotOf } from "./spikeSessionView";
import { SIZES_READOUT_INTERVAL_MS, type SpikeSizes } from "./spikeSizes";
import { type SpikeWorldName, spikeWorldNamed } from "./spikeWorld";
import { SurfaceQuery } from "./surfaceQuery";

/** The shortest time between two changes of the readouts: 4 Hz, as the `VIEW`'s. */
const READOUT_INTERVAL_MS = 250;

/** The label block's statement of what the terrain is (Design note 12). */
export const TEST_PLANET_STATEMENT = "TEST PLANET: provisional, dry and hand-parameterised";

/** The main view's accessible name over the test planet: its class, its style and its camera. */
export const MAIN_VIEW_NAME = "VIEW, SPIKE LIT, SCRIPTED";

/**
 * The main view's accessible name over a generated world (R10.T14.c): its class, the style it was
 * drawn in, as its `STYLE` line reads it, and its camera.
 */
export function bodyMainViewName(drawn: RenderStyle): string {
  return `VIEW, ${styleName(drawn)}, SCRIPTED`;
}

/**
 * The main view's canvas, on either world, for the harness, which reads its drawing size: its name
 * changes with the style drawn over a generated world.
 */
export const MAIN_CANVAS_SELECTOR = 'canvas[data-spike-view="main"]';

/** The status where the terrain could not be measured; the cause goes to the log. */
const NOT_MEASURED = `TERRAIN NOT MEASURED: surface query failed, ${SPIKE_RETRY}`;

/** The status while the terrain is measured. */
const MEASURING = "MEASURING TERRAIN: landing site and ground track";

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
  /** The world flown (`--world`, R10.T14.c): the test planet unless given. */
  readonly world?: SpikeWorldName | undefined;
  /**
   * The style the main view draws in over a generated world (`--style`, R10.T14.c):
   * photorealistic unless given; the test planet's main view is the spike's own lit view.
   */
  readonly style?: RenderStyle | undefined;
  /** Where the engine comes from: R01's, or T13.c's measured one, or a fake in a test. */
  readonly engineSource?: ViewEngineSource | undefined;
  readonly spikeWorkers?: SpikeWorkers | undefined;
  /** How the descent is measured before it flies over the test planet: `prepareDescent`, or a test's. */
  readonly prepare?: typeof prepareDescent | undefined;
  /**
   * How the descent is measured over a generated world, through the terrain session:
   * `measureWorldDescent`, or a test's.
   */
  readonly measure?: MeasureDescent | undefined;
  /**
   * T13.c's metrics: each frame's sample and each patch's progress. Read at each call, so a new
   * object does not restart the run.
   */
  readonly listeners?: SpikeListeners | undefined;
  /**
   * The measured sizes over the terrain world (`--sizes on`, plan R10, R10.T14.b): the craft
   * instrument drawn as a second session view, the passes counted, the session's height workers
   * (`spikeWorkers.height`) wrapped by its bake clock, and the main view's terrain readout asked at
   * 4 Hz, its answers not shown. Absent, the spike is T14.c's.
   */
  readonly sizes?: SpikeSizes | undefined;
}

type Preparation =
  | { readonly kind: "measuring" }
  | { readonly kind: "ready"; readonly prepared: PreparedDescent }
  | { readonly kind: "failed" }
  /** The terrain was measured, but the script cannot clear it (`DescentRefused`). */
  | { readonly kind: "refused" };

/** What the run's loop publishes for the DOM, at most every {@link READOUT_INTERVAL_MS}. */
interface Published {
  /** The frame it drew. */
  readonly frame: SpikeFrame;
  /**
   * What the main view's survey line is made from over a generated world, its `FRAME` body's survey
   * at the publish with no readout (`surveySnapshotOf`); `null` over the test planet, which has no
   * survey.
   */
  readonly ground: GroundSnapshot | null;
}

/** No survey edge drawn. */
const NO_SURVEY_EDGES: ReadonlyArray<DrawnSurveyEdge> = [];

/** The survey-edge notes the main view's list last showed, and the edges they were made from. */
interface ShownNotes {
  readonly edges: ReadonlyArray<DrawnSurveyEdge>;
  readonly notes: ReadonlyArray<SurveyEdgeListNote>;
}

/** No notes shown. */
const NO_NOTES: ShownNotes = { edges: NO_SURVEY_EDGES, notes: [] };

/**
 * The lighting a scene's photorealistic statements read (`LIGHTING`), as `VIEW`'s are built: its
 * kept scene's own star discs, with no sky asked, since the spike draws none.
 */
function spikeLightingOf(scene: ViewScene): ReturnType<typeof lightingState> {
  return lightingState(hostLights(scene, sceneHostDiscs(scene, null)), false);
}

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

/** The unit `HEIGHT ABOVE SITE` is shown in. */
export type HeightUnit = "m" | "km";

/**
 * The unit for a height above the site, metres, given the one it was last shown in: `km` above
 * 10.5 km, `m` below 9.5 km, and the last unit between, so that it never flickers at the switch
 * (decision-r05-spike-ux.md, 3a).
 */
export function heightUnitFor(heightM: number, last: HeightUnit): HeightUnit {
  if (heightM > 10_500) {
    return "km";
  }
  if (heightM < 9_500) {
    return "m";
  }
  return last;
}

/**
 * A height above the site in `unit`: below 10 m to a tenth of a metre (the hover's 2 m to 1 m),
 * then whole metres, or kilometres to one decimal.
 */
export function heightReading(heightM: number, unit: HeightUnit): string {
  if (unit === "km") {
    return `${formatNumber(heightM / 1000, 1)} km`;
  }
  return Math.abs(heightM) < 10 ? `${formatNumber(heightM, 1)} m` : `${formatNumber(heightM, 0)} m`;
}

/**
 * The landing site's `SITE ELEVATION` above the body's spheroid, in `ELEVATION`'s resolved form
 * (signoff-9 (k) part 2; plan R10, R10.T12): to 0.1 m, `-` below the datum and no `+` above it,
 * digits grouped from five (`formatElevationReading`), `412.3 m`, `-410.0 m`.
 *
 * @param elevationM - Above the spheroid, m.
 */
export function siteElevationReading(elevationM: number): string {
  return formatElevationReading(elevationM, null).value;
}

/**
 * The landing site's `SITE ELEVATION` over a generated world (plan R10, R10.T14.c; the guide's row,
 * G1): the survey's reading at the site in `ELEVATION`'s form (`formatElevationReading`), Estimated
 * with its uncertainty until the survey resolves the site (`~-410 m ± 18 m`), resolved once it does
 * (`-410.2 m`), the uncertainty after the value by a no-break space, as `VIEW`'s readout joins it;
 * or, where the readout met no surveyed ground there, its own state as `GROUND AT LINE OF SIGHT`
 * words it, `NOT SURVEYED` or `NO GROUND`.
 */
export function siteSurveyReading(reading: SiteReading): string {
  let text: string;
  switch (reading.kind) {
    case "ground": {
      const shown = formatElevationReading(
        reading.elevationM,
        reading.resolved ? null : reading.sigmaM,
      );
      text =
        shown.uncertainty === null
          ? shown.value
          : `${shown.value}${NO_BREAK_SPACE}${shown.uncertainty}`;
      break;
    }
    case "not-surveyed":
      text = NOT_SURVEYED;
      break;
    case "no-ground":
      text = NO_GROUND;
      break;
  }
  return text;
}

/** The ground speed: whole metres a second from 100 m/s, two decimals below. */
export function groundSpeedReading(mps: number): string {
  return `${formatNumber(mps, mps >= 100 ? 0 : 2)} m/s`;
}

/**
 * The patch counts, `SELECTED · DRAWN · STANDING IN · MISSING`, each count bound to its word and
 * each separator to the count before it by no-break spaces, so that a line breaks only after a
 * separator.
 */
export function patchesReading(t: {
  readonly selected: number;
  readonly drawn: number;
  readonly standingIn: number;
  readonly missing: number;
}): string {
  const nb = "\u00a0";
  return [
    `${t.selected}${nb}SELECTED`,
    `${t.drawn}${nb}DRAWN`,
    `${t.standingIn}${nb}STANDING${nb}IN`,
    `${t.missing}${nb}MISSING`,
  ].join(`${nb}· `);
}

/** The vertical speed, signed, to the centimetre a second that the hover's 0.05 m/s needs. */
function verticalSpeedReading(mps: number): string {
  return `${formatSigned(mps, 2)} m/s`;
}

/** The spike's seed as every seed is written: 16 upper-case hexadecimal digits. */
function seedReading(seed: bigint): string {
  return seed.toString(16).toUpperCase().padStart(16, "0");
}

/**
 * The main view's label lines: R02's, its camera the script, and its style the spike's lit one
 * over the test planet (`drawn` `null`) or the style drawn over a generated world (G2), with the
 * wireframe terrain's `CONTOUR INTERVAL` while it draws one.
 *
 * @param contourIntervalM - The interval its last frame drew its wireframe terrain at, m, or `null`.
 */
function mainLabelLines(
  run: ViewRun,
  drawn: RenderStyle | null,
  contourIntervalM: number | null,
): ReadonlyArray<LabelLine> {
  return labelLines(run, spikeExposure(), false, contourIntervalM).map((line) => {
    if (line.label === "STYLE") {
      return { label: line.label, value: drawn === null ? "SPIKE LIT" : styleName(drawn) };
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
  /** Its `CAMERA` value where the preset's name would be untrue: `SCRIPTED` for the orbit. */
  readonly cameraLabel: string | null;
}

/**
 * The lines an instrument keeps: what says what the picture is and its scale (the guide's Views);
 * the exposure, the stars, the scene and the statements are the main view's, so that the block
 * fits beside the canvas at 1280 × 720.
 */
const INSTRUMENT_LINES: ReadonlySet<string> = new Set(["FRAME", "TIME", "STYLE", "CAMERA", "FOV"]);

/**
 * An instrument: its canvas and its label block beside it, named in its camera's style, R02's
 * wireframe but for the second view of `--sizes on` (R10.T14.b), which draws in its own.
 */
function Instrument({ title, canvasRef, stageRef, run, listId, cameraLabel }: InstrumentProps) {
  const id = useId();
  const camera = cameraLabel ?? (run === null ? null : PRESET_NAMES[run.camera.preset]);
  const preset = camera === null ? "" : `, ${camera}`;
  const style = styleName(run?.camera.style ?? "wireframe");
  const lines =
    run === null
      ? []
      : labelLines(run, spikeExposure())
          .filter((line) => INSTRUMENT_LINES.has(line.label))
          .map((line) =>
            line.label === "CAMERA" && cameraLabel !== null
              ? { label: line.label, value: cameraLabel }
              : line,
          );
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
            aria-label={`VIEW, ${style}, ${title.toUpperCase()}${preset}`}
            aria-details={listId}
          />
        </div>
        <div className="spike-instrument__label">
          <ViewLabelBlock
            lines={lines}
            // The scene's statements are the main view's, which states them for every view.
            statements={[]}
            countLine={null}
            faults={[]}
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
 * per-view contexts, under the `MEASUREMENT` banner: it is flown by script and records, and no
 * operator input acts on it.
 *
 * @remarks
 * Before it draws, it measures the landing site's height and the terrain under the low pass's track
 * through the surface query, once, so that the script stays a function of the seed. Each canvas is
 * focusable, named, and points to the list of the scene's bodies and craft (`aria-details`), from
 * which a mark is selected; the instruments draw its bracket reticle. The label block states the
 * test planet as provisional, dry and hand-parameterised, and carries the terrain's annunciation
 * (T9). Readings change at 4 Hz, and show `—` while no run draws.
 *
 * Over a generated world (`world` `terrain-test`; plan R10, R10.T14.c) it flies `TERRAIN TEST`'s
 * star and moon instead: the fixture serves the moon's field in the page through `VIEW`'s terrain
 * session (`useTerrainSession`), whose lifetime is `VIEW`'s, and the descent is measured through
 * the session once it is made (`measureWorldDescent`), its field's revision raised before the
 * session's first step. The main view is `VIEW`'s frame drawer in `style`, its `STYLE` line and name
 * the style drawn, its label block without the test planet's statement, with the wireframe's
 * `CONTOUR INTERVAL`, the photorealistic style's statements, and the session's terrain fault on its
 * fault lines as `VIEW`'s. Its lower plate states the moon's survey line alone, and its list notes
 * the survey's edge, as `VIEW`'s primary's do; it states no `DATUM` or terrain readout and draws no
 * tripod (decision-r10-t14c-guide Q1), so that it asks the height workers nothing beside the
 * measurement's bakes. `SITE ELEVATION` is the survey's reading at the site, Estimated with its
 * uncertainty, or the readout's `NOT SURVEYED` or `NO GROUND` there, and missing until it is
 * answered.
 *
 * Its remedies read `relaunch to retry` where `VIEW`'s read `reopen VIEW to retry` (Q2): it has no
 * `VIEW` to reopen. A terrain fault in flight ends the run as failed, as does a photorealistic style
 * whose pipelines could not be made (`GRAPHICS STYLE REFUSED`): each stands on the main view's
 * fault lines, and the loop draws no more.
 *
 * With `sizes` over the terrain world (`--sizes on`, plan R10, R10.T14.b) the craft instrument's
 * canvas is the run's second session view, named and labelled in the style it drew in, the
 * session's height workers are started on the measurement's bake clock, and at `VIEW`'s 4 Hz
 * (`SIZES_READOUT_INTERVAL_MS`) the loop asks `VIEW`'s terrain readout of the main view
 * (`GroundReadouts`), whose answers are counted and never shown: nothing else on the page changes.
 */
export function DescentSpike({
  seed,
  setting,
  ridges,
  workers,
  variant,
  world = "test-planet",
  style = "photorealistic",
  engineSource = DEFAULT_ENGINE_SOURCE,
  spikeWorkers = DEFAULT_SPIKE_WORKERS,
  prepare = prepareDescent,
  measure = measureWorldDescent,
  listeners,
  sizes: measured,
}: DescentSpikeProps) {
  const id = useId();
  // The variant by its fields, so that a new object of the same variant does not restart the run.
  const vertexPath = variant?.vertexPath;
  const normals = variant?.normals;
  const listId = `${id}-targets`;
  const engineState = useViewEngine(engineSource);
  const spikeWorld = spikeWorldNamed(world);
  // `TERRAIN TEST`'s fixture, made once: its payloads are decoded only when a link first answers.
  const [fixture] = useState(() => terrainSceneFixture());
  // The generated world's terrain session, made and disposed as `VIEW`'s is, keyed on the fixture;
  // none on the test planet.
  const session = useTerrainSession({
    engineState,
    source: world === "terrain-test" ? { kind: "fixture", fixture } : null,
    // Under the measured sizes, each height worker on the bake clock: the same wrapper for the
    // same factory, so that the session is made once.
    createWorker: measured?.heightWorkers(spikeWorkers.height) ?? spikeWorkers.height,
    workers,
    // The session compiles its passes' materials at this setting as it is made (R10.T14.d).
    setting,
  });
  const terrainFaults = useTerrainFaults(session);
  const [preparation, setPreparation] = useState<Preparation>({ kind: "measuring" });
  const [published, setPublished] = useState<Published | null>(null);
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

  // The test planet's terrain is measured once, in the surface query's worker, before the run
  // starts.
  useEffect(() => {
    if (world !== "test-planet") {
      return undefined;
    }
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
  }, [world, spikeWorkers, ridges, seed, prepare]);

  // A generated world's terrain is measured once, through the terrain session, once it is made:
  // the fixture's revision is raised first, so that the session's first ask, at its first step, is
  // answered with the larger survey. The poll stops when the spike goes or the session changes.
  useEffect(() => {
    if (world !== "terrain-test" || session === null) {
      return undefined;
    }
    const life = { ended: false };
    const abort = new AbortController();
    fixture.notePlayed(TERRAIN_TEST_SCRIPT.deltaAtS);
    void measure(session, spikeWorld, seed, abort.signal)
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
      abort.abort(new Error("the descent spike's measurement was stopped"));
      setPreparation({ kind: "measuring" });
    };
  }, [world, session, fixture, spikeWorld, seed, measure]);

  const prepared = preparation.kind === "ready" ? preparation.prepared : null;
  // The script's kept scene, which the runs below and each publish's survey read.
  const kept = useMemo(
    () => (prepared === null ? null : spikeScene(prepared.profile, spikeWorld)),
    [prepared, spikeWorld],
  );

  // The run draws every frame once the engine, the measurement and the three canvases are in, and
  // over a generated world its terrain session.
  useEffect(() => {
    if (
      engineState.kind !== "ready" ||
      prepared === null ||
      kept === null ||
      mainCanvas === null ||
      orbitCanvas === null ||
      craftCanvas === null ||
      (world === "terrain-test" && session === null)
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
          ...(world === "terrain-test" && session !== null
            ? {
                body: {
                  world: spikeWorld,
                  session,
                  style,
                  ...(measured === undefined ? {} : { sizes: { measure: measured, session } }),
                },
              }
            : {}),
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
    // A failure stops the run, once: the loop draws no more, the listeners are told `status`, the
    // readings go to their missing value, and `shown`, where given, stands on the main view's fault
    // lines (the terrain's fault stands there through the session's own faults).
    const stop = (status: string, shown: string | null): void => {
      if (life.failed) {
        return;
      }
      life.failed = true;
      inputsRef.current.listeners?.onFailed?.(status);
      if (!life.ended) {
        if (shown !== null) {
          setDrawFault(shown);
        }
        setPublished(null);
      }
    };
    const fail = (what: string, error: unknown): void => {
      console.error(`the descent spike's ${what} failed:`, error);
      stop(VIEWS_NOT_MADE, VIEWS_NOT_MADE);
    };
    void run.ready().catch((error: unknown) => {
      fail("materials", error);
    });
    // Each publish's run of the main view, for its survey line's snapshot.
    const mainRunOf = (drawn: SpikeFrame): ViewRun => ({
      source: { kind: "kept", kept },
      tS: drawn.tS,
      scene: drawn.scene,
      camera: drawn.cameras.main,
    });
    let frame = 0;
    let publishedMs = Number.NEGATIVE_INFINITY;
    let readoutMs = Number.NEGATIVE_INFINITY;
    const tick = (nowMs: number): void => {
      if (life.failed) {
        return;
      }
      // A terrain fault ends the run as failed, in the spike's words: after it nothing of the
      // terrain is drawn or baked, so that a passed run measured terrain throughout
      // (decision-r10-t14c-guide Q2). The session logged its cause.
      const terrainFault = session?.terrainFaults.get(SPIKE_MAIN_VIEW_ID);
      if (terrainFault !== undefined) {
        stop(spikeFaultLine(terrainFault), null);
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
            strokes: viewStrokesAt(inputs.main.devicePixelRatio),
            selection: inputs.selection,
            mainStage: inputs.main,
            ...(measured === undefined || inputs.craft === null
              ? {}
              : { craftStage: inputs.craft }),
          });
        } catch (error: unknown) {
          // A frame the engine refused: the run stops and says so, rather than freezing silently.
          fail("frame", error);
          return;
        }
        if (drawn.refusal !== null) {
          // The style refused, the run's state rather than a throw: it stops, as the style's fault.
          // The photorealistic renderer logged why its pipelines could not be made.
          stop(drawn.refusal, drawn.refusal);
          return;
        }
        if (
          measured !== undefined &&
          world === "terrain-test" &&
          nowMs - readoutMs >= SIZES_READOUT_INTERVAL_MS
        ) {
          readoutMs = nowMs;
          // `VIEW`'s readout at its 4 Hz, so that the workers' figures include its queries; its
          // answers are counted and never shown (decision-r10-t14b-measure §6(f)).
          measured.publishReadout(session, mainRunOf(drawn), nowMs);
        }
        if (nowMs - publishedMs >= READOUT_INTERVAL_MS) {
          publishedMs = nowMs;
          setPublished({
            frame: drawn,
            ground: world === "terrain-test" ? surveySnapshotOf(session, mainRunOf(drawn)) : null,
          });
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
    kept,
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
    world,
    session,
    spikeWorld,
    style,
    measured,
  ]);

  // The runs the label blocks and the list read: the published frame's, or the script's start.
  const shownFrame = published?.frame ?? null;
  const runs = useMemo(() => {
    if (kept === null) {
      return null;
    }
    const tS = shownFrame?.tS ?? 0;
    const scene = shownFrame?.scene ?? kept.sceneAt(tS);
    const of = (key: "main" | "orbit" | "craft"): ViewRun => ({
      source: { kind: "kept", kept },
      tS,
      scene,
      camera: shownFrame?.cameras[key] ?? newCameraState(cameraSceneOf(scene), "camera"),
    });
    return { main: of("main"), orbit: of("orbit"), craft: of("craft") };
  }, [kept, shownFrame]);

  // A generated world's measurement waits on the engine, through the session, which is made once
  // the engine is ready and starts the measurement in the same commit's effects: while the adapter
  // is still acquired the line says so, and with no adapter its fault stands in place of the wait.
  const measuring =
    preparation.kind === "measuring" && (world === "test-planet" || engineState.kind === "ready");
  const status: { readonly text: string; readonly standing: "waiting" | "fault" } | null =
    preparation.kind === "failed"
      ? { text: NOT_MEASURED, standing: "fault" }
      : preparation.kind === "refused"
        ? { text: DESCENT_REFUSED, standing: "fault" }
        : measuring
          ? { text: MEASURING, standing: "waiting" }
          : engineState.kind === "unavailable"
            ? { text: NO_ADAPTER, standing: "fault" }
            : engineState.kind === "pending"
              ? { text: "GRAPHICS ACQUIRING ADAPTER", standing: "waiting" }
              : null;

  const pose = shownFrame?.pose ?? null;
  // The height's unit switches with hysteresis from the one last shown, adjusted during render.
  const [heightUnit, setHeightUnit] = useState<HeightUnit>("m");
  const nextUnit = pose === null ? heightUnit : heightUnitFor(pose.clearanceM, heightUnit);
  if (nextUnit !== heightUnit) {
    setHeightUnit(nextUnit);
  }
  const terrain = shownFrame?.terrain ?? null;
  // The main view's style as its label reads it: the spike's lit view's over the test planet, the
  // style drawn over a generated world, the style asked before its first frame, as `VIEW`'s.
  const drawnStyle = world === "test-planet" ? null : (shownFrame?.drawnStyle ?? style);
  // Its fault lines, as `VIEW`'s (`faultsFor`): its graphics fault, then the terrain's, in `VIEW`'s
  // words for every test of them, and shown with the spike's remedy (`spikeFaultLine`).
  const terrainFault = terrainFaults.get(SPIKE_MAIN_VIEW_ID);
  const faults = [
    ...(drawFault === null ? [] : [drawFault]),
    ...(terrainFault === undefined ? [] : [terrainFault]),
  ];
  // The main view's survey line over a generated world, alone on its lower plate, its units held
  // from one publish to the next as `VIEW`'s primary's are; none over the test planet.
  const terrainLines = useTerrainLines(published?.ground ?? null, faults, SPIKE_RETRY);
  const lowerParts = terrainLowerParts(terrainLines);
  // Its list notes the survey edges its last frame drew, their units held from the notes last shown.
  const [shownNotes, setShownNotes] = useState<ShownNotes>(NO_NOTES);
  const edges = shownFrame?.surveyEdges ?? NO_SURVEY_EDGES;
  let notes = shownNotes.notes;
  if (shownNotes.edges !== edges) {
    notes = runs === null ? [] : surveyEdgeNotes(runs.main.scene, edges, shownNotes.notes);
    setShownNotes({ edges, notes });
  }
  // The photorealistic style's statements, as `VIEW`'s primary states them after its others:
  // `PHOTOREALISTIC: PREPARING` while the wireframe stands in, then `LIGHTING` and the appearance
  // notes; none in the wireframe, or before the first frame.
  const photoreal =
    runs === null || drawnStyle === null
      ? []
      : photorealStatements(
          runs.main,
          spikeLightingOf(runs.main.scene),
          drawnStyle,
          litLabelsOf(runs.main.scene),
        );
  // The test planet's statement stands from the first render; a generated world's terrain line gives
  // way to the terrain's fault, as `VIEW`'s does (`terrainStatementOf`).
  const statements =
    world === "test-planet"
      ? [
          TEST_PLANET_STATEMENT,
          ...(runs === null ? [] : labelStatements(runs.main, terrain?.annunciation ?? null)),
        ]
      : runs === null
        ? []
        : [
            ...labelStatements(
              runs.main,
              terrainStatementOf(terrain?.annunciation ?? null, faults),
            ),
            ...photoreal,
          ];
  const siteElevation =
    prepared === null
      ? null
      : world === "test-planet"
        ? siteElevationReading(prepared.siteHeightM)
        : prepared.siteReading === undefined || prepared.siteReading === null
          ? null
          : siteSurveyReading(prepared.siteReading);
  return (
    <div className="spike">
      <header className="console__header">
        <div className="console__identity">
          <span className="console__ship">HYPERION</span>
          <h1 className="console__title">DESCENT SPIKE</h1>
        </div>
        <div className="console__status">
          <output className="console__banner" aria-label="Mode">
            MEASUREMENT
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
              aria-label={drawnStyle === null ? MAIN_VIEW_NAME : bodyMainViewName(drawnStyle)}
              aria-details={listId}
              data-spike-view="main"
            />
            <div className="view__overlay">
              <ViewLabelBlock
                lines={
                  runs === null
                    ? []
                    : mainLabelLines(runs.main, drawnStyle, shownFrame?.contourIntervalM ?? null)
                }
                statements={statements}
                countLine={null}
                faults={faults.map(spikeFaultLine)}
              />
              {lowerParts.length === 0 ? null : (
                // The window is never smaller than its launch size, where both plates stand whole
                // (decision-r10-t14c-guide Q1 (c)), so that nothing on it gives way.
                <ViewLowerPlate
                  id={`${id}-lower`}
                  parts={lowerParts}
                  readout="stands"
                  survey="stands"
                />
              )}
            </div>
          </div>
          <div className="spike__instruments">
            <Instrument
              title="Orbit"
              canvasRef={setOrbitCanvas}
              stageRef={orbitStageRef}
              run={runs?.orbit ?? null}
              listId={listId}
              cameraLabel="SCRIPTED"
            />
            <Instrument
              title="Craft"
              canvasRef={setCraftCanvas}
              stageRef={craftStageRef}
              run={runs?.craft ?? null}
              listId={listId}
              cameraLabel={null}
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
              <dt>Quality</dt>
              <dd>{setting.toUpperCase()}</dd>
              <dt>Segment</dt>
              <Reading value={pose?.segment.toUpperCase() ?? null} />
              <dt>Script Time</dt>
              <Reading value={pose === null ? null : `${formatNumber(pose.tS, 1)} s`} />
              <dt>Height Above Site</dt>
              <Reading value={pose === null ? null : heightReading(pose.clearanceM, heightUnit)} />
              <dt>Ground Speed</dt>
              <Reading value={pose === null ? null : groundSpeedReading(pose.horizontalSpeedMps)} />
              <dt>Vertical Speed</dt>
              <Reading value={pose === null ? null : verticalSpeedReading(pose.verticalSpeedMps)} />
              <dt>Camera ELV</dt>
              <Reading
                value={
                  pose === null
                    ? null
                    : formatSignedDeg(cameraElevationDeg(pose, spikeWorld.figure))
                }
              />
              <dt>Site Elevation</dt>
              {/* Over a generated world its field holds an Estimated reading: under its label. */}
              <Reading
                value={siteElevation}
                className={world === "test-planet" ? undefined : "spike__site-survey"}
              />
              <dt>Finest Terrain Held</dt>
              <Reading value={shownFrame === null ? null : shownFrame.contact ? "YES" : "NO"} />
              <dt>Patches</dt>
              <dd className="spike__patches">
                {terrain === null ? (
                  <span className="readout__missing">—</span>
                ) : (
                  // One count a line, four lines always, so that the panel never changes height.
                  <output aria-live="off">
                    {patchesReading(terrain)
                      .split("\u00a0· ")
                      .map((part) => (
                        <span key={part.slice(part.indexOf("\u00a0"))}>{part}</span>
                      ))}
                  </output>
                )}
              </dd>
            </dl>
          </section>
          <section className="panel spike__targets" aria-labelledby={`${id}-targets-title`}>
            <h2 className="panel__title" id={`${id}-targets-title`}>
              Targets
            </h2>
            <div id={listId} className="spike__list">
              <ViewMarkList
                rows={runs === null ? [] : markRows(runs.main)}
                notes={notes}
                fromCamera={runs !== null && rangesFromCamera(runs.main.scene)}
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
  /** More class names for its value, or none. */
  readonly className?: string | undefined;
}

/** A reading, or the missing value's `—` while no run draws. */
function Reading({ value, className }: ReadingProps) {
  return (
    <dd className={className}>
      {value === null ? (
        <span className="readout__missing">—</span>
      ) : (
        <output aria-live="off">{value}</output>
      )}
    </dd>
  );
}
