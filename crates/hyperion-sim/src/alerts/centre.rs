//! The galactic centre's walk (plan 12, P12.T5.d): its black hole's X-ray flares and its tidal
//! disruptions, each in the window its light arrives in.
//!
//! The centre is the first entry of plan 10's global list, and its black hole's events are plan
//! 09's (P09.T30): the flares ([`CentreFlare`], [`TransientKind::CentralFlare`], no class) and the
//! disruptions, each of a victim on the centre's feature-level list
//! ([`TidalDisruptionVictim`], [`ClassId::TIDAL_DISRUPTION_VICTIM`],
//! [`TransientKind::TidalDisruption`]). [`centre_detections`] lists both; [`alerts_in`](super::alerts_in)
//! calls it for the global list's centre.
//!
//! # The light
//!
//! The black hole sits at the galactic origin, at rest, and both kinds' light leaves from it, so an
//! event at T arrives at T + d ÷ c, d the observer's distance from the origin, a closed form
//! (Design note 6), and one line of sight serves every event.
//!
//! - **A flare** is light in 2–10 keV alone, as plan 09 models it: its own Gaussian above the
//!   quiescence ([`CentreFlare::luminosity_watts_at`]).
//! - **A disruption's tail** ([`DisruptionTail`]) gives [`XRAY_SHARE`] of its light in 2–10 keV
//!   ([`DisruptionTail::luminosity_watts_at`]), and a blackbody's share at [`TAIL_TEMPERATURE`] of
//!   its whole light in each photometric band ([`band_share`]), the share the registry's row left
//!   to this walk (plan 12's Risks, "P12.T5.b as built"; provisional): optically found
//!   disruptions are hot blackbodies, though the X-rays come from elsewhere, so the X-ray share is
//!   counted twice by a few per cent, and the share stays under the row's bound, the whole light.
//!   A victim swallowed whole has no tail and gives no light.
//!
//! # The window
//!
//! An event whose light arrives in the window `[t₁, t₂)` is an [`Onset`], seen at its peak in each
//! band: a flare's [`CentreFlare::peak_luminosity_watts`], a tail's at its fallback time
//! ([`DisruptionTail::peak_luminosity_watts`]), whose light arrives after the onset's, as T5.b's and
//! T5.e's onsets are seen. One whose light arrived before `t₁` is [`InProgress`] if its light at
//! `t₁` is above the limit, the flare's or the tail's at `t₁` − d ÷ c. The flares are listed over
//! the emission interval, `[t₁ − d ÷ c, t₂ − d ÷ c)`, and those running at its start
//! ([`flares_active_at`]); the disruptions are the source horizon's victims ([`CentreVictims`]),
//! every one of them.
//!
//! The band is the sensor's in which the event stands highest over the detection limit, the first
//! in [`AlertBand::ALL`]'s order on a tie, and the flux through plan 07's line of sight from the
//! origin to the observer ([`AlertContext::sightline`]), marched only when an event's bare flux
//! reaches a limit. A detection's retardation is its onset's light, emitted at T from the origin,
//! and its [`DetectionLight`] the event's flux in its band at every time of the observer's clock
//! (P12.T8.b.1), through the same distance, line of sight and shares, so that it is the walk's
//! flux at the detection's own time: an onset's at its peak's arrival, and one in progress's at
//! `t₁`.
//!
//! [`Onset`]: DetectionPhase::Onset
//! [`InProgress`]: DetectionPhase::InProgress

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::events::TimeWindow;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::ClassId;
use crate::galaxy::features::centre::events::luminosity::XRAY_SHARE;
use crate::galaxy::features::centre::events::{
    CentreFlare, CentreFlareModel, CentreVictims, DisruptionTail, TidalDisruptionVictim, flares,
    flares_active_at,
};
use crate::galaxy::gas::extinction::Sightline;
use crate::id::{CentreMemberId, EventId, SystemId};
use crate::observe::{Retardation, bearing, light_time};
use crate::time::{Span, UniverseTime};
use crate::units::{Kelvin, Metres, Watts};

