//! R09.T12.e's `σ_h` budget, its affine scale and the realised figures (plan R09, Design note 7;
//! `decision-r09-t5.md` items 1, 3, 4 and 8, as main's rulings of 2026-10-19 amend them).
//!
//! # The budget (ruling R7)
//!
//! Plan 14's `σ_h` is the whole surface's, the synthesis's at its band limit, so the coarse field
//! is matched on the surface it reconstructs (R09.T4's interpolant, [`quadrature`]) with what the
//! synthesis adds over it booked whole: `σ_h`² = `Var_recon` + L, the local share L = `S_s` +
//! `K_w` + `R_inc` + `C_c`, each an area mean over the body.
//!
//! - `S_s`, the realised structural share: the quadrature mean of
//!   [`StructuralVariance::point_variance`] at the interpolated amplitude a and belt weight b of
//!   each cell's style ([`ReliefStyle::from_codes`] on the cell's quantised codes), which counts the
//!   belts' lift (item 8); with ⟨a²⟩ the quadrature mean of the interpolated a², never of the
//!   cells' (item 1). It is wholly finer than the coarse cell's Nyquist wavelength `λ_c`.
//! - `K_w`, the small craters' whole share: [`CraterOctaves::for_body`]'s unresolved variance at
//!   twice `D_b`, where every octave counts whole, the same in every cell.
//! - `R_inc`, the coarse craters' increase: each listed crater's g² ∫ (F(w₂)² − F(`w_c`)²) dA
//!   ([`CoarseRims::increase_m4`]), what its client-side sharpening adds over its coarse profile,
//!   summed in list order over the body's area ([`spheroid_area_m2`]). Its product term takes the
//!   analytic coarse profile for the reconstructed one (labelled: the cells sample the profile at
//!   their centres, and the spline smooths it). The cells' crater-wide `unresolved_rims` would
//!   overcount the area mean, by the cover's area over the disc's.
//! - `C_c`, the channels' whole share: the area mean of [`channel_variance`] of each cell's
//!   quantised record at twice `D_b`, at least twice the coarse cell's mean width, where every
//!   level counts whole. It is zero before the erosion step, every cell's flow
//!   [`Terminal`](hyperion_surface::field::FlowDirection::Terminal), and is computed anyway so that
//!   R09.T14.d's second match reuses it.
//!
//! Test 2's figures are computed beside them, finer than `λ_c` = 2πR ÷ `l_N`(L): `S_s`, the small
//! craters' unresolved variance at `λ_c`, each coarse crater's sharpening finer than `λ_c`
//! ([`CoarseRims::sharpening_moment_m4`]) over the body's area, and the channels' at `λ_c`.
//!
//! # The scale (ruling R1)
//!
//! Each cell's elevation is its coarse craters' summed lift C (every listed crater's, the
//! craters that wait for the erosion step included, ruling R4, by [`each_lift`]) and the rest, N.
//! With μ the cells' area mean of N, `V_N`, `V_C` and K the reconstructed variances of N and C and
//! their covariance from one quadrature, and T = `σ_h`² − (`S_s` + `K_w` + `C_c`):
//!
//! 1. s\* > 0 solves s\*² (`V_N` + 2K + `V_C` + `R_inc`) = T: Design note 7's one affine scale of the
//!    whole coarse field, each crater's sharpening scaled with it.
//! 2. Where s\* ≤ 1, each coarse crater keeps s\* g of its fresh relief, g its kept relief before:
//!    its degradation byte becomes the nearest integer to 255 (1 − s\* g), at most 255, R09.T12.d's
//!    wear (the small craters' relief factor, worn to equilibrium by R09.T7.j: the larger of κ's
//!    diffusion and the octave's closure `K_v` q) times this crater scale, a stand-in for the
//!    large craters' wear that law leaves out (infill, relaxation) set by plan 14's `σ_h`
//!    (labelled). Every crater is smoothed in again at its new byte, so that its coarse profile and
//!    its client-side sharpening stay one profile, and `s_N` solves
//!    `s_N`² `V_N` + 2 `s_N` K′ + `V_C`′ + `R_inc`′ = T at the new bytes.
//! 3. Where s\* > 1 the craters keep their bytes, no crater being fresher than its fresh shape, and
//!    `s_N` solves the same equation at them: the non-crater field alone is scaled.
//! 4. Where T ≤ 0, or no `s_N` > 0 solves it, the elevation is left unscaled (labelled; no reference
//!    world nears it).
//!
//! Then z = μ + `s_N` (N − μ) + C′, the craters smoothed in after the erosion step left out until
//! then. Where every crater's byte scales alike, C′ is C times one factor, so `V_C`′ and K′ follow
//! from the one quadrature; otherwise a second quadrature reads C′. Of the quadratic's positive
//! roots `s_N` takes the one nearest s\* (the lane's lean): at s\* ≤ 1 that is s\* to the bytes'
//! rounding.
//!
//! # The realised figures (rulings R5 and R6)
//!
//! [`figures`] writes the elevation as it stands: the realised `σ_h` √(`Var_recon` + L), the
//! whole surface's at its band limit, plan 14's after the scale, with the craters in the
//! elevation by then; and the realised relief, the cells' greatest less least, which bounds the
//! reconstructed field (the spline is a convex blend of the cells) as the surface crate's readers
//! rely on, not the quadrature's narrower range, which it reports beside it.

