//! The natal kick a remnant is born with: one law behind one interface (plan 06, P06.T19.a and
//! T19.c).
//!
//! [`KickLaw`] is the interface and [`StandardKickLaw`] the generator version's law, with its
//! parameters in [`KickLawParams`]. The brainstorm's "Displaced objects: kicks and runaways" gives
//! the law:
//!
//! - **The ordinary mode.** Mandel and Müller's (2020, MNRAS 499, 3214, §3, eq. 2) score
//!   x = (`M_CO` − `M_rem`) ÷ `M_rem` × ξ orders the remnants, with ξ normal about 1 with the 45% scatter
//!   of Disberg, Mandel and Hirai (2026, arXiv:2608.19690, §2.4, eq. 22: `f_NS` = 0.45 ± 0.20). Its
//!   rank r = `F_x(x)` over the reference population ([`KickRankTable`], P06.T19.b), clamped to
//!   0.001–0.999, is mapped onto the measured speeds of isolated pulsars younger than 10 Myr, the
//!   log-normal of Disberg and Mandel (2025, ApJ Lett. 989, L8, table 1 and abstract: μ = 5.60 ± 0.12
//!   and σ = 0.68 ± 0.10 in ln(km/s), three-dimensional speeds): v = exp(μ + σ Φ⁻¹(r)) km/s, 33 to
//!   2,210 km/s. So the distribution is the measurement and only the ordering by progenitor is a
//!   model. The direction is isotropic.
//! - **The low mode**, a Maxwellian of σ = 5 km/s, the model Valli et al. (2025, arXiv:2505.08857)
//!   propose for the progenitors that stay bound to their companions (their fit gives
//!   4.5 (+2.8 −3.2) km/s): always for electron capture and accretion-induced collapse, and for a
//!   companion-stripped progenitor with probability clamp(3 − `M_CO` ÷ M☉, 0, 1).
//! - **Black holes** take the ordinary map with their own mass as `M_rem`, times 0.75, and nothing
//!   after complete fallback (Mandel and Müller's eq. 3 kicks every black hole but those).
//!   A black hole of a companion-stripped progenitor with a core of 2–3 M☉ can take the low mode
//!   too, since the ramp is the brainstorm's for "a companion-stripped star".
//! - **White dwarfs** take a Maxwellian of σ = 1 km/s, the brainstorm's "about 1 km/s, which
//!   matters only against an open cluster's escape speed" (a HYPERION default).
//!
//! The ordinary mode's rank is held to 0.001–0.999, plan 06's P06.T19.a, so that no score maps
//! beyond 3.09 σ of the log-normal.
//!
//! The 2–3 M☉ ramp, the black-hole factor and the two electron-capture windows are the four
//! defaults of the brainstorm's "Open questions" (the fifth, the fate of merged binaries, is plan
//! 11's), each a field of [`KickLawParams`] and a constant of [`tables::kick_rank`], and each
//! pinned by one of T19.d's tests. **The black-hole factor of 0.75 is a HYPERION default, not
//! Mandel and Müller's**: their table 1 has `v_BH` ÷ `v_NS` = 200 ÷ 400 = 0.5, applied to the score.
//!
//! A kick is drawn from the star's `star.kick.*` draws ([`KickDraws`]), so that plan 08's kick
//! loop can redraw it from a later attempt's draws on the same track.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;

use super::{CompactRemnant, Death, DeathKind, ProgenitorAtDeath, RemnantKind, Stripping};
use crate::coords::UnitVector;
use crate::math;
use crate::rng::{Mark, Threshold};
use crate::stellar::draws::{REDRAW_TRIES, StandardNormal, StarDraws};
use crate::tables::kick_rank;
use crate::units::{KilometresPerSecond, MetresPerSecond, SolarMasses};

/// Which mode of the kick law a remnant's kick came from (plan 06, P06.T19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KickMode {
    /// The ordinary mode: the measured log-normal, ranked by Mandel and Müller's score.
    Ordinary,
    /// The low mode, a Maxwellian of σ = 5 km/s: electron capture, accretion-induced collapse and
    /// companion-stripped progenitors.
    Low,
    /// A black hole of complete fallback, which has no kick.
    FallbackNone,
    /// A white dwarf's Maxwellian of σ = 1 km/s.
    WhiteDwarf,
}

/// A remnant's natal kick: its speed, its direction along the galactic axes, and the mode it came
/// from (plan 06's Provides).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NatalKick {
    speed: MetresPerSecond,
    direction: UnitVector,
    mode: KickMode,
}

impl NatalKick {
    /// A kick of `speed` (m/s, finite and non-negative) along `direction` from `mode`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `speed` is negative or not finite.
    #[must_use]
    pub(crate) fn new(speed: MetresPerSecond, direction: UnitVector, mode: KickMode) -> Self {
        debug_assert!(
            speed.value().is_finite() && speed.value() >= 0.0,
            "a kick's speed is finite and non-negative: {speed:?}"
        );
        Self {
            speed,
            direction,
            mode,
        }
    }

