//! The shell window: how long a supernova's shell stays distinct from the gas around it (P09.T15.a).
//!
//! A shell is distinct until its shock slows to `β c_net`, twice the signal speed of the gas
//! around it (brainstorm, "Supernova remnants": "the window is physics, not a number"), with
//! `c_net² = C₀² + σ²`, `C₀` the ambient's isothermal sound speed and `σ` = 8 km/s of turbulence
//! (Cioffi, McKee and Bertschinger 1988, ApJ 334, 252, p. 264, hereafter CMB88: the blast must
//! exceed "the ambient isothermal sound speed C₀ by a factor β of order unity", with β = 2 and a
//! "cloudy medium with a velocity dispersion of order 10 km s⁻¹"; ruling 98 of 2026-09-22).
//!
//! Two branches, joined continuously:
//!
//! - **Radiative.** When the blast is still faster than `β c_net` as it enters the pressure-driven
//!   snowplough at `t_PDS`, the window is the time CMB88's offset power law, eq. 3.32b, `v =
//!   v_PDS (4⁄3 t ÷ t_PDS − 1⁄3)^(−7⁄10)`, reaches `β c_net`: `W = t_PDS [¾ (v_PDS ÷ β
//!   c_net)^(10⁄7) + ¼]`, the exact inverse (their eq. 4.4a drops the ¼), with `t_PDS = 1.33 ×
//!   10⁴ yr E₅₁^(3⁄14) ζ^(−5⁄14) n^(−4⁄7)` (eqs. 3.10–3.11) and `v_PDS = 413 km/s n^(1⁄7)
//!   ζ^(3⁄14) E₅₁^(1⁄14)` (eq. 3.33b).
//! - **Hot.** When the blast has already slowed to `β c_net` before `t_PDS`, as it does in hot,
//!   thin gas, the shell merges while still adiabatic (CMB88 §IV, eq. 4.8, "the SNR will merge
//!   before entering the PDS stage").
//!
//! The adiabatic blast follows Tang and Wang (2005, ApJ 628, 205), whose fit to their simulations
//! of blasts in hot gas, eq. 2, `V_s = c_s (t_c ÷ t + 1)^(3⁄5)`, is `V_s^(5⁄3) = V_Sed^(5⁄3) +
//! c_s^(5⁄3)`: the Sedov speed with the ambient's adiabatic sound speed `c_s = √(5⁄3) C₀` added in
//! the 5⁄3 power (they add that the Sedov solution "is not valid even before `t = t_c`"). It is set on
//! CMB88's own Sedov clock, `V_Sed(t_PDS) = v_PDS` (CMB88, p. 264: `v_PDS = v_s(t_PDS) = 2 R_PDS ÷ 5
//! t_PDS`), so that `t_c = t_PDS (v_PDS ÷ c_s)^(5⁄3)` (ruling 136.1). With `v* = (v_PDS^(5⁄3) +
//! c_s^(5⁄3))^(3⁄5)`, the blast's speed at `t_PDS`:
//!
//! - radiative if `v* > β c_net`, with `v*` in place of `v_PDS` in the radiative form above;
//! - hot otherwise, `W = t_PDS v_PDS^(5⁄3) ÷ ((β c_net)^(5⁄3) − c_s^(5⁄3))`, which is Tang and
//!   Wang's `t_c ÷ ((β c_net ÷ c_s)^(5⁄3) − 1)`, 0.931 `t_c` in hot gas where the turbulence is
//!   negligible.
//!
//! Both give `t_PDS` where they meet, so the window is continuous in every argument, and since `β
//! c_net ≥ 2 C₀ = 1.549 c_s` it is always finite.
//!
//! The window never rises with the ambient's sound speed, so over a pressure field it is largest
//! at the lowest pressure, the floor, which is what [`WindowCaps`](super::WindowCaps) scans.
//!
//! β, σ, the site's smoothing scale and the explosion energy's law belong to the generator
//! version.

use std::error::Error;
use std::fmt;

use crate::coords::GalacticPosition;
use crate::galaxy::gas::MASS_PER_HYDROGEN_FACTOR;
use crate::galaxy::gas::field::{GasField, GasState};
use crate::galaxy::gas::noise::{NoiseCache, SmoothingScale};
use crate::math;
use crate::units::consts::{BOLTZMANN_CONSTANT, HYDROGEN_MASS_KG};
use crate::units::{
    Dex, HydrogenPerCm3, Kelvin, KelvinPerCm3, KilometresPerSecond, LightYears, MetresPerSecond,
    Years,
};

