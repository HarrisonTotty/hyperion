import { applyRows, type Rgb, XYZ_TO_SRGB } from "./toneCurve";

/**
 * The lowest and highest temperatures the Planckian-locus fit covers, K: 1,667 and 25,000 (Kang et
 * al. 2002). A star outside is drawn at the nearest end's colour.
 */
export const PLANCKIAN_FIT_RANGE_K = [1_667, 25_000] as const;

/**
 * The CIE 1931 chromaticity (x, y) of a black body at `temperatureK`, by the cubic fit of Kang,
 * Moon, Hong, Lee, Cho and Kim, "Design of advanced color temperature control system for HDTV
 * applications", J. Korean Physical Society 41 (2002) 865–871 (also Kim et al., US Patent
 * 7,024,034, 2006). Held to [1,667,
 * 25,000] K.
 */
export function planckianChromaticity(temperatureK: number): {
  readonly x: number;
  readonly y: number;
} {
  const [lowK, highK] = PLANCKIAN_FIT_RANGE_K;
  const t = Math.min(highK, Math.max(lowK, temperatureK));
  const t1 = 1e3 / t;
  const t2 = t1 * t1;
  const t3 = t2 * t1;
  const x =
    t <= 4_000
      ? -0.266_123_9 * t3 - 0.234_358_9 * t2 + 0.877_695_6 * t1 + 0.179_910
      : -3.025_846_9 * t3 + 2.107_037_9 * t2 + 0.222_634_7 * t1 + 0.240_390;
  const x2 = x * x;
  const x3 = x2 * x;
  let y: number;
  if (t <= 2_222) {
    y = -1.106_381_4 * x3 - 1.348_110_20 * x2 + 2.185_558_32 * x - 0.202_196_83;
  } else if (t <= 4_000) {
    y = -0.954_947_6 * x3 - 1.374_185_93 * x2 + 2.091_370_15 * x - 0.167_488_67;
  } else {
    y = 3.081_758_0 * x3 - 5.873_386_70 * x2 + 3.751_129_97 * x - 0.370_014_83;
  }
  return { x, y };
}

/**
 * The luminance row of linear sRGB, the Y row of Filament's `sRGB_to_XYZ` (its `LUMINANCE_Rec709`,
 * ColorSpaceUtils.h), consistent with the matrix below; ITU-R BT.709 rounds it to 0.2126, 0.7152,
 * 0.0722.
 */
const LUMINANCE_709: Rgb = [0.212_673, 0.715_152, 0.072_175];

/**
 * A star's linear Rec. 709 colour, normalised to unit luminance, from its effective temperature
 * (plan R02, Design note 12): multiplied by the pixel's luminance it gives the sprite's
 * pre-exposed colour.
 *
 * @remarks
 * The black body's chromaticity (Kang et al.) to XYZ at Y = 1, then to linear sRGB by Filament's
 * `XYZ_to_sRGB` (ColorSpaceUtils.h, after the sRGB primaries of IEC 61966-2-1, with the D65 white
 * point, so a star of about 6,500 K is white); a channel below zero (the hottest stars' red) is
 * clamped and the luminance renormalised to 1. A star with no temperature is white, and one beyond
 * the fit's range takes its end's colour. An interim approximation: R06's sky bakes each star's
 * colour from its spectrum (an M dwarf is less red than its black body) and replaces it.
 *
 * @param temperatureK - The effective temperature, K, or `null` where it is not known.
 */
export function starColour(temperatureK: number | null): Rgb {
  if (temperatureK === null || !Number.isFinite(temperatureK)) {
    return [1, 1, 1];
  }
  const { x, y } = planckianChromaticity(temperatureK);
  const bigX = x / y;
  const bigZ = (1 - x - y) / y;
  const linear = applyRows(XYZ_TO_SRGB, [bigX, 1, bigZ]);
  const rgb: Rgb = [Math.max(0, linear[0]), Math.max(0, linear[1]), Math.max(0, linear[2])];
  const luminance =
    LUMINANCE_709[0] * rgb[0] + LUMINANCE_709[1] * rgb[1] + LUMINANCE_709[2] * rgb[2];
  return [rgb[0] / luminance, rgb[1] / luminance, rgb[2] / luminance];
}
