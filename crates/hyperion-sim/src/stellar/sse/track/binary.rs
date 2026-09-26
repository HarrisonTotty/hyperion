//! Plan 11's entry points into the track (P11.T4): the naked helium star of ruling 34.1 of
//! 2026-09-22 as a [`Track`], and what the binary engine of Hurley, Tout and Pols (2002, MNRAS 329,
//! 897, "BSE") needs to read of a star whose mass a companion sets.
//!
//! Ruling 34.1 has a star stripped to its helium core reach plan 11 as a `Track` like any other,
//! so that its winds, remnant and death stay in plan 06's one place, with
//! [`HeliumStar`] crate-private. [`Track::helium_star`] and [`Track::helium_star_full`] are that
//! constructor. A helium star's age is counted from the start of its track, the moment it becomes
//! a naked helium star (its helium zero-age main sequence for the public constructors), so that a
//! binary places it with an offset, as it places every star.
//!
//! The rest is crate-private and read only by `stellar::binary`:
//!
//! - [`Track::structure_at`], a star's state on its own track's closed forms at a mass the binary
//!   gives it, with the core radius and convective envelope that BSE's tides, braking and common
//!   envelopes read (HPT section 7.2; BSE section 2.3.1, equations 36–40);
//! - [`main_sequence_structure`], the same for a main-sequence or helium main-sequence star whose
//!   mass and fractional age the binary carries (HPT section 7.1; BSE section 2.6.6);
//! - [`Track::remains_at`], what the star leaves when a companion or a common envelope removes
//!   its envelope (HPT section 6 and equation 76, as the track's own envelope losses do);
//! - [`new_star_mass`] and [`Track::age_in_phase`], which place a merger's product on a track by its
//!   core (BSE section 2.7.4).

use crate::math;
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::remnant::structure::{OXYGEN_NEON_MC_BAGB, white_dwarf_radius};
use crate::stellar::{Composition, Phase, StarState, StarStateParts};
use crate::units::{
    Megayears, SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years,
};

use super::super::agb::CHANDRASEKHAR_MSUN;
use super::super::coeffs::ZCoeffs;
use super::super::gb::{self, GiantBranch};
use super::super::helium::{self, HeliumStar};
use super::super::ms::{self, MainSequence};
use super::super::wind;
use super::build::{Builder, Entry, Keep, Resolution};
use super::model::{HeliumCore, Model};
use super::{Coordinate, MAX_INITIAL_MASS, Track, TrackOptions, reimers_eta};

/// k′₂ of BSE equation 35 (HPT equation 109): the envelope's share of I ÷ (M R²).
pub(crate) const ENVELOPE_GYRATION: f64 = 0.1;

/// k′₃ of BSE equation 35 (HPT equation 109): a core's I ÷ (M R²), an n = 3/2 polytrope.
pub(crate) const CORE_GYRATION: f64 = 0.21;

/// Whether `x` is positive: false for zero, a negative number and NaN.
#[must_use]
fn positive(x: f64) -> bool {
    x > 0.0
}

/// Bisections of the root searches that place a new star (BSE section 2.7.4).
const PLACEMENT_BISECTIONS: u32 = 60;

impl Track {
    /// A naked helium star of mass `m` from its helium zero-age main sequence, built until it
    /// covers `age_max`, years since then, under the generator's options (ruling 34.1 of
    /// 2026-09-22).
    ///
    /// This is the track plan 11 gives a star a companion strips to its helium core (P11.T4): the
    /// helium main sequence, Hertzsprung gap and giant branch of HPT section 6.1 with the wind of
    /// [`WindRecipe`](super::super::WindRecipe) and the death and remnant of every other track. Its
    /// age is counted from the helium zero-age main sequence. A helium star lighter than the core
    /// at helium ignition of an `M_HeF` star, about 0.33 M☉, cannot burn helium and is a helium
    /// white dwarf from the start, as in the published SSE code.
    ///
    /// # Panics
    ///
    /// In debug builds, if `m` is not in (0, 100] M☉ or `age_max` is not finite and non-negative.
    ///
    /// # Examples
    ///
    /// A 4 M☉ helium star burns helium for about 1.5 Myr and dies as a supernova:
    ///
    /// ```
    /// use hyperion_sim::stellar::draws::StarDraws;
    /// use hyperion_sim::stellar::sse::Track;
    /// use hyperion_sim::stellar::{Composition, Phase};
    /// use hyperion_sim::units::{SolarMasses, Years};
    ///
    /// let star = Track::helium_star_full(SolarMasses::new(4.0), &Composition::SOLAR, &StarDraws::median());
    /// assert_eq!(star.state_at(Years::new(1.0e6)).phase(), Phase::HeliumMainSequence);
    /// let death = star.death().ok_or("a full track reaches its death")?;
    /// assert!(death.kind().is_sudden());
    /// assert!((1.5e6..2.0e6).contains(&death.age().value()));
    /// # Ok::<(), &str>(())
    /// ```
    #[must_use]
    pub fn helium_star(
        m: SolarMasses,
        comp: &Composition,
        draws: &StarDraws,
        age_max: Years,
    ) -> Self {
        debug_assert!(
            age_max.value().is_finite() && age_max.value() >= 0.0,
            "a track is built to a finite, non-negative age: {age_max:?}"
        );
        Self::helium_star_from(
            checked_helium_mass(m),
            0.0,
            comp,
            draws,
            Some(age_max.value().max(0.0)),
        )
    }

    /// [`Track::helium_star`] built in full, to its death and remnant.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`Track::helium_star`].
    #[must_use]
    pub fn helium_star_full(m: SolarMasses, comp: &Composition, draws: &StarDraws) -> Self {
        Self::helium_star_from(checked_helium_mass(m), 0.0, comp, draws, None)
    }

