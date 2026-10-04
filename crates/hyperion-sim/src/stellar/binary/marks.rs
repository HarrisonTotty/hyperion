//! A binary's timeline built from the marks that define it now (plan 11, P11.T5, design note 10):
//! the catalogue side's route to [`carved_class`](super::carved_class), which does not run the
//! engine.
//!
//! A catalogue entry draws what defines its class now: for an accreting white dwarf or an X-ray
//! binary the phase it is in, with the two stars, the orbit and the transfer rate; for a merger its
//! time T, the pair before it and the star it leaves. [`BinaryTimeline::from_marks`] builds from
//! those a timeline of a detached history and the phase that holds now, or of the pair up to T and
//! the merged star after, each star held at its marked state ([`MarkedPhase`], [`MarkedMerger`]).
//! The entry is kept only if `carved_class` of that timeline is its own class, the same function
//! the grid applies to an evolved timeline, and the state it gives at the marked age is the marked
//! state exactly.
//!
//! Plan 15's table will draw the history before the marked phase (P15.T10.b). Until it does, the
//! history holds the marked stars on the marked orbit, which only a horizon reaching back before
//! the phase can see.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::orbit::KeplerElements;
use crate::stellar::draws::StarDraws;
use crate::stellar::{Composition, StarState};
use crate::time::CLOCK_WINDOW_H;
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{SolarMassesPerYear, SolarRadii, Years};

use super::params::BinaryParams;
use super::star::{Member, Path};
use super::timeline::{BinaryTimeline, Context, OrbitPath, Segment, SegmentKind};

/// The phase a pair is in at its marked age: its kind, the ages it runs between, each star's
/// state and the orbit then, and during stable transfer the donor's rate.
///
/// A plain bundle of marks, as `StarStateParts` is one of a star's values: [`BinaryMarks::phase`]
/// checks it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkedPhase {
    /// [`SegmentKind::Detached`], [`SegmentKind::StableTransfer`] or [`SegmentKind::Contact`].
    pub kind: SegmentKind,
    /// The age at which the phase began, Julian years since the stars' onset of collapse.
    pub start: Years,
    /// The age at which it ends, Julian years since the stars' onset of collapse.
    pub end: Years,
    /// Each star's state, primary first.
    pub stars: [StarState; 2],
    /// The relative orbit.
    pub orbit: KeplerElements,
    /// The rate of transfer from the donor, M☉ yr⁻¹, during stable transfer only.
    pub transfer_rate: Option<SolarMassesPerYear>,
}

/// A merger at a marked age: the phase the pair is in before it, the two stars and the orbit then,
/// and the star it leaves.
///
/// A plain bundle of marks: [`BinaryMarks::merger`] checks it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkedMerger {
    /// The age of the merger, T, Julian years since the stars' onset of collapse.
    pub age: Years,
    /// The pair's phase before T: [`SegmentKind::Detached`] for an inspiral,
    /// [`SegmentKind::Contact`] or [`SegmentKind::StableTransfer`].
    pub before: SegmentKind,
    /// Each star's state before T, primary first.
    pub stars: [StarState; 2],
    /// The relative orbit before T.
    pub orbit: KeplerElements,
    /// The rate of transfer before T, during stable transfer only.
    pub transfer_rate: Option<SolarMassesPerYear>,
    /// The merged star after T, held at this state.
    pub product: StarState,
}

/// What defines a pair now.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Present {
    Phase(MarkedPhase),
    Merger(MarkedMerger),
}

/// The marks a catalogue entry draws for its binary (plan 11, design note 10), from which
/// [`BinaryTimeline::from_marks`] builds its timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryMarks {
    composition: Composition,
    draws: [StarDraws; 2],
    params: BinaryParams,
    age_at_epoch: Years,
    present: Present,
}

/// Why [`BinaryMarks::phase`] or [`BinaryMarks::merger`] refused its marks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildBinaryMarksError {
    /// An age is negative or not finite, or a phase ends before it begins.
    AgesOutOfOrder,
    /// A phase's kind is not one a pair can be marked in (a common envelope, a merged or disrupted
    /// pair).
    UnmarkableKind(SegmentKind),
    /// A transfer rate was given outside stable transfer, or none, or one not positive and finite,
    /// during it.
    TransferRate,
}

impl fmt::Display for BuildBinaryMarksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AgesOutOfOrder => {
                write!(
                    f,
                    "a marked phase runs forward from a finite, non-negative age"
                )
            }
            Self::UnmarkableKind(kind) => write!(f, "a pair cannot be marked in {kind:?}"),
            Self::TransferRate => write!(
                f,
                "a transfer rate is positive during stable transfer and absent otherwise"
            ),
        }
    }
}

impl Error for BuildBinaryMarksError {}

