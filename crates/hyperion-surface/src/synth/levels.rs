//! The contributions each band level evaluates, and the level bound (plan R09, T8; Design notes 13
//! and 15; the brainstorm's level-of-detail consistency).
//!
//! # The sets
//!
//! A band level n resolves wavelengths down to its shortest, `λ_n` = max(2 m, 4 s) with s the
//! level's largest vertex spacing on the figure's equatorial radius: R05's rule for the octaves a
//! level evaluates, held at the 2 m band limit ([`shortest_wavelength`]). Each contribution is
//! evaluated at n wherever its own scale is at least `λ_n`, a set fixed per level and body from the
//! level's largest cell, never switched by the local spacing ([`LevelSet`]):
//!
//! - the structural octaves, octave m where its lattice spacing (1.5 of the level's mean cells,
//!   [`octave_spacing`]) is: through n + 4 on any body of flattening under 0.2;
//! - the channels, level k where its key points' spacing, a mean cell of level k, is: through
//!   n + 3, the octave k + 1's band, which T6.b's closed form places at the same wavelengths;
//! - the small craters, octave k where the smallest diameter it keeps is, so that a crater appears
//!   at the level that can represent its diameter, its rim filleted over the level's band
//!   ([`band_half_width`]), ½ `λ_n`.
//!
//! The sets nest, each level's within every finer one's, and from the shallowest level whose
//! `λ_n` is the band limit, level 19 on an Earth, every level evaluates the same set with the same
//! band, so its heights are the finest's to the bit.
//!
//! # The bound
//!
//! [`level_bound_m`] bounds |h(n) − h(m)| for every finer level m: the contributions level m
//! evaluates and n does not, and the craters' fillets, which narrow as the band does. The base
//! elevation is the same at every level. The structural octaves' and the channels' terms are hard,
//! whatever the cells; the craters' is a quantile, since a Poisson field of craters has no largest
//! overlap ([`CRATER_BOUND_TAIL`]):
//!
//! - **the octaves**: Σ [`octave_bound`] over the octaves m evaluates and n does not, since every
//!   octave's term is the same at every level that evaluates it;
//! - **the channels**: a level's raw incision is what its grooves cut below every coarser level's,
//!   so the levels from k on together cut between 0 and `G_k`, level k's groove cap
//!   ([`groove_cap_m`]), the deepest a level k or finer may cut; each level's correction is
//!   bilinear in its corner means, within [−`G_j`, 0], plus a bubble scaled by the cell's mean
//!   less the bilinear part's over the bubble's mean, within ±`G_j` ÷ [`BUBBLE_MEAN_FLOOR`]. The
//!   raw part and the bilinear part take opposite signs, and `G_k` is one of the `G_j`, so the
//!   omitted levels from k to the deepest a body's channels can reach
//!   ([`deepest_possible_level`]) change a height by at most (1 + 1 ÷ B) Σ `G_j`;
//! - **the craters**: per octave, one crater's largest share, its fresh height and its fillet's
//!   change where the octave is new to the finer level, its fillet's change between the two bands
//!   where both evaluate it, with the octave's expected count of craters reaching a point, summed
//!   by Chernoff's bound to the tail ([`poisson_tail_bound`]). Where the two levels' bands are the
//!   same, an octave both evaluate adds nothing. The coarse craters' rims (R09.T7.c) join the same
//!   sum: every level sharpens them, so each octave of diameters from `D_b` up adds its fillet's
//!   change between the two bands, held to the coarse band, at the count Trask's saturation
//!   allows, which bounds the coarse pass's draw whatever the production ([`coarse_marks`]).
//!
//! The same terms bound the heights a level can reach about the base elevation, for culling
//! ([`LevelBounds::contribution_range_m`]).
//!
//! [`octave_spacing`]: super::relief::octave_spacing
//! [`band_half_width`]: super::craters::band_half_width
//! [`octave_bound`]: super::relief::octave_bound
//! [`groove_cap_m`]: super::channels::incision::groove_cap_m
//! [`BUBBLE_MEAN_FLOOR`]: super::channels::incision::BUBBLE_MEAN_FLOOR
//! [`deepest_possible_level`]: super::channels::incision::deepest_possible_level
//! [`poisson_tail_bound`]: super::craters::bound::poisson_tail_bound
//! [`coarse_marks`]: super::craters::bound::coarse_marks

