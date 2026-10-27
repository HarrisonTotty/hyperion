//! Thin and transient air (plan 14, P14.T53.c; decision-triton-and-e101 §1.8): the ice-held
//! volatiles' retention and their ice's own temperature, and the `Tenuous` state's floor.
//!
//! Every piece is a closed form of plain quantities, read by
//! [`atmosphere`](crate::planetary::derive::atmosphere) on the snapshot path under the escape's
//! batch rules and this module's own hold (`TenuousRules`, `TENUOUS_VERSION`). Under the hold, to
//! generator version 23, none of it is read.
//!
//! - **An ice-held volatile's retention.** P14.T13.b's threshold (`JEANS_RETENTION_THRESHOLD`) is
//!   a calibrated classifier of whether a bulk air survives, not a flux. A condensable that fails
//!   it but whose saturation pressure at its ice's temperature lies below both its inventory's
//!   weight and the 100 Pa line, so that it cannot make a bulk air, is held by its ice instead, the
//!   air its ice's thin vapour, and keeps its inventory less that air's escape over the system's
//!   age ([`ice_held_loss`]; Schaller and Brown 2007, ApJ 659, L61; Johnson et al. 2015, ApJ 809,
//!   43; Zahnle and Catling 2017, ApJ 843, 122, §2.1, whose "Clausius-Clapeyron atmospheres" name
//!   Pluto and Triton). The escape is Jeans's at the exobase of the cold upper atmosphere such an
//!   air has, [`UPPER_ATMOSPHERE_MULTIPLE`] times the ice's temperature
//!   ([`vapour_escape_molecules_per_second`]); one with no exobase escapes hydrodynamically and its
//!   ice keeps nothing.
//! - **The ice's temperature** ([`ice_temperature`]). An ice-held condensable's controlling
//!   temperature, in place of the partition's coldest zone (P14.T51.a), is one temperature over all
//!   of its ice, which latent heat keeps uniform (Gladstone et al. 2016, Science 351, aad8866;
//!   Young, Braga-Ribas and Johnson 2020, Fig. 5): the energy balance of an isothermal ice at the
//!   body's Bond albedo, with the ice's own emissivity ([`NITROGEN_ICE_EMISSIVITY`]) and the heat
//!   flow beneath it. Its pressure is the registry's saturation pressure there: on β-N₂, 1.44 Pa at
//!   37.25 K (1.40 Pa on Fray and Schmitt's 2009 equation without the registry's anchor).
//! - **The floor** ([`TENUOUS_FLOOR_COLUMN`], [`holds_tenuous_air`]). Between it and the 100 Pa
//!   line ([`AIRLESS_PRESSURE`](crate::planetary::derive::atmosphere::AIRLESS_PRESSURE)) a body
//!   whose air is held is `Tenuous`, its gases listed; below it, `Airless`, with none.
//! - **No collapse-first start** (decision-p14-t53c-collapse; recorded). The greenhouse keeps the
//!   branch its whole column leads to: under 100 Pa the steps' climate holds no air, so a start
//!   from an empty column took thick warm airs onto locked planets' night floors by that convention
//!   alone, and a narrower rule for supersaturated thin airs left its one case on its own
//!   threshold.
//!
//! Exospheres are P14.T55.c's, and seasonal transport and frost by month P14.T55.e's.

