//! Initial mass functions, the mass bands of the placement layers, and the band shares.
//!
//! Placement draws each system's primary from an initial mass function, within the band of
//! primary *initial* mass that its layer owns (brainstorm, "Sizing the layers"). Two functions are
//! supported behind one interface, [`MassFunction`]: [`Chabrier`]'s (2003) system function with
//! its branch above 1 M☉ scaled, the default, and [`Kroupa`]'s (2001). Which one a universe uses
//! belongs to its generator version (plan 02, Design note 5). The share of systems in each band is
//! computed from the function by integration ([`BandShares`]) and never written down as a
//! constant.
//!
//! Masses here are bare `f64`s in solar masses, on the stellar range
//! [`MASS_LIMIT_LO`]–[`MASS_LIMIT_HI`], 0.08–150 M☉. Below it each function has a substellar
//! branch ([`MassFunction::substellar_quantile_in`]), whose shape alone gives the brown dwarfs'
//! masses (plan 13, P13.T1). The two substellar bands, [`MassBand::BrownDwarf`] and
//! [`MassBand::RoguePlanet`], are counted per system by plan 13's abundances, not by the stellar
//! normalisation.

use std::error::Error;
use std::fmt;

use super::substellar::{BROWN_DWARF_MIN_MSUN, ROGUE_PLANET_MAX_MSUN, ROGUE_PLANET_MIN_MSUN};
use crate::id::Layer;
use crate::math;
use crate::rng::Stream;

/// The edges of the five stellar mass bands, M☉: layers A to E own `[edges[i], edges[i + 1]]`.
///
/// This is the single source of truth for the bands (brainstorm, "Sizing the layers"): 0.08 M☉ is
/// the hydrogen-burning limit, 150 M☉ the upper mass limit, and 0.75 M☉ the mass below which
/// nothing has left the main sequence at any metallicity.
pub const MASS_BAND_EDGES: [f64; 6] = [0.08, 0.5, 0.75, 2.5, 8.0, 150.0];

/// The lowest stellar mass, M☉: the hydrogen-burning limit, and the lower end of band A.
pub const MASS_LIMIT_LO: f64 = MASS_BAND_EDGES[0];

/// The highest stellar mass, M☉: the upper mass limit, and the upper end of band E.
pub const MASS_LIMIT_HI: f64 = MASS_BAND_EDGES[5];

/// Iterations of the bisection that inverts the log-normal branch: enough to narrow the bracket
/// in log₁₀ m, at most three decades wide, to its last bit.
const LOG_NORMAL_BISECTIONS: u32 = 64;

/// The most pieces a mass function here has: [`Kroupa`]'s two power laws, [`Chabrier`]'s
/// log-normal and power law.
const MAX_PIECES: usize = 2;

/// Within this distance of 1 a power-law exponent takes the logarithmic form, as the samplers of
/// [`rng`](crate::rng) do.
const EXPONENT_ONE_TOLERANCE: f64 = 1e-8;

/// A band of primary initial mass: the band that one stellar layer of the placement grid owns.
///
/// | Band | Layer | Cell   | Primary initial mass |
/// | ---- | ----- | ------ | -------------------- |
/// | A    | A     | 8 ly   | 0.08–0.5 M☉          |
/// | B    | B     | 16 ly  | 0.5–0.75 M☉          |
/// | C    | C     | 32 ly  | 0.75–2.5 M☉          |
/// | D    | D     | 64 ly  | 2.5–8 M☉             |
/// | E    | E     | 128 ly | 8–150 M☉             |
///
/// Plan 13 adds the two substellar layers' bands, which are not part of the stellar
/// normalisation: [`BandShares`] and [`MassFunction::sample_in_band`] refuse them, and the share
/// matrix holds objects per system for them ([`ShareMatrix`](super::shares::ShareMatrix)).
///
/// | Band        | Layer | Cell | Mass                   |
/// | ----------- | ----- | ---- | ---------------------- |
/// | BrownDwarf  | F     | 16 ly | 13 `M_Jup`–0.08 M☉      |
/// | RoguePlanet | G     | 4 ly  | ⅓ M⊕–13 `M_Jup`         |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MassBand {
    /// 0.08–0.5 M☉, layer A.
    A,
    /// 0.5–0.75 M☉, layer B.
    B,
    /// 0.75–2.5 M☉, layer C.
    C,
    /// 2.5–8 M☉, layer D.
    D,
    /// 8–150 M☉, layer E.
    E,
    /// Free-floating brown dwarfs, 13 `M_Jup`–0.08 M☉, layer F (plan 13).
    BrownDwarf,
    /// Rogue planets, ⅓ M⊕–13 `M_Jup`, layer G (plan 13).
    RoguePlanet,
}

impl MassBand {
    /// Every stellar band, from the lightest: the five of the stellar normalisation.
    pub const ALL: [Self; 5] = [Self::A, Self::B, Self::C, Self::D, Self::E];

    /// The two substellar bands, heavier first, in the order the range query walks their layers.
    pub const SUBSTELLAR: [Self; 2] = [Self::BrownDwarf, Self::RoguePlanet];

    /// The band's index: its place in [`MassBand::ALL`] and in [`MASS_BAND_EDGES`], 0–4, for the
    /// stellar bands, and 5 and 6, its layer's value, for the substellar ones.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
            Self::C => 2,
            Self::D => 3,
            Self::E => 4,
            Self::BrownDwarf => 5,
            Self::RoguePlanet => 6,
        }
    }

    /// Whether the band is one of the five of the stellar mass function.
    #[must_use]
    pub const fn is_stellar(self) -> bool {
        match self {
            Self::A | Self::B | Self::C | Self::D | Self::E => true,
            Self::BrownDwarf | Self::RoguePlanet => false,
        }
    }

    /// The band's lower edge, M☉: 0.0124 for the brown dwarfs and 1.0 × 10⁻⁶ for the rogue
    /// planets.
    #[must_use]
    pub const fn lo(self) -> f64 {
        match self {
            Self::A | Self::B | Self::C | Self::D | Self::E => MASS_BAND_EDGES[self.index()],
            Self::BrownDwarf => BROWN_DWARF_MIN_MSUN,
            Self::RoguePlanet => ROGUE_PLANET_MIN_MSUN,
        }
    }

    /// The band's upper edge, M☉: each substellar band's is the lower edge of the band above.
    #[must_use]
    pub const fn hi(self) -> f64 {
        match self {
            Self::A | Self::B | Self::C | Self::D | Self::E => MASS_BAND_EDGES[self.index() + 1],
            Self::BrownDwarf => MASS_LIMIT_LO,
            Self::RoguePlanet => ROGUE_PLANET_MAX_MSUN,
        }
    }

    /// The layer that owns this band.
    #[must_use]
    pub const fn layer(self) -> Layer {
        match self {
            Self::A => Layer::A,
            Self::B => Layer::B,
            Self::C => Layer::C,
            Self::D => Layer::D,
            Self::E => Layer::E,
            Self::BrownDwarf => Layer::BrownDwarf,
            Self::RoguePlanet => Layer::RoguePlanet,
        }
    }

    /// The band a layer owns: every layer owns one, the substellar layers theirs since plan 13.
    #[must_use]
    pub const fn of_layer(layer: Layer) -> Self {
        match layer {
            Layer::A => Self::A,
            Layer::B => Self::B,
            Layer::C => Self::C,
            Layer::D => Self::D,
            Layer::E => Self::E,
            Layer::BrownDwarf => Self::BrownDwarf,
            Layer::RoguePlanet => Self::RoguePlanet,
        }
    }
}

