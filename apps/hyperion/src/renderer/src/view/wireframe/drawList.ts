import { norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { ColourToken } from "../../spatial/drawList";
import type { ColourTokens } from "../../spatial/paint";
import { SYMBOL_STROKE_PX } from "../../spatial/symbols";
import type { CameraPose } from "../camera/pose";
import {
  pixelSolidAngle,
  type ProjectionCamera,
  project,
  type Viewport,
} from "../camera/projection";
import { type CameraTarget, targetPosition } from "../camera/state";
import { narrow } from "../coords/narrow";
import { differenceM, type ViewPosition } from "../coords/position";
import { originMinusCamera, relativeToCamera } from "../coords/relative";
import { occluderRadius } from "../depth/depth";
import { exposureScale } from "../photometry/exposure";
import { apparentV, illuminanceLx, PSF_QUAD_PX } from "../photometry/magnitude";
import { starColour } from "../photometry/starColour";
import type { Rgb } from "../photometry/toneCurve";
import { cameraSceneOf, sceneOrigins, type ViewBody, type ViewScene } from "../scene/model";
import { angularDiameterPx, graticule, ringEllipse } from "./bodies";
import { behindLimb, sphereInFrustum } from "./cull";
import type { Polyline } from "./curve";
import { hullEdges, hullFaces } from "./hulls";
import { orbitPath } from "./orbits";
import { type ScreenPx, type SymbologyAnchor, symbologyMarks } from "./symbology";

/**
 * The width of the `--surface-0` casing on each side of every stroke, px: 1, the guide's casing
 * over a raster, which every mark over the image takes (the guide's drafted "Outlines for
 * symbology", R02.T2.b item 3; Design note 9).
 */
export const CASING_PX = 1;

/**
 * A stroke's widths, px: the thin reference lines (the guide's 1 px orbit), a step heavier (the
 * equator, the prime meridian and hull edges: 1.5 px, a choice of this plan, the symbols' own
 * stroke), and the selected orbit's 2 px (the guide's).
 */
export const STROKE_PX = { thin: 1, heavy: 1.5, selected: 2 } as const;

/**
 * The dash of a predicted path, px on and off, in screen space so that it does not crawl: 6 and 4,
 * a choice of this plan.
 */
export const PREDICTED_DASH_PX = { onPx: 6, offPx: 4 } as const;

/** The most star sprites the low setting draws, the brightest by flux (Design note 21). */
export const LOW_SETTING_MAX_SPRITES = 2_000;

/**
 * The depth bias of a hull's occluder faces, pushing them away from the camera (Design note 5):
 * positive meaning away, mapped once by R01's adapter.
 */
export const HULL_OCCLUDER_BIAS = { constant: 128, slopeScale: 2 } as const;

/** A batch of strokes of one width, colour and dash. */
export interface LineBatch {
  /** The batch's stable name, the same from frame to frame. */
  readonly id: string;
  /**
   * `view`: segments are `f32` metres from `originF32` (itself from the camera) along the camera
   * frame's axes, drawn through the view rotation and projection and depth-tested against the
   * occluders; `screen`: the symbology, segments in pixels from the view's top left with z 0,
   * placed after the CPU's horizon test, drawn with no view matrix and no depth test.
   */
  readonly space: "view" | "screen";
  /** The batch's origin from the camera, `f32` m: zero but for a hull's (Design note 2). */
  readonly originF32: Float32Array;
  /** The segments, six `f32` each: one end's x, y, z, then the other's. */
  readonly segments: Float32Array;
  /** The stroke's colour token (plan 05's `readTokens` names). */
  readonly token: ColourToken;
  /** The token's colour, as `readTokens` read it. */
  readonly colour: string;
  /** The stroke's width, px. */
  readonly widthPx: number;
  /** The casing's width on each side, px, drawn beneath the stroke in {@link LineBatch.casingColour}. */
  readonly casingWidthPx: number;
  /** The casing's colour: `--surface-0`. */
  readonly casingColour: string;
  /** The dash, for predicted paths only, or `null` for a solid stroke. */
  readonly dash: { readonly onPx: number; readonly offPx: number } | null;
}

/** A body's depth-only occluder sphere (Design note 5). */
export interface OccluderSphere {
  /** The body. */
  readonly id: string;
  /** Its centre from the camera, `f32` m. */
  readonly centreF32: Float32Array;
  /** Its radius, m: `occluderRadius`, a little inside the body. */
  readonly radiusM: number;
}

/** A hull's depth-only faces, two-sided, pushed away by the occluder pass's bias. */
export interface OccluderMesh {
  /** The craft. */
  readonly id: string;
  /** The craft's reference point from the camera, `f32` m. */
  readonly originF32: Float32Array;
  /** The triangles' corners, nine `f32` per triangle, m from the origin. */
  readonly triangles: Float32Array;
  /** The pass's depth bias, positive meaning away from the camera. */
  readonly depthBiasAway: typeof HULL_OCCLUDER_BIAS;
  /** Drawn two-sided, so that a winding flip cannot unhide every hidden line. */
  readonly twoSided: true;
}

/** A star as a sprite: where it falls on the view and its pre-exposed colour. */
export interface StarSprite {
  /** The star's system. */
  readonly id: string;
  /** Its direction from the camera, unit, `f32`. */
  readonly directionF32: Float32Array;
  /** Where it falls, px from the view's top left, with its sub-pixel part. */
  readonly xPx: number;
  /** Where it falls, px from the top. */
  readonly yPx: number;
  /**
   * Its pre-exposed linear Rec. 709 colour per unit of point-spread weight: E ÷ Ω × the exposure
   * scale × its unit-luminance colour; the sprite multiplies by each pixel's weight, then tones.
   */
  readonly exposedRgb: Rgb;
  /** Its illuminance, lx, by which the low setting keeps the brightest. */
  readonly illuminanceLx: number;
}

/** Where a pickable mark landed on the view, for the pick and the DOM labels (T15). */
export interface DrawAnchor {
  /** What the mark is. */
  readonly target: CameraTarget;
  /** Its position, px. */
  readonly xPx: number;
  /** Its position, px. */
  readonly yPx: number;
  /** Its distance from the camera, m. */
  readonly distanceM: number;
}

/**
 * Everything the wireframe style draws in one frame, engine-agnostic and `f32` camera-relative, in
 * the order it is drawn: opaque occluders, lines, sprites (plan R02, R02.T13).
 */
export interface WireframeDrawList {
  /** The bodies' occluder spheres. */
  readonly occluderSpheres: ReadonlyArray<OccluderSphere>;
  /** The hulls' occluder faces. */
  readonly occluderMeshes: ReadonlyArray<OccluderMesh>;
  /** The line batches, view-space first, then screen-space symbology. */
  readonly lines: ReadonlyArray<LineBatch>;
  /** The star sprites. */
  readonly sprites: ReadonlyArray<StarSprite>;
  /** The pickable marks. */
  readonly anchors: ReadonlyArray<DrawAnchor>;
}

/** The camera the list is built for: its pose and horizontal field of view. */
export interface DrawCamera {
  /** The pose to draw from. */
  readonly pose: CameraPose;
  /** The horizontal field of view, rad. */
  readonly fovXRad: number;
}

/** What the list draws beyond the scene itself. */
export interface DrawOptions {
  /** The wireframe's low setting (Design note 21): graticules at 30° only, 2,000 sprites. */
  readonly lowSetting: boolean;
  /** The exposure, EV100. */
  readonly ev100: number;
  /** The selected target, bracketed, whose orbit is drawn heavier; or `null`. */
  readonly selection: CameraTarget | null;
  /** The commanded destination, with its `--target` reticle; or `null`. */
  readonly destination: CameraTarget | null;
  /** The interface's rem, px, which symbol sizes follow. */
  readonly remPx: number;
}

function sameTarget(a: CameraTarget | null, b: CameraTarget): boolean {
  if (a === null) {
    return false;
  }
  let same: boolean;
  switch (a.kind) {
    case "body":
      same = b.kind === "body" && a.body === b.body;
      break;
    case "craft":
      same = b.kind === "craft" && a.craft === b.craft;
      break;
  }
  return same;
}

/** Packs polylines into segments, narrowing each point once. */
function packPolylines(polylines: ReadonlyArray<Polyline>): Float32Array {
  const count = polylines.reduce((n, line) => n + Math.max(0, line.length - 1), 0);
  const out = new Float32Array(count * 6);
  let at = 0;
  for (const line of polylines) {
    const narrowed = line.map((p) => narrow(p));
    for (let i = 1; i < narrowed.length; i += 1) {
      const a = narrowed[i - 1];
      const b = narrowed[i];
      if (a !== undefined && b !== undefined) {
        out.set(a, at);
        out.set(b, at + 3);
        at += 6;
      }
    }
  }
  return out;
}

/** Packs screen segments, z 0. */
function packScreen(segments: ReadonlyArray<readonly [ScreenPx, ScreenPx]>): Float32Array {
  const out = new Float32Array(segments.length * 6);
  segments.forEach(([a, b], i) => {
    out.set([a.xPx, a.yPx, 0, b.xPx, b.yPx, 0], i * 6);
  });
  return out;
}

/**
 * Builds the wireframe style's draw list for one frame (plan R02, R02.T13).
 *
 * @remarks
 * Bodies are culled by the frustum in `f64` (Design note 8) and drawn by their regime (Design note
 * 13): occluder spheres of `occluderRadius`, limb and graticule lines, and below 3 px their symbol;
 * rings are their ellipses; orbits solid `--text-muted` at 1 px, the selected one's `--text` at
 * 2 px; hulls their edges over their two-sided, biased occluder faces; a craft's predicted path the
 * only dashed batch; the selection's bracket reticle in `--accent`, the destination's in
 * `--target` and the own ship's flight path marker; stars as sprites pre-exposed at the exposure,
 * the brightest 2,000 at the low setting. Every stroke is cased in `--surface-0`, since every mark
 * may lie over a star. Colours come from `tokens` (plan 05's `readTokens`), never literals. Every
 * position is differenced in `f64` and narrowed once. The list is a function of its arguments alone.
 */
export function buildWireframeDrawList(
  scene: ViewScene,
  camera: DrawCamera,
  viewport: Viewport,
  tokens: ColourTokens,
  options: DrawOptions,
): WireframeDrawList {
  const origins = sceneOrigins(scene);
  const projection: ProjectionCamera = {
    orientation: camera.pose.orientation,
    fovXRad: camera.fovXRad,
  };
  const fromCamera = (p: ViewPosition): Vec3 => relativeToCamera(p, camera.pose, origins);
  const bodyCentre = (body: ViewBody): Vec3 =>
    fromCamera({ kind: "body", body: body.id, m: vec3(0, 0, 0) });
  const batch = (
    id: string,
    space: LineBatch["space"],
    segments: Float32Array,
    token: ColourToken,
    widthPx: number,
    dash: LineBatch["dash"] = null,
    originF32: Float32Array = new Float32Array(3),
    casingWidthPx: number = CASING_PX,
  ): LineBatch => ({
    id,
    space,
    originF32,
    segments,
    token,
    colour: tokens[token],
    widthPx,
    casingWidthPx,
    casingColour: tokens.surface0,
    dash,
  });

  const occluderSpheres: OccluderSphere[] = [];
  const lines: LineBatch[] = [];
  const anchors: DrawAnchor[] = [];
  const limbs: { id: string; centreM: Vec3; radiusM: number }[] = [];
  // Whether a mark is in sight: not behind the limb of a body drawn as a sphere, its own excepted.
  const shown = (pointM: Vec3, own: string | null): boolean =>
    !limbs.some((body) => body.id !== own && behindLimb(pointM, body.centreM, body.radiusM));

  for (const body of scene.bodies) {
    const centreM = bodyCentre(body);
    if (!sphereInFrustum(centreM, body.radiusM, projection, viewport)) {
      continue;
    }
    const wireframe = graticule(
      {
        centreM,
        radiusM: body.radiusM,
        rotation: body.rotation,
        unmodelledPole: body.orbitNormal,
      },
      projection,
      viewport,
      options.lowSetting,
    );
    if (wireframe.regime !== "symbol") {
      limbs.push({ id: body.id, centreM, radiusM: body.radiusM });
      occluderSpheres.push({
        id: body.id,
        centreF32: narrow(centreM),
        radiusM: occluderRadius(body.radiusM, norm(centreM)),
      });
    }
    const major = wireframe.lines.filter((line) => line.major).flatMap((line) => line.runs);
    const minor = wireframe.lines.filter((line) => !line.major).flatMap((line) => line.runs);
    if (major.length > 0) {
      lines.push(
        batch(`body:${body.id}:major`, "view", packPolylines(major), "textMuted", STROKE_PX.heavy),
      );
    }
    if (minor.length > 0) {
      lines.push(
        batch(
          `body:${body.id}:graticule`,
          "view",
          packPolylines(minor),
          "textMuted",
          STROKE_PX.thin,
        ),
      );
    }
  }

  for (const ring of scene.rings) {
    const centreM = fromCamera({ kind: "body", body: ring.body, m: vec3(0, 0, 0) });
    if (!sphereInFrustum(centreM, ring.outerRadiusM, projection, viewport)) {
      continue;
    }
    const selected = sameTarget(options.selection, { kind: "body", body: ring.body });
    lines.push(
      batch(
        `ring:${ring.body}`,
        "view",
        packPolylines(ringEllipse({ ...ring, centreM }, projection, viewport)),
        selected ? "text" : "textMuted",
        selected ? STROKE_PX.selected : STROKE_PX.thin,
      ),
    );
  }

  for (const orbit of scene.orbits) {
    const parentM =
      orbit.parent === null
        ? fromCamera({ kind: "system", system: scene.system, m: vec3(0, 0, 0) })
        : fromCamera({ kind: "body", body: orbit.parent, m: vec3(0, 0, 0) });
    const extentM = orbit.orbit.semiMajorAxisM * (1 + orbit.orbit.eccentricity);
    if (!sphereInFrustum(parentM, extentM, projection, viewport)) {
      continue;
    }
    const selected = sameTarget(options.selection, { kind: "body", body: orbit.body });
    lines.push(
      batch(
        `orbit:${orbit.body}`,
        "view",
        packPolylines(orbitPath(orbit.orbit, parentM, projection, viewport)),
        selected ? "text" : "textMuted",
        selected ? STROKE_PX.selected : STROKE_PX.thin,
      ),
    );
  }

  const occluderMeshes: OccluderMesh[] = [];
  for (const craft of scene.craft) {
    const originM = fromCamera(craft.pose.position);
    if (!sphereInFrustum(originM, craft.hull.lengthM, projection, viewport)) {
      continue;
    }
    const originF32 = originMinusCamera(
      { origin: craft.pose.position, offsetsF32: new Float32Array(0) },
      camera.pose,
      origins,
    );
    const edges = hullEdges(craft.hull, craft.pose.attitude);
    lines.push(
      batch(
        `hull:${craft.id}`,
        "view",
        packPolylines(edges.map(([a, b]) => [a, b])),
        "text",
        STROKE_PX.heavy,
        null,
        originF32,
        // Uncased: a casing would widen the stroke past the 2 px the hull occluder's slope bias
        // covers (Design note 5), and the hull's own faces hide the stars behind it.
        0,
      ),
    );
    const faces = hullFaces(craft.hull, craft.pose.attitude);
    const triangles = new Float32Array(faces.length * 9);
    faces.forEach((corners, i) => {
      corners.forEach((corner, j) => {
        triangles.set(narrow(corner), i * 9 + j * 3);
      });
    });
    occluderMeshes.push({
      id: craft.id,
      originF32,
      triangles,
      depthBiasAway: HULL_OCCLUDER_BIAS,
      twoSided: true,
    });
  }

  for (const craft of scene.craft) {
    if (craft.predictedPath === null || craft.predictedPath.length < 2) {
      continue;
    }
    const path = craft.predictedPath.map((pose) => fromCamera(pose.position));
    lines.push(
      batch(
        `predicted:${craft.id}`,
        "view",
        packPolylines([path]),
        "text",
        STROKE_PX.thin,
        PREDICTED_DASH_PX,
      ),
    );
  }

  // Symbology: body symbols, anchors, reticles and the flight path marker, in screen space.
  const markAt = (pointM: Vec3): ScreenPx | null => {
    const p = project(pointM, projection, viewport);
    return p.inFront ? { xPx: p.xPx, yPx: p.yPx } : null;
  };
  const own = scene.craft.find((craft) => craft.id === scene.ownShip);
  const symbologyAnchors: SymbologyAnchor[] = [];
  for (const target of cameraSceneOf(scene).targets) {
    const pointM = fromCamera(targetPosition(target, origins));
    const at = markAt(pointM);
    const body =
      target.kind === "body" ? scene.bodies.find((b) => b.id === target.body) : undefined;
    if (at === null || !shown(pointM, body?.id ?? null)) {
      continue;
    }
    anchors.push({ target, xPx: at.xPx, yPx: at.yPx, distanceM: norm(pointM) });
    const craft =
      target.kind === "craft" ? scene.craft.find((c) => c.id === target.craft) : undefined;
    symbologyAnchors.push({
      target,
      at,
      craft:
        craft === undefined
          ? null
          : {
              // From the own ship where there is one, else from the camera.
              relativeM:
                own === undefined
                  ? pointM
                  : differenceM(craft.pose.position, own.pose.position, origins),
              relativeVelocityMPerS:
                own === undefined || own.velocityMPerS === null || craft.velocityMPerS === null
                  ? null
                  : sub(craft.velocityMPerS, own.velocityMPerS),
            },
      body:
        body === undefined
          ? null
          : {
              symbol: body.symbol,
              diameterPx: angularDiameterPx(pointM, body.radiusM, projection, viewport),
            },
    });
  }
  const marks = symbologyMarks(
    {
      anchors: symbologyAnchors,
      selection: options.selection,
      destination: options.destination,
      ownVelocityMPerS: own?.velocityMPerS ?? null,
      remPx: options.remPx,
    },
    projection,
    viewport,
  );
  for (const [i, mark] of marks.entries()) {
    lines.push(
      batch(
        `mark:${mark.kind}:${String(i)}`,
        "screen",
        packScreen(mark.segments),
        mark.token,
        SYMBOL_STROKE_PX,
      ),
    );
  }

  return {
    occluderSpheres,
    occluderMeshes,
    lines,
    sprites: starSprites(scene, projection, viewport, options),
    anchors,
  };
}

/** How far outside the view a sprite's star may fall and still light it: half its quad, px. */
const SPRITE_MARGIN_PX = Math.ceil(PSF_QUAD_PX / 2);

/** The scene's stars as sprites, the brightest first, capped at the low setting. */
function starSprites(
  scene: ViewScene,
  projection: ProjectionCamera,
  viewport: Viewport,
  options: DrawOptions,
): StarSprite[] {
  const exposure = exposureScale(options.ev100);
  const sprites: StarSprite[] = [];
  for (const star of scene.stars) {
    // Stars are far enough that the direction from the barycentre is the direction from the camera
    // (parallax across a system is under a tenth of a pixel beyond about 9 ly; Design note 19).
    const direction = star.direction;
    const p = project(scale(direction, 1e3), projection, viewport);
    if (
      !p.inFront ||
      p.xPx < -SPRITE_MARGIN_PX ||
      p.yPx < -SPRITE_MARGIN_PX ||
      p.xPx > viewport.widthPx + SPRITE_MARGIN_PX ||
      p.yPx > viewport.heightPx + SPRITE_MARGIN_PX
    ) {
      continue;
    }
    const e = illuminanceLx(apparentV(star.absoluteV, star.distanceM));
    const perWeight = (e / pixelSolidAngle(direction, projection, viewport)) * exposure;
    const colour = starColour(star.tEffK);
    sprites.push({
      id: star.id,
      directionF32: narrow(direction),
      xPx: p.xPx,
      yPx: p.yPx,
      exposedRgb: [colour[0] * perWeight, colour[1] * perWeight, colour[2] * perWeight],
      illuminanceLx: e,
    });
  }
  const ranked = sprites.toSorted(
    (a, b) => b.illuminanceLx - a.illuminanceLx || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0),
  );
  return options.lowSetting ? ranked.slice(0, LOW_SETTING_MAX_SPRITES) : ranked;
}
