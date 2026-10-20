//! A solid surface's modifiers (plan 14, P14.T51.d; decision-p14-t51d-weathering §2,
//! decision-r11-palette-liquids §2 item 4): its space-weathering rate and maturity, its
//! irradiated-organics cover, its resurfacing rate, and an Io's volcanic deposits
//! ([`SurfaceModifiers::derive`]).
//!
//! # The weathering rate
//!
//! k is the body's orbit-mean, surface-reaching space-weathering dose rate relative to the Moon's
//! present one, computed by the same rules from stated lunar inputs, so that the Moon's is 1
//! (decision-p14-t51d-weathering §2.2):
//!
//! k = `s_w` `Φ_w` ÷ `Φ_w,Moon` + `s_d` `Φ_d` ÷ `Φ_d,Moon`,
//!
//! with the wind's and the micrometeoroids' shares `s_w` = `s_d` = 0.5 ([`WIND_SHARE`], an
//! assumption, labelled). Both terms are kept ([`WeatheringRate`]). k is dimensionless, finite and
//! at least +0, with no upper bound.
//!
//! - **The wind's term** `Φ_w` = Ṁ ÷ (4π a² √(1 − e²)) × `S_w`, the host's wind mass flux at the
//!   body's heliocentric orbit (a moon's planet's) times the unshielded share `S_w`. The sim has no
//!   wind for a cool main-sequence star (`stellar::sse::wind`'s main-sequence term switches on
//!   above 4,000 L☉), so Ṁ is Wood et al. 2002's relation ([`wind_mass_loss_kg_per_s`]), with the
//!   host's X-ray and ultraviolet luminosity standing in for its X-ray luminosity, labelled. The
//!   share `S_w` is 0 under air above the floor [`ION_STOPPING_COLUMN`] and for a moon inside its
//!   planet's dayside magnetopause, and the dipole's open-field share 1 − √(1 − R ÷ `r_mp`) under
//!   the body's own field (`open_field_share`).
//! - **The micrometeoroids' term** `Φ_d` = n(r) v(r)³ ∝ `D_sys` r^(−1.3) (G M★ ÷ r)^(3/2) at the
//!   semi-major axis, times `S_d` = exp(−Σ ÷ [`DUST_STOPPING_COLUMN`]) of the air's column
//!   Σ = p ÷ g ([`ZODIACAL_DENSITY_INDEX`], [`SYSTEM_DUST_LEVEL`]).
//! - **The Moon's reference** is the same two fluxes for the Sun ([`ModifierHost::sun`]), 1 au
//!   and e = 0.0167 ([`LUNAR_REFERENCE_ECCENTRICITY`]), no air and no field. The Moon's months in
//!   Earth's magnetotail, which the rules do not model, would scale its own term and the
//!   reference's alike, so they cancel in `k_Moon` (the ruling).
//!
//! # The maturity
//!
//! m = [`body_maturity`]\(`t_s`, k) = 1 − exp(−k `t_s` ÷
//! `τ_L`), with `t_s` P14.T24.b's surface age and `τ_L` the shared law's lunar maturation time
//! ([`LUNAR_MATURATION_TIME`](hyperion_surface::material::LUNAR_MATURATION_TIME)): the background
//! regolith's npFe⁰ content as a fraction of the Moon's mature background's, per unit of `FeO_T`
//! (decision-p14-t51d-weathering §2.1), dimensionless in [+0, 1].
//!
//! # The organic cover
//!
//! The covered fraction of the ground that can take one, dimensionless in [+0, 1], +0 where none
//! can (decision-r11-palette-liquids §2 item 4). The ground that can take one
//! ([`organic_ground`]) is the published area of P14.T51.a's solid, non-seasonal condensates whose
//! registry row carries carbon, CO₂ excepted, with nitrogen's where one of them is present: the
//! simple ices whose irradiation leaves a red refractory residue, tholin (Brunetto et al. 2006, ApJ
//! 644, 646, abstract read: methanol, methane and benzene ices irradiated to "about 350 eV per 16
//! amu" show "a strong reddening and darkening", from "an organic (C-rich) refractory residue";
//! Cruikshank, Imanaka and Dalle Ore 2005, Adv. Space Res. 36, 178, abstract read, for tholins as
//! the colouring agents of the outer Solar System's red bodies). The cover follows the first-order
//! balance dc ÷ dt = `k_irr` (1 − c) − `k_res` c from fresh ground at the surface age:
//!
//! c = `c_ss` (1 − exp(−(`k_irr` + `k_res`) `t_s`)), `c_ss` = `k_irr` ÷ (`k_irr` + `k_res`),
//!
//! with `k_irr` the surface-reaching dose flux (the host's X-ray and ultraviolet light and the
//! wind's ions' kinetic energy, over the globe and the orbit) over the saturation dose
//! [`ORGANIC_SATURATION_DOSE_EV_PER_16_AMU`] of a layer [`ORGANIC_LAYER_COLUMN`], and `k_res` =
//! 1 ÷ τ the resurfacing rate of P14.T24.b's resurfacing time. The red Kuiper-belt bodies are those
//! whose surfaces keep such ices (Brown, Schaller and Fraser 2011, ApJ 739, L60, *from memory*).
//! Under air above [`ION_STOPPING_COLUMN`] the dose does not reach the ground and the cover is +0:
//! a hazy world's tholin is its haze's fallout, P14.T53.c's.
//!
//! # The resurfacing rate
//!
//! [`RESURFACING_DEPTH`] ÷ τ, m per Julian year, at least +0: the layer that buries the craters
//! P14.T24.b's surface age counts, laid down once per its resurfacing time τ at the present heat
//! flow, tides included, so from volcanism or tidal heat alike. It stays off the field header,
//! since the surface age carries it for R10 and R11 (decision-r11-palette-liquids §2 item 4).
//!
//! # An Io's deposits
//!
//! A body of [`Volcanism::Extreme`] at its heat flow (1 W m⁻² and over, global resurfacing), whose
//! mantle holds sulphur and whose surface is below SO₂'s frost point at [`IO_SO2_PRESSURE`], is
//! resurfaced with SO₂ frost and sulphur in Io's measured shares ([`IO_SO2_FROST_SHARE`],
//! [`IO_SULPHUR_SHARE`]), labelled as Io's until P14.T24.g's outgassing gives the deposits. The
//! rest is its lid's own silicate. Neither registry row has optics yet: their albedo waits for
//! P14.T49.i.
//!
//! # Which `SurfaceModifiers`
//!
//! [`SurfaceModifiers`] here is the derivation's value.
//! [`record::SurfaceModifiers`](crate::planetary::record::SurfaceModifiers) is the record's
//! uninhabited placeholder, which P14.T54.b replaces with this type, as P14.T54.a does the crust's.
//!
//! # Not on the record yet
//!
//! Nothing on the record calls this module at generator version 22 (decision-bump-plan-after-22
//! §1.3: P14.T51.d is a pure rider of 24): P14.T54.b (23 → 24) puts [`SurfaceModifiers`] in the
//! record's surface section, fills the field header's `maturity`, `weathering_rate` and
//! `organic_cover` from it, and deletes the tripwire test that holds the line until then.
//!
//! # Determinism
//!
//! Pure functions of their arguments and the substance registry, with no draw and so no domain
//! tag; every sum in the condensates' order; every transcendental through the crate's [`math`].