    /// The kick's speed, m/s, in the frame of the progenitor at its death.
    #[must_use]
    pub const fn speed(&self) -> MetresPerSecond {
        self.speed
    }

    /// The kick's direction along the galactic axes.
    #[must_use]
    pub const fn direction(&self) -> UnitVector {
        self.direction
    }

    /// The mode of the law the kick came from.
    #[must_use]
    pub const fn mode(&self) -> KickMode {
        self.mode
    }

    /// The kick's velocity along the galactic axes, m/s: its speed times its direction.
    #[must_use]
    pub fn velocity(&self) -> [f64; 3] {
        let v = self.speed.value();
        self.direction.components().map(|c| c * v)
    }
}

/// How a remnant was born, as the kick law distinguishes it.
///
/// Plan 06's Provides sketches three channels of collapse; the law also needs to know a complete
/// fallback, which has no kick, and a white dwarf's birth by envelope loss, which has its own
/// Maxwellian, since neither the progenitor nor the remnant says so. So the enum has five.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CollapseChannel {
    /// An iron core's collapse to a neutron star, or to a black hole with partial fallback.
    IronCore,
    /// An oxygen–neon core's collapse by electron capture.
    ElectronCapture,
    /// A white dwarf's collapse to a neutron star by accretion (plan 11 supplies it).
    AccretionInduced,
    /// An iron core's collapse to a black hole with complete fallback, or pulsational pair
    /// instability's black hole ([`DeathKind::DirectCollapse`]).
    CompleteFallback,
    /// No collapse: a white dwarf born when the envelope is lost.
    EnvelopeLoss,
}

impl CollapseChannel {
    /// The channel of a single star's death of `kind`, or `None` for a death that leaves no
    /// remnant to kick (pair instability and thermonuclear disruption).
    #[must_use]
    pub const fn of(kind: DeathKind) -> Option<Self> {
        match kind {
            DeathKind::EnvelopeLoss => Some(Self::EnvelopeLoss),
            DeathKind::ElectronCapture => Some(Self::ElectronCapture),
            DeathKind::CoreCollapse { .. } => Some(Self::IronCore),
            DeathKind::DirectCollapse => Some(Self::CompleteFallback),
            DeathKind::PairInstability | DeathKind::ThermonuclearDisruption => None,
        }
    }
}

/// The parameters of [`StandardKickLaw`]; [`Default`] is the generator version's law.
///
/// Speeds are in km/s, masses in M☉ of carbon–oxygen core or of the initial mass the track's
/// `m_c_bagb` reads. The four defaults of the brainstorm's "Open questions" are read from
/// [`tables::kick_rank`], which plan 15 owns after P06.T19.e.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KickLawParams {
    /// The log-normal's μ, ln(km/s): 5.60 (Disberg and Mandel 2025).
    pub ln_mu: f64,
    /// The log-normal's σ, ln(km/s): 0.68 (Disberg and Mandel 2025).
    pub ln_sigma: f64,
    /// The relative scatter of the score's factor ξ: 0.45 (Disberg, Mandel and Hirai 2026).
    pub score_scatter: f64,
    /// The low mode's Maxwellian σ, km/s: 5 (Valli et al. 2025).
    pub low_sigma_km_s: f64,
    /// The carbon–oxygen cores, M☉, over which a companion-stripped progenitor's chance of the
    /// low mode falls from 1 to 0: 2 to 3 ([`kick_rank::LOW_RAMP`]).
    pub low_ramp: (f64, f64),
    /// The factor on a black hole's ordinary kick: 0.75 ([`kick_rank::BH_FACTOR`]), a HYPERION
    /// default.
    pub bh_factor: f64,
    /// The single-star electron-capture window's width, M☉ of the mass `m_c_bagb` reads: 0.1
    /// ([`kick_rank::EC_WINDOW_SINGLE`]). The track reads its own copy,
    /// [`SINGLE_STAR_WINDOW`](super::collapse::SINGLE_STAR_WINDOW), which a test holds equal; this
    /// field records the default for plan 15's tools.
    pub ec_window_single: f64,
    /// The companion-stripped window's width, M☉ of the same mass: 1.0
    /// ([`kick_rank::EC_WINDOW_STRIPPED`]; the track's copy is
    /// [`COMPANION_STRIPPED_WINDOW`](super::collapse::COMPANION_STRIPPED_WINDOW)).
    pub ec_window_stripped: f64,
    /// The white dwarfs' Maxwellian σ, km/s: 1, the brainstorm's "about 1 km/s".
    pub wd_sigma_km_s: f64,
    /// The ranks the ordinary mode's rank is held to: 0.001–0.999 (P06.T19.a), 33–2,210 km/s.
    pub rank_clamp: (f64, f64),
    /// The provisional share of progenitors a companion strips (plan 06, design note 11): 0.25,
    /// which plan 11 replaces.
    pub stripped_share: f64,
}