    /// A naked helium star of mass `m` entering its helium main sequence at fractional age
    /// `tau0` (HPT equation 76: the helium star core helium burning leaves), built until it covers
    /// `age_max` years from then, or in full with `None`.
    #[must_use]
    pub(crate) fn helium_star_from(
        m: SolarMasses,
        tau0: f64,
        comp: &Composition,
        draws: &StarDraws,
        age_max: Option<f64>,
    ) -> Self {
        Self::build_from_entry(
            Entry::HeliumMainSequence {
                mass: m.value(),
                tau0: tau0.clamp(0.0, 1.0),
            },
            m,
            comp,
            draws,
            age_max,
        )
    }

    /// The track of a star entering its life at `entry` at age zero, whose initial mass is
    /// reported as `initial_mass`, under the generator's options. A star built so has lost its
    /// envelope to a companion, which its electron-capture window reads (plan 06, design note 11).
    #[must_use]
    fn build_from_entry(
        entry: Entry,
        initial_mass: SolarMasses,
        comp: &Composition,
        draws: &StarDraws,
        age_max: Option<f64>,
    ) -> Self {
        let options = TrackOptions::default();
        let coeffs = ZCoeffs::new(comp.z_fit());
        let eta = reimers_eta(draws.eta().value());
        let outcome = Builder::new(
            &coeffs,
            comp,
            options,
            eta,
            RemnantDraws::of(draws),
            Resolution::GENERATOR,
            Keep::Track,
        )
        .stripped_by_companion(true)
        .run_from(entry, age_max);
        Self {
            coeffs,
            composition: *comp,
            options,
            initial_mass,
            eta,
            segments: outcome.segments,
            fate: outcome.fate,
            built_until: outcome.built_until,
        }
    }

    /// The star's structure at `age` on its track's closed forms, with its mass set to `mass` by a
    /// companion (BSE section 2): its state with the wind at that mass, and the core radius and
    /// convective envelope that tides, magnetic braking and common envelopes read.
    ///
    /// The closed forms are those of the segment that holds `age` at the track's own coordinate
    /// there; only the radius and the wind read the mass, as HPT section 7.1 has every phase after
    /// the main sequence do. A remnant keeps its own mass.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`Track::state_at`], or if `mass` is not positive and finite.
    #[must_use]
    pub(crate) fn structure_at(&self, age: f64, mass: f64) -> Structure {
        debug_assert!(
            mass.is_finite() && mass > 0.0,
            "a star has a positive mass: {mass}"
        );
        let age = self.checked_age(Years::new(age));
        let segment = self.segment_at(age);
        let (coord, own_mass) = segment.coordinate_and_mass(age);
        let phys = self.physics();
        let evaluated = if segment.model.is_remnant_model() {
            segment.evaluate_at(&phys, age, coord, own_mass)
        } else {
            segment.evaluate_at(&phys, age, coord, mass)
        };
        let mut state = self.star_state(&evaluated, age);
        if !segment.model.is_remnant_model() && state.mass().value() > mass {
            // The closed forms hold the mass above the core; a star the binary has stripped to
            // below it is all core, at the binary's mass.
            state = StarState::new(StarStateParts {
                mass: SolarMasses::new(mass),
                core_mass: SolarMasses::new(mass.min(state.core_mass().value())),
                ..state_parts(&state)
            });
        }
        let helium_core = segment.model.helium_core();
        let core_radius = core_radius(&segment.model, &state, coord, &self.coeffs);
        let envelope =
            convective_envelope(&segment.model, &state, coord, core_radius, &self.coeffs);
        Structure {
            state,
            core_radius,
            envelope,
            degenerate_core: helium_core == Some(HeliumCore::Degenerate),
            burnt: burnt_fraction(&segment.model, coord),
            phase_end: segment.end,
        }
    }

    /// The fractional age τ of the star's main sequence or helium main sequence at `age`, if it
    /// is on one: the coordinate its effective-age rule keeps (HPT section 7.1).
    #[must_use]
    pub(crate) fn main_sequence_fraction(&self, age: f64) -> Option<f64> {
        let age = self.checked_age(Years::new(age));
        let segment = self.segment_at(age);
        match segment.model {
            Model::MainSequence { .. } | Model::HeliumMainSequence { .. } => {
                Some(segment.coordinate_and_mass(age).0.clamp(0.0, 1.0))
            }
            _ => None,
        }
    }

