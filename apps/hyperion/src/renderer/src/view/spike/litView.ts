/**
 * The descent spike's lit view (plan R05, R05.T11.c, Design note 17): the terrain drawn lit into an
 * `rgba16float` target of the spike's own, with pre-exposure, then mapped to the display by R02's
 * `agx` in one full-screen pass of its own under a `MAN` exposure of EV100 15.
 *
 * @remarks
 * It is a spike scene inside R02's `VIEW`, not a render style: R07 owns the photorealistic style,
 * its scene target (decisions-r06-r07.md item 1), its histogram, bloom and tone mapping, and
 * replaces this view's shading, exposure and pass with its own. The target here is the spike's
 * measurement scratch, sized to the view and never shared.
 *
 * With an atmosphere (T13.b), the terrain target's colour and depth are laid under the sky by
 * Hillaire's composite (T12.c), drawn into a second target, `<name>:spike-sky`, since a pass may
 * not sample the depth of the target it draws into (R01's `DepthSelfSample`); the display pass
 * then maps that one.
 */

import {
  controlEv100,
  type ExposureControl,
  exposureScale,
  setManual,
} from "../photometry/exposure";
import type { ExposureTriple } from "../photometry/exposure";
import type {
  DrawItem,
  FrameSubmission,
  MeshHandle,
  RenderEngine,
  RenderTarget,
  RenderTargetFormat,
  ViewSize,
  WgslMaterialSpec,
} from "../engine/types";
import frameWgsl from "../shaders/frame.wgsl?raw";
import toneCurveWgsl from "../shaders/toneCurve.wgsl?raw";
import { TERRAIN_PASS_LABEL } from "../terrain/gpu/material";
import litAgxWgsl from "./litAgx.wgsl?raw";

/**
 * The spike's daylight exposure, EV100 15 (Design note 17): f/16 at 1/128 s and ISO 100, since
 * EV100 = log₂(N² ÷ t) + log₂(100 ÷ S) = log₂(256 × 128) = 15 (ISO 2720:1974's exposure value).
 */
export const SPIKE_DAY_TRIPLE: ExposureTriple = { aperture: 16, shutterS: 1 / 128, iso: 100 };

/** The pass's label for the display pass in `PassTimes`. */
export const LIT_VIEW_DISPLAY_LABEL = "spike display";

/** The pass's label for the atmosphere's composite in `PassTimes`. */
export const LIT_VIEW_ATMOSPHERE_LABEL = "atmosphere composite";

/** What the atmosphere's composite is laid over: the terrain target's colour and depth. */
export interface LitScene {
  readonly colour: RenderTarget["colour"];
  readonly depth: NonNullable<RenderTarget["depth"]>;
}

/** Makes the frame's atmosphere composite over the terrain target (`HillaireAtmosphere`'s). */
export type LitComposite = (scene: LitScene) => DrawItem;

