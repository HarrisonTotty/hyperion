//! One star's evolution as a fixed grid of phase segments (plan 06, P06.T10.c–e; design notes 1–4,
//! 8, 9 and 19).
//!
//! The formulae of Hurley, Pols and Tout (2000, MNRAS 315, 543; "HPT") are closed forms in mass
//! and age only while the mass is constant. A [`Track`] integrates the wind on a grid that never
//! reads the age asked for: each phase is a segment, and a segment that loses mass carries knots at
//! fixed values of a coordinate that runs from 0 to 1 over the phase, found by a fixed number of
//! midpoint steps between knots. [`Track::state_at`] interpolates the knots' masses and evaluates the
//! phase's closed forms, so the state is a continuous function of age and the same whatever was
//! asked before.
//!
//! # The coordinate of each phase
//!
//! - The main sequence and the helium main sequence run in HPT's fractional age τ, which their
//!   effective-age rule keeps as the mass changes (HPT section 7.1): the initial mass is the
//!   current one, and the wind is integrated as dM ÷ dτ = −Ṁ `t_MS`(M), on 16 knots even in τ.
//! - From the Hertzsprung gap on, the initial mass is the one the phase was entered with, so the
//!   phase's clock runs with time over a span fixed at entry. The knots sit at fixed values of u =
//!   1 − (1 − x)(1 − y), with x the phase's progress and y the share of its entry envelope lost:
//!   u runs from 0 to 1 whatever the mass does, and follows whichever of the two advances faster.
//!   x is the fraction of the span in the Hertzsprung gap, core helium burning and the thermally
//!   pulsing AGB, and the progress of the core mass on the giant branches (the first giant
//!   branch, the early AGB and the helium giants), where the luminosity diverges as a power of the
//!   time left. The knots are clustered towards u = 1 as 1 − (1 − k ÷ (n − 1))³, 16 of them, and
//!   32 on the thermally pulsing AGB. Knots at fixed fractions of the phase's time, as plan 06's
//!   design note 1 first had them, stall on massive stars' luminous-blue-variable bursts, which
//!   strip an envelope in a small part of the phase, and at the tip of the giant branch.
//! - Each interval between knots is [`STEPS_PER_KNOT`](build::STEPS_PER_KNOT) midpoint steps in
//!   u, started from the state reached at the knot, so no error in u builds up; an interval that
//!   u cannot cross (past the grid, or where it stalls) is integrated in age instead.
//!
//! HPT section 7.1 also ask that the initial mass follow the current one in the Hertzsprung gap.
//! The track keeps the mass the main sequence ended with instead, since the gap's closed forms
//! would otherwise be rebuilt at every step of the integration; the effect on any age is under
//! 10⁻⁴ of it (ruling 40 of 2026-09-22).
//!
//! A phase that its wind ends early, by the loss of the envelope, ends at the root of the envelope
//! mass on the knot interpolant, found by 40 bisections. A phase whose wind would remove under 10⁻⁶
//! of its mass has no knots and costs nothing to integrate.
//!
//! # Junctions
//!
//! Every junction between phases is continuous except a death that explodes or collapses (design
//! note 3). The helium flash is bridged over 10⁴ years ([`Bridges::Physical`]). Where HPT's
//! formulae themselves step, which happens only through the small-envelope perturbation of HPT
//! section 6.3 (the core's appearance at the start of the Hertzsprung gap of massive stars, the
//! core's fall at the second dredge-up, and an early-AGB star losing its envelope while its
//! remnant is still passing from the helium main sequence to the helium giants), the step in log L
//! and log R is carried as an offset that decays to zero over the first 2% of the new phase. Until
//! P06.T16's post-AGB bridge the end of the AGB hands over to the white dwarf directly. HPT's
//! perturbation makes that continuous for carbon–oxygen white dwarfs and leaves a step for
//! oxygen–neon ones, whose cooling law reads a heavier nucleus, and a helium star below 0.689 M☉
//! steps where it becomes a white dwarf, since the dwarf keeps the helium the star did not burn
//! while the perturbation approaches the white dwarf of its core (see `phases.rs`,
//! `helium_star_end`). Under [`RemnantRecipe::Hurley2000`] both steps stand, in luminosity and
//! radius, as in SSE. Under the default the white dwarf's cooling law is matched to the star's
//! last luminosity (P06.T20.a, `white_dwarf::cooling_origin`), so the luminosity is continuous at
//! every such hand-over and only the radius steps. Every step is at the star's death, which the
//! continuity tests exclude.
//!
//! # Remnants
//!
//! Under [`RemnantRecipe::Hurley2000`] a white dwarf cools by HPT's equation 90, a neutron star by
//! equation 93, and a supernova leaves HPT's remnant (equation 92), as in the published SSE code.
//! Under the default, [`RemnantRecipe::MandelMuller2020`] (P06.T18.d), an iron core's collapse
//! leaves the neutron star or black hole of Mandel and Müller (2020) that the star's remnant draws
//! decide, a star inside the single-star electron-capture window leaves their 1.26 M☉ neutron star,
//! and a white dwarf cools by the Montreal sequences' law (P06.T20.a, ruling 57.2). A neutron
//! star cools by HPT's equation 93 under both until P06.T21 (ruling 33). A black hole's
//! luminosity is exactly zero, so no consumer may take its logarithm unguarded (ruling 40). See `phases.rs`,
//! `iron_core_fate`, for the collapse, and `model.rs` for the remnants' states.

