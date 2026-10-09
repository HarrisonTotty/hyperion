/**
 * The `f64` twin of a frame's lit bodies in the scene target, and the oracle it is held to (plan
 * R07, T9; Design notes 2 and 3).
 *
 * @remarks
 * {@link compositeBodyFrame} draws a plan as the renderer's passes do, over black: the `bodies`
 * pass's smooth figures, each pixel a figure covers and its body covers wholly written with its
 * depth where it is nearest (reversed-Z, greater-equal), then the painter's sequence: each disc's
 * two draws at depth 0, so only where no figure drew, each mesh body's limb at its limb plane's
 * depth, premultiplied over what is beneath and kept by the depth test where a nearer figure drew,
 * and each host disc given, opaque on its own limb plane (R06's `disc.wgsl`, by its twin
 * {@link rasteriseHostDisc}), so hidden where a nearer figure drew. It keeps each body's share of
 * every pixel's light; a host disc takes the pixel from every body. Points are not drawn.
 *
 * {@link firstHitShares} is the truth: each pixel's rays, on a grid of `samples` a side, meet the
 * nearest of the bodies' analytic spheroids, in `f64`.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { cross, dot, type Vec3 } from "../../geometry/vec3";
import type { BodyFigure } from "../appearance/fromWire";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import type { Rgb } from "../photometry/toneCurve";
import { METER_CLASS, type MeterClass } from "../post/meter";
import { type HostDiscRecord, rasteriseHostDisc } from "../sky/disc";
import { type DiscPixel, type DiscRecord, rasteriseDisc, viewRay } from "./discShading";
import type { BodyFramePlan } from "./draw";
import { limbDepthAt, rasteriseSmoothMesh } from "./smoothMesh";

/** One pixel of the twin's frame. */
export interface FramePixel {
  readonly xPx: number;
  readonly yPx: number;
  /** Pre-exposed luminance per channel, over black. */
  readonly rgb: Rgb;
  /** The meter class left in alpha: the last opaque write's, `other` where none wrote. */
  readonly meterClass: MeterClass;
  /** Each body's share of the pixel's light: its covered share, less what is drawn over it. */
  readonly shares: ReadonlyMap<BodyIdHex, number>;
}

/**
 * The twin's frame, with every pixel that is mapped to a pixel the bodies or the host discs wrote.
 */
export interface BodyFrame {
  readonly pixels: ReadonlyMap<number, FramePixel>;
  /**
   * The pixels a body's figure should draw, wholly covered by the body, that no triangle of the
   * figure covers: holes, which the figure's tolerance rules out.
   */
  readonly holes: number;
}

/** A pixel's state as the passes leave it. */
interface Working {
  rgb: [number, number, number];
  depth: number;
  meterClass: MeterClass;
  shares: Map<BodyIdHex, number>;
}

/** An opaque write of a body's pixel, at `depth`: it takes the whole pixel. */
function write(w: Working, p: DiscPixel, body: BodyIdHex, depth: number): void {
  w.rgb = [p.rgb[0], p.rgb[1], p.rgb[2]];
  w.depth = depth;
  w.meterClass = p.meterClass ?? w.meterClass;
  w.shares = new Map([[body, 1]]);
}

/** A limb pixel premultiplied by its coverage over what is beneath, whose class it keeps. */
function blend(w: Working, p: DiscPixel, body: BodyIdHex): void {
  const keep = 1 - p.coverage;
  w.rgb = [p.rgb[0] + keep * w.rgb[0], p.rgb[1] + keep * w.rgb[1], p.rgb[2] + keep * w.rgb[2]];
  for (const [id, share] of w.shares) {
    w.shares.set(id, keep * share);
  }
  w.shares.set(body, (w.shares.get(body) ?? 0) + p.coverage);
}

/**
 * A plan's bodies as the renderer draws them into the scene target, in `f64`, over black.
 *
 * @param hosts - R06's host discs by star (`DiscDraw.record`), each drawn at its host's step in
 *   the plan's order, as `LitBodyRenderer.draws` places their draws; none by default.
 * @throws Error if the plan names a record it does not hold.
 */
