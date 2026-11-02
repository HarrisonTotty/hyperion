/**
 * The smoke page's captures of a host star's glare through the air (fix-view-light, the setting
 * Sun's halo, 2026-11-01): `ATMOSPHERE TEST`'s `TEST EARTH` (`view/scenes/atmosphereScene.ts`), its
 * tables built from its `earth` case as the scene's cache builds them (`builtOfCase`), drawn through
 * the view's photorealistic frame at 960 × 540 and exposed by the view's own controller under
 * `AVG`, the view's default meter, as `viewLight.ts`'s captures are.
 *
 * @remarks
 * Five framings, the Sun at the frame's centre: a sunset from 10 m above the ground with the Sun's
 * centre 5°, 1° and 0° above the horizon; and the Sun from 400 km up through the limb, its line
 * 20 km and 12 km above the ground at its tangent. Each is drawn twice, settled on its own: its
 * glare dimmed by the air (`frameGlareAir`), and, for the owner's comparison, its glare as it was
 * before, the disc's light as the frame's bodies leave it (`NO_AIR`). Each records the factor the
 * air gives the glare's light (`discLightThroughAir`); the composite's on the disc, the drawn disc's
 * light over the scene's before the composite, summed over the disc's texels in a frame drawn at
 * an exposure that holds the disc inside the target's range (`DISC_IN_RANGE_EV100`), which the
 * glare's is held to within 5% in each channel whose factor is 10⁻³ or more (below it the sky the air scatters
 * onto the disc's pixels is a share of their light: 12% in blue at 1°, and all of it at 0°); and
 * the halo's radial profile, the sky's median green code at angles from 1° to 24°, against the
 * median of each such pixel's own light plus the closed form's veil, times the transmittance,
 * through the tone curve (`tonemapTexel`, no dither), and of the same with the closed form alone.
 * Each frame's work is drained before the next is drawn: SwiftShader takes seconds over a frame in
 * the air, and its GPU process did not survive a wait on a queue of them.
 */

