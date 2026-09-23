//! What a segment of a track evaluates: one phase's closed forms at the star's effective initial
//! mass, with every radius at its current mass (HPT section 7.1), the small-envelope perturbation
//! towards the remnant its core would become (HPT section 6.3), and the remnants themselves.
//!
//! A [`Model`] is built once per segment. Phases whose initial mass is the current one (the main
//! sequence and the helium main sequence, HPT section 7.1) rebuild their closed forms at the
//! current mass on each evaluation unless the segment holds its mass fixed; every other phase keeps
//! the mass it was entered with and is built once.

use crate::stellar::Phase;
use crate::stellar::remnant::structure::{
    black_hole_radius, neutron_star_radius, white_dwarf_radius,
};
use crate::stellar::remnant::white_dwarf::{self, WhiteDwarfCore};
use crate::stellar::remnant::{RemnantRecipe, neutron_star};
use crate::units::{Megayears, MetalFraction, SolarLuminosities, SolarMasses, Years};

use super::super::PhasePoint;
use super::super::agb::{EarlyAgb, ThermallyPulsingAgb};
use super::super::cheb::CoreHeliumBurning;
use super::super::coeffs::ZCoeffs;
use super::super::envelope::{self, CoreRemnant};
use super::super::gb::FirstGiantBranch;
use super::super::helium::{self, HeliumStar};
use super::super::hg::HertzsprungGap;
use super::super::ms::MainSequence;
use super::super::wind;

/// The share of the early AGB over which its core's remnant passes from the end of the helium
/// main sequence to the helium giants' relation (see [`Model::point`]).
const EARLY_AGB_REMNANT_BLEND: f64 = 1.0 / 3.0;

/// Everything a model reads that is fixed for the whole track.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Physics<'a> {
    /// The metallicity's coefficients.
    pub(crate) coeffs: &'a ZCoeffs,
    /// The metal fraction the formulae see (and the white dwarfs' cooling reads).
    pub(crate) z: MetalFraction,
    /// The remnants' structure.
    pub(crate) remnant: RemnantRecipe,
}

/// A phase's clock across a segment: the phase's own time at the segment's coordinate x, from
/// `start` at x = 0 to `end` at x = 1, Myr from the zero-age main sequence of the phase's mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Span {
    pub(crate) start: Megayears,
    pub(crate) end: Megayears,
}

impl Span {
    /// The clock at x, exact at both ends: (1 − x) start + x end.
    #[must_use]
    pub(super) fn at(self, x: f64) -> Megayears {
        Megayears::new((1.0 - x) * self.start.value() + x * self.end.value())
    }

    /// The span's length, years.
    #[must_use]
    pub(super) fn years(self) -> f64 {
        (self.end.value() - self.start.value()) * 1e6
    }
}

/// Whether a giant's helium core is degenerate: up to `M_HeF` its envelope's loss leaves a helium
/// white dwarf, above it a naked helium star (HPT sections 6 and 6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeliumCore {
    /// Initial mass up to `M_HeF`.
    Degenerate,
    /// Initial mass above `M_HeF`.
    NonDegenerate,
}

impl HeliumCore {
    /// The core of a star of initial mass `m0`.
    #[must_use]
    pub(super) fn of(m0: SolarMasses, c: &ZCoeffs) -> Self {
        if m0.value() > c.m_hef().value() {
            Self::NonDegenerate
        } else {
            Self::Degenerate
        }
    }
}