/// Checks a marked phase's kind and rate.
fn check_phase(
    kind: SegmentKind,
    rate: Option<SolarMassesPerYear>,
) -> Result<(), BuildBinaryMarksError> {
    match kind {
        SegmentKind::StableTransfer { .. } => match rate {
            Some(r) if r.value().is_finite() && r.value() > 0.0 => Ok(()),
            Some(_) | None => Err(BuildBinaryMarksError::TransferRate),
        },
        SegmentKind::Detached | SegmentKind::Contact => match rate {
            None => Ok(()),
            Some(_) => Err(BuildBinaryMarksError::TransferRate),
        },
        SegmentKind::CommonEnvelope | SegmentKind::Merged | SegmentKind::Disrupted { .. } => {
            Err(BuildBinaryMarksError::UnmarkableKind(kind))
        }
    }
}

/// Whether `age` is finite and non-negative.
#[must_use]
fn valid_age(age: Years) -> bool {
    age.value().is_finite() && age.value() >= 0.0
}

impl BinaryMarks {
    /// The marks of a pair of `composition`, with the stars' `draws`, whose system is
    /// `age_at_epoch` old at the epoch, in `phase` then.
    ///
    /// # Errors
    ///
    /// [`BuildBinaryMarksError::AgesOutOfOrder`] if an age is negative or not finite or the phase
    /// ends before it begins; [`BuildBinaryMarksError::UnmarkableKind`] for a phase that is not
    /// detached, stable transfer or contact; [`BuildBinaryMarksError::TransferRate`] for a rate
    /// that is missing or not positive during stable transfer, or given outside it.
    pub fn phase(
        composition: Composition,
        draws: [StarDraws; 2],
        age_at_epoch: Years,
        phase: MarkedPhase,
    ) -> Result<Self, BuildBinaryMarksError> {
        if !(valid_age(age_at_epoch)
            && valid_age(phase.start)
            && valid_age(phase.end)
            && phase.end >= phase.start)
        {
            return Err(BuildBinaryMarksError::AgesOutOfOrder);
        }
        check_phase(phase.kind, phase.transfer_rate)?;
        Ok(Self {
            composition,
            draws,
            params: BinaryParams::default(),
            age_at_epoch,
            present: Present::Phase(phase),
        })
    }

    /// The marks of a pair of `composition`, with the stars' `draws`, whose system is
    /// `age_at_epoch` old at the epoch, that merges as `merger` says.
    ///
    /// # Errors
    ///
    /// [`BuildBinaryMarksError::AgesOutOfOrder`] if an age is negative or not finite;
    /// [`BuildBinaryMarksError::UnmarkableKind`] and [`BuildBinaryMarksError::TransferRate`] for
    /// the phase before the merger, as [`BinaryMarks::phase`] checks it.
    pub fn merger(
        composition: Composition,
        draws: [StarDraws; 2],
        age_at_epoch: Years,
        merger: MarkedMerger,
    ) -> Result<Self, BuildBinaryMarksError> {
        if !(valid_age(age_at_epoch) && valid_age(merger.age)) {
            return Err(BuildBinaryMarksError::AgesOutOfOrder);
        }
        check_phase(merger.before, merger.transfer_rate)?;
        Ok(Self {
            composition,
            draws,
            params: BinaryParams::default(),
            age_at_epoch,
            present: Present::Merger(merger),
        })
    }

    /// The system's age at the epoch.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }
}

/// An orbit that stays as marked.
#[must_use]
fn fixed_orbit(orbit: KeplerElements) -> OrbitPath {
    OrbitPath {
        orientation: *orbit.orientation(),
        mean_anomaly: orbit.mean_anomaly_at_epoch(),
        axis: Path::starting(0.0, orbit.semi_major_axis().value() / SOLAR_RADIUS_M),
        eccentricity: Path::starting(0.0, orbit.eccentricity().value()),
        fixed: Some(orbit),
    }
}

/// Both stars held at their marked states. A marked star is never evolved, so nothing reads its
/// core radius, which is held at zero (P11.T4.g).
#[must_use]
fn frozen(stars: [StarState; 2]) -> [Member; 2] {
    stars.map(|state| Member::Frozen {
        state,
        core_radius: SolarRadii::ZERO,
    })
}

/// A segment of `kind` over `span` (its first and last ages) holding `members` on `orbit`, at
/// `rate` during stable transfer.
#[must_use]
fn held(
    kind: SegmentKind,
    span: (f64, f64),
    members: [StarState; 2],
    orbit: KeplerElements,
    rate: Option<SolarMassesPerYear>,
) -> Segment {
    let (start, end) = span;
    Segment::new(
        kind,
        start,
        end,
        frozen(members),
        Some(fixed_orbit(orbit)),
        rate.map(|r| Path::starting(start, r.value())),
    )
}

