/**
 * The symbology over a photorealistic image (plan R07, T16.a; Design notes 16–17): R02's draw
 * list with every mark cased, drawn by R02's renderer as the frame's last canvas pass, over the
 * tone-mapped image.
 *
 * @remarks
 * Every mark over the image is stroked twice, a `--surface-0` casing of `CASING_PX` CSS px
 * on each side, drawn at the list's stroke scale ({@link casingDevicePx}), beneath its coloured stroke, never a blur or a
 * glow (the guide's "Outlines for symbology"), so that it carries its own dark surface over any
 * part of the image; the image is never dimmed under it. The wireframe style's list cases every
 * batch but the hull edges, whose own faces hide the stars behind them; over the image they are
 * cased like every mark, and the hull faces' slope term covers their cased width at every scale
 * (`occluderSlopePx`; the UX decisions, item 12; R07.T16.d). Until R11
 * draws them, rings stay R02's ellipses, cased, never omitted (Design note 17). Until hull art
 * exists, a craft is R02's hull outline, cased, on a `--surface-0` silhouette of its opaque faces,
 * so that nothing behind them shows through: the one exception to the image's never being dimmed
 * under symbology (Design notes 16 and 17; decision-r07-t16a, item 3; R07.T16.e). Its windows are
 * in no mesh, and hide nothing. The star sprites are left out: the image holds the stars. The
 * occluders stay, so that a mark behind a body or a hull stays hidden.
 *
 * A body the frame's plan hands over to its terrain with its survey's coverage mask has its datum
 * give way over the image as its reference surface does in the image (plan R10, R10.T16.f; R10's
 * Design note 12): its occluder sphere and its own lines, its limb and graticule, are drawn with
 * R10.T11's `occluderSphereSurveyed` and `linesSurveyed`, discarded over every cell the mask holds
 * surveyed ({@link surveyedDatumsOf}), so that no datum line is drawn over a surveyed valley and
 * the sphere hides no mark in one. There the graticule is not drawn, where the image draws the
 * terrain in the reference surface's place (main's ruling for R10.T16.f; a graticule on the
 * terrain's height in the image is not built).
 *
 * A body's datum is stroked inside its figure over the image (plan R10, R10.T16.g; main's ruling,
 * `decision-r08-approximate-plate.md` item 2): the list the overlay draws is built with
 * `DrawOptions.datumInside`, so that its limb's casing ends on the limb and no datum line or casing
 * covers the image beyond the body, an atmosphere's glow above the limb included; a body too small
 * to stroke inside has no datum over the image (`datumOverImage`). The wireframe centres its limb.
 *
 * A view's `GRID` (the owner's requests, 2026-10-31 and 2026-11-05), `OFF` as its photorealistic
 * style opens, leaves the image as the picture alone: the overlay keeps only the batches marked
 * {@link LineBatch.keptAtGridOff}, each craft's hull edges on its silhouette and a ring's two
 * edges, which stand for what the image does not draw yet (Design note 17), and the survey edge,
 * which tells unsurveyed ground from surveyed; every body's datum lines, orbit, ring tick,
 * predicted path, the line-of-sight mark and every symbol, reticle and chevron are left out, and
 * the occluders stay, so that a stand-in behind a body stays hidden (the UX ruling, 2026-11-05).
 * Where none of them is in the list, the view draws no symbology pass at all
 * ({@link overlayDrawsAt}).
 *
 * The marks a body's ground hides (`GeometrySettings`' `hiddenMarks` `culled`; perf-ground-gpu,
 * 2026-11-05): over a body handed over with a whole survey, whose terrain draws all the ground its
 * mask discards, the overlay leaves out its datum's lines, which give way at every pixel, and each
 * segment of a solid view-space mark that lies wholly behind the sphere inside all its ground
 * (`hiddenMarksCulled`), and says whether any mark that tests depth is left
 * (`depthObserved`): where none is, the symbology's pass draws none of the terrain's depth, whose
 * stride-1 clusters cost the pod's SwiftShader 15% of the GPU 4 km above the ground (1.5 million
 * triangles against the lit pass's 0.36 million), and the canvas is the same, bit for bit.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { dot, norm, type Vec3, vec3 } from "../../geometry/vec3";
import type { ColourTokens } from "../../spatial/paint";
import type { BodyFigure } from "../appearance/fromWire";
import type { DiscRecord } from "../bodies/discShading";
import { coverageOf, type MeshBodyPlan } from "../bodies/draw";
import { rotationColumns } from "../bodies/smoothMesh";
import type { Viewport } from "../camera/projection";
import type { FrameSubmission } from "../engine/types";
import { frustumOf } from "../terrain/cull";
import {
  casingDevicePx,
  type DrawCamera,
  type LineBatch,
  type OccluderMesh,
  type WireframeDrawList,
} from "../wireframe/drawList";
import type { SurveyedDatumFrame, WireframeRenderer } from "../wireframe/submit";
import { PHOTOREAL_PASS_LABELS } from "./passes";

/**
 * A batch as the photorealistic style strokes it: cased by at least `casingPx`, device px.
 */
