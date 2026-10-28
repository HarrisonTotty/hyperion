//! The particle spray (plan 10, P10.T5): the tracers whose orbits measure a stream's track.
//!
//! A stream's tube follows its debris, not its progenitor's orbit (brainstorm, "Streams and
//! accreted structure": "the track is measured"). About 2,000 tracers, half from each Lagrange
//! point, are released over the stripping time and integrated to the epoch, and P10.T6 reduces
//! them to the tube table. Everything here is a pure function of the seed, the stream's
//! specification and the galaxy's potential.
//!
//! # The release schedule
//!
//! The tube's mass `M` left the progenitor over the stripping time `T_s` as its [`MassLoss`]
//! says (P10.T5.a): at a constant rate for a living globular; at a constant rate until it
//! dissolved for an orphan, and nothing since, which leaves the gap; in pulses at the pericentres
//! for a dwarf, a share `f` of its bound mass at each (Design note 11). [`ReleaseSchedule`]
//! samples that history stratified in mass. Release `i` of `N` (`N` = [`TRACERS_PER_ARM`] in the
//! generator's spray, [`SprayParams`]) is at
//! the time when the mass lost reaches the share `uᵢ = (i + Uᵢ) ÷ N` of the tube's, `Uᵢ` uniform
//! on `stream.spray.release`, rounded to the nearest boundary of the stream's fixed step
//! ([`SphericalOrbit::fixed_step`](super::SphericalOrbit::fixed_step) over `T_s`), so that every
//! tracer shares the progenitor's time grid (Design note 4); an orphan's are rounded no later
//! than the last boundary before its dissolution. Each release frees one tracer from each
//! Lagrange point, each weighing half its stratum's mass, `M ÷ 2N`: the mass-loss rate times
//! the stratum's interval. So the weights sum to the tube's mass, rate × `T_s` for a living
//! globular exactly as P10.T3.a measured it (the desktop's ruling 5 on plan 10's re-validation,
//! issue #123, 2026-10-27).
//!
//! **A dwarf's pulses** fall on the pericentres of its progenitor's integrated orbit, which the
//! caller passes as step boundaries. The specification counts its pericentres in the spherical
//! approximation, whose radial period differs from the integrated orbit's in the disc's
//! flattened potential (P10.T3's cross-check read mean angular frequencies of 0.87–1.00 of the
//! quadrature's), so after a few periods its pericentre times could fall near an integrated
//! apocentre. The `k`-th most recent of the specification's pulses carries `f (1 − f)^−(k+1)` of
//! the core's mass, as Design note 11's stripping leaves it; if the integrated orbit passes fewer
//! pericentres within the stripping time, those it passes share the tube in the same proportions,
//! and if it passes none, the spherical approximation's pericentres stand in. A pulse is
//! released at its pericentre's boundary, as the note's "at each pericentre" has it.
//!
//! **The bound mass.** When release `i` leaves, the progenitor holds its mass at the epoch plus
//! what the tube lost after it, `m₀ + M (1 − uᵢ)`: a living globular's present mass, a dwarf's
//! core's, and nothing for an orphan, whose cluster shrinks to nothing as it dissolves.
//! [`ReleaseSchedule::bound_mass`] gives the same at a step boundary.

use std::num::NonZeroU32;

use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::{Seconds, SolarMasses};

use super::MassLoss;
use super::orbit::{BuildLeapfrogError, FixedStep};
use super::spec::StreamSpec;

/// Tracers released from each Lagrange point, a parameter of the generator version: 1,000, so
/// that a stream has the brainstorm's "about 2,000" ("Streams and accreted structure").
pub const TRACERS_PER_ARM: u32 = 1_000;

/// The spray's settings: how many tracers each arm releases (module documentation).
///
/// [`SprayParams::default`] is the generator's, [`TRACERS_PER_ARM`]; a smaller count serves
/// tests and benchmarks that need the shape of a spray and not its numbers.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroU32;
///
/// use hyperion_sim::galaxy::global_list::spray::{SprayParams, TRACERS_PER_ARM};
///
/// assert_eq!(SprayParams::default().tracers_per_arm(), TRACERS_PER_ARM);
/// let few = SprayParams::new(NonZeroU32::new(50).expect("not zero"));
/// assert_eq!(few.tracers(), 100);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SprayParams {
    tracers_per_arm: NonZeroU32,
}

