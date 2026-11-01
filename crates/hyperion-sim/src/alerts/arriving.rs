//! A detection's light as it arrives at the observer (plan 12, P12.T8.b.1): the flux the server's
//! alert schedule bisects for the times it crosses a sensor's limits.
//!
//! A walk's [`Detection`](super::Detection) reports its flux at one time: an onset's at its peak,
//! and an event in progress at the window's start. Its [`DetectionLight`] gives the flux at every
//! time of the observer's clock, from the source the walk read and through the line of sight it
//! read it through: the detection's own distance, transmission and light time, held over the
//! event's light. So the light at the detection's own time is the walk's flux, and the host's
//! motion over the event's light is neglected, as the walk neglects it over its window.
//!
//! - A supernova's light is its template's in the band ([`supernova_luminosity`]) at its age since
//!   its onset arrived, as the one-shot walk reads it ([`oneshot`](super::oneshot)).
//! - A binary event of plan 11's is its curve in the band while the event is active
//!   ([`binary_luminosity`]), the walk's own: a merger's luminous red nova or kilonova (the
//!   one-shot walk), or a nova, an X-ray transient's outburst, a Type I burst or a Be/X outburst
//!   (the recurrent walk).
//! - A star's flare or FU Orionis outburst is its own light, (factor − 1) times the star's
//!   quiescent luminosity, times the kind's share of it in the band, emitted one light time before
//!   ([`local`](super::local)).
//! - A magnetar's short burst or giant flare is a top hat in X-rays, its 2–10 keV light from its
//!   onset to its end, as the recurrent walk reads it ([`recurrent`](super::recurrent); research's
//!   ruling of 2026-10-31).
//! - The galactic centre's flares and disruptions ([`centre`](super::centre), P12.T5.d): a flare is
//!   its own 2–10 keV light above the quiescence; a disruption's tail is its band's share of the
//!   tail's whole light, the 2–10 keV share or a hot blackbody's in a photometric band. Both leave
//!   the black hole at rest at the origin, so their light time is the detection's.
//! - A pulsar glitch gives no light a sensor counts as flux, which its light says
//!   ([`DetectionLight::Unlit`]).
//!
//! Each light names knots on the observer's clock between which it is monotone
//! ([`ArrivingLight::knots`]); it is zero before the first and constant after the last.
//!
//! Nothing here draws a random number or changes generated output: lights only read.

use crate::galaxy::catalogue_classes::supernova::SupernovaKind;
use crate::galaxy::features::centre::events::{CentreFlare, DisruptionTail};
use crate::observe::light_time;
use crate::stellar::binary::light_curves::BinaryLightCurve;
use crate::stellar::events::flares::Flare;
use crate::stellar::events::fu_orionis::Outburst;
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{Metres, Watts, WattsPerSquareMetre, Years};

use super::AlertBand;
use super::light::{binary_duration, binary_luminosity, peak_grid, supernova_luminosity};
use super::reach::inverse_square_flux;

/// How long after its explosion a supernova's template is followed: 228 Julian years, after which
/// no template gives light, every term's exponential being below the smallest `f64`
/// ([`oneshot`](super::oneshot)).
const SUPERNOVA_LIGHT_YEARS: f64 = 228.0;

/// What a detection's light does at the observer over the observer's time (module
/// documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectionLight {
    /// The event's flux in the detection's band at each time.
    Lit(ArrivingLight),
    /// The event gives no light a sensor counts as flux: a pulsar glitch, which a radio sensor
    /// times (P12.T5.e).
    Unlit,
}

/// A lit detection's flux in its band at each time of the observer's clock (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrivingLight {
    source: Source,
    band: AlertBand,
    /// How long the light takes to the observer, held at the detection's own.
    delay: Span,
    /// The distance it crosses, held at the detection's own.
    distance: Metres,
    /// The share of it the line of sight lets through in the band ([`transmission`]).
    ///
    /// [`transmission`]: super::transmission
    transmission: f64,
}

