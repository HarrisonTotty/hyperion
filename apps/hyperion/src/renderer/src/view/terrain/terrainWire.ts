/**
 * The wireframe's terrain (plan R10, R10.T11; Design note 14): a body's surveyed terrain drawn in
 * the wireframe style as depth, with its contours, its coastline and the body's graticule drawn on
 * it in the same pass, and the rules that set them.
 *
 * @remarks
 * - **The interval** ({@link contourInterval}) is the step of the 1-2-5 sequence nearest, in log
 *   terms, to a twentieth of the camera's height above the ground at its nadir
 *   ({@link heightAboveGroundM}: the surveyed ground's drawn height there, the datum where it is
 *   not surveyed or not yet drawn), never below 1 m, with ±20% hysteresis about each geometric
 *   midpoint; every fifth contour is an index contour (USGS). It is a rule of pose alone, so that
 *   every view in one pose states the same interval.
 * - **The phase** ({@link contourPhase}). The height a fragment interpolates is patch-local,
 *   h − h₀ for the patch origin's height h₀ as its slot record holds it, plus a phase
 *   φ = h₀ mod 5 CI formed in `f64` on the CPU, so that the `f32` height above the datum (2 mm
 *   steps at ±20 km) never quantises the derivative near the camera, and the index is told.
 * - **The lines** are antialiased at constant width from u = (h − h₀ + φ) ÷ CI:
 *   d = |fract(u − ½) − ½| ÷ |∇u| px, coverage clamp(w ÷ 2 + ½ − d, 0, 1), the gradient's length
 *   rather than `fwidth` (after Golus 2021). Minor contours are 1 px and index contours 1.5 px in
 *   `--text-muted`, the coastline 2 px in `--text` at the field's sea level on a world with a sea,
 *   each in CSS px times the view's stroke scale. A family is drawn in full where its lines stand
 *   6 px apart or more and fades out as they close to 3 px ({@link contourFade}), the minor
 *   contours first; the coastline is always drawn ({@link contourShade}, the fragment's twin).
 * - **The graticule** is R02's, drawn on the terrain's own height where the terrain is surveyed
 *   (Design note 14): its parallels and meridians at the body's step, the equator and the prime
 *   meridian a step heavier, cased as R02's lines are. Each patch's latitude and longitude are
 *   formed in `f64` on the CPU at its origin ({@link graticulePatch}) and each vertex's offset
 *   from them on the GPU in a form that forms no difference of nearly equal numbers
 *   ({@link graticuleOffsetsRad}, its twin), so that a line stands still to well under a pixel at
 *   the ground.
 */

import type { Vec3 } from "../../geometry/vec3";
import {
  type QualitySetting,
  TERRAIN_SETTINGS,
  type TerrainSettings,
  type TerrainVertexPath,
  WIREFRAME_TERRAIN_SLOTS,
} from "../quality/qualitySetting";
import { terrainSlotFields } from "./slotLayout";
import type { StorageBufferSpec, WgslMaterialSpec } from "../engine/types";
import frameWgsl from "../shaders/frame.wgsl?raw";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import { PATH_SOURCES, TERRAIN_BINDINGS, terrainVertexStorage } from "./gpu/material";
import type { BodyFigure } from "./planet";
import { datumGroundUnder } from "./grounded";
import patchVertexWgsl from "./shaders/patchVertex.wgsl?raw";
import terrainPatchWgsl from "./shaders/terrainPatch.wgsl?raw";
import terrainSurveyWgsl from "./shaders/terrainSurvey.wgsl?raw";
import terrainWireWgsl from "./shaders/terrainWire.wgsl?raw";

/**
 * The share of the camera's height above its nadir's ground that the interval nears: 1 ÷ 20
 * (Design note 14; no published precedent, settled by R10.T11's legibility check).
 */
export const CONTOUR_HEIGHT_FRACTION = 1 / 20;

/**
 * The interval's hysteresis (Design note 14): it steps up once a twentieth of the height passes
 * the geometric midpoint to the next step by this share, and down once it falls this share below
 * the midpoint to the step below.
 */
