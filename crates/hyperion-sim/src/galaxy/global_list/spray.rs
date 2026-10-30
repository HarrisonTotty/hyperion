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
//! samples that history stratified in mass. Release `i` of `N` ([`TRACERS_PER_ARM`] in the
//! generator's spray, [`SprayParams`]) is at the time when the mass lost reaches the share
//! `uᵢ = (i + Uᵢ) ÷ N` of the tube's, `Uᵢ` uniform on `stream.spray.release`, rounded to the
//! nearest boundary of the stream's fixed step
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
//!
//! # The release
//!
//! The progenitor is integrated backwards from its epoch state over the stripping time and
//! forwards again, at the schedule's step in ruling 146.3's table of the total potential
//! ([`SprayForce`]), keeping its state at every boundary (P10.T5.b); its pericentres there are
//! where a dwarf's pulses fall. At a release each tracer starts from a Lagrange point with the
//! offsets of Fardal, Huang and Weinberg (2015, MNRAS 452, 301, §2.4 eqs. 3–9, §4.2 eqs. 19–24;
//! quoted and numbered here from arXiv:1410.1861v1, the preprint), in the frame of the
//! progenitor's galactocentric direction `x̂`, its orbit's pole `ẑ` and the direction ahead,
//! `ŷ = ẑ × x̂`:
//!
//! ```text
//! x = x_p ± k_r r_t x̂ + k_z r_t ẑ        v = v_p ± k_vφ Ω r_t ŷ + k_vz Ω r_t ẑ
//! ```
//!
//! the upper sign for the trailing arm, released outwards, and the lower for the leading arm,
//! inwards, with `k_φ = k_vr = 0` (their eq. 9). Their eq. 7 is printed `z = k_z r_tidal ÷ r`,
//! which leaves `z` without a length; it is read as `k_z r_tidal`, as `gala`'s `FardalStreamDF`
//! reads it. `r_t` is the tidal radius at the progenitor's
//! bound mass and radius then, their eq. 1, `(G m ÷ (Ω² − d²Φ ÷ dr²))^⅓`, which is plan 02's
//! `(G m ÷ (4Ω² − κ²))^⅓` ([`PotentialTables::tidal_radius`]), and `Ω = V_c(r) ÷ r` the circular
//! frequency at the progenitor's radius, as they define it (their eq. 6 reads it at the tracer's,
//! a difference of order `r_t ÷ r`). The offsets are Gaussian:
//!
//! - `k_r` about [`RADIAL_OFFSET`], 2.0, and `k_vφ` about [`TANGENTIAL_OFFSET`], 0.3 (eqs. 19–20),
//!   both with the dispersion of eq. 21, `min(0.15 f_t² R_acc^⅔, 0.4)`. `R_acc = g_a(r_p) ÷
//!   g_a(r_a)`, `g_a = Ω² − d²Φ ÷ dr²` (eqs. 13–14), comes from the specification's turning points,
//!   as the cube of the tidal radius's ratio between them. `f_t`, their satellites' initial King
//!   radius over the tidal radius at apocentre (eq. 10), is [`FILLING_FACTOR`], 0.8, that of the
//!   five runs of their Table 1 without a suffix, since no progenitor here has a King radius.
//! - `k_z` and `k_vz` about zero (eq. 22) with dispersion [`VERTICAL_SPREAD`], 0.5 (eqs. 23–24).
//!
//! Each is drawn by inverse transform of one word of `stream.spray.offsets`, four a tracer, from a
//! Gaussian cut at [`OFFSET_CUT`], ±2.5 of its dispersions. Fardal et al. find their Gaussians
//! "unlikely to be accurate in the extremes of the distribution" (§2.4), and trace the most
//! common deficiency of their own model streams, "an excessively long tail of debris", in part to
//! "our use of simple Gaussian distributions of the space and velocity offsets rather than a more
//! sharply truncated distribution" (§5.1). The cut leaves `1 − 2a φ(a) ÷ (2Φ(a) − 1)` of the
//! Gaussian's variance at `a` = 2.5, 0.9113, so each Gaussian is widened by [`cut_widening`],
//! 1.0476, and the cut distribution keeps the fitted dispersion (main's ruling of 2026-10-29):
//! the cut lies at ±2.619 of the fitted dispersions. It still keeps `k_r + k_vφ` at 0.204 or more
//! at the largest dispersion, 0.4, so that near a circular orbit, where a tracer's guiding radius
//! moves by about `(k_r + k_vφ) r_t`, every tracer drifts the way its arm does.
//!
//! # The integration
//!
//! A tracer feels the table's force alone, the host's potential, as Fardal et al.'s tracers do
//! ("our model does not include the satellite potential", §5.2). Their recipe leaves the
//! progenitor out on purpose: it "can be omitted with an appropriate choice of release constants
//! (equations 19–24); the time-integrated effect of the progenitor is encapsulated by our larger
//! action offset" (§6), the `k̄_r` = 2 and `k̄_vφ` = 0.3 above, so a progenitor's sphere added to
//! them would count its pull twice. Main's ruling of 2026-10-29 drops plan 10's Plummer sphere
//! for every progenitor. The recipe's reach is narrower than that:
//!
//! - Fardal et al. calibrated it on satellites of 10⁶ M☉, 3.5 × 10⁻⁶ of their host's mass (§3,
//!   Table 1), "for small progenitor masses" where "we can still specify the behavior of stars
//!   fairly well in terms of orbits in the host potential alone" (§2.4).
//! - Beyond that range they tested it on a Sagittarius of 8.5 × 10⁻⁴ of its host's mass: "the
//!   agreement is extremely good", though "the model stream is somewhat longer and in some places
//!   narrower than the N-body results" (§5.2).
//! - "At very large satellite masses the effect of the satellite will not converge within an
//!   orbital wrap and this will alter the morphology of the tidal stream (Choi et al. 2007), making
//!   explicit inclusion of the satellite force necessary" (§6).
//!
//! So a dwarf above about 10⁻³ of the host's mass, up to plan 02's 10⁹·⁵ M☉, lies beyond every
//! test of the recipe, a finding for the owner (P10.T5.b, the spray's fix, as built). The
//! progenitor's bound mass still sets each release's tidal radius. Every body sums the angle it
//! sweeps
//! about the centre at each step, `atan2(|x₀ × x₁|, x₀ · x₁)` by [`math::atan2`], the angle in its
//! own orbital plane, as [`Leapfrog::integrate`](super::orbit::Leapfrog::integrate) does. A
//! tracer's ψ is its angle less the progenitor's over the same steps: positive for one that gains
//! on the progenitor, the leading arm's, negative for one that falls behind.
//!
//! # The arms
//!
//! A tracer's arm is the Lagrange point it left from, and nearly every tracer's ψ has its arm's
//! sign: in this module's slow tests (`tests/global_list_spray.rs`) at v23, one of
//! `sagittarius_like`'s 2,000 does not, and none of six sampled streams of a drawn Milky Way
//! galaxy. The Plummer sphere this module once added sent up
//! to 1.6 % of a drawn galaxy's tracers onto the other arm's orbits, some a radian or more along
//! it (P10.T5.b, as built). Fardal et al. describe the same artefact in another recipe, stars
//! thrown "backwards into the stream emerging from the opposing Lagrange point" (§5.1). So
//! [`Arm`] is a diagnostic, and the tube table ([`tube`](super::tube), P10.T6) takes a tracer's
//! arm from the sign of its ψ.
//!
//! # A pool
//!
//! Each tracer is integrated alone from its release to the epoch, so [`Spray::tracer`] is a pure
//! function of its index: a caller's pool (plan 04's) runs tracers in parallel and gathers them by
//! index (ruling 146.3), and [`Spray::tracers`] runs them in order. The kick–drift–kick steps are
//! the leapfrog's, with `+ − × ÷`, `sqrt` and [`math`] only (P10.T2.c's source check reads this
//! file too).

