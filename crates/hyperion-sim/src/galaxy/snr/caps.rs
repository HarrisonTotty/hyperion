//! The window's caps: one constant ceiling and the supremum per environment of one galaxy
//! (P09.T15.b, Design note 22).
//!
//! [`SHELL_WINDOW_CAP`] is a constant of the generator version, because plan 08's
//! `explosion_site` prefilters with it before any galaxy is consulted. [`WindowCaps::from_galaxy`]
//! gives the tighter suprema that the catalogue's thinning envelope uses, one per
//! [`ShellEnvironment`], found by a fixed scan once per galaxy, and asserts that none exceeds the
//! constant. The carve-out cuts a window at its environment's cap on every side alike
//! ([`WindowCaps::cut`]), so the prefilter, the cells and the catalogue agree whatever the caps
//! are.
//!
//! The window never rises with the ambient's isothermal sound speed, and a site's `C₀² = s P ÷ ρ`
//! is never below the floor's `P_floor ÷ ρ` at its own density (plan 07's pressure never falls
//! below the floor, and a compressed phase only raises it), so over every gas state a site can
//! have the window is largest at the floor. The field's and the Type Ia's suprema are therefore a
//! scan over density at the floor, at the greatest energy and the least metallicity the window
//! reads. A bubble's interior has a fixed temperature and a log-normal density truncated at two
//! standard deviations (ruling 136.3), whose least value, 1.26 × 10⁻³ cm⁻³, gives the cap.

use crate::galaxy::Galaxy;
use crate::galaxy::features::kinds::nursery::{
    BUBBLE_INTERIOR_MEDIAN, BUBBLE_INTERIOR_SIGMA_DEX, BUBBLE_INTERIOR_TRUNCATION,
    BUBBLE_TEMPERATURE,
};
use crate::galaxy::gas::params::GasParams;
use crate::galaxy::snr::window::{
    ExplosionEnergy, METALLICITY_RANGE_DEX, ShellWindow, SiteGas, shell_window,
};
use crate::math;
use crate::units::{Dex, HydrogenPerCm3, KelvinPerCm3, Years};

/// The ceiling of every shell window: 4 Myr, the top of the brainstorm's "2–4 Myr" (Design note
/// 22). Plan 08 introduced it for `displaced::explosion_site`'s prefilter; it lives here, beside
/// the window it bounds. A parameter of the generator version: it is raised only if a galaxy's
/// supremum exceeds it, which is a finding against the brainstorm.
pub const SHELL_WINDOW_CAP: Years = Years::new(4e6);

/// Where a supernova explodes, as its window's cap reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShellEnvironment {
    /// A core collapse in field gas: the young field and the runaways and walkaways.
    Field,
    /// A Type Ia, in the smooth gas at its site, wherever layer D is.
    TypeIa,
    /// A core collapse inside a nursery's superbubble, in the bubble's hot interior.
    Bubble,
}

impl ShellEnvironment {
    /// Every environment, in declaration order.
    pub const ALL: [Self; 3] = [Self::Field, Self::TypeIa, Self::Bubble];
}

/// The relative margin a scanned supremum is raised by: it covers the golden-section search's
/// tolerance and a site whose floor-derived sound speed rounds a little low.
const CAP_MARGIN: f64 = 1e-6;

/// The densities a floor scan covers, cm⁻³, as decades: 10⁻⁸ to 10⁸. The window falls to nothing
/// at both ends, as `n^(1⁄2)` below and `n^(−18⁄49)` above.
const SCAN_DECADES: (f64, f64) = (-8.0, 8.0);

/// Scan steps over [`SCAN_DECADES`]: 256 a decade.
const SCAN_STEPS: u32 = 4_096;

/// Golden-section steps that refine the scan's best point.
const REFINE_STEPS: u32 = 80;

/// The suprema of the shell window over one galaxy's gas, one per [`ShellEnvironment`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::snr::{SHELL_WINDOW_CAP, ShellEnvironment, WindowCaps};
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let caps = WindowCaps::from_galaxy(&galaxy);
/// let field = caps.cap(ShellEnvironment::Field);
/// assert!(field.value() >= 2e6 && field <= SHELL_WINDOW_CAP);
/// // A bubble's hot interior keeps its shells short.
/// assert!(caps.cap(ShellEnvironment::Bubble) < field);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowCaps {
    field: Years,
    type_ia: Years,
    bubble: Years,
}

