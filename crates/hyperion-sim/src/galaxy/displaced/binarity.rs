//! The binarity seam: the one stripped share the class quadrature and plan 11's systems both read
//! (plan 08, P08.T8.a; plan 11, P11.T1.d).
//!
//! A progenitor whose envelope a companion strips dies with a lighter core and, below a
//! carbon–oxygen core of 3 M☉, may take the kick law's low mode (plan 06, design note 11), so the
//! share of stripped primaries sets how many remnants stay near their birth sites. It is built on
//! plan 11's quadrature of the companions the hierarchy draw gives,
//! [`band_share_as_drawn`](crate::stellar::multiplicity::band_share_as_drawn): the share of
//! primaries whose innermost companion's periastron lies in a band of periastra that depends on
//! the mass ratio, over plan 11's count, period, mass-ratio and eccentricity laws (ruling 81's
//! direct construction from 3 M☉, [`MultiplicityModel::default_v1`] below).
//!
//! # Stripped is not interacting (ruling 123 of 2026-09-22)
//!
//! A companion strips the primary when the primary fills its Roche lobe before its core helium
//! runs out (Case A or B; Podsiadlowski et al. 2004), unless the transfer is unstable early enough
//! to merge the pair. Each case is a periastron: the primary's largest radius up to the end of a
//! stage ([`Track::max_radius_until`] at the median draws; [`StageRadii`]) over Eggleton's (1983)
//! lobe fraction `f(m₁ ÷ m₂)`:
//!
//! - `a_A` to the end of the main sequence, `a_HG` to the end of the Hertzsprung gap, `a_B` to the
//!   end of core helium burning (the death if the star dies in it);
//! - `a_merge` is `a_HG` for `q < 1 ÷ GAP_Q` (1/4), `a_A` for `1 ÷ GAP_Q ≤ q < 1 ÷ CODE_Q` (1/3),
//!   and 0 above: the binary engine's own onset ratios (`stellar::binary`'s `GAP_Q` and `CODE_Q`,
//!   BSE sections 2.6.1 and 2.6.3), read, not copied. A gap donor's common envelope counts as a
//!   merger, a giant's or supergiant's as stripped;
//! - stripped means a periastron in `(a_merge, a_B]` ([`stripping_band`]), and the share is the
//!   quadrature over that band ([`exact_stripped_share`]): exact, and drawing nothing.
//!
//! [`interacting_periastron`] stays P11.T4.a's [`can_interact`] lobe test, either star's largest
//! radius up to the primary's death (a companion that has not arrived on its main sequence by then
//! at its zero-age main-sequence radius, as the engine carries it since P11.T4.i), for the
//! engine's gate ([`interacting_share`]); ruling 123.3 checks stripped over interacting at
//! 0.5–0.75. It is the drawn-orbit part of [`can_interact`]'s test only: since P11.T4.j the
//! pre-test also passes a pair whose orbit the engine's own sinks can shrink into it, giants'
//! tidal captures beyond this periastron among them (to the 1.6 times it where the ruling
//! p11-channels' sample of 2026-10-06 ends). Those captures are late, Case C, at the giant's
//! largest radius, outside the stripping band.
//!
//! # The table
//!
//! The quadrature and the track behind the radii cost some milliseconds a star, and plan 06's
//! mark, the kick law and plan 11's hierarchy draw read them for every massive star. So the share
//! and the three radii are tabulated offline ([`StrippingTable`], `hyperion-fit`'s `stripping`
//! task, `tables::stripping`) over initial mass and the metallicity the formulae see, and
//! interpolated: [`stripped_share`] and [`stripping_band`] read the table inside its domain and
//! the exact quadrature and track outside it. Every reader reads the same two functions, so the
//! mark, the kick law, the hierarchy draw and plan 08's class table (P08.T9) agree.
//!
//! Plan 06's companion-stripped mark is read against [`stripped_share`] (P11.T1.d, edit 4), and
//! plan 11's hierarchy draws a marked primary's innermost orbit from its stripping band and an
//! unmarked one's from the band's complement (ruling 123.5). [`kick_bins`](super::kick_bins)
//! reads the share too.
//!
//! [`can_interact`]: crate::stellar::binary::can_interact

use crate::math;
use crate::orbit::roche_lobe_radius;
use crate::rng::{Mark, Threshold};
use crate::stellar::Composition;
use crate::stellar::binary::{CODE_Q, GAP_Q};
use crate::stellar::draws::{StarDraws, StarDrawsParts};
use crate::stellar::multiplicity::{
    MultiplicityModel, band_share_as_drawn, stripped_share_as_drawn as multiplicity_share,
};
use crate::stellar::sse::{MAX_INITIAL_MASS, MIN_INITIAL_MASS, Stage, Track, main_sequence_start};
use crate::tables::stripping;
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{Metres, SolarMasses, Years};