import { photorealFrame, frameGlareAir } from "../displays/view/photorealFrame";
import { startRun, type ViewRun } from "../displays/view/viewRun";
import { add, norm, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { AtmosphereTables } from "../view/atmosphere/tables";
import type { CameraPose } from "../view/camera/pose";
import type { Viewport } from "../view/camera/projection";
import { conjugate, lookAlong, rotate } from "../view/camera/quaternion";
import { type ViewRole, viewId } from "../view/camera/state";
import type { RenderEngine, RenderView } from "../view/engine/types";
import {
  controlEv100,
  type ExposureControl,
  exposureScale,
  VIEW_CAMERA,
} from "../view/photometry/exposure";
import type { Rgb } from "../view/photometry/toneCurve";
import { type PhotorealFrame, PhotorealRenderer } from "../view/photoreal/renderer";
import { AutoExposure } from "../view/post/autoExposure";
import { type GlareSource, glareSourceVeilOf, glareSpreadTerms } from "../view/post/glare";
import type { DiscAir } from "../view/post/glareAir";
import type { Histogram } from "../view/post/histogram";
import { METER_CLASS } from "../view/post/meter";
import { tonemapTexel } from "../view/post/tonemap";
import { SETTINGS } from "../view/quality/qualitySetting";
import {
  ATMOSPHERE_EARTH,
  ATMOSPHERE_FIXTURES,
  atmosphereScene,
  builtOfCase,
} from "../view/scenes/atmosphereScene";
import { KEPT_SYSTEM } from "../view/scenes/kept";
import { HostDiscLayer } from "../view/sky/disc";
import { discLightThroughAir } from "../view/sky/discFlux";
import { DEFAULT_EYE_OBSERVER } from "../view/sky/eye";
import { emptyDrawList, viewStrokesAt } from "../view/wireframe/drawList";
import { base64Of, type CapturedImage } from "./atmosphere";
import { addCanvas } from "./frames";
import { type Checks, halfTexels } from "./harness";

/**
 * The captures' size, px: a quarter of the view's at 1080p, whose air SwiftShader's GPU process does
 * not survive (it exited 160 s in, 2026-11-02); the Sun's disc is 9 px across.
 */
const VIEWPORT: Viewport = { widthPx: 960, heightPx: 540 };

/** The field across the frame, deg. */
const FOV_DEG = 60;

const RAD_PER_DEG = Math.PI / 180;

/** No wireframe draw list: the stars are not under test. */
const NO_LIST = emptyDrawList(viewStrokesAt(1));

/** The controller's step, s: a 60 Hz frame, as `viewLight.ts`'s. */
const STEP_S = 1 / 60;

/** The most frames a framing is drawn while its exposure settles. */
const MAX_FRAMES = 12;

/** Settled: two frames in a row whose applied EV100 differ by under this. */
const SETTLED_EV = 0.02;

/** The view's exposure as a new view opens: `AUTO` at `MAN`'s −1. */
const START: ExposureControl = { kind: "auto", ev100: -1 };

/** The camera's height above the ground at the sunset, m. */
const SUNSET_HEIGHT_M = 10;

/** The camera's height above the ground in orbit, m. */
const ORBIT_HEIGHT_M = 400_000;

/** `TEST EARTH` as the scene places it. */
function testEarth(): { readonly centreM: Vec3; readonly radiusM: number } {
  const fixture = ATMOSPHERE_FIXTURES.find((each) => each.id === ATMOSPHERE_EARTH);
  if (fixture === undefined) {
    throw new Error("ATMOSPHERE TEST holds no TEST EARTH");
  }
  return { centreM: fixture.centreM, radiusM: fixture.figure.equatorialRadiusM };
}

/** One framing: its name, what it shows, and the camera's place from the barycentre. */
interface Framing {
  readonly name: string;
  readonly label: string;
  /** The camera's position, m from the barycentre along the galactic axes. */
  readonly positionM: Vec3;
  /** The local zenith, unit: the frame's up. */
  readonly up: Vec3;
}

/**
 * A camera `heightM` above `TEST EARTH` at the point whose zenith makes `angleRad` with the
 * direction to the star, which lies along −x from it (the scene's layout).
 */
function cameraAt(
  heightM: number,
  angleRad: number,
): { readonly positionM: Vec3; readonly up: Vec3 } {
  const earth = testEarth();
  const up = vec3(-Math.cos(angleRad), Math.sin(angleRad), 0);
  return { positionM: add(earth.centreM, scale(up, earth.radiusM + heightM)), up };
}

/** The Sun's centre `elevationDeg` above the horizon of a camera 10 m up. */
function sunset(elevationDeg: number): Framing {
  return {
    name: `sunset-${String(elevationDeg)}deg`,
    label: `the Sun ${String(elevationDeg)}° above the horizon, 10 m up`,
    ...cameraAt(SUNSET_HEIGHT_M, Math.PI / 2 - elevationDeg * RAD_PER_DEG),
  };
}

/** The Sun from 400 km up, its line `tangentKm` above the ground where it passes the limb. */
function throughTheLimb(tangentKm: number): Framing {
  const { radiusM } = testEarth();
  const sine = (radiusM + tangentKm * 1000) / (radiusM + ORBIT_HEIGHT_M);
  return {
    name: `orbit-limb-${String(tangentKm)}km`,
    label: `the Sun from 400 km up, its line ${String(tangentKm)} km above the limb`,
    ...cameraAt(ORBIT_HEIGHT_M, Math.PI - Math.asin(sine)),
  };
}

const FRAMINGS: ReadonlyArray<Framing> = [
  sunset(5),
  sunset(1),
  sunset(0),
  throughTheLimb(20),
  throughTheLimb(12),
];

/** The direction from `positionM` to the star at the barycentre, unit. */
function toStar(positionM: Vec3): Vec3 {
  return normalise(sub(vec3(0, 0, 0), positionM));
}

/** What one settled drawing gave. */
interface Settled {
  readonly ev100: number;
  readonly frames: number;
  readonly srgb: Uint8Array;
  /** The frame's texels after the composite, pre-exposed. */
  readonly drawn: Float32Array;
  /** The last presented frame, as drawn. */
  readonly frame: PhotorealFrame;
  /** Its glare sources as they were before the air was taken (`clearSources`). */
  readonly clear: ReadonlyArray<GlareSource>;
}

/**
 * Waits for the renderer's next histogram, read back after its frame's copy has settled: up to 10 s,
 * since SwiftShader takes seconds over a frame in the air.
 */
async function nextHistogram(renderer: PhotorealRenderer): Promise<Histogram | undefined> {
  for (let attempt = 0; attempt < 1000; attempt += 1) {
    const histogram = renderer.takeHistogram();
    if (histogram !== undefined) {
      return histogram;
    }
    // The read settles on the device's timeline: each look waits for the one before.
    // oxlint-disable-next-line no-await-in-loop
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 10);
    });
  }
  return undefined;
}