#[cfg(test)]
mod tests;

use core::fmt;
use std::error::Error;

use hyperion_surface::craters::SOLAR_SYSTEM_AGE;
use hyperion_surface::material::body_maturity;

use crate::math;
use crate::planetary::chem::partition::{CondensateEntry, CondensatePhase, Condensates};
use crate::planetary::context::XuvHistory;
use crate::planetary::derive::atmosphere::{ATOMIC_MASS_CONSTANT_KG, Gas};
use crate::planetary::hooks::figures::MagneticField;
use crate::planetary::moons::Volcanism;
use crate::substance::{self, Element, SubstanceId};
use crate::units::consts::{GM_SUN, METRES_PER_AU, SECONDS_PER_JULIAN_YEAR, SOLAR_RADIUS_M};
use crate::units::{
    Gauss, Gigayears, GravitationalParameter, Kelvin, Kilograms, KilogramsPerSquareMetre, Metres,
    MetresPerSecond, MetresPerSecondSquared, Pascals, SolarLuminosities, SolarMasses,
    SolarMassesPerYear, Watts, WattsPerSquareMetre, Years,
};

/// The stellar wind's share `s_w` of the Moon's space weathering: 0.5, an assumption, labelled
/// (decision-p14-t51d-weathering §2.2). The split between the wind and the micrometeoroids is
/// debated (Pieters and Noble 2016, JGR Planets 121, 1865, §3.1, read, which finds the rims'
/// npFe⁰ formed "largely through impact processes"). The lunar swirls, which
/// lie on magnetised crust and look immature, show the wind's share is substantial but not all of
/// it (Blewett et al. 2011, JGR 116, E02002, abstract read: "all swirls and swirl-like albedo
/// patterns are associated with areas of magnetized crust" and "exhibit spectral characteristics
/// similar to immature material", with magnetic shielding from the solar wind one of the two
/// explanations it cannot tell apart).
pub const WIND_SHARE: f64 = 0.5;

/// The micrometeoroids' share `s_d` of the Moon's space weathering, 1 − [`WIND_SHARE`].
pub const DUST_SHARE: f64 = 1.0 - WIND_SHARE;

/// The Sun's present wind mass-loss rate Ṁ☉: 2 × 10⁻¹⁴ M☉ yr⁻¹ (Wood et al. 2002's text, read:
/// the unit in which they quote their rates; about 2.7 × 10¹² protons m⁻² s⁻¹ at 1 au at
/// [`WIND_SPEED`], inside the measured solar wind's few × 10¹²). It cancels from the weathering
/// rate's wind term, a ratio of mass fluxes, and sets only the wind's ram pressure on a
/// magnetosphere and its ions' energy flux for the organic cover.
pub const SOLAR_WIND_MASS_LOSS: SolarMassesPerYear = SolarMassesPerYear::new(2e-14);

/// The index of Wood et al. 2002's relation, mass loss per unit surface area ∝ `F_X`^1.15±0.20,
/// with `F_X` the X-ray surface flux (ApJ 574, 412, abstract read: fitted to "the solar-like GK
/// dwarfs", with the active M dwarf Proxima Centauri and λ Andromedae, a very active RS Canum
/// Venaticorum binary, "inconsistent with this relation"). Wood et al. 2005 (ApJ 628, L143, §3,
/// read) refit the index as 1.34 ± 0.18 and find the relation holds only below `F_X` = 8 × 10⁵ erg
/// cm⁻² s⁻¹, its winds weakening above; Wood 2018 (arXiv:1809.01109, §3, read) gives 1.29 ± 0.16
/// below log `F_X` = 6.0. 1.15 lies inside the newer error bar and is kept, applied uncapped,
/// labelled: an active host's wind is overstated, an M dwarf's the more (plan 14's deferred
/// findings from P14.T51.d).
pub const WIND_FLUX_INDEX: f64 = 1.15;

/// The wind's speed at every body, for its ram pressure and its ions' energy: 400 km/s, the slow
/// solar wind at 1 au, the speed Wood et al. 2005 assume (read), a stand-in for every host,
/// labelled.
pub const WIND_SPEED: MetresPerSecond = MetresPerSecond::new(4.0e5);

/// The column of air, Σ = p ÷ g, above which the wind's ions and the host's X-ray and extreme
/// ultraviolet light do not reach the ground: 10⁻⁴ kg m⁻², a 1 keV proton's range in air (NIST
/// PSTAR, read: dry air at 1 keV, a CSDA range of 9.857 × 10⁻⁶ g cm⁻²). A stand-in for P14.T53.c's
/// tenuous floor, which is not built, labelled. The airs' columns, derived here as p ÷ g: Pluto's
/// 11.5 µbar over 0.62 m s⁻² is about 1.9 kg m⁻² and Triton's 14 µbar over 0.779 m s⁻² about 1.8,
/// far above it, and Io's 1 nbar ([`IO_SO2_PRESSURE`]) over 1.80 m s⁻² about 6 × 10⁻⁵, just below
/// (the pressures and gravities *from memory*).
pub const ION_STOPPING_COLUMN: KilogramsPerSquareMetre = KilogramsPerSquareMetre::new(1e-4);

