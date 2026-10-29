//! Magnetar bursts and giant flares: the outbursts of neutron stars whose decaying fields power
//! them, the third single-star event kind (plan 06, P06.T28.c; rates ruled in
//! decision-galaxy-25-train §5.1, 2026-10-27).
//!
//! # Who bursts
//!
//! A star whose neutron star ([`StarModel::neutron_star`], P06.T21) is a magnetar: every rate here
//! is zero unless [`PulsarState::is_magnetar`] holds at t (a field above the quantum critical
//! field, 4.4 × 10¹³ G, whose decay outshines the spin-down), and so zero once the field sits at
//! its floor of 10¹² G. A star whose neutron star is born at or below the critical field never is
//! one and has no model; living stars, white dwarfs, black holes and stars with no neutron star by
//! the end of the clock window have none either.
//!
//! # The common scaling
//!
//! An outburst releases crustal stress that the decaying field builds; its energy scales with B²
//! and the field releases energy at `L_B` ∝ B |Ḃ|, so the event rate per magnetar scales as
//! |Ḃ| ÷ B. Under T21's law, B(t) = B₀ ÷ (1 + t ÷ τ) with B₀ τ = 10¹⁹ G yr for every star
//! (Colpi, Geppert and Page 2000, ApJ 529, L29, α = 1; ruling 110.4), that is
//!
//! ```text
//! |d ln B ÷ dt| = 1 ÷ (τ + t) = B(t) ÷ (10¹⁹ G yr),
//! ```
//!
//! which falls with the field and is higher for the stronger fields. Each rate is a number of
//! events per e-fold of the field times it ([`field_e_folds_per_year`]). The field is read at the
//! neutron star's age at t, the star's age then less its age at death, in the arithmetic of
//! [`StarModel::remnant_age_at`]; the field only falls, so a bin's bound is the rate at its first
//! instant after the birth, and zero for a bin that ends before the birth or starts with the field
//! at or below the critical field, after which the star is never a magnetar again.
//!
//! # Episodes
//!
//! Outburst episodes (series `star.ev.magnetar_episode`) are Poisson bins of
//! [`EPISODE_BIN_SECONDS`] = 30 days at r = [`EPISODES_PER_E_FOLD`] × B(t) ÷ (10¹⁹ G yr), 2,000
//! episodes per e-fold of the field (the ruling's): 0.2 a year at 10¹⁵ G, 0.02 at 10¹⁴ G and 0.009
//! at 4.4 × 10¹³ G, and 0.05 at 2.5 × 10¹⁴ G, against Coti Zelati et al.'s (2018, MNRAS 474, 961)
//! 23 outbursts of 17 magnetars over 1998–2016 (their Table 2), about 0.05 per magnetar-year (23
//! over 19 years among the 26 sources of Olausen and Kaspi's catalogue, whose online version now
//! lists 30 magnetars and candidates). An episode's marks, word 0 and word 1 of its stream, are
//! its expected burst count `N_b`, log-uniform on [3, 1,000] ([`BURST_COUNT`]), and its e-folding
//! `τ_b`, log-uniform on 0.3–10 days ([`BURST_E_FOLDING_DAYS`]); it lasts 10 `τ_b`
//! ([`EPISODE_SPAN_E_FOLDINGS`]), at most 100 days, and [`EPISODE_LOOK_BACK_BINS`] = 4 bins of
//! look-back cover it.
//!
//! # Short bursts
//!
//! Short bursts (series `star.ev.magnetar_burst`) are Poisson bins of [`BURST_BIN_SECONDS`] = 15
//! minutes at the rate Σ over the star's episodes of (`N_b` ÷ `τ_b`) e^(−Δt ÷ `τ_b`), Δt the time
//! since each episode's onset, from 0 to 10 `τ_b`. The ruling's bins are an hour; at `N_b` ÷ `τ_b`
//! up to 1,000 ÷ 0.3 days an hour holds up to 139 expected bursts, above the 64 the construction
//! allows a bin ([`PoissonBins::MAX_MEAN_PER_BIN`]) for 1.5% of episodes, and a quarter of an hour
//! holds at most 34.7 for one episode (two overlapping near their onsets can pass 64, some 10⁻¹⁰
//! of episodes, which a debug build asserts). Each listing regenerates the episodes that can reach its bins from their own
//! series, which is pure: no event list is ever an input. A bin's bound is the sum, over the
//! episodes that overlap it, of each one's rate at the bin's start or at its onset, whichever is
//! later. Marks: word 0 the episode the burst belongs to, picked in proportion to the episodes'
//! rates at its time; word 1 its energy, dN/dE ∝ E^(−5/3) on 10³⁷–10⁴¹ erg ([`BURST_ENERGY_JOULES`],
//! Göğüş et al. 1999, ApJ 526, L93, and 2000, ApJ 532, L121; Cheng et al. 1996, Nature 382,
//! 518); word 2 its duration, log-normal about 0.1 s with σ 0.35 dex (Göğüş et al. 2001, ApJ 558,
//! 228), truncated at ±4σ.
//!
//! # Giant flares
//!
//! Giant flares (series `star.ev.magnetar_giant`) are Poisson bins of [`GIANT_FLARE_BIN_SECONDS`] =
//! one Julian year at r = [`GIANT_FLARES_PER_E_FOLD`] × B(t) ÷ (10¹⁹ G yr), the constant derived
//! from T21's own Milky Way census of active magnetars, 84, so that the Galaxy's rate is 0.044 a
//! year ([`GALACTIC_GIANT_FLARES_PER_YEAR`]): two Galactic giant flares, SGR 1900+14 on 1998-08-27
//! and SGR 1806−20 on 2004-12-27, in about 45 years of all-sky coverage since 1979 (SGR 0526−66's
//! of 1979 was the Large Magellanic Cloud's). The census gives 67.29 per e-fold, not the ruling's
//! expected 30, so a 10¹⁵ G magnetar flares once in about 150 years.
//! Mark: word 0 its energy, dN/dE ∝ E^(−1.7) on 10⁴⁴–10⁴⁷ erg ([`GIANT_FLARE_ENERGY_JOULES`]). A
//! giant flare's light here is its initial spike alone, [`GIANT_FLARE_SPIKE`]; the pulsating tail
//! that follows is not modelled.
//!
//! Every figure the ruling cited from memory was read at source on 2026-10-29 (arXiv copies, and
//! Olausen and Kaspi's online catalogue as the Internet Archive kept it), and each constant's documentation names
//! the section or table it was read in. One moved: the bursts' duration width, 0.35 dex, not the
//! ruling's 0.3 ([`BURST_DURATION_SIGMA_DEX`]).
//!
//! An event may change what a console reads, never what the star's track is (design note 14):
//! nothing here changes the star or its neutron star.

use crate::events::{
    BinEvent, EventBin, EventId, EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow,
    from_nanos_saturating, tags, to_nanos,
};
use crate::id::BodyId;
use crate::math;
use crate::rng::Seed;
use crate::stellar::remnant::neutron_star::{DECAY_TIME_AT_1E15_G, QUANTUM_CRITICAL_FIELD};
use crate::stellar::remnant::{NeutronStar, PulsarState};
use crate::stellar::system::StarModel;
use crate::time::{Span, UniverseTime};
use crate::units::consts::{SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR};
use crate::units::{Gauss, Years};

/// Seconds in one Julian year, as an integer.
const SECONDS_PER_JULIAN_YEAR_I64: i64 = 31_557_600;

/// Joules per erg.
const JOULES_PER_ERG: f64 = 1e-7;

/// The length of an episode bin, s: 30 days (the ruling's).
pub const EPISODE_BIN_SECONDS: i64 = 2_592_000;

/// How many whole bins [`episodes_active_at`] looks back: four, 120 days, at least the longest
/// episode's 100 days ([`MAX_EPISODE_SPAN`]).
pub const EPISODE_LOOK_BACK_BINS: u32 = 4;

/// The length of a short-burst bin, s: 15 minutes, a quarter of the ruling's hour, so that a bin's
/// mean stays under 64 (see "Short bursts" in the [module documentation](self)).
pub const BURST_BIN_SECONDS: i64 = 900;

/// How many whole bins [`bursts_active_at`] looks back: one, 15 minutes, far longer than any burst
/// ([`MAX_BURST_DURATION`]).
pub const BURST_LOOK_BACK_BINS: u32 = 1;

/// The length of a giant-flare bin, s: one Julian year (the ruling's).
pub const GIANT_FLARE_BIN_SECONDS: i64 = SECONDS_PER_JULIAN_YEAR_I64;

/// How many whole bins [`giant_flares_active_at`] looks back: one.
pub const GIANT_FLARE_LOOK_BACK_BINS: u32 = 1;

/// B₀ τ, the product of every neutron star's birth field and decay time, G yr: 10¹⁹ (P06.T21.b;
/// ruling 110.4), so that |d ln B ÷ dt| = B(t) ÷ (10¹⁹ G yr) while the field decays.
pub const FIELD_DECAY_PRODUCT_GAUSS_YEARS: f64 = DECAY_TIME_AT_1E15_G.value() * 1e15;

/// The episodes of a magnetar per e-fold of its field: 2,000 (the ruling's own figure,
/// decision-galaxy-25-train §5.1).
///
/// The check, from the ruling: Coti Zelati et al. (2018, MNRAS 474, 961, §1 and Table 2) catalogue
/// 23 outbursts of 17 magnetars over 1998–2016, about 0.05 per magnetar-year (derived: 23 over 19
/// years among the 26 sources of Olausen and Kaspi's catalogue, whose online version now lists 30
/// magnetars and candidates), whose dipole fields lie mostly between 10¹⁴ and 10¹⁵ G (Olausen and
/// Kaspi 2014, ApJS 212, 6, Table 2: 16 of their 21 confirmed; the online catalogue, 18 of 24); the
/// law gives 0.05 a year at 2.5 × 10¹⁴ G. Both read at source 2026-10-29 (arXiv copies; the online
/// catalogue as the Internet Archive kept it on 2026-10-22).
///
/// Recorded, not tested (the ruling's): an episode then releases about R³B² ÷ (3 `N_EP`), 4 × 10⁴³
/// erg at 5 × 10¹⁴ G with R = 10 km, above X-ray outbursts' 10⁴⁰–10⁴³ erg (Coti Zelati et al.'s
/// Table 2: 6 × 10⁴⁰ to 2.4 × 10⁴³ erg), a radiative efficiency of 10⁻⁴–10⁻¹.
pub const EPISODES_PER_E_FOLD: f64 = 2_000.0;

/// The Galaxy's rate of giant flares that the constant [`GIANT_FLARES_PER_E_FOLD`] is set to, per
/// Julian year: 0.044, two in about 45 years.
///
/// The two Galactic giant flares, SGR 1900+14 on 1998-08-27 (Hurley et al. 1999, Nature 397, 41)
/// and SGR 1806−20 on 2004-12-27 (Hurley et al. 2005, Nature 434, 1098; Palmer et al. 2005, Nature
/// 434, 1107), in the some 45 years of all-sky coverage since 1979; SGR 0526−66's flare of
/// 1979-03-05 (Mazets et al. 1979, Nature 282, 587) was in the Large Magellanic Cloud and is not
/// counted. The dates read at source 2026-10-29 (arXiv copies): Hurley et al. 1999, "August 27,
/// 1998"; Hurley et al. 2005 and Palmer et al. 2005, 27 December 2004; Olausen and Kaspi 2014,
/// "1979 March 5, from … the Large Magellanic Cloud" (Mazets et al.'s own text was not reached).
/// No other Galactic giant flare: "only three have been seen … in the Milky Way and in the Large
/// Magellanic Cloud" (Mereghetti et al. 2024, arXiv:2312.14645), "only 3 such events have been
/// observed" (Trigg et al. 2025, A&A 710, A139). The 45 years are a choice: Mereghetti et al. say
/// "∼50 years" and Trigg et al. "the last 57 years", or 0.035–0.044 a year, inside two events'
/// Poisson error.
pub const GALACTIC_GIANT_FLARES_PER_YEAR: f64 = 0.044;

/// The giant flares of a magnetar per e-fold of its field: 67.29, derived from T21's census.
///
/// At T21's two core collapses a century, 62% of them leaving a neutron star (the census of
/// P06.T21's `magnetars_are_the_high_tail_of_the_birth_field`, 83.7 active magnetars), the
/// Galaxy's magnetars decay by 6.539 × 10⁻⁴ e-folds of their fields a year in all, so 0.044 ÷
/// 6.539 × 10⁻⁴ = 67.29 giant flares per e-fold give the Galaxy 0.044 a year (a unit test derives it
/// again from the same sample). The ruling expected about 30 from a mean decay of 1.5 × 10⁻⁵
/// e-folds a year per magnetar; the census's magnetars decay at 7.8 × 10⁻⁶, most of them past their
/// strongest fields. A 10¹⁵ G magnetar flares once in about 150 years.
pub const GIANT_FLARES_PER_E_FOLD: f64 = 67.29;