/** The glare sources of `frame` as they were before the air was taken: its disc's light past the bodies. */
function clearSources(frame: PhotorealFrame, discs: HostDiscLayer): GlareSource[] {
  return discs.glareSources(
    frame.camera,
    frame.viewport,
    frame.role,
    frame.bodies.map((body) => ({
      centreM: body.centreM,
      radiusM: Math.sqrt(body.figure.equatorialRadiusM * body.figure.polarRadiusM),
    })),
    SETTINGS.high.photoreal.discAnnuli,
  );
}

/** `framing`'s run, its camera placed and turned to the Sun, and its pose. */
function framingRun(framing: Framing): { readonly run: ViewRun; readonly pose: CameraPose } {
  const start = startRun(atmosphereScene());
  const pose: CameraPose = {
    frame: { kind: "system", system: KEPT_SYSTEM },
    positionM: framing.positionM,
    orientation: lookAlong(toStar(framing.positionM), framing.up),
  };
  return {
    run: {
      ...start,
      camera: { ...start.camera, pose, fovDeg: FOV_DEG, style: "photorealistic" as const },
    },
    pose,
  };
}

/**
 * The photorealistic frame of `run` at `ev100` with `TEST EARTH`'s air, its histogram taken under
 * `meter`, or none for `null`.
 */
function airFrame(
  run: ViewRun,
  pose: CameraPose,
  discs: HostDiscLayer,
  tables: AtmosphereTables,
  ev100: number,
  meter: "average" | null,
  present: boolean,
): PhotorealFrame {
  return photorealFrame({
    run,
    pose,
    viewport: VIEWPORT,
    presentedPx: VIEWPORT,
    setting: "high",
    exposureScale: exposureScale(ev100),
    list: NO_LIST,
    sky: null,
    band: null,
    discs,
    cube: null,
    previousRegimes: new Map(),
    overlay: null,
    meter,
    present,
    atmospheres: (body) => (body === ATMOSPHERE_EARTH ? tables : null),
  });
}

/**
 * The exposure at which the composite's transmittance on the disc is read, EV100: 18, at which the
 * scene's Sun (`sunLikeHostDisc`'s mean of 1.96 × 10⁹ cd/m², resolved at 1 au) is pre-exposed to
 * some 6,200 and its brightest texel, the disc's centre in red, to some 8,500, an eighth of
 * `rgba16float`'s 65,504, whatever `AVG` settles at.
 *
 * @remarks
 * A camera's `AVG` settles below 12 on the Sun through the limb (ux-calls-view, item 3a), where the
 * disc's texels pass the target's range, which they fit only from EV100 15.05 up, and the sky the
 * air scatters onto them is a share of the clamped light: at 12 km, blue 0.0824 against the
 * glare's 0.0716, some 15% over (smoke part 1 with `--captures`, a pod run of 2026-11-01).
 */
const DISC_IN_RANGE_EV100 = 18;

/**
 * The composite's transmittance on the disc in one frame of `framing` with the air drawn at
 * {@link DISC_IN_RANGE_EV100} (`compositeDiscTransmittance`), not presented and taking no
 * histogram, so that no reading of it reaches the next framing's controller.
 */
