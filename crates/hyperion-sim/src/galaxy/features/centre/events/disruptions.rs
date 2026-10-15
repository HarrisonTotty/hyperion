//! The central black hole's tidal disruptions of stars (plan 09, P09.T30.a): when they happen, at
//! Stone and Metzger's (2016) rate for the black hole's mass.
//!
//! # What a disruption is here
//!
//! An event is the disruption of a star that has entered the black hole's loss cone. Its time is
//! the disruption itself, which P09.T30.b makes its victim's pericentre passage. In a steady state
//! stars are disrupted as often as they enter the loss cone, so the events come at the rate of
//! loss-cone entries of Stone and Metzger's (2016, MNRAS 455, 859, section 4.1) equation 27,
//!
//! > Ṅ = Ṅ₀ (M• ÷ 10⁸ M☉)^B, Ṅ₀ = 2.9 × 10⁻⁵ per year per galaxy, B = −0.404,
//!
//! their fit to the whole galaxy sample, which their fiducial model uses ("our fiducial model uses
//! a power law fit to the entire galaxy sample (B = −0.404)"), at the black hole's mass, constant
//! in time and with no clamp ([`disruption_rate`]). Their black-hole masses are McConnell and Ma's
//! (2013) M–σ relation's, as this galaxy's are. At the Milky Way fixture's 4.30 × 10⁶ M☉ it is
//! 1.0339 × 10⁻⁴ a year: 0.2068 expected in the clock window ±H (2,000 years) and 27.31 in the
//! source horizon (264,144 years), against van Velzen's (2018, ApJ 852, 72) observed
//! ≈ 1 × 10⁻⁴ per galaxy per year and Yao et al.'s (2023, arXiv:2303.06523) 3.2 × 10⁻⁵ at a host
//! of 10¹⁰ M☉.
//!
//! Its validity:
//!
//! - Stone and Metzger's sample starts at about 10⁶ M☉, so the equation is extrapolated below it
//!   (their section 6), in the direction that Yao et al.'s TDE mass function down to 10^5.3 M☉
//!   (their section 6.4) supports. Below 10^5.3 M☉, which a drawn black hole reaches only beyond
//!   about 4σ of the M–σ relation's scatter, no source supports it.
//! - Above about 10^7.5 M☉ some of the stars that enter the loss cone are swallowed whole rather
//!   than disrupted (van Velzen 2018, section 3; Yao et al. 2023, section 6.4): Stone and Metzger's
//!   rule near their equation 33 (section 4.2) is "Eq. 27 if `M• < M_Hills(M★)`, but is 0
//!   otherwise", a test on the star. So every star entering the loss cone is an event here, and
//!   P09.T30.b decides for each victim whether it is disrupted or swallowed, as an outcome on the
//!   event, which moves no time and no ID of this module.
//!
//! Here a disruption is an ID and a time only. P09.T30.b gives it a victim and a luminosity.
//!
//! # Time
//!
//! Disruptions are plan 06's Poisson bins of [`BIN_SECONDS`] = 1,024 Julian years (Design note 15),
//! so that the source horizon, [−(H + L), +H], is 258 bins. A disruption is an instant, so
//! [`LOOK_BACK_BINS`] is 0; P09.T30.b sets a look-back of its own for the tails, which moves no
//! event, since a look-back changes only what an `active_at` lists. The bound is the constant
//! rate, so nothing is thinned. A bin's mean is 0.106 at the Milky Way's black hole and 0.427 at
//! 1.5 dex below it, the ±4σ of the M–σ relation's 0.38 dex scatter that the draws reach in
//! practice; the scatter's own limit, 8.58σ, puts a black hole near 4 × 10³ M☉ at the Milky Way's
//! dispersion, about 1.8 a bin. All lie far under [`PoissonBins::MAX_MEAN_PER_BIN`], which a mass
//! below about 0.56 M☉ would exceed: [`TidalDisruptionModel::new`] refuses such a mass rather than
//! clamp its rate.
//!
//! # Marks
//!
//! None are drawn. The words of a disruption's own event stream, from word 0
//! (`series.key().event_stream(bin, number)`, [`BinEvent::marks`]), are P09.T30.b's, for its
//! victim, so that T30.b moves no time and no ID.
//!
//! The rate's constants are part of the generator version (plan 09, "Generator version").

