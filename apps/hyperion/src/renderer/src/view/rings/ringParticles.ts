/**
 * A ring's particles close to on a view (plan R11, R11.T9.d; Design note 16): the local particle
 * field of the ring nearest the camera within its largest particle's hand-over, placed on the GPU
 * (`ringParticles.wgsl`, `ringField.ts`), culled by R14's instance path (`ParticlePath`) and drawn
 * opaque with depth before the painter's sequence, its share of the optical depth taken from the
 * slab (`ringSlab.wgsl`) at every pixel's own footprint.
 *
 * @remarks
 * Particles are drawn on the discrete target alone: the high setting's `rings` is `particles`, the
 * low's `annulus`, which makes no path, kernel or material ({@link particlesOn}). The field keeps
 * {@link RING_PARTICLE_CAPACITY} particles at most; where the view would hold more (low over the
 * plane, near a grazing horizon) it is instanced at a larger pixel, by {@link fieldBudgetScale},
 * and the slab gives up the smaller share that angle's footprint holds, so that the light let
 * through slab and particles together is the unsplit slab's. The path's buffers are its own
 * (`geometry-clusters`, as R14's are); the field's parameters ride its dispatch.
 */
import type { BodyIdHex, UniverseTime } from "@hyperion/protocol";

import { dot, norm, normalise, scale, sub } from "../../geometry/vec3";
import type { BodyFramePlan, LitBodyInput } from "../bodies/draw";
import type { LitRegime } from "../bodies/regime";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import type { BucketDrawBase } from "../clusters/buckets";
import { FOLDED_GROUP_WGSL } from "../clusters/fold";
import { INSTANCE_CULL_KERNEL, instanceWorkgroups } from "../clusters/instanceCull";
import { cameraCullPassOf, viewAxisOf } from "../clusters/instanceScene";
import { ParticlePath } from "../clusters/particlePath";
import {
  PARTICLE_INSTANCE_WGSL,
  PARTICLE_STORAGE_BUFFERS,
  sphereParticleTemplates,
} from "../clusters/particles";
import type { KernelPair } from "../engine/kernels";
import type {
  ComputeHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  WgslMaterialSpec,
} from "../engine/types";
import { METER_CLASS } from "../post/meter";
import { type QualitySetting, SETTINGS } from "../quality/qualitySetting";
import type { ViewRing } from "../scene/model";
import frameWgsl from "../shaders/frame.wgsl?raw";
import {
  fieldTauAt,
  fieldWindowPixelRad,
  packRingFieldParams,
  profileSteps,
  RING_FIELD_PATCH_M,
  RING_LAYER_SIGMA_M,
  type RingField,
  ringFieldOf,
  sheetPlaceOf,
} from "./ringField";
import { ringProfile } from "./ringProfile";
import { powerLawPhase } from "./ringPhotometry";
import { ringPlanetVisible, ringSlabRecordOf } from "./ringSlab";
import {
  handOverDistance,
  instancedPerSquareMetre,
  pixelAngleRad,
  RING_PARTICLE_RADII_M,
} from "./ringSplit";
import ringParticleDrawWgsl from "./shaders/ringParticleDraw.wgsl?raw";
import ringParticlesWgsl from "./shaders/ringParticles.wgsl?raw";

/** The field's placement: `presentation-only`, its `f32` arithmetic the device's. */
export const RING_PARTICLE_PLACE_KERNEL: KernelPair = {
  name: "ring particle place",
  reference:
    FOLDED_GROUP_WGSL +
    `const FIELD_BOUNDS_SCALE : f32 = ${String(sphereParticleTemplates().boundsScale)};\n` +
    ringParticlesWgsl,
  subgroup: null,
  readback: "presentation-only",
};

/** The particles' `Draw`, after `offsetFromCameraM`: the star, its light and the bucket's base. */
const PARTICLE_DRAW_WGSL = `
struct Draw {
  offsetFromCameraM : vec3f,
  // Towards the ring's star from the field, a unit vector in the passes' frame.
  sun : vec4f,
  // The star's face-on illuminance per channel times the planet's shadow where the field is and
  // exposure ÷ π; w, the meter class the particles write.
  light : vec4f,
  // The bucket's first entry in the lists (\`clusters/buckets.ts\`' \`clusterBase\`).
  clusterBase : u32,
}

@group(1) @binding(0) var<uniform> draw : Draw;
`;