impl From<MassBand> for Layer {
    fn from(band: MassBand) -> Self {
        band.layer()
    }
}

impl From<Layer> for MassBand {
    fn from(layer: Layer) -> Self {
        Self::of_layer(layer)
    }
}

/// An initial mass function on the stellar range, 0.08–150 M☉.
///
/// Implementors give the unnormalised density, its integral and its inverse; everything else,
/// band shares and draws, is built on those three. Masses are in M☉.
pub trait MassFunction: fmt::Debug + Send + Sync {
    /// The unnormalised density ξ(m) per unit mass at `m`; 0 outside 0.08–150 M☉.
    fn pdf(&self, m: f64) -> f64;

    /// `∫ ξ(m) dm` over `[lo, hi]` intersected with 0.08–150 M☉; 0 if they do not overlap.
    fn integral(&self, lo: f64, hi: f64) -> f64;

    /// The inverse of the distribution restricted to `[lo, hi]` (intersected with the stellar
    /// range): the mass below which a fraction `u` of that range's integral lies, for `u` in
    /// `[0, 1]`. `u = 0` gives the lower end and `u = 1` the upper.
    fn quantile_in(&self, lo: f64, hi: f64, u: f64) -> f64;

    /// Masses, M☉, where ξ has a kink or a jump, in ascending order, for quadratures that put
    /// panel edges there. None by default.
    fn breaks(&self) -> &[f64] {
        &[]
    }

    /// `∫ ξ dm` of the function's substellar branch over `[lo, hi]` intersected with
    /// [`SUBSTELLAR_BRANCH_LO`]–0.08 M☉; 0 if they do not overlap.
    ///
    /// The branch is a separate method, and no stellar method reads it, so adding it changed no
    /// stellar output (plan 13, P13.T1). Only its shape is used: its normalisation is arbitrary,
    /// and the brown dwarfs' count is plan 13's abundance.
    fn substellar_integral(&self, lo: f64, hi: f64) -> f64;

    /// The inverse of the substellar branch restricted to `[lo, hi]` (intersected with the
    /// branch's range), as [`quantile_in`](Self::quantile_in) is of the stellar range.
    fn substellar_quantile_in(&self, lo: f64, hi: f64, u: f64) -> f64;

    /// A primary mass drawn from the function restricted to `band`: one uniform from `stream`,
    /// then [`quantile_in`](Self::quantile_in). One word.
    ///
    /// # Panics
    ///
    /// For a substellar band, which the stellar function does not cover: plan 13 draws those
    /// masses from their own laws.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::imf::{Kroupa, MassBand, MassFunction};
    /// use hyperion_sim::rng::{ObjectKey, Stream, tags};
    ///
    /// let mut stream = Stream::open(Seed::new(3), tags::SELFTEST_STREAM, ObjectKey::galaxy());
    /// let mass = Kroupa.sample_in_band(MassBand::C, &mut stream);
    /// assert!((0.75..=2.5).contains(&mass));
    /// assert_eq!(stream.position(), 1);
    /// ```
    fn sample_in_band(&self, band: MassBand, stream: &mut Stream) -> f64 {
        assert!(
            band.is_stellar(),
            "the stellar mass function has no {band:?} band: plan 13 draws substellar masses"
        );
        self.quantile_in(band.lo(), band.hi(), stream.uniform())
    }
}

/// The lower end of every mass function's substellar branch, M☉: 0.01, the lower end of Kroupa's
/// (2001, MNRAS 322, 231, eq. 2) α₀ segment. It lies below the brown dwarfs' 0.0124 M☉, so their
/// band is inside the branch.
pub const SUBSTELLAR_BRANCH_LO: f64 = 0.01;

/// `∫` of a substellar branch's piece over `[lo, hi]` intersected with the piece's range.
fn branch_integral(piece: &Piece, lo: f64, hi: f64) -> f64 {
    overlap(piece, lo, hi).map_or(0.0, |(a, b)| piece.integral(a, b))
}

/// The restricted inverse of a substellar branch's piece.
fn branch_quantile(piece: &Piece, lo: f64, hi: f64, u: f64) -> f64 {
    let lo = lo.max(piece.lo());
    let hi = hi.min(piece.hi());
    if lo >= hi {
        return lo;
    }
    piece.invert(lo, hi, u.clamp(0.0, 1.0)).clamp(lo, hi)
}

/// One closed-form piece of a mass function on `[lo, hi]`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Piece {
    /// `coefficient × m^−exponent`.
    Power {
        lo: f64,
        hi: f64,
        coefficient: f64,
        exponent: f64,
    },
    /// `coefficient × exp(−(log₁₀ m − centre)² ÷ 2 width²) ÷ (m ln 10)`: a log-normal in log₁₀ m
    /// per unit log₁₀ m, written per unit mass.
    LogNormal {
        lo: f64,
        hi: f64,
        coefficient: f64,
        centre: f64,
        width: f64,
    },
}

impl Piece {
    fn lo(&self) -> f64 {
        match *self {
            Self::Power { lo, .. } | Self::LogNormal { lo, .. } => lo,
        }
    }

    fn hi(&self) -> f64 {
        match *self {
            Self::Power { hi, .. } | Self::LogNormal { hi, .. } => hi,
        }
    }

    fn pdf(&self, m: f64) -> f64 {
        match *self {
            Self::Power {
                coefficient,
                exponent,
                ..
            } => coefficient * math::powf(m, -exponent),
            Self::LogNormal {
                coefficient,
                centre,
                width,
                ..
            } => {
                let d = math::log10(m) - centre;
                coefficient * math::exp(-d * d / (2.0 * width * width))
                    / (m * core::f64::consts::LN_10)
            }
        }
    }

    /// `∫ₐᵇ` of the piece, for `lo ≤ a ≤ b ≤ hi`.
    fn integral(&self, a: f64, b: f64) -> f64 {
        match *self {
            Self::Power {
                coefficient,
                exponent,
                ..
            } => {
                let g = 1.0 - exponent;
                let log_ratio = math::ln(b / a);
                if g.abs() >= EXPONENT_ONE_TOLERANCE {
                    coefficient * math::powf(a, g) * math::exp_m1(g * log_ratio) / g
                } else {
                    coefficient * log_ratio
                }
            }
            Self::LogNormal {
                coefficient,
                centre,
                width,
                ..
            } => {
                let scale = width * core::f64::consts::SQRT_2;
                let z = |m: f64| (math::log10(m) - centre) / scale;
                coefficient
                    * width
                    * (0.5 * core::f64::consts::PI).sqrt()
                    * (math::erf(z(b)) - math::erf(z(a)))
            }
        }
    }