use super::arriving::{ArrivingLight, DetectionLight};
use super::light::band_share;
use super::reach::{flux, inverse_square_flux, transmission};
use super::registry::TransientKind;
use super::{AlertBand, AlertContext, AlertQuery, BAND_COUNT, Detection, DetectionPhase};

/// The blackbody temperature of a disruption's tail's photometric light: 23,174 K, 10^4.365, the
/// median of log `T_BB` over the 36 optically discovered disruptions of Gezari's (2021, ARA&A 59, 21,
/// Table 1) census (4.06–4.85; the X-ray discovered ones' soft component, 5.67–6.30, is another
/// component of their light). Provisional: plan 09's tails model their accretion's whole light and
/// its 2–10 keV share, not its spectrum.
pub const TAIL_TEMPERATURE: Kelvin = Kelvin::new(23_174.0);

/// One event of the centre, as the walk reads it.
#[derive(Debug, Clone, Copy)]
enum Event<'a> {
    Flare(&'a CentreFlare),
    Disruption(&'a TidalDisruptionVictim, &'a DisruptionTail),
}

impl Event<'_> {
    /// Its ID.
    fn id(self) -> EventId {
        match self {
            Self::Flare(flare) => flare.id(),
            Self::Disruption(victim, _) => victim.disruption().id(),
        }
    }

    /// Its onset T.
    fn onset(self) -> UniverseTime {
        match self {
            Self::Flare(flare) => flare.onset(),
            Self::Disruption(victim, _) => victim.time(),
        }
    }

    /// Its kind.
    const fn kind(self) -> TransientKind {
        match self {
            Self::Flare(_) => TransientKind::CentralFlare,
            Self::Disruption(..) => TransientKind::TidalDisruption,
        }
    }

    /// Its host and the host's class: the black hole and none for a flare, the victim and its
    /// class for a disruption.
    fn host(self) -> (SystemId, Option<ClassId>) {
        match self {
            Self::Flare(_) => (SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE), None),
            Self::Disruption(victim, _) => (
                SystemId::from(victim.id()),
                Some(ClassId::TIDAL_DISRUPTION_VICTIM),
            ),
        }
    }

    /// Its light in each band, W: its peak's for `at` of `None`, an onset's, else its light
    /// emitted at `at`, an event in progress's at the window's start.
    fn luminosity(
        self,
        at: Option<UniverseTime>,
        shares: &[f64; BAND_COUNT],
    ) -> [Watts; BAND_COUNT] {
        match self {
            Self::Flare(flare) => {
                let xray = at.map_or(flare.peak_luminosity_watts(), |t| {
                    flare.luminosity_watts_at(t)
                });
                AlertBand::ALL.map(|band| match band {
                    AlertBand::XRay => Watts::new(xray),
                    AlertBand::Photometric(_) => Watts::ZERO,
                })
            }
            // The share times the whole light, in X-rays the tail's own 2–10 keV light to the bit
            // (`DisruptionTail::luminosity_watts_at` is `XRAY_SHARE` times it), as its
            // `ArrivingLight` reads it.
            Self::Disruption(_, tail) => {
                let whole = at.map_or(tail.peak_bolometric_watts(), |t| {
                    tail.bolometric_watts_at(t)
                });
                AlertBand::ALL.map(|band| Watts::new(shares[band.index()] * whole))
            }
        }
    }

    /// Its light at the observer in `band`, `distance` away through a line of sight of
    /// `transmission` there (P12.T8.b.1's [`ArrivingLight`]): the same at any phase, and its
    /// flux at the detection's own time the walk's.
    fn light(
        self,
        band: AlertBand,
        shares: &[f64; BAND_COUNT],
        distance: Metres,
        transmission: f64,
    ) -> DetectionLight {
        DetectionLight::Lit(match self {
            Self::Flare(flare) => ArrivingLight::centre_flare(*flare, distance, transmission),
            Self::Disruption(_, tail) => ArrivingLight::disruption_tail(
                (*tail, shares[band.index()]),
                band,
                distance,
                transmission,
            ),
        })
    }
}

