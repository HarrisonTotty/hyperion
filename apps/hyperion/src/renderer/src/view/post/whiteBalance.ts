/**
 * The photorealistic style's white balance (plan R07; the owner's report, 2026-11-05): the white
 * its tone step maps to the display's.
 *
 * @remarks
 * `D65` maps the scene's light to sRGB as it is, its white the display's own (IEC 61966-2-1), as a
 * camera held at daylight balance does: a host star's light keeps its colour, so a 3,000 K M dwarf
 * paints everything it lights orange. `HOST` first adapts the light to the scene's brightest host
 * star's white, as a camera balanced to that light draws it: one 3 × 3 in linear Rec. 709, in the
 * tone step after the bloom and the glare and before the curve, so that the stars, the discs, the
 * air and the glare all take it. The adaptation is complete, a camera's: the eye adapts only in part
 * below about 3,000 K and keeps some of the light's warmth (Smet et al. 2017; Zhai and Luo 2018,
 * Optics Express; neither read in the pod), which no setting draws.
 *
 * The adaptation is the linear Bradford transform: von Kries's coefficient rule in Bradford's
 * sharpened cone space, adaptation complete (Lam 1985, PhD thesis, University of Bradford; the
 * "linear Bradford" of ICC.1:2010, Annex E, and of Adobe's DNG specification 1.4, chapter 6, which
 * adapts a camera's white with it; not read in the pod). The host's white goes to D65's at its own
 * luminance, so that a grey it lights stays at the luminance the meter exposed for. The host's
 * white is its disc's colour (`HostDiscDto.mean_luminance_cd_m2`), R06's from its model spectrum
 * (PHOENIX-ACES below 3,500 K, ATLAS9, TLUSTY for O stars, Koester's for white dwarfs:
 * `tables/star_colour.rs`), never a black body's.
 */
import type { HostDiscDto } from "@hyperion/protocol";

import type { PlacedLight } from "../lighting/hostLights";
import { photopicIlluminance, starIlluminance } from "../lighting/illuminance";
import {
  applyRows,
  inverseRows,
  type Rgb,
  type Rows3,
  SRGB_TO_XYZ,
  times,
  XYZ_TO_SRGB,
} from "../photometry/toneCurve";
import { norm } from "../../geometry/vec3";

/** A view's white balance: the display's own white, or the scene's brightest host star's. */
export type WhiteBalance = "d65" | "host";

/** The white balances in the control's order. */
export const WHITE_BALANCES: ReadonlyArray<WhiteBalance> = ["d65", "host"];

/**
 * The white balance a view opens at: `HOST`, the light adapted to its star as a camera balanced to
 * that light takes it, so that a body is drawn in its own colours (the UX ruling, 2026-11-06: every
 * view is a camera's picture).
 */
export const DEFAULT_WHITE_BALANCE: WhiteBalance = "host";

/** A white balance's name, as the exposure panel and the label block show it. */
export function whiteBalanceName(balance: WhiteBalance): string {
  let name: string;
  switch (balance) {
    case "d65":
      name = "D65";
      break;
    case "host":
      name = "HOST";
      break;
  }
  return name;
}

/**
 * Bradford's cone response, CIE XYZ to its sharpened LMS, as rows (Lam 1985; the linear Bradford
 * of ICC.1:2010, Annex E): the matrix as every source prints it, to four decimals.
 */
export const BRADFORD: Rows3 = [
  [0.8951, 0.2664, -0.1614],
  [-0.7502, 1.7135, 0.0367],
  [0.0389, -0.0685, 1.0296],
];

const BRADFORD_INVERSE = inverseRows(BRADFORD);

/** D65's white in CIE XYZ at Y = 1: Filament's `sRGB_to_XYZ` of linear white. */
const D65_XYZ = applyRows(SRGB_TO_XYZ, [1, 1, 1]);

/**
 * A host disc's light as a linear Rec. 709 colour of unit luminance: its mean luminance's channels
 * (B, V, R on the wire) as red, green and blue, over their luminance; white for a disc with no
 * light.
 */
export function discWhite(disc: HostDiscDto): Rgb {
  const [b, g, r] = disc.mean_luminance_cd_m2;
  const rgb: Rgb = [r, g, b];
  const y = applyRows(SRGB_TO_XYZ, rgb)[1];
  return y > 0 && Number.isFinite(y) ? [r / y, g / y, b / y] : [1, 1, 1];
}

