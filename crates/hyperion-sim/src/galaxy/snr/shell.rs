//! A shell's radius, shock speed, phase and emission at an age (P09.T16.a).
//!
//! Four phases, each a closed form, joined so that the radius is continuous in age:
//!
//! 1. **Free expansion and Sedov–Taylor**, after Truelove and McKee (1999, ApJS 120, 299, Table 5,
//!    uniform ejecta, `n = 0`), in their characteristic units `R_ch = M_ej^(1⁄3) ρ^(−1⁄3)` and
//!    `t_ch = E^(−1⁄2) M_ej^(5⁄6) ρ^(−1⁄3)`: `R* = 2.01 t* (1 + 1.72 t*^(3⁄2))^(−2⁄3)` to `t*_ST =
//!    0.495`, then `R* = (R*_ST^(5⁄2) + ξ₀^(1⁄2) (t* − t*_ST))^(2⁄5)` with `ξ₀ = 2.026`. Here
//!    `R*_ST` is the first form's value at `t*_ST` (0.7276 against the paper's rounded 0.727), so
//!    the join is exact.
//! 2. **The pressure-driven snowplough** from `t_PDS`, CMB88's offset power law `R = R_PDS ((t −
//!    c t_PDS) ÷ ((1 − c) t_PDS))^(3⁄10)` (their eqs. 3.28–3.32a), with `R_PDS` the Sedov–Taylor
//!    radius at `t_PDS`, which is CMB88's 14.0 pc `E₅₁^(2⁄7) n^(−3⁄7) ζ^(−1⁄7)` (eq. 3.33a) to
//!    under 1%, and the offset `c` chosen, as CMB88's eq. 3.30 does, so that the speed is
//!    continuous too: `c = 1 − (3⁄10) R_PDS ÷ (t_PDS v_ST)`, which is their ¼ wherever the blast
//!    is Sedov's `v = 2R ÷ 5t`, and departs from it only in gas so dense that `t_PDS` falls
//!    within a few `t_ch` of the free expansion.
//! 3. **The momentum-conserving snowplough** from CMB88's `t_MCS` (their eq. 4.2, `t_MCS ÷ t_PDS
//!    = min[61 v_ej,8³ ÷ (ζ^(9⁄14) n^(3⁄7) E₅₁^(3⁄14)), 476 ÷ ζ^(9⁄14)]` with Spitzer conduction,
//!    `v_ej,8 = 10 (E₅₁ ÷ M_ej)^(1⁄2)`), at constant momentum: `R⁴ = R_m⁴ + 4 R_m³ v_m (t − t_m)`,
//!    which keeps the radius and the speed continuous (ours: CMB88's eq. 4.3 is the same law
//!    with their fitted momentum). A shell reaches it before its window ends only in gas denser
//!    than about 10³ cm⁻³.
//!
//! The window of [`shell_window`](super::shell_window) ends the shell. On its hot branch that is
//! before `t_PDS`, so a shell in hot gas never leaves the Sedov–Taylor phase; inside a superbubble
//! the window also ends where the shell reaches the bubble's wall
//! ([`ShellWindow::ended_at_wall`]).
//!
//! Emission follows the phase (brainstorm, "Supernova remnants"): X-ray and radio while
//! non-radiative (free expansion and Sedov–Taylor), optical filaments while the shock is above 70
//! km/s, and the 21 cm shell of neutral hydrogen from the radiative phases to the end.

use crate::galaxy::gas::MASS_PER_HYDROGEN_FACTOR;
use crate::galaxy::snr::window::ShellWindow;
use crate::math;
use crate::units::consts::{
    HYDROGEN_MASS_KG, METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR, SOLAR_MASS_KG,
};
use crate::units::{KilometresPerSecond, LightYears, SolarMasses, Years};

/// The mass a supernova ejects, which sets the free-expansion phase: 3 M☉ (ours, provisional, a
/// typical core collapse's; a Type Ia ejects about 1.4). It moves only the first few thousand
/// years of a shell and CMB88's `t_MCS`, and never the window.
pub const EJECTA_MASS: SolarMasses = SolarMasses::new(3.0);

/// The shock speed above which a shell shows optical filaments: 70 km/s (brainstorm, "Supernova
/// remnants").
pub const OPTICAL_SHOCK_SPEED: KilometresPerSecond = KilometresPerSecond::new(70.0);

