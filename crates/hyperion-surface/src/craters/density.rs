//! The cumulative crater density N(>D) of a body and its inversion within a diameter octave (plan
//! R09, Design note 12).
//!
//! # The production
//!
//! The shape is Neukum, Ivanov and Hartmann 2001's lunar production polynomial,
//! log₁₀ N(>D) = Σ aₙ (log₁₀ D)ⁿ with D in km, holding a1…a11 only. a0 is log₁₀ N☾(>1 km), the
//! body's lunar-equivalent density, so the Moon's N(>1 km) comes out exactly as given. Beyond the
//! polynomial's range, 10 m to 300 km, the cumulative density continues as a power law of the end
//! slopes: −2 below 10 m (the plan's), and the polynomial's own slope at 300 km, −1.83, above it,
//! so it is continuous everywhere and continuously differentiable above.
//!
//! # The diameter map
//!
//! The contract's density is lunar-equivalent: the Moon's crater density for the body's
//! projectile flux (science-r09-craters.md F1). A body other than the Moon is struck at another
//! speed, by projectiles of another density, and craters under another gravity in another target,
//! so the projectile that makes a crater of D on it makes a crater of φ(D) on the Moon, and the
//! body's production is the Moon's at φ(D): N(>D) = N☾ × 10^(Σ aₙ (log₁₀ φ(D))ⁿ), not renormalised
//! at the body's 1 km. Its own N(>1 km) is N☾ at φ(1 km) ([`own_density_at_1km`]): 0.53 of N☾ on
//! Venus at 25.7 km/s, and 0.26 on Mars at Ivanov 2001's velocities, whose Mars chronology, 0.49
//! of the Moon's (Craterstats), then implies his Mars-to-Moon impact rate of about 2 (Ivanov et al.
//! 2002, §5.2). The map is π-group scaling in the gravity regime (Holsapple 1993), as Ivanov 2001
//! used it to carry the lunar production function to Mars, and is the identity without an impact
//! velocity and, exactly, at the lunar reference: g☾ = 1.62 m s⁻², `D_t` = 19 km, v☾ =
//! [`LUNAR_IMPACT_VELOCITY`], 16.7 km/s, a non-porous target of 2,550 kg m⁻³
//! ([`LUNAR_TARGET`]) and projectiles of 3,000 ([`LUNAR_PROJECTILE_DENSITY`]).
//!
//! 1. The body's crater D becomes the diameter of the bowl its transient cavity would have made,
//!    D itself below the body's transition diameter `D_t`, and `D_t` (D ÷ `D_t`)^(1 ÷ 1.13) above
//!    it, where a complex crater has collapsed from a smaller cavity (McKinnon and Schenk 1985's
//!    collapse law, D = 1.17 `D_tc`^1.13 `D_t`^−0.13, as Collins, Melosh and Marcus 2005 state it,
//!    made continuous at `D_t` by dropping the 1.17, which cancels where a crater is complex on
//!    both bodies; Ivanov 2001 used Croft 1985's D ∝ `D_tc`^1.18).
//! 2. That cavity is the π-group cavity of a projectile of radius a: with ν = 1/3,
//!    `D_tc` = C K₁^(1/3) a^(2 ÷ (2 + μ)) (`ρ_p` ÷ `ρ_t`)^(2 ÷ (3 (2 + μ))) (U² ÷ g)^(μ ÷ (2 + μ)),
//!    from Holsapple 1993's `π_V` = K₁ π₂^(−3μ ÷ (2 + μ)) (`ρ_t` ÷ `ρ_p`)^(μ ÷ (2 + μ)), with the
//!    target class's μ and K₁ ([`TargetClass`]) and the mean impact speed as given, with no angle
//!    factor. C turns the cavity's volume into its diameter: a paraboloid as deep as the class's
//!    fresh bowl, C = (4/3 × 8 ÷ k)^(1/3), 3.764 for a non-porous bowl 0.20 D deep (Singer et al.
//!    2019's transient depth of 0.2) and 4.239 for a porous one 0.14 D deep, since porous
//!    transients are shallower (Holsapple 2022, Table 2; science-r09-craters.md F4.2). At μ = 0.55
//!    the cavity goes as U^0.431, g^−0.216 and the density ratio^0.261 (Ivanov et al. 2002, eq. 4,
//!    round them to β = 0.43 and γ = 0.22), and at μ = 0.41 as U^0.340, g^−0.170 and the
//!    ratio^0.277.
//! 3. The same projectile's cavity on the Moon follows from the lunar reference's, so the lunar
//!    cavity is a power of the body's, of exponent (2 + μ) ÷ (2 + μ☾): 1 for a non-porous body,
//!    between which and the reference C, K₁ and the absolute speed cancel, and 0.945 for a porous
//!    one, for which they do not.
//! 4. It collapses on the Moon by the same law about the Moon's 19 km.
//!
//! At Mars's gravity (3.71 m s⁻²) and Ivanov 2001's ratio of mean impact velocities, 9.6 ÷ 16.1,
//! the map reproduces Ivanov's Mars production function (Craterstats' "Mars, Ivanov (2001)"
//! coefficients) to within 3.9% in N at 20 diameters a decade over 1–100 km (4.2% sampled
//! continuously), both normalised at 1 km, and P14.T48.f's Mars at 10.6 km/s to within 9.9% (10.2%)
//! (3.3% and 9.2% under R09.T7.a's rounded exponents); the lunar shape
//! alone departs from it by up to 2.6× at 10 km. The strength regime, below Y ÷ (`ρ_t` g),
//! kilometres on low-gravity icy bodies, is not mapped (plan R09, Risks).
//!
//! # Screening
//!
//! An atmosphere stops projectiles smaller than about the projectile scale d\* = 1.5 (P ÷ g) ÷
//! `ρ_p`, the size that meets its own mass of air, so the craters below the cutoff `D_c` = 20 d\*
//! are missing. The differential production is tapered by f(D) = 1 ÷ (1 + (`D_c` ÷ D)^4.2), one
//! rule for every body (Design note 12: the exponent is fitted on Venus's 881 named craters
//! through the diameter map, science-r09-craters.md F1). The screened cumulative density
//! N(>D) = ∫ f n dD′ from D up has no closed form: it is integrated in ln D by eight-point
//! Gauss–Legendre panels a quarter of a decade wide, on a grid fixed per body and split wherever
//! the integrand is not smooth, up to 1,000 `D_c`, beyond which f differs from 1 by less than
//! 3 × 10⁻¹³ and the production's own closed form takes over. The panels are summed from the top
//! down, so the density never rises with D, to the last bit.

