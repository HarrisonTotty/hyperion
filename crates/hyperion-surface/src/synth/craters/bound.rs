//! The craters' terms in the level bound and the heights' range (plan R09, T8 and T8.d): what one
//! crater of each octave, small or coarse, can add to a height, or change in it between two
//! levels, and how many of an octave's craters reach a point.
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
//! The areas are discs and annuli in the chord the synthesis measures between spheroid points.
//! Each count takes the figure's [`chord_area_allowance`], (a ÷ c)², for the ground they hold on
//! a flattened body, which is their planar area only on a sphere (R09.T8.d).
//!
//! The coarse craters' rims (R09.T7.c) are counted not over their reach but over the ground their
//! fillets change ([`filleted_area_m2`]), since a sharpening, and its change between levels, is
//! nothing elsewhere: the fillet's span about the rim, and the central cone where the fillet
//! reaches into it (R09.T8.d).
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
//!
//! The coarse rims' shares are the gaps between two filleted profiles themselves,
//! sup |F(w′) − F(`w_c`)| at a level and sup |F(w′_n) − F(w′_m)| over every finer level m, not
//! δ(w′) + δ(`w_c`) through the fresh profile (R09.T8.d; [`profile_gap`]): sampled at every
//! breakpoint of both profiles, between which each is a polynomial, and across the wider span,
//! widened alike.

use core::f64::consts::PI;

use hyperion_base::math;
use hyperion_base::units::Metres;

use super::{CraterOctaves, CraterProfile, MAX_EXTERIOR_WIDTH, kept_span};
use crate::craters::crater_shape;
use crate::craters::shape::WIDEST_PEAK_FRACTION;
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
/// [module documentation](self)), with `bands` each band level's fillet half-width, metres, and
/// `allowance` the figure's [`chord_area_allowance`].
#[must_use]
pub(crate) fn octave_marks(
    octaves: &CraterOctaves,
    bands: &[f64; crate::synth::BAND_LEVELS],
    allowance: f64,
) -> Vec<OctaveMarks> {
    let params = &octaves.inputs.params;
    let list = &octaves.octaves;
    let mut out = Vec::with_capacity(list.len());
    for octave in list {
        let (smallest, upper) = kept_span(octave);
        // A marked Poisson field covers a point a Poisson number of times, of mean its density
        // times its marks' mean area (Chiu, Stoyan, Kendall and Mecke 2013, Stochastic Geometry
        // and its Applications, 3rd ed., §3.1), here the reach's, by the octave's own inversion,
        // with the figure's allowance for the ground a chord disc holds (R09.T8.d).
        let expected = octave.density().value() * 1e-6 * octave.reach_area_m2 * allowance;
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

/// One octave of coarse craters' shares in the level bound (R09.T7.c's rims): how many change a
/// point and how far a sharpening, or its change between bands, reaches.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RimMarks {
    /// μ at the coarse band: the expected number of the octave's craters whose coarse fillet
    /// changes their fresh profile at a point, at saturation ([`filleted_area_m2`], with the
    /// figure's [`chord_area_allowance`]). A crater's sharpening at any level lies there.
    pub(crate) expected: f64,
    /// μ at each band level's band held to the coarse band alike, by level: the sharpening's
    /// change between that level and any finer one lies where that level's fillet changes the
    /// fresh profile.
    pub(crate) expected_at: [f64; crate::synth::BAND_LEVELS],
    /// The largest |F(w′) − F(`w_c`)| of any of its craters at each band level's band w′ held to
    /// the coarse band `w_c`, metres, by level: one crater's largest sharpening there before its
    /// relief, sampled ([`profile_gap`]) and widened by [`SAMPLED_MARGIN`]; zero at a level whose
    /// band is at or beyond the coarse band.
    pub(crate) height_m: [f64; crate::synth::BAND_LEVELS],
    /// The largest |F(w′_n) − F(w′_m)| of any of its craters over every finer level m, at each
    /// band level n, metres: one crater's largest change in its sharpening from level n on,
    /// sampled and widened alike; zero where no finer level fillets with another band.
    pub(crate) change_m: [f64; crate::synth::BAND_LEVELS],
}

/// How the rims' shares are sampled: at `diameters` even steps in ln D across the octave, each
/// profile pair at both profiles' breakpoints and the ends of `steps` intervals across the wider
/// fillet's span, each supremum widened by `margin`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RimSampling {
    diameters: u32,
    steps: u32,
    margin: f64,
}