/// The median draws with the companion-stripped mark [`NEVER_STRIPPED`]: a star evolving as it
/// would alone, whose radii say where a companion would strip it.
#[must_use]
fn unstripped_median() -> StarDraws {
    StarDraws::from_parts(StarDrawsParts {
        stripped: NEVER_STRIPPED,
        ..StarDrawsParts::MEDIAN
    })
}

/// A star's largest radius up to `until` (or its whole life), in solar radii, from its track at
/// the median draws; zero below the tracks' lightest mass. Tracks stop at [`MAX_INITIAL_MASS`]
/// (150 M☉ since P06.T14), so a heavier star is taken as one of that mass. A star that has not
/// arrived on its main sequence by `until` takes its radius at its arrival, its zero-age
/// main-sequence radius, as [`can_interact`] reads it (P11.T4.i). The age returned is `until`,
/// or the star's death.
///
/// [`can_interact`]: crate::stellar::binary::can_interact
fn largest_radius(m: SolarMasses, comp: &Composition, until: Option<Years>) -> (f64, Years) {
    if m < MIN_INITIAL_MASS {
        return (0.0, Years::ZERO);
    }
    let m0 = if m > MAX_INITIAL_MASS {
        MAX_INITIAL_MASS
    } else {
        m
    };
    let draws = unstripped_median();
    let track = match until {
        Some(age) => {
            let reach = age.value().max(main_sequence_start(m0, comp));
            Track::to_age(m0, comp, &draws, Years::new(reach))
        }
        None => Track::full(m0, comp, &draws),
    };
    let end = until
        .or_else(|| track.lifetime())
        .unwrap_or_else(|| track.built_until());
    let read = match track.main_sequence_arrival() {
        Some(arrival) if arrival > end => arrival,
        Some(_) | None => end,
    };
    (track.max_radius_until(read).value(), end)
}

/// The largest periastron at which stars of `r_1` and `r_2` solar radii and masses `m1` and `m2`
/// fill a Roche lobe (module documentation).
fn threshold(r_1: f64, r_2: f64, m1: f64, m2: f64) -> Metres {
    let unit = Metres::new(SOLAR_RADIUS_M);
    let reach = |r: f64, m: f64, other: f64| {
        if r > 0.0 && m > 0.0 && other > 0.0 {
            r * SOLAR_RADIUS_M / roche_lobe_radius(m / other, unit).value() * SOLAR_RADIUS_M
        } else {
            0.0
        }
    };
    Metres::new(reach(r_1, m1, m2).max(reach(r_2, m2, m1)))
}

/// The largest periastron at which a pair of primary `m1`, mass ratio `q` and composition `comp`
/// reaches a Roche lobe on its drawn orbit before the primary's core collapse: the lobe test of
/// P11.T4.a's [`can_interact`] (module documentation), which since P11.T4.j also passes some pairs
/// beyond it.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::binarity::interacting_periastron;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::{AstronomicalUnits, SolarMasses};
///
/// // A 15 M☉ red supergiant reaches a companion several au out.
/// let a = interacting_periastron(SolarMasses::new(15.0), 0.5, &Composition::SOLAR);
/// let au = AstronomicalUnits::from(a).value();
/// assert!((2.0..40.0).contains(&au), "{au} au");
/// ```
///
/// [`can_interact`]: crate::stellar::binary::can_interact
#[must_use]
pub fn interacting_periastron(m1: SolarMasses, q: f64, comp: &Composition) -> Metres {
    let (r_1, death) = largest_radius(m1, comp, None);
    let m2 = m1 * q;
    let (r_2, _) = largest_radius(m2, comp, Some(death));
    threshold(r_1, r_2, m1.value(), m2.value())
}

/// The periastra a companion of mass ratio `q` strips its primary between (ruling 123.2): a
/// periastron in `(merge, strip]` is Case A or B transfer that does not merge the pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrippingBand {
    /// `a_merge`: at or inside it the pair merges (0 for `q ≥ 1/3`).
    pub merge: Metres,
    /// `a_B`: beyond it the primary never fills its lobe before its core helium runs out.
    pub strip: Metres,
}

impl StrippingBand {
    /// Whether a periastron of `periastron` lies in the band: after `merge`, at most `strip`.
    #[must_use]
    pub fn contains(&self, periastron: Metres) -> bool {
        self.merge < periastron && periastron <= self.strip
    }
}

/// A primary's largest radii, solar radii, up to the end of its main sequence, of its Hertzsprung
/// gap and of core helium burning (the death if it dies sooner), from its full track at the median
/// draws: what its stripping band is made of (ruling 123.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageRadii {
    /// R☉.
    main_sequence: f64,
    /// R☉.
    hertzsprung_gap: f64,
    /// R☉.
    core_helium_burning: f64,
}