use hyperion_base::math;
use hyperion_base::units::{
    KilogramsPerCubicMetre, Metres, MetresPerSecond, MetresPerSecondSquared, PerSquareKilometre,
};

use super::shape::fresh_bowl_depth;
use super::{
    CraterParams, LUNAR_IMPACT_VELOCITY, LUNAR_PROJECTILE_DENSITY, LUNAR_TARGET,
    MOON_GRAVITY_M_PER_S2, MOON_TRANSITION_M, Screening, TargetClass, transition_diameter,
};

/// Neukum, Ivanov and Hartmann 2001's lunar production polynomial, a1…a11 of log₁₀ N(>D) =
/// Σ aₙ (log₁₀ D)ⁿ, D in km and N per km² (Space Science Reviews 96, 55–86, Table 1, "new N(D)"),
/// as Michael's Craterstats lists them in `functions.txt` ("Moon, Neukum et al. (2001)", range
/// 0.01–300 km).
///
/// a0 is not held: it is log₁₀ N(>1 km), the body's parameter, so neither published value enters.
/// Neukum et al. 2001's −3.0876 (Table 1, and Ivanov et al. 2002, Table 1, "New N(D)") is this
/// polynomial's own 1-Gyr level: it matches Neukum 1983's polynomial (a0 −3.0768) below 1 km to
/// 0.011 dex, where the rework did not reach. The lunar chronology's linear term, 8.38 × 10⁻⁴ =
/// 10^−3.0768, is the 1983 normalisation, kept as published and as Craterstats pairs it with this
/// polynomial (Craterstats calls −3.0876 "a typo"; Cratermaker instead rescales the linear term to
/// 8.17 × 10⁻⁴). See science-r09-craters.md F3.
const NEUKUM: [f64; 11] = [
    -3.557_528, 0.781_027, 1.021_521, -0.156_012, -0.444_058, 0.019_977, 0.086_850, -0.005_874,
    -0.006_809, 8.25e-4, 5.54e-5,
];

/// The polynomial's lower end, log₁₀ of 0.01 km (10 m).
const LOG_D_MIN: f64 = -2.0;

/// The polynomial's upper end, log₁₀ of 300 km.
const LOG_D_MAX: f64 = 2.477_121_254_719_662_4;

/// The cumulative production's log-log slope below 10 m: −2 (Design note 12, the plan's figure;
/// the polynomial's own slope at 10 m is −3.16).
const SLOPE_BELOW: f64 = -2.0;

/// A target class's π-group constants in the gravity regime, (μ, K₁): non-porous 0.55 and 0.095,
/// porous 0.41 and 0.132, with ν = 1/3 for both (Holsapple 1993, Annu. Rev. Earth Planet. Sci. 21,
/// 333, eq. 18, p. 351's "ν = 1/3 in all cases", and its fitted laws, Fig. 4 for dry soils and
/// sand, `π_V` = 0.132 (…)^−0.51 at μ 0.41, and Fig. 5 for wet soils, 0.095 (…)^−0.65 at μ 0.55;
/// Table 1 gives wet soil and rock one μ, so rock takes wet soil's gravity-regime law; as Singer et
/// al. 2019, Science 363, 955, supplement, apply them to non-porous and 30%-porous ice; Williams et
/// al. 2014, arXiv 1309.2849, for lunar regolith; science-r09-craters.md item 2). Holsapple 2022's "cold ice" K₁ of 1.93, inconsistent with his
/// own §3.1.4, is not used.
#[must_use]
const fn class_constants(class: TargetClass) -> (f64, f64) {
    match class {
        TargetClass::NonPorous => (0.55, 0.095),
        TargetClass::Porous => (0.41, 0.132),
    }
}

/// The diameter of a transient cavity of `class` over the cube root of its volume, C = (4/3 × 8 ÷
/// k)^(1/3) for a paraboloid k D deep, k the class's fresh bowl depth ([`fresh_bowl_depth`]):
/// 3.764 non-porous and 4.239 porous ([`transient_coefficient`]; science-r09-craters.md F4.2).
#[must_use]
fn transient_shape(class: TargetClass) -> f64 {
    math::cbrt(4.0 / 3.0 * 8.0 / fresh_bowl_depth(class))
}

