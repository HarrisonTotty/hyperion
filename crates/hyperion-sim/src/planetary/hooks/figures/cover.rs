//! What covers a body's surface (plan 14, P14.T24.b, with P14.T48.a's hypsometry): its surface
//! water and the ocean it fills, its ice and its cloud (science-p14-t24b-laws §4.2–4.4).
//!
//! # The ocean (§4.2)
//!
//! - **The surface water** `σ_s` ([`surface_water`]): P14.T13.a's water over 4πR², less the
//!   vapour P14.T13.c holds in the air, while the present outgassing O is at least P14.T48.b's
//!   edge
//!   ([`VOLCANIC_GREENHOUSE_OUTGASSING`](super::wet_epoch::VOLCANIC_GREENHOUSE_OUTGASSING)), the
//!   volcanic greenhouse still running, and [`DRY_SURFACE_WATER_SHARE`] of it after: a world whose
//!   volcanic supply has failed has put most of its water into its crust and ground ice, or lost
//!   it, as Mars has.
//! - **The potential ocean** `f_pot` ([`potential_ocean`]): the area that water fills of
//!   P14.T48.a's own hypsometry ([`Hypsometry`]), each population a Gaussian truncated at ±3σ, to
//!   the level whose volume is the liquid's, `σ_s` ÷ `ρ_l` (T49.a's
//!   [`triple_liquid_density`](crate::substance::PhaseColumns::triple_liquid_density)). It reaches
//!   1, an ocean world, when the level tops the upper population, and is continuous throughout.
//!   It needs no width or capacity constant, which the logistic it replaces would have had to fit.
//! - **The published ocean** `f_o` is `f_pot` where the body holds liquid now (P14.T24.a's
//!   liquid-water flag, or a `Snowball` whose water outruns its conducting ice,
//!   [`under_ice_ocean`]), and zero otherwise
//!   ([`GlobalFigures::with_surface`](super::GlobalFigures::with_surface)).
//!
//! `θ`, the ocean's weight in P14.T24.a's contrasts, reads `f_pot` ([`OceanWeight`]), not the
//! published `f_o`, which waits on the liquid flag `θ` feeds: the order has no loop (§4.6).
//!
//! # The ice (§4.3)
//!
//! Perennial water ice covers the area where P14.T24.a's annual-mean zonal profile lies below
//! [`PERENNIAL_ICE_TEMPERATURE`] under air or [`AIRLESS_ICE_TEMPERATURE`] airless
//! ([`SurfaceConditions::cold_area`]); a world locked 1:1 to its seasonal host takes the eyeball's
//! cap from its day and night means, and a `Snowball` the whole sphere. On a silicate crust the
//! cover is at most the surface water at [`ICE_SHEET_THICKNESS`] of ice; an ice-rich crust is its
//! own ice, uncapped ([`ice_fraction`]).
//!
//! # The cloud (§4.4)
//!
//! P14.T13.c's [`SurfaceState::cloud_fraction`], unchanged, since T13.c's Bond albedo was solved
//! with it: a cover weighted by θ or the climate here would publish one the albedo does not have.

use crate::math;
use crate::planetary::derive::atmosphere::{Atmosphere, Gas, SurfaceState};
use crate::planetary::hooks::figures::Crust;
use crate::planetary::hooks::figures::interior::REGIME_WATER_DENSITY;
use crate::planetary::hooks::figures::relief::Hypsometry;
use crate::planetary::hooks::figures::wet_epoch::VOLCANIC_GREENHOUSE_OUTGASSING;
#[cfg(doc)]
use crate::planetary::hooks::surface::OceanWeight;
use crate::planetary::hooks::surface::SurfaceConditions;
use crate::planetary::moons::regular::ICE_CONDUCTIVITY_COEFFICIENT;
use crate::substance;
use crate::units::{
    Kelvin, Kilograms, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres,
    MetresPerSecondSquared, Pascals, WattsPerSquareMetre,
};

#[cfg(test)]
pub(super) mod tests;

/// The share of P14.T13.a's water left at the surface of a world whose volcanic greenhouse has
/// failed: 0.02, the generated Mars's 20 m of polar deposits (Carr and Head 2003's low end, R09's
/// reference) out of its about 1 km (science-p14-t24b-laws §4.2). The rest is in its crust and
/// ground ice, or lost (Scheller et al. 2021, Science 372, 56: 30–99% into the crust; from
/// memory). Calibrated on Mars alone, low confidence; P14.T51.f's cold traps supersede it for ices.
pub const DRY_SURFACE_WATER_SHARE: f64 = 0.02;

