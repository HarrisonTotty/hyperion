//! A small body's thermal history (plan 14, P14.T51.c; science-composition-crusts §1.4): the peak
//! central temperature of a compact conductive sphere heated by ²⁶Al and the long-lived nuclides
//! from its formation time, which decides between a crust and the body's own `bulk` material
//! ([`super::CrustComposition`]).
//!
//! # The model
//!
//! A sphere of radius R and thermal diffusivity κ, its surface held at `T_s`, starts at `T_s` at
//! its formation time `t_f` and is heated throughout by sources Hₖ e^(−λₖ t), W per kilogram of the
//! body, t counted from `t_f` ([`HeatSource`]). Its central temperature is the eigenfunction
//! series of the uniformly heated sphere, derived from its eigenfunction expansion (cf. Carslaw
//! and Jaeger 1959, Conduction of Heat in Solids, ch. IX, not read; the science check matched it
//! to a finite-difference sphere within 2 × 10⁻⁶):
//!
//! ```text
//! T_c(t) − T_s = Σₖ (Hₖ ÷ c) Σ_{n ≥ 1} 2 (−1)^(n+1) (e^(−λₖ t) − e^(−μₙ t)) ÷ (μₙ − λₖ),
//! μₙ = κ n² π² ÷ R²
//! ```
//!
//! ([`central_temperature`]). Its steady state under a constant source is H R² ÷ (6 κ c), and its
//! early limit the adiabatic Σₖ (Hₖ ÷ c)(1 − e^(−λₖ t)) ÷ λₖ, which bounds it above
//! ([`adiabatic_central_temperature`]). Latent heat and porosity are omitted: the compact body is
//! Henke et al.'s (2012) conservative case, since a porous body conducts less and melts smaller.
//!
//! # The series as summed
//!
//! - **[`SERIES_TERMS`] terms, tapered.** Where few modes have damped (a large body at early
//!   times, μₙ t ≪ 1 for many n) the terms alternate at a near-constant size and the plain partial
//!   sums swing between about 0 and twice the answer. The sum taken is therefore the Euler
//!   transform of order 3 of the last four partial sums, (`S_N` + 3 `S_{N−1}` + 3 `S_{N−2}` + `S_{N−3}`) ÷ 8:
//!   the last three terms weighted 7/8, 1/2 and 1/8 (`taper`). It is exact on the plateau and
//!   holds the constant source's steady state within 3 × 10⁻⁷ at 24 terms.
//! - **Each term's quotient** (e^(−λt) − e^(−μt)) ÷ (μ − λ) is t e^(−λt) (1 − e^(−x)) ÷ x with
//!   x = (μ − λ) t, taken by its series where |x| < [`SMALL_EXPONENT`], so that a mode whose rate
//!   meets a source's (μₙ ≈ λₖ, a 20 km body under ²⁶Al) loses no digits.
//! - **The adiabatic hold.** The sum is held at most the adiabatic limit, which the exact series
//!   never exceeds and the truncated one does, by under 10⁻⁴ of it (6 × 10⁻⁵ at most over radii
//!   of 100 m to 8,000 km and times of 1 yr to 10 Gyr), where few modes have damped by the last
//!   term (a 1 km body at 100 yr, whose centre the surface has not reached).
//! - **The peak** is the greatest value over [`TIME_GRID_POINTS`] times, log-spaced from
//!   [`EARLIEST_GRID_TIME`] to the body's age since its formation, refined by
//!   [`REFINEMENT_STEPS`] golden-section steps in log time about the best of them
//!   ([`peak_central_temperature`]). Against 400 terms on 200 times refined 40 times, over radii
//!   of 1 m to 4,000 km, formation times to 12 Myr and ages to 13 Gyr, its rise above `T_s` is
//!   within 4 × 10⁻⁵ (the lane's check, 2026-10-16).
//!
//! # The sources ([`chondritic_sources`])
//!
//! ²⁶Al first, then [`RadiogenicIsotope::ALL`]'s four, per kilogram of the body's rock, its iron and
//! rock (a chondrite's abundances are per kilogram of the whole meteorite, metal included), and in
//! proportion to that rock's share of the body, so that an icy body's ice dilutes them:
//!
//! - **²⁶Al**: `X_Al` ÷ `M_Al` × `N_A` × (²⁶Al ÷ ²⁷Al)₀ × E × λ e^(−λ `t_f`), with `X_Al` the rock's
//!   aluminium ([`ChondriticRock`]), (²⁶Al ÷ ²⁷Al)₀ [`ALUMINIUM_26_INITIAL_RATIO`], E
//!   [`ALUMINIUM_26_DECAY_ENERGY_MEV`] and λ from [`ALUMINIUM_26_HALF_LIFE`]. It does not scale
//!   with the host's \[Fe/H\]: its ratio is the disc's injection, and its aluminium the rock's.
//! - **²³⁸U, ²³⁵U, ²³²Th and ⁴⁰K**, at the CI chondrites' uranium, thorium and potassium per unit
//!   of aluminium, [`CI_URANIUM_MASS_FRACTION`], [`CI_THORIUM_MASS_FRACTION`] and
//!   [`CI_POTASSIUM_MASS_FRACTION`] over [`CI_ALUMINIUM_MASS_FRACTION`], times the rock's own
//!   aluminium: each isotope's share of its element, half-life and specific heat are
//!   [`RadiogenicIsotope`]'s, so that the crate holds one set of decay data, and its production at
//!   `t_f` is [`radiogenic_heat_production_w_per_kg`]'s at a host age of `t_f`, extrapolated back
//!   from the present (the meteorites' age,
//!   [`RADIOGENIC_REFERENCE_AGE`](crate::planetary::hooks::figures::interior::RADIOGENIC_REFERENCE_AGE)). They scale with
//!   10^\[Fe/H\], as that function's do, so that the crate's long-lived heating reads one rule.
//!
//! # Determinism
//!
//! Fixed term, time and refinement counts; every sum in a declared order (each source in the given
//! order, its modes ascending); every exponential through the crate's [`math`]. No state, no
//! caches.

