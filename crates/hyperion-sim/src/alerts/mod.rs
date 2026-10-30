//! Alerts: transients seen from a distance, each in the window its light arrives in (plan 12,
//! P12.T5; brainstorm, "Events in time" and "Alerts").
//!
//! "What went off?" walks the catalogue classes within a sensor's horizon and evaluates each host
//! over the retarded interval, under the census rule: complete per class or nothing. This module
//! holds what every part of that walk shares (P12.T5.a):
//!
//! - the bands a detection is made in ([`AlertBand`]) and a sensor's two flux limits in each
//!   ([`SensorHorizon`], [`BandLimits`]);
//! - the query ([`AlertQuery`]), the classes it asks about ([`ClassSet`]), the caller's caches and
//!   sources ([`AlertContext`]), and what a walk returns ([`AlertResult`]: each [`Detection`] and a
//!   [`ClassCensus`] per class);
//! - the registry ([`registry`]): a row per [`TransientKind`] with its classes, the bound on its
//!   peak luminosity in each band, its longest duration and its light curve ([`TransientRow`]);
//! - reach and flux ([`reach`](mod@reach)): how far a sensor can see a class (Design note 5),
//!   the flux at the observer with plan 07's extinction, and the X-ray band's optical depth
//!   (Design note 14);
//! - a source's light in each band ([`light`]): the bands' top hats, a blackbody's share of its
//!   light in a band, and plan 09's supernova templates band by band (P12.T5.b).
//!
//! The walk itself belongs to the later subtasks: the one-shot classes (P12.T5.b), the recurrent
//! ones (P12.T5.c), the census with the feature-level and global lists (P12.T5.d) and the local
//! events of a range query's systems (P12.T5.e). They build the [`Detection`]s, the
//! [`ClassCensus`] and the [`AlertResult`], which have no public constructor. Nothing here draws a
//! random number or changes generated output: alerts only read.

use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;

use crate::coords::GalacticPosition;
use crate::events::TimeWindow;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::{ALL_CLASSES, CatalogueCellKey, ClassId};
use crate::galaxy::gas::ccm::Band;
use crate::galaxy::gas::extinction::{NoiseMode, SIGHTLINE_QUALITY, Sightline, sightline};
use crate::galaxy::gas::modifiers::{GasModifier, GasModifierSource};
use crate::galaxy::gas::noise::NoiseCache;
use crate::id::{EventId, SystemId};
use crate::observe::{Bearing, Retardation};
use crate::time::{ClockWindow, UniverseTime};
use crate::units::{LightYears, WattsPerSquareMetre};

pub mod light;
pub mod oneshot;
pub mod reach;
pub mod registry;

pub use light::{band_share, supernova_luminosity, supernova_peak};
pub use oneshot::{NoOneShotCells, OneShotCells, OneShotClass, OneShotEvent, one_shot_detections};
pub use reach::{
    ROOT_CUBE_DIAGONAL, class_reach, flux, inverse_square_flux, kind_reach, reach, transmission,
    xray_optical_depth,
};
pub use registry::{
    CORE_COLLAPSE_BAND_PEAKS, CORE_COLLAPSE_PEAK, GIANT_ERUPTION_PEAK, LightCurveSource,
    LongestDuration, MAGNETAR_BURST_PEAK, PeakBound, Route, TYPE_IA_BAND_PEAKS, TYPE_IA_PEAK,
    TransientKind, TransientRow,
};

/// How many bands there are: plan 07's ten photometric bands and X-rays.
const BAND_COUNT: usize = 11;

/// The band a detection is made in (plan 12's Provides; Design note 14).
///
/// Photometric bands are plan 07's [`Band`], from U to radio. X-rays are a band of their own here
/// and not a value of [`Band`]: plan 07's extinction runs through the Cardelli law, which stops at
/// 0.1 µm, while X-rays are absorbed photoelectrically by the metals whose column the dust traces,
/// which [`xray_optical_depth`] models from `A_V`.
///
/// A band's luminosity, and a sensor's limit in it, are in-band powers (P12.T5.b): the light
/// through a top hat of the band's full width at half maximum about plan 07's
/// [`Band::wavelength`], Bessell's (2005) widths for U to I and Tokunaga's (2000) for J to K and
/// the mid infrared's N band, and the light between 2 and 10 keV in X-rays. Radio takes no thermal
/// light ([`light`] has the widths and their sources).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AlertBand {
    /// One of plan 07's photometric bands.
    Photometric(Band),
    /// X-rays, the 2–10 keV band.
    XRay,
}