impl RimSampling {
    /// The bound's: [`DIAMETER_SAMPLES`], [`FILLET_SAMPLES`] and [`SAMPLED_MARGIN`].
    pub(crate) const BOUND: Self = Self::new(DIAMETER_SAMPLES, FILLET_SAMPLES, SAMPLED_MARGIN);

    /// A sampling of `diameters` diameters, `steps` intervals a span and the widening `margin`.
    ///
    /// # Panics
    ///
    /// If `diameters` is under 2 or `steps` under 1, which would sample no octave's ends or no
    /// span, or `margin` is under 1.
    #[must_use]
    pub(crate) const fn new(diameters: u32, steps: u32, margin: f64) -> Self {
        assert!(
            diameters >= 2 && steps >= 1 && margin >= 1.0,
            "a rims' sampling takes at least 2 diameters, 1 step and a margin of 1"
        );
        Self {
            diameters,
            steps,
            margin,
        }
    }
}

/// The figure's allowance for the ground a chord disc or annulus holds, k = (a ÷ c)² for an oblate
/// figure of equatorial radius a and polar radius c, the larger over the smaller in general: the
/// ground within a chord r of a point is at most k π r², and the ground between chords r₁ and r₂
/// at most π (k r₂² − r₁²) (R09.T8.d; decision-r09-t8d-reach §1 and its review).
///
/// The craters' counts take a crater's reach, or its fillet's span, as a disc or an annulus in the
/// chord the synthesis measures between spheroid points. On a sphere the ground within a chord c of
/// a point is π c² exactly, for c up to its diameter (Archimedes, On the Sphere and Cylinder I,
/// props. 42–43), and less beyond. The spheroid is the image of the sphere of radius a under
/// M = diag(1, 1, c ÷ a), which enlarges no area and shortens no chord by more than the factor
/// c ÷ a. So the spheroid's ground within r of a point is the image of sphere ground within at
/// most r a ÷ c, and its ground between r₁ and r₂ the image of sphere ground between r₁ and
/// r₂ a ÷ c, neither larger than the sphere ground it comes from. The worst disc is the whole body
/// seen from a pole, 1.071, 1.153,
/// 1.359 and 1.647 times π r² at f = 0.05, 0.1, 0.2 and 0.3, an excess of 4f ÷ 3 at small f,
/// against k's 1.108, 1.235, 1.563 and 2.041; a thin ring near the pole's chord 2c holds up to
/// a² ÷ (2c² − a²) times its planar area, 1.61 at f = 0.1, which only the annulus form bounds
/// (`chord_discs_and_annuli_hold_within_the_allowance` checks both by quadrature). The reach discs
/// before R09.T8.d had the same gap, and the 1 + 2f ruled first held only to f ≈ 0.26 and for
/// discs alone.
#[must_use]
pub(crate) fn chord_area_allowance(figure: &crate::spheroid::Spheroid) -> f64 {
    let (a, c) = (figure.equatorial_radius_m, figure.polar_radius_m);
    let ratio = num::max(a, c) / num::min(a, c);
    ratio * ratio
}