use std::num::NonZeroU32;

use crate::coords::{GalacticDisplacement, GalacticVelocity};
use crate::galaxy::potential::PotentialTables;
use crate::galaxy::{Galaxy, PointLy};
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};
use crate::units::{LightYears, Radians, Seconds, SolarMasses, Years};

use super::MassLoss;
use super::orbit::{BuildLeapfrogError, FixedStep, cross, dot};
use super::spec::StreamSpec;

mod force;

pub use force::{NODES, SprayForce};

/// `k̄_r`, the mean of a tracer's release offset along the galactocentric radius in tidal radii
/// (Fardal, Huang and Weinberg 2015, eq. 19), a parameter of the generator version.
pub const RADIAL_OFFSET: f64 = 2.0;

/// `k̄_vφ`, the mean of its tangential velocity offset in units of `Ω r_t` (Fardal et al. 2015,
/// eq. 20), a parameter of the generator version.
pub const TANGENTIAL_OFFSET: f64 = 0.3;

/// `σ(k_z) = σ(k_vz)`, the dispersion of the offsets out of the orbital plane (Fardal et al.
/// 2015, eqs. 23–24), a parameter of the generator version.
pub const VERTICAL_SPREAD: f64 = 0.5;

/// The coefficient and the cap of the in-plane offsets' dispersion, `σ(k_r) = σ(k_vφ) =
/// min(0.15 f_t² R_acc^⅔, 0.4)` (Fardal et al. 2015, eq. 21).
pub const IN_PLANE_SPREAD: (f64, f64) = (0.15, 0.4);