/// The coefficient P of a transient cavity of a target of `class` and density `target`, struck at
/// `speed` by projectiles of density `projectile` under `gravity`: the cavity's diameter
/// is P a^(2 ÷ (2 + μ)) metres for a projectile of radius a metres, with P = C K₁^(1/3)
/// (`ρ_p` ÷ `ρ_t`)^(2 ÷ (3 (2 + μ))) (U² ÷ g)^(μ ÷ (2 + μ)) ([the module documentation](self),
/// step 2).
///
/// C = (4/3 × 8 ÷ k)^(1/3) is the diameter over the cube root of the volume of a paraboloid
/// k D deep, k the class's fresh bowl depth ([`fresh_bowl_depth`]): 3.764 at 0.20 (Singer et al.
/// 2019's transient depth-to-diameter ratio of 0.2, supplement) and 4.239 at 0.14 (porous
/// transients are shallower: Holsapple 2022, arXiv 2203.07476, Table 2, gives sand's 0.096 of the
/// rim diameter against rock's 0.21; science-r09-craters.md F4.2).
#[must_use]
fn transient_coefficient(
    class: TargetClass,
    target: KilogramsPerCubicMetre,
    projectile: KilogramsPerCubicMetre,
    speed: MetresPerSecond,
    gravity: MetresPerSecondSquared,
) -> f64 {
    let (mu, k1) = class_constants(class);
    let (u, g) = (speed.value(), gravity.value());
    transient_shape(class)
        * math::cbrt(k1)
        * math::powf(
            projectile.value() / target.value(),
            2.0 / (3.0 * (2.0 + mu)),
        )
        * math::powf(u * u / g, mu / (2.0 + mu))
}

/// The collapse exponent of a complex crater, D ∝ `D_tc`^1.13 `D_t`^−0.13: McKinnon and Schenk 1985
/// (LPSC XVI, 544), as Collins, Melosh and Marcus 2005 (Meteoritics & Planetary Science 40, 817,
/// eq. 27) state it, and as Zahnle et al. 2003 use it.
const COLLAPSE_EXPONENT: f64 = 1.13;

/// The crater cutoff over the projectile scale, `D_c` ÷ d\*: 20 (Design note 12, after Melosh
/// 1989).
const CUTOFF_PER_PROJECTILE: f64 = 20.0;

/// The projectile scale over the column mass per projectile density: d\* = 1.5 (P ÷ g) ÷ `ρ_p`,
/// the projectile that meets about its own mass of air (Design note 12, after Melosh 1989).
const PROJECTILE_PER_COLUMN: f64 = 1.5;

/// The screening taper's exponent: f(D) = 1 ÷ (1 + (`D_c` ÷ D)^4.2) on the differential production
/// (Design note 12; science-r09-craters.md F1 and F4.1). It is fitted by Poisson likelihood against
/// the 881 named craters of the IAU's Gazetteer of Planetary Nomenclature for Venus, in 23
/// logarithmic bins from 1.5 to 300 km, with `D_c` held at 20 d\* and the production through the
/// diameter map at P14.T48.f's 25.7 km/s: the likelihood peaks at 4.25, 0.14 above 4.2; 4.5 lies
/// 2.1 below, 4.0 3.0 and 5.0 17.5. Its lunar-equivalent N☾(>1 km) is then 3.36 × 10⁻⁴ km⁻², and
/// its cumulative counts lie within 11% of the catalogue's at 3–40 km (10.8% at 20 km). R09.T0.b's
/// fit without the map gave 4.5. Low confidence for thinner atmospheres, on which it was not
/// measured, and tied to Venus's impact velocity and the collapse law (plan R09, Risks).
const TAPER_EXPONENT: f64 = 4.2;

/// How far above the cutoff the screened density is integrated, in units of `D_c`: beyond 1,000
/// `D_c` the taper differs from 1 by 1,000^−4.2 ≈ 2.5 × 10⁻¹³, and the production's closed form
/// carries the rest.
pub(super) const SCREENED_REACH: f64 = 1_000.0;

/// The width of one quadrature panel in ln D: a quarter of a decade, ln 10 ÷ 4.
///
/// Quartering it changes Mars's screened density at 10⁻³ `D_c` by under 10⁻¹⁰ of itself (measured
/// for R09.T7.a), and the tests hold the quadrature to an independent Simpson integration.
const PANEL_WIDTH: f64 = core::f64::consts::LN_10 / 4.0;

/// The eight-point Gauss–Legendre rule on [−1, 1]: its nodes, ascending, and weights (Abramowitz
/// and Stegun, Table 25.4, n = 8, as the sim's `tables::gauss_legendre` rounds them).
pub(crate) const GAUSS_LEGENDRE_8: [(f64, f64); 8] = [
    (-0.960_289_856_497_536_3, 0.101_228_536_290_376_26),
    (-0.796_666_477_413_626_7, 0.222_381_034_453_374_48),
    (-0.525_532_409_916_329, 0.313_706_645_877_887_27),
    (-0.183_434_642_495_649_8, 0.362_683_783_378_362),
    (0.183_434_642_495_649_8, 0.362_683_783_378_362),
    (0.525_532_409_916_329, 0.313_706_645_877_887_27),
    (0.796_666_477_413_626_7, 0.222_381_034_453_374_48),
    (0.960_289_856_497_536_3, 0.101_228_536_290_376_26),
];