mod binary;
mod build;
mod interp;
mod model;
mod phases;

use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::remnant::{CompactRemnant, Death, RemnantRecipe, SupernovaType};
use crate::stellar::{Composition, Phase, StarState, StarStateParts};
use crate::units::{
    Megayears, SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years,
};

use super::coeffs::ZCoeffs;
use super::wind::{self, ReimersEta, WindRecipe};

// Plan 11's hooks: the helium star of ruling 34.1 and a star whose mass a companion sets.
pub(crate) use binary::{
    CORE_GYRATION, ConvectiveEnvelope, ENVELOPE_GYRATION, NewStar, Remains, Structure,
    giant_radius_exponent, lightest_helium_star, main_sequence_lifetime, main_sequence_structure,
    new_star_mass,
};
use build::Builder;
pub(crate) use build::Resolution;
use interp::Interval;
use model::{Model, Physics};

/// The lowest initial mass a [`Track`] covers, M☉: HPT's formulae start at 0.1 M☉. Below it an
/// object is substellar, and [`evolve`](super::evolve) hands it to P06.T13's cooling fits.
pub const MIN_INITIAL_MASS: SolarMasses = SolarMasses::new(0.1);

/// The highest initial mass a [`Track`] covers, M☉: HPT's formulae end at 100 M☉, and P06.T14
/// extends them to 150.
pub const MAX_INITIAL_MASS: SolarMasses = SolarMasses::new(100.0);

/// Reimers' η for a star whose draw is the standard normal `z`: 0.5 + 0.07 z, held non-negative
/// (plan 06, design note 7).
///
/// The mean is HPT's η (section 7.1), and σ = 0.07 is the spread McDonald and Zijlstra (2015,
/// MNRAS 448, 502) measure across globular clusters, whose own mean is 0.477 ± 0.070. η falls below
/// zero only 7.1σ below the mean, for about one star in 10¹²; such a star has no Reimers wind.
#[must_use]
pub(crate) fn reimers_eta(z: f64) -> ReimersEta {
    let eta = 0.5 + 0.07 * z;
    ReimersEta::new(if eta > 0.0 { eta } else { 0.0 })
}

/// Whether the helium flash is bridged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Bridges {
    /// The generator's tracks: the flash takes 10⁴ years (plan 06, design note 3).
    #[default]
    Physical,
    /// The flash is instantaneous, as in HPT and the published SSE code; for validation against
    /// SSE (P06.T12.b). A track built so has a step at the flash.
    Instant,
}

/// The choices a track is built with: the wind and remnant recipes and the bridges.
///
/// The default is the generator's: [`WindRecipe::Modern`], [`RemnantRecipe::MandelMuller2020`]
/// and [`Bridges::Physical`]. [`TrackOptions::hurley2000`] is the published SSE code's, for the
/// validation of P06.T12.b.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::sse::{Bridges, RemnantRecipe, TrackOptions, WindRecipe};
///
/// let sse = TrackOptions::hurley2000();
/// assert_eq!(sse.wind(), WindRecipe::Hurley2000);
/// assert_eq!(sse.remnant(), RemnantRecipe::Hurley2000);
/// assert_eq!(sse.bridges(), Bridges::Instant);
/// assert_eq!(TrackOptions::default().with_bridges(Bridges::Instant).wind(), WindRecipe::Modern);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct TrackOptions {
    wind: WindRecipe,
    remnant: RemnantRecipe,
    bridges: Bridges,
}

impl TrackOptions {
    /// The recipes of HPT and the published SSE code, with no bridges.
    #[must_use]
    pub const fn hurley2000() -> Self {
        Self {
            wind: WindRecipe::Hurley2000,
            remnant: RemnantRecipe::Hurley2000,
            bridges: Bridges::Instant,
        }
    }

    /// These options with the wind recipe `wind`.
    #[must_use]
    pub const fn with_wind(self, wind: WindRecipe) -> Self {
        Self { wind, ..self }
    }

    /// These options with the remnant recipe `remnant`.
    #[must_use]
    pub const fn with_remnant(self, remnant: RemnantRecipe) -> Self {
        Self { remnant, ..self }
    }

    /// These options with the bridges `bridges`.
    #[must_use]
    pub const fn with_bridges(self, bridges: Bridges) -> Self {
        Self { bridges, ..self }
    }

    /// The wind recipe.
    #[must_use]
    pub const fn wind(&self) -> WindRecipe {
        self.wind
    }

    /// The remnant recipe.
    #[must_use]
    pub const fn remnant(&self) -> RemnantRecipe {
        self.remnant
    }

