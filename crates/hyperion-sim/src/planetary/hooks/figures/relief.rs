//! A body's relief (plan 14, P14.T48.a, with P14.T24.b): its hypsometric standard deviation `σ_h`,
//! its continental fraction `f_c` and its volcanism level V, with the hypsometry its oceans fill
//! (science-p14-t24b-laws §4.1–4.2; rendering plan R09's Design note 3).
//!
//! # The relief (P14.T48.a)
//!
//! `σ_h`, the area-weighted RMS elevation about the mean relative to the reference equipotential,
//! degree 1 included, is the sum in quadrature of three shares ([`Relief::of`]):
//!
//! - **structural**, with no gravity term: on a mobile lid `f_c` (1 − `f_c`) Δ² + `f_c` `σ_c`² +
//!   (1 − `f_c`) `σ_o`², from Earth2014's two crustal populations ([`CONTINENTAL_OFFSET`],
//!   [`CONTINENTAL_WIDTH`], [`OCEANIC_WIDTH`]; 2.51 km at `f_c` = 0.405), and on every other lid
//!   [`STAGNANT_LID_RELIEF`], Venus's;
//! - **basin**, ∝ 1 ÷ g: [`BASIN_RELIEF`] × ([`BASIN_REFERENCE_GRAVITY`] ÷ g) × `k_comp` ×
//!   √min(1, N☾(>1 km) ÷ [`BASIN_SATURATION_DENSITY`]), `k_comp` 1 on a silicate crust and
//!   [`ICE_RICH_BASIN_FACTOR`] on an ice-rich one ([`Crust`]), capped at [`BASIN_RELIEF_CAP`] of
//!   the radius;
//! - **constructional**, ∝ 1 ÷ g and gated by the lithosphere: [`CONSTRUCTIONAL_RELIEF`] ×
//!   ([`CONSTRUCTIONAL_REFERENCE_GRAVITY`] ÷ g) × V × min(1, `T_e` ÷ [`SUPPORTING_LITHOSPHERE`]),
//!   `T_e` the lithosphere's thickness at today's heat flow ([`lithosphere_thickness`]).
//!
//! Every constant calibrates on one body (Earth the mobile lid's terms, Venus the stagnant lid's,
//! the Moon the basin share, Mars the constructional share, Ceres `k_comp` and Vesta the cap), so
//! the fit is empirical; Mercury, the one body out of the sample, comes out 21% high (1.27 against
//! 1.05 km). The greatest relief is withdrawn as a published figure: R09's field reports its own
//! realised relief, which lies at 7–15 `σ_h` on every body measured.
//!
//! # The continents (P14.T48.a)
//!
//! On a mobile lid of a world that entered P14.T48.b's water window holding water
//! ([`WindowEntry`]), `f_c` = f∞ (1 − e^(−(t − `t_p`) ÷ τ)) after [`CONTINENTAL_GROWTH_ONSET`] from
//! the body's formation and zero before, with τ [`CONTINENTAL_GROWTH_TIMESCALE`] and the asymptote
//! f∞ drawn once per body, uniform on [0.2, 0.7] ([`continental_asymptote`]);
//! zero on every other lid. Höning and Spohn 2023 (Astrobiology 23(4), 372–394) find continental
//! growth on an Earth-sized plate-tectonic planet history-dependent: an ocean planet near 20%, an
//! Earth-like one near 40% and a land planet near 70% after 4.5 Gyr. The Solar System table gives
//! Earth its measured [`EARTH_CONTINENTAL_FRACTION`] ([`Continents::Given`]).
//!
//! # The volcanism level (science-p14-t24b-laws §4.1)
//!
//! V = (I ÷ I♂) × ((g `D_m`³) ÷ (g `D_m`³)♂)^1.5 ([`volcanism_level`]): I the melt the surface's
//! own life has erupted while its lithosphere held it, ∫ over the last S of max(0, F(t) − `F_c`) ×
//! min(1, `T_e`(t) ÷ 70 km) dt, by 64-point Gauss–Legendre; g `D_m`³ the plume vigour, a mantle
//! Rayleigh number at fixed viscosity and temperature contrast, with `D_m` = R (1 − √x) the
//! mantle's depth over a core of iron share x; ♂ the Solar System table's Mars, so that its V is 1.
//! It is zero on an ice-rich crust, whose cryovolcanic constructs are not modelled, and held to
//! [`VOLCANISM_SATURATION`], 1.984, the strength limit of the constructional share (§4.x.1).
//!
//! # The hypsometry (science-p14-t24b-laws §4.2)
//!
//! The populations whose untruncated variance is `σ_h`², each a Gaussian truncated at ±3σ, which
//! keeps 0.973 of it ([`Hypsometry`]): on
//! a mobile lid the continental (`f_c`, (1 − `f_c`) Δ, `σ_c′`) and the oceanic (1 − `f_c`,
//! −`f_c` Δ, `σ_o′`), the basin and constructional shares in quadrature in each, `σ_c′`² =
//! `σ_c`² + `σ_crat`² + `σ_volc`²; on any other lid one of `σ_h`. Their mean is the datum, zero.

