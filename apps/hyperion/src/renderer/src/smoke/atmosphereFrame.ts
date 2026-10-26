/**
 * R08.T10.b: a body's air drawn over its own ground in the photorealistic frame
 * (`decision-r08-t10b-depth.md` §6). An Earth with R05's medium, seen from outside its air, 40 px
 * across, is drawn through `PhotorealRenderer` twice under the same tables, with two disc surfaces
 * that differ (its albedo halved). The body, an air body, is drawn as a mesh, so the scene's depth
 * is above 0 at the disc's centre, and the composite lays the air over the ground drawn there:
 * (air output A − B) ÷ (scene colour A − B) lies in (0, 1] in every channel, the view path's
 * transmittance. Over the march's own bare ground it would be 0, since that reads the medium's
 * albedo whatever the disc's.
 *
 * Recorded, not asserted: how many of the disc's pixels stand at depth 0, the limb's ring
 * (`decision-r08-t10b-depth.md` §4, deferred to R08.T16.b).
 */

import { add, scale, vec3 } from "../geometry/vec3";
import { lawFor } from "../view/appearance/phase";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import type { BodyPhotometry } from "../view/appearance/fromWire";
import { R05_EARTH_REFERENCE } from "../view/atmosphere/earth";
import { AtmosphereTables } from "../view/atmosphere/tables";
import type { ProjectionCamera } from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import type { RenderEngine } from "../view/engine/types";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PhotorealRenderer } from "../view/photoreal/renderer";
import type { Rgb } from "../view/photometry/toneCurve";
import { AU_M } from "../view/scenes/kept";
import { addCanvas } from "./frames";
import { type Checks, halfTexels } from "./harness";

/** R05's spherical Earth's mean radius, m, on which its medium's tables are built. */
const EARTH_MEAN_RADIUS_M = 6_371_008.8;

const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const VIEWPORT = { widthPx: 96, heightPx: 54 };

/** The disc's photometry at `albedo`: the provisional law's, as R07's Lambert sphere. */
function photometryAt(albedo: number): BodyPhotometry {
  const p: Rgb = [albedo, albedo, albedo];
  const q: Rgb = [1.5, 1.5, 1.5];
  return {
    ...PROVISIONAL_PHOTOMETRY,
    geometricAlbedo: p,
    phaseIntegral: q,
    law: lawFor(p, q, "lambert"),
  };
}

/** What one frame left at the disc's centre, and how many of the disc's pixels hold no depth. */
interface FrameAtCentre {
  readonly depth: number;
  readonly scene: Rgb;
  readonly drawn: Rgb;
  readonly finite: boolean;
  readonly ringPx: number;
  readonly discPx: number;
}

/** Draws the Earth at `albedo` under `tables` and reads the frame's colours and depth back. */
async function frameAt(
  engine: RenderEngine,
  renderer: PhotorealRenderer,
  tables: AtmosphereTables,
  albedo: number,
): Promise<FrameAtCentre> {
  const view = engine.createView(addCanvas(), "smoke atmosphere frame");
  view.resize(VIEWPORT);
  try {
    const pxPerRad = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
    const distanceM = EARTH_MEAN_RADIUS_M / Math.sin(20 / pxPerRad);
    const centreM = vec3(0, 0, -distanceM);
    // The Sun 40° off the line of sight, so that the face the camera sees is lit.
    const towards = vec3(Math.sin((40 * Math.PI) / 180), 0, Math.cos((40 * Math.PI) / 180));
    const id = "0200080020000000.0300";
    renderer.render(view, {
      camera: CAMERA,
      viewport: VIEWPORT,
      presentedPx: VIEWPORT,
      role: "eye",
      setting: "high",
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
          photometry: photometryAt(albedo),
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
    const colours = renderer.frameColours;
    const depthTexture = colours?.scene.depth ?? null;
    if (colours === null || depthTexture === null) {
      throw new Error("the photorealistic frame left no scene target to read");
    }
    const [scene, drawn, depths] = await Promise.all([
      engine.readTexture(colours.scene.colour).then(halfTexels),
      engine.readTexture(colours.drawn).then(halfTexels),
      engine.readTexture(depthTexture).then((bytes) => new Float32Array(bytes)),
    ]);
    const cx = VIEWPORT.widthPx / 2;
    const cy = VIEWPORT.heightPx / 2;
    const at = (texels: Float32Array, x: number, y: number): Rgb => {
      const i = (y * VIEWPORT.widthPx + x) * 4;
      return [texels[i] ?? Number.NaN, texels[i + 1] ?? Number.NaN, texels[i + 2] ?? Number.NaN];
    };
    let ringPx = 0;
    let discPx = 0;
    for (let y = 0; y < VIEWPORT.heightPx; y += 1) {
      for (let x = 0; x < VIEWPORT.widthPx; x += 1) {
        if (Math.hypot(x + 0.5 - cx, y + 0.5 - cy) <= 20) {
          discPx += 1;
          if ((depths[y * VIEWPORT.widthPx + x] ?? 0) === 0) {
            ringPx += 1;
          }
        }
      }
    }
    return {
      depth: depths[cy * VIEWPORT.widthPx + cx] ?? Number.NaN,
      scene: at(scene, cx, cy),
      drawn: at(drawn, cx, cy),
      finite: drawn.every((value) => Number.isFinite(value)),
      ringPx,
      discPx,
    };
  } finally {
    view.dispose();
  }
}

/**
 * The air over a drawn ground (the module's remarks): depth at the disc's centre, the composite's
 * share of the difference two grounds make, and finite texels.
 */
export async function checkAtmosphereFrame(engine: RenderEngine, checks: Checks): Promise<void> {
  const renderer = new PhotorealRenderer(engine, "smoke atmosphere frame");
  const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);
  try {
    await renderer.prepare(VIEWPORT, "high", "eye", CAMERA);
    const a = await frameAt(engine, renderer, tables, 0.3);
    const b = await frameAt(engine, renderer, tables, 0.15);
    checks.check(
      "R08.T10.b an air body is drawn with depth: the scene's depth is above 0 at its disc's centre",
      a.depth > 0 && b.depth > 0,
      `depth ${String(a.depth)} and ${String(b.depth)}; ${String(a.ringPx)} of the disc's ` +
        `${String(a.discPx)} pixels at depth 0, the limb's ring (recorded)`,
    );
    const shares = [0, 1, 2].map((ch) => {
      const out = (a.drawn[ch] ?? Number.NaN) - (b.drawn[ch] ?? Number.NaN);
      const ground = (a.scene[ch] ?? Number.NaN) - (b.scene[ch] ?? Number.NaN);
      return out / ground;
    });
    checks.check(
      "R08.T10.b the composite lays the air over the ground drawn: (out A − B) ÷ (ground A − B) in (0, 1]",
      shares.every((share) => share > 0 && share <= 1),
      `shares r ${shares[0]?.toFixed(4) ?? "?"}, g ${shares[1]?.toFixed(4) ?? "?"}, ` +
        `b ${shares[2]?.toFixed(4) ?? "?"}; grounds ${a.scene.map((v) => v.toPrecision(4)).join(", ")} ` +
        `and ${b.scene.map((v) => v.toPrecision(4)).join(", ")}`,
    );
    checks.check(
      "R08.T10.b every texel of the composite's output is finite",
      a.finite && b.finite,
      `finite: ${String(a.finite)}, ${String(b.finite)}`,
    );
  } finally {
    renderer.dispose();
    tables.dispose();
  }
}