export function compositeBodyFrame(
  plan: BodyFramePlan,
  camera: ProjectionCamera,
  viewport: Viewport,
  hosts: ReadonlyMap<number, ReadonlyArray<HostDiscRecord>> = new Map(),
): BodyFrame {
  const width = viewport.widthPx;
  const working = new Map<number, Working>();
  const at = (p: { readonly xPx: number; readonly yPx: number }): Working => {
    const index = p.yPx * width + p.xPx;
    let w = working.get(index);
    if (w === undefined) {
      w = { rgb: [0, 0, 0], depth: 0, meterClass: METER_CLASS.other, shares: new Map() };
      working.set(index, w);
    }
    return w;
  };
  const recordOf = (index: number): DiscRecord => {
    const record = plan.discs[index];
    if (record === undefined) {
      throw new Error(`the plan has no record ${index}`);
    }
    return record;
  };
  const rasterised = new Map<number, DiscPixel[]>();
  const pixelsOf = (index: number): DiscPixel[] => {
    let pixels = rasterised.get(index);
    if (pixels === undefined) {
      pixels = rasteriseDisc(recordOf(index), camera, viewport);
      rasterised.set(index, pixels);
    }
    return pixels;
  };
  let holes = 0;
  // The bodies pass: each figure's wholly covered pixels, nearest first by the depth test.
  for (const { index, mesh } of plan.meshes) {
    const raster = rasteriseSmoothMesh(mesh, camera, viewport, 1);
    for (const p of pixelsOf(index)) {
      if (p.draw !== "interior") {
        continue;
      }
      const depth = raster.depth[p.yPx * width + p.xPx] ?? -1;
      if (depth < 0) {
        holes += 1;
        continue;
      }
      const w = at(p);
      if (depth >= w.depth) {
        write(w, p, mesh.body, depth);
      }
    }
  }
  // The painter's sequence: discs at depth 0, limbs and host discs on their planes; none writes
  // depth.
  for (const step of plan.steps) {
    switch (step.kind) {
      case "disc": {
        const body = recordOf(step.index).body;
        for (const p of pixelsOf(step.index)) {
          const w = at(p);
          if (0 < w.depth) {
            continue;
          }
          if (p.draw === "interior") {
            write(w, p, body, w.depth);
          } else {
            blend(w, p, body);
          }
        }
        break;
      }
      case "limb": {
        const mesh = plan.meshes[step.mesh];
        if (mesh === undefined) {
          throw new Error(`the plan has no mesh ${step.mesh}`);
        }
        const record = recordOf(mesh.index);
        for (const p of pixelsOf(mesh.index)) {
          if (p.draw !== "limb") {
            continue;
          }
          const w = at(p);
          if (limbDepthAt(record.rect, mesh.limbDepths, p.xPx + 0.5, p.yPx + 0.5) >= w.depth) {
            blend(w, p, record.body);
          }
        }
        break;
      }
      case "host":
        for (const record of hosts.get(step.star) ?? []) {
          for (const p of rasteriseHostDisc(record, camera, viewport)) {
            const w = at(p);
            if (p.depth >= w.depth) {
              w.rgb = [p.rgb[0], p.rgb[1], p.rgb[2]];
              w.meterClass = METER_CLASS.hostDisc;
              w.shares = new Map();
            }
          }
        }
        break;
      case "points":
        break;
    }
  }
  const pixels = new Map<number, FramePixel>();
  for (const [index, w] of working) {
    pixels.set(index, {
      xPx: index % width,
      yPx: Math.floor(index / width),
      rgb: w.rgb,
      meterClass: w.meterClass,
      shares: w.shares,
    });
  }
  return { pixels, holes };
}

/** A body as the oracle sees it: its spheroid about its pole. */
export interface OracleBody {
  readonly id: BodyIdHex;
  /** Its centre from the camera, m along the galactic axes. */
  readonly centreM: Vec3;
  readonly figure: BodyFigure;
  /** The pole it is drawn about, unit. */
  readonly pole: Vec3;
}

/** x stretched along the unit `pole` by `factor`. */
function stretchAlong(pole: Vec3, factor: number, x: Vec3): Vec3 {
  const along = (factor - 1) * dot(x, pole);
  return { x: x.x + along * pole.x, y: x.y + along * pole.y, z: x.z + along * pole.z };
}

/**
 * The distance along the unit ray `ray` from the camera to its first hit on a body's spheroid, m,
 * or `null` for a miss: in the space stretched along the pole by a ÷ c, where it is a sphere of
 * radius a, by cross products, as the shader's `hit_spheroid` keeps a small body's limb.
 */
function firstHitM(body: OracleBody, ray: Vec3): number | null {
  const { equatorialRadiusM: a, polarRadiusM: c } = body.figure;
  const r = stretchAlong(body.pole, a / c, ray);
  const centre = stretchAlong(body.pole, a / c, body.centreM);
  const rr = dot(r, r);
  const across = cross(r, centre);
  const discriminant = rr * a * a - dot(across, across);
  if (discriminant < 0) {
    return null;
  }
  const t = (dot(r, centre) - Math.sqrt(discriminant)) / rr;
  return t > 0 ? t : null;
}

/**
 * Each pixel's share of each body by first hits: `samples` × `samples` rays a pixel, each taken by
 * the nearest spheroid it meets, over the pixels of `region` (each a pixel index, row by row).
 */
export function firstHitShares(
  bodies: ReadonlyArray<OracleBody>,
  camera: ProjectionCamera,
  viewport: Viewport,
  samples: number,
  region: Iterable<number>,
): Map<number, Map<BodyIdHex, number>> {
  const shares = new Map<number, Map<BodyIdHex, number>>();
  const weight = 1 / (samples * samples);
  for (const index of region) {
    const x0 = index % viewport.widthPx;
    const y0 = Math.floor(index / viewport.widthPx);
    const pixel = new Map<BodyIdHex, number>();
    for (let j = 0; j < samples; j += 1) {
      for (let i = 0; i < samples; i += 1) {
        const ray = rotate(
          camera.orientation,
          viewRay(x0 + (i + 0.5) / samples, y0 + (j + 0.5) / samples, camera, viewport),
        );
        let nearest: { readonly id: BodyIdHex; readonly t: number } | null = null;
        for (const body of bodies) {
          const t = firstHitM(body, ray);
          if (t !== null && (nearest === null || t < nearest.t)) {
            nearest = { id: body.id, t };
          }
        }
        if (nearest !== null) {
          pixel.set(nearest.id, (pixel.get(nearest.id) ?? 0) + weight);
        }
      }
    }
    shares.set(index, pixel);
  }
  return shares;
}
