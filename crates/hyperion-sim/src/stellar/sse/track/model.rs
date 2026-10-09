//! What a segment of a track evaluates: one phase's closed forms at the star's effective initial
//! mass, with every radius at its current mass (HPT section 7.1), the small-envelope perturbation
//! towards the remnant its core would become (HPT section 6.3), and the remnants themselves.
//!
//! A [`Model`] is built once per segment. Phases whose initial mass is the current one (the main
//! sequence and the helium main sequence, HPT section 7.1) rebuild their closed forms at the
//! current mass on each evaluation unless the segment holds its mass fixed; every other phase keeps
//! the mass it was entered with and is built once.

use crate::stellar::Phase;
use crate::stellar::premain::{Contraction, Protostar};
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
use super::Bridges;
use super::excess::HeliumHook;
use super::post_agb::PostAgb;

/// Everything a model reads that is fixed for the whole track.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Physics<'a> {
    /// The metallicity's coefficients.
    pub(crate) coeffs: &'a ZCoeffs,
    /// The metal fraction the formulae see (and the white dwarfs' cooling reads).
    pub(crate) z: MetalFraction,
    /// The remnants' structure.
    pub(crate) remnant: RemnantRecipe,
    /// Whether the end of the AGB is bridged ([`Bridges::Physical`]): the thermally pulsing AGB
    /// then keeps its giant's luminosity and radius to the loss of its envelope, and the post-AGB
    /// bridge crosses to the white dwarf (P06.T16.a).
    pub(crate) bridges: Bridges,
    /// The helium-excess correction, for a star with ΔY > 0 (P06.T17).
    pub(crate) helium: Option<HeliumHook>,
}

/// A phase's clock across a segment: the phase's own time at the segment's coordinate x, from
/// `start` at x = 0 to `end` at x = 1, Myr from the zero-age main sequence of the phase's mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Span {
    pub(crate) start: Megayears,
    pub(crate) end: Megayears,
    /// The factor by which the phase's time runs slower than its clock: 1, but for the helium
    /// excess's correction of the Hertzsprung gap and the first giant branch (P06.T17).
    pub(crate) stretch: f64,
}

impl Span {
    /// The span from `start` to `end` of the phase's clock, run at the clock's own pace.
    #[must_use]
    pub(super) const fn new(start: Megayears, end: Megayears) -> Self {
        Self {
            start,
            end,
            stretch: 1.0,
        }
    }

    /// This span with its time stretched by `stretch` (see [`Span::stretch`]).
    #[must_use]
    pub(super) const fn stretched(self, stretch: f64) -> Self {
        Self { stretch, ..self }
    }

    /// The clock at x, exact at both ends: (1 − x) start + x end.
    #[must_use]
    pub(super) fn at(self, x: f64) -> Megayears {
        Megayears::new((1.0 - x) * self.start.value() + x * self.end.value())
    }

