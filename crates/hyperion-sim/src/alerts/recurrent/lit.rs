//! What the recurrent walk's classes other than the magnetars share (P12.T5.c): a host and its
//! motion, the interval its events are listed over, and the detection of each event from its
//! light in each band.
//!
//! An event's light is its own source's at its age since its onset: plan 11's curve for a binary
//! event ([`BinaryLightCurve`]), and for a giant eruption its own light, as the registry's
//! [`LightCurveSource`](super::super::LightCurveSource) names them. It lasts from its onset for its
//! duration, after which the event is over: plan 11's, until the light stays under a hundredth of
//! its peak, and an eruption's own. The walk (module documentation of
//! [`recurrent`](super)):
//!
//! 1. lists each host's events over its emission interval ([`emission_interval`]), as the
//!    host's class finds them, passing over a host that stays beyond its kind's reach;
//! 2. takes each event's arrival, its onset plus the light time from where the host was then: an
//!    [`Onset`] in the window, seen by its peak, or [`InProgress`] if it arrived before the
//!    window and is not over at its start, seen by its light then;
//! 3. finds its flux in each of the sensor's bands, through plan 07's line from the host's position
//!    at the start of the Julian year that holds the onset, marched once per host and year and only
//!    for an event whose light could reach a band's limit with nothing in the way, so that a
//!    detection's flux depends on the event alone, never on the window;
//! 4. detects it in the band where its flux stands highest over the detection limit, the first in
//!    [`AlertBand::ALL`]'s order on a tie, if it reaches the limit there.
//!
//! [`Onset`]: DetectionPhase::Onset
//! [`InProgress`]: DetectionPhase::InProgress
//! [`BinaryLightCurve`]: crate::stellar::binary::light_curves::BinaryLightCurve

use crate::coords::GalacticPosition;
use crate::events::TimeWindow;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::{CatalogueMotion, ClassId};
use crate::galaxy::gas::extinction::Sightline;
use crate::id::{EventId, SystemId};
use crate::observe::{Retardation, bearing};
use crate::stellar::binary::events::{BinaryEvent, BinaryEventKind};
use crate::stellar::events::lbv::Eruption;
use crate::time::{Span, UniverseTime};
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SOLAR_LUMINOSITY_W, SPEED_OF_LIGHT};
use crate::units::{LightYears, Metres, Watts, WattsPerSquareMetre};

use super::super::arriving::{ArrivingLight, DetectionLight};
use super::super::light::binary_luminosity;
use super::super::reach::{flux, inverse_square_flux, transmission};
use super::super::registry::{LongestDuration, TransientKind};
use super::super::{AlertBand, AlertContext, AlertQuery, BAND_COUNT, Detection, DetectionPhase};
use super::{DAY, GIANT_ERUPTION_SHARES, arrival_of, year_start};

/// A host the walk reads: its system, its class and how it moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LitHost {
    pub(super) host: SystemId,
    pub(super) class: ClassId,
    pub(super) motion: CatalogueMotion,
}

/// One event as the walk evaluates it: its ID, kind, onset, rise and duration, and its light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LitEvent {
    id: EventId,
    kind: TransientKind,
    onset: UniverseTime,
    peak: Span,
    duration: Span,
    light: EventLight,
}

/// What gives an event its light.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EventLight {
    /// Plan 11's binary event, its curve.
    Binary(BinaryEvent),
    /// Plan 06's giant eruption and the star's quiescent luminosity at its onset, W.
    Eruption(Eruption, Watts),
}

impl LitEvent {
    /// Plan 11's binary `event`: its curve's peak and its duration to a hundredth of the peak.
    #[must_use]
    pub(super) fn of_binary(event: &BinaryEvent) -> Self {
        Self {
            id: event.id(),
            kind: binary_kind(event.kind()),
            onset: event.onset(),
            peak: event.curve().peak(),
            duration: event.duration(),
            light: EventLight::Binary(*event),
        }
    }

