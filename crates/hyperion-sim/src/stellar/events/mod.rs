//! Single-star events in time: flares, pulsar glitches, magnetar bursts and giant flares, FU
//! Orionis outbursts, the giant eruptions of luminous blue variables and thermal pulses, each built
//! on the generic, random-access constructions of `hyperion_sim::events` and keyed on the star's own
//! streams (plan 06, phase F).
//!
//! An event may change what a console reads, never what the star's track is (plan 06, design
//! note 14).
//!
//! - [`flares`]: the flares of stars with convective envelopes, Poisson bins of a day (P06.T28.a).
//! - [`fu_orionis`]: the FU Orionis outbursts of young stars, Poisson bins of 500 years
//!   (P06.T28.d).
//! - [`glitches`]: pulsar glitches, Crab-like on Poisson bins of 30 days and Vela-like on a
//!   monotone phase (P06.T28.b).
//! - [`lbv`]: the giant eruptions of luminous blue variables, Poisson bins of 100 years
//!   (P06.T28.e).
//! - [`magnetar`]: the outburst episodes, short bursts and giant flares of magnetars, Poisson bins
//!   of 30 days, 15 minutes and a year, their rates scaled by the field's fractional decay
//!   (P06.T28.c).
//! - [`thermal_pulse`]: the thermal pulses of stars on the thermally pulsing AGB, on a monotone
//!   phase of their track's own pulse count (P06.T28.f).
//!
//! # Assembly (P06.T28.g)
//!
//! [`StarEvents`] builds each kind's model of one star once and lists its events of every kind as
//! [`StarEvent`]s, over a window ([`events_in`]) or running at a time ([`active_at`]), each by
//! onset, then kind, then ID; [`StarEvents::at`] also composes the light they make,
//! [`EventEffects`], which a star's summary carries beside its track's state, the state itself
//! unchanged:
//!
//! - L = min(L(t) + Σ `ΔLᵢ`, max(`L_Edd`, L(t))) + ΔL of a giant eruption: L(t) the track's, and the
//!   excesses `ΔLᵢ` = (Fᵢ − 1) × L summed in the order flares (L the flare model's, the epoch's),
//!   FU Orionis outbursts and the thermal pulse (L = L(t)), which is L(t) [1 + Σ (Fᵢ − 1)] with
//!   each flare's factor rescaled to L(t) (P06.T28.a and T28.d's leans); held at the Eddington
//!   luminosity of electron scattering at the surface hydrogen, as the outbursts' own peak factors
//!   are;
//! - the eruption's excess, (F − 1) × its quiescent luminosity, beyond that cap by design
//!   (P06.T28.e);
//! - a thermal pulse scales the radius by √F, so that `T_eff` holds (P06.T28.f's lean);
//! - `M_V` follows L at the track's bolometric correction, `M_V`(t) − 2.5 log₁₀(L ÷ L(t))
//!   (research-leans-12 §3): exact for a thermal pulse, whose `T_eff` holds; for a flare's, an
//!   outburst's or an eruption's light, which is not at the photosphere's colour, the bolometric
//!   change alone ([`EventEffects::absolute_magnitude_v`] gives its error's sign and size);
//! - a magnetar's bursts and giant flares carry their own mean luminosities, which are listed and
//!   never added (P06.T28.c);
//! - a pulsar's spin frequency is offset by its Crab-like glitches still recovering and its
//!   Vela-like sawtooth (P06.T28.b).

pub mod flares;
pub mod fu_orionis;
pub mod glitches;
pub mod lbv;
pub mod magnetar;
pub mod thermal_pulse;

use crate::events::{EventId, EventSeries, TimeWindow, tags};
use crate::id::BodyId;
use crate::math;
use crate::rng::Seed;
use crate::stellar::photometry::absolute_magnitude_v_under;
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::wind;
use crate::stellar::system::StarModel;
use crate::stellar::{Composition, StarState};
use crate::time::{Span, UniverseTime};
use crate::units::{Magnitudes, SolarLuminosities, SolarRadii};

use flares::{Flare, FlareModel};
use fu_orionis::{FuOrionisModel, Outburst, OutburstClass};
use glitches::{Glitch, GlitchCycle, GlitchModel, LargeGlitch};
use lbv::{Eruption, LbvModel};
use magnetar::{Burst, GiantFlare, MagnetarModel, MagnetarSeries};
use thermal_pulse::{ThermalPulse, ThermalPulseModel};

/// What kind of single-star event a [`StarEvent`] is (P06.T28.g): the wire's kinds. A magnetar's
/// outburst episode is none: its bursts are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StarEventKind {
    /// A flare of a star with a convective envelope ([`flares`]).
    Flare,
    /// A pulsar's glitch, Crab-like or Vela-like ([`glitches`]).
    Glitch,
    /// A magnetar's short burst ([`magnetar`]).
    MagnetarBurst,
    /// A magnetar's giant flare ([`magnetar`]).
    MagnetarGiantFlare,
    /// An FU Orionis outburst of a young star ([`fu_orionis`]), of either class.
    FuOrionisOutburst,
    /// A giant eruption of a luminous blue variable ([`lbv`]).
    GiantEruption,
    /// A thermal pulse of a star on the thermally pulsing AGB ([`thermal_pulse`]).
    ThermalPulse,
}

/// One event of a single star, of any kind (P06.T28.g): its kind, ID, onset and duration, an FU
/// Orionis outburst's class, and a magnetar event's own mean luminosity.
///
/// The duration is how long the event runs from its onset: a flare's, an outburst's, an
/// eruption's and a burst's own; a thermal pulse's transient (its rapid peak and slow dip); a
/// giant flare's initial spike ([`GIANT_FLARE_SPIKE`](magnetar::GIANT_FLARE_SPIKE)); a Crab-like
/// glitch's [`RECOVERY_SPAN`](glitches::RECOVERY_SPAN), to its offset's end; and a Vela-like
/// glitch's recovery time `τ_d` ([`LARGE_GLITCH_DURATION`](glitches::LARGE_GLITCH_DURATION)), its
/// sawtooth running on to the next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarEvent {
    kind: StarEventKind,
    id: EventId,
    onset: UniverseTime,
    duration: Span,
    outburst_class: Option<OutburstClass>,
    luminosity_watts: Option<f64>,
}

impl StarEvent {
    /// The event's kind.
    #[must_use]
    pub const fn kind(&self) -> StarEventKind {
        self.kind
    }

    /// The event's ID, from which its kind's module regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The onset.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// How long the event runs from its onset (see [`StarEvent`]).
    #[must_use]
    pub const fn duration(&self) -> Span {
        self.duration
    }

    /// An FU Orionis outburst's class, FU Ori or V1647 Ori; `None` for any other kind. The wire
    /// does not carry it (the lane's lean, 2026-10-31).
    #[must_use]
    pub const fn outburst_class(&self) -> Option<OutburstClass> {
        self.outburst_class
    }

    /// A magnetar burst's or giant flare's mean luminosity over its duration, W: absolute, listed
    /// and never composed into the star's light; `None` for any other kind.
    #[must_use]
    pub const fn luminosity_watts(&self) -> Option<f64> {
        self.luminosity_watts
    }