use hyperion_surface::field::{CoarseCrater, SynthesisCell, boundary_diameter};
use hyperion_surface::num;
use hyperion_surface::synth::BandSpectrum;
use hyperion_surface::synth::channels::{ChannelBody, channel_variance};
use hyperion_surface::synth::craters::CraterOctaves;
use hyperion_surface::synth::craters::rims::{CoarseRims, kept_relief};
use hyperion_surface::synth::relief::{ReliefStyle, StructuralVariance, nyquist_degree};

use super::super::craters::{Phase, each_lift, smoothed_in_by, spheroid_area_m2, waits};
use super::super::plates::slot;
use super::super::{Pass, Working};
use super::quadrature::{self, Moments, NODES, Values};
use crate::planetary::surface::quantise::synthesis_record;
use crate::units::{Metres, SquareMetres};

/// What the budget reads of the body beside its cells: the wavelengths its shares are taken at,
/// its area and the closed forms' bodies.
#[derive(Debug, Clone)]
pub(crate) struct Body {
    /// The coarse cell's Nyquist wavelength `λ_c` = 2πR ÷ `l_N`(L), test 2's.
    pub(crate) nyquist_wavelength: Metres,
    /// The wavelength at which every contribution counts whole: twice the boundary diameter.
    pub(crate) whole: Metres,
    /// The body's area, its spheroid's, square metres.
    area_m2: f64,
    rims: CoarseRims,
    small: CraterOctaves,
    channels: ChannelBody,
    structure: StructuralVariance,
}

impl Body {
    /// The body of `pass` under the structural spectrum `spectrum`.
    #[must_use]
    pub(crate) fn of(pass: &Pass<'_>, spectrum: &BandSpectrum) -> Self {
        let inputs = pass.inputs();
        let level = pass.level();
        let radius = inputs.radius();
        let d_b = boundary_diameter(level, radius);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a coarse level's Nyquist degree is below 2^10, exact in f64"
        )]
        let nyquist = nyquist_degree(level.get()) as f64;
        Self {
            nyquist_wavelength: Metres::new(2.0 * core::f64::consts::PI * radius.value() / nyquist),
            whole: d_b * 2.0,
            area_m2: spheroid_area_m2(&inputs.figure()),
            rims: CoarseRims::for_body(*inputs.craters(), inputs.figure(), level),
            small: CraterOctaves::for_body(
                *inputs.craters(),
                inputs.figure(),
                d_b,
                inputs.surface_age(),
            ),
            channels: ChannelBody {
                radius,
                level: level.get(),
                gravity: inputs.craters().gravity(),
            },
            structure: StructuralVariance::new(spectrum, level, radius),
        }
    }
}

