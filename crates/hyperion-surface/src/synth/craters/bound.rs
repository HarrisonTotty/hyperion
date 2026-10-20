//! The small craters' terms in the level bound and the heights' range (plan R09, T8): what one
//! crater of each octave can add to a height, or change in it between two levels, and how many of
//! an octave's craters reach a point.
//!
//! # A quantile, not a hard bound
//!
//! The relief's and the channels' terms are hard: each octave or level is bounded whatever the
//! cell. The craters' cannot be. A cell's count is a Poisson draw, so any number of an octave's
//! craters may overlap a point, and the sum of their heights has no largest value. What is bounded
//! is a quantile: the number of an octave's craters whose reach holds a point is Poisson, of mean
//! μ (the octave's density times the area its craters reach, [`OctaveMarks::expected`]), and the
//! octaves are drawn independently, so the craters' sum at a point, at most Σ m N over the octaves
//! with m one crater's largest share ([`OctaveMarks`]) and N its octave's count there, exceeds a t
//! with probability at most e^(−θt) Π exp(μ (e^(θm) − 1)) for every θ > 0 (Chernoff's bound on a
//! sum of independent Poisson counts). [`poisson_tail_bound`] takes the least t over θ for which
//! that is the stated tail; the level bound reads it at
//! [`CRATER_BOUND_TAIL`](crate::synth::levels::CRATER_BOUND_TAIL) per point.
//!
//! # One crater's shares
//!
//! A crater's profile at a level is its fresh shape F₀ filleted over the level's band, `F_w`
//! ([`CraterProfile::filleted`]), times its relief factor g ≤ 1, so with δ(w) = sup |`F_w` − F₀| over
//! the crater, it adds at most sup |F₀| + δ(w) at a level of band w, and changes by at most
//! δ(w) + δ(w′) between bands w and w′. The supremums are sampled: each octave's diameters at
//! [`DIAMETER_SAMPLES`] even steps in ln D across its kept span, each crater's fresh height on
//! [`PROFILE_SAMPLES`] radii out to its reach with its centre and rim, and each fillet's change
//! at the ends of [`FILLET_SAMPLES`] steps across the fillet's span, the rim among them, and at
//! its centre, each widened by [`SAMPLED_MARGIN`]. The change is nothing outside the span but for
//! the central cone where the fillet reaches into it (R09.T7.i): there the profile keeps the share
//! p = (1 − w) ÷ `c_p` of the cone, so from the centre to the span's inner end it changes by the
//! rest of the cone, (1 − p) of its relief at the centre and falling linearly outwards. So the
//! span's samples and the centre take the change's largest wherever it lies: on the span, where
//! the fillet and its compensating term sit, or at the centre, where the cone fades.

use core::f64::consts::PI;

use hyperion_base::math;
use hyperion_base::units::Metres;

use super::{CraterOctaves, CraterProfile, MAX_EXTERIOR_WIDTH, kept_span};
use crate::craters::crater_shape;
use crate::num;

/// The diameters at which each octave's shares are sampled: 9, an eighth of the octave's kept span
/// apart in ln D, its smallest and its upper among them.
pub(crate) const DIAMETER_SAMPLES: u32 = 9;

/// The radii, evenly spaced from the centre to the reach, at which a fresh crater's largest height
/// is sampled, beside its centre and rim: 129.
pub(crate) const PROFILE_SAMPLES: u32 = 129;

/// The intervals across a fillet, from 1 − w to 1 + w rim radii, at whose 33 ends, the rim among
/// them, its change from the fresh profile is sampled, beside the centre: 32.
pub(crate) const FILLET_SAMPLES: u32 = 32;

/// The factor by which every sampled supremum is widened, for the values between samples: 1.1.
pub(crate) const SAMPLED_MARGIN: f64 = 1.1;

/// One small-crater octave's shares in the level bound (the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OctaveMarks {
    /// The smallest diameter the octave keeps, metres: a level evaluates it where this is at least
    /// the level's shortest wavelength.
    pub(crate) smallest_m: f64,
    /// μ, the expected number of the octave's craters whose reach holds a point.
    pub(crate) expected: f64,
    /// The largest |height| of any of its craters' fresh profiles, metres.
    pub(crate) fresh_m: f64,
    /// δ at each band level's band half-width, metres: the largest change any of its craters'
    /// profiles takes from the fresh one when filleted over the level's band, by level.
    pub(crate) fillet_m: [f64; crate::synth::BAND_LEVELS],
}