    /// Plan 06's giant `eruption`: its rise and its duration, and its own light above the star's
    /// quiescent luminosity at its onset.
    #[must_use]
    pub(super) fn of_eruption(eruption: &Eruption) -> Self {
        let quiescent = Watts::new(eruption.quiescent_luminosity().value() * SOLAR_LUMINOSITY_W);
        Self {
            id: eruption.id(),
            kind: TransientKind::GiantEruption,
            onset: eruption.onset(),
            peak: eruption.rise(),
            duration: eruption.duration(),
            light: EventLight::Eruption(*eruption, quiescent),
        }
    }

    /// The event's light in `band` `age` after its onset, W.
    #[must_use]
    fn luminosity(&self, age: Span, band: AlertBand) -> Watts {
        match &self.light {
            EventLight::Binary(event) => binary_luminosity(event.curve(), age, band),
            EventLight::Eruption(eruption, quiescent) => {
                let at = self
                    .onset
                    .checked_add(age)
                    .expect("an eruption's light ends within decades of a clock time");
                let factor = eruption.luminosity_factor_at(at);
                Watts::new((factor - 1.0) * quiescent.value() * GIANT_ERUPTION_SHARES[band.index()])
            }
        }
    }

    /// The event's light in `band` at the observer, `distance` away through a line of sight of
    /// `transmission` there (P12.T8.b.1).
    #[must_use]
    fn arriving(&self, band: AlertBand, distance: Metres, transmission: f64) -> ArrivingLight {
        match &self.light {
            EventLight::Binary(event) => {
                ArrivingLight::binary(*event.curve(), event.onset(), band, distance, transmission)
            }
            EventLight::Eruption(eruption, quiescent) => ArrivingLight::eruption(
                (*eruption, *quiescent, GIANT_ERUPTION_SHARES[band.index()]),
                band,
                distance,
                transmission,
            ),
        }
    }
}

/// The registry's kind of plan 11's `kind`.
#[must_use]
const fn binary_kind(kind: BinaryEventKind) -> TransientKind {
    match kind {
        BinaryEventKind::Nova => TransientKind::Nova,
        BinaryEventKind::DwarfNova => TransientKind::DwarfNova,
        BinaryEventKind::XrayTransient => TransientKind::XrayTransient,
        BinaryEventKind::XrayBurst => TransientKind::XrayBurst,
        BinaryEventKind::BeXGiantOutburst => TransientKind::BeXOutburst,
        BinaryEventKind::LuminousRedNova => TransientKind::LuminousRedNova,
        BinaryEventKind::Kilonova => TransientKind::Kilonova,
    }
}

/// The longest an event of `kind` lasts: its row's duration.
///
/// # Panics
///
/// If the row has no end or is not built, which no kind the walk lists is.
#[must_use]
pub(super) fn longest(kind: TransientKind) -> Span {
    match kind.row().duration() {
        Some(LongestDuration::AtMost(span)) => span,
        other => panic!("a recurrent kind's events end: {kind:?} lasts {other:?}"),
    }
}