/// The column of air `Σ_d` in which a micrometeoroid is stopped, the e-folding column of the dust's
/// share `S_d` = exp(−Σ ÷ `Σ_d`): 0.3 kg m⁻², a 100 µm grain at 3,000 kg m⁻³, `ρ_p` d
/// (decision-p14-t51d-weathering §2.2, derived there, labelled). Mars's air and thicker shield the
/// ground, Pluto and Triton nearly, Io not at all.
pub const DUST_STOPPING_COLUMN: KilogramsPerSquareMetre = KilogramsPerSquareMetre::new(0.3);

/// The index of the zodiacal cloud's number density, n ∝ r^(−1.3) (Leinert et al. 1981, A&A 103,
/// 177, Summary, read, from Helios's zodiacal light over 0.3–1.0 au). Beyond that span, where the
/// source says nothing, the law is an extrapolation, labelled. With the impact speed
/// v ∝ √(G M★ ÷ r), the dust's term n v³ goes as r^(−2.8): Mercury's 0.387^(−2.8) ≈ 14 against
/// Cintala 1992's "almost 14 times more impact melt and over 20 times more vapor" than the Moon's
/// (JGR 97, 947, abstract read), the ruling's check.
pub const ZODIACAL_DENSITY_INDEX: f64 = 1.3;

/// A system's dust level `D_sys` relative to the Solar System's zodiacal cloud: 1 for every system,
/// a labelled constant, not a draw, since exozodiacal levels are unknown per system
/// (decision-p14-t51d-weathering §1.7 and §2.2).
pub const SYSTEM_DUST_LEVEL: f64 = 1.0;

/// The equatorial surface field standing in for a [`MagneticField::Weak`] dynamo: 195 nT,
/// Mercury's dipole moment of 195 ± 10 nT `R_M³` (Anderson et al. 2011, Science 333, 1859, read),
/// inside the class's "under about 1 μT, as Mercury's 0.2–0.3 μT". A stand-in, labelled: the sim
/// carries the class, not a moment. At Mercury the model puts the subsolar magnetopause at 1.27
/// `R_M`, against the 1.45 `R_M` Winslow et al. 2013 measure (JGR 118, 2213, abstract read), about
/// 12% low (plan 14's deferred findings from P14.T51.d).
pub const WEAK_FIELD: Gauss = Gauss::new(1.95e-3);

/// The equatorial surface field standing in for a [`MagneticField::Strong`] dynamo: 30 μT, Earth's
/// dipole's 29.7 μT (IGRF-14, 2025, read) rounded, the class's "Earth's 30 μT". A stand-in,
/// labelled: the class's giants run
/// 14–420 μT, so a Jupiter's magnetosphere is about 2.4 times smaller here than its field alone
/// would make it, and a far moon such as Callisto falls outside it.
pub const STRONG_FIELD: Gauss = Gauss::new(0.30);

/// The magnetic constant μ₀, N A⁻²: 1.256 637 061 27 × 10⁻⁶ (CODATA 2022, read through NIST).
const VACUUM_PERMEABILITY: f64 = 1.256_637_061_27e-6;

/// The factor on the dipole's pressure at the subsolar magnetopause, 2: Chapman–Ferraro pressure
/// balance with the field doubled by the boundary's current sheet's compression, so that
/// `r_mp` ÷ R = (2 B₀² ÷ (μ₀ ρ v²))^(1/6) (decision-p14-t51d-weathering §2.2, derived there,
/// labelled).
pub const MAGNETOPAUSE_COMPRESSION: f64 = 2.0;

/// The irradiation dose that saturates simple ices' organic residue: 100 eV per 16 amu, Cooper et
/// al. 2003's chemically significant dose (Earth Moon Planets 92, 261, abstract read: "times in
/// years to accumulate dosages ∼100 eV per molecule", computed "at unit density"), on the
/// per-16-amu scale of Brunetto et al. 2006's irradiation (abstract read), so that it is a dose per
/// unit mass whatever the ice: 6.03 × 10⁸ J kg⁻¹. Strazzulla and Johnson 1991 (IAU Colloq. 116,
/// 243, abstract read) review the residue's growth without a figure in their abstract.
pub const ORGANIC_SATURATION_DOSE_EV_PER_16_AMU: f64 = 100.0;

/// The layer the organic cover saturates in, as its column: 0.1 µm at unit density, 10⁻⁴ kg m⁻²
/// (adopted, labelled; Cooper et al. 2003 give their depths at unit density). The wind's ions stop
/// within some tens of nanometres and the extreme ultraviolet within about a tenth of a micrometre
/// (*from memory*).
pub const ORGANIC_LAYER_COLUMN: KilogramsPerSquareMetre = KilogramsPerSquareMetre::new(1e-4);

/// The joule per electronvolt (the SI's exact elementary charge, 2019).
const JOULES_PER_ELECTRONVOLT: f64 = 1.602_176_634e-19;

/// The depth a resurfacing lays down once per P14.T24.b's resurfacing time: 200 m, the depth of
/// the 1 km crater its density N☾(>1 km) counts, at the simple bowl's 0.20 D (the surface crate's
/// crater shape, within 2% of Pike 1980b's lunar 0.196 D^1.010, as `craters/shape.rs` cites it):
/// the deposit that drapes such a crater to its rim erases it. Labelled. Io's heat flow of about
/// 2.5 W m⁻² (*from memory*) gives a resurfacing time of some 4,400 yr and so about 4.5 cm per
/// year, against the about 1 cm per year of Johnson et al. 1979 (Nature 280, 746, whose abstract
/// gives no figure; the figure *from memory*): within an order of magnitude, recorded.
pub const RESURFACING_DEPTH: Metres = Metres::new(200.0);

/// The SO₂ partial pressure an Io's frost point is taken at: 10⁻⁴ Pa, 1 nbar, the order of Io's
/// dayside SO₂ atmosphere (*from memory*), which its plumes sustain; a stand-in, labelled. The
/// registry's SO₂ sublimation curve gives the frost point at it.
pub const IO_SO2_PRESSURE: Pascals = Pascals::new(1e-4);

/// Io's share of its surface under SO₂ frost: 0.27, its "white and gray materials ... identified
/// with moderate-to-coarse-grained SO₂" (Geissler et al. 1999, Icarus 140, 265, abstract read).
/// Labelled as Io's until P14.T24.g's outgassing gives an Io's deposits.
pub const IO_SO2_FROST_SHARE: f64 = 0.27;