/// Trask's equilibrium density N(>1 km), per km²: N(>D) = 0.079 D⁻² km⁻² with D in km, Trask 1966's
/// 10^10.9 D⁻² per 10⁶ km² with D in metres (JPL Technical Report 32-800, p. 263, eq. 1, from the
/// Ranger photographs), whose 10^−1.1 = 0.0794 Design note 12 rounds. Hartmann 1984's equilibrium
/// agrees with it near 1 km but falls as D^−1.83.
const SATURATION_PER_KM2_AT_1_KM: f64 = 0.079;

/// The bisection steps of [`diameter_in_octave`]: 40 halvings of the octave's ln 2 leave an
/// interval of 6.3 × 10⁻¹³ in ln D.
const BISECTION_STEPS: u32 = 40;

/// Σ aₙ xⁿ over n = 1…11, Neukum's polynomial without a0, by Horner's rule from a11.
#[must_use]
fn neukum(x: f64) -> f64 {
    let mut acc = 0.0;
    for a in NEUKUM.iter().rev() {
        acc = acc * x + a;
    }
    acc * x
}

/// Σ n aₙ xⁿ⁻¹ over n = 1…11, the polynomial's slope d log₁₀ N ÷ d log₁₀ D, by Horner's rule.
#[must_use]
fn neukum_slope(x: f64) -> f64 {
    let mut acc = 0.0;
    for (n, a) in (1_u8..=11).zip(NEUKUM).rev() {
        acc = acc * x + f64::from(n) * a;
    }
    acc
}

/// log₁₀ N(>D) ÷ N(>1 km) of the lunar production at x = log₁₀ D (D in km), with the end slopes
/// outside 10 m–300 km.
#[must_use]
fn lunar_log_shape(x: f64) -> f64 {
    if x < LOG_D_MIN {
        neukum(LOG_D_MIN) + SLOPE_BELOW * (x - LOG_D_MIN)
    } else if x > LOG_D_MAX {
        neukum(LOG_D_MAX) + neukum_slope(LOG_D_MAX) * (x - LOG_D_MAX)
    } else {
        neukum(x)
    }
}

/// The log-log slope of the lunar production at x = log₁₀ D (D in km), with its end slopes.
#[must_use]
fn lunar_slope(x: f64) -> f64 {
    if x < LOG_D_MIN {
        SLOPE_BELOW
    } else if x > LOG_D_MAX {
        neukum_slope(LOG_D_MAX)
    } else {
        neukum_slope(x)
    }
}

/// The π-group map from a body's crater diameters to the Moon's (the module documentation).
#[derive(Debug, Clone, Copy)]
struct DiameterMap {
    /// The body's transition diameter, km.
    transition_km: f64,
    /// The power of the body's cavity that the Moon's is, (2 + μ) ÷ (2 + μ☾): 1 for a non-porous
    /// body.
    exponent: f64,
    /// The lunar cavity, km, over the body's to the power [`exponent`](Self::exponent), km.
    cavity_scale: f64,
}

/// The Moon's transition diameter in km.
const MOON_TRANSITION_KM: f64 = MOON_TRANSITION_M / 1e3;

impl DiameterMap {
    /// The map of `p`: none without an impact velocity, and none at the lunar reference, where it
    /// is the identity exactly.
    #[must_use]
    fn of(p: &CraterParams) -> Option<Self> {
        let map = Self::built(p)?;
        let identity = map.exponent.total_cmp(&1.0).is_eq()
            && map.cavity_scale.total_cmp(&1.0).is_eq()
            && map.transition_km.total_cmp(&MOON_TRANSITION_KM).is_eq();
        (!identity).then_some(map)
    }

    /// The map of `p` whenever it has an impact velocity, the identity included.
    #[must_use]
    fn built(p: &CraterParams) -> Option<Self> {
        let speed = p.impact_velocity()?;
        let target = p.target();
        let body = transient_coefficient(
            target.class,
            target.density,
            p.projectile_density(),
            speed,
            p.gravity(),
        );
        let moon = transient_coefficient(
            LUNAR_TARGET.class,
            LUNAR_TARGET.density,
            LUNAR_PROJECTILE_DENSITY,
            LUNAR_IMPACT_VELOCITY,
            MetresPerSecondSquared::new(MOON_GRAVITY_M_PER_S2),
        );
        let (mu, _) = class_constants(target.class);
        let (mu_moon, _) = class_constants(LUNAR_TARGET.class);
        let exponent = (2.0 + mu) / (2.0 + mu_moon);
        // In km: the Moon's cavity is (P☾ ÷ 1,000) (D ÷ (P ÷ 1,000))^e of the body's D. A
        // non-porous body's e is exactly 1, and pow(x, 1) is x, so its scale is P☾ ÷ P.
        Some(Self {
            transition_km: transition_diameter(p).value() / 1e3,
            exponent,
            cavity_scale: (moon / 1e3) / math::powf(body / 1e3, exponent),
        })
    }