/// Each of `octaves`' shares in the level bound, from the largest octave (the
/// [module documentation](self)), with `bands` each band level's fillet half-width, metres.
#[must_use]
pub(crate) fn octave_marks(
    octaves: &CraterOctaves,
    bands: &[f64; crate::synth::BAND_LEVELS],
) -> Vec<OctaveMarks> {
    let params = &octaves.inputs.params;
    let list = &octaves.octaves;
    let mut out = Vec::with_capacity(list.len());
    for octave in list {
        let (smallest, upper) = kept_span(octave);
        // A marked Poisson field covers a point a Poisson number of times, of mean its density
        // times its marks' mean area (Chiu, Stoyan, Kendall and Mecke 2013, Stochastic Geometry
        // and its Applications, 3rd ed., §3.1), here the reach's, by the octave's own inversion.
        let expected = octave.density().value() * 1e-6 * octave.reach_area_m2;
        let mut fresh = 0.0_f64;
        let mut fillet = [0.0_f64; crate::synth::BAND_LEVELS];
        for i in 0..DIAMETER_SAMPLES {
            let along = f64::from(i) / f64::from(DIAMETER_SAMPLES - 1);
            let diameter = smallest * math::powf(upper / smallest, along);
            let shape = crater_shape(params, Metres::new(diameter));
            let fresh_profile = CraterProfile::filleted(shape, Metres::ZERO);
            fresh = num::max(fresh, largest_height(&fresh_profile));
            let mut previous: Option<(f64, f64)> = None;
            for (slot, &band) in fillet.iter_mut().zip(bands) {
                // Levels of one band (every level from the band limit's down) fillet alike.
                let change = match previous {
                    Some((b, c)) if b.total_cmp(&band).is_eq() => c,
                    _ => fillet_change(&shape, &fresh_profile, band),
                };
                previous = Some((band, change));
                *slot = num::max(*slot, change);
            }
        }
        out.push(OctaveMarks {
            smallest_m: smallest,
            expected,
            fresh_m: SAMPLED_MARGIN * fresh,
            fillet_m: fillet.map(|f| SAMPLED_MARGIN * f),
        });
    }
    out
}

/// One octave of coarse craters' shares in the level bound (R09.T7.c's rims): how many reach a
/// point and how far a sharpening changes between bands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RimMarks {
    /// μ, the expected number of the octave's craters whose reach holds a point, at saturation.
    pub(crate) expected: f64,
    /// δ at the coarse band: the largest change of any of its craters' profiles filleted over the
    /// coarse band from the fresh one, metres.
    pub(crate) coarse_m: f64,
    /// δ at each band level's band held to the coarse band, metres, by level.
    pub(crate) fillet_m: [f64; crate::synth::BAND_LEVELS],
}

/// The shares of the coarse craters of a body of crater contract `params` and mean radius
/// `radius_m` in the level bound: per diameter octave from the boundary diameter
/// `boundary_diameter_m` up to π R, with `coarse_band_m` the coarse band and `bands` each band
/// level's fillet half-width, metres.
///
/// The coarse pass draws an octave's coarse craters uniformly over the body at min(production,
/// saturation) (Design note 10), so Trask's saturation bounds their density whatever the
/// production, so long as the pass caps each of these octaves on its own (R09.T12.d's contract,
/// the plan's Risks). The reach is taken at its widest, (1 + a) R with a at its largest and R the
/// octave's upper rim radius, whatever the diameters' law within the octave. A coarse crater's
/// sharpening at a level of band w is g (F(w′) − F(`w_c`)) with w′ = min(w, `w_c`) (R09.T7.c), so it
/// changes between two levels by at most δ(w′) + δ(w″), and is at most δ(w′) + δ(`w_c`) at one.
#[must_use]
pub(crate) fn coarse_marks(
    params: &crate::craters::CraterParams,
    coarse_band_m: f64,
    boundary_diameter_m: f64,
    radius_m: f64,
    bands: &[f64; crate::synth::BAND_LEVELS],
) -> Vec<RimMarks> {
    let mut out = Vec::new();
    let mut lower = boundary_diameter_m;
    let reach = f64::midpoint(1.0, MAX_EXTERIOR_WIDTH);
    while lower < PI * radius_m {
        let upper = 2.0 * lower;
        let capped = crate::craters::saturation(Metres::new(lower)).value()
            - crate::craters::saturation(Metres::new(upper)).value();
        let expected = capped * 1e-6 * PI * (reach * upper) * (reach * upper);
        let (mut coarse, mut fillet) = (0.0_f64, [0.0_f64; crate::synth::BAND_LEVELS]);
        for i in 0..DIAMETER_SAMPLES {
            let along = f64::from(i) / f64::from(DIAMETER_SAMPLES - 1);
            let diameter = lower * math::powf(2.0, along);
            let shape = crater_shape(params, Metres::new(diameter));
            let fresh = CraterProfile::filleted(shape, Metres::ZERO);
            coarse = num::max(coarse, fillet_change(&shape, &fresh, coarse_band_m));
            let mut previous: Option<(f64, f64)> = None;
            for (slot, &band) in fillet.iter_mut().zip(bands) {
                let held = num::min(band, coarse_band_m);
                let change = match previous {
                    Some((b, c)) if b.total_cmp(&held).is_eq() => c,
                    _ => fillet_change(&shape, &fresh, held),
                };
                previous = Some((held, change));
                *slot = num::max(*slot, change);
            }
        }
        out.push(RimMarks {
            expected,
            coarse_m: SAMPLED_MARGIN * coarse,
            fillet_m: fillet.map(|f| SAMPLED_MARGIN * f),
        });
        lower = upper;
    }
    out
}

