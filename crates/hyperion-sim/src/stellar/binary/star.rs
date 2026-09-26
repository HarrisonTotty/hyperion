//! A member of a binary as the engine carries it and as a segment of the timeline holds it: its
//! stellar type in the sense of Hurley, Tout and Pols (2002, "BSE", section 1), and how its state
//! is evaluated at any age inside a segment (plan 11, design note 6).
//!
//! A [`Member`] is one of a few forms. A star that has not yet interacted follows its own
//! single-star [`Track`] from an offset in age, so that its state is plan 06's bit for bit. A star
//! whose mass a companion has changed keeps the closed forms of its track and takes its mass from
//! the binary (HPT section 7.1: after the main sequence only the radius and the wind read the
//! current mass). On the main sequence, where the initial mass is the current one, the binary
//! carries the mass and the fractional age τ itself (BSE section 2.6.6). A remnant the binary made
//! or feeds cools on plan 06's laws at the mass the binary gives it. Each quantity the binary sets
//! is a [`Path`], its values at the engine's steps joined linearly, so that the state inside a
//! segment is a continuous function of age and a lookup, with no replay.

use std::sync::Arc;

use crate::stellar::remnant::structure::{
    black_hole_radius, neutron_star_radius, white_dwarf_radius,
};
use crate::stellar::remnant::white_dwarf::{self, WhiteDwarfCore};
use crate::stellar::remnant::{RemnantRecipe, neutron_star};
use crate::stellar::sse::{self, ConvectiveEnvelope, Structure, Track};
use crate::stellar::{Phase, StarState, StarStateParts, substellar};
use crate::units::consts::{GM_SUN, SECONDS_PER_JULIAN_YEAR, SOLAR_RADIUS_M};
use crate::units::{
    Megayears, SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years,
};

use super::timeline::Context;

/// The gravitational constant in the engine's units, R☉³ M☉⁻¹ yr⁻²: GM☉ (IAU 2015 B3) over the
/// nominal R☉, with Julian years. BSE's code carries the same as 3.920659 × 10⁸ in an R☉ of
/// 6.96 × 10⁸ m.
pub(crate) const G: f64 = GM_SUN * SECONDS_PER_JULIAN_YEAR * SECONDS_PER_JULIAN_YEAR
    / (SOLAR_RADIUS_M * SOLAR_RADIUS_M * SOLAR_RADIUS_M);

/// A star's stellar type in BSE's numbering (section 1), which its rules are written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Kind {
    /// 0: a main-sequence star below 0.7 M☉, deeply or fully convective.
    LowMassMainSequence,
    /// 1: a main-sequence star from 0.7 M☉.
    MainSequence,
    /// 2.
    HertzsprungGap,
    /// 3.
    GiantBranch,
    /// 4.
    CoreHeliumBurning,
    /// 5.
    EarlyAgb,
    /// 6.
    PulsingAgb,
    /// 7.
    HeliumMainSequence,
    /// 8.
    HeliumGap,
    /// 9.
    HeliumGiant,
    /// 10.
    HeliumWhiteDwarf,
    /// 11.
    CarbonOxygenWhiteDwarf,
    /// 12.
    OxygenNeonWhiteDwarf,
    /// 13.
    NeutronStar,
    /// 14.
    BlackHole,
    /// 15: nothing is left.
    Massless,
}

impl Kind {
    /// The type of a star in `phase` of mass `mass`, M☉. A star before or below the main sequence
    /// (plan 06's protostars and cooling-fit stars) is type 0, a post-AGB star type 6.
    #[must_use]
    pub(crate) fn of(phase: Phase, mass: f64) -> Self {
        match phase {
            Phase::Protostar | Phase::PreMainSequence | Phase::MainSequence | Phase::Substellar => {
                if mass < 0.7 {
                    Self::LowMassMainSequence
                } else {
                    Self::MainSequence
                }
            }
            Phase::HertzsprungGap => Self::HertzsprungGap,
            Phase::FirstGiantBranch => Self::GiantBranch,
            Phase::CoreHeliumBurning => Self::CoreHeliumBurning,
            Phase::EarlyAgb => Self::EarlyAgb,
            Phase::ThermallyPulsingAgb | Phase::PostAgb => Self::PulsingAgb,
            Phase::HeliumMainSequence => Self::HeliumMainSequence,
            Phase::HeliumHertzsprungGap => Self::HeliumGap,
            Phase::HeliumGiantBranch => Self::HeliumGiant,
            Phase::HeliumWhiteDwarf => Self::HeliumWhiteDwarf,
            Phase::CarbonOxygenWhiteDwarf => Self::CarbonOxygenWhiteDwarf,
            Phase::OxygenNeonWhiteDwarf => Self::OxygenNeonWhiteDwarf,
            Phase::NeutronStar => Self::NeutronStar,
            Phase::BlackHole => Self::BlackHole,
            Phase::NoRemnant => Self::Massless,
        }
    }