/// The closed forms of one segment.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Model {
    /// The main sequence, whose initial mass is the current one: the formulae at the current mass,
    /// held in `fixed` when the segment's mass does not change.
    MainSequence {
        /// The closed forms at the segment's constant mass, if it has one.
        fixed: Option<MainSequence>,
    },
    /// The Hertzsprung gap, at the mass the main sequence ended with.
    HertzsprungGap {
        gap: HertzsprungGap,
        core: HeliumCore,
        span: Span,
    },
    /// The first giant branch.
    FirstGiantBranch {
        branch: FirstGiantBranch,
        core: HeliumCore,
        span: Span,
    },
    /// The helium flash (plan 06, design note 3): log L, log R and the core interpolated linearly
    /// in time from the tip of the giant branch to the zero-age horizontal branch.
    FlashBridge {
        /// log₁₀ L, log₁₀ R and the core mass at the tip.
        from: [f64; 3],
        /// The same on the zero-age horizontal branch.
        to: [f64; 3],
    },
    /// Core helium burning.
    CoreHeliumBurning {
        phase: CoreHeliumBurning,
        span: Span,
    },
    /// The early asymptotic giant branch; `helium` is the naked helium star of its helium core,
    /// the remnant of HPT section 6.3.
    EarlyAgb {
        phase: EarlyAgb,
        helium: HeliumStar,
        span: Span,
    },
    /// The thermally pulsing asymptotic giant branch.
    ThermallyPulsingAgb {
        phase: ThermallyPulsingAgb,
        span: Span,
    },
    /// The helium main sequence, whose initial mass is the current one, held in `fixed` when the
    /// segment's mass does not change.
    HeliumMainSequence { fixed: Option<HeliumStar> },
    /// A naked helium star after its main sequence: the helium Hertzsprung gap and giant branch.
    HeliumShellBurning { star: HeliumStar, span: Span },
    /// A remnant, from its formation at `birth` (years since the onset of collapse).
    Remnant {
        phase: Phase,
        mass: SolarMasses,
        birth: f64,
    },
}

/// A phase's state at one instant: its phase, L, R and core mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Point {
    pub(crate) phase: Phase,
    pub(crate) point: PhasePoint,
}