export const CONTOUR_HYSTERESIS = 0.2;

/** The finest interval, m: 1 (Design note 14). */
export const MIN_CONTOUR_INTERVAL_M = 1;

/** Every fifth contour is an index contour, as on USGS maps. */
export const CONTOUR_INDEX_EVERY = 5;

/**
 * The contours' widths, CSS px, drawn at the view's stroke scale (the guide's Views, signoff-9):
 * minor 1, index 1.5 and the coastline 2, so that each kind differs from the others by width.
 */
export const CONTOUR_PX = { minor: 1, index: 1.5, coastline: 2 } as const;

/**
 * A contour family's fade, CSS px of spacing at the view's stroke scale (the guide's Views): drawn
 * in full from 6 px, faded linearly to nothing at 3 px.
 */
export const CONTOUR_FADE_PX = { full: 6, gone: 3 } as const;

/** The 1-2-5 sequence's mantissas. */
const MANTISSAS = [1, 2, 5] as const;

/** The most steps the interval runs to: step 45, 1 × 10¹⁵ m, far past any body's. */
const MAX_STEP = 3 * 15;

/**
 * Step `k` of the 1-2-5 sequence from {@link MIN_CONTOUR_INTERVAL_M}, m: 1, 2, 5, 10, 20, 50, …
 *
 * @throws RangeError for a step that is not a whole number from 0 to {@link MAX_STEP}.
 */
export function contourStepM(k: number): number {
  if (!Number.isInteger(k) || k < 0 || k > MAX_STEP) {
    throw new RangeError(`step ${String(k)} is not one of the interval's`);
  }
  const mantissa = MANTISSAS[k % 3] ?? 1;
  return mantissa * 10 ** Math.floor(k / 3);
}

/** The step of the sequence that `intervalM` is, or `null` for a value that is none. */
function stepOf(intervalM: number): number | null {
  for (let k = 0; k <= MAX_STEP; k += 1) {
    if (contourStepM(k) === intervalM) {
      return k;
    }
  }
  return null;
}

/** The geometric midpoint between step `k` and the step above it, m. */
function midpointAboveM(k: number): number {
  return Math.sqrt(contourStepM(k) * contourStepM(k + 1));
}

/**
 * The contour interval at a camera's height above the ground at its nadir, m (Design note 14): the
 * 1-2-5 step nearest a twentieth of the height in log terms, never below 1 m; from
 * `previousIntervalM`, the interval last stated, it steps up only once a twentieth of the height
 * passes the geometric midpoint to the next step by 20%, and down only once it falls 20% below the
 * midpoint to the step below, as many steps as the height has moved.
 *
 * @param aglM - The camera's height above the ground at its nadir, m; at or below the ground, the
 *   finest interval.
 * @param previousIntervalM - The interval last stated, m, or `null` with none (or one that is not
 *   a step).
 * @throws RangeError for a height that is not finite.
 */
export function contourInterval(aglM: number, previousIntervalM: number | null): number {
  if (!Number.isFinite(aglM)) {
    throw new RangeError(`a height of ${String(aglM)} m sets no contour interval`);
  }
  const target = Math.max(aglM, 0) * CONTOUR_HEIGHT_FRACTION;
  const held = previousIntervalM === null ? null : stepOf(previousIntervalM);
  if (held === null) {
    // The nearest step in log terms: the one whose midpoints bracket the target.
    let k = 0;
    while (k < MAX_STEP && target >= midpointAboveM(k)) {
      k += 1;
    }
    return contourStepM(k);
  }
  let k = held;
  while (k < MAX_STEP && target > (1 + CONTOUR_HYSTERESIS) * midpointAboveM(k)) {
    k += 1;
  }
  while (k > 0 && target < (1 - CONTOUR_HYSTERESIS) * midpointAboveM(k - 1)) {
    k -= 1;
  }
  return contourStepM(k);
}