    /// BSE's number for the type, 0–15, the index of the collision matrix (its table 2).
    #[must_use]
    pub(crate) const fn number(self) -> usize {
        match self {
            Self::LowMassMainSequence => 0,
            Self::MainSequence => 1,
            Self::HertzsprungGap => 2,
            Self::GiantBranch => 3,
            Self::CoreHeliumBurning => 4,
            Self::EarlyAgb => 5,
            Self::PulsingAgb => 6,
            Self::HeliumMainSequence => 7,
            Self::HeliumGap => 8,
            Self::HeliumGiant => 9,
            Self::HeliumWhiteDwarf => 10,
            Self::CarbonOxygenWhiteDwarf => 11,
            Self::OxygenNeonWhiteDwarf => 12,
            Self::NeutronStar => 13,
            Self::BlackHole => 14,
            Self::Massless => 15,
        }
    }

    /// The type of BSE's number `n`; 15 for any number past 14.
    #[must_use]
    pub(crate) const fn from_number(n: usize) -> Self {
        match n {
            0 => Self::LowMassMainSequence,
            1 => Self::MainSequence,
            2 => Self::HertzsprungGap,
            3 => Self::GiantBranch,
            4 => Self::CoreHeliumBurning,
            5 => Self::EarlyAgb,
            6 => Self::PulsingAgb,
            7 => Self::HeliumMainSequence,
            8 => Self::HeliumGap,
            9 => Self::HeliumGiant,
            10 => Self::HeliumWhiteDwarf,
            11 => Self::CarbonOxygenWhiteDwarf,
            12 => Self::OxygenNeonWhiteDwarf,
            13 => Self::NeutronStar,
            14 => Self::BlackHole,
            _ => Self::Massless,
        }
    }

    /// A main-sequence star, hydrogen (0, 1).
    #[must_use]
    pub(crate) const fn is_main_sequence(self) -> bool {
        matches!(self, Self::LowMassMainSequence | Self::MainSequence)
    }

    /// A giant-like star with a dense core and an envelope: types 2–6, 8 and 9 (BSE sections
    /// 2.6.3 and 2.7).
    #[must_use]
    pub(crate) const fn is_giant_like(self) -> bool {
        matches!(
            self,
            Self::HertzsprungGap
                | Self::GiantBranch
                | Self::CoreHeliumBurning
                | Self::EarlyAgb
                | Self::PulsingAgb
                | Self::HeliumGap
                | Self::HeliumGiant
        )
    }

    /// A naked helium star (7–9).
    #[must_use]
    pub(crate) const fn is_helium_star(self) -> bool {
        matches!(
            self,
            Self::HeliumMainSequence | Self::HeliumGap | Self::HeliumGiant
        )
    }

    /// A white dwarf (10–12).
    #[must_use]
    pub(crate) const fn is_white_dwarf(self) -> bool {
        matches!(
            self,
            Self::HeliumWhiteDwarf | Self::CarbonOxygenWhiteDwarf | Self::OxygenNeonWhiteDwarf
        )
    }

    /// A compact remnant or nothing (10–15).
    #[must_use]
    pub(crate) const fn is_remnant(self) -> bool {
        matches!(
            self,
            Self::HeliumWhiteDwarf
                | Self::CarbonOxygenWhiteDwarf
                | Self::OxygenNeonWhiteDwarf
                | Self::NeutronStar
                | Self::BlackHole
                | Self::Massless
        )
    }