    /// The mass `x` in `[a, b]` with `∫ₐˣ = fraction × ∫ₐᵇ`, for `fraction` in `[0, 1]`.
    fn invert(&self, a: f64, b: f64, fraction: f64) -> f64 {
        // The ends exactly: a bisection whose target sits on its bracket's end would drift away.
        if fraction <= 0.0 {
            return a;
        }
        if fraction >= 1.0 {
            return b;
        }
        let x = match *self {
            Self::Power { exponent, .. } => {
                let g = 1.0 - exponent;
                let log_ratio = math::ln(b / a);
                if g.abs() >= EXPONENT_ONE_TOLERANCE {
                    a * math::exp(math::ln_1p(fraction * math::exp_m1(g * log_ratio)) / g)
                } else {
                    a * math::exp(fraction * log_ratio)
                }
            }
            Self::LogNormal { centre, width, .. } => {
                let scale = width * core::f64::consts::SQRT_2;
                let erf_a = math::erf((math::log10(a) - centre) / scale);
                let erf_b = math::erf((math::log10(b) - centre) / scale);
                let target = erf_a + fraction * (erf_b - erf_a);
                let log_m = invert_erf(centre, scale, target, math::log10(a), math::log10(b));
                math::exp10(log_m)
            }
        };
        x.clamp(a, b)
    }
}

/// [`bisect`](super::quad::bisect) of `f(x) = erf((x − centre) ÷ scale) − target` on `[lo, hi]` over
/// [`LOG_NORMAL_BISECTIONS`] halvings, bit for bit, from about a fifth of its evaluations (perf08).
///
/// Placement inverts the log-normal for every primary of layers A to C and every brown dwarf, and
/// the bisection's 65 error functions were the largest part of such a candidate's cost. Its result
/// is fixed by the sign of `f` at each midpoint, so any midpoint whose sign is certain need not be
/// evaluated:
///
/// - A computed `f(x)` has the sign of `ê − target`, where `ê` is the computed error function:
///   the subtraction is correctly rounded. `ê` lies within `E` of the exact `erf(z(x))`, `E`
///   covering the error function's own rounding and the rounding of `z = (x − centre) ÷ scale`,
///   with a wide margin.
/// - The exact `erf(z(x)) − target` rises with `x` at a slope of at least `s`, the smaller of
///   `erf′(z) ÷ scale` at the bracket's ends (`erf′` is largest at 0 and falls on either side).
/// - So with `r` an estimate of the root, found from the normal quantile, and `|f(r)|` measured,
///   every root of the exact function lies within `(|f(r)| + E) ÷ s` of `r`, and the computed
///   sign is certain wherever the exact `f` is further than `E` from 0: below `r − d` it is
///   negative and above `r + d` it is not, for `d = 2 (|f(r)| + 2E) ÷ s`.
///
/// Midpoints inside `[r − d, r + d]`, about twelve of them from `d` of some 10⁻¹³ down to the last
/// bit, are evaluated, as are all of them when the estimate is poor or not a number. The loop also
/// stops once a halving leaves the bracket as it was, since every later one would repeat it.
/// `the_log_normal_inversion_is_the_bisection_bit_for_bit` holds it to the bisection bit for bit.
fn invert_erf(centre: f64, scale: f64, target: f64, lo: f64, hi: f64) -> f64 {
    let f = |x: f64| math::erf((x - centre) / scale) - target;
    let (mut lo, mut hi) = (lo, hi);
    // The bisection's orientation, as `bisect` takes it; the certain signs below assume the
    // lower end is negative, which it is for every target above the error function there.
    let lo_negative = f(lo) < 0.0;
    let (certain_below, certain_above) = if lo_negative {
        certain_signs(centre, scale, target, lo, hi)
    } else {
        (f64::NAN, f64::NAN)
    };
    for _ in 0..LOG_NORMAL_BISECTIONS {
        let mid = lo + 0.5 * (hi - lo);
        let negative = if mid < certain_below {
            true
        } else if mid > certain_above {
            false
        } else {
            f(mid) < 0.0
        };
        let (next_lo, next_hi) = if negative == lo_negative {
            (mid, hi)
        } else {
            (lo, mid)
        };
        if next_lo.total_cmp(&lo).is_eq() && next_hi.total_cmp(&hi).is_eq() {
            break;
        }
        (lo, hi) = (next_lo, next_hi);
    }
    lo + 0.5 * (hi - lo)
}

/// For [`invert_erf`]: the points below which `erf((x − centre) ÷ scale) − target` is certainly
/// computed negative and above which it is certainly not, over `[lo, hi]`; NaNs, which decide
/// nothing, when no estimate of the root is to be had.
fn certain_signs(centre: f64, scale: f64, target: f64, lo: f64, hi: f64) -> (f64, f64) {
    let p = f64::midpoint(1.0, target);
    if !(p > 0.0 && p < 1.0) {
        return (f64::NAN, f64::NAN);
    }
    // erf(z) = target where Φ(z √2) = (1 + target) ÷ 2.
    let root = centre + scale * math::normal_quantile(p) * core::f64::consts::FRAC_1_SQRT_2;
    // The slope bound holds over the bracket, so an estimate far outside it (which the normal
    // quantile's accuracy rules out, but nothing here should rest on) decides nothing.
    let width = hi - lo;
    if !(root >= lo - width && root <= hi + width) {
        return (f64::NAN, f64::NAN);
    }
    let (z_lo, z_hi) = ((lo - centre) / scale, (hi - centre) / scale);
    let z_most = z_lo.abs().max(z_hi.abs());
    // The error function's rounding and that of z, each within a few ulps, taken sixteen times
    // over: 16 × 2⁻⁵² (1 + |z|) against a function of size at most 1 with slope at most 1.13.
    let error = 16.0 * f64::EPSILON * (1.0 + z_most);
    // erf′(z) = 2 ÷ √π e^(−z²), halved against the rounding of its exponential.
    let slope = 0.5 * core::f64::consts::FRAC_2_SQRT_PI * math::exp(-z_most * z_most) / scale;
    let at_root = (math::erf((root - centre) / scale) - target).abs();
    let reach = 2.0 * (at_root + 2.0 * error) / slope;
    (root - reach, root + reach)
}

