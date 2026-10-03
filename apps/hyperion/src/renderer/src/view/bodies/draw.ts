/**
 * Lit bodies as points and discs in the photorealistic style's HDR target (plan R07, T8.a; Design
 * notes 1, 2, 4, 6, 10 and 19).
 *
 * @remarks
 * {@link planLitBodies} is pure: each body's regime (`litRegimes`), the painter's sequence by power
 * (`painterOrder`), and per body the stars that light it (the brightest two past
 * `STAR_CUT_RELATIVE`), their illuminance at it (`starIlluminance`), their annuli per channel
 * (`annulusEdges`) and the bodies that may eclipse them (`occludersFor`, the two largest). A disc
 * becomes a {@link DiscRecord}; a point a sprite of flux F = E p (a ÷ Δ)² Φ(α) per channel ({@link
 * pointFlux}, the law integrated over the figure for a spheroid), cut by the same eclipse term from
 * the body's centre, laid into R02's point-spread sprite. Host discs keep their
 * place in the order, where R06.T13.e's pass draws them (decision-r07-t8a, R06 coordination (d),
 * 2026-10-03): a host's step places R06's disc draw. {@link LitBodyRenderer} binds the plan to the
 * engine: one draw per disc for its wholly covered pixels and one for its limb, one sprite draw per
 * run of consecutive points through R06's HDR sprite (`POINT SPRITES HDR`).
 */
import type { BodyIdHex, HostDiscDto } from "@hyperion/protocol";

import { dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { BodyFigure, BodyPhotometry } from "../appearance/fromWire";
import {
  PHASE_TABLE_SAMPLES,
  phaseFactorFromTable,
  phaseFactorTableOf,
  type PhotometricLaw,
} from "../appearance/law";
import { packPhaseFactorRows } from "../appearance/litBodyProbe";
import { discIntegratedPhase, geometricAlbedo } from "../appearance/phase";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type {
  BufferHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { annulusEdges, type AnnulusSet, eclipseVisible } from "../lighting/annuli";
import type { PlacedLight } from "../lighting/hostLights";
import { shiningStars, starIlluminance, photopicIlluminance } from "../lighting/illuminance";
import { type LightingBody, type LightingSphere, occludersFor } from "../lighting/occluders";
import { PSF_QUAD_PX } from "../photometry/magnitude";
import { type SpriteRecord, spriteRecord } from "../wireframe/drawList";
import type { Rgb } from "../photometry/toneCurve";
import frameWgsl from "../shaders/frame.wgsl?raw";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import bodyDiscWgsl from "../shaders/bodyDisc.wgsl?raw";
import { sphereScreenRect, WIREFRAME_MESHES } from "../wireframe/submit";
import {
  type DiscLight,
  type DiscOccluder,
  type DiscRecord,
  LIMB_SAMPLES,
  MAX_DISC_LIGHTS,
  MAX_DISC_OCCLUDERS,
  packDiscRecords,
  SMALL_DISC_PX,
  SMALL_DISC_SAMPLES,
} from "./discShading";
import { oblateAlbedoScale, spheroidGeometricIntegral } from "./oblate";
import { type HostSphere, type PainterEntry, painterOrder } from "./painter";
import { type LitRegime, type LitSphere, litRegimes } from "./regime";
import { angularDiameterPx } from "../wireframe/bodies";

/** A lit body of the frame. */
export interface LitBodyInput {
  readonly id: BodyIdHex;
  /** Its centre from the camera, m along the galactic axes, in `f64`. */
  readonly centreM: Vec3;
  readonly figure: BodyFigure;
  readonly photometry: BodyPhotometry;
}

/** What the plan needs of the view. */
export interface BodyFrameOptions {
  readonly camera: ProjectionCamera;
  readonly viewport: Viewport;
  /** The exposure scale the target is pre-exposed with (`exposureScale`), 1 ÷ (cd/m²). */
  readonly exposureScale: number;
  /** The eclipse term's annuli: `DISC_ANNULI_HIGH` or `DISC_ANNULI_LOW`. */
  readonly annuli: number;
}

/** A point body's sprite: R02's sprite record, its depth its own (decision-r07-t8a, item 2). */
export interface PointSprite {
  readonly id: BodyIdHex;
  readonly record: SpriteRecord;
}

/** One step of the painter's sequence as it is drawn. */
export type BodyStep =
  | { readonly kind: "disc"; readonly index: number }
  | { readonly kind: "points"; readonly sprites: ReadonlyArray<PointSprite> }
  /** A host star's place in the order, where R06's disc pass draws it (`HostDiscDto.star`). */
  | { readonly kind: "host"; readonly star: number };

/** One frame's lit bodies, ready to bind. */
export interface BodyFramePlan {
  readonly regimes: ReadonlyMap<BodyIdHex, LitRegime>;
  readonly order: ReadonlyArray<PainterEntry>;
  readonly discs: ReadonlyArray<DiscRecord>;
  /** The phase table's rows, the frame's distinct laws in first use. */
  readonly laws: ReadonlyArray<PhotometricLaw>;
  readonly steps: ReadonlyArray<BodyStep>;
}

/** The pole a body of unknown rotation is drawn about: a sphere needs none. */
const DEFAULT_POLE = vec3(0, 0, 1);

/** A body whose centre is within this fraction of its radius is seen from inside, and not drawn. */
const INSIDE_MARGIN = 1e-9;

/** Each host's annuli per channel and K, made once per disc: the construction bisects. */
const ANNULI = new WeakMap<
  HostDiscDto,
  Map<number, readonly [AnnulusSet, AnnulusSet, AnnulusSet]>
>();

/** A host's annuli in display order (r, g, b) from R06's B, V, R limb laws. */
export function hostAnnuli(
  disc: HostDiscDto,
  k: number,
): readonly [AnnulusSet, AnnulusSet, AnnulusSet] {
  let byK = ANNULI.get(disc);
  if (byK === undefined) {
    byK = new Map();
    ANNULI.set(disc, byK);
  }
  const cached = byK.get(k);
  if (cached !== undefined) {
    return cached;
  }
  const [b, v, r] = disc.limb;
  const sets = [
    annulusEdges(r.c, r.alpha, k),
    annulusEdges(v.c, v.alpha, k),
    annulusEdges(b.c, b.alpha, k),
  ] as const;
  byK.set(k, sets);
  return sets;
}

/** One star as it lights one body. */
interface BodyLight {
  readonly host: PlacedLight;
  /** From the body's centre to the star's, m. */
  readonly toStarM: Vec3;
  readonly illuminance: Rgb;
}

/** The stars that light a body: past the cut, the brightest {@link MAX_DISC_LIGHTS}. */
function lightsOf(body: LitBodyInput, hosts: ReadonlyArray<PlacedLight>): BodyLight[] {
  const all = hosts.map((host) => {
    const toStarM = sub(host.centreM, body.centreM);
    return { host, toStarM, illuminance: starIlluminance(host.disc, norm(toStarM)) };
  });
  const kept = shiningStars(all.map((light) => light.illuminance)).flatMap((index) => {
    const light = all[index];
    return light === undefined ? [] : [light];
  });
  return kept
    .toSorted((a, b) => photopicIlluminance(b.illuminance) - photopicIlluminance(a.illuminance))
    .slice(0, MAX_DISC_LIGHTS);
}

/** The bodies that may eclipse a body's lights, the largest seen from it first, at most two. */
function occludersOf(
  body: LitBodyInput,
  lights: ReadonlyArray<BodyLight>,
  lighting: ReadonlyArray<LightingBody>,
): LightingBody[] {
  const self: LightingBody = {
    id: body.id,
    centreM: body.centreM,
    radiusM: body.figure.equatorialRadiusM,
  };
  const stars: LightingSphere[] = lights.map((light) => ({
    centreM: light.host.centreM,
    radiusM: light.host.disc.radius_m,
  }));
  const angular = (other: LightingBody): number =>
    other.radiusM / norm(sub(other.centreM, body.centreM));
  return occludersFor(self, stars, lighting)
    .toSorted((a, b) => angular(b) - angular(a) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
    .slice(0, MAX_DISC_OCCLUDERS);
}

/** The body's pole, or +z for a body whose rotation is not known (drawn as a sphere). */
function poleOf(figure: BodyFigure): Vec3 {
  return figure.pole === null ? DEFAULT_POLE : normalise(figure.pole);
}

/**
 * A point body's flux at the camera per display channel, lx, each star's cut by the eclipse term
 * from the body's centre: E p (a ÷ Δ)² Φ(α) for a sphere; for a spheroid the law integrated over
 * its figure, (E ÷ π)(a ÷ Δ)² A′ f(α) K (`spheroidGeometricIntegral`, A′ the disc's scaled A), so
 * that the point and the disc draw one flux.
 *
 * @param occluders - The bodies that may eclipse its stars (`occludersFor`).
 * @param k - The eclipse term's annuli.
 */
export function pointFlux(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
  occluders: ReadonlyArray<LightingBody>,
  k: number,
): Rgb {
  const { equatorialRadiusM: a, polarRadiusM: c } = body.figure;
  const { law } = body.photometry;
  const distanceM = norm(body.centreM);
  const toCamera = scale(body.centreM, -1 / distanceM);
  const solid = (a / distanceM) ** 2;
  const p = geometricAlbedo(law);
  const albedo = oblateAlbedoScale(law.lommelSeeligerShare, c / a);
  const blockers = occluders.map(({ centreM, radiusM }) => ({ centreM, radiusM }));
  const flux: [number, number, number] = [0, 0, 0];
  for (const light of lightsOf(body, hosts)) {
    const towards = normalise(light.toStarM);
    const alpha = Math.acos(Math.min(1, Math.max(-1, dot(towards, toCamera))));
    let reflected: Rgb;
    if (c >= a) {
      const phase = discIntegratedPhase(law, alpha);
      reflected = [p[0] * phase[0], p[1] * phase[1], p[2] * phase[2]];
    } else {
      const f = phaseFactorFromTable(phaseFactorTableOf(law), alpha);
      const geometric =
        spheroidGeometricIntegral(
          law.lommelSeeligerShare,
          c / a,
          poleOf(body.figure),
          towards,
          toCamera,
        ) / Math.PI;
      reflected = [
        law.a[0] * albedo * f[0] * geometric,
        law.a[1] * albedo * f[1] * geometric,
        law.a[2] * albedo * f[2] * geometric,
      ];
    }
    const [bLaw, vLaw, rLaw] = light.host.disc.limb;
    const laws = [rLaw, vLaw, bLaw] as const;
    for (const ch of [0, 1, 2] as const) {
      const visible =
        blockers.length === 0
          ? 1
          : eclipseVisible(
              {
                centreM: light.host.centreM,
                radiusM: light.host.disc.radius_m,
                limbC: laws[ch].c,
                limbAlpha: laws[ch].alpha,
              },
              body.centreM,
              blockers,
              k,
            );
      flux[ch] += light.illuminance[ch] * solid * reflected[ch] * visible;
    }
  }
  return flux;
}

/** The disc record of a body drawn as a disc, or `null` where it is off the view or about the camera. */
function discRecordOf(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
  occluders: ReadonlyArray<LightingBody>,
  tableRow: number,
  options: BodyFrameOptions,
): DiscRecord | null {
  const { equatorialRadiusM: a, polarRadiusM: c } = body.figure;
  const distanceM = norm(body.centreM);
  if (!(distanceM > a * (1 + INSIDE_MARGIN))) {
    return null;
  }
  const rect = sphereScreenRect(body.centreM, a, options.camera, options.viewport);
  if (rect === null) {
    return null;
  }
  const direction = scale(body.centreM, 1 / distanceM);
  const pole = poleOf(body.figure);
  const aOverD = a / distanceM;
  const small =
    angularDiameterPx(body.centreM, a, options.camera, options.viewport) < SMALL_DISC_PX;
  const lights: DiscLight[] = lightsOf(body, hosts).map((light) => {
    const d = norm(light.toStarM);
    return {
      direction: scale(light.toStarM, 1 / d),
      distance: d / a,
      radius: light.host.disc.radius_m / a,
      illuminance: light.illuminance,
      annuli: hostAnnuli(light.host.disc, options.annuli),
    };
  });
  const blockers: DiscOccluder[] = occluders.map((occluder) => ({
    centre: scale(sub(occluder.centreM, body.centreM), 1 / a),
    radius: occluder.radiusM / a,
  }));
  return {
    body: body.id,
    rect,
    direction,
    radiusOverDistance: aOverD,
    pole,
    polarOverEquatorial: c / a,
    interiorSamples: small ? SMALL_DISC_SAMPLES : 1,
    limbSamples: small ? SMALL_DISC_SAMPLES : LIMB_SAMPLES,
    law: body.photometry.law,
    albedoScale: oblateAlbedoScale(body.photometry.law.lommelSeeligerShare, c / a),
    tableRow,
    exposureOverPi: options.exposureScale / Math.PI,
    lights,
    occluders: blockers,
  };
}

/** How far outside the view a point's sprite may fall and still light it: half its quad, px. */
const SPRITE_MARGIN_PX = Math.ceil(PSF_QUAD_PX / 2);

/** A point body's sprite, or `null` where it falls off the view or behind the camera. */
function pointSpriteOf(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
  occluders: ReadonlyArray<LightingBody>,
  options: BodyFrameOptions,
): PointSprite | null {
  const { camera, viewport } = options;
  const p = project(body.centreM, camera, viewport);
  if (
    !p.inFront ||
    p.xPx < -SPRITE_MARGIN_PX ||
    p.yPx < -SPRITE_MARGIN_PX ||
    p.xPx > viewport.widthPx + SPRITE_MARGIN_PX ||
    p.yPx > viewport.heightPx + SPRITE_MARGIN_PX
  ) {
    return null;
  }
  const flux = pointFlux(body, hosts, occluders, options.annuli);
  return {
    id: body.id,
    record: spriteRecord(
      { xPx: p.xPx, yPx: p.yPx, depth: p.depth },
      flux,
      options.exposureScale,
      body.centreM,
      camera,
      viewport,
    ),
  };
}

/**
 * The frame's lit bodies: regimes, the painter's sequence, disc records and point sprites.
 *
 * @param hosts - The host stars, which light the bodies and keep their place in the order.
 * @param previous - Each body's regime on the previous frame, for the hysteresis.
 */
export function planLitBodies(
  bodies: ReadonlyArray<LitBodyInput>,
  hosts: ReadonlyArray<PlacedLight>,
  options: BodyFrameOptions,
  previous: ReadonlyMap<BodyIdHex, LitRegime>,
): BodyFramePlan {
  const spheres: LitSphere[] = bodies.map((body) => ({
    id: body.id,
    centreM: body.centreM,
    radiusM: body.figure.equatorialRadiusM,
  }));
  const regimes = litRegimes(spheres, options.camera, options.viewport, previous);
  const hostSpheres: HostSphere[] = hosts.map((host) => ({
    star: host.disc.star,
    centreM: host.centreM,
    radiusM: host.disc.radius_m,
  }));
  const order = painterOrder(spheres, regimes, hostSpheres);
  const byId = new Map(bodies.map((body) => [body.id, body]));
  const lighting: LightingBody[] = spheres.map(({ id, centreM, radiusM }) => ({
    id,
    centreM,
    radiusM,
  }));
  const laws: PhotometricLaw[] = [];
  const discs: DiscRecord[] = [];
  const steps: BodyStep[] = [];
  let run: PointSprite[] = [];
  const closeRun = (): void => {
    if (run.length > 0) {
      steps.push({ kind: "points", sprites: run });
      run = [];
    }
  };
  for (const entry of order) {
    if (entry.kind === "host") {
      closeRun();
      steps.push({ kind: "host", star: entry.star });
      continue;
    }
    const body = byId.get(entry.body);
    if (body === undefined) {
      continue;
    }
    const occluders = occludersOf(body, lightsOf(body, hosts), lighting);
    if (regimes.get(body.id) === "point") {
      const sprite = pointSpriteOf(body, hosts, occluders, options);
      if (sprite !== null) {
        run.push(sprite);
      }
      continue;
    }
    let row = laws.indexOf(body.photometry.law);
    if (row < 0) {
      row = laws.length;
      laws.push(body.photometry.law);
    }
    const record = discRecordOf(body, hosts, occluders, row, options);
    if (record === null) {
      continue;
    }
    closeRun();
    steps.push({ kind: "disc", index: discs.length });
    discs.push(record);
  }
  closeRun();
  return { regimes, order, discs, laws, steps };
}

const DISC_SOURCE = frameWgsl + litBodyWgsl + bodyDiscWgsl;

/** The disc's two materials: its wholly covered pixels, opaque, and its limb, premultiplied. */
export const BODY_DISC_MATERIALS: Readonly<Record<"interior" | "limb", WgslMaterialSpec>> = {
  interior: {
    name: "bodies:disc",
    displayName: "BODY DISCS",
    vertexWgsl: DISC_SOURCE,
    fragmentWgsl: DISC_SOURCE,
    uniforms: [
      { name: "disc", type: "u32" },
      { name: "edgePass", type: "u32" },
    ],
    samplers: [],
    textures: [{ name: "phaseFactorTable", binding: 1, sampleType: "unfilterable-float" }],
    storageBuffers: [{ name: "discs", binding: 0 }],
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: "none",
  },
  limb: {
    name: "bodies:discLimb",
    displayName: "BODY DISC LIMBS",
    vertexWgsl: DISC_SOURCE,
    fragmentWgsl: DISC_SOURCE,
    uniforms: [
      { name: "disc", type: "u32" },
      { name: "edgePass", type: "u32" },
    ],
    samplers: [],
    textures: [{ name: "phaseFactorTable", binding: 1, sampleType: "unfilterable-float" }],
    storageBuffers: [{ name: "discs", binding: 0 }],
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: "premultiplied",
  },
};

/** The smallest buffer made, bytes; each grows by doubling. */
const MIN_BUFFER_BYTES = 4_096;

/** The part of the engine the renderer uses. */
export type LitBodyEngine = Pick<
  RenderEngine,
  | "createMaterial"
  | "createMesh"
  | "createBuffer"
  | "createTexture"
  | "writeBuffer"
  | "writeTexture"
  | "releaseBuffer"
  | "releaseTexture"
  | "onRestored"
>;

interface DeviceResources {
  readonly interior: MaterialHandle;
  readonly limb: MaterialHandle;
  readonly sprite: MaterialHandle;
  readonly quad: MeshHandle;
  discs: BufferHandle;
  /** One sprite buffer per run of points in the frame, reused frame to frame. */
  readonly sprites: BufferHandle[];
  table: TextureHandle;
  tableRows: number;
}

/**
 * Binds a frame's lit-body plan to the engine (plan R07, T8.a).
 *
 * @remarks
 * It makes its materials, the quad, the disc buffer and the phase table on construction and again
 * after a device loss; buffers grow by doubling and the table by rows. Each disc is two draws, its
 * wholly covered pixels and then its limb; each run of points one draw of `spriteMaterial`, an HDR
 * twin of R02's star sprite reading `sprites` at binding 0.
 */
export class LitBodyRenderer {
  readonly #engine: LitBodyEngine;
  readonly #spriteMaterial: WgslMaterialSpec;
  #resources: DeviceResources;
  #tableLaws: ReadonlyArray<PhotometricLaw> = [];
  readonly #unsubscribe: () => void;

  constructor(engine: LitBodyEngine, spriteMaterial: WgslMaterialSpec) {
    this.#engine = engine;
    this.#spriteMaterial = spriteMaterial;
    this.#resources = this.#create();
    this.#unsubscribe = engine.onRestored(() => {
      this.#resources = this.#create();
      this.#tableLaws = [];
    });
  }

  #create(): DeviceResources {
    const engine = this.#engine;
    return {
      interior: engine.createMaterial(BODY_DISC_MATERIALS.interior),
      limb: engine.createMaterial(BODY_DISC_MATERIALS.limb),
      sprite: engine.createMaterial(this.#spriteMaterial),
      quad: engine.createMesh({ ...WIREFRAME_MESHES.quad, name: "bodies:quad" }),
      discs: this.#buffer("bodies:discs", MIN_BUFFER_BYTES),
      sprites: [],
      table: this.#table(1),
      tableRows: 1,
    };
  }

  #buffer(name: string, bytes: number): BufferHandle {
    return this.#engine.createBuffer({
      name,
      bytes,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
  }

  #table(rows: number): TextureHandle {
    return this.#engine.createTexture({
      name: "bodies:phase table",
      size: { width: PHASE_TABLE_SAMPLES, height: rows },
      dimension: "2d",
      format: "rgba32float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "other",
    });
  }

  /** Writes `data` into a buffer, replacing it by a doubled one if it is too small. */
  #write(handle: BufferHandle, data: Float32Array): BufferHandle {
    let target = handle;
    if (data.byteLength > handle.bytes) {
      let bytes = handle.bytes;
      while (bytes < data.byteLength) {
        bytes *= 2;
      }
      this.#engine.releaseBuffer(handle);
      target = this.#buffer(handle.name, bytes);
    }
    if (data.byteLength > 0) {
      this.#engine.writeBuffer(target, 0, data);
    }
    return target;
  }

  #writeTable(laws: ReadonlyArray<PhotometricLaw>): void {
    const same =
      laws.length === this.#tableLaws.length && laws.every((law, i) => law === this.#tableLaws[i]);
    if (same || laws.length === 0) {
      return;
    }
    const resources = this.#resources;
    if (laws.length > resources.tableRows) {
      this.#engine.releaseTexture(resources.table);
      resources.table = this.#table(laws.length);
      resources.tableRows = laws.length;
    }
    this.#engine.writeTexture(
      resources.table,
      { x: 0, y: 0 },
      { width: PHASE_TABLE_SAMPLES, height: laws.length },
      packPhaseFactorRows(laws),
    );
    this.#tableLaws = laws;
  }

  /** A disc's two draws: its wholly covered pixels, then its limb. */
  #discDraws(index: number): DrawItem[] {
    const resources = this.#resources;
    return (
      [
        [resources.interior, 0],
        [resources.limb, 1],
      ] as const
    ).map(([material, edgePass]) => ({
      mesh: resources.quad,
      material,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {
        disc: new Float32Array([index]),
        edgePass: new Float32Array([edgePass]),
      },
      textures: { phaseFactorTable: resources.table },
      storageBuffers: { discs: resources.discs },
    }));
  }

  /** A run of points' one instanced draw, from the run's own buffer. */
  #pointDraw(sprites: ReadonlyArray<PointSprite>, run: number): DrawItem {
    const resources = this.#resources;
    const rows = new Float32Array(sprites.length * 8);
    sprites.forEach((sprite, i) => {
      rows.set(sprite.record, i * 8);
    });
    const existing =
      resources.sprites[run] ?? this.#buffer(`bodies:sprites ${run}`, MIN_BUFFER_BYTES);
    const buffer = this.#write(existing, rows);
    resources.sprites[run] = buffer;
    return {
      mesh: resources.quad,
      material: resources.sprite,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
      instanceCount: sprites.length,
      storageBuffers: { sprites: buffer },
    };
  }

  /**
   * The draws of a plan, in its painter's order, to add to the scene target's submission.
   *
   * @param hostDraws - R06's host-disc draws by star (`DiscFrame.draws`), placed at each host's
   *   entry in the order; a host with none (a sprite, or off the view) draws nothing there.
   */
  draws(
    plan: BodyFramePlan,
    hostDraws: ReadonlyMap<number, ReadonlyArray<DrawItem>> = new Map(),
  ): DrawItem[] {
    const resources = this.#resources;
    this.#writeTable(plan.laws);
    resources.discs = this.#write(resources.discs, packDiscRecords(plan.discs));
    const draws: DrawItem[] = [];
    let run = 0;
    for (const step of plan.steps) {
      switch (step.kind) {
        case "host":
          draws.push(...(hostDraws.get(step.star) ?? []));
          break;
        case "disc":
          draws.push(...this.#discDraws(step.index));
          break;
        case "points":
          draws.push(this.#pointDraw(step.sprites, run));
          run += 1;
          break;
      }
    }
    return draws;
  }

  /** Stops following device restores. The engine owns its resources' release. */
  dispose(): void {
    this.#unsubscribe();
  }
}