impl Model {
    /// The state at coordinate `coord` for current mass `mt`: for the main sequence and the helium
    /// main sequence `coord` is the fractional age τ of the phase, which the effective-age rule of
    /// HPT section 7.1 keeps as the mass changes, and the phase's clock is τ times its lifetime at
    /// `mt`; for every other phase it is the fraction x of the segment's [`Span`]. `age` is the
    /// star's age (only a remnant reads it).
    ///
    /// The helium star that the early AGB's core would become (HPT section 6.3) has its core on
    /// the helium giants' relation (equation 84) at the early AGB's carbon–oxygen core, and the
    /// luminosity at the end of its main sequence at the start of the early AGB, which is where
    /// core helium burning leaves the core's remnant. The published SSE code passes from the second
    /// to the first geometrically over the first third of the time from the base of the AGB to the
    /// star's nuclear end (`hrdiag`, stellar type 5), so that the perturbed luminosity is
    /// continuous there; HPT print only the relation. The track does the same over the first third
    /// of the early AGB's own span, which, unlike SSE's nuclear time, does not change with the
    /// current mass.
    #[must_use]
    pub(super) fn point(&self, phys: &Physics<'_>, coord: f64, mt: SolarMasses, age: f64) -> Point {
        let c = phys.coeffs;
        match self {
            Self::MainSequence { fixed } => {
                let rebuilt;
                let ms = if let Some(ms) = fixed {
                    ms
                } else {
                    rebuilt = MainSequence::new(mt, c);
                    &rebuilt
                };
                Point {
                    phase: Phase::MainSequence,
                    point: ms.at(ms.t_ms() * coord),
                }
            }
            Self::HertzsprungGap { gap, core, span } => {
                let point = gap.at_mass(span.at(coord), mt, c);
                giant(Phase::HertzsprungGap, point, mt, *core, phys)
            }
            Self::FirstGiantBranch { branch, core, span } => {
                let point = branch.at_mass(span.at(coord), mt, c);
                giant(Phase::FirstGiantBranch, point, mt, *core, phys)
            }
            Self::FlashBridge { from, to } => {
                let lerp = |i: usize| (1.0 - coord) * from[i] + coord * to[i];
                Point {
                    phase: Phase::CoreHeliumBurning,
                    point: PhasePoint {
                        luminosity: SolarLuminosities::new(crate::math::exp10(lerp(0))),
                        radius: crate::units::SolarRadii::new(crate::math::exp10(lerp(1))),
                        core_mass: SolarMasses::new(lerp(2)),
                    },
                }
            }
            Self::CoreHeliumBurning { phase, span } => Point {
                phase: Phase::CoreHeliumBurning,
                point: core_helium_burning(phase, *span, coord, mt, c),
            },
            Self::EarlyAgb {
                phase,
                helium,
                span,
            } => Point {
                phase: Phase::EarlyAgb,
                point: early_agb(phase, helium, *span, coord, mt, c),
            },
            Self::ThermallyPulsingAgb { phase, span } => {
                let point = phase.at_mass(span.at(coord), mt, c);
                let mu = wind::small_envelope_mu(mt, point.core_mass, point.luminosity);
                Point {
                    phase: Phase::ThermallyPulsingAgb,
                    point: perturb_to_white_dwarf(
                        point,
                        mt,
                        mu,
                        WhiteDwarfCore::CarbonOxygen,
                        phys,
                    ),
                }
            }
            Self::HeliumMainSequence { fixed } => {
                let rebuilt;
                let star = if let Some(star) = fixed {
                    star
                } else {
                    rebuilt = HeliumStar::new(mt);
                    &rebuilt
                };
                Point {
                    phase: Phase::HeliumMainSequence,
                    point: star.at(star.t_ms() * coord),
                }
            }
            Self::HeliumShellBurning { star, span } => {
                let clock = span.at(coord);
                let point = star.at_mass(clock, mt);
                let mu = envelope::helium_giant_mu(point.core_mass, helium::shell_limit_at(mt));
                Point {
                    phase: star.phase_at_mass(clock, mt),
                    point: perturb_to_white_dwarf(
                        point,
                        mt,
                        mu,
                        WhiteDwarfCore::CarbonOxygen,
                        phys,
                    ),
                }
            }
            Self::Remnant { phase, mass, birth } => Point {
                phase: *phase,
                point: remnant(*phase, *mass, Years::new((age - birth).max(0.0)), phys),
            },
        }
    }
}

/// Core helium burning at the fraction `coord` of its `span` for current mass `mt`, perturbed
/// towards the helium main-sequence star of its core at the same fractional age (HPT section 6.3
/// and equation 76) as its envelope thins.
#[must_use]
fn core_helium_burning(
    phase: &CoreHeliumBurning,
    span: Span,
    coord: f64,
    mt: SolarMasses,
    c: &ZCoeffs,
) -> PhasePoint {
    let point = phase.at_mass(span.at(coord), mt, c);
    let mu = wind::small_envelope_mu(mt, point.core_mass, point.luminosity);
    if mu < 1.0 {
        let (luminosity, radius) = helium::main_sequence_point(point.core_mass, coord);
        envelope::perturb(point, mt, mu, CoreRemnant { luminosity, radius })
    } else {
        point
    }
}

/// The early AGB at the fraction `coord` of its `span` for current mass `mt`, perturbed towards
/// the naked helium star `helium` of its helium core as its envelope thins (see [`Model::point`]
/// for the remnant's luminosity).
#[must_use]
fn early_agb(
    phase: &EarlyAgb,
    helium: &HeliumStar,
    span: Span,
    coord: f64,
    mt: SolarMasses,
    c: &ZCoeffs,
) -> PhasePoint {
    let clock = span.at(coord);
    let point = phase.at_mass(clock, mt, c);
    let mu = wind::small_envelope_mu(mt, point.core_mass, point.luminosity);
    let thin = mu < 1.0;
    if !thin {
        return point;
    }
    let relation = helium.relation().luminosity(phase.co_core_mass(clock));
    let blend = coord / EARLY_AGB_REMNANT_BLEND;
    let luminosity = if blend < 1.0 {
        let l_tms = helium.l_tms().value();
        SolarLuminosities::new(l_tms * crate::math::powf(relation.value() / l_tms, blend))
    } else {
        relation
    };
    let remnant = CoreRemnant {
        luminosity,
        radius: helium.shell_radius(luminosity),
    };
    envelope::perturb(point, mt, mu, remnant)
}

