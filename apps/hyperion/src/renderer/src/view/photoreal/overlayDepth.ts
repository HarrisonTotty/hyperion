/**
 * The symbology's terrain depth from the scene's (plan R14, R14.T1.e; `GeometrySettings.overlayDepth`
 * `scene-depth`): where the scene target's size is the canvas's, the photorealistic renderer draws
 * the scene's depth into the symbology's pass in place of the terrain's redraw, pushed by
 * `TERRAIN_DEPTH_FRACTION` as the terrain's depth material pushes it, so that no terrain is drawn
 * again for the marks' depth test.
 *
 * @remarks
 * The scene's depth holds every opaque pass's depth beside the terrain's: the mesh bodies' and the
 * reference surfaces', whose tessellated depth hides marks on their datum that the redraw, testing
 * them against the bodies' analytic occluder spheres alone, leaves in sight. R14.T1.e's smoke finds
 * a body's own lines beyond its survey so (`R14.T1.e every mark over the image ...`), so no setting
 * takes it: both take `cluster`. The survey edge pulls its own depth towards the camera
 * (`lineStroke.wgsl`), as it does over the redraw. Where the sizes differ, the low setting's 720
 * rows or an internal scale under 1, the pass draws the terrain's depth from the cluster path at
 * stride 1 (`TerrainPass`' overlay pass) instead. A stride above 1 in the camera pass (plan R14's
 * T3.c) would move the scene's terrain by more than the push, which is a second reason for
 * `cluster`.
 */

import FRAME_WGSL from "../shaders/frame.wgsl?raw";
import type {
  DrawItem,
  MaterialHandle,
  MeshHandle,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import OVERLAY_DEPTH_WGSL from "./overlayDepth.wgsl?raw";

const SOURCE = `${FRAME_WGSL}\n${OVERLAY_DEPTH_WGSL}`;

/** The scene's depth into the symbology's pass, a full-screen draw onto the canvas. */
export const OVERLAY_SCENE_DEPTH_MATERIAL: WgslMaterialSpec = {
  name: "overlay scene depth",
  displayName: "SYMBOLOGY DEPTH",
  vertexWgsl: SOURCE,
  fragmentWgsl: SOURCE,
  uniforms: [],
  samplers: [],
  textures: [{ name: "sceneDepth", binding: 0, sampleType: "depth" }],
  cullMode: "none",
  depthWrite: true,
  colourWrites: false,
  blend: "none",
};

/** No offset: the draw is the screen's. */
const NO_OFFSET = new Float32Array(3);

/** The draw of `sceneDepth` into the symbology's pass, on the full-screen `triangle`. */
export function overlaySceneDepthDraw(
  triangle: MeshHandle,
  material: MaterialHandle,
  sceneDepth: TextureHandle,
): DrawItem {
  return {
    mesh: triangle,
    material,
    offsetFromCameraM: NO_OFFSET,
    uniforms: {},
    textures: { sceneDepth },
  };
}

/** Whether two sizes, device px, are the same: the scene's depth may stand in only then. */
export function sizesAgree(
  a: { readonly widthPx: number; readonly heightPx: number },
  b: { readonly widthPx: number; readonly heightPx: number },
): boolean {
  return a.widthPx === b.widthPx && a.heightPx === b.heightPx;
}