/// The local share's contributions, area means over the body, square metres: what the synthesis
/// adds over the reconstructed field (the module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LocalShare {
    /// `S_s`, the realised structural share.
    pub(crate) structural_m2: f64,
    /// ⟨a²⟩, the quadrature mean of the interpolated squared amplitude.
    pub(crate) amplitude_square: f64,
    /// `K_w`, the small craters' whole share.
    pub(crate) small_craters_m2: f64,
    /// `R_inc`, the coarse craters' increase.
    pub(crate) increase_m2: f64,
    /// The coarse craters' sharpening's own variance, ∫ (F(w₂) − F(`w_c`))² dA over the body (the
    /// rest of the increase is twice its product with the coarse profile).
    pub(crate) sharpening_m2: f64,
    /// `C_c`, the channels' whole share.
    pub(crate) channels_m2: f64,
}

impl LocalShare {
    /// L = `S_s` + `K_w` + `R_inc` + `C_c`, in that order.
    #[must_use]
    pub(crate) fn total_m2(&self) -> f64 {
        self.structural_m2 + self.small_craters_m2 + self.increase_m2 + self.channels_m2
    }
}

/// Test 2's local share finer than `λ_c`, area means over the body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FinerShare {
    /// The realised structural share, wholly finer than `λ_c`.
    pub(crate) structural: SquareMetres,
    /// The small craters' unresolved variance at `λ_c`.
    pub(crate) small_craters: SquareMetres,
    /// The coarse craters' sharpening finer than `λ_c`.
    pub(crate) sharpening: SquareMetres,
    /// The channels' at `λ_c`.
    pub(crate) channels: SquareMetres,
}

#[cfg(test)]
impl FinerShare {
    /// The four, in their order.
    #[must_use]
    pub(crate) fn total(&self) -> SquareMetres {
        self.structural + self.small_craters + self.sharpening + self.channels
    }

    /// The craters' part, the small craters' and the coarse craters' sharpening.
    #[must_use]
    pub(crate) fn craters(&self) -> SquareMetres {
        self.small_craters + self.sharpening
    }
}

/// Which rule scaled the elevation (the module documentation, "The scale").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    /// s\* ≤ 1: the whole field, the craters through their bytes.
    Whole,
    /// s\* > 1: the non-crater field alone.
    NonCrater,
    /// No positive scale meets plan 14's `σ_h`: left as it was (labelled).
    Unscaled,
}

/// The realised structural share and the mean square amplitude it is made of
/// (`decision-r09-t5.md` items 1 and 8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StructuralShare {
    /// `S_s`, the quadrature mean of the point variance at the interpolated a and b, square
    /// metres.
    pub(crate) share_m2: f64,
    /// ⟨a²⟩, the quadrature mean of the interpolated squared amplitude.
    pub(crate) amplitude_square: f64,
}

/// What the budget books of the cells' quantised records: their styles' realised structural share
/// and the channels' cell means, whole and finer than `λ_c` ([`book`]). The scale and the sea
/// change no code these read, so the figures after them take the match's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Booked {
    /// The styles' realised structural share.
    pub(crate) structural: StructuralShare,
    /// The channels' share whole, the cells' area mean, square metres.
    pub(crate) channels_m2: f64,
    /// The channels' share finer than `λ_c`, the cells' area mean, square metres.
    pub(crate) channels_finer_m2: f64,
}

/// Each cell's record as the quantiser makes it, in cell-index order, so that the styles and
/// channels read the field's codes bit for bit.
///
/// # Panics
///
/// If a cell's state has no code: a fault of the step that wrote it, which the quantiser would
/// name.
pub(crate) fn records(working: &Working) -> impl Iterator<Item = SynthesisCell> + '_ {
    (0_u32..).zip(&working.cells).map(|(n, cell)| {
        synthesis_record(n, cell)
            .unwrap_or_else(|e| panic!("the pass's state quantises, but {e} ({e:?})"))
    })
}