/** The particles' source: frame, `Draw`, R14's vertex library, this material's stages. */
const PARTICLE_SOURCE =
  frameWgsl + PARTICLE_DRAW_WGSL + PARTICLE_INSTANCE_WGSL + ringParticleDrawWgsl;

/** The particles' material: opaque, depth written, back faces culled (`ringParticleDraw.wgsl`). */
export const RING_PARTICLE_MATERIAL: WgslMaterialSpec = {
  name: "rings:particles",
  displayName: "RING PARTICLES",
  vertexWgsl: PARTICLE_SOURCE,
  fragmentWgsl: PARTICLE_SOURCE,
  uniforms: [
    { name: "sun", type: "vec4f" },
    { name: "light", type: "vec4f" },
    { name: "clusterBase", type: "u32" },
  ],
  samplers: [],
  cullMode: "back",
  depthWrite: true,
  colourWrites: true,
  blend: "none",
  storageBuffers: PARTICLE_STORAGE_BUFFERS,
};

/** The particles' draw pass's label (R12's `PASS_ROWS` `primary:ring particles`). */
export const RING_PARTICLES_PASS_LABEL = "ring particles";

/** Their cull's dispatch's label (`primary:ring particle cull`). */
export const RING_PARTICLE_CULL_LABEL = "ring particle cull";

/** The most particles a view's field holds: 2²⁰, 32 MiB of records and 16 MiB of lists. */
export const RING_PARTICLE_CAPACITY = 1 << 20;

/** The share of the capacity a field is budgeted to fill on average. */
const BUDGET_SHARE = 0.6;

/** The field's seed, the same on every view: presentation, which no peer compares. */
const FIELD_SEED = 0x7269_6e67;

/** Whether a setting draws ring particles: its `rings` is `particles` (the high setting's). */
export function particlesOn(setting: QualitySetting): boolean {
  return SETTINGS[setting].rings === "particles";
}

/** The ring a frame's field is of, and its planet. */
export interface FieldRing {
  readonly ring: ViewRing;
  readonly planet: LitBodyInput;
}

/**
 * The massive ring whose annulus the camera is nearest, within its largest particle's hand-over at
 * `pixelRad` and its planet drawn larger than a point: the one the view's field is of, or `null`.
 */
export function fieldRingOf(
  rings: ReadonlyArray<ViewRing>,
  bodies: ReadonlyArray<LitBodyInput>,
  regimes: ReadonlyMap<BodyIdHex, LitRegime>,
  pixelRad: number,
): FieldRing | null {
  let best: { readonly at: FieldRing; readonly apartM: number } | null = null;
  for (const ring of rings) {
    const handOverM = handOverDistance(ring, pixelRad);
    const planet = bodies.find((body) => body.id === ring.body);
    const regime = regimes.get(ring.body);
    // A dusty ring's grains are its slab's at any distance (`handOverDistance`'s 0).
    if (!(handOverM > 0) || planet === undefined || regime === undefined || regime === "point") {
      continue;
    }
    const reachM = handOverM + 3 * RING_LAYER_SIGMA_M;
    const place = sheetPlaceOf(planet.centreM, ring.normal);
    const radialM = Math.max(
      ring.innerRadiusM - place.radiusM,
      place.radiusM - ring.outerRadiusM,
      0,
    );
    const apartM = Math.hypot(radialM, place.heightM);
    if (apartM < reachM && (best === null || apartM < best.apartM)) {
      best = { at: { ring, planet }, apartM };
    }
  }
  return best?.at ?? null;
}

/**
 * The particles a field places in view, estimated by rays of the view against the plane: each
 * octave's number a square metre (`instancedPerSquareMetre`'s, an octave's) at the field's own τ
 * where the ray meets the plane (its steps', the gaps' and the annulus's edges' with them) times the
 * area within its reach that the ray's pixel cell covers, its solid angle at its distance over its
 * slant.
 *
 * @remarks
 * A camera within the layer's 3σ sees its particles on both sides of the plane: the estimate then
 * takes both halves of the view, each against a plane 3σ away, which counts the layer's column
 * twice where the camera sees each half of it once, a margin on the budget's side.
 */
