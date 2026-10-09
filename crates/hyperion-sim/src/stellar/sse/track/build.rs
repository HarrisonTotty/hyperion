//! The integrator: a track's segments, phase by phase, from the zero-age main sequence to the
//! remnant (plan 06, P06.T10.c–e).
//!
//! Each phase is entered with the mass and, where it applies, the effective initial mass the
//! previous one ended with (HPT section 7.1), and builds one [`Segment`]. The wind is integrated
//! by [`STEPS_PER_KNOT`] midpoint steps between knots at fixed values of the phase's coordinate
//! (see the module documentation of [`track`](super)); a phase whose wind would remove under
//! [`NEGLIGIBLE_LOSS`] of its mass has no knots. A phase ends at its nominal end, at the loss of
//! its envelope (the root of the envelope mass on the knot interpolant, by [`BISECTIONS`]
//! bisections), or where its core reaches a limit, and hands over to the next phase or to the
//! star's death.
//!
//! Nothing here reads the age a track is asked for except to stop building (design note 2): a
//! segment depends only on those before it, so [`Track::to_age`](super::Track::to_age)'s segments
//! are bit for bit [`Track::full`](super::Track::full)'s.

use std::error::Error;
use std::fmt;

use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::remnant::white_dwarf::{self, WhiteDwarfCore};
use crate::stellar::{Composition, Phase, StarState};
use crate::units::{Megayears, SolarLuminosities, SolarMasses, Years};

use super::super::PhasePoint;
use super::super::agb::ThermallyPulsingAgb;
use super::super::coeffs::ZCoeffs;
use super::super::gb::{self, GiantBranch};
use super::super::helium::HeliumStar;
use super::super::ms::MainSequence;
use super::super::wind::{self, ReimersEta};
use super::excess::{HeliumHook, HeliumTable};
use super::model::{Model, Physics, Span};
use super::{Coordinate, Evaluated, Fate, Junction, Knot, Sample, Segment, TrackOptions};

/// Knots of a segment with mass loss, both ends included (plan 06, P06.T10.c).
pub(crate) const KNOTS: usize = 16;

/// Knots of the thermally pulsing AGB, where the superwind strips the envelope.
pub(crate) const PULSING_KNOTS: usize = 32;

/// Midpoint steps between two knots.
pub(crate) const STEPS_PER_KNOT: usize = 4;

/// A phase whose wind would remove less than this share of its mass, at the largest of its rates
/// at the start, middle and end of the phase at constant mass, has no knots.
pub(crate) const NEGLIGIBLE_LOSS: f64 = 1e-6;

/// The most knots a phase with an envelope takes, as a multiple of its grid: past it, the next
/// knot is the span's end.
const MAX_KNOT_FACTOR: usize = 2;

/// The most knots the envelope integrator takes past its grid ([`Builder::envelope_knots`])
/// where the age's steps resolve the phase's laws. Past them a phase may go on for the stall
/// allowance, [`EnvelopeClock::still_years`] of age, for the rounding of the age. One that goes further
/// has stalled, and its build panics ([`IntegrateEnvelopeError`]).
///
/// Past the grid each knot goes to twice the envelope's remaining life at its present rate of
/// loss, so the phase ends in one interval unless the rate's mean over it is under half its rate
/// at the start. In the R06 census of 10⁵ systems, about 1 in 4,000 envelope phases takes more
/// than one knot past the grid, and none takes more than 5. The longest are the rounding that
/// [`EnvelopeClock::still_years`] allows for.
///
/// This bound is for the worst wind whose rate falls no faster than the envelope: a rate in
/// proportion to it. Over one interval of s = [`STEPS_PER_KNOT`] midpoint steps that wind leaves
/// (1 − 2/s + 2/s²)^s of the envelope, 0.153 at s = 4 (0.139 at the convergence test's 8).
/// - It starts from an envelope of at most [`MAX_INITIAL_MASS`], 150 M☉.
/// - It must reach the rounding of a core of at least [`MIN_EVALUATED_MASS`], 10⁻³ M☉: half a
///   unit in the core's last place, at least 2⁻⁵⁴ of the core.
/// - That takes at most ⌈ln(150 ÷ (2⁻⁵⁴ × 10⁻³)) ÷ ln(1 ÷ 0.153)⌉ = 27 intervals that fall short.
/// - There a step can no longer shorten the envelope, so one interval more is the last that can
///   end the phase: 28 in all.
///
/// No wind here falls faster than the envelope, which would then never run out: as it runs out,
/// its rate of loss tends to the bare core's wind, which is not zero.
///
/// The grid takes at most `n` knots, since each interval passes at least one more of its values.
/// So an envelope segment holds at most `n` + 29 knots and one more for each unit in the age's last
/// place within the stall allowance, whatever its laws do.
///
/// P11.T4.h's work in progress stalled for good (plan 06's Risks). Its wind dropped where the mass
/// passed the core, so each midpoint saw a weaker wind than its knot, and the mass never moved.
/// The envelope stuck at 3 × 10⁻¹⁵ M☉ on a core that cannot grow, each knot moved the age by one
/// unit in its last place, and the span's end was some 4 × 10¹⁰ knots away. With P11.T4.h's
/// discontinuous wind restored, the guard stops record 0x61f85aa800000001 of the census's galaxy
/// at 45 knots: 28, and 17 more within the stall allowance.
///
/// [`MAX_INITIAL_MASS`]: super::MAX_INITIAL_MASS
const MAX_KNOTS_PAST_GRID: u32 = 28;

/// Bisections of a knot interval for the loss of the envelope.
const BISECTIONS: u32 = 40;

/// The helium flash's duration, years (plan 06, design note 3).
pub(super) const FLASH_YEARS: f64 = 1e4;

/// The share of a phase's coordinate over which a junction's offsets decay.
pub(crate) const JUNCTION_RAMP: f64 = 0.02;

/// Samples of a segment without knots, both ends included, for the monotone maxima.
const SAMPLES: usize = 17;

/// How far before a sample, as a share of the interval before it, the maxima's samples probe
/// whether the radius or luminosity is still rising into it.
const SLOPE_PROBE: f64 = 1e-6;

/// Iterations of the golden-section search for a local maximum between samples.
const PEAK_SEARCH: u32 = 40;

/// The smallest mass a phase's formulae are evaluated at, M☉: a guard that keeps an integration
/// step that overshoots the end of a phase from reaching a mass of zero. No phase a star reaches
/// is this light.
pub(crate) const MIN_EVALUATED_MASS: f64 = 1e-3;

/// How finely a track is integrated: [`Resolution::GENERATOR`] for every generated track, and
/// finer for the convergence test (P06.T10.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Resolution {
    /// Knots of a segment with mass loss.
    pub(crate) knots: usize,
    /// Knots of the thermally pulsing AGB.
    pub(crate) pulsing_knots: usize,
    /// Midpoint steps between knots.
    pub(crate) steps: usize,
}

impl Resolution {
    /// The generator's: [`KNOTS`], [`PULSING_KNOTS`] and [`STEPS_PER_KNOT`]. Changing any of them
    /// changes stellar output and is a generator-version change.
    pub(crate) const GENERATOR: Self = Self {
        knots: KNOTS,
        pulsing_knots: PULSING_KNOTS,
        steps: STEPS_PER_KNOT,
    };

    /// Twice the knots and twice the steps.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn doubled(self) -> Self {
        Self {
            knots: 2 * self.knots - 1,
            pulsing_knots: 2 * self.pulsing_knots - 1,
            steps: 2 * self.steps,
        }
    }
}

/// How far a junction's offsets have decayed at coordinate progress `u` (0 at the segment's
/// start): 0 until they start, 1 once they are gone at [`JUNCTION_RAMP`], and the smooth step 3s²
/// − 2s³ between.
#[must_use]
pub(crate) fn ramp(u: f64) -> f64 {
    let s = u / JUNCTION_RAMP;
    if s >= 1.0 {
        1.0
    } else if s > 0.0 {
        s * s * (3.0 - 2.0 * s)
    } else {
        0.0
    }
}

/// What a build keeps: everything a [`Track`](super::Track) needs, or only what the lifetime
/// does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keep {
    /// Every segment with its samples, and the remnant.
    Track,
    /// Nothing but the fate: no samples, no segments kept.
    Lifetime,
    /// The fate and the remnant's segment, with no other segment and no samples: what the range
    /// brief of a dead star reads (P06.T38.e).
    Remnant,
}

/// A built track's parts.
#[derive(Debug)]
pub(crate) struct Outcome {
    pub(crate) segments: Vec<Segment>,
    pub(crate) fate: Option<Fate>,
    /// The age up to which the segments reach, years; infinite once the remnant is built.
    pub(crate) built_until: f64,
}