/// Io's share of its surface under sulphur: 0.716, the rest of its surface less its frost and its
/// dark silicates, "about 1.4%" (Geissler et al. 1999, abstract read), its red and orange
/// ("almost a third") and yellow ("about 40%") sulphur compounds. Labelled as Io's.
pub const IO_SULPHUR_SHARE: f64 = 1.0 - IO_SO2_FROST_SHARE - 0.014;

/// The eccentricity of the Moon's reference orbit about the Sun, Earth's: 0.0167 (*from memory*;
/// 0.016 71 at J2000).
pub const LUNAR_REFERENCE_ECCENTRICITY: f64 = 0.0167;

/// The Sun's zero-age luminosity, from which the reference Sun's X-ray and ultraviolet history is
/// computed: 0.7 L☉, the faint young Sun "~30% fainter" (Feulner 2012, arXiv:1204.4449, read;
/// Gough 1981's 0.714), the value [`XuvHistory`]'s own example takes.
const SOLAR_ZERO_AGE_LUMINOSITY: SolarLuminosities = SolarLuminosities::new(0.7);

/// The host star a body's weathering and organic cover read: its present wind and light.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ModifierHost {
    /// Its present radius, positive.
    pub radius: Metres,
    /// Its present X-ray and ultraviolet luminosity, P14.T1.a's [`XuvHistory::luminosity`] at its
    /// age, at least 0: the stand-in for its X-ray luminosity in Wood et al.'s relation.
    pub xuv_luminosity: Watts,
    /// Its gravitational parameter G M★, at least 0, which sets the micrometeoroids' speed.
    pub gravitational_parameter: GravitationalParameter,
}

impl ModifierHost {
    /// The Sun of the Moon's reference: R☉, G M☉, and the X-ray and ultraviolet luminosity the
    /// sim's own history gives a 1 M☉ star of zero-age luminosity 0.7 L☉ at the Solar System's
    /// 4.567 Gyr ([`solar_xuv_luminosity`]).
    #[must_use]
    pub fn sun() -> Self {
        Self {
            radius: Metres::new(SOLAR_RADIUS_M),
            xuv_luminosity: solar_xuv_luminosity(),
            gravitational_parameter: GravitationalParameter::new(GM_SUN),
        }
    }
}

/// The present Sun's X-ray and ultraviolet luminosity in the sim's own model, the reference of
/// Wood et al.'s relation: P14.T1.a's [`XuvHistory`] of 1 M☉ and zero-age 0.7 L☉ at the Solar
/// System's age, 4.567 Gyr (`hyperion_surface::craters::SOLAR_SYSTEM_AGE`), about 2.7 × 10²⁰ W,
/// some 1 mW m⁻² at 1 au.
#[must_use]
pub fn solar_xuv_luminosity() -> Watts {
    XuvHistory::new(SolarMasses::new(1.0), SOLAR_ZERO_AGE_LUMINOSITY)
        .luminosity(Years::from(SOLAR_SYSTEM_AGE))
}

/// The present wind mass-loss rate of `host`, kg s⁻¹: Wood et al. 2002's relation,
/// Ṁ = Ṁ☉ (R★ ÷ R☉)² (F★ ÷ F☉)^1.15, with F = L ÷ 4π R² the X-ray and ultraviolet surface flux
/// standing in for the X-ray surface flux (labelled) and F☉ the reference Sun's
/// ([`solar_xuv_luminosity`]). Zero for a host with no X-ray and ultraviolet light.
#[must_use]
pub fn wind_mass_loss_kg_per_s(host: &ModifierHost) -> f64 {
    let xuv = host.xuv_luminosity.value();
    if xuv <= 0.0 {
        return 0.0;
    }
    let radius = host.radius.value() / SOLAR_RADIUS_M;
    let surface_flux = xuv / (radius * radius) / solar_xuv_luminosity().value();
    let solar = Kilograms::from(SolarMasses::new(SOLAR_WIND_MASS_LOSS.value())).value()
        / SECONDS_PER_JULIAN_YEAR;
    solar * radius * radius * math::powf(surface_flux, WIND_FLUX_INDEX)
}

/// A body's orbit about its host star: its own, or for a moon its planet's.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct HeliocentricOrbit {
    /// The semi-major axis, positive.
    pub semi_major_axis: Metres,
    /// The eccentricity, in [0, 1).
    pub eccentricity: f64,
}

/// What a body orbits, for the wind's shielding: its host star, or a planet whose magnetosphere
/// may enclose it.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Primary {
    /// A body orbiting its host star.
    Star,
    /// A moon, with its planet's magnetosphere.
    Planet(PlanetMagnetosphere),
}

/// A moon's planet, as its magnetosphere shields the moon.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct PlanetMagnetosphere {
    /// The planet's radius, positive.
    pub radius: Metres,
    /// The planet's field class (a gas envelope's is [`MagneticField::Strong`]).
    pub field: MagneticField,
    /// The moon's semi-major axis about the planet, positive.
    pub semi_major_axis: Metres,
}

/// What [`SurfaceModifiers::derive`] reads of a body with a solid surface, as plain values, each
/// with its source on the record (which P14.T54.b wires).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModifierInputs<'a> {
    /// Its host star's wind and light.
    pub host: ModifierHost,
    /// Its heliocentric orbit, a moon's planet's.
    pub orbit: HeliocentricOrbit,
    /// What it orbits.
    pub primary: Primary,
    /// Its surface gravity, positive.
    pub gravity: MetresPerSecondSquared,
    /// Its surface pressure, P14.T24.a's, finite and at least 0.
    pub surface_pressure: Pascals,
    /// Its global field, P14.T24.b's `GlobalFigures::magnetic_field`.
    pub field: MagneticField,
    /// Its surface age `t_s`, P14.T24.b's `GlobalFigures::surface_age`, finite and at least 0.
    pub surface_age: Years,
    /// Its resurfacing time τ at its present heat flow, P14.T24.b's `surface_age::resurfacing_time`
    /// of its crust under the record's tide rule: positive, and infinite where nothing resurfaces
    /// it.
    pub resurfacing_time: Years,
    /// Its present heat flow, tides included, P14.T24.b's `GlobalFigures::heat_flow`, finite and
    /// at least 0.
    pub heat_flow: WattsPerSquareMetre,
    /// Its mean surface temperature, P14.T24.a's, positive.
    pub surface_temperature: Kelvin,
    /// Its mantle's sulphur, P14.T50.a's `ElementInventory::mass` of sulphur in the mantle, finite
    /// and at least 0.
    pub mantle_sulphur: Kilograms,
    /// Its published condensates, P14.T51.a's [`Condensates`].
    pub condensates: &'a Condensates,
}