impl SprayParams {
    /// A spray of `tracers_per_arm` releases, each freeing one tracer from each Lagrange point.
    #[must_use]
    pub const fn new(tracers_per_arm: NonZeroU32) -> Self {
        Self { tracers_per_arm }
    }

    /// The tracers each arm releases.
    #[must_use]
    pub const fn tracers_per_arm(&self) -> u32 {
        self.tracers_per_arm.get()
    }

    /// The tracers of both arms.
    #[must_use]
    pub const fn tracers(&self) -> u32 {
        // At most 2 × u32::MAX ÷ 2: a count that large never fits in memory anyway.
        self.tracers_per_arm.get().saturating_mul(2)
    }
}

impl Default for SprayParams {
    /// The generator's spray: [`TRACERS_PER_ARM`] from each Lagrange point.
    fn default() -> Self {
        Self::new(NonZeroU32::new(TRACERS_PER_ARM).expect("the generator's count is not zero"))
    }
}

/// One release of a [`ReleaseSchedule`]: a step boundary at which one tracer leaves from each
/// Lagrange point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Release {
    step: u32,
    weight: SolarMasses,
    bound_mass: SolarMasses,
}

impl Release {
    /// The step boundary it falls at, counted from the start of the stripping time: boundary `s`
    /// is `T_s − s h` before the epoch, `h` the [`ReleaseSchedule::step`].
    #[must_use]
    pub const fn step(&self) -> u32 {
        self.step
    }

    /// The mass each of its two tracers stands for: half its stratum's.
    #[must_use]
    pub const fn weight(&self) -> SolarMasses {
        self.weight
    }

    /// The progenitor's bound mass as its tracers leave (module documentation, "The bound mass").
    #[must_use]
    pub const fn bound_mass(&self) -> SolarMasses {
        self.bound_mass
    }
}

/// A stream's releases over its stripping time (module documentation, "The release schedule").
///
/// # Examples
///
/// A living globular releases at a constant rate, so its releases spread evenly over the
/// stripping time and weigh the same (not run: the debris needs the full potential):
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::global_list::spray::{ReleaseSchedule, SprayParams};
/// use hyperion_sim::galaxy::global_list::{Debris, MassLoss};
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// let debris = Debris::generate(&galaxy);
/// let stream = debris
///     .streams()
///     .iter()
///     .find(|s| matches!(s.mass_loss(), MassLoss::Steady(_)))
///     .expect("a living globular with a tube");
/// let schedule = ReleaseSchedule::new(galaxy.seed(), stream, &SprayParams::default(), &[])?;
/// let total = schedule.total_weight().value();
/// assert!((total / stream.mass().value() - 1.0).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseSchedule {
    step: FixedStep,
    history: History,
    /// `m₀`, M☉: the progenitor's bound mass at the epoch.
    progenitor_mass: f64,
    /// `M`, M☉: the tube's.
    tube_mass: f64,
    releases: Vec<Release>,
}