    /// Whether what the star gives up is hydrogen-rich (0–6), helium-rich (7–10) or carbon- and
    /// oxygen-rich (11 and 12), for an accreting white dwarf (BSE section 2.6.6).
    #[must_use]
    pub(crate) const fn surface(self) -> Surface {
        match self {
            Self::LowMassMainSequence
            | Self::MainSequence
            | Self::HertzsprungGap
            | Self::GiantBranch
            | Self::CoreHeliumBurning
            | Self::EarlyAgb
            | Self::PulsingAgb => Surface::Hydrogen,
            Self::HeliumMainSequence
            | Self::HeliumGap
            | Self::HeliumGiant
            | Self::HeliumWhiteDwarf => Surface::Helium,
            Self::CarbonOxygenWhiteDwarf
            | Self::OxygenNeonWhiteDwarf
            | Self::NeutronStar
            | Self::BlackHole
            | Self::Massless => Surface::Carbon,
        }
    }
}

/// What a donor's transferred material is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    Hydrogen,
    Helium,
    Carbon,
}

/// Whether `x` is positive: false for zero, a negative number and NaN.
#[must_use]
pub(crate) fn positive(x: f64) -> bool {
    x > 0.0
}

/// A quantity the binary sets, at the engine's steps: its values joined linearly in age, held at
/// the ends.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Path {
    /// (age, value) in rising age.
    knots: Vec<[f64; 2]>,
}

impl Path {
    /// A path of one knot, `value` at `age`.
    #[must_use]
    pub(crate) fn starting(age: f64, value: f64) -> Self {
        Self {
            knots: vec![[age, value]],
        }
    }

    /// Adds a knot; a knot at the last one's age replaces it.
    pub(crate) fn push(&mut self, age: f64, value: f64) {
        if let Some(last) = self.knots.last_mut()
            && age <= last[0]
        {
            *last = [age, value];
            return;
        }
        self.knots.push([age, value]);
    }

    /// The value at `age`.
    #[must_use]
    pub(crate) fn at(&self, age: f64) -> f64 {
        let n = self.knots.len();
        debug_assert!(n > 0, "a path has a knot");
        let i = self.knots.partition_point(|k| k[0] <= age);
        if i == 0 {
            return self.knots[0][1];
        }
        if i >= n {
            return self.knots[n - 1][1];
        }
        let [a0, v0] = self.knots[i - 1];
        let [a1, v1] = self.knots[i];
        let x = (age - a0) / (a1 - a0);
        (1.0 - x) * v0 + x * v1
    }

    /// The last value.
    #[must_use]
    pub(crate) fn last(&self) -> f64 {
        self.knots.last().map_or(0.0, |k| k[1])
    }

    /// The knots, for the tests and the heap accounting.
    #[must_use]
    pub(crate) fn knots(&self) -> &[[f64; 2]] {
        &self.knots
    }
}

/// How a member's state is evaluated inside a segment.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Member {
    /// A star on its own single-star track, at the track's age `age − offset`: plan 06's state.
    Track { track: Arc<Track>, offset: f64 },
    /// A star on its track's closed forms at `age − offset` whose mass the binary sets.
    Shaped {
        track: Arc<Track>,
        offset: f64,
        mass: Path,
    },
    /// A main-sequence or (with `helium`) helium main-sequence star whose mass and fractional age
    /// τ the binary carries.
    MainSequence { helium: bool, mass: Path, tau: Path },
    /// A star below 0.1 M☉ on P06.T13's cooling fits, at age `age − offset`.
    Cooling { offset: f64, mass: Path },
    /// A star held at its last living state (plan 11, design note 16: a massive primary whose own
    /// track would die before plan 06's death age waits for it).
    Frozen { state: StarState },
    /// A white dwarf, neutron star or black hole the binary made or feeds, formed at `birth`,
    /// whose white dwarf's cooling law starts at `origin` of its own clock.
    Remnant {
        phase: Phase,
        birth: f64,
        origin: Megayears,
        mass: Path,
    },
    /// Nothing: merged into the other member, or destroyed.
    Gone,
}

