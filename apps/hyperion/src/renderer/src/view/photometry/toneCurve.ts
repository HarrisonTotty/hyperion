/*
 * Portions of this file are a port of Filament's AgX tone mapper, `AgxToneMapper` in
 * filament/src/ToneMapper.cpp, and of its colour-space matrices in filament/src/ColorSpaceUtils.h
 * (https://github.com/google/filament, main branch, fetched 2026-09-29):
 *
 * Copyright (C) 2021 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * Changes: ported to TypeScript in f64, without the looks (the `NONE` look only), and wrapped in the
 * linear Rec. 709 ↔ Rec. 2020 conversions and the final clamp that Filament's colour grading
 * applies around the tone mapper. See NOTICE at the repository root.
 */

/**
 * A linear colour, one value per channel (red, green, blue): the one per-channel triple R05, R07
 * and R08 share.
 */
export type Rgb = readonly [number, number, number];

/** A 3 × 3 matrix as its rows. */
export type Rows3 = readonly [Rgb, Rgb, Rgb];

/**
 * Filament's `mat3f` nine-number constructor takes columns (libs/math/include/math/mat3.h), so a
 * matrix written as Filament writes it is the transpose of its rows.
 */
function fromColumns(m: readonly number[]): Rows3 {
  const at = (i: number): number => m[i] ?? Number.NaN;
  return [
    [at(0), at(3), at(6)],
    [at(1), at(4), at(7)],
    [at(2), at(5), at(8)],
  ];
}

function times(a: Rows3, b: Rows3): Rows3 {
  const cell = (i: 0 | 1 | 2, j: 0 | 1 | 2): number =>
    a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
  return [
    [cell(0, 0), cell(0, 1), cell(0, 2)],
    [cell(1, 0), cell(1, 1), cell(1, 2)],
    [cell(2, 0), cell(2, 1), cell(2, 2)],
  ];
}

/** A 3 × 3 matrix's inverse, by the adjugate, for a matrix that has one. */
export function inverseRows(m: Rows3): Rows3 {
  const [[a, b, c], [d, e, f], [g, h, i]] = m;
  const determinant = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
  const k = 1 / determinant;
  return [
    [(e * i - f * h) * k, (c * h - b * i) * k, (b * f - c * e) * k],
    [(f * g - d * i) * k, (a * i - c * g) * k, (c * d - a * f) * k],
    [(d * h - e * g) * k, (b * g - a * h) * k, (a * e - b * d) * k],
  ];
}

/** `m · v`. */
export function applyRows(m: Rows3, v: Rgb): Rgb {
  return [
    m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
    m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
    m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
  ];
}

// Filament's ColorSpaceUtils.h, as written there (columns).
const SRGB_TO_XYZ = fromColumns([
  0.412456, 0.212673, 0.0193339, 0.357576, 0.715152, 0.119192, 0.180438, 0.072175, 0.950304,
]);
/** CIE XYZ to linear sRGB (D65), as rows: Filament's `XYZ_to_sRGB`. */
export const XYZ_TO_SRGB = fromColumns([
  3.2404542, -0.969266, 0.0556434, -1.5371385, 1.8760108, -0.2040259, -0.4985314, 0.041556,
  1.0572252,
]);
const REC2020_TO_XYZ = fromColumns([
  0.636953, 0.2626983, 0, 0.1446169, 0.6780088, 0.0280731, 0.1688558, 0.0592929, 1.0608272,
]);
const XYZ_TO_REC2020 = fromColumns([
  1.7166634, -0.6666738, 0.0176425, -0.3556733, 1.6164557, -0.042777, -0.2533681, 0.0157683,
  0.9422433,
]);

/** Linear Rec. 709 (sRGB primaries) to linear Rec. 2020, as rows: Filament's `sRGB_to_Rec2020`. */
export const REC709_TO_REC2020: Rows3 = times(XYZ_TO_REC2020, SRGB_TO_XYZ);

/** Linear Rec. 2020 to linear Rec. 709, as rows: Filament's `Rec2020_to_sRGB`. */
export const REC2020_TO_REC709: Rows3 = times(XYZ_TO_SRGB, REC2020_TO_XYZ);

/**
 * AgX's inset matrix, as rows: Filament's `AgXInsetMatrix`, from Blender's AgX
 * (EaryChow/AgX_LUT_Gen, AgXBaseRec2020.py), which works in Rec. 2020 primaries.
 */
export const AGX_INSET: Rows3 = fromColumns([
  0.856627153315983, 0.137318972929847, 0.11189821299995, 0.0951212405381588, 0.761241990602591,
  0.0767994186031903, 0.0482516061458583, 0.101439036467562, 0.811302368396859,
]);

/** The inverse of AgX's outset matrix, as rows: Filament's `AgXOutsetMatrixInv`. */
const AGX_OUTSET_INVERSE: Rows3 = fromColumns([
  0.899796955911611, 0.11142098895748, 0.11142098895748, 0.0871996192028351, 0.875575586156966,
  0.0871996192028349, 0.013003424885555, 0.0130034248855548, 0.801379391839686,
]);

/** AgX's outset matrix, as rows: the inverse of `AgXOutsetMatrixInv`, as Filament builds it. */
export const AGX_OUTSET: Rows3 = inverseRows(AGX_OUTSET_INVERSE);

/** The bottom of AgX's log encoding, stops: log₂(2^−10 × 0.18) (Filament's `AgxMinEv`). */
export const AGX_MIN_EV = -12.47393;