/// An input of [`ModifierInputs`] out of its range ([`DeriveModifiersError::OutOfRange`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ModifierInput {
    /// [`ModifierHost::radius`].
    HostRadius,
    /// [`ModifierHost::xuv_luminosity`].
    HostXuvLuminosity,
    /// [`ModifierHost::gravitational_parameter`].
    HostGravitationalParameter,
    /// [`HeliocentricOrbit::semi_major_axis`].
    SemiMajorAxis,
    /// [`HeliocentricOrbit::eccentricity`].
    Eccentricity,
    /// [`PlanetMagnetosphere::radius`].
    PlanetRadius,
    /// [`PlanetMagnetosphere::semi_major_axis`].
    PlanetocentricSemiMajorAxis,
    /// [`ModifierInputs::gravity`].
    Gravity,
    /// [`ModifierInputs::surface_pressure`].
    SurfacePressure,
    /// [`ModifierInputs::surface_age`].
    SurfaceAge,
    /// [`ModifierInputs::resurfacing_time`].
    ResurfacingTime,
    /// [`ModifierInputs::heat_flow`].
    HeatFlow,
    /// [`ModifierInputs::surface_temperature`].
    SurfaceTemperature,
    /// [`ModifierInputs::mantle_sulphur`].
    MantleSulphur,
}

impl ModifierInput {
    /// The input's name, for a message.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::HostRadius => "host's radius",
            Self::HostXuvLuminosity => "host's X-ray and ultraviolet luminosity",
            Self::HostGravitationalParameter => "host's gravitational parameter",
            Self::SemiMajorAxis => "semi-major axis",
            Self::Eccentricity => "eccentricity",
            Self::PlanetRadius => "planet's radius",
            Self::PlanetocentricSemiMajorAxis => "semi-major axis about its planet",
            Self::Gravity => "surface gravity",
            Self::SurfacePressure => "surface pressure",
            Self::SurfaceAge => "surface age",
            Self::ResurfacingTime => "resurfacing time",
            Self::HeatFlow => "heat flow",
            Self::SurfaceTemperature => "surface temperature",
            Self::MantleSulphur => "mantle's sulphur",
        }
    }
}

/// Why [`SurfaceModifiers::derive`] refused its inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeriveModifiersError {
    /// An input was not finite or lay outside its range, as [`ModifierInputs`] states it.
    OutOfRange(ModifierInput),
    /// The inputs are each in range, but the weathering rate k they give is not finite: an
    /// exposure that overflows, such as a host's G M★ so large that the micrometeoroids' speed
    /// term (G M★ ÷ r)^(3/2) does, or a semi-major axis so small that its square underflows to 0
    /// under the wind's flux.
    RateNotFinite,
}

impl fmt::Display for DeriveModifiersError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange(input) => {
                write!(
                    f,
                    "the body's {} is not finite or out of its range",
                    input.name()
                )
            }
            Self::RateNotFinite => f.write_str("the body's weathering rate is not finite"),
        }
    }
}

impl Error for DeriveModifiersError {}

/// The two terms of the weathering rate k ([`SurfaceModifiers::weathering`]): the wind's and the
/// micrometeoroids' surface-reaching dose rates, each relative to the Moon's.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct WeatheringRate {
    wind: f64,
    dust: f64,
}

impl WeatheringRate {
    /// The wind's term `Φ_w` ÷ `Φ_w,Moon`, dimensionless, at least +0: its shielded mass flux over
    /// the Moon's.
    #[must_use]
    pub const fn wind(&self) -> f64 {
        self.wind
    }

    /// The micrometeoroids' term `Φ_d` ÷ `Φ_d,Moon`, dimensionless, at least +0.
    #[must_use]
    pub const fn dust(&self) -> f64 {
        self.dust
    }

    /// The weathering rate k = `s_w` × [`wind`](Self::wind) + `s_d` × [`dust`](Self::dust)
    /// ([`WIND_SHARE`], [`DUST_SHARE`]), dimensionless, finite and at least +0: the Moon's 1.
    #[must_use]
    pub fn value(&self) -> f64 {
        WIND_SHARE * self.wind + DUST_SHARE * self.dust
    }
}

/// One volcanic deposit an Io is resurfaced with: a registry row and its share of the surface.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct VolcanicDeposit {
    substance: SubstanceId,
    area_fraction: f64,
}

impl VolcanicDeposit {
    /// The deposit's substance: `SO2`'s frost or `S8`'s sulphur.
    #[must_use]
    pub const fn substance(&self) -> SubstanceId {
        self.substance
    }

    /// Its share of the surface, dimensionless in (0, 1).
    #[must_use]
    pub const fn area_fraction(&self) -> f64 {
        self.area_fraction
    }
}

/// A solid surface's modifiers (P14.T51.d; the [module](self) documentation): its weathering rate
/// and maturity, its organic cover, its resurfacing rate and an Io's volcanic deposits.
///
/// The record's [`record::SurfaceModifiers`](crate::planetary::record::SurfaceModifiers) is a
/// placeholder of the same name, which P14.T54.b replaces with this type.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceModifiers {
    maturity: f64,
    weathering: WeatheringRate,
    organic_ground: f64,
    organic_cover: f64,
    resurfacing_m_per_yr: f64,
    volcanic_deposits: Vec<VolcanicDeposit>,
}

