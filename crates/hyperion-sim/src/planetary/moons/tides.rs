//! A moon's tides by its composition and state: one complex Love number per moon (plan 14,
//! P14.T56.d; decision-p14-moons §3 and §F-ocean F3, decision-p14-t56d-membrane).
//!
//! # The convention
//!
//! A moon's tidal response is a complex Love number k₂ = |k₂| e^(−iε), whose modulus is the
//! Love number k₂ that despins and figures read, and whose loss −Im k₂ = |k₂| sin ε = k₂ ÷ Q, with
//! Q⁻¹ = sin ε, is what dissipates: "Im k₂" here is that positive loss. The eccentricity tide's
//! heat is (21 ÷ 2)(k₂ ÷ Q) G `M_p`² R⁵ n e² ÷ a⁶
//! ([`tidal_heating`]), so the tidal heat flux is
//! Φ × Im k₂, with the forcing Φ = (21 ÷ 2) G `M_p`² R⁵ n e² ÷ (a⁶ 4πR²) ([`tidal_forcing`]), and
//! the despinning time reads Q ÷ k₂, its inverse.
//!
//! # The three branches
//!
//! A moon's branch reads its whole heat flow F (§F-ocean F3): P14.T24.b's non-tidal `F_nt`, the
//! radiogenic term with its interior's residual, plus the branch's own tidal heat. The first that
//! holds decides ([`MoonTides::of`]):
//!
//! 1. **The ocean branch** ([`TidalBranch::Ocean`]), for a moon with water, if
//!    [`has_subsurface_ocean`](crate::planetary::moons::regular::has_subsurface_ocean) holds at
//!    `F_ocean`. Its ice shell is a thin membrane over the ocean on a rigid mantle, Beuthe's own
//!    form (2015, Icarus 248, 109, eqs. 28 and 43–44): k₂ = `k_oc` ÷ (1 + h°Λ)
//!    ([`membrane_love_number`]), with the ocean's limit `k_oc` = 3`ρ_w` ÷ (5ρ̄ − 3`ρ_w`)
//!    ([`ocean_love_number`]), h° = 1 + `k_oc`, and the membrane's Λ = 8(1 + ν) ÷ (5 + ν) × μ̄ d ÷
//!    (`ρ_w` g R²) over the ocean's density, d the conductive shell 651 ln(273 K ÷ `T_s`) ÷ F
//!    (decision-p14-t56d-membrane, which corrects decision-p14-moons §F-ocean F2's ρ̄ and its
//!    missing h°). Λ is complex through the shell's Maxwell rigidity, Beuthe 2019's Arrhenius ice
//!    at 10¹⁴ Pa s, averaged over its conductive profile ([`shell_modulus`]), so its loss factor is
//!    the moon's own, from its period and surface temperature: no loss factor is fitted. `F_ocean`
//!    is the largest stable root of F = `F_nt` + Φ Im k₂(d(F)), the warm start a moon formed hot in
//!    its planet's disc keeps ([`ocean_heat_flow`]). From P14.T51.g.b's bump at generator version
//!    24 the test is the ocean's own presence test, at `F_ocean`, over the moon's water and
//!    solutes, and d(F) its spherical shell, its modulus averaged to its base's liquidus: a
//!    bracketed root of F − `F_nt` − Φ Im k₂(Λ(d(F))) (decision-p14-t51g's T56.d branch).
//! 2. **The heated branch** ([`TidalBranch::Heated`]), for a rocky moon if `F_nt` and its tidal
//!    flux at Io's Im k₂ = 0.015
//!    ([`MOON_TIDAL_RESPONSE`]) reach
//!    P14.T24.b's `F_c` ([`SURFACE_MELT_HEAT_FLOW`]): a partially molten interior dissipating in
//!    thermal-orbital equilibrium, as Io does. Its k₂ is the elastic body's at
//!    [`HEATED_ROCK_RIGIDITY`], and its Im k₂ Io's.
//! 3. **The cold branch** ([`TidalBranch::Cold`]) otherwise: the elastic body's k₂ =
//!    `k_f` ÷ (1 + 19μ ÷ 2ρ̄gR) at its material's rigidity ([`elastic_love_number`]:
//!    [`ROCK_RIGIDITY`] or [`ICE_RIGIDITY`]), with the fluid Love number `k_f` of its moment of
//!    inertia ([`fluid_love_number`]), and its material's cold Q ([`COLD_ROCK_TIDAL_Q`] or
//!    [`COLD_ICE_TIDAL_Q`]).
//!
//! The ocean test needs only the water a moon holds, so a rocky moon with some water (Europa, 8%
//! by mass) may take it; an icy moon that fails it is cold. A moon whose heated shell is a rock
//! lag (P14.T56.g) takes no ocean branch at all (`MoonTides::without_an_ocean`, P14.T51.g.a): the
//! lag holds no ice to float on an ocean (decision-p14-t56g §1), so its branch falls to the heated
//! or the cold one, as an icy moon's that fails the test does. The full thermal-orbital
//! equilibrium, a Maxwell or Andrade body whose viscosity balances tidal heat against convective
//! loss, would give Im k₂ continuously in place of the branches: P14.T55.j.
//!
//! # The porous core (P14.T56.e)
//!
//! A conductive shell alone keeps no Enceladus ocean, as the literature finds
//! (decision-p14-t56d-membrane §2.4): its heat is a porous, water-saturated, unconsolidated rock
//! core's (Choblet et al. 2017, Nature Astronomy 1, 841). The ocean branch's rock, under the
//! solve's column ([`OceanCore`]), is porous where its effective
//! pressure leaves it so ([`CORE_COMPACTION_PRESSURE`]), with a complex rigidity of
//! [`ComplexRigidity::POROUS_CORE`], and the shell's membrane floats on it
//! ([`deformable_membrane_love_number`], over the core term [`core_love_number`]); a compacted
//! core, Titan's, Ganymede's, Callisto's and Europa's, stays the rigid mantle, its figures
//! unchanged to the bit. The rival, dissipation at the shell's base (Hemingway and Mittal 2019,
//! Icarus 332, 111), is not built: it is tuned to the shell's thickness, not read from a rheology.
//! Only an ocean frees the core to deform: under a shell frozen to it, ice some 350 times stiffer
//! holds it, so the heated and cold branches read no core.
//!
//! # The surface's own heat (P14.T55.j)
//!
//! A generated moon's surface, as its tides read it, radiates its whole heat flow F as well as its
//! sunlight, σ `T_s`⁴ = σ `T_eq`⁴ + F ([`radiating_surface_temperature`]), and its shell's top
//! and Maxwell profile read that surface (`with_radiating_surface`, solved to
//! [`SURFACE_HEAT_TOLERANCE`] of `T_s`). Io's 2.2 W m⁻² raises its surface by some 7 K, Enceladus's by 0.35 K: the rise of the
//! effective temperature that radiates the whole flux, not of the passive surface between Io's hot
//! spots, which carry most of its heat. The air and the record's surface conditions read it too: a
//! moon's air starts from that surface, held at its night's floor, wherever its heat flow is
//! positive (lean, 2026-11-02), where every other body's starts from its sunlight, held at the
//! night's floor that F sets (P14.T24.i; ruling 112.7's split).
//!
//! Every function here is pure and deterministic: the shell's modulus is a fixed midpoint sum
//! ([`SHELL_QUADRATURE_NODES`]) and the root is refined by a fixed count of bisections
//! ([`ROOT_STEPS`]).

use std::collections::BTreeMap;

use crate::math;
use crate::planetary::derive::rotation::SpinTides;
use crate::planetary::hooks::figures::Subsurface;
use crate::planetary::hooks::ocean::{
    Hydrosphere, ICE_CONDUCTIVITY_FLOOR, OceanSolutes, SHELL_BASE_HALVINGS, ShellBase,
};
use crate::planetary::moons::regular::{
    ICE_CONDUCTIVITY_COEFFICIENT, ICE_MELTING_POINT, MOON_TIDAL_RESPONSE, WATER_DENSITY,
    tidal_heating,
};
use crate::planetary::params::{
    COLD_ICE_TIDAL_Q, COLD_ROCK_TIDAL_Q, CORE_COMPACTION_PRESSURE, HEATED_ROCK_RIGIDITY,
    ICE_CREEP_ACTIVATION_ENERGY_J_PER_MOL, ICE_MELTING_VISCOSITY_PA_S, ICE_POISSON_RATIO,
    ICE_RIGIDITY, ICY_WATER_FRACTION, POROUS_CORE_DISSIPATION, POROUS_CORE_RIGIDITY, ROCK_RIGIDITY,
    SURFACE_MELT_HEAT_FLOW,
};
use crate::substance::phase::MOLAR_GAS_CONSTANT;
use crate::units::consts::{GRAVITATIONAL_CONSTANT, STEFAN_BOLTZMANN};
use crate::units::{
    Kelvin, Kilograms, KilogramsPerCubicMetre, Metres, MetresPerSecondSquared, Pascals,
    WattsPerSquareMetre,
};

/// The fluid Love number `k_f` of a body of moment of inertia C ÷ M a², the Darwin–Radau relation
/// inverted: P14.T56.a's, re-exported so that a moon's tides and its parent's J₂ read one
/// function (decision-p14-moons §3.2: 3 ÷ 2 at C = 0.4, 0.93 at the rocky class's 0.33 and 1.00
/// at the icy class's 0.34).
pub use crate::planetary::laplace::fluid_love_number;

/// The bisections that refine [`ocean_heat_flow`]'s root: 64, which shrink its bracket, at most
/// the quadratic's root wide, to adjacent doubles for any root within a factor of 10³ of it.
pub const ROOT_STEPS: u32 = 64;

/// The elastic Love number of a homogeneous body of fluid Love number `fluid`, rigidity
/// `rigidity`, mean density `density`, surface gravity `gravity` and radius `radius`:
/// `k_f` ÷ (1 + μ̃), with the effective rigidity μ̃ = 19μ ÷ (2ρ̄gR) (decision-p14-moons §3.2).
///
/// It is the homogeneous elastic sphere's form, (3 ÷ 2) ÷ (1 + μ̃) (Love 1911; Munk and `MacDonald`
/// 1960; as Gladman et al. 1996, Icarus 122, 166, and Meyer and Wisdom 2007, Icarus 188, 535,
/// eq. 21, take it), with decision-p14-moons §3.2's `k_f` in place of the 3 ÷ 2, so that a
/// centrally condensed body takes its own fluid limit: the ruling's ansatz, which the rigidities
/// calibrated on the Moon and Io absorb. It tends to `k_f` as μ → 0, a fluid body's, and falls as
/// 1 ÷ μ̃ for a small, stiff one. Dimensionless.
///
/// # Examples
///
/// The Moon, at its measured moment of inertia, has GRAIL's k₂ of 0.024:
///
/// ```
/// use hyperion_sim::planetary::moons::tides::{elastic_love_number, fluid_love_number};
/// use hyperion_sim::planetary::params::ROCK_RIGIDITY;
/// use hyperion_sim::units::{KilogramsPerCubicMetre, Metres, MetresPerSecondSquared};
///
/// let k2 = elastic_love_number(
///     fluid_love_number(0.392_863),
///     ROCK_RIGIDITY,
///     KilogramsPerCubicMetre::new(3_344.0),
///     MetresPerSecondSquared::new(1.62),
///     Metres::new(1.7374e6),
/// );
/// assert!((k2 / 0.0242 - 1.0).abs() < 0.02);
/// ```
#[must_use]
pub fn elastic_love_number(
    fluid: f64,
    rigidity: Pascals,
    density: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared,
    radius: Metres,
) -> f64 {
    let effective =
        19.0 * rigidity.value() / (2.0 * density.value() * gravity.value() * radius.value());
    fluid / (1.0 + effective)
}