    /// The Moon's crater, km, that the projectile of the body's crater of `d_km` makes.
    #[must_use]
    fn lunar(&self, d_km: f64) -> f64 {
        let cavity = if d_km <= self.transition_km {
            d_km
        } else {
            self.transition_km * math::powf(d_km / self.transition_km, 1.0 / COLLAPSE_EXPONENT)
        };
        let lunar = self.cavity_scale * math::powf(cavity, self.exponent);
        if lunar <= MOON_TRANSITION_KM {
            lunar
        } else {
            MOON_TRANSITION_KM * math::powf(lunar / MOON_TRANSITION_KM, COLLAPSE_EXPONENT)
        }
    }

    /// The body's crater, km, whose projectile makes the Moon's crater of `lunar_km`: the inverse
    /// of [`lunar`](Self::lunar).
    #[must_use]
    fn body(&self, lunar_km: f64) -> f64 {
        let lunar = if lunar_km <= MOON_TRANSITION_KM {
            lunar_km
        } else {
            MOON_TRANSITION_KM * math::powf(lunar_km / MOON_TRANSITION_KM, 1.0 / COLLAPSE_EXPONENT)
        };
        let cavity = math::powf(lunar / self.cavity_scale, 1.0 / self.exponent);
        if cavity <= self.transition_km {
            cavity
        } else {
            self.transition_km * math::powf(cavity / self.transition_km, COLLAPSE_EXPONENT)
        }
    }

    /// d ln φ ÷ d ln D at the body's crater of `d_km`: the product of the two collapses' slopes
    /// and the cavities' exponent.
    #[must_use]
    fn log_slope(&self, d_km: f64) -> f64 {
        let body = if d_km <= self.transition_km {
            1.0
        } else {
            1.0 / COLLAPSE_EXPONENT
        };
        let moon = if self.lunar(d_km) <= MOON_TRANSITION_KM {
            1.0
        } else {
            COLLAPSE_EXPONENT
        };
        body * self.exponent * moon
    }
}

/// A body's unscreened production, N(>D) per km² at D in km.
#[derive(Debug, Clone, Copy)]
struct Production {
    /// The lunar-equivalent N☾(>1 km), km⁻².
    n_1km: f64,
    /// The diameter map, if any.
    map: Option<DiameterMap>,
}

impl Production {
    /// The production of `p`.
    #[must_use]
    fn of(p: &CraterParams) -> Self {
        Self {
            n_1km: p.n_1km().value(),
            map: DiameterMap::of(p),
        }
    }

    /// log₁₀ of the lunar diameter, km, of the body's `d_km`.
    #[must_use]
    fn lunar_log10(&self, d_km: f64) -> f64 {
        math::log10(match &self.map {
            Some(map) => map.lunar(d_km),
            None => d_km,
        })
    }

    /// N(>D) per km² at D = `d_km`: N☾ at the lunar image of D, not renormalised at the body's
    /// 1 km (science-r09-craters.md F1).
    #[must_use]
    fn cumulative(&self, d_km: f64) -> f64 {
        self.n_1km * math::exp10(lunar_log_shape(self.lunar_log10(d_km)))
    }

    /// −dN(>D) ÷ d ln D per km² at D = `d_km`: the differential production per unit ln D.
    #[must_use]
    fn differential(&self, d_km: f64) -> f64 {
        let x = self.lunar_log10(d_km);
        let jacobian = match &self.map {
            Some(map) => map.log_slope(d_km),
            None => 1.0,
        };
        -lunar_slope(x) * jacobian * self.n_1km * math::exp10(lunar_log_shape(x))
    }

    /// The body's diameters, km, at which the integrand of the screened density is not smooth,
    /// ascending: the images of the polynomial's ends and, under a map, the two collapses.
    #[must_use]
    fn kinks(&self) -> [f64; 4] {
        let ten_m = 0.01;
        let top = 300.0;
        let mut kinks = match &self.map {
            Some(map) => [
                map.body(ten_m),
                map.body(MOON_TRANSITION_KM),
                map.transition_km,
                map.body(top),
            ],
            // Without a map the collapses do not exist; repeating an end adds no split.
            None => [ten_m, ten_m, top, top],
        };
        kinks.sort_by(f64::total_cmp);
        kinks
    }
}

/// The screening taper f(D) = 1 ÷ (1 + (`D_c` ÷ D)^4.2) at ln D = `u` with `ln_cutoff` = ln `D_c`
/// (both in km).
#[must_use]
fn taper(u: f64, ln_cutoff: f64) -> f64 {
    1.0 / (1.0 + math::exp(TAPER_EXPONENT * (ln_cutoff - u)))
}

/// One eight-point Gauss–Legendre panel of the screened integrand, ∫ f n d ln D from ln D = `a` to
/// `b` (D in km).
#[must_use]
fn panel(production: &Production, ln_cutoff: f64, a: f64, b: f64) -> f64 {
    let half = 0.5 * (b - a);
    let mid = f64::midpoint(a, b);
    let mut sum = 0.0;
    for (node, weight) in GAUSS_LEGENDRE_8 {
        let u = mid + half * node;
        sum += weight * taper(u, ln_cutoff) * production.differential(math::exp(u));
    }
    half * sum
}

/// A body's screened integrand: its production, ln `D_c` and the ln D (km) of the production's
/// kinks, fixed per body.
#[derive(Debug, Clone, Copy)]
struct Screened {
    production: Production,
    ln_cutoff: f64,
    ln_kinks: [f64; 4],
}