impl SurfaceModifiers {
    /// The modifiers of the body `inputs` describe (the [module](self) documentation).
    ///
    /// # Errors
    ///
    /// - [`DeriveModifiersError::OutOfRange`] for an input that is not finite or lies outside the
    ///   range [`ModifierInputs`] states;
    /// - [`DeriveModifiersError::RateNotFinite`] where the inputs are in range but the weathering
    ///   rate k they give overflows to infinity or is not a number (a host's G M★ near the largest
    ///   `f64`, or a semi-major axis whose square underflows), which the maturity cannot take.
    ///
    /// # Examples
    ///
    /// The Moon, airless and without a field at the Sun's 1 au, has the reference rate k = 1, and
    /// its 3.9 Gyr surface is mature; it carries no organic cover and no volcanic deposits.
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::partition::Condensates;
    /// use hyperion_sim::planetary::hooks::figures::MagneticField;
    /// use hyperion_sim::planetary::hooks::modifiers::{
    ///     HeliocentricOrbit, LUNAR_REFERENCE_ECCENTRICITY, ModifierHost, ModifierInputs, Primary,
    ///     SurfaceModifiers,
    /// };
    /// use hyperion_sim::units::consts::METRES_PER_AU;
    /// use hyperion_sim::units::{
    ///     Kelvin, Kilograms, Metres, MetresPerSecondSquared, Pascals, WattsPerSquareMetre, Years,
    /// };
    ///
    /// let none = Condensates::default();
    /// let moon = ModifierInputs {
    ///     host: ModifierHost::sun(),
    ///     orbit: HeliocentricOrbit {
    ///         semi_major_axis: Metres::new(METRES_PER_AU),
    ///         eccentricity: LUNAR_REFERENCE_ECCENTRICITY,
    ///     },
    ///     primary: Primary::Star,
    ///     gravity: MetresPerSecondSquared::new(1.62),
    ///     surface_pressure: Pascals::ZERO,
    ///     field: MagneticField::None,
    ///     surface_age: Years::new(3.9e9),
    ///     resurfacing_time: Years::new(f64::INFINITY),
    ///     heat_flow: WattsPerSquareMetre::new(0.012),
    ///     surface_temperature: Kelvin::new(250.0),
    ///     mantle_sulphur: Kilograms::new(1e19),
    ///     condensates: &none,
    /// };
    /// let modifiers = SurfaceModifiers::derive(&moon)?;
    /// assert!((modifiers.weathering_rate() - 1.0).abs() < 1e-12);
    /// assert!(modifiers.maturity() > 0.99);
    /// assert!(modifiers.organic_cover() < 1e-12);
    /// assert_eq!(modifiers.volcanic_deposits(), []);
    /// # Ok::<(), hyperion_sim::planetary::hooks::modifiers::DeriveModifiersError>(())
    /// ```
    pub fn derive(inputs: &ModifierInputs<'_>) -> Result<Self, DeriveModifiersError> {
        validate(inputs)?;
        let column =
            KilogramsPerSquareMetre::new(inputs.surface_pressure.value() / inputs.gravity.value());
        let exposure = Exposure::of(&ExposureInputs {
            host: &inputs.host,
            orbit: inputs.orbit,
            primary: inputs.primary,
            field: inputs.field,
            column,
        });
        let moon = Exposure::lunar_reference();
        let weathering = WeatheringRate {
            wind: exposure.wind_mass_flux / moon.wind_mass_flux,
            dust: exposure.dust / moon.dust,
        };
        let rate = weathering.value();
        // `body_maturity` panics on a rate that is not finite; validated inputs can still give one
        // through an exposure that overflows.
        if !rate.is_finite() {
            return Err(DeriveModifiersError::RateNotFinite);
        }
        let maturity = body_maturity(Gigayears::from(inputs.surface_age), rate);
        let tau = inputs.resurfacing_time.value();
        // An infinite resurfacing time renews nothing: +0.
        let resurfacing_per_yr = 1.0 / tau;
        let organic_ground = organic_ground(inputs.condensates);
        let organic_cover = if organic_ground > 0.0 {
            organic_cover(
                exposure.dose_flux,
                resurfacing_per_yr,
                inputs.surface_age.value(),
            )
        } else {
            0.0
        };
        Ok(Self {
            maturity,
            weathering,
            organic_ground,
            organic_cover,
            resurfacing_m_per_yr: RESURFACING_DEPTH.value() * resurfacing_per_yr,
            volcanic_deposits: volcanic_deposits(inputs),
        })
    }

    /// The space-weathering maturity m = 1 − exp(−k `t_s` ÷ `τ_L`), dimensionless in [+0, 1]: the
    /// background regolith's npFe⁰ content as a fraction of the Moon's mature background's, per
    /// unit of `FeO_T` (decision-p14-t51d-weathering §2.1), 0 for fresh ground.
    #[must_use]
    pub const fn maturity(&self) -> f64 {
        self.maturity
    }

    /// The weathering rate k, dimensionless, finite and at least +0, the Moon's 1
    /// ([`WeatheringRate::value`]).
    #[must_use]
    pub fn weathering_rate(&self) -> f64 {
        self.weathering.value()
    }

    /// The weathering rate's two terms.
    #[must_use]
    pub const fn weathering(&self) -> WeatheringRate {
        self.weathering
    }

    /// The share of the surface that can take an organic cover ([`organic_ground`]),
    /// dimensionless in [+0, 1].
    #[must_use]
    pub const fn organic_ground(&self) -> f64 {
        self.organic_ground
    }

    /// The organic cover: the covered fraction of the ground that can take one
    /// ([`organic_ground`](Self::organic_ground)), dimensionless in [+0, 1], +0 where none can.
    #[must_use]
    pub const fn organic_cover(&self) -> f64 {
        self.organic_cover
    }

    /// The resurfacing rate, m per Julian year, at least +0: [`RESURFACING_DEPTH`] once per the
    /// resurfacing time.
    #[must_use]
    pub const fn resurfacing_m_per_yr(&self) -> f64 {
        self.resurfacing_m_per_yr
    }

    /// An Io's volcanic deposits, SO₂ frost then sulphur, each with its share of the surface;
    /// empty for every other body.
    #[must_use]
    pub fn volcanic_deposits(&self) -> &[VolcanicDeposit] {
        &self.volcanic_deposits
    }
}