    /// What the star leaves at `age` when its envelope is removed at once, by a companion's Roche
    /// lobe or a common envelope: its naked core as the track's own loss of the envelope leaves it
    /// (HPT section 6, BSE section 2.7.1), with the helium star's track built to cover `age_max`
    /// years after the stripping, or in full with `None`.
    ///
    /// - A Hertzsprung-gap or giant-branch star leaves a helium white dwarf of its core if the
    ///   core is degenerate (initial mass up to `M_HeF`) and a naked helium star at zero age
    ///   otherwise; a star in the helium flash, a zero-age helium star.
    /// - A core-helium-burning star leaves the helium star of its core at the same fractional age
    ///   (HPT equation 76).
    /// - An early-AGB star leaves the helium giant whose carbon–oxygen core its own is
    ///   (`HeliumStar::from_early_agb`).
    /// - A thermally pulsing AGB star and a helium giant leave the white dwarf of their core:
    ///   carbon–oxygen, or oxygen–neon from a core at the base of the AGB (or a helium star) of
    ///   1.6 M☉ (HPT sections 6.1 and 6.2.1).
    /// - A main-sequence star, a helium main-sequence star and a remnant have no envelope to lose.
    #[must_use]
    pub(crate) fn remains_at(
        &self,
        age: f64,
        mass: f64,
        draws: &StarDraws,
        age_max: Option<f64>,
    ) -> Remains {
        let age = self.checked_age(Years::new(age));
        let index = self
            .segments
            .partition_point(|s| s.start <= age)
            .saturating_sub(1);
        let segment = &self.segments[index];
        let (coord, _) = segment.coordinate_and_mass(age);
        let phys = self.physics();
        let point = segment
            .evaluate_at(&phys, age, coord, segment.mass)
            .point
            .point;
        // A companion may have taken the star below its own core: what is left is no more than the
        // star's mass.
        let mc = point.core_mass.value().min(mass);
        let luminosity = point.luminosity;
        let helium_star = |mass: f64, tau0: f64| {
            Remains::HeliumStar(Box::new(Self::helium_star_from(
                SolarMasses::new(mass),
                tau0,
                &self.composition,
                draws,
                age_max,
            )))
        };
        match &segment.model {
            Model::MainSequence { .. }
            | Model::HeliumMainSequence { .. }
            | Model::Remnant { .. } => Remains::Nothing,
            Model::HertzsprungGap { core, .. } | Model::FirstGiantBranch { core, .. } => match core
            {
                HeliumCore::Degenerate => Remains::WhiteDwarf {
                    phase: Phase::HeliumWhiteDwarf,
                    mass: SolarMasses::new(mc),
                    last_luminosity: luminosity,
                },
                HeliumCore::NonDegenerate => helium_star(mc, 0.0),
            },
            Model::FlashBridge { .. } => helium_star(mc, 0.0),
            Model::CoreHeliumBurning { .. } => helium_star(mc, coord),
            Model::EarlyAgb { phase, span, .. } => {
                let (star, clock0) = HeliumStar::from_early_agb(phase, span.at(coord));
                let mass = SolarMasses::new(star.mass().value().min(mass));
                Remains::HeliumStar(Box::new(Self::build_from_entry(
                    Entry::HeliumShellBurning {
                        star: Box::new(star),
                        clock0,
                        mass: mass.value(),
                    },
                    mass,
                    &self.composition,
                    draws,
                    age_max,
                )))
            }
            Model::ThermallyPulsingAgb { .. } => {
                let oxygen_neon = self.segments[..index].iter().rev().any(|s| {
                    matches!(&s.model, Model::EarlyAgb { phase, .. } if phase.mc_bagb() >= OXYGEN_NEON_MC_BAGB)
                });
                Remains::WhiteDwarf {
                    phase: if oxygen_neon {
                        Phase::OxygenNeonWhiteDwarf
                    } else {
                        Phase::CarbonOxygenWhiteDwarf
                    },
                    mass: SolarMasses::new(mc),
                    last_luminosity: luminosity,
                }
            }
            Model::HeliumShellBurning { star, .. } => Remains::WhiteDwarf {
                phase: if star.mass() >= OXYGEN_NEON_MC_BAGB {
                    Phase::OxygenNeonWhiteDwarf
                } else {
                    Phase::CarbonOxygenWhiteDwarf
                },
                mass: SolarMasses::new(mc.min(CHANDRASEKHAR_MSUN)),
                last_luminosity: luminosity,
            },
        }
    }

    /// The age on this track at which the star is at `fraction` (0–1) through the first segment
    /// of `phase`: its fractional age τ on a main sequence (found by bisection on the segment's
    /// coordinate), and the share of the segment's span otherwise. `None` if the track has no
    /// such segment as built. For placing a merger's product (BSE section 2.7.4).
    #[must_use]
    pub(crate) fn age_in_phase(&self, phase: Phase, fraction: f64) -> Option<f64> {
        let fraction = fraction.clamp(0.0, 1.0);
        let phys = self.physics();
        let segment = self.segments.iter().find(|s| {
            let probe = s.start + 1e-9 * (s.end - s.start).min(1.0);
            s.evaluate(&phys, probe).point.phase == phase
        })?;
        let (start, end) = (segment.start, segment.end);
        if !end.is_finite() {
            return Some(start);
        }
        Some(match segment.coordinate {
            Coordinate::Fraction { tau0 } => {
                if fraction <= tau0 {
                    start
                } else {
                    let (mut lo, mut hi) = (start, end);
                    for _ in 0..PLACEMENT_BISECTIONS {
                        let mid = lo + 0.5 * (hi - lo);
                        if segment.coordinate_and_mass(mid).0 < fraction {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    hi
                }
            }
            Coordinate::Linear { .. } => (1.0 - fraction) * start + fraction * end,
        })
    }

    /// The remnant's phase, the age at which the track forms it and its white dwarf's cooling
    /// origin (`white_dwarf::cooling_origin`), if the track is built to its remnant: what a binary
    /// that feeds the remnant takes over.
    #[must_use]
    pub(crate) fn remnant_clock(&self) -> Option<(Phase, f64, Megayears)> {
        match self.segments.last().map(|s| &s.model) {
            Some(Model::Remnant {
                phase,
                birth,
                origin,
                ..
            }) => Some((*phase, *birth, *origin)),
            _ => None,
        }
    }

    /// The start and end of the track's segment that holds `age`, years: the phase boundaries
    /// the binary engine's steps stop at.
    #[must_use]
    pub(crate) fn phase_span(&self, age: f64) -> (f64, f64) {
        let age = self.checked_age(Years::new(age));
        let segment = self.segment_at(age);
        (segment.start, segment.end)
    }
}

impl Model {
    /// Whether the model is a remnant's.
    #[must_use]
    fn is_remnant_model(&self) -> bool {
        matches!(self, Self::Remnant { .. })
    }

    /// The degeneracy of the helium core, for the phases that carry it.
    #[must_use]
    fn helium_core(&self) -> Option<HeliumCore> {
        match self {
            Self::HertzsprungGap { core, .. } | Self::FirstGiantBranch { core, .. } => Some(*core),
            Self::MainSequence { .. }
            | Self::FlashBridge { .. }
            | Self::CoreHeliumBurning { .. }
            | Self::EarlyAgb { .. }
            | Self::ThermallyPulsingAgb { .. }
            | Self::HeliumMainSequence { .. }
            | Self::HeliumShellBurning { .. }
            | Self::Remnant { .. } => None,
        }
    }
}

/// A star's structure as the binary engine reads it (BSE section 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Structure {
    /// The state, with the wind's rate at the star's mass.
    pub(crate) state: StarState,
    /// The core's radius, R☉: zero on a main sequence; a naked helium star's of the core for a
    /// non-degenerate helium core and for core helium burning and the early AGB; five times a
    /// white dwarf's for a degenerate core, which is a hot subdwarf's; the whole star for a
    /// remnant (HPT section 6 as the published SSE code applies it, `hrdiag`).
    pub(crate) core_radius: SolarRadii,
    /// The convective envelope.
    pub(crate) envelope: ConvectiveEnvelope,
    /// Whether the star has a degenerate helium core (a giant of initial mass up to `M_HeF`), for
    /// the merger of two such cores (BSE section 2.7.2).
    pub(crate) degenerate_core: bool,
    /// y of BSE equation 82, the share of central helium burning done: 0 before helium ignition,
    /// the fraction of core helium burning, 1 after it; τ on the helium main sequence.
    pub(crate) burnt: f64,
    /// The end of the track's segment holding the age, years (infinite for a remnant or a star the
    /// binary carries itself).
    pub(crate) phase_end: f64,
}

/// A star's convective envelope: its mass (HPT section 7.2) and depth (BSE equations 36–40).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ConvectiveEnvelope {
    /// M☉.
    pub(crate) mass: f64,
    /// R☉.
    pub(crate) depth: f64,
}