/// Books the cells of `working` on the body `body` of `pass` ([`Booked`]): each cell's style
/// from `ReliefStyle::of` of its record, their realised structural share by the quadrature, and
/// the channels' share of each record, averaged over the cells' solid angles in cell-index order.
///
/// # Panics
///
/// As [`records`].
#[must_use]
pub(crate) fn book(pass: &Pass<'_>, working: &Working, body: &Body) -> Booked {
    let omega = pass.grid().solid_angles_sr();
    let count = working.cells.len();
    let (mut amplitude, mut belt) = (Vec::with_capacity(count), Vec::with_capacity(count));
    let (mut total, mut whole, mut finer) = (0.0, 0.0, 0.0);
    for (record, &w) in records(working).zip(omega) {
        let style = ReliefStyle::of(&record);
        amplitude.push(style.amplitude());
        belt.push(style.belt());
        total += w;
        whole += w * channel_variance(&record, body.channels, body.whole).value();
        finer += w * channel_variance(&record, body.channels, body.nyquist_wavelength).value();
    }
    Booked {
        structural: structural_share(pass, body, &amplitude, &belt),
        channels_m2: whole / total,
        channels_finer_m2: finer / total,
    }
}

/// The realised structural share of the cells' style amplitudes `amplitude` and belt weights
/// `belt` on the body `body` of `pass`: the quadrature means of the point variance and of a², at
/// the interpolated a and b.
#[must_use]
fn structural_share(
    pass: &Pass<'_>,
    body: &Body,
    amplitude: &[f64],
    belt: &[f64],
) -> StructuralShare {
    let grid = pass.grid();
    let a = Values::new(grid, amplitude);
    let b = Values::new(grid, belt);
    let (mut weight, mut structural, mut square) = (0.0, 0.0, 0.0);
    quadrature::each_node(grid, &quadrature::gauss_legendre(NODES), |n, dir, w| {
        let (a, b) = (a.at(n, dir), b.at(n, dir));
        weight += w;
        structural += w * body.structure.point_variance(a, b).value();
        square += w * (a * a);
    });
    StructuralShare {
        share_m2: structural / weight,
        amplitude_square: square / weight,
    }
}

/// The coarse craters' increase and their sharpening's own variance, whole, and their sharpening
/// finer than `λ_c`, each summed over `craters` in their order and taken over the body's area,
/// square metres.
#[must_use]
fn crater_shares<'c>(
    body: &Body,
    craters: impl IntoIterator<Item = &'c CoarseCrater>,
) -> (f64, f64, f64) {
    let (mut increase, mut whole, mut finer) = (0.0, 0.0, 0.0);
    for crater in craters {
        increase += body.rims.increase_m4(crater);
        whole += body.rims.sharpening_moment_m4(crater, body.whole);
        finer += body
            .rims
            .sharpening_moment_m4(crater, body.nyquist_wavelength);
    }
    (
        increase / body.area_m2,
        whole / body.area_m2,
        finer / body.area_m2,
    )
}

/// The local share and test 2's finer share of the body `body` and its cells' `booked` shares,
/// with the coarse craters `craters`.
#[must_use]
fn shares<'c>(
    body: &Body,
    booked: &Booked,
    craters: impl IntoIterator<Item = &'c CoarseCrater>,
) -> (LocalShare, FinerShare) {
    let (increase, sharpening, finer) = crater_shares(body, craters);
    let structural = booked.structural;
    (
        LocalShare {
            structural_m2: structural.share_m2,
            amplitude_square: structural.amplitude_square,
            small_craters_m2: body.small.unresolved_variance(body.whole).value(),
            increase_m2: increase,
            sharpening_m2: sharpening,
            channels_m2: booked.channels_m2,
        },
        FinerShare {
            structural: SquareMetres::new(structural.share_m2),
            small_craters: body.small.unresolved_variance(body.nyquist_wavelength),
            sharpening: SquareMetres::new(finer),
            channels: SquareMetres::new(booked.channels_finer_m2),
        },
    )
}