impl Default for KickLawParams {
    fn default() -> Self {
        Self {
            ln_mu: 5.60,
            ln_sigma: 0.68,
            score_scatter: 0.45,
            low_sigma_km_s: 5.0,
            low_ramp: kick_rank::LOW_RAMP,
            bh_factor: kick_rank::BH_FACTOR,
            ec_window_single: kick_rank::EC_WINDOW_SINGLE,
            ec_window_stripped: kick_rank::EC_WINDOW_STRIPPED,
            wd_sigma_km_s: 1.0,
            rank_clamp: (0.001, 0.999),
            stripped_share: 0.25,
        }
    }
}

impl KickLawParams {
    /// Whether a star's companion-stripped mark `mark` is set: below the probability
    /// [`KickLawParams::stripped_share`] (plan 06, design note 11).
    #[must_use]
    pub fn is_stripped(&self, mark: Mark) -> bool {
        mark.is_below(Threshold::from_probability(self.stripped_share))
    }

    /// The probability that a companion-stripped progenitor with a carbon–oxygen core of
    /// `co_core` takes the low mode: 1 below the ramp, 0 above it, linear between (with the
    /// defaults, clamp(3 − `M_CO` ÷ M☉, 0, 1)).
    #[must_use]
    pub fn low_mode_probability(&self, co_core: SolarMasses) -> f64 {
        let (lo, hi) = self.low_ramp;
        ((hi - co_core.value()) / (hi - lo)).clamp(0.0, 1.0)
    }

    /// The ordinary mode's speed, km/s, at the rank `rank`: exp(μ + σ Φ⁻¹(r)), with the rank
    /// first held to [`KickLawParams::rank_clamp`].
    #[must_use]
    pub fn ordinary_speed_km_s(&self, rank: f64) -> f64 {
        let (lo, hi) = self.rank_clamp;
        let r = rank.clamp(lo, hi);
        math::exp(math::mul_add(
            self.ln_sigma,
            math::normal_quantile(r),
            self.ln_mu,
        ))
    }

    /// The score's factor ξ from the tries `tries`: the first of 1 + [`score_scatter`] × z that is
    /// positive, which is ξ normal about 1 redrawn until positive (1.3% of tries fail at the
    /// default scatter). If all eight fail, about once in 10¹⁵ stars, ξ is 1, the median.
    ///
    /// [`score_scatter`]: KickLawParams::score_scatter
    #[must_use]
    pub fn score_factor(&self, tries: &[StandardNormal; REDRAW_TRIES]) -> f64 {
        tries
            .iter()
            .map(|z| math::mul_add(self.score_scatter, z.value(), 1.0))
            .find(|&xi| xi > 0.0)
            .unwrap_or(1.0)
    }
}

/// Mandel and Müller's (2020, eq. 2) kick score with its scatter: (`M_CO` − `M_rem`) ÷ `M_rem` × ξ.
///
/// It is negative for a remnant heavier than the core, which ranks it below the whole reference
/// population.
///
/// # Panics
///
/// In debug builds, unless `remnant` is positive.
///
/// # Examples
///
/// A 1.4 M☉ neutron star from a 2.1 M☉ core at the median factor scores ½:
///
/// ```
/// use hyperion_sim::stellar::remnant::ordinary_score;
/// use hyperion_sim::units::SolarMasses;
///
/// let x = ordinary_score(SolarMasses::new(2.1), SolarMasses::new(1.4), 1.0);
/// assert!((x - 0.5).abs() < 1e-12);
/// ```
#[must_use]
pub fn ordinary_score(co_core: SolarMasses, remnant: SolarMasses, xi: f64) -> f64 {
    debug_assert!(
        remnant.value() > 0.0,
        "a kicked remnant has a mass: {remnant:?}"
    );
    (co_core.value() - remnant.value()) / remnant.value() * xi
}

/// `F_x`, the distribution of the ordinary kick score over the reference population (P06.T19.b),
/// by linear interpolation between its quantiles at evenly spaced ranks.
///
/// The generator's table ([`KickRankTable::generator`]) holds the 257 quantiles at ranks i ÷ 256
/// of [`kick_rank::SCORE_QUANTILES`]; any other, such as plan 15's, is built with
/// [`KickRankTable::new`].
#[derive(Debug, Clone, PartialEq)]
pub struct KickRankTable {
    quantiles: Cow<'static, [f64]>,
}

/// Why a [`KickRankTable`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildKickRankTableError {
    /// Fewer than two quantiles.
    TooFewQuantiles,
    /// A quantile is not finite, or not above the one before it; the index is the offender's.
    NotStrictlyIncreasing(usize),
}

impl fmt::Display for BuildKickRankTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewQuantiles => write!(f, "a rank table needs at least two quantiles"),
            Self::NotStrictlyIncreasing(i) => {
                write!(f, "quantile {i} is not finite and above the one before it")
            }
        }
    }
}

impl Error for BuildKickRankTableError {}