use hyperion_base::math;
use hyperion_base::units::Metres;

use super::channels::incision::{
    BUBBLE_MEAN_FLOOR, deepest_possible_level, groove_cap_m, mean_cell_width,
};
use super::craters::bound::{coarse_marks, octave_marks, poisson_tail_bound};
use super::craters::rims::coarse_band_half_width;
use super::craters::{CraterOctaves, band_half_width};
use super::relief::{finest_octave, first_octave, octave_bound, octave_spacing};
use super::{BAND_LEVELS, BandLevel};
use crate::cube::MAX_LEVEL;
use crate::field::FieldHeader;
use crate::geometry::{BAND_LIMIT_M, vertex_spacing};
use crate::num;
use crate::spheroid::Spheroid;

/// The probability per point at which the craters' share of the level bound, the small craters'
/// and the coarse rims', may be exceeded: 10⁻⁹. A cell's crater count is a Poisson draw, so their
/// overlap at a point has no largest value and their share is a 1 − 10⁻⁹ quantile, while the
/// structural octaves' and the channels' terms are hard (the [module documentation](self)). A body's
/// 10⁴ sampled points at 19 levels give a union bound of about 2 × 10⁻⁴, and Chernoff's slack makes
/// the true odds far smaller. A design choice (main's acceptance, 2026-10-13): the bound selects a
/// level of detail, where a rare exceedance is a small crack or pop, and collision reads the
/// finest level.
pub const CRATER_BOUND_TAIL: f64 = 1e-9;

/// The shortest wavelength band level `level` resolves on `figure`'s spheroid, `λ_n`: max(2 m, 4 s)
/// with s the level's largest vertex spacing on the equatorial radius (R05's rule, held at the
/// band limit, [`BAND_LIMIT_M`]); twice the small craters' fillet half-width at the level
/// ([`band_half_width`]).
///
/// [`band_half_width`]: super::craters::band_half_width
#[must_use]
pub fn shortest_wavelength(figure: &Spheroid, level: BandLevel) -> Metres {
    let spacing = vertex_spacing(figure.equatorial_radius_m, level.get()).max_m;
    Metres::new(num::max(BAND_LIMIT_M, 4.0 * spacing))
}

/// The contributions a band level evaluates on a body, beside the base elevation, which every level
/// does: the deepest structural octave, channel level and small-crater octave, each `None` where
/// the level evaluates none (the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelSet {
    level: BandLevel,
    shortest: Metres,
    relief: Option<u8>,
    channels: Option<u8>,
    craters: Option<u8>,
}

impl LevelSet {
    /// The set of band level `level` on the body of `header`, whose small craters' octaves are
    /// `octaves` ([`CraterOctaves::new`] of the header).
    #[must_use]
    pub(crate) fn of(header: &FieldHeader, octaves: &CraterOctaves, level: BandLevel) -> Self {
        let shortest = shortest_wavelength(&header.figure(), level);
        let lambda = shortest.value();
        let radius = header.radius();
        let first = first_octave(header.level());
        let relief = (first..=finest_octave(radius))
            .take_while(|&m| octave_spacing(radius, m).value() >= lambda)
            .last();
        let coarse = header.level().get();
        let channels = (coarse..=MAX_LEVEL)
            .take_while(|&k| mean_cell_width(radius.value(), k) >= lambda)
            .last();
        let craters = octaves
            .octaves()
            .iter()
            .take_while(|octave| octave.smallest().value() >= lambda)
            .last()
            .map(super::craters::CraterOctave::index);
        Self {
            level,
            shortest,
            relief,
            channels,
            craters,
        }
    }

    /// The band level.
    #[must_use]
    pub const fn level(&self) -> BandLevel {
        self.level
    }