/// The mean annual surface temperature below which water ice lasts the year under air: 263.15 K,
/// −10 °C, the classical annual-mean ice edge of energy-balance models, standing for the seasonal
/// cycle (Budyko 1969, Tellus 21, 611; North 1975, J. Atmos. Sci. 32, 1301; North, Cahalan and
/// Coakley 1981, Rev. Geophys. 19, 91; from memory; science-p14-t24b-laws §4.3).
pub const PERENNIAL_ICE_TEMPERATURE: Kelvin = Kelvin::new(263.15);

/// The temperature below which water ice lasts on an airless body: 110 K, where its Hertz–Knudsen
/// sublimation is 144 kg m⁻² Gyr⁻¹ (Schorghofer and Williams 2020, PSJ 1, 54, Table 1; R09's
/// `COLD_TRAP_H2O_K`, P14.T51.f's threshold; science-p14-t24b-laws §4.3). It is a peak
/// temperature's threshold, read here against the annual-mean profile, which overstates the cold
/// area where days and seasons swing (plan 14's Risks; P14.T51.f's cold traps refine it).
pub const AIRLESS_ICE_TEMPERATURE: Kelvin = Kelvin::new(110.0);

/// The thickness of ice a silicate crust's surface water is spread at to cap its ice cover: 1.93
/// km, the ruling's rounding of Antarctica's mean, 1,937 m, 26.92 × 10⁶ km³ over 13.924 × 10⁶ km²
/// (Fretwell et al. 2013, The Cryosphere 7, 375, Bedmap2, Table 7; science-p14-t24b-laws §4.3).
pub const ICE_SHEET_THICKNESS: Metres = Metres::new(1_930.0);

/// The melting point at the base of a `Snowball`'s conducting ice: 273.15 K
/// (science-p14-t24b-laws §4.2), water's at 1 atm (IAPWS R10-06's 273.152 519 K, to 0.01 K).
pub const UNDER_ICE_MELTING_POINT: Kelvin = Kelvin::new(273.15);

/// How many halvings locate the ocean's level: 64, always that many, past the last bit of the
/// populations' span (science-p14-t24b-laws §4.2's "64 halvings over ±50 km").
pub const OCEAN_LEVEL_HALVINGS: u32 = 64;

/// The half-width of each population of the hypsometry, in its widths: 3, a truncation keeping
/// 99.73% of a Gaussian's area (science-p14-t24b-laws §4.2).
const TRUNCATION: f64 = 3.0;

/// The melting point of water ice, the triple temperature of water's registry row: 273.16 K, the
/// solid–liquid line of the registry's diagram, which P14.T24.a's liquid flag reads too, so the
/// ice and the flag share one line (science-p14-t24b-laws §4.x.2).
///
/// # Panics
///
/// Never: water's registry row carries its phase columns (the registry's tests).
#[must_use]
pub fn melting_point() -> Kelvin {
    Gas::Water
        .substance()
        .substance()
        .phase()
        .expect("water's registry row carries its phase columns")
        .triple_temperature()
}

/// The density of liquid water at its triple point, kg m⁻³: T49.a's row (P14.T24.b's `ρ_l`, which
/// T49.b's `ρ_l`(T) supersedes).
///
/// # Panics
///
/// Never: water's registry row carries its phase columns (the registry's tests).
#[must_use]
pub fn liquid_water_density() -> KilogramsPerCubicMetre {
    Gas::Water
        .substance()
        .substance()
        .phase()
        .expect("water's registry row carries its phase columns")
        .triple_liquid_density()
}