impl AlertBand {
    /// Every band: plan 07's photometric bands in their order ([`Band::ALL`]), then X-rays.
    pub const ALL: [Self; BAND_COUNT] = [
        Self::Photometric(Band::U),
        Self::Photometric(Band::B),
        Self::Photometric(Band::V),
        Self::Photometric(Band::R),
        Self::Photometric(Band::I),
        Self::Photometric(Band::J),
        Self::Photometric(Band::H),
        Self::Photometric(Band::K),
        Self::Photometric(Band::MidInfrared),
        Self::Photometric(Band::Radio),
        Self::XRay,
    ];

    /// The band's place in [`ALL`](Self::ALL).
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::Photometric(band) => match band {
                Band::U => 0,
                Band::B => 1,
                Band::V => 2,
                Band::R => 3,
                Band::I => 4,
                Band::J => 5,
                Band::H => 6,
                Band::K => 7,
                Band::MidInfrared => 8,
                Band::Radio => 9,
            },
            Self::XRay => 10,
        }
    }
}

/// A sensor's two flux limits in one band (plan 12's Provides; Design note 10): the faintest flux
/// it detects, and the faintest at which it resolves the host, which is no fainter.
///
/// A flux here is the energy flux in the band at the observer, W m⁻², after extinction: the
/// in-band power through the band's top hat ([`AlertBand`]).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BandLimits {
    detect: WattsPerSquareMetre,
    resolve: WattsPerSquareMetre,
}

impl BandLimits {
    /// The limits `detect` and `resolve`.
    ///
    /// # Errors
    ///
    /// [`BuildBandLimitsError::NotPositive`] unless both are positive and finite, and
    /// [`BuildBandLimitsError::ResolveBelowDetect`] if `resolve` is fainter than `detect`: a host
    /// is resolved only once its event is detected.
    pub fn new(
        detect: WattsPerSquareMetre,
        resolve: WattsPerSquareMetre,
    ) -> Result<Self, BuildBandLimitsError> {
        let positive = |flux: WattsPerSquareMetre| flux.value().is_finite() && flux.value() > 0.0;
        if !positive(detect) || !positive(resolve) {
            return Err(BuildBandLimitsError::NotPositive);
        }
        if resolve < detect {
            return Err(BuildBandLimitsError::ResolveBelowDetect);
        }
        Ok(Self { detect, resolve })
    }

    /// The faintest flux the sensor detects: an event under it is dropped (Design note 5).
    #[must_use]
    pub const fn detect(&self) -> WattsPerSquareMetre {
        self.detect
    }

    /// The faintest flux at which the sensor resolves the host, at or above
    /// [`detect`](Self::detect): a sighting at it makes its contact resolved (Design note 10).
    #[must_use]
    pub const fn resolve(&self) -> WattsPerSquareMetre {
        self.resolve
    }
}

/// [`BandLimits::new`] was given limits no sensor has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildBandLimitsError {
    /// A limit is zero, negative or not finite.
    NotPositive,
    /// The resolve limit is fainter than the detection limit.
    ResolveBelowDetect,
}

impl fmt::Display for BuildBandLimitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPositive => f.write_str("a sensor's flux limit is not positive and finite"),
            Self::ResolveBelowDetect => {
                f.write_str("a sensor's resolve limit is fainter than its detection limit")
            }
        }
    }
}

impl Error for BuildBandLimitsError {}

/// What a sensor can see: its [`BandLimits`] in each band it has (plan 12's Provides).
///
/// A sensor here is only these limits (the plan's non-goals): apertures, integration and sky
/// background belong to the sensor consoles.
///
/// # Examples
///
/// A sensor with a visual band and an X-ray band, and the reach it gives the central black
/// hole's flares:
///
/// ```
/// use hyperion_sim::alerts::{AlertBand, BandLimits, SensorHorizon, TransientKind, kind_reach};
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let w = WattsPerSquareMetre::new;
/// let sensor = SensorHorizon::new([
///     (AlertBand::Photometric(Band::V), BandLimits::new(w(1e-12), w(1e-10))?),
///     (AlertBand::XRay, BandLimits::new(w(3.6e-13), w(3.6e-11))?),
/// ])?;
/// assert!(sensor.limits(AlertBand::Photometric(Band::K)).is_none());
/// // The flares are X-rays only, so the X-ray limit alone sets their reach: about 9,420 ly.
/// let reach = kind_reach(TransientKind::CentralFlare, &sensor);
/// assert!((reach.value() - 9_424.0).abs() < 1.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct SensorHorizon {
    /// Per band, in [`AlertBand::ALL`]'s order.
    limits: [Option<BandLimits>; BAND_COUNT],
}