/// Truelove and McKee's `t*_ST` for uniform ejecta.
const T_ST: f64 = 0.495;

/// Truelove and McKee's free-expansion coefficients for uniform ejecta: `R* = A t* (1 + B
/// t*^(3⁄2))^(−2⁄3)`.
const ED_A: f64 = 2.01;
const ED_B: f64 = 1.72;

/// `ξ₀^(1⁄2)`, the Sedov–Taylor constant for γ = 5⁄3, `ξ₀ = 2.026`.
const XI_SQRT: f64 = 1.423_376_267_892_646_7;

/// The pressure-driven snowplough's index, `R ∝ (t − c t_PDS)^(3⁄10)` (CMB88, eq. 3.32a).
const PDS_INDEX: f64 = 0.3;

/// CMB88's two bounds on `t_MCS ÷ t_PDS` (their eq. 4.2).
const MCS_EJECTA: f64 = 61.0;
const MCS_CONDUCTION: f64 = 476.0;

/// A shell's phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShellPhase {
    /// The ejecta coast; the swept-up mass is still below theirs.
    FreeExpansion,
    /// The adiabatic blast wave.
    SedovTaylor,
    /// A cooled, dense shell pushed by the hot interior's pressure.
    PressureDrivenSnowplough,
    /// A dense shell coasting on its momentum.
    MomentumConservingSnowplough,
}

impl ShellPhase {
    /// Whether the shell still radiates little, which is when it is bright in X-rays and radio.
    #[must_use]
    pub const fn is_non_radiative(self) -> bool {
        match self {
            Self::FreeExpansion | Self::SedovTaylor => true,
            Self::PressureDrivenSnowplough | Self::MomentumConservingSnowplough => false,
        }
    }
}

/// What a shell shines in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShellEmission {
    x_ray_and_radio: bool,
    optical: bool,
    neutral_hydrogen: bool,
}

impl ShellEmission {
    /// Bright in X-rays and radio: non-radiative.
    #[must_use]
    pub const fn x_ray_and_radio(&self) -> bool {
        self.x_ray_and_radio
    }

    /// Optical filaments: the shock is above [`OPTICAL_SHOCK_SPEED`].
    #[must_use]
    pub const fn optical(&self) -> bool {
        self.optical
    }

    /// A shell of neutral hydrogen seen at 21 cm: radiative.
    #[must_use]
    pub const fn neutral_hydrogen(&self) -> bool {
        self.neutral_hydrogen
    }
}

/// A shell at one age.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellState {
    age: Years,
    radius: LightYears,
    shock_speed: KilometresPerSecond,
    phase: ShellPhase,
}

impl ShellState {
    /// Its age since the explosion.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }

    /// The radius of its outer shock, from the explosion site.
    #[must_use]
    pub const fn radius(&self) -> LightYears {
        self.radius
    }

    /// The speed of its outer shock.
    #[must_use]
    pub const fn shock_speed(&self) -> KilometresPerSecond {
        self.shock_speed
    }

    /// Its phase.
    #[must_use]
    pub const fn phase(&self) -> ShellPhase {
        self.phase
    }

    /// What it shines in.
    #[must_use]
    pub fn emission(&self) -> ShellEmission {
        ShellEmission {
            x_ray_and_radio: self.phase.is_non_radiative(),
            optical: self.shock_speed > OPTICAL_SHOCK_SPEED,
            neutral_hydrogen: !self.phase.is_non_radiative(),
        }
    }
}