/// `f_t`, a satellite's initial King radius over its tidal radius at apocentre, for eq. 21: 0.8,
/// the value of the five `m6.0` runs of Fardal et al.'s Table 1 (2015) without a suffix, and the
/// "tidal factor `f_t` = 0.8" of their Sagittarius run (§5.2), a parameter of the generator
/// version.
pub const FILLING_FACTOR: f64 = 0.8;

/// Where each release offset's Gaussian is cut, in its dispersions (module documentation, "The
/// release"), a parameter of the generator version.
pub const OFFSET_CUT: f64 = 2.5;

/// How much each release offset's Gaussian is widened, so that, cut at [`OFFSET_CUT`], it keeps
/// the dispersion Fardal et al. fitted (module documentation, "The release").
///
/// It is `1 ÷ √(1 − 2a φ(a) ÷ (2Φ(a) − 1))` at `a` = [`OFFSET_CUT`], 1.0476, with `φ` and `Φ` the
/// standard normal density and distribution function: the inverse square root of the variance of
/// a standard normal truncated to `[−a, a]` (Johnson, Kotz and Balakrishnan 1994, *Continuous
/// Univariate Distributions*, vol. 1, 2nd ed., ch. 13 §10.1, the truncated normal distribution).
#[must_use]
pub fn cut_widening() -> f64 {
    let a = OFFSET_CUT;
    let density = math::exp(-0.5 * a * a) / core::f64::consts::TAU.sqrt();
    let inside = 1.0 - math::erfc(a * core::f64::consts::FRAC_1_SQRT_2);
    1.0 / (1.0 - 2.0 * a * density / inside).sqrt()
}