impl SensorHorizon {
    /// The sensor with `bands`' limits.
    ///
    /// # Errors
    ///
    /// [`BuildSensorHorizonError::NoBand`] if `bands` is empty, and
    /// [`BuildSensorHorizonError::BandTwice`] if it names a band twice.
    pub fn new(
        bands: impl IntoIterator<Item = (AlertBand, BandLimits)>,
    ) -> Result<Self, BuildSensorHorizonError> {
        let mut limits = [None; BAND_COUNT];
        for (band, band_limits) in bands {
            let slot = &mut limits[band.index()];
            if slot.is_some() {
                return Err(BuildSensorHorizonError::BandTwice(band));
            }
            *slot = Some(band_limits);
        }
        if limits.iter().all(Option::is_none) {
            return Err(BuildSensorHorizonError::NoBand);
        }
        Ok(Self { limits })
    }

    /// The sensor's limits in `band`, or `None` for a band it does not have.
    #[must_use]
    pub const fn limits(&self, band: AlertBand) -> Option<BandLimits> {
        self.limits[band.index()]
    }

    /// The sensor's bands with their limits, in [`AlertBand::ALL`]'s order.
    pub fn bands(&self) -> impl Iterator<Item = (AlertBand, BandLimits)> {
        AlertBand::ALL
            .into_iter()
            .zip(self.limits)
            .filter_map(|(band, limits)| limits.map(|limits| (band, limits)))
    }
}

/// [`SensorHorizon::new`] was given bands no sensor has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildSensorHorizonError {
    /// No band was given: the sensor would see nothing.
    NoBand,
    /// The band was given twice.
    BandTwice(AlertBand),
}

impl fmt::Display for BuildSensorHorizonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBand => f.write_str("a sensor has no band"),
            Self::BandTwice(band) => write!(f, "a sensor's band {band:?} is given twice"),
        }
    }
}

impl Error for BuildSensorHorizonError {}

/// A set of catalogue classes: those an [`AlertQuery`] asks about, or the hosts of a
/// [`TransientKind`].
///
/// A class value is below 64 ([`ClassId::LIMIT`]), so the set is one bit per value.
///
/// # Examples
///
/// ```
/// use hyperion_sim::alerts::ClassSet;
/// use hyperion_sim::galaxy::catalogue_classes::ClassId;
///
/// let novae = ClassSet::of(ClassId::ACCRETING_WHITE_DWARF)
///     .with(ClassId::ACCRETING_WHITE_DWARF_SLOW);
/// assert!(novae.contains(ClassId::ACCRETING_WHITE_DWARF_SLOW));
/// assert!(!novae.contains(ClassId::TYPE_IA));
/// assert_eq!(novae.iter().count(), 2);
/// assert_eq!(ClassSet::ALL.iter().count(), 9);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassSet(u64);

impl ClassSet {
    /// No class.
    pub const EMPTY: Self = Self(0);

    /// Every registered class ([`ALL_CLASSES`]).
    pub const ALL: Self = Self::of_all(&ALL_CLASSES);

    /// The set of `class` alone.
    #[must_use]
    pub const fn of(class: ClassId) -> Self {
        Self(1 << class.value())
    }

    /// This set with `class` added.
    #[must_use]
    pub const fn with(self, class: ClassId) -> Self {
        Self(self.0 | Self::of(class).0)
    }

    /// Whether `class` is in the set.
    #[must_use]
    pub const fn contains(self, class: ClassId) -> bool {
        self.0 & Self::of(class).0 != 0
    }

    /// Whether the set holds no class.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The classes in the set, in value order.
    pub fn iter(self) -> impl Iterator<Item = ClassId> {
        ALL_CLASSES
            .into_iter()
            .filter(move |&class| self.contains(class))
    }

    /// The set of every class in `classes`.
    #[must_use]
    const fn of_all(classes: &[ClassId]) -> Self {
        let mut set = Self::EMPTY;
        let mut i = 0;
        while i < classes.len() {
            set = set.with(classes[i]);
            i += 1;
        }
        set
    }
}

impl FromIterator<ClassId> for ClassSet {
    fn from_iter<I: IntoIterator<Item = ClassId>>(classes: I) -> Self {
        classes.into_iter().fold(Self::EMPTY, Self::with)
    }
}

/// Above this many expected hosts in its reach a class is not walked interactively: `alerts_in`
/// reports it [`ClassStatus::NeedsBackgroundScan`], and the caller drives the background scan
/// (Design note 7): 10⁵.
pub const BACKGROUND_THRESHOLD_HOSTS: f64 = 1e5;