function cased(line: LineBatch, casingPx: number): LineBatch {
  return line.casingWidthPx >= casingPx ? line : { ...line, casingWidthPx: casingPx };
}

/**
 * R02's draw list as the overlay draws it over the image: every line batch cased in its
 * `--surface-0` casing colour at `CASING_PX` times the list's stroke scale ({@link casingDevicePx}), the
 * hull edges included; every hull's opaque faces filled in `--surface-0`, its silhouette
 * (R07.T16.e); no star sprite; and, with the view's `GRID` at `OFF`, only the batches it keeps
 * there ({@link LineBatch.keptAtGridOff}).
 *
 * @remarks
 * Its list is built with the datum inside the figure (`DrawOptions.datumInside`; plan R10,
 * R10.T16.g), at the casing this cases its lines at, so that no datum line or casing covers the
 * image beyond a body.
 *
 * @param tokens - The tokens the list was built with, whose `--surface-0` its casings take: the
 *   silhouette is one surface with them (decision-r07-t16a, item 3).
 * @param grid - The view's `GRID`: `true` (`ON`) draws every batch over the image; `false`
 *   (`OFF`, as a view's photorealistic style opens) only the stand-ins and survey edges.
 */
export function overlayDrawList(
  list: WireframeDrawList,
  tokens: Pick<ColourTokens, "surface0">,
  grid = true,
): WireframeDrawList {
  const casingPx = casingDevicePx(list.strokeScale);
  const silhouette = (mesh: OccluderMesh): OccluderMesh => ({ ...mesh, fill: tokens.surface0 });
  return {
    ...list,
    occluderMeshes: list.occluderMeshes.map(silhouette),
    lines: list.lines
      .filter((line) => grid || line.keptAtGridOff === true)
      .map((line) => cased(line, casingPx)),
    sprites: [],
  };
}

/**
 * Whether a view's overlay draws anything over its photorealistic image, and so its symbology's
 * pass is drawn: at `GRID` `ON` always, as R07 built it; at `OFF` only where the list holds a batch
 * kept there ({@link LineBatch.keptAtGridOff}), so that the picture alone costs no pass (the
 * owner's request, 2026-11-05).
 */
export function overlayDrawsAt(list: Pick<WireframeDrawList, "lines">, grid: boolean): boolean {
  return grid || list.lines.some((line) => line.keptAtGridOff === true);
}

/**
 * A body behind whose ground the overlay's marks may lie hidden (`GeometrySettings`' `hiddenMarks`;
 * {@link overlayHidersOf}): one the frame's plan hands over to terrain with a mask that holds every
 * cell, which it is given only where its terrain draws all the ground the mask discards, so that its
 * figure, its limb and its datum's lines give way at every pixel, and its ground stands nearer than
 * every point behind its inner sphere.
 */
export interface OverlayHider {
  /** The body, whose occluder sphere in the list gives its centre. */
  readonly body: string;
  /**
   * R_occ, m: the sphere about its centre inside every surface its terrain can draw, its polar
   * radius and its lowest height (`terrain/cull.ts`' `horizonCone`).
   */
  readonly innerRadiusM: number;
}

/**
 * The stroke's reach beyond its half-width on each side that a hidden segment's test allows for,
 * device px: `lineStroke.wgsl`'s antialiasing fringe, 1 px, and 1 px more.
 */
const HIDDEN_STROKE_REACH_PX = 2;