use crate::math;
use crate::planetary::derive::atmosphere::Gas;
use crate::planetary::hooks::cold_traps::COLLISION_CROSS_SECTION_M2;
use crate::planetary::hooks::surface::REGOLITH_EMISSIVITY;
use crate::units::consts::{
    BOLTZMANN_CONSTANT, GRAVITATIONAL_CONSTANT, SECONDS_PER_JULIAN_YEAR, STEFAN_BOLTZMANN,
};
use crate::units::{
    Kelvin, Kilograms, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared, Pascals,
    WattsPerSquareMetre, Years,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

#[cfg(test)]
mod tests;

/// The generator version from which P14.T53.c is on the record: 24, the 23 → 24 batch's bump,
/// P14.T54.b, if it rides it (a rider; else the next planetary bump, its hold renumbered).
///
/// It is P14.T53.c's one hold. Below it [`TenuousRules::in_force`] is [`TenuousRules::Held`], so
/// that every figure on the record is version 23's bit for bit; its rules are read only under the
/// escape's batch rules too (`EscapeRules::Batch`), whose path they amend. P14.T54.b deletes this
/// hold, [`TenuousRules`] with its `Held` arm, the tests' switch and its tripwire
/// (`the_tenuous_air_is_held_only_until_the_24_bump`).
pub(crate) const TENUOUS_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether P14.T53.c's thin and transient air is in force (the hold, [`TENUOUS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum TenuousRules {
    /// Version 23's and before: P14.T54.a's ice-held stand-in, no `Tenuous` state, the
    /// greenhouse from the whole column.
    Held,
    /// The 23 → 24 batch's: the ice-held retention at the ice's own temperature, the `Tenuous`
    /// state above its floor, and no ocean under a `Snowball`'s ice among its liquids.
    Tenuous,
}

impl TenuousRules {
    /// The rules this generator version gives: P14.T53.c's from [`TENUOUS_VERSION`], none below
    /// it; in the tests, the rules a test's switch sets on its thread ([`with_tenuous`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= TENUOUS_VERSION {
            Self::Tenuous
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The tenuous air's rules a test sets on its thread, `None` for the version's.
    static RULES: core::cell::Cell<Option<TenuousRules>> = const { core::cell::Cell::new(None) };
}

/// `run`'s result with the tenuous air's rules `rules` on this thread, for the tests to reach 24's
/// thin airs ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. P14.T54.b deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_tenuous<R>(rules: TenuousRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<TenuousRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The thermal emissivity of nitrogen ice: 0.59 (Hillier et al. 1990, Science 250, 419, abstract:
/// "our Bond albedo implies a surface emissivity of 0.59 ± 0.16", Triton's 14 µbar N₂ air in vapour
/// equilibrium with its ice at 37.5 K under its 0.82 ± 0.05 Bond albedo; decision-triton-and-e101
/// §1.6).
///
/// It is inferred from the body's whole-disc Bond albedo by the global balance [`ice_temperature`]
/// takes, so the two are read together. β-N₂ is a weak far-infrared absorber, and the bare
/// surface's [`REGOLITH_EMISSIVITY`], 0.95, would hold Triton's ice near 33.5 K, on α-N₂, at about
/// 0.1 Pa, a factor of 14 low.
pub const NITROGEN_ICE_EMISSIVITY: f64 = 0.59;

/// The emissivity of a condensable's ice in its energy balance ([`ice_temperature`]): nitrogen's
/// [`NITROGEN_ICE_EMISSIVITY`], and every other ice's the bare surface's [`REGOLITH_EMISSIVITY`],
/// a labelled stand-in (argon's ice has no measured figure the lane has read).
#[must_use]
pub const fn ice_emissivity(gas: Gas) -> f64 {
    match gas {
        Gas::Nitrogen => NITROGEN_ICE_EMISSIVITY,
        Gas::Hydrogen
        | Gas::Helium
        | Gas::Water
        | Gas::Methane
        | Gas::Ammonia
        | Gas::Oxygen
        | Gas::CarbonDioxide
        | Gas::Argon => REGOLITH_EMISSIVITY,
    }
}