impl KickRankTable {
    /// The generator version's table, [`kick_rank::SCORE_QUANTILES`].
    #[must_use]
    pub const fn generator() -> Self {
        Self {
            quantiles: Cow::Borrowed(&kick_rank::SCORE_QUANTILES),
        }
    }

    /// The table of `quantiles` at the evenly spaced ranks i ÷ (n − 1).
    ///
    /// # Errors
    ///
    /// [`BuildKickRankTableError::TooFewQuantiles`] for fewer than two, and
    /// [`BuildKickRankTableError::NotStrictlyIncreasing`] unless every quantile is finite and
    /// above the one before it.
    ///
    /// # Examples
    ///
    /// A two-knot table is `F_x` uniform between its knots:
    ///
    /// ```
    /// use hyperion_sim::stellar::remnant::KickRankTable;
    ///
    /// let table = KickRankTable::new(vec![0.0, 2.0])?;
    /// assert!((table.rank(0.5) - 0.25).abs() < 1e-12);
    /// assert_eq!(table.rank(-1.0), 0.0);
    /// assert_eq!(table.rank(3.0), 1.0);
    /// # Ok::<(), hyperion_sim::stellar::remnant::BuildKickRankTableError>(())
    /// ```
    pub fn new(quantiles: Vec<f64>) -> Result<Self, BuildKickRankTableError> {
        if quantiles.len() < 2 {
            return Err(BuildKickRankTableError::TooFewQuantiles);
        }
        if let Some(i) = (0..quantiles.len())
            .find(|&i| !quantiles[i].is_finite() || (i > 0 && quantiles[i] <= quantiles[i - 1]))
        {
            return Err(BuildKickRankTableError::NotStrictlyIncreasing(i));
        }
        Ok(Self {
            quantiles: Cow::Owned(quantiles),
        })
    }

    /// The quantiles, at ranks i ÷ (n − 1).
    #[must_use]
    pub fn quantiles(&self) -> &[f64] {
        &self.quantiles
    }

    /// `F_x(`score`)`: the share of the reference population scoring below it, in [0, 1], linear
    /// between the quantiles, 0 at or below the first and 1 at or above the last.
    #[must_use]
    pub fn rank(&self, score: f64) -> f64 {
        let q = &self.quantiles;
        let last = q.len() - 1;
        let above = q.partition_point(|&k| k <= score);
        if above == 0 {
            return 0.0;
        }
        if above > last {
            return 1.0;
        }
        let i = above - 1;
        let within = (score - q[i]) / (q[i + 1] - q[i]);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a table's index is at most a few million, exact in an f64"
        )]
        let (i, last) = (i as f64, last as f64);
        (i + within) / last
    }
}

impl Default for KickRankTable {
    fn default() -> Self {
        Self::generator()
    }
}

/// A star's kick draws: the `star.kick.*` fields of its [`StarDraws`], or explicit variates for
/// plan 08's quadrature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KickDraws {
    score: [StandardNormal; REDRAW_TRIES],
    mode: Mark,
    low: [StandardNormal; 3],
    direction: UnitVector,
}

impl KickDraws {
    /// The kick draws of `draws`: `star.kick.score`, `star.kick.mode`, `star.kick.low` and
    /// `star.kick.direction`.
    #[must_use]
    pub const fn of(draws: &StarDraws) -> Self {
        Self {
            score: *draws.kick_score(),
            mode: draws.kick_mode(),
            low: *draws.kick_low(),
            direction: draws.kick_direction(),
        }
    }

    /// Kick draws from explicit variates: the score factor's tries, the low mode's mark, the three
    /// Maxwellian normals, and the ordinary mode's direction.
    #[must_use]
    pub const fn from_parts(
        score: [StandardNormal; REDRAW_TRIES],
        mode: Mark,
        low: [StandardNormal; 3],
        direction: UnitVector,
    ) -> Self {
        Self {
            score,
            mode,
            low,
            direction,
        }
    }

    /// The score factor's tries, in draw order.
    #[must_use]
    pub const fn score(&self) -> &[StandardNormal; REDRAW_TRIES] {
        &self.score
    }

    /// The mark a companion-stripped progenitor's low mode is decided by.
    #[must_use]
    pub const fn mode(&self) -> Mark {
        self.mode
    }

    /// The three normals of a Maxwellian kick (the low mode's, or a white dwarf's).
    #[must_use]
    pub const fn low(&self) -> &[StandardNormal; 3] {
        &self.low
    }

    /// The ordinary mode's direction.
    #[must_use]
    pub const fn direction(&self) -> UnitVector {
        self.direction
    }
}

/// A natal kick law: the kick a remnant is born with, from how it was born, its progenitor, its
/// kind and mass, and its draws.
pub trait KickLaw {
    /// The kick of `remnant`, born by `channel` from `progenitor`, drawn with `draws`.
    #[must_use]
    fn kick(
        &self,
        channel: CollapseChannel,
        progenitor: &ProgenitorAtDeath,
        remnant: &CompactRemnant,
        draws: &KickDraws,
    ) -> NatalKick;
}