    /// Whether the event runs at `t`: from its onset, inclusive, for its duration.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        t.checked_since(self.onset)
            .is_some_and(|elapsed| !elapsed.is_negative() && elapsed < self.duration)
    }

    /// The order events are listed in: by onset, then kind, then ID.
    #[must_use]
    fn order(&self, other: &Self) -> core::cmp::Ordering {
        self.onset
            .cmp(&other.onset)
            .then(self.kind.cmp(&other.kind))
            .then(self.id.cmp(&other.id))
    }

    /// An event of `kind` with no class and no luminosity of its own.
    #[must_use]
    const fn plain(kind: StarEventKind, id: EventId, onset: UniverseTime, duration: Span) -> Self {
        Self {
            kind,
            id,
            onset,
            duration,
            outburst_class: None,
            luminosity_watts: None,
        }
    }

    #[must_use]
    fn of_flare(flare: &Flare) -> Self {
        Self::plain(
            StarEventKind::Flare,
            flare.id(),
            flare.onset(),
            flare.duration(),
        )
    }

    #[must_use]
    const fn of_glitch(glitch: &Glitch) -> Self {
        Self::plain(
            StarEventKind::Glitch,
            glitch.id(),
            glitch.onset(),
            glitches::RECOVERY_SPAN,
        )
    }

    #[must_use]
    const fn of_large_glitch(glitch: &LargeGlitch) -> Self {
        Self::plain(
            StarEventKind::Glitch,
            glitch.id(),
            glitch.onset(),
            glitches::LARGE_GLITCH_DURATION,
        )
    }

    #[must_use]
    fn of_burst(burst: &Burst) -> Self {
        Self {
            luminosity_watts: Some(burst.mean_luminosity_watts()),
            ..Self::plain(
                StarEventKind::MagnetarBurst,
                burst.id(),
                burst.onset(),
                burst.duration(),
            )
        }
    }

    #[must_use]
    fn of_giant_flare(flare: &GiantFlare) -> Self {
        Self {
            luminosity_watts: Some(flare.mean_luminosity_watts()),
            ..Self::plain(
                StarEventKind::MagnetarGiantFlare,
                flare.id(),
                flare.onset(),
                magnetar::GIANT_FLARE_SPIKE,
            )
        }
    }

    #[must_use]
    fn of_outburst(outburst: &Outburst) -> Self {
        Self {
            outburst_class: Some(outburst.class()),
            ..Self::plain(
                StarEventKind::FuOrionisOutburst,
                outburst.id(),
                outburst.onset(),
                outburst.duration(),
            )
        }
    }

    #[must_use]
    fn of_eruption(eruption: &Eruption) -> Self {
        Self::plain(
            StarEventKind::GiantEruption,
            eruption.id(),
            eruption.onset(),
            eruption.duration(),
        )
    }

    #[must_use]
    fn of_pulse(pulse: &ThermalPulse) -> Self {
        let duration = pulse
            .transient_end()
            .checked_since(pulse.onset())
            .unwrap_or(Span::ZERO);
        Self::plain(
            StarEventKind::ThermalPulse,
            pulse.id(),
            pulse.onset(),
            duration,
        )
    }
}

/// What a star's active events make of what a console reads at one time, on top of its track's
/// state, which they never change (P06.T28.g; design note 14): its bolometric luminosity, its
/// radius, its absolute visual magnitude and a pulsar's spin frequency.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventEffects {
    luminosity: SolarLuminosities,
    radius: SolarRadii,
    absolute_magnitude_v: Option<Magnitudes>,
    spin_frequency_offset_hz: f64,
}

impl EventEffects {
    /// The bolometric luminosity of the star and its disc, L☉: the track's L(t) plus the active
    /// flares', FU Orionis outbursts' and thermal pulse's excesses, held at the Eddington
    /// luminosity (or the track's own, where that is above it), plus a giant eruption's excess,
    /// which exceeds it by design (see the [module documentation](self)). A magnetar's bursts and
    /// giant flares, magnetospheric, are listed with their own luminosities and not added.
    #[must_use]
    pub const fn luminosity(&self) -> SolarLuminosities {
        self.luminosity
    }

    /// The radius, R☉: the track's, times √F during a thermal pulse, so that `T_eff` holds.
    #[must_use]
    pub const fn radius(&self) -> SolarRadii {
        self.radius
    }

    /// The absolute visual magnitude, mag, of the star with this [`luminosity`](Self::luminosity):
    /// the track's `M_V` less 2.5 log₁₀(L ÷ L(t)), or `None` where the track's state has none.
    ///
    /// It is the bolometric change at the track's bolometric correction (research-leans-12 §3),
    /// which takes the event light at the photosphere's colour. That is exact for a thermal pulse,
    /// whose `T_eff` holds. The V change of light at another colour, x = ΔL ÷ L(t) at a correction
    /// `BC_ev`, is −2.5 log₁₀[1 + x 10^(0.4 (`BC_ev` − `BC_*`))], so this understates the
    /// brightening of a hot flare on a K or M dwarf (by up to an order of magnitude in V flux at
    /// M5), of an FU Orionis outburst's F–G disc light and of an eruption's cooler
    /// pseudo-photosphere on a hotter star, and slightly overstates a flare's on an F or G dwarf.
    /// A magnetar's listed luminosities do not move it.
    #[must_use]
    pub const fn absolute_magnitude_v(&self) -> Option<Magnitudes> {
        self.absolute_magnitude_v
    }

    /// The offset of a pulsar's spin frequency from its spin-down's, Hz: its Crab-like glitches
    /// still recovering, summed in listing order, plus a Vela-like pulsar's sawtooth; 0 for any
    /// other star.
    #[must_use]
    pub const fn spin_frequency_offset_hz(&self) -> f64 {
        self.spin_frequency_offset_hz
    }
}