/// The range of an episode's expected burst count `N_b`, log-uniform: 3–1,000, median 55 and mean
/// 172 (the ruling's own).
///
/// The ruling's checks: over 400 bursts in a day from SGR J1550−5418 in January 2009 (Kaneko et
/// al. 2010, ApJ 710, 1335; van der Horst et al. 2012, ApJ 749, 122), hundreds from SGR 1935+2154
/// in April 2020 (Younes et al. 2020, ApJL 904, L21) and about 440 bursts of 8 magnetars in 5 years
/// of Fermi GBM (Collazzi et al. 2015, ApJS 218, 11). Read at source 2026-10-29 (arXiv copies):
/// Kaneko et al. §1, "a total of ∼450 bursts during this 24 hour period" from 2009 January 22 (van
/// der Horst et al. merge their 555 untriggered events of that day into 286 bursts over January
/// 22–29); Younes et al.'s abstract, "over 217 bursts" in the first 1,120 s of 2020-04-28; and
/// Collazzi et al.'s abstract, 440 bursts from July 2008 to June 2013 (their Table 1: 427 from 8
/// magnetars, 19 of unknown origin).
pub const BURST_COUNT: (f64, f64) = (3.0, 1_000.0);

/// The range of an episode's burst e-folding time `τ_b`, days, log-uniform: 0.3–10 (the ruling's
/// own).
pub const BURST_E_FOLDING_DAYS: (f64, f64) = (0.3, 10.0);

/// How many e-foldings an episode's bursts run for: 10, after which its rate is e^(−10) of its
/// first and is cut to zero, so that every burst falls within 10 `τ_b` of its episode's onset.
pub const EPISODE_SPAN_E_FOLDINGS: f64 = 10.0;

/// The longest an episode lasts: 10 × 10 days, 100 days.
pub const MAX_EPISODE_SPAN: Span = Span::from_seconds(100 * 86_400);

/// The range of a short burst's energy, J, under dN/dE ∝ E^(−5/3): 10³⁷–10⁴¹ erg.
///
/// The index is the bursts' of SGR 1900+14 and SGR 1806−20, read at source 2026-10-29 (arXiv
/// copies): Göğüş et al. 1999 (ApJ 526, L93, §4), a differential γ = 1.66 (+0.13/−0.12) from BATSE
/// and 1.66 ± 0.05 from RXTE "over 4 orders of magnitude", and Göğüş et al. 2000 (ApJ 532, L121),
/// 1.67–1.76 from BATSE and ICE (1.43 ± 0.06 from RXTE). Cheng et al. (1996, Nature 382, 518)
/// liken the law to earthquakes'; their index was not read (Göğüş et al. 1999 quote their γ =
/// 1.6). The range is the ruling's, inside the observed 7 × 10³⁵–1.6 × 10⁴¹ erg (BATSE, RXTE and
/// ICE, the same papers); its floor is a choice.
pub const BURST_ENERGY_JOULES: (f64, f64) = (1e37 * JOULES_PER_ERG, 1e41 * JOULES_PER_ERG);

/// The index of the short bursts' energy law: dN/dE ∝ E^(−5/3).
pub const BURST_ENERGY_INDEX: f64 = 5.0 / 3.0;

/// The median of a short burst's duration, s: 0.1 (Göğüş et al. 2001, ApJ 558, 228, read at source
/// 2026-10-29, arXiv copy: §3.1's log-Gaussians in T90 peak at 93.9 ± 0.2 ms for SGR 1900+14's 455
/// bursts and 161.8 ± 0.2 ms for SGR 1806−20's 190, and the abstract has "a peak at ∼0.1 s").
pub const BURST_DURATION_MEDIAN_SECONDS: f64 = 0.1;

/// The standard deviation of log₁₀ of a short burst's duration: 0.35 dex.
///
/// Göğüş et al. 2001, ApJ 558, 228, §3.1: a log-Gaussian in T90, 0.35 ± 0.01 decades wide (SGR
/// 1900+14, peak 93.9 ms) and 0.34 ± 0.02 (SGR 1806−20, 161.8 ms); read at source 2026-10-29
/// (arXiv copy), which moved it from the ruling's 0.3. Later Fermi samples of SGR J1550−5418 are
/// wider, 0.41 ± 0.02 (van der Horst et al. 2012) and 0.40 ± 0.02 (Collazzi et al. 2015).
pub const BURST_DURATION_SIGMA_DEX: f64 = 0.35;

/// Where the log-normal of a burst's duration is cut, in standard deviations: ±4, so 3.98 ms to
/// 2.512 s (this module's choice, as P06.T28.b cuts its large glitches' sizes).
pub const BURST_DURATION_TRUNCATION: f64 = 4.0;

/// A bound on a short burst's duration: 3 s, above the duration law's top of 0.1 s × 10^1.4 =
/// 2.512 s and far under the 15-minute look-back.
pub const MAX_BURST_DURATION: Span = Span::from_seconds(3);

/// The range of a giant flare's energy, J, under dN/dE ∝ E^(−1.7): 10⁴⁴–10⁴⁷ erg.
///
/// The range is the plan's, consistent with Burns et al.'s (2021, ApJL 907, L28) search over
/// 1.0 × 10⁴⁴–5.75 × 10⁴⁷ erg, whose largest observed flare is 2.3 × 10⁴⁶ erg. The index is their
/// fit's, α = 1.7 ± 0.4 (90%), a differential index (their PDF (1 − α) E^(−α) ÷
/// (`E_max`^(1−α) − `E_min`^(1−α))). They put the volumetric rate of giant flares above 4 × 10⁴⁴
/// erg at 3.8 (+4.0 −3.1) × 10⁵ Gpc⁻³ yr⁻¹ (their abstract; `E_iso,min` = 3.7 × 10⁴⁴ erg, §4.1),
/// the ruling's cross-check, recorded. Read at source 2026-10-29 (arXiv copy).
pub const GIANT_FLARE_ENERGY_JOULES: (f64, f64) = (1e44 * JOULES_PER_ERG, 1e47 * JOULES_PER_ERG);

/// The index of the giant flares' energy law: dN/dE ∝ E^(−1.7).
pub const GIANT_FLARE_ENERGY_INDEX: f64 = 1.7;

/// The light of a giant flare, its initial spike: 0.2 s.
///
/// SGR 1806−20's spike lasted ~0.2 s (Hurley et al. 2005, Nature 434, 1098; ~0.5 s in Swift-BAT,
/// Palmer et al. 2005, Nature 434, 1107), SGR 1900+14's about 1 s (Hurley et al. 1999, Nature 397,
/// 41, Table 1), before pulsating tails of 380 s and ~300 s; read at source 2026-10-29 (arXiv
/// copies). The tail is not modelled.
pub const GIANT_FLARE_SPIKE: Span = match Span::new(0, 200_000_000) {
    Ok(span) => span,
    Err(_) => panic!("0.2 s is a span"),
};

/// The relative padding of a bin's bound over the rate at its first instant: 2⁻⁴⁰, far above the
/// few units in the last place by which rounding can lift a rate within a bin, as P06.T28.d pads
/// its bound.
const BOUND_PADDING: f64 = 1.0 / 1_099_511_627_776.0;

/// e-folds of the field per year at a field of `field`: B ÷ (10¹⁹ G yr), the field's fractional
/// decay rate |d ln B ÷ dt| while it decays above its floor (see "The common scaling" in the
/// [module documentation](self)).
#[must_use]
pub fn field_e_folds_per_year(field: Gauss) -> f64 {
    field.value() / FIELD_DECAY_PRODUCT_GAUSS_YEARS
}

/// The three series of one star's magnetar events, its episodes', short bursts' and giant
/// flares', opened together from the seed and the star's body so that they cannot come from two
/// universes: the bursts are thinned by the episodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MagnetarSeries {
    episodes: EventSeries,
    bursts: EventSeries,
    giant_flares: EventSeries,
}

impl MagnetarSeries {
    /// The series of the star of `body` in the universe of `seed`: `star.ev.magnetar_episode`,
    /// `star.ev.magnetar_burst` and `star.ev.magnetar_giant`.
    #[must_use]
    pub fn new(seed: Seed, body: BodyId) -> Self {
        Self {
            episodes: EventSeries::new(seed, tags::STAR_MAGNETAR_EPISODE, body.into()),
            bursts: EventSeries::new(seed, tags::STAR_MAGNETAR_BURST, body.into()),
            giant_flares: EventSeries::new(seed, tags::STAR_MAGNETAR_GIANT, body.into()),
        }
    }

    /// The episodes' series.
    #[must_use]
    pub const fn episodes(&self) -> &EventSeries {
        &self.episodes
    }

    /// The short bursts' series.
    #[must_use]
    pub const fn bursts(&self) -> &EventSeries {
        &self.bursts
    }

    /// The giant flares' series.
    #[must_use]
    pub const fn giant_flares(&self) -> &EventSeries {
        &self.giant_flares
    }
}

/// The clock time, as nanoseconds since the epoch, at which a star whose age at the epoch is
/// `age_at_epoch_years` is `age_years` old: the epoch plus (age − age at the epoch) × 31,557,600
/// s, floored to the nanosecond, held to the clock's ends.
#[must_use]
fn clock_nanos_of_age(age_at_epoch_years: f64, age_years: f64) -> i128 {
    let seconds = (age_years - age_at_epoch_years) * SECONDS_PER_JULIAN_YEAR;
    match Span::from_seconds_f64(seconds).and_then(|span| UniverseTime::EPOCH.checked_add(span)) {
        Some(t) => to_nanos(t),
        None if seconds < 0.0 => to_nanos(from_nanos_saturating(i128::MIN)),
        None => to_nanos(from_nanos_saturating(i128::MAX)),
    }
}

/// The magnetar events of one star: its neutron star, its death and the span in which it can be a
/// magnetar (plan 06, P06.T28.c; see the [module documentation](self)).
///
/// It gives the rates of the star's episodes and giant flares, and regenerates the episodes that
/// thin its short bursts. Its methods list from a model already built; the free functions build
/// one per call.
///
/// # Examples
///
/// A 15 M☉ star whose neutron star is born with a field of 10¹⁵ G and a period of 0.2 s, 2,000
/// years before the epoch: a magnetar then, at 8.3 × 10¹⁴ G, with about 0.17 episodes a year and
/// one giant flare in some 180 years.
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
/// use hyperion_sim::stellar::events::magnetar::MagnetarModel;
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let z = |value: f64| StandardNormal::new(value).ok_or("a finite draw");
/// // log₁₀ B₀ = 12.95 + 0.6 z = 15 and P₀ = 0.25 + 0.1 z = 0.2 s.
/// let draws = StarDraws::from_parts(StarDrawsParts {
///     ns_field: z(2.05 / 0.6)?,
///     ns_spin: [z(-0.5)?; 8],
///     ..StarDrawsParts::MEDIAN
/// });
/// let (mass, solar) = (SolarMasses::new(15.0), Composition::SOLAR);
/// let lifetime = StarModel::new(mass, solar, draws.clone(), Years::new(1e9))?
///     .lifetime()
///     .ok_or("a 15 M☉ star dies")?;
/// let star = StarModel::new(mass, solar, draws, Years::new(lifetime.value() + 2e3))?;
/// let model = MagnetarModel::of(&star).ok_or("born above the critical field")?;
/// assert!(model.is_magnetar_at(UniverseTime::EPOCH));
/// let per_year = |rate: f64| rate * 31_557_600.0;
/// assert!((0.1..0.25).contains(&per_year(model.episode_rate_at(UniverseTime::EPOCH).value())));
/// assert!((4e-3..7e-3).contains(&per_year(model.giant_flare_rate_at(UniverseTime::EPOCH).value())));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MagnetarModel<'a> {
    star: &'a StarModel,
    neutron_star: NeutronStar,
    /// The star's age at its death.
    death_age: Years,
    /// The neutron star's birth, nanoseconds since the epoch.
    birth_nanos: i128,
    /// The first instant at which its field is at or below the critical field, nanoseconds since
    /// the epoch: from it the star is never a magnetar.
    quiet_nanos: i128,
}

/// The margin by which listings are cut to the span a star can be a magnetar in, beyond its birth
/// and its field's reaching the critical field, ns: a day, far above the rounding of the ages that
/// place them, so that the cut drops nothing the bounds allow.
const CUT_MARGIN_NANOS: i128 = 86_400 * 1_000_000_000;

impl<'a> MagnetarModel<'a> {
    /// The magnetar events of `star`, or `None` for a star whose neutron star, if it leaves one by
    /// the end of the clock window, is born at or below the critical field and so is never a
    /// magnetar.
    #[must_use]
    pub fn of(star: &'a StarModel) -> Option<Self> {
        let neutron_star = star.neutron_star()?;
        let b0 = neutron_star.birth_field().value();
        if b0 <= QUANTUM_CRITICAL_FIELD.value() {
            return None;
        }
        let death_age = star.death()?.age();
        // τ (B₀ ÷ B_Q − 1) after the birth the field reaches the critical field.
        let tau = FIELD_DECAY_PRODUCT_GAUSS_YEARS / b0;
        let critical_age = tau * (b0 / QUANTUM_CRITICAL_FIELD.value() - 1.0);
        let epoch_age = star.age_at_epoch().value();
        Some(Self {
            star,
            neutron_star,
            death_age,
            birth_nanos: clock_nanos_of_age(epoch_age, death_age.value()),
            quiet_nanos: clock_nanos_of_age(epoch_age, death_age.value() + critical_age),
        })
    }