/// ξ at `m` for a list of contiguous pieces covering 0.08–150 M☉; 0 outside.
fn pieces_pdf(pieces: &[Piece], m: f64) -> f64 {
    if !(MASS_LIMIT_LO..=MASS_LIMIT_HI).contains(&m) {
        return 0.0;
    }
    let piece = pieces
        .iter()
        .find(|p| m < p.hi())
        .unwrap_or(&pieces[pieces.len() - 1]);
    piece.pdf(m)
}

/// The overlap of `[lo, hi]` with piece `p`, if it is not empty.
fn overlap(p: &Piece, lo: f64, hi: f64) -> Option<(f64, f64)> {
    let a = lo.max(p.lo());
    let b = hi.min(p.hi());
    (a < b).then_some((a, b))
}

/// `∫ ξ` over `[lo, hi]`, summed piece by piece in order.
fn pieces_integral(pieces: &[Piece], lo: f64, hi: f64) -> f64 {
    pieces
        .iter()
        .filter_map(|p| overlap(p, lo, hi).map(|(a, b)| p.integral(a, b)))
        .fold(0.0, |sum, x| sum + x)
}

/// The restricted inverse of [`MassFunction::quantile_in`] for a list of pieces.
fn pieces_quantile(pieces: &[Piece], lo: f64, hi: f64, u: f64) -> f64 {
    let lo = lo.max(MASS_LIMIT_LO);
    let hi = hi.min(MASS_LIMIT_HI);
    if lo >= hi {
        return lo;
    }
    let u = u.clamp(0.0, 1.0);
    // Placement draws a mass for every system, so this allocates nothing. Each overlap's integral
    // is taken once and summed as `pieces_integral` sums it, in order from 0, which is its total
    // bit for bit without integrating twice (perf08: a third of a power-law draw's cost).
    assert!(
        pieces.len() <= MAX_PIECES,
        "a mass function of {} pieces",
        pieces.len()
    );
    let mut overlaps = [None; MAX_PIECES];
    for (slot, found) in overlaps.iter_mut().zip(
        pieces
            .iter()
            .filter_map(|p| overlap(p, lo, hi).map(|(a, b)| (p, a, b, p.integral(a, b)))),
    ) {
        *slot = Some(found);
    }
    let total = overlaps
        .iter()
        .flatten()
        .fold(0.0, |sum, &(_, _, _, mass)| sum + mass);
    let mut remaining = u * total;
    // The pieces are contiguous and cover the stellar range, so the overlap that reaches `hi` is
    // the last one.
    for &(piece, a, b, mass) in overlaps.iter().flatten() {
        if remaining <= mass || b >= hi {
            let fraction = if mass > 0.0 {
                (remaining / mass).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return piece.invert(a, b, fraction).clamp(lo, hi);
        }
        remaining -= mass;
    }
    unreachable!("the pieces cover the stellar range, so one overlap reaches its upper end")
}

/// Kroupa's (2001) initial mass function on the stellar range: ξ ∝ m^−1.3 on 0.08–0.5 M☉ and
/// m^−2.3 above, continuous at 0.5 M☉.
///
/// The exponents are α₁ = 1.3 and α₂ = 2.3 of Kroupa (2001, MNRAS 322, 231, eq. 2); the segment
/// below 0.08 M☉ is substellar and not part of this function. The density is `m^−1.3` below
/// 0.5 M☉ and `0.5 m^−2.3` from there, which meet at 0.5 M☉.
///
/// Used for primaries with companions at the observed frequencies, it reproduces the observed mix
/// of all stars, which is why it is the default (brainstorm, "Sizing the layers").
///
/// # Examples
///
/// The share of systems in layer A's band, 76%:
///
/// ```
/// use hyperion_sim::galaxy::imf::{BandShares, Kroupa, MassBand};
///
/// let shares = BandShares::of(&Kroupa);
/// assert!((shares.share(MassBand::A) - 0.76).abs() < 0.005);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Kroupa;

impl Kroupa {
    /// The break between the two slopes, M☉.
    pub const BREAK: f64 = 0.5;

    /// The exponent below the break.
    pub const LOW_EXPONENT: f64 = 1.3;

    /// The exponent above the break.
    pub const HIGH_EXPONENT: f64 = 2.3;

    /// The exponent of the substellar branch, 0.01–0.08 M☉: α₀ = 0.3 (Kroupa 2001, eq. 2).
    pub const SUBSTELLAR_EXPONENT: f64 = 0.3;

    /// The substellar branch, ξ ∝ m^−0.3 on 0.01–0.08 M☉, with an arbitrary coefficient: only its
    /// shape is used (plan 13, Design note 6).
    const SUBSTELLAR: Piece = Piece::Power {
        lo: SUBSTELLAR_BRANCH_LO,
        hi: MASS_LIMIT_LO,
        coefficient: 1.0,
        exponent: Self::SUBSTELLAR_EXPONENT,
    };

    const PIECES: [Piece; 2] = [
        Piece::Power {
            lo: MASS_LIMIT_LO,
            hi: Self::BREAK,
            coefficient: 1.0,
            exponent: Self::LOW_EXPONENT,
        },
        // 0.5^(2.3 − 1.3) = 0.5, exactly: continuity at the break.
        Piece::Power {
            lo: Self::BREAK,
            hi: MASS_LIMIT_HI,
            coefficient: Self::BREAK,
            exponent: Self::HIGH_EXPONENT,
        },
    ];
}

impl MassFunction for Kroupa {
    fn pdf(&self, m: f64) -> f64 {
        pieces_pdf(&Self::PIECES, m)
    }

    fn integral(&self, lo: f64, hi: f64) -> f64 {
        pieces_integral(&Self::PIECES, lo, hi)
    }

    fn quantile_in(&self, lo: f64, hi: f64, u: f64) -> f64 {
        pieces_quantile(&Self::PIECES, lo, hi, u)
    }

    fn breaks(&self) -> &[f64] {
        &[Self::BREAK]
    }

    fn substellar_integral(&self, lo: f64, hi: f64) -> f64 {
        branch_integral(&Self::SUBSTELLAR, lo, hi)
    }

    fn substellar_quantile_in(&self, lo: f64, hi: f64, u: f64) -> f64 {
        branch_quantile(&Self::SUBSTELLAR, lo, hi, u)
    }
}

/// A [`Chabrier`] function could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildChabrierError {
    /// The high-mass scale is zero, negative, NaN or infinite.
    ScaleNotPositive,
}

impl fmt::Display for BuildChabrierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScaleNotPositive => {
                f.write_str("the high-mass scale must be positive and finite")
            }
        }
    }
}

impl Error for BuildChabrierError {}