/// The shares of the coarse craters of a body of crater contract `params` and mean radius
/// `radius_m` in the level bound: per diameter octave from the boundary diameter
/// `boundary_diameter_m` up to π R, with `coarse_band_m` the coarse band, `bands` each band level's
/// fillet half-width, metres, and `allowance` the figure's [`chord_area_allowance`].
///
/// The coarse pass draws an octave's coarse craters uniformly over the body at min(production,
/// saturation) (Design note 10), so Trask's saturation bounds their density whatever the
/// production, so long as the pass caps each of these octaves on its own (R09.T12.d's contract,
/// the plan's Risks). A coarse crater's sharpening at a level of band w is g (F(w′) − F(`w_c`))
/// with w′ = min(w, `w_c`) and g ≤ 1 (R09.T7.c), so it is at most |F(w′) − F(`w_c`)| at one level
/// and changes by at most |F(w′) − F(w″)| between two, each sampled directly ([`profile_gap`];
/// R09.T8.d) rather than through the fresh profile. A fillet changes the fresh profile only on its
/// span and in the central cone it reaches (the [module documentation](self), "One crater's
/// shares"), and a narrower band's change lies within a wider one's, so the craters counted at a
/// point are those whose coarse fillet changes it, and for the change from a level on, those whose
/// fillet at that level's band does ([`filleted_area_m2`]; R09.T8.d), not every crater whose reach
/// holds it.
#[must_use]
pub(crate) fn coarse_marks(
    params: &crate::craters::CraterParams,
    coarse_band_m: f64,
    boundary_diameter_m: f64,
    radius_m: f64,
    bands: &[f64; crate::synth::BAND_LEVELS],
    allowance: f64,
) -> Vec<RimMarks> {
    coarse_marks_sampled(
        params,
        coarse_band_m,
        boundary_diameter_m,
        radius_m,
        bands,
        allowance,
        RimSampling::BOUND,
    )
}

/// [`coarse_marks`] sampled by `sampling`.
#[must_use]
pub(crate) fn coarse_marks_sampled(
    params: &crate::craters::CraterParams,
    coarse_band_m: f64,
    boundary_diameter_m: f64,
    radius_m: f64,
    bands: &[f64; crate::synth::BAND_LEVELS],
    allowance: f64,
    sampling: RimSampling,
) -> Vec<RimMarks> {
    const LEVELS: usize = crate::synth::BAND_LEVELS;
    // Each level's band held to the coarse band, and its place among the distinct ones, which run
    // from the coarse band itself, narrowing with the level.
    let held = bands.map(|band| num::min(band, coarse_band_m));
    let mut distinct = vec![coarse_band_m];
    let mut group = [0_usize; LEVELS];
    for (slot, &band) in group.iter_mut().zip(&held) {
        if distinct
            .last()
            .is_some_and(|last| last.total_cmp(&band).is_ne())
        {
            distinct.push(band);
        }
        *slot = distinct.len() - 1;
    }
    let mut out = Vec::new();
    let mut profiles = Vec::with_capacity(distinct.len());
    let mut lower = boundary_diameter_m;
    while lower < PI * radius_m {
        let upper = 2.0 * lower;
        let capped = crate::craters::saturation(Metres::new(lower)).value()
            - crate::craters::saturation(Metres::new(upper)).value();
        let count = |band: f64| capped * 1e-6 * filleted_area_m2(lower, band, allowance);
        let expected = count(coarse_band_m);
        let expected_at = held.map(count);
        // gap[j][k]: the largest |F(distinct j) − F(distinct k)| over the sampled craters, j < k.
        let mut gap = vec![vec![0.0_f64; distinct.len()]; distinct.len()];
        for i in 0..sampling.diameters {
            let along = f64::from(i) / f64::from(sampling.diameters - 1);
            let shape = crater_shape(params, Metres::new(lower * math::powf(2.0, along)));
            profiles.clear();
            profiles.extend(
                distinct
                    .iter()
                    .map(|&band| CraterProfile::filleted(shape, Metres::new(band))),
            );
            for (j, wide) in profiles.iter().enumerate() {
                for (k, narrow) in profiles.iter().enumerate().skip(j + 1) {
                    gap[j][k] = num::max(gap[j][k], profile_gap(wide, narrow, sampling.steps));
                }
            }
        }
        // Each level's largest change to any finer one: the gap to every narrower band.
        let finer: Vec<f64> = (0..distinct.len())
            .map(|j| {
                gap[j]
                    .iter()
                    .skip(j + 1)
                    .fold(0.0_f64, |m, &g| num::max(m, g))
            })
            .collect();
        out.push(RimMarks {
            expected,
            expected_at,
            height_m: group.map(|j| sampling.margin * gap[0][j]),
            change_m: group.map(|j| sampling.margin * finer[j]),
        });
        lower = upper;
    }
    out
}