/// The surface water `σ_s` of a body of atmosphere `atmosphere` holding P14.T13.a's `water` over a
/// radius `radius` at a surface gravity `surface_gravity`, at the present outgassing `outgassing`
/// (science-p14-t24b-laws §4.2), kg m⁻²: the water condensed at the surface, over 4πR², while O is
/// at least [`VOLCANIC_GREENHOUSE_OUTGASSING`], and [`DRY_SURFACE_WATER_SHARE`] of it after.
///
/// The condensed water is T13.a's less what P14.T13.c holds in the air: T13.c keeps water vapour
/// at its saturation pressure at the mean surface temperature, the rest condensed, so the surface
/// holds `M` ÷ 4πR² − `p_H₂O` ÷ g where the air is saturated, and none where all the water is
/// vapour, a steam atmosphere's: continuous at the edge, where the surface's share falls to
/// nothing. None too for a surface that holds no water: a body that keeps no atmosphere or no H₂O
/// (P14.T13.b), a runaway greenhouse, which has lost its oceans (P14.T13.c), and a molten or
/// enveloped one, which has no solid surface (the lane's readings of "T13.a's water", plan 14's
/// Risks). It reads O and T13.c's state, not the climate, so the order has no loop.
///
/// # Examples
///
/// The same Earth-like water, 2.7 × 10⁶ kg m⁻² over its surface, is all at the surface of a world
/// outgassing as Earth does, but for the vapour in its air, and 2% of it once its volcanoes fail:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::SurfaceState;
/// use hyperion_sim::planetary::hooks::figures::cover::{DRY_SURFACE_WATER_SHARE, surface_water};
/// use hyperion_sim::units::MetresPerSecondSquared;
/// # use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// # use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
/// # use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// # use hyperion_sim::stellar::Composition;
/// # use hyperion_sim::stellar::draws::UnitUniform;
/// # use hyperion_sim::time::UniverseTime;
/// # use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// # use hyperion_sim::units::{
/// #     Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres, Radians,
/// #     SolarLuminosities, SolarMasses, SolarRadii, Years,
/// # };
/// # let host = DiscHost::new(SolarMasses::new(1.0), Dex::new(0.0), SolarLuminosities::new(0.7), SolarRadii::new(0.89))?;
/// # let disc = disc::derive(&host, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
/// # let disc = disc.profile().expect("a median disc exists");
/// # let a = Metres::new(METRES_PER_AU);
/// # let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
/// # let orientation = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?;
/// # let orbit = KeplerElements::from_semi_major_axis(a, mu, Eccentricity::new(0.0167)?, orientation, Radians::ZERO)?;
/// # let earth = PlacedBody::new(EarthMasses::new(1.0), orbit, a, UnitUniform::new(0.384).expect("inside (0, 1)"))?;
/// # let sun = [HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?];
/// # let hosts = BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &sun, &[])?;
/// # let derived = derive_body(&earth, &hosts, disc, Years::new(4.57e9), UniverseTime::EPOCH)?;
/// let air = derived.atmosphere();
/// assert_eq!(air.state(), SurfaceState::Temperate);
/// let (water, radius, g) = (Kilograms::new(1.4e21), Metres::new(6.371e6), MetresPerSecondSquared::new(9.82));
/// let wet = surface_water(air, water, radius, g, 1.0).value();
/// assert!((wet / 2.745e6 - 1.0).abs() < 0.01, "{wet}");
/// let dry = surface_water(air, water, radius, g, 0.3).value();
/// assert!((dry / wet - DRY_SURFACE_WATER_SHARE).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn surface_water(
    atmosphere: &Atmosphere,
    water: Kilograms,
    radius: Metres,
    surface_gravity: MetresPerSecondSquared,
    outgassing: f64,
) -> KilogramsPerSquareMetre {
    let holds = match atmosphere.state() {
        SurfaceState::Temperate | SurfaceState::Snowball => true,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse => false,
    };
    let (kg, r, g) = (water.value(), radius.value(), surface_gravity.value());
    // By comparison, so that a NaN water, radius or gravity holds none.
    if !(holds && atmosphere.retention().retains(Gas::Water) && kg > 0.0 && r > 0.0 && g > 0.0) {
        return KilogramsPerSquareMetre::ZERO;
    }
    // T13.c's test of a gas held at saturation, as P14.T24.a's greenhouse gain takes it: equal to
    // the bit where the air holds all it can.
    let vapour = atmosphere.partial_pressures().of(Gas::Water);
    let saturation = Gas::Water
        .substance()
        .substance()
        .phase()
        .map_or(Pascals::new(f64::INFINITY), |phase| {
            phase.saturation_pressure(atmosphere.surface_temperature())
        });
    if vapour < saturation {
        return KilogramsPerSquareMetre::ZERO;
    }
    let condensed = kg / (4.0 * core::f64::consts::PI * r * r) - vapour.value() / g;
    if condensed.is_nan() || condensed <= 0.0 {
        return KilogramsPerSquareMetre::ZERO;
    }
    let share = if outgassing >= VOLCANIC_GREENHOUSE_OUTGASSING {
        1.0
    } else {
        DRY_SURFACE_WATER_SHARE
    };
    KilogramsPerSquareMetre::new(condensed * share)
}