/// Chabrier's (2003) system initial mass function on the stellar range, with its branch above
/// 1 M☉ scaled.
///
/// Per unit log₁₀ m it is a log-normal of centre 0.22 M☉ and width 0.57 dex up to 1 M☉, and
/// m^−1.3 above, continuous at 1 M☉ (Chabrier 2003, PASP 115, 763, eq. 18 and Table 1, the disc
/// system function: A = 0.086, `m_c` = 0.22 M☉, σ = 0.57; the power law's A = 4.43 × 10⁻², x =
/// 1.3, meets the log-normal at 1 M☉ to 0.3%). Per unit mass that is m^−2.3 above 1 M☉.
/// The overall constant A drops out, so the log-normal carries coefficient 1 and the power law the
/// log-normal's value at 1 M☉.
///
/// It is the default (brainstorm, Decisions, "2026-09-21: local density rulings", 2): the 20 pc
/// census has 66–68% of its primaries below 0.5 M☉ (tallied from Kirkpatrick et al. 2024, ApJS
/// 271, 55, Table 4, within 20 pc and 10 pc), where this function gives 66% and Kroupa's 76%, and
/// 69% of all its stars (their Table 18). Used for
/// primaries with the provisional companions (plan 02, Design note 4), the function as published
/// makes systems too heavy, 0.66 M☉ each at the Sun against the census's 0.55–0.59, and puts 67%
/// of all stars below 0.5 M☉. So its branch above 1 M☉ is multiplied by
/// [`high_mass_scale`](Self::high_mass_scale), a constant fitted offline together with the binary
/// stage's companions (plan 15). The scale is P15.T4.b's fit,
/// [`PROVISIONAL_HIGH_MASS_SCALE`](Self::PROVISIONAL_HIGH_MASS_SCALE), 0.920: the maximum-likelihood
/// scale of the 20 pc census's primaries over 0.08–8 M☉ (ruling 138), which retired plan 02's
/// scratch 0.68 (Design note 5); at it 69.8% of all stars in systems with a primary below 8 M☉ lie
/// below 0.5 M☉ (`tables::chabrier`'s acceptance line has the checks). A scale of 1 is Chabrier's
/// function as published.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chabrier {
    high_mass_scale: f64,
    pieces: [Piece; 2],
}

impl Chabrier {
    /// The log-normal's centre, M☉.
    pub const CENTRE: f64 = 0.22;

    /// The log-normal's width, dex.
    pub const WIDTH_DEX: f64 = 0.57;

    /// Where the log-normal meets the power law, M☉.
    pub const BREAK: f64 = 1.0;

    /// The power law's exponent per unit mass above [`BREAK`](Self::BREAK).
    pub const HIGH_EXPONENT: f64 = 2.3;

    /// The scale of the branch above 1 M☉: plan 15's P15.T4.b fit to the 20 pc census's primaries
    /// (ruling 138), [`tables::chabrier::HIGH_MASS_BRANCH_SCALE`](crate::tables::chabrier::HIGH_MASS_BRANCH_SCALE).
    /// The name is P15.T4.a's, from when it held plan 02's scratch 0.68, which ruling 138 retired;
    /// plans 02 and 11 name it.
    pub const PROVISIONAL_HIGH_MASS_SCALE: f64 = crate::tables::chabrier::HIGH_MASS_BRANCH_SCALE;

    /// Chabrier's function with its branch above 1 M☉ multiplied by `high_mass_scale`.
    ///
    /// # Errors
    ///
    /// [`BuildChabrierError::ScaleNotPositive`] unless the scale is positive and finite.
    pub fn new(high_mass_scale: f64) -> Result<Self, BuildChabrierError> {
        if high_mass_scale.is_finite() && high_mass_scale > 0.0 {
            Ok(Self::with_scale(high_mass_scale))
        } else {
            Err(BuildChabrierError::ScaleNotPositive)
        }
    }

    /// Chabrier's function with the fitted high-mass scale
    /// ([`PROVISIONAL_HIGH_MASS_SCALE`](Self::PROVISIONAL_HIGH_MASS_SCALE)): the default.
    #[must_use]
    pub fn provisional() -> Self {
        Self::with_scale(Self::PROVISIONAL_HIGH_MASS_SCALE)
    }

    /// The function with a scale already known to be positive and finite.
    fn with_scale(high_mass_scale: f64) -> Self {
        let centre = math::log10(Self::CENTRE);
        let d = math::log10(Self::BREAK) - centre;
        let at_break = math::exp(-d * d / (2.0 * Self::WIDTH_DEX * Self::WIDTH_DEX));
        Self {
            high_mass_scale,
            pieces: [
                Piece::LogNormal {
                    lo: MASS_LIMIT_LO,
                    hi: Self::BREAK,
                    coefficient: 1.0,
                    centre,
                    width: Self::WIDTH_DEX,
                },
                Piece::Power {
                    lo: Self::BREAK,
                    hi: MASS_LIMIT_HI,
                    coefficient: high_mass_scale * at_break / core::f64::consts::LN_10,
                    exponent: Self::HIGH_EXPONENT,
                },
            ],
        }
    }

    /// The factor on the branch above 1 M☉.
    #[must_use]
    pub fn high_mass_scale(&self) -> f64 {
        self.high_mass_scale
    }
}

impl Default for Chabrier {
    fn default() -> Self {
        Self::provisional()
    }
}

impl MassFunction for Chabrier {
    fn pdf(&self, m: f64) -> f64 {
        pieces_pdf(&self.pieces, m)
    }

    fn integral(&self, lo: f64, hi: f64) -> f64 {
        pieces_integral(&self.pieces, lo, hi)
    }

    fn quantile_in(&self, lo: f64, hi: f64, u: f64) -> f64 {
        pieces_quantile(&self.pieces, lo, hi, u)
    }

    fn breaks(&self) -> &[f64] {
        &[Self::BREAK]
    }

    fn substellar_integral(&self, lo: f64, hi: f64) -> f64 {
        branch_integral(&self.substellar_branch(), lo, hi)
    }

    fn substellar_quantile_in(&self, lo: f64, hi: f64, u: f64) -> f64 {
        branch_quantile(&self.substellar_branch(), lo, hi, u)
    }
}

impl Chabrier {
    /// The substellar branch: the log-normal continued below 0.08 M☉ to 0.01 M☉, as Chabrier's
    /// (2003) system function runs into the brown dwarfs. Only its shape is used (plan 13, Design
    /// note 6), and the high-mass scale does not touch it.
    fn substellar_branch(&self) -> Piece {
        match self.pieces[0] {
            Piece::LogNormal {
                centre,
                width,
                coefficient,
                ..
            } => Piece::LogNormal {
                lo: SUBSTELLAR_BRANCH_LO,
                hi: MASS_LIMIT_LO,
                coefficient,
                centre,
                width,
            },
            Piece::Power { .. } => unreachable!("Chabrier's first piece is its log-normal"),
        }
    }
}

/// Which mass function a galaxy uses: part of its generator version.
///
/// The default is [`Chabrier`]'s system function with the provisional high-mass scale; [`Kroupa`]'s
/// stays supported (brainstorm, Decisions, "2026-09-21: local density rulings", 2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MassFunctionKind {
    /// [`Kroupa`].
    Kroupa,
    /// [`Chabrier`] with the provisional high-mass scale: the default.
    #[default]
    Chabrier,
}