use crate::Seed;
use crate::id::BodyId;
use crate::math;
use crate::planetary::hooks::figures::{Crust, TectonicRegime, WindowEntry};
use crate::planetary::moons::regular::ICE_CONDUCTIVITY_COEFFICIENT;
use crate::planetary::params::SURFACE_MELT_HEAT_FLOW;
use crate::rng::{ObjectKey, Stream, tags};
use crate::tables::gauss_legendre::{GL64_NODES, GL64_WEIGHTS};
use crate::units::{
    Gigayears, Kelvin, Metres, MetresPerSecondSquared, PerSquareKilometre, WattsPerSquareMetre,
    Years,
};

#[cfg(test)]
mod tests;

/// Earth's continental fraction, its land-plus-shelf area: 0.405 (R09's Design note 3, from
/// Earth2014's two crustal populations, Hirt and Rexer 2015), which the Solar System table passes
/// ([`Continents::Given`]).
pub const EARTH_CONTINENTAL_FRACTION: f64 = 0.405;

/// The age from a body's formation at which its continents begin to grow, `t_p`: 0.5 Gyr, Höning
/// and Spohn 2023's onset of continental growth (Astrobiology 23(4), 372–394; R09.T0.b's reading,
/// 2026-10-09, medium-low confidence).
pub const CONTINENTAL_GROWTH_ONSET: Gigayears = Gigayears::new(0.5);

/// The timescale of continental growth, τ: 1.1 Gyr, fitted by eye to Höning and Spohn 2023's Fig.
/// 5 Earth-like curve (0.15, 0.25, 0.31 and 0.37 of coverage at 1, 1.5, 2 and 3 Gyr; R09.T0.b).
/// Earth's 0.405 at 4.5 Gyr is then f∞ = 0.416.
pub const CONTINENTAL_GROWTH_TIMESCALE: Gigayears = Gigayears::new(1.1);

/// The least continental asymptote f∞: 0.2, Höning and Spohn 2023's ocean planet after 4.5 Gyr
/// (their Figs. 4–5 and §5; signoff-3.md). The asymptote is uniform from it over
/// [`CONTINENTAL_ASYMPTOTE_WIDTH`], to their land planet's 0.7.
pub const CONTINENTAL_ASYMPTOTE_FLOOR: f64 = 0.2;

/// The width of the continental asymptote's uniform range: 0.5, from
/// [`CONTINENTAL_ASYMPTOTE_FLOOR`]'s 0.2 to 0.7 (signoff-3.md).
pub const CONTINENTAL_ASYMPTOTE_WIDTH: f64 = 0.5;

/// Words of [`tags::BODY_CONTINENTS`] a body reads or reserves: word 0, the asymptote's uniform,
/// then seven reserved.
pub const CONTINENT_WORDS: u64 = 8;

/// The mean elevation of Earth's continental population over its oceanic one, Δ: 4.69 km, from
/// Earth2014's two crustal populations (R09's Design note 3, researched 2026-09-29 with pyshtools,
/// relative to the reference equipotential with degree 1 included).
pub const CONTINENTAL_OFFSET: Metres = Metres::new(4_690.0);

/// The width of Earth's continental population, `σ_c`: 1.08 km (R09's Design note 3, Earth2014).
pub const CONTINENTAL_WIDTH: Metres = Metres::new(1_080.0);

/// The width of Earth's oceanic population, `σ_o`: 0.93 km (R09's Design note 3, Earth2014).
pub const OCEANIC_WIDTH: Metres = Metres::new(930.0);

/// The structural relief of any lid but a mobile one: 0.9 km, Venus's (R09's Design note 3, from
/// `VenusTopo719`'s 0.94 km with almost no basin or constructional share).
pub const STAGNANT_LID_RELIEF: Metres = Metres::new(900.0);

/// The basin share at saturation on a silicate crust at the Moon's gravity: 2.05 km (R09's Design
/// note 3, the share the Moon calibrates, LOLA's 2.239 km with [`STAGNANT_LID_RELIEF`]).
pub const BASIN_RELIEF: Metres = Metres::new(2_050.0);

/// The gravity the basin share is stated at, m s⁻²: the Moon's, 1.62 (R09's Design note 3).
pub const BASIN_REFERENCE_GRAVITY: MetresPerSecondSquared = MetresPerSecondSquared::new(1.62);

/// The basin share's composition factor `k_comp` on an ice-rich crust: 0.16, the factor Ceres
/// calibrates (R09's Design note 3, Dawn's shape); a silicate crust's is 1.
pub const ICE_RICH_BASIN_FACTOR: f64 = 0.16;

/// The crater density N(>1 km) at which the basin share saturates, per km²: 0.018 (R09's Design
/// note 3, kept as a global factor read at the surface age).
pub const BASIN_SATURATION_DENSITY: PerSquareKilometre = PerSquareKilometre::new(0.018);