/// The standard normal's cumulative distribution Φ(z).
#[must_use]
fn normal_cdf(z: f64) -> f64 {
    0.5 * math::erfc(-z / core::f64::consts::SQRT_2)
}

/// The standard normal's density φ(z).
#[must_use]
fn normal_density(z: f64) -> f64 {
    math::exp(-0.5 * z * z) / (2.0 * core::f64::consts::PI).sqrt()
}

/// The truncated standard normal's cumulative distribution `Φ_t`(z): (Φ(z) − Φ(−3)) ÷ (Φ(3) −
/// Φ(−3)), 0 below −3 and 1 above 3.
#[must_use]
fn truncated_cdf(z: f64) -> f64 {
    if z.is_nan() || z <= -TRUNCATION {
        return 0.0;
    }
    if z >= TRUNCATION {
        return 1.0;
    }
    let low = normal_cdf(-TRUNCATION);
    (normal_cdf(z) - low) / (normal_cdf(TRUNCATION) - low)
}

/// The truncated standard normal's mean depth below a level z, `G_t`(z) = ∫ from −3 to z of
/// `Φ_t`(u) du: (z Φ(z) + φ(z) + 3 Φ(−3) − φ(−3) − Φ(−3)(z + 3)) ÷ (Φ(3) − Φ(−3)), 0 below −3 and
/// z above 3, where the whole population, of mean 0, lies below.
#[must_use]
fn truncated_depth(z: f64) -> f64 {
    if z.is_nan() || z <= -TRUNCATION {
        return 0.0;
    }
    if z >= TRUNCATION {
        return z;
    }
    let (low, high) = (normal_cdf(-TRUNCATION), normal_cdf(TRUNCATION));
    let edge = normal_density(-TRUNCATION);
    (z * normal_cdf(z) + normal_density(z) + TRUNCATION * low - edge - low * (z + TRUNCATION))
        / (high - low)
}

impl Hypsometry {
    /// The share of the surface lying below the level `level` over the datum, 0 to 1:
    /// Σₚ wₚ `Φ_t`((h − μₚ) ÷ σₚ).
    #[must_use]
    pub fn area_below(&self, level: Metres) -> f64 {
        self.populations().iter().fold(0.0, |sum, p| {
            sum + p.weight * truncated_cdf((level - p.mean) / p.width)
        })
    }

    /// The volume per unit area of the surface below the level `level`, m: Σₚ wₚ σₚ
    /// `G_t`((h − μₚ) ÷ σₚ), the global layer that fills it to that level.
    #[must_use]
    pub fn volume_below(&self, level: Metres) -> Metres {
        Metres::new(self.populations().iter().fold(0.0, |sum, p| {
            sum + p.weight * p.width.value() * truncated_depth((level - p.mean) / p.width)
        }))
    }

    /// The lowest and highest points of the populations that hold any of the surface: their
    /// means less and plus three widths.
    #[must_use]
    fn span(&self) -> (Metres, Metres) {
        self.populations().iter().filter(|p| p.weight > 0.0).fold(
            (Metres::new(f64::INFINITY), Metres::new(f64::NEG_INFINITY)),
            |(low, high), p| {
                let (bottom, top) = (p.mean - p.width * TRUNCATION, p.mean + p.width * TRUNCATION);
                (
                    if bottom < low { bottom } else { low },
                    if top > high { top } else { high },
                )
            },
        )
    }

    /// The level, over the datum, to which a global layer `depth` deep fills the hypsometry, and
    /// the share of the surface below it (science-p14-t24b-laws §4.2): `σ_s` ÷ `ρ_l` of water fills
    /// it to the level h whose [`volume_below`](Self::volume_below) is the layer.
    ///
    /// The level is found by [`OCEAN_LEVEL_HALVINGS`] halvings over the populations' span. A layer
    /// that is not positive fills nothing; one at least the volume below the span's top covers
    /// the whole surface, its level the top plus what is left over.
    ///
    /// # Examples
    ///
    /// Earth's 1.335 × 10⁹ km³ of ocean, a 2,617 m global layer, fills 73% of Earth2014's two
    /// crustal populations, its level some 2.4 km over the mean surface, as Earth's sea level
    /// stands over its mean elevation:
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
    ///     crater_density: PerSquareKilometre::ZERO,
    ///     volcanism: 0.0,
    ///     lithosphere: Metres::ZERO,
    /// });
    /// let (level, ocean) = earth.hypsometry().fill(Metres::new(2_617.0));
    /// assert!((ocean - 0.731).abs() < 0.001, "{ocean}");
    /// assert!(level.value() > 2_000.0 && level.value() < 2_800.0, "{}", level.value());
    /// ```
    #[must_use]
    pub fn fill(&self, depth: Metres) -> (Metres, f64) {
        let (bottom, top) = self.span();
        if !(depth.value() > 0.0 && bottom.value().is_finite()) {
            return (bottom, 0.0);
        }
        let full = self.volume_below(top);
        if depth >= full {
            return (top + (depth - full), 1.0);
        }
        let (mut low, mut high) = (bottom.value(), top.value());
        for _ in 0..OCEAN_LEVEL_HALVINGS {
            let middle = low + (high - low) * 0.5;
            if self.volume_below(Metres::new(middle)) < depth {
                low = middle;
            } else {
                high = middle;
            }
        }
        let level = Metres::new(low + (high - low) * 0.5);
        (level, self.area_below(level))
    }
}