/// The Love number of a body of mean density `density` whose surface floats on a global ocean,
/// the shell's rigidity neglected: `k_oc` = 3`ρ_w` ÷ (5ρ̄ − 3`ρ_w`), with `ρ_w`
/// [`WATER_DENSITY`] (decision-p14-moons §3.2; Beuthe 2015, Icarus 248, 109, eq. 44, a fluid
/// ocean over a rigid mantle). Dimensionless; positive for ρ̄ above 3`ρ_w` ÷ 5, which every moon
/// is.
///
/// Europa's elements give 0.249, against Moore and Schubert's (2000, Icarus 147, 317) ≈ 0.25 with
/// an ocean (as the ruling read it); Titan's 0.47, under Iess et al.'s (2012, Science 337, 457)
/// measured 0.589 ± 0.150 (2σ) and Durante et al.'s (2019, Icarus 326, 123) 0.616 ± 0.067 (1σ),
/// 20–24% low. Durante et al. allow an ocean denser than water, a partly viscous interior or a
/// dynamic tide.
#[must_use]
pub fn ocean_love_number(density: KilogramsPerCubicMetre) -> f64 {
    let water = WATER_DENSITY.value();
    3.0 * water / (5.0 * density.value() - 3.0 * water)
}

/// A complex Love number k₂ = `real` + i `imaginary`, dimensionless (see the [module](self)
/// documentation's convention): its modulus is the Love number, and −`imaginary` the loss
/// k₂ ÷ Q.
///
/// The fields are plain values with no invariant between them, so they are public.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct ComplexLoveNumber {
    /// Re k₂.
    pub real: f64,
    /// Im k₂: negative for a body that dissipates.
    pub imaginary: f64,
}

impl ComplexLoveNumber {
    /// |k₂|, the Love number that despins and figures read.
    #[must_use]
    pub fn modulus(&self) -> f64 {
        math::hypot(self.real, self.imaginary)
    }

    /// The loss −Im k₂ = |k₂| ÷ Q, positive for a body that dissipates.
    #[must_use]
    pub fn loss(&self) -> f64 {
        -self.imaginary
    }
}

/// A shell's membrane stiffness Λ, complex and dimensionless (Beuthe 2015, Icarus 248, 109,
/// eq. 28): its real part the shell's elastic resistance against the ocean's gravity, its
/// imaginary part the shell's loss.
///
/// The fields are plain values, so they are public.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct MembraneStiffness {
    /// Re Λ.
    pub real: f64,
    /// Im Λ, not negative for a shell that dissipates.
    pub imaginary: f64,
}

/// The complex Love number of a thin shell over a global ocean on a rigid mantle (Beuthe 2015,
/// Icarus 248, 109, eqs. 43–44; decision-p14-t56d-membrane §2.1): k₂ = `k_oc` ÷ (1 + h°Λ), with
/// the ocean's limit `ocean` (`k_oc`, [`ocean_love_number`]), h° = 1 + `k_oc` = 5ρ̄ ÷ (5ρ̄ − 3`ρ_w`),
/// and the shell's membrane stiffness `lambda` (Λ, [`ShellStiffness::at`]).
///
/// Re k₂ = `k_oc`(1 + h°`Λ_R`) ÷ |1 + h°Λ|² and −Im k₂ = `k_oc` h°`Λ_I` ÷ |1 + h°Λ|², all of it the
/// shell's, since the mantle is rigid (his eq. 101). A large moon's shell is soft against its
/// ocean's gravity (h°|Λ| ≪ 1, k₂ near `k_oc`), a small moon's stiff (h°|Λ| ≫ 1, k₂ near 0). At
/// Λ = 0 it is `k_oc`, real.
///
/// # Examples
///
/// Europa's 33.2 km Maxwell shell, Λ = 0.07049 + 0.002786i (decision-p14-t56d-membrane §3), keeps
/// its k₂ near the ocean's limit and dissipates a three-hundredth of it:
///
/// ```
/// use hyperion_sim::planetary::moons::tides::{MembraneStiffness, membrane_love_number};
///
/// let lambda = MembraneStiffness { real: 0.070_49, imaginary: 0.002_786 };
/// let k2 = membrane_love_number(0.249, lambda);
/// assert!((k2.modulus() - 0.228_85).abs() < 1e-4);
/// assert!((k2.loss() - 7.318e-4).abs() < 1e-6);
/// ```
#[must_use]
pub fn membrane_love_number(ocean: f64, lambda: MembraneStiffness) -> ComplexLoveNumber {
    let shell = 1.0 + ocean;
    let (re, im) = (1.0 + shell * lambda.real, shell * lambda.imaginary);
    let norm = re * re + im * im;
    ComplexLoveNumber {
        real: ocean * re / norm,
        // 0 − x, so that a shell of no loss gives +0, not −0.
        imaginary: 0.0 - ocean * im / norm,
    }
}

/// The surface temperature of a solid body of equilibrium temperature `equilibrium` that also
/// radiates the heat flow `heat_flow` it brings to its surface from within:
/// σ `T_s`⁴ = σ `T_eq`⁴ + F, so `T_s` = (`T_eq`⁴ + F ÷ σ)^¼ (P14.T55.j), with σ the
/// Stefan–Boltzmann constant. `T_eq` itself for a flow that is not positive.
///
/// Io's 2.2 W m⁻² raises a 110 K surface by some 7 K, Enceladus's 34 mW m⁻² a 75 K one by 0.35 K,
/// and 413 W m⁻² sets it at 292 K or more whatever its sunlight.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::moons::tides::radiating_surface_temperature;
/// use hyperion_sim::units::{Kelvin, WattsPerSquareMetre};
///
/// let io = radiating_surface_temperature(Kelvin::new(110.0), WattsPerSquareMetre::new(2.2));
/// assert!((io.value() - 116.6).abs() < 0.1, "{io:?}");
/// let hot = radiating_surface_temperature(Kelvin::new(0.0), WattsPerSquareMetre::new(413.0));
/// assert!((hot.value() - 292.2).abs() < 0.1, "{hot:?}");
/// ```
#[must_use]
pub fn radiating_surface_temperature(
    equilibrium: Kelvin,
    heat_flow: WattsPerSquareMetre,
) -> Kelvin {
    let f = heat_flow.value();
    if f.is_nan() || f <= 0.0 {
        return equilibrium;
    }
    let t4 = math::powi(equilibrium.value(), 4) + f / STEFAN_BOLTZMANN;
    Kelvin::new(t4.sqrt().sqrt())
}

/// The relative tolerance on a moon's surface temperature to which `with_radiating_surface`
/// solves its balance: 10⁻¹², so that the balance σ `T_s`⁴ − σ `T_eq`⁴ = F holds to 10⁻⁹ of F
/// wherever F is at least 4 × 10⁻³ of σ `T_s`⁴ (Enceladus's is 2 × 10⁻²), some 10⁴ ulp of `T_s`,
/// well above the arithmetic's own error.
pub const SURFACE_HEAT_TOLERANCE: f64 = 1e-12;

/// The most solves `with_radiating_surface` takes of a moon's balance: 64, beyond the 42
/// halvings that close a bracket of 300 K to [`SURFACE_HEAT_TOLERANCE`] of 100 K. A balance on a
/// jump took 49 at most over P14.T55.j's census, one on a root 9.
pub const SURFACE_HEAT_SOLVES: u32 = 64;

/// How a moon's surface balance ended ([`radiating_balance`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum BalanceEnd {
    /// At a root: its surface radiates the heat its tides make there, to
    /// [`SURFACE_HEAT_TOLERANCE`].
    Root,
    /// At a jump with no root: its bracket closed to [`SURFACE_HEAT_TOLERANCE`] on a branch's edge,
    /// its heat making the surface warm on one side and cool on the other, and it takes the side
    /// on which its surface radiates more than its heat.
    Jump,
    /// At the last of [`SURFACE_HEAT_SOLVES`], at neither.
    Unsettled,
}

/// The tides `solve` gives the moon of `inputs`, whose surface temperature there is its
/// equilibrium temperature, and the surface temperature they are read at: the surface radiates the
/// moon's whole heat flow too, F its tides' ([`MoonTides::heat_flow`]: the ocean branch's root, or
/// the rock's heat with its tidal flux), which its shell's depth and top read in turn, solved to
/// convergence ([`radiating_balance`]).
#[must_use]
pub(crate) fn with_radiating_surface(
    inputs: &TideInputs,
    solve: impl FnMut(&TideInputs) -> (MoonTides, Subsurface),
) -> (MoonTides, Subsurface, Kelvin) {
    let (tides, subsurface, surface, _) = radiating_balance(inputs, solve);
    (tides, subsurface, surface)
}

