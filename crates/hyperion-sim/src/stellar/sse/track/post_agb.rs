//! The post-AGB bridge (plan 06, P06.T16.a; design note 8; ruling 124.2–3): the crossing of a star
//! that has lost its envelope on the asymptotic giant branch, from the giant's temperature to the
//! white dwarf's knee, where the white dwarf's cooling law takes over, and the inflated radius the
//! young white dwarf keeps as it fades.
//!
//! Under [`Bridges::Physical`](super::Bridges::Physical), the generator's, the thermally pulsing
//! AGB keeps its giant's luminosity and radius to the loss of its envelope, without HPT section
//! 6.3's perturbation towards the white dwarf (which, as HPT and the published SSE code have it,
//! fades the star to the white dwarf's luminosity while its last tenth of a solar mass of envelope
//! goes). The bridge replaces it and stays a living star,
//! [`Phase::PostAgb`](crate::stellar::Phase::PostAgb), until the knee, where it dies. Under
//! [`Bridges::Instant`](super::Bridges::Instant) the perturbation stands and the AGB hands over to
//! the white dwarf directly, as in SSE.
//!
//! # The crossing and the knee (ruling 124.2)
//!
//! The plan's "constant L to the knee temperature of the white dwarf" is withdrawn: Miller
//! Bertolami's (2016, A&A 588, A25; CDS J/A+A/588/A25) 24 sequences fade by 0.18–0.54 dex from the
//! end of the AGB to their highest temperature, and knee at 2.2–5.5 times the cold white dwarf's
//! radius. Fitted over all 24, linear in log₁₀ of the final mass `M_f` (research `r-track06`,
//! `kneefit.txt`):
//!
//! - log₁₀ L falls linearly in time from the AGB's `L_AGB` to `L_AGB` − Δ, Δ = 0.2939 −
//!   0.8304 log₁₀ `M_f` (rms 0.065 dex): 0.52 at 0.53 M☉, 0.36 at 0.83 ([`knee_fade_dex`]).
//! - The knee's radius is ρ `R_WD`(0 K), log₁₀ ρ = 0.1863 − 1.8306 log₁₀ `M_f` (rms 0.036 dex): 4.9 at
//!   0.53 M☉, 3.9 at 0.6, 2.2 at 0.83 ([`knee_inflation`]), with `R_WD` the track's remnant recipe's
//!   (HPT's equation 91 by default).
//! - log₁₀ `T_eff` rises linearly in time from the AGB's to 25,000 K at [`ionising_years`] and on to the
//!   knee's, which follows from the two by Stefan–Boltzmann: MB16's log₁₀ `T_max` = 5.8384 +
//!   2.6186 log₁₀ `M_f` to 0.037 dex.
//! - Above 0.83 M☉, the top of MB16's grid, Δ and ρ are held at their 0.83 M☉ values (0.36 and
//!   2.16): the oxygen–neon and heavy carbon–oxygen knees are an extrapolation.
//!
//! After the knee the white dwarf's radius is `R_WD` [1 + (ρ − 1)(L ÷ `L_knee`)^0.4]
//! ([`inflated_radius`]; MB16 at log L = 2 give exponents of 0.29–0.46), so L and R are continuous
//! at the hand-over.
//!
//! # The fade (ruling 127.1)
//!
//! The Montreal cooling law's clock is too slow at its bright end (12.7 times MB16's from log L 2.5
//! to 2, 1.9 times from 2 to 1; Bédard et al. 2020 warn that cooling ages under 10⁵ years depend on
//! their initial models), and agrees below 10 L☉. So a white dwarf that crossed the bridge follows
//! MB16's fade from the knee to log L = 1, over `t₁` ([`fade_years`]): log₁₀ `t₁` = 4.6189 −
//! 3.9917 log₁₀ `M_f` (years; rms 0.088 dex over 22 sequences, `fadefit.txt`; extrapolated above
//! 0.83 M☉). Its shape is MB16's median ([`fade_log_l`]): log L falls linearly in time from the
//! knee's to 2.5 at 10^−3.09 `t₁`, then linearly in log t through 2.0 at 10^−1.53 `t₁`, 1.5 at
//! 10^−0.68 `t₁` and 1.0 at `t₁`. After `t₁` the Montreal law runs from its 10 L☉ point
//! ([`bridged_origin`]): ruling 46.2's match, moved from the knee to 10 L☉. Every other white dwarf
//! keeps the direct match to its star's last luminosity.
//!
//! # The crossing time (ruling 124.3)
//!
//! Both laws are offset power laws, log₁₀ t = a + b log₁₀(M − `M₀`), t in years, over all 24
//! sequences (`crossfit.txt`):
//!
//! - [`crossing_years`] (`τ_tr` + `τ_cross`, from an envelope of 1% of the star to the highest
//!   temperature, MB16 Table 3): `M₀` 0.517, a 2.0690, b −1.4443, rms 0.13 dex (0.28 for the single
//!   power law it replaces); held at 10⁵ years (MB16's slowest is 99 kyr).
//! - [`ionising_years`] (to 25,000 K, from the CDS tracks): `M₀` 0.519, a 2.1550, b −1.1362, rms
//!   0.16 dex; held at 95% of the crossing.
//!
//! MB16's four 0.528–0.534 M☉ sequences reach 25,000 K in 13.7–67 kyr, and the nebula's
//! visibility (16.9–33.9 kyr) then cuts at 0.527–0.534 M☉, the plan's "about 0.53".