/// The temperature of an ice-held condensable's ice, `gas`'s, on a body whose equilibrium
/// temperature at the Bond albedo in use is `equilibrium` and whose surface heat flow is
/// `heat_flow`: ((σ `T_eq`⁴ + F) ÷ ε σ)^¼, ε the ice's emissivity ([`ice_emissivity`]).
///
/// One temperature over all of the ice, which the latent heat its vapour carries keeps uniform
/// (Gladstone et al. 2016: Pluto's ice ΔT ÷ T < 0.002, after Young 2012, Icarus 221, 80; Young,
/// Braga-Ribas and Johnson 2020, Fig. 5), set by the energy balance of an isothermal ice that
/// absorbs the body's orbit-mean light at its Bond albedo, (1 − A) F⋆ ÷ 4 = σ `T_eq`⁴, and radiates
/// at its own emissivity: Hillier et al.'s (1990) balance, by which they inferred
/// [`NITROGEN_ICE_EMISSIVITY`] from Triton's whole-disc albedo. The registry's frost rows are
/// pure-frost upper bounds (`N2` at 1.000 under a Sun), so the body's albedo stands for the ice's,
/// as Hillier et al.'s did.
///
/// Labelled: the ice's own share of the light, by where it lies (a polar cap's annual mean against
/// the whole sphere's) and by month, is P14.T55.e's; the global balance is the isothermal frost of
/// a body whose ice spans its latitudes, as Triton's and Pluto's do.
///
/// A temperature that is not finite or not positive, or a heat flow that is not, reads as none.
///
/// # Examples
///
/// Triton's nitrogen ice under its measured 0.82 Bond albedo at Neptune's 30.07 au lies within 2 K
/// of its measured 38 ± 1 K at nitrogen ice's low emissivity, and some 4 K colder at the bare
/// surface's:
///
/// ```
/// use hyperion_sim::planetary::chem::tenuous::ice_temperature;
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
/// use hyperion_sim::units::{EarthFluxes, WattsPerSquareMetre};
///
/// let flux = EarthFluxes::new(1.0 / (30.07 * 30.07));
/// let albedo = BondAlbedo::new(0.82).ok_or("0.82 is a Bond albedo")?;
/// let t_eq = equilibrium_temperature(flux, albedo);
/// let ice = ice_temperature(Gas::Nitrogen, t_eq, WattsPerSquareMetre::ZERO);
/// assert!((ice.value() - 38.0).abs() < 2.0);
/// let bare = ice_temperature(Gas::Water, t_eq, WattsPerSquareMetre::ZERO);
/// assert!(bare.value() < ice.value() - 4.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn ice_temperature(gas: Gas, equilibrium: Kelvin, heat_flow: WattsPerSquareMetre) -> Kelvin {
    let t = equilibrium.value();
    let light = if t.is_finite() && t > 0.0 {
        STEFAN_BOLTZMANN * math::powi(t, 4)
    } else {
        0.0
    };
    let f = heat_flow.value();
    let heat = if f.is_finite() && f > 0.0 { f } else { 0.0 };
    Kelvin::new(math::powf(
        (light + heat) / (ice_emissivity(gas) * STEFAN_BOLTZMANN),
        0.25,
    ))
}

/// The upper atmosphere's temperature over its ice's in an ice-held condensable's vapour air, at
/// which its escape is taken ([`vapour_escape_molecules_per_second`]): 2.0.
///
/// Such airs have a cold upper atmosphere, 1.7–2.7 times their ice's temperature: Pluto's 65–68 K
/// at its exobase, some 2,900 km in radius, over its 37 K ice (Young et al. 2018, Icarus 300, 174,
/// abstract; Gladstone et al. 2016), Triton's 102 K at its exobase, 870 km in altitude, over 38 K
/// (Krasnopolsky et al. 1993, JGR 98, 3065), and Zahnle and Catling's (2017, §2.1) upper
/// atmospheres "roughly twice as hot at high altitudes as they are at the surface". The lane's
/// value, within that span, is the one at which Pluto's computed escape lies within a factor of 10
/// of its measured 1 × 10²³ N₂ s⁻¹ (Gladstone et al. 2016): at 1.0 Pa on the registry's β-N₂
/// curve (36.65 K), 1.9 × 10²³ s⁻¹ at 2.0 (2.4 and 3.1 × 10²³ at 1.1 and 1.2 Pa), against 4.7 ×
/// 10²¹ at 1.8 and 4.1 × 10²⁴ at 2.2, the rate being exponential in it. The isothermal upper
/// atmosphere from the surface places Pluto's exobase at 2,501 km in radius, below its measured
/// 2,900 km, which the multiple absorbs. Their hot upper atmospheres are their methane's heating;
/// argon's held air takes the same multiple, labelled.
pub const UPPER_ATMOSPHERE_MULTIPLE: f64 = 2.0;