use std::error::Error;
use std::fmt;

use crate::Seed;
use crate::events::{
    BinEvent, EventId, EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow, to_nanos,
};
use crate::galaxy::Galaxy;
use crate::id::event_tags::TIDAL_DISRUPTION;
use crate::math;
use crate::time::UniverseTime;
use crate::units::SolarMasses;

/// The length of a disruption bin, s: 1,024 Julian years, 32,314,982,400 s, so that the source
/// horizon is 258 bins (plan 09, P09.T30.a).
pub const BIN_SECONDS: i64 = 1_024 * 31_557_600;

/// How many whole bins a disruption's `active_at` would look back: none, since a disruption is an
/// instant.
pub const LOOK_BACK_BINS: u32 = 0;

/// Ṅ₀, the rate of disruptions of a galaxy whose black hole is [`PIVOT_MASS`], per Julian year:
/// 2.9 × 10⁻⁵, Stone and Metzger's (2016, section 4.1) equation 27 fitted to their whole sample.
/// Part of the generator version.
pub const RATE_AT_PIVOT_PER_YEAR: f64 = 2.9e-5;

/// The pivot of equation 27, 10⁸ M☉ (Stone and Metzger 2016, section 4.1). Part of the generator
/// version.
pub const PIVOT_MASS: SolarMasses = SolarMasses::new(1e8);

/// B, the power of the black hole's mass in equation 27: −0.404, the whole sample's (Stone and
/// Metzger 2016, section 4.1; their core galaxies alone give −0.247 and their cusp galaxies alone
/// −0.223). Part of the generator version.
pub const MASS_INDEX: f64 = -0.404;

/// [`BIN_SECONDS`] as a float, exactly: 32,314,982,400 is below 2⁵³.
const BIN_SECONDS_F64: f64 = 32_314_982_400.0;

/// The disruption bins: 1,024 years long, looking back none.
#[must_use]
fn bins() -> PoissonBins {
    PoissonBins::new(BIN_SECONDS, LOOK_BACK_BINS).expect("1,024 years lies between 16 s and 2^44 s")
}

/// The rate at which stars enter the loss cone of a black hole of `black_hole_mass`: Stone and
/// Metzger's (2016, section 4.1) equation 27, Ṅ₀ (M• ÷ 10⁸ M☉)^B per Julian year, for any finite
/// positive mass and with no clamp (see the [module documentation](self)).
///
/// # Panics
///
/// In debug builds, unless the mass is finite and positive.
///
/// # Examples
///
/// About one disruption in ten thousand years at the Milky Way's black hole:
///
/// ```
/// use hyperion_sim::galaxy::features::centre::events::disruptions::disruption_rate;
/// use hyperion_sim::units::SolarMasses;
///
/// let per_year = disruption_rate(SolarMasses::new(4.3e6)).value() * 31_557_600.0;
/// assert!((per_year - 1.034e-4).abs() < 1e-7);
/// ```
#[must_use]
pub fn disruption_rate(black_hole_mass: SolarMasses) -> EventsPerSecond {
    let mass = black_hole_mass.value();
    debug_assert!(
        mass.is_finite() && mass > 0.0,
        "a black hole's mass is finite and positive, got {mass} M☉"
    );
    EventsPerSecond::per_julian_year(
        RATE_AT_PIVOT_PER_YEAR * math::powf(mass / PIVOT_MASS.value(), MASS_INDEX),
    )
}