/// The potential ocean `f_pot` of the surface water `surface_water` on the hypsometry
/// `hypsometry`, 0 to 1 (science-p14-t24b-laws §4.2): the share of the surface its liquid,
/// `σ_s` ÷ `ρ_l` ([`liquid_water_density`]), fills ([`Hypsometry::fill`]).
///
/// Earth's 2.6 km on its two populations gives 0.731 (0.7095 measured, Eakins and Sharman 2010;
/// the generated Earth's 2.69 km, 0.746), where one Gaussian of its `σ_h` would give 0.831; the
/// ocean world comes at about 6 km on an Earth and 2.7 km on a 0.9 km stagnant lid.
///
/// # Examples
///
/// A Mars's 20 m of surface water fills the deepest few per cent of a 3 km stagnant lid, and a
/// layer of three of its widths, 9.1 km, drowns it:
///
/// ```
/// use hyperion_sim::planetary::hooks::figures::cover::potential_ocean;
/// use hyperion_sim::planetary::hooks::figures::relief::{Relief, ReliefInputs};
/// use hyperion_sim::planetary::hooks::figures::{Crust, TectonicRegime};
/// use hyperion_sim::units::{
///     KilogramsPerSquareMetre, Metres, MetresPerSecondSquared, PerSquareKilometre,
/// };
///
/// let mars = Relief::of(&ReliefInputs {
///     regime: TectonicRegime::StagnantLid,
///     continental_fraction: 0.0,
///     crust: Crust::Silicate,
///     surface_gravity: MetresPerSecondSquared::new(3.71),
///     radius: Metres::new(3.39e6),
///     crater_density: PerSquareKilometre::new(0.1),
///     volcanism: 1.0,
///     lithosphere: Metres::new(100e3),
/// });
/// let wet = |kg: f64| potential_ocean(mars.hypsometry(), KilogramsPerSquareMetre::new(kg));
/// assert!((0.01..0.05).contains(&wet(2.0e4)), "{}", wet(2.0e4));
/// assert!((wet(2.6e6) - 1.0).abs() > 0.1);
/// assert!((wet(9.2e6) - 1.0).abs() < 1e-12);
/// ```
#[must_use]
pub fn potential_ocean(hypsometry: &Hypsometry, surface_water: KilogramsPerSquareMetre) -> f64 {
    let depth = Metres::new(surface_water.value() / liquid_water_density().value());
    hypsometry.fill(depth).1
}

/// Whether a `Snowball` holds an ocean under its ice (science-p14-t24b-laws §4.2): its surface
/// water's global layer, `σ_s` ÷ [`REGIME_WATER_DENSITY`], outruns the ice that conducts its heat
/// flow F from [`UNDER_ICE_MELTING_POINT`] to its surface at `surface_temperature`, 651 ln(273.15 K
/// ÷ `T_s`) ÷ F ([`ICE_CONDUCTIVITY_COEFFICIENT`]). False for any other state, and with no heat
/// flow.
#[must_use]
pub fn under_ice_ocean(
    state: SurfaceState,
    surface_water: KilogramsPerSquareMetre,
    surface_temperature: Kelvin,
    heat_flow: WattsPerSquareMetre,
) -> bool {
    match state {
        SurfaceState::Snowball => {}
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate => return false,
    }
    let (f, t_s) = (heat_flow.value(), surface_temperature.value());
    if !(f > 0.0 && t_s > 0.0) {
        return false;
    }
    let layer = surface_water.value() / REGIME_WATER_DENSITY.value();
    let shell = ICE_CONDUCTIVITY_COEFFICIENT * math::ln(UNDER_ICE_MELTING_POINT.value() / t_s) / f;
    layer > shell
}