    /// The shortest wavelength the level resolves, `λ_n` ([`shortest_wavelength`]).
    #[must_use]
    pub const fn shortest_wavelength(&self) -> Metres {
        self.shortest
    }

    /// The deepest structural octave the level evaluates, from the first ([`Relief::at`]'s
    /// `through`), or `None`.
    ///
    /// [`Relief::at`]: super::Relief::at
    #[must_use]
    pub const fn relief_through(&self) -> Option<u8> {
        self.relief
    }

    /// The deepest channel level the level evaluates, from the coarse level
    /// ([`Channels::incision_at`]'s `through`), or `None`.
    ///
    /// [`Channels::incision_at`]: super::Channels::incision_at
    #[must_use]
    pub const fn channels_through(&self) -> Option<u8> {
        self.channels
    }

    /// The last small-crater octave the level evaluates, from the largest, octave 0
    /// ([`SmallCraters::at`]'s `through`), or `None`.
    ///
    /// [`SmallCraters::at`]: super::SmallCraters::at
    #[must_use]
    pub const fn craters_through(&self) -> Option<u8> {
        self.craters
    }
}

/// A body's level sets, its level bound and the range its contributions can reach at each band
/// level, from its field's header alone (the [module documentation](self)): built once per body.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelBounds {
    sets: [LevelSet; BAND_LEVELS],
    bound_m: [f64; BAND_LEVELS],
    low_m: [f64; BAND_LEVELS],
    high_m: [f64; BAND_LEVELS],
}

impl LevelBounds {
    /// The bounds of the body of `header`. It builds the small craters' octaves
    /// ([`CraterOctaves::new`], about 1 to 10 ms) and samples their profiles, a few milliseconds
    /// more.
    ///
    /// # Panics
    ///
    /// As [`CraterOctaves::new`], which no header a field holds reaches.
    #[must_use]
    pub fn new(header: &FieldHeader) -> Self {
        Self::of(header, &CraterOctaves::new(header))
    }

    /// The bounds of the body of `header`, whose small craters' octaves are `octaves`.
    #[must_use]
    pub(crate) fn of(header: &FieldHeader, octaves: &CraterOctaves) -> Self {
        let figure = header.figure();
        let sets: [LevelSet; BAND_LEVELS] =
            core::array::from_fn(|n| LevelSet::of(header, octaves, band(n)));
        let bands: [f64; BAND_LEVELS] =
            core::array::from_fn(|n| band_half_width(&figure, band(n)).value());
        let relief = Octaves::of(header);
        let channels = ChannelCaps::of(header);
        let marks = octave_marks(octaves, &bands);
        let rims = coarse_marks(
            header.craters(),
            coarse_band_half_width(&figure, header.level()).value(),
            header.boundary_diameter().value(),
            header.radius().value(),
            &bands,
        );
        // Each octave's largest fillet change at any level from n on: δ*(n) = max over m ≥ n.
        let later: Vec<[f64; BAND_LEVELS + 1]> =
            marks.iter().map(|o| suffix_max(&o.fillet_m)).collect();
        let rims_later: Vec<[f64; BAND_LEVELS + 1]> =
            rims.iter().map(|o| suffix_max(&o.fillet_m)).collect();
        let ln_tail = -math::ln(CRATER_BOUND_TAIL);
        let mut bound_m = [0.0; BAND_LEVELS];
        let mut low_m = [0.0; BAND_LEVELS];
        let mut high_m = [0.0; BAND_LEVELS];
        let mut changes = Vec::with_capacity(marks.len() + rims.len());
        let mut heights = Vec::with_capacity(marks.len() + rims.len());
        for n in 0..BAND_LEVELS {
            let set = &sets[n];
            // Whether some finer level fillets with another band than n's: the bands narrow with
            // the level until they reach the band limit's.
            let narrows = bands[n + 1..]
                .iter()
                .any(|b| b.total_cmp(&bands[n]).is_ne());
            changes.clear();
            heights.clear();
            for (k, (octave, later)) in marks.iter().zip(&later).enumerate() {
                let evaluated = set.craters.is_some_and(|last| usize::from(last) >= k);
                let finer = later[n + 1];
                let change = if evaluated {
                    if narrows {
                        octave.fillet_m[n] + finer
                    } else {
                        0.0
                    }
                } else {
                    octave.fresh_m + finer
                };
                changes.push((octave.expected, change));
                if evaluated {
                    heights.push((octave.expected, octave.fresh_m + octave.fillet_m[n]));
                }
            }
            // The coarse craters' rims sharpen at every level, from the coarse band down.
            for (octave, later) in rims.iter().zip(&rims_later) {
                let change = if narrows {
                    octave.fillet_m[n] + later[n + 1]
                } else {
                    0.0
                };
                changes.push((octave.expected, change));
                heights.push((octave.expected, octave.fillet_m[n] + octave.coarse_m));
            }
            bound_m[n] = num::assert_finite(
                relief.omitted_m(set.relief)
                    + channels.omitted_m(set.channels)
                    + poisson_tail_bound(&changes, ln_tail),
            );
            let (relief_reach, (channels_low, channels_high)) =
                (relief.reach_m(set.relief), channels.reach_m(set.channels));
            let craters_reach = poisson_tail_bound(&heights, ln_tail);
            low_m[n] = num::assert_finite(-(relief_reach + channels_low + craters_reach));
            high_m[n] = num::assert_finite(relief_reach + channels_high + craters_reach);
        }
        Self {
            sets,
            bound_m,
            low_m,
            high_m,
        }
    }