/// The reconstructed variances of the cells' `first` and `second` values and their covariance,
/// (Var x, Cov(x, y), Var y), from one quadrature over `pass`'s grid, about their cell means.
#[must_use]
fn reconstructed_pair(pass: &Pass<'_>, first: &[f64], second: &[f64]) -> (f64, f64, f64) {
    let grid = pass.grid();
    let (x, y) = (Values::new(grid, first), Values::new(grid, second));
    let mut moments = Moments::new([
        quadrature::cell_mean(grid, first),
        quadrature::cell_mean(grid, second),
    ]);
    quadrature::each_node(grid, &quadrature::gauss_legendre(NODES), |n, dir, w| {
        moments.add(w, [x.at(n, dir), y.at(n, dir)]);
    });
    (
        moments.covariance(0, 0),
        moments.covariance(0, 1),
        moments.covariance(1, 1),
    )
}

/// What R09.T12.e's match of `σ_h` found and did (the module documentation): the record its tests
/// and the plan read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Match {
    /// T = `σ_h`² − (`S_s` + `K_w` + `C_c`), square metres.
    pub(crate) target_m2: f64,
    /// μ, the cells' area mean of the non-crater field N, metres.
    pub(crate) mean_m: f64,
    /// `V_N`, the reconstructed variance of N before the scale, square metres.
    pub(crate) non_crater_m2: f64,
    /// K, the reconstructed covariance of N and C before the scale, square metres.
    pub(crate) covariance_m2: f64,
    /// `V_C`, the reconstructed variance of the coarse craters' lift C at R09.T12.d's bytes.
    pub(crate) craters_m2: f64,
    /// The cells' area-weighted variance of N, C and their covariance, before the scale.
    pub(crate) cells_m2: (f64, f64, f64),
    /// The local share before the scale, at R09.T12.d's bytes.
    pub(crate) before: LocalShare,
    /// Test 2's share before the scale.
    pub(crate) finer_before: FinerShare,
    /// s\*, the whole field's scale, or `None` where T or the field's whole variance is not
    /// positive.
    pub(crate) whole_scale: Option<f64>,
    /// The rule that scaled the elevation.
    pub(crate) rule: Rule,
    /// `s_N`, the non-crater field's scale: 1 where unscaled.
    pub(crate) non_crater_scale: f64,
    /// K′ and `V_C`′ at the bytes after the scale, square metres.
    pub(crate) after_m2: (f64, f64),
    /// The local share after the scale.
    pub(crate) after: LocalShare,
    /// Test 2's share after the scale.
    pub(crate) finer_after: FinerShare,
    /// The largest degradation byte after the scale.
    pub(crate) largest_byte: u8,
    /// Whether the bytes scaled unevenly, so that a second quadrature read C′.
    pub(crate) second_quadrature: bool,
    /// What it booked of the cells' records, which the figures after it read.
    pub(crate) booked: Booked,
}

/// The degradation byte of a coarse crater that keeps `kept` of its fresh relief: the nearest
/// integer to 255 (1 − `kept`), at most 255 and at least 0 (R09.T12.d's byte rule).
///
/// # Panics
///
/// If `kept` is not finite, which would otherwise saturate to a fresh crater's byte (a bug).
#[must_use]
fn byte_of(kept: f64) -> u8 {
    let lost = (255.0 * (1.0 - num::assert_finite(kept)))
        .round()
        .clamp(0.0, 255.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a rounded value clamped to 0–255"
    )]
    {
        lost as u8
    }
}