/// The [`AlertQuery`]'s host limit unless the caller sets one: 2²⁴, 16,777,216, above the whole
/// galaxy's accreting white dwarfs, the largest class (6–12 × 10⁶ from the measured local density,
/// Pala et al. 2020, MNRAS 494, 3799, as the brainstorm's "Events in time" takes it),
/// so that by default only [`BACKGROUND_THRESHOLD_HOSTS`] parts the interactive classes from the
/// background ones and a galaxy-wide background scan is not refused (provisional; plan 12's Risks,
/// "P12.T5.a as built").
pub const DEFAULT_HOST_LIMIT: NonZeroU32 = NonZeroU32::new(1 << 24).expect("2^24 is not zero");

/// What went off, asked of a sensor at one place over one window of its time (plan 12's
/// Provides).
///
/// An event belongs to the window its light arrives in (Design note 6), so a window split in two
/// returns each onset of the whole once, in the half its light arrives in; each half also reports
/// in progress the events still above the limit at its own start, so the first half's events in
/// progress are the whole's.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertQuery {
    observer_position: GalacticPosition,
    window: TimeWindow,
    sensor: SensorHorizon,
    classes: ClassSet,
    host_limit: NonZeroU32,
}

impl AlertQuery {
    /// A query for the sensor `sensor` at `observer_position` over `window` of the observer's
    /// time, asking about every class with the [`DEFAULT_HOST_LIMIT`]; the builder changes either.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::alerts::{AlertBand, AlertQuery, BandLimits, ClassSet, SensorHorizon};
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::events::TimeWindow;
    /// use hyperion_sim::galaxy::catalogue_classes::ClassId;
    /// use hyperion_sim::galaxy::gas::ccm::Band;
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::WattsPerSquareMetre;
    ///
    /// let w = WattsPerSquareMetre::new;
    /// let visual = BandLimits::new(w(1e-12), w(1e-10))?;
    /// let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), visual)])?;
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in the cube")?;
    /// let next_year = UniverseTime::from_julian_years(1).ok_or("in range")?;
    /// let year = TimeWindow::new(UniverseTime::EPOCH, next_year)?;
    /// let query = AlertQuery::builder(sun, year, sensor)
    ///     .classes(ClassSet::of(ClassId::CORE_COLLAPSE).with(ClassId::TYPE_IA))
    ///     .build()?;
    /// assert_eq!(query.classes().iter().count(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn builder(
        observer_position: GalacticPosition,
        window: TimeWindow,
        sensor: SensorHorizon,
    ) -> AlertQueryBuilder {
        AlertQueryBuilder {
            query: Self {
                observer_position,
                window,
                sensor,
                classes: ClassSet::ALL,
                host_limit: DEFAULT_HOST_LIMIT,
            },
        }
    }

    /// Where the sensor is, at rest in the galactic frame over the window.
    #[must_use]
    pub const fn observer_position(&self) -> &GalacticPosition {
        &self.observer_position
    }

    /// The window of the observer's time, `[t₁, t₂)`, inside the clock window: a detection is in
    /// it when its light arrives in it.
    #[must_use]
    pub const fn window(&self) -> TimeWindow {
        self.window
    }

    /// The sensor's limits.
    #[must_use]
    pub const fn sensor(&self) -> &SensorHorizon {
        &self.sensor
    }

    /// The classes asked about.
    #[must_use]
    pub const fn classes(&self) -> ClassSet {
        self.classes
    }

    /// The most hosts a class may be expected to have in its reach for it to be walked: above it
    /// the class is [`ClassStatus::OverLimit`] and yields nothing (Design note 7).
    #[must_use]
    pub const fn host_limit(&self) -> NonZeroU32 {
        self.host_limit
    }
}

/// Sets an [`AlertQuery`]'s optional parts.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertQueryBuilder {
    query: AlertQuery,
}

impl AlertQueryBuilder {
    /// The classes asked about.
    ///
    /// Default: [`ClassSet::ALL`].
    #[must_use]
    pub const fn classes(mut self, classes: ClassSet) -> Self {
        self.query.classes = classes;
        self
    }

    /// The most hosts a class may be expected to have in its reach.
    ///
    /// Default: [`DEFAULT_HOST_LIMIT`].
    #[must_use]
    pub const fn host_limit(mut self, host_limit: NonZeroU32) -> Self {
        self.query.host_limit = host_limit;
        self
    }