/// The share of a tail's whole light in each band, in [`AlertBand::ALL`]'s order: a blackbody's
/// at [`TAIL_TEMPERATURE`] in the photometric bands, and [`XRAY_SHARE`] in X-rays, which the
/// walk reads from the tail itself.
#[must_use]
fn tail_shares() -> [f64; BAND_COUNT] {
    AlertBand::ALL.map(|band| match band {
        AlertBand::Photometric(_) => band_share(TAIL_TEMPERATURE, band),
        AlertBand::XRay => XRAY_SHARE,
    })
}

/// The walk's state over one call: the line of sight, marched once and only if needed.
struct Walk<'q, 'c, 'g> {
    galaxy: &'g Galaxy,
    ctx: &'q mut AlertContext<'c>,
    query: &'q AlertQuery,
    distance: Metres,
    light: Span,
    line: Option<Sightline>,
    shares: [f64; BAND_COUNT],
}

impl Walk<'_, '_, '_> {
    /// The detection of `event` by the sensor, if its light arrives in the window, or arrived
    /// before it and is still above the limit at its start (module documentation).
    fn detect(&mut self, event: Event<'_>) -> Option<Detection> {
        let window = self.query.window();
        let onset = event.onset();
        let arrival = onset
            .checked_add(self.light)
            .expect("a light time across the cube keeps a time on the clock");
        let (phase, at) = if window.contains(arrival) {
            (DetectionPhase::Onset, None)
        } else if arrival < window.start() {
            let emitted = window
                .start()
                .checked_sub(self.light)
                .expect("a light time across the cube keeps a time on the clock");
            (DetectionPhase::InProgress, Some(emitted))
        } else {
            return None;
        };
        let luminosity = event.luminosity(at, &self.shares);
        let sensor = self.query.sensor();
        let reaches = sensor.bands().any(|(band, limits)| {
            inverse_square_flux(luminosity[band.index()], self.distance)
                .is_some_and(|bare| bare >= limits.detect())
        });
        if !reaches {
            return None;
        }
        let observer = *self.query.observer_position();
        let line = self.line.get_or_insert_with(|| {
            self.ctx
                .sightline(self.galaxy, &GalacticPosition::ORIGIN, &observer)
        });
        let mut best: Option<(AlertBand, crate::units::WattsPerSquareMetre, f64)> = None;
        for (band, limits) in sensor.bands() {
            let Some(seen) = flux(luminosity[band.index()], self.distance, line, band) else {
                continue;
            };
            let over = seen.value() / limits.detect().value();
            if over >= 1.0 && best.is_none_or(|(_, _, most)| over > most) {
                best = Some((band, seen, over));
            }
        }
        let (band, seen, _) = best?;
        let limits = sensor
            .limits(band)
            .expect("the band is one of the sensor's");
        let retardation = Retardation::of_emission(
            &observer,
            onset,
            GalacticPosition::ORIGIN,
            GalacticVelocity::default(),
            &GalacticPosition::ORIGIN,
        );
        debug_assert_eq!(retardation.observed(), arrival);
        let light = event.light(band, &self.shares, self.distance, transmission(line, band));
        let (host, class) = event.host();
        Some(Detection::new(
            event.id(),
            host,
            class,
            event.kind(),
            phase,
            band,
            seen,
            bearing(&observer, &GalacticPosition::ORIGIN)?,
            retardation,
            seen >= limits.resolve(),
            light,
        ))
    }
}

