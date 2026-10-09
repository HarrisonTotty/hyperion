import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { aHostDisc } from "../../test/litFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import {
  NEAR_PLANE_M,
  pixelSolidAngle,
  type ProjectionCamera,
  project,
  toViewAxes,
  type Viewport,
} from "../camera/projection";
import { IDENTITY_QUATERNION, lookAlong, rotate } from "../camera/quaternion";
import { rotateToBody } from "../coords/rotation";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import { AU_M } from "../scenes/kept";
import { vertexDir } from "../terrain/cube";
import { INSTANCE_RECORD_BYTES, patchTerms, SLOT_RECORD_BYTES } from "../terrain/gpu/uniforms";
import { faceDifferencePositionF32, type SlotTermsF32 } from "../terrain/gpu/vertexEmulation";
import type { BodyFigure } from "../terrain/planet";
import { compositeDiscPixels, type DiscRecord, rasteriseDisc, viewRay } from "./discShading";
import { type BodyFrameOptions, type LitBodyInput, type MeshBodyPlan, planLitBodies } from "./draw";
import { compositeBodyFrame } from "./frameTwin";
import { sphereFootprint } from "./regime";
import {
  limbDepthAt,
  MAX_SMOOTH_MESH_PATCHES,
  packSmoothMeshes,
  poleAxes,
  rasteriseSmoothMesh,
  SMOOTH_MESH_TAU_PX,
  smoothMeshOf,
  smoothMeshTauPx,
  smoothPatchVertices,
} from "./smoothMesh";

const RAD = Math.PI / 180;
const EXPOSURE = 1e-3;
const EARTH: BodyFigure = { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6, pole: null };
const WGS84: BodyFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_356_752.314_245,
  pole: vec3(0, 0, 1),
};
const VIEW: Viewport = { widthPx: 256, heightPx: 144 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };

/** The centre pixel's scale of a view, px per radian. */
function pxPerRad(camera: ProjectionCamera, viewport: Viewport): number {
  return viewport.widthPx / (2 * Math.tan(camera.fovXRad / 2));
}

/** A body `diameterPx` across at view position (`xPx`, `yPx`) of a camera at the origin. */
function placed(
  figure: BodyFigure,
  diameterPx: number,
  camera: ProjectionCamera,
  viewport: Viewport,
  xPx = viewport.widthPx / 2,
  yPx = viewport.heightPx / 2,
): Vec3 {
  const distance = figure.equatorialRadiusM / Math.sin(diameterPx / 2 / pxPerRad(camera, viewport));
  return scale(rotate(camera.orientation, viewRay(xPx, yPx, camera, viewport)), distance);
}

/** One body lit at `phaseDeg` by a Sun 1 au away, drawn as a mesh over a depth writer. */
function meshBody(
  figure: BodyFigure,
  centreM: Vec3,
  camera: ProjectionCamera,
  viewport: Viewport,
  phaseDeg = 60,
): {
  readonly body: LitBodyInput;
  readonly mesh: MeshBodyPlan;
  readonly record: DiscRecord;
  readonly plan: ReturnType<typeof planLitBodies>;
  readonly options: BodyFrameOptions;
} {
  const towardsCamera = normalise(scale(centreM, -1));
  const side = normalise(
    cross(towardsCamera, Math.abs(towardsCamera.y) < 0.9 ? vec3(0, 1, 0) : vec3(1, 0, 0)),
  );
  const towardsStar = add(
    scale(towardsCamera, Math.cos(phaseDeg * RAD)),
    scale(side, Math.sin(phaseDeg * RAD)),
  );
  const body: LitBodyInput = {
    id: "0200080020000000.0001",
    centreM,
    figure,
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
  };
  const writer = sphereFootprint(centreM, figure.equatorialRadiusM, camera, viewport);
  const options: BodyFrameOptions = {
    camera,
    viewport,
    exposureScale: EXPOSURE,
    annuli: 4,
    planetshine: PLANETSHINE_SOURCES_HIGH,
    setting: "high" as const,
    depthWriters: writer === null ? [] : [writer],
  };
  const plan = planLitBodies(
    [body],
    [{ disc: aHostDisc(), centreM: add(centreM, scale(towardsStar, AU_M)) }],
    options,
    new Map([[body.id, "disc"]]),
  );
  const mesh = plan.meshes[0];
  const record = mesh === undefined ? undefined : plan.discs[mesh.index];
  if (mesh === undefined || record === undefined) {
    throw new Error("the body was not drawn as a mesh");
  }
  return { body, mesh, record, plan, options };
}