    /// The query.
    ///
    /// # Errors
    ///
    /// [`BuildAlertQueryError::ObserverOutsideRootCube`] for an observer outside the root cube,
    /// and [`BuildAlertQueryError::WindowOutsideClockWindow`] for a window that starts before −H
    /// or ends after +H.
    pub fn build(self) -> Result<AlertQuery, BuildAlertQueryError> {
        let query = self.query;
        if !query.observer_position.in_root_cube() {
            return Err(BuildAlertQueryError::ObserverOutsideRootCube);
        }
        if !ClockWindow::contains(query.window.start())
            || !ClockWindow::contains(query.window.end())
        {
            return Err(BuildAlertQueryError::WindowOutsideClockWindow(query.window));
        }
        Ok(query)
    }
}

/// An [`AlertQuery`]'s parameters were rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildAlertQueryError {
    /// The observer lies outside the root cube.
    ObserverOutsideRootCube,
    /// The window reaches outside the [`ClockWindow`], ±1,000 Julian years about the epoch, the
    /// observer times of play.
    WindowOutsideClockWindow(TimeWindow),
}

impl fmt::Display for BuildAlertQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObserverOutsideRootCube => {
                f.write_str("the alert query's observer lies outside the root cube")
            }
            Self::WindowOutsideClockWindow(window) => write!(
                f,
                "the alert query's window from {} to {} reaches outside the clock window",
                window.start(),
                window.end()
            ),
        }
    }
}

impl Error for BuildAlertQueryError {}

/// Where a detection stands in its event when its light is first counted (Design note 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DetectionPhase {
    /// The event's onset arrives in the window.
    Onset,
    /// The onset arrived before the window, and the event is still above the detection limit at
    /// the window's start.
    InProgress,
}

/// One event seen by the sensor (plan 12's Provides): which event, of which host, when it left and
/// arrived, how bright it is and where it appears.
///
/// Built by the alert walks (P12.T5.b–P12.T5.e), through its crate-private constructor: the
/// one-shot classes' by [`one_shot_detections`].
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    event: EventId,
    host: SystemId,
    class: Option<ClassId>,
    kind: TransientKind,
    emitted: UniverseTime,
    arrival: UniverseTime,
    phase: DetectionPhase,
    band: AlertBand,
    flux: WattsPerSquareMetre,
    bearing: Bearing,
    retardation: Retardation,
    resolvable: bool,
}

impl Detection {
    /// The detection of `event` of `host`, of its `class` and `kind`, in `phase`, seen in `band`
    /// at `flux` from `bearing`, its light read by `retardation`, and whether it is `resolvable`.
    ///
    /// The emitted and arrival times are the retardation's, so that they cannot disagree with it.
    #[expect(
        clippy::too_many_arguments,
        reason = "a detection's fields, each found apart by the walk; the times come from the \
                  retardation"
    )]
    #[must_use]
    pub(super) const fn new(
        event: EventId,
        host: SystemId,
        class: Option<ClassId>,
        kind: TransientKind,
        phase: DetectionPhase,
        band: AlertBand,
        flux: WattsPerSquareMetre,
        bearing: Bearing,
        retardation: Retardation,
        resolvable: bool,
    ) -> Self {
        Self {
            event,
            host,
            class,
            kind,
            emitted: retardation.emitted(),
            arrival: retardation.observed(),
            phase,
            band,
            flux,
            bearing,
            retardation,
            resolvable,
        }
    }

    /// The event, the same from any distance (the shared re-observation test).
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// The system the event is of.
    #[must_use]
    pub const fn host(&self) -> SystemId {
        self.host
    }

    /// The host's catalogue class, or `None` for a local event of an ordinary system.
    #[must_use]
    pub const fn class(&self) -> Option<ClassId> {
        self.class
    }

    /// What kind of event it is.
    #[must_use]
    pub const fn kind(&self) -> TransientKind {
        self.kind
    }

    /// When the event left its host: its onset T.
    #[must_use]
    pub const fn emitted(&self) -> UniverseTime {
        self.emitted
    }

    /// When the event's light reaches the observer, T + |`x_s(T)` − `x_o`| ÷ c (Design note 6).
    #[must_use]
    pub const fn arrival(&self) -> UniverseTime {
        self.arrival
    }

    /// Whether the onset arrives in the window or arrived before it.
    #[must_use]
    pub const fn phase(&self) -> DetectionPhase {
        self.phase
    }

    /// The band the event is seen in.
    #[must_use]
    pub const fn band(&self) -> AlertBand {
        self.band
    }

    /// The event's flux at the observer in [`band`](Self::band), after extinction.
    #[must_use]
    pub const fn flux(&self) -> WattsPerSquareMetre {
        self.flux
    }

    /// The direction the light comes from, at the observer.
    #[must_use]
    pub const fn bearing(&self) -> Bearing {
        self.bearing
    }

    /// When and where the light left the host.
    #[must_use]
    pub const fn retardation(&self) -> &Retardation {
        &self.retardation
    }

    /// Whether the flux reaches the sensor's resolve limit in the band.
    #[must_use]
    pub const fn is_resolvable(&self) -> bool {
        self.resolvable
    }
}