/// What an [`ArrivingLight`] is the light of.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Source {
    /// A supernova of this kind, which went off at this time.
    Supernova(SupernovaKind, UniverseTime),
    /// A merger's binary event of this curve, whose onset was at this time.
    Binary(BinaryLightCurve, UniverseTime),
    /// A star's flare, the star's quiescent luminosity, and the flare's share of its light in the
    /// band.
    Flare(Flare, Watts, f64),
    /// An FU Orionis outburst, the star's quiescent luminosity at its onset, and the outburst's
    /// share of its light in the band.
    Outburst(Outburst, Watts, f64),
    /// A magnetar's outburst: its light in the band from its onset, inclusive, to its end,
    /// exclusive, both as emitted.
    TopHat(Watts, UniverseTime, UniverseTime),
    /// One of the central black hole's flares, its own light in 2–10 keV.
    CentreFlare(CentreFlare),
    /// A tidal disruption's tail, and its band's share of the tail's whole light.
    Tail(DisruptionTail, f64),
}

impl ArrivingLight {
    /// The light in `band` of a supernova of `kind` that went off at `onset`, `distance` away
    /// through a line of sight of `transmission` in the band: its light arrives one
    /// [`light_time`] after it left.
    #[must_use]
    pub(super) fn supernova(
        kind: SupernovaKind,
        onset: UniverseTime,
        band: AlertBand,
        distance: Metres,
        transmission: f64,
    ) -> Self {
        Self {
            source: Source::Supernova(kind, onset),
            band,
            delay: light_time(distance),
            distance,
            transmission,
        }
    }

    /// The light in `band` of a merger's binary event of `curve` whose onset was at `onset`,
    /// `distance` away through a line of sight of `transmission` in the band: its light arrives
    /// one [`light_time`] after it left.
    #[must_use]
    pub(super) fn binary(
        curve: BinaryLightCurve,
        onset: UniverseTime,
        band: AlertBand,
        distance: Metres,
        transmission: f64,
    ) -> Self {
        Self {
            source: Source::Binary(curve, onset),
            band,
            delay: light_time(distance),
            distance,
            transmission,
        }
    }

    /// The light in `band`, of `share` of its own light, of `flare` of a star of `quiescent`
    /// luminosity, emitted `delay` before it arrives `distance` away through a line of sight of
    /// `transmission`.
    #[must_use]
    pub(super) const fn flare(
        (flare, quiescent, share): (Flare, Watts, f64),
        band: AlertBand,
        delay: Span,
        distance: Metres,
        transmission: f64,
    ) -> Self {
        Self {
            source: Source::Flare(flare, quiescent, share),
            band,
            delay,
            distance,
            transmission,
        }
    }

    /// The light of an FU Orionis outburst, as [`flare`](Self::flare)'s.
    #[must_use]
    pub(super) const fn outburst(
        (outburst, quiescent, share): (Outburst, Watts, f64),
        band: AlertBand,
        delay: Span,
        distance: Metres,
        transmission: f64,
    ) -> Self {
        Self {
            source: Source::Outburst(outburst, quiescent, share),
            band,
            delay,
            distance,
            transmission,
        }
    }

    /// The light in X-rays of a magnetar's short burst or giant flare of `xray` in 2–10 keV from
    /// `onset` to `end`, `distance` away through a line of sight of `transmission` in X-rays: it
    /// arrives one [`light_time`] after it left.
    #[must_use]
    pub(super) fn magnetar(
        (xray, onset, end): (Watts, UniverseTime, UniverseTime),
        distance: Metres,
        transmission: f64,
    ) -> Self {
        Self {
            source: Source::TopHat(xray, onset, end),
            band: AlertBand::XRay,
            delay: light_time(distance),
            distance,
            transmission,
        }
    }

    /// The light in X-rays of `flare`, one of the central black hole's, `distance` away through a
    /// line of sight of `transmission` in X-rays: it arrives one [`light_time`] after it left the
    /// black hole, which is at rest (P12.T5.d).
    #[must_use]
    pub(super) fn centre_flare(flare: CentreFlare, distance: Metres, transmission: f64) -> Self {
        Self {
            source: Source::CentreFlare(flare),
            band: AlertBand::XRay,
            delay: light_time(distance),
            distance,
            transmission,
        }
    }

    /// The light in `band` of `tail`, of `share` of its whole light there, `distance` away through
    /// a line of sight of `transmission` in the band: it arrives one [`light_time`] after it left
    /// the black hole (P12.T5.d).
    #[must_use]
    pub(super) fn disruption_tail(
        (tail, share): (DisruptionTail, f64),
        band: AlertBand,
        distance: Metres,
        transmission: f64,
    ) -> Self {
        Self {
            source: Source::Tail(tail, share),
            band,
            delay: light_time(distance),
            distance,
            transmission,
        }
    }

    /// The band the flux is in: the detection's.
    #[must_use]
    pub const fn band(&self) -> AlertBand {
        self.band
    }