/// Words of `stream.spray.offsets` a tracer reads: `k_r`, `k_vφ`, `k_z`, `k_vz`, one each.
pub const OFFSET_WORDS: u64 = 4;

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
    /// The most tracers an arm may release, 2³¹ − 1, so that both arms' count is a `u32`.
    pub const MAX_TRACERS_PER_ARM: u32 = u32::MAX / 2;

    /// A spray of `tracers_per_arm` releases, each freeing one tracer from each Lagrange point.
    ///
    /// # Panics
    ///
    /// If `tracers_per_arm` exceeds [`MAX_TRACERS_PER_ARM`](Self::MAX_TRACERS_PER_ARM), a count
    /// no memory holds the tracers of.
    #[must_use]
    pub const fn new(tracers_per_arm: NonZeroU32) -> Self {
        assert!(
            tracers_per_arm.get() <= Self::MAX_TRACERS_PER_ARM,
            "a spray releases at most 2^31 - 1 tracers an arm"
        );
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
        // At most 2 × (2³¹ − 1), by `new`.
        2 * self.tracers_per_arm.get()
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
    /// `m₀`: the progenitor's bound mass at the epoch.
    progenitor_mass: SolarMasses,
    /// `M`: the tube's.
    tube_mass: SolarMasses,
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
        let (progenitor_mass, tube_mass) = (spec.progenitor_mass(), spec.mass());
        let n = params.tracers_per_arm();
        let weight = SolarMasses::new(0.5 * tube_mass.value() / f64::from(n));
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
                    bound_mass: SolarMasses::new(
                        progenitor_mass.value() + tube_mass.value() * (1.0 - share),
                    ),
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
        SolarMasses::new(self.progenitor_mass.value() + self.tube_mass.value() * (1.0 - lost))
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
    /// `spec`'s history on `step`'s grid, a dwarf's pulses at `pericentres` (module
    /// documentation, "A dwarf's pulses").
    #[must_use]
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
                let mut inside: Vec<u32> = pericentres
                    .iter()
                    .copied()
                    .filter(|&s| s <= step.count())
                    .collect();
                // Oldest first, as documented, whatever order the caller gave.
                inside.sort_unstable();
                inside.dedup();
                let at = if inside.is_empty() {
                    spherical_pericentres(spec, step, last_pericentre, count)
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
    #[must_use]
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
    #[must_use]
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
#[must_use]
fn spherical_pericentres(
    spec: &StreamSpec,
    step: FixedStep,
    last_pericentre: Years,
    count: u32,
) -> Vec<u32> {
    let (window, period) = (
        spec.stripping_time().value(),
        spec.orbit().radial_period().value(),
    );
    let steps = f64::from(step.count());
    (0..count)
        .rev()
        .map(|k| last_pericentre.value() + f64::from(k) * period)
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

/// Which Lagrange point a tracer left from (module documentation, "The release"), or which arm of
/// a tube, which takes a tracer's from the sign of its ψ
/// ([`TubeTable::arm`](super::TubeTable::arm)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arm {
    /// The inner point: the tracer gains on the progenitor, so its ψ is positive, but for the few
    /// that end on the other arm's orbits (module documentation, "The arms").
    Leading,
    /// The outer point: the tracer falls behind, so its ψ is negative, but for the same few.
    Trailing,
}

/// One tracer of a stream's spray at the epoch (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tracer {
    arm: Arm,
    release_step: u32,
    released: Years,
    weight: SolarMasses,
    position: GalacticDisplacement,
    velocity: GalacticVelocity,
    psi: Radians,
}

impl Tracer {
    /// The Lagrange point it left from.
    #[must_use]
    pub const fn arm(&self) -> Arm {
        self.arm
    }

    /// The step boundary it was released at ([`Release::step`]).
    #[must_use]
    pub const fn release_step(&self) -> u32 {
        self.release_step
    }

    /// How long before the epoch it was released.
    #[must_use]
    pub const fn released(&self) -> Years {
        self.released
    }

    /// The mass it stands for ([`Release::weight`]).
    #[must_use]
    pub const fn weight(&self) -> SolarMasses {
        self.weight
    }

    /// Its position about the galactic centre at the epoch, m.
    #[must_use]
    pub const fn position(&self) -> GalacticDisplacement {
        self.position
    }

    /// Its velocity in the galactic frame at the epoch.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// ψ, the angle it gained on the progenitor since its release, each body's angle swept about
    /// the centre in its own orbital plane: positive ahead of the progenitor, negative behind.
    #[must_use]
    pub const fn psi(&self) -> Radians {
        self.psi
    }

    /// A tracer from its parts, for the tube table's tests of synthetic streams.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn from_parts(
        arm: Arm,
        release_step: u32,
        released: Years,
        weight: SolarMasses,
        position: GalacticDisplacement,
        velocity: GalacticVelocity,
        psi: Radians,
    ) -> Self {
        Self {
            arm,
            release_step,
            released,
            weight,
            position,
            velocity,
            psi,
        }
    }
}

/// The progenitor at one step boundary of the forward integration.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Boundary {
    /// About the galactic centre.
    position_m: [f64; 3],
    velocity_m_s: [f64; 3],
    position_ly: [f64; 3],
    /// The angle swept since boundary 0.
    swept_rad: f64,
}