/// β, the multiple of the ambient's signal speed at which a shell merges with the gas: 2
/// (CMB88, p. 264). A parameter of the generator version.
pub const MERGE_FACTOR: f64 = 2.0;

/// σ, the ambient's turbulent velocity dispersion added in quadrature to its isothermal sound
/// speed: 8 km/s (CMB88, p. 264, "a cloudy medium with a velocity dispersion of order 10 km s⁻¹";
/// plan 07, P07.T12). A parameter of the generator version.
pub const TURBULENT_SPEED: KilometresPerSecond = KilometresPerSecond::new(8.0);

/// CMB88's `t_PDS` at 10⁵¹ erg, solar metallicity and 1 cm⁻³, years: `t_sf ÷ e` with `t_sf` =
/// 3.61 × 10⁴ yr (eqs. 3.10–3.11), 1.328 × 10⁴, to the three figures the plan and P07.T12 use.
const PDS_TIME_YR: f64 = 1.33e4;

/// CMB88's `v_PDS` at the same point, m/s: 413 km/s (eq. 3.33b).
const PDS_SPEED_M_S: f64 = 413e3;

/// `√(5⁄3)`: the adiabatic sound speed over the isothermal, for a monatomic gas.
const ADIABATIC_FACTOR: f64 = 1.290_994_448_735_805_6;

/// The explosion's energy is always positive and finite; this rejects the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildExplosionEnergyError {
    /// The energy, in units of 10⁵¹ erg, lies outside [`ExplosionEnergy::MIN`]–[`ExplosionEnergy::MAX`].
    OutOfRange(f64),
}

impl fmt::Display for BuildExplosionEnergyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange(foes) => write!(
                f,
                "an explosion energy of {foes} × 10⁵¹ erg lies outside the generator's range"
            ),
        }
    }
}

impl Error for BuildExplosionEnergyError {}

/// A supernova's kinetic energy, in units of 10⁵¹ erg (the "foe", `E₅₁`).
///
/// Its law belongs to the generator version: log-normal about 10⁵¹ erg with 0.2 dex, truncated at
/// two standard deviations, so 0.40–2.51 × 10⁵¹ erg (ours, provisional: the plan asks for "a
/// log-normal about 10⁵¹ erg" and names no width; core-collapse energies inferred from light
/// curves spread over a few tenths to a few 10⁵¹ erg). The truncation is what keeps the window's
/// supremum finite, since the window rises as about `E₅₁^0.32`. The constructor rejects anything
/// outside the range, so every energy a caller holds is one the caps allow for.
///
/// # Examples
///
/// The mark a system draws, one uniform on its own stream, and the median it falls about:
///
/// ```
/// use hyperion_sim::galaxy::snr::ExplosionEnergy;
///
/// assert_eq!(ExplosionEnergy::from_uniform(0.5), ExplosionEnergy::MEDIAN);
/// let low = ExplosionEnergy::from_uniform(0.0);
/// let high = ExplosionEnergy::from_uniform(1.0);
/// assert!((low.foes() - ExplosionEnergy::MIN.foes()).abs() < 1e-12);
/// assert!((high.foes() - ExplosionEnergy::MAX.foes()).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ExplosionEnergy(f64);

impl ExplosionEnergy {
    /// The median, 10⁵¹ erg.
    pub const MEDIAN: Self = Self(1.0);

    /// The log-normal's width, dex.
    pub const SIGMA_DEX: f64 = 0.2;

    /// The truncation, in standard deviations either side of the median.
    pub const TRUNCATION: f64 = 2.0;

    /// The least energy, `10^(−0.4)` × 10⁵¹ erg.
    pub const MIN: Self = Self(0.398_107_170_553_497_2);

    /// The greatest energy, `10^0.4` × 10⁵¹ erg.
    pub const MAX: Self = Self(2.511_886_431_509_580_4);

    /// The energy of `foes` × 10⁵¹ erg.
    ///
    /// # Errors
    ///
    /// [`BuildExplosionEnergyError::OutOfRange`] for a value outside [`MIN`](Self::MIN)–[`MAX`](Self::MAX),
    /// NaN included.
    pub fn from_foes(foes: f64) -> Result<Self, BuildExplosionEnergyError> {
        if (Self::MIN.0..=Self::MAX.0).contains(&foes) {
            Ok(Self(foes))
        } else {
            Err(BuildExplosionEnergyError::OutOfRange(foes))
        }
    }