export function expectedFieldParticles(
  field: RingField,
  camera: ProjectionCamera,
  viewport: Viewport,
  rays: readonly [number, number] = [32, 18],
): number {
  const [nx, ny] = rays;
  const tanX = Math.tan(camera.fovXRad / 2);
  const tanY = (tanX * viewport.heightPx) / viewport.widthPx;
  const cellSr = ((2 * tanX) / nx) * ((2 * tanY) / ny);
  const [radial, , normal] = field.axes;
  const [sMin, sMax] = RING_PARTICLE_RADII_M;
  const layerM = 3 * field.sigmaM;
  const within = Math.abs(field.heightM) < layerM;
  let total = 0;
  for (let i = 0; i < nx; i += 1) {
    for (let j = 0; j < ny; j += 1) {
      // A ray of the view in its own axes, its direction's solid angle at the cell's tangent.
      const x = ((i + 0.5) / nx) * 2 - 1;
      const y = ((j + 0.5) / ny) * 2 - 1;
      const along = { x: x * tanX, y: y * tanY, z: -1 };
      const length = norm(along);
      const view = scale(along, 1 / length);
      const ray = rotate(camera.orientation, view);
      const den = -dot(normal, ray);
      const slant = Math.max(Math.abs(den), 1e-3);
      const distanceM = within ? layerM / slant : field.heightM / den;
      if (!(distanceM > 0) || !Number.isFinite(distanceM)) {
        continue;
      }
      const solidAngle = cellSr / length ** 3;
      const areaM2 = (distanceM * distanceM * solidAngle) / slant;
      const footprintM = distanceM * field.pixelRad;
      const tau = fieldTauAt(field, distanceM * dot(ray, radial));
      total += instancedPerSquareMetre(tau, footprintM, sMin, sMax) * areaM2;
    }
  }
  return total;
}

/**
 * The factor ≥ 1 the field's pixel angle is widened by, so that its expected particles fit
 * {@link BUDGET_SHARE} of {@link RING_PARTICLE_CAPACITY}: their number falls about as the square of
 * the angle, so its root of the excess, once and again.
 */
export function fieldBudgetScale(estimate: (scaleBy: number) => number): number {
  let factor = 1;
  for (let pass = 0; pass < 3; pass += 1) {
    const count = estimate(factor);
    const budget = BUDGET_SHARE * RING_PARTICLE_CAPACITY;
    if (!(count > budget)) {
      break;
    }
    factor *= Math.sqrt(count / budget) * 1.05;
  }
  return factor;
}

/** What one frame's particles are drawn from. */
export interface RingParticlesFrameInput {
  readonly rings: ReadonlyArray<ViewRing>;
  readonly bodies: ReadonlyArray<LitBodyInput>;
  readonly plan: Pick<BodyFramePlan, "regimes" | "sources">;
  readonly camera: ProjectionCamera;
  readonly viewport: Viewport;
  readonly viewRotation: Float32Array;
  readonly projection: Float32Array;
  /**
   * The width of the target drawn into, px, which the cull's tolerance takes with
   * {@link RingParticlesFrameInput.projection}: a still's tile's, through its M · P (plan R11,
   * R11.T10.e.2), so that a tile culls at the still's pixel; the viewport's where absent.
   */
  readonly targetWidthPx?: number | undefined;
  /** The scene's time, from which the field turns and shears. */
  readonly time: UniverseTime;
  readonly exposureScale: number;
  readonly annuli: number;
}

/** One frame's particles: the ring they are of, the pixel's angle they are instanced at, the draws. */
export interface RingParticlesFrame {
  readonly ring: ViewRing;
  readonly pixelRad: number;
  readonly draws: ReadonlyArray<DrawItem>;
}

/** What the particles make and release their resources with. */
export type RingParticlesEngine = Pick<
  RenderEngine,
  | "createBuffer"
  | "writeBuffer"
  | "releaseBuffer"
  | "dispatch"
  | "createComputeAsync"
  | "releaseCompute"
  | "createMaterialAsync"
  | "releaseMaterial"
  | "createMesh"
  | "releaseMesh"
>;

/** The particles' pipelines: their kernels, material and mesh, made before their path. */
interface ParticlePipelines {
  /** The templates its mesh was made from, which its path takes. */
  readonly templates: ReturnType<typeof sphereParticleTemplates>;
  readonly place: ComputeHandle;
  readonly cull: ComputeHandle;
  readonly material: MaterialHandle;
  readonly mesh: MeshHandle;
}

interface ParticleResources extends ParticlePipelines {
  readonly path: ParticlePath;
}