    /// The star.
    #[must_use]
    pub const fn star(&self) -> &'a StarModel {
        self.star
    }

    /// The star's neutron star.
    #[must_use]
    pub const fn neutron_star(&self) -> NeutronStar {
        self.neutron_star
    }

    /// The neutron star's birth, as a clock time.
    #[must_use]
    pub fn birth(&self) -> UniverseTime {
        from_nanos_saturating(self.birth_nanos)
    }

    /// The first instant at which the field has decayed to the critical field, as a clock time:
    /// from it the star is never a magnetar.
    #[must_use]
    pub fn quiet_from(&self) -> UniverseTime {
        from_nanos_saturating(self.quiet_nanos)
    }

    /// The neutron star's age at `t`, Julian years, negative before its birth: the star's age then
    /// less its age at death, as [`StarModel::remnant_age_at`] forms it.
    #[must_use]
    pub fn neutron_star_age_at(&self, t: UniverseTime) -> Years {
        Years::new(self.star.age_at(t).value() - self.death_age.value())
    }

    /// The pulsar at `t`, or `None` before the neutron star's birth.
    #[must_use]
    pub fn pulsar_at(&self, t: UniverseTime) -> Option<PulsarState> {
        let age = self.neutron_star_age_at(t);
        (age.value() >= 0.0).then(|| self.neutron_star.state_at(age))
    }

    /// Whether the star is a magnetar at `t` ([`PulsarState::is_magnetar`]).
    #[must_use]
    pub fn is_magnetar_at(&self, t: UniverseTime) -> bool {
        self.pulsar_at(t).is_some_and(|pulsar| pulsar.is_magnetar())
    }

    /// e-folds of the field per year at `t` where the star is a magnetar, else 0.
    #[must_use]
    fn e_folds_per_year_at(&self, t: UniverseTime) -> f64 {
        self.pulsar_at(t)
            .filter(PulsarState::is_magnetar)
            .map_or(0.0, |pulsar| field_e_folds_per_year(pulsar.field()))
    }

    /// The largest e-folds of the field per year over `bin` that a magnetar's rate can reach: at
    /// its first instant after the birth, padded by 2⁻⁴⁰ of itself; zero for a bin that ends
    /// before the birth or starts with the field at or below the critical field. The ages are the
    /// rate's own, [`neutron_star_age_at`](Self::neutron_star_age_at), which never falls as t
    /// grows, so the field at the bin's first instant bounds it everywhere in the bin.
    #[must_use]
    fn e_folds_bound(&self, bin: TimeWindow) -> f64 {
        let last = from_nanos_saturating(to_nanos(bin.end()) - 1);
        if self.neutron_star_age_at(last).value() < 0.0 {
            return 0.0;
        }
        let age = self.neutron_star_age_at(bin.start()).value().max(0.0);
        let field = self.neutron_star.field_at(Years::new(age));
        if field <= QUANTUM_CRITICAL_FIELD {
            return 0.0;
        }
        let bound = field_e_folds_per_year(field);
        bound + bound * BOUND_PADDING
    }

    /// The rate of episodes at `t`: [`EPISODES_PER_E_FOLD`] × B(t) ÷ (10¹⁹ G yr) where the star is
    /// a magnetar, else 0.
    #[must_use]
    pub fn episode_rate_at(&self, t: UniverseTime) -> EventsPerSecond {
        EventsPerSecond::per_julian_year(EPISODES_PER_E_FOLD * self.e_folds_per_year_at(t))
    }

    /// The rate of giant flares at `t`: [`GIANT_FLARES_PER_E_FOLD`] × B(t) ÷ (10¹⁹ G yr) where the
    /// star is a magnetar, else 0.
    #[must_use]
    pub fn giant_flare_rate_at(&self, t: UniverseTime) -> EventsPerSecond {
        EventsPerSecond::per_julian_year(GIANT_FLARES_PER_E_FOLD * self.e_folds_per_year_at(t))
    }

    /// `window` cut to the span in which the star can be a magnetar, from the birth to the field's
    /// reaching the critical field, each widened by a day, or `None` where they do not meet:
    /// outside it every bound is zero, so the cut drops nothing and spares the empty bins.
    #[must_use]
    fn active_part(&self, window: TimeWindow) -> Option<TimeWindow> {
        let start = window.start().max(from_nanos_saturating(
            self.birth_nanos.saturating_sub(CUT_MARGIN_NANOS),
        ));
        let end = window.end().min(from_nanos_saturating(
            self.quiet_nanos.saturating_add(CUT_MARGIN_NANOS),
        ));
        (start < end).then(|| TimeWindow::new(start, end).expect("start before end"))
    }

    /// Appends to `out` the episodes with onsets in `window`, sorted by onset and then by ID.
    pub fn episodes_in(&self, series: &MagnetarSeries, window: TimeWindow, out: &mut Vec<Episode>) {
        let Some(window) = self.active_part(window) else {
            return;
        };
        let mut events = Vec::new();
        episode_bins().events_in(&series.episodes, window, &EpisodeRate(*self), &mut events);
        out.extend(events.into_iter().map(Episode::of));
    }

    /// Appends to `out` the episodes running at `t`: those with an onset from the start of the bin
    /// four before `t`'s up to `t` whose span of 10 `τ_b` is not over.
    pub fn episodes_active_at(
        &self,
        series: &MagnetarSeries,
        t: UniverseTime,
        out: &mut Vec<Episode>,
    ) {
        if to_nanos(t) < self.birth_nanos.saturating_sub(CUT_MARGIN_NANOS) {
            return;
        }
        let mut events = Vec::new();
        episode_bins().active_at(&series.episodes, t, &EpisodeRate(*self), &mut events);
        out.extend(
            events
                .into_iter()
                .map(Episode::of)
                .filter(|episode| episode.is_active_at(t)),
        );
    }

    /// The episode `id` names, regenerated on its own; `None` if `id` is not of the episodes'
    /// series, or names a rejected candidate or a number beyond its bin's count.
    #[must_use]
    pub fn episode(&self, series: &MagnetarSeries, id: EventId) -> Option<Episode> {
        episode_bins()
            .event(&series.episodes, id, &EpisodeRate(*self))
            .map(Episode::of)
    }

    /// The episodes that can reach the burst bins from `first` to `last`: those with onsets from
    /// [`MAX_EPISODE_SPAN`] before the first bin's start to the last bin's end.
    #[must_use]
    fn episodes_reaching(&self, series: &MagnetarSeries, first: i64, last: i64) -> Vec<Episode> {
        let start = i128::from(first) * i128::from(BURST_BIN_SECONDS) * 1_000_000_000
            - i128::from(MAX_EPISODE_SPAN.seconds()) * 1_000_000_000;
        let end = (i128::from(last) + 1) * i128::from(BURST_BIN_SECONDS) * 1_000_000_000;
        let window = TimeWindow::new(from_nanos_saturating(start), from_nanos_saturating(end))
            .expect("the reach of a bin's episodes ends after it starts");
        let mut episodes = Vec::new();
        self.episodes_in(series, window, &mut episodes);
        episodes
    }

    /// Appends to `out` the short bursts with onsets in `window`, sorted by onset and then by ID.
    ///
    /// # Panics
    ///
    /// Only on a broken invariant: the runs of bins lie inside the window's own bins, which the
    /// clock holds, and each meets the window. In debug builds also where overlapping episodes
    /// lift a bin's mean above 64, which takes two near their onsets of more than 29 expected
    /// bursts a bin each, some 10⁻¹⁰ of episodes.
    pub fn bursts_in(&self, series: &MagnetarSeries, window: TimeWindow, out: &mut Vec<Burst>) {
        if window.is_empty() {
            return;
        }
        let bins = burst_bins();
        // The bins the window touches, cut to the 40-bit bin numbers, whose bins beyond hold no
        // events, as `PoissonBins::events_in` cuts them.
        let bin_of = |t: UniverseTime| {
            t.seconds()
                .div_euclid(BURST_BIN_SECONDS)
                .clamp(EventBin::MIN.get(), EventBin::MAX.get())
        };
        let first = bin_of(window.start());
        let last = bin_of(from_nanos_saturating(to_nanos(window.end()) - 1));
        let episodes = self.episodes_reaching(series, first, last);
        // The bins the episodes' spans touch, merged, each run listed on its own with the episodes
        // that can overlap it, a slice of the sorted list: the other bins' bounds are zero, so this
        // drops nothing, no bin is generated twice, and an episode that overlaps none of a run's
        // bins adds exactly nothing to its bounds, rates or picks.
        let mut runs: Vec<(i64, i64)> = Vec::new();
        for episode in &episodes {
            let from = bin_of(episode.onset).max(first);
            let to = bin_of(from_nanos_saturating(to_nanos(episode.end()) - 1)).min(last);
            if from > to {
                continue;
            }
            match runs.last_mut() {
                Some((_, end)) if from <= *end + 1 => *end = (*end).max(to),
                _ => runs.push((from, to)),
            }
        }
        let begin = out.len();
        let (mut events, mut rates) = (Vec::new(), Vec::new());
        for (from, to) in runs {
            let run = TimeWindow::new(
                UniverseTime::new(from * BURST_BIN_SECONDS, 0).expect("a bin's start"),
                UniverseTime::new((to + 1) * BURST_BIN_SECONDS, 0).expect("a bin's end"),
            )
            .expect("a run of bins");
            let reach = from_nanos_saturating(
                to_nanos(run.start()) - i128::from(MAX_EPISODE_SPAN.seconds()) * 1_000_000_000,
            );
            let lo = episodes.partition_point(|e| e.onset <= reach);
            let hi = episodes.partition_point(|e| e.onset < run.end());
            let rate = BurstRate {
                episodes: &episodes[lo..hi],
            };
            let piece =
                TimeWindow::new(window.start().max(run.start()), window.end().min(run.end()))
                    .expect("a run inside the window's bins meets the window");
            events.clear();
            bins.events_in(&series.bursts, piece, &rate, &mut events);
            out.extend(
                events
                    .drain(..)
                    .map(|event| rate.burst_of(event, &mut rates)),
            );
        }
        out[begin..].sort_by(|a, b| a.onset.cmp(&b.onset).then(a.id.cmp(&b.id)));
    }

    /// Appends to `out` the short bursts running at `t`.
    pub fn bursts_active_at(&self, series: &MagnetarSeries, t: UniverseTime, out: &mut Vec<Burst>) {
        let bins = burst_bins();
        let Some(bin) = bins.bin_of(t) else {
            return;
        };
        let episodes = self.episodes_reaching(series, bin.get() - 1, bin.get());
        if episodes.is_empty() {
            return;
        }
        let rate = BurstRate {
            episodes: &episodes,
        };
        let (mut events, mut rates) = (Vec::new(), Vec::new());
        bins.active_at(&series.bursts, t, &rate, &mut events);
        out.extend(
            events
                .into_iter()
                .map(|event| rate.burst_of(event, &mut rates))
                .filter(|burst| burst.is_active_at(t)),
        );
    }

    /// The short burst `id` names, regenerated on its own with the episodes that reach its bin.
    #[must_use]
    pub fn burst(&self, series: &MagnetarSeries, id: EventId) -> Option<Burst> {
        if !series.bursts.names(id) {
            return None;
        }
        let bin = id.word().bin().get();
        let episodes = self.episodes_reaching(series, bin, bin);
        let rate = BurstRate {
            episodes: &episodes,
        };
        burst_bins()
            .event(&series.bursts, id, &rate)
            .map(|event| rate.burst_of(event, &mut Vec::new()))
    }

    /// Appends to `out` the giant flares with onsets in `window`, sorted by onset and then by ID.
    pub fn giant_flares_in(
        &self,
        series: &MagnetarSeries,
        window: TimeWindow,
        out: &mut Vec<GiantFlare>,
    ) {
        let Some(window) = self.active_part(window) else {
            return;
        };
        let mut events = Vec::new();
        giant_flare_bins().events_in(
            &series.giant_flares,
            window,
            &GiantFlareRate(*self),
            &mut events,
        );
        out.extend(events.into_iter().map(GiantFlare::of));
    }

    /// Appends to `out` the giant flares running at `t`.
    pub fn giant_flares_active_at(
        &self,
        series: &MagnetarSeries,
        t: UniverseTime,
        out: &mut Vec<GiantFlare>,
    ) {
        if to_nanos(t) < self.birth_nanos.saturating_sub(CUT_MARGIN_NANOS) {
            return;
        }
        let mut events = Vec::new();
        giant_flare_bins().active_at(&series.giant_flares, t, &GiantFlareRate(*self), &mut events);
        out.extend(
            events
                .into_iter()
                .map(GiantFlare::of)
                .filter(|flare| flare.is_active_at(t)),
        );
    }

    /// The giant flare `id` names, regenerated on its own.
    #[must_use]
    pub fn giant_flare(&self, series: &MagnetarSeries, id: EventId) -> Option<GiantFlare> {
        giant_flare_bins()
            .event(&series.giant_flares, id, &GiantFlareRate(*self))
            .map(GiantFlare::of)
    }
}

/// A magnetar's episodes as a [`RateModel`]: [`MagnetarModel::episode_rate_at`], bounded per bin
/// by the rate at the bin's first instant after the birth.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EpisodeRate<'a>(MagnetarModel<'a>);

impl RateModel for EpisodeRate<'_> {
    fn bound(&self, bin: TimeWindow) -> EventsPerSecond {
        EventsPerSecond::per_julian_year(EPISODES_PER_E_FOLD * self.0.e_folds_bound(bin))
    }

    fn rate(&self, t: UniverseTime) -> EventsPerSecond {
        self.0.episode_rate_at(t)
    }
}