/// How a class's census ended (Design note 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ClassStatus {
    /// Every host in the class's reach was walked: its detections are complete.
    Complete,
    /// The class expects more hosts in its reach than the query's host limit: no detections.
    OverLimit,
    /// The class expects more than [`BACKGROUND_THRESHOLD_HOSTS`] in its reach: the caller scans it
    /// in the background, and until then it yields nothing.
    NeedsBackgroundScan,
}

/// One class's census in an [`AlertResult`] (plan 12's Provides).
///
/// Built by `alerts_in` (P12.T5.d).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ClassCensus {
    class: ClassId,
    reach: LightYears,
    expected_hosts: f64,
    status: ClassStatus,
}

impl ClassCensus {
    /// The class.
    #[must_use]
    pub const fn class(&self) -> ClassId {
        self.class
    }

    /// Its reach at the query's sensor ([`class_reach`]).
    #[must_use]
    pub const fn reach(&self) -> LightYears {
        self.reach
    }

    /// The hosts it is expected to have in the widened reach sphere, from plan 09's class
    /// density, decided before walking.
    #[must_use]
    pub const fn expected_hosts(&self) -> f64 {
        self.expected_hosts
    }

    /// How its census ended.
    #[must_use]
    pub const fn status(&self) -> ClassStatus {
        self.status
    }
}

/// What went off: the detections, by arrival and then by event, and a census per class asked
/// about (plan 12's Provides).
///
/// Built by `alerts_in` (P12.T5.d).
#[derive(Debug, Clone, PartialEq)]
pub struct AlertResult {
    detections: Vec<Detection>,
    census: Vec<ClassCensus>,
}

impl AlertResult {
    /// The detections, by arrival and then by [`EventId`].
    #[must_use]
    pub fn detections(&self) -> &[Detection] {
        &self.detections
    }

    /// The census of each class asked about, in class order.
    #[must_use]
    pub fn census(&self) -> &[ClassCensus] {
        &self.census
    }
}

/// The caller's caches and sources, since the sim holds none (plan 12's Provides).
///
/// T5.a's part is what the flux needs: plan 07's [`NoiseCache`], and the [`GasModifierSource`]
/// whose holes and clouds lie along a line of sight, plan 09's
/// [`FeatureGas`](crate::galaxy::features::gas_overlay::FeatureGas) in production (at an
/// evaluation time the caller picks, its features at their epoch positions) or
/// [`NoModifiers`](crate::galaxy::gas::modifiers::NoModifiers). T5.b's is the one-shot classes'
/// cells ([`OneShotCells`], [`with_one_shot_cells`](Self::with_one_shot_cells)); without one, each
/// cell is generated for its visit. The later walks add theirs: plan 09's `FeatureCatalogue` with
/// its cell cache (P12.T5.c–P12.T5.d) and plan 10's `GlobalList` (P12.T5.d).
pub struct AlertContext<'a> {
    noise: &'a mut NoiseCache,
    gas: &'a dyn GasModifierSource,
    one_shot_cells: Option<&'a mut dyn OneShotCells>,
    /// The modifiers of the last line of sight, kept for its allocation.
    modifiers: Vec<GasModifier>,
}

impl fmt::Debug for AlertContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AlertContext").finish_non_exhaustive()
    }
}

impl<'a> AlertContext<'a> {
    /// The context of `noise` and the modifiers of `gas`.
    #[must_use]
    pub fn new(noise: &'a mut NoiseCache, gas: &'a dyn GasModifierSource) -> Self {
        Self {
            noise,
            gas,
            one_shot_cells: None,
            modifiers: Vec::new(),
        }
    }

    /// The same context, lending the one-shot walk its classes' cells from `cells` (P12.T5.b).
    #[must_use]
    pub fn with_one_shot_cells(self, cells: &'a mut dyn OneShotCells) -> Self {
        Self {
            one_shot_cells: Some(cells),
            ..self
        }
    }

    /// Calls `visit` with the one-shot events of the class cell `key`, from the context's cells or
    /// from `generate`.
    pub(crate) fn one_shot_events(
        &mut self,
        key: CatalogueCellKey,
        generate: &mut dyn FnMut() -> Vec<OneShotEvent>,
        visit: &mut dyn FnMut(&[OneShotEvent]),
    ) {
        match self.one_shot_cells.as_deref_mut() {
            Some(cells) => cells.with_events(key, generate, visit),
            None => NoOneShotCells.with_events(key, generate, visit),
        }
    }