impl MassFunctionKind {
    /// The mass function of this kind.
    #[must_use]
    pub fn to_mass_function(self) -> Box<dyn MassFunction> {
        match self {
            Self::Kroupa => Box::new(Kroupa),
            Self::Chabrier => Box::new(Chabrier::provisional()),
        }
    }
}

/// The share of systems in each mass band: the mass function integrated over
/// [`MASS_BAND_EDGES`], each band over the whole stellar range.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::imf::{BandShares, Kroupa, MassBand};
///
/// let shares = BandShares::of(&Kroupa);
/// let total: f64 = MassBand::ALL.iter().map(|&b| shares.share(b)).sum();
/// assert!((total - 1.0).abs() < 1e-14);
/// // Layer E holds well under 1% of systems.
/// assert!(shares.share(MassBand::E) < 0.01);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandShares([f64; 5]);

impl BandShares {
    /// The band shares of `f`.
    #[must_use]
    pub fn of(f: &(impl MassFunction + ?Sized)) -> Self {
        let total = f.integral(MASS_LIMIT_LO, MASS_LIMIT_HI);
        Self(MassBand::ALL.map(|band| f.integral(band.lo(), band.hi()) / total))
    }

    /// The share of systems whose primary lies in `band`.
    ///
    /// # Panics
    ///
    /// For a substellar band, which is not part of the stellar normalisation: plan 13 counts those
    /// objects per system ([`SubstellarAbundance`](super::substellar::SubstellarAbundance)).
    #[must_use]
    pub fn share(&self, band: MassBand) -> f64 {
        assert!(
            band.is_stellar(),
            "{band:?} is not a band of the stellar normalisation"
        );
        self.0[band.index()]
    }

    /// The five shares, band A first.
    #[must_use]
    pub fn as_array(&self) -> [f64; 5] {
        self.0
    }
}

/// A mass function with its primaries cut above `top` M☉: `f`'s density up to `top` and none above
/// (P15.T4.b; ruling 138.4). The 20 pc census holds no neutron-star or black-hole system, so its
/// figures are compared with the model's systems whose primary lies below 8 M☉.
///
/// Its stellar range stays 0.08–150 M☉, so quadratures over it read zero above `top`, which is a
/// break. The substellar branch is `f`'s.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::imf::{Chabrier, MassFunction, Truncated};
///
/// let f = Chabrier::default();
/// let below = Truncated::new(&f, 8.0);
/// assert_eq!(below.pdf(20.0), 0.0);
/// assert!((below.integral(0.08, 150.0) - f.integral(0.08, 8.0)).abs() < 1e-15);
/// ```
#[derive(Debug, Clone)]
pub struct Truncated<'f> {
    f: &'f dyn MassFunction,
    top: f64,
    breaks: Vec<f64>,
}

impl<'f> Truncated<'f> {
    /// `f` with no primary above `top` M☉.
    #[must_use]
    pub fn new(f: &'f dyn MassFunction, top: f64) -> Self {
        let mut breaks: Vec<f64> = f.breaks().iter().copied().filter(|&m| m < top).collect();
        if top > MASS_LIMIT_LO && top < MASS_LIMIT_HI {
            breaks.push(top);
        }
        Self { f, top, breaks }
    }
}