use crate::math;
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::structure::white_dwarf_radius;
use crate::stellar::remnant::white_dwarf::{self, WhiteDwarfCore};
use crate::units::{Megayears, MetalFraction, SolarLuminosities, SolarMasses};

/// log₁₀ of the Sun's effective temperature, K.
const LOG_SOLAR_TEMPERATURE: f64 = 3.761_326_322_421_456_6;

/// log₁₀ of the temperature from which a central star ionises its nebula, 25,000 K (plan 06,
/// P06.T16.b).
pub(crate) const LOG_IONISING_TEMPERATURE: f64 = 4.397_940_008_672_037_5;

/// [`crossing_years`]'s (`M₀`, a, b): log₁₀(t ÷ yr) = a + b log₁₀(M − `M₀`).
const CROSSING: [f64; 3] = [0.517, 2.0690, -1.4443];

/// [`ionising_years`]'s (`M₀`, a, b).
const IONISING: [f64; 3] = [0.519, 2.1550, -1.1362];

/// The longest crossing, years: MB16's slowest sequence takes 99 kyr.
const MAX_CROSSING_YEARS: f64 = 1e5;

/// The most of the crossing the time to 25,000 K may take.
const MAX_IONISING_SHARE: f64 = 0.95;

/// The final mass above which [`knee_fade_dex`] and [`knee_inflation`] are held, M☉: the top of
/// MB16's grid.
const KNEE_FIT_MAX_MASS: f64 = 0.83;

/// [`knee_fade_dex`]'s (c₀, c₁): Δ = c₀ + c₁ log₁₀ `M_f`.
const KNEE_FADE: [f64; 2] = [0.2939, -0.8304];

/// [`knee_inflation`]'s (c₀, c₁): log₁₀ ρ = c₀ + c₁ log₁₀ `M_f`.
const KNEE_INFLATION: [f64; 2] = [0.1863, -1.8306];

/// How the young white dwarf's inflation fades with its luminosity: (L ÷ `L_knee`)^0.4.
const INFLATION_EXPONENT: f64 = 0.4;

/// [`fade_years`]'s (c₀, c₁): log₁₀(`t₁` ÷ yr) = c₀ + c₁ log₁₀ `M_f`.
const FADE_TIME: [f64; 2] = [4.6189, -3.9917];

/// The fade's shape after its first, linear piece: (log₁₀(t ÷ `t₁`), log₁₀ L), MB16's medians.
const FADE_SHAPE: [(f64, f64); 4] = [(-3.09, 2.5), (-1.53, 2.0), (-0.68, 1.5), (0.0, 1.0)];

/// The luminosity at the fade's end, where the cooling law takes over, L☉.
pub(crate) const FADE_END_LUMINOSITY: f64 = 10.0;

/// The time a white dwarf of `core` M☉ takes from its knee to 10 L☉, `t₁`, years (see the module).
#[must_use]
pub(crate) fn fade_years(core: f64) -> f64 {
    math::exp10(FADE_TIME[0] + FADE_TIME[1] * math::log10(core))
}