    /// The contributions `level` evaluates.
    #[must_use]
    pub fn set(&self, level: BandLevel) -> &LevelSet {
        &self.sets[usize::from(level.get())]
    }

    /// The level bound at `level`, metres: a bound on |h(n) − h(m)| at any point for every finer
    /// level m ([`level_bound_m`]).
    #[must_use]
    pub fn bound_m(&self, level: BandLevel) -> f64 {
        self.bound_m[usize::from(level.get())]
    }

    /// The lowest and highest height `level`'s contributions can add to the base elevation, metres:
    /// the octaves' and channels' hard reach and the craters' quantile at [`CRATER_BOUND_TAIL`].
    #[must_use]
    pub fn contribution_range_m(&self, level: BandLevel) -> (f64, f64) {
        let n = usize::from(level.get());
        (self.low_m[n], self.high_m[n])
    }
}

/// Each level's largest value of `by_level` from it on, δ*(n) = max over m ≥ n, with a zero beyond
/// the deepest level.
#[must_use]
fn suffix_max(by_level: &[f64; BAND_LEVELS]) -> [f64; BAND_LEVELS + 1] {
    let mut out = [0.0; BAND_LEVELS + 1];
    for n in (0..BAND_LEVELS).rev() {
        out[n] = num::max(by_level[n], out[n + 1]);
    }
    out
}

/// The band level `n`, an index below [`BAND_LEVELS`].
#[must_use]
fn band(n: usize) -> BandLevel {
    BandLevel::new(u8::try_from(n).expect("a band level index fits a u8"))
        .expect("an index below BAND_LEVELS is a band level")
}

/// The relief's octave indices, 0 to 31 (`relief::octave_spacing`'s range).
const OCTAVE_INDICES: usize = 32;

/// The structural octaves' bounds on a body: each octave's [`octave_bound`], by index.
struct Octaves {
    first: u8,
    finest: u8,
    bound_m: [f64; OCTAVE_INDICES],
}

impl Octaves {
    #[must_use]
    fn of(header: &FieldHeader) -> Self {
        let first = first_octave(header.level());
        let finest = finest_octave(header.radius());
        let mut bound_m = [0.0; OCTAVE_INDICES];
        for m in first..=finest {
            bound_m[usize::from(m)] = octave_bound(header.spectrum(), m).value();
        }
        Self {
            first,
            finest,
            bound_m,
        }
    }