use crate::math;
use crate::planetary::hooks::figures::interior::{
    BULK_SILICATE_EARTH_POTASSIUM_MASS_FRACTION, BULK_SILICATE_EARTH_THORIUM_MASS_FRACTION,
    BULK_SILICATE_EARTH_URANIUM_MASS_FRACTION, RadiogenicIsotope,
    radiogenic_heat_production_w_per_kg,
};
use crate::substance::Element;
use crate::units::consts::{SECONDS_PER_JULIAN_YEAR, SECONDS_PER_MEGAYEAR};
use crate::units::{Dex, Kelvin, Megayears, Metres, Years};

/// The terms of the eigenfunction series summed, N: 24 ([module](self)'s "The series as summed").
///
/// With the Euler taper they hold the constant source's steady state, (2 ÷ π²) Σ (−1)^(n+1) ÷ n²
/// = (2 ÷ π²)(π² ÷ 12) = 1/6 of H R² ÷ (κ c), within 2.8 × 10⁻⁷; 16 terms would hold it within
/// 2.4 × 10⁻⁶, short of the 10⁻⁶ the steady-state test asks.
pub const SERIES_TERMS: u32 = 24;

/// The times the peak is first sought on: 24, log-spaced from [`EARLIEST_GRID_TIME`] to the age
/// since formation, a step of 0.35 dex over 8 decades.
pub const TIME_GRID_POINTS: u32 = 24;

/// The golden-section steps that refine the peak in log time about the grid's best time and its
/// neighbours: 12, which narrow the bracket by 0.618¹² = 3.1 × 10⁻³ of its two grid steps.
pub const REFINEMENT_STEPS: u32 = 12;

/// The earliest time after formation the grid reads: 100 yr, under a 1 km body's conduction time
/// R² ÷ (π² κ) of 3,200 yr, so that every body that warms measurably peaks after it.
pub const EARLIEST_GRID_TIME: Years = Years::new(100.0);

/// The size of x = (μ − λ) t below which a term's quotient is taken by its series,
/// (1 − e^(−x)) ÷ x = 1 − x ÷ 2 + x² ÷ 6 − x³ ÷ 24, whose next term, x⁴ ÷ 120, is under
/// 10⁻¹⁴ there; above it the difference e^(−λt) − e^(−μt) loses at most three of its digits.
pub const SMALL_EXPONENT: f64 = 1e-3;

/// ²⁶Al's half-life: 0.717 Myr, 717 ± 24 kyr (NUBASE2020: Kondev et al. 2021, Chin. Phys. C 45,
/// 030001, its ²⁶Al ground-state row, after Basunia and Hurst 2016; and the LNHB/DDEP evaluation,
/// Mougeot et al. 2025, Metrologia 62, 039001; both read), a mean life of 1.034 Myr. Henke et al.
/// (2012, A&A 537, A45, Table 4, read) take an e-folding time of 1.0 Myr.
pub const ALUMINIUM_26_HALF_LIFE: Megayears = Megayears::new(0.717);

