//! Local events: the events of a range query's own systems, each over its retarded interval (plan
//! 12, P12.T5.e; brainstorm, "Alerts": "Flares, dwarf novae and glitches are local and come with
//! the range query").
//!
//! [`local_events`] takes the systems of an observed-mode range result and lists the events of each
//! of their stars whose light arrives in an [`AlertQuery`]'s window, through plan 06's
//! constructions:
//!
//! - star flares ([`flares`](crate::stellar::events::flares), P06.T28.a), [`TransientKind::Flare`];
//! - FU Orionis outbursts ([`fu_orionis`], P06.T28.d), [`TransientKind::FuOrionisOutburst`];
//! - pulsar glitches, Crab-like and Vela-like ([`glitches`], P06.T28.b), [`TransientKind::Glitch`].
//!
//! Two local kinds wait for plan 11's binary events, P11.T9, whose rows in the registry are
//! pending on it, and are not listed: [`TransientKind::DwarfNova`] and, while its row is local,
//! [`TransientKind::XrayBurst`]. Magnetar bursts are walked with the core collapses (P12.T5.c).
//!
//! # The window
//!
//! An event emitted at T arrives at T + |`x_s(T)` − `x_o`| ÷ c, a closed form (Design note 6), and
//! is an [`Onset`](DetectionPhase::Onset) when that falls in the window [t₁, t₂). One whose onset
//! arrived before t₁ is [`InProgress`](DetectionPhase::InProgress) if it is still running in the
//! light that arrives at t₁. A host whose light time is s at the range query's time t is asked for
//! the onsets in [t₁ − s − ε − D, t₂ − s + ε], with D the kind's longest duration
//! ([`TransientRow::duration`](super::TransientRow::duration)) and ε = β (W + s) ÷ (1 − β) plus a
//! second, where β is the host's speed over c and W the window's furthest end from t: Design note
//! 6's ε, s × β, bounds the light time's change only while the window is near t, and this one holds
//! for any window. The arrival of each onset found then decides. The light that arrives at t₁ is
//! found by iterating the light-cone step to the nanosecond.
//!
//! Each host moves on the line its row was read on, its record's epoch position at its velocity
//! then, which is [`Drift::of_record`]'s: so a row read at any time gives the same detections.
//!
//! # The light
//!
//! A flare's and an outburst's light is their own, (factor − 1) times the star's quiescent
//! luminosity (the registry's [`LightCurveSource`](super::LightCurveSource)): the star's at the
//! epoch for a flare, as its model has it
//! ([`FlareModel::quiescent_luminosity`]), and at an
//! outburst's onset, the state its Eddington cap reads. Both are bolometric. A band takes its
//! share of a blackbody ([`band_share`](super::band_share)) through a top hat at plan 07's
//! [`Band::wavelength`] with the band's full width at half maximum (Bessell 2005, ARA&A 43, 293,
//! Table 1, for U to I; Tokunaga 2000, Allen's Astrophysical Quantities, Table 7.5, for J, H, K and
//! the N band's 7.88–13.07 µm for the mid infrared). Radio has no thermal share, and neither kind
//! has an X-ray one: plan 06 models white light only (`FLARE_SHARES`, `OUTBURST_SHARES`).
//!
//! A detection's band is the sensor's band in which the event stands highest over the detection
//! limit, the first in [`AlertBand::ALL`]'s order on a tie: the shares and a host's sightline are
//! fixed, so that band is the same at every time. An onset is seen by its peak, its flux the peak's
//! at the host's distance then, though that light arrives after the onset's, whatever part of the
//! event the window holds, so that a window split in two finds each onset in the part its onset
//! arrives in, as the whole does (Design note 6). An event in progress is seen by its light at t₁,
//! as the window opens. Each is detected when that flux reaches the limit, and resolvable when it
//! reaches the resolve limit. The flux is taken through one sightline per host
//! ([`AlertContext::sightline`]), marched only for a host with an event the bare flux could show,
//! from where the host's line passes at the epoch to the observer: so it depends neither on the
//! window nor on when the row was read, and the host strays from that point by under a light-year
//! over the clock window at 220 km/s, against a line of tens of light-years (provisional).
//!
//! # Glitches
//!
//! A glitch gives no light: a sensor sees it by timing the pulsar, in the radio. It is reported
//! when the sensor has a radio band, the pulsar is above the death line when it glitches
//! ([`PulsarState::is_radio_alive`](crate::stellar::remnant::PulsarState::is_radio_alive)) and its
//! beam sweeps the observer, as an onset in [`Band::Radio`] with no flux, never resolvable. Plan 06
//! models no pulsar's radio luminosity, so no flux limit is applied: any radio sensor times a
//! beaming pulsar inside a range query (provisional).
//!
//! # Classes
//!
//! A local event's class is `None`. A range query finds grid systems and feature members, none of
//! them a catalogue class's host, so no local event is of one yet.
//!
//! Nothing here draws a random number or changes generated output: the events are plan 06's.

use std::collections::HashMap;
use std::f64::consts::PI;

use crate::coords::UnitVector;
use crate::events::{BinEvent, EventSeries, TimeWindow, tags};
use crate::galaxy::Galaxy;
use crate::galaxy::gas::ccm::Band;
use crate::galaxy::gas::extinction::Sightline;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemKind;
use crate::galaxy::rules::GalaxyRules;
use crate::id::{BodyId, EventId};
use crate::observe::{
    Drift, Motion, ObservedSystem, Observer, Retardation, StarsCache, Trajectory, bearing,
    light_time, retarded,
};
use crate::rng::Seed;
use crate::stellar::events::flares::{Flare, FlareModel, FlareRates};
use crate::stellar::events::fu_orionis::{self, FuOrionisModel, Outburst};
use crate::stellar::events::glitches::{self, Glitch, LargeGlitch};
use crate::stellar::system::StarModel;
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::{SOLAR_LUMINOSITY_W, SPEED_OF_LIGHT};
use crate::units::{Metres, Watts, WattsPerSquareMetre};

use super::arriving::{ArrivingLight, DetectionLight};
use super::reach::{flux, transmission};
use super::{
    AlertBand, AlertContext, AlertQuery, BAND_COUNT, BandLimits, Detection, DetectionPhase,
    LongestDuration, TransientKind,
};

/// The share of a star flare's own light in each band, in [`AlertBand::ALL`]'s order: a 10,000 K
/// blackbody's through each band's top hat (module documentation), none in radio or X-rays.
///
/// 10,000 K is the temperature Shibayama et al. (2013, ApJS 209, 5, section 2.3) take for the
/// superflares that are plan 06's solar-type anchor, inside the 9,000–14,000 K that Kowalski et al.
/// (2013, ApJS 207, 15) find for the blue continuum of M dwarfs' flares at their peaks. The shares
/// are [`band_share`](super::band_share)'s at 10,000 K to 10⁻¹², held by
/// `the_band_shares_are_blackbodies_through_the_top_hats`, and keep the bits of the integral they
/// were first stored from.
pub(crate) const FLARE_SHARES: [f64; BAND_COUNT] = [
    0.132_442_267_280_078_88,
    0.140_976_411_610_259_12,
    0.087_249_980_273_734_86,
    0.116_634_411_122_468_32,
    0.064_718_159_393_727_94,
    0.026_653_957_510_285_18,
    0.011_466_409_546_993_314,
    0.005_824_026_155_493_473,
    0.000_225_883_090_809_039_2,
    0.0,
    0.0,
];

/// The share of an FU Orionis outburst's own light in each band, in [`AlertBand::ALL`]'s order: a
/// 6,500 K blackbody's from U to I and a 4,000 K blackbody's from J to the mid infrared, through
/// each band's top hat (module documentation), none in radio or X-rays.
///
/// An outburst is a hot accretion disc, no one temperature: its spectrum is an F–G supergiant's in
/// the optical and a K–M giant's or supergiant's in the near infrared (Audard et al. 2014,
/// Protostars and Planets VI, 387, section 2.1). The two blackbodies stand for those types, the
/// middle of 6,000–7,000 K and a late K star's 4,000 K, so that the near-infrared bands are not
/// under-counted as a single optical blackbody would leave them (provisional). Held to
/// [`band_share`](super::band_share)'s to 10⁻¹² by
/// `the_band_shares_are_blackbodies_through_the_top_hats`, with the bits of the integral they were
/// first stored from.
pub(crate) const OUTBURST_SHARES: [f64; BAND_COUNT] = [
    0.084_404_683_662_944_75,
    0.130_105_799_892_860_25,
    0.112_040_994_393_378_56,
    0.176_863_241_975_995_2,
    0.120_229_358_508_333_72,
    0.131_698_262_780_811_54,
    0.078_629_243_546_181_24,
    0.050_425_573_348_227_226,
    0.003_133_475_956_248_471_6,
    0.0,
    0.0,
];

/// The most steps of the light-cone iteration that finds the light arriving at a given time: the
/// step contracts by β, at most 1/300 for anything plan 03 pads for, so a host across the cube is
/// within a nanosecond in ten.
const MAX_LIGHT_STEPS: u32 = 16;