/// The phase a star enters next, with what it enters it with.
#[derive(Debug)]
pub(super) enum Entry {
    /// The onset of collapse of a star of final mass `mass` (P06.T15.a).
    Protostar {
        mass: f64,
    },
    /// The contraction from the end of accretion to the arrival on the main sequence at `arrival`
    /// years (P06.T15.b).
    PreMainSequence {
        mass: f64,
        arrival: f64,
    },
    /// The main sequence from fractional age `tau0`: zero but for a star that is on it when its
    /// accretion ends.
    MainSequence {
        mass: f64,
        tau0: f64,
        /// The segment, if the contraction built it already to meet its slopes.
        built: Option<Box<FractionBuilt>>,
    },
    HertzsprungGap {
        m0: f64,
        mass: f64,
    },
    FirstGiantBranch {
        m0: f64,
        mass: f64,
    },
    /// The helium flash, after which HPT section 7.1 reset the initial mass to the current one.
    Flash {
        mass: f64,
    },
    CoreHeliumBurning {
        m0: f64,
        mass: f64,
    },
    EarlyAgb {
        m0: f64,
        mass: f64,
    },
    /// The thermal pulses of a star whose early AGB had the initial mass `m0`, the mass
    /// `m_c_bagb` read, and a core at the base of the AGB of `mc_bagb`.
    ThermallyPulsingAgb {
        phase: Box<ThermallyPulsingAgb>,
        m0: f64,
        mc_bagb: f64,
        mass: f64,
    },
    /// A naked helium star on its main sequence at fractional age `tau0`.
    HeliumMainSequence {
        mass: f64,
        tau0: f64,
    },
    /// A naked helium star after its main sequence, from its clock `clock0`.
    HeliumShellBurning {
        star: Box<HeliumStar>,
        clock0: Megayears,
        mass: f64,
    },
    /// The post-AGB crossing of a star that has lost its envelope on the AGB, towards the white
    /// dwarf of `fate` (P06.T16.a).
    PostAgb(Fate),
    Dead(Fate),
}

impl Entry {
    /// The mass the star enters the phase with: none for the post-AGB crossing and a death, which
    /// carry their fate instead.
    #[must_use]
    fn mass(&self) -> Option<SolarMasses> {
        match self {
            Self::Protostar { mass }
            | Self::PreMainSequence { mass, .. }
            | Self::MainSequence { mass, .. }
            | Self::HertzsprungGap { mass, .. }
            | Self::FirstGiantBranch { mass, .. }
            | Self::Flash { mass }
            | Self::CoreHeliumBurning { mass, .. }
            | Self::EarlyAgb { mass, .. }
            | Self::ThermallyPulsingAgb { mass, .. }
            | Self::HeliumMainSequence { mass, .. }
            | Self::HeliumShellBurning { mass, .. } => Some(SolarMasses::new(*mass)),
            Self::PostAgb(_) | Self::Dead(_) => None,
        }
    }
}

/// One phase built: its segment (none for a phase of no duration), what follows, and the state it
/// ends in.
pub(super) struct Step {
    pub(super) segment: Option<Segment>,
    pub(super) next: Entry,
    pub(super) end: f64,
    /// log₁₀ L, log₁₀ R and the core mass at the end, for the next junction or, where the star
    /// dies, for the white dwarf's cooling origin; `None` where nothing was evaluated.
    pub(super) end_state: Option<[f64; 3]>,
}

/// The track integrator for one star.
#[derive(Debug)]
pub(crate) struct Builder<'a> {
    pub(super) phys: Physics<'a>,
    composition: &'a Composition,
    pub(super) options: TrackOptions,
    eta: ReimersEta,
    /// The star's remnant draws, which decide an iron core's collapse under
    /// [`RemnantRecipe::MandelMuller2020`](crate::stellar::remnant::RemnantRecipe).
    pub(super) remnant_draws: RemnantDraws,
    /// Whether the star's provisional companion-stripped mark is set (plan 06, design note 11):
    /// it widens the electron-capture window ([`Builder::stripped_by_companion`]).
    pub(super) companion_stripped: bool,
    pub(super) resolution: Resolution,
    pub(super) keep: Keep,
}

impl<'a> Builder<'a> {
    /// A builder for a star of `composition` (whose coefficients are `coeffs`), Reimers `eta` and
    /// the remnant draws `remnant_draws`.
    #[must_use]
    pub(crate) fn new(
        coeffs: &'a ZCoeffs,
        composition: &'a Composition,
        options: TrackOptions,
        eta: ReimersEta,
        remnant_draws: RemnantDraws,
        resolution: Resolution,
        keep: Keep,
    ) -> Self {
        Self {
            phys: Physics {
                coeffs,
                z: composition.z_fit(),
                remnant: options.remnant(),
                bridges: options.bridges(),
                helium: HeliumHook::of(composition, &HeliumTable::COMMITTED),
            },
            composition,
            options,
            eta,
            remnant_draws,
            companion_stripped: false,
            resolution,
            keep,
        }
    }

    /// This builder with the helium-excess correction `helium` in place of the committed table's
    /// (P06.T17).
    #[must_use]
    pub(crate) fn with_helium(mut self, helium: Option<HeliumHook>) -> Self {
        self.phys.helium = helium;
        self
    }

    /// This builder for a star whose provisional companion-stripped mark is `stripped` (plan 06,
    /// design note 11, P06.T19.c): a marked star's electron-capture window is the companion-stripped
    /// one, 1 M☉ wide, and not the single star's 0.1 M☉ ([`ElectronCaptureWindows`]). Nothing
    /// else on the track reads the mark: its envelope is a single star's until plan 11.
    ///
    /// [`ElectronCaptureWindows`]: crate::stellar::remnant::collapse::ElectronCaptureWindows
    #[must_use]
    pub(crate) fn stripped_by_companion(self, stripped: bool) -> Self {
        Self {
            companion_stripped: stripped,
            ..self
        }
    }

    /// The core at helium ignition of an `M_HeF` star, the lightest helium star that burns helium
    /// (see [`Builder::helium_main_sequence`]). Found where it is asked, once a helium main
    /// sequence is entered, rather than for every build: a main sequence alone never needs it
    /// (P06.T38.b's fast path, which it cost a sixth of).
    #[must_use]
    pub(super) fn lightest_helium_star(&self) -> f64 {
        let coeffs = self.phys.coeffs;
        let m_hef = coeffs.m_hef();
        GiantBranch::new(m_hef, coeffs)
            .core_mass(gb::l_hei(m_hef, coeffs))
            .value()
    }

    /// The track of a star of initial mass `m0` (M☉), built to the segment that holds `age_max`,
    /// or to the remnant.
    ///
    /// # Panics
    ///
    /// As [`Builder::run_from`].
    #[must_use]
    pub(crate) fn run(&self, m0: f64, age_max: Option<f64>) -> Outcome {
        self.run_from(Entry::Protostar { mass: m0 }, age_max)
    }

    /// The track of a star that enters its life at `entry`, at age zero: [`Builder::run`] from the
    /// zero-age main sequence, and plan 11's naked helium stars from a helium-star entry (ruling 34
    /// of 2026-09-22, `track/binary.rs`).
    ///
    /// # Panics
    ///
    /// If a phase's envelope cannot be integrated to its end ([`IntegrateEnvelopeError`]), which
    /// is a bug in that phase's laws. The message names the star by the mass it enters its life
    /// with, its composition, its Reimers η, the options, the resolution and the companion-stripped
    /// mark, and says where the phase stalled.
    #[must_use]
    pub(super) fn run_from(&self, entry: Entry, age_max: Option<f64>) -> Outcome {
        // What a stalled phase's panic names the star by.
        let entry_mass = entry.mass();
        let mut segments: Vec<Segment> = Vec::new();
        let mut entry = entry;
        let mut start = 0.0;
        let mut previous: Option<[f64; 3]> = None;
        let mut max_before = [0.0_f64; 2];
        let mut bridged = false;
        loop {
            if let Entry::Dead(fate) = entry {
                if self.keep != Keep::Lifetime {
                    let knee = previous.filter(|_| bridged).map(|[log_l, ..]| log_l);
                    segments.push(self.remnant_segment(fate, max_before, previous, knee));
                }
                return Outcome {
                    segments,
                    fate: Some(fate),
                    built_until: f64::INFINITY,
                };
            }
            bridged = matches!(entry, Entry::PostAgb(_));
            let step = self.phase(entry, start, previous).unwrap_or_else(|error| {
                let with =
                    entry_mass.map_or_else(String::new, |m| format!(" with {} M☉", m.value()));
                panic!(
                    "the track of a star entering its life{with} ({:?}, {:?}, {:?}, {:?}, \
                     companion-stripped mark {}) stalled: {error}",
                    self.composition,
                    self.eta,
                    self.options,
                    self.resolution,
                    self.companion_stripped
                )
            });
            if let Some(mut segment) = step.segment
                && self.keep == Keep::Track
            {
                // The maxima count from the main sequence: see `Track::max_radius_until`.
                if !segment.model.is_before_main_sequence() {
                    self.sample(&mut segment, max_before);
                    if let Some(last) = segment.samples.last() {
                        max_before = [last.max_radius, last.max_luminosity];
                    }
                }
                segments.push(segment);
            }
            entry = step.next;
            start = step.end;
            previous = step.end_state;
            if !matches!(entry, Entry::Dead(_)) && age_max.is_some_and(|a| start > a) {
                return Outcome {
                    segments,
                    fate: None,
                    built_until: start,
                };
            }
        }
    }