/// One stream's spray, ready to release and integrate its tracers (module documentation): the
/// progenitor's orbit over the stripping time, its release schedule and its offsets' stream.
///
/// # Examples
///
/// A galaxy's first stream, its tracers integrated in two halves as a pool would and gathered by
/// index (not run: the debris needs the full potential, and the spray takes some more seconds):
///
/// ```no_run
/// use std::num::NonZeroU32;
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::global_list::Debris;
/// use hyperion_sim::galaxy::global_list::spray::{Arm, Spray, SprayForce, SprayParams};
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// let debris = Debris::generate(&galaxy);
/// let stream = &debris.streams()[0];
/// let (seed, tables) = (galaxy.seed(), galaxy.potential());
/// let force = SprayForce::new(tables)?;
/// let params = SprayParams::new(NonZeroU32::new(100).expect("not zero"));
/// let spray = Spray::new(seed, &force, tables, stream, &params)?;
/// let half = spray.tracer_count() / 2;
/// let mut tracers: Vec<_> = (half..spray.tracer_count()).map(|i| spray.tracer(i)).collect();
/// tracers.splice(0..0, (0..half).map(|i| spray.tracer(i)));
/// assert_eq!(tracers, spray.tracers());
/// // The leading arm runs ahead of the progenitor, the trailing arm behind it, on average.
/// let mean = |arm| {
///     let on: Vec<_> = tracers.iter().filter(|t| t.arm() == arm).collect();
///     on.iter().map(|t| t.psi().value()).sum::<f64>() / on.len() as f64
/// };
/// assert!(mean(Arm::Leading) > 0.0 && mean(Arm::Trailing) < 0.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Spray<'a> {
    force: &'a SprayForce,
    tables: &'a PotentialTables,
    schedule: ReleaseSchedule,
    stripping_time: Years,
    /// The progenitor at boundaries 0 to the step count.
    track: Vec<Boundary>,
    offsets: Stream,
    /// `σ(k_r) = σ(k_vφ)` (eq. 21).
    in_plane_spread: f64,
    /// The in-plane and the vertical offsets' Gaussians' dispersions before the cut: eq. 21's and
    /// [`VERTICAL_SPREAD`], each times [`cut_widening`].
    widened: [f64; 2],
    /// The Gaussian's distribution function at the lower cut, and its share between the cuts.
    cut: [f64; 2],
}

impl<'a> Spray<'a> {
    /// The spray of `spec`'s stream in the galaxy of `seed`, whose (R, |z|) tables are `tables`
    /// and their spray table `force`, with `params`' releases per arm: the progenitor integrated
    /// backwards over the stripping time and forwards again, and the release schedule (module
    /// documentation). The tracers are integrated when asked for.
    ///
    /// # Errors
    ///
    /// As [`SphericalOrbit::fixed_step`](super::SphericalOrbit::fixed_step) over the stripping
    /// time: [`BuildLeapfrogError::InvalidStep`] for a span the step rule cannot cover.
    ///
    /// # Panics
    ///
    /// If the tidal radii at the specification's turning points are not finite and positive,
    /// which a specification's orbit, bound between positive radii, always gives.
    pub fn new(
        seed: Seed,
        force: &'a SprayForce,
        tables: &'a PotentialTables,
        spec: &StreamSpec,
        params: &SprayParams,
    ) -> Result<Self, BuildLeapfrogError> {
        let fixed = spec
            .orbit()
            .fixed_step(Seconds::from(spec.stripping_time()))?;
        let (h, steps) = (fixed.step().value(), fixed.count());
        let field = |x: [f64; 3]| force.acceleration_m_s2(&GalacticDisplacement::new(x));
        // Backwards from the epoch to the start of the stripping time, then forwards again.
        let mut body = (
            spec.position().metres(),
            spec.velocity().metres_per_second(),
        );
        let mut acceleration = field(body.0);
        for _ in 0..steps {
            kick_drift_kick(&mut body, &mut acceleration, -h, field);
        }
        let mut track = Vec::with_capacity(usize::try_from(steps).map_or(0, |n| n + 1));
        let mut angle = 0.0;
        track.push(boundary(body, angle));
        for _ in 0..steps {
            let before = body.0;
            kick_drift_kick(&mut body, &mut acceleration, h, field);
            angle += swept(before, body.0);
            track.push(boundary(body, angle));
        }
        // The pericentres: the boundaries nearer the centre than both neighbours.
        let pericentres: Vec<u32> = (1_u32..)
            .zip(track.windows(3))
            .filter(|(_, w)| {
                let [a, b, c] = [w[0], w[1], w[2]].map(|b| dot(b.position_m, b.position_m));
                a > b && b <= c
            })
            .map(|(s, _)| s)
            .collect();
        let schedule = ReleaseSchedule::new(seed, spec, params, &pericentres)?;
        // R_acc^⅔ = (r_t(r_a) ÷ r_t(r_p))², at any one mass.
        let at = |r: LightYears| {
            tables
                .tidal_radius(SolarMasses::new(1.0), &PointLy::new(r.value(), 0.0, 0.0))
                .value()
        };
        let ratio = at(spec.orbit().apocentre()) / at(spec.orbit().pericentre());
        assert!(
            ratio.is_finite() && ratio > 0.0,
            "the tidal radii at a specification's turning points are finite and positive: {ratio}"
        );
        let (coefficient, cap) = IN_PLANE_SPREAD;
        let in_plane_spread =
            (coefficient * FILLING_FACTOR * FILLING_FACTOR * ratio * ratio).min(cap);
        let widening = cut_widening();
        let below = 0.5 * math::erfc(OFFSET_CUT * core::f64::consts::FRAC_1_SQRT_2);
        Ok(Self {
            force,
            tables,
            schedule,
            stripping_time: spec.stripping_time(),
            track,
            offsets: Stream::open(
                seed,
                tags::STREAM_SPRAY_OFFSETS,
                ObjectKey::galaxy_item(u64::from(spec.number().get())),
            ),
            in_plane_spread,
            widened: [in_plane_spread * widening, VERTICAL_SPREAD * widening],
            cut: [below, 1.0 - 2.0 * below],
        })
    }