async function inRangeDiscTransmittance(
  engine: RenderEngine,
  renderer: PhotorealRenderer,
  view: RenderView,
  discs: HostDiscLayer,
  tables: AtmosphereTables,
  framing: Framing,
): Promise<Rgb> {
  const { run, pose } = framingRun(framing);
  renderer.render(view, airFrame(run, pose, discs, tables, DISC_IN_RANGE_EV100, null, false));
  const colours = renderer.frameColours;
  if (colours === null) {
    throw new Error("the photorealistic renderer drew no frame");
  }
  const [drawn, scene] = await Promise.all([
    engine.readTexture(colours.drawn).then(halfTexels),
    engine.readTexture(colours.scene.colour).then(halfTexels),
  ]);
  return compositeDiscTransmittance(drawn, scene);
}

/**
 * Draws `framing` until the controller's exposure settles, its glare through the air where
 * `air` holds and as before where not, and reads its last frame back.
 */
async function settle(
  engine: RenderEngine,
  renderer: PhotorealRenderer,
  view: RenderView,
  discs: HostDiscLayer,
  tables: AtmosphereTables,
  framing: Framing,
  air: boolean,
): Promise<Settled> {
  const { run, pose } = framingRun(framing);
  const exposure = new AutoExposure({
    source: viewId("smoke"),
    program: VIEW_CAMERA,
    control: START,
    meter: "average",
  });
  exposure.cut();
  let histogram: Histogram | undefined;
  let last = Number.NaN;
  let frames = 0;
  let srgb: Uint8Array = new Uint8Array(0);
  let presented: { readonly frame: PhotorealFrame; readonly clear: GlareSource[] } | null = null;
  for (; frames < MAX_FRAMES; frames += 1) {
    const ev100 = controlEv100(exposure.step(histogram, STEP_S).control);
    const present = !exposure.holds;
    const drawnFrame = airFrame(run, pose, discs, tables, ev100, "average", present);
    // Of the disc's light at this frame's exposure, which the layer's last frame holds.
    const clear = clearSources(drawnFrame, discs);
    const frame = air ? drawnFrame : { ...drawnFrame, glareSources: clear };
    renderer.render(view, frame);
    exposure.noteImage(true);
    // Each frame's work is drained before the next, so that no wait holds a queue of them:
    // SwiftShader's GPU process does not survive one (it exited in such a wait, 2026-11-02).
    const target = renderer.frameColours;
    if (target !== null) {
      // oxlint-disable-next-line no-await-in-loop
      await engine.readTexture(target.drawn);
    }
    if (present) {
      presented = { frame, clear };
      // The canvas is read back in the task that drew it, each frame before the next.
      // oxlint-disable-next-line no-await-in-loop
      const bytes = await view.readBack();
      srgb = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes);
    }
    // oxlint-disable-next-line no-await-in-loop
    histogram = await nextHistogram(renderer);
    if (present && Math.abs(ev100 - last) < SETTLED_EV) {
      break;
    }
    last = ev100;
  }
  const colours = renderer.frameColours;
  if (colours === null || presented === null) {
    throw new Error("the photorealistic renderer presented no frame");
  }
  const drawn = halfTexels(await engine.readTexture(colours.drawn));
  return { ev100: last, frames: frames + 1, srgb, drawn, ...presented };
}

/** The view-space direction through pixel (x, y)'s centre, unit. */
function pixelDirection(x: number, y: number): Vec3 {
  const t = Math.tan((FOV_DEG * RAD_PER_DEG) / 2);
  return normalise(
    vec3(
      (((x + 0.5) / VIEWPORT.widthPx) * 2 - 1) * t,
      (1 - ((y + 0.5) / VIEWPORT.heightPx) * 2) * (t * (VIEWPORT.heightPx / VIEWPORT.widthPx)),
      -1,
    ),
  );
}

/** The median of `values`, or `NaN` for none. */
function median(values: number[]): number {
  if (values.length === 0) {
    return Number.NaN;
  }
  values.sort((a, b) => a - b);
  return values[Math.floor(values.length / 2)] ?? Number.NaN;
}