/// The solar system's initial ²⁶Al ÷ ²⁷Al by number, in the calcium–aluminium-rich inclusions
/// (CAIs): 5.23 ± 0.13 × 10⁻⁵, the CAIs' whole-rock isochron (Jacobsen et al. 2008, EPSL 272,
/// 353, abstract and Fig. 4a, read). Henke et al. (2012, Table 4) take 5.1 × 10⁻⁵, and Hunt et al.
/// (2018) and Neumann et al. (2023) 5.25 × 10⁻⁵ (Kita et al. 2013).
pub const ALUMINIUM_26_INITIAL_RATIO: f64 = 5.23e-5;

/// The heat one ²⁶Al decay leaves in rock, MeV, its neutrinos' energy excluded: 3.123, its decay
/// energy Q⁺ = 4,004.4 keV less the neutrinos' mean 881 keV, from LNHB's emission tables (Mougeot
/// et al. 2025, read; the lane's sum, 2026-10-16). Neumann et al. (2023, PSJ 4, 196, Table 5,
/// read) take 4.99 × 10⁻¹³ J, 3.115 MeV; Henke et al.'s (2012, Table 4) 3.188 MeV, after Finocchi
/// and Gail 1997, is 2.1% higher. With [`ALUMINIUM_26_INITIAL_RATIO`] it gives an H chondrite
/// 2.02 × 10⁻⁷ W kg⁻¹ at the CAIs' time, the ruling's check (science-composition-crusts §1.4).
pub const ALUMINIUM_26_DECAY_ENERGY_MEV: f64 = 3.123;

/// An ordinary (H) chondrite's aluminium, kg per kg: 1.13 wt%, "the same Al abundance per unit
/// mass (1.13 wt%)" (Ghosh and McSween 1999, MAPS 34, 121, p. 122, read); Jarosewich's (1990,
/// Meteoritics 25, 323, Table 4, read) 26 H falls hold `Al₂O₃` 2.14 ± 0.15 wt%, 1.133 wt% of Al.
/// The rock of a body formed inside its host's snow line.
pub const ORDINARY_CHONDRITE_ALUMINIUM_MASS_FRACTION: f64 = 0.0113;

/// A carbonaceous chondrite's aluminium on an anhydrous basis, kg per kg of rock: 1.2 wt%, the
/// rock of a body formed beyond its host's snow line, whose ice is counted apart from it.
///
/// No source read states it. It is the lane's, between the anhydrous CI's 1.03–1.16 wt% (Lodders,
/// Palme and Gail 2009's CI table less its water, carbon, nitrogen and sulphur) and the CM2s'
/// 1.28 water-free (Jarosewich 1990, Table 4: `Al₂O₃` 2.17 wt% as analysed); Neumann et al. (2023,
/// Table 5, read) take 1.21 wt% for their protolith.
pub const CARBONACEOUS_ROCK_ALUMINIUM_MASS_FRACTION: f64 = 0.012;

/// The CI chondrites' aluminium, kg per kg: 8,500 ppm ± 3% (Lodders, Palme and Gail 2009,
/// Landolt-Börnstein VI/4B, 712, arXiv:0901.1149, Table 3; Lodders 2010, arXiv:1010.2746; both
/// read), the unit the long-lived nuclides' abundances are taken per.
pub const CI_ALUMINIUM_MASS_FRACTION: f64 = 8_500e-6;

/// The CI chondrites' potassium, kg per kg: 544 ppm ± 5% (Lodders, Palme and Gail 2009, Table 3;
/// Lodders 2010; both read), as measured today.
pub const CI_POTASSIUM_MASS_FRACTION: f64 = 544e-6;

/// The CI chondrites' thorium, kg per kg: 31.0 ppb ± 8% (Lodders, Palme and Gail 2009, Table 3;
/// Lodders 2010; both read), as measured today.
pub const CI_THORIUM_MASS_FRACTION: f64 = 31.0e-9;

/// The CI chondrites' uranium, kg per kg: 8.10 ppb ± 8% (Lodders, Palme and Gail 2009, Table 3;
/// Lodders 2010; both read), as measured today. Henke et al.'s (2012, Table 2) and Neumann et
/// al.'s (2023) H-chondrite K, Th and U are these extrapolated back to the CAIs, with initial
/// isotope ratios, so are not taken here beside present-day ones.
pub const CI_URANIUM_MASS_FRACTION: f64 = 8.10e-9;