    /// Builds the phase `entry` from age `start`, continuing from `previous`.
    ///
    /// # Errors
    ///
    /// As [`Builder::envelope_segment`], for the phases with an envelope.
    fn phase(
        &self,
        entry: Entry,
        start: f64,
        previous: Option<[f64; 3]>,
    ) -> Result<Step, IntegrateEnvelopeError> {
        Ok(match entry {
            Entry::Protostar { mass } => self.protostar(mass),
            Entry::PreMainSequence { mass, arrival } => {
                self.pre_main_sequence(start, mass, arrival)
            }
            Entry::MainSequence { mass, tau0, built } => {
                self.main_sequence(start, mass, tau0, built)
            }
            Entry::HertzsprungGap { m0, mass } => {
                self.hertzsprung_gap(start, m0, mass, previous)?
            }
            Entry::FirstGiantBranch { m0, mass } => self.giant_branch(start, m0, mass, previous)?,
            Entry::Flash { mass } => self.flash(start, mass, previous),
            Entry::CoreHeliumBurning { m0, mass } => {
                self.core_helium_burning(start, m0, mass, previous)?
            }
            Entry::EarlyAgb { m0, mass } => self.early_agb(start, m0, mass, previous)?,
            Entry::ThermallyPulsingAgb {
                phase,
                m0,
                mc_bagb,
                mass,
            } => self.pulsing_agb(start, &phase, m0, mc_bagb, mass, previous)?,
            Entry::HeliumMainSequence { mass, tau0 } => {
                self.helium_main_sequence(start, mass, tau0, previous)
            }
            Entry::HeliumShellBurning { star, clock0, mass } => {
                self.helium_shell_burning(start, &star, clock0, mass, previous)?
            }
            Entry::PostAgb(fate) => self.post_agb(start, fate, previous),
            Entry::Dead(fate) => Step {
                segment: None,
                next: Entry::Dead(fate),
                end: start,
                end_state: None,
            },
        })
    }

    // ---------------------------------------------------------------------------------------------
    // Integration.