/// What `observed`'s systems are seen to do over `query`'s window: the events of plan 06's local
/// kinds whose light arrives in it, appended to `out` by arrival and then by event (plan 12's
/// Provides; module documentation).
///
/// `observed` are the rows of an observed-mode range result asked from
/// [`query.observer_position()`](AlertQuery::observer_position), at any time: each row is evaluated
/// on its own line, so the result depends neither on their order, nor on when they were read, nor
/// on what was asked before. `stars` lends each system's stars, as it does to
/// [`range_query_observed`](crate::galaxy::query::range_query_observed); a rogue planet has none,
/// and is not asked for. `cache` keeps what each star's events read of it once
/// ([`LocalEventsCache`]), which changes the cost and never the result. The query's classes and
/// host limit are the catalogue's, and local events read neither. `out` is only appended to;
/// nothing in it is read.
///
/// # Panics
///
/// - As `stars` does for a system it cannot build, which no row of an observed result is.
/// - In debug builds, for a row whose source follows an orbit, which plan 09's P09.T28 will bring
///   (its events would need the orbit, not a line), and for a row read from another observer than
///   the query's.
///
/// # Examples
///
/// The flares that a sensitive visual sensor at the Sun-like point sees from its neighbours in a
/// week:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::local::LocalEventsCache;
/// use hyperion_sim::alerts::{
///     AlertBand, AlertContext, AlertQuery, BandLimits, SensorHorizon, TransientKind, local_events,
/// };
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::galaxy::placement::NoCache;
/// use hyperion_sim::galaxy::query::{QueryMode, RangeQuery, range_query_observed};
/// use hyperion_sim::observe::NoStarsCache;
/// use hyperion_sim::time::{Span, UniverseTime};
/// use hyperion_sim::units::{LightYears, WattsPerSquareMetre};
///
/// let galaxy = Galaxy::new(Seed::new(12));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// let found = range_query_observed(
///     &galaxy,
///     &mut NoCache::new(),
///     &mut NoStarsCache,
///     &[],
///     &RangeQuery::builder(sun, LightYears::new(10.0))
///         .mode(QueryMode::ObservedFrom(sun))
///         .build()?,
/// )?;
///
/// let w = WattsPerSquareMetre::new;
/// let sensor = SensorHorizon::new([
///     (AlertBand::Photometric(Band::V), BandLimits::new(w(1e-16), w(1e-14))?),
/// ])?;
/// let week = UniverseTime::EPOCH
///     .checked_add(Span::from_seconds(7 * 86_400))
///     .ok_or("in range")?;
/// let window = TimeWindow::new(UniverseTime::EPOCH, week)?;
/// let query = AlertQuery::builder(sun, window, sensor).build()?;
/// let mut noise = NoiseCache::with_capacity(1_024);
/// let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
/// let mut cache = LocalEventsCache::with_capacity(1_024);
/// let mut seen = Vec::new();
/// let rows = found.observed();
/// local_events(&galaxy, &mut ctx, &query, rows, &mut NoStarsCache, &mut cache, &mut seen);
/// // Every one arrives in the week or is still on at its start, in arrival order.
/// assert!(seen.iter().all(|d| d.kind() == TransientKind::Flare && d.arrival() < week));
/// assert!(seen.windows(2).all(|pair| pair[0].arrival() <= pair[1].arrival()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn local_events<S: StarsCache>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    observed: &[ObservedSystem],
    stars: &mut S,
    cache: &mut LocalEventsCache,
    out: &mut Vec<Detection>,
) {
    let lights = Lights::of(query);
    // A sensor that sees none of the kinds builds no system's stars.
    if lights.flare.best_bare_m2_per_w <= 0.0
        && lights.outburst.best_bare_m2_per_w <= 0.0
        && !lights.radio
    {
        return;
    }
    let first = out.len();
    cache.bind(galaxy);
    let mut walk = Walk {
        galaxy,
        ctx,
        query,
        lights,
        scratch: Scratch::default(),
        cache,
        out,
    };
    for system in observed {
        if system.hit().record().kind() == SystemKind::RoguePlanet {
            continue;
        }
        let mut host = Host::new(query, system);
        stars.with_stars(galaxy, system.hit().record(), |stars| {
            for (star, slot) in stars.stars().iter().zip(stars.hierarchy().stars()) {
                walk.star(&mut host, star, slot.body());
            }
        });
    }
    out[first..].sort_by(|a, b| a.arrival.cmp(&b.arrival).then(a.event.cmp(&b.event)));
}

/// A caller-owned cache of what [`local_events`] reads of each star once: its flare rates at the
/// epoch (plan 06's `FlareRates`) and the span in which it can have FU Orionis outbursts, keyed by
/// the star's body, holding at most a fixed number of stars.
///
/// A star's flare rates cost its state, rotation and activity at the epoch, 1.6 µs a star (pod, 16
/// vCPU), and its outbursts' model 0.12 µs more, which a walk asked again at each step of the clock
/// pays for every star of every row: over half of a day's walk over the 50 ly query. A cache
/// changes no result, only its cost: every detection is bit-identical with a cache of any capacity,
/// none included, and after [`clear`](Self::clear). A full cache empties itself before it keeps
/// another star. It serves one galaxy at a time: it remembers the galaxy its entries were read in,
/// by its seed, parameters and rules, and empties itself when asked about another. The default is
/// a cache that keeps no star.
///
/// # Examples
///
/// ```
/// use hyperion_sim::alerts::local::LocalEventsCache;
///
/// // Room for every star of a 50 ly query's 1,346, and more.
/// let cache = LocalEventsCache::with_capacity(4_096);
/// assert_eq!(cache.capacity(), 4_096);
/// assert_eq!(LocalEventsCache::default().capacity(), 0);
/// ```
#[derive(Debug, Clone, Default)]
pub struct LocalEventsCache {
    /// The galaxy the entries were read in, by its seed, rules and parameters; `None` while the
    /// cache is empty.
    galaxy: Option<(Seed, GalaxyRules, GalaxyParams)>,
    /// The most stars the cache keeps.
    capacity: usize,
    /// Each star kept, by its body. Only looked up, never walked, so its order reaches nothing.
    stars: HashMap<BodyId, StarEntry>,
}

/// What [`local_events`] reads of one star once.
#[derive(Debug, Clone, Copy, PartialEq)]
struct StarEntry {
    /// Its flare rates, `None` for a star that does not flare.
    flares: Option<FlareRates>,
    /// From its onset of collapse to its last instant with outbursts
    /// ([`FuOrionisModel::formation`], [`FuOrionisModel::last_instant`]), `None` for an object
    /// below 0.1 M☉, which has none.
    outbursts: Option<(UniverseTime, UniverseTime)>,
}

impl StarEntry {
    /// What `star`'s events read of it.
    #[must_use]
    fn of(star: &StarModel) -> Self {
        Self {
            flares: FlareRates::of(star),
            outbursts: FuOrionisModel::of(star)
                .map(|model| (model.formation(), model.last_instant())),
        }
    }
}

impl LocalEventsCache {
    /// An empty cache that keeps up to `stars` stars. Zero is a cache that keeps nothing, whose
    /// every lookup reads the star afresh.
    #[must_use]
    pub fn with_capacity(stars: usize) -> Self {
        Self {
            galaxy: None,
            capacity: stars,
            stars: HashMap::with_capacity(stars),
        }
    }

    /// The most stars the cache keeps.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Empties the cache, keeping its allocation.
    pub fn clear(&mut self) {
        self.stars.clear();
        self.galaxy = None;
    }

    /// Makes the cache hold `galaxy`'s stars, emptying it if it held another galaxy's.
    fn bind(&mut self, galaxy: &Galaxy) {
        let held = self.galaxy.as_ref().is_some_and(|(seed, rules, params)| {
            *seed == galaxy.seed() && *rules == galaxy.rules() && params == galaxy.params()
        });
        if !held {
            self.clear();
            self.galaxy = Some((galaxy.seed(), galaxy.rules(), galaxy.params().clone()));
        }
    }

    /// What the events of `star`, whose body is `body`, read of it, kept or read afresh.
    ///
    /// # Panics
    ///
    /// In debug builds, if what is kept for `body` is not `star`'s.
    fn star(&mut self, star: &StarModel, body: BodyId) -> StarEntry {
        if let Some(entry) = self.stars.get(&body) {
            debug_assert_eq!(
                *entry,
                StarEntry::of(star),
                "the star kept for {body:?} is another"
            );
            return *entry;
        }
        let entry = StarEntry::of(star);
        if self.capacity > 0 {
            if self.stars.len() >= self.capacity {
                self.stars.clear();
            }
            self.stars.insert(body, entry);
        }
        entry
    }
}

/// One lit kind's shares, and the most its light can stand over the sensor's detection limit in any
/// band before extinction.
#[derive(Clone, Copy)]
struct Light {
    shares: &'static [f64; BAND_COUNT],
    /// The largest share ÷ detection limit over the sensor's bands, m² W⁻¹: zero if the sensor has
    /// no band the kind lights.
    best_bare_m2_per_w: f64,
}

impl Light {
    /// The light of a kind with `shares`, seen by `query`'s sensor.
    #[must_use]
    fn of(shares: &'static [f64; BAND_COUNT], query: &AlertQuery) -> Self {
        let best_bare_m2_per_w = query
            .sensor()
            .bands()
            .map(|(band, limits)| shares[band.index()] / limits.detect().value())
            .fold(0.0, f64::max);
        Self {
            shares,
            best_bare_m2_per_w,
        }
    }

    /// The sensor's band in which this light stands highest over the detection limit through
    /// `sightline`, the first in [`AlertBand::ALL`]'s order on a tie, with its limits; `None` if
    /// the kind lights none of the sensor's bands.
    #[must_use]
    fn best_band(
        self,
        query: &AlertQuery,
        sightline: &Sightline,
    ) -> Option<(AlertBand, BandLimits)> {
        let mut best: Option<(AlertBand, BandLimits, f64)> = None;
        for (band, limits) in query.sensor().bands() {
            let share = self.shares[band.index()];
            if share <= 0.0 {
                continue;
            }
            let over = share * transmission(sightline, band) / limits.detect().value();
            if best.is_none_or(|(.., most)| over > most) {
                best = Some((band, limits, over));
            }
        }
        best.map(|(band, limits, _)| (band, limits))
    }

    /// Whether `light` of this kind reaches the sensor's detection limit in its best band at
    /// `distance` before extinction, which only dims: L s ÷ (4π d² F) ≥ 1.
    #[must_use]
    fn stands_over(self, light: Watts, distance: Metres) -> bool {
        let d = distance.value();
        light.value() * self.best_bare_m2_per_w >= 4.0 * PI * d * d
    }
}

/// A lit kind.
#[derive(Clone, Copy)]
enum Lit {
    Flare,
    Outburst,
}

impl Lit {
    /// Every lit kind, in [`Lit::index`]'s order.
    const ALL: [Self; 2] = [Self::Flare, Self::Outburst];

    /// The kind's place in a host's best bands.
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::Flare => 0,
            Self::Outburst => 1,
        }
    }
}