/// The Avogadro constant, mol⁻¹: 6.022 140 76 × 10²³, exact in the SI since 2019.
const AVOGADRO_PER_MOL: f64 = 6.022_140_76e23;

/// One mega-electronvolt, J: 1.602 176 634 × 10⁻¹³, exact in the SI since 2019.
const JOULES_PER_MEV: f64 = 1.602_176_634e-13;

/// A source of heat in a body, W per kilogram of the body at its formation, decaying at its own
/// rate: H e^(−λ t), t from the formation ([module](self)).
///
/// Plain data with public fields: the series reads any finite values, a production of zero adding
/// nothing, a decay rate of zero holding the source constant and a negative one growing it (no
/// source here grows).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatSource {
    /// Its heat production at the body's formation, H, W per kilogram of the body.
    pub production_w_per_kg: f64,
    /// Its decay rate λ, s⁻¹, at least 0: ln 2 over its half-life.
    pub decay_rate_per_s: f64,
}

/// A compact conductive sphere, the body the thermal history models ([module](self)).
///
/// Plain data with public fields; [`super::CrustComposition::derive`] builds it from validated
/// inputs, and the series answers no rise for a time that is not positive. A radius,
/// diffusivity or specific heat that is not finite and positive gives no meaningful value (an
/// infinite or NaN rise), which no caller here passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConductiveSphere {
    /// Its radius R, positive.
    pub radius: Metres,
    /// Its thermal diffusivity κ, m² s⁻¹, positive.
    pub diffusivity_m2_per_s: f64,
    /// Its specific heat c, J kg⁻¹ K⁻¹, positive.
    pub specific_heat_j_per_kg_k: f64,
    /// Its surface temperature `T_s`, at which it starts and its surface is held.
    pub surface_temperature: Kelvin,
}

/// The rock a body's chondritic sources are taken for, by where it formed: its aluminium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChondriticRock {
    /// An ordinary chondrite's, inside the snow line:
    /// [`ORDINARY_CHONDRITE_ALUMINIUM_MASS_FRACTION`].
    Ordinary,
    /// A carbonaceous chondrite's on an anhydrous basis, beyond it:
    /// [`CARBONACEOUS_ROCK_ALUMINIUM_MASS_FRACTION`].
    Carbonaceous,
}

impl ChondriticRock {
    /// Its aluminium, kg per kg of rock.
    #[must_use]
    pub const fn aluminium_mass_fraction(self) -> f64 {
        match self {
            Self::Ordinary => ORDINARY_CHONDRITE_ALUMINIUM_MASS_FRACTION,
            Self::Carbonaceous => CARBONACEOUS_ROCK_ALUMINIUM_MASS_FRACTION,
        }
    }
}

/// ²⁶Al's decay rate λ, s⁻¹: ln 2 over [`ALUMINIUM_26_HALF_LIFE`].
#[must_use]
pub fn aluminium_26_decay_rate_per_s() -> f64 {
    core::f64::consts::LN_2 / (ALUMINIUM_26_HALF_LIFE.value() * SECONDS_PER_MEGAYEAR)
}

/// The heat ²⁶Al makes in a kilogram of rock holding `aluminium` kg of aluminium at the
/// inclusions' time (CAI), W kg⁻¹: `X_Al` ÷ `M_Al` × `N_A` × (²⁶Al ÷ ²⁷Al)₀ × E × λ.
///
/// # Examples
///
/// An H chondrite makes 2.02 × 10⁻⁷ W kg⁻¹ (science-composition-crusts §1.4's check):
///
/// ```
/// use hyperion_sim::planetary::hooks::crust::history::{
///     ORDINARY_CHONDRITE_ALUMINIUM_MASS_FRACTION, aluminium_26_production_w_per_kg,
/// };
///
/// let p0 = aluminium_26_production_w_per_kg(ORDINARY_CHONDRITE_ALUMINIUM_MASS_FRACTION);
/// assert!((p0 / 2.02e-7 - 1.0).abs() < 0.005);
/// ```
#[must_use]
pub fn aluminium_26_production_w_per_kg(aluminium: f64) -> f64 {
    let moles = aluminium / (Element::Al.standard_atomic_weight_g_per_mol() * 1e-3);
    moles
        * AVOGADRO_PER_MOL
        * ALUMINIUM_26_INITIAL_RATIO
        * ALUMINIUM_26_DECAY_ENERGY_MEV
        * JOULES_PER_MEV
        * aluminium_26_decay_rate_per_s()
}