    /// The bridges.
    #[must_use]
    pub const fn bridges(&self) -> Bridges {
        self.bridges
    }
}

/// One star's evolution: its phases as segments on a grid fixed by `(m0, Composition, StarDraws)`
/// alone (plan 06, design note 1), built lazily up to an age or in full (design note 2).
///
/// The age is counted from the onset of collapse (design note 4); until P06.T15 adds the
/// pre-main sequence that is the zero-age main sequence.
///
/// # Examples
///
/// The Sun at 4.57 Gyr, and what it becomes:
///
/// ```
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::sse::Track;
/// use hyperion_sim::stellar::{Composition, Phase};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let sun = Track::full(SolarMasses::new(1.0), &Composition::SOLAR, &StarDraws::median());
/// let today = sun.state_at(Years::new(4.57e9));
/// assert_eq!(today.phase(), Phase::MainSequence);
/// assert!((today.luminosity().value() - 1.0).abs() < 0.05);
/// let death = sun.death().ok_or("a full track reaches the star's death")?;
/// assert!(death.age().value() > 1.1e10);
/// assert_eq!(sun.state_at(Years::new(1.3e10)).phase(), Phase::CarbonOxygenWhiteDwarf);
/// # Ok::<(), &str>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    coeffs: ZCoeffs,
    composition: Composition,
    options: TrackOptions,
    initial_mass: SolarMasses,
    eta: ReimersEta,
    segments: Vec<Segment>,
    fate: Option<Fate>,
    built_until: f64,
}

/// How the star ends: its death and the remnant it leaves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Fate {
    pub(crate) death: Death,
    pub(crate) remnant: CompactRemnant,
    /// The remnant's phase, which tells the kind of white dwarf.
    pub(crate) phase: Phase,
    /// What an iron core's collapse was built from, if the star died so: the remnant draws decide
    /// its remnant under [`RemnantRecipe::MandelMuller2020`], and [`Track::fate_with`] redraws it.
    pub(crate) iron_core: Option<IronCore>,
}

/// An iron core's collapse before the remnant draws are read: the supernova its envelopes make,
/// and the carbon–oxygen core HPT's remnant reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IronCore {
    pub(crate) supernova: SupernovaType,
    /// The carbon–oxygen core HPT's equation 92 reads under [`RemnantRecipe::Hurley2000`].
    pub(crate) co_core: SolarMasses,
}

impl Track {
    /// The star's track built until it covers `age_max`: every segment up to the one that holds
    /// that age, which are bit for bit the same segments as those of [`Track::full`] (design note
    /// 2), under the generator's options.
    ///
    /// `m0` is the initial mass, 0.1–100 M☉ ([`MIN_INITIAL_MASS`] to [`MAX_INITIAL_MASS`]);
    /// outside that range the formulae are evaluated at its nearer end.
    ///
    /// # Panics
    ///
    /// In debug builds, if `m0` lies outside the range or `age_max` is not finite and
    /// non-negative.
    #[must_use]
    pub fn to_age(m0: SolarMasses, comp: &Composition, draws: &StarDraws, age_max: Years) -> Self {
        Self::to_age_with(m0, comp, draws, TrackOptions::default(), age_max)
    }

    /// The star's whole track, to its death and the remnant, under the generator's options.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`Track::to_age`].
    #[must_use]
    pub fn full(m0: SolarMasses, comp: &Composition, draws: &StarDraws) -> Self {
        Self::full_with(m0, comp, draws, TrackOptions::default())
    }

    /// [`Track::to_age`] under `options`.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`Track::to_age`].
    #[must_use]
    pub fn to_age_with(
        m0: SolarMasses,
        comp: &Composition,
        draws: &StarDraws,
        options: TrackOptions,
        age_max: Years,
    ) -> Self {
        debug_assert!(
            age_max.value().is_finite() && age_max.value() >= 0.0,
            "a track is built to a finite, non-negative age: {age_max:?}"
        );
        Self::build(
            m0,
            comp,
            draws,
            options,
            Resolution::GENERATOR,
            Some(age_max.value().max(0.0)),
        )
    }

    /// [`Track::full`] under `options`.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`Track::to_age`].
    #[must_use]
    pub fn full_with(
        m0: SolarMasses,
        comp: &Composition,
        draws: &StarDraws,
        options: TrackOptions,
    ) -> Self {
        Self::build(m0, comp, draws, options, Resolution::GENERATOR, None)
    }

    /// Builds the track at `resolution`, to the segment holding `age_max` or, with `None`, in
    /// full.
    #[must_use]
    pub(crate) fn build(
        m0: SolarMasses,
        comp: &Composition,
        draws: &StarDraws,
        options: TrackOptions,
        resolution: Resolution,
        age_max: Option<f64>,
    ) -> Self {
        let coeffs = ZCoeffs::new(comp.z_fit());
        let m0 = checked_initial_mass(m0);
        let eta = reimers_eta(draws.eta().value());
        let outcome = Builder::new(
            &coeffs,
            comp,
            options,
            eta,
            RemnantDraws::of(draws),
            resolution,
            build::Keep::Track,
        )
        .stripped_by_companion(is_companion_stripped(draws))
        .run(m0.value(), age_max);
        Self {
            coeffs,
            composition: *comp,
            options,
            initial_mass: m0,
            eta,
            segments: outcome.segments,
            fate: outcome.fate,
            built_until: outcome.built_until,
        }
    }