/// The surface temperature `T_s` at which the moon of `inputs`, of equilibrium temperature
/// `T_eq` their surface's, radiates the heat flow F(`T_s`) of the tides `solve` gives it there:
/// the root of h(T) = T − g(T), g(T) = (`T_eq`⁴ + F(T) ÷ σ)^¼
/// ([`radiating_surface_temperature`]), with the tides there, and how the solve ended, after so
/// many solves.
///
/// From `T_eq`, where h ≤ 0, it steps to g(T) until h changes sign, by the secant through its
/// last two solves where that rises past the last; then it closes the bracket by the Illinois
/// method of false position, halving it where two steps left it more than half its width. It
/// stops where |h| ≤ [`SURFACE_HEAT_TOLERANCE`] T ([`BalanceEnd::Root`]), where the bracket
/// closes to that tolerance on a jump in F with no root ([`BalanceEnd::Jump`], its end where h > 0),
/// or after [`SURFACE_HEAT_SOLVES`] ([`BalanceEnd::Unsettled`], its last solve). A cold or heated
/// moon's heat reads no surface, so it settles at its second solve, h exactly 0.
#[must_use]
pub(crate) fn radiating_balance(
    inputs: &TideInputs,
    mut solve: impl FnMut(&TideInputs) -> (MoonTides, Subsurface),
) -> (MoonTides, Subsurface, Kelvin, (BalanceEnd, u32)) {
    /// One solve at a surface `t`, K, with its h, K.
    #[derive(Clone, Copy)]
    struct Probe {
        t: f64,
        excess: f64,
        tides: MoonTides,
        subsurface: Subsurface,
    }
    let equilibrium = inputs.surface_temperature;
    let mut probe = |t: f64| {
        let (tides, subsurface) = solve(&TideInputs {
            surface_temperature: Kelvin::new(t),
            ..*inputs
        });
        let radiated = radiating_surface_temperature(equilibrium, tides.heat_flow()).value();
        Probe {
            t,
            excess: t - radiated,
            tides,
            subsurface,
        }
    };
    let done = |p: Probe, end: BalanceEnd, solves: u32| {
        (p.tides, p.subsurface, Kelvin::new(p.t), (end, solves))
    };
    let settled = |p: &Probe| p.excess.abs() <= SURFACE_HEAT_TOLERANCE * p.t;
    // The secant through two solves' h, `None` where it is not finite.
    let secant = |q: &Probe, p: &Probe| {
        let t = p.t - p.excess * (p.t - q.t) / (p.excess - q.excess);
        t.is_finite().then_some(t)
    };
    let mut last = probe(equilibrium.value());
    let mut solves = 1;
    if settled(&last) {
        return done(last, BalanceEnd::Root, solves);
    }
    // Before a bracket: the solves so far, all with h < 0.
    let mut previous: Option<Probe> = None;
    // The bracket: its ends with h < 0 and h > 0, each with the h the false position reads.
    let mut below = (last, last.excess);
    let mut above: Option<(Probe, f64)> = None;
    // Which end the last step kept, and the bracket's width two steps before.
    let (mut kept_below, mut widths) = (None::<bool>, [f64::INFINITY; 2]);
    while solves < SURFACE_HEAT_SOLVES {
        let next = match above {
            None => {
                let fixed = last.t - last.excess;
                previous
                    .as_ref()
                    .and_then(|q| secant(q, &last))
                    .filter(|&t| t > last.t)
                    .unwrap_or(fixed)
            }
            Some((hi, hi_excess)) => {
                let (lo, lo_excess) = below;
                let width = (hi.t - lo.t).abs();
                if width <= SURFACE_HEAT_TOLERANCE * hi.t.max(lo.t) {
                    return done(hi, BalanceEnd::Jump, solves);
                }
                let false_position = lo.t - lo_excess * (hi.t - lo.t) / (hi_excess - lo_excess);
                let inside = (false_position - lo.t) * (false_position - hi.t) < 0.0;
                // Before two steps in the bracket its width two steps back is infinite.
                if inside && width <= 0.5 * widths[0] {
                    false_position
                } else {
                    f64::midpoint(lo.t, hi.t)
                }
            }
        };
        let p = probe(next);
        solves += 1;
        if settled(&p) {
            return done(p, BalanceEnd::Root, solves);
        }
        if let Some((hi, hi_excess)) = above {
            widths = [widths[1], (hi.t - below.0.t).abs()];
            // Illinois: an end kept twice running reads half its h.
            if p.excess < 0.0 {
                below = (p, p.excess);
                if kept_below == Some(false) {
                    above = Some((hi, 0.5 * hi_excess));
                }
                kept_below = Some(false);
            } else {
                above = Some((p, p.excess));
                if kept_below == Some(true) {
                    below.1 *= 0.5;
                }
                kept_below = Some(true);
            }
        } else if p.excess < 0.0 {
            below = (p, p.excess);
            previous = Some(last);
        } else {
            above = Some((p, p.excess));
        }
        last = p;
    }
    done(last, BalanceEnd::Unsettled, solves)
}

/// A complex shear modulus μ̃ = `real` + i `imaginary`, Pa, whose imaginary part dissipates (the
/// [module](self) documentation's convention, as the shell's [`ShellModulus`]).
///
/// The fields are plain values, so they are public.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct ComplexRigidity {
    /// Re μ̃.
    pub real: Pascals,
    /// Im μ̃, not negative for a material that dissipates.
    pub imaginary: Pascals,
}

impl ComplexRigidity {
    /// A porous, water-saturated, unconsolidated rock core's (P14.T56.e): [`POROUS_CORE_RIGIDITY`]
    /// × (1 + i [`POROUS_CORE_DISSIPATION`]), whatever the tide's frequency.
    pub const POROUS_CORE: Self = Self {
        real: POROUS_CORE_RIGIDITY,
        imaginary: Pascals::new(POROUS_CORE_RIGIDITY.value() * POROUS_CORE_DISSIPATION),
    };
}

/// Whether a rock core under an ocean keeps its pores (P14.T56.e; [`OceanCore::compaction`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CoreCompaction {
    /// Porous and unconsolidated: its effective pressure lies below [`CORE_COMPACTION_PRESSURE`].
    Porous,
    /// Compacted: its tide is rigid rock's, as P14.T56.d's mantle.
    Compacted,
}

/// The rock under a moon's global ocean, as its tide reads it (P14.T56.e): a homogeneous,
/// incompressible sphere, under water at [`WATER_DENSITY`] from its surface to the moon's, the
/// ice shell's water included, as the solve's column spreads it
/// (P14.T51.g.b's `WaterColumn`).
///
/// The fields carry no invariant its tide relies on, so they are public; [`under`](Self::under)
/// gives the generator's, a core denser than water.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct OceanCore {
    /// Its radius `r_c`, the water's base.
    pub radius: Metres,
    /// Its mean density `ρ_c`, above [`WATER_DENSITY`].
    pub density: KilogramsPerCubicMetre,
}

impl OceanCore {
    /// The rock under the water of a body of radius `radius` and mass `mass` whose water's base is
    /// at the radius `base`: the mass M − (4π ÷ 3) `ρ_w` (R³ − `r_w`³) within `r_w`, the body's
    /// rock (1 − x) M for a water fraction x. `None` where it is not denser than water, or its base
    /// is not a positive radius within the body's.
    #[must_use]
    pub fn under(radius: Metres, mass: Kilograms, base: Metres) -> Option<Self> {
        let (big_r, r_c) = (radius.value(), base.value());
        let sphere = |r: f64| 4.0 / 3.0 * core::f64::consts::PI * math::powi(r, 3);
        let rock = mass.value() - WATER_DENSITY.value() * (sphere(big_r) - sphere(r_c));
        let density = rock / sphere(r_c);
        let within = r_c > 0.0 && r_c <= big_r;
        (within && density.is_finite() && density > WATER_DENSITY.value()).then(|| Self {
            radius: base,
            density: KilogramsPerCubicMetre::new(density),
        })
    }

    /// The effective pressure at its centre, Pa: its weight's lithostatic pressure less its pore
    /// water's, P′ = (2π ÷ 3) G `ρ_c` (`ρ_c` − `ρ_w`) `r_c`², the stress its grains carry, which
    /// compacts them (Terzaghi's principle). The ocean over it weighs on its pore water as on its
    /// grains, so it adds nothing. Enceladus's core is at 21 MPa, Titan's at 7 GPa.
    #[must_use]
    pub fn effective_pressure(&self) -> Pascals {
        let (rho, r) = (self.density.value(), self.radius.value());
        Pascals::new(
            2.0 * core::f64::consts::PI / 3.0
                * GRAVITATIONAL_CONSTANT
                * rho
                * (rho - WATER_DENSITY.value())
                * r
                * r,
        )
    }

    /// Whether it keeps its pores: [`CoreCompaction::Porous`] below [`CORE_COMPACTION_PRESSURE`]
    /// ([`effective_pressure`](Self::effective_pressure)), [`CoreCompaction::Compacted`] from it.
    #[must_use]
    pub fn compaction(&self) -> CoreCompaction {
        if self.effective_pressure().value() < CORE_COMPACTION_PRESSURE.value() {
            CoreCompaction::Porous
        } else {
            CoreCompaction::Compacted
        }
    }
}

/// A complex number, for the core's Love numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    #[must_use]
    const fn real(re: f64) -> Self {
        Self { re, im: 0.0 }
    }

    #[must_use]
    fn scaled(self, x: f64) -> Self {
        Self {
            re: self.re * x,
            im: self.im * x,
        }
    }

    /// Its squared modulus.
    #[must_use]
    fn norm(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

impl core::ops::Add for Complex {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }
}

impl core::ops::Sub for Complex {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            re: self.re - other.re,
            im: self.im - other.im,
        }
    }
}

impl core::ops::Mul for Complex {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }
}

impl core::ops::Div for Complex {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        let norm = other.norm();
        Self {
            re: (self.re * other.re + self.im * other.im) / norm,
            im: (self.im * other.re - self.re * other.im) / norm,
        }
    }
}

impl From<ComplexLoveNumber> for Complex {
    fn from(k: ComplexLoveNumber) -> Self {
        Self {
            re: k.real,
            im: k.imaginary,
        }
    }
}

impl From<Complex> for ComplexLoveNumber {
    fn from(z: Complex) -> Self {
        Self {
            real: z.re,
            imaginary: z.im,
        }
    }
}

/// The core term s of an ocean world over the deformable rock core `core` of complex rigidity
/// `rigidity`, its mean density `density`, surface gravity `gravity` and radius `radius`
/// (P14.T56.e): its k₂ at the surface were that surface held still, the core's own tide under its
/// ocean, with the ocean's load on the core. Dimensionless; its imaginary part is negative for a
/// core that dissipates, as a Love number's (the [module](self) documentation's convention).
///
/// The core is a homogeneous, incompressible sphere of radius `r_c` = y R, density `ρ_c` and
/// rigidity μ̃, under water at [`WATER_DENSITY`] to the surface. Its interface's displacement
/// answers its effective load (`ρ_c` − `ρ_w`)(Φ′ − `g_c` u) at Kelvin's homogeneous elastic
/// sphere's 5 `r_c` ÷ 19μ̃ per unit of pressure (Love 1911; [`elastic_love_number`]'s
/// 19μ ÷ 2ρgR), where Φ′ is the perturbing potential there, the tide's and the two displaced
/// surfaces', and the incompressible core has no density change within. Solved with the ocean's
/// surface held:
///
/// s = 3 Δρ² y⁷ ÷ (19 μ̃ ρ̄ ÷ (g R) + Δρ y² (2`ρ_c` + 3`ρ_w`)), with Δρ = `ρ_c` − `ρ_w`.
///
/// It is 0 for a rigid core (μ̃ → ∞) and for one of the ocean's density; with no ocean over it
/// (`ρ_w` → 0) it is Kelvin's (3 ÷ 2) ÷ (1 + 19μ̃ ÷ 2`ρ_c` `g_c` `r_c`) times y⁵, the core's own
/// k₂ seen from R; a fluid core gives the two-layer fluid body's.
///
/// # Examples
///
/// Enceladus's porous core (P14.T56.e): its rock, 0.6 of its mass at 2,712 kg m⁻³, under 73 km of
/// water, dissipates where a rigid one would not:
///
/// ```
/// use hyperion_sim::planetary::moons::tides::{ComplexRigidity, OceanCore, core_love_number};
/// use hyperion_sim::units::{KilogramsPerCubicMetre, Metres, MetresPerSecondSquared};
///
/// let core = OceanCore {
///     radius: Metres::new(1.787e5),
///     density: KilogramsPerCubicMetre::new(2_712.0),
/// };
/// let s = core_love_number(
///     core,
///     ComplexRigidity::POROUS_CORE,
///     KilogramsPerCubicMetre::new(1_610.0),
///     MetresPerSecondSquared::new(0.1135),
///     Metres::new(2.521e5),
/// );
/// assert!((0.03..0.04).contains(&s.real) && (0.015..0.02).contains(&s.loss()), "{s:?}");
/// ```
#[must_use]
pub fn core_love_number(
    core: OceanCore,
    rigidity: ComplexRigidity,
    density: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared,
    radius: Metres,
) -> ComplexLoveNumber {
    let (rho_c, rho_w) = (core.density.value(), WATER_DENSITY.value());
    let contrast = rho_c - rho_w;
    let y = core.radius.value() / radius.value();
    let stiffness = 19.0 * density.value() / (gravity.value() * radius.value());
    let below = Complex {
        re: stiffness * rigidity.real.value(),
        im: stiffness * rigidity.imaginary.value(),
    } + Complex::real(contrast * y * y * (2.0 * rho_c + 3.0 * rho_w));
    (Complex::real(3.0 * contrast * contrast * math::powi(y, 7)) / below).into()
}