/**
 * The signed distance of a point of the view from the record's analytic limb, px, positive
 * outside: the limb angle in the stretched space over its gradient.
 */
function limbDistancePx(
  record: DiscRecord,
  camera: ProjectionCamera,
  viewport: Viewport,
  xPx: number,
  yPx: number,
): number {
  const centre = toViewAxes(record.direction, camera.orientation);
  const pole = normalise(toViewAxes(record.pole, camera.orientation));
  const stretch = 1 / record.polarOverEquatorial;
  const stretched = (v: Vec3): Vec3 => add(v, scale(pole, (stretch - 1) * dot(v, pole)));
  const scaledCentre = stretched(centre);
  const angle = (x: number, y: number): number => {
    const r = stretched(viewRay(x, y, camera, viewport));
    return (
      Math.atan2(norm(cross(r, scaledCentre)), dot(r, scaledCentre)) -
      Math.asin(record.radiusOverDistance / norm(scaledCentre))
    );
  };
  const h = 1 / 64;
  const a0 = angle(xPx, yPx);
  const slope = Math.hypot(angle(xPx + h, yPx) - a0, angle(xPx, yPx + h) - a0) / h;
  return a0 / slope;
}

/**
 * How far the figure's outline departs from the limb, px, over a grid of `samples`² points a
 * pixel within 2 px of the limb: the deepest point inside the limb it leaves uncovered, and the
 * furthest outside it covers.
 */
function silhouetteDeparture(
  body: ReturnType<typeof meshBody>,
  camera: ProjectionCamera,
  viewport: Viewport,
  samples: number,
): { readonly inside: number; readonly outside: number; readonly probed: number } {
  const raster = rasteriseSmoothMesh(body.mesh.mesh, camera, viewport, samples);
  const { rect } = body.record;
  let inside = 0;
  let outside = 0;
  let probed = 0;
  for (let j = Math.floor(rect.topPx * samples); j < Math.ceil(rect.bottomPx * samples); j += 1) {
    for (let i = Math.floor(rect.leftPx * samples); i < Math.ceil(rect.rightPx * samples); i += 1) {
      const d = limbDistancePx(
        body.record,
        camera,
        viewport,
        (i + 0.5) / samples,
        (j + 0.5) / samples,
      );
      if (Math.abs(d) > 2) {
        continue;
      }
      probed += 1;
      const covered = (raster.depth[j * raster.widthSamples + i] ?? -1) >= 0;
      if (covered) {
        outside = Math.max(outside, d);
      } else {
        inside = Math.max(inside, -d);
      }
    }
  }
  return { inside, outside, probed };
}

/** A composite's flux in the green channel, lx. */
function fluxOf(
  pixels: Iterable<{ readonly xPx: number; readonly yPx: number; readonly rgb: readonly number[] }>,
  camera: ProjectionCamera,
  viewport: Viewport,
): number {
  let flux = 0;
  for (const p of pixels) {
    const omega = pixelSolidAngle(
      viewRay(p.xPx + 0.5, p.yPx + 0.5, camera, viewport),
      camera,
      viewport,
    );
    flux += ((p.rgb[1] ?? 0) * omega) / EXPOSURE;
  }
  return flux;
}

/** A Saturn-like figure, f = 0.098, about `pole`. */
function saturnLike(pole: Vec3): BodyFigure {
  return { equatorialRadiusM: 6.0268e7, polarRadiusM: 6.0268e7 * (1 - 0.098), pole };
}