/// A [`TidalDisruptionModel`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum BuildTidalDisruptionModelError {
    /// The black hole's mass is not finite and positive.
    InvalidMass {
        /// The mass asked for.
        mass: SolarMasses,
    },
    /// The mass is so small, below about 0.56 M☉, that equation 27 gives a bin of 1,024 years a
    /// mean count above [`PoissonBins::MAX_MEAN_PER_BIN`], which the bins cannot list.
    RateAboveBinLimit {
        /// The mass asked for.
        mass: SolarMasses,
        /// Equation 27's mean count in one bin at that mass.
        mean_per_bin: f64,
    },
}

impl fmt::Display for BuildTidalDisruptionModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMass { mass } => write!(
                f,
                "a black hole of {} solar masses has no disruption rate: the mass must be finite \
                 and positive",
                mass.value()
            ),
            Self::RateAboveBinLimit { mass, mean_per_bin } => write!(
                f,
                "a black hole of {} solar masses has {mean_per_bin} disruptions a bin of 1,024 \
                 years, above the bins' limit of {}",
                mass.value(),
                PoissonBins::MAX_MEAN_PER_BIN
            ),
        }
    }
}

impl Error for BuildTidalDisruptionModelError {}

/// The tidal disruptions of the central black hole of one universe: their series and their
/// constant rate (see the [module documentation](self)).
///
/// It is the [`RateModel`] of the black hole's `centre.ev.tidal_disruption` series, whose bound
/// and rate are both [`disruption_rate`] at the black hole's mass at every instant. Built from its
/// parts, the seed and the mass, so that a test can sweep masses without solving a galaxy's
/// parameters; [`of`](Self::of) reads both from a galaxy.
///
/// # Examples
///
/// The disruptions expected in the source horizon at the Milky Way's black hole:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::features::centre::events::TidalDisruptionModel;
/// use hyperion_sim::time::SourceHorizon;
/// use hyperion_sim::units::SolarMasses;
///
/// let model = TidalDisruptionModel::new(Seed::new(7), SolarMasses::new(4.3e6))?;
/// let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END)?;
/// assert!((model.expected_count(horizon) - 27.31).abs() < 0.01);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct TidalDisruptionModel {
    series: EventSeries,
    black_hole_mass: SolarMasses,
    rate: EventsPerSecond,
}

impl TidalDisruptionModel {
    /// The disruptions of a black hole of `black_hole_mass` in the universe of `seed`: the series of
    /// [`TIDAL_DISRUPTION`] with the black hole's own ID as subject (Design note 23).
    ///
    /// # Errors
    ///
    /// [`BuildTidalDisruptionModelError::InvalidMass`] unless the mass is finite and positive;
    /// [`BuildTidalDisruptionModelError::RateAboveBinLimit`] for a mass below about 0.56 M☉,
    /// whose bins' mean count would exceed [`PoissonBins::MAX_MEAN_PER_BIN`]. No galaxy the
    /// builder allows has one.
    pub fn new(
        seed: Seed,
        black_hole_mass: SolarMasses,
    ) -> Result<Self, BuildTidalDisruptionModelError> {
        let mass = black_hole_mass.value();
        if !(mass.is_finite() && mass > 0.0) {
            return Err(BuildTidalDisruptionModelError::InvalidMass {
                mass: black_hole_mass,
            });
        }
        let rate = disruption_rate(black_hole_mass);
        let mean_per_bin = rate.value() * BIN_SECONDS_F64;
        if mean_per_bin > PoissonBins::MAX_MEAN_PER_BIN {
            return Err(BuildTidalDisruptionModelError::RateAboveBinLimit {
                mass: black_hole_mass,
                mean_per_bin,
            });
        }
        Ok(Self {
            series: EventSeries::new(seed, TIDAL_DISRUPTION, super::subject()),
            black_hole_mass,
            rate,
        })
    }