/**
 * A patch's contour phase, m (Design note 14): its origin's height h₀ as the slot record narrows it
 * to `f32`, modulo the index's period 5 CI, formed in `f64`, in [0, 5 CI). The fragment's contour
 * coordinate (h − h₀ + φ) ÷ CI then crosses a whole number at every contour level and a multiple of
 * five at every index level, h − h₀ formed patch-local in `f32`.
 *
 * @param originHeightM - The patch origin's height above the datum, h₀, m (`SlotUpload.originHeightM`).
 * @param intervalM - The contour interval, m, positive.
 * @throws RangeError for a height that is not finite or an interval that is not positive.
 */
export function contourPhase(originHeightM: number, intervalM: number): number {
  if (!Number.isFinite(originHeightM) || !(Number.isFinite(intervalM) && intervalM > 0)) {
    throw new RangeError(
      `no phase for an origin at ${String(originHeightM)} m and an interval of ${String(intervalM)} m`,
    );
  }
  const period = CONTOUR_INDEX_EVERY * intervalM;
  const h0 = Math.fround(originHeightM);
  const phase = h0 - period * Math.floor(h0 / period);
  return phase >= period ? 0 : phase;
}

/**
 * A contour family's opacity at a spacing of its lines, 0 to 1 (the guide's Views): 1 from
 * {@link CONTOUR_FADE_PX}'s 6 CSS px, falling linearly to 0 at its 3 px, each at the view's stroke
 * scale.
 *
 * @param spacingPx - The family's lines' spacing on the view, device px.
 * @param strokeScale - The device px drawn for each CSS px of a line (`ViewStrokes.strokeScale`).
 */
export function contourFade(spacingPx: number, strokeScale: number): number {
  const gone = CONTOUR_FADE_PX.gone * strokeScale;
  const full = CONTOUR_FADE_PX.full * strokeScale;
  return Math.min(1, Math.max(0, (spacingPx - gone) / (full - gone)));
}

/**
 * The coverage of a line of `widthPx` at `distancePx` from its centre, device px: clamp(w ÷ 2 + ½ −
 * d, 0, 1), as R02's lines take it (`lines.wgsl`), so that its coverage across it sums to w.
 */
export function lineCoverage(distancePx: number, widthPx: number): number {
  return Math.min(1, Math.max(0, widthPx / 2 + 0.5 - distancePx));
}

/**
 * The distance to the nearest whole number of a coordinate `u` that changes by `perPx` a device
 * pixel, px: |fract(u − ½) − ½| ÷ |∇u|.
 */
export function lineDistancePx(u: number, perPx: number): number {
  const f = u - 0.5 - Math.floor(u - 0.5);
  return Math.abs(f - 0.5) / Math.max(perPx, Number.MIN_VALUE);
}

/** A fragment's contour coverages: each family's, faded, and the coastline's. */
export interface ContourShade {
  /** The minor contours', faded, 0 where the nearest contour is an index one. */
  readonly minor: number;
  /** The index contours', faded. */
  readonly index: number;
  /** The coastline's; 0 with no sea. */
  readonly coastline: number;
}

/** A fragment's height above the sea level, for the coastline. */
export interface CoastInput {
  /** The fragment's height above the sea level, m. */
  readonly aboveSeaM: number;
  /** How far that height changes a device pixel, m (the length of its screen gradient). */
  readonly perPxM: number;
}

/**
 * A fragment's contours (Design note 14): `terrainWire.wgsl`'s `wire_contours`, its twin in `f64`.
 *
 * @param u - The contour coordinate (h − h₀ + φ) ÷ CI.
 * @param uPerPx - How far `u` changes a device pixel (the length of its screen gradient).
 * @param coast - The fragment's height above the sea, or `null` on a world with no sea.
 * @param strokeScale - The device px drawn for each CSS px of a line.
 */