describe("a body's smooth figure (R07.T9)", () => {
  it("is R05's zero-height spheroid: patch terms at h = 0 and origins on the datum", () => {
    const centreM = placed(EARTH, 100, CAMERA, VIEW);
    const { mesh } = meshBody(EARTH, centreM, CAMERA, VIEW);
    expect(mesh.mesh.limited).toBe(false);
    expect(mesh.mesh.patches.length).toBeGreaterThan(0);
    for (const patch of mesh.mesh.patches) {
      expect(patch.terms).toEqual(patchTerms(patch.key, EARTH, 0));
      // The origin is M d₀ about the body's centre, R05's centre vertex (32, 32).
      const [dx, dy, dz] = vertexDir(patch.key, 32, 32);
      const datum = rotateToBody(mesh.mesh.axes, vec3(6.371e6 * dx, 6.371e6 * dy, 6.371e6 * dz));
      expect(norm(sub(sub(patch.originFromCameraM, centreM), datum))).toBeLessThan(1e-6);
    }
    // A distant body needs no level past the roots' at a quarter pixel.
    expect(Math.max(...mesh.mesh.patches.map((p) => p.key.level))).toBe(0);
  });

  it("places each vertex where R05's f32 arithmetic at zero height does, to f32's step", () => {
    const centreM = placed(WGS84, 200, CAMERA, VIEW);
    const { mesh } = meshBody(WGS84, centreM, CAMERA, VIEW);
    const patch = mesh.mesh.patches[0];
    if (patch === undefined) {
      throw new Error("no patch");
    }
    const records = packSmoothMeshes([{ ...mesh.mesh, patches: [patch] }]);
    const f = records.slots;
    const v = (i: number): Vec3 => vec3(f[i] ?? 0, f[i + 1] ?? 0, f[i + 2] ?? 0);
    const u = new Uint32Array(f.buffer, f.byteOffset, f.length);
    const rec: SlotTermsF32 = {
      axisA: v(0),
      s0: f[3] ?? 0,
      axisE1: v(4),
      t0: f[7] ?? 0,
      axisE2: v(8),
      u0: f[11] ?? 0,
      scale: v(12),
      v0: f[15] ?? 0,
      m0: v(16),
      step: f[19] ?? 0,
      nu0: v(20),
      h0M: f[23] ?? 0,
      straddles: u[25] === 1,
    };
    // The twin's own offsets, unmorphed: the vertices less the origin, back in body-fixed axes.
    const unmorphed = { ...patch, morphStartM: 0, morphEndM: 0, skirtDepthM: 0 };
    const vertices = smoothPatchVertices(mesh.mesh, unmorphed);
    let worst = 0;
    for (const [x, y] of [
      [0, 0],
      [64, 0],
      [0, 64],
      [64, 64],
      [17, 40],
      [33, 31],
    ] as const) {
      const offset = sub(vertices[65 * y + x] ?? vec3(0, 0, 0), patch.originFromCameraM);
      const [r0, r1, r2] = mesh.mesh.axes.rows;
      const bodyFixed = vec3(
        r0.x * offset.x + r1.x * offset.y + r2.x * offset.z,
        r0.y * offset.x + r1.y * offset.y + r2.y * offset.z,
        r0.z * offset.x + r1.z * offset.y + r2.z * offset.z,
      );
      // A root patch's offsets reach 5,000 km, where f32's step is half a metre.
      const error = norm(sub(faceDifferencePositionF32(rec, x, y, 0), bodyFixed));
      worst = Math.max(worst, error / (1e-3 + 2 ** -20 * norm(bodyFixed)));
    }
    expect(worst).toBeLessThan(1);
  });

  it.each([
    { name: "an Earth 100 px across at the view's centre", figure: EARTH, px: 100, at: [128, 72] },
    { name: "an Earth 20 px across in the view's corner", figure: EARTH, px: 20, at: [12, 12] },
    {
      name: "a Saturn-like giant 120 px across, its pole tilted 30° towards the camera",
      figure: saturnLike(normalise(vec3(0, Math.cos(30 * RAD), Math.sin(30 * RAD)))),
      px: 120,
      at: [128, 72],
    },
    {
      name: "a Saturn-like giant 6 px across, equator-on",
      figure: saturnLike(vec3(0, 1, 0)),
      px: 6,
      at: [100, 50],
    },
  ] as const)("keeps its outline within half a pixel of the limb: $name", ({ figure, px, at }) => {
    const centreM = placed(figure, px, CAMERA, VIEW, at[0], at[1]);
    const body = meshBody(figure, centreM, CAMERA, VIEW);
    const { inside, outside, probed } = silhouetteDeparture(body, CAMERA, VIEW, 4);
    expect(probed).toBeGreaterThan(0);
    expect(inside).toBeLessThanOrEqual(SMOOTH_MESH_TAU_PX);
    expect(outside).toBeLessThanOrEqual(0.5);
  });

  it("keeps its outline within half a pixel of the limb from close by, refined past the roots", () => {
    // A Jupiter-like giant from 1.1 radii, its limb (65.4° off its centre) across a 640 × 360,
    // 60° view looking 65° off its centre, where the patches under the camera and at the limb
    // refine past the roots.
    const jupiter: BodyFigure = {
      equatorialRadiusM: 7.1492e7,
      polarRadiusM: 7.1492e7 * (1 - 0.0649),
      pole: vec3(0, 1, 0),
    };
    const near: ProjectionCamera = {
      orientation: lookAlong(vec3(Math.sin(65 * RAD), 0, -Math.cos(65 * RAD)), vec3(0, 1, 0)),
      fovXRad: Math.PI / 3,
    };
    const wideView: Viewport = { widthPx: 640, heightPx: 360 };
    const close = meshBody(jupiter, vec3(0, 0, -1.1 * 7.1492e7), near, wideView);
    expect(Math.max(...close.mesh.mesh.patches.map((p) => p.key.level))).toBeGreaterThan(0);
    const { inside, outside, probed } = silhouetteDeparture(close, near, wideView, 1);
    expect(probed).toBeGreaterThan(0);
    expect(inside).toBeLessThanOrEqual(SMOOTH_MESH_TAU_PX);
    expect(outside).toBeLessThanOrEqual(0.5);
  });

  it("keeps its outline within half a pixel of the limb in a 120° view's corner", () => {
    // An Earth 30 px across at the corner of a 120° view, magnified radially by sec²θ there.
    const wide: ProjectionCamera = {
      orientation: IDENTITY_QUATERNION,
      fovXRad: (120 * Math.PI) / 180,
    };
    const corner = meshBody(EARTH, placed(EARTH, 30, wide, VIEW, 14, 14), wide, VIEW);
    const { inside, outside, probed } = silhouetteDeparture(corner, wide, VIEW, 4);
    expect(probed).toBeGreaterThan(0);
    expect(inside).toBeLessThanOrEqual(SMOOTH_MESH_TAU_PX);
    expect(outside).toBeLessThanOrEqual(0.5);
  });

  it("takes a quarter pixel at the view's corner, over sec²θ there", () => {
    const tau = smoothMeshTauPx(CAMERA, { widthPx: 1920, heightPx: 1080 });
    const tanX = Math.tan(Math.PI / 6);
    expect(tau).toBeCloseTo(0.25 / (1 + tanX ** 2 * (1 + (1080 / 1920) ** 2)), 12);
    expect(tau).toBeCloseTo(0.1738, 4);
  });

  it.each([
    { name: "3.3 px, at the disc's switch from a point", px: 3.3, phase: 90, figure: EARTH },
    { name: "6 px at 150° of phase", px: 6, phase: 150, figure: EARTH },
    { name: "40 px at 0°", px: 40, phase: 0, figure: EARTH },
    {
      name: "a Saturn-like giant 64 px across at 60°",
      px: 64,
      phase: 60,
      figure: saturnLike(normalise(vec3(0, 1, 0.4))),
    },
  ] as const)("draws the flux the promoted disc draws, to 1%: $name", ({ px, phase, figure }) => {
    const centreM = placed(figure, px, CAMERA, VIEW, 101.3, 60.7);
    const { plan, record } = meshBody(figure, centreM, CAMERA, VIEW, phase);
    const asMesh = compositeBodyFrame(plan, CAMERA, VIEW);
    expect(asMesh.holes).toBe(0);
    const asDisc = compositeDiscPixels(rasteriseDisc(record, CAMERA, VIEW));
    const disc = fluxOf(asDisc, CAMERA, VIEW);
    const mesh = fluxOf(asMesh.pixels.values(), CAMERA, VIEW);
    expect(disc).toBeGreaterThan(0);
    expect(Math.abs(mesh / disc - 1)).toBeLessThan(0.01);
  });

  it("serves a camera 2 m above an Earth at 1080p within its budget", () => {
    const view: Viewport = { widthPx: 1920, heightPx: 1080 };
    const up = vec3(0, 0, 1);
    const ground = vec3(0, 0, 6.371e6 + 2);
    const camera: ProjectionCamera = {
      orientation: lookAlong(vec3(1, 0, -0.2), up),
      fovXRad: Math.PI / 3,
    };
    const mesh = smoothMeshOf(
      "0200080020000000.0001",
      scale(ground, -1),
      EARTH,
      up,
      camera,
      view,
      "high",
    );
    expect(mesh.limited).toBe(false);
    expect(mesh.patches.length).toBeLessThan(MAX_SMOOTH_MESH_PATCHES / 4);
  });

  it("puts its limb draw's corners on the limb's plane, which holds the limb", () => {
    const figure = saturnLike(normalise(vec3(0.3, 1, 0.5)));
    const centreM = placed(figure, 80, CAMERA, VIEW, 120, 70);
    const { mesh, record } = meshBody(figure, centreM, CAMERA, VIEW);
    // The limb point on the ray past the centre's direction, across the pole: found by bisecting
    // the limb angle along a line of the view, then the ray's tangent point on the spheroid.
    const centrePx = project(centreM, CAMERA, VIEW);
    let inner = 0;
    let outer = 80;
    for (let k = 0; k < 60; k += 1) {
      const mid = (inner + outer) / 2;
      if (limbDistancePx(record, CAMERA, VIEW, centrePx.xPx + mid, centrePx.yPx) < 0) {
        inner = mid;
      } else {
        outer = mid;
      }
    }
    const x = centrePx.xPx + inner;
    const ray = viewRay(x, centrePx.yPx, CAMERA, VIEW);
    // The tangent ray's closest approach in the stretched space is its limb point.
    const pole = normalise(toViewAxes(record.pole, CAMERA.orientation));
    const stretch = 1 / record.polarOverEquatorial;
    const stretched = (v: Vec3): Vec3 => add(v, scale(pole, (stretch - 1) * dot(v, pole)));
    const r = stretched(ray);
    const c = stretched(toViewAxes(centreM, CAMERA.orientation));
    const t = dot(r, c) / dot(r, r);
    const w = -t * ray.z;
    const depth = limbDepthAt(record.rect, mesh.limbDepths, x, centrePx.yPx);
    expect(Math.abs(depth / (NEAR_PLANE_M / w) - 1)).toBeLessThan(1e-6);
  });

  it("puts its limb on the plane from 400 km up, where the view's upper corners see the plane behind", () => {
    // An Earth 400 km below, the camera pitched 15° down: the horizon, 19.8° below the level, is
    // 4.8° below the view's centre, 2,290 km away.
    const camera: ProjectionCamera = {
      orientation: lookAlong(vec3(0, -Math.sin(15 * RAD), -Math.cos(15 * RAD)), vec3(0, 1, 0)),
      fovXRad: Math.PI / 3,
    };
    const centreM = vec3(0, -(6.371e6 + 4e5), 0);
    const { mesh, record } = meshBody(EARTH, centreM, camera, VIEW);
    // The limb on the view's middle column, by bisecting the distance from it.
    let above = 0;
    let below = VIEW.heightPx;
    for (let k = 0; k < 60; k += 1) {
      const mid = (above + below) / 2;
      if (limbDistancePx(record, camera, VIEW, VIEW.widthPx / 2, mid) > 0) {
        above = mid;
      } else {
        below = mid;
      }
    }
    const ray = viewRay(VIEW.widthPx / 2, above, camera, VIEW);
    // The tangent ray's limb point is its closest approach to the centre.
    const centre = toViewAxes(centreM, camera.orientation);
    const t = dot(ray, centre);
    expect(t).toBeGreaterThan(2.28e6);
    expect(t).toBeLessThan(2.3e6);
    const depth = limbDepthAt(record.rect, mesh.limbDepths, VIEW.widthPx / 2, above);
    expect(Math.abs(depth / (NEAR_PLANE_M / (-t * ray.z)) - 1)).toBeLessThan(1e-6);
    // The upper corners see the plane behind the camera: the clip removes them.
    expect(Math.min(...mesh.limbDepths)).toBeLessThan(0);
  });

  it("packs each figure's patches as R05's slot and instance records, slot by slot", () => {
    const a = smoothMeshOf(
      "0200080020000000.0001",
      placed(EARTH, 50, CAMERA, VIEW, 60, 70),
      EARTH,
      vec3(0, 1, 0),
      CAMERA,
      VIEW,
      "high",
    );
    const b = smoothMeshOf(
      "0200080020000000.0002",
      placed(EARTH, 30, CAMERA, VIEW, 200, 70),
      EARTH,
      vec3(0, 1, 0),
      CAMERA,
      VIEW,
      "low",
    );
    const records = packSmoothMeshes([a, b]);
    expect(records.count).toBe(a.patches.length + b.patches.length);
    expect(records.firstInstance).toEqual([0, a.patches.length]);
    expect(records.slots.byteLength).toBe(records.count * SLOT_RECORD_BYTES);
    expect(records.instances.byteLength).toBe(records.count * INSTANCE_RECORD_BYTES);
    const words = new Uint32Array(records.instances.buffer, 0, records.instances.length);
    const stride = INSTANCE_RECORD_BYTES / 4;
    for (let i = 0; i < records.count; i += 1) {
      expect(words[i * stride + 3]).toBe(i);
    }
    const last = b.patches.at(-1);
    const at = (records.count - 1) * stride;
    expect(records.instances[at]).toBe(Math.fround(last?.originFromCameraM.x ?? Number.NaN));
  });

  it("builds its axes about the pole", () => {
    const pole = normalise(vec3(0.2, -0.5, 0.8));
    const z = rotateToBody(poleAxes(pole), vec3(0, 0, 1));
    expect(norm(sub(z, pole))).toBeLessThan(1e-15);
  });
});