/// The halvings that place an exobase between its column's minimum and the surface
/// ([`vapour_escape_molecules_per_second`]): 64, a fixed count from a deterministic bracket, which
/// closes it to the last bit of x = R ÷ r (decision-composition §1.3's rule for every solver).
const EXOBASE_HALVINGS: u32 = 64;

/// The escape rate, molecules per second, of an ice-held condensable's vapour air: Jeans's flux at
/// the exobase of its upper atmosphere; `None` where it has none, so that the air escapes
/// hydrodynamically.
///
/// The air is `gas`'s, of surface pressure `vapour` over its ice at `ice`, on a body of mass
/// `mass` and radius `radius`, its upper atmosphere isothermal at [`UPPER_ATMOSPHERE_MULTIPLE`]
/// times the ice's temperature.
///
/// The column above a radius r of an isothermal atmosphere under 1 ÷ r² gravity is, to leading
/// order in 1 ÷ λ, N = n H with H = r ÷ λ: N(x) = `N_s` x⁻² e^(−`λ_s` (1 − x)), x = R ÷ r, with
/// `N_s` = p ÷ m g the surface's and `λ_s` = G M m ÷ k `T_u` R the surface's Jeans parameter (the
/// barometric law in a gravitational potential; Chamberlain and Hunten 1987, _Theory of Planetary
/// Atmospheres_, 2nd ed.). The exobase is where σ n H = 1, σ [`COLLISION_CROSS_SECTION_M2`] (the
/// mean free path the scale height): at the surface where the whole column holds fewer, and
/// otherwise the root above the surface, which N reaches before its minimum at x = 2 ÷ `λ_s`, the
/// isothermal sonic point (Zahnle and Catling 2017, §2.1). Where even that minimum holds more than
/// 1 ÷ σ, the air is collisional out to its sonic point: no exobase, and the regime is
/// hydrodynamic, the transition from Jeans's escape lying at λ ≈ 2–3 (Volkov et al. 2011, ApJ 729,
/// L24). At the exobase, of Jeans parameter λ = `λ_s` x and
/// number density n = N λ ÷ r (N = 1 ÷ σ, or `N_s` at the surface), the flux is Jeans's, n `v₀`
/// (1 + λ) e^(−λ) ÷ 2√π with `v₀` = √(2k `T_u` ÷ m), over 4π r² (Chamberlain and Hunten 1987;
/// Hunten 1973, J. Atmos. Sci. 30, 1481).
///
/// Pluto's, at 1.0 Pa over its 36.65 K ice, is 1.9 × 10²³ s⁻¹ (its measured 1 × 10²³, Gladstone et
/// al. 2016; (3–7) × 10²², Young et al. 2018), Triton's at 37.7 K some 6 × 10¹⁷, and Charon's,
/// `λ_s` ≈ 8, has no exobase. A vapour, a temperature, a mass or a radius that is not positive and
/// finite has no rate (`Some(0)`), as does a flux that underflows.
///
/// # Examples
///
/// Pluto's nitrogen air escapes within a factor of 10 of its measured 1 × 10²³ s⁻¹, where a Charon
/// at the same ice has no exobase:
///
/// ```
/// use hyperion_sim::planetary::chem::tenuous::vapour_escape_molecules_per_second;
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::units::consts::GRAVITATIONAL_CONSTANT;
/// use hyperion_sim::units::{Kelvin, Kilograms, Metres, Pascals};
///
/// let body = |gm_km3: f64, km: f64| {
///     (Kilograms::new(gm_km3 * 1e9 / GRAVITATIONAL_CONSTANT), Metres::new(km * 1e3))
/// };
/// let (ice, vapour) = (Kelvin::new(36.65), Pascals::new(1.0));
/// let (mass, radius) = body(869.3, 1_188.3);
/// let pluto = vapour_escape_molecules_per_second(mass, radius, Gas::Nitrogen, ice, vapour)
///     .ok_or("Pluto's air has an exobase")?;
/// assert!((1e22..=1e24).contains(&pluto));
/// let (mass, radius) = body(106.1, 606.0);
/// assert_eq!(
///     vapour_escape_molecules_per_second(mass, radius, Gas::Nitrogen, ice, vapour),
///     None
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn vapour_escape_molecules_per_second(
    mass: Kilograms,
    radius: Metres,
    gas: Gas,
    ice: Kelvin,
    vapour: Pascals,
) -> Option<f64> {
    let (m_body, r_body, pressure) = (mass.value(), radius.value(), vapour.value());
    let t_u = UPPER_ATMOSPHERE_MULTIPLE * ice.value();
    let positive = |v: f64| v.is_finite() && v > 0.0;
    if !(positive(m_body) && positive(r_body) && positive(pressure) && positive(t_u)) {
        return Some(0.0);
    }
    let molecule = gas.molecular_mass().value();
    let gm = GRAVITATIONAL_CONSTANT * m_body;
    let gravity = gm / (r_body * r_body);
    let kt = BOLTZMANN_CONSTANT * t_u;
    let column = pressure / (molecule * gravity);
    let lambda_s = gm * molecule / (kt * r_body);
    let sigma = COLLISION_CROSS_SECTION_M2;
    // ln(σ N(x)), whose root is the exobase.
    let ln_sigma_n = |x: f64| math::ln(sigma * column) - 2.0 * math::ln(x) - lambda_s * (1.0 - x);
    let (x, n_column) = if sigma * column <= 1.0 {
        (1.0, column)
    } else {
        let lowest = 2.0 / lambda_s;
        if lowest >= 1.0 || ln_sigma_n(lowest) >= 0.0 {
            return None;
        }
        // σ N falls from above 1 at the surface (x = 1) to below it at its minimum (x = 2 ÷ λ_s).
        let (mut below, mut above) = (lowest, 1.0);
        for _ in 0..EXOBASE_HALVINGS {
            let mid = f64::midpoint(below, above);
            if ln_sigma_n(mid) > 0.0 {
                above = mid;
            } else {
                below = mid;
            }
        }
        (f64::midpoint(below, above), 1.0 / sigma)
    };
    let exobase = r_body / x;
    let lambda = lambda_s * x;
    let density = n_column * lambda / exobase;
    let speed = (2.0 * kt / molecule).sqrt();
    // 1 ÷ 2√π is (2 ÷ √π) ÷ 4.
    let flux =
        density * speed * (1.0 + lambda) * math::exp(-lambda) * core::f64::consts::FRAC_2_SQRT_PI
            / 4.0;
    let rate = 4.0 * core::f64::consts::PI * exobase * exobase * flux;
    Some(if rate > 0.0 { rate } else { 0.0 })
}