impl Screened {
    /// The screened integrand of `production` under a cutoff of `cutoff_km`.
    #[must_use]
    fn new(production: Production, cutoff_km: f64) -> Self {
        Self {
            production,
            ln_cutoff: math::ln(cutoff_km),
            ln_kinks: production.kinks().map(math::ln),
        }
    }

    /// `total` plus ∫ f n d ln D from ln D = `lo` to `hi` (D in km): the screened craters per km²
    /// between the two.
    ///
    /// The panels lie on one grid fixed for the body, the multiples of [`PANEL_WIDTH`] in ln D
    /// (km) with the production's kinks added, so that no panel straddles a kink, and are summed
    /// from `hi` down: every panel above the grid point next above `lo` is the same whatever `lo`
    /// is, and the stretch from `lo` to that point is added last. So, at a fixed `hi`, the result
    /// never rises as `lo` rises, to the last bit.
    ///
    /// # Panics
    ///
    /// If `lo` or `hi` is not finite, or `lo` exceeds `hi`.
    #[must_use]
    fn down(&self, lo: f64, hi: f64, total: f64) -> f64 {
        assert!(
            lo.is_finite() && hi.is_finite() && lo <= hi,
            "a screened integral runs between finite bounds in order, not from {lo} to {hi}"
        );
        #[expect(
            clippy::cast_possible_truncation,
            reason = "hi is finite, and |ln D| is under 745 for every positive f64 D, so hi over \
                      the panel width is under 1,300, inside an i32; the floor is exact"
        )]
        let mut k = (hi / PANEL_WIDTH).floor() as i32;
        let mut total = total;
        let mut upper = hi;
        loop {
            let mut grid = f64::from(k) * PANEL_WIDTH;
            while grid >= upper {
                k -= 1;
                grid = f64::from(k) * PANEL_WIDTH;
            }
            let lower = self
                .ln_kinks
                .iter()
                .rev()
                .find(|&&kink| kink < upper)
                .map_or(grid, |&kink| if kink > grid { kink } else { grid });
            if lower <= lo {
                return total + panel(&self.production, self.ln_cutoff, lo, upper);
            }
            total += panel(&self.production, self.ln_cutoff, lower, upper);
            upper = lower;
        }
    }
}

/// The crater cutoff in km of `p`, or none if nothing screens it.
#[must_use]
fn cutoff_km(p: &CraterParams) -> Option<f64> {
    crater_cutoff(p).map(|d| d.value() / 1e3)
}

/// Whether a diameter of `m` metres is one the density can take: finite, and positive in km too,
/// so that its logarithm in km is finite.
#[must_use]
fn diameter_in_range(m: f64) -> bool {
    m.is_finite() && m / 1e3 > 0.0
}

/// Asserts `d` is a diameter in range ([`diameter_in_range`]) and returns it in km.
#[must_use]
fn diameter_km(d: Metres) -> f64 {
    let m = d.value();
    assert!(
        diameter_in_range(m),
        "a crater diameter must be finite and positive, got {m} m"
    );
    m / 1e3
}

/// Whether `screening`'s cutoff over projectiles of density `projectile`, if any, is one the
/// density can integrate to: positive in km, and finite at [`SCREENED_REACH`] times it, so that
/// the quadrature's bounds are finite. Checked by [`CraterParams::new`].
#[must_use]
pub(super) fn screening_in_range(screening: Screening, projectile: KilogramsPerCubicMetre) -> bool {
    screening_cutoff(screening, projectile).is_none_or(|d| {
        let km = d.value() / 1e3;
        km > 0.0 && (km * SCREENED_REACH).is_finite()
    })
}

/// The diameter map's cavity scale of `p`, if it has a map and the scale is not finite and
/// positive: parts each in range whose combination lies beyond the scaling's, such as a density
/// ratio that underflows while the speed's square overflows. Checked by [`CraterParams::new`].
#[must_use]
pub(super) fn map_out_of_range(p: &CraterParams) -> Option<f64> {
    DiameterMap::built(p)
        .map(|map| map.cavity_scale)
        .filter(|&scale| !(scale.is_finite() && scale > 0.0))
}

/// The projectile scale of `screening` over projectiles of density `projectile`, as
/// [`projectile_scale`].
#[must_use]
fn screening_projectile(
    screening: Screening,
    projectile: KilogramsPerCubicMetre,
) -> Option<Metres> {
    match screening {
        Screening::Atmosphere { column_mass } if column_mass.value() > 0.0 => Some(Metres::new(
            PROJECTILE_PER_COLUMN * column_mass.value() / projectile.value(),
        )),
        Screening::None | Screening::Atmosphere { .. } | Screening::Cutoff { .. } => None,
    }
}

/// The crater cutoff of `screening` over projectiles of density `projectile`, as
/// [`crater_cutoff`].
#[must_use]
fn screening_cutoff(screening: Screening, projectile: KilogramsPerCubicMetre) -> Option<Metres> {
    match screening {
        Screening::Cutoff { diameter } => Some(diameter),
        Screening::None | Screening::Atmosphere { .. } => {
            screening_projectile(screening, projectile).map(|d| d * CUTOFF_PER_PROJECTILE)
        }
    }
}