impl Member {
    /// The member's state at `age`, the member being slot `slot` of `ctx`'s pair.
    #[must_use]
    pub(crate) fn state_at(&self, ctx: &Context, slot: usize, age: f64) -> StarState {
        match self {
            Self::Track { track, offset } => track.state_at(Years::new((age - offset).max(0.0))),
            Self::Frozen { state } => *state,
            Self::Gone => nothing(age),
            Self::Shaped { mass, .. }
            | Self::MainSequence { mass, .. }
            | Self::Cooling { mass, .. }
            | Self::Remnant { mass, .. } => {
                let tau = match self {
                    Self::MainSequence { tau, .. } => tau.at(age),
                    _ => 0.0,
                };
                self.evaluate(ctx, slot, age, mass.at(age), tau)
                    .map_or_else(|| nothing(age), |s| s.state)
            }
        }
    }

    /// The member's structure at `age` with the mass and τ the binary gives it (ignored by a star
    /// on its own track, a frozen one and nothing), or `None` for nothing.
    #[must_use]
    pub(crate) fn evaluate(
        &self,
        ctx: &Context,
        slot: usize,
        age: f64,
        mass: f64,
        tau: f64,
    ) -> Option<Structure> {
        match self {
            Self::Track { track, offset } => track.own_structure_at((age - offset).max(0.0)),
            Self::Shaped { track, offset, .. } => {
                Some(track.structure_at((age - offset).max(0.0), mass.max(1e-6)))
            }
            Self::MainSequence { helium, .. } => {
                let (structure, _) = sse::main_sequence_structure(
                    ctx.coeffs(),
                    ctx.composition(),
                    ctx.draws(slot),
                    *helium,
                    mass,
                    tau,
                    age,
                );
                Some(structure)
            }
            Self::Cooling { offset, .. } => {
                let m = mass.clamp(substellar::MIN_MASS.value(), substellar::MAX_MASS.value());
                let state = substellar::cooling(
                    SolarMasses::new(m),
                    Years::new((age - offset).max(0.0)),
                    ctx.composition(),
                )
                .ok()?;
                let r = state.radius().value();
                Some(Structure {
                    state,
                    core_radius: SolarRadii::ZERO,
                    envelope: ConvectiveEnvelope { mass: m, depth: r },
                    degenerate_core: false,
                    burnt: 0.0,
                    phase_end: f64::INFINITY,
                })
            }
            Self::Frozen { state } => Some(frozen_structure(*state)),
            Self::Remnant {
                phase,
                birth,
                origin,
                ..
            } => remnant_structure(ctx, *phase, mass, age, age - birth, *origin),
            Self::Gone => None,
        }
    }

    /// The radius, R☉, of the member's structure at `age` with `mass` and `tau`
    /// ([`Member::evaluate`]), bit for bit, or `None` for nothing: for a star the binary reshapes
    /// or carries on the main sequence, without the wind and envelope the radius does not read.
    #[must_use]
    pub(crate) fn radius(
        &self,
        ctx: &Context,
        slot: usize,
        age: f64,
        mass: f64,
        tau: f64,
    ) -> Option<f64> {
        match self {
            Self::Shaped { track, offset, .. } => {
                Some(track.radius_at((age - offset).max(0.0), mass.max(1e-6)))
            }
            Self::MainSequence { helium, .. } => {
                Some(sse::main_sequence_radius(ctx.coeffs(), *helium, mass, tau))
            }
            Self::Track { .. }
            | Self::Cooling { .. }
            | Self::Frozen { .. }
            | Self::Remnant { .. }
            | Self::Gone => self
                .evaluate(ctx, slot, age, mass, tau)
                .map(|s| s.state.radius().value()),
        }
    }

    /// The member's mass at `age`, M☉.
    #[must_use]
    pub(crate) fn mass_at(&self, age: f64) -> f64 {
        match self {
            Self::Track { track, offset } => track.mass_at((age - offset).max(0.0)),
            Self::Shaped { mass, .. }
            | Self::MainSequence { mass, .. }
            | Self::Cooling { mass, .. }
            | Self::Remnant { mass, .. } => mass.at(age),
            Self::Frozen { state } => state.mass().value(),
            Self::Gone => 0.0,
        }
    }

