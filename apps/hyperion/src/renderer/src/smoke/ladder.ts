/**
 * The smoke page's `ladder` part (plan R12, R12.T7.a): what one quality setting makes, made and drawn
 * at that setting on SwiftShader, so that a setting left to rot fails `just test-render`.
 *
 * @remarks
 * The brainstorm's third budget rule builds the low setting beside the high one, "or it will rot"
 * (Performance budget). The catalogue's own parts compile every entry once, at `default`
 * (`catalogue.ts`' {@link CatalogueSetting}): an entry's WGSL and pipelines are the same at every
 * setting. What a setting decides is which of them a view makes and at what sizes it draws them, so
 * the run here, one per setting and capability path (`scripts/testRender.sh`'s `--part=ladder`),
 * makes what the setting's own code makes and draws a frame with it:
 * - the terrain session's materials for the setting (`TerrainMaterialSet`: its vertex path, as the
 *   device's limits fit it, and on high the shadow cascades' casters), each compiled into its
 *   pipelines against the patches' own mesh, still forms included;
 * - the photorealistic view prepared at the setting (its bloom chain's levels among what it makes)
 *   and one frame drawn through it at the setting, an Earth with R05's air whose sky view, aerial
 *   perspective and ray march take the setting's `TABLE_SIZES`, its histogram the setting's stride
 *   and its lit bodies the setting's annuli and planetshine, asserting R01's frame properties
 *   (T9.a): every texel finite, the clear where nothing was drawn, the body where it was;
 * - and every pipeline those compiled ({@link RenderEngine.onCompile}) is a `WGSL_CATALOGUE` entry,
 *   so that the parts that compile the catalogue compile what each setting draws with; a terrain
 *   material's still form (`stillMaterialSpec`, R10.T16.i), which differs from its material by one
 *   constant, counts as its material's.
 *
 * The frame is 96 × 54 px, not the setting's render height (`TerrainSettings.renderHeightPx`), whose
 * 1,080 and 720 rows SwiftShader would take minutes over; `view/perf/ladder.test.ts` holds the
 * resolution row to its consumers.
 */

import { add, scale, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { R05_EARTH_REFERENCE } from "../view/atmosphere/earth";
import { AtmosphereTables } from "../view/atmosphere/tables";
import type { ProjectionCamera } from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import { type CatalogueSetting, WGSL_CATALOGUE } from "../view/engine/catalogue";
import type { CompileEvent, RenderEngine } from "../view/engine/types";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PhotorealRenderer } from "../view/photoreal/renderer";
import type { QualitySetting } from "../view/quality/qualitySetting";
import { AU_M } from "../view/scenes/kept";
import { TerrainMaterialSet } from "../view/terrain/materialSet";
import { addCanvas } from "./frames";
import { type Checks, halfTexels } from "./harness";

/** R05's spherical Earth's mean radius, m, on which its medium's tables are built. */
const EARTH_MEAN_RADIUS_M = 6_371_008.8;

/** The frame's camera: 60° across, looking down −z. */
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };

/** The frame's size, px: the display's 16 : 9, small enough for SwiftShader. */
const VIEWPORT = { widthPx: 96, heightPx: 54 } as const;

/** The Earth's radius on the frame, px. */
const BODY_RADIUS_PX = 20;

/**
 * The most a canvas texel where nothing was drawn may hold, in 8-bit green codes: R07.T8.a's black
 * sky (`bodies.ts`' photorealistic frame), the tone curve's toe and dither under it.
 */
const CLEAR_MAX_CODE = 2;

/**
 * The least the lit body's centre holds, in 8-bit green codes: well above the clear, and below the
 * 64 R07.T8.a's airless planet holds at the same exposure, since the air's transmittance dims it.
 */
const LIT_MIN_CODE = 32;

/** The pipeline owners' names the catalogue holds, by spec name. */
const CATALOGUED: ReadonlySet<string> = new Set(WGSL_CATALOGUE.map((entry) => entry.spec.name));

/** The suffix `stillMaterialSpec` puts on a terrain material's still form (R10.T16.i). */
const STILL_SUFFIX = "-still";

/** Whether the catalogue holds the pipeline owner `name`, a still form as its material. */
export function isCatalogued(name: string, catalogued: ReadonlySet<string> = CATALOGUED): boolean {
  if (catalogued.has(name)) {
    return true;
  }
  return name.endsWith(STILL_SUFFIX) && catalogued.has(name.slice(0, -STILL_SUFFIX.length));
}

/** A make the engine refused: its owner's name and why, its causes' messages in turn. */
export interface Refusal {
  readonly name: string;
  readonly why: string;
}