/// The mass of an ice-held condensable a body loses over its age as its ice's vapour; `None` where
/// its ice keeps none of it.
///
/// It is `gas`'s on a body of mass `mass` and radius `radius` over `age`, its ice at `ice`
/// ([`ice_temperature`]): [`vapour_escape_molecules_per_second`] at the registry's saturation
/// pressure there, times the molecule's mass and the age. `None` where the row has no phase
/// diagram, its vapour is not finite (above its critical temperature) or its air has no exobase
/// (the hydrodynamic regime, whose outcome is the literature's for Charon: Johnson et al. 2015 find
/// that Charon "is the only body that has clearly lost its full inventory", by escape driven by
/// the ultraviolet it absorbs, as Young, Braga-Ribas and Johnson 2020 report; an energy-limited
/// rate, 0.7–4 × 10²⁷ s⁻¹ at Pluto before the flyby, would empty even the model's median Pluto,
/// decision-triton-and-e101 §1.4).
///
/// The loss is the present rate times the age, with no fainter young host and no orbit; and the
/// rule's test of an ice-held gas, its vapour under its inventory's weight, holds where any of it
/// lies as ice, not most of it (stated).
///
/// # Examples
///
/// Over 4.5 Gyr Triton's nitrogen ice at 37.7 K loses some 10⁹ kg, a few parts in 10⁹ of its median
/// inventory of 1.4 × 10¹⁸ kg:
///
/// ```
/// use hyperion_sim::planetary::chem::tenuous::ice_held_loss;
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::units::consts::GRAVITATIONAL_CONSTANT;
/// use hyperion_sim::units::{Kelvin, Kilograms, Metres, Years};
///
/// let (mass, radius) = (
///     Kilograms::new(1_428.495e9 / GRAVITATIONAL_CONSTANT),
///     Metres::new(1_352.6e3),
/// );
/// let loss = ice_held_loss(mass, radius, Gas::Nitrogen, Kelvin::new(37.7), Years::new(4.5e9))
///     .ok_or("Triton's ice holds its nitrogen")?;
/// assert!(loss.value() < 1e-6 * 1.4e18);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn ice_held_loss(
    mass: Kilograms,
    radius: Metres,
    gas: Gas,
    ice: Kelvin,
    age: Years,
) -> Option<Kilograms> {
    let vapour = crate::substance::saturation_pressure(gas.substance(), ice)?;
    if !vapour.value().is_finite() {
        return None;
    }
    let rate = vapour_escape_molecules_per_second(mass, radius, gas, ice, vapour)?;
    let years = if age.value() > 0.0 { age.value() } else { 0.0 };
    let loss = rate * gas.molecular_mass().value() * years * SECONDS_PER_JULIAN_YEAR;
    Some(Kilograms::new(if loss > 0.0 { loss } else { 0.0 }))
}