impl ReleaseSchedule {
    /// The schedule of `spec`'s stream in the galaxy of `seed`, with `params`' releases per arm.
    ///
    /// `pericentres` are the step boundaries at which the progenitor's integrated orbit passes a
    /// pericentre, oldest first, which a dwarf's pulses fall at (module documentation); the other
    /// kinds of stream ignore them.
    ///
    /// # Errors
    ///
    /// As [`SphericalOrbit::fixed_step`](super::SphericalOrbit::fixed_step) over the stripping
    /// time: [`BuildLeapfrogError::InvalidStep`] for a span the step rule cannot cover.
    pub fn new(
        seed: Seed,
        spec: &StreamSpec,
        params: &SprayParams,
        pericentres: &[u32],
    ) -> Result<Self, BuildLeapfrogError> {
        let step = spec
            .orbit()
            .fixed_step(Seconds::from(spec.stripping_time()))?;
        let history = History::new(spec, step, pericentres);
        let (progenitor_mass, tube_mass) = (spec.progenitor_mass().value(), spec.mass().value());
        let n = params.tracers_per_arm();
        let weight = SolarMasses::new(0.5 * tube_mass / f64::from(n));
        let mut jitter = Stream::open(
            seed,
            tags::STREAM_SPRAY_RELEASE,
            ObjectKey::galaxy_item(u64::from(spec.number().get())),
        );
        let releases = (0..n)
            .map(|i| {
                let share = (f64::from(i) + jitter.uniform()) / f64::from(n);
                Release {
                    step: history.step_at(share, step.count()),
                    weight,
                    bound_mass: SolarMasses::new(progenitor_mass + tube_mass * (1.0 - share)),
                }
            })
            .collect();
        Ok(Self {
            step,
            history,
            progenitor_mass,
            tube_mass,
            releases,
        })
    }

    /// The stream's fixed step over its stripping time, forwards: `count` steps of `step`.
    #[must_use]
    pub fn step(&self) -> FixedStep {
        self.step
    }

    /// The releases, in stratum order, which is time order.
    #[must_use]
    pub fn releases(&self) -> &[Release] {
        &self.releases
    }

    /// The mass the tracers of both arms stand for, summed in release order: the tube's mass to
    /// rounding.
    #[must_use]
    pub fn total_weight(&self) -> SolarMasses {
        SolarMasses::new(
            self.releases
                .iter()
                .fold(0.0, |sum, r| sum + 2.0 * r.weight.value()),
        )
    }

    /// The progenitor's bound mass at step boundary `step` (module documentation, "The bound
    /// mass"), with the tracers released at that boundary gone; its mass at the epoch beyond the
    /// last boundary.
    #[must_use]
    pub fn bound_mass(&self, step: u32) -> SolarMasses {
        let lost = self.history.share_lost_by(step);
        SolarMasses::new(self.progenitor_mass + self.tube_mass * (1.0 - lost))
    }
}

/// The shape in time of a stream's mass loss over its stripping time, in steps of its fixed step
/// from the start (module documentation).
#[derive(Debug, Clone, PartialEq)]
enum History {
    /// At a constant rate from the start until `end` steps: the epoch, or an orphan's
    /// dissolution.
    Steady { end: f64 },
    /// In pulses at step boundaries, oldest first, each with its cumulative mass, in any unit,
    /// and the last's the total.
    Pulsed { pulses: Vec<(u32, f64)> },
}

impl History {
    fn new(spec: &StreamSpec, step: FixedStep, pericentres: &[u32]) -> Self {
        let steps = f64::from(step.count());
        match spec.mass_loss() {
            MassLoss::Steady(_) => Self::Steady { end: steps },
            MassLoss::Dissolved { ago } => {
                let window = spec.stripping_time().value();
                Self::Steady {
                    end: (steps * ((window - ago.value()) / window)).clamp(0.0, steps),
                }
            }
            MassLoss::Pulsed {
                fraction,
                last_pericentre,
                pericentres: count,
            } => {
                let inside: Vec<u32> = pericentres
                    .iter()
                    .copied()
                    .filter(|&s| s <= step.count())
                    .collect();
                let at = if inside.is_empty() {
                    spherical_pericentres(spec, step, last_pericentre.value(), count)
                } else {
                    inside
                };
                // The `count` most recent, the `k`-th most recent carrying `(1 − f)^−(k+1)`, summed
                // oldest first.
                let kept = at.len().min(usize::try_from(count).unwrap_or(usize::MAX));
                let recent = &at[at.len() - kept..];
                let mut cumulative = 0.0;
                let pulses = recent
                    .iter()
                    .zip((1..=kept).rev())
                    .map(|(&s, k)| {
                        // A dwarf passes a few hundred pericentres at most (`dwarfs`), far inside
                        // i32.
                        let power = i32::try_from(k).unwrap_or(i32::MAX);
                        cumulative += math::powi(1.0 - fraction, -power);
                        (s, cumulative)
                    })
                    .collect();
                Self::Pulsed { pulses }
            }
        }
    }