/// log₁₀ L of a white dwarf of `core` M☉ `years` after its knee, where the knee's log₁₀ L is
/// `knee_log_l`, on MB16's median fade, or `None` from `t₁` on, where the cooling law has taken
/// over (see the module). A knee dimmer than the shape's first node, 10^2.5 L☉, starts there.
#[must_use]
pub(crate) fn fade_log_l(knee_log_l: f64, core: f64, years: f64) -> Option<f64> {
    let t1 = fade_years(core);
    if years >= t1 {
        return None;
    }
    let s = years.max(0.0);
    let (first_x, first_l) = FADE_SHAPE[0];
    let first_t = t1 * math::exp10(first_x);
    if s <= first_t {
        return Some(lerp(knee_log_l.max(first_l), first_l, s / first_t));
    }
    let x = math::log10(s / t1);
    let index = FADE_SHAPE[1..FADE_SHAPE.len() - 1]
        .iter()
        .take_while(|&&(node, _)| x >= node)
        .count();
    let ((x0, l0), (x1, l1)) = (FADE_SHAPE[index], FADE_SHAPE[index + 1]);
    Some(lerp(l0, l1, (x - x0) / (x1 - x0)))
}

/// The cooling-law origin of a white dwarf of `core` and `mass` that crossed the bridge: its
/// law's point at 10 L☉ less `t₁`, so that the law at the fade's end gives 10 L☉ (ruling 127.1;
/// `white_dwarf::cooling_origin`, matched at 10 L☉).
#[must_use]
pub(crate) fn bridged_origin(
    recipe: RemnantRecipe,
    core: WhiteDwarfCore,
    mass: SolarMasses,
    z: MetalFraction,
) -> Megayears {
    let at_ten = white_dwarf::cooling_origin(
        recipe,
        core,
        mass,
        Some(SolarLuminosities::new(FADE_END_LUMINOSITY)),
        z,
    );
    Megayears::new(at_ten.value() - fade_years(mass.value()) * 1e-6)
}

/// log₁₀(t ÷ yr) = a + b log₁₀(M − `M₀`), held at `cap`; `cap` at or below `M₀`.
#[must_use]
fn offset_power_law([m0, a, b]: [f64; 3], core: f64, cap: f64) -> f64 {
    if core <= m0 {
        return cap;
    }
    math::exp10(a + b * math::log10(core - m0)).min(cap)
}

/// The post-AGB crossing time of a core of `core` M☉, years: from the loss of the envelope to the
/// knee, at most 10⁵ years (see the module).
#[must_use]
pub(crate) fn crossing_years(core: f64) -> f64 {
    offset_power_law(CROSSING, core, MAX_CROSSING_YEARS)
}

/// The time a core of `core` M☉ takes from the loss of its envelope to 25,000 K, years, held to
/// 95% of [`crossing_years`] (see the module).
#[must_use]
pub(crate) fn ionising_years(core: f64) -> f64 {
    let cap = MAX_IONISING_SHARE * crossing_years(core);
    offset_power_law(IONISING, core, cap)
}

/// The fade in log₁₀ L from the end of the AGB to the knee of a core of `core` M☉, dex: 0.2939 −
/// 0.8304 log₁₀ `M_f`, held above 0.83 M☉ (see the module).
#[must_use]
pub(crate) fn knee_fade_dex(core: f64) -> f64 {
    KNEE_FADE[0] + KNEE_FADE[1] * math::log10(core.min(KNEE_FIT_MAX_MASS))
}

/// The knee's radius over the cold white dwarf's for a core of `core` M☉, ρ: log₁₀ ρ = 0.1863 −
/// 1.8306 log₁₀ `M_f`, held above 0.83 M☉ (see the module).
#[must_use]
pub(crate) fn knee_inflation(core: f64) -> f64 {
    math::exp10(KNEE_INFLATION[0] + KNEE_INFLATION[1] * math::log10(core.min(KNEE_FIT_MAX_MASS)))
}

/// The radius of a young white dwarf of cold radius `cold` (R☉) and core `core` M☉ at luminosity
/// `luminosity`, having left its knee at `knee_luminosity` (both L☉): `R_WD` [1 + (ρ − 1)(L ÷
/// `L_knee`)^0.4], ρ R☉ at the knee and the cold radius as it fades (see the module).
#[must_use]
pub(crate) fn inflated_radius(cold: f64, core: f64, luminosity: f64, knee_luminosity: f64) -> f64 {
    let ratio = (luminosity / knee_luminosity).clamp(0.0, 1.0);
    let fade = if ratio > 0.0 {
        math::powf_positive(ratio, INFLATION_EXPONENT)
    } else {
        0.0
    };
    cold * (1.0 + (knee_inflation(core) - 1.0) * fade)
}