impl BinaryTimeline {
    /// The timeline that `marks` define (plan 11's design note 10), without running the engine.
    ///
    /// For a marked phase: a detached history from zero age to the phase's start, the phase to its
    /// end, where the timeline stops, each star held at its marked state on the marked orbit. For
    /// a merger: the marked phase before it from zero age to T, then the merged star, held at its
    /// marked state, from T to H after the later of T and the epoch. Its [`state_at`] the marked
    /// phase's ages is the marked state exactly; it records no supernova and pools no Type Ia
    /// candidate.
    ///
    /// # Examples
    ///
    /// A catalogue entry of a dwarf nova, drawn now in its phase of transfer, is an accreting
    /// white dwarf by the same test the grid applies to an evolved pair:
    ///
    /// ```
    /// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// use hyperion_sim::stellar::binary::{
    ///     BinaryMarks, BinaryTimeline, CarvedClass, Component, MarkedPhase, SegmentKind,
    ///     carved_class,
    /// };
    /// use hyperion_sim::stellar::draws::StarDraws;
    /// use hyperion_sim::stellar::{Composition, Phase, StarState, StarStateParts};
    /// use hyperion_sim::units::{
    ///     GravitationalParameter, Radians, Seconds, SolarLuminosities, SolarMasses,
    ///     SolarMassesPerYear, SolarRadii, Years,
    /// };
    ///
    /// let star = |phase, mass: f64, luminosity: f64, radius: f64| {
    ///     StarState::new(StarStateParts {
    ///         phase,
    ///         age: Years::new(5.0e9),
    ///         mass: SolarMasses::new(mass),
    ///         core_mass: SolarMasses::new(if phase == Phase::MainSequence { 0.0 } else { mass }),
    ///         luminosity: SolarLuminosities::new(luminosity),
    ///         radius: SolarRadii::new(radius),
    ///         mass_loss_rate: SolarMassesPerYear::ZERO,
    ///         phase_fraction: 0.5,
    ///     })
    /// };
    /// let orbit = KeplerElements::from_period(
    ///     Seconds::new(3.0 * 3_600.0),
    ///     GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
    ///     Eccentricity::CIRCULAR,
    ///     Orientation::new(Radians::new(0.5), Radians::new(0.0), Radians::new(0.0))?,
    ///     Radians::new(0.0),
    /// )?;
    /// let now = Years::new(5.0e9);
    /// let marks = BinaryMarks::phase(
    ///     Composition::SOLAR,
    ///     [StarDraws::median(), StarDraws::median()],
    ///     now,
    ///     MarkedPhase {
    ///         kind: SegmentKind::StableTransfer { donor: Component::Secondary },
    ///         start: Years::new(4.0e9),
    ///         end: Years::new(6.0e9),
    ///         stars: [
    ///             star(Phase::CarbonOxygenWhiteDwarf, 0.8, 1.0e-3, 0.011),
    ///             star(Phase::MainSequence, 0.2, 5.0e-3, 0.22),
    ///         ],
    ///         orbit,
    ///         transfer_rate: Some(SolarMassesPerYear::new(1.0e-10)),
    ///     },
    /// )?;
    /// let timeline = BinaryTimeline::from_marks(marks);
    /// assert!(matches!(
    ///     carved_class(&timeline, now),
    ///     Some(CarvedClass::AccretingWdFast | CarvedClass::AccretingWdSlow)
    /// ));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// [`state_at`]: BinaryTimeline::state_at
    #[must_use]
    pub fn from_marks(marks: BinaryMarks) -> Self {
        let context = Arc::new(Context::from_parts(
            marks.composition,
            marks.draws,
            marks.params,
            marks.age_at_epoch,
        ));
        let (segments, until) = match marks.present {
            Present::Phase(phase) => {
                let (start, end) = (phase.start.value(), phase.end.value());
                let mut segments = Vec::with_capacity(2);
                if start > 0.0 {
                    segments.push(held(
                        SegmentKind::Detached,
                        (0.0, start),
                        phase.stars,
                        phase.orbit,
                        None,
                    ));
                }
                segments.push(held(
                    phase.kind,
                    (start, end),
                    phase.stars,
                    phase.orbit,
                    phase.transfer_rate,
                ));
                (segments, end)
            }
            Present::Merger(merger) => {
                let t = merger.age.value();
                let until =
                    t.max(marks.age_at_epoch.value()) + CLOCK_WINDOW_H.as_julian_years_f64();
                let before = held(
                    merger.before,
                    (0.0, t),
                    merger.stars,
                    merger.orbit,
                    merger.transfer_rate,
                );
                let after = Segment::new(
                    SegmentKind::Merged,
                    t,
                    until,
                    [
                        Member::Frozen {
                            state: merger.product,
                            core_radius: SolarRadii::ZERO,
                        },
                        Member::Gone,
                    ],
                    None,
                    None,
                );
                (vec![before, after], until)
            }
        };
        Self::new(segments, None, Vec::new(), until, context, false)
    }
}