/** The full-screen AgX material, drawing into a view's canvas or, in the harness, an HDR target. */
export const LIT_AGX_MATERIAL: WgslMaterialSpec = {
  name: "spike-agx",
  displayName: "SPIKE DISPLAY",
  vertexWgsl: frameWgsl + toneCurveWgsl + litAgxWgsl,
  fragmentWgsl: frameWgsl + toneCurveWgsl + litAgxWgsl,
  uniforms: [],
  samplers: [{ name: "hdrSampler", filter: "linear", address: "clamp-to-edge", binding: 1 }],
  textures: [{ name: "hdrColour", binding: 0 }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The render size for a presented `size`: `renderHeightPx` tall at its aspect, or itself. */
export function renderSizeOf(size: ViewSize, renderHeightPx: number | null): ViewSize {
  if (renderHeightPx === null || renderHeightPx >= size.heightPx) {
    return size;
  }
  return {
    widthPx: Math.max(1, Math.round((size.widthPx * renderHeightPx) / size.heightPx)),
    heightPx: renderHeightPx,
  };
}

/** What the lit view draws its display pass into: a view, or the harness's target. */
export interface LitOutput {
  render(frame: FrameSubmission): void;
}

/** The spike's `MAN` exposure at {@link SPIKE_DAY_TRIPLE}, through R02's `setManual`. */
export function spikeExposure(triple: ExposureTriple = SPIKE_DAY_TRIPLE): ExposureControl {
  const result = setManual(triple);
  if (result.kind !== "accepted") {
    throw new Error(`the spike's exposure triple was refused: ${result.reason}`);
  }
  return result.control;
}

/** The terrain target's colour and depth. */
function sceneOf(target: RenderTarget): LitScene {
  const depth = target.depth;
  if (depth === null) {
    throw new Error(`the lit view's target ${target.name} was made without its depth`);
  }
  return { colour: target.colour, depth };
}

/** One view's lit drawing: its HDR target, the display pass and the exposure. */
export class LitView {
  readonly #engine: RenderEngine;
  readonly #mesh: MeshHandle;
  readonly #target: RenderTarget;
  /** The sky's target, with an atmosphere; `null` without. */
  readonly #sky: RenderTarget | null;
  /** The terrain target's handles, renewed when a resize remakes its textures. */
  #scene: LitScene;
  /** The display pass's texture, likewise. */
  #textures: Readonly<{ hdrColour: RenderTarget["colour"] }>;
  readonly #renderHeightPx: number | null;
  #renderSize: ViewSize;
  #exposure: ExposureControl;
  #display: DrawItem | null = null;

  /**
   * @param name - The view's name; the target is `<name>:spike-hdr`.
   * @param size - The view's presented size in device pixels.
   * @param renderHeightPx - The setting's render height (`TerrainSettings.renderHeightPx`): 720 on
   * the low setting, drawn at the view's aspect and presented upscaled; `null` for the view's own.
   * @param atmosphere - Whether frames lay an atmosphere's composite over the terrain, into a
   * second target the display pass then maps.
   */
  constructor(
    engine: RenderEngine,
    name: string,
    size: ViewSize,
    renderHeightPx: number | null,
    atmosphere = false,
  ) {
    this.#engine = engine;
    this.#renderHeightPx = renderHeightPx;
    this.#renderSize = renderSizeOf(size, renderHeightPx);
    this.#exposure = spikeExposure();
    this.#mesh = engine.createMesh({
      name: `${name}:spike-display`,
      positions: new Float32Array([-1, -1, 0, 3, -1, 0, -1, 3, 0]),
      indices: null,
      topology: "triangle-list",
      attributes: {},
    });
    this.#target = engine.createRenderTarget({
      name: `${name}:spike-hdr`,
      size: this.#renderSize,
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    this.#scene = sceneOf(this.#target);
    this.#sky = atmosphere
      ? engine.createRenderTarget({
          name: `${name}:spike-sky`,
          size: this.#renderSize,
          format: "rgba16float",
          mips: 1,
          depth: false,
          category: "render-targets",
        })
      : null;
    this.#textures = { hdrColour: (this.#sky ?? this.#target).colour };
  }

  /** The exposure the label reads, `EV100 15.0 MAN` by day. */
  get exposure(): ExposureControl {
    return this.#exposure;
  }

  /** The size the terrain is drawn at, which selection's view takes as its viewport (DN26). */
  get renderSize(): ViewSize {
    return this.#renderSize;
  }

  /** The pre-exposure the terrain pass shades with (R02's `exposureScale`). */
  get exposureScale(): number {
    return exposureScale(controlEv100(this.#exposure));
  }

  /** Sets the exposure, per segment of the script (Design note 17). */
  setExposure(triple: ExposureTriple): void {
    this.#exposure = spikeExposure(triple);
  }

  /** Compiles the display pass for the outputs it draws into. */
  async ready(outputs: ReadonlyArray<RenderTargetFormat> = ["canvas"]): Promise<void> {
    const material = await this.#engine.createMaterialAsync(LIT_AGX_MATERIAL, outputs, [
      this.#mesh,
    ]);
    this.#display = {
      mesh: this.#mesh,
      material,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: this.#textures,
    };
  }

  /** Resizes the HDR target with the view's presented size. */
  resize(size: ViewSize): void {
    this.#renderSize = renderSizeOf(size, this.#renderHeightPx);
    this.#target.resize(this.#renderSize);
    this.#sky?.resize(this.#renderSize);
    // A resize remakes a target's textures, and with them their handles.
    this.#scene = sceneOf(this.#target);
    this.#textures = { hdrColour: (this.#sky ?? this.#target).colour };
    if (this.#display !== null) {
      this.#display = { ...this.#display, textures: this.#textures };
    }
  }

  /**
   * Draws `draws` (the terrain pass's) lit into the HDR target, then, with an atmosphere,
   * `composite`'s draw over it into the sky's target, then the display pass into `output`; the
   * display pass is left out until {@link ready} resolves.
   *
   * @throws Error if a view made with an atmosphere is given no composite, or one without is.
   */
  render(
    output: LitOutput,
    viewRotation: Float32Array,
    projection: Float32Array,
    draws: ReadonlyArray<DrawItem>,
    composite: LitComposite | null = null,
  ): void {
    if ((composite === null) !== (this.#sky === null)) {
      throw new Error("a lit view draws a composite exactly when it was made with an atmosphere");
    }
    this.#target.render({
      label: TERRAIN_PASS_LABEL,
      viewRotation,
      projection,
      draws,
      postProcesses: [],
    });
    if (this.#sky !== null && composite !== null) {
      this.#sky.render({
        label: LIT_VIEW_ATMOSPHERE_LABEL,
        viewRotation,
        projection,
        draws: [composite(this.#scene)],
        postProcesses: [],
      });
    }
    output.render({
      label: LIT_VIEW_DISPLAY_LABEL,
      viewRotation,
      projection,
      draws: this.#display === null ? [] : [this.#display],
      postProcesses: [],
    });
  }

  /** The HDR target, for the harness's readback and T13.b's atmosphere composite. */
  get target(): RenderTarget {
    return this.#target;
  }

  /** Disposes the targets. */
  dispose(): void {
    this.#target.dispose();
    this.#sky?.dispose();
  }
}