/// What a star leaves when its envelope is removed at once ([`Track::remains_at`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Remains {
    /// No envelope to lose: a main-sequence star, a helium main-sequence star or a remnant.
    Nothing,
    /// A naked helium star, whose track starts at the stripping.
    HeliumStar(Box<Track>),
    /// A white dwarf of `phase` and `mass`, from a star of `last_luminosity` then, which its
    /// cooling law is matched to (`white_dwarf::cooling_origin`).
    WhiteDwarf {
        phase: Phase,
        mass: SolarMasses,
        last_luminosity: SolarLuminosities,
    },
}

/// The parts of `state`.
#[must_use]
fn state_parts(state: &StarState) -> StarStateParts {
    StarStateParts {
        phase: state.phase(),
        age: state.age(),
        mass: state.mass(),
        core_mass: state.core_mass(),
        luminosity: state.luminosity(),
        radius: state.radius(),
        mass_loss_rate: state.mass_loss_rate(),
        phase_fraction: state.phase_fraction(),
    }
}

/// The kinds of star a merger's product is placed as by its core (BSE section 2.7.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum NewStar {
    /// At the base of the first giant branch (k₃ = 3).
    GiantBranch,
    /// In core helium burning with `burnt` of it done (k₃ = 4, BSE equations 82–84).
    CoreHeliumBurning { burnt: f64 },
    /// At the base of the AGB (k₃ = 5).
    EarlyAgb,
    /// At the start of the thermally pulsing AGB (k₃ = 6, BSE equations 86 and 87).
    PulsingAgb,
    /// A helium star at the end of its main sequence (k₃ = 8 or 9).
    HeliumGiant,
}

