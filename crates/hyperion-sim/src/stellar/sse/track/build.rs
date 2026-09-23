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

use crate::stellar::{Composition, StarState};
use crate::units::{Megayears, SolarMasses};

use super::super::PhasePoint;
use super::super::agb::ThermallyPulsingAgb;
use super::super::coeffs::ZCoeffs;
use super::super::gb::{self, GiantBranch};
use super::super::helium::HeliumStar;
use super::super::wind::{self, ReimersEta};
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
    MainSequence {
        mass: f64,
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
    ThermallyPulsingAgb {
        phase: Box<ThermallyPulsingAgb>,
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
    Dead(Fate),
}

/// One phase built: its segment (none for a phase of no duration), what follows, and the state it
/// ends in.
pub(super) struct Step {
    pub(super) segment: Option<Segment>,
    pub(super) next: Entry,
    pub(super) end: f64,
    /// log₁₀ L, log₁₀ R and the core mass at the end, for the next junction; `None` if the next
    /// junction is a declared step.
    pub(super) end_state: Option<[f64; 3]>,
}

/// The track integrator for one star.
#[derive(Debug)]
pub(crate) struct Builder<'a> {
    pub(super) phys: Physics<'a>,
    composition: &'a Composition,
    pub(super) options: TrackOptions,
    eta: ReimersEta,
    pub(super) resolution: Resolution,
    keep: Keep,
    /// The core at helium ignition of an `M_HeF` star, the lightest helium star that burns helium
    /// (see [`Builder::helium_main_sequence`]).
    pub(super) lightest_helium_star: f64,
}

impl<'a> Builder<'a> {
    /// A builder for a star of `composition` (whose coefficients are `coeffs`) and Reimers `eta`.
    #[must_use]
    pub(crate) fn new(
        coeffs: &'a ZCoeffs,
        composition: &'a Composition,
        options: TrackOptions,
        eta: ReimersEta,
        resolution: Resolution,
        keep: Keep,
    ) -> Self {
        let m_hef = coeffs.m_hef();
        let lightest = GiantBranch::new(m_hef, coeffs).core_mass(gb::l_hei(m_hef, coeffs));
        Self {
            phys: Physics {
                coeffs,
                z: composition.z_fit(),
                remnant: options.remnant(),
            },
            composition,
            options,
            eta,
            resolution,
            keep,
            lightest_helium_star: lightest.value(),
        }
    }