/// The positive root `s` of `v` s² + 2 `k` s + `rest` = 0 nearest `near`, the larger at a tie, by
/// the pair of roots that does not cancel: q = −(k + sign(k) √(k² − v rest)), s = q ÷ v and
/// rest ÷ q; `None` where neither is positive and finite.
#[must_use]
fn positive_root(v: f64, k: f64, rest: f64, near: f64) -> Option<f64> {
    let discriminant = k * k - v * rest;
    if discriminant.is_nan() || discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let q = if k >= 0.0 { -(k + root) } else { root - k };
    let roots = [q / v, rest / q];
    roots
        .into_iter()
        .filter(|s| s.is_finite() && *s > 0.0)
        .fold(None, |best: Option<f64>, s| match best {
            None => Some(s),
            Some(b) => {
                let (gap, best_gap) = ((s - near).abs(), (b - near).abs());
                if gap < best_gap || (gap.total_cmp(&best_gap).is_eq() && s > b) {
                    Some(s)
                } else {
                    Some(b)
                }
            }
        })
}

/// Matches the coarse elevation of `working` to plan 14's `σ_h` on the body of `pass` (the module
/// documentation): splits it into its coarse craters' lift and the rest, books the local share,
/// solves the scale, sets the craters' bytes where the whole field scales down, and writes each
/// cell's elevation and dry water surface. `working.spectrum` must be the body's structural
/// spectrum already, since `S_s` reads it.
///
/// # Panics
///
/// Never on valid inputs: every value it writes is finite.
pub(crate) fn match_sigma_h(pass: &Pass<'_>, working: &mut Working) -> Match {
    let inputs = pass.inputs();
    let body = Body::of(pass, &working.spectrum);
    let booked = book(pass, working, &body);
    let (lift, non) = split(pass, working, &body);
    let mean = quadrature::cell_mean(pass.grid(), &non);
    let cells_m2 = cell_covariances(pass, &non, &lift);
    let (v_n, k, v_c) = reconstructed_pair(pass, &non, &lift);
    let (before, finer_before) = shares(&body, &booked, &working.craters);
    let sigma = inputs.sigma_h().value();
    let target =
        sigma * sigma - (before.structural_m2 + before.small_craters_m2 + before.channels_m2);
    let whole = v_n + 2.0 * k + v_c + before.increase_m2;
    let whole_scale = (target > 0.0 && whole > 0.0).then(|| (target / whole).sqrt());
    let mut record = Match {
        target_m2: target,
        mean_m: mean,
        non_crater_m2: v_n,
        covariance_m2: k,
        craters_m2: v_c,
        cells_m2,
        before,
        finer_before,
        whole_scale,
        rule: Rule::Unscaled,
        non_crater_scale: 1.0,
        after_m2: (k, v_c),
        after: before,
        finer_after: finer_before,
        largest_byte: largest_byte(&working.craters),
        second_quadrature: false,
        booked,
    };
    let Some(s_star) = whole_scale else {
        return record;
    };
    let rule = if s_star <= 1.0 {
        Rule::Whole
    } else {
        Rule::NonCrater
    };
    let old: Vec<u8> = working.craters.iter().map(|c| c.degradation).collect();
    let (k_after, v_c_after) = if rule == Rule::Whole {
        let (after, second) = scale_bytes(pass, working, &body, &non, s_star, (k, v_c));
        record.second_quadrature = second;
        after
    } else {
        (k, v_c)
    };
    let (after, finer_after) = shares(&body, &booked, &working.craters);
    let rest = v_c_after + after.increase_m2 - target;
    let Some(s_n) = positive_root(v_n, k_after, rest, s_star) else {
        // Unscaled: the craters keep the bytes their profiles were smoothed in at.
        for (crater, &byte) in working.craters.iter_mut().zip(&old) {
            crater.degradation = byte;
        }
        return record;
    };
    write_scaled(pass, working, &body, &non, mean, s_n);
    record.rule = rule;
    record.non_crater_scale = s_n;
    record.after_m2 = (k_after, v_c_after);
    record.after = after;
    record.finer_after = finer_after;
    record.largest_byte = largest_byte(&working.craters);
    record
}