/// The largest |height| of `profile`, metres, sampled at its centre, its rim and
/// [`PROFILE_SAMPLES`] radii out to its reach.
#[must_use]
fn largest_height(profile: &CraterProfile) -> f64 {
    let reach = 1.0 + profile.shape.exterior_width();
    let mut largest = num::max(
        profile.height_and_slope(0.0).0.abs(),
        profile.height_and_slope(1.0).0.abs(),
    );
    for i in 0..PROFILE_SAMPLES {
        let r = reach * f64::from(i) / f64::from(PROFILE_SAMPLES - 1);
        largest = num::max(largest, profile.height_and_slope(r).0.abs());
    }
    largest
}

/// δ(w), the largest change from `fresh` of `shape`'s profile filleted over the half-width `band`
/// metres, sampled across its fillet's span, where the fillet and its compensating term change it,
/// and at the centre, where the faded cone changes it most (the module documentation).
#[must_use]
fn fillet_change(shape: &crate::craters::CraterShape, fresh: &CraterProfile, band: f64) -> f64 {
    let filleted = CraterProfile::filleted(*shape, Metres::new(band));
    let w = filleted.fillet;
    if w <= 0.0 {
        return 0.0;
    }
    let change = |r: f64| (filleted.height_and_slope(r).0 - fresh.height_and_slope(r).0).abs();
    let mut largest = change(0.0);
    for i in 0..=FILLET_SAMPLES {
        let r = 1.0 - w + 2.0 * w * f64::from(i) / f64::from(FILLET_SAMPLES);
        largest = num::max(largest, change(r));
    }
    largest
}

/// The least t found for which Chernoff's bound on Σ m N, the counts N independent and Poisson of
/// means μ, gives P(Σ m N ≥ t) ≤ e^(−`ln_inverse_tail`): t(θ) = (Σ μ (e^(θm) − 1) + ln(1 ÷ ε)) ÷ θ,
/// minimised over θ by 64 golden-section steps in ln θ from 10⁻³ to 64 over the largest m. Each
/// `terms` entry is (μ, m); terms of no mean or no share add nothing. Every θ gives a valid bound,
/// so the search's precision moves only how tight it is. Zero where no term adds anything.
#[must_use]
pub(crate) fn poisson_tail_bound(terms: &[(f64, f64)], ln_inverse_tail: f64) -> f64 {
    let live = |&(mu, m): &(f64, f64)| mu > 0.0 && m > 0.0;
    let largest = terms
        .iter()
        .filter(|t| live(t))
        .fold(0.0_f64, |a, &(_, m)| num::max(a, m));
    if largest <= 0.0 {
        return 0.0;
    }
    let bound_at = |ln_u: f64| {
        let theta = math::exp(ln_u) / largest;
        let mut sum = 0.0;
        for &(mu, m) in terms.iter().filter(|t| live(t)) {
            sum += mu * math::exp_m1(theta * m);
        }
        (sum + ln_inverse_tail) / theta
    };
    // 1 ÷ φ, the golden section.
    let inverse_phi = 0.5 * (5.0_f64.sqrt() - 1.0);
    let (mut lo, mut hi) = (math::ln(1e-3), math::ln(64.0));
    let mut a = hi - inverse_phi * (hi - lo);
    let mut b = lo + inverse_phi * (hi - lo);
    let (mut fa, mut fb) = (bound_at(a), bound_at(b));
    let mut best = num::min(fa, fb);
    for _ in 0..64 {
        if fa <= fb {
            hi = b;
            b = a;
            fb = fa;
            a = hi - inverse_phi * (hi - lo);
            fa = bound_at(a);
        } else {
            lo = a;
            a = b;
            fa = fb;
            b = lo + inverse_phi * (hi - lo);
            fb = bound_at(b);
        }
        best = num::min(best, num::min(fa, fb));
    }
    best
}

#[cfg(test)]
mod tests;