/// The complex Love number of a thin shell over a global ocean on a deformable core (P14.T56.e):
/// Beuthe's (2015, Icarus 248, 109) membrane over his fluid-crust Love numbers, which here read
/// the core term `core` (s, [`core_love_number`]) of a moon of mean density `density`, under the
/// shell's membrane stiffness `lambda` (Λ, [`ShellStiffness::at`]).
///
/// With ξ = 3`ρ_w` ÷ 5ρ̄ ([`WATER_DENSITY`]), the fluid-crust Love numbers, the shell replaced by
/// water, are h° = (1 + s) ÷ (1 − ξ(1 + s)) and k° = h° − 1, and the membrane gives
///
/// h₂ = h° ÷ (1 + (1 + ξh°)Λ), k₂ = (1 + Λ) h₂ − 1 = (k° + s(1 + ξh°)Λ) ÷ (1 + (1 + ξh°)Λ),
///
/// the second form free of cancellation. It is derived for this task: the shell's restoring
/// pressure `ρ_w` g Λ u, which carries no mass, on the ocean's surface over the core
/// (decision-p14-t56d-membrane §2.4's call for his eqs. 57–59; the paper was not reached in the
/// pod). It keeps his k₂–h₂ relation, (1 + Λ)h₂ = 1 + k₂ (his eq. 34), and at s = 0, a rigid
/// mantle, it is [`membrane_love_number`] (his eq. 43), since there 1 + ξh° = h°. Its loss
/// splits into the core's and the shell's, −Im k₂ = −Im s |1 + ξh₂|² + ξ|h₂|² Im Λ, the shell's
/// his eq. 101, so that each is positive. A stiff shell (|Λ| → ∞) holds the surface still and
/// leaves the core's tide, k₂ → s.
///
/// # Examples
///
/// A rigid core gives the rigid mantle's membrane, and a porous one dissipates under Enceladus's
/// stiff shell, Λ ≈ 22, where the shell alone hardly does:
///
/// ```
/// use hyperion_sim::planetary::moons::tides::{
///     ComplexLoveNumber, MembraneStiffness, deformable_membrane_love_number, membrane_love_number,
///     ocean_love_number,
/// };
/// use hyperion_sim::units::KilogramsPerCubicMetre;
///
/// let density = KilogramsPerCubicMetre::new(1_610.0);
/// let lambda = MembraneStiffness { real: 21.6, imaginary: 0.36 };
/// let rigid = deformable_membrane_love_number(density, ComplexLoveNumber::default(), lambda);
/// let shell = membrane_love_number(ocean_love_number(density), lambda);
/// assert!((rigid.loss() / shell.loss() - 1.0).abs() < 1e-12);
/// let porous = ComplexLoveNumber { real: 0.036, imaginary: -0.017 };
/// let k2 = deformable_membrane_love_number(density, porous, lambda);
/// assert!(k2.loss() > 20.0 * shell.loss());
/// ```
#[must_use]
pub fn deformable_membrane_love_number(
    density: KilogramsPerCubicMetre,
    core: ComplexLoveNumber,
    lambda: MembraneStiffness,
) -> ComplexLoveNumber {
    deformable_membrane(density, core, lambda).1.into()
}

/// [`deformable_membrane_love_number`]'s h₂ and k₂.
#[must_use]
fn deformable_membrane(
    density: KilogramsPerCubicMetre,
    core: ComplexLoveNumber,
    lambda: MembraneStiffness,
) -> (Complex, Complex) {
    let xi = 0.6 * WATER_DENSITY.value() / density.value();
    let s = Complex::from(core);
    let one = Complex::real(1.0);
    let raised = one + s;
    let free = one - raised.scaled(xi);
    let (h0, k0) = (raised / free, (s + raised.scaled(xi)) / free);
    let held = (one + h0.scaled(xi))
        * Complex {
            re: lambda.real,
            im: lambda.imaginary,
        };
    let below = one + held;
    (h0 / below, (k0 + s * held) / below)
}

/// The midpoint nodes in ln T over which [`shell_modulus`] averages a shell's Maxwell rigidity:
/// 512 (decision-p14-t56d-membrane §2.2). Their worst error against 16,384 nodes is 0.12%, at a
/// 10 K surface and a 0.2-day period, and smaller over the rest of 10–270 K and 0.2–100 d; 64 nodes
/// err by up to 7% in `S_I` at a 10 K surface and short periods (3% from 30 K), where the
/// dissipating layer is thin.
pub const SHELL_QUADRATURE_NODES: u32 = 512;

/// An ice shell's complex depth-mean rigidity μ̄ = `ICE_RIGIDITY` × (`storage` + i `loss`), as
/// shares of [`ICE_RIGIDITY`] ([`shell_modulus`]).
///
/// The fields are plain values, so they are public.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct ShellModulus {
    /// Re μ̄ ÷ `μ_ice`, `S_R`, 0 to 1.
    pub storage: f64,
    /// Im μ̄ ÷ `μ_ice`, `S_I`, 0 to ½.
    pub loss: f64,
}

impl ShellModulus {
    /// Its loss factor δ = `S_I` ÷ `S_R`, the shell's own (decision-p14-t56d-membrane §2.2); 0 for a
    /// shell of no storage.
    #[must_use]
    pub fn loss_factor(&self) -> f64 {
        if self.storage > 0.0 {
            self.loss / self.storage
        } else {
            0.0
        }
    }
}

/// The complex depth-mean rigidity of an ice shell whose top is at `surface` and whose base is at
/// [`ICE_MELTING_POINT`], conducting its heat, tidally flexed at the mean motion
/// `mean_motion_rad_per_s`, positive and finite (decision-p14-t56d-membrane §2.2).
///
/// The shell is Beuthe 2019's (Icarus 332, 66, Tables 1 and 2) Arrhenius Maxwell ice at 10¹⁴ Pa s: of
/// rigidity [`ICE_RIGIDITY`] and viscosity η(T) = `η_m` exp((`E_a` ÷ R)(1 ÷ T − 1 ÷ `T_m`))
/// ([`ICE_MELTING_VISCOSITY_PA_S`], [`ICE_CREEP_ACTIVATION_ENERGY_J_PER_MOL`],
/// [`MOLAR_GAS_CONSTANT`]). Its complex rigidity at each depth is μ ÷ (1 − i m), with Maxwell's
/// loss tangent m = μ ÷ (n η(T)), so `S_R` = ⟨1 ÷ (1 + m²)⟩ and `S_I` = ⟨m ÷ (1 + m²)⟩. Under a
/// conductivity of 651 ÷ T the conductive profile is uniform in ln T with depth
/// (dz = (651 ÷ F) dT ÷ T), so the mean ⟨·⟩ is over ln T from ln `T_s` to ln `T_m`, whatever the
/// heat flow: [`SHELL_QUADRATURE_NODES`] midpoint nodes. m is computed as (μ ÷ n `η_m`) ×
/// exp(−(`E_a` ÷ R)(1 ÷ T − 1 ÷ `T_m`)), so the cold end underflows to the elastic limit and never
/// overflows. Its loss factor rises with the period and with `T_s`, as a Maxwell shell's warm base
/// does: Europa's 0.0395 (`S_R` 0.974), Mimas's 0.011. Its limits (decision-p14-t56d-membrane
/// §2.2, as corrected by its §7.3): as `T_s` nears `T_m` the profile shrinks to its base, so `S_R`
/// → 1 ÷ (1 + m₀²) and δ → m₀, with m₀ = μ ÷ (n `η_m`), 1.71 at Europa's period (`S_R` 0.255);
/// `S_R` → 1 and `S_I` → 0 as m₀ → 0, a fast tide or stiff ice; at or above `T_m` no shell stands,
/// and both shares are 0.
///
/// Maxwell is Beuthe's (2015, Icarus 248, 109, §7.4.3) example rheology; Andrade, grain size and
/// convection are P14.T55.j's and T55.k's.
///
/// # Examples
///
/// Europa's shell under a 102 K surface at its 3.55-day period:
///
/// ```
/// use hyperion_sim::planetary::moons::tides::shell_modulus;
/// use hyperion_sim::units::Kelvin;
///
/// let n = core::f64::consts::TAU / (3.551_181 * 86_400.0);
/// let shell = shell_modulus(Kelvin::new(102.0), n);
/// assert!((shell.loss_factor() - 0.0395).abs() < 0.001);
/// assert!((shell.storage - 0.974).abs() < 0.003);
/// ```
#[must_use]
pub fn shell_modulus(surface: Kelvin, mean_motion_rad_per_s: f64) -> ShellModulus {
    shell_modulus_with(
        surface,
        mean_motion_rad_per_s,
        IceRheology::SHELL,
        SHELL_QUADRATURE_NODES,
    )
}

/// The Maxwell ice a shell modulus reads: its viscosity at the melting point, Pa s, and its creep's
/// activation energy, J mol⁻¹ (crate-private, so that a benchmark can read Beuthe 2019's own).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IceRheology {
    /// `η_m`, Pa s.
    pub(crate) melting_viscosity_pa_s: f64,
    /// `E_a`, J mol⁻¹.
    pub(crate) activation_j_per_mol: f64,
}

impl IceRheology {
    /// The shell's ice ([`ICE_MELTING_VISCOSITY_PA_S`], [`ICE_CREEP_ACTIVATION_ENERGY_J_PER_MOL`]).
    pub(crate) const SHELL: Self = Self {
        melting_viscosity_pa_s: ICE_MELTING_VISCOSITY_PA_S,
        activation_j_per_mol: ICE_CREEP_ACTIVATION_ENERGY_J_PER_MOL,
    };
}

/// [`shell_modulus`] for the ice `rheology` over `nodes` midpoint nodes, for its benchmark and its
/// convergence test.
#[must_use]
pub(crate) fn shell_modulus_with(
    surface: Kelvin,
    mean_motion_rad_per_s: f64,
    rheology: IceRheology,
    nodes: u32,
) -> ShellModulus {
    let melting = ICE_MELTING_POINT.value();
    // As the ocean flag reads it, which holds the surface at 1 K at the least.
    let t_s = surface.value().max(1.0);
    if t_s >= melting || nodes == 0 {
        return ShellModulus::default();
    }
    let (lo, hi) = (math::ln(t_s), math::ln(melting));
    let step = (hi - lo) / f64::from(nodes);
    let at_melting =
        ICE_RIGIDITY.value() / (mean_motion_rad_per_s * rheology.melting_viscosity_pa_s);
    let arrhenius = rheology.activation_j_per_mol / MOLAR_GAS_CONSTANT;
    let (mut storage, mut loss) = (0.0, 0.0);
    for k in 0..nodes {
        let t = math::exp(lo + (f64::from(k) + 0.5) * step);
        let m = at_melting * math::exp(-arrhenius * (1.0 / t - 1.0 / melting));
        let share = 1.0 / (1.0 + m * m);
        storage += share;
        loss += m * share;
    }
    let count = f64::from(nodes);
    ShellModulus {
        storage: storage / count,
        loss: loss / count,
    }
}

/// [`SHELL_QUADRATURE_NODES`] as an index bound: the nodes of a [`ShellProfile`].
const PROFILE_NODES: usize = 512;