export function contourShade(
  u: number,
  uPerPx: number,
  coast: CoastInput | null,
  strokeScale: number,
): ContourShade {
  const nearest = Math.round(u);
  const onIndex = nearest - CONTOUR_INDEX_EVERY * Math.floor(nearest / CONTOUR_INDEX_EVERY) === 0;
  const minorSpacingPx = 1 / Math.max(uPerPx, Number.MIN_VALUE);
  const minor = onIndex
    ? 0
    : lineCoverage(lineDistancePx(u, uPerPx), CONTOUR_PX.minor * strokeScale) *
      contourFade(minorSpacingPx, strokeScale);
  const index =
    lineCoverage(
      lineDistancePx(u / CONTOUR_INDEX_EVERY, uPerPx / CONTOUR_INDEX_EVERY),
      CONTOUR_PX.index * strokeScale,
    ) * contourFade(CONTOUR_INDEX_EVERY * minorSpacingPx, strokeScale);
  const coastline =
    coast === null
      ? 0
      : lineCoverage(
          Math.abs(coast.aboveSeaM) / Math.max(coast.perPxM, Number.MIN_VALUE),
          CONTOUR_PX.coastline * strokeScale,
        );
  return { minor, index, coastline };
}

/**
 * A patch's place for the graticule drawn on it (Design note 14), formed in `f64` from its origin
 * P₀, body-fixed: the geocentric latitude φ₀ and longitude λ₀ of P₀ ÷ |P₀|, as R02's graticule
 * takes them (`bodies.ts`' `unitAt`), each less its nearest line's, with those lines' indices and
 * the local frame's sines and cosines.
 */
export interface GraticulePatch {
  /** |P₀|, m. */
  readonly radiusM: number;
  /** φ₀ less its nearest parallel's latitude, rad, in [−step ÷ 2, step ÷ 2]; 0 with no graticule. */
  readonly latPhaseRad: number;
  /** λ₀ less its nearest meridian's longitude, rad; 0 with no graticule. */
  readonly lonPhaseRad: number;
  /** The nearest parallel's index, round(φ₀ ÷ step), 0 the equator; 0 with no graticule. */
  readonly latIndex: number;
  /** The nearest meridian's index, round(λ₀ ÷ step), 0 the prime meridian; 0 with no graticule. */
  readonly lonIndex: number;
  readonly sinLat: number;
  readonly cosLat: number;
  readonly sinLon: number;
  readonly cosLon: number;
}

/**
 * A patch's place for the graticule ({@link GraticulePatch}), from its origin.
 *
 * @param originM - The patch origin P₀, body-fixed, m (`ResidentPatch.originM`).
 * @param stepRad - The graticule's step, rad (R02's `graticuleStepDeg`), or 0 where none is drawn.
 */
export function graticulePatch(originM: Vec3, stepRad: number): GraticulePatch {
  const radiusM = Math.hypot(originM.x, originM.y, originM.z);
  const horizontal = Math.hypot(originM.x, originM.y);
  const latRad = Math.atan2(originM.z, horizontal);
  const lonRad = Math.atan2(originM.y, originM.x);
  const latIndex = stepRad > 0 ? Math.round(latRad / stepRad) : 0;
  const lonIndex = stepRad > 0 ? Math.round(lonRad / stepRad) : 0;
  return {
    radiusM,
    latPhaseRad: stepRad > 0 ? latRad - latIndex * stepRad : 0,
    lonPhaseRad: stepRad > 0 ? lonRad - lonIndex * stepRad : 0,
    latIndex,
    lonIndex,
    sinLat: radiusM > 0 ? originM.z / radiusM : 0,
    cosLat: radiusM > 0 ? horizontal / radiusM : 1,
    sinLon: horizontal > 0 ? originM.y / horizontal : 0,
    cosLon: horizontal > 0 ? originM.x / horizontal : 1,
  };
}

/** A point's latitude and longitude less its patch origin's, rad. */
export interface GraticuleOffsets {
  readonly dLatRad: number;
  readonly dLonRad: number;
}