impl MassFunction for Truncated<'_> {
    fn pdf(&self, m: f64) -> f64 {
        if m > self.top { 0.0 } else { self.f.pdf(m) }
    }

    fn integral(&self, lo: f64, hi: f64) -> f64 {
        let hi = hi.min(self.top);
        if hi <= lo {
            0.0
        } else {
            self.f.integral(lo, hi)
        }
    }

    fn quantile_in(&self, lo: f64, hi: f64, u: f64) -> f64 {
        self.f.quantile_in(lo, hi.min(self.top), u)
    }

    fn breaks(&self) -> &[f64] {
        &self.breaks
    }

    fn substellar_integral(&self, lo: f64, hi: f64) -> f64 {
        self.f.substellar_integral(lo, hi)
    }

    fn substellar_quantile_in(&self, lo: f64, hi: f64, u: f64) -> f64 {
        self.f.substellar_quantile_in(lo, hi, u)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::Seed;
    use crate::rng::{ObjectKey, tags};

    fn functions() -> [(&'static str, Box<dyn MassFunction>); 3] {
        [
            ("kroupa", Box::new(Kroupa)),
            ("chabrier", Box::new(Chabrier::new(1.0).unwrap())),
            ("chabrier 0.68", Box::new(Chabrier::new(0.68).unwrap())),
        ]
    }

    /// `actual` rounds to `printed`, which has `decimals` decimal places.
    fn assert_rounds_to(what: &str, actual: f64, printed: f64, decimals: i32) {
        let half_step = 0.5 * math::powi(10.0, -decimals);
        assert!(
            (actual - printed).abs() <= half_step,
            "{what}: {actual} does not round to {printed}"
        );
    }

    /// The brainstorm's "Sizing the layers" table, each share to the precision printed.
    #[test]
    fn band_shares_match_the_brainstorm_table() {
        let kroupa = BandShares::of(&Kroupa);
        for (band, printed, decimals) in [
            (MassBand::A, 76.0, 0),
            (MassBand::B, 9.8, 1),
            (MassBand::C, 11.0, 0),
            (MassBand::D, 2.3, 1),
            (MassBand::E, 0.64, 2),
        ] {
            assert_rounds_to("kroupa", 100.0 * kroupa.share(band), printed, decimals);
        }
        let chabrier = BandShares::of(&Chabrier::new(1.0).unwrap());
        for (band, printed, decimals) in [
            (MassBand::A, 66.0, 0),
            (MassBand::B, 12.0, 0),
            (MassBand::C, 17.0, 0),
            (MassBand::D, 3.7, 1),
            (MassBand::E, 1.0, 1),
        ] {
            assert_rounds_to("chabrier", 100.0 * chabrier.share(band), printed, decimals);
        }
        // The brainstorm's default row: Chabrier's with its branch above 1 M☉ scaled by the fitted
        // 0.92 (ruling 138; the row's figures, ruling 140).
        let default = BandShares::of(MassFunctionKind::default().to_mass_function().as_ref());
        for (band, printed, decimals) in [
            (MassBand::A, 67.0, 0),
            (MassBand::B, 12.0, 0),
            (MassBand::C, 17.0, 0),
            (MassBand::D, 3.4, 1),
            (MassBand::E, 0.95, 2),
        ] {
            assert_rounds_to("default", 100.0 * default.share(band), printed, decimals);
        }
    }

    /// The table's "Per cell" columns: 0.003 systems per cubic light-year times the cell volume
    /// times the band's share.
    #[test]
    fn per_cell_counts_at_the_reference_density_match_the_table() {
        const REFERENCE_DENSITY: f64 = 0.003;
        let per_cell = |shares: &BandShares, band: MassBand| {
            let edge = f64::from(band.layer().cell_size_ly());
            REFERENCE_DENSITY * edge * edge * edge * shares.share(band)
        };
        let kroupa = BandShares::of(&Kroupa);
        for (band, printed, decimals) in [
            (MassBand::A, 1.2, 1),
            (MassBand::B, 1.2, 1),
            (MassBand::C, 11.0, 0),
            (MassBand::D, 18.0, 0),
            (MassBand::E, 40.0, 0),
        ] {
            assert_rounds_to(
                "kroupa per cell",
                per_cell(&kroupa, band),
                printed,
                decimals,
            );
        }
        let chabrier = BandShares::of(&Chabrier::new(1.0).unwrap());
        for (band, printed, decimals) in [
            (MassBand::A, 1.0, 1),
            (MassBand::B, 1.4, 1),
            (MassBand::C, 17.0, 0),
            (MassBand::D, 29.0, 0),
            (MassBand::E, 64.0, 0),
        ] {
            assert_rounds_to(
                "chabrier per cell",
                per_cell(&chabrier, band),
                printed,
                decimals,
            );
        }
        // The brainstorm's default row, at the fitted 0.92 (see the table's test above).
        let default = BandShares::of(MassFunctionKind::default().to_mass_function().as_ref());
        for (band, printed, decimals) in [
            (MassBand::A, 1.0, 1),
            (MassBand::B, 1.5, 1),
            (MassBand::C, 16.0, 0),
            (MassBand::D, 27.0, 0),
            (MassBand::E, 60.0, 0),
        ] {
            assert_rounds_to(
                "default per cell",
                per_cell(&default, band),
                printed,
                decimals,
            );
        }
    }

    #[test]
    fn shares_sum_to_one() {
        for (name, f) in functions() {
            let shares = BandShares::of(f.as_ref());
            let total: f64 = shares.as_array().iter().sum();
            assert!((total - 1.0).abs() < 1e-14, "{name}: {total}");
            assert!(shares.as_array().iter().all(|&s| s > 0.0));
        }
    }

    /// Both functions are continuous at their break; the scaled Chabrier jumps by its scale.
    #[test]
    fn densities_meet_at_the_breaks() {
        let below = |f: &dyn MassFunction, m: f64| f.pdf(m.next_down());
        let relative_jump = |f: &dyn MassFunction, m: f64| f.pdf(m) / below(f, m) - 1.0;
        assert!(relative_jump(&Kroupa, Kroupa::BREAK).abs() < 1e-12);
        let unscaled = Chabrier::new(1.0).unwrap();
        // Chabrier's published constants: A = 0.086 on the log-normal and 4.43 × 10⁻² on the
        // power law per unit log mass meet at 1 M☉ to within the rounding of 4.43, 0.3%.
        let at_break_per_log = unscaled.pdf(1.0) * core::f64::consts::LN_10;
        assert!((0.086 * at_break_per_log / 4.43e-2 - 1.0).abs() < 3e-3);
        assert!(relative_jump(&unscaled, Chabrier::BREAK).abs() < 1e-12);
        let scaled = Chabrier::provisional();
        let step = Chabrier::PROVISIONAL_HIGH_MASS_SCALE - 1.0;
        assert!((relative_jump(&scaled, Chabrier::BREAK) - step).abs() < 1e-12);
        assert_same_bits(Kroupa.pdf(0.079), 0.0);
        assert_same_bits(Kroupa.pdf(150.1), 0.0);
        assert!(Kroupa.pdf(150.0) > 0.0);
    }

    /// The closed-form integrals against a fine quadrature of the density.
    #[test]
    fn integrals_match_quadrature_of_the_density() {
        use super::super::quad::gl_log_panels;
        for (name, f) in functions() {
            let mut edges = vec![MASS_LIMIT_LO];
            edges.extend_from_slice(f.breaks());
            edges.push(MASS_LIMIT_HI);
            let quadrature = gl_log_panels(|m| f.pdf(m), &edges);
            let closed = f.integral(MASS_LIMIT_LO, MASS_LIMIT_HI);
            assert!(((quadrature - closed) / closed).abs() < 1e-12, "{name}");
            assert_same_bits(f.integral(0.01, 0.05), 0.0);
            assert_same_bits(f.integral(0.3, 0.2), 0.0);
        }
    }

    /// perf08: the log-normal's inversion ends where the plain bisection of the error function
    /// ends, bit for bit: over the fixed brackets the generator inverts (bands A to C, the brown
    /// dwarfs and the whole substellar branch, one spanning the stellar log-normal and one far into
    /// its tail) at 20,000 fractions each, the ends and the smallest and largest steps included,
    /// and over some 2,000 brackets drawn inside 0.01–1 M☉ as runtime callers build them (a
    /// cluster's members up to its turn-off mass, `Truncated`'s top), narrow ones and ones ending
    /// near 1 M☉ among them, at 50 fractions each.
    #[test]
    fn the_log_normal_inversion_is_the_bisection_bit_for_bit() {
        let scale = Chabrier::WIDTH_DEX * core::f64::consts::SQRT_2;
        let centre = math::log10(Chabrier::CENTRE);
        let n = 20_000_u32;
        let fixed_fractions: Vec<f64> = (0..=n)
            .map(|i| f64::from(i) / f64::from(n))
            .chain([
                f64::EPSILON / 4.0,
                f64::EPSILON,
                1.0 - f64::EPSILON / 2.0,
                1e-300,
            ])
            .collect();
        let mut cases: Vec<((f64, f64), Vec<f64>)> = [
            (MASS_LIMIT_LO, 0.5),
            (0.5, 0.75),
            (0.75, Chabrier::BREAK),
            (BROWN_DWARF_MIN_MSUN, MASS_LIMIT_LO),
            (SUBSTELLAR_BRANCH_LO, MASS_LIMIT_LO),
            (MASS_LIMIT_LO, Chabrier::BREAK),
            (Chabrier::BREAK, MASS_LIMIT_HI),
        ]
        .into_iter()
        .map(|bracket| (bracket, fixed_fractions.clone()))
        .collect();
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x9e80_0008);
        for k in 0..2_000 {
            let lo = SUBSTELLAR_BRANCH_LO * math::powf(100.0, lcg.next_f64());
            let hi = match k % 4 {
                // Narrow: a part in 10³–10⁹ of the lower end.
                0 => lo * (1.0 + math::powf(10.0, -3.0 - 6.0 * lcg.next_f64())),
                // Ending near 1 M☉, as an old cluster's turn-off mass does.
                1 => Chabrier::BREAK * (1.0 - 0.3 * lcg.next_f64()),
                _ => lo + (Chabrier::BREAK - lo) * lcg.next_f64(),
            };
            if lo < hi {
                let fractions = (0..50).map(|_| lcg.next_f64()).collect();
                cases.push(((lo, hi), fractions));
            }
        }
        assert!(cases.len() > 1_500, "{} brackets", cases.len());
        for ((a, b), fractions) in cases {
            let erf_a = math::erf((math::log10(a) - centre) / scale);
            let erf_b = math::erf((math::log10(b) - centre) / scale);
            for fraction in fractions {
                let target = erf_a + fraction * (erf_b - erf_a);
                let (lo, hi) = (math::log10(a), math::log10(b));
                let direct = crate::galaxy::quad::bisect(
                    |x| math::erf((x - centre) / scale) - target,
                    lo,
                    hi,
                    LOG_NORMAL_BISECTIONS,
                );
                let fast = invert_erf(centre, scale, target, lo, hi);
                assert_same_bits(fast, direct);
            }
        }
    }

    /// The restricted inverse at 1,000 points per band: the mass it returns has the asked-for
    /// share of the band below it.
    #[test]
    fn quantile_in_inverts_the_restricted_distribution() {
        for (name, f) in functions() {
            for band in MassBand::ALL {
                let (lo, hi) = (band.lo(), band.hi());
                let total = f.integral(lo, hi);
                assert_same_bits(f.quantile_in(lo, hi, 0.0), lo);
                assert_same_bits(f.quantile_in(lo, hi, 1.0), hi);
                for i in 0..1_000 {
                    let u = (f64::from(i) + 0.5) / 1_000.0;
                    let m = f.quantile_in(lo, hi, u);
                    assert!((lo..=hi).contains(&m), "{name} {band:?}: {m}");
                    let achieved = f.integral(lo, m) / total;
                    assert!(
                        (achieved - u).abs() < 1e-12,
                        "{name} {band:?}: u = {u} gave {achieved}"
                    );
                }
            }
        }
    }

    /// 10⁵ draws per band by `sample_in_band`, against the restricted distribution.
    #[test]
    fn band_samples_follow_the_restricted_distribution() {
        for (test, (name, f)) in (0_u64..).zip(functions().into_iter().take(2)) {
            for band in MassBand::ALL {
                let (lo, hi) = (band.lo(), band.hi());
                let total = f.integral(lo, hi);
                let key = ObjectKey::galaxy_item(test * 8 + u64::try_from(band.index()).unwrap());
                let mut stream = Stream::open(Seed::new(0x1bf0_0002), tags::SELFTEST_STREAM, key);
                let mut sample: Vec<f64> = (0..100_000)
                    .map(|_| f.sample_in_band(band, &mut stream))
                    .collect();
                assert_eq!(stream.position(), 100_000, "one word per draw");
                let ks =
                    ks_one_sample(&mut sample, |m| (f.integral(lo, m) / total).clamp(0.0, 1.0));
                assert_p_value(&format!("{name} band {band:?}"), ks.p_value, ALPHA);
            }
        }
    }

    #[test]
    fn every_layer_converts_to_its_band_and_back() {
        for band in MassBand::ALL.into_iter().chain(MassBand::SUBSTELLAR) {
            assert_eq!(MassBand::from(band.layer()), band);
            assert_eq!(Layer::from(band), band.layer());
            assert_eq!(band.index(), usize::from(band.layer().value()));
            assert!(band.lo() < band.hi());
        }
        assert!(MassBand::ALL.iter().all(|band| band.is_stellar()));
        assert!(MassBand::SUBSTELLAR.iter().all(|band| !band.is_stellar()));
        // The substellar bands meet each other and the stellar range with no gap.
        assert_same_bits(MassBand::BrownDwarf.hi(), MassBand::A.lo());
        assert_same_bits(MassBand::RoguePlanet.hi(), MassBand::BrownDwarf.lo());
    }

    #[test]
    #[should_panic(expected = "the stellar mass function has no BrownDwarf band")]
    fn a_stellar_draw_refuses_a_substellar_band() {
        let mut stream = Stream::open(Seed::new(1), tags::SELFTEST_STREAM, ObjectKey::galaxy());
        let _ = Kroupa.sample_in_band(MassBand::BrownDwarf, &mut stream);
    }

    #[test]
    #[should_panic(expected = "RoguePlanet is not a band of the stellar normalisation")]
    fn band_shares_refuse_a_substellar_band() {
        let _ = BandShares::of(&Kroupa).share(MassBand::RoguePlanet);
    }

    /// The substellar branches: Kroupa's is m^−0.3 and Chabrier's the log-normal continued, both
    /// on 0.01–0.08 M☉, and their inverses invert them.
    #[test]
    fn the_substellar_branches_have_their_sources_shapes() {
        let kroupa =
            Kroupa.substellar_integral(0.02, 0.04) / Kroupa.substellar_integral(0.04, 0.08);
        // ∫ m^−0.3 from 0.02 to 0.04 over 0.04 to 0.08 is 2^−0.7.
        assert!((kroupa - math::powf(2.0, -0.7)).abs() < 1e-12, "{kroupa}");
        let chabrier = Chabrier::provisional();
        let published = Chabrier::new(1.0).unwrap();
        assert_same_bits(
            chabrier.substellar_integral(0.02, 0.05),
            published.substellar_integral(0.02, 0.05),
        );
        // The log-normal falls towards lower masses below its 0.22 M☉ peak, per unit log mass.
        let per_dex = |lo: f64| chabrier.substellar_integral(lo, 2.0 * lo);
        assert!(per_dex(0.02) < per_dex(0.04));
        for f in [&Kroupa as &dyn MassFunction, &chabrier] {
            assert_same_bits(f.substellar_integral(0.08, 0.1), 0.0);
            assert_same_bits(f.substellar_integral(0.001, 0.005), 0.0);
            let (lo, hi) = (MassBand::BrownDwarf.lo(), MassBand::BrownDwarf.hi());
            let total = f.substellar_integral(lo, hi);
            for i in 0..100 {
                let u = (f64::from(i) + 0.5) / 100.0;
                let m = f.substellar_quantile_in(lo, hi, u);
                assert!((lo..=hi).contains(&m));
                assert!((f.substellar_integral(lo, m) / total - u).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn chabrier_rejects_a_bad_scale() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                Chabrier::new(scale),
                Err(BuildChabrierError::ScaleNotPositive)
            );
        }
        assert_same_bits(
            Chabrier::default().high_mass_scale(),
            crate::tables::chabrier::HIGH_MASS_BRANCH_SCALE,
        );
    }

    #[test]
    fn a_kind_builds_its_function() {
        let kroupa = MassFunctionKind::Kroupa.to_mass_function();
        assert_same_bits(kroupa.pdf(0.3), Kroupa.pdf(0.3));
        let chabrier = MassFunctionKind::Chabrier.to_mass_function();
        assert_same_bits(chabrier.pdf(3.0), Chabrier::provisional().pdf(3.0));
        assert_eq!(MassFunctionKind::default(), MassFunctionKind::Chabrier);
    }
}
