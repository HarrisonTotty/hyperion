/**
 * The symbology over a photorealistic image (plan R07, T16.a; Design notes 16–17): R02's draw
 * list with every mark cased, drawn by R02's renderer as the frame's last canvas pass, over the
 * tone-mapped image.
 *
 * @remarks
 * Every mark over the image is stroked twice, a `--surface-0` casing of {@link CASING_PX} CSS px
 * on each side, drawn at the list's stroke scale, beneath its coloured stroke, never a blur or a
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
 */
import type { ColourTokens } from "../../spatial/paint";
import type { BodyFigure } from "../appearance/fromWire";
import type { DiscRecord } from "../bodies/discShading";
import { coverageOf, type MeshBodyPlan } from "../bodies/draw";
import { rotationColumns } from "../bodies/smoothMesh";
import type { Viewport } from "../camera/projection";
import type { FrameSubmission } from "../engine/types";
import {
  CASING_PX,
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
 * `--surface-0` casing colour at {@link CASING_PX} times the list's stroke scale, the hull edges
 * included; every hull's opaque faces filled in `--surface-0`, its silhouette (R07.T16.e); and no
 * star sprite.
 *
 * @param tokens - The tokens the list was built with, whose `--surface-0` its casings take: the
 *   silhouette is one surface with them (decision-r07-t16a, item 3).
 */
export function overlayDrawList(
  list: WireframeDrawList,
  tokens: Pick<ColourTokens, "surface0">,
): WireframeDrawList {
  const casingPx = CASING_PX * list.strokeScale;
  const silhouette = (mesh: OccluderMesh): OccluderMesh => ({ ...mesh, fill: tokens.surface0 });
  return {
    ...list,
    occluderMeshes: list.occluderMeshes.map(silhouette),
    lines: list.lines.map((line) => cased(line, casingPx)),
    sprites: [],
  };
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
 */
export function overlaySubmission(
  renderer: Pick<WireframeRenderer, "frame">,
  list: WireframeDrawList,
  tokens: Pick<ColourTokens, "surface0">,
  camera: DrawCamera,
  viewport: Viewport,
  surveyed: ReadonlyMap<string, SurveyedDatumFrame>,
): FrameSubmission {
  return {
    // No terrain draws: the photorealistic renderer draws the terrain's depth before the pass.
    ...renderer.frame(overlayDrawList(list, tokens), camera, viewport, [], { draws: [], surveyed }),
    label: PHOTOREAL_PASS_LABELS.symbology,
    colourLoad: "load",
  };
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