    /// A segment of `model` whose coordinate is the fractional age τ from `tau0` of a main
    /// sequence of lifetime `duration`(M) years, entered at `start` with `mass`. `limit`(M) is
    /// positive while the phase can go on; `fixed`(M) is the model at a constant mass, for a
    /// segment without knots.
    #[expect(
        clippy::too_many_arguments,
        reason = "a phase's model, entry state and three laws of its mass"
    )]
    pub(super) fn fraction_segment(
        &self,
        model: Model,
        start: f64,
        mass: f64,
        tau0: f64,
        previous: Option<[f64; 3]>,
        duration: impl Fn(f64) -> f64,
        limit: impl Fn(f64) -> f64,
        fixed: impl Fn(f64) -> Model,
    ) -> FractionBuilt {
        let mut segment = Segment {
            model,
            start,
            end: start,
            coordinate: Coordinate::Fraction { tau0 },
            mass,
            knots: Vec::new(),
            junction: Junction::STEP,
            samples: Vec::new(),
            max_before: [0.0; 2],
        };
        segment.junction = self.junction(&segment, previous, tau0, mass, start);
        let remaining = (1.0 - tau0) * duration(mass);
        let rates = [tau0, f64::midpoint(tau0, 1.0), 1.0].map(|tau| {
            let age = start + (tau - tau0) * duration(mass);
            self.wind_rate(&segment.evaluate_at(&self.phys, age, tau, mass), age)
        });
        if !significant(rates, remaining, mass) {
            segment.model = fixed(mass);
            segment.end = start + remaining;
            return FractionBuilt {
                segment,
                end_mass: mass,
                ended: false,
            };
        }
        let n = self.resolution.knots;
        let grid: Vec<f64> = (0..n)
            .map(|k| {
                let x = index_fraction(k, n);
                (1.0 - x) * tau0 + x
            })
            .collect();
        let derive = |tau: f64, state: [f64; 2]| -> ([f64; 2], Knot) {
            let [age, m] = state;
            // The rebuilt closed forms carry the lifetime at the mass, `duration`'s bit for bit.
            let (evaluated, rebuilt) = segment.evaluate_with_lifetime(&self.phys, age, tau, m);
            let rate = self.wind_rate(&evaluated, age);
            let lifetime = rebuilt.map_or_else(
                || duration(m.max(MIN_EVALUATED_MASS)),
                |t_ms| t_ms.value() * 1e6,
            );
            let knot = Knot {
                age,
                mass: m,
                mass_rate: -rate,
                tau,
                tau_rate: 1.0 / lifetime,
                pulses: 0.0,
                pulse_rate: 0.0,
                luminosity: evaluated.point.point.luminosity.value(),
                radius: evaluated.point.point.radius.value(),
            };
            ([lifetime, -rate * lifetime], knot)
        };
        let (knots, stopped) =
            self.integrate(&grid, [start, mass], derive, |knot| limit(knot.mass) <= 0.0);
        segment.knots = knots;
        let n = segment.knots.len();
        let (end, end_mass) = if stopped {
            let (a, b) = (segment.knots[n - 2].age, segment.knots[n - 1].age);
            let root = bisect(a, b, |age| limit(segment.coordinate_and_mass(age).1));
            (root, segment.coordinate_and_mass(root).1)
        } else {
            let last = segment.knots[n - 1];
            (last.age, last.mass)
        };
        segment.end = end;
        FractionBuilt {
            segment,
            end_mass,
            ended: stopped,
        }
    }

    /// A segment of `model` whose phase clock runs over `span` from `start`, entered with `mass`,
    /// that ends at the end of its span or when its envelope ([`EnvelopeLaws::envelope`], M☉,
    /// positive while the phase can go on) is gone, whichever comes first.
    ///
    /// With mass loss the knots sit at fixed values of u = 1 − (1 − x)(1 − y), with x the phase's
    /// progress ([`EnvelopeLaws::progress`]), 0 at its start and 1 at the end of its span, and y the
    /// fraction of the entry envelope lost: u runs from 0 to 1 whatever the mass does, reaching 1 at
    /// the span's end or the envelope's, and follows whichever of the two is advancing, so that a
    /// wind that strips the envelope in a small part of the phase gets the knots it needs. The
    /// knots are clustered towards u = 1, where the envelope's last thousandths (and the
    /// small-envelope perturbation) or the tip of a giant branch lie ([`clustered_grid`]).
    /// `pulse_period`(Mc, `M_env`), where it is given, is the interpulse period in years at the
    /// core and the envelope of the moment, whose inverse the knots integrate (the thermally
    /// pulsing AGB).
    ///
    /// # Errors
    ///
    /// [`IntegrateEnvelopeError::Stalled`] if the knots cannot end the phase
    /// ([`Builder::envelope_knots`]).
    #[expect(
        clippy::too_many_arguments,
        reason = "a phase's model, span, entry state, knot count and its laws"
    )]
    pub(super) fn envelope_segment(
        &self,
        model: Model,
        start: f64,
        span: Span,
        mass: f64,
        previous: Option<[f64; 3]>,
        knots: usize,
        laws: &EnvelopeLaws<impl Fn(f64) -> f64, impl Fn(f64) -> f64, impl Fn(f64, f64) -> f64>,
        pulse_period: Option<&dyn Fn(f64, f64) -> f64>,
    ) -> Result<EnvelopeBuilt, IntegrateEnvelopeError> {
        let nominal_end = start + span.years();
        let mut segment = Segment {
            model,
            start,
            end: nominal_end,
            coordinate: Coordinate::Linear { nominal_end },
            mass,
            knots: Vec::new(),
            junction: Junction::STEP,
            samples: Vec::new(),
            max_before: [0.0; 2],
        };
        segment.junction = self.junction(&segment, previous, 0.0, mass, start);
        let envelope0 = laws.envelope(start, mass);
        // Written so that a NaN envelope, like a negative one, ends the phase at once.
        let has_envelope = envelope0 > 0.0;
        if !has_envelope {
            return Ok(EnvelopeBuilt {
                ending: Ending::Envelope,
                end: start,
                end_mass: mass,
                segment: None,
            });
        }
        let rates = [0.0, 0.5, 1.0].map(|x| {
            let age = (1.0 - x) * start + x * nominal_end;
            self.wind_rate(&segment.evaluate_at(&self.phys, age, x, mass), age)
        });
        if !significant(rates, span.years(), mass) {
            // Constant mass: the phase ends at its span's end, or where its core reaches the mass.
            if laws.envelope(nominal_end, mass) <= 0.0 {
                segment.end = bisect(start, nominal_end, |age| laws.envelope(age, mass));
                return Ok(EnvelopeBuilt {
                    ending: Ending::Envelope,
                    end: segment.end,
                    end_mass: mass,
                    segment: Some(segment),
                });
            }
            return Ok(EnvelopeBuilt {
                ending: Ending::Nominal,
                end: nominal_end,
                end_mass: mass,
                segment: Some(segment),
            });
        }
        let clock = EnvelopeClock::of(span, nominal_end);
        let delta = clock.delta;
        // The derivative of the mass and the pulses in age at (age, M), the knot there, u and du ÷
        // dt there, and the envelope's probes there. Each core mass is evaluated once, at the
        // age and at the age ± δ, and shared by the envelope, its rate and the progress.
        let derive = |age: f64, m: f64, pulses: f64| -> Derived {
            let coord = segment_coordinate(start, span, age);
            let evaluated = segment.evaluate_at(&self.phys, age, coord, m);
            let rate = self.wind_rate(&evaluated, age);
            let core = if laws.core_in_point {
                evaluated.point.point.core_mass.value()
            } else {
                (laws.core)(age)
            };
            let (core_before, core_after) = ((laws.core)(age - delta), (laws.core)(age + delta));
            let envelope = (laws.shell)(m) - core;
            let x = (laws.progress)(age, core).clamp(0.0, 1.0);
            let y = (1.0 - envelope / envelope0).clamp(0.0, 1.0);
            let advance = ((laws.progress)(age + delta, core_after)
                - (laws.progress)(age - delta, core_before))
                / (2.0 * delta);
            // The envelope δ earlier with the mass the wind has not yet taken, less the envelope
            // δ later with the mass it has.
            let fall = ((laws.shell)(m + rate * delta) - core_before)
                - ((laws.shell)(m - rate * delta) - core_after);
            let lost = fall / (2.0 * delta * envelope0);
            let u = 1.0 - (1.0 - x) * (1.0 - y);
            let du_dt = (1.0 - y) * advance.max(0.0) + (1.0 - x) * lost.max(0.0);
            let pulse_rate = pulse_period.map_or(0.0, |period| 1.0 / period(core, envelope));
            let knot = Knot {
                age,
                mass: m,
                mass_rate: -rate,
                tau: 0.0,
                tau_rate: 0.0,
                pulses,
                pulse_rate,
                luminosity: evaluated.point.point.luminosity.value(),
                radius: evaluated.point.point.radius.value(),
            };
            Derived {
                rates: [-rate, pulse_rate],
                knot,
                phase: evaluated.point.phase,
                u,
                du_dt,
                envelope,
                fall,
            }
        };
        let (built, last_envelope) = self.envelope_knots(&derive, [start, mass], clock, knots)?;
        segment.knots = built;
        Ok(envelope_ending(segment, laws, last_envelope, nominal_end))
    }

    /// The knots of an envelope segment ([`Builder::envelope_segment`]) entered at `entry` = (age,
    /// M) on `n` knots of u, with `derive`(age, M, pulses) giving the rates of M and the pulses in
    /// age, the knot, u, du ÷ dt and the envelope's probes: from the entry to the first knot at
    /// which the envelope is gone or the span has ended. Returns the knots and the envelope left
    /// at the last.
    ///
    /// Each interval is integrated in u from the state reached, with [`Resolution::steps`]
    /// midpoint steps, so no error in u builds up. Past the grid, or where u stalls, the interval
    /// is integrated in age instead (see the comments within).
    ///
    /// # Errors
    ///
    /// [`IntegrateEnvelopeError::Stalled`] if the phase has an envelope left before its span's end
    /// after [`MAX_KNOTS_PAST_GRID`] knots past the grid, at an age past the stall allowance
    /// ([`EnvelopeClock::still_years`]) beyond the last of them.
    fn envelope_knots(
        &self,
        derive: &impl Fn(f64, f64, f64) -> Derived,
        entry: [f64; 2],
        clock: EnvelopeClock,
        n: usize,
    ) -> Result<(Vec<Knot>, f64), IntegrateEnvelopeError> {
        let EnvelopeClock {
            nominal_end,
            years,
            delta,
            ..
        } = clock;
        let grid = clustered_grid(n);
        let steps = self.resolution.steps;
        // d(age, M, pulses) ÷ du at a state, from its rates in age.
        let per_u = |rates: [f64; 2], du_dt: f64| -> [f64; 3] {
            let dt_du = if du_dt * years > 1e-3 {
                1.0 / du_dt
            } else {
                1e3 * years
            };
            [dt_du, rates[0] * dt_du, rates[1] * dt_du]
        };
        let in_u = |state: [f64; 3]| -> [f64; 3] {
            let [age, m, pulses] = state;
            let derived = derive(age, m, pulses);
            per_u(derived.rates, derived.du_dt)
        };
        let mut built: Vec<Knot> = Vec::with_capacity(2 * grid.len());
        let mut last = derive(entry[0], entry[1], 0.0);
        built.push(last.knot);
        let mut target = 1;
        let mut past_grid: u32 = 0;
        // Past `MAX_KNOTS_PAST_GRID` knots, the end of the stall allowance in age.
        let mut stall_end = f64::NAN;
        // The envelope at the last knot is its derivative's own, at the knot's age and mass.
        while !(last.envelope <= 0.0 || last.knot.age >= nominal_end) {
            let knot = last.knot;
            let (slope, u, du_dt) = (last.rates, last.u, last.du_dt);
            // The next value of the grid past the knot's own u, or 1 past the grid's end: each
            // interval starts from the state reached, so no error in u builds up.
            while target < grid.len() && grid[target] <= u {
                target += 1;
            }
            let u1 = grid.get(target).copied().unwrap_or(1.0);
            let extra = built.len() > MAX_KNOT_FACTOR * grid.len();
            // Written so that a NaN u, like a stalled one, takes the integration in age.
            let advancing = u1 > u;
            let mut state = [knot.age, knot.mass, knot.pulses];
            if target >= grid.len() {
                // Past the grid, u approaches 1 only as the envelope's loss slows: the last knot
                // instead goes, in age, to twice the envelope's remaining life at its present
                // rate of loss, or to the span's end if that is sooner, so that the phase ends in
                // one interval. The knot's derivative already holds the envelope and its fall
                // over ± δ at the knot's age and mass.
                if past_grid == MAX_KNOTS_PAST_GRID {
                    stall_end = knot.age + clock.still_years;
                } else if past_grid > MAX_KNOTS_PAST_GRID
                    && (knot.age > stall_end || stall_end.is_nan())
                {
                    return Err(IntegrateEnvelopeError::Stalled {
                        knots: past_grid,
                        phase: last.phase,
                        start: Years::new(entry[0]),
                        entry_mass: SolarMasses::new(entry[1]),
                        age: Years::new(knot.age),
                        mass: SolarMasses::new(knot.mass),
                        envelope: SolarMasses::new(last.envelope),
                        nominal_end: Years::new(nominal_end),
                    });
                }
                past_grid = past_grid.saturating_add(1);
                let age = knot.age;
                let falling = last.fall / (2.0 * delta);
                let reach = if falling > 0.0 {
                    age + 2.0 * last.envelope / falling
                } else {
                    nominal_end
                };
                let next_age = if reach < nominal_end && reach > age {
                    reach
                } else {
                    nominal_end
                };
                state = self.midpoint_in_age(derive, state, slope, next_age);
            } else if !extra && advancing {
                for i in 0..steps {
                    let from = lerp(u, u1, index_fraction(i, steps + 1));
                    let to = lerp(u, u1, index_fraction(i + 1, steps + 1));
                    let step = to - from;
                    let k1 = if i == 0 {
                        per_u(slope, du_dt)
                    } else {
                        in_u(state)
                    };
                    let mid: [f64; 3] = core::array::from_fn(|j| state[j] + 0.5 * step * k1[j]);
                    let k2 = in_u(mid);
                    state = core::array::from_fn(|j| state[j] + step * k2[j]);
                }
            }
            if target < grid.len() && (extra || !advancing || state[0] >= nominal_end) {
                // Past the span's end: the interval again, in age, to the span's end exactly.
                state = self.midpoint_in_age(
                    derive,
                    [knot.age, knot.mass, knot.pulses],
                    slope,
                    nominal_end,
                );
            }
            last = derive(state[0], state[1], state[2]);
            built.push(last.knot);
            target += 1;
        }
        Ok((built, last.envelope))
    }

    /// The state (age, M, pulses) after [`Resolution::steps`] midpoint steps in age from `state`,
    /// whose rates are `slope`, to `end`, with `derive` giving the rates of M and the pulses.
    fn midpoint_in_age(
        &self,
        derive: &impl Fn(f64, f64, f64) -> Derived,
        state: [f64; 3],
        slope: [f64; 2],
        end: f64,
    ) -> [f64; 3] {
        let steps = self.resolution.steps;
        let [start, mut m, mut pulses] = state;
        for i in 0..steps {
            let a = lerp(start, end, index_fraction(i, steps + 1));
            let b = lerp(start, end, index_fraction(i + 1, steps + 1));
            let h = b - a;
            let k1 = if i == 0 {
                slope
            } else {
                derive(a, m, pulses).rates
            };
            let k2 = derive(a + 0.5 * h, m + 0.5 * h * k1[0], pulses + 0.5 * h * k1[1]).rates;
            m += h * k2[0];
            pulses += h * k2[1];
        }
        [end, m, pulses]
    }

    /// Integrates a segment's state from `state0` at `grid[0]` over the knots at `grid` by
    /// midpoint steps, `derive`(s, state) giving the state's derivative in the coordinate s and the
    /// knot record there, and stops after the first knot at which `stop` holds. Returns the knots
    /// and whether it stopped.
    pub(super) fn integrate<const N: usize>(
        &self,
        grid: &[f64],
        state0: [f64; N],
        derive: impl Fn(f64, [f64; N]) -> ([f64; N], Knot),
        stop: impl Fn(&Knot) -> bool,
    ) -> (Vec<Knot>, bool) {
        let steps = self.resolution.steps;
        let mut knots = Vec::with_capacity(grid.len());
        let mut state = state0;
        let (mut slope, knot) = derive(grid[0], state);
        knots.push(knot);
        if stop(&knot) {
            return (knots, true);
        }
        for pair in grid.windows(2) {
            let (s0, s1) = (pair[0], pair[1]);
            for i in 0..steps {
                let a = lerp(s0, s1, index_fraction(i, steps + 1));
                let b = lerp(s0, s1, index_fraction(i + 1, steps + 1));
                let h = b - a;
                let k1 = if i == 0 { slope } else { derive(a, state).0 };
                let mid: [f64; N] = core::array::from_fn(|j| state[j] + 0.5 * h * k1[j]);
                let k2 = derive(a + 0.5 * h, mid).0;
                state = core::array::from_fn(|j| state[j] + h * k2[j]);
            }
            let (next, knot) = derive(s1, state);
            slope = next;
            knots.push(knot);
            if stop(&knot) {
                return (knots, true);
            }
        }
        (knots, false)
    }

    /// The junction of `segment` with the state `previous` ended in: the offsets that make it
    /// continuous at its start, where its coordinate is `coord0` and its mass `mass`, or a declared
    /// step if there is no previous state.
    #[must_use]
    pub(super) fn junction(
        &self,
        segment: &Segment,
        previous: Option<[f64; 3]>,
        coord0: f64,
        mass: f64,
        start: f64,
    ) -> Junction {
        let Some(previous) = previous else {
            return Junction::STEP;
        };
        let here = segment.evaluate_at(&self.phys, start, coord0, mass);
        let p = here.point.point;
        Junction {
            log_l: previous[0] - crate::math::log10(p.luminosity.value()),
            log_r: previous[1] - crate::math::log10(p.radius.value()),
            continuous: true,
        }
    }

    /// log₁₀ L, log₁₀ R and the core mass of `model` at coordinate `coord` with `mass`, at `age`,
    /// with no junction.
    #[must_use]
    pub(super) fn state_of(&self, model: &Model, coord: f64, mass: f64, age: f64) -> [f64; 3] {
        let point = model
            .point(&self.phys, coord, SolarMasses::new(mass), age)
            .point;
        [
            crate::math::log10(point.luminosity.value()),
            crate::math::log10(point.radius.value()),
            point.core_mass.value(),
        ]
    }

    /// log₁₀ L, log₁₀ R and the core mass at the end of `segment`.
    #[must_use]
    fn end_state(&self, segment: &Segment) -> [f64; 3] {
        let end = segment.evaluate(&self.phys, segment.end);
        let p = end.point.point;
        [
            crate::math::log10(p.luminosity.value()),
            crate::math::log10(p.radius.value()),
            p.core_mass.value(),
        ]
    }

    /// The step that ends at `end` with `segment` (none for a phase of no duration, which passes
    /// `previous` on to the next junction) and enters `next`.
    pub(super) fn finish(
        &self,
        segment: Option<Segment>,
        end: f64,
        next: Entry,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let end_state = match (&next, &segment) {
            // Nothing follows a death but the remnant, which needs only the star's last luminosity:
            // a build that keeps no remnant evaluates nothing for it.
            (Entry::Dead(_), Some(_)) if self.keep == Keep::Lifetime => None,
            (_, Some(segment)) => Some(self.end_state(segment)),
            (_, None) => previous,
        };
        Step {
            segment,
            next,
            end,
            end_state,
        }
    }

    /// The wind's rate at an evaluated state at `age`, M☉ per year.
    #[must_use]
    pub(super) fn wind_rate(&self, evaluated: &Evaluated, age: f64) -> f64 {
        let state = StarState::new(evaluated.parts(age));
        wind::rate(self.options.wind(), &state, self.composition, self.eta).value()
    }

    /// The remnant segment of `fate`, from its death on, after maxima of `max_before`, where the
    /// star's last living state was `last` (log₁₀ L, log₁₀ R and the core mass), if it is known.
    #[must_use]
    pub(super) fn remnant_segment(
        &self,
        fate: Fate,
        max_before: [f64; 2],
        last: Option<[f64; 3]>,
        knee: Option<f64>,
    ) -> Segment {
        let age = fate.death.age().value();
        // A white dwarf off the post-AGB bridge fades on MB16's shape to 10 L☉ first, and its
        // cooling law is matched there (ruling 127.1); every other remnant at its star's last
        // luminosity (ruling 46.2).
        let core = WhiteDwarfCore::of(fate.phase);
        let knee = knee.filter(|_| {
            core.is_some() && self.options.remnant() == RemnantRecipe::MandelMuller2020
        });
        let origin = core.map_or(Megayears::ZERO, |core| {
            if knee.is_some() {
                super::post_agb::bridged_origin(
                    self.options.remnant(),
                    core,
                    fate.remnant.mass(),
                    self.phys.z,
                )
            } else {
                white_dwarf::cooling_origin(
                    self.options.remnant(),
                    core,
                    fate.remnant.mass(),
                    last.map(|[log_l, ..]| SolarLuminosities::new(crate::math::exp10(log_l))),
                    self.phys.z,
                )
            }
        });
        let mut segment = Segment {
            model: Model::Remnant {
                phase: fate.phase,
                mass: fate.remnant.mass(),
                birth: age,
                origin,
                knee,
            },
            start: age,
            end: f64::INFINITY,
            coordinate: Coordinate::Linear {
                nominal_end: f64::INFINITY,
            },
            mass: fate.remnant.mass().value(),
            knots: Vec::new(),
            junction: Junction::STEP,
            samples: Vec::new(),
            max_before,
        };
        // A remnant only fades: its largest luminosity and radius are at its birth.
        let p = segment.evaluate(&self.phys, age).point.point;
        segment.samples.push(Sample {
            age,
            max_radius: max_before[0].max(p.radius.value()),
            max_luminosity: max_before[1].max(p.luminosity.value()),
        });
        segment
    }

    /// Fills `segment`'s samples for the monotone maxima: its knots before its end, or evenly
    /// spaced ages without knots, its end, and every local maximum between them, with the running
    /// maxima from `max_before`.
    fn sample(&self, segment: &mut Segment, max_before: [f64; 2]) {
        segment.max_before = max_before;
        let (start, end) = (segment.start, segment.end);
        let mut points: Vec<(f64, f64, f64)> = if segment.knots.len() >= 2 {
            segment
                .knots
                .iter()
                .filter(|k| k.age < end)
                .map(|k| (k.age, k.radius, k.luminosity))
                .collect()
        } else {
            (0..SAMPLES - 1)
                .map(|j| {
                    let age = lerp(start, end, index_fraction(j, SAMPLES));
                    let p = segment.evaluate(&self.phys, age).point.point;
                    (age, p.radius.value(), p.luminosity.value())
                })
                .collect()
        };
        if let Coordinate::Fraction { tau0 } = segment.coordinate
            && matches!(segment.model, Model::MainSequence { .. })
        {
            // The hook's dip can fall between samples, or knots, and hide the peak before it:
            // sample around it too, at its fractional ages at the entry mass, mapped to ages as
            // if τ ran linearly (exactly so without knots).
            let hook = MainSequence::new(SolarMasses::new(segment.mass), self.phys.coeffs)
                .hook_fractions();
            points.extend(
                hook.into_iter()
                    .filter(|&tau| tau > tau0 && tau < 1.0)
                    .map(|tau| {
                        let age = lerp(start, end, (tau - tau0) / (1.0 - tau0));
                        let p = segment.evaluate(&self.phys, age).point.point;
                        (age, p.radius.value(), p.luminosity.value())
                    }),
            );
            points.sort_by(|x, y| x.0.total_cmp(&y.0));
        }
        let last = segment.evaluate(&self.phys, end).point.point;
        points.push((end, last.radius.value(), last.luminosity.value()));
        // A local maximum shows either as a sample above both its neighbours, or as a rise into a
        // sample that is already falling there (the main sequence's hook, for one), which a
        // point just before the sample tells.
        let mut peaks = Vec::new();
        for window in points.windows(3) {
            let [(a, ra, la), (_, rb, lb), (c, rc, lc)] = [window[0], window[1], window[2]];
            if ra < rb && rb >= rc {
                peaks.push(self.peak(segment, a, c, |p| p.radius.value()));
            }
            if la < lb && lb >= lc {
                peaks.push(self.peak(segment, a, c, |p| p.luminosity.value()));
            }
        }
        for pair in points.windows(2) {
            let [(a, ra, la), (b, rb, lb)] = [pair[0], pair[1]];
            if b <= a {
                continue;
            }
            // A rise out of a sample into an interval that ends lower: the peak lies between
            // (the main sequence's hook can hide one so between samples).
            if ra > rb || la > lb {
                let after = segment
                    .evaluate(&self.phys, a + SLOPE_PROBE * (b - a))
                    .point
                    .point;
                if ra > rb && after.radius.value() > ra {
                    peaks.push(self.peak(segment, a, b, |p| p.radius.value()));
                }
                if la > lb && after.luminosity.value() > la {
                    peaks.push(self.peak(segment, a, b, |p| p.luminosity.value()));
                }
            }
            if ra > rb && la > lb {
                continue;
            }
            let before = segment
                .evaluate(&self.phys, b - SLOPE_PROBE * (b - a))
                .point
                .point;
            if ra <= rb && before.radius.value() > rb {
                peaks.push(self.peak(segment, a, b, |p| p.radius.value()));
            }
            if la <= lb && before.luminosity.value() > lb {
                peaks.push(self.peak(segment, a, b, |p| p.luminosity.value()));
            }
        }
        points.extend(peaks);
        points.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut running = max_before;
        segment.samples = points
            .into_iter()
            .map(|(age, r, l)| {
                running = [running[0].max(r), running[1].max(l)];
                Sample {
                    age,
                    max_radius: running[0],
                    max_luminosity: running[1],
                }
            })
            .collect();
    }

    /// The local maximum of `quantity` over [`a`, `b`] of `segment`, by a golden-section search of
    /// [`PEAK_SEARCH`] iterations: its age, radius and luminosity.
    fn peak(
        &self,
        segment: &Segment,
        a: f64,
        b: f64,
        quantity: impl Fn(&PhasePoint) -> f64,
    ) -> (f64, f64, f64) {
        const INVERSE_PHI: f64 = 0.618_033_988_749_894_8;
        // Each probe keeps its whole state, so the peak's needs no evaluation of its own.
        let value = |age: f64| {
            let point = segment.evaluate(&self.phys, age).point.point;
            (quantity(&point), point)
        };
        let (mut lo, mut hi) = (a, b);
        let mut x1 = hi - INVERSE_PHI * (hi - lo);
        let mut x2 = lo + INVERSE_PHI * (hi - lo);
        let (mut f1, mut f2) = (value(x1), value(x2));
        for _ in 0..PEAK_SEARCH {
            if f1.0 < f2.0 {
                lo = x1;
                x1 = x2;
                f1 = f2;
                x2 = lo + INVERSE_PHI * (hi - lo);
                f2 = value(x2);
            } else {
                hi = x2;
                x2 = x1;
                f2 = f1;
                x1 = hi - INVERSE_PHI * (hi - lo);
                f1 = value(x1);
            }
        }
        let (age, p) = if f1.0 < f2.0 { (x2, f2.1) } else { (x1, f1.1) };
        (age, p.radius.value(), p.luminosity.value())
    }
}