/// The cap of the basin share, as a fraction of the radius: 0.02, the cap Vesta calibrates (R09's
/// Design note 3).
pub const BASIN_RELIEF_CAP: f64 = 0.02;

/// The constructional share of a fully supported surface at V = 1 at Mars's gravity: 2.76 km, the
/// share Mars calibrates (R09's Design note 3, MOLA's 2.90 km).
pub const CONSTRUCTIONAL_RELIEF: Metres = Metres::new(2_760.0);

/// The gravity the constructional share is stated at, m s⁻²: Mars's, 3.71 (R09's Design note 3).
pub const CONSTRUCTIONAL_REFERENCE_GRAVITY: MetresPerSecondSquared =
    MetresPerSecondSquared::new(3.71);

/// The lithosphere's thickness from which it holds up an edifice fully: 70 km, the normalisation
/// R09's Design note 3 calibrated with [`MECHANICAL_ISOTHERM`], Olympus Mons's > 70 km (McGovern
/// et al. 2004's correction).
pub const SUPPORTING_LITHOSPHERE: Metres = Metres::new(70e3);

/// The mantle's thermal conductivity in the lithosphere's thickness, W m⁻¹ K⁻¹: 3.1, the plate
/// model's (Parsons and Sclater 1977, JGR 82, 803; Stein and Stein 1992, Nature 359, 123; R09's
/// Design note 3, medium confidence).
pub const MANTLE_CONDUCTIVITY_W_PER_M_K: f64 = 3.1;

/// The isotherm at the base of a silicate crust's mechanical lithosphere: 870 K, about 600 °C,
/// "near 550–600 °C, depending on strain rate" (McNutt 1984, as McGovern et al. 2002, JGR
/// 107(E12), 5136, §4.2 ¶68, use it for Mars), kept as a mechanical-thickness proxy with which
/// [`SUPPORTING_LITHOSPHERE`] was calibrated (ruled by R09.T0.b, 2026-10-09).
pub const MECHANICAL_ISOTHERM: Kelvin = Kelvin::new(870.0);

/// The isotherm at the base of an ice-rich crust's lithosphere: 170 K, R09's lean (plan 14's
/// P14.T48.a, signoff-3.md), labelled uncalibrated; its source is not cited yet (plan 14's Risks).
pub const ICE_LITHOSPHERE_ISOTHERM: Kelvin = Kelvin::new(170.0);

/// The exponent of the plume vigour in the volcanism level: 1.5, fitted inside the 1.1–1.8 the
/// `σ_h` calibrations allow (the Moon needs at least 1.1, Earth at most 1.84 and Venus at most
/// 1.95; science-p14-t24b-laws §4.1), labelled empirical.
pub const VOLCANISM_EXPONENT: f64 = 1.5;

/// The relief over its base of the tallest volcanic edifice on the body that calibrates the
/// constructional share, h♂: 21.9 km, Olympus Mons (Plescia 2004, JGR 109, E03003; MOLA's summit
/// stands 21.287 km over the datum), a shield at its strength limit, whose basal scarp and flank
/// failures record its spreading (McGovern and Morgan 2009, Geology 37, 139; from memory). Edifice
/// heights scale as 1 ÷ g (Weisskopf 1975, Science 187, 605; Johnson and McGetchin 1973, Icarus
/// 18, 612; science-p14-t24b-laws §4.x.1).
pub const TALLEST_EDIFICE_RELIEF: Metres = Metres::new(21_900.0);

/// The volcanism level at which the constructional share reaches the strength limit, `V_s` =
/// h♂ ÷ (4 `C₀`) = 1.984 (science-p14-t24b-laws §4.x.1): a surface three-quarters covered by
/// conical edifices at `h_max`(g), whose RMS is `h_max` ÷ 4, the greatest any straight- or
/// concave-flanked shield field reaches. Both sides scale as the lithosphere's support over g, so
/// the strength bound on the share is this bound on V ([`TALLEST_EDIFICE_RELIEF`],
/// [`CONSTRUCTIONAL_RELIEF`]).
pub const VOLCANISM_SATURATION: f64 = 1.983_695_652_173_913;

/// The melt the Solar System table's Mars has erupted while its lithosphere held it, I♂,
/// W m⁻² Gyr: its integral by [`erupted_melt_w_gyr_per_m2`] over its surface age, so that its volcanism level
/// is 1 (science-p14-t24b-laws §4.1, whose replica gives about 0.0335): 0.0349 over its 3.90 Gyr
/// surface. `Fitted`: a test refits it.
pub const MARS_ERUPTED_MELT_W_GYR_PER_M2: f64 = 0.034_943_949_696_657_574;

/// The Solar System table's Mars's plume vigour, g `D_m`³, m⁴ s⁻² ([`plume_vigour_m4_per_s2`]), so that its
/// volcanism level is 1 (science-p14-t24b-laws §4.1): 2.22 × 10¹⁹, its mantle 1,814 km deep over
/// a core of iron share 0.216. `Fitted`: a test refits it.
pub const MARS_PLUME_VIGOUR_M4_PER_S2: f64 = 2.224_010_759_706_974_6e19;