/// A star's event kinds, each kind's model built once (P06.T28.g): for listing its events over a
/// window, those running at a time, and the light they make then.
///
/// `seed` and `body` are the star's universe and body, from which every kind's series is opened:
/// [`StarModel`] holds neither. Each listing depends on its arguments alone, never on what was
/// asked before, and a listing over a window is the union of the listings over any partition of
/// it, in any order of asking, since each kind's is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarEvents<'a> {
    flares: Option<(FlareModel<'a>, EventSeries)>,
    outbursts: Option<(FuOrionisModel<'a>, EventSeries)>,
    glitches: Option<(GlitchModel<'a>, EventSeries)>,
    vela: Option<GlitchCycle<'a>>,
    magnetar: Option<(MagnetarModel<'a>, MagnetarSeries)>,
    lbv: Option<(LbvModel<'a>, EventSeries)>,
    pulses: Option<ThermalPulseModel<'a>>,
    composition: &'a Composition,
    rules: StellarRules,
}

impl<'a> StarEvents<'a> {
    /// The event kinds of `star`, whose body in the universe of `seed` is `body`: each kind's
    /// model where the star has the kind, built once.
    #[must_use]
    pub fn of(star: &'a StarModel, seed: Seed, body: BodyId) -> Self {
        let series = |tag| EventSeries::new(seed, tag, body.into());
        let glitches = GlitchModel::of(star);
        Self {
            flares: FlareModel::of(star).map(|m| (m, series(tags::STAR_FLARE))),
            outbursts: FuOrionisModel::of(star).map(|m| (m, series(tags::STAR_FU_ORIONIS))),
            vela: glitches.and_then(|m| GlitchCycle::of_model(m, seed, body)),
            glitches: glitches.map(|m| (m, series(tags::STAR_GLITCH))),
            magnetar: MagnetarModel::of(star).map(|m| (m, MagnetarSeries::new(seed, body))),
            lbv: LbvModel::of(star).map(|m| (m, series(tags::STAR_LBV_ERUPTION))),
            pulses: ThermalPulseModel::of(star, &series(tags::STAR_THERMAL_PULSE)),
            composition: star.composition(),
            rules: star.rules(),
        }
    }

    /// The star's thermal pulses, if it pulses: what P06.T24.b's C/O reads.
    #[must_use]
    pub(crate) const fn pulses(&self) -> Option<&ThermalPulseModel<'a>> {
        self.pulses.as_ref()
    }

    /// Appends to `out` the events of every kind with onsets in `window`, by onset, then kind,
    /// then ID.
    pub fn events_in(&self, window: TimeWindow, out: &mut Vec<StarEvent>) {
        let from = out.len();
        let listed = out;
        if let Some((model, series)) = &self.flares {
            let mut flares = Vec::new();
            model.flares_in(series, window, &mut Vec::new(), &mut flares);
            listed.extend(flares.iter().map(StarEvent::of_flare));
        }
        if let Some((model, series)) = &self.glitches {
            let mut glitches = Vec::new();
            model.glitches_in(series, window, &mut glitches);
            listed.extend(glitches.iter().map(StarEvent::of_glitch));
        }
        if let Some(cycle) = &self.vela {
            let mut large = Vec::new();
            cycle.large_glitches_in(window, &mut large);
            listed.extend(large.iter().map(StarEvent::of_large_glitch));
        }
        if let Some((model, series)) = &self.magnetar {
            let (mut bursts, mut giant) = (Vec::new(), Vec::new());
            model.bursts_in(series, window, &mut bursts);
            model.giant_flares_in(series, window, &mut giant);
            listed.extend(bursts.iter().map(StarEvent::of_burst));
            listed.extend(giant.iter().map(StarEvent::of_giant_flare));
        }
        if let Some((model, series)) = &self.outbursts {
            let mut outbursts = Vec::new();
            model.outbursts_in(series, window, &mut outbursts);
            listed.extend(outbursts.iter().map(StarEvent::of_outburst));
        }
        if let Some((model, series)) = &self.lbv {
            let mut eruptions = Vec::new();
            model.eruptions_in(series, window, &mut eruptions);
            listed.extend(eruptions.iter().map(StarEvent::of_eruption));
        }
        if let Some(model) = &self.pulses {
            let mut pulses = Vec::new();
            model.pulses_in(window, &mut pulses);
            listed.extend(pulses.iter().map(StarEvent::of_pulse));
        }
        listed[from..].sort_by(StarEvent::order);
    }

    /// Appends to `out` the events of every kind running at `t`, by onset, then kind, then ID.
    pub fn active_at(&self, t: UniverseTime, out: &mut Vec<StarEvent>) {
        let _ = self.compose(t, None, out);
    }

    /// The events running at `t`, by onset, then kind, then ID, and the light they make of a star
    /// whose track's state then is `state` ([`EventEffects`]), or `None` where none of them
    /// changes its light, radius or spin.
    #[must_use]
    pub fn at(&self, t: UniverseTime, state: &StarState) -> (Vec<StarEvent>, Option<EventEffects>) {
        let mut active = Vec::new();
        let effects = self.compose(t, Some(state), &mut active);
        (active, effects)
    }

    /// Appends the events running at `t` to `active`, sorted, and with `state` gives the light
    /// they make; without it, only lists them.
    fn compose(
        &self,
        t: UniverseTime,
        state: Option<&StarState>,
        active: &mut Vec<StarEvent>,
    ) -> Option<EventEffects> {
        let from = active.len();
        let quiescent = state.map_or(0.0, |s| s.luminosity().value());
        // The excesses over the track's light, L☉: those held at the Eddington luminosity, and
        // the giant eruptions', which are not.
        let (mut held, mut erupting) = (0.0, 0.0);
        let (mut radius_factor, mut offset_hz) = (1.0, 0.0);
        let mut changed = false;
        if let Some((model, series)) = &self.flares {
            let mut flares = Vec::new();
            model.flares_active_at(series, t, &mut flares);
            for flare in &flares {
                // The flare's factor is against the luminosity at the epoch, its model's.
                held +=
                    (flare.luminosity_factor_at(t) - 1.0) * model.quiescent_luminosity().value();
                changed = true;
            }
            active.extend(flares.iter().map(StarEvent::of_flare));
        }
        if let Some((model, series)) = &self.glitches {
            let mut glitches = Vec::new();
            model.glitches_active_at(series, t, &mut glitches);
            for glitch in &glitches {
                offset_hz += glitch.frequency_offset_hz_at(t);
                changed = true;
            }
            active.extend(glitches.iter().map(StarEvent::of_glitch));
        }
        if let Some(cycle) = &self.vela {
            // The sawtooth runs through every cycle while the star is a neutron star.
            if cycle.model().glitches_at(t) {
                offset_hz += cycle.frequency_offset_hz_at(t);
                changed = true;
            }
            active.extend(
                cycle
                    .large_glitch_active_at(t)
                    .as_ref()
                    .map(StarEvent::of_large_glitch),
            );
        }
        if let Some((model, series)) = &self.magnetar {
            let (mut bursts, mut giant) = (Vec::new(), Vec::new());
            model.bursts_active_at(series, t, &mut bursts);
            model.giant_flares_active_at(series, t, &mut giant);
            active.extend(bursts.iter().map(StarEvent::of_burst));
            active.extend(giant.iter().map(StarEvent::of_giant_flare));
        }
        if let Some((model, series)) = &self.outbursts {
            let mut outbursts = Vec::new();
            model.outbursts_active_at(series, t, &mut outbursts);
            for outburst in &outbursts {
                held += (outburst.luminosity_factor_at(t) - 1.0) * quiescent;
                changed = true;
            }
            active.extend(outbursts.iter().map(StarEvent::of_outburst));
        }
        if let Some((model, series)) = &self.lbv {
            let mut eruptions = Vec::new();
            model.eruptions_active_at(series, t, &mut eruptions);
            for eruption in &eruptions {
                erupting +=
                    eruption.luminosity_at(t).value() - eruption.quiescent_luminosity().value();
                changed = true;
            }
            active.extend(eruptions.iter().map(StarEvent::of_eruption));
        }
        if let Some(model) = &self.pulses
            && let Some(pulse) = model.pulse_active_at(t)
        {
            let factor = model.luminosity_factor_at(t);
            held += (factor - 1.0) * quiescent;
            radius_factor = factor.sqrt();
            changed = true;
            active.push(StarEvent::of_pulse(&pulse));
        }
        active[from..].sort_by(StarEvent::order);
        let state = state?;
        if !changed {
            return None;
        }
        // The cap: the Eddington luminosity, as the FU Orionis outbursts' own cap reads it, or the
        // track's light where that is above it.
        let gamma = surface_eddington_factor(state, self.composition);
        let cap = if gamma > 0.0 {
            (quiescent / gamma).max(quiescent)
        } else {
            f64::INFINITY
        };
        let luminosity = (quiescent + held).min(cap) + erupting;
        // At the track's bolometric correction; where the track has an M_V, its L(t) is positive.
        let absolute_magnitude_v = absolute_magnitude_v_under(state, self.rules).map(|track| {
            Magnitudes::new(track.value() - 2.5 * math::log10(luminosity / quiescent))
        });
        Some(EventEffects {
            luminosity: SolarLuminosities::new(luminosity),
            radius: SolarRadii::new(state.radius().value() * radius_factor),
            absolute_magnitude_v,
            spin_frequency_offset_hz: offset_hz,
        })
    }
}

/// The electron-scattering Eddington factor Γ = Γₑ(X = 0) × (1 + X) of a star in `state` of
/// `composition`, at the surface hydrogen X = 0.76 − 3Z of Hurley, Pols and Tout (2000, section
/// 2): the one the FU Orionis outbursts' peak factors and the composed light are held at
/// (P06.T28.d and T28.g).
#[must_use]
pub(crate) fn surface_eddington_factor(state: &StarState, composition: &Composition) -> f64 {
    let x = 0.76 - 3.0 * composition.z_fit().value();
    wind::eddington_factor(state.luminosity(), state.mass()) * (1.0 + x)
}

/// Appends to `out` the events of every kind of `star`, whose body in the universe of `seed` is
/// `body`, with onsets in `window`, by onset, then kind, then ID ([`StarEvents::events_in`]).
///
/// # Examples
///
/// A saturated M dwarf's events over a day, which are all flares, and what the light of one of
/// them makes of the star while it runs:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::events::{StarEventKind, StarEvents, events_in};
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::{Span, UniverseTime};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let dwarf = StarModel::new(SolarMasses::new(0.2), Composition::SOLAR, StarDraws::median(), Years::new(3e8))?;
/// let (seed, body) = (Seed::new(7), BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0));
/// let day = UniverseTime::EPOCH.checked_add(Span::from_seconds(86_400)).ok_or("in range")?;
/// let mut events = Vec::new();
/// events_in(&dwarf, seed, body, TimeWindow::new(UniverseTime::EPOCH, day)?, &mut events);
/// assert!(!events.is_empty() && events.iter().all(|e| e.kind() == StarEventKind::Flare));
/// // A third of the way through the first, the star is brighter than its track.
/// let flare = &events[0];
/// let span = Span::from_seconds_f64(flare.duration().as_seconds_f64() / 3.0).ok_or("a span")?;
/// let t = flare.onset().checked_add(span).ok_or("in range")?;
/// let state = dwarf.state_at(t).ok_or("formed")?;
/// let (running, effects) = StarEvents::of(&dwarf, seed, body).at(t, &state);
/// assert!(running.contains(flare));
/// let effects = effects.ok_or("a flare's light")?;
/// assert!(effects.luminosity() > state.luminosity());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn events_in(
    star: &StarModel,
    seed: Seed,
    body: BodyId,
    window: TimeWindow,
    out: &mut Vec<StarEvent>,
) {
    StarEvents::of(star, seed, body).events_in(window, out);
}

/// Appends to `out` the events of every kind of `star`, whose body in the universe of `seed` is
/// `body`, running at `t`, by onset, then kind, then ID ([`StarEvents::active_at`]). The result
/// depends on its arguments alone, never on what was asked before.
pub fn active_at(
    star: &StarModel,
    seed: Seed,
    body: BodyId,
    t: UniverseTime,
    out: &mut Vec<StarEvent>,
) {
    StarEvents::of(star, seed, body).active_at(t, out);
}

#[cfg(test)]
impl<'a> StarEvents<'a> {
    /// These kinds' models on `star`'s body `body` of the universe of `seed`, the models that need
    /// no series reused: for counting events over many bodies of one star without building its
    /// models again.
    #[must_use]
    fn on_body(&self, star: &'a StarModel, seed: Seed, body: BodyId) -> Self {
        let series = |tag| EventSeries::new(seed, tag, body.into());
        Self {
            flares: self.flares.map(|(m, _)| (m, series(tags::STAR_FLARE))),
            outbursts: self
                .outbursts
                .map(|(m, _)| (m, series(tags::STAR_FU_ORIONIS))),
            vela: self
                .glitches
                .and_then(|(m, _)| GlitchCycle::of_model(m, seed, body)),
            glitches: self.glitches.map(|(m, _)| (m, series(tags::STAR_GLITCH))),
            magnetar: self
                .magnetar
                .map(|(m, _)| (m, MagnetarSeries::new(seed, body))),
            lbv: self.lbv.map(|(m, _)| (m, series(tags::STAR_LBV_ERUPTION))),
            pulses: ThermalPulseModel::of(star, &series(tags::STAR_THERMAL_PULSE)),
            composition: self.composition,
            rules: self.rules,
        }
    }
}

#[cfg(test)]
mod tests {
    //! P06.T28.g's tests: each kind's listing through the assembly is its module's; per kind, the
    //! mean count over 10⁴ stars matches the rate integral; order independence; the bounds; a star
    //! evaluated cold or after earlier times; and the composed light.

    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_poisson_count, poisson_interval};

    use super::*;
    use crate::events::testing::assert_partition_independent;
    use crate::events::{PoissonBins, RateModel};
    use crate::id::SystemId;
    use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
    use crate::stellar::remnant::neutron_star::{
        BIRTH_PERIOD_MEAN, BIRTH_PERIOD_SIGMA, LOG_BIRTH_FIELD_MEAN, LOG_BIRTH_FIELD_SIGMA,
    };
    use crate::stellar::sse::{PhasePredicate, Track};
    use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
    use crate::units::{SolarMasses, Years};

    const SEED: Seed = Seed::new(0x0628_9000_0000_5eed);

    /// Body `index` of one system.
    fn body(index: u16) -> BodyId {
        BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), index)
    }

    /// A solar star of `mass` M☉ at the median draws, `age` years old at the epoch.
    fn star(mass: f64, age: f64) -> StarModel {
        StarModel::new(
            SolarMasses::new(mass),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(age),
        )
        .unwrap()
    }

    /// A 15 M☉ star whose neutron star is born at `period` s with a field of 10^`log_b0` G, `years`
    /// old at the epoch, as `magnetar.rs`' tests build one.
    fn neutron_star(log_b0: f64, period: f64, years: f64) -> StarModel {
        let z = |value: f64| StandardNormal::new(value).unwrap();
        let draws = StarDraws::from_parts(StarDrawsParts {
            ns_field: z((log_b0 - LOG_BIRTH_FIELD_MEAN) / LOG_BIRTH_FIELD_SIGMA),
            ns_spin: [z((period - BIRTH_PERIOD_MEAN.value()) / BIRTH_PERIOD_SIGMA.value()); 8],
            ..StarDrawsParts::MEDIAN
        });
        let lifetime = star(15.0, 1e9).lifetime().unwrap().value();
        StarModel::new(
            SolarMasses::new(15.0),
            Composition::SOLAR,
            draws,
            Years::new(lifetime + years),
        )
        .unwrap()
    }

    /// A solar star of `mass` M☉ whose age at the epoch is `share` of the way through the ages
    /// `ages` gives its full track.
    fn star_through(mass: f64, share: f64, ages: impl Fn(&Track) -> (f64, f64)) -> StarModel {
        let track = Track::full(
            SolarMasses::new(mass),
            &Composition::SOLAR,
            &StarDraws::median(),
        );
        let (start, end) = ages(&track);
        star(mass, start + share * (end - start))
    }

    /// The representative star of each kind: a saturated M dwarf, a Class I protostar, a young
    /// Crab-like pulsar, a Vela-aged one, a magnetar, a luminous blue variable half-way through its
    /// window, and a star half-way through its thermally pulsing AGB.
    fn kinds() -> Vec<(&'static str, StarModel)> {
        vec![
            ("flaring dwarf", star(0.2, 3e8)),
            ("protostar", star(1.0, 2e5)),
            ("young pulsar", neutron_star(12.6, 0.03, 1e3)),
            ("Vela-aged pulsar", neutron_star(12.5, 0.06, 2e4)),
            ("magnetar", neutron_star(15.0, 0.2, 2e3)),
            (
                "luminous blue variable",
                star_through(60.0, 0.5, |t| {
                    let w = t.window_where(PhasePredicate::Lbv).unwrap();
                    (w.start().value(), w.end().value())
                }),
            ),
            (
                "pulsing AGB star",
                star_through(3.0, 0.5, |t| {
                    let w = t.pulsing_agb().unwrap();
                    (w.start().value(), w.end().value())
                }),
            ),
        ]
    }

    /// Half the span, years, over which the tests list a representative star's events: a day for
    /// the dwarf's flares, some eight a day; two interpulse periods for the pulsing AGB star; and
    /// five centuries for the rest.
    fn half_span(name: &str) -> f64 {
        match name {
            "flaring dwarf" => 1.0 / 365.25,
            "pulsing AGB star" => 5e4,
            _ => 500.0,
        }
    }

    fn years(y: f64) -> UniverseTime {
        UniverseTime::EPOCH
            .checked_add(Span::from_seconds_f64(y * SECONDS_PER_JULIAN_YEAR).unwrap())
            .unwrap()
    }

    fn window(from: f64, to: f64) -> TimeWindow {
        TimeWindow::new(years(from), years(to)).unwrap()
    }

    /// The events of every kind its module lists for `star` on `body` over `window`, as
    /// [`StarEvent`]s in the assembly's order: what the assembly must give.
    fn by_module(star: &StarModel, body: BodyId, window: TimeWindow) -> Vec<StarEvent> {
        let series = |tag| EventSeries::new(SEED, tag, body.into());
        let mut all = Vec::new();
        let mut flares = Vec::new();
        flares::flares_in(star, &series(tags::STAR_FLARE), window, &mut flares);
        all.extend(flares.iter().map(StarEvent::of_flare));
        let (mut small, mut large) = (Vec::new(), Vec::new());
        glitches::glitches_in(star, &series(tags::STAR_GLITCH), window, &mut small);
        glitches::large_glitches_in(star, SEED, body, window, &mut large);
        all.extend(small.iter().map(StarEvent::of_glitch));
        all.extend(large.iter().map(StarEvent::of_large_glitch));
        let magnetar_series = MagnetarSeries::new(SEED, body);
        let (mut bursts, mut giant) = (Vec::new(), Vec::new());
        magnetar::bursts_in(star, &magnetar_series, window, &mut bursts);
        magnetar::giant_flares_in(star, &magnetar_series, window, &mut giant);
        all.extend(bursts.iter().map(StarEvent::of_burst));
        all.extend(giant.iter().map(StarEvent::of_giant_flare));
        let mut outbursts = Vec::new();
        fu_orionis::outbursts_in(star, &series(tags::STAR_FU_ORIONIS), window, &mut outbursts);
        all.extend(outbursts.iter().map(StarEvent::of_outburst));
        let mut eruptions = Vec::new();
        lbv::eruptions_in(
            star,
            &series(tags::STAR_LBV_ERUPTION),
            window,
            &mut eruptions,
        );
        all.extend(eruptions.iter().map(StarEvent::of_eruption));
        let mut pulses = Vec::new();
        thermal_pulse::thermal_pulses_in(
            star,
            &series(tags::STAR_THERMAL_PULSE),
            window,
            &mut pulses,
        );
        all.extend(pulses.iter().map(StarEvent::of_pulse));
        all.sort_by(StarEvent::order);
        all
    }

    /// The assembly lists exactly what each kind's module lists, over a window long enough to
    /// hold several of each kind's events, on eight bodies of each representative star; every kind
    /// appears.
    #[test]
    fn each_kind_s_listing_is_its_module_s() {
        let mut seen = std::collections::BTreeSet::new();
        for (name, star) in kinds() {
            let window = window(-half_span(name), half_span(name));
            for index in 0..8 {
                let mut listed = Vec::new();
                events_in(&star, SEED, body(index), window, &mut listed);
                assert_eq!(listed, by_module(&star, body(index), window), "{name}");
                seen.extend(listed.iter().map(StarEvent::kind));
            }
        }
        assert_eq!(seen.len(), 7, "{seen:?}");
    }

    /// Order independence (the plan's test through the public functions): a listing over a window
    /// is the union of the listings over any partition of it, and what runs at a time is the same
    /// whatever was asked before, forwards or backwards.
    #[test]
    fn the_listings_are_the_same_in_any_order() {
        for (name, star) in kinds() {
            let h = half_span(name);
            let whole = window(-h, h);
            let cuts = [-0.6, -0.005, 0.0, 0.333, 0.99].map(|f| years(f * h));
            assert_partition_independent(whole, &cuts, |w| {
                let mut out = Vec::new();
                events_in(&star, SEED, body(3), w, &mut out);
                out
            });
            let times: Vec<f64> = (0..40).map(|k| -1_000.0 + 50.0 * f64::from(k)).collect();
            let events = StarEvents::of(&star, SEED, body(3));
            let state = |t| star.state_at(t).unwrap();
            assert_order_independent(&times, |&y| {
                let t = years(y);
                events.at(t, &state(t))
            });
            let _ = name;
        }
    }

    /// The plan's test: a star evaluated cold at t = +900 years equals one evaluated after 100
    /// earlier times; no function in `stellar` takes a list of past events.
    #[test]
    fn a_star_evaluated_cold_at_900_years_equals_one_evaluated_after_100_earlier_times() {
        for (name, star) in kinds() {
            let late = years(900.0);
            let cold = StarEvents::of(&star, SEED, body(5)).at(late, &star.state_at(late).unwrap());
            let warm = StarEvents::of(&star, SEED, body(5));
            let h = half_span(name);
            for k in 0..100 {
                let t = years(-1_000.0 + 19.0 * f64::from(k));
                let _ = warm.at(t, &star.state_at(t).unwrap());
                let mut listed = Vec::new();
                warm.events_in(
                    TimeWindow::new(t, years(-1_000.0 + 19.0 * f64::from(k) + h)).unwrap(),
                    &mut listed,
                );
            }
            assert_eq!(cold, warm.at(late, &star.state_at(late).unwrap()), "{name}");
        }
    }

    /// What runs at a time is what the window's listing holds running then, each event for its
    /// listed duration: `active_at` and `at` list it, a third of the way into events of every kind
    /// and between them, on eight bodies of each representative star, from a window that reaches
    /// back past every kind's longest event (two half spans: a day, 10⁵ years, a millennium);
    /// `active_at` keeps what `out` held.
    #[test]
    fn what_runs_at_a_time_is_what_the_window_s_listing_holds_running() {
        for (name, star) in kinds() {
            let h = half_span(name);
            let mut running = 0;
            for index in 0..8 {
                let mut whole = Vec::new();
                events_in(&star, SEED, body(index), window(-3.0 * h, h), &mut whole);
                let inside = whole
                    .iter()
                    .filter(|e| e.onset() >= years(-h))
                    .map(|e| {
                        let third = Span::from_seconds_f64(e.duration().as_seconds_f64() / 3.0);
                        e.onset().checked_add(third.unwrap()).unwrap()
                    })
                    .filter(|&t| t < years(h))
                    .take(4);
                let between = [-1.0, 0.0, 0.333].map(|f| years(f * h));
                let events = StarEvents::of(&star, SEED, body(index));
                let prefix = &whole[..whole.len().min(1)];
                for t in inside.chain(between) {
                    let expected: Vec<StarEvent> = whole
                        .iter()
                        .filter(|e| e.is_active_at(t))
                        .copied()
                        .collect();
                    running += expected.len();
                    let mut listed = prefix.to_vec();
                    active_at(&star, SEED, body(index), t, &mut listed);
                    assert_eq!(&listed[..prefix.len()], prefix, "{name}: what out held");
                    assert_eq!(&listed[prefix.len()..], &expected[..], "{name} at {t:?}");
                    let (at, _) = events.at(t, &star.state_at(t).unwrap());
                    assert_eq!(at, expected, "{name} at {t:?}");
                }
            }
            assert!(running > 0, "{name}: nothing runs at the sampled times");
        }
    }

    /// `bound` is never exceeded (the plan's sweep across each kind's parameter range): the rates of
    /// the Poisson kinds built through the assembly, flares, FU Orionis outbursts, Crab-like
    /// glitches and giant eruptions, stay at or below their bins' bounds at 40 times in each of 200
    /// bins, over masses and ages that span each kind. The magnetar's rates are its module's own
    /// (`magnetar.rs` sweeps them), and thermal pulses and Vela-like glitches have no rate.
    #[test]
    fn no_rate_passes_its_bin_s_bound() {
        fn sweep(name: &str, model: &impl RateModel, bin_seconds: i64, from: UniverseTime) {
            let bins = PoissonBins::new(bin_seconds, 1).unwrap();
            let first = bins.bin_of(from).unwrap();
            for b in 0..200 {
                let bin = bins
                    .bin_window(crate::events::EventBin::new(first.get() + b).unwrap())
                    .unwrap();
                let bound = model.bound(bin).value();
                let span = bin
                    .end()
                    .checked_since(bin.start())
                    .unwrap()
                    .as_seconds_f64();
                for k in 0..40 {
                    let t = bin
                        .start()
                        .checked_add(Span::from_seconds_f64(span * f64::from(k) / 40.0).unwrap())
                        .unwrap();
                    let rate = model.rate(t).value();
                    assert!(rate <= bound, "{name}: {rate} over {bound} at {t}");
                }
            }
        }
        let mut swept = [0_u32; 4];
        for (mass, age) in [
            (0.1, 1e8),
            (0.3, 2e9),
            (0.8, 4e9),
            (1.0, 1e5),
            (1.5, 3e6),
            (4.0, 1e5),
        ] {
            let s = star(mass, age);
            let events = StarEvents::of(&s, SEED, body(0));
            if let Some((model, _)) = &events.flares {
                sweep("flares", model, flares::BIN_SECONDS, years(-1_000.0));
                swept[0] += 1;
            }
            if let Some((model, _)) = &events.outbursts {
                sweep(
                    "outbursts",
                    model,
                    fu_orionis::BIN_SECONDS,
                    model.formation(),
                );
                swept[1] += 1;
            }
        }
        for (log_b0, period, age) in [
            (12.0, 0.1, 1e3),
            (12.6, 0.03, 1e3),
            (12.5, 0.06, 2e4),
            (13.0, 0.3, 1e6),
        ] {
            let s = neutron_star(log_b0, period, age);
            if let Some((model, _)) = &StarEvents::of(&s, SEED, body(0)).glitches {
                sweep("glitches", model, glitches::BIN_SECONDS, years(-1_000.0));
                swept[2] += 1;
            }
        }
        // From 26 a solar star under 55 M☉ has no window (P06.T42.b's enforced limit).
        for mass in [55.0, 60.0, 80.0] {
            let s = star_through(mass, 0.5, |t| {
                let w = t.window_where(PhasePredicate::Lbv).unwrap();
                (w.start().value(), w.end().value())
            });
            if let Some((model, _)) = &StarEvents::of(&s, SEED, body(0)).lbv {
                sweep("eruptions", model, lbv::BIN_SECONDS, model.start());
                swept[3] += 1;
            }
        }
        assert!(swept.iter().all(|&n| n > 0), "{swept:?}");
    }

    /// ∫ `rate` dt over `window`, by the midpoint rule on 2,000 steps: events.
    fn integral(window: TimeWindow, rate: impl Fn(UniverseTime) -> f64) -> f64 {
        let span = window
            .end()
            .checked_since(window.start())
            .unwrap()
            .as_seconds_f64();
        let step = span / 2_000.0;
        (0..2_000)
            .map(|k| {
                let t = window
                    .start()
                    .checked_add(Span::from_seconds_f64(step * (f64::from(k) + 0.5)).unwrap())
                    .unwrap();
                rate(t) * step
            })
            .sum()
    }

    /// What 10⁴ bodies of one star list over one window ([`counted`]).
    struct Counted<'a> {
        /// The star's kinds on body 0, whose models every body shares.
        base: StarEvents<'a>,
        /// The events `events_in` lists, by kind, over every body.
        counts: std::collections::BTreeMap<StarEventKind, u64>,
        /// The bursts the magnetar's own listing gives, over every body.
        bursts: u64,
        /// The Vela-like cycles in the window, over the bodies that are Vela-like.
        cycles: f64,
    }

    /// The bodies a mean count is taken over.
    const BODIES: u16 = 10_000;

    /// [`events_in`] over `window` on [`BODIES`] bodies of `star`.
    fn counted(star: &StarModel, window: TimeWindow) -> Counted<'_> {
        let base = StarEvents::of(star, SEED, body(0));
        let years = window
            .end()
            .checked_since(window.start())
            .unwrap()
            .as_seconds_f64();
        let mut counted = Counted {
            base,
            counts: std::collections::BTreeMap::new(),
            bursts: 0,
            cycles: 0.0,
        };
        let mut listed = Vec::new();
        for i in 0..BODIES {
            let events = counted.base.on_body(star, SEED, body(i));
            listed.clear();
            events.events_in(window, &mut listed);
            for event in &listed {
                *counted.counts.entry(event.kind()).or_default() += 1;
            }
            if let Some((model, series)) = &events.magnetar {
                let mut own = Vec::new();
                model.bursts_in(series, window, &mut own);
                counted.bursts += u64::try_from(own.len()).unwrap();
            }
            if let Some(cycle) = &events.vela {
                counted.cycles += years / cycle.period().as_seconds_f64();
            }
        }
        counted
    }

    impl Counted<'_> {
        fn count(&self, kind: StarEventKind) -> u64 {
            self.counts.get(&kind).copied().unwrap_or(0)
        }

        /// Asserts that the `kind` events number `n` × ∫ `rate` dt over `window` within a Poisson
        /// interval.
        fn poisson(
            &self,
            what: &str,
            kind: StarEventKind,
            window: TimeWindow,
            rate: impl Fn(UniverseTime) -> f64,
        ) {
            let expected = f64::from(BODIES) * integral(window, rate);
            eprintln!(
                "{what}: {} against {expected:.1}, interval {:?}",
                self.count(kind),
                poisson_interval(expected, ALPHA)
            );
            assert_poisson_count(what, self.count(kind), expected, ALPHA);
        }
    }

    /// The plan's statistical test, through the assembly: per kind, the events [`events_in`] lists
    /// on 10⁴ bodies of the kind's representative star number the rate integral within a Poisson
    /// interval (flares in a day, Crab-like glitches in a century, giant flares in a year, FU
    /// Orionis outbursts and giant eruptions in a millennium); a magnetar's bursts, which come in
    /// episodes and whose own integral `magnetar.rs` tests, number their module's listing exactly;
    /// and thermal pulses and Vela-like glitches, on monotone phases, number the window's cycles
    /// within one a body on the mean, a count of whole cycles rather than a Poisson one.
    #[test]
    #[ignore = "slow: 10⁴ bodies of each of seven stars"]
    fn the_mean_count_over_10_000_stars_matches_the_rate_integral() {
        let n = f64::from(BODIES);
        let mut checked = std::collections::BTreeSet::new();
        for (name, star) in kinds() {
            let span = match name {
                "flaring dwarf" => 1.0 / 365.25,
                "young pulsar" | "Vela-aged pulsar" => 100.0,
                "magnetar" => 1.0,
                "pulsing AGB star" => 2e5,
                _ => 1_000.0,
            };
            let window = window(0.0, span);
            let c = counted(&star, window);
            let tagged = |what: &str| format!("{name}: {what}");
            if let Some((model, _)) = &c.base.flares {
                c.poisson(&tagged("flares"), StarEventKind::Flare, window, |t| {
                    model.rate(t).value()
                });
            }
            if let Some((model, _)) = &c.base.outbursts {
                c.poisson(
                    &tagged("outbursts"),
                    StarEventKind::FuOrionisOutburst,
                    window,
                    |t| model.rate(t).value(),
                );
            }
            if let Some((model, _)) = &c.base.lbv {
                c.poisson(
                    &tagged("eruptions"),
                    StarEventKind::GiantEruption,
                    window,
                    |t| model.rate(t).value(),
                );
            }
            if let Some((model, _)) = &c.base.magnetar {
                c.poisson(
                    &tagged("giant flares"),
                    StarEventKind::MagnetarGiantFlare,
                    window,
                    |t| model.giant_flare_rate_at(t).value(),
                );
                assert_eq!(c.count(StarEventKind::MagnetarBurst), c.bursts, "{name}");
            }
            #[expect(clippy::cast_precision_loss, reason = "counts under 2⁵³")]
            match name {
                "young pulsar" => {
                    let (model, _) = c.base.glitches.as_ref().unwrap();
                    c.poisson(
                        &tagged("Crab-like glitches"),
                        StarEventKind::Glitch,
                        window,
                        |t| model.rate(t).value(),
                    );
                }
                "Vela-aged pulsar" => {
                    // The Crab-like glitches of the same star come on top of the cycles.
                    let (model, _) = c.base.glitches.as_ref().unwrap();
                    let small = n * integral(window, |t| model.rate(t).value());
                    let large = c.count(StarEventKind::Glitch) as f64 - small;
                    eprintln!(
                        "{name}: {large:.0} Vela-like glitches against {:.1}",
                        c.cycles
                    );
                    assert!(c.cycles > 1e4, "{}", c.cycles);
                    assert!((large - c.cycles).abs() < n + 5.0 * small.sqrt(), "{large}");
                }
                "pulsing AGB star" => {
                    let track = star.track().unwrap();
                    let phase = |t: UniverseTime| {
                        track.pulse_phase_at(star.age_at(t).value(), 0.0).unwrap()
                    };
                    let expected = phase(window.end()) - phase(window.start());
                    let mean = c.count(StarEventKind::ThermalPulse) as f64 / n;
                    eprintln!("{name}: {mean:.3} pulses a body against {expected:.3} cycles");
                    assert!((mean - expected).abs() < 1.0, "{mean} against {expected}");
                }
                _ => {}
            }
            checked.extend(c.counts.keys().copied());
        }
        assert_eq!(checked.len(), 7, "{checked:?}");
    }

    /// The first event of `kind` that [`events_in`] lists for `star` over `window` on bodies 0,
    /// 1, …, with its body's events.
    fn first_of(
        star: &StarModel,
        kind: StarEventKind,
        window: TimeWindow,
    ) -> (StarEvents<'_>, StarEvent) {
        (0..u16::MAX)
            .find_map(|i| {
                let mut listed = Vec::new();
                let events = StarEvents::of(star, SEED, body(i));
                events.events_in(window, &mut listed);
                listed
                    .into_iter()
                    .find(|e| e.kind() == kind)
                    .map(|e| (events, e))
            })
            .expect("an event of the kind on some body")
    }

    /// `event`'s onset plus `share` of its duration.
    fn into(event: &StarEvent, share: f64) -> UniverseTime {
        let span = event.duration().as_seconds_f64() * share;
        event
            .onset()
            .checked_add(Span::from_seconds_f64(span).unwrap())
            .unwrap()
    }

    /// The light the active events make, by the kinds' records' rules (P06.T28.a and T28.f's
    /// leans): a thermal pulse's L × F with R × √F, so that `T_eff` holds; a flare's excess
    /// against the epoch's luminosity added to L(t).
    #[test]
    fn a_pulse_and_a_flare_light_the_star_by_their_kinds_rules() {
        let stars = kinds();
        let get = |name: &str| &stars.iter().find(|(n, _)| *n == name).unwrap().1;
        // A thermal pulse, a fifth of the way through its transient.
        let agb = get("pulsing AGB star");
        let (events, pulse) = first_of(agb, StarEventKind::ThermalPulse, window(-5e4, 5e4));
        let t = into(&pulse, 0.2);
        let state = agb.state_at(t).unwrap();
        let (active, effects) = events.at(t, &state);
        assert!(active.contains(&pulse), "{active:?}");
        let effects = effects.unwrap();
        let factor = events.pulses().unwrap().luminosity_factor_at(t);
        assert!((factor - 1.0).abs() > 1e-3, "{factor}");
        let (l, r) = (state.luminosity().value(), state.radius().value());
        let relative = |a: f64, b: f64| (a / b - 1.0).abs();
        assert!(relative(effects.luminosity().value(), l * factor) < 1e-12);
        assert!(relative(effects.radius().value(), r * factor.sqrt()) < 1e-12);
        let flux = |l: f64, r: f64| l / (r * r);
        assert!(
            relative(
                flux(effects.luminosity().value(), effects.radius().value()),
                flux(l, r)
            ) < 1e-12
        );
        // A flare at a third of its span.
        let dwarf = get("flaring dwarf");
        let (events, flare) = first_of(dwarf, StarEventKind::Flare, window(0.0, 1.0 / 365.25));
        let t = into(&flare, 1.0 / 3.0);
        let state = dwarf.state_at(t).unwrap();
        let (model, series) = events.flares.as_ref().unwrap();
        let mut running = Vec::new();
        model.flares_active_at(series, t, &mut running);
        let excess: f64 = running
            .iter()
            .map(|f| (f.luminosity_factor_at(t) - 1.0) * model.quiescent_luminosity().value())
            .sum();
        let effects = events.at(t, &state).1.unwrap();
        assert!(excess > 0.0);
        assert!(
            relative(
                effects.luminosity().value(),
                state.luminosity().value() + excess
            ) < 1e-12
        );
        assert_same_bits(effects.radius().value(), state.radius().value());
    }

    /// The light of a giant eruption and of a magnetar (P06.T28.c and T28.e's leans): an
    /// eruption at its peak passes the Eddington cap; a magnetar's burst is listed with its own
    /// luminosity and not added to the star's.
    #[test]
    fn an_eruption_passes_the_cap_and_a_burst_is_only_listed() {
        let stars = kinds();
        let get = |name: &str| &stars.iter().find(|(n, _)| *n == name).unwrap().1;
        let lbv = get("luminous blue variable");
        let (events, eruption) = first_of(lbv, StarEventKind::GiantEruption, window(-1e3, 1e3));
        let t = into(&eruption, lbv::RISE_SHARE);
        let state = lbv.state_at(t).unwrap();
        let x = 0.76 - 3.0 * Composition::SOLAR.z_fit().value();
        let edd = state.luminosity().value()
            / (wind::eddington_factor(state.luminosity(), state.mass()) * (1.0 + x));
        let effects = events.at(t, &state).1.unwrap();
        assert!(
            effects.luminosity().value() > edd,
            "{effects:?} against {edd}"
        );
        // A magnetar's burst, half-way through.
        let magnetar = get("magnetar");
        let (events, burst) = first_of(magnetar, StarEventKind::MagnetarBurst, window(0.0, 1.0));
        let t = into(&burst, 0.5);
        let state = magnetar.state_at(t).unwrap();
        let (active, effects) = events.at(t, &state);
        let listed = active.iter().find(|e| e.id() == burst.id()).unwrap();
        assert!(listed.luminosity_watts().is_some_and(|w| w > 0.0));
        if let Some(effects) = effects {
            assert_same_bits(effects.luminosity().value(), state.luminosity().value());
        }
    }

    /// A Vela-like glitch (P06.T28.b's rule left to T28.g): listed for its recovery time `τ_d`,
    /// and the pulsar's spin offset its module's own, to the bit.
    #[test]
    fn a_vela_like_glitch_runs_for_its_recovery_time() {
        let stars = kinds();
        let vela = &stars
            .iter()
            .find(|(n, _)| *n == "Vela-aged pulsar")
            .unwrap()
            .1;
        let (events, glitch) = (0..u16::MAX)
            .find_map(|i| {
                let events = StarEvents::of(vela, SEED, body(i));
                let mut large = Vec::new();
                events
                    .vela
                    .as_ref()?
                    .large_glitches_in(window(-500.0, 500.0), &mut large);
                large
                    .first()
                    .map(|g| (events, StarEvent::of_large_glitch(g)))
            })
            .unwrap();
        let tau = glitches::LARGE_GLITCH_DURATION;
        let just = |t: UniverseTime| events.at(t, &vela.state_at(t).unwrap()).0;
        let before_end = glitch
            .onset()
            .checked_add(tau)
            .unwrap()
            .checked_sub(Span::from_seconds(1))
            .unwrap();
        assert!(just(before_end).contains(&glitch));
        assert!(!just(glitch.onset().checked_add(tau).unwrap()).contains(&glitch));
        let found = (0..u16::MAX)
            .find(|&i| {
                StarEvents::of(vela, SEED, body(i))
                    .vela
                    .as_ref()
                    .is_some_and(|c| c.series() == events.vela.as_ref().unwrap().series())
            })
            .unwrap();
        let t = into(&glitch, 0.5);
        let effects = events.at(t, &vela.state_at(t).unwrap()).1.unwrap();
        assert_same_bits(
            effects.spin_frequency_offset_hz(),
            glitches::frequency_offset_hz_at(vela, SEED, body(found), t),
        );
    }

    /// The cap (P06.T28.a and T28.d's lean): over each representative star at 2,000 times across
    /// the clock window, the composed light never passes the Eddington luminosity, or the track's
    /// own where that is above it, but by a giant eruption's excess; and it is positive.
    #[test]
    fn the_composed_light_is_held_at_the_eddington_luminosity() {
        let mut lit = std::collections::BTreeMap::new();
        for (name, star) in kinds() {
            let events = StarEvents::of(&star, SEED, body(7));
            for k in 0..2_000 {
                let t = years(-1_000.0 + f64::from(k));
                let state = star.state_at(t).unwrap();
                let (active, effects) = events.at(t, &state);
                let Some(effects) = effects else { continue };
                *lit.entry(name).or_insert(0_u32) += 1;
                let l = state.luminosity().value();
                assert!(effects.luminosity().value() > 0.0, "{name}");
                if active
                    .iter()
                    .any(|e| e.kind() == StarEventKind::GiantEruption)
                {
                    continue;
                }
                let cap = (l / surface_eddington_factor(&state, star.composition())).max(l);
                assert!(
                    effects.luminosity().value() <= cap * (1.0 + 1e-12),
                    "{name} at {t}"
                );
            }
        }
        for name in ["flaring dwarf", "young pulsar", "pulsing AGB star"] {
            assert!(lit.get(name).is_some_and(|&n| n > 0), "{name}: {lit:?}");
        }
    }

    /// A Vela-like glitch's listed span is the Crab-like recovery time, to the bit.
    #[test]
    fn a_vela_like_glitch_is_listed_for_the_recovery_time() {
        assert_same_bits(
            glitches::LARGE_GLITCH_DURATION.as_seconds_f64(),
            glitches::RECOVERY_TIME_DAYS * crate::units::consts::SECONDS_PER_DAY,
        );
    }

    /// The digest (`f64_digest`) of the assembly's output for the representative stars: each one's
    /// listing on body 1 over its span (kind, onset since the epoch and duration, s), and the light
    /// its first event makes a third of the way through, and the first two of its events that run
    /// together a second into the later's (L, R, `M_V`, or −99 for none, and the spin offset).
    /// Blessed natively on x86-64, and re-blessed at the 26 bump, from which a star's summary
    /// carries its events (P06.T28.g): it pins the composition's form and its sum's order.
    const ASSEMBLY_DIGEST: u64 = 0xaf56_dd98_16da_6aa9;

    /// The assembly's listing order and composed light are pinned by their bits
    /// ([`ASSEMBLY_DIGEST`]): a rewrite of the sum's order or form changes them.
    #[test]
    fn the_assembly_s_output_is_pinned() {
        let mut values = Vec::new();
        let mut overlaps = Vec::new();
        for (name, star) in kinds() {
            let events = StarEvents::of(&star, SEED, body(1));
            let mut listed = Vec::new();
            events.events_in(window(-half_span(name), half_span(name)), &mut listed);
            for event in &listed {
                values.push(f64::from(event.kind() as u8));
                values.push(event.onset().since_epoch().as_seconds_f64());
                values.push(event.duration().as_seconds_f64());
            }
            let together = listed.windows(2).find_map(|pair| {
                let t = pair[1].onset().checked_add(Span::from_seconds(1))?;
                (pair[0].is_active_at(t) && pair[1].is_active_at(t)).then_some(t)
            });
            if together.is_some() {
                overlaps.push(name);
            }
            let times = listed.first().map(|first| into(first, 1.0 / 3.0));
            for t in times.into_iter().chain(together) {
                if let Some(effects) = events.at(t, &star.state_at(t).unwrap()).1 {
                    values.extend([
                        effects.luminosity().value(),
                        effects.radius().value(),
                        effects
                            .absolute_magnitude_v()
                            .map_or(-99.0, Magnitudes::value),
                        effects.spin_frequency_offset_hz(),
                    ]);
                }
            }
        }
        // Two flares, and two recovering glitches, run together.
        assert!(
            overlaps.contains(&"flaring dwarf") && overlaps.contains(&"young pulsar"),
            "{overlaps:?}"
        );
        let digest = hyperion_testkit::golden::f64_digest(&values);
        assert_eq!(
            digest,
            ASSEMBLY_DIGEST,
            "{digest:#018x} over {} values",
            values.len()
        );
    }
}