/// A magnetar's giant flares as a [`RateModel`]: [`MagnetarModel::giant_flare_rate_at`], bounded
/// per bin as the episodes are.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GiantFlareRate<'a>(MagnetarModel<'a>);

impl RateModel for GiantFlareRate<'_> {
    fn bound(&self, bin: TimeWindow) -> EventsPerSecond {
        EventsPerSecond::per_julian_year(GIANT_FLARES_PER_E_FOLD * self.0.e_folds_bound(bin))
    }

    fn rate(&self, t: UniverseTime) -> EventsPerSecond {
        self.0.giant_flare_rate_at(t)
    }
}

/// `lo` × (`hi` ÷ `lo`)^`u`: log-uniform between `lo` and `hi` for a uniform `u` in [0, 1).
#[must_use]
fn log_uniform((lo, hi): (f64, f64), u: f64) -> f64 {
    lo * math::powf(hi / lo, u)
}

/// The value at `u` of the truncated power law dN/dE ∝ E^(−`index`) on `(lo, hi)`, by inverse
/// transform: [lo^(1−a) + u (hi^(1−a) − lo^(1−a))]^(1 ÷ (1−a)), from `lo` at u = 0 towards `hi`.
#[must_use]
fn power_law_at((lo, hi): (f64, f64), index: f64, u: f64) -> f64 {
    let power = 1.0 - index;
    let (low, high) = (math::powf(lo, power), math::powf(hi, power));
    math::powf(math::mul_add(u, high - low, low), 1.0 / power).clamp(lo, hi)
}

/// A short burst's duration at `u`, s: 10^(log₁₀ 0.1 + 0.35 z), z the standard normal truncated at
/// ±4 by inverse transform, z = Φ⁻¹(Φ(−4) + u [Φ(4) − Φ(−4)]).
#[must_use]
fn burst_duration_seconds_at(u: f64) -> f64 {
    let edge = BURST_DURATION_TRUNCATION / core::f64::consts::SQRT_2;
    let tail = 0.5 * math::erfc(edge);
    let kept = math::erf(edge);
    let z = math::normal_quantile(math::mul_add(u, kept, tail))
        .clamp(-BURST_DURATION_TRUNCATION, BURST_DURATION_TRUNCATION);
    BURST_DURATION_MEDIAN_SECONDS * math::exp10(BURST_DURATION_SIGMA_DEX * z)
}

/// The clock time `t` plus `span`, held to the clock's end.
#[must_use]
fn saturating_add(t: UniverseTime, span: Span) -> UniverseTime {
    t.checked_add(span)
        .unwrap_or_else(|| from_nanos_saturating(i128::MAX))
}

/// One outburst episode of a magnetar: its ID, onset, expected burst count and e-folding.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Episode {
    id: EventId,
    onset: UniverseTime,
    expected_bursts: f64,
    e_folding_seconds: f64,
    span: Span,
}

impl Episode {
    /// The episode of `event`: `N_b` from word 0 of its marks and `τ_b` from word 1.
    #[must_use]
    fn of(event: BinEvent) -> Self {
        let id = event.id();
        let onset = event.time();
        let mut marks = event.into_marks();
        let expected_bursts = log_uniform(BURST_COUNT, marks.uniform());
        let e_folding_seconds =
            log_uniform(BURST_E_FOLDING_DAYS, marks.uniform()) * SECONDS_PER_DAY;
        let span = Span::from_seconds_f64(EPISODE_SPAN_E_FOLDINGS * e_folding_seconds)
            .expect("ten e-foldings of at most ten days are a span");
        debug_assert!(span <= MAX_EPISODE_SPAN, "an episode lasts {span}");
        Self {
            id,
            onset,
            expected_bursts,
            e_folding_seconds,
            span,
        }
    }

    /// The episode's ID, from which [`episode`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The onset, from which its bursts come.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// The first instant after the episode: its onset plus 10 `τ_b`, floored to the nanosecond.
    #[must_use]
    pub fn end(&self) -> UniverseTime {
        saturating_add(self.onset, self.span)
    }

    /// `N_b`, the bursts the episode is expected to hold, 3–1,000 (not a whole number: the bursts
    /// themselves are a Poisson process).
    #[must_use]
    pub const fn expected_bursts(&self) -> f64 {
        self.expected_bursts
    }

    /// `τ_b`, the e-folding of its burst rate, s: 0.3–10 days.
    #[must_use]
    pub const fn e_folding_seconds(&self) -> f64 {
        self.e_folding_seconds
    }

    /// Whether the episode runs at `t`: from its onset, inclusive, to its end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        self.onset <= t && t < self.end()
    }

    /// The rate of the episode's bursts at `t`: (`N_b` ÷ `τ_b`) e^(−Δt ÷ `τ_b`) for Δt from 0 to 10
    /// `τ_b` since the onset, and 0 outside that.
    #[must_use]
    pub fn burst_rate_at(&self, t: UniverseTime) -> EventsPerSecond {
        if !self.is_active_at(t) {
            return EventsPerSecond::ZERO;
        }
        let elapsed = t
            .checked_since(self.onset)
            .map_or(0.0, Span::as_seconds_f64);
        let tau = self.e_folding_seconds;
        EventsPerSecond::new(self.expected_bursts / tau * math::exp(-elapsed / tau))
    }

    /// The largest the rate gets over `bin`: at the bin's start, or at the onset if that is later;
    /// zero if the episode does not overlap the bin.
    #[must_use]
    fn burst_bound(&self, bin: TimeWindow) -> f64 {
        if self.onset >= bin.end() || self.end() <= bin.start() {
            return 0.0;
        }
        self.burst_rate_at(bin.start().max(self.onset)).value()
    }
}

/// The short bursts' [`RateModel`] over the episodes that can reach the bins asked about.
#[derive(Debug, Clone, Copy)]
struct BurstRate<'e> {
    episodes: &'e [Episode],
}

impl RateModel for BurstRate<'_> {
    fn bound(&self, bin: TimeWindow) -> EventsPerSecond {
        let sum = self
            .episodes
            .iter()
            .fold(0.0, |sum, episode| sum + episode.burst_bound(bin));
        EventsPerSecond::new(sum + sum * BOUND_PADDING)
    }

    fn rate(&self, t: UniverseTime) -> EventsPerSecond {
        EventsPerSecond::new(
            self.episodes
                .iter()
                .fold(0.0, |sum, episode| sum + episode.burst_rate_at(t).value()),
        )
    }
}

impl BurstRate<'_> {
    /// The burst of `event`: its episode from word 0 of its marks, picked in proportion to the
    /// episodes' rates at its time (written into `rates`, a scratch buffer), its energy from word 1
    /// and its duration from word 2.
    #[must_use]
    fn burst_of(&self, event: BinEvent, rates: &mut Vec<f64>) -> Burst {
        let id = event.id();
        let onset = event.time();
        let mut marks = event.into_marks();
        rates.clear();
        rates.extend(
            self.episodes
                .iter()
                .map(|episode| episode.burst_rate_at(onset).value()),
        );
        let total = rates.iter().fold(0.0, |sum, &rate| sum + rate);
        let episode = marks
            .mark()
            .pick_weighted(rates, total)
            .map(|i| self.episodes[i].id)
            .expect("an accepted burst has a positive rate, and the bound is the rates' own sum");
        let energy_joules = power_law_at(BURST_ENERGY_JOULES, BURST_ENERGY_INDEX, marks.uniform());
        let duration_seconds = burst_duration_seconds_at(marks.uniform());
        Burst {
            id,
            episode,
            onset,
            energy_joules,
            duration: Span::from_seconds_f64(duration_seconds)
                .expect("a burst of at most 2.512 s is a span"),
        }
    }
}

/// One short burst of a magnetar: its ID, its episode's, its onset, energy and duration.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Burst {
    id: EventId,
    episode: EventId,
    onset: UniverseTime,
    energy_joules: f64,
    duration: Span,
}

impl Burst {
    /// The burst's ID, from which [`burst`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The ID of the episode the burst belongs to.
    #[must_use]
    pub const fn episode(&self) -> EventId {
        self.episode
    }

    /// The onset.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// The first instant after the burst.
    #[must_use]
    pub fn end(&self) -> UniverseTime {
        saturating_add(self.onset, self.duration)
    }

    /// The duration, 4.0 ms to 2.5 s, floored to the nanosecond.
    #[must_use]
    pub const fn duration(&self) -> Span {
        self.duration
    }

    /// The energy, J: 10³⁰–10³⁴ (10³⁷–10⁴¹ erg).
    #[must_use]
    pub const fn energy_joules(&self) -> f64 {
        self.energy_joules
    }

    /// The burst's mean luminosity over its duration, W: its energy over its duration.
    #[must_use]
    pub fn mean_luminosity_watts(&self) -> f64 {
        self.energy_joules / self.duration.as_seconds_f64()
    }

    /// Whether the burst runs at `t`: from its onset, inclusive, to its end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        self.onset <= t && t < self.end()
    }
}

/// One giant flare of a magnetar: its ID, onset and energy.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct GiantFlare {
    id: EventId,
    onset: UniverseTime,
    energy_joules: f64,
}

impl GiantFlare {
    /// The giant flare of `event`: its energy from word 0 of its marks.
    #[must_use]
    fn of(event: BinEvent) -> Self {
        let id = event.id();
        let onset = event.time();
        let energy_joules = power_law_at(
            GIANT_FLARE_ENERGY_JOULES,
            GIANT_FLARE_ENERGY_INDEX,
            event.into_marks().uniform(),
        );
        Self {
            id,
            onset,
            energy_joules,
        }
    }

    /// The flare's ID, from which [`giant_flare`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The onset of its initial spike.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// The end of its initial spike, [`GIANT_FLARE_SPIKE`] after the onset.
    #[must_use]
    pub fn end(&self) -> UniverseTime {
        saturating_add(self.onset, GIANT_FLARE_SPIKE)
    }

    /// The energy, J: 10³⁷–10⁴⁰ (10⁴⁴–10⁴⁷ erg).
    #[must_use]
    pub const fn energy_joules(&self) -> f64 {
        self.energy_joules
    }

    /// The spike's mean luminosity, W: its energy over [`GIANT_FLARE_SPIKE`].
    #[must_use]
    pub fn mean_luminosity_watts(&self) -> f64 {
        self.energy_joules / GIANT_FLARE_SPIKE.as_seconds_f64()
    }

    /// Whether the spike runs at `t`.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        self.onset <= t && t < self.end()
    }
}

/// The episode bins: 30 days, looking back four.
#[must_use]
fn episode_bins() -> PoissonBins {
    PoissonBins::new(EPISODE_BIN_SECONDS, EPISODE_LOOK_BACK_BINS)
        .expect("30 days lie between 16 s and 2^44 s")
}

/// The burst bins: 15 minutes, looking back one.
#[must_use]
fn burst_bins() -> PoissonBins {
    PoissonBins::new(BURST_BIN_SECONDS, BURST_LOOK_BACK_BINS)
        .expect("15 minutes lie between 16 s and 2^44 s")
}

/// The giant-flare bins: a Julian year, looking back one.
#[must_use]
fn giant_flare_bins() -> PoissonBins {
    PoissonBins::new(GIANT_FLARE_BIN_SECONDS, GIANT_FLARE_LOOK_BACK_BINS)
        .expect("a year lies between 16 s and 2^44 s")
}

/// Appends to `out` the outburst episodes of `star` with onsets in `window`, sorted by onset and
/// then by ID: none for a star that is never a magnetar ([`MagnetarModel::of`]).
///
/// `series` is the star's three series ([`MagnetarSeries::new`] of the seed and its body):
/// [`StarModel`] holds no seed or ID, so they come in beside the star. Every bin the window touches
/// is generated whole, so a listing over a window is the union of the listings over any partition
/// of it, in any order of asking. `out` is only appended to.
pub fn episodes_in(
    star: &StarModel,
    series: &MagnetarSeries,
    window: TimeWindow,
    out: &mut Vec<Episode>,
) {
    if let Some(model) = MagnetarModel::of(star) {
        model.episodes_in(series, window, out);
    }
}

/// Appends to `out` the episodes of `star` running at `t` ([`MagnetarModel::episodes_active_at`]).
pub fn episodes_active_at(
    star: &StarModel,
    series: &MagnetarSeries,
    t: UniverseTime,
    out: &mut Vec<Episode>,
) {
    if let Some(model) = MagnetarModel::of(star) {
        model.episodes_active_at(series, t, out);
    }
}

/// The episode `id` names, regenerated on its own: `None` if `id` is not of the star's episode
/// series, names a rejected candidate or a number beyond its bin's count, or the star is never a
/// magnetar.
#[must_use]
pub fn episode(star: &StarModel, series: &MagnetarSeries, id: EventId) -> Option<Episode> {
    MagnetarModel::of(star)?.episode(series, id)
}

