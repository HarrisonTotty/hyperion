/**
 * The HDR twin of R02's star sprites, for the photorealistic style's scene target (plan R06,
 * T13.c; decision record item 1 of 2026-10-02).
 *
 * @remarks
 * It draws R07's point bodies too (decision record R07.T8.a, item 2), hence its name. The same
 * `starSprite.wgsl`, composed with an identity `agxSprite` in place of
 * `toneCurve.wgsl`'s, so the tone step is compiled out and each pixel writes its pre-exposed
 * linear light into an `rgba16float` target, which R07's tone-mapping pass tones once for the
 * whole image. R01's additive blend keeps the destination's alpha, R07's meter class. The
 * wireframe keeps R02's material, toned per sprite, unchanged.
 */

import { MATERIAL_BUFFER, WIREFRAME_MATERIALS } from "../wireframe/submit";
import frameWgsl from "../shaders/frame.wgsl?raw";
import starSpriteWgsl from "../shaders/starSprite.wgsl?raw";
import type { WgslMaterialSpec } from "../engine/types";

/** `agxSprite` as the identity: the HDR target holds linear light, toned later by R07. */
const LINEAR_SPRITE_WGSL = `
// The HDR twin's tone step, compiled out: the sprite writes its pre-exposed linear light.
fn agxSprite(rgbLinear: vec3f) -> vec3f {
  return rgbLinear;
}
`;

/** The star sprites into an HDR scene target, linear and untoned. */
export const SKY_SPRITE_HDR_MATERIAL: WgslMaterialSpec = {
  ...WIREFRAME_MATERIALS.starSprite,
  name: "sky:starSpriteHdr",
  displayName: "POINT SPRITES HDR",
  vertexWgsl: frameWgsl + LINEAR_SPRITE_WGSL + starSpriteWgsl,
  fragmentWgsl: frameWgsl + LINEAR_SPRITE_WGSL + starSpriteWgsl,
  storageBuffers: [{ name: MATERIAL_BUFFER.starSprite, binding: 0 }],
};