/// The generator version's kick law: its parameters and its rank table.
///
/// # Examples
///
/// The ordinary mode orders neutron stars by their progenitors: at the same draws, a heavier core
/// for the same neutron star gives a faster kick.
///
/// ```
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::remnant::{KickDraws, StandardKickLaw};
/// use hyperion_sim::units::SolarMasses;
///
/// let law = StandardKickLaw::default();
/// let draws = KickDraws::of(&StarDraws::median());
/// let ns = SolarMasses::new(1.3);
/// let v = law.ordinary_speed_km_s(SolarMasses::new(1.9), ns, &draws);
/// assert!((33.0..2_300.0).contains(&v));
/// assert!(law.ordinary_speed_km_s(SolarMasses::new(2.6), ns, &draws) > v);
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StandardKickLaw {
    params: KickLawParams,
    table: KickRankTable,
}

impl StandardKickLaw {
    /// The law of `params` over the rank table `table`.
    #[must_use]
    pub const fn new(params: KickLawParams, table: KickRankTable) -> Self {
        Self { params, table }
    }

    /// The law's parameters.
    #[must_use]
    pub const fn params(&self) -> &KickLawParams {
        &self.params
    }

    /// The law's rank table.
    #[must_use]
    pub const fn table(&self) -> &KickRankTable {
        &self.table
    }

    /// The ordinary mode's speed, km/s, of a remnant of `remnant` M☉ from a core of `co_core`:
    /// the score's rank over the table mapped onto the log-normal.
    #[must_use]
    pub fn ordinary_speed_km_s(
        &self,
        co_core: SolarMasses,
        remnant: SolarMasses,
        draws: &KickDraws,
    ) -> f64 {
        let xi = self.params.score_factor(&draws.score);
        let rank = self.table.rank(ordinary_score(co_core, remnant, xi));
        self.params.ordinary_speed_km_s(rank)
    }
}

impl StandardKickLaw {
    /// A single star's death with the provisional companion-stripped mark of plan 06's design note
    /// 11 applied: a collapse (an iron core's, complete fallback's or electron capture's) whose
    /// progenitor's `star.stripped` mark [`is_stripped`](KickLawParams::is_stripped) is
    /// [`Stripping::Companion`]; every other death as it was.
    ///
    /// Besides the kick, only the track reads the mark: a marked star's electron-capture window is
    /// the companion-stripped one, 1 M☉ wide, and its envelope is still a single star's, as design
    /// note 11 has it until plan 11. Every collapse is of a star above the lowest initial mass the
    /// mark is drawn from, `m_cc(Z)` − 1 M☉ (ruling 45.2), since the widest electron-capture window
    /// begins there and iron cores at `m_cc`.
    #[must_use]
    pub fn with_stripped_mark(&self, death: Death, draws: &StarDraws) -> Death {
        let collapse = matches!(
            CollapseChannel::of(death.kind()),
            Some(
                CollapseChannel::IronCore
                    | CollapseChannel::ElectronCapture
                    | CollapseChannel::CompleteFallback
            )
        );
        if !collapse || !self.params.is_stripped(draws.stripped()) {
            return death;
        }
        let p = death.progenitor();
        let progenitor = ProgenitorAtDeath::new(
            p.co_core_mass(),
            p.helium_core_mass(),
            p.envelope_mass(),
            Stripping::Companion,
        );
        Death::new(death.age(), death.kind(), progenitor)
    }

    /// The natal kick of a single star's `remnant` after `death`, from its `draws`: the law's kick
    /// on the death's channel ([`CollapseChannel::of`]), or `None` where nothing is left. `death`
    /// is taken as [`StandardKickLaw::with_stripped_mark`] gives it.
    ///
    /// This is `StarModel`'s remnant stage (P06.T29.a), which reads only the `star.stripped` and
    /// `star.kick.*` draws besides the remnant's, so that plan 08's kick loop can repeat it with a
    /// later attempt's draws on the one built track.
    #[must_use]
    pub fn natal_kick(
        &self,
        death: &Death,
        remnant: &CompactRemnant,
        draws: &StarDraws,
    ) -> Option<NatalKick> {
        let channel = CollapseChannel::of(death.kind())?;
        match remnant.kind() {
            RemnantKind::None => None,
            RemnantKind::WhiteDwarf | RemnantKind::NeutronStar | RemnantKind::BlackHole => {
                Some(self.kick(channel, &death.progenitor(), remnant, &KickDraws::of(draws)))
            }
        }
    }
}