/** The angles from the Sun's centre the halo's profile is read at, deg. */
const PROFILE_DEG: ReadonlyArray<number> = [1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24];

/** One angle of the halo's profile: the sky's median code, and the closed form's with and without the air. */
interface ProfilePoint {
  readonly deg: number;
  readonly measured: number;
  readonly withAir: number;
  readonly withoutAir: number;
}

/** The green code of the pre-exposed `light` plus `veil`'s light at `exposure`, no dither. */
function codeOf(light: Rgb, veil: Rgb, exposure: number): number {
  const [, g] = tonemapTexel(
    [light[0] + veil[0] * exposure, light[1] + veil[1] * exposure, light[2] + veil[2] * exposure],
    1,
    null,
  );
  return Math.round(g * 255);
}

/** The veil of `sources` at `thetaRad` from the Sun in a view of `role`, cd/m². */
function veilOf(sources: ReadonlyArray<GlareSource>, thetaRad: number, role: ViewRole): Rgb {
  const terms = glareSpreadTerms(role, DEFAULT_EYE_OBSERVER);
  const veil: [number, number, number] = [0, 0, 0];
  for (const source of sources) {
    const [r, g, b] = glareSourceVeilOf(source, thetaRad, terms);
    veil[0] += r;
    veil[1] += g;
    veil[2] += b;
  }
  return veil;
}

/**
 * The halo's radial profile in `settled` (the module's remarks): over the sky's pixels within 3% of
 * each angle, their median code, and the median of the codes their own light gives with the veil of
 * `dimmed`, the closed form times the transmittance, and of `clear`, the closed form alone.
 */
function haloProfile(
  settled: Settled,
  dimmed: ReadonlyArray<GlareSource>,
  clear: ReadonlyArray<GlareSource>,
): ProfilePoint[] {
  const sun = dimmed[0] ?? clear[0];
  if (sun === undefined) {
    return [];
  }
  const towards = rotate(conjugate(settled.frame.camera.orientation), sun.direction);
  const s = settled.frame.exposureScale;
  const bins = PROFILE_DEG.map(
    (): { measured: number[]; withAir: number[]; withoutAir: number[] } => ({
      measured: [],
      withAir: [],
      withoutAir: [],
    }),
  );
  const veils = PROFILE_DEG.map((deg) => ({
    dimmed: veilOf(dimmed, deg * RAD_PER_DEG, settled.frame.role),
    clear: veilOf(clear, deg * RAD_PER_DEG, settled.frame.role),
  }));
  for (let y = 0; y < VIEWPORT.heightPx; y += 1) {
    for (let x = 0; x < VIEWPORT.widthPx; x += 1) {
      const i = y * VIEWPORT.widthPx + x;
      if (Math.round(settled.drawn[4 * i + 3] ?? 1) !== METER_CLASS.other) {
        continue;
      }
      const d = pixelDirection(x, y);
      const cos = d.x * towards.x + d.y * towards.y + d.z * towards.z;
      const deg = Math.acos(Math.min(1, cos)) / RAD_PER_DEG;
      PROFILE_DEG.forEach((at, k) => {
        const bin = bins[k];
        const veil = veils[k];
        if (bin === undefined || veil === undefined || Math.abs(deg / at - 1) > 0.03) {
          return;
        }
        const light: Rgb = [
          settled.drawn[4 * i] ?? 0,
          settled.drawn[4 * i + 1] ?? 0,
          settled.drawn[4 * i + 2] ?? 0,
        ];
        bin.measured.push(settled.srgb[4 * i + 1] ?? 0);
        bin.withAir.push(codeOf(light, veil.dimmed, s));
        bin.withoutAir.push(codeOf(light, veil.clear, s));
      });
    }
  }
  return PROFILE_DEG.flatMap((deg, k): ProfilePoint[] => {
    const bin = bins[k];
    return bin === undefined || bin.measured.length < 16
      ? []
      : [
          {
            deg,
            measured: median(bin.measured),
            withAir: median(bin.withAir),
            withoutAir: median(bin.withoutAir),
          },
        ];
  });
}