    /// The release schedule.
    #[must_use]
    pub fn schedule(&self) -> &ReleaseSchedule {
        &self.schedule
    }

    /// The tracers of both arms: two per release.
    ///
    /// # Panics
    ///
    /// Never: [`SprayParams::new`] holds the releases an arm to 2³¹ − 1.
    #[must_use]
    pub fn tracer_count(&self) -> u32 {
        u32::try_from(2 * self.schedule.releases().len()).expect(
            "a schedule holds SprayParams' releases an arm, at most 2³¹ − 1, so twice is a u32",
        )
    }

    /// `σ(k_r) = σ(k_vφ)`, the in-plane release offsets' dispersion, eq. 21's, which their cut
    /// distribution keeps (module documentation, "The release").
    #[must_use]
    pub fn in_plane_spread(&self) -> f64 {
        self.in_plane_spread
    }

    /// The progenitor's position and velocity at step boundary `step` of its forward integration,
    /// or `None` past the last: at the last, the epoch, they differ from the specification's state
    /// by the leapfrog's rounding over the two passes.
    #[must_use]
    pub fn progenitor(&self, step: u32) -> Option<(GalacticDisplacement, GalacticVelocity)> {
        let b = self.track.get(usize::try_from(step).ok()?)?;
        Some((
            GalacticDisplacement::new(b.position_m),
            GalacticVelocity::new(b.velocity_m_s),
        ))
    }

    /// Tracer `index`, released by release `index ÷ 2`, from the leading arm for an even index and
    /// the trailing for an odd one, and integrated to the epoch: a pure function of the index
    /// (module documentation, "The integration").
    ///
    /// # Panics
    ///
    /// If `index` is not below [`tracer_count`](Self::tracer_count), or if the integration leaves
    /// a value that is not finite, which no tracer of a valid specification does.
    #[must_use]
    pub fn tracer(&self, index: u32) -> Tracer {
        assert!(
            index < self.tracer_count(),
            "tracer {index} of a spray of {}",
            self.tracer_count()
        );
        let release = self.schedule.releases()[usize::try_from(index / 2)
            .expect("a u32 fits a usize on every target the sim builds for, of 32 bits or more")];
        let arm = if index.is_multiple_of(2) {
            Arm::Leading
        } else {
            Arm::Trailing
        };
        let start = usize::try_from(release.step())
            .expect("a u32 fits a usize on every target the sim builds for, of 32 bits or more");
        let from = self.track[start];
        let mut body = self.release(&from, release.bound_mass(), arm, index);
        let mut acceleration = self.acceleration(body.0);
        let h = self.schedule.step().step().value();
        let steps = self.schedule.step().count();
        let mut angle = 0.0;
        for _ in release.step()..steps {
            let before = body.0;
            kick_drift_kick(&mut body, &mut acceleration, h, |x| self.acceleration(x));
            angle += swept(before, body.0);
        }
        let last = self
            .track
            .last()
            .expect("the track holds its first boundary");
        let psi = angle - (last.swept_rad - from.swept_rad);
        // P10.T6 sorts the tracers by ψ, and a NaN's sign, which `total_cmp` reads, is not
        // portable (the sim-determinism skill's "Hazards across targets").
        assert!(
            body.0.iter().chain(&body.1).all(|c| c.is_finite()) && psi.is_finite(),
            "tracer {index} left the integration with a value that is not finite"
        );
        Tracer {
            arm,
            release_step: release.step(),
            released: Years::new(
                self.stripping_time.value()
                    * (1.0 - f64::from(release.step()) / f64::from(steps.max(1))),
            ),
            weight: release.weight(),
            position: GalacticDisplacement::new(body.0),
            velocity: GalacticVelocity::new(body.1),
            psi: Radians::new(psi),
        }
    }