/**
 * How much farther than the tangent length to the inner sphere, the farthest the ground stands along
 * a ray that meets the sphere, a hidden segment must be: its depth's push away (2^-16 of itself,
 * `terrainDepth.wgsl`'s), the stroke's rays off its segment's, which lie within its reach of them,
 * a fragment's depth, which ramps across the stroke's caps to a point of the segment up to its reach
 * beyond the part tested (`lineStroke.wgsl`), at least a third of the part's least distance for
 * any stroke under 8 px, and the `f32` of the list's points take far less.
 */
const HIDDEN_RANGE_FACTOR = 4;

/** The least margin a hidden segment's range clears its ground by, m. */
const HIDDEN_RANGE_MARGIN_M = 10_000;

/**
 * The margin taken off the inner sphere for the `f32` centre the list holds, as a share of its
 * distance: four of `f32`'s epsilons, with {@link CENTRE_MARGIN_M} beside it.
 */
const CENTRE_ROUNDING = 4 * 2 ** -24;

/** The margin taken off the inner sphere beside {@link CENTRE_ROUNDING}'s, m. */
const CENTRE_MARGIN_M = 1;

/** A sphere a segment may lie wholly behind, in the list's axes from the camera. */
interface HidingSphere {
  /** Its centre's direction. */
  readonly axis: Vec3;
  /** The sine and cosine of its cone's half-angle from the camera. */
  readonly sinHalf: number;
  readonly cosHalf: number;
  /** The tangent length from the camera, m: the farthest it is met along a ray. */
  readonly tangentM: number;
}

/** `hider`'s sphere from the camera, or `null` where the list holds no occluder for its body. */
function hidingSphereOf(list: WireframeDrawList, hider: OverlayHider): HidingSphere | null {
  const occluder = list.occluderSpheres.find((sphere) => sphere.id === hider.body);
  if (occluder === undefined) {
    return null;
  }
  const c = occluder.centreF32;
  const centre = vec3(c[0] ?? Number.NaN, c[1] ?? Number.NaN, c[2] ?? Number.NaN);
  const distanceM = norm(centre);
  const radiusM = hider.innerRadiusM - CENTRE_ROUNDING * distanceM - CENTRE_MARGIN_M;
  if (!(distanceM > radiusM && radiusM > 0)) {
    return null;
  }
  const sinHalf = radiusM / distanceM;
  return {
    axis: vec3(centre.x / distanceM, centre.y / distanceM, centre.z / distanceM),
    sinHalf,
    cosHalf: Math.sqrt(1 - sinHalf * sinHalf),
    tangentM: Math.sqrt(distanceM * distanceM - radiusM * radiusM),
  };
}

/** Whether `p` lies within `cosLimit` of the sphere's axis: inside a cone about it. */
function withinCone(p: Vec3, sphere: HidingSphere, cosLimit: number): boolean {
  return dot(p, sphere.axis) >= norm(p) * cosLimit;
}

/** The least distance from the camera of the segment from `a` to `b`, m. */
function nearestM(a: Vec3, b: Vec3): number {
  const d = vec3(b.x - a.x, b.y - a.y, b.z - a.z);
  const length2 = dot(d, d);
  const t = length2 > 0 ? Math.min(Math.max(-dot(a, d) / length2, 0), 1) : 0;
  return norm(vec3(a.x + t * d.x, a.y + t * d.y, a.z + t * d.z));
}

/**
 * The share of the segment from `a` to `b` inside every plane of `planes`, as [t0, t1] along it,
 * or `null` where none of it is (Cyrus and Beck's clip).
 */
function clipped(
  a: Vec3,
  b: Vec3,
  planes: ReadonlyArray<{ readonly normal: Vec3; readonly offsetM: number }>,
): readonly [number, number] | null {
  let t0 = 0;
  let t1 = 1;
  for (const { normal, offsetM } of planes) {
    const fa = dot(normal, a) + offsetM;
    const fb = dot(normal, b) + offsetM;
    if (fa < 0 && fb < 0) {
      return null;
    }
    if (fa < 0) {
      t0 = Math.max(t0, fa / (fa - fb));
    } else if (fb < 0) {
      t1 = Math.min(t1, fa / (fa - fb));
    }
    if (!(t0 <= t1)) {
      return null;
    }
  }
  return [t0, t1];
}

/** Segment `i` of `line`'s ends from the camera, m, its origin added as the stroke's vertex adds it. */
function segmentEnds(line: LineBatch, i: number): readonly [Vec3, Vec3] {
  const o = line.originF32;
  const s = line.segments;
  const end = (at: number): Vec3 =>
    vec3(
      (o[0] ?? 0) + (s[at] ?? Number.NaN),
      (o[1] ?? 0) + (s[at + 1] ?? Number.NaN),
      (o[2] ?? 0) + (s[at + 2] ?? Number.NaN),
    );
  return [end(6 * i), end(6 * i + 3)];
}