    /// What lies between `from` and `to`: plan 07's line integral in [`NoiseMode::Realised`] at
    /// [`SIGHTLINE_QUALITY`], with the context's modifiers along the segment (P12.T5.a).
    ///
    /// The budget is plan 07's extinction request's, shared for the same reason: a line's cost
    /// bounded whatever its length, up to the cube's diagonal. The same bits whichever way round
    /// the ends are given, as plan 07's [`sightline`] and its modifier sources promise; the noise
    /// cache changes what the call costs, never what it returns. For an event, `from` is the
    /// host's apparent position and `to` the observer.
    #[must_use]
    pub fn sightline(
        &mut self,
        galaxy: &Galaxy,
        from: &GalacticPosition,
        to: &GalacticPosition,
    ) -> Sightline {
        self.modifiers.clear();
        self.gas
            .modifiers_near_segment(from, to, &mut self.modifiers);
        sightline(
            galaxy.gas(),
            from,
            to,
            NoiseMode::Realised,
            SIGHTLINE_QUALITY,
            &self.modifiers,
            self.noise,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::rng::Seed;
    use crate::time::Span;

    fn w(value: f64) -> WattsPerSquareMetre {
        WattsPerSquareMetre::new(value)
    }

    fn at(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).expect("inside the root cube")
    }

    fn visual() -> SensorHorizon {
        let limits = BandLimits::new(w(1e-12), w(1e-10)).unwrap();
        SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).unwrap()
    }

    #[test]
    fn every_band_has_its_own_place_in_order() {
        for (i, band) in AlertBand::ALL.into_iter().enumerate() {
            assert_eq!(band.index(), i, "{band:?}");
        }
        let photometric: Vec<_> = Band::ALL.into_iter().map(AlertBand::Photometric).collect();
        assert_eq!(AlertBand::ALL[..10], photometric[..]);
        assert_eq!(AlertBand::ALL[10], AlertBand::XRay);
    }

    #[test]
    fn band_limits_must_be_positive_and_resolve_no_fainter_than_detect() {
        assert_eq!(
            BandLimits::new(w(0.0), w(1.0)),
            Err(BuildBandLimitsError::NotPositive)
        );
        assert_eq!(
            BandLimits::new(w(1.0), w(f64::INFINITY)),
            Err(BuildBandLimitsError::NotPositive)
        );
        assert_eq!(
            BandLimits::new(w(f64::NAN), w(1.0)),
            Err(BuildBandLimitsError::NotPositive)
        );
        assert_eq!(
            BandLimits::new(w(1e-10), w(1e-12)),
            Err(BuildBandLimitsError::ResolveBelowDetect)
        );
        let same = BandLimits::new(w(1e-12), w(1e-12)).unwrap();
        assert_eq!(same.detect(), same.resolve());
    }

    #[test]
    fn a_sensor_has_each_band_at_most_once_and_at_least_one() {
        let limits = BandLimits::new(w(1e-12), w(1e-10)).unwrap();
        assert_eq!(SensorHorizon::new([]), Err(BuildSensorHorizonError::NoBand));
        assert_eq!(
            SensorHorizon::new([(AlertBand::XRay, limits), (AlertBand::XRay, limits)]),
            Err(BuildSensorHorizonError::BandTwice(AlertBand::XRay))
        );
        let sensor = SensorHorizon::new([
            (AlertBand::XRay, limits),
            (AlertBand::Photometric(Band::K), limits),
        ])
        .unwrap();
        let bands: Vec<_> = sensor.bands().map(|(band, _)| band).collect();
        assert_eq!(bands, [AlertBand::Photometric(Band::K), AlertBand::XRay]);
        assert_eq!(sensor.limits(AlertBand::XRay), Some(limits));
        assert_eq!(sensor.limits(AlertBand::Photometric(Band::V)), None);
    }