/// The ground that can take an organic cover in `condensates`, as a share of the surface in
/// [+0, 1]: the areas of the solid, non-seasonal entries whose registry row carries carbon, CO₂
/// excepted, and, where one of them covers any ground, nitrogen's, summed in the entries' order and
/// held to 1 (the entries' areas, each its own cold area, may overlap, so the sum is an upper
/// bound, labelled).
///
/// A seasonal ice takes no cover: its ground is bared every year.
#[must_use]
pub fn organic_ground(condensates: &Condensates) -> f64 {
    let carbon_dioxide = Gas::CarbonDioxide.substance();
    let nitrogen = Gas::Nitrogen.substance();
    let perennial_ices = || {
        condensates.entries().iter().filter(|e| {
            e.phase() == CondensatePhase::Solid && !e.is_seasonal() && e.area_fraction() > 0.0
        })
    };
    let carbon_bearing = |e: &&CondensateEntry| {
        let id = e.substance();
        id != carbon_dioxide
            && id
                .substance()
                .elements()
                .iter()
                .any(|&(element, _)| element == Element::C)
    };
    let mut ground = 0.0;
    let mut carbon = false;
    for entry in perennial_ices().filter(carbon_bearing) {
        ground += entry.area_fraction();
        carbon = true;
    }
    if carbon {
        for entry in perennial_ices().filter(|e| e.substance() == nitrogen) {
            ground += entry.area_fraction();
        }
    }
    if ground > 1.0 { 1.0 } else { ground }
}

/// The organic cover, dimensionless in [+0, 1], of ground that can take one under the
/// surface-reaching dose flux `dose_flux`, W m⁻², renewed at `resurfacing_per_yr` (1 ÷ τ, per
/// Julian year), at the surface age `surface_age_yr` (the [module](self) documentation).
#[must_use]
fn organic_cover(dose_flux: f64, resurfacing_per_yr: f64, surface_age_yr: f64) -> f64 {
    let saturation = ORGANIC_SATURATION_DOSE_EV_PER_16_AMU * JOULES_PER_ELECTRONVOLT
        / (16.0 * ATOMIC_MASS_CONSTANT_KG)
        * ORGANIC_LAYER_COLUMN.value();
    let irradiation_per_yr = dose_flux / saturation * SECONDS_PER_JULIAN_YEAR;
    if irradiation_per_yr <= 0.0 {
        return 0.0;
    }
    let rate = irradiation_per_yr + resurfacing_per_yr;
    let steady = irradiation_per_yr / rate;
    let x = rate * surface_age_yr;
    // Fresh ground at a zero surface age: +0, which −expm1(−0) would not give for a negative zero.
    if x > 0.0 {
        steady * -math::exp_m1(-x)
    } else {
        0.0
    }
}

/// An Io's deposits for the body `inputs` describe: SO₂ frost and sulphur in Io's shares where its
/// volcanism is [`Volcanism::Extreme`], its mantle holds sulphur and its surface is below SO₂'s
/// frost point at [`IO_SO2_PRESSURE`]; none otherwise.
#[must_use]
fn volcanic_deposits(inputs: &ModifierInputs<'_>) -> Vec<VolcanicDeposit> {
    let so2 = substance::by_key("SO2").expect("SO2 is a registry row");
    let sulphur = substance::by_key("S8").expect("S8 is a registry row");
    let extreme = Volcanism::of(inputs.heat_flow) == Volcanism::Extreme;
    let frost_point = so2
        .substance()
        .phase_diagram()
        .and_then(|diagram| diagram.saturation_temperature(IO_SO2_PRESSURE));
    let frozen = frost_point.is_some_and(|t| inputs.surface_temperature.value() < t.value());
    if extreme && inputs.mantle_sulphur.value() > 0.0 && frozen {
        vec![
            VolcanicDeposit {
                substance: so2,
                area_fraction: IO_SO2_FROST_SHARE,
            },
            VolcanicDeposit {
                substance: sulphur,
                area_fraction: IO_SULPHUR_SHARE,
            },
        ]
    } else {
        Vec::new()
    }
}

/// Refuses an input that is not finite or lies outside the range [`ModifierInputs`] states.
fn validate(inputs: &ModifierInputs<'_>) -> Result<(), DeriveModifiersError> {
    let positive = |v: f64| v.is_finite() && v > 0.0;
    let non_negative = |v: f64| v.is_finite() && v >= 0.0;
    let body = [
        (
            positive(inputs.host.radius.value()),
            ModifierInput::HostRadius,
        ),
        (
            non_negative(inputs.host.xuv_luminosity.value()),
            ModifierInput::HostXuvLuminosity,
        ),
        (
            non_negative(inputs.host.gravitational_parameter.value()),
            ModifierInput::HostGravitationalParameter,
        ),
        (
            positive(inputs.orbit.semi_major_axis.value()),
            ModifierInput::SemiMajorAxis,
        ),
        (
            (0.0..1.0).contains(&inputs.orbit.eccentricity),
            ModifierInput::Eccentricity,
        ),
        (positive(inputs.gravity.value()), ModifierInput::Gravity),
        (
            non_negative(inputs.surface_pressure.value()),
            ModifierInput::SurfacePressure,
        ),
        (
            non_negative(inputs.surface_age.value()),
            ModifierInput::SurfaceAge,
        ),
        // Positive, and infinite where nothing resurfaces the body.
        (
            inputs.resurfacing_time.value() > 0.0,
            ModifierInput::ResurfacingTime,
        ),
        (
            non_negative(inputs.heat_flow.value()),
            ModifierInput::HeatFlow,
        ),
        (
            positive(inputs.surface_temperature.value()),
            ModifierInput::SurfaceTemperature,
        ),
        (
            non_negative(inputs.mantle_sulphur.value()),
            ModifierInput::MantleSulphur,
        ),
    ];
    let planet = match inputs.primary {
        Primary::Star => (true, true),
        Primary::Planet(planet) => (
            positive(planet.radius.value()),
            positive(planet.semi_major_axis.value()),
        ),
    };
    let planet = [
        (planet.0, ModifierInput::PlanetRadius),
        (planet.1, ModifierInput::PlanetocentricSemiMajorAxis),
    ];
    match body.into_iter().chain(planet).find(|&(ok, _)| !ok) {
        Some((_, input)) => Err(DeriveModifiersError::OutOfRange(input)),
        None => Ok(()),
    }
}

/// What a body's exposure reads: its host, orbit, primary, field and air's column. Its radius is
/// not among them: its field's open share reads `r_mp` ÷ R alone, which its surface field sets.
struct ExposureInputs<'a> {
    host: &'a ModifierHost,
    orbit: HeliocentricOrbit,
    primary: Primary,
    field: MagneticField,
    /// The air's column mass Σ = p ÷ g.
    column: KilogramsPerSquareMetre,
}