    /// The star's state at `age` (years since the onset of collapse).
    ///
    /// The state is a continuous function of age except at a death that explodes or collapses
    /// (plan 06, design note 3; see the module documentation for the white dwarf's hand-over
    /// before P06.T16), and does not depend on what was asked before.
    ///
    /// # Panics
    ///
    /// In debug builds, if `age` is negative or not finite, or lies beyond [`Track::built_until`]
    /// of a track built by [`Track::to_age`]; release builds clamp it into range.
    #[must_use]
    pub fn state_at(&self, age: Years) -> StarState {
        let age = self.checked_age(age);
        let segment = self.segment_at(age);
        let evaluated = segment.evaluate(&self.physics(), age);
        self.star_state(&evaluated, age)
    }

    /// The age at which the star dies, if the build has reached it.
    #[must_use]
    pub fn lifetime(&self) -> Option<Years> {
        self.fate.map(|fate| fate.death.age())
    }

    /// The star's death, if the build has reached it: its age, its kind and the progenitor at its
    /// last living instant.
    #[must_use]
    pub fn death(&self) -> Option<Death> {
        self.fate.map(|fate| fate.death)
    }

    /// The remnant the star leaves, if the build has reached its death.
    ///
    /// A white dwarf's mass is the core mass the track ends with (plan 06, design note 9). Under
    /// [`RemnantRecipe::Hurley2000`] a supernova leaves HPT's neutron star or black hole (their
    /// equation 92). Under [`RemnantRecipe::MandelMuller2020`] an iron core leaves Mandel and
    /// Müller's (2020) neutron star or black hole, decided by the star's remnant draws
    /// ([`StarDraws::remnant_type`], [`remnant_fallback`](StarDraws::remnant_fallback) and
    /// [`remnant_mass`](StarDraws::remnant_mass)), or nothing after pair instability, and electron
    /// capture leaves their 1.26 M☉ neutron star (P06.T18.d).
    #[must_use]
    pub fn remnant(&self) -> Option<CompactRemnant> {
        self.fate.map(|fate| fate.remnant)
    }

    /// The star's death and remnant as the remnant draws `draws` would decide them on this track,
    /// if the build has reached the death: the track's own for a star that did not die by an iron
    /// core's collapse, or under [`RemnantRecipe::Hurley2000`], whose remnants read no draws.
    ///
    /// This is the remnant stage of P06.T29 (`StarModel`), which plan 08's kick loop repeats with
    /// the same fields of later attempts on the one built track. For the draws the track was built
    /// with it is the track's own fate, bit for bit. The track's remnant segment keeps the remnant
    /// of its own draws.
    #[must_use]
    pub(crate) fn fate_with(&self, draws: RemnantDraws) -> Option<Fate> {
        let fate = self.fate?;
        Some(match (self.options.remnant, fate.iron_core) {
            (RemnantRecipe::MandelMuller2020, Some(core)) => phases::iron_core_fate(
                self.options.remnant,
                fate.death.age().value(),
                core.supernova,
                core.co_core,
                fate.death.progenitor(),
                draws,
            ),
            (RemnantRecipe::Hurley2000, _) | (RemnantRecipe::MandelMuller2020, None) => fate,
        })
    }

    /// The largest radius the star has had up to `age`: non-decreasing in age and never below
    /// [`StarState::radius`] at `age` (plans 11 and 14).
    ///
    /// It is the largest of the radii at the segments' boundaries, at their knots (or at 17
    /// points of a segment without knots), at every local maximum between those points, located
    /// by a golden-section search at build time, and at `age` itself.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`Track::state_at`].
    #[must_use]
    pub fn max_radius_until(&self, age: Years) -> SolarRadii {
        SolarRadii::new(self.max_until(age, Sampled::Radius))
    }

    /// The largest luminosity the star has had up to `age`, in the same way as
    /// [`Track::max_radius_until`] (plan 14).
    ///
    /// # Panics
    ///
    /// In debug builds, as [`Track::state_at`].
    #[must_use]
    pub fn max_luminosity_until(&self, age: Years) -> SolarLuminosities {
        SolarLuminosities::new(self.max_until(age, Sampled::Luminosity))
    }

    /// The age up to which the track is built: its death's for a track built by
    /// [`Track::to_age`] past it, and infinite for [`Track::full`], whose remnant lasts.
    #[must_use]
    pub fn built_until(&self) -> Years {
        Years::new(self.built_until)
    }