/// The column of air, p ÷ g, from which an air below the 100 Pa line is `Tenuous`: 10⁻⁴ kg m⁻².
///
/// Below it a body is `Airless`, with no gases listed. It is P14.T51.d's ion-stopping column
/// (`ION_STOPPING_COLUMN`, the same figure, which the surface modifiers' tests hold to this one's
/// bits), the column above which the wind's ions do not reach the ground (a 1 keV proton's range
/// in air, NIST PSTAR: dry air's CSDA range of 9.857 × 10⁻⁶ g cm⁻²), which that task labelled as
/// this floor's stand-in. The X-ray light takes the same floor by plan 14's labelled
/// simplification, though it is not stopped there: at 1 keV dry air's μ ÷ ρ is 3,606 cm² g⁻¹ (NIST
/// XCOM), τ 0.036 through the floor. The surface modifiers are not read on the record before
/// P14.T54.b, so the figure is written here.
///
/// An air that shields its ground from the wind is an atmosphere, however thin: Pluto's 11 ± 1
/// µbar (Gladstone et al. 2016) over 0.62 m s⁻² is about 1.8 kg m⁻² and Triton's 14 µbar over
/// 0.779 m s⁻² about 1.8, far above it. Io's measured equatorial SO₂, 3 × 10¹⁶ to 1.5 × 10¹⁷ cm⁻²
/// (Lellouch et al. 2015, Icarus 253, 99), 3 × 10⁻⁵ to 1.6 × 10⁻⁴ kg m⁻², straddles it, and the
/// Galilean moons' and the Moon's exospheres lie far below; those are P14.T55.c's. A column is the
/// floor in place of a pressure, since what the air does to its ground is its column's.
pub const TENUOUS_FLOOR_COLUMN: KilogramsPerSquareMetre = KilogramsPerSquareMetre::new(1e-4);

/// Whether an air of surface pressure `pressure` on a body of surface gravity `gravity` reaches the
/// tenuous floor, its column p ÷ g at least [`TENUOUS_FLOOR_COLUMN`].
///
/// False for a pressure or a gravity that is not positive and finite.
#[must_use]
pub fn holds_tenuous_air(pressure: Pascals, gravity: MetresPerSecondSquared) -> bool {
    let (p, g) = (pressure.value(), gravity.value());
    p.is_finite() && p > 0.0 && g.is_finite() && g > 0.0 && p / g >= TENUOUS_FLOOR_COLUMN.value()
}