/// A body's continental asymptote f∞ in the universe of `seed` (P14.T48.a): 0.2 + 0.5 u
/// ([`CONTINENTAL_ASYMPTOTE_FLOOR`], [`CONTINENTAL_ASYMPTOTE_WIDTH`]), u the open uniform of word
/// 0 of its [`tags::BODY_CONTINENTS`] stream, so uniform on (0.2, 0.7). No other word is read.
///
/// # Examples
///
/// The asymptote depends on the body's ID and nothing else:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::hooks::figures::relief::continental_asymptote;
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let earth = BodyId::new(system, 0x0300);
/// let f = continental_asymptote(Seed::new(7), earth);
/// assert!((0.2..0.7).contains(&f));
/// assert_eq!(f.to_bits(), continental_asymptote(Seed::new(7), earth).to_bits());
/// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
/// ```
#[must_use]
pub fn continental_asymptote(seed: Seed, body: BodyId) -> f64 {
    let mut stream = Stream::open(seed, tags::BODY_CONTINENTS, ObjectKey::from(body));
    CONTINENTAL_ASYMPTOTE_FLOOR + CONTINENTAL_ASYMPTOTE_WIDTH * stream.uniform_open()
}

/// What sets a body's continental fraction before its regime gates it (P14.T48.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Continents {
    /// Grown toward the asymptote `asymptote` (f∞, [`continental_asymptote`]) after
    /// [`CONTINENTAL_GROWTH_ONSET`], on a world whose history is `window`: none on a world that
    /// never entered P14.T48.b's water window holding water.
    Grown {
        /// The asymptote f∞, from 0.2 to 0.7.
        asymptote: f64,
        /// Whether the world entered the window holding water (T48.b's start condition,
        /// science-p14-t24b-laws §4.6).
        window: WindowEntry,
    },
    /// A fraction given whatever the history, 0 to 1: the Solar System table's Earth's measured
    /// [`EARTH_CONTINENTAL_FRACTION`].
    Given(f64),
}

/// The continental fraction `f_c`, 0 to 1, of a body under the tectonic regime `regime` at the age
/// `age` from its formation, whose continents `continents` describe (P14.T48.a; the [module](self)
/// documentation): zero on every lid but a mobile one, before [`CONTINENTAL_GROWTH_ONSET`], and on
/// a world that never entered the water window holding water.
///
/// It rises monotonically to f∞ (P14.T48.a's test), continuously in the age, and reads no mass.
///
/// # Examples
///
/// An Earth whose asymptote is 0.416 has 0.405 of its surface in continents at 4.57 Gyr, and none
/// at 0.4 Gyr or under a stagnant lid:
///
/// ```
/// use hyperion_sim::planetary::hooks::figures::relief::{Continents, continental_fraction};
/// use hyperion_sim::planetary::hooks::figures::{TectonicRegime, WindowEntry};
/// use hyperion_sim::units::Years;
///
/// let earth = Continents::Grown { asymptote: 0.416, window: WindowEntry::Entered };
/// let now = continental_fraction(earth, TectonicRegime::MobileLid, Years::new(4.567e9));
/// assert!((now - 0.405).abs() < 0.002, "{now}");
/// assert_eq!(continental_fraction(earth, TectonicRegime::MobileLid, Years::new(4e8)), 0.0);
/// assert_eq!(continental_fraction(earth, TectonicRegime::StagnantLid, Years::new(4.567e9)), 0.0);
/// ```
#[must_use]
pub fn continental_fraction(continents: Continents, regime: TectonicRegime, age: Years) -> f64 {
    match regime {
        TectonicRegime::MobileLid => {}
        TectonicRegime::StagnantLid
        | TectonicRegime::HeatPipe
        | TectonicRegime::Episodic
        | TectonicRegime::IceShell => return 0.0,
    }
    match continents {
        Continents::Given(fraction) => fraction.clamp(0.0, 1.0),
        Continents::Grown { window, asymptote } => {
            let entered = match window {
                WindowEntry::Entered => true,
                WindowEntry::Never => false,
            };
            let since = age.value() - Years::from(CONTINENTAL_GROWTH_ONSET).value();
            // By comparison, so that a NaN takes none.
            if !(entered && since > 0.0) {
                return 0.0;
            }
            let tau = Years::from(CONTINENTAL_GROWTH_TIMESCALE).value();
            -asymptote * math::exp_m1(-since / tau)
        }
    }
}