    /// The disruptions of `galaxy`'s central black hole: [`new`](Self::new) at the galaxy's seed and
    /// its black hole's mass, as the centre's member 0 reads it (solved on the first read of the
    /// parameters' black hole, about 100 ms).
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new), which no galaxy the builder allows reaches.
    pub fn of(galaxy: &Galaxy) -> Result<Self, BuildTidalDisruptionModelError> {
        Self::new(galaxy.seed(), galaxy.params().black_hole().mass())
    }

    /// The series: the black hole's subject, [`TIDAL_DISRUPTION`] and their event key.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The black hole's mass.
    #[must_use]
    pub const fn black_hole_mass(&self) -> SolarMasses {
        self.black_hole_mass
    }

    /// The rate of disruptions, in events per second: [`disruption_rate`] at the black hole's mass,
    /// the same at every instant.
    #[must_use]
    pub const fn listed_rate(&self) -> EventsPerSecond {
        self.rate
    }

    /// The number of disruptions expected in `window`: the rate times the window's span. A rate
    /// times a span, not a draw.
    #[must_use]
    pub fn expected_count(&self, window: TimeWindow) -> f64 {
        // In 128-bit nanoseconds, which no window of the clock overflows.
        let nanos = to_nanos(window.end()) - to_nanos(window.start());
        #[expect(
            clippy::cast_precision_loss,
            reason = "a span of at most 2^64 s in nanoseconds, to f64's relative 1.1e-16"
        )]
        let seconds = nanos as f64 / 1e9;
        self.rate.value() * seconds
    }
}

impl RateModel for TidalDisruptionModel {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        self.rate
    }

    fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
        self.rate
    }
}

/// One tidal disruption of the central black hole: its ID and the instant of the disruption (see
/// the [module documentation](self)).
///
/// P09.T30.b gives it its victim, from the words of its own event stream, whose pericentre passage
/// is this instant, and its luminosity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TidalDisruption {
    id: EventId,
    time: UniverseTime,
}

impl TidalDisruption {
    /// The disruption's ID, from which [`tidal_disruption`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The instant of the disruption.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }
}

/// The disruption of `event`: its ID and its time, no mark drawn.
#[must_use]
fn disruption_of(event: &BinEvent) -> TidalDisruption {
    TidalDisruption {
        id: event.id(),
        time: event.time(),
    }
}

/// Appends to `out` the black hole's disruptions in `window`, sorted by time and then by ID.
///
/// Every bin the window touches is generated whole, so a listing over a window is the union of the
/// listings over any partition of it, in any order of asking. `out` is only appended to; nothing in
/// it is read.
///
/// # Examples
///
/// The disruptions in the source horizon at the Milky Way fixture's black hole, and one of them
/// regenerated from its ID alone:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::features::centre::events::{
///     TidalDisruptionModel, tidal_disruption, tidal_disruptions,
/// };
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::time::SourceHorizon;
///
/// let mass = GalaxyParams::milky_way_like().black_hole().mass();
/// let model = TidalDisruptionModel::new(Seed::new(7), mass)?;
/// let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END)?;
/// let mut events = Vec::new();
/// tidal_disruptions(&model, horizon, &mut events);
/// // About 27 expected.
/// assert!((8..60).contains(&events.len()));
/// let last = events.last().ok_or("the horizon holds a disruption")?;
/// assert_eq!(tidal_disruption(&model, last.id()).as_ref(), Some(last));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn tidal_disruptions(
    model: &TidalDisruptionModel,
    window: TimeWindow,
    out: &mut Vec<TidalDisruption>,
) {
    let mut events = Vec::new();
    bins().events_in(model.series(), window, model, &mut events);
    out.extend(events.iter().map(disruption_of));
}