impl StageRadii {
    /// The radii, R☉, to the end of the main sequence, of the Hertzsprung gap and of core helium
    /// burning.
    #[must_use]
    pub const fn new(
        main_sequence_rsun: f64,
        hertzsprung_gap_rsun: f64,
        core_helium_burning_rsun: f64,
    ) -> Self {
        Self {
            main_sequence: main_sequence_rsun,
            hertzsprung_gap: hertzsprung_gap_rsun,
            core_helium_burning: core_helium_burning_rsun,
        }
    }

    /// The largest radius to the end of the main sequence, R☉.
    #[must_use]
    pub const fn main_sequence_rsun(&self) -> f64 {
        self.main_sequence
    }

    /// The largest radius to the end of the Hertzsprung gap, R☉.
    #[must_use]
    pub const fn hertzsprung_gap_rsun(&self) -> f64 {
        self.hertzsprung_gap
    }

    /// The largest radius to the end of core helium burning, R☉.
    #[must_use]
    pub const fn core_helium_burning_rsun(&self) -> f64 {
        self.core_helium_burning
    }

    /// The radii of a primary of initial mass `m1` and composition `comp` from its own track (at
    /// 150 M☉ above the tracks' range): a full track, about a millisecond.
    #[must_use]
    pub fn of_track(m1: SolarMasses, comp: &Composition) -> Self {
        let m0 = if m1 > MAX_INITIAL_MASS {
            MAX_INITIAL_MASS
        } else {
            m1
        };
        Self::of_built(&Track::full(m0, comp, &unstripped_median()))
    }

    /// The radii read from a primary's full track `track`, built at [`StageRadii::of_track`]'s
    /// draws (or at draws the track cannot tell from them).
    pub(crate) fn of_built(track: &Track) -> Self {
        let death = track.lifetime().unwrap_or_else(|| track.built_until());
        let [
            main_sequence_rsun,
            hertzsprung_gap_rsun,
            core_helium_burning_rsun,
        ] = [
            Stage::MainSequence,
            Stage::HertzsprungGap,
            Stage::CoreHeliumBurning,
        ]
        .map(|stage| {
            let end = track
                .stage_end(stage)
                .map_or(death, |e| if e < death { e } else { death });
            track.max_radius_until(end).value()
        });
        Self::new(
            main_sequence_rsun,
            hertzsprung_gap_rsun,
            core_helium_burning_rsun,
        )
    }

    /// The radii the seam reads: [`StrippingTable::generator`]'s inside its domain, else
    /// [`StageRadii::of_track`].
    #[must_use]
    pub fn of(m1: SolarMasses, comp: &Composition) -> Self {
        stripping_node(m1, comp).radii
    }

    /// The band these radii give a companion of mass ratio `q` (module documentation).
    ///
    /// # Panics
    ///
    /// If `q` is not positive and finite.
    #[must_use]
    pub fn band(&self, q: f64) -> StrippingBand {
        let fraction = roche_lobe_radius(1.0 / q, Metres::new(1.0)).value();
        let reach = |r: f64| Metres::new(r * SOLAR_RADIUS_M / fraction);
        let merge = if q < 1.0 / GAP_Q {
            reach(self.hertzsprung_gap)
        } else if q < 1.0 / CODE_Q {
            reach(self.main_sequence)
        } else {
            Metres::ZERO
        };
        StrippingBand {
            merge,
            strip: reach(self.core_helium_burning),
        }
    }
}

/// The stripping band of a pair of primary `m1`, mass ratio `q` and composition `comp` (module
/// documentation, "Stripped is not interacting"): what plan 11's hierarchy draws a marked
/// innermost orbit from, the radii [`StageRadii::of`]'s.
///
/// # Panics
///
/// If `q` is not positive and finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::binarity::stripping_band;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::{AstronomicalUnits, SolarMasses};
///
/// let m = SolarMasses::new(15.0);
/// // An equal-mass companion never merges; a light one merges out to the gap's reach.
/// let twin = stripping_band(m, 0.9, &Composition::SOLAR);
/// let light = stripping_band(m, 0.2, &Composition::SOLAR);
/// assert!(twin.merge.value().abs() < f64::EPSILON && light.merge > twin.merge);
/// assert!(AstronomicalUnits::from(twin.strip).value() > 1.0);
/// ```
#[must_use]
pub fn stripping_band(m1: SolarMasses, q: f64, comp: &Composition) -> StrippingBand {
    StageRadii::of(m1, comp).band(q)
}

/// The share of primaries of initial mass `m` and composition `comp` whose envelope a companion
/// strips before their core helium runs out, mergers excluded, by quadrature: plan 11's
/// [`band_share_as_drawn`] over the band of the primary's own track's radii
/// ([`StageRadii::of_track`]), some tens of milliseconds (ruling 123.2; P11.T1.d).
///
/// # Panics
///
/// If `m` is not positive and finite.
#[must_use]
pub fn exact_stripped_share(m: SolarMasses, comp: &Composition) -> f64 {
    band_share_of(m, comp, StageRadii::of_track(m, comp))
}