/// A body's surface-reaching exposure, before the Moon's divides it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Exposure {
    /// The wind's orbit-mean mass flux at the ground, kg m⁻² s⁻¹: `Φ_w`.
    wind_mass_flux: f64,
    /// The micrometeoroids' n v³ at the ground, relative to 1 at 1 au from the Sun: `Φ_d`.
    dust: f64,
    /// The dose flux of the host's X-ray and ultraviolet light and the wind's ions' kinetic energy
    /// at the ground, W m⁻², over the globe (a quarter of the flux on the disk) and the orbit.
    dose_flux: f64,
}

impl Exposure {
    /// The exposure of the body `inputs` describe.
    #[must_use]
    fn of(inputs: &ExposureInputs<'_>) -> Self {
        let HeliocentricOrbit {
            semi_major_axis,
            eccentricity,
        } = inputs.orbit;
        let a = semi_major_axis.value();
        let area = 4.0 * core::f64::consts::PI * a * a * (1.0 - eccentricity * eccentricity).sqrt();
        let wind_flux = wind_mass_loss_kg_per_s(inputs.host) / area;
        let ram_pressure = wind_flux * WIND_SPEED.value();
        let exposed = inputs.column <= ION_STOPPING_COLUMN;
        let share = if !exposed || wind_flux <= 0.0 {
            0.0
        } else if let Primary::Planet(planet) = inputs.primary
            && inside_magnetopause(&planet, ram_pressure)
        {
            // The planet's own magnetospheric plasma, which weathers such a moon (Europa's and
            // Ganymede's), is not modelled.
            0.0
        } else {
            open_field_share(inputs.field, ram_pressure)
        };
        let wind_mass_flux = wind_flux * share;
        let speed_ratio = inputs.host.gravitational_parameter.value() / GM_SUN;
        let au = a / METRES_PER_AU;
        // Leinert et al.'s r^(−1.3) is measured over 0.3–1.0 au; beyond it, an extrapolation.
        let dust = SYSTEM_DUST_LEVEL
            * math::powf(au, -(ZODIACAL_DENSITY_INDEX + 1.5))
            * math::powf(speed_ratio, 1.5)
            * math::exp(-(inputs.column / DUST_STOPPING_COLUMN));
        let light = if exposed {
            inputs.host.xuv_luminosity.value() / area
        } else {
            0.0
        };
        let ions = 0.5 * wind_mass_flux * WIND_SPEED.value() * WIND_SPEED.value();
        Self {
            wind_mass_flux,
            dust,
            dose_flux: (light + ions) / 4.0,
        }
    }

    /// The Moon's reference exposure: the Sun's ([`ModifierHost::sun`]) at 1 au and
    /// [`LUNAR_REFERENCE_ECCENTRICITY`], with no air and no field.
    #[must_use]
    fn lunar_reference() -> Self {
        Self::of(&ExposureInputs {
            host: &ModifierHost::sun(),
            orbit: HeliocentricOrbit {
                semi_major_axis: Metres::new(METRES_PER_AU),
                eccentricity: LUNAR_REFERENCE_ECCENTRICITY,
            },
            primary: Primary::Star,
            field: MagneticField::None,
            column: KilogramsPerSquareMetre::ZERO,
        })
    }
}

/// The equatorial surface field standing in for the class `field`, T; `None` for no dynamo.
#[must_use]
fn surface_field_tesla(field: MagneticField) -> Option<f64> {
    let gauss = match field {
        MagneticField::None => return None,
        MagneticField::Weak => WEAK_FIELD,
        MagneticField::Strong => STRONG_FIELD,
    };
    Some(gauss.value() * 1e-4)
}

/// The subsolar magnetopause's distance in units of the body's radius under the field class
/// `field` and the wind's ram pressure `ram_pressure`, Pa: (2 B₀² ÷ (μ₀ ρ v²))^(1/6)
/// ([`MAGNETOPAUSE_COMPRESSION`]); `None` for no dynamo. Infinite under no wind.
#[must_use]
fn standoff_ratio(field: MagneticField, ram_pressure: f64) -> Option<f64> {
    let b = surface_field_tesla(field)?;
    let magnetic = MAGNETOPAUSE_COMPRESSION * b * b / VACUUM_PERMEABILITY;
    Some(math::powf(magnetic / ram_pressure, 1.0 / 6.0))
}

/// Whether a moon lies inside its planet's dayside magnetopause, labelled: one inside it is
/// shielded from the wind all its orbit; one outside it is taken as exposed all its orbit, its
/// passes through the planet's magnetotail not modelled.
#[must_use]
fn inside_magnetopause(planet: &PlanetMagnetosphere, ram_pressure: f64) -> bool {
    standoff_ratio(planet.field, ram_pressure)
        .is_some_and(|ratio| planet.semi_major_axis.value() < ratio * planet.radius.value())
}

/// The share of a body's surface its own field leaves open to the wind, in [0, 1]: 1 for no
/// dynamo or where the magnetopause lies at or below the surface, else the dipole's two polar caps
/// whose field lines reach past the subsolar magnetopause, 1 − √(1 − R ÷ `r_mp`): a line from
/// colatitude θ reaches R ÷ sin²θ at the equator (decision-p14-t51d-weathering §2.2, labelled).
/// Against Mercury, the ruling's check: the model's subsolar magnetopause at 14.3 nPa lies at 1.27
/// `R_M` where Winslow et al. 2013 (JGR 118, 2213, abstract read) measure 1.45, about 12% low; at
/// 1.45 the open share would fall from 0.52 to 0.44 and k from 8.89 to 8.64. Winslow et al. 2014
/// (GRL 41, 4463) find most of Mercury's southern hemisphere bombarded by the wind's ions, more
/// than the polar caps alone. Recorded, with no change.
#[must_use]
fn open_field_share(field: MagneticField, ram_pressure: f64) -> f64 {
    match standoff_ratio(field, ram_pressure) {
        Some(ratio) if ratio > 1.0 => 1.0 - (1.0 - 1.0 / ratio).sqrt(),
        Some(_) | None => 1.0,
    }
}