    /// The energy at rank `u` in [0, 1] of the truncated log-normal: `10^(σ z)` with `z` the
    /// standard normal's quantile at `Φ(−2) + u (Φ(2) − Φ(−2))`. It is the one law of the
    /// generator version's explosion energies; plan 09's `DeathMarks::of` feeds it one uniform of
    /// the system's `snr.energy` stream. A `u` outside [0, 1] is held to it.
    #[must_use]
    pub fn from_uniform(u: f64) -> Self {
        let z = truncated_standard_normal(u, Self::TRUNCATION);
        Self(math::exp10(Self::SIGMA_DEX * z).clamp(Self::MIN.0, Self::MAX.0))
    }

    /// The energy in units of 10⁵¹ erg.
    #[must_use]
    pub const fn foes(self) -> f64 {
        self.0
    }
}

/// `Φ(x)`, the standard normal's distribution function, through [`math::erfc`]: the rank of a
/// standard normal, which [`truncated_standard_normal`] maps into a truncated normal.
#[must_use]
pub fn standard_normal_cdf(x: f64) -> f64 {
    0.5 * math::erfc(-x * core::f64::consts::FRAC_1_SQRT_2)
}

/// The standard normal truncated at `±limit` at rank `u` in [0, 1]: the quantile at `Φ(−limit) +
/// u (Φ(limit) − Φ(−limit))`, held to `±limit`. A NaN rank reads as ½. It is the law of
/// [`ExplosionEnergy`] (`limit` 2) and of a superbubble's interior density (P09.T4.b, ruling
/// 136.3), which passes its drawn normal's rank [`standard_normal_cdf`].
#[must_use]
pub fn truncated_standard_normal(u: f64, limit: f64) -> f64 {
    let u = if u.is_nan() { 0.5 } else { u.clamp(0.0, 1.0) };
    let lo = standard_normal_cdf(-limit);
    let hi = standard_normal_cdf(limit);
    let p = lo + u * (hi - lo);
    math::normal_quantile(p).clamp(-limit, limit)
}

/// The gas a [`SiteGas`] was asked to hold is not a gas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildSiteGasError {
    /// The hydrogen density, cm⁻³, is not positive and finite.
    Density(f64),
    /// The pressure `P ÷ k`, K cm⁻³, or the temperature, K, is not positive and finite.
    Pressure(f64),
}

impl fmt::Display for BuildSiteGasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Density(n) => write!(f, "a site's hydrogen density of {n} cm⁻³ is not a gas"),
            Self::Pressure(p) => write!(f, "a site's pressure or temperature of {p} is not a gas"),
        }
    }
}

impl Error for BuildSiteGasError {}

/// The gas a supernova explodes into, as its window reads it: the ambient's in-situ hydrogen
/// density, its isothermal sound speed `C₀ = √(P ÷ ρ)` and its pressure.
///
/// At a site in the galaxy it is [`GasField::state`] at [`SiteGas::SMOOTHING`], with no
/// modifiers ([`SiteGas::at`]): the point's own phase, its density and its sound speed (ruling
/// 103 of 2026-09-22), with the pressure never below the corona's floor. Inside a superbubble the
/// caller builds it from the bubble's interior ([`SiteGas::hot_interior`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SiteGas {
    density: HydrogenPerCm3,
    sound_speed: MetresPerSecond,
    pressure: KelvinPerCm3,
}

impl SiteGas {
    /// The smoothing scale a site's gas is read at: the noise's octaves at least 250 ly apart,
    /// about a shell's size (plan 09, P09.T15.a). A parameter of the generator version.
    pub const SMOOTHING: SmoothingScale = SmoothingScale::AtLeast(LightYears::new(250.0));

    /// The gas at `p` in `gas`, read at [`SMOOTHING`](Self::SMOOTHING). `cache` changes what the
    /// call costs, never what it returns.
    #[must_use]
    pub fn at(gas: &GasField, p: &GalacticPosition, cache: &mut NoiseCache) -> Self {
        Self::of(&gas.state(p, Self::SMOOTHING, cache))
    }

    /// The site of a gas state: its phase's density and isothermal sound speed, and the parcel's
    /// pressure. A hot phase with no corona (an infinite temperature) or a phase of no density
    /// gives a site whose window is zero.
    #[must_use]
    pub fn of(state: &GasState) -> Self {
        Self {
            density: state.local_density(),
            sound_speed: state.isothermal_sound_speed(),
            pressure: state.pressure(),
        }
    }