    /// The age at which the main sequence ends, if the track has been built past it: the start
    /// of the first segment after the last main-sequence one (the rotation of an evolved star
    /// reads the star there, P06.T25).
    #[must_use]
    pub(crate) fn main_sequence_end(&self) -> Option<Years> {
        let last = self
            .segments
            .iter()
            .rposition(|segment| matches!(segment.model, Model::MainSequence { .. }))?;
        self.segments
            .get(last + 1)
            .map(|next| Years::new(next.start))
    }

    /// The initial mass the track was built for, M☉ (clamped into the covered range).
    #[must_use]
    pub const fn initial_mass(&self) -> SolarMasses {
        self.initial_mass
    }

    /// The bytes the track owns on the heap, beyond `size_of::<Track>()`: its segments and each
    /// segment's knots, samples and boxed closed forms, by capacity. For the server's byte-bounded caches (plan 06,
    /// P06.T34); nothing generated reads it.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.segments.iter().fold(
            self.segments.capacity() * size_of::<Segment>(),
            |bytes, segment| {
                bytes
                    + segment.knots.capacity() * size_of::<Knot>()
                    + segment.samples.capacity() * size_of::<Sample>()
                    + segment.model.heap_bytes()
            },
        )
    }

    /// The composition the track was built for.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The options the track was built with.
    #[must_use]
    pub const fn options(&self) -> TrackOptions {
        self.options
    }

    /// The number of thermal pulses since the start of the thermally pulsing AGB at `age`, the
    /// integral of 1 ÷ `τ_ip`(Mc, `M_env`) (P06.T8.b's interpulse period) over the phase, or `None`
    /// outside it (for P06.T24.b and T28.f).
    #[must_use]
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "P06.T24.b and T28.f are the first callers")
    )]
    pub(crate) fn pulse_phase_at(&self, age: Years) -> Option<f64> {
        let age = self.checked_age(age);
        let segment = self.segment_at(age);
        match segment.model {
            Model::ThermallyPulsingAgb { .. } => Some(segment.pulse_phase(age)),
            _ => None,
        }
    }

    /// The ages at which the track's state may step, with what steps there: every sudden death,
    /// the white dwarf's hand-over (until P06.T16), the helium flash when it is not bridged, and a
    /// helium main-sequence star too light to burn helium becoming a helium white dwarf. For the
    /// continuity tests.
    #[cfg(test)]
    pub(crate) fn declared_steps(&self) -> Vec<f64> {
        let mut steps: Vec<f64> = self
            .segments
            .windows(2)
            .filter(|pair| !pair[1].junction.continuous)
            .map(|pair| pair[1].start)
            .collect();
        if let Some(fate) = self.fate {
            steps.push(fate.death.age().value());
        }
        steps.dedup_by(|a, b| a.total_cmp(b).is_eq());
        steps
    }

    /// The segments, for the crate's tests.
    #[cfg(test)]
    pub(crate) fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The model physics of this track.
    #[must_use]
    fn physics(&self) -> Physics<'_> {
        Physics {
            coeffs: &self.coeffs,
            z: self.composition.z_fit(),
            remnant: self.options.remnant,
        }
    }

    /// `age` as a finite age within the built range.
    #[must_use]
    fn checked_age(&self, age: Years) -> f64 {
        let age = age.value();
        debug_assert!(
            age.is_finite() && age >= 0.0,
            "an age is finite and non-negative: {age}"
        );
        debug_assert!(
            age <= self.built_until,
            "the track is built to {} years, not {age}",
            self.built_until
        );
        if age > 0.0 {
            age.min(self.built_until)
        } else {
            0.0
        }
    }

    /// The segment that holds `age`, a checked age: the last that starts at or before it.
    #[must_use]
    fn segment_at(&self, age: f64) -> &Segment {
        let index = self.segments.partition_point(|s| s.start <= age);
        &self.segments[index.saturating_sub(1)]
    }

    /// A [`StarState`] of the evaluated segment state at `age`, with the wind's rate at it.
    #[must_use]
    fn star_state(&self, evaluated: &Evaluated, age: f64) -> StarState {
        let parts = evaluated.parts(age);
        let state = StarState::new(parts);
        let rate = wind::rate(self.options.wind, &state, &self.composition, self.eta);
        StarState::new(StarStateParts {
            mass_loss_rate: rate,
            ..parts
        })
    }

    /// The largest `quantity` up to `age` (see [`Track::max_radius_until`]).
    #[must_use]
    fn max_until(&self, age: Years, quantity: Sampled) -> f64 {
        let age = self.checked_age(age);
        let segment = self.segment_at(age);
        let now = segment.evaluate(&self.physics(), age);
        let current = match quantity {
            Sampled::Radius => now.point.point.radius.value(),
            Sampled::Luminosity => now.point.point.luminosity.value(),
        };
        segment.max_until(age, quantity).max(current)
    }
}

/// The lifetime of a star of initial mass `m0`, `comp` and `draws` under `options`: the same build
/// as [`Track::build`]'s in full, keeping nothing but the death.
#[must_use]
pub(crate) fn lifetime_of(
    m0: SolarMasses,
    comp: &Composition,
    draws: &StarDraws,
    options: TrackOptions,
) -> Years {
    fate_of(m0, comp, draws, options).death.age()
}