impl KickLaw for StandardKickLaw {
    fn kick(
        &self,
        channel: CollapseChannel,
        progenitor: &ProgenitorAtDeath,
        remnant: &CompactRemnant,
        draws: &KickDraws,
    ) -> NatalKick {
        let p = &self.params;
        match channel {
            CollapseChannel::ElectronCapture | CollapseChannel::AccretionInduced => {
                maxwellian(p.low_sigma_km_s, draws, KickMode::Low)
            }
            CollapseChannel::CompleteFallback => NatalKick::new(
                MetresPerSecond::ZERO,
                draws.direction,
                KickMode::FallbackNone,
            ),
            CollapseChannel::EnvelopeLoss => {
                maxwellian(p.wd_sigma_km_s, draws, KickMode::WhiteDwarf)
            }
            CollapseChannel::IronCore => {
                debug_assert!(
                    matches!(
                        remnant.kind(),
                        RemnantKind::NeutronStar | RemnantKind::BlackHole
                    ),
                    "an iron core leaves a neutron star or a black hole: {remnant:?}"
                );
                let co_core = progenitor.co_core_mass();
                let low = progenitor.stripping() == Stripping::Companion
                    && draws
                        .mode
                        .is_below(Threshold::from_probability(p.low_mode_probability(co_core)));
                if low {
                    return maxwellian(p.low_sigma_km_s, draws, KickMode::Low);
                }
                let factor = match remnant.kind() {
                    RemnantKind::BlackHole => p.bh_factor,
                    RemnantKind::NeutronStar | RemnantKind::WhiteDwarf | RemnantKind::None => 1.0,
                };
                let speed = self.ordinary_speed_km_s(co_core, remnant.mass(), draws) * factor;
                NatalKick::new(
                    MetresPerSecond::from(KilometresPerSecond::new(speed)),
                    draws.direction,
                    KickMode::Ordinary,
                )
            }
        }
    }
}