/// [`exact_stripped_share`]'s quadrature over the bands of `radii`.
fn band_share_of(m: SolarMasses, comp: &Composition, radii: StageRadii) -> f64 {
    let model = MultiplicityModel::default_v1();
    band_share_as_drawn(
        &model,
        m,
        comp,
        |_: SolarMasses, q: f64, _: &Composition| {
            let band = radii.band(q);
            (band.merge, band.strip)
        },
    )
}

/// The share of primaries of initial mass `m` and composition `comp` whose envelope a companion
/// strips before their core helium runs out, mergers excluded: [`StrippingTable::generator`]'s
/// inside its domain, else [`exact_stripped_share`]. It is the probability plan 06's
/// companion-stripped mark is read against (P11.T1.d).
///
/// # Panics
///
/// If `m` is not positive and finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::binarity::stripped_share;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::SolarMasses;
///
/// let share = stripped_share(SolarMasses::new(18.0), &Composition::SOLAR);
/// assert!((0.25..0.7).contains(&share), "{share}");
/// ```
#[must_use]
pub fn stripped_share(m: SolarMasses, comp: &Composition) -> f64 {
    StrippingTable::generator()
        .at(m, comp)
        .map_or_else(|| exact_stripped_share(m, comp), |node| node.share)
}

/// [`stripped_share`] with the primary's full track `track`, already built at the median draws
/// with the mark [`NEVER_STRIPPED`], standing in for the exact path's own (P08.T9's build shares
/// it with the kick bins' unmarked fate, ruling 128.5): the same bits, and no second track outside
/// the table's domain.
pub(crate) fn stripped_share_on(m: SolarMasses, comp: &Composition, track: &Track) -> f64 {
    StrippingTable::generator().at(m, comp).map_or_else(
        || band_share_of(m, comp, StageRadii::of_built(track)),
        |node| node.share,
    )
}

/// The stripped share and stage radii of a primary of initial mass `m1` and composition `comp`
/// in one reading: [`StrippingTable::generator`]'s node inside its domain, else
/// [`StrippingNode::of`] (the exact track and quadrature). [`stripped_share`] and
/// [`StageRadii::of`] are its two halves, and they agree with it.
///
/// # Panics
///
/// If `m1` is not positive and finite.
#[must_use]
pub fn stripping_node(m1: SolarMasses, comp: &Composition) -> StrippingNode {
    StrippingTable::generator()
        .at(m1, comp)
        .unwrap_or_else(|| StrippingNode::of(m1, comp))
}

/// A companion-stripped mark that no share below 1 sets: the largest mark, 2⁵³ − 1, which lies
/// below a probability's threshold only for p > 1 − 2⁻⁵³. [`is_stripped`] answers it without the
/// share, which is how the radii's own tracks ([`StageRadii::of_track`]) stay unstripped without
/// asking for the share they are part of.
pub const NEVER_STRIPPED: Mark = Mark::from_word(u64::MAX);

/// Whether plan 06's companion-stripped mark `mark` is set for a primary of initial mass `m` and
/// composition `comp`: below the probability [`stripped_share`] (plan 11, P11.T1.d, edit 4; plan
/// 06, design note 11). [`NEVER_STRIPPED`] never is.
///
/// # Panics
///
/// If `m` is not positive and finite.
#[must_use]
pub fn is_stripped(mark: Mark, m: SolarMasses, comp: &Composition) -> bool {
    mark != NEVER_STRIPPED && is_stripped_at(mark, stripped_share(m, comp))
}

/// Whether the companion-stripped mark `mark` is set against the stripped share `share`, the
/// comparison [`is_stripped`] makes once it has the share (for a caller that already holds it,
/// through [`stripping_node`]). [`NEVER_STRIPPED`] is set only for a share of 1.
#[must_use]
pub fn is_stripped_at(mark: Mark, share: f64) -> bool {
    mark.is_below(Threshold::from_probability(share))
}

/// The share of primaries of initial mass `m` and composition `comp` that interact with their
/// innermost companion before they die, mergers and Case C included: plan 11's quadrature of the
/// drawn companions at [`interacting_periastron`], the primary's track built once. It counts the
/// drawn orbits the lobe test passes, not the giants' tidal captures beyond it (module
/// documentation).
///
/// # Panics
///
/// If `m` is not positive and finite.
#[must_use]
pub fn interacting_share(m: SolarMasses, comp: &Composition) -> f64 {
    let (r_1, death) = largest_radius(m, comp, None);
    multiplicity_share(
        &MultiplicityModel::default_v1(),
        m,
        comp,
        |m1: SolarMasses, q: f64, comp: &Composition| {
            let m2 = m1 * q;
            let (r_2, _) = largest_radius(m2, comp, Some(death));
            threshold(r_1, r_2, m1.value(), m2.value())
        },
    )
}