/// An ice shell's Maxwell rigidity averaged from its top to any base temperature (P14.T51.g.b): the
/// depth mean over its conductive profile, by [`PROFILE_NODES`] midpoint nodes in ln T from its
/// top (at least 1 K, as [`shell_modulus`] holds it) to water's triple temperature, each weighted
/// by its depth, k dT = C min(1, T ÷ 30 K) d ln T under the conductivity's floor
/// ([`ICE_CONDUCTIVITY_FLOOR`]), summed once into running totals that a base temperature reads by
/// its place among them, its last node in proportion. A base at 273 K and a top above 30 K give
/// [`shell_modulus`]'s mean to its nodes' spacing; η is still referenced to 273 K
/// (decision-p14-t56d-membrane §5 (5)).
#[derive(Debug, Clone, PartialEq)]
struct ShellProfile {
    /// ln of the first node's lower edge, the top's temperature.
    lo: f64,
    /// The nodes' spacing in ln T.
    step: f64,
    /// The running totals of the weights, of the weighted storage shares and of the weighted loss
    /// shares, before each node and after the last.
    totals: [[f64; 3]; PROFILE_NODES + 1],
}

impl ShellProfile {
    /// The profile of a shell whose top is at `surface`, flexed at the mean motion
    /// `mean_motion_rad_per_s`, up to the temperature `top`.
    #[must_use]
    fn new(surface: Kelvin, mean_motion_rad_per_s: f64, top: Kelvin) -> Self {
        let rheology = IceRheology::SHELL;
        let melting = ICE_MELTING_POINT.value();
        let t_s = surface.value();
        let lo = math::ln(if t_s > 1.0 { t_s } else { 1.0 });
        let step = (math::ln(top.value()) - lo) / f64::from(SHELL_QUADRATURE_NODES);
        let at_melting =
            ICE_RIGIDITY.value() / (mean_motion_rad_per_s * rheology.melting_viscosity_pa_s);
        let arrhenius = rheology.activation_j_per_mol / MOLAR_GAS_CONSTANT;
        let floor = ICE_CONDUCTIVITY_FLOOR.value();
        let mut totals = [[0.0; 3]; PROFILE_NODES + 1];
        let mut sum = [0.0; 3];
        for (k, node) in (0..SHELL_QUADRATURE_NODES).zip(1..=PROFILE_NODES) {
            let t = math::exp(lo + (f64::from(k) + 0.5) * step);
            let m = at_melting * math::exp(-arrhenius * (1.0 / t - 1.0 / melting));
            let share = 1.0 / (1.0 + m * m);
            let weight = if t < floor { t / floor } else { 1.0 };
            sum[0] += weight;
            sum[1] += weight * share;
            sum[2] += weight * m * share;
            totals[node] = sum;
        }
        Self { lo, step, totals }
    }

    /// The shell's depth-mean rigidity from its top to a base at `base`, as shares of
    /// [`ICE_RIGIDITY`]: the first node's alone for a base at or under the top, all of them for one
    /// at or above the profile's end.
    #[must_use]
    fn to(&self, base: Kelvin) -> ShellModulus {
        let t_b = base.value();
        let place = (math::ln(if t_b > 1.0 { t_b } else { 1.0 }) - self.lo) / self.step;
        let end = f64::from(SHELL_QUADRATURE_NODES);
        let total = if place > 0.0 {
            let place = if place < end { place } else { end };
            let whole = place.floor();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "`whole` is a whole number in [0, 512], which a usize holds on every target"
            )]
            let node = whole as usize;
            let part = place - whole;
            let before = self.totals[node];
            match self.totals.get(node + 1) {
                Some(after) => [0, 1, 2].map(|i| before[i] + part * (after[i] - before[i])),
                None => before,
            }
        } else {
            self.totals[1]
        };
        ShellModulus {
            storage: total[1] / total[0],
            loss: total[2] / total[0],
        }
    }
}

/// An ice shell's stiffness against its ocean's gravity times the heat flow it conducts,
/// K = ΛF, complex, W m⁻² ([`shell_stiffness`]): a constant of the moon, so that the membrane's
/// Λ = K ÷ F at a heat flow F ([`at`](Self::at)).
///
/// The fields are plain values, so they are public.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct ShellStiffness {
    /// `K_R`, W m⁻².
    pub real: WattsPerSquareMetre,
    /// `K_I`, W m⁻², not negative.
    pub imaginary: WattsPerSquareMetre,
}

impl ShellStiffness {
    /// The membrane stiffness Λ = K ÷ F of the shell at the heat flow `heat_flow`, positive and
    /// finite for a positive, finite heat flow.
    #[must_use]
    pub fn at(&self, heat_flow: WattsPerSquareMetre) -> MembraneStiffness {
        let f = heat_flow.value();
        MembraneStiffness {
            real: self.real.value() / f,
            imaginary: self.imaginary.value() / f,
        }
    }
}

/// The stiffness K = ΛF of an ice shell of complex depth-mean rigidity `modulus` and Poisson's
/// ratio `poisson` over the ocean of a moon of surface gravity `gravity` and radius `radius`,
/// whose surface is at `surface` (decision-p14-t56d-membrane §2.1).
///
/// Λ = 8(1 + ν) ÷ (5 + ν) × μ̄ d ÷ (`ρ_w` g R²), the degree-2 membrane stiffness of a thin shell
/// (Beuthe 2015, Icarus 248, 109, eq. 28, with ρ "a reference density taken to be the ocean (or
/// crust) density", his §4.3: [`WATER_DENSITY`] here; the coefficient is Turcotte, Willemann,
/// Haxby and Norberry 1981, JGR 86, 3951's membrane term E d (λ − 2) ÷ (R²(λ − 1 + ν)) at
/// λ = n(n + 1) = 6, E = 2μ(1 + ν)). At its elastic limit it is Beuthe's "Λ ∼ 3.4 d/R" for Europa
/// and "∼ 240 d/R" for Enceladus. The shell conducts F from its melting point to its surface,
/// d = 651 ln(273 K ÷ `T_s`) ÷ F ([`ICE_CONDUCTIVITY_COEFFICIENT`], [`ICE_MELTING_POINT`]), so
/// ΛF is a constant of the moon. Zero at or above the melting point, where no shell stands.
///
/// # Examples
///
/// Europa's ocean branch, step by step: its shell's modulus at its period and surface, its
/// stiffness, its hot root, and its Love number there (decision-p14-t56d-membrane §3):
///
/// ```
/// use hyperion_sim::planetary::moons::tides::{
///     membrane_love_number, ocean_heat_flow, shell_modulus, shell_stiffness,
/// };
/// use hyperion_sim::planetary::params::ICE_POISSON_RATIO;
/// use hyperion_sim::units::{Kelvin, Metres, MetresPerSecondSquared, WattsPerSquareMetre};
///
/// let surface = Kelvin::new(102.0);
/// let n = core::f64::consts::TAU / (3.551_181 * 86_400.0);
/// let modulus = shell_modulus(surface, n);
/// let gravity = MetresPerSecondSquared::new(1.3147);
/// let shell = shell_stiffness(modulus, ICE_POISSON_RATIO, surface, gravity, Metres::new(1.5608e6));
/// assert!((shell.real.value() - 1.3620e-3).abs() < 1e-6);
/// let watts = WattsPerSquareMetre::new;
/// let f = ocean_heat_flow(watts(0.0082), watts(15.2), 0.249, shell);
/// let k2 = membrane_love_number(0.249, shell.at(f));
/// assert!((f.value() - 0.0193).abs() < 2e-4 && (k2.modulus() - 0.2289).abs() < 1e-3);
/// ```
#[must_use]
pub fn shell_stiffness(
    modulus: ShellModulus,
    poisson: f64,
    surface: Kelvin,
    gravity: MetresPerSecondSquared,
    radius: Metres,
) -> ShellStiffness {
    let t_s = surface.value();
    if t_s >= ICE_MELTING_POINT.value() {
        return ShellStiffness::default();
    }
    // As the ocean flag reads it, which holds the surface at 1 K at the least.
    let conducted =
        ICE_CONDUCTIVITY_COEFFICIENT * math::ln(ICE_MELTING_POINT.value() / t_s.max(1.0));
    let r = radius.value();
    let scale = 8.0 * (1.0 + poisson) / (5.0 + poisson) * ICE_RIGIDITY.value() * conducted
        / (WATER_DENSITY.value() * gravity.value() * r * r);
    ShellStiffness {
        real: WattsPerSquareMetre::new(scale * modulus.storage),
        imaginary: WattsPerSquareMetre::new(scale * modulus.loss),
    }
}

/// The tidal forcing Φ of a planet of mass `planet_mass` on a synchronous moon of radius
/// `radius` on an orbit of semi-major axis `a`, eccentricity `e` and mean motion
/// `mean_motion_rad_per_s`, W m⁻²: the eccentricity tide's heat flux per unit of Im k₂,
/// (21 ÷ 2) G `M_p`² R⁵ n e² ÷ (a⁶ 4πR²) ([`tidal_heating`] at k₂ ÷ Q = 1, over the surface).
#[must_use]
pub fn tidal_forcing(
    planet_mass: Kilograms,
    radius: Metres,
    a: Metres,
    e: f64,
    mean_motion_rad_per_s: f64,
) -> WattsPerSquareMetre {
    let r = radius.value();
    let heat = tidal_heating(planet_mass, radius, 1.0, a, e, mean_motion_rad_per_s);
    WattsPerSquareMetre::new(heat.value() / (4.0 * core::f64::consts::PI * r * r))
}

/// The mean density of a body of mass `mass` and radius `radius`, M ÷ (4π R³ ÷ 3), as a moon's
/// derivation reads it, so that its tides and its derivation take one density to the bit.
#[must_use]
pub(crate) fn mean_density(mass: Kilograms, radius: Metres) -> KilogramsPerCubicMetre {
    let volume = 4.0 / 3.0 * core::f64::consts::PI * math::powi(radius.value(), 3);
    KilogramsPerCubicMetre::new(mass.value() / volume)
}

/// The stiff-shell gain X = Φ `k_oc` `κ_I` ÷ (`κ_R`² + `κ_I`²) of an ocean world of forcing `forcing`
/// (Φ), ocean limit `ocean` (`k_oc`) and shell stiffness `stiffness` (κ = h°K), dimensionless
/// (decision-p14-t56d-membrane §2.1): where the shell is stiff (h°|Λ| ≫ 1) the hot root is
/// `F_nt` ÷ (1 − X), and from X = 1 it takes the thin-shell branch. Infinite for a shell of no
/// stiffness. Only a stiff shell's gain means this: a soft shell's (h°|Λ| ≪ 1 at its heat flow, a
/// large moon's) can be large while its root stays near the small-Λ one.
///
/// # Examples
///
/// Enceladus at its measured elements: its stiff shell is far from the edge, so it stays cold
/// (decision-p14-t56d-membrane §2.4):
///
/// ```
/// use hyperion_sim::planetary::moons::tides::{ShellStiffness, stiff_shell_gain};
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let watts = WattsPerSquareMetre::new;
/// let shell = ShellStiffness { real: watts(0.8107), imaginary: watts(0.013_51) };
/// let gain = stiff_shell_gain(watts(1.861), 0.594, shell);
/// assert!((gain - 0.0143).abs() < 5e-4);
/// ```
#[must_use]
pub fn stiff_shell_gain(
    forcing: WattsPerSquareMetre,
    ocean: f64,
    stiffness: ShellStiffness,
) -> f64 {
    let shell = 1.0 + ocean;
    let (k_r, k_i) = (
        shell * stiffness.real.value(),
        shell * stiffness.imaginary.value(),
    );
    let damped = k_r * k_r + k_i * k_i;
    if damped > 0.0 {
        forcing.value() * ocean * k_i / damped
    } else {
        f64::INFINITY
    }
}