/// The shell of `window` at `age` after the explosion: `None` before it (a negative age) and from
/// the end of its window on.
///
/// # Examples
///
/// A shell in warm gas: young, it is a hot blast wave; old, a cool shell of neutral hydrogen.
///
/// ```
/// use hyperion_sim::galaxy::snr::{ExplosionEnergy, ShellPhase, SiteGas, shell_state_at, shell_window};
/// use hyperion_sim::units::{Dex, HydrogenPerCm3, KelvinPerCm3, Years};
///
/// let site = SiteGas::uniform(HydrogenPerCm3::new(1.0), KelvinPerCm3::new(3_800.0))?;
/// let window = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
/// let young = shell_state_at(&window, Years::new(5_000.0)).expect("inside the window");
/// assert_eq!(young.phase(), ShellPhase::SedovTaylor);
/// assert!(young.emission().x_ray_and_radio());
/// let old = shell_state_at(&window, Years::new(5e5)).expect("inside the window");
/// assert_eq!(old.phase(), ShellPhase::PressureDrivenSnowplough);
/// assert!(old.emission().neutral_hydrogen() && old.radius() > young.radius());
/// assert!(shell_state_at(&window, window.duration()).is_none());
/// # Ok::<(), hyperion_sim::galaxy::snr::BuildSiteGasError>(())
/// ```
#[must_use]
pub fn shell_state_at(window: &ShellWindow, age: Years) -> Option<ShellState> {
    let t = age.value();
    if !(t >= 0.0 && t < window.duration().value()) {
        return None;
    }
    let (radius, speed, phase) = Evolution::of(window).at(t * SECONDS_PER_JULIAN_YEAR);
    Some(ShellState {
        age,
        radius: LightYears::new(radius / METRES_PER_LIGHT_YEAR),
        shock_speed: KilometresPerSecond::new(speed / 1e3),
        phase,
    })
}