/** `error`'s message and its causes' after it, each `name: message`. */
export function causesOf(error: unknown): string {
  const parts: string[] = [];
  let at: unknown = error;
  for (let depth = 0; depth < 4 && at !== undefined; depth += 1) {
    parts.push(
      at instanceof Error
        ? `${at.name}: ${at.message}`
        : typeof at === "string"
          ? at
          : JSON.stringify(at),
    );
    at = at instanceof Error ? at.cause : undefined;
  }
  return parts.join(" <- ");
}

/** The engine's asynchronous makes the ladder watches for a refusal. */
const WATCHED = new Set<PropertyKey>([
  "createMaterialAsync",
  "createComputeAsync",
  "createPointSplatAsync",
  "prepareMaterial",
]);

/**
 * `engine` as its users see it, but with each refusal of a watched make (a pipeline the device
 * would not make) added to `refusals` before it is passed on: the renderers fall back from some
 * (a refused sea draws no sea), which the frame alone would not show.
 */
export function watchedEngine(engine: RenderEngine, refusals: Refusal[]): RenderEngine {
  return new Proxy(engine, {
    get(target, property): unknown {
      const value: unknown = Reflect.get(target, property, target);
      if (typeof value !== "function") {
        return value;
      }
      if (!WATCHED.has(property)) {
        return (...args: unknown[]): unknown => Reflect.apply(value, target, args);
      }
      return async (...args: unknown[]): Promise<unknown> => {
        try {
          // The engine's own make, awaited so that its refusal is seen here first.
          return await Reflect.apply(value, target, args);
        } catch (error: unknown) {
          const [first] = args;
          const name =
            typeof first === "object" && first !== null && "name" in first
              ? String(first.name)
              : String(property);
          refusals.push({ name, why: causesOf(error) });
          throw error;
        }
      };
    },
  });
}

/** A check's name in the ladder's part at `setting`. */
function named(what: string, setting: CatalogueSetting): string {
  return `R12.T7.a ${what} (${setting})`;
}

/** The terrain session's materials at `setting`, each compiled into its pipelines. */
async function checkTerrainMaterials(
  engine: RenderEngine,
  setting: QualitySetting,
  checks: Checks,
): Promise<void> {
  const set = new TerrainMaterialSet(engine, setting);
  try {
    const compiled = await set.compiled;
    const keys = set.keys;
    const paths = [...new Set(keys.map((key) => key.path))].join(", ");
    const kinds = keys.filter((key) => key.still !== true).map((key) => key.kind);
    checks.check(
      named("every material the terrain draws with compiles", setting),
      keys.length > 0 && compiled === keys.length,
      `${String(compiled)} of ${String(keys.length)} on ${paths === "" ? "no path" : paths}: ` +
        `${kinds.join(", ")}, each with its still form`,
    );
  } finally {
    set.dispose();
  }
}

/** What the frame left: the canvas's 8-bit codes and the composite's HDR texels. */
interface LadderFrame {
  readonly planned: boolean;
  readonly canvas: Uint8Array;
  readonly drawn: Float32Array;
}

/** A canvas's read-back as 8-bit codes: as read where 8-bit, else each value in [0, 1] coded. */
function codesOf(texels: Float32Array | Uint8Array): Uint8Array {
  return texels instanceof Uint8Array
    ? texels
    : Uint8Array.from(texels, (value) => Math.round(Math.min(Math.max(value, 0), 1) * 255));
}

/** Draws the Earth with its air at `setting` through `renderer` and reads the frame back. */
async function drawFrame(
  engine: RenderEngine,
  renderer: PhotorealRenderer,
  tables: AtmosphereTables,
  setting: QualitySetting,
): Promise<LadderFrame> {
  const view = engine.createView(addCanvas(), `smoke ladder ${setting}`);
  view.resize(VIEWPORT);
  try {
    const pxPerRad = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
    const distanceM = EARTH_MEAN_RADIUS_M / Math.sin(BODY_RADIUS_PX / pxPerRad);
    const centreM = vec3(0, 0, -distanceM);
    // The Sun 40° off the line of sight, so that the face the camera sees is lit.
    const towards = vec3(Math.sin((40 * Math.PI) / 180), 0, Math.cos((40 * Math.PI) / 180));
    const id = "0200080020000000.0300";
    const plan = renderer.render(view, {
      camera: CAMERA,
      viewport: VIEWPORT,
      presentedPx: VIEWPORT,
      role: "eye",
      setting,
      exposureScale: 1e-4,
      sky: [],
      starSprites: [],
      hostDraws: new Map(),
      glareSources: [],
      lights: [{ disc: sunLikeHostDisc(), centreM: add(centreM, scale(towards, AU_M)) }],
      bodies: [
        {
          id,
          centreM,
          figure: {
            equatorialRadiusM: EARTH_MEAN_RADIUS_M,
            polarRadiusM: EARTH_MEAN_RADIUS_M,
            pole: null,
          },
          photometry: PROVISIONAL_PHOTOMETRY,
          lighting: undefined,
        },
      ],
      depthWriters: [],
      previousRegimes: new Map([[id, "disc"]]),
      overlay: null,
      meter: null,
      atmospheres: [
        {
          id,
          tables,
          camera: {
            positionM: vec3(0, 0, distanceM),
            orientation: IDENTITY_QUATERNION,
            fovXRad: CAMERA.fovXRad,
            viewport: VIEWPORT,
          },
          toBodyFixed: IDENTITY_QUATERNION,
        },
      ],
    });
    // The canvas is read back in the task that drew it (`bodies.ts`' photorealistic frame).
    const canvas = codesOf(await view.readBack());
    const colours = renderer.frameColours;
    if (colours === null) {
      throw new Error("the photorealistic frame left no scene target to read");
    }
    const drawn = halfTexels(await engine.readTexture(colours.drawn));
    return { planned: plan !== null, canvas, drawn };
  } finally {
    view.dispose();
  }
}