/// The heat flow of an ocean world in its hot thermal-orbital equilibrium, `F_ocean`, W m⁻²: the
/// largest stable root of F = `F_nt` + Φ Im k₂(Λ = K ÷ F), with Im k₂ the membrane's loss
/// ([`membrane_love_number`]) at the ocean's limit `ocean`, the non-tidal heat flow `non_tidal`
/// (`F_nt`), the forcing `forcing` (Φ, [`tidal_forcing`]) and the shell's stiffness `stiffness`
/// (K, [`shell_stiffness`]) (decision-p14-t56d-membrane §2.1).
///
/// With κ = h°K, h° = 1 + `k_oc`, F is a root of the cubic (F − `F_nt`)((F + `κ_R`)² + `κ_I`²) =
/// A F, with A = Φ `k_oc` `κ_I`. With `F_nt` > 0 it has one root above zero (the tidal term's slope
/// is at most X at F = 0 and falls to |κ|), which the bracket finds as its largest: the cubic
/// rises past it, so it is stable. It lies between `F_nt` and the small-Λ
/// root (`F_nt` + √(`F_nt`² + 4A)) ÷ 2, which bounds it from above, since the cubic is positive
/// there and beyond; past the cubic's larger turning point where that is negative, and before its
/// smaller one otherwise, so that one sign change is bisected [`ROOT_STEPS`] times.
///
/// For a soft shell, a large moon's, the root is near the small-Λ root. For a stiff shell, a small
/// moon's, it is `F_nt` ÷ (1 − X) ([`stiff_shell_gain`]), and past X = 1 its root moves
/// continuously onto the thin-shell branch, the only equilibrium at a held eccentricity
/// (decision-p14-t56d-membrane §2.5 and §7.1; the eccentricity's damping and a resonance's budget
/// are P14.T55.j's). `F_nt` at no forcing.
///
/// # Examples
///
/// Europa at its measured elements, with some 8 mW m⁻² from its rock, reaches about 19 mW m⁻²
/// (decision-p14-t56d-membrane §3):
///
/// ```
/// use hyperion_sim::planetary::moons::tides::{ShellStiffness, ocean_heat_flow};
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let watts = WattsPerSquareMetre::new;
/// let shell = ShellStiffness { real: watts(1.3620e-3), imaginary: watts(5.3825e-5) };
/// let f = ocean_heat_flow(watts(0.0082), watts(15.2), 0.249, shell);
/// assert!((f.value() - 0.019_323).abs() < 2e-4, "{f:?}");
/// // Under the small-Λ root.
/// assert!(f.value() < 0.020_570);
/// ```
#[must_use]
pub fn ocean_heat_flow(
    non_tidal: WattsPerSquareMetre,
    forcing: WattsPerSquareMetre,
    ocean: f64,
    stiffness: ShellStiffness,
) -> WattsPerSquareMetre {
    let shell = 1.0 + ocean;
    let f_nt = non_tidal.value();
    let (k_r, k_i) = (
        shell * stiffness.real.value(),
        shell * stiffness.imaginary.value(),
    );
    let drive = forcing.value() * ocean * k_i;
    let damped = k_r * k_r + k_i * k_i;
    let cubic = |f: f64| (f - f_nt) * ((f + k_r) * (f + k_r) + k_i * k_i) - drive * f;
    let quadratic = f64::midpoint(f_nt, (f_nt * f_nt + 4.0 * drive).sqrt());
    let (mut lo, mut hi) = (f_nt, quadratic);
    // The turning points of the cubic F³ + (2κ_R − F_nt) F² + (|κ|² − 2κ_R F_nt − A) F − ⋯.
    let squared = 2.0 * k_r - f_nt;
    let linear = damped - 2.0 * k_r * f_nt - drive;
    let discriminant = squared * squared - 3.0 * linear;
    if discriminant > 0.0 {
        let root = discriminant.sqrt();
        let (smaller, larger) = ((-squared - root) / 3.0, (-squared + root) / 3.0);
        if larger > lo {
            if cubic(larger) < 0.0 {
                lo = larger;
            } else if smaller > lo {
                hi = hi.min(smaller);
            }
        }
    }
    for _ in 0..ROOT_STEPS {
        let mid = f64::midpoint(lo, hi);
        if cubic(mid) > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    WattsPerSquareMetre::new(f64::midpoint(lo, hi))
}

/// The points by which [`ShellBranch::largest_root_cell`] scans T* for the branch's largest root:
/// 48, geometric in 273.16 K − T* over [`ROOT_SCAN_DECADES`] decades below the deepest base's,
/// about 0.75 a step, so that a shell's depth, nearly proportional to its base's pressure and so to
/// 273.16 K − T* near the surface, is scanned evenly in its logarithm from the deepest base its
/// water allows to some 10⁻⁶ of it.
pub(crate) const ROOT_SCAN_POINTS: u32 = 48;

/// The decades of 273.16 K − T* that [`ROOT_SCAN_POINTS`] span: 6.
pub(crate) const ROOT_SCAN_DECADES: f64 = 6.0;

/// The ocean branch of a moon on the solve's spherical shell (P14.T51.g.b; [`MoonTides`]'s
/// `on_its_shell`): its water, its shell's profile, the membrane's scale, its ocean's limit, and
/// its non-tidal heat and forcing.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShellBranch {
    /// Its water over its rock, under its shell's top.
    pub(crate) water: Hydrosphere,
    profile: ShellProfile,
    /// 8(1 + ν) ÷ (5 + ν) `μ_ice` ÷ (`ρ_w` g R²), m⁻¹: Λ ÷ (μ̄ ÷ `μ_ice`) d.
    membrane_per_m: f64,
    /// `k_oc`.
    ocean: f64,
    /// Its porous core's tide (P14.T56.e): `None` for a rigid mantle, a compacted core.
    core: Option<PorousCore>,
    /// `F_nt`.
    non_tidal: WattsPerSquareMetre,
    /// Φ.
    forcing: WattsPerSquareMetre,
}

/// A porous core's tide under its moon's ocean ([`ShellBranch`]'s, P14.T56.e): the core term s
/// ([`core_love_number`]) and the moon's mean density, which [`deformable_membrane_love_number`]
/// reads.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PorousCore {
    /// s.
    term: ComplexLoveNumber,
    /// ρ̄.
    density: KilogramsPerCubicMetre,
}

/// One state of a moon's ocean branch, at a base's pure-water melting point T*
/// ([`ShellBranch::state`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BranchState {
    /// The shell's base.
    pub(crate) base: ShellBase,
    /// The membrane's complex Love number over the shell to it.
    pub(crate) love: ComplexLoveNumber,
    /// F(T*) − `F_nt` − Φ Im k₂, W m⁻²: what the shell conducts beyond the heat the moon makes.
    pub(crate) excess_w_per_m2: f64,
}

/// One state of a moon's ocean branch with no forcing read ([`ShellBranch::shell_at`]): its base
/// and its membrane's Love number, which a [`BranchState`] holds beside its excess, the one term
/// that reads Φ ([`ShellBranch::state_of`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct ShellAt {
    /// The shell's base.
    base: ShellBase,
    /// The membrane's complex Love number over the shell to it.
    love: ComplexLoveNumber,
}

impl ShellBranch {
    /// The branch of the moon `inputs` describe, of mean density `density` and surface gravity
    /// `gravity`, dissolving `solutes`: `None` without water, with an ocean limit that is not
    /// positive, or under a surface at or above water's triple point. The rock (P14.T56.e)
    /// under its water ([`OceanCore::under`] the column's base) is porous where it is not compacted
    /// ([`OceanCore::compaction`]), and its membrane then floats on it.
    #[must_use]
    pub(crate) fn of(
        inputs: &TideInputs,
        solutes: &OceanSolutes,
        density: KilogramsPerCubicMetre,
        gravity: MetresPerSecondSquared,
    ) -> Option<Self> {
        let ocean = ocean_love_number(density);
        if inputs.water <= 0.0 || !(ocean.is_finite() && ocean > 0.0) {
            return None;
        }
        let water = Hydrosphere::new(
            inputs.radius,
            inputs.mass,
            inputs.water,
            inputs.surface_temperature,
            *solutes,
        )?;
        let profile = ShellProfile::new(
            inputs.surface_temperature,
            inputs.mean_motion_rad_per_s,
            water.top(),
        );
        let r = inputs.radius.value();
        let membrane = 8.0 * (1.0 + ICE_POISSON_RATIO) / (5.0 + ICE_POISSON_RATIO)
            * ICE_RIGIDITY.value()
            / (WATER_DENSITY.value() * gravity.value() * r * r);
        let core = OceanCore::under(inputs.radius, inputs.mass, water.column().base())
            .filter(|core| core.compaction() == CoreCompaction::Porous)
            .map(|core| PorousCore {
                term: core_love_number(
                    core,
                    ComplexRigidity::POROUS_CORE,
                    density,
                    gravity,
                    inputs.radius,
                ),
                density,
            });
        Some(Self {
            water,
            profile,
            membrane_per_m: membrane,
            ocean,
            core,
            non_tidal: inputs.non_tidal_heat_flow,
            forcing: inputs.forcing,
        })
    }

    /// The branch's state at the base whose pure-water melting point is `melting_point_k`, K, on
    /// [T*₀, 273.16 K]: the base, the membrane's k₂ over the shell d = R − `r_b` at its modulus to
    /// the base's temperature, Λ = scale × μ̄ ÷ `μ_ice` × d, and what the shell conducts beyond the
    /// moon's heat. The membrane is Beuthe 2015's thin shell, his §1's "less than 5 to 10%" of the
    /// radius, and the depth mean weights each node by k dT, a flat shell's dz = k dT ÷ q, which
    /// over-weights a thick spherical shell's base by (R ÷ `r_b`)² (recorded). Over a porous core
    /// (P14.T56.e) the membrane floats on it ([`deformable_membrane_love_number`]), and its loss
    /// is the core's and the shell's.
    ///
    /// # Panics
    ///
    /// For a melting point outside [251.165 K, 273.16 K], as [`Hydrosphere::base_at`]: the
    /// branch reads it only on [T*₀, 273.16 K].
    #[must_use]
    pub(crate) fn state(&self, melting_point_k: f64) -> BranchState {
        self.state_of(self.shell_at(melting_point_k), self.forcing)
    }

    /// The branch's [`state`](Self::state) at the base whose pure-water melting point is
    /// `melting_point_k`, K, but for its excess: the base and the membrane's k₂, which read no
    /// forcing.
    ///
    /// # Panics
    ///
    /// As [`state`](Self::state).
    #[must_use]
    fn shell_at(&self, melting_point_k: f64) -> ShellAt {
        self.shell_on(self.water.base_at(melting_point_k))
    }

    /// The branch's [`shell_at`](Self::shell_at) the base `base` of its water
    /// ([`Hydrosphere::base_at`]): its membrane's k₂ over the shell to it.
    #[must_use]
    fn shell_on(&self, base: ShellBase) -> ShellAt {
        let modulus = self.profile.to(base.temperature);
        let d = base.shell(self.water.column().radius()).value();
        let lambda = MembraneStiffness {
            real: self.membrane_per_m * modulus.storage * d,
            imaginary: self.membrane_per_m * modulus.loss * d,
        };
        let love = match self.core {
            None => membrane_love_number(self.ocean, lambda),
            Some(core) => deformable_membrane_love_number(core.density, core.term, lambda),
        };
        ShellAt { base, love }
    }