/// Where a merger's product with a core of `mc` is placed ([`NewStar`]): the initial mass of the
/// track it follows, and the phase and fraction of it where it starts, or `None` where no star of
/// that kind has such a core. The caller builds the track (a hydrogen star, or a naked helium star
/// for [`NewStar::HeliumGiant`]) and finds the age with [`Track::age_in_phase`].
///
/// BSE section 2.7.4, with the paper's bisections: the base of the giant branch where
/// `Mc,BGB`(M₀) (HPT equation 44, or the giant branch's relation at `L_BGB` up to `M_HeF`) is the
/// core, a core-helium-burning star by equation 84, the base of the AGB where `Mc,BAGB`(M₀) (HPT
/// equation 66) is the core, the thermally pulsing AGB through equations 86 and 87, and a helium
/// giant where the luminosity at the end of the helium main sequence, `L_THe`, is the helium
/// giants' relation at the core (HPT equations 80 and 84).
#[must_use]
pub(crate) fn new_star_mass(
    kind: NewStar,
    mc: SolarMasses,
    c: &ZCoeffs,
) -> Option<(SolarMasses, Phase, f64)> {
    let mc = mc.value();
    if !positive(mc) {
        return None;
    }
    let m_hef = c.m_hef().value();
    let m_fgb = c.m_fgb().value();
    match kind {
        NewStar::GiantBranch => {
            let at_hef = gb::mc_bgb(SolarMasses::new(m_hef), c).value();
            let m0 = if mc > at_hef {
                let top = gb::mc_bgb(SolarMasses::new(m_fgb), c).value();
                if mc >= top {
                    return new_star_mass(
                        NewStar::CoreHeliumBurning { burnt: 0.0 },
                        SolarMasses::new(mc),
                        c,
                    );
                }
                bisect_increasing(m_hef, m_fgb, mc, |m| {
                    gb::mc_bgb(SolarMasses::new(m), c).value()
                })
            } else {
                let core_at_bgb = |m: f64| {
                    let m = SolarMasses::new(m);
                    GiantBranch::new(m, c).core_mass(ms::l_bgb(m, c)).value()
                };
                bisect_increasing(0.5, m_hef, mc, core_at_bgb)
            };
            Some((SolarMasses::new(m0), Phase::FirstGiantBranch, 0.0))
        }
        NewStar::CoreHeliumBurning { burnt } => {
            let y = burnt.clamp(0.0, 1.0);
            let hei = |m: f64| gb::mc_hei(SolarMasses::new(m), c).value();
            let bagb = |m: f64| gb::mc_bagb(SolarMasses::new(m), c).value();
            let m_min = if mc >= bagb(m_hef) {
                inverse_mc_bagb(mc, c)?
            } else {
                m_hef
            };
            let m_max = bisect_increasing(m_hef, MAX_INITIAL_MASS.value(), mc, hei);
            let core = |m: f64| (1.0 - y) * hei(m) + y * bagb(m);
            let (lo, hi) = (core(m_min) - mc, core(m_max) - mc);
            if lo > 0.0 || hi < 0.0 || lo.is_nan() || hi.is_nan() || m_max <= m_min {
                return new_star_mass(NewStar::GiantBranch, SolarMasses::new(mc), c)
                    .filter(|_| mc < hei(m_hef));
            }
            let m0 = bisect_increasing(m_min, m_max, mc, core);
            Some((SolarMasses::new(m0), Phase::CoreHeliumBurning, y))
        }
        NewStar::EarlyAgb => Some((
            SolarMasses::new(inverse_mc_bagb(mc, c)?),
            Phase::EarlyAgb,
            0.0,
        )),
        NewStar::PulsingAgb => {
            let helium_core = if mc >= 0.44 * 2.25 + 0.448 {
                (mc + 0.35) / 0.773
            } else if mc > 0.8 {
                (mc - 0.448) / 0.44
            } else {
                mc
            };
            let m0 = inverse_mc_bagb(helium_core, c)?;
            Some((SolarMasses::new(m0), Phase::ThermallyPulsingAgb, 0.0))
        }
        NewStar::HeliumGiant => {
            let mismatch = |m: f64| {
                let star = HeliumStar::new(SolarMasses::new(m));
                star.l_tms().value() - star.relation().luminosity(SolarMasses::new(mc)).value()
            };
            // L_THe rises with the helium star's mass faster than the relation at a fixed core.
            if mismatch(mc) >= 0.0 {
                return None;
            }
            let mut hi = 2.0 * mc;
            let mut found = false;
            for _ in 0..30 {
                if mismatch(hi) > 0.0 {
                    found = true;
                    break;
                }
                hi *= 2.0;
            }
            if !found {
                return None;
            }
            let m0 = bisect_increasing(mc, hi, 0.0, mismatch);
            Some((
                SolarMasses::new(m0.min(MAX_INITIAL_MASS.value())),
                Phase::HeliumHertzsprungGap,
                0.0,
            ))
        }
    }
}

/// The initial mass whose core at the base of the AGB is `mc` (HPT equation 66 inverted:
/// ((Mc⁴ − b38) ÷ b36)^(1 ÷ b37)), or `None` below the relation's floor.
#[must_use]
fn inverse_mc_bagb(mc: f64, c: &ZCoeffs) -> Option<f64> {
    let inner = (math::powi(mc, 4) - c.b(38)) / c.b(36);
    if !positive(inner) {
        return None;
    }
    let m = math::powf_positive(inner, 1.0 / c.b(37));
    (m.is_finite() && m > 0.0).then_some(m.min(MAX_INITIAL_MASS.value()))
}