    /// Every tracer, in index order ([`tracer`](Self::tracer)).
    #[must_use]
    pub fn tracers(&self) -> Vec<Tracer> {
        (0..self.tracer_count()).map(|i| self.tracer(i)).collect()
    }

    /// The release offsets `[k_r, k_vφ, k_z, k_vz]` of tracer `index`, from its four words.
    #[must_use]
    fn offsets(&self, index: u32) -> [f64; 4] {
        let mut words = self.offsets.clone();
        words.seek(OFFSET_WORDS * u64::from(index));
        let [below, between] = self.cut;
        let mut draw = || math::normal_quantile(below + words.uniform() * between);
        let [in_plane, out_of_plane] = self.widened;
        // k_r, k_vφ, k_z and k_vz, in that order.
        let radial = RADIAL_OFFSET + in_plane * draw();
        let tangential = TANGENTIAL_OFFSET + in_plane * draw();
        let vertical = out_of_plane * draw();
        let vertical_speed = out_of_plane * draw();
        [radial, tangential, vertical, vertical_speed]
    }

    /// Tracer `index`'s state at its release from `from`, its progenitor then of bound mass
    /// `bound`, from `arm`'s Lagrange point.
    #[must_use]
    fn release(&self, from: &Boundary, bound: SolarMasses, arm: Arm, index: u32) -> Body {
        let [outward, ahead, pole] = frame(from.position_m, from.velocity_m_s);
        let [x, y, z] = from.position_ly;
        let tidal = self
            .tables
            .tidal_radius(bound, &PointLy::new(x, y, z))
            .value();
        let radius = dot(from.position_ly, from.position_ly).sqrt();
        let omega = self.tables.omega(LightYears::new(radius)).value() / SECONDS_PER_JULIAN_YEAR;
        let sign = match arm {
            Arm::Leading => -1.0,
            Arm::Trailing => 1.0,
        };
        let [radial, tangential, vertical, vertical_speed] = self.offsets(index);
        let (along, up) = (sign * radial * tidal, vertical * tidal);
        let (forward, lift) = (
            sign * tangential * omega * tidal,
            vertical_speed * omega * tidal,
        );
        (
            core::array::from_fn(|i| from.position_m[i] + along * outward[i] + up * pole[i]),
            core::array::from_fn(|i| from.velocity_m_s[i] + forward * ahead[i] + lift * pole[i]),
        )
    }

    /// The acceleration at `x` (m), m/s²: the table's, the host's alone, as the progenitor's
    /// (module documentation, "The integration").
    #[must_use]
    fn acceleration(&self, x: [f64; 3]) -> [f64; 3] {
        self.force.acceleration_m_s2(&GalacticDisplacement::new(x))
    }
}

/// The spray of `spec`'s stream in `galaxy`, at the generator's [`SprayParams`]: its tracers in
/// index order (module documentation). It builds the galaxy's [`SprayForce`] each time; a caller
/// spraying several streams keeps one and calls [`Spray::new`].
///
/// # Errors
///
/// [`BuildLeapfrogError::NoVerticalGrid`] for a galaxy without the full potential
/// ([`Galaxy::with_full_potential`]); as [`Spray::new`] otherwise.
pub fn spray(galaxy: &Galaxy, spec: &StreamSpec) -> Result<Vec<Tracer>, BuildLeapfrogError> {
    let tables = galaxy.potential();
    let force = SprayForce::new(tables)?;
    let spray = Spray::new(galaxy.seed(), &force, tables, spec, &SprayParams::default())?;
    Ok(spray.tracers())
}