    /// The branch of the same moon as `inputs` read it, which are this branch's but for their
    /// forcing and their surface temperature: [`of`](Self::of)'s at `inputs` to the bit, its water
    /// under their surface ([`Hydrosphere::under_surface`]) and its profile from it, the rest
    /// read no surface; `None` where that water is none.
    #[must_use]
    fn under(&self, inputs: &TideInputs) -> Option<Self> {
        let water = self.water.under_surface(inputs.surface_temperature)?;
        Some(Self {
            water,
            profile: ShellProfile::new(
                inputs.surface_temperature,
                inputs.mean_motion_rad_per_s,
                water.top(),
            ),
            non_tidal: inputs.non_tidal_heat_flow,
            forcing: inputs.forcing,
            ..*self
        })
    }

    /// The branch's state at the shell `shell` under the forcing `forcing`: what the shell conducts
    /// beyond the heat the moon makes then, F(T*) − `F_nt` − Φ Im k₂.
    #[must_use]
    fn state_of(&self, shell: ShellAt, forcing: WattsPerSquareMetre) -> BranchState {
        let ShellAt { base, love } = shell;
        let excess =
            base.heat_flow.value() - self.non_tidal.value() - forcing.value() * love.loss();
        BranchState {
            base,
            love,
            excess_w_per_m2: excess,
        }
    }

    /// The cell of T* that holds the branch's largest stable root, the warm start a moon formed
    /// hot in its planet's disc keeps (version 23's choice of root, [`ocean_heat_flow`]): scanning
    /// down from the top `top`, where the shell vanishes and conducts more than the moon makes, by
    /// [`ROOT_SCAN_POINTS`] points geometric in `top` − T* to the deepest base `deepest`, the first
    /// point whose shell conducts at most what the moon makes, with the point above it. A shell's
    /// tidal heat can peak at an intermediate depth, so the excess F(T*) − `F_nt` − Φ Im k₂ may
    /// change sign more than once; the root in this cell is the one with the largest heat flow,
    /// and stable (the excess rises through it as the shell thins). `None` where every point
    /// conducts more than the moon makes: a cold moon. Both ends of the cell and of the scan,
    /// `deepest_k` and `top_k`, are melting points, K, and `state` gives the branch's state at
    /// each point.
    ///
    /// # Panics
    ///
    /// For ends outside [251.165 K, 273.16 K], as [`state`](Self::state).
    #[must_use]
    fn largest_root_cell(
        deepest_k: f64,
        top_k: f64,
        mut state: impl FnMut(f64) -> BranchState,
    ) -> Option<(f64, f64)> {
        let (deepest, top) = (deepest_k, top_k);
        let span = top - deepest;
        let mut above = top;
        for point in 0..=ROOT_SCAN_POINTS {
            let t = if point == ROOT_SCAN_POINTS {
                deepest
            } else {
                let decades = ROOT_SCAN_DECADES * f64::from(ROOT_SCAN_POINTS - point)
                    / f64::from(ROOT_SCAN_POINTS);
                top - span * math::exp10(-decades)
            };
            if state(t).excess_w_per_m2 <= 0.0 {
                return Some((t, above));
            }
            above = t;
        }
        None
    }
}

/// A moon's tidal state, the branch its whole heat flow puts it on ([`MoonTides::of`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TidalBranch {
    /// Elastic and cold: its material's rigidity and cold Q.
    Cold,
    /// An ice shell over an ocean the whole heat flow keeps: the membrane's k₂.
    Ocean,
    /// A rocky interior tidally heated to partial melt: Io's state.
    Heated,
}

/// What a moon's solid tide is raised in, for its rigidity and cold Q: rock or ice
/// ([`MoonTides::of`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TidalMaterial {
    /// Rock: [`ROCK_RIGIDITY`] and [`COLD_ROCK_TIDAL_Q`] cold, or heated.
    Rock,
    /// Ice: [`ICE_RIGIDITY`] and [`COLD_ICE_TIDAL_Q`].
    Ice,
}

impl TidalMaterial {
    /// The material of a body whose water mass fraction is `water`: ice from
    /// [`ICY_WATER_FRACTION`], as its class is `Icy`
    /// ([`PlanetClass::of`](crate::planetary::derive::PlanetClass::of)), and rock below it.
    #[must_use]
    pub fn of_water(water: f64) -> Self {
        if water >= ICY_WATER_FRACTION {
            Self::Ice
        } else {
            Self::Rock
        }
    }
}

/// What a moon's tides read ([`MoonTides::of`]), as plain values.
///
/// The fields carry no invariant the rule relies on, so they are public: a mass or radius that is
/// not positive and finite gives a tide that is not finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TideInputs {
    /// Its material, by its water ([`TidalMaterial::of_water`]).
    pub material: TidalMaterial,
    /// Its moment of inertia C ÷ M R², its class's
    /// ([`moment_of_inertia_factor`](crate::planetary::derive::rotation::moment_of_inertia_factor)),
    /// so that its tides and its figure agree.
    pub moment_factor: f64,
    /// Its mass.
    pub mass: Kilograms,
    /// Its mean radius, its own bulk's.
    pub radius: Metres,
    /// Its water mass fraction, 0 to 1, whose layer an ocean needs.
    pub water: f64,
    /// The temperature at the top of its ice shell: its surface's, as its ocean flag reads it.
    pub surface_temperature: Kelvin,
    /// Its non-tidal heat flow `F_nt`, P14.T24.b's: the radiogenic term with its interior's
    /// residual ([`GlobalFigures`](crate::planetary::hooks::GlobalFigures)'s), with no tide.
    pub non_tidal_heat_flow: WattsPerSquareMetre,
    /// Its planet's tidal forcing Φ on it ([`tidal_forcing`]).
    pub forcing: WattsPerSquareMetre,
    /// Its mean motion about its planet, rad s⁻¹, the eccentricity tide's frequency for a
    /// synchronous moon, at which its ice shell flexes ([`shell_modulus`]).
    pub mean_motion_rad_per_s: f64,
}

/// A moon's tides by its composition and state (P14.T56.d): its Love number k₂ = |k₂|, its tidal
/// Q, its loss Im k₂ = k₂ ÷ Q, the branch that gives them and the whole heat flow F that branch
/// reads (see the [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonTides {
    k2: f64,
    q: f64,
    loss: f64,
    branch: TidalBranch,
    heat_flow: WattsPerSquareMetre,
}

impl MoonTides {
    /// The tides of the moon `inputs` describe (decision-p14-moons §3.2 and §F-ocean F3,
    /// decision-p14-t56d-membrane §2; the [module](self) documentation): the ocean branch where
    /// its water and its hot root keep an ocean, else the heated branch for a rocky moon whose
    /// `F_nt` and tidal flux at Io's Im k₂ reach `F_c`, else the cold branch.
    ///
    /// Reading `F_nt` alone, not the age, the branch is read at each time: an ocean that the
    /// falling radiogenic heat freezes is a recorded state change.
    ///
    /// From P14.T51.g.b's bump at generator version 24 the ocean branch is the spherical shell's
    /// over fresh water, its base at ice Ih's melting point at its pressure; the generator's moons
    /// dissolve their own chlorine and ammonia, which this public form does not take.
    ///
    /// # Examples
    ///
    /// A cold Moon-like moon of Earth's: elastic rock at Q = 38.
    ///
    /// ```
    /// use hyperion_sim::planetary::moons::tides::{MoonTides, TidalBranch, TidalMaterial, TideInputs};
    /// use hyperion_sim::units::{Kelvin, Kilograms, Metres, WattsPerSquareMetre};
    ///
    /// let moon = MoonTides::of(&TideInputs {
    ///     material: TidalMaterial::Rock,
    ///     moment_factor: 0.33,
    ///     mass: Kilograms::new(7.346e22),
    ///     radius: Metres::new(1.7374e6),
    ///     water: 0.0,
    ///     surface_temperature: Kelvin::new(250.0),
    ///     non_tidal_heat_flow: WattsPerSquareMetre::new(0.012),
    ///     forcing: WattsPerSquareMetre::new(0.026),
    ///     mean_motion_rad_per_s: 2.66e-6,
    /// });
    /// assert_eq!(moon.branch(), TidalBranch::Cold);
    /// assert!((moon.q() - 38.0).abs() < 1e-12);
    /// assert!((0.01..0.02).contains(&moon.k2()));
    /// ```
    #[must_use]
    pub fn of(inputs: &TideInputs) -> Self {
        Self::with_ocean(inputs, &OceanSolutes::FRESH).0
    }

    /// The tides of the moon `inputs` describe, whose ocean would dissolve `solutes`, with the ocean
    /// its branch keeps (crate-private; [`of`](Self::of) reads it at fresh water): the ocean branch
    /// on the solve's spherical shell ([`on_its_shell`](Self::on_its_shell), P14.T51.g.b), with the
    /// solve's ocean at the branch's root, and [`Subsurface::NoOcean`] off it.
    #[must_use]
    pub(crate) fn with_ocean(inputs: &TideInputs, solutes: &OceanSolutes) -> (Self, Subsurface) {
        let (density, gravity) = Self::bulk(inputs);
        Self::on_its_shell(inputs, solutes, density, gravity).unwrap_or_else(|| {
            (
                Self::off_an_ocean(inputs, density, gravity),
                Subsurface::NoOcean,
            )
        })
    }

    /// The tides of a moon `inputs` describe that takes no ocean branch, whatever its water and
    /// heat: one whose heated shell is a rock lag (P14.T56.g's `SurfaceLag::Lagged`), which holds
    /// no ice to float on an ocean (decision-p14-t56g §1; decision-p14-t51g Q4). Its branch is the
    /// heated or the cold one, as [`of`](Self::of) gives a moon that fails the ocean test.
    #[must_use]
    pub(crate) fn without_an_ocean(inputs: &TideInputs) -> Self {
        let (density, gravity) = Self::bulk(inputs);
        Self::off_an_ocean(inputs, density, gravity)
    }