/// The detections of the galactic centre's events that `query`'s sensor sees in its window
/// (module documentation), appended to `out` by arrival and then by event: the black hole's flares
/// whatever the classes asked about, since they have none, and the disruptions of `victims`, the
/// galaxy's ([`CentreVictims::of`]), if `victims` is given.
///
/// Nothing is appended for an observer at the black hole itself, which has no bearing.
///
/// # Panics
///
/// If the window, moved back by the light time, leaves the clock, which a window inside the clock
/// window cannot.
///
/// # Examples
///
/// The flares of a week that an X-ray sensor 8,000 ly from the centre sees, and the tails of the
/// disruptions still above its limit:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::{
///     AlertBand, AlertContext, AlertQuery, BandLimits, SensorHorizon, TransientKind,
///     centre_detections,
/// };
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::centre::events::CentreVictims;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::time::{Span, UniverseTime};
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let galaxy = Galaxy::new(Seed::new(7));
/// let w = WattsPerSquareMetre::new;
/// let sensor = SensorHorizon::new([(AlertBand::XRay, BandLimits::new(w(1e-14), w(1e-12))?)])?;
/// let here = GalacticPosition::from_light_years([0.0, 8_000.0, 0.0]).ok_or("in the cube")?;
/// let week = UniverseTime::EPOCH.checked_add(Span::from_seconds(7 * 86_400)).ok_or("on the clock")?;
/// let query = AlertQuery::builder(here, TimeWindow::new(UniverseTime::EPOCH, week)?, sensor).build()?;
/// let victims = CentreVictims::of(&galaxy)?;
/// let mut noise = NoiseCache::with_capacity(1_024);
/// let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
/// let mut seen = Vec::new();
/// centre_detections(&galaxy, &mut ctx, &query, Some(&victims), &mut seen);
/// assert!(seen.iter().any(|d| d.kind() == TransientKind::CentralFlare));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn centre_detections(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    victims: Option<&CentreVictims>,
    out: &mut Vec<Detection>,
) {
    let observer = query.observer_position();
    let distance = observer.distance_to(&GalacticPosition::ORIGIN);
    if distance.value() <= 0.0 {
        return;
    }
    let light = light_time(distance);
    let window = query.window();
    let back = |t: UniverseTime| {
        t.checked_sub(light)
            .expect("a light time across the cube keeps a time on the clock")
    };
    let emission = TimeWindow::new(back(window.start()), back(window.end()))
        .expect("a window moved back by one span is a window");
    let model = CentreFlareModel::of(galaxy);
    let mut listed = Vec::new();
    flares_active_at(&model, emission.start(), &mut listed);
    // A flare whose onset is the interval's start arrives at t₁, an onset, which the listing over
    // the interval gives.
    listed.retain(|flare| flare.onset() < emission.start());
    flares(&model, emission, &mut listed);
    let mut walk = Walk {
        galaxy,
        ctx,
        query,
        distance,
        light,
        line: None,
        shares: tail_shares(),
    };
    let start = out.len();
    out.extend(
        listed
            .iter()
            .filter_map(|flare| walk.detect(Event::Flare(flare))),
    );
    if let Some(victims) = victims {
        out.extend(victims.victims().iter().filter_map(|victim| {
            let tail = victim.tail()?;
            walk.detect(Event::Disruption(victim, tail))
        }));
    }
    out[start..].sort_by(|a, b| {
        a.arrival()
            .cmp(&b.arrival())
            .then_with(|| a.event().cmp(&b.event()))
    });
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::gas::ccm::Band;
    use crate::math;

    /// The tails' temperature is the census's median, 10^4.365 K, to the kelvin.
    #[test]
    fn the_tails_temperature_is_the_optical_censuss_median() {
        assert!((TAIL_TEMPERATURE.value() - math::exp10(4.365)).abs() < 0.5);
    }

    /// The tails' shares are a 23,174 K blackbody's through the top hats, none in radio, and the
    /// accretion's own 2–10 keV share, together under the whole light; most in the blue and the
    /// ultraviolet, as a hot disruption's.
    #[test]
    fn the_tails_shares_are_a_hot_blackbodys_and_the_accretions_xray_share() {
        let shares = tail_shares();
        for band in AlertBand::ALL {
            let share = shares[band.index()];
            match band {
                AlertBand::XRay => assert_same_bits(share, XRAY_SHARE),
                AlertBand::Photometric(Band::Radio) => assert!(share <= 0.0),
                AlertBand::Photometric(_) => {
                    assert_same_bits(share, band_share(TAIL_TEMPERATURE, band));
                    assert!(share > 0.0, "{band:?}");
                }
            }
        }
        assert!(shares.iter().sum::<f64>() < 1.0);
        let at = |band| shares[AlertBand::Photometric(band).index()];
        assert!(at(Band::U) > at(Band::B) && at(Band::B) > at(Band::V));
        assert!(at(Band::V) > at(Band::K));
    }
}