    /// Gas of hydrogen density `density` at pressure `pressure`, whose isothermal sound speed is
    /// `√(P ÷ ρ)` with `ρ = 1.4 m_H n`: the form a window table is written in.
    ///
    /// # Errors
    ///
    /// [`BuildSiteGasError::Density`] and [`BuildSiteGasError::Pressure`] for a value that is not
    /// positive and finite.
    pub fn uniform(
        density: HydrogenPerCm3,
        pressure: KelvinPerCm3,
    ) -> Result<Self, BuildSiteGasError> {
        let n = density.value();
        let p = pressure.value();
        if !(n.is_finite() && n > 0.0) {
            return Err(BuildSiteGasError::Density(n));
        }
        if !(p.is_finite() && p > 0.0) {
            return Err(BuildSiteGasError::Pressure(p));
        }
        let c2 = p * BOLTZMANN_CONSTANT / (MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG * n);
        Ok(Self {
            density,
            sound_speed: MetresPerSecond::new(c2.sqrt()),
            pressure,
        })
    }

    /// Fully ionised gas of hydrogen density `density` at `temperature`, 2.3 particles per
    /// hydrogen nucleus ([`IONISED_PARTICLES_PER_HYDROGEN`](crate::galaxy::gas::IONISED_PARTICLES_PER_HYDROGEN)):
    /// a superbubble's interior, whose pressure is `2.3 n T`.
    ///
    /// # Errors
    ///
    /// As [`uniform`](Self::uniform), for a temperature that is not positive and finite.
    pub fn hot_interior(
        density: HydrogenPerCm3,
        temperature: Kelvin,
    ) -> Result<Self, BuildSiteGasError> {
        let t = temperature.value();
        if !(t.is_finite() && t > 0.0) {
            return Err(BuildSiteGasError::Pressure(t));
        }
        let p = crate::galaxy::gas::IONISED_PARTICLES_PER_HYDROGEN * density.value() * t;
        Self::uniform(density, KelvinPerCm3::new(p))
    }

    /// The ambient's in-situ hydrogen density.
    #[must_use]
    pub const fn density(&self) -> HydrogenPerCm3 {
        self.density
    }

    /// The ambient's isothermal sound speed `C₀`.
    #[must_use]
    pub const fn sound_speed(&self) -> MetresPerSecond {
        self.sound_speed
    }

    /// The ambient's thermal pressure `P ÷ k`.
    #[must_use]
    pub const fn pressure(&self) -> KelvinPerCm3 {
        self.pressure
    }
}

/// Which of the window's two branches a shell ends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowBranch {
    /// The shell cools and ends in the pressure-driven snowplough (CMB88).
    Radiative,
    /// The shell slows to the merge speed while still adiabatic, in hot, thin gas.
    Hot,
}

/// A shell's window at one site, with what its evolution needs: the ambient's density, the
/// explosion's energy, the metallicity ratio `ζ`, the merge speed `β c_net` and CMB88's `t_PDS`
/// and `v_PDS`.
///
/// Built by [`shell_window`]; a caller may end it early ([`ended_at`](Self::ended_at)), at the
/// environment's cap or, inside a superbubble, where the shell reaches the bubble's wall
/// ([`ended_at_wall`](Self::ended_at_wall)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellWindow {
    duration: Years,
    natural: Years,
    branch: WindowBranch,
    density: HydrogenPerCm3,
    energy: ExplosionEnergy,
    zeta: f64,
    merge_speed: MetresPerSecond,
    pds_time: Years,
    pds_speed: MetresPerSecond,
    sound_speed: MetresPerSecond,
}

impl ShellWindow {
    /// How long the shell stays distinct from its explosion: its window, `W`, as it may have been
    /// ended early.
    #[must_use]
    pub const fn duration(&self) -> Years {
        self.duration
    }

    /// The window [`shell_window`] gave, before any early end.
    #[must_use]
    pub const fn natural_duration(&self) -> Years {
        self.natural
    }

    /// The branch the natural window ends on.
    #[must_use]
    pub const fn branch(&self) -> WindowBranch {
        self.branch
    }

    /// The ambient's hydrogen density.
    #[must_use]
    pub const fn density(&self) -> HydrogenPerCm3 {
        self.density
    }

    /// The explosion's energy.
    #[must_use]
    pub const fn energy(&self) -> ExplosionEnergy {
        self.energy
    }

    /// The ambient's metallicity over solar, `ζ`, as the window read it (held to 0.1–10^0.5).
    #[must_use]
    pub const fn metallicity_ratio(&self) -> f64 {
        self.zeta
    }

    /// The shock speed at which the shell merges, `β c_net`.
    #[must_use]
    pub const fn merge_speed(&self) -> MetresPerSecond {
        self.merge_speed
    }