    /// The step boundary of the release whose share of the tube's mass is lost by then is
    /// `share`, on a grid of `steps` steps.
    fn step_at(&self, share: f64, steps: u32) -> u32 {
        match self {
            Self::Steady { end } => {
                let at = (share * end).round().clamp(0.0, end.floor());
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a whole number in 0..=steps, the grid's u32 count"
                )]
                let at = at as u32;
                at.min(steps)
            }
            Self::Pulsed { pulses } => {
                let total = pulses.last().map_or(0.0, |&(_, cumulative)| cumulative);
                pulses
                    .iter()
                    .find(|&&(_, cumulative)| cumulative > share * total)
                    .or(pulses.last())
                    .map_or(steps, |&(s, _)| s)
            }
        }
    }

    /// The share of the tube's mass lost by step boundary `step`, its releases there included.
    fn share_lost_by(&self, step: u32) -> f64 {
        match self {
            Self::Steady { end } => {
                if *end <= 0.0 {
                    1.0
                } else {
                    (f64::from(step) / end).min(1.0)
                }
            }
            Self::Pulsed { pulses } => {
                let total = pulses.last().map_or(0.0, |&(_, cumulative)| cumulative);
                let lost = pulses
                    .iter()
                    .take_while(|&&(s, _)| s <= step)
                    .last()
                    .map_or(0.0, |&(_, cumulative)| cumulative);
                if total > 0.0 { lost / total } else { 1.0 }
            }
        }
    }
}

