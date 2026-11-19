/**
 * The shadow map's rows' count (plan R11, R11.T7.c; `cloudShadowMap.ts`), in a module of its own so
 * that the layer's state (`cloudLayer.ts`), which makes the map that holds nothing, imports no
 * shader of the volume's.
 */
export const CLOUD_SHADOW_ROWS = 9;