/// The interval over which a host moving by `motion` is asked for the onsets whose light reaches
/// `observer` in `window`, or arrived before it within `look_back`, the longest its events last
/// (Design note 6), or `None` for a host that stays beyond `reach`, the farthest any event of its
/// kind can be seen from, at every onset the interval could hold.
///
/// The interval is `[t₁ − s − ε − R, t₂ − s + ε + DAY)`, with s the light time from where the host
/// is at its motion's own time, R twice `look_back` and a day (an event in progress needs one
/// look-back; the second is a margin, as the magnetars' interval takes twice D), and
/// ε = β (s + W + R) ÷ (1 − β), β
/// the host's faster speed, before or after its motion's break, over light's, and W the window's
/// furthest end from that time. The light time
/// from wherever the host is at an onset in the interval lies within ε of s, so the interval holds
/// every such onset, and each onset's own arrival then decides; a host is passed over only if
/// s − ε, less a day, is beyond the reach. One distance and no step on the light cone, for the
/// millions of hosts a scan walks, most of them in the cells' padding, beyond the reach; the day
/// absorbs the rounding.
///
/// # Panics
///
/// Never: the hosts move below light's speed, and an [`AlertQuery`]'s window and the light times
/// across the cube keep the interval's ends centuries inside the clock.
#[must_use]
pub(super) fn emission_interval(
    motion: &CatalogueMotion,
    observer: &GalacticPosition,
    window: TimeWindow,
    look_back: Span,
    reach: LightYears,
) -> Option<TimeWindow> {
    let before = motion
        .at()
        .checked_sub(Span::from_seconds(1))
        .unwrap_or(motion.at());
    let speed = [before, motion.at()]
        .map(|t| motion.velocity_at(t).speed().value())
        .into_iter()
        .fold(0.0, f64::max);
    let beta = speed / SPEED_OF_LIGHT;
    let s = motion.point().distance_to(observer).value() / SPEED_OF_LIGHT;
    let from_motion = |t: UniverseTime| {
        t.checked_since(motion.at())
            .expect("two clock times are a span apart")
            .as_seconds_f64()
            .abs()
    };
    let w = from_motion(window.start()).max(from_motion(window.end()));
    let day = DAY.as_seconds_f64();
    let r = 2.0 * look_back.as_seconds_f64() + day;
    let epsilon = beta * (s + w + r) / (1.0 - beta);
    let nearest = (s - epsilon - day) * SPEED_OF_LIGHT / METRES_PER_LIGHT_YEAR;
    if nearest > reach.value() {
        return None;
    }
    let shift = |t: UniverseTime, seconds: f64| {
        let span =
            Span::from_seconds_f64(seconds.abs()).expect("light times across the cube are spans");
        if seconds < 0.0 {
            t.checked_sub(span)
        } else {
            t.checked_add(span)
        }
        .expect("a light time across the cube keeps a time on the clock")
    };
    let from = shift(window.start(), -(s + epsilon + r));
    let to = shift(window.end(), -(s - epsilon) + day);
    Some(TimeWindow::new(from, to).expect("the interval's start comes before its end"))
}