/// The disruption `id` names, regenerated on its own.
///
/// `None` if `id` is not of the model's series (another event tag or another subject) or names a
/// number beyond its bin's count.
#[must_use]
pub fn tidal_disruption(model: &TidalDisruptionModel, id: EventId) -> Option<TidalDisruption> {
    bins()
        .event(model.series(), id, model)
        .as_ref()
        .map(disruption_of)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_poisson_count, poisson_interval};

    use super::super::{milky_way_mass, subject};
    use super::*;
    use crate::events::testing::{assert_partition_independent, partition};
    use crate::events::{EventBin, event_id};
    use crate::id::{SystemId, event_tags};
    use crate::time::{ClockWindow, SourceHorizon, Span};

    const SEED: Seed = Seed::new(0x0e7e_0009_0030_000b);

    /// [`BIN_SECONDS`] as a float, exactly.
    const BIN: f64 = 32_314_982_400.0;

    fn horizon() -> TimeWindow {
        TimeWindow::new(SourceHorizon::START, SourceHorizon::END).unwrap()
    }

    fn clock_window() -> TimeWindow {
        TimeWindow::new(ClockWindow::START, ClockWindow::END).unwrap()
    }

    fn model(mass: f64) -> TidalDisruptionModel {
        TidalDisruptionModel::new(SEED, SolarMasses::new(mass)).unwrap()
    }

    fn list(model: &TidalDisruptionModel, w: TimeWindow) -> Vec<TidalDisruption> {
        let mut out = Vec::new();
        tidal_disruptions(model, w, &mut out);
        out
    }

    /// Equation 27 written out again, per Julian year, as exp(B ln(M ÷ 10⁸)).
    fn equation_27(mass: f64) -> f64 {
        2.9e-5 * math::exp(-0.404 * math::ln(mass / 1e8))
    }

    /// The source horizon is 258 bins of 1,024 years, and 1,024 years is 32,314,982,400 s.
    #[test]
    fn the_source_horizon_is_258_bins() {
        assert_eq!(BIN_SECONDS, 32_314_982_400);
        assert_eq!(BIN_SECONDS, 1_024 * 31_557_600);
        let bins = bins();
        let first = bins.bin_of(SourceHorizon::START).unwrap().get();
        let last = bins
            .bin_of(UniverseTime::new(SourceHorizon::END.seconds() - 1, 0).unwrap())
            .unwrap()
            .get();
        assert_eq!((first, last), (-257, 0));
        assert_eq!(last - first + 1, 258);
    }

    /// The plan's figures, as §3.5 item 1 of issue #20's brief corrects them, at the Milky Way
    /// fixture's mass: 0.2068 expected in ±H and 27.31 in the source horizon, each equation 27
    /// times the span, and ±H's inside the corrected window 0.06–0.21.
    #[test]
    fn the_milky_way_expects_0_2068_in_the_clock_window_and_27_31_in_the_horizon() {
        let mass = milky_way_mass();
        let model = TidalDisruptionModel::new(SEED, mass).unwrap();
        assert_eq!(model.black_hole_mass(), mass);
        let per_year = equation_27(mass.value());
        let in_clock = model.expected_count(clock_window());
        let in_horizon = model.expected_count(horizon());
        assert!(
            (in_clock / (per_year * 2_000.0) - 1.0).abs() < 1e-12,
            "{in_clock}"
        );
        assert!(
            (in_horizon / (per_year * 264_144.0) - 1.0).abs() < 1e-12,
            "{in_horizon}"
        );
        assert!((in_clock - 0.2068).abs() < 5e-5, "{in_clock} in ±H");
        assert!(
            (in_horizon - 27.31).abs() < 5e-3,
            "{in_horizon} in the horizon"
        );
        assert!((0.06..=0.21).contains(&in_clock), "{in_clock}");
        let per_bin = model.listed_rate().value() * BIN;
        assert!((per_bin - 0.1059).abs() < 5e-5, "{per_bin} a bin");
        eprintln!(
            "the fixture's black hole, {:.6e} M☉: {per_year:.6e} a year, {in_clock:.5} in ±H, \
             {in_horizon:.4} in the horizon, {per_bin:.5} a bin",
            mass.value()
        );
    }

    /// The bound over black-hole masses from 10⁵ to 10^8.5 M☉, beyond the 1.5 dex either side of
    /// 4.3 × 10⁶ that the draws reach in practice (±4σ of the M–σ scatter, as `centre`'s
    /// `no_drawn_centre_fails_its_inversion` takes it): the rate never exceeds the bound, a bin's
    /// mean stays under 64, and the means are the brief's 0.427 (−1.5 dex), 0.484 (10⁵ M☉, the
    /// largest) and 0.0187 (10^8.5 M☉). Below 10⁶ M☉ equation 27 is an extrapolation (SM16 §6).
    #[test]
    fn the_bound_holds_from_1e5_to_10_to_the_8_5_solar_masses() {
        let bins = bins();
        let probes: Vec<TimeWindow> = [
            SourceHorizon::START,
            ClockWindow::START,
            UniverseTime::EPOCH,
            ClockWindow::END,
        ]
        .into_iter()
        .map(|t| bins.bin_window(bins.bin_of(t).unwrap()).unwrap())
        .collect();
        let mut largest: f64 = 0.0;
        for step in 0..=70 {
            let mass = math::exp10(5.0 + 0.05 * f64::from(step));
            let model = model(mass);
            for &window in &probes {
                let bound = model.bound(window);
                let mean = bound.value() * BIN;
                assert!(mean <= PoissonBins::MAX_MEAN_PER_BIN, "{mass:e} M☉");
                largest = largest.max(mean);
                for share in [0.0, 0.3, 0.999_999] {
                    let t = window
                        .start()
                        .checked_add(Span::from_seconds_f64(share * 3.2e10).unwrap())
                        .unwrap();
                    assert!(model.rate(t) <= bound, "{mass:e} M☉ at {t}");
                }
            }
        }
        assert!((largest - 0.484).abs() < 5e-4, "{largest}");
        let low = model(4.3e6 * math::exp10(-1.5)).listed_rate().value() * BIN;
        assert!((low - 0.427).abs() < 5e-4, "{low}");
        let high = model(math::exp10(8.5)).listed_rate().value() * BIN;
        assert!((high - 0.0187).abs() < 5e-5, "{high}");
    }

    /// A mass below about 0.56 M☉, whose bins' mean would exceed 64, is refused, not clamped; one
    /// solar mass, at 50.7 a bin, is not.
    #[test]
    fn a_mass_whose_bins_would_exceed_64_is_refused() {
        let at_one = model(1.0).listed_rate().value() * BIN;
        assert!((at_one - 50.7).abs() < 0.05, "{at_one} a bin at 1 M☉");
        match TidalDisruptionModel::new(SEED, SolarMasses::new(0.5)) {
            Err(BuildTidalDisruptionModelError::RateAboveBinLimit { mass, mean_per_bin }) => {
                assert_same_bits(mass.value(), 0.5);
                assert!(mean_per_bin > 64.0 && mean_per_bin < 70.0, "{mean_per_bin}");
            }
            other => panic!("{other:?}"),
        }
        // The edge, where the mean is exactly 64: (64 ÷ (2.9 × 10⁻⁵ × 1,024))^(1 ÷ B) × 10⁸ M☉.
        let edge = 1e8 * math::powf(64.0 / (2.9e-5 * 1_024.0), 1.0 / -0.404);
        assert!((edge - 0.56).abs() < 0.01, "{edge} M☉");
        assert!(TidalDisruptionModel::new(SEED, SolarMasses::new(edge * 1.001)).is_ok());
        assert!(TidalDisruptionModel::new(SEED, SolarMasses::new(edge * 0.999)).is_err());
        assert_eq!(
            BuildTidalDisruptionModelError::RateAboveBinLimit {
                mass: SolarMasses::new(0.5),
                mean_per_bin: 66.5,
            }
            .to_string(),
            "a black hole of 0.5 solar masses has 66.5 disruptions a bin of 1,024 years, above the \
             bins' limit of 64"
        );
    }

    /// A mass that is not finite and positive has no rate.
    #[test]
    fn a_mass_that_is_not_finite_and_positive_is_rejected() {
        for mass in [0.0, -4.3e6, f64::NAN, f64::INFINITY] {
            let mass = SolarMasses::new(mass);
            match TidalDisruptionModel::new(SEED, mass) {
                Err(BuildTidalDisruptionModelError::InvalidMass { mass: got }) => {
                    if mass.value().is_nan() {
                        assert!(got.value().is_nan());
                    } else {
                        assert_same_bits(got.value(), mass.value());
                    }
                }
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(
            BuildTidalDisruptionModelError::InvalidMass {
                mass: SolarMasses::new(0.0)
            }
            .to_string(),
            "a black hole of 0 solar masses has no disruption rate: the mass must be finite and \
             positive"
        );
    }

    /// The cut sets of the partition tests over the source horizon: bin edges, inside bins, an
    /// event's instant and a nanosecond after another, and the epoch.
    fn cut_sets(events: &[TidalDisruption]) -> Vec<Vec<UniverseTime>> {
        let year = |y: i64| UniverseTime::from_julian_years(y).unwrap();
        let edge = |k: i64| UniverseTime::new(k * BIN_SECONDS, 0).unwrap();
        vec![
            vec![UniverseTime::EPOCH],
            vec![
                edge(-200),
                edge(-200),
                edge(-3),
                year(-1_000),
                ClockWindow::START,
            ],
            vec![
                events[3].time(),
                events[4]
                    .time()
                    .checked_add(Span::new(0, 1).unwrap())
                    .unwrap(),
            ],
            // Every bin edge inside the horizon, each moved on by a few days.
            (-256..=0)
                .map(|k| edge(k).checked_add(Span::from_seconds(17 * k * k)).unwrap())
                .collect(),
        ]
    }

    /// The brainstorm's time test through the public function: the listing over the source
    /// horizon is the union of the listings over any partition of it, in any order of asking.
    #[test]
    fn listings_are_the_same_over_any_partition_in_any_order() {
        let model = model(4.3e6);
        let listing = |w: TimeWindow| list(&model, w);
        let events = listing(horizon());
        assert!(events.len() > 10, "{} disruptions", events.len());
        for cuts in &cut_sets(&events) {
            assert_partition_independent(horizon(), cuts, listing);
            assert_order_independent(&partition(horizon(), cuts), |w| listing(*w));
        }
    }

    /// The pieces listed on threads of their own give the same disruptions. Not on wasm32-wasip1,
    /// which has no threads; the other CI architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn listings_are_the_same_on_other_threads() {
        let model = model(4.3e6);
        let listing = |w: TimeWindow| list(&model, w);
        let events = listing(horizon());
        for cuts in &cut_sets(&events) {
            let pieces = partition(horizon(), cuts);
            let threaded: Vec<Vec<TidalDisruption>> = std::thread::scope(|scope| {
                let handles: Vec<_> = pieces
                    .iter()
                    .map(|&w| scope.spawn(move || listing(w)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(threaded.concat(), events);
        }
    }

    /// `tidal_disruption(id)` regenerates every listed disruption and returns `None` for a number
    /// beyond its bin's count and for an ID of another series, the other tag or another subject;
    /// the listing is the bins' own events, with no mark drawn.
    #[test]
    fn an_id_regenerates_its_disruption_and_names_nothing_else() {
        let model = model(1e5);
        let events = list(&model, horizon());
        let mut bare = Vec::new();
        bins().events_in(model.series(), horizon(), &model, &mut bare);
        assert_eq!(events, bare.iter().map(disruption_of).collect::<Vec<_>>());
        let mut beyond = 0;
        for k in -257..=0 {
            for j in 0..8 {
                let id = model.series().event_id(EventBin::new(k).unwrap(), j);
                let found = events.iter().find(|e| e.id() == id);
                assert_eq!(tidal_disruption(&model, id).as_ref(), found, "bin {k}, {j}");
                if found.is_none() {
                    beyond += 1;
                }
            }
        }
        assert!(beyond > 1_000, "{beyond} beyond the counts");
        let word = events[0].id().word();
        let other_member = SystemId::from_raw(0xF000_0007_0000_0001).unwrap();
        for foreign in [
            event_id(
                subject(),
                event_tags::CENTRE_FLARE,
                word.bin(),
                word.number(),
            ),
            event_id(
                other_member.into(),
                TIDAL_DISRUPTION,
                word.bin(),
                word.number(),
            ),
        ] {
            assert_eq!(tidal_disruption(&model, foreign), None);
        }
    }

    /// No history: the disruptions of the bin holding +900 years, asked cold, are the same after
    /// asking about 100 earlier windows. At one solar mass, 50.7 a bin, so that the bin is full.
    #[test]
    fn the_disruptions_at_900_years_need_no_history() {
        let mass = 1.0;
        let bins = bins();
        let bin = bins
            .bin_of(UniverseTime::from_julian_years(900).unwrap())
            .unwrap();
        let window = bins.bin_window(bin).unwrap();
        let cold = list(&model(mass), window);
        let model = model(mass);
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x0930_b900);
        for _ in 0..100 {
            let start = SourceHorizon::START
                .checked_add(Span::from_seconds_f64(lcg.next_f64() * 8.0e12).unwrap())
                .unwrap();
            let end = start.checked_add(Span::from_seconds(BIN_SECONDS)).unwrap();
            let _ = list(&model, TimeWindow::new(start, end).unwrap());
        }
        assert_eq!(list(&model, window), cold);
        assert!(cold.len() > 20, "{} disruptions", cold.len());
        assert_eq!(
            cold,
            list(&model, horizon())
                .into_iter()
                .filter(|e| window.contains(e.time()))
                .collect::<Vec<_>>()
        );
    }

    /// The same seed and mass give the same disruptions; another seed gives others, and another
    /// mass the same bins at another mean.
    #[test]
    fn the_same_seed_and_mass_give_the_same_disruptions() {
        let a = list(&model(4.3e6), horizon());
        assert_eq!(list(&model(4.3e6), horizon()), a);
        let other = TidalDisruptionModel::new(Seed::new(1), SolarMasses::new(4.3e6)).unwrap();
        assert_ne!(list(&other, horizon()), a);
    }

    /// The rate integral: over 10⁴ seeds at the fixture's mass and at two others, the disruptions
    /// listed in the source horizon number 10⁴ times the rate times its span, within a Poisson
    /// interval. About 1 s in a debug build, so not marked slow.
    #[test]
    fn ten_thousand_seeds_horizons_match_the_rate_integral() {
        const SEEDS: u32 = 10_000;
        let mut out = Vec::new();
        for (i, mass) in [milky_way_mass().value(), 1e5, math::exp10(8.5)]
            .into_iter()
            .enumerate()
        {
            let mut counted = 0_u64;
            let mut expected = 0.0;
            for n in 0..SEEDS {
                let seed = 0x0930_b000_0000_0000 | (u64::try_from(i).unwrap() << 32) | u64::from(n);
                let model =
                    TidalDisruptionModel::new(Seed::new(seed), SolarMasses::new(mass)).unwrap();
                out.clear();
                tidal_disruptions(&model, horizon(), &mut out);
                counted += u64::try_from(out.len()).unwrap();
                expected += model.expected_count(horizon());
            }
            eprintln!(
                "{SEEDS} seeds at {mass:.4e} M☉ over the source horizon: expected {expected:.2}, \
                 counted {counted}, interval {:?} at alpha {ALPHA:e}",
                poisson_interval(expected, ALPHA)
            );
            assert_poisson_count(
                &format!("disruptions of 10⁴ seeds at {mass:e} M☉"),
                counted,
                expected,
                ALPHA,
            );
            let per_year = equation_27(mass);
            let integral = f64::from(SEEDS) * per_year * 264_144.0;
            assert!(
                (expected / integral - 1.0).abs() < 1e-9,
                "{expected} against {integral}"
            );
        }
    }
}