    /// The track of a star of initial mass `m0` (M☉), built to the segment that holds `age_max`,
    /// or to the remnant.
    #[must_use]
    pub(crate) fn run(&self, m0: f64, age_max: Option<f64>) -> Outcome {
        let mut segments: Vec<Segment> = Vec::new();
        let mut entry = Entry::MainSequence { mass: m0 };
        let mut start = 0.0;
        let mut previous: Option<[f64; 3]> = None;
        let mut max_before = [0.0_f64; 2];
        loop {
            if let Entry::Dead(fate) = entry {
                if self.keep == Keep::Track {
                    segments.push(self.remnant_segment(fate, max_before));
                }
                return Outcome {
                    segments,
                    fate: Some(fate),
                    built_until: f64::INFINITY,
                };
            }
            let step = self.phase(entry, start, previous);
            if let Some(mut segment) = step.segment
                && self.keep == Keep::Track
            {
                self.sample(&mut segment, max_before);
                if let Some(last) = segment.samples.last() {
                    max_before = [last.max_radius, last.max_luminosity];
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
    fn phase(&self, entry: Entry, start: f64, previous: Option<[f64; 3]>) -> Step {
        match entry {
            Entry::MainSequence { mass } => self.main_sequence(start, mass, previous),
            Entry::HertzsprungGap { m0, mass } => self.hertzsprung_gap(start, m0, mass, previous),
            Entry::FirstGiantBranch { m0, mass } => self.giant_branch(start, m0, mass, previous),
            Entry::Flash { mass } => self.flash(start, mass, previous),
            Entry::CoreHeliumBurning { m0, mass } => {
                self.core_helium_burning(start, m0, mass, previous)
            }
            Entry::EarlyAgb { m0, mass } => self.early_agb(start, m0, mass, previous),
            Entry::ThermallyPulsingAgb {
                phase,
                mc_bagb,
                mass,
            } => self.pulsing_agb(start, &phase, mc_bagb, mass, previous),
            Entry::HeliumMainSequence { mass, tau0 } => {
                self.helium_main_sequence(start, mass, tau0, previous)
            }
            Entry::HeliumShellBurning { star, clock0, mass } => {
                self.helium_shell_burning(start, &star, clock0, mass, previous)
            }
            Entry::Dead(fate) => Step {
                segment: None,
                next: Entry::Dead(fate),
                end: start,
                end_state: None,
            },
        }
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
            let evaluated = segment.evaluate_at(&self.phys, age, tau, m);
            let rate = self.wind_rate(&evaluated, age);
            let lifetime = duration(m.max(MIN_EVALUATED_MASS));
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
    /// that ends at the end of its span or when its envelope `envelope`(age, M) (M☉, positive while
    /// the phase can go on) is gone, whichever comes first.
    ///
    /// With mass loss the knots sit at fixed values of u = 1 − (1 − x)(1 − y), with x =
    /// `progress`(age) the phase's progress, 0 at its start and 1 at the end of its span, and y the
    /// fraction of the entry envelope lost: u runs from 0 to 1 whatever the mass does, reaching 1 at
    /// the span's end or the envelope's, and follows whichever of the two is advancing, so that a
    /// wind that strips the envelope in a small part of the phase gets the knots it needs. The
    /// knots are clustered towards u = 1, where the envelope's last thousandths (and the
    /// small-envelope perturbation) or the tip of a giant branch lie ([`clustered_grid`]).
    /// `pulse_period`(age, M), where it is given, is the interpulse period in years, whose inverse
    /// the knots integrate (the thermally pulsing AGB).
    #[expect(
        clippy::too_many_arguments,
        reason = "a phase's model, span, entry state, knot count and three laws"
    )]
    pub(super) fn envelope_segment(
        &self,
        model: Model,
        start: f64,
        span: Span,
        mass: f64,
        previous: Option<[f64; 3]>,
        knots: usize,
        progress: impl Fn(f64) -> f64,
        envelope: impl Fn(f64, f64) -> f64,
        pulse_period: Option<&dyn Fn(f64, f64) -> f64>,
    ) -> EnvelopeBuilt {
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
        let envelope0 = envelope(start, mass);
        // Written so that a NaN envelope, like a negative one, ends the phase at once.
        let has_envelope = envelope0 > 0.0;
        if !has_envelope {
            return EnvelopeBuilt {
                ending: Ending::Envelope,
                end: start,
                end_mass: mass,
                segment: None,
            };
        }
        let rates = [0.0, 0.5, 1.0].map(|x| {
            let age = (1.0 - x) * start + x * nominal_end;
            self.wind_rate(&segment.evaluate_at(&self.phys, age, x, mass), age)
        });
        if !significant(rates, span.years(), mass) {
            // Constant mass: the phase ends at its span's end, or where its core reaches the mass.
            if envelope(nominal_end, mass) <= 0.0 {
                segment.end = bisect(start, nominal_end, |age| envelope(age, mass));
                return EnvelopeBuilt {
                    ending: Ending::Envelope,
                    end: segment.end,
                    end_mass: mass,
                    segment: Some(segment),
                };
            }
            return EnvelopeBuilt {
                ending: Ending::Nominal,
                end: nominal_end,
                end_mass: mass,
                segment: Some(segment),
            };
        }
        let clock = EnvelopeClock {
            nominal_end,
            years: span.years(),
            // A step in age for the progress's and the envelope's rates of change, small against
            // the phase.
            delta: 1e-7 * span.years(),
        };
        let delta = clock.delta;
        // The derivative of the mass and the pulses in age at (age, M), the knot there, and u and
        // du ÷ dt there.
        let derive = |age: f64, m: f64, pulses: f64| -> ([f64; 2], Knot, f64, f64) {
            let coord = segment_coordinate(start, span, age);
            let evaluated = segment.evaluate_at(&self.phys, age, coord, m);
            let rate = self.wind_rate(&evaluated, age);
            let x = progress(age).clamp(0.0, 1.0);
            let y = (1.0 - envelope(age, m) / envelope0).clamp(0.0, 1.0);
            let advance = (progress(age + delta) - progress(age - delta)) / (2.0 * delta);
            let lost = (envelope(age - delta, m + rate * delta)
                - envelope(age + delta, m - rate * delta))
                / (2.0 * delta * envelope0);
            let u = 1.0 - (1.0 - x) * (1.0 - y);
            let du_dt = (1.0 - y) * advance.max(0.0) + (1.0 - x) * lost.max(0.0);
            let pulse_rate = pulse_period.map_or(0.0, |period| 1.0 / period(age, m));
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
            ([-rate, pulse_rate], knot, u, du_dt)
        };
        let built = self.envelope_knots(&derive, &envelope, [start, mass], clock, knots);
        segment.knots = built;
        envelope_ending(segment, &envelope, nominal_end)
    }

    /// The knots of an envelope segment ([`Builder::envelope_segment`]) entered at `entry` = (age,
    /// M) on `n` knots of u, with `derive`(age, M, pulses) giving the rates of M and the pulses in
    /// age, the knot, u and du ÷ dt, and `envelope`(age, M) the envelope left: from the entry to the
    /// first knot at which the envelope is gone or the span has ended.
    ///
    /// Each interval is integrated in u from the state reached, with [`Resolution::steps`]
    /// midpoint steps, so no error in u builds up. Past the grid, or where u stalls, the interval
    /// is integrated in age instead (see the comments within).
    fn envelope_knots(
        &self,
        derive: &impl Fn(f64, f64, f64) -> ([f64; 2], Knot, f64, f64),
        envelope: &impl Fn(f64, f64) -> f64,
        entry: [f64; 2],
        clock: EnvelopeClock,
        n: usize,
    ) -> Vec<Knot> {
        let EnvelopeClock {
            nominal_end,
            years,
            delta,
        } = clock;
        let ended = |knot: &Knot| envelope(knot.age, knot.mass) <= 0.0 || knot.age >= nominal_end;
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
            let (rates, _, _, du_dt) = derive(age, m, pulses);
            per_u(rates, du_dt)
        };
        let mut built: Vec<Knot> = Vec::with_capacity(2 * grid.len());
        let (mut slope, first, mut u, mut du_dt) = derive(entry[0], entry[1], 0.0);
        built.push(first);
        let mut target = 1;
        while !ended(&built[built.len() - 1]) {
            let knot = built[built.len() - 1];
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
                // one interval.
                let [age, mass, _] = state;
                let (rate, _, _, _) = derive(age, mass, knot.pulses);
                let left = envelope(age, mass);
                let falling = (envelope(age - delta, mass - rate[0] * delta)
                    - envelope(age + delta, mass + rate[0] * delta))
                    / (2.0 * delta);
                let reach = if falling > 0.0 {
                    age + 2.0 * left / falling
                } else {
                    nominal_end
                };
                let next_age = if reach < nominal_end && reach > age {
                    reach
                } else {
                    nominal_end
                };
                state = self.midpoint_in_age(derive, [age, mass, knot.pulses], slope, next_age);
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
            let (next_slope, next, next_u, next_du_dt) = derive(state[0], state[1], state[2]);
            built.push(next);
            (slope, u, du_dt) = (next_slope, next_u, next_du_dt);
            target += 1;
        }
        built
    }

    /// The state (age, M, pulses) after [`Resolution::steps`] midpoint steps in age from `state`,
    /// whose rates are `slope`, to `end`, with `derive` giving the rates of M and the pulses.
    fn midpoint_in_age(
        &self,
        derive: &impl Fn(f64, f64, f64) -> ([f64; 2], Knot, f64, f64),
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
                derive(a, m, pulses).0
            };
            let k2 = derive(a + 0.5 * h, m + 0.5 * h * k1[0], pulses + 0.5 * h * k1[1]).0;
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
            (Entry::Dead(_), _) => None,
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

    /// The remnant segment of `fate`, from its death on, after maxima of `max_before`.
    fn remnant_segment(&self, fate: Fate, max_before: [f64; 2]) -> Segment {
        let age = fate.death.age().value();
        let mut segment = Segment {
            model: Model::Remnant {
                phase: fate.phase,
                mass: fate.remnant.mass(),
                birth: age,
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
            let ordered = b > a;
            if !ordered || (ra > rb && la > lb) {
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
        let value = |age: f64| quantity(&segment.evaluate(&self.phys, age).point.point);
        let (mut lo, mut hi) = (a, b);
        let mut x1 = hi - INVERSE_PHI * (hi - lo);
        let mut x2 = lo + INVERSE_PHI * (hi - lo);
        let (mut f1, mut f2) = (value(x1), value(x2));
        for _ in 0..PEAK_SEARCH {
            if f1 < f2 {
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
        let age = if f1 < f2 { x2 } else { x1 };
        let p = segment.evaluate(&self.phys, age).point.point;
        (age, p.radius.value(), p.luminosity.value())
    }
}

/// A phase built on the fractional age of its main sequence.
pub(super) struct FractionBuilt {
    pub(super) segment: Segment,
    pub(super) end_mass: f64,
    /// Whether it stopped before τ = 1.
    pub(super) ended: bool,
}

/// The clock of an envelope segment: the end of its span (years since the onset of collapse), the
/// span's length in years, and the step in age for rates of change.
#[derive(Debug, Clone, Copy)]
struct EnvelopeClock {
    nominal_end: f64,
    years: f64,
    delta: f64,
}

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

/// How an envelope `segment` whose knots are built ends: at the root of its `envelope`(age, M) on
/// the knot interpolant if its last knot has none left, or at the end of its span, `nominal_end`.
fn envelope_ending(
    mut segment: Segment,
    envelope: &impl Fn(f64, f64) -> f64,
    nominal_end: f64,
) -> EnvelopeBuilt {
    let n = segment.knots.len();
    let last = segment.knots[n - 1];
    if envelope(last.age, last.mass) <= 0.0 {
        let a = segment.knots[n - 2].age;
        let end = bisect(a, last.age, |age| {
            let (_, m) = segment.coordinate_and_mass(age);
            envelope(age, m)
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