/// The initial masses, M☉, of [`StrippingTable`]'s first and last nodes: from below the lowest
/// primary plan 06's mark is read for (5.72 M☉, `stripped_mark_min_mass`'s floor) to the tracks'
/// top.
pub const STRIPPING_MASS_RANGE: (f64, f64) = (5.5, 150.0);

/// The number of [`StrippingTable`]'s masses, even in ln m: 34, a step of 0.100 in ln m.
pub const STRIPPING_MASSES: usize = 34;

/// The number of [`StrippingTable`]'s metallicities, even in \[Fe/H\] of `z_fit` over the fitted
/// range, Z = 10⁻⁴ to 0.03: 11, a step of 0.248 dex.
pub const STRIPPING_METALLICITIES: usize = 11;

/// One node of [`StrippingTable`]: a primary's stripped share and its stage radii.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrippingNode {
    share: f64,
    radii: StageRadii,
}

impl StrippingNode {
    /// The stripped share, [`exact_stripped_share`] at a node.
    #[must_use]
    pub const fn share(&self) -> f64 {
        self.share
    }

    /// The stage radii, [`StageRadii::of_track`] at a node.
    #[must_use]
    pub const fn radii(&self) -> StageRadii {
        self.radii
    }

    /// The node of a primary of initial mass `m1` and composition `comp`: its track and the
    /// quadrature, as `hyperion-fit`'s `stripping` task computes each node.
    #[must_use]
    pub fn of(m1: SolarMasses, comp: &Composition) -> Self {
        Self {
            share: exact_stripped_share(m1, comp),
            radii: StageRadii::of_track(m1, comp),
        }
    }

    /// The node as the table stores it: (share, log₁₀ of each radius in R☉).
    #[must_use]
    pub fn to_row(self) -> (f64, f64, f64, f64) {
        (
            self.share,
            math::log10(self.radii.main_sequence),
            math::log10(self.radii.hertzsprung_gap),
            math::log10(self.radii.core_helium_burning),
        )
    }
}

/// The stripped share and the stage radii tabulated over initial mass and metallicity (P11.T1.d):
/// [`STRIPPING_MASSES`] masses even in ln m over [`STRIPPING_MASS_RANGE`] by
/// [`STRIPPING_METALLICITIES`] values of \[Fe/H\] of `z_fit`, `log₁₀(z_fit ÷ 0.02)`, even from
/// Z = 10⁻⁴ to 0.03 ([`stripping_metallicities`]), each node [`StrippingNode::of`] at no helium
/// excess, interpolated bilinearly in ln m and \[Fe/H\], the radii in their logarithms.
///
/// A radius can jump between neighbouring nodes, where a star's giant branch changes from blue to
/// red, and the interpolant then gives radii between the two; the share, an integral over mass
/// ratios and periods, varies smoothly. `hyperion-fit`'s `stripping` task writes the table and
/// validates it at the cells' centres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrippingTable<'t> {
    /// The nodes, `(share, log₁₀ r_MS, log₁₀ r_HG, log₁₀ r_CHeB)`, mass fastest.
    nodes: &'t [(f64, f64, f64, f64)],
}

/// The masses of [`StrippingTable`]'s nodes, M☉, ascending.
#[must_use]
pub fn stripping_masses() -> Vec<f64> {
    even(
        math::ln(STRIPPING_MASS_RANGE.0),
        math::ln(STRIPPING_MASS_RANGE.1),
        STRIPPING_MASSES,
    )
    .into_iter()
    .map(math::exp)
    .collect()
}

/// The metallicities of [`StrippingTable`]'s nodes, \[Fe/H\] of `z_fit` (log₁₀(`z_fit` ÷ 0.02)),
/// ascending.
#[must_use]
pub fn stripping_metallicities() -> Vec<f64> {
    let (lowest, highest) = metallicity_range();
    even(lowest, highest, STRIPPING_METALLICITIES)
}

/// The first and last of [`stripping_metallicities`].
#[must_use]
fn metallicity_range() -> (f64, f64) {
    use crate::stellar::composition::{Z_FIT_MAX, Z_FIT_MIN, Z_SOLAR};
    (
        math::log10(Z_FIT_MIN.value() / Z_SOLAR.value()),
        math::log10(Z_FIT_MAX.value() / Z_SOLAR.value()),
    )
}

/// `n` values evenly from `lo` to `hi`, the ends exact.
#[must_use]
fn even(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let last = u32::try_from(n - 1).expect("a few dozen nodes");
    (0..=last)
        .map(|k| {
            if k == last {
                hi
            } else {
                lo + (hi - lo) * f64::from(k) / f64::from(last)
            }
        })
        .collect()
}