/// The number of chondritic sources: ²⁶Al and [`RadiogenicIsotope::ALL`]'s four.
pub const CHONDRITIC_SOURCES: usize = 5;

/// The heat sources of a body formed at `formation` (Myr after its host's onset of collapse, the
/// inclusions' time) of rock `rock`, which is `rock_share` of its mass, about a host of \[Fe/H\]
/// `fe_h`: ²⁶Al, then ²³⁸U, ²³⁵U, ²³²Th and ⁴⁰K ([module](self)'s "The sources").
///
/// # Examples
///
/// A dry body formed 2 Myr after the inclusions holds ²⁶Al at a seventh of its first production,
/// still a hundred times its long-lived nuclides':
///
/// ```
/// use hyperion_sim::planetary::hooks::crust::history::{ChondriticRock, chondritic_sources};
/// use hyperion_sim::units::{Dex, Megayears};
///
/// let [al, u238, u235, th, k] = chondritic_sources(Megayears::new(2.0), ChondriticRock::Ordinary, 1.0, Dex::new(0.0));
/// let long_lived = u238.production_w_per_kg + u235.production_w_per_kg + th.production_w_per_kg + k.production_w_per_kg;
/// assert!(al.production_w_per_kg > 100.0 * long_lived);
/// ```
#[must_use]
pub fn chondritic_sources(
    formation: Megayears,
    rock: ChondriticRock,
    rock_share: f64,
    fe_h: Dex,
) -> [HeatSource; CHONDRITIC_SOURCES] {
    let aluminium = rock.aluminium_mass_fraction();
    let decay = aluminium_26_decay_rate_per_s();
    let since_inclusions = formation.value() * SECONDS_PER_MEGAYEAR;
    let aluminium_26 = HeatSource {
        production_w_per_kg: aluminium_26_production_w_per_kg(aluminium)
            * math::exp(-decay * since_inclusions)
            * rock_share,
        decay_rate_per_s: decay,
    };
    let per_aluminium = aluminium / CI_ALUMINIUM_MASS_FRACTION;
    let at_formation = radiogenic_heat_production_w_per_kg(fe_h, Years::from(formation));
    let mut sources = [aluminium_26; CHONDRITIC_SOURCES];
    for ((source, isotope), production) in sources[1..]
        .iter_mut()
        .zip(RadiogenicIsotope::ALL)
        .zip(at_formation)
    {
        let ci_over_bse = match isotope {
            RadiogenicIsotope::Uranium238 | RadiogenicIsotope::Uranium235 => {
                CI_URANIUM_MASS_FRACTION / BULK_SILICATE_EARTH_URANIUM_MASS_FRACTION
            }
            RadiogenicIsotope::Thorium232 => {
                CI_THORIUM_MASS_FRACTION / BULK_SILICATE_EARTH_THORIUM_MASS_FRACTION
            }
            RadiogenicIsotope::Potassium40 => {
                CI_POTASSIUM_MASS_FRACTION / BULK_SILICATE_EARTH_POTASSIUM_MASS_FRACTION
            }
        };
        *source = HeatSource {
            production_w_per_kg: production * ci_over_bse * per_aluminium * rock_share,
            decay_rate_per_s: 1.0 / (isotope.mean_life().value() * SECONDS_PER_JULIAN_YEAR),
        };
    }
    sources
}

/// The weight of term `n` of `terms` in the tapered sum: 1, then 7/8, 1/2 and 1/8 for the last
/// three, the Euler transform of order 3 of the last four partial sums ([module](self)).
#[must_use]
const fn taper(n: u32, terms: u32) -> f64 {
    match terms.saturating_sub(n) {
        0 => 0.125,
        1 => 0.5,
        2 => 0.875,
        _ => 1.0,
    }
}

/// (1 − e^(−x)) ÷ x by its series, 1 − x ÷ 2 + x² ÷ 6 − x³ ÷ 24, for |x| < [`SMALL_EXPONENT`].
#[must_use]
fn small_share(x: f64) -> f64 {
    1.0 - x / 2.0 + x * x / 6.0 - x * x * x / 24.0
}