/** The angle a device pixel subtends at the view's centre, its widest, rad. */
function pixelRadOf(camera: DrawCamera, viewport: Viewport): number {
  return (2 * Math.tan(camera.fovXRad / 2)) / viewport.widthPx;
}

/** How far `line`'s stroke reaches beyond its segments, rad: its half-width, casing and fringe. */
function strokeReachRad(line: LineBatch, pixelRad: number): number {
  return (line.widthPx / 2 + line.casingWidthPx + HIDDEN_STROKE_REACH_PX) * pixelRad;
}

/** The view's frustum widened by `reachRad` on every side, in the list's axes. */
function widenedPlanes(
  camera: DrawCamera,
  viewport: Viewport,
  reachRad: number,
): ReadonlyArray<{ readonly normal: Vec3; readonly offsetM: number }> {
  return frustumOf(
    { orientation: camera.pose.orientation, fovXRad: camera.fovXRad, viewport },
    reachRad,
  ).planes;
}

/**
 * The overlay's list less the marks a body's ground hides (`GeometrySettings`' `hiddenMarks`
 * `culled`): the datum's lines of each of `hiders`, which its whole survey's mask discards at every
 * pixel, and each segment of a solid view-space batch whose part within the view, widened by its
 * stroke, lies wholly inside the cone from the camera tangent to a hider's inner sphere, narrowed
 * by the stroke's half-width, its casing and {@link HIDDEN_STROKE_REACH_PX}, and wholly beyond
 * {@link HIDDEN_RANGE_FACTOR} times the tangent length, so that every fragment of its stroke lies
 * behind ground its hider draws; a batch left with no segment is left out. A hider's datum lines go
 * only where the list holds its occluder sphere, without which they are drawn whole. A dashed batch, whose
 * dashes run on across its segments, and a batch on a body's ground (`groundCentreF32`) are kept
 * whole, as is every other draw.
 *
 * @remarks
 * Every pixel such a segment's stroke covers looks inside the cone at the inner sphere, which the
 * ray meets within the tangent length, and the ground, outside the sphere and drawn wherever its
 * mask discards the figure, stands nearer along it, so its every fragment failed the depth test.
 * Leaving it out changes no pixel; a segment out of the view's widened frustum draws none and is
 * kept. The view's rotation is the list's camera's, its axes the segments'.
 *
 * @param viewport - The canvas's size, px, which the overlay is drawn at.
 */
export function hiddenMarksCulled(
  list: WireframeDrawList,
  camera: DrawCamera,
  viewport: Viewport,
  hiders: ReadonlyArray<OverlayHider>,
): WireframeDrawList {
  const spheres = hiders.flatMap((hider) => {
    const sphere = hidingSphereOf(list, hider);
    return sphere === null ? [] : [sphere];
  });
  // A body's own lines give way over its mask only where the list holds its occluder sphere, by
  // which the packer finds its centre (`packWireframe`'s `surveyed`).
  const datums = new Set(
    hiders
      .filter((hider) => list.occluderSpheres.some((sphere) => sphere.id === hider.body))
      .map((hider) => hider.body),
  );
  if (spheres.length === 0 && datums.size === 0) {
    return list;
  }
  const pixelRad = pixelRadOf(camera, viewport);
  const lines: LineBatch[] = [];
  for (const line of list.lines) {
    if (line.body !== undefined && datums.has(line.body)) {
      continue;
    }
    if (
      line.space !== "view" ||
      line.dash !== null ||
      line.groundCentreF32 !== undefined ||
      spheres.length === 0
    ) {
      lines.push(line);
      continue;
    }
    const reachRad = strokeReachRad(line, pixelRad);
    const planes = widenedPlanes(camera, viewport, reachRad);
    // The cone's half-angle less the stroke's reach, α − δ, by its sine and cosine.
    const sinReach = Math.sin(reachRad);
    const cosReach = Math.cos(reachRad);
    // A segment out of the widened view draws nothing, and is kept as it is.
    const hidden = (i: number): boolean => {
      const [a, b] = segmentEnds(line, i);
      const part = clipped(a, b, planes);
      if (part === null) {
        return false;
      }
      const at = (t: number): Vec3 =>
        vec3(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y), a.z + t * (b.z - a.z));
      const p0 = at(part[0]);
      const p1 = at(part[1]);
      return spheres.some((sphere) => {
        const cosLimit = sphere.cosHalf * cosReach + sphere.sinHalf * sinReach;
        return (
          sphere.sinHalf * cosReach - sphere.cosHalf * sinReach > 0 &&
          withinCone(p0, sphere, cosLimit) &&
          withinCone(p1, sphere, cosLimit) &&
          nearestM(p0, p1) > HIDDEN_RANGE_FACTOR * sphere.tangentM + HIDDEN_RANGE_MARGIN_M
        );
      });
    };
    const s = line.segments;
    const count = s.length / 6;
    const kept: number[] = [];
    for (let i = 0; i < count; i += 1) {
      if (!hidden(i)) {
        kept.push(i);
      }
    }
    if (kept.length === count) {
      lines.push(line);
    } else if (kept.length > 0) {
      const segments = new Float32Array(kept.length * 6);
      kept.forEach((i, at) => {
        segments.set(s.subarray(6 * i, 6 * i + 6), 6 * at);
      });
      lines.push({ ...line, segments });
    }
  }
  return { ...list, lines };
}

