// The full AgX tone curve, per channel (plan R02, Design note 12): the WGSL twin of `toneCurve` in
// view/photometry/toneCurve.ts, included by the star sprites here and by R07's full-screen pass, so
// that an isolated star on black is identical in both styles.
//
// A port of Filament's AgX tone mapper, `AgxToneMapper` in filament/src/ToneMapper.cpp, and of its
// colour-space matrices in filament/src/ColorSpaceUtils.h (https://github.com/google/filament, main
// branch, fetched 2026-09-29):
//
// Copyright (C) 2021 The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Changes: ported to WGSL without the looks (the `NONE` look only), wrapped in the linear
// Rec. 709 <-> Rec. 2020 conversions and the final clamp that Filament's colour grading applies
// around the tone mapper, and with the matrices multiplied out in f64 by toneCurve.ts (whose test
// holds these literals to it). See NOTICE at the repository root.

// Linear Rec. 709 to linear Rec. 2020: Filament's sRGB_to_Rec2020 (columns).
const AGX_REC709_TO_REC2020 = mat3x3f(
  0.6275074181, 0.06910773697, 0.01639647980,
  0.3292777095, 0.9195044313, 0.08802434089,
  0.04330387124, 0.01135908159, 0.8955135244,
);

// AgX's inset matrix, Filament's AgXInsetMatrix, after Blender's AgX (columns).
const AGX_INSET = mat3x3f(
  0.8566271533, 0.1373189729, 0.1118982130,
  0.09512124054, 0.7612419906, 0.07679941860,
  0.04825160615, 0.1014390365, 0.8113023684,
);

// AgX's outset matrix, the inverse of Filament's AgXOutsetMatrixInv (columns).
const AGX_OUTSET = mat3x3f(
  1.127100582, -0.1413297635, -0.1413297635,
  -0.1106066431, 1.157823702, -0.1106066431,
  -0.01649393872, -0.01649393872, 1.251936407,
);

// Linear Rec. 2020 to linear Rec. 709: Filament's Rec2020_to_sRGB (columns).
const AGX_REC2020_TO_REC709 = mat3x3f(
  1.660213353, -0.1245520386, -0.01815502653,
  -0.5875643107, 1.132946193, -0.1006047909,
  -0.07282758224, -0.008348329956, 1.118831672,
);

// The log encoding's range, stops: log2(2^-10 * 0.18) and log2(2^6.5 * 0.18).
const AGX_MIN_EV = -12.47393;
const AGX_MAX_EV = 4.026069;

// The seventh-order sigmoid after iolite-engine's minimal AgX (Filament's
// agxDefaultContrastApprox), -17.86x^7 + 78.01x^6 - 126.7x^5 + 92.06x^4 - 28.72x^3 + 4.361x^2
// - 0.1718x + 0.002857, re-expanded exactly about x = 0.5 and evaluated by Horner's rule: the
// monomial form's terms reach 100 and cancel to under 1, which costs it 1.2e-5 in f32, and this
// form 1.5e-7 (plan R02, R02.T14.c).
fn agxSigmoid(x: vec3f) -> vec3f {
  let t = x - vec3f(0.5);
  var p = vec3f(-17.86);
  p = p * t + 15.5;
  p = p * t + 13.565;
  p = p * t - 10.29;
  p = p * t - 5.39375;
  p = p * t + 2.40975;
  p = p * t + 1.7588875;
  return p * t + 0.290957;
}

// The full AgX curve: a pre-exposed linear Rec. 709 colour to a display-linear one in [0, 1].
fn agx(rgbLinear: vec3f) -> vec3f {
  var v = AGX_REC709_TO_REC2020 * rgbLinear;
  v = max(v, vec3f(0.0));
  v = AGX_INSET * v;
  v = max(v, vec3f(1e-10));
  v = (log2(v) - AGX_MIN_EV) / (AGX_MAX_EV - AGX_MIN_EV);
  v = clamp(v, vec3f(0.0), vec3f(1.0));
  v = agxSigmoid(v);
  v = AGX_OUTSET * v;
  v = pow(max(v, vec3f(0.0)), vec3f(2.2));
  return clamp(AGX_REC2020_TO_REC709 * v, vec3f(0.0), vec3f(1.0));
}

// The curve as a star sprite writes it: max(agx(L) - agx(0), 0), so that the curve's positive
// floor does not add up over overlapping additive sprites (Design note 12).
fn agxSprite(rgbLinear: vec3f) -> vec3f {
  return max(agx(rgbLinear) - agx(vec3f(0.0)), vec3f(0.0));
}