    /// The same member with every path cut back to its value at `age`, the first knot of a new
    /// segment.
    #[must_use]
    pub(crate) fn restarted(&self, age: f64) -> Self {
        match self {
            Self::Track { .. } | Self::Frozen { .. } | Self::Gone => self.clone(),
            Self::Shaped {
                track,
                offset,
                mass,
            } => Self::Shaped {
                track: Arc::clone(track),
                offset: *offset,
                mass: Path::starting(age, mass.at(age)),
            },
            Self::MainSequence { helium, mass, tau } => Self::MainSequence {
                helium: *helium,
                mass: Path::starting(age, mass.at(age)),
                tau: Path::starting(age, tau.at(age)),
            },
            Self::Cooling { offset, mass } => Self::Cooling {
                offset: *offset,
                mass: Path::starting(age, mass.at(age)),
            },
            Self::Remnant {
                phase,
                birth,
                origin,
                mass,
            } => Self::Remnant {
                phase: *phase,
                birth: *birth,
                origin: *origin,
                mass: Path::starting(age, mass.at(age)),
            },
        }
    }

    /// Records the binary's mass `mass` and τ `tau` at `age` on the member's paths (a star on
    /// its own track keeps none).
    pub(crate) fn record(&mut self, age: f64, mass: f64, tau: f64) {
        match self {
            Self::Shaped { mass: path, .. }
            | Self::Cooling { mass: path, .. }
            | Self::Remnant { mass: path, .. } => path.push(age, mass),
            Self::MainSequence {
                mass: path,
                tau: taus,
                ..
            } => {
                path.push(age, mass);
                taus.push(age, tau);
            }
            Self::Track { .. } | Self::Frozen { .. } | Self::Gone => {}
        }
    }

    /// Whether the binary carries the member's mass (every form but a star on its own track, a
    /// frozen one and nothing).
    #[must_use]
    pub(crate) const fn carries_mass(&self) -> bool {
        matches!(
            self,
            Self::Shaped { .. }
                | Self::MainSequence { .. }
                | Self::Cooling { .. }
                | Self::Remnant { .. }
        )
    }

    /// Whether nothing is left of the member.
    #[must_use]
    pub(crate) const fn is_gone(&self) -> bool {
        matches!(self, Self::Gone)
    }

    /// The track the member follows, if it follows one.
    #[must_use]
    pub(crate) fn track(&self) -> Option<(&Arc<Track>, f64)> {
        match self {
            Self::Track { track, offset } | Self::Shaped { track, offset, .. } => {
                Some((track, *offset))
            }
            Self::MainSequence { .. }
            | Self::Cooling { .. }
            | Self::Frozen { .. }
            | Self::Remnant { .. }
            | Self::Gone => None,
        }
    }

    /// The member's form, for the tests' accounts.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn form(&self) -> String {
        match self {
            Self::Track { offset, .. } => format!("track@{offset:.4e}"),
            Self::Shaped { offset, .. } => format!("shaped@{offset:.4e}"),
            Self::MainSequence { tau, .. } => format!("ms(τ {:.6})", tau.last()),
            Self::Cooling { .. } => "cooling".into(),
            Self::Frozen { .. } => "frozen".into(),
            Self::Remnant { .. } => "remnant".into(),
            Self::Gone => "gone".into(),
        }
    }

    /// The bytes the member owns on the heap: its paths' knots (a shared track is not counted).
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        let path = |p: &Path| p.knots.capacity() * size_of::<[f64; 2]>();
        match self {
            Self::Shaped { mass, .. } | Self::Cooling { mass, .. } | Self::Remnant { mass, .. } => {
                path(mass)
            }
            Self::MainSequence { mass, tau, .. } => path(mass) + path(tau),
            Self::Track { .. } | Self::Frozen { .. } | Self::Gone => 0,
        }
    }
}

/// The state of nothing at `age`: plan 06's `NoRemnant`.
#[must_use]
pub(crate) fn nothing(age: f64) -> StarState {
    StarState::new(StarStateParts {
        phase: Phase::NoRemnant,
        age: Years::new(age.max(0.0)),
        mass: SolarMasses::ZERO,
        core_mass: SolarMasses::ZERO,
        luminosity: SolarLuminosities::ZERO,
        radius: SolarRadii::ZERO,
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.0,
    })
}