    /// The flux in the band at the observer at `t`, W m⁻², after extinction: zero before the
    /// event's light arrives, finite and non-negative throughout, and continuous after the onset.
    ///
    /// # Panics
    ///
    /// If `t` and the event's light are further apart than the clock holds, which no time on it
    /// is.
    #[must_use]
    pub fn flux_at(&self, t: UniverseTime) -> WattsPerSquareMetre {
        let luminosity = match self.source {
            Source::Supernova(kind, onset) => {
                let arrival = onset
                    .checked_add(self.delay)
                    .expect("a light time across the cube keeps a time on the clock");
                let age = t
                    .checked_since(arrival)
                    .expect("two clock times are a span apart")
                    .as_julian_years_f64();
                supernova_luminosity(kind, Years::new(age), self.band)
            }
            Source::Binary(curve, onset) => {
                let arrival = onset
                    .checked_add(self.delay)
                    .expect("a light time across the cube keeps a time on the clock");
                let age = t
                    .checked_since(arrival)
                    .expect("two clock times are a span apart");
                binary_luminosity(&curve, age, self.band)
            }
            Source::Flare(flare, quiescent, share) => {
                let factor = flare.luminosity_factor_at(self.emitted(t));
                Watts::new((factor - 1.0) * quiescent.value() * share)
            }
            Source::Outburst(outburst, quiescent, share) => {
                let factor = outburst.luminosity_factor_at(self.emitted(t));
                Watts::new((factor - 1.0) * quiescent.value() * share)
            }
            Source::TopHat(level, onset, end) => {
                let emitted = self.emitted(t);
                if onset <= emitted && emitted < end {
                    level
                } else {
                    Watts::ZERO
                }
            }
            Source::CentreFlare(flare) => Watts::new(flare.luminosity_watts_at(self.emitted(t))),
            Source::Tail(tail, share) => {
                Watts::new(share * tail.bolometric_watts_at(self.emitted(t)))
            }
        };
        inverse_square_flux(luminosity, self.distance)
            .map_or(WattsPerSquareMetre::new(0.0), |bare| {
                bare * self.transmission
            })
    }

    /// The times on the observer's clock, strictly increasing, between which the light is
    /// monotone: zero before the first, and constant from the last.
    ///
    /// A flare's and an outburst's are their onset, peak and end, arriving one light time later, and
    /// a magnetar's top hat its onset and end, the light constant between and gone at the end. The
    /// centre's flares' are their onset, peak and end too. A disruption's tail's are its
    /// disruption, dark until its fallback time, then that time, at which the light is its peak,
    /// rounded up to the nanosecond so that it holds the peak and not the dark before it, and from
    /// which it falls for ever under either of the tails' laws ([`DisruptionTail`]), so its last is
    /// the clock window's end, past which no observer's time lies, if the peak arrives before it. A
    /// merger's binary event's are its onset, its peak and its plateau's end, which a kilonova has
    /// none of, and its end, after which it gives no light ([`binary_luminosity`]): it rises
    /// linearly, holds its plateau and fades, monotone between them but for the nanosecond its
    /// peak is floored to. A
    /// supernova's are its arrival and the ages of the grid its peak is taken on (the explosion,
    /// every hour of the first day, and every twentieth of a day to 1,096 days;
    /// [`light`](super::light)), then 228 years, when no template gives light. Every template
    /// term falls after its peak, so the light is monotone between them where no term turns
    /// between two neighbours of the grid, and within one of their spacings, an hour or 72
    /// minutes, where one does.
    ///
    /// # Panics
    ///
    /// If a knot falls off the clock, which a light arriving within centuries of a time on it
    /// cannot.
    #[must_use]
    pub fn knots(&self) -> Vec<UniverseTime> {
        let arrive = |emitted: UniverseTime| {
            emitted
                .checked_add(self.delay)
                .expect("a light time across the cube keeps a time on the clock")
        };
        match self.source {
            Source::Supernova(_, onset) => {
                let arrival = arrive(onset);
                let after = |seconds: f64| {
                    Span::from_seconds_f64(seconds)
                        .and_then(|age| arrival.checked_add(age))
                        .expect("a supernova's light ends within centuries of a clock time")
                };
                let mut knots: Vec<_> = peak_grid()
                    .map(|age| after(age.value() * SECONDS_PER_JULIAN_YEAR))
                    .collect();
                knots.push(after(SUPERNOVA_LIGHT_YEARS * SECONDS_PER_JULIAN_YEAR));
                knots.dedup();
                knots
            }
            Source::Flare(flare, ..) => {
                let peak = Span::from_seconds_f64(flare.half_width_seconds())
                    .and_then(|half_width| flare.onset().checked_add(half_width))
                    .expect("a flare peaks minutes after an onset the clock holds");
                increasing([flare.onset(), peak, flare.end()].map(arrive))
            }
            Source::Outburst(outburst, ..) => {
                increasing([outburst.onset(), outburst.peak(), outburst.end()].map(arrive))
            }
            Source::TopHat(_, onset, end) => increasing([onset, end].map(arrive)),
            Source::CentreFlare(flare) => {
                let peak = Span::from_seconds_f64(flare.duration_seconds() / 2.0)
                    .and_then(|half| flare.onset().checked_add(half))
                    .expect("a flare peaks hours after an onset the clock holds");
                increasing([flare.onset(), peak, flare.end()].map(arrive))
            }
            Source::Tail(tail, _) => {
                let peak = Span::from_seconds_f64(tail.fallback_time().value())
                    .and_then(|fallback| fallback.checked_add(Span::new(0, 1).ok()?))
                    .and_then(|fallback| tail.disruption().checked_add(fallback))
                    .expect("a tail peaks months after a disruption the clock holds");
                let mut knots = increasing([tail.disruption(), peak].map(arrive));
                if knots.last().is_some_and(|&last| last < ClockWindow::END) {
                    knots.push(ClockWindow::END);
                }
                knots
            }
            Source::Binary(curve, onset) => {
                let arrival = arrive(onset);
                let top = curve
                    .fades_below(1.0)
                    .expect("a binary event's plateau ends within the clock");
                increasing(
                    [Span::ZERO, curve.peak(), top, binary_duration(&curve)].map(|age| {
                        arrival
                            .checked_add(age)
                            .expect("a binary event's light ends within the clock")
                    }),
                )
            }
        }
    }