/// The split of `working`'s elevation on the body `body` of `pass` (the module documentation, "The
/// scale"): C, every listed crater's lift summed in list order, the craters that wait for the
/// erosion step included; and N, each cell's elevation less the part of C already in it.
#[must_use]
fn split(pass: &Pass<'_>, working: &Working, body: &Body) -> (Vec<f64>, Vec<f64>) {
    let grid = pass.grid();
    let figure = pass.inputs().figure();
    let count = working.cells.len();
    let mut lift = vec![0.0; count];
    let mut placed = vec![0.0; count];
    for crater in &working.craters {
        let now = !waits(pass, crater.age);
        each_lift(grid, &figure, &body.rims, crater, |cell, l| {
            lift[slot(cell)] += l;
            if now {
                placed[slot(cell)] += l;
            }
        });
    }
    let non = working
        .cells
        .iter()
        .zip(&placed)
        .map(|(c, p)| num::assert_finite(c.elevation.value() - p))
        .collect();
    (lift, non)
}

/// Sets each of `working`'s coarse craters' degradation bytes where the whole field scales by
/// `s_star`, at most 1 (the module documentation, "The scale", 2), and returns K′ and `V_C`′ at
/// the new bytes from the non-crater field `non` and K and `V_C` before, with whether a second
/// quadrature read C′: where every crater's byte was alike C′ is C times one factor, and where
/// none was C is zero either way.
fn scale_bytes(
    pass: &Pass<'_>,
    working: &mut Working,
    body: &Body,
    non: &[f64],
    s_star: f64,
    (k, v_c): (f64, f64),
) -> ((f64, f64), bool) {
    let even = working
        .craters
        .windows(2)
        .all(|pair| pair[0].degradation == pair[1].degradation);
    let kept_before: Vec<f64> = working.craters.iter().map(kept_relief).collect();
    for (crater, &g) in working.craters.iter_mut().zip(&kept_before) {
        crater.degradation = byte_of(s_star * g);
    }
    match (even, kept_before.first()) {
        (true, Some(&g)) if g > 0.0 => {
            let factor = kept_relief(&working.craters[0]) / g;
            ((factor * k, factor * factor * v_c), false)
        }
        (true, _) => ((k, v_c), false),
        (false, _) => {
            let grid = pass.grid();
            let figure = pass.inputs().figure();
            let mut scaled = vec![0.0; working.cells.len()];
            for crater in &working.craters {
                each_lift(grid, &figure, &body.rims, crater, |cell, l| {
                    scaled[slot(cell)] += l;
                });
            }
            let (_, k2, v2) = reconstructed_pair(pass, non, &scaled);
            ((k2, v2), true)
        }
    }
}

/// Writes each of `working`'s cells' elevation and dry water surface, μ + `s_n` (N − μ) of the
/// non-crater field `non` about its mean `mean`, with the craters that do not wait for the
/// erosion step smoothed in at their bytes in list order (the one profile).
fn write_scaled(
    pass: &Pass<'_>,
    working: &mut Working,
    body: &Body,
    non: &[f64],
    mean: f64,
    s_n: f64,
) {
    for (cell, &n) in working.cells.iter_mut().zip(non) {
        let z = Metres::new(num::assert_finite(mean + s_n * (n - mean)));
        cell.elevation = z;
        cell.water_surface = z;
    }
    let grid = pass.grid();
    let figure = pass.inputs().figure();
    let Working {
        cells: states,
        craters,
        ..
    } = working;
    for crater in craters.iter().filter(|c| !waits(pass, c.age)) {
        each_lift(grid, &figure, &body.rims, crater, |cell, l| {
            let state = &mut states[slot(cell)];
            let z = Metres::new(num::assert_finite(state.elevation.value() + l));
            state.elevation = z;
            state.water_surface = z;
        });
    }
}