/// The fate, death and remnant, of a star of initial mass `m0`, `comp` and `draws` under
/// `options`: [`Track::full`]'s, bit for bit, from a build that keeps no segment (the cost of
/// [`lifetime_of`]).
#[must_use]
pub(crate) fn fate_of(
    m0: SolarMasses,
    comp: &Composition,
    draws: &StarDraws,
    options: TrackOptions,
) -> Fate {
    let coeffs = ZCoeffs::new(comp.z_fit());
    let m0 = checked_initial_mass(m0);
    let eta = reimers_eta(draws.eta().value());
    let outcome = Builder::new(
        &coeffs,
        comp,
        options,
        eta,
        RemnantDraws::of(draws),
        Resolution::GENERATOR,
        build::Keep::Lifetime,
    )
    .stripped_by_companion(is_companion_stripped(draws))
    .run(m0.value(), None);
    outcome
        .fate
        .expect("a build with no age to stop at runs to the star's death")
}

/// Whether the provisional companion-stripped mark of `draws` is set, against the kick law's
/// [`stripped_share`](crate::stellar::remnant::KickLawParams::stripped_share) (plan 06, design
/// note 11): the one question the track asks of it, for the electron-capture window.
#[must_use]
fn is_companion_stripped(draws: &StarDraws) -> bool {
    crate::stellar::remnant::KickLawParams::default().is_stripped(draws.stripped())
}

/// The state at `age` of a star of initial mass `m0`, `comp` and `draws` under `options`, if its
/// main sequence has no knots and `age` lies on it: [`Track::state_at`] of its full track, bit for
/// bit, from a build of the main-sequence segment alone (plan 06, P06.T38.b). `None` if the segment
/// has knots, if `age` is not before the segment's end (where the full track's next segment
/// starts), or if `age` is negative or not finite.
#[must_use]
pub(crate) fn main_sequence_state_of(
    m0: SolarMasses,
    comp: &Composition,
    draws: &StarDraws,
    options: TrackOptions,
    age: Years,
) -> Option<StarState> {
    let a = age.value();
    if !(a >= 0.0 && a.is_finite()) {
        return None;
    }
    let track = Track::knot_free_main_sequence(m0, comp, draws, options)?;
    (a < track.built_until).then(|| track.state_at(age))
}

impl Track {
    /// The track of a star of initial mass `m0`, `comp` and `draws` under `options` built to the
    /// end of its main sequence and no further, with no samples for the maxima, if its main
    /// sequence has no knots; `None` if it has (plan 06, P06.T38.b).
    ///
    /// Its one segment is [`Builder::run`]'s first, from the same builder, entered at age zero with
    /// no junction, under the builder's own knot rule ([`build::NEGLIGIBLE_LOSS`]). Without knots
    /// the segment's state is a closed form of the age, so the track's [`Track::state_at`] is the
    /// full track's, bit for bit, at every age before [`Track::built_until`], the main sequence's
    /// end. Its maxima are not built: [`Track::max_radius_until`] and
    /// [`Track::max_luminosity_until`] are not to be asked of it, which is why it stays in the
    /// crate.
    #[must_use]
    pub(crate) fn knot_free_main_sequence(
        m0: SolarMasses,
        comp: &Composition,
        draws: &StarDraws,
        options: TrackOptions,
    ) -> Option<Self> {
        let coeffs = ZCoeffs::new(comp.z_fit());
        let m0 = checked_initial_mass(m0);
        let eta = reimers_eta(draws.eta().value());
        let segment = Builder::new(
            &coeffs,
            comp,
            options,
            eta,
            RemnantDraws::of(draws),
            Resolution::GENERATOR,
            build::Keep::Track,
        )
        .stripped_by_companion(is_companion_stripped(draws))
        .main_sequence_segment(0.0, m0.value(), None)
        .segment;
        if !segment.knots.is_empty() {
            return None;
        }
        let end = segment.end;
        Some(Self {
            coeffs,
            composition: *comp,
            options,
            initial_mass: m0,
            eta,
            segments: vec![segment],
            fate: None,
            built_until: end,
        })
    }
}

/// The initial mass `m0` within the range the formulae cover.
#[must_use]
fn checked_initial_mass(m0: SolarMasses) -> SolarMasses {
    let m = m0.value();
    debug_assert!(
        m >= MIN_INITIAL_MASS.value() && m <= MAX_INITIAL_MASS.value(),
        "a track covers 0.1–100 M☉, not {m}"
    );
    SolarMasses::new(if m > MIN_INITIAL_MASS.value() {
        m.min(MAX_INITIAL_MASS.value())
    } else {
        MIN_INITIAL_MASS.value()
    })
}

/// Which sampled quantity a maximum is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sampled {
    Radius,
    Luminosity,
}