    /// The span's length, years: its clock's, times the stretch.
    #[must_use]
    pub(super) fn years(self) -> f64 {
        (self.end.value() - self.start.value()) * 1e6 * self.stretch
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
    /// The protostar, from the onset of collapse to the end of accretion (P06.T15.a).
    Protostar(Protostar),
    /// The contraction from the end of accretion to the zero-age main sequence (P06.T15.b), boxed:
    /// its closed forms are larger than most phases'.
    PreMainSequence(Box<Contraction>),
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
    /// Core helium burning, boxed: its closed forms are the largest of any phase's.
    CoreHeliumBurning {
        phase: Box<CoreHeliumBurning>,
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
    /// The post-AGB crossing from the loss of the envelope at `start` (years since the onset of
    /// collapse) to the white dwarf's knee (P06.T16.a).
    PostAgb { bridge: PostAgb, start: f64 },
    /// A remnant, from its formation at `birth` (years since the onset of collapse); a white
    /// dwarf's cooling law starts at `origin` of its own clock (`white_dwarf::cooling_origin`,
    /// zero for every other remnant).
    Remnant {
        phase: Phase,
        mass: SolarMasses,
        birth: f64,
        origin: Megayears,
        /// log₁₀ L at the post-AGB knee of a white dwarf that crossed the bridge (ruling 127.1).
        knee: Option<f64>,
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
    /// core helium burning leaves the core's remnant. As in the published SSE code (`hrdiag`,
    /// stellar type 5), it passes from the second to the first geometrically over the first third
    /// of the time from the base of the AGB to the star's nuclear end at its current mass, so that
    /// the perturbed luminosity is continuous there ([`early_agb_core`]); HPT print only the
    /// relation.
    #[must_use]
    pub(super) fn point(&self, phys: &Physics<'_>, coord: f64, mt: SolarMasses, age: f64) -> Point {
        let c = phys.coeffs;
        match self {
            Self::Protostar(protostar) => Point {
                phase: Phase::Protostar,
                point: protostar.at(age),
            },
            Self::PreMainSequence(contraction) => Point {
                phase: Phase::PreMainSequence,
                point: contraction.at(age),
            },
            Self::PostAgb { bridge, start } => post_agb(bridge, age - start, mt),
            Self::MainSequence { fixed: Some(ms) } => Point {
                phase: Phase::MainSequence,
                point: ms.at(ms.t_ms() * coord),
            },
            Self::MainSequence { fixed: None } | Self::HeliumMainSequence { fixed: None } => {
                self.rebuilt_point(phys, coord, mt, age).0
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
                point: horizontal_branch(core_helium_burning(phase, *span, coord, mt, c), mt, phys),
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
                if phys.bridges == Bridges::Physical {
                    // The post-AGB bridge takes the star to the white dwarf (P06.T16.a).
                    return Point {
                        phase: Phase::ThermallyPulsingAgb,
                        point,
                    };
                }
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
            Self::HeliumMainSequence { fixed: Some(star) } => Point {
                phase: Phase::HeliumMainSequence,
                point: star.at(star.t_ms() * coord),
            },
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
            Self::Remnant {
                phase,
                mass,
                birth,
                origin,
                knee,
            } => Point {
                phase: *phase,
                point: remnant(*phase, *mass, age - birth, *origin, *knee, phys),
            },
        }
    }
}

impl Model {
    /// Whether the model is one of the stages before the main sequence (P06.T15), which the
    /// track's maxima leave out.
    #[must_use]
    pub(super) const fn is_before_main_sequence(&self) -> bool {
        matches!(self, Self::Protostar(_) | Self::PreMainSequence(_))
    }

    /// The bytes the model owns on the heap, beyond `size_of::<Model>()`.
    #[must_use]
    pub(super) fn heap_bytes(&self) -> usize {
        match self {
            Self::CoreHeliumBurning { .. } => size_of::<CoreHeliumBurning>(),
            Self::PreMainSequence(_) => size_of::<Contraction>(),
            Self::Protostar(_)
            | Self::PostAgb { .. }
            | Self::MainSequence { .. }
            | Self::HertzsprungGap { .. }
            | Self::FirstGiantBranch { .. }
            | Self::FlashBridge { .. }
            | Self::EarlyAgb { .. }
            | Self::ThermallyPulsingAgb { .. }
            | Self::HeliumMainSequence { .. }
            | Self::HeliumShellBurning { .. }
            | Self::Remnant { .. } => 0,
        }
    }

    /// The state at fractional age `coord` of a main sequence or helium main sequence whose closed
    /// forms are rebuilt at the current mass `mt`, with the phase's lifetime at `mt`, Myr, which
    /// the rebuild computes on the way: `None` for every other model. [`Model::point`] is its
    /// state, bit for bit.
    #[must_use]
    pub(super) fn rebuilt_point(
        &self,
        phys: &Physics<'_>,
        coord: f64,
        mt: SolarMasses,
        age: f64,
    ) -> (Point, Option<Megayears>) {
        match self {
            Self::MainSequence { fixed: None } => {
                let ms = MainSequence::new(mt, phys.coeffs);
                let point = Point {
                    phase: Phase::MainSequence,
                    point: ms.at(ms.t_ms() * coord),
                };
                let t_ms = phys.helium.map_or(ms.t_ms(), |hook| {
                    Megayears::new(ms.t_ms().value() * hook.timescale_factor(mt.value()))
                });
                (point, Some(t_ms))
            }
            Self::HeliumMainSequence { fixed: None } => {
                let (point, t_ms) = helium::main_sequence_at_fraction(mt, coord);
                let point = Point {
                    phase: Phase::HeliumMainSequence,
                    point,
                };
                (point, Some(t_ms))
            }
            Self::Protostar(_)
            | Self::PreMainSequence(_)
            | Self::PostAgb { .. }
            | Self::MainSequence { fixed: Some(_) }
            | Self::HeliumMainSequence { fixed: Some(_) }
            | Self::HertzsprungGap { .. }
            | Self::FirstGiantBranch { .. }
            | Self::FlashBridge { .. }
            | Self::CoreHeliumBurning { .. }
            | Self::EarlyAgb { .. }
            | Self::ThermallyPulsingAgb { .. }
            | Self::HeliumShellBurning { .. }
            | Self::Remnant { .. } => (self.point(phys, coord, mt, age), None),
        }
    }
}

/// The post-AGB crossing's state `years` after it began, with `core` (P06.T16.a).
#[must_use]
fn post_agb(bridge: &PostAgb, years: f64, core: SolarMasses) -> Point {
    let [log_l, log_r] = bridge.at(years);
    Point {
        phase: Phase::PostAgb,
        point: PhasePoint {
            luminosity: SolarLuminosities::new(crate::math::exp10(log_l)),
            radius: crate::units::SolarRadii::new(crate::math::exp10(log_r)),
            core_mass: core,
        },
    }
}

/// A horizontal-branch `point` at current mass `mt`, with the helium excess's shift where the
/// star has one (P06.T17).
#[must_use]
fn horizontal_branch(point: PhasePoint, mt: SolarMasses, phys: &Physics<'_>) -> PhasePoint {
    phys.helium
        .map_or(point, |hook| hook.horizontal_branch(point, mt))
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
/// the naked helium star `helium` of its helium core as its envelope thins ([`early_agb_core`]).
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
    envelope::perturb(point, mt, mu, early_agb_core(phase, helium, clock, mt))
}

/// The core of the early AGB at its clock `clock` for current mass `mt`: the naked helium star
/// `helium` of its helium core, `Mc,He`, at luminosity `Lc`, with radius `R_HeGB`(`Mc,He`, `Lc`) =
/// min(R₁, R₂) (HPT section 6.3 after equation 105, equations 84–88, R₁ and R₂ of equations 86 and
/// 88; the published SSE code's `hrdiag`, stellar type 5, `rx` = min(`rhehgf`, `rhegbf`), which is
/// also its core radius).
///
/// `Lc` passes from the end of the helium main sequence, `L_THe`, where core helium burning leaves
/// the core's remnant, to the helium giants' relation (equation 84) at the early AGB's
/// carbon–oxygen core, geometrically: `Lc` = `L_THe` (L ÷ `L_THe`)^τ while τ < 1, then the
/// relation. τ is SSE's, 3 (t − `t_BAGB`) ÷ (`t_n` − `t_BAGB`) with `t_n` the star's nuclear end
/// at its current mass, which spans the thermally pulsing AGB ([`EarlyAgb::remnant_tau`]); HPT
/// print only the relation.
///
/// Both the small-envelope remnant ([`early_agb`]) and the core radius of the binary's structure
/// (`binary::core_radius`; BSE section 2.7.1) read it, unconditionally of the envelope's mass, so
/// that the two never disagree.
#[must_use]
pub(super) fn early_agb_core(
    phase: &EarlyAgb,
    helium: &HeliumStar,
    clock: Megayears,
    mt: SolarMasses,
) -> CoreRemnant {
    early_agb_core_at_tau(phase, helium, clock, phase.remnant_tau(clock, mt))
}

/// [`early_agb_core`] at the remnant's fractional age `tau`.
#[must_use]
pub(super) fn early_agb_core_at_tau(
    phase: &EarlyAgb,
    helium: &HeliumStar,
    clock: Megayears,
    tau: f64,
) -> CoreRemnant {
    let relation = helium.relation().luminosity(phase.co_core_mass(clock));
    let luminosity = if tau < 1.0 {
        let l_tms = helium.l_tms().value();
        SolarLuminosities::new(l_tms * crate::math::powf_positive(relation.value() / l_tms, tau))
    } else {
        relation
    };
    CoreRemnant {
        luminosity,
        radius: helium.shell_radius(luminosity),
    }
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

/// `point` perturbed towards the white dwarf of its core at formation (HPT section 6.3: the
/// recipe's cooling law at t = 0, with A = 4 for helium or 15 for carbon–oxygen, and equation 91's
/// radius; `white_dwarf::formation_luminosity`).
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
            luminosity: white_dwarf::formation_luminosity(phys.remnant, core, mc, phys.z),
            radius: white_dwarf_radius(phys.remnant, mc),
        },
    )
}

/// A remnant of `phase` and `mass`, `years_since_birth` after its formation (a negative span
/// reads as zero): a white dwarf cools by the
/// recipe's law from `origin` of its clock (the Montreal fit under the default, HPT's equation
/// 90 under `Hurley2000`; P06.T20.a), a neutron star by equation 93, and a black hole is dark
/// (HPT's 10⁻¹⁰ L☉ of equation 96 guards a division, not a physical luminosity; P06.T22). The
/// radii are those of the track's remnant recipe (P06.T11). `NoRemnant` has neither.
///
/// A white dwarf that crossed the post-AGB bridge (`knee`, its log₁₀ L at the knee) fades on
/// Miller Bertolami's shape to 10 L☉ first, over `post_agb::fade_years`, and only then follows its
/// cooling law, whose `origin` puts 10 L☉ at the fade's end (ruling 127.1); its radius is inflated
/// as it leaves the knee (ruling 124.2, `post_agb::inflated_radius`).
#[must_use]
fn remnant(
    phase: Phase,
    mass: SolarMasses,
    years_since_birth: f64,
    origin: Megayears,
    knee: Option<f64>,
    phys: &Physics<'_>,
) -> PhasePoint {
    let age = Years::new(years_since_birth.max(0.0));
    let (luminosity, radius) = match phase {
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {
            let core =
                WhiteDwarfCore::of(phase).expect("a white dwarf phase has a white dwarf core");
            let fading = knee
                .and_then(|log_l| super::post_agb::fade_log_l(log_l, mass.value(), age.value()));
            let luminosity = fading.map_or_else(
                || white_dwarf::luminosity(phys.remnant, core, mass, age, origin, phys.z),
                |log_l| SolarLuminosities::new(crate::math::exp10(log_l)),
            );
            let cold = white_dwarf_radius(phys.remnant, mass);
            let radius = knee.map_or(cold, |log_l| {
                crate::units::SolarRadii::new(super::post_agb::inflated_radius(
                    cold.value(),
                    mass.value(),
                    luminosity.value(),
                    crate::math::exp10(log_l),
                ))
            });
            (luminosity, radius)
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
