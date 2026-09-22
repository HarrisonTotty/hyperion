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
//! [`MASS_LIMIT_LO`]–[`MASS_LIMIT_HI`], 0.08–150 M☉.

use std::error::Error;
use std::fmt;

use super::quad::bisect;
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
/// The substellar layers have no band here: they are not part of the stellar normalisation, and
/// [`MassBand::try_from`] rejects them.
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
}

impl MassBand {
    /// Every band, from the lightest.
    pub const ALL: [Self; 5] = [Self::A, Self::B, Self::C, Self::D, Self::E];

    /// The band's place in [`MassBand::ALL`] and in [`MASS_BAND_EDGES`], 0–4.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
            Self::C => 2,
            Self::D => 3,
            Self::E => 4,
        }
    }

    /// The band's lower edge, M☉.
    #[must_use]
    pub const fn lo(self) -> f64 {
        MASS_BAND_EDGES[self.index()]
    }

    /// The band's upper edge, M☉.
    #[must_use]
    pub const fn hi(self) -> f64 {
        MASS_BAND_EDGES[self.index() + 1]
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
        }
    }
}

impl From<MassBand> for Layer {
    fn from(band: MassBand) -> Self {
        band.layer()
    }
}

impl TryFrom<Layer> for MassBand {
    type Error = ConvertLayerError;

    /// The band of a stellar layer.
    ///
    /// # Errors
    ///
    /// [`ConvertLayerError`] for the brown-dwarf and rogue-planet layers, which own no band of
    /// the stellar mass function.
    fn try_from(layer: Layer) -> Result<Self, Self::Error> {
        match layer {
            Layer::A => Ok(Self::A),
            Layer::B => Ok(Self::B),
            Layer::C => Ok(Self::C),
            Layer::D => Ok(Self::D),
            Layer::E => Ok(Self::E),
            Layer::BrownDwarf | Layer::RoguePlanet => Err(ConvertLayerError { layer }),
        }
    }
}

/// A layer that owns no stellar mass band was converted to a [`MassBand`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConvertLayerError {
    layer: Layer,
}

impl ConvertLayerError {
    /// The layer that has no band.
    #[must_use]
    pub const fn layer(self) -> Layer {
        self.layer
    }
}

impl fmt::Display for ConvertLayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "layer {} owns no band of the stellar mass function",
            self.layer.letter()
        )
    }
}

impl Error for ConvertLayerError {}

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

    /// A primary mass drawn from the function restricted to `band`: one uniform from `stream`,
    /// then [`quantile_in`](Self::quantile_in). One word.
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
        self.quantile_in(band.lo(), band.hi(), stream.uniform())
    }
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
                let log_m = bisect(
                    |x| math::erf((x - centre) / scale) - target,
                    math::log10(a),
                    math::log10(b),
                    LOG_NORMAL_BISECTIONS,
                );
                math::exp10(log_m)
            }
        };
        x.clamp(a, b)
    }
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
    let mut remaining = u * pieces_integral(pieces, lo, hi);
    // Placement draws a mass for every system, so this allocates nothing. The pieces are
    // contiguous and cover the stellar range, so the overlap that reaches `hi` is the last one.
    for (piece, (a, b)) in pieces
        .iter()
        .filter_map(|p| overlap(p, lo, hi).map(|range| (p, range)))
    {
        let mass = piece.integral(a, b);
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
/// stage's companions (plan 15). Until that fit lands the scale is the provisional
/// [`PROVISIONAL_HIGH_MASS_SCALE`](Self::PROVISIONAL_HIGH_MASS_SCALE), 0.68 (plan 02, Design note
/// 5), which gives 71% of all stars below 0.5 M☉: the two bracket the census. A scale of 1 is
/// Chabrier's function as published.
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

    /// The provisional scale of the branch above 1 M☉, until plan 15 fits it.
    pub const PROVISIONAL_HIGH_MASS_SCALE: f64 = 0.68;

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

    /// Chabrier's function with the provisional high-mass scale, 0.68.
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
    #[must_use]
    pub fn share(&self, band: MassBand) -> f64 {
        self.0[band.index()]
    }

    /// The five shares, band A first.
    #[must_use]
    pub fn as_array(&self) -> [f64; 5] {
        self.0
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
            ("chabrier 0.68", Box::new(Chabrier::provisional())),
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
        // The default: Chabrier's with its branch above 1 M☉ scaled by the provisional 0.68.
        let default = BandShares::of(MassFunctionKind::default().to_mass_function().as_ref());
        for (band, printed, decimals) in [
            (MassBand::A, 70.0, 0),
            (MassBand::B, 12.0, 0),
            (MassBand::C, 15.0, 0),
            (MassBand::D, 2.6, 1),
            (MassBand::E, 0.73, 2),
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
        let default = BandShares::of(&Chabrier::provisional());
        for (band, printed, decimals) in [
            (MassBand::A, 1.1, 1),
            (MassBand::B, 1.5, 1),
            (MassBand::C, 14.0, 0),
            (MassBand::D, 21.0, 0),
            (MassBand::E, 46.0, 0),
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
        assert!((relative_jump(&scaled, Chabrier::BREAK) - (0.68 - 1.0)).abs() < 1e-12);
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
    fn stellar_layers_convert_to_bands_and_back() {
        for band in MassBand::ALL {
            assert_eq!(MassBand::try_from(band.layer()), Ok(band));
            assert_eq!(Layer::from(band), band.layer());
            assert!(band.lo() < band.hi());
        }
        for layer in [Layer::BrownDwarf, Layer::RoguePlanet] {
            let error = MassBand::try_from(layer).unwrap_err();
            assert_eq!(error.layer(), layer);
        }
        assert_eq!(
            MassBand::try_from(Layer::RoguePlanet)
                .unwrap_err()
                .to_string(),
            "layer G owns no band of the stellar mass function"
        );
    }

    #[test]
    fn chabrier_rejects_a_bad_scale() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                Chabrier::new(scale),
                Err(BuildChabrierError::ScaleNotPositive)
            );
        }
        assert_same_bits(Chabrier::default().high_mass_scale(), 0.68);
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