/// The thickness of a lithosphere under a surface at `surface_temperature` carrying the heat flow
/// `heat_flow`, `T_e`, on a crust of `crust` (P14.T48.a):
///
/// - a silicate crust's, k (870 K − `T_s`) ÷ F, the conductive depth of [`MECHANICAL_ISOTHERM`] at
///   [`MANTLE_CONDUCTIVITY_W_PER_M_K`];
/// - an ice-rich crust's, (a ÷ F) ln(`T_ice` ÷ `T_s`), ice Ih's conductivity k = a ÷ T at
///   [`ICE_CONDUCTIVITY_COEFFICIENT`] (a = 651 W m⁻¹, Petrenko and Whitworth 1999; Klinger 1980's
///   567 is the other value the plan names) to [`ICE_LITHOSPHERE_ISOTHERM`], labelled
///   uncalibrated.
///
/// Zero where the surface is at or above its isotherm. Infinite where no heat flows through a
/// surface below it, a lithosphere that holds up any load.
///
/// # Examples
///
/// Earth's 92.5 mW m⁻² under 288 K make a 19.5 km lithosphere, and Europa's 50 mW m⁻² under 102 K
/// a 6.7 km one of ice:
///
/// ```
/// use hyperion_sim::planetary::hooks::figures::Crust;
/// use hyperion_sim::planetary::hooks::figures::relief::lithosphere_thickness;
/// use hyperion_sim::units::{Kelvin, WattsPerSquareMetre};
///
/// let earth = lithosphere_thickness(Crust::Silicate, Kelvin::new(288.0), WattsPerSquareMetre::new(0.0925));
/// assert!((earth.value() / 1e3 - 19.5).abs() < 0.1);
/// let europa = lithosphere_thickness(Crust::IceRich, Kelvin::new(102.0), WattsPerSquareMetre::new(0.05));
/// assert!((europa.value() / 1e3 - 6.65).abs() < 0.05, "{}", europa.value());
/// ```
#[must_use]
pub fn lithosphere_thickness(
    crust: Crust,
    surface_temperature: Kelvin,
    heat_flow: WattsPerSquareMetre,
) -> Metres {
    let (t_s, f) = (surface_temperature.value(), heat_flow.value());
    let (isotherm, drop) = match crust {
        Crust::Silicate => {
            let isotherm = MECHANICAL_ISOTHERM.value();
            (isotherm, MANTLE_CONDUCTIVITY_W_PER_M_K * (isotherm - t_s))
        }
        Crust::IceRich => {
            let isotherm = ICE_LITHOSPHERE_ISOTHERM.value();
            // At or above the isotherm the logarithm is not read, a non-positive one included.
            let drop = if t_s > 0.0 && t_s < isotherm {
                ICE_CONDUCTIVITY_COEFFICIENT * math::ln(isotherm / t_s)
            } else {
                0.0
            };
            (isotherm, drop)
        }
    };
    // By comparison, so that a NaN surface holds no lithosphere.
    if t_s.is_nan() || t_s >= isotherm {
        return Metres::ZERO;
    }
    if f > 0.0 {
        Metres::new(drop / f)
    } else {
        Metres::new(f64::INFINITY)
    }
}

/// The lithosphere's support of an edifice as it forms, min(1, `T_e` ÷ [`SUPPORTING_LITHOSPHERE`]):
/// 0 to 1.
#[must_use]
fn support(thickness: Metres) -> f64 {
    let share = thickness / SUPPORTING_LITHOSPHERE;
    if share < 1.0 { share } else { 1.0 }
}

/// A body's plume vigour g `D_m`³, m⁴ s⁻², at surface gravity `surface_gravity` over a radius
/// `radius` whose solids hold an iron share `iron_share` (science-p14-t24b-laws §4.1): the mantle's
/// depth `D_m` = R (1 − √x) over a core of radius R √x, so a mantle Rayleigh number at fixed
/// viscosity and temperature contrast. Large shields need focused, long-lived plumes, which a thin
/// or sluggish mantle cannot raise (Kiefer 2003, from memory).
#[must_use]
pub fn plume_vigour_m4_per_s2(
    surface_gravity: MetresPerSecondSquared,
    radius: Metres,
    iron_share: f64,
) -> f64 {
    let share = iron_share.clamp(0.0, 1.0);
    let depth = radius.value() * (1.0 - share.sqrt());
    surface_gravity.value() * depth * depth * depth
}

/// What [`volcanism_level`] reads of a body, as plain values, with its heat flow as a pure function
/// of the system age.
#[derive(Debug, Clone, Copy)]
pub struct VolcanismInputs<F> {
    /// Its crust: an ice-rich one takes no volcanism.
    pub crust: Crust,
    /// Its surface age S, the time since its last global resurfacing (P14.T24.b's).
    pub surface_age: Years,
    /// Its age, the system's, at the present.
    pub age: Years,
    /// Its mean surface temperature now (P14.T13.c's), which the lithosphere then reads: its
    /// history is not modelled.
    pub surface_temperature: Kelvin,
    /// Its mean surface gravity, the bulk section's.
    pub surface_gravity: MetresPerSecondSquared,
    /// Its mean radius, the bulk section's.
    pub radius: Metres,
    /// The iron share of its solids, iron ÷ (iron + rock).
    pub iron_share: f64,
    /// Its heat flow F at a system age (P14.T24.b's at that age: the regime, the radiogenic term
    /// and the residual then).
    pub heat_flow_at: F,
}