/// A phase built on the fractional age of its main sequence.
#[derive(Debug)]
pub(super) struct FractionBuilt {
    pub(super) segment: Segment,
    pub(super) end_mass: f64,
    /// Whether it stopped before τ = 1.
    pub(super) ended: bool,
}

/// The clock of an envelope segment: the end of its span (years since the onset of collapse), the
/// span's length in years, the step in age for rates of change, and the stall allowance.
#[derive(Debug, Clone, Copy)]
struct EnvelopeClock {
    nominal_end: f64,
    years: f64,
    delta: f64,
    /// The stall allowance: how far the age may go past the last of [`MAX_KNOTS_PAST_GRID`]
    /// knots beyond the grid, years. It is the longest the phase's clock can stand still while the
    /// age moves.
    ///
    /// Where the envelope's remaining life comes to a few units in the age's last place, each knot
    /// moves the age by one or two of them. A wind continuous over that step moves the mass by
    /// more than the envelope and ends the phase, so only a core can hold it up. The phase's laws
    /// read the core through its clock, which runs on the track of the star's effective initial
    /// mass and can run ahead of its age. One unit in the clock's last place then spans several of
    /// the age's, and the core stands still over them. The R06 census's longest such stall, 5
    /// knots in core helium burning, is on a clock 5.5 times the age at the phase's start.
    ///
    /// The clock, (1 − x) start + x end in Myr ([`Span::at`]), rounds to within a unit in the last
    /// place of its larger end, at most 2⁻⁵² of it, so it stands still for at most two such units.
    /// That makes 2 × 2⁻⁵² × 10⁶ years of age per Myr of that end, times the span's stretch. Once
    /// the clock moves, the core catches up its growth over the standstill. That is more than the
    /// envelope left, which was under what the core grows in a step. So a phase that has not ended
    /// within this much age has stalled.
    ///
    /// Each knot moves the age by at least one unit in its last place, since a shorter step goes to
    /// the span's end. This allows at most 1 + `still_years` ÷ (2⁻⁵³ t) knots more at age t: 23 at
    /// a clock 5.5 times the age. The stall of plan 06's Risks took 17 more: its age's unit, 2⁻¹⁸
    /// yr, is coarser than 2⁻⁵³ of its age.
    still_years: f64,
}