/**
 * The geocentric latitude and longitude of P₀ + q less P₀'s (`terrainWire.wgsl`'s
 * `wire_graticule_offsets`, its twin in `f64`), from the offset q, body-fixed, m.
 *
 * @remarks
 * In the frame of east ê, north n̂ and up û at P₀, the point is (e, n, w) with e = q · ê,
 * n = q · n̂ and w = |P₀| + q · û. Its distance from the axis is B = √(C² + e²) with
 * C = w cos φ₀ − n sin φ₀, so Δλ = atan2(e, C); and with δ = B − C = e² ÷ (B + C), formed so,
 * Δφ = atan2(n − δ sin φ₀, w + δ cos φ₀), exactly φ − φ₀ with no difference of nearly equal numbers
 * (A cos φ₀ − C sin φ₀ = n for the axial component A). Past the pole's side of the axis (C ≤ 0, a
 * patch of a face's size or more) it takes φ − φ₀ directly.
 */
export function graticuleOffsetsRad(patch: GraticulePatch, offsetM: Vec3): GraticuleOffsets {
  const { sinLat, cosLat, sinLon, cosLon } = patch;
  const e = -sinLon * offsetM.x + cosLon * offsetM.y;
  const n = -sinLat * cosLon * offsetM.x - sinLat * sinLon * offsetM.y + cosLat * offsetM.z;
  const w =
    patch.radiusM + cosLat * cosLon * offsetM.x + cosLat * sinLon * offsetM.y + sinLat * offsetM.z;
  const c = w * cosLat - n * sinLat;
  const b = Math.hypot(c, e);
  const dLonRad = Math.atan2(e, c);
  if (!(c > 0)) {
    const lat = Math.atan2(w * sinLat + n * cosLat, b);
    return { dLatRad: lat - Math.atan2(sinLat, cosLat), dLonRad };
  }
  const delta = (e * e) / (b + c);
  return { dLatRad: Math.atan2(n - delta * sinLat, w + delta * cosLat), dLonRad };
}

/**
 * A camera's height above the ground at its nadir, m (Design note 14): its height above the datum
 * along the spheroid's normal (`datumGroundUnder`), less the ground's drawn height at that point,
 * `groundHeightAtM` at its parametric direction (R10.T16.c's `ResidentHeights.groundHeightAtM`:
 * the surveyed ground's, the datum on unsurveyed ground), or less nothing where none is drawn.
 *
 * @param cameraM - The camera from the body's centre, body-fixed, m.
 * @param groundHeightAtM - The drawn ground's height above the datum at a body-fixed unit
 *   direction, the spheroid's point M d, m, or `null` where none is drawn.
 */
export function heightAboveGroundM(
  cameraM: Vec3,
  figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">,
  groundHeightAtM: (direction: Vec3) => number | null,
): number {
  const under = datumGroundUnder(figure)(cameraM);
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const direction = {
    x: under.groundPointM.x / a,
    y: under.groundPointM.y / a,
    z: under.groundPointM.z / c,
  };
  return under.heightM - (groundHeightAtM(direction) ?? 0);
}

/**
 * A length of the contours' statement (the guide's Views, `CONTOUR INTERVAL`): in `m`, and from
 * 1 km in `km`, as a map's margin states it: `200 m`, `1 km`, `2.5 km`. Every interval is a 1-2-5
 * step and every index five of one, so the figures are exact, and the unit follows the interval's
 * own hysteresis.
 */
export function contourLengthReading(lengthM: number): string {
  return lengthM >= 1000 ? `${String(lengthM / 1000)} km` : `${String(lengthM)} m`;
}

/**
 * The width of the interval's field on the `CONTOUR INTERVAL` line, ch: its longest reading,
 * `10000 km`, so that `· INDEX` never moves as the interval steps (the guide's "Numbers": "a field
 * has a fixed width sized for its longest possible value"). The index ends the line with no field
 * (decision-r10-view-block-room §5).
 *
 * @remarks
 * A view hands a body over within its relief ÷ (4 px × the pixel's angle) (`handoverRangeM`): with
 * 40 km of relief (Mars has about 30 km, from Olympus Mons's summit to Hellas's floor), through the
 * narrowest field of view, 10°, on a 3,840 px canvas, about 220,000 km, where a twentieth of the
 * height is about 11,000 km and the interval 10,000 km (Design note 14). A longer reading widens
 * its field.
 */
export const CONTOUR_FIELD_CH = contourLengthReading(contourStepM(21)).length;