/// What the query's sensor can see of each kind.
struct Lights {
    flare: Light,
    outburst: Light,
    /// Whether the sensor times pulsars: it has a radio band.
    radio: bool,
}

impl Lights {
    /// The lights of `query`'s sensor.
    #[must_use]
    fn of(query: &AlertQuery) -> Self {
        Self {
            flare: Light::of(&FLARE_SHARES, query),
            outburst: Light::of(&OUTBURST_SHARES, query),
            radio: query
                .sensor()
                .limits(AlertBand::Photometric(Band::Radio))
                .is_some(),
        }
    }

    /// The light of `kind`.
    #[must_use]
    const fn light(&self, kind: Lit) -> Light {
        match kind {
            Lit::Flare => self.flare,
            Lit::Outburst => self.outburst,
        }
    }
}

/// The walk's reusable buffers, one per listing.
#[derive(Default)]
struct Scratch {
    bin_events: Vec<BinEvent>,
    flares: Vec<Flare>,
    outbursts: Vec<Outburst>,
    glitches: Vec<Glitch>,
    large_glitches: Vec<LargeGlitch>,
}

/// One flare or outburst, with the star's quiescent luminosity its light is a factor of.
enum LitEvent {
    /// A star flare and the flare model's quiescent luminosity.
    Flare(Flare, Watts),
    /// An FU Orionis outburst and the star's luminosity at its onset.
    Outburst(Outburst, Watts),
}

impl LitEvent {
    #[must_use]
    const fn lit(&self) -> Lit {
        match self {
            Self::Flare(..) => Lit::Flare,
            Self::Outburst(..) => Lit::Outburst,
        }
    }

    #[must_use]
    const fn kind(&self) -> TransientKind {
        match self {
            Self::Flare(..) => TransientKind::Flare,
            Self::Outburst(..) => TransientKind::FuOrionisOutburst,
        }
    }

    #[must_use]
    const fn id(&self) -> EventId {
        match self {
            Self::Flare(flare, _) => flare.id(),
            Self::Outburst(outburst, _) => outburst.id(),
        }
    }

    #[must_use]
    const fn onset(&self) -> UniverseTime {
        match self {
            Self::Flare(flare, _) => flare.onset(),
            Self::Outburst(outburst, _) => outburst.onset(),
        }
    }

    /// The peak: a flare's template peaks one half width after its onset.
    #[must_use]
    fn peak(&self) -> UniverseTime {
        match self {
            Self::Flare(flare, _) => Span::from_seconds_f64(flare.half_width_seconds())
                .and_then(|half_width| flare.onset().checked_add(half_width))
                .expect("a flare peaks minutes after an onset the clock holds"),
            Self::Outburst(outburst, _) => outburst.peak(),
        }
    }

    #[must_use]
    fn is_active_at(&self, t: UniverseTime) -> bool {
        match self {
            Self::Flare(flare, _) => flare.is_active_at(t),
            Self::Outburst(outburst, _) => outburst.is_active_at(t),
        }
    }

    /// The event's own light at its peak.
    #[must_use]
    fn peak_light(&self) -> Watts {
        match *self {
            Self::Flare(flare, quiescent) => {
                Watts::new((flare.peak_factor() - 1.0) * quiescent.value())
            }
            Self::Outburst(outburst, quiescent) => {
                Watts::new((outburst.peak_factor() - 1.0) * quiescent.value())
            }
        }
    }

    /// The event's light arriving `delay` after it leaves `distance` away, through a line of
    /// sight of `transmission` in `band`, in which the kind has `share` of it (P12.T8.b.1).
    #[must_use]
    const fn arriving(
        &self,
        band: AlertBand,
        share: f64,
        delay: Span,
        distance: Metres,
        transmission: f64,
    ) -> ArrivingLight {
        match *self {
            Self::Flare(flare, quiescent) => ArrivingLight::flare(
                (flare, quiescent, share),
                band,
                delay,
                distance,
                transmission,
            ),
            Self::Outburst(outburst, quiescent) => ArrivingLight::outburst(
                (outburst, quiescent, share),
                band,
                delay,
                distance,
                transmission,
            ),
        }
    }

    /// The event's own light at `t`.
    #[must_use]
    fn light_at(&self, t: UniverseTime) -> Watts {
        let (factor, quiescent) = match *self {
            Self::Flare(flare, quiescent) => (flare.luminosity_factor_at(t), quiescent),
            Self::Outburst(outburst, quiescent) => (outburst.luminosity_factor_at(t), quiescent),
        };
        Watts::new((factor - 1.0) * quiescent.value())
    }
}

/// An event the sensor sees, before its retardation and bearing are read.
#[derive(Clone, Copy)]
struct Seen {
    event: EventId,
    kind: TransientKind,
    emitted: UniverseTime,
    arrival: UniverseTime,
    phase: DetectionPhase,
    band: AlertBand,
    flux: WattsPerSquareMetre,
    resolvable: bool,
    light: DetectionLight,
}

/// Each lit kind's best band through a host's sightline, in [`Lit::index`]'s order: `None` for a
/// kind that lights none of the sensor's bands.
type BestBands = [Option<(AlertBand, BandLimits)>; 2];

/// One system as the walk reads it: its line, and what the walk has worked out of the window for
/// it, each at most once.
struct Host<'q> {
    observed: &'q ObservedSystem,
    line: Drift,
    /// The span of onsets to ask for, before a kind's longest duration is taken off its start.
    onsets: (UniverseTime, UniverseTime),
    /// The least distance from the observer at which the host can emit light that arrives in the
    /// window.
    least_distance: Metres,
    /// The sightline and each lit kind's best band through it ([`Light::best_band`]), found on
    /// first use.
    sightline: Option<(Sightline, BestBands)>,
    start_emission: Option<UniverseTime>,
}

impl<'q> Host<'q> {
    /// The host `observed`, seen over `query`'s window (module documentation, "The window").
    #[must_use]
    fn new(query: &AlertQuery, observed: &'q ObservedSystem) -> Self {
        let retardation = observed.retardation();
        debug_assert_eq!(
            retardation.motion(),
            Motion::Drift,
            "an orbit's events need its orbit, not a line"
        );
        // A drifting row's velocity then is its line's, so this is the line the row was read on,
        // whenever it was read.
        let line = Drift::new(
            *observed.hit().record().epoch_position(),
            retardation.velocity_then(),
        );
        debug_assert!(
            {
                let then = line.position_at(retardation.observed());
                let distance = query.observer_position().distance_to(&then).value();
                let read = Metres::from(retardation.distance_now()).value();
                (read - distance).abs() <= 1e-9 * distance + 1.0
            },
            "a row read from another observer than the query's"
        );
        let beta = line.velocity().speed().value() / SPEED_OF_LIGHT;
        let window = query.window();
        let off = |t: UniverseTime| {
            t.checked_since(retardation.observed())
                .expect("two times on the clock differ by a span the clock holds")
                .as_seconds_f64()
                .abs()
        };
        let furthest = off(window.start()).max(off(window.end()));
        let light = Metres::from(retardation.distance_now()).value() / SPEED_OF_LIGHT;
        let slack = beta * (furthest + light) / (1.0 - beta) + 1.0;
        let nearest = (light - slack).max(0.0);
        let before = |t: UniverseTime, seconds: f64| {
            Span::from_seconds_f64(seconds)
                .and_then(|span| t.checked_sub(span))
                .expect("a light time across the cube keeps a time on the clock")
        };
        Self {
            observed,
            line,
            onsets: (
                before(window.start(), light + slack),
                before(window.end(), nearest),
            ),
            least_distance: Metres::new(nearest * SPEED_OF_LIGHT),
            sightline: None,
            start_emission: None,
        }
    }

    /// The onsets to ask for of a kind whose events last at most `longest`.
    #[must_use]
    fn onsets(&self, longest: Span) -> TimeWindow {
        let start = self
            .onsets
            .0
            .checked_sub(longest)
            .expect("a look-back of centuries keeps a time on the clock");
        TimeWindow::new(start, self.onsets.1).expect("the onsets start before they end")
    }
}

/// The walk over one call's systems: what it reads, its buffers and where it writes.
struct Walk<'w, 'c> {
    galaxy: &'w Galaxy,
    ctx: &'w mut AlertContext<'c>,
    query: &'w AlertQuery,
    lights: Lights,
    scratch: Scratch,
    cache: &'w mut LocalEventsCache,
    out: &'w mut Vec<Detection>,
}

impl Walk<'_, '_> {
    /// The host's distance from the observer at `t`.
    #[must_use]
    fn distance_at(&self, host: &Host<'_>, t: UniverseTime) -> Metres {
        self.query
            .observer_position()
            .distance_to(&host.line.position_at(t))
    }

    /// When the light the host emits at `t` arrives: t + |`x_s(t)` − `x_o`| ÷ c (Design note 6).
    #[must_use]
    fn arrival(&self, host: &Host<'_>, t: UniverseTime) -> UniverseTime {
        t.checked_add(light_time(self.distance_at(host, t)))
            .expect("a light time across the cube keeps a time on the clock")
    }

    /// When the light that arrives at the window's start left the host: the light-cone step
    /// iterated until it stands still to the nanosecond, or [`MAX_LIGHT_STEPS`] times, found once.
    fn start_emission(&self, host: &mut Host<'_>) -> UniverseTime {
        if let Some(emitted) = host.start_emission {
            return emitted;
        }
        let arrival = self.query.window().start();
        let back = |t: UniverseTime| {
            arrival
                .checked_sub(light_time(self.distance_at(host, t)))
                .expect("a light time across the cube keeps a time on the clock")
        };
        let mut emitted = back(arrival);
        for _ in 0..MAX_LIGHT_STEPS {
            let next = back(emitted);
            if next == emitted {
                break;
            }
            emitted = next;
        }
        *host.start_emission.insert(emitted)
    }

    /// The line of sight from where the host's line passes at the epoch to the observer, and each
    /// lit kind's best band through it, marched on first use (module documentation, "The light").
    fn sightline(&mut self, host: &mut Host<'_>) -> (Sightline, BestBands) {
        if let Some(seen) = host.sightline {
            return seen;
        }
        let sightline = self.ctx.sightline(
            self.galaxy,
            host.line.epoch_position(),
            self.query.observer_position(),
        );
        let best = Lit::ALL.map(|kind| self.lights.light(kind).best_band(self.query, &sightline));
        *host.sightline.insert((sightline, best))
    }

