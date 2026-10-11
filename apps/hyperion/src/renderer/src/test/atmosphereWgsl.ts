/**
 * The static check on the atmosphere's assembled WGSL (plan R08, Design notes 18 and 19, R08.T6.g),
 * shared by `view/atmosphere/hillaire.test.ts` and `view/atmosphere/tables.test.ts`: no module
 * calls WGSL's builtin trigonometry, keeps one of R05's radius forms, or keeps sebh's texel remap.
 */

/**
 * What no atmosphere module may call or keep, each with the pattern that finds it in WGSL whose
 * comments are stripped ({@link wgslCode}).
 *
 * @remarks
 * - WGSL's `sin`, `cos`, `tan`, `asin`, `acos`, `atan` and `atan2`, which WGSL bounds only loosely
 *   (`view/shaders/preciseTrig.wgsl` serves instead).
 * - R05's radius forms, which formed heights, discriminants and table coordinates as differences
 *   of radii: `mediumAt(r)`, `distanceToTop`, `distanceToBottom`, `intersectsGround`, `maxDistance`,
 *   `shellIntersectsGround`, `shellRMuToUv`, `transmittanceRMuToUv`, `transmittanceUvToRMu`,
 *   `shellMultiScatteringRMuToUv`, `multiScatteringUvToRMu`, `multiScatteringRMuToUv`, the ray
 *   march's `heightAbove`, and the `Medium` uniform's `topRadiusM`.
 * - sebh's sub-texel remap, `subUvsToUnit` and `unitToSubUvs`, whose read did not invert its write.
 */
export const FORBIDDEN_IN_ATMOSPHERE_WGSL: ReadonlyArray<readonly [string, RegExp]> = [
  ["builtin trigonometry", /\b(sin|cos|tan|asin|acos|atan|atan2)\s*\(/u],
  ["mediumAt(r)", /\bmediumAt\s*\(/u],
  ["distanceToTop", /\bdistanceToTop\b/u],
  ["distanceToBottom", /\bdistanceToBottom\b/u],
  ["intersectsGround", /\bintersectsGround\b/u],
  ["maxDistance", /\bmaxDistance\s*\(/u],
  ["shellIntersectsGround", /\bshellIntersectsGround\b/u],
  ["shellRMuToUv", /\bshellRMuToUv\b/u],
  ["transmittanceRMuToUv", /\btransmittanceRMuToUv\b/u],
  ["transmittanceUvToRMu", /\btransmittanceUvToRMu\b/u],
  ["shellMultiScatteringRMuToUv", /\bshellMultiScatteringRMuToUv\b/u],
  ["multiScatteringUvToRMu", /\bmultiScatteringUvToRMu\b/u],
  ["multiScatteringRMuToUv", /\bmultiScatteringRMuToUv\b/u],
  ["heightAbove", /\bheightAbove\b/u],
  ["topRadiusM", /\btopRadiusM\b/u],
  ["subUvsToUnit", /\bsubUvsToUnit\b/u],
  ["unitToSubUvs", /\bunitToSubUvs\b/u],
];

/** `wgsl` without its line and block comments, as the compiler reads it. */
export function wgslCode(wgsl: string): string {
  return wgsl.replaceAll(/\/\*[\s\S]*?\*\//gu, " ").replaceAll(/\/\/[^\n]*/gu, " ");
}

/** The names of the forbidden forms `wgsl` holds, comments aside. */
export function forbiddenIn(wgsl: string): string[] {
  const code = wgslCode(wgsl);
  return FORBIDDEN_IN_ATMOSPHERE_WGSL.filter(([, pattern]) => pattern.test(code)).map(
    ([name]) => name,
  );
}