/** The seconds from `from` to `to`, exact to the nanosecond whatever their distance from the epoch. */
function secondsBetween(from: UniverseTime, to: UniverseTime): number {
  return to.seconds - from.seconds + (to.nanos - from.nanos) * 1e-9;
}

/** A view's ring particles (see the module's remarks). */
export class RingParticles {
  readonly #engine: RingParticlesEngine;
  readonly #name: string;
  #resources: ParticleResources | null = null;
  /** Its pipelines once made, which its path joins at the first frame near a ring. */
  #pipelines: ParticlePipelines | null = null;
  #making: Promise<void> | null = null;
  #failed = false;
  #generation = 0;
  #disposed = false;
  /** The field's reference radius and time, for its ring, held while the camera stays near. */
  #reference: {
    readonly ring: BodyIdHex;
    readonly radiusM: number;
    readonly time: UniverseTime;
  } | null = null;

  /** The particles of the view `name`, made with `engine` at their first frame near a ring. */
  constructor(engine: RingParticlesEngine, name: string) {
    this.#engine = engine;
    this.#name = name;
  }

  /**
   * Makes the path, kernels, material and mesh, once a device; resolves when they are made, or
   * when their making failed, which is reported once and leaves the rings slabs alone.
   */
  async prepare(): Promise<void> {
    await this.preparePipelines();
    this.#withPath();
  }

  /**
   * Starts making the kernels, material and mesh alone, once a device (the photorealistic
   * renderer's making, with its other pipelines: plan R11, R11.T9.d over R10.T14.f), its path left
   * to its first frame near a ring, so that a view with no ring holds none of its 48 MiB; resolves
   * when they are made, or when their making failed, which is reported once.
   */
  preparePipelines(): Promise<void> {
    if (this.#making === null) {
      const generation = this.#generation;
      this.#making = this.#make(generation).catch((error: unknown) => {
        if (generation === this.#generation && !this.#disposed) {
          this.#failed = true;
          console.error(`the ring particles of the view ${this.#name} could not be made:`, error);
        }
      });
    }
    return this.#making;
  }

  async #make(generation: number): Promise<void> {
    const engine = this.#engine;
    const templates = sphereParticleTemplates();
    const mesh = engine.createMesh({ ...templates.mesh, name: `${this.#name}:ring particle` });
    const made = await Promise.allSettled([
      engine.createComputeAsync(RING_PARTICLE_PLACE_KERNEL),
      engine.createComputeAsync(INSTANCE_CULL_KERNEL),
      engine.createMaterialAsync(RING_PARTICLE_MATERIAL, ["rgba16float"], [mesh]),
    ]);
    const [place, cull, material] = made;
    // Every handle made is let go where the making gives up: refused, disposed or overtaken.
    const release = (): void => {
      for (const kernel of [place, cull]) {
        if (kernel.status === "fulfilled") {
          engine.releaseCompute(kernel.value);
        }
      }
      if (material.status === "fulfilled") {
        engine.releaseMaterial(material.value);
      }
      engine.releaseMesh(mesh);
    };
    if (
      place.status === "rejected" ||
      cull.status === "rejected" ||
      material.status === "rejected" ||
      this.#disposed ||
      generation !== this.#generation
    ) {
      release();
      const refused = made.find((each) => each.status === "rejected");
      if (refused?.status === "rejected") {
        throw new Error(`the ring particles' kernels of ${this.#name} could not be made`, {
          cause: refused.reason,
        });
      }
      return;
    }
    this.#pipelines = {
      templates,
      place: place.value,
      cull: cull.value,
      material: material.value,
      mesh,
    };
  }

  /**
   * Its resources, its path made now beside its pipelines where they are made and it has none;
   * `null` while the pipelines are not made, or where the path could not be made, which is
   * reported once and leaves the slabs alone until a restore.
   */
  #withPath(): ParticleResources | null {
    const pipelines = this.#pipelines;
    if (this.#resources !== null || pipelines === null || this.#failed || this.#disposed) {
      return this.#resources;
    }
    try {
      const path = new ParticlePath(
        this.#engine,
        `${this.#name}:ring particles`,
        pipelines.templates,
        RING_PARTICLE_CAPACITY,
        1,
      );
      this.#resources = { ...pipelines, path };
    } catch (error: unknown) {
      this.#failed = true;
      console.error(`the ring particles of the view ${this.#name} could not be made:`, error);
    }
    return this.#resources;
  }

  /**
   * The frame's particles: the field of the ring nearest the camera within its hand-over, placed
   * and culled (two dispatches, which precede every draw encoded after the call), and their bucket
   * draws, which the caller draws with depth before the painter's sequence; `null` where no ring is
   * near, or while the particles are not made, whose making the first such frame starts.
   */
  frame(input: RingParticlesFrameInput): RingParticlesFrame | null {
    const pixelRad = pixelAngleRad(input.camera.fovXRad, input.viewport.widthPx);
    const near = fieldRingOf(input.rings, input.bodies, input.plan.regimes, pixelRad);
    const stars = near === null ? undefined : input.plan.sources.get(near.planet.id)?.stars;
    if (near === null || stars === undefined) {
      return null;
    }
    const resources = this.#withPath();
    if (resources === null) {
      if (!this.#failed) {
        void this.prepare();
      }
      return null;
    }
    const { ring, planet } = near;
    const place = sheetPlaceOf(planet.centreM, ring.normal);
    const held = this.#reference;
    const reference =
      held !== null &&
      held.ring === ring.body &&
      Math.abs(place.radiusM - held.radiusM) < RING_FIELD_PATCH_M
        ? held
        : { ring: ring.body, radiusM: place.radiusM, time: input.time };
    this.#reference = reference;
    const profile = ringProfile(ring);
    const pass = cameraCullPassOf(
      input.viewRotation,
      input.projection,
      input.targetWidthPx ?? input.viewport.widthPx,
      1,
    );
    const star = stars[0];
    const towards = star === undefined ? null : normalise(star.toStarM);
    const fieldAt = (scaleBy: number): RingField | null =>
      ringFieldOf({
        centreM: planet.centreM,
        normal: ring.normal,
        profile,
        gravitationalParameterM3S2: ring.gravitationalParameterM3S2 ?? null,
        referenceRadiusM: reference.radiusM,
        sinceS: secondsBetween(reference.time, input.time),
        pixelRad: pixelRad * scaleBy,
        planes: pass.planes,
        seed: FIELD_SEED,
        towardsStar: towards,
      });
    // The pixel's least widening: the capacity's budget's, and the octaves' windows' cap's.
    const reachM = handOverDistance(ring, pixelRad) + 3 * RING_LAYER_SIGMA_M;
    const steps = profileSteps(profile, place.radiusM, reachM);
    const tauReference = Math.max(0, ...steps.steps.map((step) => step.tau));
    const windowed = tauReference > 0 ? fieldWindowPixelRad(tauReference) / pixelRad : 1;
    const factor = Math.max(
      windowed,
      fieldBudgetScale((scaleBy) => {
        const field = fieldAt(scaleBy);
        return field === null ? 0 : expectedFieldParticles(field, input.camera, input.viewport);
      }),
    );
    const field = fieldAt(factor);
    if (field === null) {
      return null;
    }
    const { path } = resources;
    path.writeCount(0);
    this.#engine.dispatch(
      resources.place,
      {
        uniforms: { params: new Uint32Array(packRingFieldParams(field, path.capacity)) },
        buffers: { instances: path.instances, instance_count: path.count },
        sampled: {},
        storage: {},
      },
      instanceWorkgroups(field.cells),
      RING_PARTICLE_PLACE_KERNEL.name,
    );
    path.cull(
      resources.cull,
      { passes: [pass], viewAxis: viewAxisOf(input.viewRotation) },
      RING_PARTICLE_CULL_LABEL,
    );
    const base: BucketDrawBase = {
      material: resources.material,
      offsetFromCameraM: new Float32Array(3),
      uniforms: lightUniforms(ring, planet, stars, place.radiusM, input),
      textures: {},
    };
    return { ring, pixelRad: field.pixelRad, draws: path.drawItems(0, resources.mesh, base) };
  }

  /** Forgets the handles a lost device took with it; the next frame near a ring makes them again. */
  restored(): void {
    this.#generation += 1;
    this.#resources = null;
    this.#pipelines = null;
    this.#making = null;
    this.#failed = false;
  }

  /**
   * Lets go of what it made, for a view whose setting stops drawing particles (the low setting's
   * 48 MiB back); the next frame near a ring on a setting that draws them makes them again.
   */
  release(): void {
    this.#generation += 1;
    this.#making = null;
    this.#failed = false;
    this.#reference = null;
    this.#resources?.path.dispose();
    this.#resources = null;
    const pipelines = this.#pipelines;
    this.#pipelines = null;
    if (pipelines !== null) {
      this.#engine.releaseCompute(pipelines.place);
      this.#engine.releaseCompute(pipelines.cull);
      this.#engine.releaseMaterial(pipelines.material);
      this.#engine.releaseMesh(pipelines.mesh);
    }
  }

  /** Releases everything it made; a making in flight is released as it ends. */
  dispose(): void {
    this.#disposed = true;
    this.release();
  }
}