    /// CMB88's `t_PDS`: when the shell would enter the pressure-driven snowplough.
    #[must_use]
    pub const fn pds_time(&self) -> Years {
        self.pds_time
    }

    /// The ambient's adiabatic sound speed `c_s = √(5⁄3) C₀`, Tang and Wang's (2005) `c_s`.
    #[must_use]
    pub const fn sound_speed(&self) -> MetresPerSecond {
        self.sound_speed
    }

    /// Tang and Wang's characteristic time on CMB88's Sedov clock, `t_c = t_PDS (v_PDS ÷
    /// c_s)^(5⁄3)`: infinite in gas with no sound speed.
    #[must_use]
    pub fn characteristic_time(&self) -> Years {
        let cs = self.sound_speed.value();
        if cs > 0.0 {
            Years::new(self.pds_time.value() * math::powf(self.pds_speed.value() / cs, 5.0 / 3.0))
        } else {
            Years::new(f64::INFINITY)
        }
    }

    /// CMB88's `v_PDS`: the Sedov speed at `t_PDS`.
    #[must_use]
    pub const fn pds_speed(&self) -> MetresPerSecond {
        self.pds_speed
    }

    /// The same shell with its window ended at `end` if that is sooner: the environment's cap, or
    /// a lifetime cap. A negative `end` ends it at once.
    #[must_use]
    pub fn ended_at(self, end: Years) -> Self {
        let end = end.value().max(0.0);
        Self {
            duration: Years::new(self.duration.value().min(end)),
            ..self
        }
    }
}

/// The least and greatest `[M/H]`, dex, the window reads the metallicity at: 0.1 to 10^0.5 times
/// solar. Plan 07 holds the gas's own metallicity to +0.5 dex; below a tenth of solar CMB88's
/// cooling fit (their eq. 3.10's `ζ^(−5⁄14)`, from solar-abundance cooling scaled by `ζ`) is not
/// used here (ours, provisional). The window varies as `ζ^(−5⁄98)` far out on the radiative
/// branch and not at all on the hot one, so between −1 and 0 dex it moves by 12–15%.
pub const METALLICITY_RANGE_DEX: (f64, f64) = (-1.0, 0.5);