/// A giant on the Hertzsprung gap or the first giant branch, perturbed towards a naked helium
/// star of its core (a non-degenerate core) or a helium white dwarf at formation (a degenerate one)
/// as its envelope thins (HPT section 6.3, where the text's "M < `M_HeF`" for the helium star has
/// lost its comparison sign: the published SSE code, `hrdiag`, gives the helium star to masses
/// above `M_HeF`, whose cores are not degenerate).
#[must_use]
fn giant(
    phase: Phase,
    point: PhasePoint,
    mt: SolarMasses,
    core: HeliumCore,
    phys: &Physics<'_>,
) -> Point {
    let mu = wind::small_envelope_mu(mt, point.core_mass, point.luminosity);
    if mu >= 1.0 {
        return Point { phase, point };
    }
    let point = match core {
        HeliumCore::NonDegenerate => envelope::perturb(
            point,
            mt,
            mu,
            CoreRemnant {
                luminosity: helium::zams_luminosity(point.core_mass),
                radius: helium::zams_radius(point.core_mass),
            },
        ),
        HeliumCore::Degenerate => {
            perturb_to_white_dwarf(point, mt, mu, WhiteDwarfCore::Helium, phys)
        }
    };
    Point { phase, point }
}

/// `point` perturbed towards the white dwarf of its core at formation (HPT section 6.3: equation
/// 90 with t = 0 and A = 4 for helium or 15 for carbon–oxygen, and equation 91's radius).
#[must_use]
fn perturb_to_white_dwarf(
    point: PhasePoint,
    mt: SolarMasses,
    mu: f64,
    core: WhiteDwarfCore,
    phys: &Physics<'_>,
) -> PhasePoint {
    if mu >= 1.0 {
        return point;
    }
    let mc = point.core_mass;
    envelope::perturb(
        point,
        mt,
        mu,
        CoreRemnant {
            luminosity: white_dwarf::hpt_luminosity(core, mc, Years::ZERO, phys.z),
            radius: white_dwarf_radius(phys.remnant, mc),
        },
    )
}

/// A remnant of `phase` and `mass`, `age` after its formation: a white dwarf cools by HPT's
/// equation 90, a neutron star by equation 93, and a black hole is dark (HPT's 10⁻¹⁰ L☉ of
/// equation 96 guards a division, not a physical luminosity; P06.T22). The radii are those of
/// the track's remnant recipe (P06.T11). `NoRemnant` has neither.
#[must_use]
fn remnant(phase: Phase, mass: SolarMasses, age: Years, phys: &Physics<'_>) -> PhasePoint {
    let (luminosity, radius) = match phase {
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {
            let core =
                WhiteDwarfCore::of(phase).expect("a white dwarf phase has a white dwarf core");
            (
                white_dwarf::hpt_luminosity(core, mass, age, phys.z),
                white_dwarf_radius(phys.remnant, mass),
            )
        }
        Phase::NeutronStar => (
            neutron_star::hpt_luminosity(mass, age),
            neutron_star_radius(phys.remnant),
        ),
        Phase::BlackHole => (
            SolarLuminosities::ZERO,
            black_hole_radius(phys.remnant, mass),
        ),
        Phase::NoRemnant
        | Phase::Protostar
        | Phase::PreMainSequence
        | Phase::MainSequence
        | Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::PostAgb
        | Phase::Substellar => (SolarLuminosities::ZERO, crate::units::SolarRadii::ZERO),
    };
    PhasePoint {
        luminosity,
        radius,
        core_mass: mass,
    }
}