    /// Lists the local events of `star`, whose body is `body`, of `host`.
    fn star(&mut self, host: &mut Host<'_>, star: &StarModel, body: BodyId) {
        let seed = self.galaxy.seed();
        let kept = self.cache.star(star, body);
        if self.lights.flare.best_bare_m2_per_w > 0.0
            && let Some(rates) = kept.flares
        {
            let series = EventSeries::new(seed, tags::STAR_FLARE, body.into());
            let mut listed = std::mem::take(&mut self.scratch.flares);
            listed.clear();
            let onsets = host.onsets(longest(TransientKind::Flare));
            let model = FlareModel::with_rates(star, rates);
            model.flares_in(&series, onsets, &mut self.scratch.bin_events, &mut listed);
            let quiescent = rates.quiescent_luminosity().value() * SOLAR_LUMINOSITY_W;
            for &one in &listed {
                self.lit(host, &LitEvent::Flare(one, Watts::new(quiescent)));
            }
            self.scratch.flares = listed;
        }
        let onsets = host.onsets(longest(TransientKind::FuOrionisOutburst));
        // The listing's own cut to the span of the star's outbursts, made first from the kept span
        // so that a star too old for any builds no model.
        if self.lights.outburst.best_bare_m2_per_w > 0.0
            && kept.outbursts.is_some_and(|(formation, last)| {
                fu_orionis::active_span(onsets, formation, last).is_some()
            })
        {
            let series = EventSeries::new(seed, tags::STAR_FU_ORIONIS, body.into());
            let mut listed = std::mem::take(&mut self.scratch.outbursts);
            listed.clear();
            fu_orionis::outbursts_in(star, &series, onsets, &mut listed);
            for &one in &listed {
                // The quiescent light is the state the outburst's Eddington cap reads; a star with
                // none by then forms after +H, which no star of the galaxy does.
                let Some(state) = star.state_at(one.onset().min(ClockWindow::END)) else {
                    continue;
                };
                let quiescent = Watts::new(state.luminosity().value() * SOLAR_LUMINOSITY_W);
                self.lit(host, &LitEvent::Outburst(one, quiescent));
            }
            self.scratch.outbursts = listed;
        }
        if self.lights.radio && star.neutron_star().is_some() {
            let onsets = host.onsets(longest(TransientKind::Glitch));
            let series = EventSeries::new(seed, tags::STAR_GLITCH, body.into());
            let mut small = std::mem::take(&mut self.scratch.glitches);
            let mut large = std::mem::take(&mut self.scratch.large_glitches);
            small.clear();
            large.clear();
            glitches::glitches_in(star, &series, onsets, &mut small);
            glitches::large_glitches_in(star, seed, body, onsets, &mut large);
            let listed = small.iter().map(|g| (g.id(), g.onset()));
            for (id, onset) in listed.chain(large.iter().map(|g| (g.id(), g.onset()))) {
                self.glitch(host, star, id, onset);
            }
            self.scratch.glitches = small;
            self.scratch.large_glitches = large;
        }
    }

    /// Reports `event` if its light arrives in the window above the detection limit: an onset by
    /// its peak, an event in progress by its light at the window's start (module documentation,
    /// "The light").
    fn lit(&mut self, host: &mut Host<'_>, event: &LitEvent) {
        let kind = event.lit();
        let light = self.lights.light(kind);
        // Not even the peak, at the least distance the host can have, reaches the limit.
        if !light.stands_over(event.peak_light(), host.least_distance) {
            return;
        }
        let window = self.query.window();
        let arrival = self.arrival(host, event.onset());
        if arrival >= window.end() {
            return;
        }
        let (phase, counted, seen) = if arrival >= window.start() {
            let peak = event.peak();
            (DetectionPhase::Onset, arrival, (event.peak_light(), peak))
        } else {
            let start = self.start_emission(host);
            if !event.is_active_at(start) {
                return;
            }
            let opening = (event.light_at(start), start);
            (DetectionPhase::InProgress, window.start(), opening)
        };
        let (watts, emitted) = seen;
        let distance = self.distance_at(host, emitted);
        if !light.stands_over(watts, distance) {
            return;
        }
        let (sightline, best) = self.sightline(host);
        let Some((band, limits)) = best[kind.index()] else {
            return;
        };
        let share = light.shares[band.index()];
        let in_band = Watts::new(watts.value() * share);
        let Some(flux) = flux(in_band, distance, &sightline, band) else {
            return;
        };
        if flux < limits.detect() {
            return;
        }
        // The light held at the detection's own: its emission, distance and line of sight.
        let delay = light_delay(phase, window, emitted, distance);
        let arriving = event.arriving(band, share, delay, distance, transmission(&sightline, band));
        let seen = Seen {
            event: event.id(),
            kind: event.kind(),
            emitted: event.onset(),
            arrival,
            phase,
            band,
            flux,
            resolvable: flux >= limits.resolve(),
            light: DetectionLight::Lit(arriving),
        };
        self.push(host, &seen, counted);
    }

    /// Reports the glitch `id` of `star` at `onset` if a radio sensor times it in the window
    /// (module documentation, "Glitches").
    fn glitch(&mut self, host: &Host<'_>, star: &StarModel, id: EventId, onset: UniverseTime) {
        let arrival = self.arrival(host, onset);
        if !self.query.window().contains(arrival) {
            return;
        }
        let Some(pulsar) = star.pulsar_at(onset) else {
            return;
        };
        if !pulsar.is_radio_alive() {
            return;
        }
        let towards = host
            .line
            .position_at(onset)
            .displacement_to(self.query.observer_position());
        let beamed = UnitVector::from_components(towards.metres())
            .is_some_and(|direction| pulsar.beam().sweeps(direction));
        if !beamed {
            return;
        }
        let seen = Seen {
            event: id,
            kind: TransientKind::Glitch,
            emitted: onset,
            arrival,
            phase: DetectionPhase::Onset,
            band: AlertBand::Photometric(Band::Radio),
            flux: WattsPerSquareMetre::new(0.0),
            resolvable: false,
            light: DetectionLight::Unlit,
        };
        self.push(host, &seen, arrival);
    }

    /// Appends the detection of `seen`, whose light is first counted at `at`: an onset's arrival,
    /// or the window's start for an event in progress.
    ///
    /// Its retardation is the onset's light, as a one-shot detection's
    /// ([`Retardation::of_emission`]): emitted at the onset from where the host's line passes then,
    /// it arrives at [`Detection::arrival`] to the bit. Its bearing is the light's first counted,
    /// [`retarded`]'s at `at`: for an event in progress, where the host appears as the window
    /// opens. A host at the observer has no bearing and is not reported.
    fn push(&mut self, host: &Host<'_>, seen: &Seen, at: UniverseTime) {
        let position = self.query.observer_position();
        let observer = Observer::new(*position, at)
            .expect("an alert query's observer is in the cube and its window in the clock window");
        let counted = retarded(&observer, &host.line);
        let Some(bearing) = bearing(position, counted.apparent_position()) else {
            return;
        };
        let retardation = Retardation::of_emission(
            position,
            seen.emitted,
            host.line.position_at(seen.emitted),
            host.line.velocity(),
            &host.line.position_at(seen.arrival),
        );
        debug_assert_eq!(retardation.observed(), seen.arrival);
        self.out.push(Detection::new(
            seen.event,
            host.observed.hit().id(),
            None,
            seen.kind,
            seen.phase,
            seen.band,
            seen.flux,
            bearing,
            retardation,
            seen.resolvable,
            seen.light,
        ));
    }
}

/// How long the light the walk read takes to the observer: from `emitted`, `distance` away, to
/// the onset's peak arriving, or, for an event in progress, to the window's start, which the light
/// that left at `emitted` reaches by the light-cone step.
#[must_use]
fn light_delay(
    phase: DetectionPhase,
    window: TimeWindow,
    emitted: UniverseTime,
    distance: Metres,
) -> Span {
    match phase {
        DetectionPhase::Onset => light_time(distance),
        DetectionPhase::InProgress => window
            .start()
            .checked_since(emitted)
            .expect("two clock times are a span apart"),
    }
}