/// The melt a body's surface has erupted while its lithosphere held it, I, W m⁻² Gyr
/// (science-p14-t24b-laws §4.1): ∫ from (age − S) to age of max(0, F(t) − `F_c`) × min(1, `T_e`(t)
/// ÷ 70 km) dt, by the 64-point Gauss–Legendre rule at fixed nodes, summed in their order, with
/// `F_c` [`SURFACE_MELT_HEAT_FLOW`] and `T_e`(t) the silicate lithosphere at the heat flow then
/// under today's surface ([`lithosphere_thickness`]). Zero for a surface age that is not positive.
///
/// Below `F_c` no melt reaches the surface (§1.3); only what erupted since the last global
/// resurfacing survives as relief, and an edifice grows only where the lithosphere holds it up
/// as it forms (Watts 2001; McGovern and Solomon 1993; from memory).
#[must_use]
pub fn erupted_melt_w_gyr_per_m2<F>(inputs: &VolcanismInputs<F>) -> f64
where
    F: Fn(Years) -> WattsPerSquareMetre,
{
    let span = inputs.surface_age.value();
    // By comparison, so that a NaN surface age erupts nothing.
    if span.is_nan() || span <= 0.0 {
        return 0.0;
    }
    let half = 0.5 * span;
    let middle = inputs.age.value() - half;
    let threshold = SURFACE_MELT_HEAT_FLOW.value();
    let sum = GL64_NODES
        .iter()
        .zip(&GL64_WEIGHTS)
        .fold(0.0, |sum, (&node, &weight)| {
            let flow = (inputs.heat_flow_at)(Years::new(middle + half * node));
            let excess = flow.value() - threshold;
            if excess.is_nan() || excess <= 0.0 {
                return sum;
            }
            let thickness =
                lithosphere_thickness(Crust::Silicate, inputs.surface_temperature, flow);
            sum + weight * excess * support(thickness)
        });
    sum * half / Years::from(Gigayears::new(1.0)).value()
}

/// The volcanism level V of the body `inputs` describe (P14.T48.a; science-p14-t24b-laws §4.1
/// and §4.x.1), dimensionless, in [0, [`VOLCANISM_SATURATION`]]: (I ÷ I♂) × ((g `D_m`³) ÷
/// (g `D_m`³)♂)^1.5 held to that range, with I [`erupted_melt_w_gyr_per_m2`], g `D_m`³
/// [`plume_vigour_m4_per_s2`] and ♂ the Solar System table's Mars
/// ([`MARS_ERUPTED_MELT_W_GYR_PER_M2`], [`MARS_PLUME_VIGOUR_M4_PER_S2`]), so that its V is 1.
/// `V_s`, 1.984, is a surface saturated with strength-limited constructs, which R09.T12.c's
/// province sizing reads as its largest extent; the raw product is not published.
///
/// Zero on an ice-rich crust: `F_c` is a silicate melt's threshold, and cryovolcanic constructs
/// (Ceres's Ahuna Mons) are not modelled. The ruling's replica gives the Moon 0.066, Mercury under
/// 0.001, Venus 1.05 and Earth 1.23. V is R09's constructional share's multiplier: it builds
/// relief over the surface's history, where the present outgassing O is the rate now (Mars has old
/// volcanic relief and no outgassing now).
///
/// # Examples
///
/// A Mars-sized body whose heat flow fell from 60 to 20 mW m⁻² over its 3.9 Gyr surface erupted
/// while it lay above `F_c`; its relief needs the melt and the vigour of its mantle, and an icy
/// crust takes none:
///
/// ```
/// use hyperion_sim::planetary::hooks::figures::Crust;
/// use hyperion_sim::planetary::hooks::figures::relief::{
///     VolcanismInputs, erupted_melt_w_gyr_per_m2, volcanism_level,
/// };
/// use hyperion_sim::units::{Kelvin, Metres, MetresPerSecondSquared, WattsPerSquareMetre, Years};
///
/// let (age, surface) = (4.57e9, 3.9e9);
/// let falling = |t: Years| {
///     WattsPerSquareMetre::new(0.060 - 0.040 * (t.value() - (age - surface)) / surface)
/// };
/// let mars = VolcanismInputs {
///     crust: Crust::Silicate,
///     surface_age: Years::new(surface),
///     age: Years::new(age),
///     surface_temperature: Kelvin::new(215.0),
///     surface_gravity: MetresPerSecondSquared::new(3.71),
///     radius: Metres::new(3.39e6),
///     iron_share: 0.216,
///     heat_flow_at: falling,
/// };
/// assert!(erupted_melt_w_gyr_per_m2(&mars) > 0.0);
/// let v = volcanism_level(&mars);
/// assert!((0.5..5.0).contains(&v), "{v}");
/// let icy = VolcanismInputs { crust: Crust::IceRich, ..mars };
/// assert!(volcanism_level(&icy) <= 0.0);
/// ```
#[must_use]
pub fn volcanism_level<F>(inputs: &VolcanismInputs<F>) -> f64
where
    F: Fn(Years) -> WattsPerSquareMetre,
{
    match inputs.crust {
        Crust::Silicate => {}
        Crust::IceRich => return 0.0,
    }
    let melt = erupted_melt_w_gyr_per_m2(inputs);
    if melt.is_nan() || melt <= 0.0 {
        return 0.0;
    }
    let vigour = plume_vigour_m4_per_s2(inputs.surface_gravity, inputs.radius, inputs.iron_share);
    bounded_volcanism(
        melt / MARS_ERUPTED_MELT_W_GYR_PER_M2
            * math::powf(vigour / MARS_PLUME_VIGOUR_M4_PER_S2, VOLCANISM_EXPONENT),
    )
}