/// The number of [`StrippingTable`]'s nodes.
pub const STRIPPING_NODES: usize = STRIPPING_MASSES * STRIPPING_METALLICITIES;

impl StrippingTable<'static> {
    /// The generator's table, [`tables::stripping`](crate::tables::stripping), which holds every
    /// node by its type.
    #[must_use]
    pub const fn generator() -> Self {
        let nodes: &[(f64, f64, f64, f64); STRIPPING_NODES] = &stripping::NODES;
        Self { nodes }
    }
}

impl<'t> StrippingTable<'t> {
    /// A table of `nodes`, mass fastest, or `None` unless it holds all [`STRIPPING_NODES`].
    #[must_use]
    pub fn new(nodes: &'t [(f64, f64, f64, f64)]) -> Option<Self> {
        (nodes.len() == STRIPPING_NODES).then_some(Self { nodes })
    }

    /// The nodes, `(share, log₁₀ r_MS, log₁₀ r_HG, log₁₀ r_CHeB)`, mass fastest.
    #[must_use]
    pub const fn nodes(&self) -> &'t [(f64, f64, f64, f64)] {
        self.nodes
    }

    /// The interpolated node of a primary of initial mass `m1` and composition `comp`, or `None`
    /// outside the table: below its lightest mass, or for a composition with a helium excess. Above
    /// 150 M☉ it reads 150 M☉, as the tracks do.
    #[must_use]
    pub fn at(&self, m1: SolarMasses, comp: &Composition) -> Option<StrippingNode> {
        if comp.helium_excess().value().abs() > 0.0 {
            return None;
        }
        let mass = m1.value().min(STRIPPING_MASS_RANGE.1);
        if mass.is_nan() || mass < STRIPPING_MASS_RANGE.0 {
            return None;
        }
        let (ln_lo, ln_hi) = (
            math::ln(STRIPPING_MASS_RANGE.0),
            math::ln(STRIPPING_MASS_RANGE.1),
        );
        let by_mass = cell(math::ln(mass), ln_lo, ln_hi, STRIPPING_MASSES);
        let fe_h = math::log10(comp.z_fit().value() / 0.02);
        let (lowest, highest) = metallicity_range();
        let by_fe_h = cell(fe_h, lowest, highest, STRIPPING_METALLICITIES);
        let node = |a: usize, b: usize| self.nodes[b * STRIPPING_MASSES + a];
        let mix = |pick: fn(&(f64, f64, f64, f64)) -> f64| {
            let (col, along) = by_mass;
            let row = |b: usize| {
                pick(&node(col, b)) + along * (pick(&node(col + 1, b)) - pick(&node(col, b)))
            };
            let (line, across) = by_fe_h;
            row(line) + across * (row(line + 1) - row(line))
        };
        Some(StrippingNode {
            share: mix(|n| n.0),
            radii: StageRadii::new(
                math::exp10(mix(|n| n.1)),
                math::exp10(mix(|n| n.2)),
                math::exp10(mix(|n| n.3)),
            ),
        })
    }
}