/** The top of AgX's log encoding, stops: log₂(2^6.5 × 0.18) (Filament's `AgxMaxEv`). */
export const AGX_MAX_EV = 4.026069;

/**
 * AgX's seventh-order sigmoid on the log-encoded value x in [0, 1], after iolite-engine's minimal
 * AgX (Filament's `agxDefaultContrastApprox`).
 *
 * @remarks
 * It reaches only 0.98206 at x = 1, so the curve's ceiling is 0.961 after the display encoding, and
 * its constant term, 0.002857, is positive, so black maps to 2.53 × 10⁻⁶ rather than 0.
 */
export function agxSigmoid(x: number): number {
  const x2 = x * x;
  const x4 = x2 * x2;
  const x6 = x4 * x2;
  return (
    -17.86 * x6 * x +
    78.01 * x6 -
    126.7 * x4 * x +
    92.06 * x4 -
    28.72 * x2 * x +
    4.361 * x2 -
    0.1718 * x +
    0.002857
  );
}

/** The AgX tone mapper on a linear Rec. 2020 colour: Filament's `AgxToneMapper` with no look. */
function agxRec2020(v: Rgb): Rgb {
  const inset = applyRows(AGX_INSET, [Math.max(0, v[0]), Math.max(0, v[1]), Math.max(0, v[2])]);
  const encoded = inset.map((channel) => {
    const logEv = Math.log2(Math.max(channel, 1e-10));
    const x = (logEv - AGX_MIN_EV) / (AGX_MAX_EV - AGX_MIN_EV);
    return agxSigmoid(Math.min(1, Math.max(0, x)));
  });
  const outset = applyRows(AGX_OUTSET, [
    encoded[0] ?? Number.NaN,
    encoded[1] ?? Number.NaN,
    encoded[2] ?? Number.NaN,
  ]);
  return [
    Math.max(0, outset[0]) ** 2.2,
    Math.max(0, outset[1]) ** 2.2,
    Math.max(0, outset[2]) ** 2.2,
  ];
}

const clamp01 = (value: number): number => Math.min(1, Math.max(0, value));

/**
 * The full AgX tone curve, per channel: a pre-exposed linear Rec. 709 colour to a display-linear
 * Rec. 709 colour in [0, 1] (plan R02, Design note 12).
 *
 * @remarks
 * Filament's port (its header and `NOTICE` entry kept): linear Rec. 709 to Rec. 2020, `max(0, v)`,
 * the inset, `max(v, 1e-10)`, log₂ normalised between {@link AGX_MIN_EV} and {@link AGX_MAX_EV}
 * and clamped to [0, 1], the sigmoid, no look, the outset, `pow(max(0, v), 2.2)`, Rec. 2020 to 709
 * and a clamp. Grey maps to grey, since the inset's and outset's rows each sum to 1; the ceiling is
 * 0.961. Its twin is `agx` in `view/shaders/toneCurve.wgsl`, which R07's full-screen pass includes,
 * so that an isolated star on black is identical in both styles.
 */
export function toneCurve(rgbLinear: Rgb): Rgb {
  const out = applyRows(REC2020_TO_REC709, agxRec2020(applyRows(REC709_TO_REC2020, rgbLinear)));
  return [clamp01(out[0]), clamp01(out[1]), clamp01(out[2])];
}

/** The tone curve's output for black, per channel: 0.002857^2.2 = 2.53 × 10⁻⁶. */
export const TONE_CURVE_BLACK: Rgb = toneCurve([0, 0, 0]);

/**
 * The tone curve as a star sprite writes it: `max(agx(L) − agx(0), 0)` per channel (Design note
 * 12).
 *
 * @remarks
 * Sprites blend additively, and the curve's positive floor summed over every pixel of every
 * overlapping quad would be visible (60 sprites reach half an 8-bit code), so the floor is taken
 * off; the clamp also removes the toe's dip below its black value (to 1.9 × 10⁻⁷ at input 2.35 ×
 * 10⁻⁴), where inputs just above black would display darker than black. It differs from R07's
 * full-screen pass by 2.5 × 10⁻⁶, below one code.
 */
export function spriteToneCurve(rgbLinear: Rgb): Rgb {
  const out = toneCurve(rgbLinear);
  return [
    Math.max(out[0] - TONE_CURVE_BLACK[0], 0),
    Math.max(out[1] - TONE_CURVE_BLACK[1], 0),
    Math.max(out[2] - TONE_CURVE_BLACK[2], 0),
  ];
}

/**
 * The HDR colour target's format: `rgba16float`, whose largest value is 65,504 (brainstorm,
 * "Luminance in physical units"). Its alpha channel is left to R07's meter weight.
 */
export const HDR_COLOUR_FORMAT = "rgba16float";

/** The largest finite value of an IEEE half-precision float: 65,504. */
export const HALF_FLOAT_MAX = 65_504;

/**
 * A luminance pre-exposed by the previous frame's exposure for storage in an `rgba16float` target,
 * clamped to the format's maximum (brainstorm, after Filament §5.2.6, "Pre-exposed lights").
 *
 * @remarks
 * Pre-exposure slides the format's 30 stops over the current scene; a star's disc beyond the window
 * saturates at 65,504, as it would on a sensor.
 *
 * @param cdPerM2 - The luminance, cd/m², not negative.
 * @param previousExposureScale - The previous frame's exposure scale (`exposureScale`), 1 ÷ (cd/m²).
 */
export function preExpose(cdPerM2: number, previousExposureScale: number): number {
  return Math.min(cdPerM2 * previousExposureScale, HALF_FLOAT_MAX);
}