/// One star's crossing: its luminosity from the AGB's to the knee's, and its temperature from the
/// AGB's to the knee's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PostAgb {
    /// log₁₀ L, L☉, at the loss of the envelope and at the knee.
    log_l_start: f64,
    log_l_knee: f64,
    /// log₁₀ `T_eff` at the loss of the envelope and at the knee, K.
    log_t_start: f64,
    log_t_knee: f64,
    /// The time to 25,000 K and the whole crossing, years.
    ionising: f64,
    duration: f64,
}

impl PostAgb {
    /// The crossing of a core of `core` M☉ whose AGB ended with log₁₀ L `log_l` and log₁₀ R
    /// `log_r`, towards the inflated white dwarf of `recipe`'s radius.
    #[must_use]
    pub(crate) fn new(log_l: f64, log_r: f64, core: f64, recipe: RemnantRecipe) -> Self {
        let cold = white_dwarf_radius(recipe, SolarMasses::new(core)).value();
        let knee_radius = math::log10(cold * knee_inflation(core));
        let knee_luminosity = log_l - knee_fade_dex(core);
        Self {
            log_l_start: log_l,
            log_l_knee: knee_luminosity,
            log_t_start: log_temperature(log_l, log_r),
            log_t_knee: log_temperature(knee_luminosity, knee_radius),
            ionising: ionising_years(core),
            duration: crossing_years(core),
        }
    }

    /// The crossing's length, years.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn duration(&self) -> f64 {
        self.duration
    }

    /// log₁₀ L and log₁₀ R at `years` since the loss of the envelope, 0 to the crossing's length.
    #[must_use]
    pub(crate) fn at(&self, years: f64) -> [f64; 2] {
        let s = years.clamp(0.0, self.duration);
        let crosses = self.log_t_start < LOG_IONISING_TEMPERATURE
            && LOG_IONISING_TEMPERATURE < self.log_t_knee;
        let log_t = if !crosses {
            lerp(self.log_t_start, self.log_t_knee, s / self.duration)
        } else if s <= self.ionising {
            lerp(
                self.log_t_start,
                LOG_IONISING_TEMPERATURE,
                s / self.ionising,
            )
        } else {
            lerp(
                LOG_IONISING_TEMPERATURE,
                self.log_t_knee,
                (s - self.ionising) / (self.duration - self.ionising),
            )
        };
        let log_l = lerp(self.log_l_start, self.log_l_knee, s / self.duration);
        [log_l, 0.5 * log_l - 2.0 * (log_t - LOG_SOLAR_TEMPERATURE)]
    }

    /// log₁₀ L and log₁₀ R at the knee.
    #[must_use]
    pub(crate) fn knee(&self) -> [f64; 2] {
        self.at(self.duration)
    }

    /// log₁₀ `T_eff` at the knee, K: the crossing's highest.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn knee_log_t(&self) -> f64 {
        self.log_t_knee
    }
}

/// log₁₀ `T_eff` of a star of log₁₀ L and log₁₀ R: Stefan–Boltzmann.
#[must_use]
fn log_temperature(log_l: f64, log_r: f64) -> f64 {
    LOG_SOLAR_TEMPERATURE + 0.25 * (log_l - 2.0 * log_r)
}