    /// When the light arriving at `t` left the host.
    #[must_use]
    fn emitted(&self, t: UniverseTime) -> UniverseTime {
        t.checked_sub(self.delay)
            .expect("a light time across the cube keeps a time on the clock")
    }
}

/// `knots` without the repeats a span floored to nothing leaves.
#[must_use]
fn increasing<const N: usize>(knots: [UniverseTime; N]) -> Vec<UniverseTime> {
    let mut knots = knots.to_vec();
    knots.dedup();
    knots
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::catalogue_classes::type_ia::IaChannel;
    use crate::galaxy::gas::ccm::Band;
    use crate::stellar::remnant::SupernovaType;

    const V: AlertBand = AlertBand::Photometric(Band::V);

    /// Every supernova template's light, seen 100 ly away through half its light.
    fn supernovae() -> Vec<ArrivingLight> {
        let kinds = [
            SupernovaKind::CoreCollapse(SupernovaType::IIP),
            SupernovaKind::CoreCollapse(SupernovaType::IIL),
            SupernovaKind::CoreCollapse(SupernovaType::IIb),
            SupernovaKind::CoreCollapse(SupernovaType::Ib),
            SupernovaKind::CoreCollapse(SupernovaType::Ic),
            SupernovaKind::TypeIa(IaChannel::Merger),
            SupernovaKind::TypeIa(IaChannel::Iax),
        ];
        let distance = Metres::new(100.0 * crate::units::consts::METRES_PER_LIGHT_YEAR);
        kinds
            .into_iter()
            .flat_map(|kind| {
                [V, AlertBand::Photometric(Band::K), AlertBand::XRay].map(|band| {
                    ArrivingLight::supernova(kind, UniverseTime::EPOCH, band, distance, 0.5)
                })
            })
            .collect()
    }

    /// A merger's light, red novae's and kilonovae's of the classes' least and most: zero before
    /// its first knot and from its last, its peak's flux throughout a red nova's plateau, and
    /// monotone between its knots to a part in 10¹² of its peak (its peak is floored to the
    /// nanosecond), rising to its peak and falling after it.
    #[test]
    fn a_mergers_light_is_monotone_between_its_knots_and_gone_after_its_end() {
        use crate::stellar::binary::light_curves::{
            BinaryLightCurve, GW170817_EJECTA_MASS, KilonovaCurve, LuminousRedNovaCurve,
        };
        use crate::units::SolarMasses;
        let red =
            |m| BinaryLightCurve::from(LuminousRedNovaCurve::of(SolarMasses::new(m)).unwrap());
        let kilonova = |m| BinaryLightCurve::from(KilonovaCurve::of(SolarMasses::new(m)).unwrap());
        let curves = [
            red(0.2),
            red(300.0),
            kilonova(GW170817_EJECTA_MASS.value()),
            kilonova(1e-4),
        ];
        let distance = Metres::new(100.0 * crate::units::consts::METRES_PER_LIGHT_YEAR);
        let year = Span::from_julian_years(1).unwrap();
        for curve in curves {
            for band in [V, AlertBand::Photometric(Band::K), AlertBand::XRay] {
                let light = ArrivingLight::binary(curve, UniverseTime::EPOCH, band, distance, 0.5);
                let knots = light.knots();
                assert!(knots.windows(2).all(|pair| pair[0] < pair[1]), "{light:?}");
                let first = knots[0];
                let last = *knots.last().unwrap();
                let nanosecond = Span::new(0, 1).unwrap();
                assert_same_bits(
                    light
                        .flux_at(first.checked_sub(nanosecond).unwrap())
                        .value(),
                    0.0,
                );
                assert_same_bits(light.flux_at(last).value(), 0.0);
                assert_same_bits(light.flux_at(last.checked_add(year).unwrap()).value(), 0.0);
                let peak = super::super::light::binary_peak(&curve, band).value();
                let flux = super::super::reach::inverse_square_flux(Watts::new(peak), distance)
                    .unwrap()
                    .value()
                    * 0.5;
                let slack = 1e-12 * flux;
                for (k, pair) in knots.windows(2).enumerate() {
                    let gap = pair[1].checked_since(pair[0]).unwrap().as_seconds_f64();
                    let samples: Vec<f64> = (0..16)
                        .map(|i| {
                            let at = Span::from_seconds_f64(gap * f64::from(i) / 16.0).unwrap();
                            light.flux_at(pair[0].checked_add(at).unwrap()).value()
                        })
                        .collect();
                    assert!(samples.iter().all(|f| f.is_finite() && *f >= 0.0));
                    let rising = k == 0;
                    for step in samples.windows(2) {
                        let ok = if rising {
                            step[1] >= step[0] - slack
                        } else {
                            step[1] <= step[0] + slack
                        };
                        assert!(ok, "{light:?} between knots {k} and {}: {samples:?}", k + 1);
                    }
                    assert!(samples.iter().all(|f| *f <= flux + slack), "{light:?}");
                }
                if let BinaryLightCurve::LuminousRedNova(nova) = curve {
                    let plateau = Span::from_seconds_f64(
                        curve.peak().as_seconds_f64() + nova.plateau().value() * 43_200.0,
                    )
                    .unwrap();
                    let middle = first.checked_add(plateau).unwrap();
                    assert!((light.flux_at(middle).value() - flux).abs() <= slack);
                }
            }
        }
    }

    /// Each light is zero before its first knot, finite and non-negative at and between its
    /// knots, and moves by a sliver of its peak over a nanosecond anywhere after its onset.
    #[test]
    fn a_supernovas_light_is_finite_non_negative_and_continuous_after_its_onset() {
        let millisecond = Span::new(0, 1_000_000).unwrap();
        let nanosecond = Span::new(0, 1).unwrap();
        for light in supernovae() {
            let knots = light.knots();
            assert!(knots.windows(2).all(|pair| pair[0] < pair[1]));
            let before = knots[0].checked_sub(millisecond).unwrap();
            assert_same_bits(light.flux_at(before).value(), 0.0);
            let peak = knots
                .iter()
                .map(|&t| light.flux_at(t).value())
                .fold(0.0, f64::max);
            // No jump: a nanosecond's step changes the flux by a sliver of its peak.
            for pair in knots.windows(2).step_by(97) {
                let middle = pair[0]
                    .checked_add(
                        Span::from_seconds_f64(
                            pair[1].checked_since(pair[0]).unwrap().as_seconds_f64() / 2.0,
                        )
                        .unwrap(),
                    )
                    .unwrap();
                for t in [pair[0], middle] {
                    let here = light.flux_at(t).value();
                    let next = light.flux_at(t.checked_add(nanosecond).unwrap()).value();
                    assert!(here.is_finite() && here >= 0.0, "{light:?} at {t}: {here}");
                    assert!(
                        (next - here).abs() <= 1e-6 * peak,
                        "{light:?} jumps at {t}: {here} to {next}"
                    );
                }
            }
        }
    }
}