/**
 * A Lambert sphere's phase at phase angle α, normalised over 4π: (8 ÷ 3π) (sin α + (π − α) cos α),
 * 8/3 at opposition (Russell 1916's Lambert sphere).
 *
 * @remarks
 * Taken in u = π − α as sin u − u cos u, and by its series u³ ÷ 3 − u⁵ ÷ 30 + u⁷ ÷ 840 below
 * u = 0.01, where the difference would lose its digits: it falls as u³ to 0 at α = π.
 */
export function lambertSpherePhase(alphaRad: number): number {
  const u = Math.PI - Math.min(Math.max(alphaRad, 0), Math.PI);
  const u2 = u * u;
  const shape =
    u < 0.01 ? u * u2 * (1 / 3 - u2 / 30 + (u2 * u2) / 840) : Math.sin(u) - u * Math.cos(u);
  return (8 / (3 * Math.PI)) * shape;
}

/** The least π − α the particles' phase ratio is taken at, rad: both phases vanish at π. */
const LEAST_BACK_RAD = 1e-6;

/**
 * The particles' brightness over a Lambert sphere's at phase angle α: the slab's power law
 * (`powerLawPhase`) over {@link lambertSpherePhase}, finite to α = π, where both fall to 0 and their
 * ratio as u^0.09 with u = π − α, taken at π − {@link LEAST_BACK_RAD} at most.
 */