/// The projectile scale d\* = 1.5 (P ÷ g) ÷ `ρ_p` of a body screened by its atmosphere, the size of
/// projectile that meets about its own mass of air before it reaches the ground, `ρ_p` the
/// contract's [`projectile_density`](CraterParams::projectile_density).
///
/// About 5.2 m for Earth, 0.52 km for Venus and 8.5 cm for Mars at 636 Pa, at a projectile density
/// of 3,000 kg m⁻³ (Design note 12, after Melosh 1989); six times as large for comets of 500. None
/// for [`Screening::None`], for an atmosphere of no column mass, and for a cutoff given directly
/// ([`Screening::Cutoff`]), which names no projectile.
#[must_use]
pub fn projectile_scale(p: &CraterParams) -> Option<Metres> {
    screening_projectile(p.screening(), p.projectile_density())
}

/// The crater cutoff `D_c` of a screened body: 20 d\* ([`projectile_scale`]) under an atmosphere,
/// or [`Screening::Cutoff`]'s diameter.
///
/// About 100 m for Earth, 10.4 km for Venus and 1.7 m for Mars (Design note 12). The differential
/// production is tapered by 1 ÷ (1 + (`D_c` ÷ D)^4.2), which passes half of it at `D_c`. None when
/// nothing screens the body: [`Screening::None`], or an atmosphere of no column mass.
#[must_use]
pub fn crater_cutoff(p: &CraterParams) -> Option<Metres> {
    screening_cutoff(p.screening(), p.projectile_density())
}

/// The body's crater production above `d` before screening, N(>D) per km².
///
/// Neukum, Ivanov and Hartmann 2001's lunar polynomial at the contract's lunar-equivalent
/// N☾(>1 km), through the diameter map when `p` has an impact velocity ([the module
/// documentation](self)). It is N☾(>1 km) exactly at 1 km at the lunar reference, the body's own
/// N(>1 km) ([`own_density_at_1km`]) under a map, and strictly decreasing in D.
///
/// # Panics
///
/// If `d` is not finite and positive (in km too: from about 10⁻³²⁰ m).
#[must_use]
pub fn production_density(p: &CraterParams, d: Metres) -> PerSquareKilometre {
    PerSquareKilometre::new(Production::of(p).cumulative(diameter_km(d)))
}

/// The body's own crater density above 1 km before screening, per km²: the lunar-equivalent
/// N☾(>1 km) carried to the body's craters by the diameter map, N☾ × 10^(Σ aₙ (log₁₀ φ(1 km))ⁿ)
/// (science-r09-craters.md F1), for tests and display.
///
/// It is [`CraterParams::n_1km`] itself at the lunar reference and without an impact velocity.
/// About 0.53 of it on Venus at 25.7 km/s and 0.26 on Mars at Ivanov 2001's velocities: a body
/// struck more slowly, or under a stronger gravity, makes smaller craters of the same projectiles.
#[must_use]
pub fn own_density_at_1km(p: &CraterParams) -> PerSquareKilometre {
    production_density(p, Metres::new(1e3))
}

/// The body's cumulative crater density N(>D) per km², the craters of diameter `d` and wider that
/// its surface accumulates over its age.
///
/// It is Design note 12's one density for the coarse pass, the synthesis and plan 14:
/// [`production_density`] where nothing screens the body, so N(>1 km) is the contract's N☾ exactly
/// at the lunar reference and the body's own ([`own_density_at_1km`]) under a map. Under an
/// atmosphere it is the integral of the differential production tapered by
/// 1 ÷ (1 + (`D_c` ÷ D)^4.2) ([`crater_cutoff`]), by fixed quadrature ([the module
/// documentation](self)): Venus's at N☾(>1 km) = 3.36 × 10⁻⁴ km⁻² and 25.7 km/s is about
/// 1.9 × 10⁻⁶ km⁻² at 1 km, 1.1% of its own production. It is non-increasing in D, and not capped
/// at [`saturation`], which the caller applies per octave.
///
/// # Examples
///
/// Plan 14's crater contract for an airless body at the lunar reference gives back its N(>1 km)
/// at 1 km, and fewer craters at every larger diameter:
///
/// ```
/// use hyperion_base::units::{Metres, MetresPerSecondSquared, PerSquareKilometre};
/// use hyperion_surface::craters::{
///     CraterParams, CraterParamsParts, LUNAR_PROJECTILE_DENSITY, LUNAR_TARGET, SOLAR_SYSTEM_AGE,
///     Screening, cumulative_density,
/// };
///
/// let moon = CraterParams::new(CraterParamsParts {
///     n_1km: PerSquareKilometre::new(0.018),
///     screening: Screening::None,
///     gravity: MetresPerSecondSquared::new(1.62),
///     k_target: 1.0,
///     impact_velocity: None,
///     projectile_density: LUNAR_PROJECTILE_DENSITY,
///     target: LUNAR_TARGET,
///     system_age: SOLAR_SYSTEM_AGE,
/// })?;
/// let at_1_km = cumulative_density(&moon, Metres::new(1e3));
/// assert_eq!(at_1_km, PerSquareKilometre::new(0.018));
/// assert!(cumulative_density(&moon, Metres::new(10e3)) < at_1_km);
/// # Ok::<(), hyperion_surface::craters::BuildCraterParamsError>(())
/// ```
///
/// # Panics
///
/// If `d` is not finite and positive (in km too: from about 10⁻³²⁰ m).
#[must_use]
pub fn cumulative_density(p: &CraterParams, d: Metres) -> PerSquareKilometre {
    let d_km = diameter_km(d);
    let production = Production::of(p);
    let Some(cutoff) = cutoff_km(p) else {
        return PerSquareKilometre::new(production.cumulative(d_km));
    };
    let lo = math::ln(d_km);
    let hi = math::ln(cutoff * SCREENED_REACH);
    if lo >= hi {
        return PerSquareKilometre::new(production.cumulative(d_km));
    }
    let tail = production.cumulative(math::exp(hi));
    PerSquareKilometre::new(Screened::new(production, cutoff).down(lo, hi, tail))
}