    /// The octaves `through` leaves out: Σ bound over them, in octave order.
    #[must_use]
    fn omitted_m(&self, through: Option<u8>) -> f64 {
        let from = through.map_or(self.first, |t| t + 1);
        (from..=self.finest)
            .map(|m| self.bound_m[usize::from(m)])
            .sum()
    }

    /// The largest |height| the octaves through `through` can reach: Σ bound over them.
    #[must_use]
    fn reach_m(&self, through: Option<u8>) -> f64 {
        through.map_or(0.0, |t| {
            (self.first..=t.min(self.finest))
                .map(|m| self.bound_m[usize::from(m)])
                .sum()
        })
    }
}

/// The channels' bounds on a body: each level's groove cap `G_k` from the coarse level to the
/// deepest any of its cells can evaluate.
struct ChannelCaps {
    first: u8,
    deepest: u8,
    cap_m: [f64; BAND_LEVELS],
}

impl ChannelCaps {
    #[must_use]
    fn of(header: &FieldHeader) -> Self {
        let radius_m = header.radius().value();
        let first = header.level().get();
        let deepest = deepest_possible_level(radius_m, first);
        let mut cap_m = [0.0; BAND_LEVELS];
        for k in first..=deepest {
            cap_m[usize::from(k)] = groove_cap_m(radius_m, k);
        }
        Self {
            first,
            deepest,
            cap_m,
        }
    }

    /// (1 + 1 ÷ B), each level's correction over its cap at most, with B the bubble's mean floor.
    #[must_use]
    fn correction_factor() -> f64 {
        1.0 + 1.0 / BUBBLE_MEAN_FLOOR
    }

    /// The levels `through` leaves out, from k on: (1 + 1 ÷ B) Σ `G_j` over them (the
    /// [module documentation](self)).
    #[must_use]
    fn omitted_m(&self, through: Option<u8>) -> f64 {
        let from = through.map_or(self.first, |t| t + 1);
        if from > self.deepest {
            return 0.0;
        }
        let caps: f64 = (from..=self.deepest)
            .map(|k| self.cap_m[usize::from(k)])
            .sum();
        Self::correction_factor() * caps
    }

    /// How far below and above the base elevation the levels through `through` can reach, both
    /// positive: their raw incision at most the coarse level's cap deep, each level's correction
    /// within −`G_k` (1 + 1 ÷ B) and `G_k` ÷ B.
    #[must_use]
    fn reach_m(&self, through: Option<u8>) -> (f64, f64) {
        let Some(t) = through else {
            return (0.0, 0.0);
        };
        let caps: f64 = (self.first..=t.min(self.deepest))
            .map(|k| self.cap_m[usize::from(k)])
            .sum();
        (
            self.cap_m[usize::from(self.first)] + caps / BUBBLE_MEAN_FLOOR,
            Self::correction_factor() * caps,
        )
    }
}

/// The level bound of the body of `header` at `level`, metres: a bound on |h(n) − h(m)| at any
/// point for every finer level m, hard for the structural and channel terms, while the crater
/// terms, the small craters' and the coarse rims', are a 1 − 10⁻⁹ per-point quantile
/// ([`CRATER_BOUND_TAIL`]; the [module documentation](self)); zero from the shallowest level that
/// evaluates every contribution with the finest band on.
///
/// It builds the body's [`LevelBounds`], a few milliseconds; a caller that reads several levels
/// builds those once.
///
/// # Panics
///
/// As [`CraterOctaves::new`], which no header a field holds reaches.
#[must_use]
pub fn level_bound_m(header: &FieldHeader, level: BandLevel) -> f64 {
    LevelBounds::new(header).bound_m(level)
}

/// The small craters' marks, for the tests.
#[cfg(test)]
pub(crate) fn marks_of(header: &FieldHeader) -> Vec<super::craters::bound::OctaveMarks> {
    let figure = header.figure();
    let bands: [f64; BAND_LEVELS] =
        core::array::from_fn(|n| band_half_width(&figure, band(n)).value());
    octave_marks(&CraterOctaves::new(header), &bands)
}

#[cfg(test)]
mod tests;