/// How a segment's coordinate follows age.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Coordinate {
    /// The fraction x of the phase's clock span, linear in age: (age − start) ÷ (`nominal_end` −
    /// start), with `nominal_end` the age at which the span would end at constant mass.
    Linear { nominal_end: f64 },
    /// The fractional age τ of a main sequence, from `tau0` at the segment's start: from the knots,
    /// or linear in age where the segment has none.
    Fraction { tau0: f64 },
}

/// The offsets in log₁₀ L and log₁₀ R that carry a step of HPT's formulae at a segment's start
/// into continuity, decaying over its first [`build::JUNCTION_RAMP`] of the coordinate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Junction {
    pub(crate) log_l: f64,
    pub(crate) log_r: f64,
    /// Whether the junction is continuous; a declared step (a death, the unbridged flash, the
    /// white dwarf's hand-over) has no offsets.
    pub(crate) continuous: bool,
}

impl Junction {
    /// A declared step, with no offsets.
    pub(crate) const STEP: Self = Self {
        log_l: 0.0,
        log_r: 0.0,
        continuous: false,
    };
}

/// A segment's knot: the integrated quantities at one value of its coordinate, with their rates in
/// age, and the star's luminosity and radius there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Knot {
    /// Age, years.
    pub(crate) age: f64,
    /// Mass, M☉.
    pub(crate) mass: f64,
    /// dM ÷ d age, M☉ per year: minus the wind's rate.
    pub(crate) mass_rate: f64,
    /// The fractional age τ of a main sequence (other phases leave it zero).
    pub(crate) tau: f64,
    /// dτ ÷ d age, per year.
    pub(crate) tau_rate: f64,
    /// Thermal pulses since the start of the thermally pulsing AGB (other phases leave it zero).
    pub(crate) pulses: f64,
    /// Pulses per year.
    pub(crate) pulse_rate: f64,
    /// L, L☉.
    pub(crate) luminosity: f64,
    /// R, R☉.
    pub(crate) radius: f64,
}

/// A point at which a segment's luminosity and radius were evaluated at build time, for the
/// monotone maxima.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sample {
    pub(crate) age: f64,
    /// The largest radius at this sample or before it in the segment, R☉.
    pub(crate) max_radius: f64,
    /// The largest luminosity at this sample or before it in the segment, L☉.
    pub(crate) max_luminosity: f64,
}

/// One phase of a track, or its remnant.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Segment {
    pub(crate) model: Model,
    /// The age at which the segment starts, years.
    pub(crate) start: f64,
    /// The age at which it ends, years; infinite for the remnant.
    pub(crate) end: f64,
    pub(crate) coordinate: Coordinate,
    /// The mass the segment starts with, M☉, which it keeps if it has no knots.
    pub(crate) mass: f64,
    pub(crate) knots: Vec<Knot>,
    pub(crate) junction: Junction,
    /// The build-time samples, in age order.
    pub(crate) samples: Vec<Sample>,
    /// The largest radius and luminosity before the segment.
    pub(crate) max_before: [f64; 2],
}

/// A segment's state at one age: the phase's point and the mass it was evaluated for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Evaluated {
    pub(crate) point: model::Point,
    /// The mass, M☉, never below the core.
    pub(crate) mass: f64,
    /// How far through the segment the age lies, 0–1.
    pub(crate) fraction: f64,
}

impl Evaluated {
    /// The parts of a [`StarState`] at `age`, with no wind.
    #[must_use]
    pub(crate) fn parts(&self, age: f64) -> StarStateParts {
        let p = self.point.point;
        let remnant = self.point.phase.is_remnant();
        StarStateParts {
            phase: self.point.phase,
            age: Years::new(age),
            mass: SolarMasses::new(self.mass),
            // A remnant is all core; a living star's core never exceeds its mass.
            core_mass: if remnant {
                SolarMasses::new(self.mass)
            } else {
                SolarMasses::new(p.core_mass.value().min(self.mass))
            },
            luminosity: p.luminosity,
            radius: p.radius,
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: self.fraction,
        }
    }
}

impl Segment {
    /// The state at `age`, which lies in the segment.
    #[must_use]
    pub(crate) fn evaluate(&self, phys: &Physics<'_>, age: f64) -> Evaluated {
        let (coord, mass) = self.coordinate_and_mass(age);
        self.evaluate_at(phys, age, coord, mass)
    }

    /// The state at `age` for the coordinate `coord` and mass `mass`, which the integration
    /// supplies while it builds the segment and [`Segment::coordinate_and_mass`] afterwards.
    #[must_use]
    pub(crate) fn evaluate_at(
        &self,
        phys: &Physics<'_>,
        age: f64,
        coord: f64,
        mass: f64,
    ) -> Evaluated {
        let mt = SolarMasses::new(mass.max(build::MIN_EVALUATED_MASS));
        self.finish_point(self.model.point(phys, coord, mt, age), age, coord, mt)
    }