    /// The mean density and surface gravity of the moon `inputs` describe.
    #[must_use]
    fn bulk(inputs: &TideInputs) -> (KilogramsPerCubicMetre, MetresPerSecondSquared) {
        let (m, r) = (inputs.mass.value(), inputs.radius.value());
        (
            mean_density(inputs.mass, inputs.radius),
            MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r)),
        )
    }

    /// The heated or the cold branch's tides of the moon of `inputs`, of mean density `density`
    /// and surface gravity `gravity`: the heated branch for a rocky moon whose `F_nt` and tidal
    /// flux at Io's Im k₂ reach `F_c`, else the cold branch.
    #[must_use]
    fn off_an_ocean(
        inputs: &TideInputs,
        density: KilogramsPerCubicMetre,
        gravity: MetresPerSecondSquared,
    ) -> Self {
        let fluid = fluid_love_number(inputs.moment_factor);
        let (f_nt, forcing) = (inputs.non_tidal_heat_flow.value(), inputs.forcing.value());
        let heated = f_nt + forcing * MOON_TIDAL_RESPONSE;
        if inputs.material == TidalMaterial::Rock && heated >= SURFACE_MELT_HEAT_FLOW.value() {
            let k2 =
                elastic_love_number(fluid, HEATED_ROCK_RIGIDITY, density, gravity, inputs.radius);
            return Self {
                k2,
                q: k2 / MOON_TIDAL_RESPONSE,
                loss: MOON_TIDAL_RESPONSE,
                branch: TidalBranch::Heated,
                heat_flow: WattsPerSquareMetre::new(heated),
            };
        }
        let (rigidity, q) = match inputs.material {
            TidalMaterial::Rock => (ROCK_RIGIDITY, COLD_ROCK_TIDAL_Q),
            TidalMaterial::Ice => (ICE_RIGIDITY, COLD_ICE_TIDAL_Q),
        };
        let k2 = elastic_love_number(fluid, rigidity, density, gravity, inputs.radius);
        let loss = k2 / q;
        Self {
            k2,
            q,
            loss,
            branch: TidalBranch::Cold,
            heat_flow: WattsPerSquareMetre::new(f_nt + forcing * loss),
        }
    }

    /// The ocean branch's tides on the solve's spherical shell (P14.T51.g.b; decision-p14-t51g's
    /// T56.d branch), with the ocean the branch keeps, if the moon of `inputs`, of mean density
    /// `density` and surface gravity `gravity`, dissolving `solutes`, holds an ocean at its root.
    ///
    /// Its water is its own bulk's under its shell's top at `inputs`' surface temperature (its
    /// equilibrium temperature, the passes'), over its rock ([`Hydrosphere`]). At each base, read by
    /// its pure-water melting point T* ([`Hydrosphere::base_at`]), the shell is d = R − `r_b` thick
    /// and conducts F(T*); its membrane's Λ = 8(1 + ν) ÷ (5 + ν) μ̄ d ÷ (`ρ_w` g R²), μ̄ its Maxwell
    /// rigidity averaged over its conductive profile from its top to the base's temperature
    /// ([`ShellProfile`]), with η still referenced to 273 K (decision-p14-t56d-membrane §5 (5)), and
    /// Λ and `k_oc` at [`WATER_DENSITY`] (decision-p14-t51g Q3). The branch's heat flow `F_ocean` is
    /// the largest stable root of F(T*) − `F_nt` − Φ Im k₂(Λ(T*)) on [T*₀, 273.16 K], as version
    /// 23's was: the excess is positive at the top, where the shell vanishes, a scan down from it
    /// finds the first cell where it is not ([`ShellBranch::largest_root_cell`]), and
    /// [`SHELL_BASE_HALVINGS`] halvings refine the root there. The excess may change sign more than
    /// once, since a stiff shell's tidal heat peaks at an intermediate depth. The branch then reads
    /// the presence test at `F_ocean`, which holds wherever the root lies on [T*₀, 273.16 K].
    /// `None` (the heated or cold branch) without water, under a surface at or above its water's
    /// liquidus, where the moon's heat keeps no ocean at any depth its water allows, and for a
    /// shell with no loss.
    #[must_use]
    fn on_its_shell(
        inputs: &TideInputs,
        solutes: &OceanSolutes,
        density: KilogramsPerCubicMetre,
        gravity: MetresPerSecondSquared,
    ) -> Option<(Self, Subsurface)> {
        let branch = ShellBranch::of(inputs, solutes, density, gravity)?;
        Self::at_its_root(&branch, inputs.surface_temperature, |t| branch.state(t))
    }

    /// [`on_its_shell`](Self::on_its_shell) on the branch `branch` under a shell whose top is at
    /// `surface`, its state at each melting point, K, read by `state`.
    #[must_use]
    fn at_its_root(
        branch: &ShellBranch,
        surface: Kelvin,
        mut state: impl FnMut(f64) -> BranchState,
    ) -> Option<(Self, Subsurface)> {
        let (deepest, top) = (branch.water.deepest().value(), branch.water.top().value());
        let top_state = state(top);
        // A surface at or above its whole water's liquidus holds no shell, and a root above the
        // top would be a shell thinner than its triple pressure's depth: no ocean branch.
        if top_state.base.temperature.value() <= surface.value() || top_state.excess_w_per_m2 <= 0.0
        {
            return None;
        }
        let (mut lo, mut hi) = ShellBranch::largest_root_cell(deepest, top, &mut state)?;
        // Each halving is a function of the bracket alone, so one that leaves it as it was, bit for
        // bit, leaves every one after it so too, and the rest are not taken, as in the column's
        // `radius_at`: a cell near the top closes to one ulp of T* before the 48th, and 13% of the
        // halvings of the 400 sample systems under every 27 hold moved nothing.
        for _ in 0..SHELL_BASE_HALVINGS {
            let mid = f64::midpoint(lo, hi);
            let (next_lo, next_hi) = if state(mid).excess_w_per_m2 > 0.0 {
                (lo, mid)
            } else {
                (mid, hi)
            };
            if next_lo.total_cmp(&lo).is_eq() && next_hi.total_cmp(&hi).is_eq() {
                break;
            }
            (lo, hi) = (next_lo, next_hi);
        }
        let root = state(f64::midpoint(lo, hi));
        // The branch reads the ocean's presence test at its whole heat flow, `F_ocean`, as the
        // plan's T56.d branch does (it holds wherever the root lies above T*₀).
        if !branch.water.holds_an_ocean(root.base.heat_flow) {
            return None;
        }
        let (modulus, loss) = (root.love.modulus(), root.love.loss());
        // A shell with no loss at all, which a finite viscosity never gives, despins nothing.
        if !(loss.is_finite() && loss > 0.0) {
            return None;
        }
        let tides = Self {
            k2: modulus,
            q: modulus / loss,
            loss,
            branch: TidalBranch::Ocean,
            heat_flow: root.base.heat_flow,
        };
        Some((tides, branch.water.publish(&root.base)))
    }

    /// Its Love number k₂ = |k₂|, dimensionless.
    #[must_use]
    pub const fn k2(&self) -> f64 {
        self.k2
    }

    /// Its tidal quality factor Q, dimensionless: the cold Q given, or k₂ ÷ Im k₂.
    #[must_use]
    pub const fn q(&self) -> f64 {
        self.q
    }

    /// Its loss Im k₂ = k₂ ÷ Q, dimensionless, which its tidal heat reads: exactly Io's
    /// [`MOON_TIDAL_RESPONSE`] on the heated branch.
    #[must_use]
    pub const fn loss(&self) -> f64 {
        self.loss
    }

    /// The branch its whole heat flow put it on.
    #[must_use]
    pub const fn branch(&self) -> TidalBranch {
        self.branch
    }

    /// The whole heat flow its branch reads, W m⁻²: `F_ocean` on the ocean branch, and `F_nt`
    /// plus its tidal flux, Φ Im k₂, otherwise.
    #[must_use]
    pub const fn heat_flow(&self) -> WattsPerSquareMetre {
        self.heat_flow
    }

    /// The pair its spin's locking reads ([`SpinTides::Given`]).
    #[must_use]
    pub const fn spin_tides(&self) -> SpinTides {
        SpinTides::Given {
            love_number: self.k2,
            tidal_q: self.q,
        }
    }
}

/// A moon's tides solved again and again, at one forcing and one surface temperature after
/// another, all else held: [`MoonTides::with_ocean`] at each to the bit, each base of its water
/// found once whatever the surface, and each state of its ocean branch under its own surface solved
/// once whatever the forcing.
///
/// A base's every member but its heat flow reads no surface ([`Hydrosphere::conducting`]), and a
/// state's every term but its excess no forcing ([`ShellBranch::state_of`]). Nearly all of a solve
/// is its bases, about 1.7 µs each, some 56 to a solve on the ocean branch, and most are another
/// solve's: P14.T55.j's balance solves a moon up to 12 times as its surface warms
/// ([`with_radiating_surface`]), and P14.T55.j.a's damping reads its heat at some twenty to eighty
/// eccentricities, each first at its own surface. Over the 400 sample systems under every 27 hold,
/// 56% of the bases solved on the ocean branch were found already within their tide's balance,
/// and the branch was built anew for each solve, at about 25 µs (pod, 16 vCPU, release).
#[derive(Debug, Clone)]
pub(crate) struct TideSweep {
    /// What its tides read, at the forcing and surface it was built at, its own.
    inputs: TideInputs,
    /// What its ocean would dissolve.
    solutes: OceanSolutes,
    /// Its mean density and surface gravity ([`MoonTides::bulk`]).
    density: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared,
    /// Its ocean branch under its own surface ([`ShellBranch::of`]), `None` where it has none.
    branch: Option<ShellBranch>,
    /// Each base of its water found so far, under its own surface, by its melting point, K.
    bases: BTreeMap<Exact, ShellBase>,
    /// Each state of its ocean branch under its own surface solved so far, by its base's melting
    /// point, K.
    shells: BTreeMap<Exact, ShellAt>,
}

impl TideSweep {
    /// The sweep of the moon `inputs` describe, whose ocean would dissolve `solutes`, at any
    /// forcing and surface (`inputs`' own forcing is not read).
    #[must_use]
    pub(crate) fn new(inputs: &TideInputs, solutes: &OceanSolutes) -> Self {
        let (density, gravity) = MoonTides::bulk(inputs);
        Self {
            inputs: *inputs,
            solutes: *solutes,
            density,
            gravity,
            branch: ShellBranch::of(inputs, solutes, density, gravity),
            bases: BTreeMap::new(),
            shells: BTreeMap::new(),
        }
    }

    /// The tides of its moon as `inputs` read it, with the ocean their branch keeps: its
    /// [`MoonTides::with_ocean`], bit for bit, for `inputs` that are its own but for their forcing
    /// and their surface temperature.
    #[must_use]
    pub(crate) fn with_ocean(&mut self, inputs: &TideInputs) -> (MoonTides, Subsurface) {
        let Self {
            solutes,
            density,
            gravity,
            branch,
            bases,
            shells,
            ..
        } = self;
        let Some(own) = branch.as_ref() else {
            return MoonTides::with_ocean(inputs, solutes);
        };
        let forcing = inputs.forcing;
        let surface = inputs.surface_temperature;
        let mut base = |t: f64| {
            *bases
                .entry(Exact(t))
                .or_insert_with(|| own.water.base_at(t))
        };
        let own_surface = surface
            .value()
            .total_cmp(&self.inputs.surface_temperature.value())
            .is_eq();
        let tides = if own_surface {
            MoonTides::at_its_root(own, surface, |t| {
                let shell = *shells
                    .entry(Exact(t))
                    .or_insert_with(|| own.shell_on(base(t)));
                own.state_of(shell, forcing)
            })
        } else {
            own.under(inputs).and_then(|branch| {
                MoonTides::at_its_root(&branch, surface, |t| {
                    let shell = branch.shell_on(branch.water.conducting(&base(t)));
                    branch.state_of(shell, forcing)
                })
            })
        };
        tides.unwrap_or_else(|| {
            (
                MoonTides::off_an_ocean(inputs, *density, *gravity),
                Subsurface::NoOcean,
            )
        })
    }
}

/// An `f64` ordered by [`f64::total_cmp`]: a memo's key, equal only to the same value bit for bit.
#[derive(Debug, Clone, Copy)]
struct Exact(f64);

impl PartialEq for Exact {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}

impl Eq for Exact {}

impl PartialOrd for Exact {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Exact {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[cfg(test)]
mod tests;