/// The root in [`lo`, `hi`] of `f`(m) = `target` for an `f` that rises with m, by
/// [`PLACEMENT_BISECTIONS`] bisections; an end if the target lies outside.
#[must_use]
fn bisect_increasing(lo: f64, hi: f64, target: f64, f: impl Fn(f64) -> f64) -> f64 {
    let (mut lo, mut hi) = (lo, hi);
    if f(lo) >= target {
        return lo;
    }
    if f(hi) <= target {
        return hi;
    }
    for _ in 0..PLACEMENT_BISECTIONS {
        let mid = lo + 0.5 * (hi - lo);
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo + 0.5 * (hi - lo)
}

/// The structure of a main-sequence star (or, with `helium`, a helium main-sequence star) of
/// mass `m` at fractional age `tau` and `age`, whose initial mass is its current one (HPT section
/// 7.1), of composition `comp` whose coefficients are `c`, with the wind of its `draws`: the state
/// the binary engine gives a star whose mass it sets on the main sequence (BSE section 2.6.6), and
/// its main-sequence lifetime, years.
///
/// A hydrogen star lighter than 0.1 M☉ is the caller's (P06.T13's cooling fits).
///
/// # Panics
///
/// In debug builds, if `m` is not positive or `tau` lies outside [0, 1].
#[must_use]
pub(crate) fn main_sequence_structure(
    c: &ZCoeffs,
    comp: &Composition,
    draws: &StarDraws,
    helium: bool,
    m: f64,
    tau: f64,
    age: f64,
) -> (Structure, f64) {
    debug_assert!(
        m > 0.0 && (0.0..=1.0 + 1e-9).contains(&tau),
        "{m} M☉ at τ = {tau}"
    );
    let tau = tau.clamp(0.0, 1.0);
    let mass = SolarMasses::new(m);
    let (phase, point, t_ms) = if helium {
        let (point, t_ms) = helium::main_sequence_at_fraction(mass, tau);
        (Phase::HeliumMainSequence, point, t_ms)
    } else {
        let ms = MainSequence::new(mass, c);
        let point = ms.at(ms.t_ms() * tau);
        (Phase::MainSequence, point, ms.t_ms())
    };
    // The state's age is the effective age of HPT section 7.1, τ t_MS at the current mass.
    let _ = age;
    let parts = StarStateParts {
        phase,
        age: Years::new((tau * t_ms.value() * 1e6).max(0.0)),
        mass,
        core_mass: SolarMasses::ZERO,
        luminosity: point.luminosity,
        radius: point.radius,
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: tau,
    };
    let state = StarState::new(parts);
    let rate = wind::rate(
        TrackOptions::default().wind(),
        &state,
        comp,
        reimers_eta(draws.eta().value()),
    );
    let state = StarState::new(StarStateParts {
        mass_loss_rate: rate,
        ..parts
    });
    let envelope = if helium {
        ConvectiveEnvelope {
            mass: 0.0,
            depth: 0.0,
        }
    } else {
        main_sequence_envelope(m, tau, point.radius.value(), c)
    };
    (
        Structure {
            state,
            core_radius: SolarRadii::ZERO,
            envelope,
            degenerate_core: false,
            burnt: if helium { tau } else { 0.0 },
            phase_end: f64::INFINITY,
        },
        t_ms.value() * 1e6,
    )
}

/// The main-sequence lifetime of a star of mass `m`, years: HPT's `t_MS` for a hydrogen star and
/// equation 79's `t_HeMS` for a helium star (with `helium`).
#[must_use]
pub(crate) fn main_sequence_lifetime(c: &ZCoeffs, helium: bool, m: f64) -> f64 {
    let mass = SolarMasses::new(m);
    if helium {
        helium::main_sequence_lifetime(mass).value() * 1e6
    } else {
        ms::t_ms(mass, c).value() * 1e6
    }
}

/// The lightest helium main-sequence star that burns helium, M☉: the core at helium ignition of
/// an `M_HeF` star, about 0.33 M☉. A lighter one is a helium white dwarf (the published SSE code,
/// `hrdiag`, stellar type 7; see the track's `helium_main_sequence`).
#[must_use]
pub(crate) fn lightest_helium_star(c: &ZCoeffs) -> f64 {
    let m_hef = c.m_hef();
    GiantBranch::new(m_hef, c)
        .core_mass(gb::l_hei(m_hef, c))
        .value()
}

/// The exponent x of R ∝ M^−x on the giant branch (HPT equation 47), which sets the critical mass
/// ratio of a giant donor (BSE equation 57): 0.30406 + 0.0805ζ + 0.0897ζ² + 0.0878ζ³ + 0.0222ζ⁴
/// with ζ = log₁₀(Z ÷ 0.02).
#[must_use]
pub(crate) fn giant_radius_exponent(comp: &Composition) -> f64 {
    let zeta = math::log10(comp.z_fit().value() / 0.02);
    0.30406 + zeta * (0.0805 + zeta * (0.0897 + zeta * (0.0878 + zeta * 0.0222)))
}

/// The helium zero-age main sequence's radius of a helium star of mass `m`, R☉ (HPT equation 78),
/// a non-degenerate helium core's radius (BSE section 2.7.1).
#[must_use]
pub(crate) fn helium_zams_radius(m: f64) -> f64 {
    if m > 0.0 {
        helium::zams_radius(SolarMasses::new(m)).value()
    } else {
        0.0
    }
}

/// The core radius of a star in `model` with `state` at coordinate `coord` ([`Structure::core_radius`]).
#[must_use]
fn core_radius(model: &Model, state: &StarState, coord: f64, c: &ZCoeffs) -> SolarRadii {
    let mc = state.core_mass().value();
    let degenerate = |mc: f64| {
        if mc > 0.0 {
            5.0 * white_dwarf_radius(
                crate::stellar::remnant::RemnantRecipe::default(),
                SolarMasses::new(mc),
            )
            .value()
        } else {
            0.0
        }
    };
    let radius = match model {
        Model::MainSequence { .. } | Model::HeliumMainSequence { .. } => 0.0,
        Model::HertzsprungGap { core, .. } | Model::FirstGiantBranch { core, .. } => match core {
            HeliumCore::NonDegenerate => helium_zams_radius(mc),
            HeliumCore::Degenerate => degenerate(mc),
        },
        Model::FlashBridge { .. } | Model::EarlyAgb { .. } => helium_zams_radius(mc),
        Model::CoreHeliumBurning { .. } => {
            let (_, r) =
                helium::main_sequence_point(SolarMasses::new(mc.max(1e-3)), coord.clamp(0.0, 1.0));
            if mc > 0.0 { r.value() } else { 0.0 }
        }
        Model::ThermallyPulsingAgb { .. } | Model::HeliumShellBurning { .. } => degenerate(mc),
        Model::Remnant { .. } => state.radius().value(),
    };
    let _ = c;
    SolarRadii::new(radius.min(state.radius().value()))
}

/// The convective envelope of a star in `model` with `state` at coordinate `coord` and core
/// radius `rc` ([`ConvectiveEnvelope`]).
///
/// Its mass follows HPT section 7.2: on the main sequence `M_env,0` (1 − τ)^¼ with `M_env,0` = M
/// below 0.35 M☉, 0.35 ((1.25 − M) ÷ 0.9)² to 1.25 M☉ and 0 above; on the Hertzsprung gap
/// τ (M − Mc) with τ the gap's fraction; M − Mc for every star with a core; none for a helium main
/// sequence or a remnant. Its depth follows BSE equations 36–40: on the main sequence
/// `R_env,0` (1 − τ)^¼ with `R_env,0` = R below 0.35 M☉, R′ ((1.25 − M) ÷ 0.9)^½ to 1.25 M☉ (R′ the
/// radius of a 0.35 M☉ star at the same τ) and 0 above; τ^½ (R − Rc) on the Hertzsprung gap; and
/// R − Rc on the giant branches.
#[must_use]
fn convective_envelope(
    model: &Model,
    state: &StarState,
    coord: f64,
    rc: SolarRadii,
    c: &ZCoeffs,
) -> ConvectiveEnvelope {
    let (m, mc, r) = (
        state.mass().value(),
        state.core_mass().value(),
        state.radius().value(),
    );
    let outside = (r - rc.value()).max(0.0);
    match model {
        Model::MainSequence { .. } => main_sequence_envelope(m, coord.clamp(0.0, 1.0), r, c),
        Model::HertzsprungGap { .. } => {
            let tau = coord.clamp(0.0, 1.0);
            ConvectiveEnvelope {
                mass: tau * (m - mc).max(0.0),
                depth: tau.sqrt() * outside,
            }
        }
        Model::FirstGiantBranch { .. }
        | Model::FlashBridge { .. }
        | Model::CoreHeliumBurning { .. }
        | Model::EarlyAgb { .. }
        | Model::ThermallyPulsingAgb { .. }
        | Model::HeliumShellBurning { .. } => ConvectiveEnvelope {
            mass: (m - mc).max(0.0),
            depth: outside,
        },
        Model::HeliumMainSequence { .. } | Model::Remnant { .. } => ConvectiveEnvelope {
            mass: 0.0,
            depth: 0.0,
        },
    }
}

/// A main-sequence star's convective envelope ([`convective_envelope`]) at mass `m`, fractional
/// age `tau` and radius `r`.
#[must_use]
fn main_sequence_envelope(m: f64, tau: f64, r: f64, c: &ZCoeffs) -> ConvectiveEnvelope {
    let fade = math::powf(1.0 - tau, 0.25);
    let (mass0, depth0) = if m <= 0.35 {
        (m, r)
    } else if m < 1.25 {
        let share = (1.25 - m) / 0.9;
        let lightest = MainSequence::new(SolarMasses::new(0.35), c);
        let r_prime = lightest.at(lightest.t_ms() * tau).radius.value();
        (0.35 * share * share, r_prime * share.sqrt())
    } else {
        (0.0, 0.0)
    };
    ConvectiveEnvelope {
        mass: mass0 * fade,
        depth: (depth0 * fade).min(r),
    }
}

/// y of BSE equation 82 for a star in `model` at coordinate `coord` ([`Structure::burnt`]).
#[must_use]
fn burnt_fraction(model: &Model, coord: f64) -> f64 {
    match model {
        Model::MainSequence { .. }
        | Model::HertzsprungGap { .. }
        | Model::FirstGiantBranch { .. }
        | Model::FlashBridge { .. } => 0.0,
        Model::CoreHeliumBurning { .. } | Model::HeliumMainSequence { .. } => coord.clamp(0.0, 1.0),
        Model::EarlyAgb { .. }
        | Model::ThermallyPulsingAgb { .. }
        | Model::HeliumShellBurning { .. }
        | Model::Remnant { .. } => 1.0,
    }
}

/// `m` within the range a helium star's track takes.
#[must_use]
fn checked_helium_mass(m: SolarMasses) -> SolarMasses {
    let value = m.value();
    debug_assert!(
        value > 0.0 && value <= MAX_INITIAL_MASS.value(),
        "a helium star's track takes (0, 100] M☉, not {value}"
    );
    SolarMasses::new(value.clamp(1e-3, MAX_INITIAL_MASS.value()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solar() -> Composition {
        Composition::SOLAR
    }

    /// Ruling 34.1: a helium star's track is its helium main sequence, then its shell burning,
    /// then its remnant, with the phase's formulae of `HeliumStar` at constant mass where the
    /// wind is negligible; a light one ends as a white dwarf and a heavy one explodes.
    #[test]
    fn a_helium_star_track_follows_its_helium_main_sequence_to_its_remnant() {
        let draws = StarDraws::median();
        let light = Track::helium_star_full(SolarMasses::new(0.5), &solar(), &draws);
        assert_eq!(
            light.state_at(Years::ZERO).phase(),
            Phase::HeliumMainSequence
        );
        let death = light.death().expect("a full track reaches its death");
        assert!(!death.kind().is_sudden(), "{death:?}");
        let later = light.state_at(Years::new(death.age().value() * 1.01));
        assert_eq!(later.phase(), Phase::CarbonOxygenWhiteDwarf);
        // The main sequence lasts equation 79's 146 Myr at constant mass.
        let t_ms = helium::main_sequence_lifetime(SolarMasses::new(0.5)).value() * 1e6;
        assert_eq!(
            light.state_at(Years::new(0.99 * t_ms)).phase(),
            Phase::HeliumMainSequence
        );
        let heavy = Track::helium_star_full(SolarMasses::new(8.0), &solar(), &draws);
        let death = heavy.death().expect("a full track reaches its death");
        assert!(death.kind().is_sudden(), "{death:?}");
    }

    /// A helium star too light to burn helium is a helium white dwarf from the start.
    #[test]
    fn a_helium_star_below_the_ignition_core_is_a_helium_white_dwarf() {
        let star = Track::helium_star_full(SolarMasses::new(0.25), &solar(), &StarDraws::median());
        assert_eq!(star.state_at(Years::ZERO).phase(), Phase::HeliumWhiteDwarf);
    }

    /// Built to an age, a helium star's segments are those of the full track bit for bit (design
    /// note 2), and its state there is the same.
    #[test]
    fn a_helium_star_built_to_an_age_is_the_full_tracks() {
        let draws = StarDraws::median();
        let m = SolarMasses::new(3.0);
        let full = Track::helium_star_full(m, &solar(), &draws);
        let part = Track::helium_star(m, &solar(), &draws, Years::new(1.0e6));
        for age in [0.0, 2.0e5, 7.5e5, 1.0e6] {
            assert_eq!(
                part.state_at(Years::new(age)),
                full.state_at(Years::new(age)),
                "at {age} yr"
            );
        }
    }

    /// At the track's own mass, the structure's state is the track's state bit for bit.
    #[test]
    fn the_structure_at_the_tracks_own_mass_is_its_state() {
        let track = Track::full(SolarMasses::new(2.5), &solar(), &StarDraws::median());
        let death = track.death().expect("full").age().value();
        for i in 0..200 {
            let age = death * f64::from(i) / 200.0;
            let state = track.state_at(Years::new(age));
            let structure = track.structure_at(age, state.mass().value());
            assert_eq!(structure.state, state, "at {age} yr");
            assert!(structure.core_radius.value() <= state.radius().value());
            assert!(structure.envelope.mass <= state.mass().value());
        }
    }

    /// Stripped on the giant branch above `M_HeF`, a star leaves a zero-age helium star of its
    /// core; below it, a helium white dwarf; in core helium burning, the helium star at the same
    /// fractional age (HPT equation 76).
    #[test]
    fn a_stripped_giant_leaves_its_core() {
        let draws = StarDraws::median();
        let light = Track::full(SolarMasses::new(1.2), &solar(), &draws);
        let giant_age = (0..4000)
            .map(|i| f64::from(i) * 2.0e6)
            .find(|&age| light.state_at(Years::new(age)).phase() == Phase::FirstGiantBranch)
            .expect("a 1.2 M☉ star climbs the giant branch");
        let core = light.state_at(Years::new(giant_age)).core_mass();
        match light.remains_at(giant_age, 10.0, &draws, None) {
            Remains::WhiteDwarf { phase, mass, .. } => {
                assert_eq!(phase, Phase::HeliumWhiteDwarf);
                assert!((mass.value() - core.value()).abs() < 1e-12);
            }
            other => panic!("expected a helium white dwarf, got {other:?}"),
        }
        let heavy = Track::full(SolarMasses::new(5.0), &solar(), &draws);
        let burning = (0..20_000)
            .map(|i| f64::from(i) * 1.0e4)
            .find(|&age| heavy.state_at(Years::new(age)).phase() == Phase::CoreHeliumBurning)
            .expect("a 5 M☉ star burns helium in its core");
        let age = burning + 1.0e6;
        match heavy.remains_at(age, 10.0, &draws, None) {
            Remains::HeliumStar(star) => {
                assert_eq!(
                    star.state_at(Years::ZERO).phase(),
                    Phase::HeliumMainSequence
                );
                let tau = star
                    .main_sequence_fraction(0.0)
                    .expect("on its main sequence");
                assert!(tau > 0.0 && tau < 1.0, "{tau}");
            }
            other => panic!("expected a helium star, got {other:?}"),
        }
    }

    /// A new giant-branch star placed by its core has that core at the base of its giant branch
    /// (BSE section 2.7.4).
    #[test]
    fn a_new_giant_is_placed_by_its_core() {
        let c = ZCoeffs::new(solar().z_fit());
        for mc in [0.12, 0.2, 0.3, 0.4] {
            let (m0, phase, _) =
                new_star_mass(NewStar::GiantBranch, SolarMasses::new(mc), &c).expect("placed");
            assert_eq!(phase, Phase::FirstGiantBranch);
            let at_bgb = if m0 <= c.m_hef() {
                GiantBranch::new(m0, &c)
                    .core_mass(ms::l_bgb(m0, &c))
                    .value()
            } else {
                gb::mc_bgb(m0, &c).value()
            };
            assert!((at_bgb / mc - 1.0).abs() < 1e-6, "{mc}: {m0:?} → {at_bgb}");
        }
        let (m0, _, _) =
            new_star_mass(NewStar::EarlyAgb, SolarMasses::new(0.9), &c).expect("placed");
        assert!((gb::mc_bagb(m0, &c).value() - 0.9).abs() < 1e-9);
        let (he, phase, _) =
            new_star_mass(NewStar::HeliumGiant, SolarMasses::new(0.6), &c).expect("placed");
        assert_eq!(phase, Phase::HeliumHertzsprungGap);
        assert!(he.value() > 0.6);
    }

    /// The main-sequence structure the binary carries is the track's own at the same mass and
    /// fractional age, bit for bit, where the track's main sequence has no wind to speak of.
    #[test]
    fn a_carried_main_sequence_is_the_tracks() {
        let draws = StarDraws::median();
        let c = ZCoeffs::new(solar().z_fit());
        let track = Track::full(SolarMasses::new(1.0), &solar(), &draws);
        for age in [1.0e8, 3.0e9, 9.0e9] {
            let tau = track
                .main_sequence_fraction(age)
                .expect("on the main sequence");
            let own = track.state_at(Years::new(age));
            let (structure, _) =
                main_sequence_structure(&c, &solar(), &draws, false, own.mass().value(), tau, age);
            assert_eq!(structure.state.luminosity(), own.luminosity(), "at {age}");
            assert_eq!(structure.state.radius(), own.radius(), "at {age}");
        }
    }
}