    #[test]
    fn a_class_set_holds_registered_classes_in_value_order() {
        assert!(ClassSet::EMPTY.is_empty());
        assert_eq!(ClassSet::default(), ClassSet::EMPTY);
        assert_eq!(ClassSet::ALL.iter().collect::<Vec<_>>(), ALL_CLASSES);
        let set: ClassSet = [
            ClassId::XRAY_BINARY,
            ClassId::CORE_COLLAPSE,
            ClassId::XRAY_BINARY,
        ]
        .into_iter()
        .collect();
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            [ClassId::CORE_COLLAPSE, ClassId::XRAY_BINARY]
        );
        assert!(!set.contains(ClassId::TYPE_IA));
        assert_eq!(
            set,
            ClassSet::of(ClassId::XRAY_BINARY).with(ClassId::CORE_COLLAPSE)
        );
    }

    #[test]
    fn an_alert_query_holds_its_observer_and_window_inside_the_clock() {
        let year = Span::from_julian_years(1).unwrap();
        let start = UniverseTime::EPOCH;
        let window = TimeWindow::new(start, start.checked_add(year).unwrap()).unwrap();
        let query = AlertQuery::builder(at([0.0, 26_000.0, 20.0]), window, visual())
            .build()
            .unwrap();
        assert_eq!(query.classes(), ClassSet::ALL);
        assert_eq!(query.host_limit(), DEFAULT_HOST_LIMIT);
        assert_eq!(query.window(), window);

        let outside = GalacticPosition::from_light_years([70_000.0, 0.0, 0.0]).unwrap();
        assert_eq!(
            AlertQuery::builder(outside, window, visual()).build(),
            Err(BuildAlertQueryError::ObserverOutsideRootCube)
        );
        let early = TimeWindow::new(
            ClockWindow::START.checked_sub(year).unwrap(),
            ClockWindow::START,
        )
        .unwrap();
        assert_eq!(
            AlertQuery::builder(at([0.0, 26_000.0, 20.0]), early, visual()).build(),
            Err(BuildAlertQueryError::WindowOutsideClockWindow(early))
        );
        let late = TimeWindow::new(
            ClockWindow::END,
            ClockWindow::END.checked_add(year).unwrap(),
        )
        .unwrap();
        assert_eq!(
            AlertQuery::builder(at([0.0, 26_000.0, 20.0]), late, visual()).build(),
            Err(BuildAlertQueryError::WindowOutsideClockWindow(late))
        );
        let whole = TimeWindow::new(ClockWindow::START, ClockWindow::END).unwrap();
        let limit = NonZeroU32::new(1_000).unwrap();
        let query = AlertQuery::builder(at([0.0, 26_000.0, 20.0]), whole, visual())
            .classes(ClassSet::of(ClassId::TYPE_IA))
            .host_limit(limit)
            .build()
            .unwrap();
        assert_eq!(query.host_limit(), limit);
        assert_eq!(query.classes(), ClassSet::of(ClassId::TYPE_IA));
    }

    /// A source that gives one cloud beside the segment from the Sun to its target, whatever the
    /// segment, as a feature's cloud near the line would.
    struct OneCloud;

    impl GasModifierSource for OneCloud {
        fn modifiers_near_segment(
            &self,
            _a: &GalacticPosition,
            _b: &GalacticPosition,
            out: &mut Vec<GasModifier>,
        ) {
            out.push(cloud());
        }
    }

    fn cloud() -> GasModifier {
        GasModifier::Cloud {
            centre: at([1_500.0, 23_000.0, -40.0]),
            core_radius: LightYears::new(30.0),
            central_density: crate::units::HydrogenPerCm3::new(300.0),
            dust_per_hydrogen: 1.0,
        }
    }

    /// The context's line is plan 07's, in the realised noise at the shared budget, with the
    /// source's modifiers gathered afresh for each line, and the same both ways.
    #[test]
    fn the_contexts_sightline_is_plan_07s_realised_line() {
        let galaxy = Galaxy::new(Seed::new(42));
        let (a, b) = (at([0.0, 26_000.0, 20.0]), at([3_000.0, 20_000.0, -100.0]));
        let mut cache = NoiseCache::with_capacity(16);
        let mut direct = |modifiers: &[GasModifier]| {
            sightline(
                galaxy.gas(),
                &a,
                &b,
                NoiseMode::Realised,
                SIGHTLINE_QUALITY,
                modifiers,
                &mut cache,
            )
        };
        let (bare, clouded) = (direct(&[]), direct(&[cloud()]));
        assert!(clouded.a_v().value() > bare.a_v().value());

        let mut noise = NoiseCache::with_capacity(4_096);
        let mut context = AlertContext::new(&mut noise, &NoModifiers);
        let line = context.sightline(&galaxy, &a, &b);
        assert_eq!(line, bare);
        assert_eq!(context.sightline(&galaxy, &b, &a), line);

        let mut context = AlertContext::new(&mut noise, &OneCloud);
        // Asked twice, the line holds the source's one cloud each time, not one more each call.
        for _ in 0..2 {
            assert_eq!(context.sightline(&galaxy, &a, &b), clouded);
        }
    }
}