impl EnvelopeClock {
    /// The clock of a segment whose phase runs over `span` and ends at `nominal_end`, years since
    /// the onset of collapse.
    #[must_use]
    fn of(span: Span, nominal_end: f64) -> Self {
        let largest = span.start.value().abs().max(span.end.value().abs());
        Self {
            nominal_end,
            years: span.years(),
            // A step in age for the progress's and the envelope's rates of change, small against
            // the phase.
            delta: 1e-7 * span.years(),
            still_years: 2.0 * f64::EPSILON * largest * 1e6 * span.stretch,
        }
    }
}

/// The laws of a phase that ends by the loss of its envelope ([`Builder::envelope_segment`]).
pub(super) struct EnvelopeLaws<C, S, P> {
    /// The core mass at an age, M☉.
    pub(super) core: C,
    /// What the core can grow into at a current mass, M☉: the mass itself, or a helium star's core
    /// limit.
    pub(super) shell: S,
    /// The phase's progress x at an age, given the core mass `core`(age) there, 0 at the start of
    /// its span and 1 at its end.
    pub(super) progress: P,
    /// Whether the core mass of the segment's evaluated state is `core`(age), bit for bit, so that
    /// the integration reads it there rather than evaluating it again.
    pub(super) core_in_point: bool,
}

impl<C: Fn(f64) -> f64, S: Fn(f64) -> f64, P: Fn(f64, f64) -> f64> EnvelopeLaws<C, S, P> {
    /// The envelope at `age` with the current mass `m`, M☉: `shell`(m) − `core`(age), positive
    /// while the phase can go on.
    #[must_use]
    pub(super) fn envelope(&self, age: f64, m: f64) -> f64 {
        (self.shell)(m) - (self.core)(age)
    }
}

/// What the derivative of an envelope segment gives at one state.
#[derive(Debug, Clone, Copy)]
struct Derived {
    /// dM ÷ d age and the pulses per year.
    rates: [f64; 2],
    /// The knot at the state.
    knot: Knot,
    /// The phase the state evaluates to, which an [`IntegrateEnvelopeError`] names.
    phase: Phase,
    /// The coordinate u and du ÷ d age.
    u: f64,
    du_dt: f64,
    /// The envelope at the state, M☉.
    envelope: f64,
    /// The envelope δ before the state, with the mass the wind has yet to remove, less the envelope
    /// δ after it, without the mass it has removed: 2δ times the envelope's rate of loss, M☉.
    fall: f64,
}