/**
 * The composite's transmittance on the disc in a frame: its `drawn` light over the `scene`'s before
 * the composite, each summed over the scene's disc texels (meter class 0), per channel; `NaN` with
 * none in frame.
 */
function compositeDiscTransmittance(drawnTexels: Float32Array, sceneTexels: Float32Array): Rgb {
  const drawn: [number, number, number] = [0, 0, 0];
  const before: [number, number, number] = [0, 0, 0];
  for (let i = 0; i < VIEWPORT.widthPx * VIEWPORT.heightPx; i += 1) {
    if (Math.round(sceneTexels[4 * i + 3] ?? 1) !== METER_CLASS.hostDisc) {
      continue;
    }
    for (const c of [0, 1, 2] as const) {
      drawn[c] += drawnTexels[4 * i + c] ?? 0;
      before[c] += sceneTexels[4 * i + c] ?? 0;
    }
  }
  return [drawn[0] / before[0], drawn[1] / before[1], drawn[2] / before[2]];
}

/**
 * The factor the air between gives the glare's light in `frame`, its first host star's
 * (`discLightThroughAir`'s `light`), over the part of the disc above the ground, the part whose
 * texels the composite's figure sums: each body's ground taken as the occluders' (`"occluders"`).
 */
function glareLight(frame: PhotorealFrame, sun: GlareSource | undefined): Rgb {
  const light = frame.lights[0];
  if (sun === undefined || light === undefined) {
    return [Number.NaN, Number.NaN, Number.NaN];
  }
  const airs = frameGlareAir(frame.atmospheres ?? [], frame.bodies, frame.lights)(
    light.disc.star,
    sun.direction,
    sun.angularRadiusRad,
    norm(light.centreM),
  ).map((air): DiscAir =>
    air.kind === "across"
      ? {
          kind: "across",
          vertical: air.vertical,
          offsets: air.offsets,
          weights: air.weights,
          lines: air.lines,
          groundBy: "occluders",
        }
      : air,
  );
  return discLightThroughAir(light.disc, airs, sun.direction, 0).light;
}

/** What one framing gave, with its air and without. */
interface Result {
  readonly framing: Framing;
  readonly withAir: Settled;
  readonly withoutAir: Settled;
  readonly glareT: Rgb;
  readonly discT: Rgb;
  readonly profile: ReadonlyArray<ProfilePoint>;
}

/** Three significant figures of each channel. */
function rgbText(rgb: Rgb): string {
  return rgb.map((value) => value.toPrecision(3)).join("/");
}

/** A result's figures as a check's detail. */
function resultText(result: Result): string {
  const profile = result.profile
    .map(
      (point) =>
        `${String(point.deg)}°: ${String(point.measured)}/${String(point.withAir)}/${String(point.withoutAir)}`,
    )
    .join(", ");
  return (
    `${result.framing.label}: EV100 ${result.withAir.ev100.toFixed(2)} after ${String(result.withAir.frames)} frames ` +
    `(without the air ${result.withoutAir.ev100.toFixed(2)}); the glare's light through the air ${rgbText(result.glareT)}, ` +
    `the composite's on the disc ${rgbText(result.discT)}; halo measured/closed form × T/closed form: ${profile}`
  );
}

/**
 * The five framings, settled with the air and without, captured and judged: the captures are named
 * `fix-view-light-air-<framing>` and `fix-view-light-air-<framing>-before`.
 */