/// (e^(−λt) − e^(−μt)) ÷ (μ − λ), given `source_decay` = e^(−λt) and `mode_decay` = e^(−μt).
#[must_use]
fn mode_share(lambda: f64, mu: f64, t: f64, source_decay: f64, mode_decay: f64) -> f64 {
    let x = (mu - lambda) * t;
    if x.abs() < SMALL_EXPONENT {
        t * source_decay * small_share(x)
    } else {
        (source_decay - mode_decay) / (mu - lambda)
    }
}

/// (1 − e^(−λt)) ÷ λ, the adiabatic share of a source of rate `lambda` at `t`, given
/// `source_decay` = e^(−λt); t for a constant source.
#[must_use]
fn adiabatic_share(lambda: f64, t: f64, source_decay: f64) -> f64 {
    let x = lambda * t;
    if x.abs() < SMALL_EXPONENT {
        t * small_share(x)
    } else {
        (1.0 - source_decay) / lambda
    }
}

/// The central rise `T_c` − `T_s` of `sphere` under `sources` at `t` seconds after formation,
/// summed over `terms` tapered terms and held at most the adiabatic limit; zero at a time that is
/// not positive.
///
/// The exact series never exceeds the adiabatic limit, and the truncated one does where few modes
/// have damped by the last term, by under 10⁻⁴ of it (a 1 km body at 100 yr, whose centre the
/// surface has not yet reached): the hold takes the exact value there.
#[must_use]
pub(crate) fn central_rise_with_terms(
    sphere: &ConductiveSphere,
    sources: &[HeatSource],
    t: f64,
    terms: u32,
) -> f64 {
    let (series, bound) = series_and_bound(sphere, sources, t, terms);
    if series > bound { bound } else { series }
}

/// The tapered series of `terms` terms for the central rise of `sphere` under `sources` at `t`
/// seconds after formation, unheld, and the adiabatic limit it is held to
/// ([`central_rise_with_terms`]); both zero at a time that is not positive.
#[must_use]
pub(crate) fn series_and_bound(
    sphere: &ConductiveSphere,
    sources: &[HeatSource],
    t: f64,
    terms: u32,
) -> (f64, f64) {
    if t.is_nan() || t <= 0.0 {
        return (0.0, 0.0);
    }
    let r = sphere.radius.value();
    let pi2 = core::f64::consts::PI * core::f64::consts::PI;
    let mu1 = sphere.diffusivity_m2_per_s * pi2 / (r * r);
    let first = math::exp(-mu1 * t);
    let first2 = first * first;
    sources.iter().fold((0.0, 0.0), |(series, bound), source| {
        let lambda = source.decay_rate_per_s;
        let source_decay = math::exp(-lambda * t);
        // e^(−μₙ t) = q^(n²), q = e^(−μ₁ t), by q^((n+1)²) = q^(n²) q^(2n+1).
        let (mut step, mut mode_decay) = (first, first);
        let mut modes = 0.0;
        for n in 1..=terms {
            let mu = mu1 * f64::from(n) * f64::from(n);
            let sign = if n % 2 == 1 { 2.0 } else { -2.0 };
            modes += taper(n, terms) * sign * mode_share(lambda, mu, t, source_decay, mode_decay);
            step *= first2;
            mode_decay *= step;
        }
        let scale = source.production_w_per_kg / sphere.specific_heat_j_per_kg_k;
        (
            series + scale * modes,
            bound + scale * adiabatic_share(lambda, t, source_decay),
        )
    })
}

/// The central temperature of `sphere` under `sources` at `since` after its formation: `T_s`
/// plus the tapered series of [`SERIES_TERMS`] terms ([module](self)); `T_s` at a time that is not
/// positive.
///
/// # Examples
///
/// A 100 km body under a constant source reaches its steady state, H R² ÷ (6 κ c), once its modes
/// have damped:
///
/// ```
/// use hyperion_sim::planetary::hooks::crust::history::{ConductiveSphere, HeatSource, central_temperature};
/// use hyperion_sim::units::{Kelvin, Metres, Years};
///
/// let sphere = ConductiveSphere {
///     radius: Metres::new(100e3),
///     diffusivity_m2_per_s: 1e-6,
///     specific_heat_j_per_kg_k: 850.0,
///     surface_temperature: Kelvin::new(200.0),
/// };
/// let constant = [HeatSource { production_w_per_kg: 1e-11, decay_rate_per_s: 0.0 }];
/// let steady = 1e-11 * 100e3 * 100e3 / (6.0 * 1e-6 * 850.0);
/// let late = central_temperature(&sphere, &constant, Years::new(1e10));
/// assert!((late.value() - 200.0 - steady).abs() < 1e-6 * steady);
/// ```
#[must_use]
pub fn central_temperature(
    sphere: &ConductiveSphere,
    sources: &[HeatSource],
    since: Years,
) -> Kelvin {
    let t = since.value() * SECONDS_PER_JULIAN_YEAR;
    Kelvin::new(
        sphere.surface_temperature.value()
            + central_rise_with_terms(sphere, sources, t, SERIES_TERMS),
    )
}