/// (1 − x) a + x b.
#[must_use]
fn lerp(a: f64, b: f64, x: f64) -> f64 {
    (1.0 - x) * a + x * b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K;

    /// Miller Bertolami's (2016) 24 sequences (research `r-track06`, `knee.txt`): final mass,
    /// log₁₀ `L_AGB`, log₁₀ `T_max`, and R ÷ `R_WD` at log L = 2 (`NaN` where the track does not
    /// reach it).
    const MB16: [(f64, f64, f64, f64); 24] = [
        (0.5281, 3.488, 5.059, 2.72),
        (0.5615, 3.730, 5.148, f64::NAN),
        (0.5760, 3.756, 5.187, 2.16),
        (0.5804, 3.877, 5.248, 2.09),
        (0.6573, 4.079, 5.360, 1.72),
        (0.8328, 4.455, 5.576, 1.35),
        (0.5319, 3.284, 5.063, 2.67),
        (0.5660, 3.675, 5.157, 2.19),
        (0.5832, 3.823, 5.221, 2.17),
        (0.5826, 3.854, 5.259, 2.04),
        (0.6160, 3.978, 5.317, 1.84),
        (0.7061, 4.147, 5.430, 1.62),
        (0.5340, 3.442, 5.074, 2.73),
        (0.5517, 3.731, 5.163, 2.40),
        (0.5849, 3.901, 5.255, 2.07),
        (0.5867, 3.962, 5.279, 1.88),
        (0.6182, 4.029, 5.347, 1.74),
        (0.7101, 4.216, 5.473, 1.48),
        (0.8314, 4.341, 5.586, 1.31),
        (0.5328, 3.613, 5.108, 2.21),
        (0.5631, 3.849, 5.233, 2.15),
        (0.6024, 4.002, 5.313, 1.91),
        (0.7130, 4.219, 5.478, 1.48),
        (0.7543, 4.306, 5.531, 1.38),
    ];

    #[test]
    fn the_logarithms_are_the_temperatures_they_name() {
        assert!((LOG_SOLAR_TEMPERATURE - math::log10(SOLAR_EFFECTIVE_TEMPERATURE_K)).abs() < 1e-15);
        assert!((LOG_IONISING_TEMPERATURE - math::log10(25_000.0)).abs() < 1e-15);
    }

    /// The crossing falls steeply with the core, as MB16's Table 3 does (within 0.4 dex at every
    /// one of three cores; the fit's rms is 0.13), is held at 10⁵ years below the offset, and the
    /// time to 25,000 K stays inside it.
    #[test]
    fn the_crossing_falls_steeply_with_the_core() {
        for (core, table) in [(0.5615, 10.06e3), (0.6573, 1.588e3), (0.8328, 0.637e3)] {
            let off = math::log10(crossing_years(core) / table).abs();
            assert!(
                off < 0.4,
                "{core}: {} against {table}",
                crossing_years(core)
            );
        }
        assert!((crossing_years(0.50) - MAX_CROSSING_YEARS).abs() < 1e-9);
        let mut last = f64::INFINITY;
        for k in 0..=200 {
            let core = 0.45 + 0.9 * f64::from(k) / 200.0;
            let t = crossing_years(core);
            assert!(t <= last && t <= MAX_CROSSING_YEARS);
            assert!(ionising_years(core) < t);
            last = t;
        }
    }

    /// Ruling 124.3: MB16's four lazy sequences (0.528–0.534 M☉) reach 25,000 K in 13.7–67 kyr;
    /// the law gives 14–40 kyr across them.
    #[test]
    fn lazy_cores_ionise_slowly() {
        for core in [0.5281, 0.5319, 0.5328, 0.5340] {
            let t = ionising_years(core);
            assert!((1.2e4..7e4).contains(&t), "{core}: {t}");
        }
    }

    /// Ruling 124.2: the knee's temperature is within 0.1 dex of MB16's highest at each of the 24
    /// sequences, from its AGB's luminosity and the cold radius; and the young white dwarf's
    /// R ÷ `R_WD` at log L = 2 is within 25% of MB16's.
    #[test]
    fn the_knee_follows_miller_bertolami() {
        let recipe = RemnantRecipe::MandelMuller2020;
        for (core, log_l_agb, log_t_max, inflation) in MB16 {
            let bridge = PostAgb::new(log_l_agb, 2.3, core, recipe);
            let off = bridge.knee_log_t() - log_t_max;
            assert!(
                off.abs() < 0.1,
                "{core}: knee log T {}",
                bridge.knee_log_t()
            );
            if inflation.is_finite() {
                let [log_l_knee, _] = bridge.knee();
                let ratio = inflated_radius(1.0, core, 100.0, math::exp10(log_l_knee));
                assert!((ratio / inflation - 1.0).abs() < 0.25, "{core}: {ratio}");
            }
        }
    }

    /// Miller Bertolami's (2016) time from the knee to log L = 1 at 22 of his sequences (research
    /// `r-track06`, `fade.txt`): final mass and years.
    const MB16_TO_TEN: [(f64, f64); 22] = [
        (0.5281, 675_062.0),
        (0.5760, 422_177.0),
        (0.6573, 227_179.0),
        (0.8328, 113_288.0),
        (0.5319, 646_119.0),
        (0.5660, 484_953.0),
        (0.5832, 393_921.0),
        (0.5826, 268_689.0),
        (0.6160, 238_912.0),
        (0.7061, 243_487.0),
        (0.5340, 666_371.0),
        (0.5517, 468_316.0),
        (0.5849, 341_262.0),
        (0.5867, 274_176.0),
        (0.6182, 202_513.0),
        (0.7101, 124_335.0),
        (0.8314, 90_307.0),
        (0.5328, 485_704.0),
        (0.5631, 329_913.0),
        (0.6024, 288_979.0),
        (0.7130, 136_695.0),
        (0.7543, 127_393.0),
    ];

    /// Ruling 127.1: the fade's time to log L = 1 is within 0.2 dex of each of MB16's sequences.
    #[test]
    fn the_fade_to_ten_solar_luminosities_follows_miller_bertolami() {
        for (core, years) in MB16_TO_TEN {
            let off = math::log10(fade_years(core) / years);
            assert!(
                off.abs() < 0.2,
                "{core}: {} against {years}",
                fade_years(core)
            );
        }
    }

    /// The fade runs from the knee's luminosity through MB16's median nodes to 10 L☉ at `t₁`,
    /// falling throughout, and hands over there.
    #[test]
    fn the_fade_passes_its_nodes_and_ends_at_ten_solar_luminosities() {
        let (knee, core) = (3.6, 0.6);
        let t1 = fade_years(core);
        assert!((fade_log_l(knee, core, 0.0).unwrap_or(f64::NAN) - knee).abs() < 1e-12);
        for &(x, log_l) in &FADE_SHAPE[..3] {
            let at = fade_log_l(knee, core, t1 * math::exp10(x)).unwrap_or(f64::NAN);
            assert!((at - log_l).abs() < 1e-9, "{x}: {at}");
        }
        let end = fade_log_l(knee, core, t1 * (1.0 - 1e-12)).unwrap_or(f64::NAN);
        assert!((end - 1.0).abs() < 1e-9);
        assert_eq!(fade_log_l(knee, core, t1), None);
        let mut last = f64::INFINITY;
        for k in 0..=1_000 {
            let s = t1 * math::exp10(-6.0 + 6.0 * f64::from(k) / 1_000.0) * (1.0 - 1e-12);
            let log_l = fade_log_l(knee, core, s).unwrap_or(f64::NAN);
            assert!(log_l <= last, "{s}: {log_l}");
            last = log_l;
        }
    }

    /// The crossing fades in L, heats monotonically, passes 25,000 K at the ionising time and ends
    /// at the inflated radius, where the white dwarf's own radius starts.
    #[test]
    fn a_crossing_heats_and_fades_to_the_inflated_white_dwarf() {
        let core = 0.6;
        let bridge = PostAgb::new(3.9, 2.3, core, RemnantRecipe::MandelMuller2020);
        let n = 200;
        let (mut last_t, mut last_l) = (f64::NEG_INFINITY, f64::INFINITY);
        for k in 0..=n {
            let s = bridge.duration() * f64::from(k) / f64::from(n);
            let [log_l, log_r] = bridge.at(s);
            let log_t = log_temperature(log_l, log_r);
            assert!(log_t > last_t && log_l <= last_l, "{s}: {log_t}");
            (last_t, last_l) = (log_t, log_l);
        }
        let [log_l, log_r] = bridge.at(ionising_years(core));
        assert!((log_temperature(log_l, log_r) - LOG_IONISING_TEMPERATURE).abs() < 1e-12);
        let cold =
            white_dwarf_radius(RemnantRecipe::MandelMuller2020, SolarMasses::new(core)).value();
        let [knee_l, knee_r] = bridge.knee();
        assert!((knee_l - (3.9 - knee_fade_dex(core))).abs() < 1e-12);
        let at_knee = inflated_radius(cold, core, math::exp10(knee_l), math::exp10(knee_l));
        assert!((math::log10(at_knee) - knee_r).abs() < 1e-12);
        assert!((bridge.at(0.0)[1] - 2.3).abs() < 1e-12);
        // Faded to 10⁻⁸ of the knee's L, the dwarf is within 1% of the cold radius.
        let faded = inflated_radius(cold, core, 1e-8 * math::exp10(knee_l), math::exp10(knee_l));
        assert!(faded / cold < 1.01);
    }
}