/// Trask's crater saturation, the equilibrium density N(>D) = 0.079 D⁻² km⁻² with D in km.
///
/// Above it a surface's craters erase as many as form (Trask 1966, JPL Technical Report 32-800,
/// p. 263, eq. 1, rounded): the cap on the expected count of each octave (Design notes 10 and
/// 12).
///
/// # Panics
///
/// If `d` is not finite and positive (in km too: from about 10⁻³²⁰ m).
#[must_use]
pub fn saturation(d: Metres) -> PerSquareKilometre {
    let d_km = diameter_km(d);
    PerSquareKilometre::new(SATURATION_PER_KM2_AT_1_KM / (d_km * d_km))
}

/// A diameter octave, the craters from its lower diameter up to twice it: the unit in which the
/// synthesis draws its small craters (Design note 13).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DiameterOctave {
    lower: Metres,
}

/// Why [`DiameterOctave::new`] refused its lower diameter: it is not a diameter the density can
/// take, or twice it is not finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildDiameterOctaveError(f64);

impl BuildDiameterOctaveError {
    /// The lower diameter refused, metres.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for BuildDiameterOctaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "an octave's lower diameter {} m is not finite and positive, or twice it is not finite",
            self.0
        )
    }
}

impl std::error::Error for BuildDiameterOctaveError {}

impl DiameterOctave {
    /// The octave of the craters from `lower` up to 2 `lower`.
    ///
    /// # Errors
    ///
    /// [`BuildDiameterOctaveError`] if `lower` is not finite and positive (in km too: from about
    /// 10⁻³²⁰ m), or twice it overflows.
    pub fn new(lower: Metres) -> Result<Self, BuildDiameterOctaveError> {
        let d = lower.value();
        if diameter_in_range(d) && (2.0 * d).is_finite() {
            Ok(Self { lower })
        } else {
            Err(BuildDiameterOctaveError(d))
        }
    }

    /// The octave's smallest diameter, which it includes.
    #[must_use]
    pub const fn lower(&self) -> Metres {
        self.lower
    }

    /// The octave's largest diameter, twice its lower, which it excludes.
    #[must_use]
    pub fn upper(&self) -> Metres {
        self.lower * 2.0
    }
}

/// A uniform variate in [0, 1), which [`diameter_in_octave`] turns into a diameter.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct UnitUniform(f64);

/// Why [`UnitUniform::new`] refused a value: it is not in [0, 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildUnitUniformError(f64);

impl BuildUnitUniformError {
    /// The value refused.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for BuildUnitUniformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "a unit uniform {} is not in [0, 1)", self.0)
    }
}

impl std::error::Error for BuildUnitUniformError {}

impl UnitUniform {
    /// The variate `u`.
    ///
    /// # Errors
    ///
    /// [`BuildUnitUniformError`] if `u` is not in [0, 1).
    pub fn new(u: f64) -> Result<Self, BuildUnitUniformError> {
        if (0.0..1.0).contains(&u) {
            Ok(Self(u))
        } else {
            Err(BuildUnitUniformError(u))
        }
    }

    /// The value, in [0, 1).
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// The diameter within `octave` below which a share `u` of the octave's craters lie, by the
/// body's [`cumulative_density`].
///
/// It inverts the density's distribution within the octave, so that a uniform `u` draws a crater
/// of the octave by the body's size–frequency distribution (Design note 13). It bisects in ln D
/// with a fixed 40 steps, the same work for every `u` on every target, comparing the craters
/// between the octave's lower diameter and the midpoint with `u` of the octave's. Under an
/// atmosphere each count is the tapered production's integral over that stretch alone, so the
/// density above the octave never enters. The result lies in the octave: at least its lower
/// diameter, and below its upper.
#[must_use]
pub fn diameter_in_octave(p: &CraterParams, octave: DiameterOctave, u: UnitUniform) -> Metres {
    let production = Production::of(p);
    let lower_km = octave.lower().value() / 1e3;
    let upper_km = octave.upper().value() / 1e3;
    let lo = math::ln(lower_km);
    let hi = math::ln(upper_km);
    let screened = cutoff_km(p).map(|cutoff| Screened::new(production, cutoff));
    let below = production.cumulative(lower_km);
    // The craters per km² from the octave's lower diameter to e^v km.
    let between = |v: f64| match &screened {
        Some(screened) => screened.down(lo, v, 0.0),
        None => below - production.cumulative(math::exp(v)),
    };
    let target = u.get() * between(hi);
    let (mut a, mut b) = (lo, hi);
    for _ in 0..BISECTION_STEPS {
        let mid = f64::midpoint(a, b);
        if between(mid) < target {
            a = mid;
        } else {
            b = mid;
        }
    }
    let d_km = math::exp(f64::midpoint(a, b));
    Metres::new((d_km * 1e3).clamp(octave.lower().value(), octave.upper().value().next_down()))
}

#[cfg(test)]
mod tests;