/**
 * Whether any draw of the overlay's `list` tests depth where it draws over the image, so that the
 * terrain's depth beneath it can change a pixel: a view-space batch with a segment in the view's
 * frustum, widened by its stroke, or a hull's filled silhouette. The occluders write depth alone,
 * and the screen-space symbology tests none.
 *
 * @param viewport - The canvas's size, px.
 */
export function depthObserved(
  list: WireframeDrawList,
  camera: DrawCamera,
  viewport: Viewport,
): boolean {
  if (list.occluderMeshes.some((mesh) => mesh.fill !== null)) {
    return true;
  }
  const pixelRad = pixelRadOf(camera, viewport);
  return list.lines.some((line) => {
    if (line.space !== "view") {
      return false;
    }
    const planes = widenedPlanes(camera, viewport, strokeReachRad(line, pixelRad));
    for (let i = 0; i < line.segments.length / 6; i += 1) {
      const [a, b] = segmentEnds(line, i);
      if (clipped(a, b, planes) !== null) {
        return true;
      }
    }
    return false;
  });
}

/**
 * The symbology's canvas pass (Design note 8's "symbology cased over the result"): the overlay's
 * draw list packed and bound by R02's renderer, labelled `symbology` for the pass timer, loading
 * the tone-mapped image beneath it through the canvas's sRGB view. It draws the bodies' occluder
 * spheres, then the hulls' silhouettes, then the lines, after tone mapping, so that the meter and
 * the HDR scene are unchanged (R07.T16.e).
 *
 * @param tokens - The tokens the list was built with ({@link overlayDrawList}).
 * @param viewport - The canvas's size, px: the symbology is drawn at the display's resolution,
 *   never at the scene target's internal one.
 * @param surveyed - The bodies whose datum gives way over their surveyed ground, from the frame's
 *   plan ({@link surveyedDatumsOf}; plan R10, R10.T16.f): each one's occluder sphere and own lines
 *   packed as R10.T11's wireframe packs them over its terrain (`packWireframe`'s `surveyed`),
 *   binding its mask; an empty map draws every datum whole.
 * @param grid - The view's `GRID` ({@link overlayDrawList}): `ON` unless given.
 * @param hiders - The bodies whose ground hides the marks behind it (`GeometrySettings`'
 *   `hiddenMarks` `culled`; {@link hiddenMarksCulled}), none unless given; of them, those whose
 *   datum gives way in `surveyed` alone.
 */
export function overlaySubmission(
  renderer: Pick<WireframeRenderer, "frame">,
  list: WireframeDrawList,
  tokens: Pick<ColourTokens, "surface0">,
  camera: DrawCamera,
  viewport: Viewport,
  surveyed: ReadonlyMap<string, SurveyedDatumFrame>,
  grid = true,
  hiders: ReadonlyArray<OverlayHider> = [],
): OverlaySubmission {
  const drawn = overlayDrawList(list, tokens, grid);
  // Only a body whose datum gives way over its mask in this pass hides its marks.
  const hiding = hiders.filter((hider) => surveyed.has(hider.body));
  const shown = hiding.length === 0 ? drawn : hiddenMarksCulled(drawn, camera, viewport, hiding);
  return {
    // No terrain draws: the photorealistic renderer draws the terrain's depth before the pass.
    ...renderer.frame(shown, camera, viewport, [], {
      draws: [],
      surveyed,
    }),
    label: PHOTOREAL_PASS_LABELS.symbology,
    colourLoad: "load",
    depthTested: hiding.length === 0 || depthObserved(shown, camera, viewport),
  };
}