/// A Maxwellian kick of `sigma_km_s` per component from the three normals of `draws`: the velocity
/// σ (z₁, z₂, z₃), so that its direction is isotropic and its speed σ|z|; along the ordinary
/// direction if all three are zero.
#[must_use]
fn maxwellian(sigma_km_s: f64, draws: &KickDraws, mode: KickMode) -> NatalKick {
    let z = draws.low.map(StandardNormal::value);
    let norm = math::hypot(math::hypot(z[0], z[1]), z[2]);
    let direction = UnitVector::from_components(z).unwrap_or(draws.direction);
    NatalKick::new(
        MetresPerSecond::from(KilometresPerSecond::new(sigma_km_s * norm)),
        direction,
        mode,
    )
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::stellar::draws::StarDrawsParts;

    fn m(v: f64) -> SolarMasses {
        SolarMasses::new(v)
    }

    fn z(v: f64) -> StandardNormal {
        StandardNormal::new(v).unwrap()
    }

    fn progenitor(co: f64, stripping: Stripping) -> ProgenitorAtDeath {
        ProgenitorAtDeath::new(m(co), m(co * 1.3), m(5.0), stripping)
    }

    /// A two-knot test table, `F_x` uniform on scores 0–2 (P06.T19.a's unit tests).
    fn two_knots() -> StandardKickLaw {
        StandardKickLaw::new(
            KickLawParams::default(),
            KickRankTable::new(vec![0.0, 2.0]).unwrap(),
        )
    }

    fn draws_with(score: f64, mode: u64, low: [f64; 3]) -> KickDraws {
        KickDraws::from_parts(
            [z(score); REDRAW_TRIES],
            Mark::from_word(mode),
            low.map(z),
            UnitVector::X,
        )
    }

    fn ns(mass: f64) -> CompactRemnant {
        CompactRemnant::new(RemnantKind::NeutronStar, m(mass))
    }

    #[test]
    fn a_kick_keeps_its_parts() {
        let kick = NatalKick::new(
            MetresPerSecond::new(2.7e5),
            UnitVector::NORTH,
            KickMode::Ordinary,
        );
        assert!((kick.speed().value() - 2.7e5).abs() < 1e-9);
        assert_eq!(kick.direction(), UnitVector::NORTH);
        assert_eq!(kick.mode(), KickMode::Ordinary);
        for (v, want) in kick.velocity().into_iter().zip([0.0, 0.0, 2.7e5]) {
            assert_same_bits(v, want);
        }
    }

    #[test]
    fn the_defaults_are_the_brainstorms_and_the_tracks() {
        let p = KickLawParams::default();
        assert_same_bits(p.low_ramp.0, 2.0);
        assert_same_bits(p.low_ramp.1, 3.0);
        assert!((p.bh_factor - 0.75).abs() < 1e-15);
        assert!(
            (p.ec_window_single - super::super::collapse::SINGLE_STAR_WINDOW.value()).abs() < 1e-15
        );
        assert!(
            (p.ec_window_stripped - super::super::collapse::COMPANION_STRIPPED_WINDOW.value())
                .abs()
                < 1e-15
        );
        // The clamp spans 33–2,210 km/s.
        assert!((p.ordinary_speed_km_s(0.0) - 33.1).abs() < 0.1);
        assert!((p.ordinary_speed_km_s(1.0) - 2_210.0).abs() < 2.0);
        assert!((p.ordinary_speed_km_s(0.5) - math::exp(5.6)).abs() < 1e-9);
    }

    #[test]
    fn the_rank_is_linear_between_quantiles_and_held_outside() {
        let table = KickRankTable::new(vec![0.0, 1.0, 3.0]).unwrap();
        assert_same_bits(table.rank(-0.1), 0.0);
        assert_same_bits(table.rank(0.0), 0.0);
        assert!((table.rank(0.5) - 0.25).abs() < 1e-15);
        assert!((table.rank(1.0) - 0.5).abs() < 1e-15);
        assert!((table.rank(2.0) - 0.75).abs() < 1e-15);
        assert_same_bits(table.rank(3.0), 1.0);
        assert_same_bits(table.rank(9.0), 1.0);
        assert_eq!(
            KickRankTable::new(vec![1.0]),
            Err(BuildKickRankTableError::TooFewQuantiles)
        );
        assert_eq!(
            KickRankTable::new(vec![0.0, 1.0, 1.0]),
            Err(BuildKickRankTableError::NotStrictlyIncreasing(2))
        );
        assert_eq!(
            KickRankTable::new(vec![0.0, f64::NAN]),
            Err(BuildKickRankTableError::NotStrictlyIncreasing(1))
        );
    }

    #[test]
    fn the_generator_table_is_strictly_increasing() {
        let table = KickRankTable::generator();
        assert_eq!(table.quantiles().len(), 257);
        assert!(KickRankTable::new(table.quantiles().to_vec()).is_ok());
    }

    #[test]
    fn the_ordinary_speed_is_the_log_normal_at_the_scores_rank() {
        let law = two_knots();
        let p = law.params();
        // Score (2.1 − 1.4) ÷ 1.4 = 0.5 at ξ = 1: rank 0.25 on the two-knot table.
        let kick = law.kick(
            CollapseChannel::IronCore,
            &progenitor(2.1, Stripping::None),
            &ns(1.4),
            &draws_with(0.0, 0, [1.0, 0.0, 0.0]),
        );
        let expected = math::exp(5.6 + 0.68 * math::normal_quantile(0.25)) * 1e3;
        assert!((kick.speed().value() - expected).abs() < 1e-6 * expected);
        assert_eq!(kick.mode(), KickMode::Ordinary);
        assert_eq!(kick.direction(), UnitVector::X);
        // A higher score factor ranks higher; ranks beyond the clamp are held to it.
        let faster = law.ordinary_speed_km_s(m(2.1), m(1.4), &draws_with(1.0, 0, [0.0; 3]));
        assert!(faster * 1e3 > kick.speed().value());
        let floor = law.ordinary_speed_km_s(m(1.3), m(1.4), &draws_with(0.0, 0, [0.0; 3]));
        assert!((floor - p.ordinary_speed_km_s(0.001)).abs() < 1e-9);
        let ceiling = law.ordinary_speed_km_s(m(9.0), m(1.4), &draws_with(0.0, 0, [0.0; 3]));
        assert!((ceiling - p.ordinary_speed_km_s(0.999)).abs() < 1e-9);
    }

    #[test]
    fn the_score_factor_is_the_first_positive_try() {
        let p = KickLawParams::default();
        let mut tries = [z(-3.0); REDRAW_TRIES];
        assert!(
            (p.score_factor(&tries) - 1.0).abs() < 1e-15,
            "all eight fail"
        );
        tries[5] = z(1.0);
        tries[6] = z(2.0);
        assert!((p.score_factor(&tries) - 1.45).abs() < 1e-15);
        tries[0] = z(-2.0);
        assert!((p.score_factor(&tries) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn black_holes_take_three_quarters_of_the_map_and_none_after_complete_fallback() {
        let law = two_knots();
        let draws = draws_with(0.0, 0, [1.0, 2.0, 2.0]);
        let bh = CompactRemnant::new(RemnantKind::BlackHole, m(4.0));
        let kick = law.kick(
            CollapseChannel::IronCore,
            &progenitor(5.0, Stripping::None),
            &bh,
            &draws,
        );
        let map = law.ordinary_speed_km_s(m(5.0), m(4.0), &draws);
        assert!((kick.speed().value() - 750.0 * map).abs() < 1e-6 * kick.speed().value());
        assert_eq!(kick.mode(), KickMode::Ordinary);
        let kick = law.kick(
            CollapseChannel::CompleteFallback,
            &progenitor(9.0, Stripping::None),
            &CompactRemnant::new(RemnantKind::BlackHole, m(12.0)),
            &draws,
        );
        assert_eq!(kick.speed(), MetresPerSecond::ZERO);
        assert_eq!(kick.mode(), KickMode::FallbackNone);
    }

    #[test]
    fn electron_capture_and_white_dwarfs_take_their_maxwellians() {
        let law = two_knots();
        // |z| = 3, so the low mode's speed is 15 km/s and a white dwarf's 3 km/s, along z.
        let draws = draws_with(0.0, 0, [1.0, 2.0, -2.0]);
        let p = progenitor(1.3, Stripping::None);
        for channel in [
            CollapseChannel::ElectronCapture,
            CollapseChannel::AccretionInduced,
        ] {
            let kick = law.kick(channel, &p, &ns(1.26), &draws);
            assert_eq!(kick.mode(), KickMode::Low);
            assert!((kick.speed().value() - 15e3).abs() < 1e-9);
            let d = kick.direction().components();
            assert!((d[0] - 1.0 / 3.0).abs() < 1e-12 && (d[2] + 2.0 / 3.0).abs() < 1e-12);
        }
        let wd = CompactRemnant::new(RemnantKind::WhiteDwarf, m(0.6));
        let kick = law.kick(CollapseChannel::EnvelopeLoss, &p, &wd, &draws);
        assert_eq!(kick.mode(), KickMode::WhiteDwarf);
        assert!((kick.speed().value() - 3e3).abs() < 1e-9);
        // All three normals zero: no speed, along the ordinary direction.
        let kick = law.kick(
            CollapseChannel::EnvelopeLoss,
            &p,
            &wd,
            &draws_with(0.0, 0, [0.0; 3]),
        );
        assert_eq!(kick.speed(), MetresPerSecond::ZERO);
        assert_eq!(kick.direction(), UnitVector::X);
    }

    #[test]
    fn a_companion_stripped_progenitor_takes_the_low_mode_on_the_ramp() {
        let law = two_knots();
        let p = law.params();
        assert!((p.low_mode_probability(m(1.5)) - 1.0).abs() < 1e-15);
        assert!((p.low_mode_probability(m(2.25)) - 0.75).abs() < 1e-15);
        assert!(p.low_mode_probability(m(3.5)).abs() < 1e-15);
        let low_mark = 1_u64 << 62; // a uniform of 1/4
        let high_mark = 7_u64 << 61; // 7/8
        let mode_of = |co: f64, stripping, mark| {
            law.kick(
                CollapseChannel::IronCore,
                &progenitor(co, stripping),
                &ns(1.4),
                &draws_with(0.0, mark, [1.0, 1.0, 1.0]),
            )
            .mode()
        };
        assert_eq!(mode_of(1.9, Stripping::Companion, high_mark), KickMode::Low);
        assert_eq!(mode_of(2.5, Stripping::Companion, low_mark), KickMode::Low);
        assert_eq!(
            mode_of(2.5, Stripping::Companion, high_mark),
            KickMode::Ordinary
        );
        assert_eq!(
            mode_of(3.2, Stripping::Companion, 0),
            KickMode::Ordinary,
            "above the ramp every mark is ordinary"
        );
        for stripping in [Stripping::None, Stripping::Wind] {
            assert_eq!(mode_of(1.9, stripping, 0), KickMode::Ordinary);
        }
    }

    #[test]
    fn the_stripped_mark_is_set_for_a_quarter_of_marks() {
        let p = KickLawParams::default();
        assert!(p.is_stripped(Mark::from_word((1 << 62) - (1 << 11))));
        assert!(!p.is_stripped(Mark::from_word(1 << 62)));
    }

    #[test]
    fn the_draws_are_the_stars_kick_fields() {
        let parts = StarDrawsParts {
            kick_mode: Mark::from_word(12_345 << 11),
            kick_direction: UnitVector::Y,
            kick_low: [z(0.5), z(-1.0), z(2.0)],
            kick_score: [z(0.25); REDRAW_TRIES],
            ..StarDrawsParts::MEDIAN
        };
        let draws = KickDraws::of(&StarDraws::from_parts(parts.clone()));
        assert_eq!(
            draws,
            KickDraws::from_parts(
                parts.kick_score,
                parts.kick_mode,
                parts.kick_low,
                parts.kick_direction
            )
        );
        assert_eq!(draws.mode(), parts.kick_mode);
        assert_eq!(draws.direction(), UnitVector::Y);
        assert_eq!(draws.low(), &parts.kick_low);
        assert_eq!(draws.score(), &parts.kick_score);
    }

    #[test]
    fn every_death_has_its_channel() {
        use super::super::SupernovaType;
        assert_eq!(
            CollapseChannel::of(DeathKind::CoreCollapse {
                supernova: SupernovaType::IIP
            }),
            Some(CollapseChannel::IronCore)
        );
        assert_eq!(
            CollapseChannel::of(DeathKind::ElectronCapture),
            Some(CollapseChannel::ElectronCapture)
        );
        assert_eq!(
            CollapseChannel::of(DeathKind::DirectCollapse),
            Some(CollapseChannel::CompleteFallback)
        );
        assert_eq!(
            CollapseChannel::of(DeathKind::EnvelopeLoss),
            Some(CollapseChannel::EnvelopeLoss)
        );
        assert_eq!(CollapseChannel::of(DeathKind::PairInstability), None);
        assert_eq!(
            CollapseChannel::of(DeathKind::ThermonuclearDisruption),
            None
        );
    }
}