export async function captureViewLightAir(
  engine: RenderEngine,
  checks: Checks,
): Promise<CapturedImage[]> {
  const loaded = await import("../view/atmosphere/reference/cases");
  const built = builtOfCase(loaded.EARTH_CASE, "earth");
  const tables = new AtmosphereTables(
    engine,
    built.medium,
    built.bottomRadiusM,
    built.curves,
    built.body,
  );
  const view = engine.createView(addCanvas(), "smoke view light air");
  view.resize(VIEWPORT);
  const renderer = new PhotorealRenderer(engine, "smoke view light air");
  const discs = new HostDiscLayer(engine);
  const images: CapturedImage[] = [];
  const results: Result[] = [];
  try {
    const first = FRAMINGS[0];
    // The view's role as its runs start, a camera's (`startRun`).
    await renderer.prepare(VIEWPORT, "high", startRun(atmosphereScene()).camera.role, {
      orientation: lookAlong(toStar(first?.positionM ?? vec3(1, 0, 0)), first?.up ?? vec3(0, 1, 0)),
      fovXRad: FOV_DEG * RAD_PER_DEG,
    });
    for (const framing of FRAMINGS) {
      // Each drawing settles before the next, as the views' frames do.
      // oxlint-disable-next-line no-await-in-loop
      const withAir = await settle(engine, renderer, view, discs, tables, framing, true);
      // Its comparison drawn on the same renderer after it, settled on its own.
      // oxlint-disable-next-line no-await-in-loop
      const withoutAir = await settle(engine, renderer, view, discs, tables, framing, false);
      // The in-range frame reads the renderer's latest targets, so it follows the framing's
      // settles.
      // oxlint-disable-next-line no-await-in-loop
      const discT = await inRangeDiscTransmittance(engine, renderer, view, discs, tables, framing);
      const dimmed = withAir.frame.glareSources;
      const { clear } = withAir;
      results.push({
        framing,
        withAir,
        withoutAir,
        glareT: glareLight(withAir.frame, dimmed[0] ?? clear[0]),
        discT,
        profile: haloProfile(withAir, dimmed, clear),
      });
      for (const [suffix, settled] of [
        ["", withAir],
        ["-before", withoutAir],
      ] as const) {
        images.push({
          name: `fix-view-light-air-${framing.name}${suffix}`,
          width: VIEWPORT.widthPx,
          height: VIEWPORT.heightPx,
          rgba: base64Of(settled.srgb),
        });
      }
    }
  } finally {
    renderer.dispose();
    view.dispose();
    tables.dispose();
  }
  const byName = (name: string): Result | undefined =>
    results.find((result) => result.framing.name === name);
  for (const result of results) {
    checks.check(
      `fix-view-light air (${result.framing.name}) the record`,
      true,
      resultText(result),
    );
  }
  const sunsets = ["sunset-5deg", "sunset-1deg", "sunset-0deg"].map(byName);
  checks.check(
    "fix-view-light air: a setting Sun's glare is dimmed by the air, each channel more from 5° to 1° to 0°, and reddened, red above green above blue",
    sunsets.every(
      (result) =>
        result !== undefined &&
        result.glareT[0] < 1 &&
        result.glareT[0] > result.glareT[1] &&
        result.glareT[1] > result.glareT[2],
    ) &&
      [0, 1, 2].every((c) => {
        const [five, one, zero] = sunsets.map((result) => result?.glareT[c] ?? Number.NaN);
        return (five ?? 0) > (one ?? 0) && (one ?? 0) > (zero ?? 0);
      }),
    sunsets
      .map((result) => (result === undefined ? "not drawn" : rgbText(result.glareT)))
      .join("; "),
  );
  for (const result of results) {
    const compared = [0, 1, 2].filter((c) => (result.glareT[c] ?? 0) >= 1e-3);
    checks.check(
      `fix-view-light air (${result.framing.name}) the glare is dimmed as the composite dims the disc: within 5% in each channel at 10⁻³ or more (none at 0°, recorded)`,
      result.glareT.every((value) => Number.isFinite(value)) &&
        compared.every(
          (c) => Math.abs((result.glareT[c] ?? 0) / (result.discT[c] ?? Number.NaN) - 1) <= 0.05,
        ),
      `channels compared: ${compared.length === 0 ? "none" : compared.join(", ")}; ${resultText(result)}`,
    );
    checks.check(
      `fix-view-light air (${result.framing.name}) the Sun's halo falls as the closed form times the air's transmittance: every angle within 3 codes`,
      result.profile.length >= 5 &&
        result.profile.every((point) => Math.abs(point.measured - point.withAir) <= 3),
      resultText(result),
    );
  }
  return images;
}