/// The temperature below which water ice lasts at the surface of a body in the state `state`:
/// [`AIRLESS_ICE_TEMPERATURE`] for an airless one, and [`PERENNIAL_ICE_TEMPERATURE`] under air.
#[must_use]
pub const fn ice_temperature(state: SurfaceState) -> Kelvin {
    match state {
        SurfaceState::Airless => AIRLESS_ICE_TEMPERATURE,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Snowball => PERENNIAL_ICE_TEMPERATURE,
    }
}

/// The temperature below which the climate of a body in the state `state` holds its ice cover
/// ([`ice_fraction`]): a `Snowball`'s melting point ([`melting_point`]; science-p14-t24b-laws
/// §4.x.2), and otherwise [`ice_temperature`].
///
/// P14.T48.d's climate regime reads it to tell an equatorial belt from caps.
#[must_use]
pub fn ice_threshold(state: SurfaceState) -> Kelvin {
    match state {
        SurfaceState::Snowball => melting_point(),
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate => ice_temperature(state),
    }
}

/// What [`ice_fraction`] reads of a body, as plain values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IceInputs<'a> {
    /// Its surface conditions (P14.T24.a's, at its θ): the zonal profile, or a locked world's day
    /// and night, and its mean surface temperature.
    pub conditions: &'a SurfaceConditions,
    /// Its surface state (P14.T13.c's): a `Snowball` is wholly iced, and an airless body's ice
    /// lasts below [`AIRLESS_ICE_TEMPERATURE`].
    pub state: SurfaceState,
    /// Its crust: a silicate one's ice is capped by its surface water.
    pub crust: Crust,
    /// Its surface water `σ_s` ([`surface_water`]).
    pub surface_water: KilogramsPerSquareMetre,
}

/// The fraction of a body's surface under perennial water ice, 0 to 1 (P14.T24.b;
/// science-p14-t24b-laws §4.3):
///
/// - a `Snowball`'s cold area below the melting point ([`melting_point`]; science-p14-t24b-laws
///   §4.x.2), uncapped, as T13.c's 0.50 albedo takes it: a `Snowball`'s ice lasts until its
///   warmest zone reaches melting, so a correctly labelled one is wholly iced, and one T13.c's
///   flux labels a `Snowball` above melting (a 516 K world) holds none;
/// - otherwise the area where the climate lies below [`ice_temperature`]: the annual-mean zonal
///   profile's cold area, or a world locked 1:1's eyeball cap
///   ([`SurfaceConditions::cold_area`]);
/// - on any other silicate crust at most `σ_s` ÷ (`ρ_ice` × [`ICE_SHEET_THICKNESS`]), `ρ_ice` ice Ih's at
///   the mean surface temperature ([`ice_ih_density`](crate::substance::ice_ih_density)): the
///   generated Mars's 20 m of water gives 0.011, against its residual caps' 0.007, and Earth's 2.6
///   km does not bind. An ice-rich crust is its own ice, uncapped.
///
/// Each threshold is [`ice_threshold`]'s. The climate regime (P14.T48.d,
/// [`regime`](crate::planetary::hooks::surface::regime)) reads this cover, and the cover reads no
/// regime: the regime's condensable is water here, which P14.T51.a widens, and an
/// [`IceBelt`](crate::planetary::hooks::surface::Forcing::IceBelt)'s band is the profile's own cold
/// equator, with no extra hysteresis (science-p14-t24b-laws §4.3), so a high-obliquity world's
/// ice is its profile's cold area whatever its history.
/// [`GlobalFigures::with_surface`](super::GlobalFigures::with_surface)'s example publishes it on
/// an Earth. Ice and ocean may overlap (sea ice over an
/// ocean), as R09's cover convention has it.
#[must_use]
pub fn ice_fraction(inputs: &IceInputs<'_>) -> f64 {
    let cold = inputs.conditions.cold_area(ice_threshold(inputs.state));
    match inputs.state {
        SurfaceState::Snowball => return cold,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate => {}
    }
    match inputs.crust {
        Crust::IceRich => cold,
        Crust::Silicate => {
            let density = substance::ice_ih_density(inputs.conditions.mean_surface_temperature());
            let cap =
                inputs.surface_water.value() / (density.value() * ICE_SHEET_THICKNESS.value());
            if cold < cap { cold } else { cap }
        }
    }
}