/// The cell of `n` even nodes from `lo` to `hi` holding `x` (clamped to them) and the weight of
/// its upper node.
#[must_use]
fn cell(x: f64, lo: f64, hi: f64, n: usize) -> (usize, f64) {
    let cells = f64::from(u32::try_from(n - 1).expect("a few dozen nodes"));
    let position = ((x - lo) / (hi - lo) * cells).clamp(0.0, cells);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a position clamped to 0 and the last cell"
    )]
    let index = (position.floor() as usize).min(n - 2);
    (
        index,
        position - f64::from(u32::try_from(index).expect("a few dozen nodes")),
    )
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::stellar::remnant::RemnantKind;
    use crate::units::{Dex, HeliumExcess};

    fn layer_e(i: i32) -> SolarMasses {
        SolarMasses::new(8.0 * math::exp(f64::from(i) / 32.0 * math::ln(150.0 / 8.0)))
    }

    /// P08.T8.a with ruling 123.2 and P11.T1.d: the exact share is plan 11's quadrature of the
    /// drawn companions over the band, bit for bit, and the interacting share is it at
    /// `can_interact`'s boundary, at masses across layer E's band and three metallicities.
    #[test]
    fn the_seam_is_plan_elevens_quadrature_over_the_band() {
        let model = MultiplicityModel::default_v1();
        for fe_h in [-1.5, 0.0, 0.3] {
            let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            for m in (0..33).step_by(16).map(layer_e) {
                let radii = StageRadii::of_track(m, &comp);
                let band = |_: SolarMasses, q: f64, _: &Composition| {
                    let b = radii.band(q);
                    (b.merge, b.strip)
                };
                let share = exact_stripped_share(m, &comp);
                assert_same_bits(share, band_share_as_drawn(&model, m, &comp, band));
                assert!((0.0..=1.0).contains(&share), "{share} at {m:?}");
                let interacting = multiplicity_share(&model, m, &comp, interacting_periastron);
                assert_same_bits(interacting_share(m, &comp), interacting);
                assert!(share <= interacting, "{share} over {interacting} at {m:?}");
            }
        }
    }

    /// The band's edges: `a_merge` is the gap's reach below q = 1/4, the main sequence's to 1/3 and
    /// 0 above, never beyond `a_B`, and `a_B` never beyond `can_interact`'s boundary.
    #[test]
    fn the_band_follows_the_engines_onset_ratios() {
        let comp = Composition::SOLAR;
        for m in [9.0, 15.0, 40.0] {
            let m = SolarMasses::new(m);
            let radii = StageRadii::of_track(m, &comp);
            assert!(
                radii.main_sequence_rsun() <= radii.hertzsprung_gap_rsun()
                    && radii.hertzsprung_gap_rsun() <= radii.core_helium_burning_rsun(),
                "{radii:?}"
            );
            let light = radii.band(0.2);
            let middle = radii.band(0.3);
            let heavy = radii.band(0.5);
            let reach = |r: f64, q: f64| {
                r * SOLAR_RADIUS_M / roche_lobe_radius(1.0 / q, Metres::new(1.0)).value()
            };
            assert!(
                (light.merge.value() / reach(radii.hertzsprung_gap_rsun(), 0.2) - 1.0).abs()
                    < 1e-12
            );
            assert!(
                (middle.merge.value() / reach(radii.main_sequence_rsun(), 0.3) - 1.0).abs() < 1e-12
            );
            assert!(heavy.merge.value().abs() < f64::EPSILON);
            for (band, q) in [(light, 0.2), (middle, 0.3), (heavy, 0.5)] {
                assert!(band.merge <= band.strip);
                assert!(
                    band.strip.value()
                        <= interacting_periastron(m, q, &comp).value() * (1.0 + 1e-12)
                );
                assert!(band.contains(band.strip) && !band.contains(band.merge));
            }
        }
        assert!((1.0 / GAP_Q - 0.25).abs() < 1e-15 && (1.0 / CODE_Q - 1.0 / 3.0).abs() < 1e-15);
    }

    /// The table holds every node, and at the centres of its cells it follows the exact share and
    /// radii: the share within 0.002 in the median cell and 0.1 in every one, the
    /// core-helium-burning radius within 5% where the cell's four radii agree to 25%. A cell
    /// across a blue-to-red change of the giant branch interpolates between the two, and the share
    /// with it (see [`StrippingTable`]; `hyperion-fit`'s `stripping` task validates every cell).
    /// Outside its domain the seam reads the exact functions.
    #[test]
    fn the_table_follows_the_quadrature_and_the_tracks() {
        let table = StrippingTable::generator();
        assert_eq!(
            table.nodes.len(),
            STRIPPING_MASSES * STRIPPING_METALLICITIES
        );
        let masses = stripping_masses();
        let metallicities = stripping_metallicities();
        let mut shares = Vec::new();
        let mut worst_radius: f64 = 0.0;
        for (j, pair) in metallicities.windows(2).enumerate().step_by(3) {
            let fe_h = f64::midpoint(pair[0], pair[1]);
            let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            for (i, masses) in masses.windows(2).enumerate().step_by(4) {
                let m = SolarMasses::new(math::exp(f64::midpoint(
                    math::ln(masses[0]),
                    math::ln(masses[1]),
                )));
                let node = table.at(m, &comp).expect("inside the table");
                let exact = StrippingNode::of(m, &comp);
                shares.push((node.share() - exact.share()).abs());
                let corners = [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)]
                    .map(|(a, b)| table.nodes()[b * STRIPPING_MASSES + a].3);
                let spread = corners.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                    - corners.iter().copied().fold(f64::INFINITY, f64::min);
                if spread < math::log10(1.25) {
                    let r = node.radii().core_helium_burning_rsun()
                        / exact.radii().core_helium_burning_rsun();
                    worst_radius = worst_radius.max((r - 1.0).abs());
                }
            }
        }
        shares.sort_by(f64::total_cmp);
        let (median, worst) = (shares[shares.len() / 2], shares[shares.len() - 1]);
        println!(
            "share errors over {} cells: median {median:.2e}, worst {worst:.4}; worst smooth \
             radius error {worst_radius:.4}",
            shares.len()
        );
        assert!(median < 2e-3 && worst < 0.1, "{median} {worst}");
        assert!(worst_radius < 0.05, "{worst_radius}");
        let light = SolarMasses::new(5.0);
        assert!(table.at(light, &Composition::SOLAR).is_none());
        let rich = Composition::from_fe_h(Dex::new(0.0), HeliumExcess::new(0.05));
        assert!(table.at(SolarMasses::new(20.0), &rich).is_none());
        assert!(StrippingTable::new(&table.nodes()[1..]).is_none());
        let node = stripping_node(SolarMasses::new(12.0), &Composition::SOLAR);
        assert_same_bits(
            node.share(),
            stripped_share(SolarMasses::new(12.0), &Composition::SOLAR),
        );
        assert_same_bits(
            stripped_share(light, &Composition::SOLAR),
            exact_stripped_share(light, &Composition::SOLAR),
        );
    }

    /// The threshold is the boundary of `can_interact`'s lobe test: a pair just inside it can
    /// interact before the primary's death, and one just outside does not reach a lobe on its
    /// drawn orbit (`lobe_reached`). The decay the engine's sinks can make passes some pairs
    /// beyond it (P11.T4.j).
    #[test]
    fn the_threshold_is_can_interacts_boundary() {
        use crate::orbit::{Eccentricity, KeplerElements, Orientation};
        use crate::stellar::binary::{BinaryInput, can_interact, lobe_reached};
        use crate::units::{GravitationalParameter, Radians};
        let comp = Composition::SOLAR;
        for (m1, q) in [(12.0, 0.6), (25.0, 0.3), (9.0, 0.95)] {
            let m1 = SolarMasses::new(m1);
            let a = interacting_periastron(m1, q, &comp);
            let death = Track::full(m1, &comp, &StarDraws::median())
                .lifetime()
                .expect("a full track dies");
            let pair = |factor: f64| {
                let orbit = KeplerElements::from_semi_major_axis(
                    Metres::new(a.value() * factor),
                    GravitationalParameter::from_solar_masses(m1 * (1.0 + q)),
                    Eccentricity::CIRCULAR,
                    Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))
                        .unwrap(),
                    Radians::new(0.0),
                )
                .unwrap();
                BinaryInput::new(
                    m1,
                    m1 * q,
                    comp,
                    orbit,
                    [StarDraws::median(), StarDraws::median()],
                    death,
                )
                .unwrap()
            };
            assert!(
                can_interact(&pair(0.999), death),
                "{m1:?} q {q} just inside"
            );
            assert!(
                lobe_reached(&pair(0.999), death),
                "{m1:?} q {q} just inside"
            );
            assert!(
                !lobe_reached(&pair(1.001), death),
                "{m1:?} q {q} just outside"
            );
        }
    }

    /// Ruling 137.2 (amending 123.3), per primary: over neutron-star progenitors (the masses of
    /// layer E whose track at the median draws leaves a neutron star), weighted by the default mass
    /// function, the stripped share lies in 0.33–0.47, over the whole band in 0.35–0.52 (Moe and
    /// Di Stefano's Table 13 ± 1σ), and stripped ÷ interacting in 0.5–0.75. Per exploding star
    /// (0.28–0.40) is checked once the engine adds the companions' remnants (P11.T6/T11).
    #[test]
    fn the_band_averaged_stripped_share() {
        let imf = MassFunctionKind::default().to_mass_function();
        let comp = Composition::SOLAR;
        // [stripped, interacting, weight] over the band and over its neutron-star progenitors.
        let (mut band, mut ns) = ([0.0; 3], [0.0; 3]);
        for i in 0..33 {
            let m = layer_e(i);
            let end = i == 0 || i == 32;
            let w = imf.pdf(m.value()) * m.value() * if end { 0.5 } else { 1.0 };
            let (stripped, interacting) =
                (exact_stripped_share(m, &comp), interacting_share(m, &comp));
            if i % 8 == 0 {
                eprintln!(
                    "{:.1} M☉: stripped {stripped:.4}, interacting {interacting:.4}",
                    m.value()
                );
            }
            let m0 = if m > MAX_INITIAL_MASS {
                MAX_INITIAL_MASS
            } else {
                m
            };
            let neutron_star = Track::full(m0, &comp, &StarDraws::median())
                .remnant()
                .is_some_and(|r| r.kind() == RemnantKind::NeutronStar);
            for (sum, on) in [(&mut band, true), (&mut ns, neutron_star)] {
                if on {
                    sum[0] += w * stripped;
                    sum[1] += w * interacting;
                    sum[2] += w;
                }
            }
        }
        let (layer, progenitors) = (band[0] / band[2], ns[0] / ns[2]);
        let ratio = ns[0] / ns[1];
        eprintln!(
            "stripped share: layer E {layer:.4}, neutron-star progenitors {progenitors:.4}; \
             stripped ÷ interacting {ratio:.4}"
        );
        assert!((0.33..=0.47).contains(&progenitors), "{progenitors}");
        assert!((0.35..=0.52).contains(&layer), "{layer}");
        assert!((0.5..=0.75).contains(&ratio), "{ratio}");
    }
}
