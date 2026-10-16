//! A moon's tides by its composition and state: one complex Love number per moon (plan 14,
//! P14.T56.d; decision-p14-moons §3 and §F-ocean F3, decision-p14-t56d-membrane).
//!
//! # The convention
//!
//! A moon's tidal response is a complex Love number k₂ = |k₂| e^(−iε), whose modulus is the
//! Love number k₂ that despins and figures read, and whose loss −Im k₂ = |k₂| sin ε = k₂ ÷ Q, with
//! Q⁻¹ = sin ε, is what dissipates: "Im k₂" here is that positive loss. The eccentricity tide's
//! heat is (21 ÷ 2)(k₂ ÷ Q) G `M_p`² R⁵ n e² ÷ a⁶
//! ([`tidal_heating`](crate::planetary::moons::regular::tidal_heating)), so the tidal heat flux is
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
//!    ([`ocean_love_number`]), h° = 1 + `k_oc`, and the membrane's
//!    Λ = 8(1 + ν) ÷ (5 + ν) × μ̄ d ÷ (`ρ_w` g R²) over the ocean's density, d the conductive shell
//!    651 ln(273 K ÷ `T_s`) ÷ F (decision-p14-t56d-membrane, which corrects decision-p14-moons
//!    §F-ocean F2's ρ̄ and its missing h°). Λ is complex through the shell's Maxwell rigidity,
//!    Beuthe 2019's Arrhenius ice at 10¹⁴ Pa s, averaged over its conductive profile
//!    ([`shell_modulus`]), so its loss factor is the moon's own, from its period and surface
//!    temperature: no loss factor is fitted. `F_ocean` is the
//!    largest stable root of F = `F_nt` + Φ Im k₂(d(F)), the warm start a moon formed hot in its
//!    planet's disc keeps ([`ocean_heat_flow`]).
//! 2. **The heated branch** ([`TidalBranch::Heated`]), for a rocky moon if `F_nt` and its tidal
//!    flux at Io's Im k₂ = 0.015
//!    ([`MOON_TIDAL_RESPONSE`](crate::planetary::moons::regular::MOON_TIDAL_RESPONSE)) reach
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
//! by mass) may take it; an icy moon that fails it is cold. The full thermal-orbital equilibrium,
//! a Maxwell or Andrade body whose viscosity balances tidal heat against convective loss, would
//! give Im k₂ continuously in place of the branches: P14.T55.j. A porous rock core's dissipation,
//! which Enceladus needs, is P14.T56.e's: a conductive shell alone keeps no Enceladus ocean, as
//! the literature finds (decision-p14-t56d-membrane §2.4).
//!
//! Every function here is pure and deterministic: the shell's modulus is a fixed midpoint sum
//! ([`SHELL_QUADRATURE_NODES`]) and the root is refined by a fixed count of bisections
//! ([`ROOT_STEPS`]).

use crate::math;
use crate::planetary::derive::rotation::SpinTides;
use crate::planetary::moons::regular::{
    ICE_CONDUCTIVITY_COEFFICIENT, ICE_MELTING_POINT, MOON_TIDAL_RESPONSE, WATER_DENSITY,
    has_subsurface_ocean, tidal_heating,
};
use crate::planetary::params::{
    COLD_ICE_TIDAL_Q, COLD_ROCK_TIDAL_Q, HEATED_ROCK_RIGIDITY,
    ICE_CREEP_ACTIVATION_ENERGY_J_PER_MOL, ICE_MELTING_VISCOSITY_PA_S, ICE_POISSON_RATIO,
    ICE_RIGIDITY, ICY_WATER_FRACTION, ROCK_RIGIDITY, SURFACE_MELT_HEAT_FLOW,
};
use crate::substance::phase::MOLAR_GAS_CONSTANT;
use crate::units::consts::GRAVITATIONAL_CONSTANT;
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
        let (m, r) = (inputs.mass.value(), inputs.radius.value());
        let density = mean_density(inputs.mass, inputs.radius);
        let gravity = MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r));
        if let Some(ocean) = Self::on_an_ocean(inputs, density, gravity) {
            return ocean;
        }
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

    /// The ocean branch's tides, if the moon of `inputs`, of mean density `density` and surface
    /// gravity `gravity`, holds an ocean at its hot root.
    #[must_use]
    fn on_an_ocean(
        inputs: &TideInputs,
        density: KilogramsPerCubicMetre,
        gravity: MetresPerSecondSquared,
    ) -> Option<Self> {
        let ocean = ocean_love_number(density);
        if inputs.water <= 0.0 || !(ocean.is_finite() && ocean > 0.0) {
            return None;
        }
        let modulus = shell_modulus(inputs.surface_temperature, inputs.mean_motion_rad_per_s);
        let stiffness = shell_stiffness(
            modulus,
            ICE_POISSON_RATIO,
            inputs.surface_temperature,
            gravity,
            inputs.radius,
        );
        if stiffness.real.value() <= 0.0 {
            return None;
        }
        let heat_flow =
            ocean_heat_flow(inputs.non_tidal_heat_flow, inputs.forcing, ocean, stiffness);
        let holds = has_subsurface_ocean(
            inputs.radius,
            density,
            inputs.water,
            inputs.surface_temperature,
            heat_flow,
        );
        if !holds {
            return None;
        }
        let k2 = membrane_love_number(ocean, stiffness.at(heat_flow));
        let (modulus, loss) = (k2.modulus(), k2.loss());
        // A shell with no loss at all, which a finite viscosity never gives, despins nothing.
        if !(loss.is_finite() && loss > 0.0) {
            return None;
        }
        Some(Self {
            k2: modulus,
            q: modulus / loss,
            loss,
            branch: TidalBranch::Ocean,
            heat_flow,
        })
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

#[cfg(test)]
mod tests;