/// Appends to `out` the detections of `host`'s `events`, listed in time order, by `query`'s
/// sensor (module documentation, steps 2–4).
pub(super) fn host_detections(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    host: &LitHost,
    events: &[LitEvent],
    out: &mut Vec<Detection>,
) {
    let observer = query.observer_position();
    let window = query.window();
    let motion = &host.motion;
    let mut line: Option<(UniverseTime, Sightline)> = None;
    for event in events {
        let site = motion.position_at(event.onset);
        let distance = site.distance_to(observer);
        let arrival = arrival_of(event.onset, distance);
        let (phase, age) = if window.contains(arrival) {
            (DetectionPhase::Onset, event.peak)
        } else if arrival < window.start() {
            let age = window
                .start()
                .checked_since(arrival)
                .expect("two clock times are a span apart");
            if age >= event.duration {
                continue;
            }
            (DetectionPhase::InProgress, age)
        } else {
            continue;
        };
        let mut luminosity = [Watts::ZERO; BAND_COUNT];
        let mut reaches = false;
        for (band, limits) in query.sensor().bands() {
            let watts = event.luminosity(age, band);
            luminosity[band.index()] = watts;
            reaches |=
                inverse_square_flux(watts, distance).is_some_and(|bare| bare >= limits.detect());
        }
        if !reaches {
            continue;
        }
        let year = year_start(event.onset);
        if line.as_ref().is_none_or(|(at, _)| *at != year) {
            let from = motion.position_at(year);
            line = Some((year, ctx.sightline(galaxy, &from, observer)));
        }
        let Some((_, sightline)) = line.as_ref() else {
            continue;
        };
        let mut best: Option<(AlertBand, WattsPerSquareMetre, f64)> = None;
        for (band, limits) in query.sensor().bands() {
            let Some(seen) = flux(luminosity[band.index()], distance, sightline, band) else {
                continue;
            };
            let over = seen.value() / limits.detect().value();
            if over >= 1.0 && best.is_none_or(|(_, _, most)| over > most) {
                best = Some((band, seen, over));
            }
        }
        let Some((band, seen, _)) = best else {
            continue;
        };
        let limits = query
            .sensor()
            .limits(band)
            .expect("the band is one of the sensor's");
        let Some(towards) = bearing(observer, &site) else {
            continue;
        };
        let retardation = Retardation::of_emission(
            observer,
            event.onset,
            site,
            motion.velocity_at(event.onset),
            &motion.position_at(arrival),
        );
        let light = event.arriving(band, distance, transmission(sightline, band));
        out.push(Detection::new(
            event.id,
            host.host,
            Some(host.class),
            event.kind,
            phase,
            band,
            seen,
            towards,
            retardation,
            seen >= limits.resolve(),
            DetectionLight::Lit(light),
        ));
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::coords::GalacticVelocity;
    use crate::time::CLOCK_WINDOW_H;

    /// The walk's arrival of light that leaves a host moving by `motion` at `t`.
    fn arrival(
        motion: &CatalogueMotion,
        observer: &GalacticPosition,
        t: UniverseTime,
    ) -> UniverseTime {
        arrival_of(t, motion.position_at(t).distance_to(observer))
    }

    /// The emission interval holds every onset whose light, by the walk's own arrival, reaches the
    /// observer in the window or within the look-back before it: the light that leaves at the
    /// interval's start arrives before the window's start less the look-back, and the light that
    /// leaves at its end at or after the window's end, the arrival rising with the onset. For
    /// 2,000 hosts at up to 3,000 km/s, observers up to 100,000 ly away, windows of a second to a
    /// century across the clock window and look-backs of none to twenty years. A host passed over
    /// stays beyond the reach at both ends.
    #[test]
    fn the_emission_interval_holds_every_onset_whose_light_can_be_seen() {
        let mut lcg = Lcg::new(0x0125_0c0a);
        let h = CLOCK_WINDOW_H.as_seconds_f64();
        let mut passed_over = 0;
        for _ in 0..2_000 {
            let mut uniform = |lo: f64, hi: f64| lo + (hi - lo) * lcg.next_f64();
            let site = GalacticPosition::from_light_years([
                uniform(-60_000.0, 60_000.0),
                uniform(-60_000.0, 60_000.0),
                uniform(-5_000.0, 5_000.0),
            ])
            .unwrap();
            let away = crate::math::exp10(uniform(-1.0, 5.0)).min(60_000.0);
            let [x, y, z] = site.to_light_years_f64();
            let observer = GalacticPosition::from_light_years([x + away, y, z]).unwrap();
            let speed = uniform(0.0, 3.0e6);
            let direction = [uniform(-1.0, 1.0), uniform(-1.0, 1.0), uniform(-1.0, 1.0)];
            let norm = direction.iter().map(|d| d * d).sum::<f64>().sqrt();
            let velocity = GalacticVelocity::new(direction.map(|d| d / norm * speed));
            let motion = CatalogueMotion::straight(site, velocity);
            let length = crate::math::exp10(uniform(0.0, 9.5));
            let start = Span::from_seconds_f64(uniform(-h, h - length))
                .and_then(|span| UniverseTime::EPOCH.checked_add(span))
                .unwrap();
            let window = TimeWindow::new(
                start,
                start
                    .checked_add(Span::from_seconds_f64(length).unwrap())
                    .unwrap(),
            )
            .unwrap();
            let look_back = Span::from_seconds_f64(uniform(0.0, 20.0 * 31_557_600.0)).unwrap();
            let reach = LightYears::new(crate::math::exp10(uniform(0.0, 5.5)));
            let Some(interval) = emission_interval(&motion, &observer, window, look_back, reach)
            else {
                passed_over += 1;
                for t in [window.start(), window.end()] {
                    let near = motion.position_at(t).distance_to(&observer);
                    assert!(
                        LightYears::from(near) > reach,
                        "{motion:?} from {observer:?}"
                    );
                }
                continue;
            };
            let earliest = window.start().checked_sub(look_back).unwrap();
            assert!(
                arrival(&motion, &observer, interval.start()) < earliest,
                "{motion:?} from {observer:?} over {window:?}"
            );
            assert!(
                arrival(&motion, &observer, interval.end()) >= window.end(),
                "{motion:?} from {observer:?} over {window:?}"
            );
        }
        assert!(
            (100..1_900).contains(&passed_over),
            "{passed_over} passed over"
        );
    }
}