impl ShellWindow {
    /// The same shell with its window ended when its radius reaches `wall`, if that is sooner:
    /// inside a superbubble, the distance from the explosion to the bubble's wall. The time is
    /// found by 64 bisection steps on the radius, which never falls with age.
    #[must_use]
    pub fn ended_at_wall(self, wall: LightYears) -> Self {
        let wall_m = wall.value().max(0.0) * METRES_PER_LIGHT_YEAR;
        let end = self.duration().value() * SECONDS_PER_JULIAN_YEAR;
        let evolution = Evolution::of(&self);
        if evolution.at(end).0 <= wall_m {
            return self;
        }
        let (mut lo, mut hi) = (0.0, end);
        for _ in 0..64 {
            let mid = f64::midpoint(lo, hi);
            if evolution.at(mid).0 < wall_m {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        self.ended_at(Years::new(lo / SECONDS_PER_JULIAN_YEAR))
    }
}

/// The joins of one shell's phases, in SI units: metres, seconds, metres per second.
struct Evolution {
    r_ch: f64,
    t_ch: f64,
    r_st: f64,
    t_st: f64,
    t_p: f64,
    r_p: f64,
    /// The offset `c t_p` of the snowplough's power law, seconds.
    offset: f64,
    t_m: f64,
    r_m: f64,
    v_m: f64,
}

impl Evolution {
    fn of(window: &ShellWindow) -> Self {
        let n = window.density().value();
        let e = window.energy().foes();
        let zeta = window.metallicity_ratio();
        let rho = MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG * n * 1e6;
        let energy = e * 1e44;
        let mass = EJECTA_MASS.value() * SOLAR_MASS_KG;
        let rho_third = math::cbrt(rho);
        let r_ch = math::cbrt(mass) / rho_third;
        let t_ch = math::powf(mass, 5.0 / 6.0) / (energy.sqrt() * rho_third);
        let r_st = free_expansion(T_ST).0;
        let t_st = T_ST * t_ch;
        let t_pds = window.pds_time().value() * SECONDS_PER_JULIAN_YEAR;
        let t_p = t_pds.max(t_st);
        let (r_p_star, v_p_star) = sedov_taylor(r_st, t_p / t_ch);
        let r_p = r_p_star * r_ch;
        let v_p = v_p_star * r_ch / t_ch;
        let offset = t_p - PDS_INDEX * r_p / v_p;
        let v_ej8 = 10.0 * (e / EJECTA_MASS.value()).sqrt();
        let mcs = (MCS_EJECTA * v_ej8 * v_ej8 * v_ej8
            / (math::powf(zeta, 9.0 / 14.0)
                * math::powf(n, 3.0 / 7.0)
                * math::powf(e, 3.0 / 14.0)))
        .min(MCS_CONDUCTION / math::powf(zeta, 9.0 / 14.0));
        let t_m = (mcs * t_pds).max(t_p);
        let (r_m, v_m) = snowplough(r_p, t_p, offset, t_m);
        Self {
            r_ch,
            t_ch,
            r_st,
            t_st,
            t_p,
            r_p,
            offset,
            t_m,
            r_m,
            v_m,
        }
    }

    /// The radius, speed and phase at `t` seconds, `t ≥ 0`.
    fn at(&self, t: f64) -> (f64, f64, ShellPhase) {
        if t < self.t_st {
            let (r, v) = free_expansion(t / self.t_ch);
            (
                r * self.r_ch,
                v * self.r_ch / self.t_ch,
                ShellPhase::FreeExpansion,
            )
        } else if t < self.t_p {
            let (r, v) = sedov_taylor(self.r_st, t / self.t_ch);
            (
                r * self.r_ch,
                v * self.r_ch / self.t_ch,
                ShellPhase::SedovTaylor,
            )
        } else if t < self.t_m {
            let (r, v) = snowplough(self.r_p, self.t_p, self.offset, t);
            (r, v, ShellPhase::PressureDrivenSnowplough)
        } else {
            let r3 = self.r_m * self.r_m * self.r_m;
            let r = math::powf(r3 * self.r_m + 4.0 * r3 * self.v_m * (t - self.t_m), 0.25);
            (
                r,
                self.v_m * r3 / (r * r * r),
                ShellPhase::MomentumConservingSnowplough,
            )
        }
    }
}

/// CMB88's offset power law from `(t_p, r_p)` with offset `offset`: `(R, v)` at `t ≥ t_p`.
fn snowplough(r_p: f64, t_p: f64, offset: f64, t: f64) -> (f64, f64) {
    let r = r_p * math::powf((t - offset) / (t_p - offset), PDS_INDEX);
    (r, PDS_INDEX * r / (t - offset))
}

/// Truelove and McKee's free expansion of uniform ejecta: `(R*, v*)` at `t*`.
fn free_expansion(t: f64) -> (f64, f64) {
    let q = 1.0 + ED_B * t * t.sqrt();
    (
        ED_A * t * math::powf(q, -2.0 / 3.0),
        ED_A * math::powf(q, -5.0 / 3.0),
    )
}

/// Truelove and McKee's Sedov–Taylor phase from `(t*_ST, r_st)`: `(R*, v*)` at `t* ≥ t*_ST`.
fn sedov_taylor(r_st: f64, t: f64) -> (f64, f64) {
    let base = r_st * r_st * r_st.sqrt() + XI_SQRT * (t - T_ST);
    let r = math::powf(base, 0.4);
    (r, 0.4 * XI_SQRT / (r * r.sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::kinds::nursery::{BUBBLE_INTERIOR_MEDIAN, BUBBLE_TEMPERATURE};
    use crate::galaxy::snr::testing::window_table;
    use crate::galaxy::snr::window::{ExplosionEnergy, SiteGas, shell_window};
    use crate::units::{Dex, HydrogenPerCm3, KelvinPerCm3};

    fn window(n: f64, p: f64) -> ShellWindow {
        let site = SiteGas::uniform(HydrogenPerCm3::new(n), KelvinPerCm3::new(p)).unwrap();
        shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0))
    }

    /// The constant is `√2.026`.
    #[test]
    fn the_sedov_constant_is_the_root_of_xi() {
        assert!((XI_SQRT * XI_SQRT - 2.026).abs() < 1e-15);
        // Far into the Sedov–Taylor phase the radius is 1.15 (E t² ÷ ρ)^(1⁄5).
        let (r, _) = sedov_taylor(free_expansion(T_ST).0, 1e6);
        assert!((r / math::powf(1e6, 0.4) - 1.151).abs() < 2e-3, "{r}");
    }

    /// P09.T16.a: the largest radii across the window table, the shell at the end of its window,
    /// are 10–800 ly from 10⁻³ to 10³ cm⁻³. The densest entry, 10⁴ cm⁻³, stays under 10 ly (a
    /// finding: about 6 ly).
    #[test]
    fn radii_across_the_window_table() {
        for (n, w) in window_table(KelvinPerCm3::new(3_800.0)) {
            let end = shell_state_at(&w, Years::new(w.duration().value() * 0.999_999)).unwrap();
            let r = end.radius().value();
            eprintln!(
                "n = {:e} cm⁻³: {r:.1} ly at {:.3e} yr, {:?}",
                n.value(),
                end.age().value(),
                end.phase()
            );
            if n.value() <= 1e3 {
                assert!((10.0..=800.0).contains(&r), "n = {}: {r} ly", n.value());
            } else {
                assert!((3.0..10.0).contains(&r), "n = {}: {r} ly", n.value());
            }
        }
    }

    /// P09.T16.a: the radius never falls with age and is continuous, through every join, at
    /// every density of the table and at the floor.
    #[test]
    fn the_radius_is_monotone_and_continuous() {
        for p in [300.0, 3_800.0] {
            for (n, _) in window_table(KelvinPerCm3::new(p)) {
                let w = window(n.value(), p);
                let evolution = Evolution::of(&w);
                let end = w.duration().value() * SECONDS_PER_JULIAN_YEAR;
                let (mut last_r, mut last_v) = (0.0, f64::INFINITY);
                let mut phases = Vec::new();
                for k in 1..=40_000 {
                    // Logarithmic steps from 10⁻⁶ of the window to its end.
                    let t = end * math::exp10(-6.0 + 6.0 * f64::from(k) / 40_000.0);
                    let (r, v, phase) = evolution.at(t);
                    assert!(r >= last_r, "n = {}: radius fell at {t}", n.value());
                    if last_r > 0.0 {
                        assert!(r / last_r - 1.0 < 2e-3, "n = {}: jump at {t}", n.value());
                        assert!(
                            (v / last_v - 1.0).abs() < 2e-2,
                            "n = {}: speed jump {last_v} → {v} at {t} ({phase:?})",
                            n.value()
                        );
                    }
                    if phases.last() != Some(&phase) {
                        phases.push(phase);
                    }
                    last_r = r;
                    last_v = v;
                }
                assert!(phases.windows(2).all(|w| w[0] < w[1]), "{phases:?}");
            }
        }
    }

    /// The phases and their emission: a shell in warm gas is non-radiative, then a radiative
    /// shell of neutral hydrogen, and shows optical filaments while its shock is fast.
    #[test]
    fn emission_follows_the_phase() {
        let w = window(1.0, 3_800.0);
        let at = |years: f64| shell_state_at(&w, Years::new(years)).unwrap();
        let early = at(100.0);
        assert_eq!(early.phase(), ShellPhase::FreeExpansion);
        assert!(early.emission().x_ray_and_radio() && early.emission().optical());
        assert!(!early.emission().neutral_hydrogen());
        let radiative = at(2e4);
        assert_eq!(radiative.phase(), ShellPhase::PressureDrivenSnowplough);
        assert!(radiative.emission().neutral_hydrogen() && radiative.emission().optical());
        let late = at(8e5);
        assert!(!late.emission().optical() && late.shock_speed().value() < 70.0);
        assert!(shell_state_at(&w, Years::new(-1.0)).is_none());
        // A dense site reaches the momentum-conserving snowplough before its window ends.
        let dense = window(1e4, 3_800.0);
        let end = shell_state_at(&dense, Years::new(dense.duration().value() * 0.999)).unwrap();
        assert_eq!(end.phase(), ShellPhase::MomentumConservingSnowplough);
        // A shell in hot gas never leaves the Sedov–Taylor phase.
        let hot = window(1e-3, 3_800.0);
        let end = shell_state_at(&hot, Years::new(hot.duration().value() * 0.999)).unwrap();
        assert_eq!(end.phase(), ShellPhase::SedovTaylor);
    }

    /// P09.T16.a: a shell in a superbubble's hot interior is gone in about 10⁵ yr, sooner where it
    /// reaches the wall.
    #[test]
    fn a_shell_in_a_bubble_is_gone_in_about_a_hundred_thousand_years() {
        let site = SiteGas::hot_interior(BUBBLE_INTERIOR_MEDIAN, BUBBLE_TEMPERATURE).unwrap();
        let w = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
        let years = w.duration().value();
        assert!((0.5e5..=2e5).contains(&years), "{years}");
        let end = shell_state_at(&w, Years::new(years * 0.999)).unwrap();
        assert!(end.emission().x_ray_and_radio());
        let walled = w.ended_at_wall(LightYears::new(100.0));
        assert!(walled.duration() < w.duration());
        let at_wall = shell_state_at(&walled, Years::new(walled.duration().value() * 0.999_999))
            .unwrap()
            .radius()
            .value();
        assert!((at_wall - 100.0).abs() < 0.01, "{at_wall}");
        assert_eq!(w.ended_at_wall(LightYears::new(10_000.0)), w);
    }
}