/**
 * The linear Rec. 709 matrix, as rows, that adapts light of the white `white` (linear Rec. 709)
 * to D65's white of the same luminance by the linear Bradford transform, adaptation complete: so
 * that `white` itself maps to a neutral grey of its own luminance.
 *
 * @remarks
 * M = `XYZ_to_sRGB` · B⁻¹ · diag(ρ_D65 ÷ ρ_white) · B · `sRGB_to_XYZ`, with B {@link BRADFORD}
 * and ρ each white's cone response, D65's scaled to the white's Y. Filament's two sRGB matrices
 * are inverse to about 2 × 10⁻⁶, so D65's own white gives the identity to that, not to the bit:
 * `D65` applies no matrix at all.
 *
 * @param white - The light's white, linear Rec. 709, positive luminance.
 */
export function whiteBalanceRows(white: Rgb): Rows3 {
  const source = applyRows(SRGB_TO_XYZ, white);
  const scale = source[1] / D65_XYZ[1];
  const target: Rgb = [D65_XYZ[0] * scale, D65_XYZ[1] * scale, D65_XYZ[2] * scale];
  const s = applyRows(BRADFORD, source);
  const t = applyRows(BRADFORD, target);
  const gains: Rows3 = [
    [t[0] / s[0], 0, 0],
    [0, t[1] / s[1], 0],
    [0, 0, t[2] / s[2]],
  ];
  return times(XYZ_TO_SRGB, times(times(BRADFORD_INVERSE, times(gains, BRADFORD)), SRGB_TO_XYZ));
}

/**
 * How much brighter at the camera another host star must be than the one a view adapts to before
 * the view adapts to it instead: 1.25 times, 0.24 mag, so that two stars of near-equal light never
 * swap the whole picture's white from frame to frame (the guide's flash threshold: no large area
 * flashes).
 */
export const BRIGHTEST_HYSTERESIS = 1.25;

/**
 * The scene's brightest host star from the camera: the placed light of the largest photopic
 * illuminance at the camera (`starIlluminance` at its centre's distance), the first in the frame's
 * order among equals; but the star `kept` (`HostDiscDto.star`), the one adapted to before, while no
 * other is {@link BRIGHTEST_HYSTERESIS} times brighter. `null` with none.
 */
export function brightestLight(
  lights: ReadonlyArray<PlacedLight>,
  kept: number | null = null,
): PlacedLight | null {
  let best: PlacedLight | null = null;
  let bestLx = 0;
  let held: PlacedLight | null = null;
  let heldLx = 0;
  for (const light of lights) {
    const lx = photopicIlluminance(starIlluminance(light.disc, norm(light.centreM)));
    if (!(Number.isFinite(lx) && lx > 0)) {
      continue;
    }
    if (lx > bestLx) {
      best = light;
      bestLx = lx;
    }
    if (light.disc.star === kept) {
      held = light;
      heldLx = lx;
    }
  }
  return held !== null && bestLx < BRIGHTEST_HYSTERESIS * heldLx ? held : best;
}

/** A frame's white balance as its tone step applies it: the matrix and the star it adapts to. */
export interface BalancedWhite {
  /** The adaptation, linear Rec. 709, as rows ({@link whiteBalanceRows}). */
  readonly rows: Rows3;
  /** The star whose white it adapts to, its index in its system (`HostDiscDto.star`). */
  readonly star: number;
  /** That star's effective temperature, K. */
  readonly teffK: number;
}

/**
 * The white balance the photorealistic frames of a display's views apply, from its `PRIMARY`
 * view's lights: under `HOST` the adaptation to the brightest of them ({@link brightestLight}),
 * the star adapted to before (`previous`) kept against a near-equal one; `null` under `D65`, which
 * applies nothing, and under `HOST` with no light, which has no white to adapt to.
 */
export function frameWhiteBalance(
  balance: WhiteBalance,
  lights: ReadonlyArray<PlacedLight>,
  previous: BalancedWhite | null = null,
): BalancedWhite | null {
  if (balance === "d65") {
    return null;
  }
  const light = brightestLight(lights, previous?.star ?? null);
  if (light === null) {
    return null;
  }
  const { star, teff_k: teffK } = light.disc;
  // The same star's disc changes little from frame to frame: its rows are kept with it.
  return previous !== null && previous.star === star && previous.teffK === teffK
    ? previous
    : { rows: whiteBalanceRows(discWhite(light.disc)), star, teffK };
}