impl WindowCaps {
    /// The caps of `galaxy`'s gas.
    ///
    /// # Panics
    ///
    /// If a cap exceeds [`SHELL_WINDOW_CAP`]: a finding against the brainstorm's 2–4 Myr, never
    /// met by the galaxies the parameters allow.
    #[must_use]
    pub fn from_galaxy(galaxy: &Galaxy) -> Self {
        Self::from_gas_params(galaxy.gas().params())
    }

    /// The caps of gas with the parameters `params`.
    ///
    /// # Panics
    ///
    /// As [`from_galaxy`](Self::from_galaxy).
    #[must_use]
    pub fn from_gas_params(params: &GasParams) -> Self {
        Self::at_floor(params.pressure_floor())
    }

    /// The caps of gas whose pressure never falls below `floor`.
    ///
    /// # Panics
    ///
    /// As [`from_galaxy`](Self::from_galaxy), and if `floor` is not positive and finite.
    #[must_use]
    pub fn at_floor(floor: KelvinPerCm3) -> Self {
        let field = floor_supremum(floor);
        let caps = Self {
            field,
            type_ia: field,
            bubble: bubble_supremum(),
        };
        for environment in ShellEnvironment::ALL {
            let cap = caps.cap(environment);
            assert!(
                cap <= SHELL_WINDOW_CAP,
                "the {environment:?} cap, {} yr at a floor of {} K cm⁻³, exceeds SHELL_WINDOW_CAP",
                cap.value(),
                floor.value()
            );
        }
        caps
    }

    /// The supremum of the window in `environment`.
    #[must_use]
    pub const fn cap(&self, environment: ShellEnvironment) -> Years {
        match environment {
            ShellEnvironment::Field => self.field,
            ShellEnvironment::TypeIa => self.type_ia,
            ShellEnvironment::Bubble => self.bubble,
        }
    }

    /// `window` ended at `environment`'s cap, as the carve-out reads every window on every side.
    ///
    /// # Panics
    ///
    /// In debug builds, if the window exceeds the cap: the cap is a supremum, so that is a bug in
    /// the scan or a site outside the gas the scan covers.
    #[must_use]
    pub fn cut(&self, environment: ShellEnvironment, window: ShellWindow) -> ShellWindow {
        let cap = self.cap(environment);
        debug_assert!(
            window.duration() <= cap,
            "a {environment:?} window of {} yr exceeds its cap of {} yr",
            window.duration().value(),
            cap.value()
        );
        window.ended_at(cap)
    }
}

/// The window at density `n` (cm⁻³) at the floor, at the greatest energy and least metallicity.
fn floor_window(n: f64, floor: KelvinPerCm3) -> f64 {
    let site = SiteGas::uniform(HydrogenPerCm3::new(n), floor)
        .expect("a scanned density and a positive, finite floor");
    shell_window(
        &site,
        ExplosionEnergy::MAX,
        Dex::new(METALLICITY_RANGE_DEX.0),
    )
    .duration()
    .value()
}