/// The volcanism level the law gives, `level`, held to [0, [`VOLCANISM_SATURATION`]], the
/// strength limit of the constructional share (science-p14-t24b-laws §4.x.1), by comparisons, so
/// that a NaN takes 0. Old, massive stagnant lids reach a raw level of some 10³ on the sample;
/// every calibrating body lies at 1 or under, and keeps its own.
#[must_use]
fn bounded_volcanism(level: f64) -> f64 {
    if level > 0.0 {
        if level < VOLCANISM_SATURATION {
            level
        } else {
            VOLCANISM_SATURATION
        }
    } else {
        0.0
    }
}

/// What [`Relief::of`] reads of a body, as plain values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReliefInputs {
    /// Its tectonic regime: a mobile lid has two crustal populations.
    pub regime: TectonicRegime,
    /// Its continental fraction `f_c` ([`continental_fraction`]), 0 to 1.
    pub continental_fraction: f64,
    /// Its crust, which sets the basin share's `k_comp`.
    pub crust: Crust,
    /// Its mean surface gravity, the bulk section's, positive.
    pub surface_gravity: MetresPerSecondSquared,
    /// Its mean radius, the bulk section's, which caps the basin share.
    pub radius: Metres,
    /// Its lunar-equivalent crater density N☾(>1 km) (P14.T48.c's), per km².
    pub crater_density: PerSquareKilometre,
    /// Its volcanism level V ([`volcanism_level`]).
    pub volcanism: f64,
    /// Its lithosphere's thickness at today's heat flow ([`lithosphere_thickness`]).
    pub lithosphere: Metres,
}

/// One population of a body's hypsometry: its share of the surface, its mean elevation over the
/// datum and its width, a Gaussian truncated at ±3 widths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Population {
    /// Its share of the surface, 0 to 1.
    pub(super) weight: f64,
    /// Its mean elevation over the datum, the whole surface's mean.
    pub(super) mean: Metres,
    /// Its width σ, positive.
    pub(super) width: Metres,
}

impl Population {
    /// Its share of the surface, 0 to 1.
    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }

    /// Its mean elevation over the datum, the whole surface's mean, m.
    #[must_use]
    pub const fn mean(&self) -> Metres {
        self.mean
    }

    /// Its width σ, m, positive: the Gaussian's before its truncation at ±3σ.
    #[must_use]
    pub const fn width(&self) -> Metres {
        self.width
    }
}

/// A body's hypsometry, the distribution of its surface's elevations whose variance is `σ_h`²
/// (P14.T48.a; science-p14-t24b-laws §4.2): two populations on a mobile lid, one on any other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hypsometry {
    /// A mobile lid's continental population, then its oceanic one.
    TwoPopulations([Population; 2]),
    /// Any other lid's one population of `σ_h`, about the datum.
    One(Population),
}

impl Hypsometry {
    /// Its populations: the continental then the oceanic on a mobile lid, or the one.
    #[must_use]
    pub fn populations(&self) -> &[Population] {
        match self {
            Self::TwoPopulations(populations) => populations,
            Self::One(population) => core::slice::from_ref(population),
        }
    }
}

/// A body's relief (P14.T48.a): `σ_h` and its three shares, its continental fraction, and its
/// hypsometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relief {
    structural: Metres,
    basin: Metres,
    constructional: Metres,
    continental_fraction: f64,
    hypsometry: Hypsometry,
}