/** The photorealistic view made at `setting`, and its frame's properties (R01.T9.a's). */
async function checkPhotorealFrame(
  engine: RenderEngine,
  setting: QualitySetting,
  checks: Checks,
): Promise<void> {
  const renderer = new PhotorealRenderer(engine, `smoke ladder ${setting}`);
  const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);
  try {
    await renderer.prepare(VIEWPORT, setting, "eye", CAMERA);
    checks.check(
      named("the photorealistic view is made", setting),
      renderer.status === "ready",
      `status ${renderer.status}`,
    );
    const frame = await drawFrame(engine, renderer, tables, setting);
    checks.check(
      named("every texel of the photorealistic frame is finite", setting),
      frame.drawn.length > 0 && frame.drawn.every(Number.isFinite),
      `${String(frame.drawn.length / 4)} texels of the composite's output`,
    );
    const green = (x: number, y: number): number =>
      frame.canvas[(y * VIEWPORT.widthPx + x) * 4 + 1] ?? Number.NaN;
    const corners = [
      green(1, 1),
      green(VIEWPORT.widthPx - 2, 1),
      green(1, VIEWPORT.heightPx - 2),
      green(VIEWPORT.widthPx - 2, VIEWPORT.heightPx - 2),
    ];
    const centre = green(VIEWPORT.widthPx / 2, VIEWPORT.heightPx / 2);
    checks.check(
      named(
        "the frame holds the clear where nothing was drawn and the lit body where it was",
        setting,
      ),
      frame.planned && corners.every((code) => code <= CLEAR_MAX_CODE) && centre >= LIT_MIN_CODE,
      `planned ${String(frame.planned)}; corners ${corners.join(", ")} (at most ` +
        `${String(CLEAR_MAX_CODE)}), the body's centre ${String(centre)} (at least ` +
        `${String(LIT_MIN_CODE)}), green codes`,
    );
  } finally {
    renderer.dispose();
    tables.dispose();
  }
}

/**
 * The ladder's part at `setting` (the module's remarks): the terrain's materials, the
 * photorealistic frame, and every pipeline they compiled found in the catalogue.
 *
 * @param group - The page's group runner, each group timed and a throw its failure.
 */
export async function checkLadder(
  engine: RenderEngine,
  setting: QualitySetting,
  checks: Checks,
  group: (name: string, work: () => Promise<void>) => Promise<void>,
): Promise<void> {
  const compiled: CompileEvent[] = [];
  const refusals: Refusal[] = [];
  const watched = watchedEngine(engine, refusals);
  const stop = engine.onCompile((event) => {
    compiled.push(event);
  });
  try {
    await group(named("the terrain's materials", setting), () =>
      checkTerrainMaterials(watched, setting, checks),
    );
    await group(named("the photorealistic frame", setting), () =>
      checkPhotorealFrame(watched, setting, checks),
    );
  } finally {
    stop();
  }
  checks.check(
    named("the device makes every pipeline the setting asks for", setting),
    refusals.length === 0,
    refusals.length === 0
      ? "none refused"
      : refusals.map(({ name, why }) => `${name} refused: ${why}`).join("; "),
  );
  const names = [...new Set(compiled.map((event) => event.name))].toSorted();
  const missing = names.filter((name) => !isCatalogued(name));
  const ms = compiled.reduce((sum, event) => sum + event.ms, 0);
  checks.check(
    named("every pipeline the setting compiled is a catalogue entry", setting),
    names.length > 0 && missing.length === 0,
    `${String(compiled.length)} pipelines of ${String(names.length)} owners, ` +
      `${(ms / 1_000).toFixed(1)} s compiling in all; not in the catalogue: ` +
      (missing.length === 0 ? "none" : missing.join(", ")),
  );
}