/// The envelope integrator could not end a phase ([`Builder::envelope_knots`]).
///
/// This is a bug in the phase's laws, never a property of a star, so the build that meets it
/// panics with it ([`Builder::run_from`]). Plan 06's Risks record the one case known: a wind that
/// drops where the mass passes the core.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum IntegrateEnvelopeError {
    /// The phase took [`MAX_KNOTS_PAST_GRID`] knots past its grid, and more past the stall
    /// allowance of age beyond them ([`EnvelopeClock::still_years`]), and still had an envelope before
    /// the end of its span.
    Stalled {
        /// The knots it took past its grid.
        knots: u32,
        /// The phase the last knot evaluates to.
        phase: Phase,
        /// When it started, since the onset of collapse.
        start: Years,
        /// The mass it started with.
        entry_mass: SolarMasses,
        /// The last knot's age, since the onset of collapse.
        age: Years,
        /// The last knot's mass.
        mass: SolarMasses,
        /// The envelope left at the last knot.
        envelope: SolarMasses,
        /// The end of the phase's span, since the onset of collapse.
        nominal_end: Years,
    },
}

impl fmt::Display for IntegrateEnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stalled {
                knots,
                phase,
                start,
                entry_mass,
                age,
                mass,
                envelope,
                nominal_end,
            } => write!(
                f,
                "the {phase:?} phase entered at {:e} yr with {} M☉ still had an envelope after \
                 {knots} knots past its grid: {:e} M☉ of {} M☉ at {:e} yr, {:e} yr before its \
                 span's end",
                start.value(),
                entry_mass.value(),
                envelope.value(),
                mass.value(),
                age.value(),
                nominal_end.value() - age.value()
            ),
        }
    }
}

impl Error for IntegrateEnvelopeError {}

/// A phase built on its clock.
pub(super) struct EnvelopeBuilt {
    pub(super) ending: Ending,
    pub(super) end: f64,
    /// The mass at the end, M☉.
    pub(super) end_mass: f64,
    pub(super) segment: Option<Segment>,
}

/// How a phase on its clock ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ending {
    /// At the end of its span.
    Nominal,
    /// Early, at the root of its envelope (or of the core's limit).
    Envelope,
}

/// How an envelope `segment` whose knots are built ends: at the root of its envelope
/// ([`EnvelopeLaws::envelope`]) on the knot interpolant if its last knot has none left
/// (`last_envelope`, the envelope there), or at the end of its span, `nominal_end`.
fn envelope_ending(
    mut segment: Segment,
    laws: &EnvelopeLaws<impl Fn(f64) -> f64, impl Fn(f64) -> f64, impl Fn(f64, f64) -> f64>,
    last_envelope: f64,
    nominal_end: f64,
) -> EnvelopeBuilt {
    let n = segment.knots.len();
    let last = segment.knots[n - 1];
    if last_envelope <= 0.0 {
        let a = segment.knots[n - 2].age;
        let end = bisect(a, last.age, |age| {
            let (_, m) = segment.coordinate_and_mass(age);
            laws.envelope(age, m)
        });
        segment.end = end;
        let (_, end_mass) = segment.coordinate_and_mass(end);
        EnvelopeBuilt {
            ending: Ending::Envelope,
            end,
            end_mass,
            segment: Some(segment),
        }
    } else {
        segment.end = nominal_end;
        EnvelopeBuilt {
            ending: Ending::Nominal,
            end: nominal_end,
            end_mass: last.mass,
            segment: Some(segment),
        }
    }
}

/// Whether a phase's wind is significant: the largest of `rates` (M☉ per year) over `years`
/// removes at least [`NEGLIGIBLE_LOSS`] of `mass`.
#[must_use]
fn significant(rates: [f64; 3], years: f64, mass: f64) -> bool {
    let largest = rates.into_iter().fold(0.0, f64::max);
    largest * years >= NEGLIGIBLE_LOSS * mass
}