/// The supremum of [`floor_window`] over density: the best of a fixed scan in `log n`, refined by
/// a fixed number of golden-section steps between its neighbours, raised by [`CAP_MARGIN`].
fn floor_supremum(floor: KelvinPerCm3) -> Years {
    let (lo, hi) = SCAN_DECADES;
    let steps = SCAN_STEPS;
    let at = |k: u32| lo + (hi - lo) * f64::from(k) / f64::from(steps);
    let window = |x: f64| floor_window(math::exp10(x), floor);
    let (best, best_value) =
        (0..=steps)
            .map(|k| (k, window(at(k))))
            .fold(
                (0, f64::NEG_INFINITY),
                |acc, (k, w)| {
                    if w > acc.1 { (k, w) } else { acc }
                },
            );
    let (mut a, mut b) = (at(best.saturating_sub(1)), at((best + 1).min(steps)));
    let ratio = 0.5 * (5.0_f64.sqrt() - 1.0);
    let mut c = b - ratio * (b - a);
    let mut d = a + ratio * (b - a);
    let (mut fc, mut fd) = (window(c), window(d));
    for _ in 0..REFINE_STEPS {
        if fc > fd {
            b = d;
            d = c;
            fd = fc;
            c = b - ratio * (b - a);
            fc = window(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + ratio * (b - a);
            fd = window(d);
        }
    }
    Years::new(best_value.max(fc).max(fd) * (1.0 + CAP_MARGIN))
}

/// The supremum of the window in a superbubble's interior: at its least density, where the hot
/// branch's `W ∝ n^(−1⁄3)` is longest, at the greatest energy (the metallicity drops out of the
/// hot branch).
fn bubble_supremum() -> Years {
    let least = BUBBLE_INTERIOR_MEDIAN.value()
        * math::exp10(-BUBBLE_INTERIOR_SIGMA_DEX * BUBBLE_INTERIOR_TRUNCATION);
    let steps = 1_024_u32;
    let span = 2.0 * BUBBLE_INTERIOR_SIGMA_DEX * BUBBLE_INTERIOR_TRUNCATION;
    let longest = (0..=steps)
        .map(|k| {
            let n = least * math::exp10(span * f64::from(k) / f64::from(steps));
            let site = SiteGas::hot_interior(HydrogenPerCm3::new(n), BUBBLE_TEMPERATURE)
                .expect("a bubble's interior is a gas");
            shell_window(
                &site,
                ExplosionEnergy::MAX,
                Dex::new(METALLICITY_RANGE_DEX.0),
            )
            .duration()
            .value()
        })
        .fold(0.0, f64::max);
    Years::new(longest * (1.0 + CAP_MARGIN))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::Seed;
    use crate::coords::GalacticPosition;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;

    /// P09.T15.b: the field's cap is 2–4 Myr and the bubble's well under it; each is at least
    /// every window the scan could have missed nearby.
    #[test]
    fn the_caps_are_the_brainstorms() {
        let caps = WindowCaps::from_gas_params(&GasParams::milky_way_like());
        let field = caps.cap(ShellEnvironment::Field).value();
        let bubble = caps.cap(ShellEnvironment::Bubble).value();
        eprintln!("caps: field {field:.4e} yr, bubble {bubble:.4e} yr");
        assert!((2e6..=4e6).contains(&field), "{field}");
        assert_eq!(
            caps.cap(ShellEnvironment::TypeIa),
            caps.cap(ShellEnvironment::Field)
        );
        assert!(bubble < field, "{bubble}");
        // Ruling 136.3: about 0.6 Myr, at the truncated interior's least density.
        assert!((0.5e6..=0.65e6).contains(&bubble), "{bubble}");
        // Ruling 136.1: 3.15–3.67 Myr over the floor's range.
        assert!((3.15e6..=3.67e6).contains(&field), "{field}");
        // No density near the peak beats the cap.
        let floor = GasParams::milky_way_like().pressure_floor();
        for k in 0..=10_000 {
            let n = math::exp10(-3.0 + f64::from(k) * 3e-4);
            assert!(floor_window(n, floor) <= field);
        }
    }

    /// The cap at the lowest floor the parameters allow, 300 K cm⁻³, is the largest, and still
    /// under the constant.
    #[test]
    fn the_lowest_floor_gives_the_largest_cap_under_the_constant() {
        let low = WindowCaps::at_floor(KelvinPerCm3::new(300.0)).cap(ShellEnvironment::Field);
        let high = WindowCaps::at_floor(KelvinPerCm3::new(450.0)).cap(ShellEnvironment::Field);
        eprintln!(
            "field caps: {:.4e} yr at 300, {:.4e} at 450",
            low.value(),
            high.value()
        );
        assert!(high < low && low <= SHELL_WINDOW_CAP);
    }

    #[test]
    #[should_panic(expected = "exceeds SHELL_WINDOW_CAP")]
    fn a_cap_above_the_constant_is_refused() {
        let _ = WindowCaps::at_floor(KelvinPerCm3::new(30.0));
    }

    /// A Field window of mid-disc gas, and caps whose bubble cap of 10⁵ yr it exceeds.
    fn window_and_short_bubble_cap() -> (ShellWindow, WindowCaps) {
        let caps = WindowCaps::from_gas_params(&GasParams::milky_way_like());
        let site = SiteGas::uniform(HydrogenPerCm3::new(0.03), KelvinPerCm3::new(400.0)).unwrap();
        let window = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
        let bubble = WindowCaps {
            bubble: Years::new(1e5),
            ..caps
        };
        (window, bubble)
    }

    #[test]
    fn cut_leaves_a_window_within_its_cap() {
        let (window, caps) = window_and_short_bubble_cap();
        assert_eq!(caps.cut(ShellEnvironment::Field, window), window);
    }

    /// Release builds end an over-long window at its cap. Debug builds panic instead, which
    /// `an_over_long_window_panics_in_debug` checks: a `should_panic` test rather than
    /// `catch_unwind`, which aborts the whole test binary on wasm32-wasip1 (panic = abort).
    #[test]
    #[cfg(not(debug_assertions))]
    fn cut_ends_an_over_long_window_at_its_cap() {
        let (window, caps) = window_and_short_bubble_cap();
        let cut = caps.cut(ShellEnvironment::Bubble, window);
        assert_eq!(cut.duration(), Years::new(1e5));
    }

    /// Skipped, not failed, where panics abort (wasm32-wasip1); the other CI architectures run it.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "exceeds its cap of 100000 yr")]
    fn an_over_long_window_panics_in_debug() {
        let (window, caps) = window_and_short_bubble_cap();
        let _ = caps.cut(ShellEnvironment::Bubble, window);
    }

    /// Slow, P09.T15.b: no window of 10⁶ random sites in a galaxy exceeds its cap, for random
    /// energies and metallicities, nor any of 10⁶ random bubble interiors.
    #[test]
    #[ignore = "slow: 10⁶ gas states and 10⁶ bubble interiors"]
    fn no_site_exceeds_its_cap() {
        let galaxy =
            Galaxy::from_params(Seed::new(0x0915_0b01), GalaxyParams::milky_way_like()).unwrap();
        let caps = WindowCaps::from_galaxy(&galaxy);
        let mut lcg = Lcg::new(0x0915_0b02);
        let mut cache = NoiseCache::with_capacity(4_096);
        let (mut longest, mut worst_ratio) = (0.0_f64, 0.0_f64);
        for _ in 0..1_000_000 {
            // Half the sites near the disc, half anywhere in the inner cube.
            let across = [
                lcg.next_f64() * 120_000.0 - 60_000.0,
                lcg.next_f64() * 120_000.0 - 60_000.0,
            ];
            let height = if lcg.next_f64() < 0.5 {
                lcg.next_f64() * 4_000.0 - 2_000.0
            } else {
                lcg.next_f64() * 120_000.0 - 60_000.0
            };
            let position =
                GalacticPosition::from_light_years([across[0], across[1], height]).unwrap();
            let site = SiteGas::at(galaxy.gas(), &position, &mut cache);
            let energy = ExplosionEnergy::from_uniform(lcg.next_f64());
            let metallicity = Dex::new(lcg.next_f64() * 4.0 - 3.0);
            let w = shell_window(&site, energy, metallicity).duration().value();
            longest = longest.max(w);
            for environment in [ShellEnvironment::Field, ShellEnvironment::TypeIa] {
                let cap = caps.cap(environment).value();
                worst_ratio = worst_ratio.max(w / cap);
                assert!(w <= cap, "{site:?}: {w} yr over {cap}");
            }
        }
        let bubble = caps.cap(ShellEnvironment::Bubble).value();
        let mut bubble_longest = 0.0_f64;
        for _ in 0..1_000_000 {
            let z = (lcg.next_f64() * 2.0 - 1.0) * BUBBLE_INTERIOR_TRUNCATION;
            let n = BUBBLE_INTERIOR_MEDIAN.value() * math::exp10(BUBBLE_INTERIOR_SIGMA_DEX * z);
            let site = SiteGas::hot_interior(HydrogenPerCm3::new(n), BUBBLE_TEMPERATURE).unwrap();
            let energy = ExplosionEnergy::from_uniform(lcg.next_f64());
            let w = shell_window(&site, energy, Dex::new(0.0))
                .duration()
                .value();
            bubble_longest = bubble_longest.max(w);
            assert!(w <= bubble, "n = {n}: {w} yr over {bubble}");
        }
        eprintln!(
            "longest field window {longest:.4e} yr ({worst_ratio:.4} of the cap), bubble \
             {bubble_longest:.4e} yr against {bubble:.4e}"
        );
    }

    /// Slow, P09.T15.b: every cap is under the constant for 200 seeds' galaxies and at the corners
    /// of the gas parameters' ranges (the floor at 300 and 450 K cm⁻³, the bubble's temperature
    /// fixed).
    #[test]
    #[ignore = "slow: 200 galaxies' gas parameters"]
    fn every_cap_is_under_the_constant_over_seeds() {
        let (mut least, mut most) = (f64::INFINITY, 0.0_f64);
        for n in 0..200_u64 {
            let seed = Seed::new(0x0915_0c00_0000_0000 | n);
            let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
            let gas = GasParams::from_galaxy(seed, &params).unwrap();
            let cap = WindowCaps::from_gas_params(&gas)
                .cap(ShellEnvironment::Field)
                .value();
            least = least.min(cap);
            most = most.max(cap);
        }
        for floor in [300.0, 450.0] {
            let caps = WindowCaps::at_floor(KelvinPerCm3::new(floor));
            for environment in ShellEnvironment::ALL {
                assert!(caps.cap(environment) <= SHELL_WINDOW_CAP);
            }
        }
        eprintln!("field caps over 200 seeds: {least:.4e}–{most:.4e} yr");
        assert!(most <= SHELL_WINDOW_CAP.value() && least >= 2e6);
    }
}