/// The adiabatic limit of `sphere`'s centre under `sources` at `since` after its formation, the
/// series' bound above: `T_s` + Σₖ (Hₖ ÷ c)(1 − e^(−λₖ t)) ÷ λₖ, and Hₖ t ÷ c for a constant
/// source.
#[must_use]
pub fn adiabatic_central_temperature(
    sphere: &ConductiveSphere,
    sources: &[HeatSource],
    since: Years,
) -> Kelvin {
    let t = since.value() * SECONDS_PER_JULIAN_YEAR;
    let t = if t > 0.0 { t } else { 0.0 };
    let rise = sources.iter().fold(0.0, |rise, source| {
        let lambda = source.decay_rate_per_s;
        // A rate of either sign through its exponential, and a constant source exactly.
        let held = if lambda.abs() > 0.0 {
            -math::exp_m1(-lambda * t) / lambda
        } else {
            t
        };
        rise + source.production_w_per_kg / sphere.specific_heat_j_per_kg_k * held
    });
    Kelvin::new(sphere.surface_temperature.value() + rise)
}

/// The golden-section ratio, (√5 − 1) ÷ 2.
const GOLDEN: f64 = 0.618_033_988_749_894_9;

/// The peak central temperature `T_c,max` of `sphere` under `sources` over the `duration` since
/// its formation: the greatest of the series on [`TIME_GRID_POINTS`] times log-spaced from
/// [`EARLIEST_GRID_TIME`] (or the duration, if shorter) to the duration, refined by
/// [`REFINEMENT_STEPS`] golden-section steps in log time ([module](self)); `T_s` for a duration
/// that is not positive.
///
/// # Examples
///
/// Henke et al.'s 100 km H-chondrite body formed 2.3 Myr after the inclusions stays below the
/// chondritic solidus, 1,423 K, where a 25 km body formed at 0.1 Myr melts (Neumann et al.
/// 2023):
///
/// ```
/// use hyperion_sim::planetary::hooks::crust::history::{
///     ChondriticRock, ConductiveSphere, DRY_ROCK_SPECIFIC_HEAT_J_PER_KG_K, chondritic_sources,
///     peak_central_temperature,
/// };
/// use hyperion_sim::planetary::hooks::figures::interior::ROCK_THERMAL_DIFFUSIVITY_M2_PER_S;
/// use hyperion_sim::units::{Dex, Kelvin, Megayears, Metres, Years};
///
/// let peak = |radius_km: f64, formed_myr: f64| {
///     let sphere = ConductiveSphere {
///         radius: Metres::new(radius_km * 1e3),
///         diffusivity_m2_per_s: ROCK_THERMAL_DIFFUSIVITY_M2_PER_S,
///         specific_heat_j_per_kg_k: DRY_ROCK_SPECIFIC_HEAT_J_PER_KG_K,
///         surface_temperature: Kelvin::new(200.0),
///     };
///     let sources = chondritic_sources(Megayears::new(formed_myr), ChondriticRock::Ordinary, 1.0, Dex::new(0.0));
///     peak_central_temperature(&sphere, &sources, Years::new(4.57e9)).value()
/// };
/// assert!(peak(100.0, 2.3) < 1_423.15);
/// assert!(peak(25.0, 0.1) > 1_423.15);
/// ```
#[must_use]
pub fn peak_central_temperature(
    sphere: &ConductiveSphere,
    sources: &[HeatSource],
    duration: Years,
) -> Kelvin {
    let end = duration.value() * SECONDS_PER_JULIAN_YEAR;
    let surface = sphere.surface_temperature.value();
    if end.is_nan() || end <= 0.0 {
        return Kelvin::new(surface);
    }
    let earliest = EARLIEST_GRID_TIME.value() * SECONDS_PER_JULIAN_YEAR;
    let (low, high) = (
        math::ln(if earliest < end { earliest } else { end }),
        math::ln(end),
    );
    let last = TIME_GRID_POINTS - 1;
    let at = |i: u32| low + (high - low) * f64::from(i) / f64::from(last);
    let rise =
        |log_t: f64| central_rise_with_terms(sphere, sources, math::exp(log_t), SERIES_TERMS);
    let (mut best, mut best_i) = (0.0, 0);
    for i in 0..TIME_GRID_POINTS {
        let r = rise(at(i));
        if r > best {
            (best, best_i) = (r, i);
        }
    }
    // Golden-section search on the bracket of the best grid time's neighbours.
    let (mut a, mut b) = (at(best_i.saturating_sub(1)), at((best_i + 1).min(last)));
    let mut x1 = b - GOLDEN * (b - a);
    let mut x2 = a + GOLDEN * (b - a);
    let (mut f1, mut f2) = (rise(x1), rise(x2));
    for _ in 0..REFINEMENT_STEPS {
        if f1 > f2 {
            (b, x2, f2) = (x2, x1, f1);
            x1 = b - GOLDEN * (b - a);
            f1 = rise(x1);
        } else {
            (a, x1, f1) = (x1, x2, f2);
            x2 = a + GOLDEN * (b - a);
            f2 = rise(x2);
        }
        for f in [f1, f2] {
            if f > best {
                best = f;
            }
        }
    }
    Kelvin::new(surface + best)
}