/**
 * The symbology's pass as {@link overlaySubmission} makes it, and whether any of its draws tests
 * the depth beneath it ({@link depthObserved}): where none does, the renderer draws none of the
 * terrain's depth before it. Always `true` without hiders, as before the lever.
 */
export interface OverlaySubmission extends FrameSubmission {
  readonly depthTested: boolean;
}

/** What {@link surveyedDatumsOf} reads of a frame's lit-body plan. */
export interface SurveyedPlan {
  /** The bodies drawn as meshes, each with its role (`MeshBodyPlan.role`). */
  readonly meshes: ReadonlyArray<Pick<MeshBodyPlan, "index" | "role">>;
  /** The records, by which a mesh names its body (`BodyFramePlan.discs`). */
  readonly discs: ReadonlyArray<Pick<DiscRecord, "body">>;
}

/** What {@link surveyedDatumsOf} reads of a lit body (`LitBodyInput`'s). */
export interface SurveyedBody {
  readonly id: string;
  /** Its figure, whose a ÷ c stretches the mask's direction along its pole. */
  readonly figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">;
}

/**
 * The bodies behind whose ground the overlay's marks may lie hidden (`GeometrySettings`'
 * `hiddenMarks` `culled`; {@link hiddenMarksCulled}), from the frame's plan: each it hands over to
 * terrain with a mask that holds every cell (`MeshBodyCoverage.whole`), which the plan gives only
 * where the terrain draws all the ground the mask discards, and for which `innerRadiusM` names the
 * sphere inside all its ground.
 *
 * @param innerRadiusM - Each body's R_occ, m (`FrameTerrain.innerRadiusM`), or `null` for none.
 */
export function overlayHidersOf(
  plan: SurveyedPlan,
  innerRadiusM: (body: BodyIdHex) => number | null,
): ReadonlyArray<OverlayHider> {
  const hiders: OverlayHider[] = [];
  for (const mesh of plan.meshes) {
    const body = plan.discs[mesh.index]?.body;
    const radiusM = body === undefined ? null : innerRadiusM(body);
    if (body !== undefined && radiusM !== null && coverageOf(mesh.role)?.whole === true) {
      hiders.push({ body, innerRadiusM: radiusM });
    }
  }
  return hiders;
}

/**
 * The datums that give way over a photorealistic frame's image (plan R10, R10.T16.f; R10's Design
 * note 12), from the frame's plan: each body it hands over to terrain with a coverage mask
 * (`MeshBodyRole` `handed-over`), read with the mask and in the rotation that discard its reference
 * surface (`coverageOf` its role, the figure's own), so that its datum gives way exactly where that
 * surface does; and its figure's equatorial radius over its polar, a ÷ c, by which the mask's direction is
 * stretched along its pole, as the figure's own discard stretches it (`smoothMesh.wgsl`). None for
 * a body promoted to a mesh, or handed over without a mask, whose figure is drawn whole.
 *
 * @param bodies - The lit bodies the plan was made from, by which each figure is found.
 */
export function surveyedDatumsOf(
  plan: SurveyedPlan,
  bodies: ReadonlyArray<SurveyedBody>,
): ReadonlyMap<string, SurveyedDatumFrame> {
  const figures = new Map(bodies.map((body) => [body.id, body.figure]));
  const surveyed = new Map<string, SurveyedDatumFrame>();
  for (const mesh of plan.meshes) {
    // The mask that discards its figure, or none where the figure is whole.
    const coverage = coverageOf(mesh.role);
    const body = plan.discs[mesh.index]?.body;
    const figure = body === undefined ? undefined : figures.get(body);
    if (coverage === null || body === undefined || figure === undefined) {
      continue;
    }
    surveyed.set(body, {
      mask: coverage.mask,
      axes: rotationColumns(coverage.rotation),
      stretch: figure.equatorialRadiusM / figure.polarRadiusM,
    });
  }
  return surveyed;
}