/// Appends to `out` the short bursts of `star` with onsets in `window`, sorted by onset and then by
/// ID: none for a star that is never a magnetar.
///
/// The episodes that thin them are regenerated from their own series for each call, as
/// [`episodes_in`] would list them, so the listing is a pure function of the star, the series and
/// the window, and the union of the listings over any partition of it.
///
/// # Examples
///
/// A young magnetar's bursts over thirty years, each within ten e-foldings of its episode's onset,
/// and one regenerated from its ID alone:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
/// use hyperion_sim::stellar::events::magnetar::{MagnetarSeries, burst, bursts_in, episode};
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let z = |value: f64| StandardNormal::new(value).ok_or("a finite draw");
/// let draws = StarDraws::from_parts(StarDrawsParts {
///     ns_field: z(2.05 / 0.6)?,
///     ns_spin: [z(-0.5)?; 8],
///     ..StarDrawsParts::MEDIAN
/// });
/// let (mass, solar) = (SolarMasses::new(15.0), Composition::SOLAR);
/// let lifetime = StarModel::new(mass, solar, draws.clone(), Years::new(1e9))?
///     .lifetime()
///     .ok_or("a 15 M☉ star dies")?;
/// let star = StarModel::new(mass, solar, draws, Years::new(lifetime.value() + 2e3))?;
/// let body = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let series = MagnetarSeries::new(Seed::new(7), body);
/// let decades = TimeWindow::new(UniverseTime::EPOCH, UniverseTime::from_julian_years(30).ok_or("")?)?;
/// let mut bursts = Vec::new();
/// bursts_in(&star, &series, decades, &mut bursts);
/// for b in &bursts {
///     let e = episode(&star, &series, b.episode()).ok_or("its episode")?;
///     assert!(e.onset() <= b.onset() && b.onset() < e.end());
/// }
/// let first = bursts.first().ok_or("some five episodes in thirty years")?;
/// assert_eq!(burst(&star, &series, first.id()).as_ref(), Some(first));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn bursts_in(
    star: &StarModel,
    series: &MagnetarSeries,
    window: TimeWindow,
    out: &mut Vec<Burst>,
) {
    if let Some(model) = MagnetarModel::of(star) {
        model.bursts_in(series, window, out);
    }
}

/// Appends to `out` the short bursts of `star` running at `t` ([`MagnetarModel::bursts_active_at`]).
pub fn bursts_active_at(
    star: &StarModel,
    series: &MagnetarSeries,
    t: UniverseTime,
    out: &mut Vec<Burst>,
) {
    if let Some(model) = MagnetarModel::of(star) {
        model.bursts_active_at(series, t, out);
    }
}

/// The short burst `id` names, regenerated on its own with the episodes that reach its bin: `None`
/// if `id` is not of the star's burst series, names a rejected candidate or a number beyond its
/// bin's count, or the star is never a magnetar.
#[must_use]
pub fn burst(star: &StarModel, series: &MagnetarSeries, id: EventId) -> Option<Burst> {
    MagnetarModel::of(star)?.burst(series, id)
}

/// Appends to `out` the giant flares of `star` with onsets in `window`, sorted by onset and then by
/// ID: none for a star that is never a magnetar.
pub fn giant_flares_in(
    star: &StarModel,
    series: &MagnetarSeries,
    window: TimeWindow,
    out: &mut Vec<GiantFlare>,
) {
    if let Some(model) = MagnetarModel::of(star) {
        model.giant_flares_in(series, window, out);
    }
}

/// Appends to `out` the giant flares of `star` whose spike runs at `t`.
pub fn giant_flares_active_at(
    star: &StarModel,
    series: &MagnetarSeries,
    t: UniverseTime,
    out: &mut Vec<GiantFlare>,
) {
    if let Some(model) = MagnetarModel::of(star) {
        model.giant_flares_active_at(series, t, out);
    }
}