/// The linear coordinate at `age` of a segment from `start` over the clock `span`: what
/// [`Segment::coordinate_and_mass`] gives it.
#[must_use]
pub(super) fn segment_coordinate(start: f64, span: Span, age: f64) -> f64 {
    let years = span.years();
    if years > 0.0 {
        ((age - start) / years).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// k ÷ (n − 1) as an `f64`, exact for the counts a track uses.
#[must_use]
fn index_fraction(k: usize, n: usize) -> f64 {
    let (k, n) = (
        u32::try_from(k).expect("a knot index fits in 32 bits"),
        u32::try_from(n - 1).expect("a knot count fits in 32 bits"),
    );
    f64::from(k) / f64::from(n)
}

/// (1 − x) a + x b, exact at both ends.
#[must_use]
fn lerp(a: f64, b: f64, x: f64) -> f64 {
    (1.0 - x) * a + x * b
}

/// `n` knots of the coordinate u from 0 to 1, closer towards 1 as the cube of the distance to
/// it: 1 − (1 − k ÷ (n − 1))³. With 16 knots the first interval is 0.19 of the phase and the last
/// 3 × 10⁻⁴; with 32, 0.09 and 3 × 10⁻⁵.
#[must_use]
pub(super) fn clustered_grid(n: usize) -> Vec<f64> {
    (0..n)
        .map(|k| {
            if k == n - 1 {
                1.0
            } else {
                let rest = 1.0 - index_fraction(k, n);
                1.0 - rest * rest * rest
            }
        })
        .collect()
}

/// The root in [`a`, `b`] of `f`, positive at `a` and not at `b`, by [`BISECTIONS`] bisections:
/// the upper end of the last bracket, where `f` is no longer positive, within 2⁻⁴⁰ (b − a) of the
/// root.
#[must_use]
fn bisect(a: f64, b: f64, f: impl Fn(f64) -> f64) -> f64 {
    let (mut lo, mut hi) = (a, b);
    for _ in 0..BISECTIONS {
        let mid = lo + 0.5 * (hi - lo);
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;

    use hyperion_testkit::float::bits;

    use super::*;
    use crate::stellar::draws::StarDraws;

    /// The laws of an envelope phase for [`Builder::envelope_knots`] alone: a core of `core`(age)
    /// M☉ under a shell of the mass itself, and a wind of `rate`(M) M☉ per year. `u`, du ÷ dt and
    /// the fall are [`Builder::envelope_segment`]'s, with the phase's progress linear in age from
    /// `phase_start` over the clock's span. `calls` counts the derivative's evaluations.
    struct Laws<'a, C, R> {
        core: C,
        rate: R,
        phase_start: f64,
        envelope0: f64,
        clock: EnvelopeClock,
        calls: &'a Cell<usize>,
    }

    impl<C: Fn(f64) -> f64, R: Fn(f64) -> f64> Laws<'_, C, R> {
        fn derive(&self, age: f64, m: f64, pulses: f64) -> Derived {
            self.calls.set(self.calls.get() + 1);
            // The most any test here needs is some 360: a guard that failed would otherwise push
            // knots until the machine ran out of memory, as the stall once did.
            assert!(
                self.calls.get() <= 10_000,
                "the envelope integrator ran past its guard"
            );
            let EnvelopeClock { years, delta, .. } = self.clock;
            let rate = (self.rate)(m);
            let envelope = m - (self.core)(age);
            let x = ((age - self.phase_start) / years).clamp(0.0, 1.0);
            let y = (1.0 - envelope / self.envelope0).clamp(0.0, 1.0);
            let fall = ((m + rate * delta) - (self.core)(age - delta))
                - ((m - rate * delta) - (self.core)(age + delta));
            let lost = fall / (2.0 * delta * self.envelope0);
            Derived {
                rates: [-rate, 0.0],
                knot: Knot {
                    age,
                    mass: m,
                    mass_rate: -rate,
                    tau: 0.0,
                    tau_rate: 0.0,
                    pulses,
                    pulse_rate: 0.0,
                    luminosity: 1.0,
                    radius: 1.0,
                },
                phase: Phase::EarlyAgb,
                u: 1.0 - (1.0 - x) * (1.0 - y),
                du_dt: (1.0 - y) / years + (1.0 - x) * lost.max(0.0),
                envelope,
                fall,
            }
        }

        /// The knots from `entry` = (age, M) on a grid of one knot (u = 1), so that every
        /// interval is past it.
        fn knots(&self, entry: [f64; 2]) -> Result<(Vec<Knot>, f64), IntegrateEnvelopeError> {
            let comp = Composition::SOLAR;
            let coeffs = ZCoeffs::new(comp.z_fit());
            let builder = Builder::new(
                &coeffs,
                &comp,
                TrackOptions::default(),
                ReimersEta::new(0.5),
                RemnantDraws::of(&StarDraws::median()),
                Resolution::GENERATOR,
                Keep::Lifetime,
            );
            builder.envelope_knots(&|t, m, p| self.derive(t, m, p), entry, self.clock, 1)
        }
    }

    /// 1.5 × 2³⁴ yr (2.6 × 10¹⁰), the age of the stalled star of plan 06's Risks to its order of
    /// magnitude, where a unit in the age's last place is [`AGE_UNIT`].
    const AGE: f64 = 25_769_803_776.0;

    /// A unit in the last place of [`AGE`] and of the ages just past it, 2⁻¹⁸ yr.
    const AGE_UNIT: f64 = 1.0 / 262_144.0;

    /// The clock of a phase 10⁷ yr long that ends 10⁵ yr after [`AGE`], on a clock that runs
    /// `ahead` times the age at its end.
    fn clock_ahead(ahead: f64) -> EnvelopeClock {
        let nominal_end = AGE + 1e5;
        let end = ahead * nominal_end / 1e6;
        EnvelopeClock::of(
            Span::new(Megayears::new(end - 10.0), Megayears::new(end)),
            nominal_end,
        )
    }

    /// The stall of plan 06's Risks (found by P11.T4.h), to its orders of magnitude: 2⁻⁴⁸ M☉
    /// (3.6 × 10⁻¹⁵) of envelope on a core of 0.5 M☉ at [`AGE`], on a clock 5 times the age. Above
    /// the core the wind is 2.7 × 10⁻⁹ M☉ per year, which would take the envelope in 0.35 of a unit
    /// in the age's last place; below it the wind is a millionth of that. So the midpoint of each
    /// step past the grid sees the weak wind while its knot sees the strong one: the mass never
    /// moves, each knot moves the age by one unit, and without the guard the loop would push some
    /// 10¹⁰ knots before the span's end. The guard stops it after [`MAX_KNOTS_PAST_GRID`] knots
    /// and the stall allowance, at 8 evaluations of the derivative per knot.
    #[test]
    fn a_wind_that_drops_at_the_core_stalls_and_trips_the_guard() {
        let core = 0.5;
        let envelope = 1.0 / 281_474_976_710_656.0;
        let above = 2.0 * envelope / (0.7 * AGE_UNIT);
        let clock = clock_ahead(5.0);
        let calls = Cell::new(0);
        let laws = Laws {
            core: |_| core,
            rate: |m: f64| if m >= core { above } else { 1e-6 * above },
            phase_start: clock.nominal_end - clock.years,
            envelope0: envelope,
            clock,
            calls: &calls,
        };
        let entry_mass = core + envelope;
        let error = laws
            .knots([AGE, entry_mass])
            .expect_err("the stall trips the guard");
        let IntegrateEnvelopeError::Stalled {
            knots,
            phase,
            start,
            entry_mass: reported_mass,
            age,
            mass,
            envelope: left,
            nominal_end,
        } = error;
        assert_eq!(phase, Phase::EarlyAgb);
        let (age, nominal_end) = (age.value(), nominal_end.value());
        assert_eq!(bits(start.value()), bits(AGE));
        assert_eq!(bits(reported_mass.value()), bits(entry_mass));
        assert_eq!(bits(mass.value()), bits(entry_mass), "the mass never moves");
        assert_eq!(bits(left.value()), bits(envelope));
        assert_eq!(
            bits(age - AGE),
            bits(f64::from(knots) * AGE_UNIT),
            "one unit a knot"
        );
        // The first knot past the allowance, one unit a knot from the last of the 28.
        let allowance = clock.still_years / AGE_UNIT;
        assert!((14.9..15.1).contains(&allowance), "{allowance} units");
        assert_eq!(
            bits(f64::from(knots - MAX_KNOTS_PAST_GRID - 1)),
            bits(allowance.floor()),
            "{knots} knots"
        );
        assert!(
            (nominal_end - age) / AGE_UNIT > 1e10,
            "the span's end is {} knots away",
            (nominal_end - age) / AGE_UNIT
        );
        let calls_per_knot = 2 * STEPS_PER_KNOT;
        let knots = usize::try_from(knots).expect("a few knots");
        assert_eq!(calls.get(), 1 + knots * calls_per_knot);
        let text = error.to_string();
        assert!(
            text.starts_with("the EarlyAgb phase entered at ")
                && text.contains(&format!(
                    " still had an envelope after {knots} knots past its"
                )),
            "{text}"
        );
    }

    /// The same star with a wind continuous at the core ends its phase in one knot past the
    /// grid, as almost every current star does.
    #[test]
    fn a_wind_continuous_at_the_core_ends_the_phase_in_one_knot() {
        let core = 0.5;
        let envelope = 1.0 / 281_474_976_710_656.0;
        let above = 2.0 * envelope / (0.7 * AGE_UNIT);
        let clock = clock_ahead(5.0);
        let calls = Cell::new(0);
        let laws = Laws {
            core: |_| core,
            rate: |_| above,
            phase_start: clock.nominal_end - clock.years,
            envelope0: envelope,
            clock,
            calls: &calls,
        };
        let (knots, left) = laws.knots([AGE, core + envelope]).expect("the phase ends");
        assert_eq!(knots.len(), 2);
        assert!(left <= 0.0, "{left}");
    }

    /// The rounding the allowance is for, as in the R06 census's core-helium-burning stalls but
    /// longer: no wind to speak of, and a core that grows at 2 × 10⁻¹⁰ M☉ per year but is read
    /// through a clock 13.5 times the age, which stands still for 40 units in the age's last
    /// place. On 7 units of its last place of envelope the core stands still for some 40 knots
    /// past the grid, more than [`MAX_KNOTS_PAST_GRID`], and then ends the phase within the
    /// allowance.
    #[test]
    fn a_core_read_through_a_coarse_clock_stalls_and_ends_within_the_allowance() {
        let growth = 2e-10;
        let quantum = 40.0 * AGE_UNIT;
        let clock = clock_ahead(13.5);
        assert!(
            clock.still_years >= quantum,
            "{} against {quantum}",
            clock.still_years
        );
        let core = |age: f64| 0.48 + growth * ((age - AGE) / quantum).floor() * quantum;
        let envelope = 7.0 / 18_014_398_509_481_984.0;
        let calls = Cell::new(0);
        let laws = Laws {
            core,
            rate: |_| 1e-14,
            phase_start: clock.nominal_end - clock.years,
            envelope0: envelope,
            clock,
            calls: &calls,
        };
        let (knots, left) = laws
            .knots([AGE, core(AGE) + envelope])
            .expect("the phase ends");
        let past = u32::try_from(knots.len() - 1).expect("a few knots");
        assert!(past > MAX_KNOTS_PAST_GRID, "{past} knots past the grid");
        assert!(left <= 0.0, "{left}");
    }

    /// The intervals that fall short which a wind in proportion to the envelope takes, at
    /// `steps` midpoint steps an interval, from an envelope of [`MAX_INITIAL_MASS`] to 2⁻⁵⁴ of a
    /// core of [`MIN_EVALUATED_MASS`]: [`MAX_KNOTS_PAST_GRID`]'s derivation.
    ///
    /// [`MAX_INITIAL_MASS`]: super::super::MAX_INITIAL_MASS
    fn proportional_intervals(steps: usize) -> u32 {
        let s = f64::from(u32::try_from(steps).expect("a few steps"));
        let one = 1.0 - 2.0 / s + 2.0 / (s * s);
        let share: f64 = (0..steps).map(|_| one).product();
        let rounding = 0.25 * f64::EPSILON * MIN_EVALUATED_MASS;
        let mut envelope = super::super::MAX_INITIAL_MASS.value();
        let mut intervals = 0;
        while envelope > rounding {
            envelope *= share;
            intervals += 1;
        }
        intervals
    }

    #[test]
    fn the_past_grid_bound_is_a_proportional_wind_s_way_to_the_rounding_and_one_more() {
        assert_eq!(proportional_intervals(STEPS_PER_KNOT), 27);
        assert!(
            proportional_intervals(Resolution::GENERATOR.doubled().steps)
                <= proportional_intervals(STEPS_PER_KNOT)
        );
        assert_eq!(
            MAX_KNOTS_PAST_GRID,
            proportional_intervals(STEPS_PER_KNOT) + 1
        );
    }

    /// The bound's own case: a wind in proportion to the envelope, from 150 M☉ on a core of
    /// 10⁻³ M☉, on a clock that never stands still, ends its phase within
    /// [`MAX_KNOTS_PAST_GRID`] knots past the grid. It takes 26, where the mass rounds to the core,
    /// so the bound is within two of tight.
    #[test]
    fn a_wind_in_proportion_to_the_envelope_ends_within_the_bound() {
        let core = MIN_EVALUATED_MASS;
        let mass = super::super::MAX_INITIAL_MASS.value();
        // An e-folding time of 2 yr, so that the fall's probes, ± 1 yr, resolve the envelope down
        // to the core's rounding.
        let per_year = 0.5;
        let clock = EnvelopeClock {
            nominal_end: 1e6 + 1e7,
            years: 1e7,
            delta: 1.0,
            still_years: 0.0,
        };
        let calls = Cell::new(0);
        let laws = Laws {
            core: |_| core,
            rate: |m: f64| per_year * (m - core),
            phase_start: 1e6,
            envelope0: mass - core,
            clock,
            calls: &calls,
        };
        let (knots, left) = laws.knots([1e6, mass]).expect("the phase ends");
        let past = u32::try_from(knots.len() - 1).expect("a few knots");
        assert!(
            (MAX_KNOTS_PAST_GRID - 2..=MAX_KNOTS_PAST_GRID).contains(&past),
            "{past} knots past the grid"
        );
        assert!(left <= 0.0, "{left}");
    }
}