/// The largest |a − b| of two profiles of one crater, `wide` filleted over the wider band and
/// `narrow` over the narrower, metres: sampled at every breakpoint of both (each span's ends, the
/// rim, the cone's base, the floor's edge, the centre and the reach), whose pieces are polynomials
/// between them, and at the ends of `steps` even intervals across the wider fillet's span, where
/// both differ from the fresh profile (the [module documentation](self), "One crater's shares").
#[must_use]
fn profile_gap(wide: &CraterProfile, narrow: &CraterProfile, steps: u32) -> f64 {
    let gap = |r: f64| (wide.height_and_slope(r).0 - narrow.height_and_slope(r).0).abs();
    let mut largest = 0.0_f64;
    for r in wide.breakpoints().into_iter().chain(narrow.breakpoints()) {
        largest = num::max(largest, gap(r));
    }
    let w = wide.fillet;
    if w > 0.0 {
        for i in 0..=steps {
            largest = num::max(
                largest,
                gap(1.0 - w + 2.0 * w * f64::from(i) / f64::from(steps)),
            );
        }
    }
    largest
}

/// The widest ground, square metres, over which a fillet of half-width `band_m` metres changes the
/// fresh profile of a crater of the diameter octave from `lower_m` to twice it, whatever its
/// shape, on a figure of [`chord_area_allowance`] `allowance`: zero for no band (R09.T8.d).
///
/// A crater of rim radius R ≤ `lower_m` filleted over the band holds the fillet at
/// w = min(band ÷ R, min(1, a)) rim radii ([`CraterProfile::filleted`]), so the fillet's
/// half-width in metres, w R, is at most min(band, R). The profile then differs from the fresh one
/// on the span (1 − w, 1 + w) rim radii alone, an annulus of 4π R (w R), unless the span's inner end
/// lies within the central cone's base `c_p`, where the faded cone changes the disc within it too,
/// π (R + w R)². The cone's base is below [`WIDEST_PEAK_FRACTION`] of the rim radius on every
/// crater, and w is at most min(band ÷ R₀, 1) with R₀ the octave's least rim radius, so where
/// 1 − that is at least the bound no crater of the octave reaches its cone, and the annulus holds.
/// Both are at most the reach's disc, π ((1 + a) R)², at a's widest, [`MAX_EXTERIOR_WIDTH`], which
/// is the area taken where it is less. On the figure the disc of chord r holds at most k π r² and
/// the annulus from r₁ to r₂ at most π (k r₂² − r₁²), k the allowance, each still largest at the
/// octave's widest rim radius and fillet; on a sphere, k = 1, they are π r² and 4π R (w R).
#[must_use]
fn filleted_area_m2(lower_m: f64, band_m: f64, allowance: f64) -> f64 {
    if band_m <= 0.0 {
        return 0.0;
    }
    let (least_rim_m, rim_m) = (0.5 * lower_m, lower_m);
    let half_m = num::min(band_m, rim_m);
    let widest = num::min(band_m / least_rim_m, 1.0);
    let (outer_m, inner_m) = (rim_m + half_m, rim_m - half_m);
    let area = if 1.0 - widest < WIDEST_PEAK_FRACTION {
        PI * allowance * outer_m * outer_m
    } else {
        PI * (allowance * outer_m * outer_m - inner_m * inner_m)
    };
    let reach_m = (1.0 + MAX_EXTERIOR_WIDTH) * rim_m;
    num::min(area, PI * allowance * reach_m * reach_m)
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