/// The step boundaries, oldest first, of the `count` pericentres the spherical approximation puts
/// inside the stripping time, the last `last_pericentre` years before the epoch and each one
/// radial period before the next, rounded to the nearest boundary: where a dwarf's pulses fall
/// when the integrated orbit passes none inside it (module documentation).
fn spherical_pericentres(
    spec: &StreamSpec,
    step: FixedStep,
    last_pericentre: f64,
    count: u32,
) -> Vec<u32> {
    let (window, period) = (
        spec.stripping_time().value(),
        spec.orbit().radial_period().value(),
    );
    let steps = f64::from(step.count());
    (0..count)
        .rev()
        .map(|k| last_pericentre + f64::from(k) * period)
        .filter(|&ago| ago <= window)
        .map(|ago| {
            let at = (steps * (1.0 - ago / window)).round().clamp(0.0, steps);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a whole number in 0..=steps, the grid's u32 count"
            )]
            let at = at as u32;
            at
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::{GalacticDisplacement, GalacticVelocity};
    use crate::galaxy::global_list::spec::SphericalOrbit;
    use crate::galaxy::global_list::{StreamNumber, StreamOrigin};
    use crate::units::{LightYears, SolarMassesPerYear, Years};

    /// G M in (km/s)² ly for 10¹¹ M☉, as `spec`'s tests take it.
    const GM: f64 = 4.300_9e-3 * 3.261_563_777 * 1e11;

    const SEED: Seed = Seed::new(0x0a10_5a00_0000_0000);

    /// A stream on a Kepler orbit between 20,000 and 60,000 ly, stripped over 3 Gyr, of 2 × 10⁴
    /// M☉ lost by a progenitor of `progenitor` M☉ as `mass_loss` says.
    fn stream(number: u16, progenitor: f64, mass_loss: MassLoss) -> StreamSpec {
        let orbit = SphericalOrbit::from_turning_points(|r| -GM / r, 20_000.0, 60_000.0)
            .expect("Kepler turning points bound an orbit");
        StreamSpec {
            number: StreamNumber::new(number).expect("below the limit"),
            origin: StreamOrigin::Orphan,
            position: GalacticDisplacement::new([0.0; 3]),
            velocity: GalacticVelocity::new([0.0; 3]),
            orbit,
            stripping_time: Years::new(3e9),
            mass: SolarMasses::new(2e4),
            progenitor_mass: SolarMasses::new(progenitor),
            tidal_radius: LightYears::new(100.0),
            mass_loss,
            stars: None,
        }
    }

    fn steady() -> StreamSpec {
        stream(3, 3e4, MassLoss::Steady(SolarMassesPerYear::new(2e4 / 3e9)))
    }

    fn orphan() -> StreamSpec {
        stream(
            4,
            0.0,
            MassLoss::Dissolved {
                ago: Years::new(1.2e9),
            },
        )
    }

    /// A dwarf of 5 × 10⁵ M☉ left in its core after three pulses of 30% each: the tube holds
    /// `5 × 10⁵ × (0.7⁻³ − 1)` M☉.
    fn dwarf() -> StreamSpec {
        let core = 5e5;
        StreamSpec {
            mass: SolarMasses::new(core * (math::powi(0.7, -3) - 1.0)),
            ..stream(
                5,
                core,
                MassLoss::Pulsed {
                    fraction: 0.3,
                    last_pericentre: Years::new(2e8),
                    pericentres: 3,
                },
            )
        }
    }

    fn schedule(spec: &StreamSpec, pericentres: &[u32]) -> ReleaseSchedule {
        ReleaseSchedule::new(SEED, spec, &SprayParams::default(), pericentres)
            .expect("a finite stripping time")
    }

    /// P10.T5.a: the weights sum to the tube's mass, for each kind of mass loss, and each release
    /// frees two tracers of half its stratum's mass.
    #[test]
    fn weights_sum_to_the_tube_s_mass() {
        let pericentres = [100, 2_000, 4_000, 6_000];
        for spec in [steady(), orphan(), dwarf()] {
            let schedule = schedule(&spec, &pericentres);
            let releases = schedule.releases();
            assert_eq!(releases.len(), 1_000);
            let total = schedule.total_weight().value();
            assert!(
                (total / spec.mass().value() - 1.0).abs() < 1e-12,
                "{:?}: {total} against {}",
                spec.mass_loss(),
                spec.mass().value()
            );
            let each = spec.mass().value() / 2_000.0;
            for r in releases {
                assert!((r.weight().value() / each - 1.0).abs() < 1e-15);
            }
        }
    }

    /// P10.T5.a: an orphan releases nothing after its dissolution, 1.2 Gyr before the epoch, and
    /// its releases spread over the 1.8 Gyr before it.
    #[test]
    fn an_orphan_releases_nothing_after_its_dissolution() {
        let spec = orphan();
        let schedule = schedule(&spec, &[]);
        let step = schedule.step();
        let h = Years::from(step.step()).value();
        let dissolved = 3e9 - 1.2e9;
        let latest = schedule
            .releases()
            .iter()
            .map(Release::step)
            .max()
            .expect("a thousand releases");
        assert!(
            f64::from(latest) * h <= dissolved,
            "a release {} Gyr after the start, past the dissolution at {} Gyr",
            f64::from(latest) * h / 1e9,
            dissolved / 1e9
        );
        // The last stratum ends at the dissolution: the latest release lies within a stratum of
        // it, 1.8 Gyr ÷ 1,000.
        assert!(f64::from(latest) * h > dissolved - 2.0 * 1.8e6 - h);
        // Nothing is bound after it, and its cluster shrinks to nothing as it dissolves.
        assert!(schedule.bound_mass(step.count()).value().abs() < 1e-9);
        let last = schedule.releases().last().expect("a thousand releases");
        assert!(last.bound_mass().value() < 2e4 / 1_000.0);
        assert!((schedule.bound_mass(0).value() / 2e4 - 1.0).abs() < 1e-12);
    }

    /// A living globular releases at a constant rate: one release in each thousandth of the
    /// stripping time, in time order, with the bound mass falling from `m₀ + M` to `m₀`.
    #[test]
    fn a_steady_stream_releases_evenly_and_its_mass_falls_to_the_present() {
        let spec = steady();
        let schedule = schedule(&spec, &[]);
        let steps = f64::from(schedule.step().count());
        for (i, r) in (0_u32..).zip(schedule.releases()) {
            let (from, to) = (
                f64::from(i) / 1_000.0 * steps,
                f64::from(i + 1) / 1_000.0 * steps,
            );
            let at = f64::from(r.step());
            assert!(at >= from - 0.5 && at <= to + 0.5, "release {i} at {at}");
            let share = 1.0 - (r.bound_mass().value() - 3e4) / 2e4;
            assert!(
                share * steps >= from - 1e-6 && share * steps <= to + 1e-6,
                "release {i}"
            );
        }
        assert!((schedule.bound_mass(0).value() - 5e4).abs() < 1e-9);
        assert!((schedule.bound_mass(schedule.step().count()).value() - 3e4).abs() < 1e-9);
        let masses: Vec<f64> = (0..=schedule.step().count())
            .map(|s| schedule.bound_mass(s).value())
            .collect();
        assert!(masses.windows(2).all(|w| w[1] <= w[0]));
    }

    /// A dwarf's pulses fall on the integrated orbit's pericentres, the specification's count of
    /// the most recent, each with its share of the tube: `0.7⁻³ : 0.7⁻² : 0.7⁻¹` of the core's
    /// mass, oldest first. With none inside the stripping time, the spherical approximation's
    /// stand in.
    #[test]
    fn a_dwarf_releases_in_pulses_at_its_pericentres() {
        let spec = dwarf();
        let integrated = [100, 2_000, 4_000, 6_000];
        let schedule = schedule(&spec, &integrated);
        let count = |at: u32| {
            schedule
                .releases()
                .iter()
                .filter(|r| r.step() == at)
                .count()
        };
        assert_eq!(count(100), 0, "the oldest pericentre is beyond the count");
        let weights = [
            math::powi(0.7, -3),
            math::powi(0.7, -2),
            math::powi(0.7, -1),
        ];
        let sum: f64 = weights.iter().sum();
        let mut released = 0;
        for (at, w) in [2_000, 4_000, 6_000].into_iter().zip(weights) {
            let n = count(at);
            released += n;
            // Stratified: within one release of its share of the thousand.
            let expected = 1_000.0 * w / sum;
            assert!(
                (f64::from(u32::try_from(n).unwrap()) - expected).abs() <= 1.0,
                "{n} releases at {at} against {expected}"
            );
        }
        assert_eq!(released, 1_000);
        // The bound mass steps down at each pulse to the core's at the last.
        let core = spec.progenitor_mass().value();
        let tube = spec.mass().value();
        assert!((schedule.bound_mass(1_999).value() / (core + tube) - 1.0).abs() < 1e-12);
        assert!((schedule.bound_mass(6_000).value() / core - 1.0).abs() < 1e-12);
        // Before the last pulse the core held its mass over 0.7.
        let after_two = core / 0.7;
        assert!(schedule.bound_mass(4_000).value() > core);
        assert!((schedule.bound_mass(4_000).value() / after_two - 1.0).abs() < 1e-12);

        // Pericentres beyond the grid only: the spherical approximation's, 0.2 Gyr before the
        // epoch and one radial period apart, three of them inside 3 Gyr.
        let fallback = schedule_spherical(&spec);
        let period = spec.orbit().radial_period().value();
        let steps = f64::from(fallback.step().count());
        for k in 0..3_u32 {
            let ago = 2e8 + f64::from(k) * period;
            let at = (steps * (1.0 - ago / 3e9)).round();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a test"
            )]
            let at = at as u32;
            assert!(
                fallback.releases().iter().any(|r| r.step() == at),
                "no pulse at {at}"
            );
        }
    }

    fn schedule_spherical(spec: &StreamSpec) -> ReleaseSchedule {
        schedule(spec, &[u32::MAX])
    }

    /// The same seed and stream give the same schedule; another stream number draws another.
    #[test]
    fn the_same_stream_gives_the_same_schedule() {
        let spec = steady();
        assert_eq!(schedule(&spec, &[]), schedule(&spec, &[]));
        let other = StreamSpec {
            number: StreamNumber::new(9).unwrap(),
            ..steady()
        };
        assert_ne!(schedule(&spec, &[]), schedule(&other, &[]));
        let seed = ReleaseSchedule::new(Seed::new(1), &spec, &SprayParams::default(), &[]).unwrap();
        assert_ne!(schedule(&spec, &[]), seed);
    }
}