/// The structure of a star held at `state`: no wind and no envelope to speak of.
#[must_use]
fn frozen_structure(state: StarState) -> Structure {
    let state = StarState::new(StarStateParts {
        phase: state.phase(),
        age: state.age(),
        mass: state.mass(),
        core_mass: state.core_mass(),
        luminosity: state.luminosity(),
        radius: state.radius(),
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: state.phase_fraction(),
    });
    Structure {
        state,
        core_radius: SolarRadii::new(state.radius().value().min(0.1 * state.radius().value())),
        envelope: ConvectiveEnvelope {
            mass: 0.0,
            depth: 0.0,
        },
        degenerate_core: false,
        burnt: 1.0,
        phase_end: f64::INFINITY,
    }
}

/// The structure of a remnant of `phase` and `mass` at `age`, `since` years after its formation,
/// on plan 06's laws under the default recipe: a white dwarf cools from `origin` of its law's
/// clock (`white_dwarf::luminosity`) with HPT equation 91's radius, a neutron star by HPT equation
/// 93, a black hole is dark. `None` for nothing.
#[must_use]
pub(crate) fn remnant_structure(
    ctx: &Context,
    phase: Phase,
    mass: f64,
    age: f64,
    since: f64,
    origin: Megayears,
) -> Option<Structure> {
    let recipe = RemnantRecipe::default();
    let m = SolarMasses::new(mass.max(1e-6));
    let since = Years::new(since.max(0.0));
    let (luminosity, radius) = match phase {
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {
            let core = WhiteDwarfCore::of(phase)?;
            (
                white_dwarf::luminosity(recipe, core, m, since, origin, ctx.composition().z_fit()),
                white_dwarf_radius(recipe, m),
            )
        }
        Phase::NeutronStar => (
            neutron_star::hpt_luminosity(m, since),
            neutron_star_radius(recipe),
        ),
        Phase::BlackHole => (SolarLuminosities::ZERO, black_hole_radius(recipe, m)),
        _ => return None,
    };
    let state = StarState::new(StarStateParts {
        phase,
        age: Years::new(age.max(0.0)),
        mass: m,
        core_mass: m,
        luminosity,
        radius,
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.0,
    });
    Some(Structure {
        state,
        core_radius: radius,
        envelope: ConvectiveEnvelope {
            mass: 0.0,
            depth: 0.0,
        },
        degenerate_core: false,
        burnt: 1.0,
        phase_end: f64::INFINITY,
    })
}

/// A star's moment of inertia, M☉ R☉²: k′₂ (M − Mc) R² + k′₃ Mc Rc² (BSE equation 35), and
/// k′₃ M R² for a remnant.
#[must_use]
pub(crate) fn moment_of_inertia(s: &Structure) -> f64 {
    let (m, mc, r, rc) = (
        s.state.mass().value(),
        s.state.core_mass().value(),
        s.state.radius().value(),
        s.core_radius.value(),
    );
    if s.state.phase().is_remnant() {
        return sse::CORE_GYRATION * m * r * r;
    }
    sse::ENVELOPE_GYRATION * (m - mc).max(0.0) * r * r + sse::CORE_GYRATION * mc * rc * rc
}

/// A star's rotation on the zero-age main sequence, rad yr⁻¹: 45.35 v̄ ÷ R with v̄ = 330 M^3.3 ÷
/// (15 + M^3.45) km s⁻¹ (HPT equations 107 and 108).
#[must_use]
pub(crate) fn zams_spin(mass: f64, radius: f64) -> f64 {
    let v = 330.0 * crate::math::powf_positive(mass, 3.3)
        / (15.0 + crate::math::powf_positive(mass, 3.45));
    45.35 * v / radius.max(1e-6)
}

/// A white dwarf's cooling origin for a dwarf of `phase` and `mass` born from a star whose last
/// luminosity was `last`, under the default recipe (`white_dwarf::cooling_origin`).
#[must_use]
pub(crate) fn cooling_origin(
    ctx: &Context,
    phase: Phase,
    mass: f64,
    last: Option<SolarLuminosities>,
) -> Megayears {
    WhiteDwarfCore::of(phase).map_or(Megayears::ZERO, |core| {
        white_dwarf::cooling_origin(
            RemnantRecipe::default(),
            core,
            SolarMasses::new(mass.max(1e-6)),
            last,
            ctx.composition().z_fit(),
        )
    })
}