/**
 * The `CONTOUR INTERVAL` line's reading (the guide's Views and its `CONTOUR INTERVAL` and `INDEX`
 * rows): the interval, then its index after a middle dot, `200 m · INDEX 1 km`.
 */
export function contourIntervalReading(intervalM: number): string {
  return `${contourLengthReading(intervalM)} · INDEX ${contourLengthReading(
    CONTOUR_INDEX_EVERY * intervalM,
  )}`;
}

/**
 * The wireframe pass's terrain settings on `setting` (R10.T11): the setting's own, its vertex path
 * and normals, with a cache of {@link WIREFRAME_TERRAIN_SLOTS} wireframe slots, whose bytes the
 * wireframe's slot layout gives (its heights, offsets on their path and survey mask).
 */
export function wireframeTerrainSettings(setting: QualitySetting): TerrainSettings {
  const terrain = TERRAIN_SETTINGS[setting];
  const bytesPerSlot = terrainSlotFields(terrain, "wire").reduce(
    (sum, field) => sum + field.bytes,
    0,
  );
  return { ...terrain, cacheBytes: WIREFRAME_TERRAIN_SLOTS * bytesPerSlot };
}

/**
 * The `@group(2)` bindings of the wireframe's terrain: R05's vertex storage, the survey mask at the
 * lit material's binding (`terrainSurvey.wgsl`) and the view's patch phases.
 */
export const TERRAIN_WIRE_BINDINGS = {
  ...TERRAIN_BINDINGS,
  /** Each slot's survey mask (`slotLayout.ts`' `survey-mask`), as the lit material binds it. */
  surveyMask: 17,
  /** The view's phases, one record a drawn patch (`uniforms.ts`' `WirePhaseRecords`). */
  phases: 18,
} as const;

/** The console names of the two paths' wireframe terrain materials. */
const DISPLAY_NAMES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": "WIREFRAME TERRAIN OFFSETS",
  "face-differences": "WIREFRAME TERRAIN",
};

/**
 * The wireframe's terrain material of a vertex path (R10.T11): `frame.wgsl`, `patchVertex.wgsl`,
 * `terrainPatch.wgsl`, the survey's placement and discard (`terrainSurvey.wgsl`, which R10's lit
 * material shares), `terrainWire.wgsl` and the path's source, drawn on the lit material's mesh
 * into the canvas with depth, its colour the contours and graticule over black.
 */
export function terrainWireMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  const source =
    frameWgsl +
    preciseTrigWgsl +
    patchVertexWgsl +
    terrainPatchWgsl +
    terrainSurveyWgsl +
    terrainWireWgsl +
    PATH_SOURCES[path];
  const vertex = ["vertex"] as const;
  const storageBuffers: StorageBufferSpec[] = [
    ...terrainVertexStorage(path),
    {
      name: "surveyMask",
      binding: TERRAIN_WIRE_BINDINGS.surveyMask,
      stages: ["vertex", "fragment"],
    },
    { name: "wirePhases", binding: TERRAIN_WIRE_BINDINGS.phases, stages: vertex },
  ];
  return {
    name: `terrain-wire-${path}`,
    displayName: DISPLAY_NAMES[path],
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [
      { name: "bodyRotation", type: "mat4x4f" },
      { name: "atlas", type: "vec4f" },
      { name: "contour", type: "vec4f" },
      { name: "contourWidthsPx", type: "vec4f" },
      { name: "graticule", type: "vec4f" },
      { name: "muted", type: "vec4f" },
      { name: "text", type: "vec4f" },
      { name: "casing", type: "vec4f" },
      { name: "occluderSlopePx", type: "f32" },
    ],
    samplers: [],
    textures: [],
    cullMode: "back",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
    storageBuffers,
  };
}

/** Both paths' wireframe terrain materials, for the catalogue. */
export const TERRAIN_WIRE_MATERIALS: ReadonlyArray<WgslMaterialSpec> = [
  terrainWireMaterialSpec("baked-offsets"),
  terrainWireMaterialSpec("face-differences"),
];