/// The giant flare `id` names, regenerated on its own: `None` if `id` is not of the star's giant
/// flare series, names a rejected candidate or a number beyond its bin's count, or the star is
/// never a magnetar.
#[must_use]
pub fn giant_flare(star: &StarModel, series: &MagnetarSeries, id: EventId) -> Option<GiantFlare> {
    MagnetarModel::of(star)?.giant_flare(series, id)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{
        ALPHA, assert_p_value, assert_poisson_count, ks_one_sample, poisson_interval,
    };

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::coords::UnitVector;
    use crate::events::event_id;
    use crate::events::testing::{assert_partition_independent, partition};
    use crate::id::{SystemId, event_tags};
    use crate::stellar::Composition;
    use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts, UnitUniform};
    use crate::stellar::remnant::neutron_star::{
        BIRTH_PERIOD_MEAN, BIRTH_PERIOD_SIGMA, LOG_BIRTH_FIELD_MEAN, LOG_BIRTH_FIELD_SIGMA,
    };
    use crate::time::{ClockWindow, SourceHorizon};
    use crate::units::SolarMasses;

    const SEED: Seed = Seed::new(0x0e7e_0006_0028_000c);
    const YEAR: f64 = SECONDS_PER_JULIAN_YEAR;
    const DAY: f64 = SECONDS_PER_DAY;

    /// The progenitor: 15 M☉ of solar composition, which leaves a neutron star.
    const PROGENITOR: f64 = 15.0;

    /// The draws of a star whose neutron star is born at `period` s with a field of 10^`log_b0` G.
    fn draws(log_b0: f64, period: f64) -> StarDraws {
        let z = |value: f64| StandardNormal::new(value).unwrap();
        StarDraws::from_parts(StarDrawsParts {
            ns_field: z((log_b0 - LOG_BIRTH_FIELD_MEAN) / LOG_BIRTH_FIELD_SIGMA),
            ns_spin: [z((period - BIRTH_PERIOD_MEAN.value()) / BIRTH_PERIOD_SIGMA.value()); 8],
            ..StarDrawsParts::MEDIAN
        })
    }

    /// The progenitor's lifetime, years (its other draws are the median's, whatever the neutron
    /// star's).
    fn lifetime() -> f64 {
        StarModel::new(
            SolarMasses::new(PROGENITOR),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(1e9),
        )
        .unwrap()
        .lifetime()
        .unwrap()
        .value()
    }

    /// The progenitor with a neutron star born at `period` s with a field of 10^`log_b0` G, which
    /// is `years` old at the epoch (negative for one born after it).
    fn magnetar(log_b0: f64, period: f64, years: f64) -> StarModel {
        StarModel::new(
            SolarMasses::new(PROGENITOR),
            Composition::SOLAR,
            draws(log_b0, period),
            Years::new(lifetime() + years),
        )
        .unwrap()
    }

    fn body(index: u16) -> BodyId {
        BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), index)
    }

    fn series(index: u16) -> MagnetarSeries {
        MagnetarSeries::new(SEED, body(index))
    }

    fn after(t: UniverseTime, seconds: f64) -> UniverseTime {
        t.checked_add(Span::from_seconds_f64(seconds).unwrap())
            .unwrap()
    }

    /// The instant `n` nanoseconds from `t`.
    fn shifted(t: UniverseTime, n: i64) -> UniverseTime {
        from_nanos_saturating(to_nanos(t) + i128::from(n))
    }

    /// `years` Julian years from the epoch.
    fn years(years: f64) -> UniverseTime {
        after(UniverseTime::EPOCH, years * YEAR)
    }

    fn window(from: UniverseTime, to: UniverseTime) -> TimeWindow {
        TimeWindow::new(from, to).unwrap()
    }

    fn episodes(model: &MagnetarModel<'_>, s: &MagnetarSeries, w: TimeWindow) -> Vec<Episode> {
        let mut out = Vec::new();
        model.episodes_in(s, w, &mut out);
        out
    }

    fn bursts(model: &MagnetarModel<'_>, s: &MagnetarSeries, w: TimeWindow) -> Vec<Burst> {
        let mut out = Vec::new();
        model.bursts_in(s, w, &mut out);
        out
    }

    fn flares(model: &MagnetarModel<'_>, s: &MagnetarSeries, w: TimeWindow) -> Vec<GiantFlare> {
        let mut out = Vec::new();
        model.giant_flares_in(s, w, &mut out);
        out
    }

    fn relative(a: f64, b: f64) -> f64 {
        ((a - b) / b).abs()
    }

    /// The field's fractional decay rate is 1 ÷ (τ + t) while it decays above its floor, and the
    /// ruling's figures follow: 0.2, 0.02 and 0.0088 episodes a year at 10¹⁵, 10¹⁴ and 4.4 × 10¹³ G,
    /// and 0.05 at 2.5 × 10¹⁴ G.
    #[test]
    fn the_rates_scale_with_the_fields_fractional_decay() {
        assert_same_bits(FIELD_DECAY_PRODUCT_GAUSS_YEARS, 1e19);
        let ns = NeutronStar::new(
            crate::units::Seconds::new(0.25),
            Gauss::new(1e15),
            UnitVector::NORTH,
            crate::units::Radians::new(1.0),
            0.0,
        );
        let tau = FIELD_DECAY_PRODUCT_GAUSS_YEARS / 1e15;
        for t in [0.0, 10.0, 1e3, 1e4, 1e5] {
            let e = field_e_folds_per_year(ns.field_at(Years::new(t)));
            assert!(relative(e, 1.0 / (tau + t)) < 1e-12, "{t}");
        }
        for (field, expected) in [(1e15, 0.2), (1e14, 0.02), (4.4e13, 0.0088), (2.5e14, 0.05)] {
            let r = EPISODES_PER_E_FOLD * field_e_folds_per_year(Gauss::new(field));
            assert!(relative(r, expected) < 1e-12, "{field}: {r}");
        }
        // A 10¹⁵ G magnetar flares about once in 150 years.
        let once_in = 1.0 / (GIANT_FLARES_PER_E_FOLD * field_e_folds_per_year(Gauss::new(1e15)));
        assert!((145.0..155.0).contains(&once_in), "{once_in}");
    }

    /// An LCG uniform in the open interval (0, 1), as P06.T21's census draws.
    fn open_unit(rng: &mut Lcg) -> f64 {
        (rng.next_f64() + f64::EPSILON) / (1.0 + 2.0 * f64::EPSILON)
    }

    /// P06.T21's census sample, `magnetars_are_the_high_tail_of_the_birth_field`'s: 20,000 neutron
    /// stars of the birth draws from the seed 11, in its construction.
    fn census() -> Vec<NeutronStar> {
        let mut rng = Lcg::new(11);
        (0..20_000)
            .map(|_| {
                let mut normal =
                    || StandardNormal::new(math::normal_quantile(open_unit(&mut rng))).unwrap();
                let ns_spin = core::array::from_fn(|_| normal());
                let ns_field = normal();
                let z = 2.0 * open_unit(&mut rng) - 1.0;
                let phi = 2.0 * core::f64::consts::PI * open_unit(&mut rng);
                let s = (1.0 - z * z).sqrt();
                let axis = UnitVector::from_components([s * math::cos(phi), s * math::sin(phi), z])
                    .unwrap();
                NeutronStar::from_draws(&StarDraws::from_parts(StarDrawsParts {
                    ns_spin,
                    ns_field,
                    ns_spin_axis: axis,
                    ns_inclination: UnitUniform::new(open_unit(&mut rng)).unwrap(),
                    ns_phase: UnitUniform::new(open_unit(&mut rng)).unwrap(),
                    ..StarDrawsParts::MEDIAN
                }))
            })
            .collect()
    }

    /// The constant derived again from T21's census: over its 20,000 neutron stars and its sweep of
    /// ages (701 log-spaced edges to 10 Myr), at two core collapses a century and 62% of them
    /// leaving a neutron star, the mean years as a magnetar give its 84 active magnetars, and the
    /// mean e-folds of the field while one, ln(B(a) ÷ B(b)) over each step whose middle is a
    /// magnetar, give the Galaxy's e-folds a year; 0.044 over them is [`GIANT_FLARES_PER_E_FOLD`],
    /// to 10⁻³.
    #[test]
    fn the_giant_flare_constant_is_t21s_census_at_the_galactic_rate() {
        let stars = census();
        let edges: Vec<f64> = (0..=700)
            .map(|i| math::exp10(f64::from(i) * 0.01))
            .collect();
        let (mut years_sum, mut e_folds_sum) = (0.0, 0.0);
        for ns in &stars {
            for w in edges.windows(2) {
                let middle = (w[0] * w[1]).sqrt();
                if ns.state_at(Years::new(middle)).is_magnetar() {
                    years_sum += w[1] - w[0];
                    e_folds_sum += math::ln(
                        ns.field_at(Years::new(w[0])).value()
                            / ns.field_at(Years::new(w[1])).value(),
                    );
                }
            }
        }
        let n = f64::from(u32::try_from(stars.len()).unwrap());
        let births = 0.02 * 0.62;
        let active = births * years_sum / n;
        let e_folds_a_year = births * e_folds_sum / n;
        let derived = GALACTIC_GIANT_FLARES_PER_YEAR / e_folds_a_year;
        eprintln!(
            "{active:.1} active magnetars, {e_folds_a_year:.4e} e-folds a year in all: \
             {derived:.4} giant flares per e-fold"
        );
        assert!((83.5..84.5).contains(&active), "{active}");
        assert!(
            relative(GIANT_FLARES_PER_E_FOLD, derived) < 1e-3,
            "{derived}"
        );
    }

    /// A star whose neutron star is born at or below the critical field, a living star, a white
    /// dwarf, a black hole and a brown dwarf have no model and no events.
    #[test]
    fn only_a_neutron_star_born_above_the_critical_field_has_a_model() {
        let horizon = window(SourceHorizon::START, ClockWindow::END);
        let star = |mass: f64, age: f64| {
            StarModel::new(
                SolarMasses::new(mass),
                Composition::SOLAR,
                StarDraws::median(),
                Years::new(age),
            )
            .unwrap()
        };
        for s in [
            magnetar(13.6, 0.25, 1e3),
            star(PROGENITOR, 1e7),
            star(1.0, 1.2e10),
            star(60.0, 1e7),
            star(0.05, 1e9),
        ] {
            assert!(MagnetarModel::of(&s).is_none(), "{:?}", s.initial_mass());
            let sr = series(0);
            let (mut e, mut b, mut g) = (Vec::new(), Vec::new(), Vec::new());
            episodes_in(&s, &sr, horizon, &mut e);
            bursts_in(&s, &sr, window(UniverseTime::EPOCH, years(1.0)), &mut b);
            giant_flares_in(&s, &sr, horizon, &mut g);
            assert!(e.is_empty() && b.is_empty() && g.is_empty());
        }
    }

    /// The cases of the magnetar tests: born at 10¹⁵ G and 0.2 s 2,000 years before the epoch; at
    /// 10^14.5 G and 0.3 s 10,000 years before it; and at 10^15.2 G and 0.25 s 300 years after it.
    fn cases() -> [StarModel; 3] {
        [
            magnetar(15.0, 0.2, 2e3),
            magnetar(14.5, 0.3, 1e4),
            magnetar(15.2, 0.25, -300.0),
        ]
    }

    /// No episode, burst or giant flare while the star is not a magnetar; each rate is
    /// non-increasing while it is one, and zero before the birth and from the field's reaching the
    /// critical field; and every bound holds at 43 instants of each bin around the birth, the
    /// magnetar's start and its end.
    #[test]
    fn nothing_happens_unless_a_magnetar_and_the_rates_only_fall() {
        for s in cases() {
            let model = MagnetarModel::of(&s).unwrap();
            // The magnetar's own span and a day beyond it, sampled: at the quiet instant, placed
            // from the ages, the field may still be a hair above the critical field.
            let (birth, quiet) = (model.birth(), model.quiet_from());
            let span = quiet.checked_since(birth).unwrap().as_seconds_f64() + DAY;
            let (mut last_episode, mut last_flare) = (f64::INFINITY, f64::INFINITY);
            let (mut onset, mut ended) = (None, false);
            for i in 0..=4_000 {
                let t = after(birth, span * f64::from(i) / 4_000.0);
                let (e, g) = (
                    model.episode_rate_at(t).value(),
                    model.giant_flare_rate_at(t).value(),
                );
                if model.is_magnetar_at(t) {
                    // Once it stops being one, it never is again.
                    assert!(!ended, "a magnetar again at {t}");
                    assert!(e <= last_episode && g <= last_flare, "{t}");
                    assert!(e > 0.0 && g > 0.0);
                    onset.get_or_insert(t);
                    (last_episode, last_flare) = (e, g);
                } else {
                    assert_same_bits(e, 0.0);
                    assert_same_bits(g, 0.0);
                    ended |= onset.is_some();
                }
            }
            assert!(onset.is_some() && ended);
            assert_eq!(GIANT_FLARE_SPIKE, Span::new(0, 200_000_000).unwrap());
            assert_eq!(
                model.episode_rate_at(shifted(birth, -1)),
                EventsPerSecond::ZERO
            );
            assert_eq!(
                model.episode_rate_at(after(quiet, DAY)),
                EventsPerSecond::ZERO
            );
            // The bounds about the birth, the magnetar's start and its end.
            let start = onset.unwrap();
            for (bins, rate) in [
                (episode_bins(), &EpisodeRate(model) as &dyn Fn2),
                (giant_flare_bins(), &GiantFlareRate(model) as &dyn Fn2),
            ] {
                for t in [birth, start, quiet, years(0.0)] {
                    let k = bins.bin_of(t).unwrap().get();
                    for k in k - 2..=k + 2 {
                        let bin = bins.bin_window(EventBin::new(k).unwrap()).unwrap();
                        let bound = rate.bound_of(bin);
                        let width = bin
                            .end()
                            .checked_since(bin.start())
                            .unwrap()
                            .as_seconds_f64();
                        for i in 0..=42 {
                            let at = if i == 42 {
                                shifted(bin.end(), -1)
                            } else {
                                after(bin.start(), width * f64::from(i) / 42.0)
                            };
                            assert!(rate.rate_of(at) <= bound, "{at}");
                        }
                        assert!(bound * width <= PoissonBins::MAX_MEAN_PER_BIN);
                    }
                }
            }
            let sr = series(1);
            let horizon = window(SourceHorizon::START, ClockWindow::END);
            for e in episodes(&model, &sr, horizon) {
                assert!(model.is_magnetar_at(e.onset()));
            }
            for g in flares(&model, &sr, horizon) {
                assert!(model.is_magnetar_at(g.onset()));
            }
        }
    }

    /// The bounds of the episodes and giant flares hold at 43 instants of each bin from a bin
    /// before the birth to the quiet's, in 160 bins log-spaced in the neutron star's age, over
    /// birth fields of 10^13.7–10^16 G and periods of 0.02–0.6 s; and a burst bin's bound holds
    /// for the marks' corners alone and in overlapping pairs, its mean under 64 for one episode and
    /// for every pair but the strongest, two of which a minute apart exceed it (the documented
    /// case of some 10⁻¹⁰ of episodes).
    #[test]
    fn the_bounds_hold_across_fields_periods_ages_and_the_marks_corners() {
        let mut checked = 0_u32;
        for log_b0 in [13.7, 14.0, 14.5, 15.0, 15.5, 16.0] {
            for period in [0.02, 0.1, 0.25, 0.6] {
                let s = magnetar(log_b0, period, 2e5);
                let model = MagnetarModel::of(&s).unwrap();
                let (birth, quiet) = (model.birth(), model.quiet_from());
                let span = quiet.checked_since(birth).unwrap().as_seconds_f64();
                for (bins, rate) in [
                    (episode_bins(), &EpisodeRate(model) as &dyn Fn2),
                    (giant_flare_bins(), &GiantFlareRate(model) as &dyn Fn2),
                ] {
                    let mut instants = vec![after(birth, -DAY), birth, after(quiet, DAY)];
                    instants.extend((0..160).map(|k| {
                        after(birth, span * math::exp10(-6.0 + 6.0 * f64::from(k) / 159.0))
                    }));
                    for t in instants {
                        let bin = bins.bin_window(bins.bin_of(t).unwrap()).unwrap();
                        let bound = rate.bound_of(bin);
                        let width = bin
                            .end()
                            .checked_since(bin.start())
                            .unwrap()
                            .as_seconds_f64();
                        for i in 0..=42 {
                            let at = if i == 42 {
                                shifted(bin.end(), -1)
                            } else {
                                after(bin.start(), width * f64::from(i) / 42.0)
                            };
                            assert!(rate.rate_of(at) <= bound, "10^{log_b0} G, {period} s, {at}");
                            checked += 1;
                        }
                        assert!(bound * width <= PoissonBins::MAX_MEAN_PER_BIN);
                    }
                }
            }
        }
        // The marks' corners: N_b of 3 and 1,000 (just under, as a uniform below 1 gives),
        // τ_b of 0.3 and 10 days, alone and two at once a minute apart.
        let onset = UniverseTime::new(1_000 * BURST_BIN_SECONDS + 77, 0).unwrap();
        let episode = |n: f64, days: f64, at: UniverseTime| {
            let tau = days * DAY;
            Episode {
                id: series(0).episodes().event_id(EventBin::new(0).unwrap(), 0),
                onset: at,
                expected_bursts: n,
                e_folding_seconds: tau,
                span: Span::from_seconds_f64(EPISODE_SPAN_E_FOLDINGS * tau).unwrap(),
            }
        };
        let mut sets = Vec::new();
        for n in [3.0, 999.999] {
            for days in [0.3, 10.0] {
                sets.push(vec![episode(n, days, onset)]);
                sets.push(vec![
                    episode(n, days, onset),
                    episode(n, days, after(onset, 60.0)),
                ]);
            }
        }
        let bins = burst_bins();
        for set in &sets {
            let rate = BurstRate { episodes: set };
            let k0 = bins.bin_of(onset).unwrap().get();
            for k in k0 - 1..k0 + 200 {
                let bin = bins.bin_window(EventBin::new(k).unwrap()).unwrap();
                let bound = rate.bound(bin);
                for i in 0..=42_i64 {
                    let t = if i == 42 {
                        shifted(bin.end(), -1)
                    } else {
                        UniverseTime::new(k * BURST_BIN_SECONDS + i * 21, 0).unwrap()
                    };
                    assert!(rate.rate(t) <= bound);
                    checked += 1;
                }
                // Two of the strongest episodes a minute apart are the documented excess.
                let strongest_pair = set.len() == 2
                    && set[0].expected_bursts > 900.0
                    && set[0].e_folding_seconds < DAY;
                let mean = bound.value() * 900.0;
                if !strongest_pair {
                    assert!(mean <= PoissonBins::MAX_MEAN_PER_BIN, "{k}: {mean}");
                } else if k == k0 + 1 {
                    assert!(mean > PoissonBins::MAX_MEAN_PER_BIN, "{mean}");
                }
            }
        }
        // The strongest episode alone fills a quarter-hour bin to 34.7.
        let strongest = [episode(999.999, 0.3, onset)];
        let top = BurstRate {
            episodes: &strongest,
        }
        .bound(bins.bin_window(bins.bin_of(onset).unwrap()).unwrap())
        .value()
            * 900.0;
        assert!((34.0..34.8).contains(&top), "{top}");
        assert!(checked > 100_000, "{checked} instants");
    }

    /// A rate model read through a trait object, for the bound sweep over both kinds.
    trait Fn2 {
        fn bound_of(&self, bin: TimeWindow) -> f64;
        fn rate_of(&self, t: UniverseTime) -> f64;
    }

    impl<R: RateModel> Fn2 for R {
        fn bound_of(&self, bin: TimeWindow) -> f64 {
            self.bound(bin).value()
        }

        fn rate_of(&self, t: UniverseTime) -> f64 {
            self.rate(t).value()
        }
    }

    /// Every burst falls within 10 `τ_b` of its episode's onset, after it, and only while the
    /// star is a magnetar; the burst-rate bound holds at 43 instants of every bin of the episodes'
    /// spans; and the bursts of an episode number about its `N_b`.
    #[test]
    fn every_burst_falls_within_its_episode() {
        let s = magnetar(15.0, 0.2, 2e3);
        let model = MagnetarModel::of(&s).unwrap();
        let w = window(UniverseTime::EPOCH, years(20.0));
        let (mut checked, mut listed, mut expected) = (0_u32, 0_u64, 0.0);
        for index in 0..4 {
            let sr = series(index);
            let eps = episodes(&model, &sr, window(after(w.start(), -100.0 * DAY), w.end()));
            let all = bursts(&model, &sr, w);
            for b in &all {
                let e = eps.iter().find(|e| e.id() == b.episode()).unwrap();
                assert!(e.onset() <= b.onset() && b.onset() < e.end());
                assert!(model.is_magnetar_at(b.onset()));
                assert_eq!(model.episode(&sr, e.id()).as_ref(), Some(e));
            }
            listed += u64::try_from(all.len()).unwrap();
            for e in &eps {
                // The episode's expected bursts inside w: N_b (e^(−a ÷ τ) − e^(−b ÷ τ)) over its
                // part in w, a and b the times since its onset.
                let tau = e.e_folding_seconds();
                let since = |t: UniverseTime| t.checked_since(e.onset()).unwrap().as_seconds_f64();
                let a = since(w.start().max(e.onset())).max(0.0);
                let b = since(w.end().min(e.end())).max(0.0);
                if b > a {
                    expected += e.expected_bursts() * (math::exp(-a / tau) - math::exp(-b / tau));
                }
            }
            // The bound over the bins of the first episode's span.
            let rate = BurstRate { episodes: &eps };
            if let Some(first) = eps.iter().find(|e| w.contains(e.onset())) {
                let bins = burst_bins();
                let k0 = bins.bin_of(first.onset()).unwrap().get();
                for k in (k0 - 1..k0 + 400).step_by(7) {
                    let bin = bins.bin_window(EventBin::new(k).unwrap()).unwrap();
                    let bound = rate.bound(bin);
                    for i in 0..=42_i64 {
                        let t = if i == 42 {
                            shifted(bin.end(), -1)
                        } else {
                            UniverseTime::new(k * BURST_BIN_SECONDS + i * 21, 0).unwrap()
                        };
                        assert!(rate.rate(t) <= bound);
                        checked += 1;
                    }
                    assert!(bound.value() * 900.0 <= PoissonBins::MAX_MEAN_PER_BIN);
                }
            }
        }
        eprintln!("{listed} bursts against {expected:.1} expected from their episodes");
        assert!(
            checked > 1_000 && listed > 100,
            "{checked} instants, {listed} bursts"
        );
        assert_poisson_count("bursts of four series", listed, expected, ALPHA);
    }

    /// The K–S test of `values` against `cdf`.
    fn assert_follows(what: &str, mut values: Vec<f64>, cdf: impl Fn(f64) -> f64) {
        assert!(values.len() > 200, "{what}: {}", values.len());
        let ks = ks_one_sample(&mut values, |x| cdf(x).clamp(0.0, 1.0));
        assert_p_value(what, ks.p_value, ALPHA);
    }

    /// The cdf of the truncated power law dN/dE ∝ E^(−a) on `(lo, hi)`.
    fn power_law_cdf((lo, hi): (f64, f64), index: f64) -> impl Fn(f64) -> f64 {
        let p = 1.0 - index;
        let (low, high) = (math::powf(lo, p), math::powf(hi, p));
        move |x| (math::powf(x, p) - low) / (high - low)
    }

    /// Every mark lies in its range and follows its law (K–S): `N_b` and `τ_b` log-uniform, the
    /// bursts' energies E^(−5/3), their durations the truncated log-normal, and the giant flares'
    /// energies E^(−1.7).
    #[test]
    fn every_mark_follows_its_law() {
        let s = magnetar(15.0, 0.2, 2e3);
        let model = MagnetarModel::of(&s).unwrap();
        let (mut eps, mut bs, mut gs) = (Vec::new(), Vec::new(), Vec::new());
        for index in 0..60 {
            let sr = series(index);
            eps.extend(episodes(&model, &sr, window(years(-1e3), years(1e3))));
            if index < 3 {
                bs.extend(bursts(&model, &sr, window(UniverseTime::EPOCH, years(5.0))));
            }
            gs.extend(flares(&model, &sr, window(years(-2e3), years(2e4))));
        }
        let log_uniform_cdf =
            |(lo, hi): (f64, f64)| move |x: f64| math::ln(x / lo) / math::ln(hi / lo);
        assert!(
            eps.iter()
                .all(|e| (3.0..1_000.0).contains(&e.expected_bursts()))
        );
        assert_follows(
            "N_b",
            eps.iter().map(Episode::expected_bursts).collect(),
            log_uniform_cdf(BURST_COUNT),
        );
        assert_follows(
            "τ_b",
            eps.iter().map(|e| e.e_folding_seconds() / DAY).collect(),
            log_uniform_cdf(BURST_E_FOLDING_DAYS),
        );
        assert!(
            bs.iter()
                .all(|b| (BURST_ENERGY_JOULES.0..=BURST_ENERGY_JOULES.1)
                    .contains(&b.energy_joules()))
        );
        assert_follows(
            "burst energy",
            bs.iter().map(Burst::energy_joules).collect(),
            power_law_cdf(BURST_ENERGY_JOULES, BURST_ENERGY_INDEX),
        );
        let edge = BURST_DURATION_TRUNCATION / core::f64::consts::SQRT_2;
        let (tail, kept) = (0.5 * math::erfc(edge), math::erf(edge));
        assert!(bs.iter().all(|b| b.duration() <= MAX_BURST_DURATION));
        assert_follows(
            "burst duration",
            bs.iter().map(|b| b.duration().as_seconds_f64()).collect(),
            move |x| {
                let z = math::log10(x / BURST_DURATION_MEDIAN_SECONDS) / BURST_DURATION_SIGMA_DEX;
                (0.5 * math::erfc(-z / core::f64::consts::SQRT_2) - tail) / kept
            },
        );
        assert!(gs.iter().all(|g| {
            (GIANT_FLARE_ENERGY_JOULES.0..=GIANT_FLARE_ENERGY_JOULES.1).contains(&g.energy_joules())
        }));
        assert_follows(
            "giant flare energy",
            gs.iter().map(GiantFlare::energy_joules).collect(),
            power_law_cdf(GIANT_FLARE_ENERGY_JOULES, GIANT_FLARE_ENERGY_INDEX),
        );
        for g in &gs {
            assert_eq!(g.end().checked_since(g.onset()), Some(GIANT_FLARE_SPIKE));
        }
    }

    /// Every listed event of each kind is regenerated from its ID, and IDs of other series or kinds
    /// name nothing.
    #[test]
    fn an_id_regenerates_its_event_and_names_nothing_else() {
        let s = magnetar(15.0, 0.2, 2e3);
        let model = MagnetarModel::of(&s).unwrap();
        let sr = series(2);
        let eps = episodes(&model, &sr, window(years(-200.0), years(200.0)));
        let bs = bursts(&model, &sr, window(UniverseTime::EPOCH, years(3.0)));
        let gs = flares(&model, &sr, window(years(-2e3), years(1e4)));
        assert!(eps.len() > 20 && bs.len() > 50 && gs.len() > 5);
        for e in &eps {
            assert_eq!(episode(&s, &sr, e.id()).as_ref(), Some(e));
        }
        for b in &bs {
            assert_eq!(burst(&s, &sr, b.id()).as_ref(), Some(b));
        }
        for g in &gs {
            assert_eq!(giant_flare(&s, &sr, g.id()).as_ref(), Some(g));
        }
        let other = series(3);
        let word = bs[0].id().word();
        for foreign in [
            event_id(
                body(9).into(),
                tags::STAR_MAGNETAR_BURST,
                word.bin(),
                word.number(),
            ),
            event_id(
                body(2).into(),
                event_tags::SELF_TEST,
                word.bin(),
                word.number(),
            ),
            event_id(
                body(2).into(),
                tags::STAR_MAGNETAR_EPISODE,
                word.bin(),
                word.number(),
            ),
        ] {
            assert_eq!(model.burst(&sr, foreign), None);
        }
        assert_eq!(model.episode(&other, eps[0].id()), None);
        assert_eq!(model.giant_flare(&other, gs[0].id()), None);
    }

    /// The window of the burst partition tests: 20 days from just before the first episode of
    /// `s`'s series 5 in the 30 years from the epoch with at least 50 expected bursts.
    fn burst_window(s: &StarModel) -> TimeWindow {
        let model = MagnetarModel::of(s).unwrap();
        let busy = episodes(&model, &series(5), window(UniverseTime::EPOCH, years(30.0)))
            .into_iter()
            .find(|e| e.expected_bursts() > 50.0)
            .unwrap();
        window(
            after(busy.onset(), -1_234.5),
            after(busy.onset(), 20.0 * DAY),
        )
    }

    /// The cut sets of the partition tests for bursts over `w`, from its listing `listed`.
    fn burst_cases(w: TimeWindow, listed: &[Burst]) -> (TimeWindow, Vec<Vec<UniverseTime>>) {
        assert!(listed.len() > 20, "{} bursts", listed.len());
        let cuts = vec![
            vec![after(w.start(), 3.0 * DAY)],
            vec![
                listed[3].onset(),
                listed[3].onset(),
                shifted(listed[4].onset(), 1),
            ],
            vec![
                shifted(listed[5].onset(), -1),
                after(listed[9].onset(), 450.0),
            ],
            (1..40)
                .map(|k| after(w.start(), 43_000.0 * f64::from(k) + 13.0))
                .collect(),
        ];
        (w, cuts)
    }

    /// The brainstorm's time test through the public functions, for all three kinds: a listing over
    /// a window is the union of the listings over any partition of it, in any order of asking.
    #[test]
    fn listings_are_the_same_over_any_partition_in_any_order() {
        let s = magnetar(15.0, 0.2, 2e3);
        let sr = series(5);
        let listing = |w: TimeWindow| {
            let mut out = Vec::new();
            bursts_in(&s, &sr, w, &mut out);
            out
        };
        let whole = burst_window(&s);
        let (w, cut_sets) = burst_cases(whole, &listing(whole));
        for cuts in &cut_sets {
            assert_partition_independent(w, cuts, listing);
            assert_order_independent(&partition(w, cuts), |piece| listing(*piece));
        }
        let long = window(years(-3e3), years(3e3));
        let long_cuts = vec![
            years(-1e3),
            years(0.0),
            shifted(years(17.0), 1),
            years(2_000.5),
        ];
        let episode_listing = |w: TimeWindow| {
            let mut out = Vec::new();
            episodes_in(&s, &sr, w, &mut out);
            out
        };
        let flare_listing = |w: TimeWindow| {
            let mut out = Vec::new();
            giant_flares_in(&s, &sr, w, &mut out);
            out
        };
        assert_partition_independent(long, &long_cuts, episode_listing);
        assert_order_independent(&partition(long, &long_cuts), |piece| {
            episode_listing(*piece)
        });
        assert_partition_independent(long, &long_cuts, flare_listing);
        assert_order_independent(&partition(long, &long_cuts), |piece| flare_listing(*piece));
        // At the magnetar's edges: its birth and its quiet, a nanosecond either side.
        let model = MagnetarModel::of(&s).unwrap();
        let edges = vec![
            shifted(model.birth(), -1),
            model.birth(),
            shifted(model.quiet_from(), -1),
            model.quiet_from(),
        ];
        let span = window(after(model.birth(), -YEAR), after(model.quiet_from(), YEAR));
        assert_partition_independent(span, &edges, episode_listing);
        assert_partition_independent(span, &edges, flare_listing);
    }

    /// The pieces listed on threads of their own give the same bursts. Not on wasm32-wasip1, which
    /// has no threads; the other CI architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn listings_are_the_same_on_other_threads() {
        let s = magnetar(15.0, 0.2, 2e3);
        let sr = series(5);
        let listing = |w: TimeWindow| {
            let mut out = Vec::new();
            bursts_in(&s, &sr, w, &mut out);
            out
        };
        let whole = burst_window(&s);
        let all = listing(whole);
        let (w, cut_sets) = burst_cases(whole, &all);
        for cuts in &cut_sets {
            let pieces = partition(w, cuts);
            let threaded: Vec<Vec<Burst>> = std::thread::scope(|scope| {
                let handles: Vec<_> = pieces
                    .iter()
                    .map(|&piece| scope.spawn(move || listing(piece)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(threaded.concat(), all);
        }
    }

    /// Each kind's `active_at` gives exactly the listed events running at t.
    #[test]
    fn the_active_events_are_the_listed_ones_running_then() {
        let s = magnetar(15.0, 0.2, 2e3);
        let model = MagnetarModel::of(&s).unwrap();
        let sr = series(6);
        let w = window(years(-30.0), years(30.0));
        let eps = episodes(&model, &sr, window(after(w.start(), -100.0 * DAY), w.end()));
        let bs = bursts(&model, &sr, window(UniverseTime::EPOCH, years(2.0)));
        let gs = flares(&model, &sr, window(years(-2e3), years(1e4)));
        let mut seen = [0_usize; 3];
        for i in 0..3_000 {
            let t = after(w.start(), 60.0 * YEAR * (f64::from(i) + 0.5) / 3_000.0);
            let mut got = Vec::new();
            model.episodes_active_at(&sr, t, &mut got);
            let expected: Vec<Episode> =
                eps.iter().copied().filter(|e| e.is_active_at(t)).collect();
            assert_eq!(got, expected, "{t}");
            seen[0] += got.len();
        }
        for b in bs.iter().take(200) {
            for t in [b.onset(), shifted(b.end(), -1), b.end()] {
                let mut got = Vec::new();
                model.bursts_active_at(&sr, t, &mut got);
                let expected: Vec<Burst> =
                    bs.iter().copied().filter(|x| x.is_active_at(t)).collect();
                assert_eq!(got, expected, "{t}");
                seen[1] += got.len();
            }
        }
        for g in &gs {
            for t in [g.onset(), shifted(g.end(), -1), g.end()] {
                let mut got = Vec::new();
                giant_flares_active_at(&s, &sr, t, &mut got);
                let expected: Vec<GiantFlare> =
                    gs.iter().copied().filter(|x| x.is_active_at(t)).collect();
                assert_eq!(got, expected, "{t}");
                seen[2] += got.len();
            }
        }
        assert!(seen.iter().all(|&n| n > 10), "{seen:?}");
    }

    /// The parent's no-history test for this kind: the episodes, bursts and giant flares running at
    /// +900 years and the bursts of its hour, asked cold, are the same after asking about 100
    /// earlier times.
    #[test]
    fn the_events_at_900_years_need_no_history() {
        // A magnetar born a century before the epoch, so that it is one at +900 years.
        let make = || magnetar(15.2, 0.25, 100.0);
        let probe = make();
        let model = MagnetarModel::of(&probe).unwrap();
        let base = years(900.0);
        assert!(model.is_magnetar_at(base));
        let (index, t) = (0..400)
            .flat_map(|i| (0..20).map(move |k| (i, after(base, f64::from(k) * 0.5 * YEAR))))
            .find(|&(i, t)| {
                let mut out = Vec::new();
                model.episodes_active_at(&series(i), t, &mut out);
                !out.is_empty()
            })
            .unwrap();
        let sr = series(index);
        let ask = |s: &StarModel| {
            let (mut e, mut b, mut g) = (Vec::new(), Vec::new(), Vec::new());
            episodes_active_at(s, &sr, t, &mut e);
            bursts_in(s, &sr, window(t, after(t, 3_600.0)), &mut b);
            giant_flares_active_at(s, &sr, t, &mut g);
            (e, b, g)
        };
        let cold = ask(&make());
        let s = make();
        let fresh = MagnetarModel::of(&s).unwrap();
        let mut lcg = Lcg::new(0x0928_c900);
        for _ in 0..100 {
            let earlier = after(ClockWindow::START, lcg.next_f64() * 1.899e3 * YEAR);
            assert!(earlier < t);
            let mut out = Vec::new();
            fresh.episodes_active_at(&sr, earlier, &mut out);
            let _ = bursts(&fresh, &sr, window(earlier, after(earlier, 2.0 * DAY)));
        }
        assert_eq!(ask(&s), cold);
        assert_ne!(cold.0, []);
    }

    /// The first series of `model` from `from` whose episodes in the 20 years from `first` hold
    /// two that overlap, with that pair: what the pick of a burst's episode, word 0, chooses
    /// between.
    fn overlapping_pair(
        model: &MagnetarModel<'_>,
        from: u16,
        first: UniverseTime,
    ) -> (u16, Episode, Episode) {
        (from..from + 2_000)
            .find_map(|index| {
                let eps = episodes(
                    model,
                    &series(index),
                    window(first, after(first, 20.0 * YEAR)),
                );
                eps.windows(2)
                    .find(|pair| pair[1].onset() < pair[0].end())
                    .map(|pair| (index, pair[0], pair[1]))
            })
            .unwrap()
    }

    /// Writes the bounds of the bins holding `t` of the episodes, the giant flares and the bursts.
    fn pin_bounds(
        w: &mut GoldenWriter,
        label: &str,
        model: &MagnetarModel<'_>,
        t: UniverseTime,
        bursts: &BurstRate<'_>,
    ) {
        let bin = |bins: PoissonBins| bins.bin_window(bins.bin_of(t).unwrap()).unwrap();
        w.f64(
            &format!("  episode bound at {label} (per s)"),
            EpisodeRate(*model).bound(bin(episode_bins())).value(),
        );
        w.f64(
            &format!("  giant-flare bound at {label} (per s)"),
            GiantFlareRate(*model)
                .bound(bin(giant_flare_bins()))
                .value(),
        );
        w.f64(
            &format!("  burst bound at {label} (per s)"),
            bursts.bound(bin(burst_bins())).value(),
        );
    }

    /// Pinned: three magnetars' births, critical-field crossings and rates at five instants; the
    /// bounds of the bins at the birth, the first episode's onset and the quiet; their episodes
    /// over 100 years, the bursts of the first episode's first two days and giant flares over 3,000
    /// years, each event's ID, onset and marks, on a series of each star's own; and the bursts of
    /// the 10¹⁵ G star where two episodes overlap, whose word 0 picks between them.
    #[test]
    fn magnetar_events_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        w.f64("giant flares per e-fold", GIANT_FLARES_PER_E_FOLD);
        let names = [
            "born at 10^15 G and 0.2 s 2,000 years before the epoch",
            "born at 10^14.5 G and 0.3 s 10,000 years before the epoch",
            "born at 10^15.2 G and 0.25 s 300 years after the epoch",
        ];
        for ((name, s), index) in names.iter().zip(cases()).zip([0_u16, 1, 2]) {
            let model = MagnetarModel::of(&s).unwrap();
            let sr = series(index);
            w.line("");
            w.line(&format!("# {name}, series {index}"));
            w.line(&format!(
                "birth at {}, quiet from {}",
                model.birth(),
                model.quiet_from()
            ));
            for t in [
                years(-1e3),
                years(0.0),
                years(300.5),
                years(1e3),
                years(5e3),
            ] {
                w.line(&format!("at {t}: magnetar {}", model.is_magnetar_at(t)));
                w.f64("  episodes (per s)", model.episode_rate_at(t).value());
                w.f64(
                    "  giant flares (per s)",
                    model.giant_flare_rate_at(t).value(),
                );
            }
            // Episodes from the first year of the magnetar's own (after its first stretch, while
            // its spin-down still outshines its field's decay), or from 10 years before the epoch.
            let first = after(model.birth(), YEAR).max(years(-10.0));
            let eps = episodes(&model, &sr, window(first, after(first, 100.0 * YEAR)));
            assert!(!eps.is_empty(), "{name}: no episode in 100 years");
            let reaching = episodes(
                &model,
                &sr,
                window(
                    after(eps[0].onset(), -100.0 * DAY),
                    after(eps[0].onset(), DAY),
                ),
            );
            let rate = BurstRate {
                episodes: &reaching,
            };
            for (label, t) in [
                ("the birth", model.birth()),
                ("the first episode", eps[0].onset()),
                ("the quiet", model.quiet_from()),
            ] {
                w.line(&format!("bins at {label}"));
                pin_bounds(&mut w, label, &model, t, &rate);
            }
            for e in &eps {
                w.line(&format!("episode {} at {}", e.id(), e.onset()));
                w.f64("  expected bursts", e.expected_bursts());
                w.f64("  e-folding (s)", e.e_folding_seconds());
            }
            let early = bursts(
                &model,
                &sr,
                window(eps[0].onset(), after(eps[0].onset(), 2.0 * DAY)),
            );
            assert!(
                !early.is_empty(),
                "{name}: no burst in its first episode's first days"
            );
            for b in early.iter().take(25) {
                pin_burst(&mut w, b);
            }
            for g in flares(&model, &sr, window(years(-3e3), years(3e3))) {
                w.line(&format!("giant flare {} at {}", g.id(), g.onset()));
                w.f64("  energy (J)", g.energy_joules());
            }
        }
        pin_overlap(&mut w);
        golden!("stellar/magnetars", w.as_str());
    }

    /// Writes a burst's ID, onset, episode, duration and energy.
    fn pin_burst(w: &mut GoldenWriter, b: &Burst) {
        w.line(&format!(
            "burst {} at {} of {}, lasting {}",
            b.id(),
            b.onset(),
            b.episode(),
            b.duration()
        ));
        w.f64("  energy (J)", b.energy_joules());
    }

    /// Writes the bursts of the 10¹⁵ G star where two of its episodes overlap, whose word 0 picks
    /// between them, and the bounds of the bins at the second onset.
    fn pin_overlap(w: &mut GoldenWriter) {
        let s = magnetar(15.0, 0.2, 2e3);
        let model = MagnetarModel::of(&s).unwrap();
        let (index, a, b) = overlapping_pair(&model, 10, years(-10.0));
        w.line("");
        w.line(&format!(
            "# two episodes overlapping, {} and {}, on series {index} of the 10^15 G star",
            a.id(),
            b.id()
        ));
        let sr = series(index);
        let both = bursts(&model, &sr, window(b.onset(), after(b.onset(), 0.5 * DAY)));
        assert!(both.iter().any(|x| x.episode() == b.id()));
        let reaching = episodes(
            &model,
            &sr,
            window(after(b.onset(), -100.0 * DAY), after(b.onset(), DAY)),
        );
        let rate = BurstRate {
            episodes: &reaching,
        };
        pin_bounds(w, "the second onset", &model, b.onset(), &rate);
        for x in both.iter().take(25) {
            pin_burst(w, x);
        }
    }

    /// Where two episodes overlap, the burst listings are the same over any partition, in any
    /// order, with a cut between the two onsets, and each burst is regenerated from its ID with
    /// the episodes that reach its bin.
    #[test]
    fn listings_are_the_same_where_episodes_overlap() {
        let s = magnetar(15.0, 0.2, 2e3);
        let model = MagnetarModel::of(&s).unwrap();
        let (index, a, b) = overlapping_pair(&model, 10, years(-10.0));
        let sr = series(index);
        let whole = window(after(a.onset(), -DAY), after(b.onset(), 10.0 * DAY));
        let listing = |w: TimeWindow| bursts(&model, &sr, w);
        let all = listing(whole);
        assert!(all.iter().any(|x| x.episode() == a.id()));
        assert!(all.iter().any(|x| x.episode() == b.id()));
        let cuts = vec![
            after(b.onset(), -1.0),
            shifted(b.onset(), 1),
            after(b.onset(), 3_600.0),
        ];
        assert_partition_independent(whole, &cuts, listing);
        assert_order_independent(&partition(whole, &cuts), |piece| listing(*piece));
        for x in &all {
            assert_eq!(model.burst(&sr, x.id()).as_ref(), Some(x));
        }
    }

    /// The bursts of a magnetar near the end of the 40-bit burst bins, 15,678,639 years before the
    /// epoch, list the same over a window that reaches past them as over its piece inside them.
    #[test]
    fn listings_reach_the_ends_of_the_bin_numbers() {
        // A magnetar born 61 years before the bins' lower end.
        let s = magnetar(15.0, 0.2, 15_678_700.0);
        let model = MagnetarModel::of(&s).unwrap();
        let edge = UniverseTime::new(EventBin::MIN.get() * BURST_BIN_SECONDS, 0).unwrap();
        let whole = window(after(edge, -5.0 * YEAR), after(edge, 20.0 * YEAR));
        let inside = window(edge, whole.end());
        let (index, listed) = (0..200)
            .map(|i| (i, bursts(&model, &series(i), whole)))
            .find(|(_, l)| !l.is_empty())
            .unwrap();
        assert_eq!(listed, bursts(&model, &series(index), inside));
        assert_partition_independent(whole, &[edge], |w| bursts(&model, &series(index), w));
    }

    /// The expected counts of a magnetar's episodes and giant flares over `w`: their rates'
    /// integrals, by the exact e-folds ln(B(a) ÷ B(b)) of each of `steps` equal steps whose middle
    /// is a magnetar.
    fn expected_counts(model: &MagnetarModel<'_>, w: TimeWindow, steps: u32) -> f64 {
        let span = w.end().checked_since(w.start()).unwrap().as_seconds_f64();
        let ns = model.neutron_star();
        let mut e_folds = 0.0;
        for k in 0..steps {
            let a = after(w.start(), span * f64::from(k) / f64::from(steps));
            let b = after(w.start(), span * f64::from(k + 1) / f64::from(steps));
            let middle = after(w.start(), span * (f64::from(k) + 0.5) / f64::from(steps));
            if model.is_magnetar_at(middle) {
                let field = |t: UniverseTime| {
                    ns.field_at(Years::new(model.neutron_star_age_at(t).value().max(0.0)))
                        .value()
                };
                e_folds += math::ln(field(a) / field(b));
            }
        }
        e_folds
    }

    /// The parent's statistical test for this kind: 10⁴ magnetars, ten series each of 1,000
    /// neutron stars born above the critical field (log B₀ uniform on 13.7–15.4, P₀ of the birth
    /// law, ages to 3 × 10⁵ years, from a test-local LCG), whose episodes over five years, giant flares
    /// over 300 years and bursts over 30 days number their rate integrals within Poisson
    /// intervals, the bursts' given the episodes listed.
    #[test]
    #[ignore = "slow: 1,000 tracks of 15 M☉ to their deaths, 10⁴ series"]
    fn the_mean_counts_over_10_000_magnetars_match_the_rate_integrals() {
        let mut lcg = Lcg::new(0x0628_c016);
        let life = lifetime();
        let (mut n_e, mut x_e, mut n_g, mut x_g, mut n_b, mut x_b) =
            (0_u64, 0.0, 0_u64, 0.0, 0_u64, 0.0);
        let mut magnetars = 0;
        for i in 0..1_000_u16 {
            let log_b0 = 13.7 + 1.7 * lcg.next_f64();
            let z = math::normal_quantile(open_unit(&mut lcg));
            let period = (0.25 + 0.1 * z).max(0.011);
            let age = 3e5 * lcg.next_f64();
            let s = StarModel::new(
                SolarMasses::new(PROGENITOR),
                Composition::SOLAR,
                draws(log_b0, period),
                Years::new(life + age),
            )
            .unwrap();
            let model = MagnetarModel::of(&s).unwrap();
            magnetars += u32::from(model.is_magnetar_at(UniverseTime::EPOCH));
            let year = window(UniverseTime::EPOCH, years(5.0));
            let centuries = window(UniverseTime::EPOCH, years(300.0));
            let month = window(UniverseTime::EPOCH, years(30.0 / 365.25));
            let (e_folds_year, e_folds_centuries) = (
                expected_counts(&model, year, 1_826),
                expected_counts(&model, centuries, 3_000),
            );
            for k in 0..10 {
                let sr = series(i * 10 + k);
                n_e += u64::try_from(episodes(&model, &sr, year).len()).unwrap();
                x_e += EPISODES_PER_E_FOLD * e_folds_year;
                n_g += u64::try_from(flares(&model, &sr, centuries).len()).unwrap();
                x_g += GIANT_FLARES_PER_E_FOLD * e_folds_centuries;
                let reach = episodes(
                    &model,
                    &sr,
                    window(after(month.start(), -100.0 * DAY), month.end()),
                );
                for e in &reach {
                    let tau = e.e_folding_seconds();
                    let since =
                        |t: UniverseTime| t.checked_since(e.onset()).unwrap().as_seconds_f64();
                    let a = since(month.start().max(e.onset())).max(0.0);
                    let b = since(month.end().min(e.end())).max(0.0);
                    if b > a {
                        x_b += e.expected_bursts() * (math::exp(-a / tau) - math::exp(-b / tau));
                    }
                }
                n_b += u64::try_from(bursts(&model, &sr, month).len()).unwrap();
            }
        }
        eprintln!(
            "1,000 neutron stars, {magnetars} magnetars at the epoch, 10 series each: episodes \
             {n_e} against {x_e:.1} {:?}; giant flares {n_g} against {x_g:.1} {:?}; bursts {n_b} \
             against {x_b:.1} {:?}, at alpha {ALPHA:e}",
            poisson_interval(x_e, ALPHA),
            poisson_interval(x_g, ALPHA),
            poisson_interval(x_b, ALPHA),
        );
        assert!(x_e > 500.0 && x_g > 100.0 && x_b > 1_000.0);
        assert_poisson_count("episodes of 10⁴ magnetars", n_e, x_e, ALPHA);
        assert_poisson_count("giant flares of 10⁴ magnetars", n_g, x_g, ALPHA);
        assert_poisson_count("bursts of 10⁴ magnetars", n_b, x_b, ALPHA);
    }
}