export function particlePhaseRatio(alphaRad: number): number {
  const a = Math.min(Math.max(alphaRad, 0), Math.PI - LEAST_BACK_RAD);
  return powerLawPhase(a) / lambertSpherePhase(a);
}

/**
 * The particles' light: towards the ring's brightest star from the camera's foot on the plane, and
 * its face-on illuminance there per channel times the planet's shadow (`ringPlanetVisible`),
 * exposure ÷ π and the power law's phase over the Lambert sphere's, with the meter class `other`.
 */
function lightUniforms(
  ring: ViewRing,
  planet: LitBodyInput,
  stars: NonNullable<ReturnType<BodyFramePlan["sources"]["get"]>>["stars"],
  radiusM: number,
  input: RingParticlesFrameInput,
): Readonly<Record<string, Float32Array>> {
  const record = ringSlabRecordOf(ring, planet, stars, [], {
    annuli: input.annuli,
    exposureScale: input.exposureScale,
  });
  const light = record.lights[0];
  if (light === undefined) {
    return { sun: new Float32Array(4), light: Float32Array.of(0, 0, 0, METER_CLASS.other) };
  }
  // The camera's foot on the plane, from the planet's centre ÷ a.
  const a = planet.figure.equatorialRadiusM;
  const n = normalise(ring.normal);
  const camera = scale(planet.centreM, -1 / a);
  const foot = sub(camera, scale(n, dot(camera, n)));
  const footAtRadius = norm(foot) > 0 ? scale(foot, radiusM / a / norm(foot)) : foot;
  const visible = ringPlanetVisible(record, light, footAtRadius);
  const towards = normalise(sub(scale(light.direction, light.distance), footAtRadius));
  // The particles' phase is the slab's power law, not a Lambert sphere's: their ratio at the view's
  // phase angle, between the star and the camera along the view's axis.
  const back = scale(viewAxisOf(input.viewRotation), -1);
  const alpha = Math.acos(Math.min(Math.max(dot(towards, back), -1), 1));
  const k = record.exposureOverPi * particlePhaseRatio(alpha);
  return {
    sun: Float32Array.of(towards.x, towards.y, towards.z, 0),
    light: Float32Array.of(
      light.illuminance[0] * visible[0] * k,
      light.illuminance[1] * visible[1] * k,
      light.illuminance[2] * visible[2] * k,
      METER_CLASS.other,
    ),
  };
}