/// The shell window of a supernova of `energy` into the gas of `site` whose ambient metallicity
/// is `metallicity` (`[M/H]`, dex; CMB88's `ζ` is the metallicity of the gas the shell sweeps up
/// and cools in).
///
/// A site whose density is not positive and finite, or whose sound speed is infinite (a hot phase
/// with no corona), gives a window of zero on the hot branch: the shell never stands out from
/// such gas.
///
/// # Examples
///
/// Warm gas near the Sun: a shell stands out for close to a million years.
///
/// ```
/// use hyperion_sim::galaxy::snr::{ExplosionEnergy, SiteGas, WindowBranch, shell_window};
/// use hyperion_sim::units::{Dex, HydrogenPerCm3, KelvinPerCm3};
///
/// let site = SiteGas::uniform(HydrogenPerCm3::new(0.3), KelvinPerCm3::new(3_800.0))?;
/// let window = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
/// assert_eq!(window.branch(), WindowBranch::Radiative);
/// assert!((8e5..1.0e6).contains(&window.duration().value()));
/// # Ok::<(), hyperion_sim::galaxy::snr::BuildSiteGasError>(())
/// ```
#[must_use]
pub fn shell_window(site: &SiteGas, energy: ExplosionEnergy, metallicity: Dex) -> ShellWindow {
    let n = site.density.value();
    let c0 = site.sound_speed.value();
    let (lo, hi) = METALLICITY_RANGE_DEX;
    let fe_h = if metallicity.value().is_nan() {
        0.0
    } else {
        metallicity.value().clamp(lo, hi)
    };
    let zeta = math::exp10(fe_h);
    let e = energy.foes();
    let sigma = MetresPerSecond::from(TURBULENT_SPEED).value();
    let merge = MERGE_FACTOR * (c0 * c0 + sigma * sigma).sqrt();
    if !(n.is_finite() && n > 0.0 && merge.is_finite()) {
        return ShellWindow {
            duration: Years::ZERO,
            natural: Years::ZERO,
            branch: WindowBranch::Hot,
            density: site.density,
            energy,
            zeta,
            merge_speed: MetresPerSecond::new(merge),
            pds_time: Years::ZERO,
            pds_speed: MetresPerSecond::ZERO,
            sound_speed: MetresPerSecond::new(c0),
        };
    }
    let t_pds = PDS_TIME_YR
        * math::powf(e, 3.0 / 14.0)
        * math::powf(zeta, -5.0 / 14.0)
        * math::powf(n, -4.0 / 7.0);
    let v_pds = PDS_SPEED_M_S
        * math::powf(n, 1.0 / 7.0)
        * math::powf(zeta, 3.0 / 14.0)
        * math::powf(e, 1.0 / 14.0);
    let cs = ADIABATIC_FACTOR * c0;
    let (cs53, vp53, merge53) = (
        math::powf(cs, 5.0 / 3.0),
        math::powf(v_pds, 5.0 / 3.0),
        math::powf(merge, 5.0 / 3.0),
    );
    let v_star = math::powf(vp53 + cs53, 0.6);
    let (w, branch) = if v_star > merge {
        let x = v_star / merge;
        (
            t_pds * (0.75 * math::powf(x, 10.0 / 7.0) + 0.25),
            WindowBranch::Radiative,
        )
    } else {
        (t_pds * vp53 / (merge53 - cs53), WindowBranch::Hot)
    };
    ShellWindow {
        duration: Years::new(w),
        natural: Years::new(w),
        branch,
        density: site.density,
        energy,
        zeta,
        merge_speed: MetresPerSecond::new(merge),
        pds_time: Years::new(t_pds),
        pds_speed: MetresPerSecond::new(v_pds),
        sound_speed: MetresPerSecond::new(cs),
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::gas::params::GasParams;
    use crate::galaxy::gas::pressure::Pressure;
    use crate::galaxy::gas::smooth::SmoothGas;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::snr::testing::{WINDOW_TABLE_DENSITIES, window_table};

    fn site(n: f64, p: f64) -> SiteGas {
        SiteGas::uniform(HydrogenPerCm3::new(n), KelvinPerCm3::new(p)).unwrap()
    }

    fn window_yr(n: f64, p: f64) -> f64 {
        shell_window(&site(n, p), ExplosionEnergy::MEDIAN, Dex::new(0.0))
            .duration()
            .value()
    }

    /// The energy's bounds are `10^(∓0.4)`, and the law is the truncated log-normal's.
    #[test]
    fn the_energy_law_is_a_truncated_log_normal() {
        assert!((ExplosionEnergy::MIN.foes() - math::exp10(-0.4)).abs() < 1e-15);
        assert!((ExplosionEnergy::MAX.foes() - math::exp10(0.4)).abs() < 1e-15);
        let mut last = 0.0;
        for k in 0..=1_000 {
            let e = ExplosionEnergy::from_uniform(f64::from(k) / 1_000.0).foes();
            assert!(
                e >= last && e >= ExplosionEnergy::MIN.foes() && e <= ExplosionEnergy::MAX.foes()
            );
            last = e;
        }
        // The quartiles of a normal truncated at ±2: ±0.6397 σ, where Φ is 0.7386.
        let q = ExplosionEnergy::from_uniform(0.75).foes();
        assert!((math::log10(q) / 0.2 - 0.6397).abs() < 1e-3, "{q}");
        assert!(matches!(
            ExplosionEnergy::from_foes(3.0),
            Err(BuildExplosionEnergyError::OutOfRange(_))
        ));
        assert!(matches!(
            ExplosionEnergy::from_foes(f64::NAN),
            Err(BuildExplosionEnergyError::OutOfRange(_))
        ));
        assert_eq!(ExplosionEnergy::from_foes(1.0), Ok(ExplosionEnergy::MEDIAN));
    }

    /// P09.T15.a: at P ÷ k = 3,800 K cm⁻³ the window table reproduces, to 25%, ruling 136.1's
    /// 4.4, 5.3, 8.5, 8.4, 4.4, 1.9, 0.82 and 0.35 × 10⁵ yr from 10⁻³ to 10⁴ cm⁻³, with the switch
    /// to the hot branch at 1.5 × 10⁻³ cm⁻³.
    #[test]
    fn the_window_table_matches_the_plan() {
        let expected = [4.4, 5.3, 8.5, 8.4, 4.4, 1.9, 0.82, 0.35];
        let table = window_table(KelvinPerCm3::new(3_800.0));
        assert_eq!(table.len(), expected.len());
        for ((n, window), want) in table.iter().zip(expected) {
            let got = window.duration().value() / 1e5;
            assert!(
                (got / want - 1.0).abs() <= 0.25,
                "n = {} cm⁻³: {got:.3} × 10⁵ yr against {want}",
                n.value()
            );
            let branch = if n.value() < 1.5e-3 {
                WindowBranch::Hot
            } else {
                WindowBranch::Radiative
            };
            assert_eq!(window.branch(), branch, "n = {}", n.value());
        }
        assert_same_bits(WINDOW_TABLE_DENSITIES[0], 1e-3);
    }

    /// The window is continuous where its branches meet: at `v_PDS = β c_net` both give `t_PDS`.
    #[test]
    fn the_window_is_continuous_across_the_branch() {
        // Walk the density finely through the switch at P ÷ k = 3,800 K cm⁻³.
        let mut last: Option<(f64, WindowBranch)> = None;
        let mut switched = 0;
        for k in 0..=20_000 {
            let n = math::exp10(-3.0 + f64::from(k) * 1e-4);
            let w = shell_window(&site(n, 3_800.0), ExplosionEnergy::MEDIAN, Dex::new(0.0));
            let now = (w.duration().value(), w.branch());
            if let Some((before, branch)) = last {
                // A step of 10⁻⁴ dex moves a smooth window by well under 0.1%.
                assert!(
                    (now.0 / before - 1.0).abs() < 1e-3,
                    "n = {n}: {before} → {}",
                    now.0
                );
                if branch != now.1 {
                    switched += 1;
                    assert!((now.0 / w.pds_time().value() - 1.0).abs() < 1e-3);
                }
            }
            last = Some(now);
        }
        assert_eq!(switched, 1);
        // In hot gas, where the turbulence is negligible, the hot branch is 0.93 t_c:
        // 1 ÷ ((2 ÷ √(5⁄3))^(5⁄3) − 1) = 0.931.
        let hot = shell_window(&site(1e-6, 3_800.0), ExplosionEnergy::MEDIAN, Dex::new(0.0));
        assert_eq!(hot.branch(), WindowBranch::Hot);
        let ratio = hot.duration().value() / hot.characteristic_time().value();
        let expected = 1.0 / (math::powf(2.0 / ADIABATIC_FACTOR, 5.0 / 3.0) - 1.0);
        assert!(
            (ratio / expected - 1.0).abs() < 1e-3,
            "{ratio} against {expected}"
        );
        assert!((expected - 0.931).abs() < 1e-3, "{expected}");
    }

    /// W → 0 at both ends of density at a fixed pressure.
    #[test]
    fn the_window_vanishes_at_both_ends_of_density() {
        for p in [300.0, 3_800.0, 1e5] {
            let low = window_yr(1e-12, p);
            let high = window_yr(1e12, p);
            let peak = (0..=600)
                .map(|k| window_yr(math::exp10(-6.0 + f64::from(k) * 0.02), p))
                .fold(0.0, f64::max);
            assert!(low < 1e-3 * peak, "p = {p}: {low} against {peak}");
            assert!(high < 1e-3 * peak, "p = {p}: {high} against {peak}");
            assert!(window_yr(1e-18, p) < low && window_yr(1e18, p) < high);
        }
        let dead = SiteGas {
            density: HydrogenPerCm3::new(0.0),
            sound_speed: MetresPerSecond::new(f64::INFINITY),
            pressure: KelvinPerCm3::new(300.0),
        };
        let w = shell_window(&dead, ExplosionEnergy::MAX, Dex::new(0.0));
        assert_eq!(w.duration(), Years::ZERO);
        assert_eq!(w.branch(), WindowBranch::Hot);
    }

    /// The window grows with energy (as E^0.32 on the radiative branch at the floor), shrinks
    /// with the sound speed and moves little with metallicity.
    #[test]
    fn the_window_scales_with_energy_and_metallicity() {
        let s = site(0.03, 300.0);
        let w = |e: ExplosionEnergy, z: f64| shell_window(&s, e, Dex::new(z)).duration().value();
        let slope = math::ln(w(ExplosionEnergy::MAX, 0.0) / w(ExplosionEnergy::MIN, 0.0))
            / math::ln(ExplosionEnergy::MAX.foes() / ExplosionEnergy::MIN.foes());
        assert!((0.30..0.33).contains(&slope), "{slope}");
        assert!(w(ExplosionEnergy::MEDIAN, -1.0) > w(ExplosionEnergy::MEDIAN, 0.0));
        assert!(w(ExplosionEnergy::MEDIAN, -1.0) < 1.16 * w(ExplosionEnergy::MEDIAN, 0.0));
        // The clamp: [M/H] below −1 reads as −1.
        assert_same_bits(
            w(ExplosionEnergy::MEDIAN, -3.0),
            w(ExplosionEnergy::MEDIAN, -1.0),
        );
        assert!(window_yr(0.03, 3_000.0) < window_yr(0.03, 300.0));
    }

    /// P09.T15.a: at the model's own pressure in the plane, the Milky Way fixture's at 26,000 ly,
    /// the longest windows are 0.5–1 Myr and fall at 0.1–0.5 cm⁻³; at a floor of 300–450 K cm⁻³
    /// the largest at 10⁵¹ erg is 2.07–2.40 Myr (ruling 136.1's rule; ruling 98 had 2.06–2.39).
    #[test]
    fn the_longest_windows_fall_in_warm_gas() {
        let fixture = GasParams::milky_way_like();
        let plane = Pressure::new(&fixture).at(&SmoothGas::new(&fixture), 26_000.0, 0.0);
        let (w, n) = (0..=4_000)
            .map(|k| {
                let n = math::exp10(-3.0 + f64::from(k) / 1_000.0);
                (window_yr(n, plane), n)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap();
        assert!((0.5e6..=1e6).contains(&w), "{w} yr at P ÷ k = {plane}");
        assert!((0.1..=0.5).contains(&n), "{n} cm⁻³");
        for floor in [300.0, 450.0] {
            let largest = (0..=4_000)
                .map(|k| window_yr(math::exp10(-3.0 + f64::from(k) / 1_000.0), floor))
                .fold(0.0, f64::max);
            assert!((2.0e6..=2.45e6).contains(&largest), "{largest} at {floor}");
        }
    }

    /// The sites the galaxy gives: in the plane most are warm or cold gas whose windows are at
    /// most a few 10⁵ to 10⁶ years, and every window is finite and non-negative.
    #[test]
    fn sites_in_the_galaxy_give_finite_windows() {
        let galaxy =
            Galaxy::from_params(Seed::new(0x0915_0001), GalaxyParams::milky_way_like()).unwrap();
        let mut cache = NoiseCache::with_capacity(256);
        let mut longest = 0.0_f64;
        for k in 0..2_000_u32 {
            let phi = f64::from(k) * 0.618_033_988_749_895 * core::f64::consts::TAU;
            let r = 20_000.0 + 10_000.0 * (f64::from(k % 97) / 97.0);
            let z = -600.0 + 1_200.0 * (f64::from(k % 13) / 13.0);
            let p = GalacticPosition::from_light_years([r * math::cos(phi), r * math::sin(phi), z])
                .unwrap();
            let site = SiteGas::at(galaxy.gas(), &p, &mut cache);
            let w = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
            let years = w.duration().value();
            assert!(years.is_finite() && years >= 0.0, "{site:?}");
            assert!(site.pressure() >= galaxy.gas().params().pressure_floor());
            longest = longest.max(years);
        }
        assert!((3e5..2.5e6).contains(&longest), "{longest}");
    }

    #[test]
    fn a_site_rejects_what_is_not_gas() {
        assert_eq!(
            SiteGas::uniform(HydrogenPerCm3::new(0.0), KelvinPerCm3::new(1.0)),
            Err(BuildSiteGasError::Density(0.0))
        );
        assert_eq!(
            SiteGas::uniform(HydrogenPerCm3::new(1.0), KelvinPerCm3::new(f64::INFINITY)),
            Err(BuildSiteGasError::Pressure(f64::INFINITY))
        );
        assert_eq!(
            SiteGas::hot_interior(HydrogenPerCm3::new(0.005), Kelvin::new(-1.0)),
            Err(BuildSiteGasError::Pressure(-1.0))
        );
        // A bubble's interior at 10^6.2 K: C₀ = √(2.3 k T ÷ 1.4 m_H), about 146 km/s.
        let hot =
            SiteGas::hot_interior(HydrogenPerCm3::new(0.005), Kelvin::new(1_584_893.2)).unwrap();
        let c = hot.sound_speed().value() / 1e3;
        assert!((140.0..150.0).contains(&c), "{c}");
    }

    #[test]
    fn ending_a_window_early_keeps_the_rest() {
        let w = shell_window(&site(0.3, 3_800.0), ExplosionEnergy::MEDIAN, Dex::new(0.0));
        let cut = w.ended_at(Years::new(1e5));
        assert_eq!(cut.duration(), Years::new(1e5));
        assert_eq!(cut.natural_duration(), w.duration());
        assert_eq!(w.ended_at(Years::new(1e9)), w);
        assert_eq!(w.ended_at(Years::new(-5.0)).duration(), Years::ZERO);
    }
}