/// The cells' area-weighted variances of `first` and `second` and their covariance, (Var x,
/// Cov(x, y), Var y), by their solid angles in cell-index order.
#[must_use]
fn cell_covariances(pass: &Pass<'_>, first: &[f64], second: &[f64]) -> (f64, f64, f64) {
    let grid = pass.grid();
    let mut moments = Moments::new([
        quadrature::cell_mean(grid, first),
        quadrature::cell_mean(grid, second),
    ]);
    for ((&x, &y), &w) in first.iter().zip(second).zip(grid.solid_angles_sr()) {
        moments.add(w, [x, y]);
    }
    (
        moments.covariance(0, 0),
        moments.covariance(0, 1),
        moments.covariance(1, 1),
    )
}

/// The largest degradation byte of `craters`, 0 for none.
#[must_use]
fn largest_byte(craters: &[CoarseCrater]) -> u8 {
    craters.iter().map(|c| c.degradation).max().unwrap_or(0)
}

/// The realised figures of an elevation (the module documentation, "The realised figures").
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Figures {
    /// The realised `σ_h`, √(`Var_recon` + L), metres.
    pub(crate) sigma_h_m: f64,
    /// `Var_recon`, the reconstructed field's variance, square metres.
    pub(crate) reconstructed_m2: f64,
    /// The cells' area-weighted variance, square metres.
    pub(crate) cells_m2: f64,
    /// The local share, with the craters in the elevation.
    pub(crate) local: LocalShare,
    /// Test 2's share, with the craters in the elevation.
    pub(crate) finer: FinerShare,
    /// The realised relief, the cells' greatest less least elevation, metres.
    pub(crate) relief_m: f64,
    /// The reconstructed field's range at the quadrature's nodes, metres.
    pub(crate) reconstructed_range_m: f64,
}

/// Writes the realised `σ_h` and relief of `working`'s elevation as it stands on the body of
/// `pass`, with the coarse craters smoothed in by the end of `phase`, and returns them (the module
/// documentation, "The realised figures").
///
/// `booked` is the cells' shares ([`book`]) for the records as they stand: the match's after the
/// scale and the sea, which change no code the book reads, and a fresh book otherwise.
///
/// # Panics
///
/// Never on valid inputs: every elevation is finite.
pub(crate) fn figures(
    pass: &Pass<'_>,
    working: &mut Working,
    phase: Phase,
    booked: &Booked,
) -> Figures {
    let grid = pass.grid();
    let body = Body::of(pass, &working.spectrum);
    let (local, finer) = shares(
        &body,
        booked,
        working
            .craters
            .iter()
            .filter(|c| smoothed_in_by(pass, c.age, phase)),
    );
    let z: Vec<f64> = working
        .cells
        .iter()
        .map(|c| num::assert_finite(c.elevation.value()))
        .collect();
    let lowest = z.iter().copied().fold(f64::INFINITY, num::min);
    let highest = z.iter().copied().fold(f64::NEG_INFINITY, num::max);
    let values = Values::new(grid, &z);
    let mut moments = Moments::new([quadrature::cell_mean(grid, &z)]);
    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
    quadrature::each_node(grid, &quadrature::gauss_legendre(NODES), |n, dir, w| {
        let at = values.at(n, dir);
        low = num::min(low, at);
        high = num::max(high, at);
        moments.add(w, [at]);
    });
    let reconstructed = moments.covariance(0, 0);
    let mut cell_moments = Moments::new([quadrature::cell_mean(grid, &z)]);
    for (&v, &w) in z.iter().zip(grid.solid_angles_sr()) {
        cell_moments.add(w, [v]);
    }
    let sigma = (reconstructed + local.total_m2()).sqrt();
    working.realised_sigma_h = Metres::new(num::assert_finite(sigma));
    working.realised_relief = Metres::new(highest - lowest);
    Figures {
        sigma_h_m: sigma,
        reconstructed_m2: reconstructed,
        cells_m2: cell_moments.covariance(0, 0),
        local,
        finer,
        relief_m: highest - lowest,
        reconstructed_range_m: high - low,
    }
}