/// The least and greatest distances from the galactic centre, ly, over the step boundaries of the
/// body that starts at `position` and `velocity` and takes `fixed`'s steps in `force`'s field:
/// the radii the global list's bounding shells are widened from (P10.T4,
/// [`GlobalList`](super::GlobalList)).
///
/// Over a specification's stripping time backwards these are the steps [`Spray::new`] takes on
/// its backward pass, in the same field from the same state, so the body is the spray's
/// progenitor. The turning points are not refined between boundaries, as
/// [`OrbitSummary`](super::orbit::OrbitSummary)'s are: at the step rule's 64 steps a pericentre
/// crossing a turning point lies within about `r̈ h² ÷ 8 ≈ r_p ÷ 32,768` of the nearer boundary's
/// distance (`r̈` near `v_p² ÷ r_p`), under 10⁻⁴ of itself, which the shells' widening dwarfs.
#[must_use]
pub(super) fn radii_over_ly(
    force: &SprayForce,
    position: GalacticDisplacement,
    velocity: GalacticVelocity,
    fixed: FixedStep,
) -> [f64; 2] {
    let field = |x: [f64; 3]| force.acceleration_m_s2(&GalacticDisplacement::new(x));
    let mut body = (position.metres(), velocity.metres_per_second());
    let mut acceleration = field(body.0);
    let h = fixed.step().value();
    let start = dot(body.0, body.0);
    let (mut least, mut greatest) = (start, start);
    for _ in 0..fixed.count() {
        kick_drift_kick(&mut body, &mut acceleration, h, field);
        let r2 = dot(body.0, body.0);
        // Squared distances are never NaN or a negative zero, so the comparisons are exact.
        if r2 < least {
            least = r2;
        }
        if r2 > greatest {
            greatest = r2;
        }
    }
    [least, greatest].map(|r2| r2.sqrt() / METRES_PER_LIGHT_YEAR)
}

/// A body's position (m) and velocity (m/s).
type Body = ([f64; 3], [f64; 3]);

/// One kick–drift–kick step of `h` seconds of `body`, whose acceleration at its position is
/// `acceleration` and is left at its new one, in the field `field` (m/s²): the leapfrog's step
/// ([`Leapfrog::step`](super::orbit::Leapfrog::step)).
fn kick_drift_kick(
    body: &mut Body,
    acceleration: &mut [f64; 3],
    h: f64,
    field: impl Fn([f64; 3]) -> [f64; 3],
) {
    let half = 0.5 * h;
    let (x, v) = body;
    for (v, a) in v.iter_mut().zip(*acceleration) {
        *v += a * half;
    }
    for (x, v) in x.iter_mut().zip(*v) {
        *x += v * h;
    }
    *acceleration = field(*x);
    for (v, a) in v.iter_mut().zip(*acceleration) {
        *v += a * half;
    }
}

/// The angle between two positions about the centre, radians: what a body sweeps in a step.
#[must_use]
fn swept(before: [f64; 3], after: [f64; 3]) -> f64 {
    let normal = cross(before, after);
    math::atan2(dot(normal, normal).sqrt(), dot(before, after))
}

/// The progenitor's boundary record of `body` at swept angle `angle` (radians).
#[must_use]
fn boundary((position_m, velocity_m_s): Body, swept_rad: f64) -> Boundary {
    Boundary {
        position_m,
        velocity_m_s,
        position_ly: position_m.map(|c| c / METRES_PER_LIGHT_YEAR),
        swept_rad,
    }
}

/// The release frame of a progenitor at `position` moving at `velocity`: its galactocentric
/// direction, the direction ahead of it in its orbital plane, and its orbit's pole. A body at the
/// centre takes `x`, and one with no angular momentum a pole perpendicular to its direction.
#[must_use]
fn frame(position: [f64; 3], velocity: [f64; 3]) -> [[f64; 3]; 3] {
    let unit = |v: [f64; 3]| {
        let length = dot(v, v).sqrt();
        (length > 0.0).then(|| v.map(|c| c / length))
    };
    let outward = unit(position).unwrap_or([1.0, 0.0, 0.0]);
    let pole = unit(cross(position, velocity))
        .or_else(|| {
            let axis = if outward[2].abs() < 0.9 {
                [0.0, 0.0, 1.0]
            } else {
                [1.0, 0.0, 0.0]
            };
            unit(cross(outward, axis))
        })
        .expect("a unit vector crossed with an axis 26° or more from it is not zero");
    [outward, cross(pole, outward), pole]
}

#[cfg(test)]
mod tests;