impl Relief {
    /// The relief of the body `inputs` describe, by the [module](self) documentation's shares.
    ///
    /// # Examples
    ///
    /// Earth's mobile lid, with 0.405 of it continental, a young surface and its 19.5 km
    /// lithosphere, has a `σ_h` of about 2.5 km, almost all of it structural:
    ///
    /// ```
    /// use hyperion_sim::planetary::hooks::figures::relief::{Relief, ReliefInputs};
    /// use hyperion_sim::planetary::hooks::figures::{Crust, TectonicRegime};
    /// use hyperion_sim::units::{Metres, MetresPerSecondSquared, PerSquareKilometre};
    ///
    /// let earth = Relief::of(&ReliefInputs {
    ///     regime: TectonicRegime::MobileLid,
    ///     continental_fraction: 0.405,
    ///     crust: Crust::Silicate,
    ///     surface_gravity: MetresPerSecondSquared::new(9.82),
    ///     radius: Metres::new(6.371e6),
    ///     crater_density: PerSquareKilometre::new(5e-5),
    ///     volcanism: 1.2,
    ///     lithosphere: Metres::new(19.5e3),
    /// });
    /// assert!((earth.sigma_h().value() / 2_510.0 - 1.0).abs() < 0.05);
    /// assert!((earth.structural().value() - 2_507.0).abs() < 1.0);
    /// ```
    #[must_use]
    pub fn of(inputs: &ReliefInputs) -> Self {
        let g = inputs.surface_gravity.value();
        let k_comp = match inputs.crust {
            Crust::Silicate => 1.0,
            Crust::IceRich => ICE_RICH_BASIN_FACTOR,
        };
        let saturation = inputs.crater_density / BASIN_SATURATION_DENSITY;
        let saturation = if saturation < 1.0 {
            // By comparison, so that a NaN or negative density gives no basins.
            if saturation > 0.0 { saturation } else { 0.0 }
        } else {
            1.0
        };
        let basin = BASIN_RELIEF.value()
            * (BASIN_REFERENCE_GRAVITY.value() / g)
            * k_comp
            * saturation.sqrt();
        let cap = BASIN_RELIEF_CAP * inputs.radius.value();
        let basin = if basin < cap { basin } else { cap };
        let volcanism = if inputs.volcanism > 0.0 {
            inputs.volcanism
        } else {
            0.0
        };
        let constructional = CONSTRUCTIONAL_RELIEF.value()
            * (CONSTRUCTIONAL_REFERENCE_GRAVITY.value() / g)
            * volcanism
            * support(inputs.lithosphere);
        let shared = basin * basin + constructional * constructional;
        let f_c = inputs.continental_fraction.clamp(0.0, 1.0);
        let (structural, hypsometry) = match inputs.regime {
            TectonicRegime::MobileLid => {
                let (offset, sigma_c, sigma_o) = (
                    CONTINENTAL_OFFSET.value(),
                    CONTINENTAL_WIDTH.value(),
                    OCEANIC_WIDTH.value(),
                );
                let variance = f_c * (1.0 - f_c) * offset * offset
                    + f_c * sigma_c * sigma_c
                    + (1.0 - f_c) * sigma_o * sigma_o;
                let populations = [
                    Population {
                        weight: f_c,
                        mean: Metres::new((1.0 - f_c) * offset),
                        width: Metres::new((sigma_c * sigma_c + shared).sqrt()),
                    },
                    Population {
                        weight: 1.0 - f_c,
                        mean: Metres::new(-f_c * offset),
                        width: Metres::new((sigma_o * sigma_o + shared).sqrt()),
                    },
                ];
                (variance.sqrt(), Hypsometry::TwoPopulations(populations))
            }
            TectonicRegime::StagnantLid
            | TectonicRegime::HeatPipe
            | TectonicRegime::Episodic
            | TectonicRegime::IceShell => {
                let structural = STAGNANT_LID_RELIEF.value();
                let sigma = (structural * structural + shared).sqrt();
                (
                    structural,
                    Hypsometry::One(Population {
                        weight: 1.0,
                        mean: Metres::ZERO,
                        width: Metres::new(sigma),
                    }),
                )
            }
        };
        let continental_fraction = match inputs.regime {
            TectonicRegime::MobileLid => f_c,
            TectonicRegime::StagnantLid
            | TectonicRegime::HeatPipe
            | TectonicRegime::Episodic
            | TectonicRegime::IceShell => 0.0,
        };
        Self {
            structural: Metres::new(structural),
            basin: Metres::new(basin),
            constructional: Metres::new(constructional),
            continental_fraction,
            hypsometry,
        }
    }

    /// The hypsometric standard deviation `σ_h`, m: the three shares in quadrature, the
    /// area-weighted RMS elevation about the mean relative to the reference equipotential, degree
    /// 1 included. It is not a greatest relief, which no finite realisation can match.
    #[must_use]
    pub fn sigma_h(&self) -> Metres {
        let (s, b, c) = (
            self.structural.value(),
            self.basin.value(),
            self.constructional.value(),
        );
        Metres::new((s * s + b * b + c * c).sqrt())
    }

    /// The structural share, m: the mobile lid's two populations' spread, or a stagnant lid's
    /// [`STAGNANT_LID_RELIEF`].
    #[must_use]
    pub const fn structural(&self) -> Metres {
        self.structural
    }

    /// The basin share, m, at most [`BASIN_RELIEF_CAP`] of the radius.
    #[must_use]
    pub const fn basin(&self) -> Metres {
        self.basin
    }

    /// The constructional share, m.
    #[must_use]
    pub const fn constructional(&self) -> Metres {
        self.constructional
    }

    /// The continental fraction `f_c` it was built with, 0 to 1: zero on every lid but a mobile
    /// one.
    #[must_use]
    pub const fn continental_fraction(&self) -> f64 {
        self.continental_fraction
    }

    /// Its hypsometry, which the oceans fill.
    #[must_use]
    pub const fn hypsometry(&self) -> &Hypsometry {
        &self.hypsometry
    }
}