/// The specific heat of a dry body's rock, c: 800 J kg⁻¹ K⁻¹, an H chondrite's near its solidus,
/// where the melting test reads the history: 630 at 300 K and 800 at 1,100 K, "the average
/// specific heat capacity of H chondrites may be said to be ~700–800", and about 800 for L and LL
/// (Ghosh and McSween 1999, MAPS 34, 121, Table 1 and p. 122, after Robie et al. 1978; read).
///
/// The ruling's check takes 850 (science-composition-crusts §1.4); its table is reproduced at its
/// own c (this module's tests).
pub const DRY_ROCK_SPECIFIC_HEAT_J_PER_KG_K: f64 = 800.0;

/// The specific heat of an ice–rock mixture's ice, c: 866.3 J kg⁻¹ K⁻¹, ice Ih's at 100 K (IAPWS
/// R10-06(2009), Table 6, 866.333 at 100 K and 100 MPa, read; its Gibbs function gives 874.1 at
/// 611 Pa, the lane's evaluation, and 2,096.8 at the triple point).
///
/// `Fitted`: with [`COLD_ROCK_SPECIFIC_HEAT_J_PER_KG_K`], the one calibration of the thermal
/// history, the low-temperature ends of its parts' ranges, which pass Bland and Travis's anchor (an
/// icy 100 km body of water-to-rock ratio 0.6 melts its ice at 3 Myr and not after 4 Myr) at
/// Ceres's present equilibrium temperature. The parts at the history's own 160–273 K, about 1,000
/// for the mixture, miss it (the ruling's finding; plan 14's record of P14.T51.c).
pub const ICE_SPECIFIC_HEAT_J_PER_KG_K: f64 = 866.3;

/// The specific heat of an ice–rock mixture's rock, c: 556 J kg⁻¹ K⁻¹, chondrites' at 200 K by the
/// fit "between 75 and 250 K … Cp = −8.1 + 2.82 T" (Flynn et al. 2018, Geochemistry 78, 269, §4.2,
/// read), whose ordinary chondrites average 494 over 77–294 K (Consolmagno et al. 2013, PSS 87,
/// 146, as Flynn et al. quote them). `Fitted`, with [`ICE_SPECIFIC_HEAT_J_PER_KG_K`].
pub const COLD_ROCK_SPECIFIC_HEAT_J_PER_KG_K: f64 = 556.0;

/// The specific heat of a body formed beyond its host's snow line holding `ice` of its mass as
/// ice: its parts' at low temperature by mass, `ice` × [`ICE_SPECIFIC_HEAT_J_PER_KG_K`] + (1 −
/// `ice`) × [`COLD_ROCK_SPECIFIC_HEAT_J_PER_KG_K`], J kg⁻¹ K⁻¹.
#[must_use]
pub fn ice_rock_specific_heat_j_per_kg_k(ice: f64) -> f64 {
    ice * ICE_SPECIFIC_HEAT_J_PER_KG_K + (1.0 - ice) * COLD_ROCK_SPECIFIC_HEAT_J_PER_KG_K
}