    /// [`Segment::evaluate_at`], with the lifetime at the current mass, Myr, of a main sequence or
    /// helium main sequence whose closed forms are rebuilt at it (`None` for any other segment),
    /// which the integration of its fractional age reads.
    #[must_use]
    pub(crate) fn evaluate_with_lifetime(
        &self,
        phys: &Physics<'_>,
        age: f64,
        coord: f64,
        mass: f64,
    ) -> (Evaluated, Option<Megayears>) {
        let mt = SolarMasses::new(mass.max(build::MIN_EVALUATED_MASS));
        let (point, lifetime) = self.model.rebuilt_point(phys, coord, mt, age);
        (self.finish_point(point, age, coord, mt), lifetime)
    }

    /// The segment's state from the model's `point` at `age`, coordinate `coord` and evaluated
    /// mass `mt`: with the junction's offsets, the mass never below the core, and the fraction.
    #[must_use]
    fn finish_point(
        &self,
        mut point: model::Point,
        age: f64,
        coord: f64,
        mt: SolarMasses,
    ) -> Evaluated {
        let ramp = build::ramp(self.ramp_coordinate(coord));
        if ramp < 1.0 {
            let keep = 1.0 - ramp;
            let p = &mut point.point;
            p.luminosity = p.luminosity * crate::math::exp10(self.junction.log_l * keep);
            p.radius = p.radius * crate::math::exp10(self.junction.log_r * keep);
        }
        let mass = if point.phase.is_remnant() {
            point.point.core_mass.value()
        } else {
            mt.value().max(point.point.core_mass.value())
        };
        let span = self.end - self.start;
        let fraction = if span.is_finite() && span > 0.0 {
            ((age - self.start) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Evaluated {
            point,
            mass,
            fraction,
        }
    }

    /// The coordinate and the mass at `age`: interpolated between the knots, or from the
    /// segment's constant mass and linear coordinate where it has none.
    #[must_use]
    pub(crate) fn coordinate_and_mass(&self, age: f64) -> (f64, f64) {
        if self.knots.len() < 2 {
            return (self.linear_coordinate(age), self.mass);
        }
        let (a, b) = self.knot_interval(age);
        let mass = Interval {
            x0: a.age,
            x1: b.age,
            y0: a.mass,
            y1: b.mass,
            d0: a.mass_rate,
            d1: b.mass_rate,
        }
        .at(age);
        let coord = match self.coordinate {
            Coordinate::Linear { .. } => self.linear_coordinate(age),
            Coordinate::Fraction { .. } => Interval {
                x0: a.age,
                x1: b.age,
                y0: a.tau,
                y1: b.tau,
                d0: a.tau_rate,
                d1: b.tau_rate,
            }
            .at(age),
        };
        (coord, mass)
    }

    /// The thermal pulses at `age`, interpolated between the knots (zero without them).
    #[must_use]
    pub(crate) fn pulse_phase(&self, age: f64) -> f64 {
        if self.knots.len() < 2 {
            return 0.0;
        }
        let (a, b) = self.knot_interval(age);
        Interval {
            x0: a.age,
            x1: b.age,
            y0: a.pulses,
            y1: b.pulses,
            d0: a.pulse_rate,
            d1: b.pulse_rate,
        }
        .at(age)
    }

    /// The two knots around `age`: the interval whose start is the last knot at or before it.
    #[must_use]
    fn knot_interval(&self, age: f64) -> (&Knot, &Knot) {
        let n = self.knots.len();
        let index = self
            .knots
            .partition_point(|k| k.age <= age)
            .saturating_sub(1)
            .min(n - 2);
        (&self.knots[index], &self.knots[index + 1])
    }

    /// The coordinate where it is linear in age.
    #[must_use]
    fn linear_coordinate(&self, age: f64) -> f64 {
        match self.coordinate {
            Coordinate::Linear { nominal_end } => {
                let span = nominal_end - self.start;
                if span > 0.0 {
                    ((age - self.start) / span).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
            Coordinate::Fraction { tau0 } => {
                let span = self.end - self.start;
                let x = if span > 0.0 {
                    ((age - self.start) / span).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (1.0 - x) * tau0 + x
            }
        }
    }

    /// How far the junction's offsets have decayed at coordinate `coord`: the coordinate's
    /// progress from the segment's start.
    #[must_use]
    fn ramp_coordinate(&self, coord: f64) -> f64 {
        match self.coordinate {
            Coordinate::Linear { .. } => coord,
            Coordinate::Fraction { tau0 } => {
                if tau0 < 1.0 {
                    (coord - tau0) / (1.0 - tau0)
                } else {
                    1.0
                }
            }
        }
    }

    /// The largest sampled `quantity` in the segment up to `age`, with the maximum before it.
    #[must_use]
    fn max_until(&self, age: f64, quantity: Sampled) -> f64 {
        let index = self.samples.partition_point(|s| s.age <= age);
        let before = match quantity {
            Sampled::Radius => self.max_before[0],
            Sampled::Luminosity => self.max_before[1],
        };
        match index.checked_sub(1).map(|i| &self.samples[i]) {
            Some(sample) => before.max(match quantity {
                Sampled::Radius => sample.max_radius,
                Sampled::Luminosity => sample.max_luminosity,
            }),
            None => before,
        }
    }
}

#[cfg(test)]
mod tests;