/// The longest an event of the local `kind` lasts: its registry row's D.
///
/// # Panics
///
/// For a kind whose row is pending or unending, which no kind listed here is.
#[must_use]
fn longest(kind: TransientKind) -> Span {
    match kind.row().duration() {
        Some(LongestDuration::AtMost(span)) => span,
        other @ (Some(LongestDuration::Unending) | None) => {
            unreachable!("a built local row has an end, not {other:?}")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::coords::{GalacticDisplacement, GalacticPosition};
    use crate::galaxy::features::members::NoInteriorCache;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellKey, NoCache, SystemOrigin, SystemRecord, generate_cell};
    use crate::galaxy::query::{
        QueryMode, RangeQuery, SystemHit, range_query, range_query_observed,
    };
    use crate::galaxy::rules::GalaxyRules;
    use crate::id::{Layer, SystemId};
    use crate::math;
    use crate::observe::{Bearing, NoStarsCache, observe_hit, stars_of};
    use crate::rng::Seed;
    use crate::stellar::events::flares;
    use crate::stellar::remnant::DeathKind;
    use crate::stellar::system::{ClockDeath, SystemStars};
    use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};
    use crate::units::{Kelvin, LightYears, Years};

    use super::super::{SensorHorizon, band_share};

    /// The stored shares are T5.b's blackbodies through the top hats ([`band_share`]) to 10⁻¹² of
    /// each, and keep the bits of the integral they were first stored from: 10,000 K for flares,
    /// 6,500 K to I and 4,000 K from J for outbursts, nothing in radio or X-rays, and under the
    /// whole light. A 10,000 K blackbody holds 0.087 of it in V, as the research advice for
    /// P12.T5.b has it.
    #[test]
    fn the_band_shares_are_blackbodies_through_the_top_hats() {
        let flare = |_| 10_000.0;
        let outburst = |band| if band < Band::J { 6_500.0 } else { 4_000.0 };
        for (name, stored, kelvin) in [
            ("flare", FLARE_SHARES, &flare as &dyn Fn(Band) -> f64),
            ("outburst", OUTBURST_SHARES, &outburst),
        ] {
            for (i, band) in AlertBand::ALL.into_iter().enumerate() {
                let share = match band {
                    AlertBand::Photometric(photometric) => {
                        band_share(Kelvin::new(kelvin(photometric)), band)
                    }
                    AlertBand::XRay => 0.0,
                };
                if share > 0.0 {
                    assert!(
                        (stored[i] / share - 1.0).abs() <= 1e-12,
                        "{name} in {band:?}: {:e} stored against {share:e}",
                        stored[i]
                    );
                } else {
                    assert_same_bits(stored[i], 0.0);
                }
            }
            assert!(stored.iter().sum::<f64>() < 1.0, "{name}");
        }
        let v = AlertBand::Photometric(Band::V).index();
        assert!(
            (FLARE_SHARES[v] - 0.087).abs() < 5e-4,
            "{}",
            FLARE_SHARES[v]
        );
    }

    /// The Milky Way fixture with its kinematic tables, so that every system moves on its own
    /// line, built once per process: some 5 s in a test build, so the properties that read it run
    /// in one test.
    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x125e_0000), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral")
                .with_full_potential()
        })
    }

    fn years(y: i64) -> UniverseTime {
        UniverseTime::from_julian_years(y).unwrap()
    }

    fn julian_years(y: f64) -> Span {
        Span::from_seconds_f64(y * 31_557_600.0).unwrap()
    }

    /// A sensor that sees faint light in V and K and times pulsars in the radio.
    fn sensor() -> SensorHorizon {
        let w = WattsPerSquareMetre::new;
        let limits = |detect, resolve| BandLimits::new(w(detect), w(resolve)).unwrap();
        SensorHorizon::new([
            (AlertBand::Photometric(Band::V), limits(1e-16, 1e-13)),
            (AlertBand::Photometric(Band::K), limits(3e-16, 1e-13)),
            (AlertBand::Photometric(Band::Radio), limits(1e-28, 1e-26)),
        ])
        .unwrap()
    }

    /// Keeps the stars of every system it lends, for one galaxy; a re-aged system's are put in
    /// first.
    #[derive(Default)]
    struct KeepStars(BTreeMap<SystemId, SystemStars>);

    impl StarsCache for KeepStars {
        fn with_stars<R>(
            &mut self,
            galaxy: &Galaxy,
            record: &SystemRecord,
            f: impl FnOnce(&SystemStars) -> R,
        ) -> R {
            f(self.0.entry(record.id()).or_insert_with(|| {
                stars_of(galaxy, &NoInteriorCache, record).expect(
                    "the observed query asks for no catalogue system's stars (`StarsCache`)",
                )
            }))
        }
    }

    /// An alert query and the observed rows it is asked of, with their stars.
    struct Scene {
        query: AlertQuery,
        rows: Vec<ObservedSystem>,
        stars: RefCell<KeepStars>,
    }

    /// The neighbours of a point like the Sun's, 12 ly about it, observed 120 years after the epoch
    /// at `now`: their rows and stars.
    fn neighbours_at(now: UniverseTime) -> (Vec<ObservedSystem>, KeepStars) {
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let range = RangeQuery::builder(sun, LightYears::new(12.0))
            .time(now)
            .mode(QueryMode::ObservedFrom(sun))
            .build()
            .unwrap();
        let mut stars = KeepStars::default();
        let found =
            range_query_observed(galaxy(), &mut NoCache::new(), &mut stars, &[], &range).unwrap();
        (found.observed().to_vec(), stars)
    }

    /// The neighbours of [`neighbours_at`] 120 years after the epoch, asked about ten days that
    /// open a minute after the onset of the flare that stands highest over the limit in the ten
    /// days from three hours later: their flares, starting and in progress.
    fn neighbours() -> Scene {
        let galaxy = galaxy();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let now = years(120);
        let (rows, mut stars) = neighbours_at(now);
        let ten_days = |opens: UniverseTime| {
            let closes = opens.checked_add(Span::from_seconds(10 * 86_400)).unwrap();
            AlertQuery::builder(sun, TimeWindow::new(opens, closes).unwrap(), sensor())
                .build()
                .unwrap()
        };
        let first = ten_days(now.checked_add(Span::from_seconds(3 * 3_600 + 17)).unwrap());
        let mut noise = NoiseCache::with_capacity(256);
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
        let mut seen = Vec::new();
        let mut cache = LocalEventsCache::with_capacity(256);
        local_events(
            galaxy, &mut ctx, &first, &rows, &mut stars, &mut cache, &mut seen,
        );
        let over = |d: &Detection| {
            d.flux().value() / first.sensor().limits(d.band()).unwrap().detect().value()
        };
        let brightest = seen
            .iter()
            .max_by(|a, b| over(a).total_cmp(&over(b)))
            .expect("the neighbours flare in ten days");
        let opens = brightest
            .arrival()
            .checked_add(Span::from_seconds(60))
            .unwrap();
        Scene {
            query: ten_days(opens),
            rows,
            stars: RefCell::new(stars),
        }
    }

    /// A layer-E core collapse near the solar circle re-aged so that it died 3,000 years before
    /// the epoch, leaving a young pulsar that glitches and shines in the radio at the epoch: its
    /// record and stars.
    fn young_pulsar() -> (SystemRecord, SystemStars) {
        let galaxy = galaxy();
        let mut cell = Vec::new();
        for x in -40..40 {
            generate_cell(
                galaxy,
                CellKey::new(Layer::E, [x, 203, 0]).unwrap(),
                &mut cell,
            );
            for grown in &cell {
                let ClockDeath::At(death, DeathKind::CoreCollapse { .. }) =
                    SystemStars::generate(galaxy, grown).death_time()
                else {
                    continue;
                };
                let lifetime =
                    death.since_epoch().as_julian_years_f64() + grown.age_at_epoch().value();
                let record = SystemRecord::from_parts(
                    grown.id(),
                    *grown.epoch_position(),
                    grown.origin(),
                    grown.population(),
                    grown.primary_initial_mass(),
                    Years::new(lifetime + 3_000.0),
                );
                let stars = SystemStars::generate(galaxy, &record);
                let star = stars.primary();
                if glitches::GlitchModel::of(star).is_some()
                    && star
                        .pulsar_at(UniverseTime::EPOCH)
                        .is_some_and(|pulsar| pulsar.is_radio_alive())
                {
                    return (record, stars);
                }
            }
        }
        panic!("no layer-E core collapse at the solar circle leaves a glitching radio pulsar");
    }

    /// The `i`-th of `n` directions spread evenly over the sphere (a Fibonacci lattice).
    fn direction(i: u32, n: u32) -> UnitVector {
        let z = 1.0 - (2.0 * f64::from(i) + 1.0) / f64::from(n);
        let r = (1.0 - z * z).sqrt();
        let phi = f64::from(i) * 2.399_963_229_728_653;
        UnitVector::from_components([r * math::cos(phi), r * math::sin(phi), z]).unwrap()
    }

    /// The young pulsar, observed from 30 ly out along a direction its beam sweeps throughout the
    /// clock window, and a grid star within 10 ly of that observer re-aged to a Class 0 protostar,
    /// all seen at the epoch and asked about 150 years from a third of the way through one of the
    /// protostar's outbursts: glitches, and outbursts in progress and starting.
    fn young_and_dead() -> Scene {
        let galaxy = galaxy();
        let (pulsar_record, pulsar_stars) = young_pulsar();
        let pulsar_line = Drift::of_record(galaxy, &pulsar_record).unwrap();
        let star = pulsar_stars.primary();
        let at = |d: UnitVector| {
            let ly = d.components().map(|c| c * 30.0 * METRES_PER_LIGHT_YEAR);
            pulsar_line
                .position_at(UniverseTime::EPOCH)
                .translated(GalacticDisplacement::new(ly))
                .unwrap()
        };
        let beamed = |observer: &GalacticPosition| {
            (-10..=10).all(|century| {
                let t = years(100 * century);
                let towards = pulsar_line.position_at(t).displacement_to(observer);
                let pulsar = star.pulsar_at(t).unwrap();
                UnitVector::from_components(towards.metres())
                    .is_some_and(|d| pulsar.beam().sweeps(d))
            })
        };
        let observer = (0..512)
            .map(|i| at(direction(i, 512)))
            .find(beamed)
            .expect("a young pulsar's beams sweep a band of the sky");

        let near = range_query(
            galaxy,
            &mut NoCache::new(),
            &[],
            &RangeQuery::builder(observer, LightYears::new(10.0))
                .build()
                .unwrap(),
        );
        let (young, young_stars, window) = near
            .systems()
            .iter()
            .map(SystemHit::record)
            .filter(|record| {
                record.kind() == SystemKind::Stellar
                    && matches!(record.origin(), SystemOrigin::Grid(_))
                    && (0.3..2.0).contains(&record.primary_initial_mass().value())
            })
            .find_map(|grown| {
                let young = SystemRecord::from_parts(
                    grown.id(),
                    *grown.epoch_position(),
                    grown.origin(),
                    grown.population(),
                    grown.primary_initial_mass(),
                    Years::new(5e4),
                );
                let stars = SystemStars::generate(galaxy, &young);
                let line = Drift::of_record(galaxy, &young).unwrap();
                let body = stars.hierarchy().stars()[0].body();
                let series = EventSeries::new(galaxy.seed(), tags::STAR_FU_ORIONIS, body.into());
                let mut outbursts = Vec::new();
                let span = TimeWindow::new(years(-900), years(700)).unwrap();
                fu_orionis::outbursts_in(stars.primary(), &series, span, &mut outbursts);
                let long = outbursts
                    .iter()
                    .find(|o| o.duration() > julian_years(8.0))?;
                let light = light_time(observer.distance_to(&line.position_at(long.onset())));
                let third = Span::from_seconds_f64(long.duration().as_seconds_f64() / 3.0)?;
                let opens = long.onset().checked_add(light)?.checked_add(third)?;
                let window =
                    TimeWindow::new(opens, opens.checked_add(julian_years(150.0))?).ok()?;
                Some((young, stars, window))
            })
            .expect("some star near the observer, re-aged, has an outburst of 8 years or more");

        let mut stars = KeepStars::default();
        let seen = Observer::new(observer, UniverseTime::EPOCH).unwrap();
        let rows = [(pulsar_record, pulsar_stars), (young, young_stars)]
            .map(|(record, system)| {
                let line = Drift::of_record(galaxy, &record).unwrap();
                let position = line.position_at(UniverseTime::EPOCH);
                let distance = LightYears::from(observer.distance_to(&position));
                let hit = SystemHit::new(record, position, distance);
                let row = observe_hit(galaxy, &hit, &system, &seen).unwrap();
                stars.0.insert(record.id(), system);
                row
            })
            .to_vec();
        Scene {
            query: AlertQuery::builder(observer, window, sensor())
                .build()
                .unwrap(),
            rows,
            stars: RefCell::new(stars),
        }
    }

    /// A detection's parts as the direct evaluation finds them.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Direct {
        event: EventId,
        kind: TransientKind,
        emitted: UniverseTime,
        arrival: UniverseTime,
        phase: DetectionPhase,
        band: AlertBand,
        flux: f64,
        resolvable: bool,
        /// Where the light first counted comes from: an onset's at its arrival, an event's in
        /// progress at the window's start.
        bearing: Bearing,
    }

    impl From<&Detection> for Direct {
        fn from(d: &Detection) -> Self {
            Self {
                event: d.event(),
                kind: d.kind(),
                emitted: d.emitted(),
                arrival: d.arrival(),
                phase: d.phase(),
                band: d.band(),
                flux: d.flux().value(),
                resolvable: d.is_resolvable(),
                bearing: d.bearing(),
            }
        }
    }

    /// The emission whose light arrives at `t`, by bisection on `arrival`, which rises with the
    /// emitted time: the first nanosecond whose light arrives at or after `t`.
    fn bisect(
        arrival: &dyn Fn(UniverseTime) -> UniverseTime,
        t: UniverseTime,
        mut lo: UniverseTime,
        mut hi: UniverseTime,
    ) -> UniverseTime {
        assert!(arrival(lo) < t && arrival(hi) >= t);
        loop {
            let gap = hi.checked_since(lo).unwrap();
            if gap <= Span::new(0, 1).unwrap() {
                return hi;
            }
            let half = Span::from_seconds_f64(gap.as_seconds_f64() / 2.0).unwrap();
            let mid = lo.checked_add(half).unwrap();
            let mid = if mid <= lo {
                lo.checked_add(Span::new(0, 1).unwrap()).unwrap()
            } else {
                mid
            };
            if arrival(mid) < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }

    /// The direct evaluation of one row: the system's own line ([`Drift::of_record`]), a span of
    /// onsets well beyond its retarded interval, the light that arrives at the window's start by
    /// bisection, and its sightline.
    struct Oracle<'a> {
        query: &'a AlertQuery,
        line: Drift,
        /// The row's light time at its own time, s.
        light: f64,
        /// How far the light time can move over the window, s, three times over and a day more.
        margin: f64,
        start_light: UniverseTime,
        sightline: Sightline,
    }

    impl<'a> Oracle<'a> {
        fn new(
            galaxy: &Galaxy,
            ctx: &mut AlertContext<'_>,
            query: &'a AlertQuery,
            row: &ObservedSystem,
        ) -> Self {
            let window = query.window();
            let far = |t: UniverseTime| {
                t.checked_since(row.retardation().observed())
                    .unwrap()
                    .as_seconds_f64()
                    .abs()
            };
            let light = row.retardation().distance_now().value() * SECONDS_PER_JULIAN_YEAR;
            let furthest = far(window.start()).max(far(window.end()));
            let mut oracle = Self {
                query,
                line: Drift::of_record(galaxy, row.hit().record()).unwrap(),
                light,
                margin: 3.0 * 1e6 / SPEED_OF_LIGHT * (furthest + light) + 86_400.0,
                start_light: UniverseTime::EPOCH,
                sightline: ctx.sightline(
                    galaxy,
                    row.hit().record().epoch_position(),
                    query.observer_position(),
                ),
            };
            let t = window.start();
            oracle.start_light = bisect(&|e| oracle.arrival(e), t, oracle.early(t), oracle.late(t));
            oracle
        }

        fn distance(&self, t: UniverseTime) -> Metres {
            self.query
                .observer_position()
                .distance_to(&self.line.position_at(t))
        }

        fn arrival(&self, t: UniverseTime) -> UniverseTime {
            t.checked_add(light_time(self.distance(t))).unwrap()
        }

        /// Where the light that arrives at `t` comes from.
        fn bearing_at(&self, t: UniverseTime) -> Bearing {
            let position = self.query.observer_position();
            let light = retarded(&Observer::new(*position, t).unwrap(), &self.line);
            bearing(position, light.apparent_position()).unwrap()
        }

        /// The earliest emission whose light can arrive at `t`.
        fn early(&self, t: UniverseTime) -> UniverseTime {
            t.checked_sub(Span::from_seconds_f64(self.light + self.margin).unwrap())
                .unwrap()
        }

        /// The latest emission whose light can arrive at `t`.
        fn late(&self, t: UniverseTime) -> UniverseTime {
            let back = (self.light - self.margin).max(0.0);
            t.checked_sub(Span::from_seconds_f64(back).unwrap())
                .unwrap()
        }

        /// The onsets to list of a kind whose events last at most `longest`.
        fn span(&self, longest: Span) -> TimeWindow {
            let window = self.query.window();
            let start = self.early(window.start()).checked_sub(longest).unwrap();
            TimeWindow::new(start, self.late(window.end())).unwrap()
        }

        /// `event`, of a kind lit by `shares`, if its light arrives in the window above the limit:
        /// an onset seen by its peak, an event in progress by the light that arrives at t₁.
        fn lit(&self, event: &LitEvent, shares: &[f64; BAND_COUNT], out: &mut Vec<Direct>) {
            let window = self.query.window();
            let onset = event.onset();
            let arrival = self.arrival(onset);
            let (phase, light, emitted, counted_at) = if window.contains(arrival) {
                let peak = event.peak();
                let factor_less_one = match event {
                    LitEvent::Flare(flare, _) => flare.peak_factor() - 1.0,
                    LitEvent::Outburst(outburst, _) => outburst.peak_factor() - 1.0,
                };
                let quiescent = match event {
                    LitEvent::Flare(_, q) | LitEvent::Outburst(_, q) => q.value(),
                };
                (
                    DetectionPhase::Onset,
                    factor_less_one * quiescent,
                    peak,
                    arrival,
                )
            } else if arrival < window.start() && event.is_active_at(self.start_light) {
                let light = event.light_at(self.start_light).value();
                (
                    DetectionPhase::InProgress,
                    light,
                    self.start_light,
                    window.start(),
                )
            } else {
                return;
            };
            // The band in which the event stands highest over the limit, the first on a tie.
            let mut best: Option<(AlertBand, BandLimits, f64)> = None;
            for (band, limits) in self.query.sensor().bands() {
                let share = shares[band.index()];
                let over = share * transmission(&self.sightline, band) / limits.detect().value();
                if share > 0.0 && best.is_none_or(|(.., most)| over > most) {
                    best = Some((band, limits, over));
                }
            }
            let Some((band, limits, _)) = best else {
                return;
            };
            let watts = Watts::new(light * shares[band.index()]);
            let flux = flux(watts, self.distance(emitted), &self.sightline, band)
                .unwrap()
                .value();
            if flux >= limits.detect().value() {
                out.push(Direct {
                    event: event.id(),
                    kind: event.kind(),
                    emitted: onset,
                    arrival,
                    phase,
                    band,
                    flux,
                    resolvable: flux >= limits.resolve().value(),
                    bearing: self.bearing_at(counted_at),
                });
            }
        }

        /// The glitch `id` of `star` at `onset`, if it is timed in the radio in the window.
        fn glitch(
            &self,
            star: &StarModel,
            id: EventId,
            onset: UniverseTime,
            out: &mut Vec<Direct>,
        ) {
            let arrival = self.arrival(onset);
            let pulsar = star.pulsar_at(onset).unwrap();
            let towards = self
                .line
                .position_at(onset)
                .displacement_to(self.query.observer_position());
            let beamed = pulsar
                .beam()
                .sweeps(UnitVector::from_components(towards.metres()).unwrap());
            if self.query.window().contains(arrival) && pulsar.is_radio_alive() && beamed {
                out.push(Direct {
                    event: id,
                    kind: TransientKind::Glitch,
                    emitted: onset,
                    arrival,
                    phase: DetectionPhase::Onset,
                    band: AlertBand::Photometric(Band::Radio),
                    flux: 0.0,
                    resolvable: false,
                    bearing: self.bearing_at(arrival),
                });
            }
        }
    }

    /// The local events of `row`, whose stars are `stars`, evaluated directly ([`Oracle`]): every
    /// event plan 06 lists over the oracle's span, each tested on its own, its flare model's
    /// quiescent luminosity read from the model.
    fn direct(
        galaxy: &Galaxy,
        ctx: &mut AlertContext<'_>,
        query: &AlertQuery,
        row: &ObservedSystem,
        stars: &SystemStars,
        out: &mut Vec<Direct>,
    ) {
        let oracle = Oracle::new(galaxy, ctx, query, row);
        let seed = galaxy.seed();
        let watts = |l: crate::units::SolarLuminosities| Watts::new(l.value() * SOLAR_LUMINOSITY_W);
        for (star, slot) in stars.stars().iter().zip(stars.hierarchy().stars()) {
            let body = slot.body();
            if let Some(model) = FlareModel::of(star) {
                let mut listed = Vec::new();
                let series = EventSeries::new(seed, tags::STAR_FLARE, body.into());
                let span = oracle.span(flares::MAX_DURATION);
                flares::flares_in(star, &series, span, &mut listed);
                let quiescent = watts(model.quiescent_luminosity());
                for flare in listed {
                    oracle.lit(&LitEvent::Flare(flare, quiescent), &FLARE_SHARES, out);
                }
            }
            let mut listed = Vec::new();
            let series = EventSeries::new(seed, tags::STAR_FU_ORIONIS, body.into());
            let span = oracle.span(fu_orionis::MAX_DURATION);
            fu_orionis::outbursts_in(star, &series, span, &mut listed);
            for outburst in listed {
                let state = star
                    .state_at(outburst.onset().min(ClockWindow::END))
                    .unwrap();
                let quiescent = watts(state.luminosity());
                oracle.lit(
                    &LitEvent::Outburst(outburst, quiescent),
                    &OUTBURST_SHARES,
                    out,
                );
            }
            let (mut small, mut large) = (Vec::new(), Vec::new());
            let series = EventSeries::new(seed, tags::STAR_GLITCH, body.into());
            glitches::glitches_in(star, &series, oracle.span(Span::ZERO), &mut small);
            glitches::large_glitches_in(star, seed, body, oracle.span(Span::ZERO), &mut large);
            for (id, onset) in small.iter().map(|g| (g.id(), g.onset())) {
                oracle.glitch(star, id, onset, out);
            }
            for (id, onset) in large.iter().map(|g| (g.id(), g.onset())) {
                oracle.glitch(star, id, onset, out);
            }
        }
    }

    /// `local_events` over `scene` asked of `query`, in a fresh context with a fresh cache.
    fn run(scene: &Scene, query: &AlertQuery, rows: &[ObservedSystem]) -> Vec<Detection> {
        run_with(
            scene,
            query,
            rows,
            &mut LocalEventsCache::with_capacity(4_096),
        )
    }

    /// `local_events` over `scene` asked of `query`, in a fresh context with `cache`.
    fn run_with(
        scene: &Scene,
        query: &AlertQuery,
        rows: &[ObservedSystem],
        cache: &mut LocalEventsCache,
    ) -> Vec<Detection> {
        let mut noise = NoiseCache::with_capacity(256);
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
        let mut out = Vec::new();
        let mut stars = scene.stars.borrow_mut();
        local_events(
            galaxy(),
            &mut ctx,
            query,
            rows,
            &mut *stars,
            cache,
            &mut out,
        );
        out
    }

    /// `local_events` over the scene equals each row's direct evaluation: the same events, kinds,
    /// phases, bands, emitted and arrival times, resolvability, bearings and onsets' fluxes to the
    /// bit, and in-progress fluxes within 10⁻⁹, since the light at t₁ is found by bisection here.
    fn equals_direct_evaluation(scene: &Scene) -> Vec<Detection> {
        let galaxy = galaxy();
        let out = run(scene, &scene.query, &scene.rows);
        let mut expected = Vec::new();
        let mut noise = NoiseCache::with_capacity(256);
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
        let mut stars = scene.stars.borrow_mut();
        for row in &scene.rows {
            if row.hit().record().kind() == SystemKind::RoguePlanet {
                continue;
            }
            stars.with_stars(galaxy, row.hit().record(), |system| {
                direct(galaxy, &mut ctx, &scene.query, row, system, &mut expected);
            });
        }
        expected.sort_by(|a, b| a.arrival.cmp(&b.arrival).then(a.event.cmp(&b.event)));
        let found: Vec<Direct> = out.iter().map(Direct::from).collect();
        let exact = |d: &Direct| (d.event, d.kind, d.phase, d.band, d.emitted, d.arrival);
        assert_eq!(
            found.iter().map(exact).collect::<Vec<_>>(),
            expected.iter().map(exact).collect::<Vec<_>>(),
        );
        for (d, e) in found.iter().zip(&expected) {
            assert_eq!(
                (d.resolvable, d.bearing),
                (e.resolvable, e.bearing),
                "{d:?}"
            );
            match d.phase {
                DetectionPhase::Onset => assert_same_bits(d.flux, e.flux),
                DetectionPhase::InProgress => {
                    assert!(
                        (d.flux - e.flux).abs() <= 1e-9 * e.flux,
                        "{d:?} against {e:?}"
                    );
                }
            }
        }
        out
    }

    /// The result does not depend on the order of the rows, or on what the context's noise cache,
    /// the stars cache and the local events' cache held, or on the last's size: each row alone, in
    /// any order, gives its own detections, and the whole in any order is their union in arrival
    /// order, through caches of any size, cold or warm. What `out` held before is kept.
    fn order_independent(scene: &Scene, whole: &[Detection]) {
        let galaxy = galaxy();
        let noise = RefCell::new(NoiseCache::with_capacity(256));
        let cache = RefCell::new(LocalEventsCache::with_capacity(64));
        let warm = |rows: &[ObservedSystem]| {
            let mut noise = noise.borrow_mut();
            let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
            let mut out = Vec::new();
            let mut stars = scene.stars.borrow_mut();
            let mut cache = cache.borrow_mut();
            local_events(
                galaxy,
                &mut ctx,
                &scene.query,
                rows,
                &mut *stars,
                &mut cache,
                &mut out,
            );
            out
        };
        for entries in [0, 1, 7, 4_096] {
            let mut cold = LocalEventsCache::with_capacity(entries);
            assert_eq!(run_with(scene, &scene.query, &scene.rows, &mut cold), whole);
            assert_eq!(run_with(scene, &scene.query, &scene.rows, &mut cold), whole);
            cold.clear();
            assert_eq!(run_with(scene, &scene.query, &scene.rows, &mut cold), whole);
        }
        assert_order_independent(&scene.rows, |row| warm(std::slice::from_ref(row)));
        let mut each: Vec<Detection> = scene
            .rows
            .iter()
            .flat_map(|row| warm(std::slice::from_ref(row)))
            .collect();
        each.sort_by(|a, b| {
            a.arrival()
                .cmp(&b.arrival())
                .then(a.event().cmp(&b.event()))
        });
        assert_eq!(each, whole);
        let mut reversed = scene.rows.clone();
        reversed.reverse();
        assert_eq!(warm(&reversed), whole);
        let mut kept = whole.to_vec();
        let mut cold = NoiseCache::with_capacity(16);
        let mut ctx = AlertContext::new(&mut cold, &NoModifiers);
        let mut stars = scene.stars.borrow_mut();
        let mut cache = cache.borrow_mut();
        local_events(
            galaxy,
            &mut ctx,
            &scene.query,
            &reversed,
            &mut *stars,
            &mut cache,
            &mut kept,
        );
        assert_eq!(kept[..whole.len()], *whole);
        assert_eq!(kept[whole.len()..], *whole);
    }

    /// Design note 6: the window split at an instant gives each onset in the part its onset arrives
    /// in, as the whole does, to the bit; the first part's events in progress are the whole's; and
    /// the second part's are the events still on at the split, which the whole holds as onsets or
    /// in progress.
    fn a_split_window_finds_the_wholes_onsets(
        scene: &Scene,
        whole: &[Detection],
        split: UniverseTime,
    ) {
        let window = scene.query.window();
        let part = |start, end| {
            let query = AlertQuery::builder(
                *scene.query.observer_position(),
                TimeWindow::new(start, end).unwrap(),
                scene.query.sensor().clone(),
            )
            .build()
            .unwrap();
            run(scene, &query, &scene.rows)
        };
        let (first, second) = (part(window.start(), split), part(split, window.end()));
        let onsets = |found: &[Detection]| {
            found
                .iter()
                .filter(|d| d.phase() == DetectionPhase::Onset)
                .cloned()
                .collect::<Vec<_>>()
        };
        let mut parts = onsets(&first);
        parts.extend(onsets(&second));
        assert_eq!(parts, onsets(whole));
        let in_progress = |found: &[Detection]| {
            found
                .iter()
                .filter(|d| d.phase() == DetectionPhase::InProgress)
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(in_progress(&first), in_progress(whole));
        for d in in_progress(&second) {
            assert!(whole.iter().any(|w| w.event() == d.event()), "{d:?}");
        }
    }

    /// P12.T5.e: the local events of an observed range result are each system's own, evaluated
    /// directly, and do not depend on the order of asking or on how the window is split: the
    /// neighbours' flares over ten days, starting and in progress, and a young pulsar's glitches
    /// and a protostar's outbursts, starting and in progress, over 150 years.
    /// Each of `seen`'s lights, the walk's over `query`'s window (P12.T8.b.1): a glitch's unlit, and
    /// a flare's or an outburst's in the detection's band, zero before its first knot, finite and
    /// non-negative, with no jump after its onset, and the walk's flux at the detection's own time:
    /// an onset's peak, the second knot, and the window's start for an event in progress.
    fn lights_agree(query: &AlertQuery, seen: &[Detection]) {
        let nanosecond = Span::new(0, 1).unwrap();
        for d in seen {
            let light = match (d.kind(), d.light()) {
                (TransientKind::Glitch, DetectionLight::Unlit) => continue,
                (
                    TransientKind::Flare | TransientKind::FuOrionisOutburst,
                    DetectionLight::Lit(l),
                ) => l,
                (kind, light) => panic!("{kind:?} has the light {light:?}"),
            };
            assert_eq!(light.band(), d.band());
            let knots = light.knots();
            assert_eq!(knots.len(), 3, "{d:?}");
            assert!(knots.windows(2).all(|pair| pair[0] < pair[1]));
            let before = knots[0].checked_sub(nanosecond).unwrap();
            assert_same_bits(light.flux_at(before).value(), 0.0);
            let own = match d.phase() {
                DetectionPhase::Onset => {
                    // The walk's is the template's peak; the knot is floored to the nanosecond
                    // before it, a few 10⁻¹¹ of a minute-long flare's rise down.
                    let peak = light.flux_at(knots[1]).value();
                    let off = (peak - d.flux().value()).abs() / d.flux().value();
                    assert!(off < 1e-9, "{d:?}: its peak {peak}, {off} off");
                    peak
                }
                DetectionPhase::InProgress => {
                    let opening = light.flux_at(query.window().start()).value();
                    assert_same_bits(opening, d.flux().value());
                    opening
                }
            };
            let peak = light.flux_at(knots[1]).value().max(own);
            for pair in knots.windows(2) {
                let gap = pair[1].checked_since(pair[0]).unwrap().as_seconds_f64();
                for share in [0.0, 0.25, 0.5, 0.999] {
                    let t = pair[0]
                        .checked_add(Span::from_seconds_f64(gap * share).unwrap())
                        .unwrap();
                    let here = light.flux_at(t).value();
                    let next = light.flux_at(t.checked_add(nanosecond).unwrap()).value();
                    assert!(here.is_finite() && here >= 0.0, "{d:?} at {t}: {here}");
                    assert!(
                        (next - here).abs() <= 1e-6 * peak,
                        "{d:?} jumps at {t}: {here} to {next}"
                    );
                }
            }
        }
    }

    #[test]
    fn local_events_are_each_systems_own_in_any_order() {
        let near = neighbours();
        let flares = equals_direct_evaluation(&near);
        lights_agree(&near.query, &flares);
        order_independent(&near, &flares);
        let middle = near
            .query
            .window()
            .start()
            .checked_add(Span::from_seconds(5 * 86_400))
            .unwrap();
        a_split_window_finds_the_wholes_onsets(&near, &flares, middle);
        let count = |seen: &[Detection], kind, phase| {
            seen.iter()
                .filter(|d| d.kind() == kind && d.phase() == phase)
                .count()
        };
        assert!(flares.iter().all(|d| d.kind() == TransientKind::Flare));
        assert!(count(&flares, TransientKind::Flare, DetectionPhase::Onset) >= 3);
        assert!(count(&flares, TransientKind::Flare, DetectionPhase::InProgress) >= 1);

        let young = young_and_dead();
        let seen = equals_direct_evaluation(&young);
        lights_agree(&young.query, &seen);
        order_independent(&young, &seen);
        let middle = young
            .query
            .window()
            .start()
            .checked_add(julian_years(75.0))
            .unwrap();
        a_split_window_finds_the_wholes_onsets(&young, &seen, middle);
        for (kind, phase) in [
            (TransientKind::Glitch, DetectionPhase::Onset),
            (TransientKind::FuOrionisOutburst, DetectionPhase::Onset),
            (TransientKind::FuOrionisOutburst, DetectionPhase::InProgress),
        ] {
            assert!(
                count(&seen, kind, phase) >= 1,
                "{kind:?} {phase:?}: {seen:#?}"
            );
        }
        assert!(seen.iter().any(Detection::is_resolvable));
        // A glitch has no light and is timed in the radio; an outburst is seen in V or K.
        for d in &seen {
            match d.kind() {
                TransientKind::Glitch => {
                    assert_eq!(d.band(), AlertBand::Photometric(Band::Radio));
                    assert!(d.flux().value().abs() < f64::MIN_POSITIVE);
                    assert!(!d.is_resolvable());
                }
                TransientKind::Flare | TransientKind::FuOrionisOutburst => {
                    assert!(matches!(
                        d.band(),
                        AlertBand::Photometric(Band::V | Band::K)
                    ));
                }
                other => panic!("{other:?} is not a local kind built here"),
            }
            assert_eq!(d.class(), None);
        }
    }

    /// One cache asked about galaxies in turn, of two seeds and of one seed under other rules,
    /// gives each what a fresh cache gives it: it empties itself for another galaxy, and in debug
    /// builds checks every star it lends.
    #[test]
    fn a_cache_serves_one_galaxy_at_a_time() {
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let day = UniverseTime::EPOCH
            .checked_add(Span::from_seconds(86_400))
            .unwrap();
        let window = TimeWindow::new(UniverseTime::EPOCH, day).unwrap();
        let query = AlertQuery::builder(sun, window, sensor()).build().unwrap();
        let range = RangeQuery::builder(sun, LightYears::new(8.0))
            .mode(QueryMode::ObservedFrom(sun))
            .build()
            .unwrap();
        let first = Galaxy::new(Seed::new(31));
        let second = Galaxy::new(Seed::new(32));
        let ruled = first.clone().with_rules(GalaxyRules::every_26_hold());
        let seen = |galaxy: &Galaxy, rows: &[ObservedSystem], cache: &mut LocalEventsCache| {
            let mut noise = NoiseCache::with_capacity(64);
            let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
            let mut out = Vec::new();
            local_events(
                galaxy,
                &mut ctx,
                &query,
                rows,
                &mut NoStarsCache,
                cache,
                &mut out,
            );
            out
        };
        let mut shared = LocalEventsCache::with_capacity(4_096);
        let mut found = 0;
        for galaxy in [&first, &second, &ruled, &first] {
            let rows =
                range_query_observed(galaxy, &mut NoCache::new(), &mut NoStarsCache, &[], &range)
                    .unwrap()
                    .observed()
                    .to_vec();
            let fresh = seen(galaxy, &rows, &mut LocalEventsCache::with_capacity(4_096));
            assert_eq!(seen(galaxy, &rows, &mut shared), fresh);
            found += fresh.len();
        }
        assert!(found >= 10, "{found} detections in all");
    }

    /// P06.T28.g's agreement: over the stars of both scenes and each scene's window, the stellar
    /// stage's assembly (`stellar::events::events_in`) lists exactly the flares, FU Orionis
    /// outbursts and glitches that their kinds' listings give, the ones `local_events` calls; and
    /// every flare, outburst or glitch `local_events` detects is one the assembly lists, over the
    /// span from its emission back by its kind's longest event.
    #[test]
    fn the_stellar_assembly_lists_the_local_events_kinds() {
        use crate::events::EventSubject;
        use crate::stellar::events::{StarEvent, StarEventKind, events_in};
        let galaxy = galaxy();
        let seed = galaxy.seed();
        let local = |e: &StarEvent| {
            matches!(
                e.kind(),
                StarEventKind::Flare | StarEventKind::FuOrionisOutburst | StarEventKind::Glitch
            )
        };
        let mut detected = 0;
        for scene in [neighbours(), young_and_dead()] {
            let detections = run(&scene, &scene.query, &scene.rows);
            let window = scene.query.window();
            let mut stars = scene.stars.borrow_mut();
            for row in &scene.rows {
                if row.hit().record().kind() == SystemKind::RoguePlanet {
                    continue;
                }
                stars.with_stars(galaxy, row.hit().record(), |system| {
                    for (star, slot) in system.stars().iter().zip(system.hierarchy().stars()) {
                        let body = slot.body();
                        let mut listed = Vec::new();
                        events_in(star, seed, body, window, &mut listed);
                        let mut assembled: Vec<_> = listed
                            .iter()
                            .filter(|e| local(e))
                            .map(|e| (e.id(), e.onset()))
                            .collect();
                        let series = |tag| EventSeries::new(seed, tag, body.into());
                        let (mut flares, mut outbursts) = (Vec::new(), Vec::new());
                        let (mut small, mut large) = (Vec::new(), Vec::new());
                        flares::flares_in(star, &series(tags::STAR_FLARE), window, &mut flares);
                        fu_orionis::outbursts_in(
                            star,
                            &series(tags::STAR_FU_ORIONIS),
                            window,
                            &mut outbursts,
                        );
                        glitches::glitches_in(star, &series(tags::STAR_GLITCH), window, &mut small);
                        glitches::large_glitches_in(star, seed, body, window, &mut large);
                        let mut own: Vec<_> = flares
                            .iter()
                            .map(|f| (f.id(), f.onset()))
                            .chain(outbursts.iter().map(|o| (o.id(), o.onset())))
                            .chain(small.iter().map(|g| (g.id(), g.onset())))
                            .chain(large.iter().map(|g| (g.id(), g.onset())))
                            .collect();
                        assembled.sort();
                        own.sort();
                        assert_eq!(assembled, own, "{body:?}");
                        for d in detections.iter().filter(|d| {
                            d.event().subject() == EventSubject::from(body)
                                && matches!(
                                    d.kind(),
                                    TransientKind::Flare
                                        | TransientKind::FuOrionisOutburst
                                        | TransientKind::Glitch
                                )
                        }) {
                            // A glitch is an instant, detected at its onset.
                            let back = if d.kind() == TransientKind::Flare {
                                flares::MAX_DURATION
                            } else if d.kind() == TransientKind::FuOrionisOutburst {
                                fu_orionis::MAX_DURATION
                            } else {
                                Span::from_seconds(1)
                            };
                            let around = TimeWindow::new(
                                d.emitted().checked_sub(back).unwrap(),
                                d.emitted().checked_add(Span::from_seconds(1)).unwrap(),
                            )
                            .unwrap();
                            let mut near = Vec::new();
                            events_in(star, seed, body, around, &mut near);
                            assert!(
                                near.iter().any(|e| local(e) && e.id() == d.event()),
                                "{d:?}"
                            );
                            detected += 1;
                        }
                    }
                });
            }
        }
        assert!(detected >= 5, "{detected}");
    }

    /// The same systems read by range queries thirty years apart give the same detections, to the
    /// bit: each host's line is its record's, not the light's of the row.
    #[test]
    fn rows_read_at_two_times_see_the_same_events() {
        let (early, stars) = neighbours_at(years(120));
        let (late, _) = neighbours_at(years(150));
        let both: Vec<ObservedSystem> = early
            .iter()
            .filter(|row| late.iter().any(|other| other.hit().id() == row.hit().id()))
            .copied()
            .collect();
        let later: Vec<ObservedSystem> = both
            .iter()
            .map(|row| {
                *late
                    .iter()
                    .find(|other| other.hit().id() == row.hit().id())
                    .unwrap()
            })
            .collect();
        assert!(both.len() >= 10, "{} systems in both", both.len());
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let opens = years(130);
        let closes = opens.checked_add(Span::from_seconds(5 * 86_400)).unwrap();
        let query = AlertQuery::builder(sun, TimeWindow::new(opens, closes).unwrap(), sensor())
            .build()
            .unwrap();
        let scene = Scene {
            query,
            rows: both,
            stars: RefCell::new(stars),
        };
        let seen = run(&scene, &scene.query, &scene.rows);
        assert!(seen.len() >= 3, "{} detections", seen.len());
        assert_eq!(run(&scene, &scene.query, &later), seen);
    }
}
