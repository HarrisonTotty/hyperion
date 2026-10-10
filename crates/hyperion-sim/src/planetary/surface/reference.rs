//! The reference worlds: Earth-, Mars-, Moon- and Ceres-like inputs to the coarse pass, which
//! R09.T11–T16 and R10's world tests run it on (plan R09, T10; the crate's `testing` feature).
//!
//! These are test inputs, not generated worlds: each figure is the real body's, or what plan 14's
//! own rules would state for it, written as the builder argument plan 14 will one day supply
//! (Design note 3). Where plan 14 has a rule the generator already follows, a world follows it,
//! so that the reference worlds and the generated ones are one universe (the owner's "Milky
//! Way-like" rule): an airless world's mean surface temperature is its equilibrium temperature
//! at the airless Bond albedo, as `derive::atmosphere` sets it. Each figure carries its source,
//! checked by a science review of 2026-10-09; one with none is marked a placeholder, standing for
//! the plan-14 figure that has no rule yet (P14.T24.a's contrasts, P14.T24.b's ages and heat
//! flows, P14.T48.a's V), and a task that tests against one treats it as such. `σ_h` is Design
//! note 3's: measured for Earth and Mars, and from the fit for the Moon and Ceres, whose constants
//! they calibrate (R09.T0.b: "T10's reference worlds may take `σ_h` from the fit"). An area is the
//! cover all year, and a reservoir a mass per square metre of the whole surface
//! ([`Condensate`]).
//!
//! The substances are plan 14's registry keys ([`SubstanceKey`], decision-composition §1.1),
//! checked by their grammar alone until P14.T49.b's rows exist, whose test then checks every key
//! these worlds use. Each world's palette (decision-composition §1.7) carries the values the server
//! will resolve from those rows (P14.T49.b–c), which are not built: each substance's figures are
//! the ones its row below states with their sources. They began as R09.T2's synthetic palettes'
//! (`hyperion_surface::testing`) and are kept apart from them, so that a correction to either set
//! moves only its own output (the wire goldens pin the synthetic fields' palettes, and R09.T16's
//! coarse goldens will pin these): a science review of 2026-10-10 read a basalt solidus 70 K
//! below the synthetic one, which these rows take, and labelled what it could not read.

use hyperion_surface::craters::{CraterParams, CraterParamsParts, Screening};
use hyperion_surface::field::{
    BodyRef, ClimateModelKind, MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole,
};
use hyperion_surface::spheroid::Spheroid;
use hyperion_surface::substance_key::SubstanceKey;

use super::inputs::{
    AreaShare, ClimateRegime, CoarseInputs, Condensate, CondensatePhase, CrustInputs, Forcing,
    GasShare, Persistence, SeasonalOrbit, Spin, TectonicRegime, ThermalRegime, WetEpoch,
};
use crate::math;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::derive::irradiation;
use crate::planetary::hooks::SurfaceSeed;
use crate::units::consts::{SECONDS_PER_DAY, SOLAR_CONSTANT_W_PER_M2};
use crate::units::{
    EarthFluxes, Gigayears, Kelvin, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres,
    MetresPerSecondSquared, Pascals, PerSquareKilometre, Radians, Seconds, WattsPerSquareMetre,
    Years,
};

/// The surface seed the reference worlds' tests run under.
pub const SEED: SurfaceSeed = SurfaceSeed::new(0x5eed_0009_0010);

/// Water.
const H2O: SubstanceKey = SubstanceKey::new_const("H2O");
/// Carbon dioxide.
const CO2: SubstanceKey = SubstanceKey::new_const("CO2");
/// Basalt, the secondary crust's default (P14.T51.c).
const BASALT_KEY: SubstanceKey = SubstanceKey::new_const("basalt");
/// Granite, a tertiary crust (P14.T51.c).
const GRANITE_KEY: SubstanceKey = SubstanceKey::new_const("granite");
/// Anorthosite, a primary flotation crust (P14.T51.c).
const ANORTHOSITE_KEY: SubstanceKey = SubstanceKey::new_const("anorthosite");
/// A phyllosilicate-rich rock (P14.T49.b's lithologies).
const PHYLLOSILICATE_KEY: SubstanceKey = SubstanceKey::new_const("phyllosilicate");

/// A substance of the reference palettes with its figures: what a [`PaletteEntry`] holds but its
/// role, as the server would resolve it from the substance's registry row (P14.T49.b–c).
///
/// The normal albedos are the same in B, V and R: a band's value is P14.T49.c's to resolve from
/// spectra, so each row takes one visible figure, the midpoint of R10's Design note 8's range for
/// its class (researched there, mostly from memory, and to be settled in R10.T1.a), but the
/// anorthosite's, the top of its range, the phyllosilicate's, Ceres's measured albedo, and liquid
/// water's, the Fresnel reflectance of its surface at normal incidence in each band. Every phase
/// row is 0 until R10 assigns its rows. The densities follow no one convention yet (a grain
/// density for the phyllosilicate, a porous bulk for the highlands and the dust), which P14.T49.b's
/// rows settle, with the porosity apart.
struct Substance {
    key: SubstanceKey,
    normal_albedo_bvr: [f64; 3],
    density_kg_m3: f64,
    transition_k: f64,
    mechanics: MechanicsFamily,
}

impl Substance {
    /// The entry of the substance in `role`.
    #[must_use]
    const fn entry(&self, role: PaletteRole) -> PaletteEntry {
        PaletteEntry {
            substance: self.key,
            role,
            normal_albedo_bvr: self.normal_albedo_bvr,
            phase_row: 0,
            density_kg_m3: self.density_kg_m3,
            transition: Kelvin::new(self.transition_k),
            mechanics: self.mechanics,
        }
    }
}

/// The solidus of a basalt at about 1 atm, 980 °C, kelvin: a natural Hawaiian tholeiite's, below
/// which its residual glass, 4 per cent, stops falling (Wright and Okamura 1977, USGS Professional
/// Paper 1004, abstract and Table 15, after Wright and Weiblen 1967; ±10 °C). A dry basalt's is
/// higher, about 1,050 °C (Yoder and Tilley 1962, J. Petrol. 3, 342, from memory and not read; low
/// confidence). Basalt's, and, by assumption without a source, the basaltic dust's and the
/// phyllosilicate's dehydrated residue's.
const BASALT_SOLIDUS_K: f64 = 1_253.15;

/// Basalt: Design note 8's basaltic rock, `A_N` 0.05–0.15; a typical bulk density of 2,900 kg m⁻³
/// (2,700–3,100; Philpotts and Ague 2009, Principles of Igneous and Metamorphic Petrology, p. 22,
/// not read; low confidence).
const BASALT: Substance = Substance {
    key: BASALT_KEY,
    normal_albedo_bvr: [0.10; 3],
    density_kg_m3: 2_900.0,
    transition_k: BASALT_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// Granite: Design note 8's felsic rock, `A_N` 0.2–0.35; a bulk density of 2,650 kg m⁻³ (2,650–
/// 2,750, from memory; no source read; low confidence) and a dry solidus at 1 atm of about 960 °C
/// (from memory; the dry haplogranite solidus's primary source is Huang and Wyllie 1975, J. Geol.
/// 83, 737, not read; low confidence).
const GRANITE: Substance = Substance {
    key: GRANITE_KEY,
    normal_albedo_bvr: [0.275; 3],
    density_kg_m3: 2_650.0,
    transition_k: 1_233.15,
    mechanics: MechanicsFamily::Silicate,
};

/// Anorthosite, as the lunar highlands' mature regolith: the top of Design note 8's regolith
/// range, 0.07–0.20, the highlands being its brightest; the highland crust's bulk density, 2,550
/// kg m⁻³ at 12 per cent porosity (Wieczorek et al. 2013, Science 339, 671, from GRAIL); and the
/// solidus of its calcic plagioclase (An₉₅) alone at 1 atm, about 1,500 °C (an ideal-solution loop
/// gives 1,515–1,527 °C; Bowen 1913, Am. J. Sci. s4-35, 577, not read; low confidence), an upper
/// bound for the rock: its few per cent of pyroxene begin to melt with the plagioclase near the
/// anorthite–diopside eutectic, about 1,270 °C (Osborn 1942, Am. J. Sci. 240, 751, from memory and
/// not read).
const ANORTHOSITE: Substance = Substance {
    key: ANORTHOSITE_KEY,
    normal_albedo_bvr: [0.20; 3],
    density_kg_m3: 2_550.0,
    transition_k: 1_773.15,
    mechanics: MechanicsFamily::Silicate,
};

/// A phyllosilicate-rich rock, as Ceres's crust: Ceres's geometric albedo, 0.094 ± 0.007 at
/// 0.55 µm (Ciarniello et al. 2017, A&A 598, A130), which is its normal albedo (Schröder et al.
/// 2017, Icarus 288, 201, §4, who adopt Tedesco 1989's 0.10 ± 0.01); a grain density of 2,500
/// kg m⁻³, between saponite's 2,240–2,300 and lizardite's 2,550 (Anthony et al., Handbook of
/// Mineralogy), not Ceres's bulk crust, 1,200–1,400 kg m⁻³ with its ice and salts (Ermakov et al.
/// 2017, JGR Planets 122, 2267); and, for its dehydrated residue's solidus, basalt's (an
/// assumption, no source; low confidence: a magnesian residue, olivine and pyroxene, would melt
/// hotter).
const PHYLLOSILICATE: Substance = Substance {
    key: PHYLLOSILICATE_KEY,
    normal_albedo_bvr: [0.094; 3],
    density_kg_m3: 2_500.0,
    transition_k: BASALT_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// Sodium carbonate, the salt of Ceres's faculae (De Sanctis et al. 2016, Nature 536, 54):
/// Design note 8's evaporite, `A_N` 0.5–0.8, which Cerealia Facula's visual normal albedo,
/// 0.6 ± 0.1, bears out (Schröder et al. 2017, Icarus 288, 201); its density, 2,540 kg m⁻³ (CRC
/// Handbook of Chemistry and Physics, 95th edition, not read; International Chemical Safety Card
/// 1135's 2.5 g cm⁻³ agrees), and melting point, 851 °C (International Chemical Safety Card 1135;
/// the literature spans 850–856 °C, NIST's WebBook putting the change at 1,123 K and the CRC's
/// 95th edition at 856 °C).
const SODIUM_CARBONATE: Substance = Substance {
    key: SubstanceKey::new_const("Na2CO3"),
    normal_albedo_bvr: [0.65; 3],
    density_kg_m3: 2_540.0,
    transition_k: 1_124.15,
    mechanics: MechanicsFamily::Salt,
};

/// Mars's bright dust: Design note 8's 0.18–0.25 in V; the bulk density of the Viking landers'
/// undisturbed drift material, 1,200 kg m⁻³ (Moore et al. 1987, USGS Professional Paper 1389,
/// p. 126); basaltic, so basalt's solidus (an assumption, no source).
const MARS_DUST: Substance = Substance {
    key: SubstanceKey::new_const("mars_dust"),
    normal_albedo_bvr: [0.215; 3],
    density_kg_m3: 1_200.0,
    transition_k: BASALT_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// The melting point of water ice at 1 atm, kelvin: 273.152 519 K (IAPWS R10-06), to 0.01 K.
const WATER_MELTING_K: f64 = 273.15;

/// Water ice, as snow-covered ice: Design note 8's fresh snow, 0.95–0.98 in the visible
/// (Wiscombe and Warren 1980; Grenfell et al. 1994); ice Ih's density at its melting point at
/// 1 atm, 916.72 kg m⁻³ (IAPWS R10-06(2009), Table 6: 916.721 463 kg m⁻³; the equation of Feistel
/// and Wagner 2006, J. Phys. Chem. Ref. Data 35, 1021).
const WATER_ICE: Substance = Substance {
    key: H2O,
    normal_albedo_bvr: [0.965; 3],
    density_kg_m3: 916.72,
    transition_k: WATER_MELTING_K,
    mechanics: MechanicsFamily::WaterIce,
};

/// Liquid water: the Fresnel reflectance at normal incidence, ((n − 1) ÷ (n + 1))², of n = 1.337,
/// 1.333 and 1.331 at 0.45, 0.55 and 0.65 µm (Hale and Querry 1973's grid points, which give the
/// same three at the bands' 0.44 and 0.64 µm), at 25 °C (Hale and Querry 1973, Appl. Opt. 12,
/// 555), standing in for `A_N`, which a specular surface does not have, until R11's ocean
/// replaces it; its density at 0 °C and 1 atm, 999.84 kg m⁻³ (IAPWS-95, Wagner and Pruß 2002, J.
/// Phys. Chem. Ref. Data 31, 387).
const WATER: Substance = Substance {
    key: H2O,
    normal_albedo_bvr: [0.020_8, 0.020_4, 0.020_2],
    density_kg_m3: 999.84,
    transition_k: WATER_MELTING_K,
    mechanics: MechanicsFamily::WaterIce,
};

/// CO₂ ice: Design note 8's carbon dioxide frost, 0.4–0.8; its density at 150 K, a Mars-like
/// frost's temperature, 1,621 kg m⁻³ (Mangan et al. 2017, Icarus 294, 201, whose fit over 80–195
/// K is ρ = 1.723 91 − 2.53 × 10⁻⁴ T − 2.87 × 10⁻⁶ T² g cm⁻³); and its triple point, 216.592 K,
/// where it can first melt (Span and Wagner 1996, J. Phys. Chem. Ref. Data 25, 1509).
const CARBON_DIOXIDE_ICE: Substance = Substance {
    key: CO2,
    normal_albedo_bvr: [0.6; 3],
    density_kg_m3: 1_621.0,
    transition_k: 216.592,
    mechanics: MechanicsFamily::VolatileIce,
};

/// The reference Earth's dry air by mole, as [`earth_like`]'s table sources it: a `const`, so that
/// a malformed key fails to compile.
const EARTH_AIR: [GasShare; 7] = [
    gas("N2", 0.780_84),
    gas("O2", 0.209_476),
    gas("Ar", 0.009_34),
    gas("CO2", 0.000_314),
    gas("Ne", 0.000_018_18),
    gas("He", 0.000_005_24),
    gas("CH4", 0.000_002),
];

/// The reference Mars's air by mole, as [`mars_like`]'s table sources it.
const MARS_AIR: [GasShare; 5] = [
    gas("CO2", 0.951),
    gas("N2", 0.0259),
    gas("Ar", 0.0194),
    gas("O2", 0.001_61),
    gas("CO", 0.000_58),
];

/// The palette of `entries`, given in (role, key) order.
#[must_use]
fn palette(entries: Vec<PaletteEntry>) -> MaterialPalette {
    MaterialPalette::new(entries).expect("a reference palette is valid")
}

/// The density of a stony projectile that screening assumes, kg m⁻³: 3,000, the value that turns
/// Design note 12's d\* = 1.5 (P ÷ g) ÷ `ρ_p` into its 5.2 m for Earth and 0.52 km for Venus (its
/// 8.2 cm for Mars takes about 610 Pa; NASA's 636 Pa gives 8.5 cm).
const PROJECTILE_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(3_000.0);

/// The sidereal year, days (NASA's Earth fact sheet: 365.256 d).
const SIDEREAL_YEAR_DAYS: f64 = 365.256;

/// N(>1 km), km⁻², at a surface age of `t` by the lunar chronology of Neukum, Ivanov and Hartmann
/// 2001 (Space Science Reviews 96, 55), as plan 14's P14.T24.b cites it: 5.44 × 10⁻¹⁴ (e^(6.93 T)
/// − 1) plus 8.38 × 10⁻⁴ T with T in Gyr, before plan 14's scaling by the system's belts.
#[must_use]
fn lunar_chronology(t: Gigayears) -> PerSquareKilometre {
    let t = t.value();
    PerSquareKilometre::new(5.44e-14 * (math::exp(6.93 * t) - 1.0) + 8.38e-4 * t)
}

/// The Sun's light averaged over an orbit of semi-major axis `a_au` astronomical units and
/// eccentricity `e`: S☉ ÷ (a² √(1 − e²)), P14.T12's orbit average.
#[must_use]
fn orbit_mean_flux(a_au: f64, e: f64) -> WattsPerSquareMetre {
    WattsPerSquareMetre::new(SOLAR_CONSTANT_W_PER_M2 / (a_au * a_au * (1.0 - e * e).sqrt()))
}

/// The mean surface temperature plan 14 gives an airless rocky body under `flux`: its equilibrium
/// temperature at the airless rock's Bond albedo, by the generator's own functions
/// (`derive::irradiation::equilibrium_temperature` and `SurfaceState::albedo`, P14.T13.c).
#[must_use]
fn airless_rock_temperature(flux: WattsPerSquareMetre) -> Kelvin {
    irradiation::equilibrium_temperature(
        EarthFluxes::from(flux),
        SurfaceState::Airless.albedo(SurfaceMaterial::Rock),
    )
}

/// The mean solar day of a prograde rotation of sidereal period `rotation` on an orbit of period
/// `orbit`: 1 ÷ (1 ÷ `P_rot` − 1 ÷ `P_orb`).
#[must_use]
fn solar_day(rotation: Seconds, orbit: Seconds) -> Seconds {
    Seconds::new(1.0 / (1.0 / rotation.value() - 1.0 / orbit.value()))
}

/// An angle of `deg` degrees in radians.
#[must_use]
fn degrees(deg: f64) -> Radians {
    Radians::new(deg.to_radians())
}

/// The crater contract at the lunar chronology's N(>1 km) for `surface_age`, under `gravity`,
/// screened by `screening`, on a target of factor `k_target`.
#[must_use]
fn crater_params(
    surface_age: Gigayears,
    gravity: MetresPerSecondSquared,
    screening: Screening,
    k_target: f64,
) -> CraterParams {
    CraterParams::new(CraterParamsParts {
        n_1km: lunar_chronology(surface_age),
        screening,
        gravity,
        k_target,
        impact_velocity: None,
    })
    .expect("a reference world's crater contract is valid")
}

/// An atmosphere's screening of projectiles: its column mass P ÷ g over a stony projectile
/// (Design note 12).
#[must_use]
fn atmosphere_screening(pressure: Pascals, gravity: MetresPerSecondSquared) -> Screening {
    Screening::Atmosphere {
        column_mass: KilogramsPerSquareMetre::new(pressure.value() / gravity.value()),
        projectile_density: PROJECTILE_DENSITY,
    }
}

/// The figure of volumetric radius `radius_m` and flattening `f`.
#[must_use]
fn figure(radius_m: f64, f: f64) -> Spheroid {
    Spheroid::from_volumetric(radius_m, f).expect("a reference world's figure is valid")
}

/// An Earth-like world: a mobile lid with oceans and ice under a temperate, seasonal climate.
///
/// | Figure | Value | Source |
/// | --- | --- | --- |
/// | radius, flattening | 6,371.0 km, 1 ÷ 298.257 223 563 | NASA's Earth fact sheet (volumetric mean); WGS 84 |
/// | gravity | GM ÷ R², GM = 3.986 004 418 × 10¹⁴ m³ s⁻² | IERS Conventions 2010 (TN 36, Table 1.1) |
/// | `σ_h` | 2.51 km | Design note 3 (Earth2014, R09.T0.b) |
/// | ocean | water over 0.7095 of the surface; 1.335 × 10⁹ km³ at 1,030 kg m⁻³ | 361.9 of 510.082 × 10⁶ km² and the volume (Eakins and Sharman 2010, NOAA NGDC, Table 1); a mean seawater density, from memory |
/// | ice | water ice over 0.041 all year; 29.9 × 10⁶ km³ at 917 kg m⁻³ | Antarctica's 13.92 × 10⁶ km² and 26.92 × 10⁶ km³ with its shelves (Bedmap2, Fretwell et al. 2013) and Greenland's 1.71 × 10⁶ km² and about 3 × 10⁶ km³ (Morlighem et al. 2017's bed model), with the two hemispheres' summer-minimum sea ice, about 5.3 × 10⁶ km² of area (NSIDC Sea Ice Index, from memory); the annual-mean sea ice is the climate's |
/// | mean temperature | 288 K | NASA's fact sheet |
/// | equator–pole contrast | 42 K | −(3/2) T₂ of the annual zonal fit T₀ + T₂ P₂(sin φ) with T₂ = −28 K (North, Cahalan and Coakley 1981, Rev. Geophys. 19, 91; not read, medium confidence) |
/// | day–night contrast | 3 K | a placeholder for P14.T24.a's: about 10 K over land and under 1 K over the ocean, area-weighted (low confidence) |
/// | spin | obliquity 23.44°, sidereal day 23.9345 h; the March equinox at a true anomaly of 77.053°, 180° less the perihelion's longitude 102.947° | NASA's fact sheet and its J2000 elements |
/// | orbit | 1.000 au, e 0.0167, 365.256 d | NASA's fact sheet |
/// | air | 98.55 kPa; dry, by mole: N₂ 0.780 84, O₂ 0.209 476, Ar 0.009 34, CO₂ 0.000 314, Ne 1.818 × 10⁻⁵, He 5.24 × 10⁻⁶, CH₄ 2 × 10⁻⁶ | the mean total surface pressure over the topography (Trenberth and Smith 2005, J. Climate 18, 864; sea level's is 101.3 kPa); the U.S. Standard Atmosphere 1976's sea-level composition (Table 3), whose CO₂ is about 420 × 10⁻⁶ today; water vapour, about 1% and variable, is the climate's |
/// | regime | mobile lid, `f_c` 0.405 | Design note 3 |
/// | volcanism V | 1 | a placeholder: plan 14 has not set V's scale, which Mars calibrates at 1 (P14.T48.a) |
/// | heat flow | 0.0916 W m⁻² | Davies and Davies 2010, Solid Earth 1, 5, Table 7 (46.7 TW) |
/// | surface age | 0.1 Gyr | a placeholder until P14.T24.b defines its resurfacing time, of the order of the oceanic crust's mean age, 64.2 Myr (Seton et al. 2020, G³ 21, e2020GC009214) |
/// | craters | the lunar chronology at that age, screened by the air | Design note 12 |
/// | wet epoch | 4.4 Gyr ago to now, the ocean's inventory | the oldest zircons' evidence of liquid water (Wilde, Valley, Peck and Graham 2001, Nature 409, 175) |
/// | crust | basaltic oceanic, granitic continents, basaltic provinces | P14.T51.c's Earth |
/// | palette | basalt (secondary crust), granite (tertiary), water as ice and as liquid | decision-composition §1.7; each figure its substance's row in this module, with its sources |
///
/// # Panics
///
/// Never: its figures are valid, as a test builds it.
#[must_use]
pub fn earth_like() -> CoarseInputs {
    let radius_m = 6_371.0e3;
    let gravity = MetresPerSecondSquared::new(3.986_004_418e14 / (radius_m * radius_m));
    let pressure = Pascals::new(98_550.0);
    let year = Seconds::new(SIDEREAL_YEAR_DAYS * SECONDS_PER_DAY);
    let rotation = Seconds::new(23.9345 * 3_600.0);
    let surface_age = Gigayears::new(0.1);
    let area_m2 = 510.082e12;
    let ocean = KilogramsPerSquareMetre::new(1.335e18 * 1_030.0 / area_m2);
    let ice = KilogramsPerSquareMetre::new(29.9e15 * 917.0 / area_m2);
    CoarseInputs::builder()
        .body(BodyRef::new(0, 1))
        .figure(figure(radius_m, 1.0 / 298.257_223_563))
        .gravity(gravity)
        .sigma_h(Metres::new(2_510.0))
        .liquids([AreaShare {
            substance: H2O,
            area_fraction: 0.7095,
        }])
        .ices([AreaShare {
            substance: H2O,
            area_fraction: 0.041,
        }])
        .mean_surface_temperature(Kelvin::new(288.0))
        .equator_pole_contrast_k(42.0)
        .day_night_contrast_k(3.0)
        .spin(Spin {
            obliquity: degrees(23.44),
            rotation_period: rotation,
            solar_day: Some(solar_day(rotation, year)),
            equinox_true_anomaly: degrees(180.0 - 102.947),
        })
        .host_flux(orbit_mean_flux(1.0, 0.0167))
        .seasonal_orbit(SeasonalOrbit {
            eccentricity: 0.0167,
            period: year,
        })
        .surface_pressure(pressure)
        .gases(EARTH_AIR)
        .surface(SurfaceState::Temperate, SurfaceMaterial::Rock)
        .tectonics(TectonicRegime::MobileLid, 0.405)
        .volcanism(1.0)
        .heat_flow(WattsPerSquareMetre::new(0.0916))
        .surface_age(surface_age)
        .craters(crater_params(
            surface_age,
            gravity,
            atmosphere_screening(pressure, gravity),
            1.0,
        ))
        .wet_epoch(WetEpoch {
            start: Gigayears::new(4.4),
            end: Gigayears::new(0.0),
            effective_flow: Years::new(4.4e9),
            paleo_inventory: ocean,
        })
        .climate(ClimateRegime {
            thermal: ThermalRegime::Efficient,
            forcing: Forcing::Seasonal,
            condensable: Some(H2O),
            model: ClimateModelKind::EnergyBalance,
        })
        .crust(CrustInputs {
            primary: None,
            secondary: BASALT_KEY,
            tertiary: Some(GRANITE_KEY),
            provinces: BASALT_KEY,
            melt_area_fraction: 0.0,
        })
        .palette(palette(vec![
            BASALT.entry(PaletteRole::SecondaryCrust),
            GRANITE.entry(PaletteRole::TertiaryCrust),
            WATER_ICE.entry(PaletteRole::Ice),
            WATER.entry(PaletteRole::Liquid),
        ]))
        .condensates([
            condensate(
                H2O,
                CondensatePhase::Liquid,
                ocean,
                0.7095,
                Persistence::Perennial,
            ),
            condensate(
                H2O,
                CondensatePhase::Solid,
                ice,
                0.041,
                Persistence::Seasonal,
            ),
        ])
        .build()
        .expect("the reference Earth's inputs are valid")
}

/// A Mars-like world: a stagnant lid under a thin carbon-dioxide air, dry now after a wet epoch,
/// with water- and carbon-dioxide-ice caps.
///
/// | Figure | Value | Source |
/// | --- | --- | --- |
/// | radius, flattening | 3,389.5 km, 0.005 89 | NASA's Mars fact sheet |
/// | gravity | GM ÷ R², GM = 4.282 837 × 10¹³ m³ s⁻² | JPL's Mars parameters (NASA's fact sheet gives 0.042 828 × 10⁶ km³ s⁻²) |
/// | `σ_h` | 2.90 km | Design note 3 (MOLA, R09.T0.b) |
/// | ice | water over 0.007 all year, 2.0 × 10⁴ kg m⁻²; carbon dioxide over 0.0006 all year, 170 kg m⁻², seasonal beyond | the north residual cap's about 1 × 10⁶ km² of 144.4 × 10⁶ km² (from memory) and the polar deposits' 20–30 m of water as a global layer (Carr and Head 2003, JGR 108, 5042, its low end); the south residual cap's about 0.09 × 10⁶ km² (from memory) and the buried CO₂ that would double the air's pressure (Bierson et al. 2016, GRL 43, 4172), so about the air's own column; low confidence |
/// | mean temperature | 214 K | NASA's fact sheet ("~214 K") |
/// | contrasts | 55 K equator–pole, 60 K day–night | placeholders for P14.T24.a's: about 215 K at the equator against 160 K at the poles, and the diurnal swing at low latitudes (from memory, low confidence) |
/// | spin | obliquity 25.19°, sidereal day 24.6229 h; the northern spring equinox at a true anomaly of 109.0°, 360° less perihelion's Ls of 251.0° | NASA's fact sheet; Allison and McEwen 2000, PSS 48, 215, eqs. 16–17, at J2000 |
/// | orbit | 1.523 68 au, e 0.0935, 686.980 d | plan 14's Solar System table (`derive`); NASA's fact sheet |
/// | air | 636 Pa: CO₂ 0.951, N₂ 0.0259, Ar 0.0194, O₂ 0.001 61, CO 0.000 58 | NASA's fact sheet's pressure; Franz et al. 2017 (PSS 138, 44, Curiosity's SAM; not read), whose rounding NASA's 95.1, 2.59, 1.94, 0.16 and 0.06% match |
/// | regime | stagnant lid, V = 1 | Design note 3 (Mars calibrates V) |
/// | heat flow | 0.019 W m⁻² | Parro et al. 2017, Sci. Rep. 7, 45629 (Design note 3) |
/// | surface age | 3.9 Gyr | a placeholder: the Noachian highlands' age, from memory |
/// | craters | the lunar chronology at that age, screened by the air (d\* = 8.5 cm at 636 Pa) | Design note 12, whose 8.2 cm takes about 610 Pa; the Mars–Moon flux ratio is plan 14's belt scaling, not applied |
/// | wet epoch | 3.9 to 3.6 Gyr ago, 10⁶ Earth-equivalent years, 1.56 × 10⁵ kg m⁻² of water | the valley networks of about 3.6–3.8 Ga (Hoke and Hynek 2009) in 10⁵–10⁷ such years (Hoke, Hynek and Tucker 2011; not read); Carr and Head 2003's ocean, about 2.3 × 10⁷ km³, 156 m as a global layer (low confidence) |
/// | crust | basaltic, basaltic provinces | P14.T51.c's Mars |
/// | palette | basalt (secondary crust, its provinces' too), CO₂ and water ices, the bright dust as its deposit | decision-composition §1.7; each figure its row's |
///
/// # Panics
///
/// Never: its figures are valid, as a test builds it.
#[must_use]
pub fn mars_like() -> CoarseInputs {
    let gravity = MetresPerSecondSquared::new(4.282_837e13 / (3_389.5e3 * 3_389.5e3));
    let pressure = Pascals::new(636.0);
    let year = Seconds::new(686.980 * SECONDS_PER_DAY);
    let rotation = Seconds::new(24.6229 * 3_600.0);
    let surface_age = Gigayears::new(3.9);
    CoarseInputs::builder()
        .body(BodyRef::new(0, 2))
        .figure(figure(3_389.5e3, 0.005_89))
        .gravity(gravity)
        .sigma_h(Metres::new(2_900.0))
        .ices([
            AreaShare {
                substance: H2O,
                area_fraction: 0.007,
            },
            AreaShare {
                substance: CO2,
                area_fraction: 0.000_6,
            },
        ])
        .mean_surface_temperature(Kelvin::new(214.0))
        .equator_pole_contrast_k(55.0)
        .day_night_contrast_k(60.0)
        .spin(Spin {
            obliquity: degrees(25.19),
            rotation_period: rotation,
            solar_day: Some(solar_day(rotation, year)),
            equinox_true_anomaly: degrees(360.0 - 251.0),
        })
        .host_flux(orbit_mean_flux(1.523_68, 0.0935))
        .seasonal_orbit(SeasonalOrbit {
            eccentricity: 0.0935,
            period: year,
        })
        .surface_pressure(pressure)
        .gases(MARS_AIR)
        .surface(SurfaceState::Temperate, SurfaceMaterial::Rock)
        .tectonics(TectonicRegime::StagnantLid, 0.0)
        .volcanism(1.0)
        .heat_flow(WattsPerSquareMetre::new(0.019))
        .surface_age(surface_age)
        .craters(crater_params(
            surface_age,
            gravity,
            atmosphere_screening(pressure, gravity),
            1.0,
        ))
        .wet_epoch(WetEpoch {
            start: Gigayears::new(3.9),
            end: Gigayears::new(3.6),
            effective_flow: Years::new(1e6),
            paleo_inventory: KilogramsPerSquareMetre::new(156.0 * 1_000.0),
        })
        .climate(ClimateRegime {
            thermal: ThermalRegime::Transitional,
            forcing: Forcing::Seasonal,
            condensable: Some(CO2),
            model: ClimateModelKind::RadiativeEquilibrium,
        })
        .crust(CrustInputs {
            primary: None,
            secondary: BASALT_KEY,
            tertiary: None,
            provinces: BASALT_KEY,
            melt_area_fraction: 0.0,
        })
        .palette(palette(vec![
            BASALT.entry(PaletteRole::SecondaryCrust),
            CARBON_DIOXIDE_ICE.entry(PaletteRole::Ice),
            WATER_ICE.entry(PaletteRole::Ice),
            MARS_DUST.entry(PaletteRole::Deposit),
        ]))
        .condensates([
            condensate(
                H2O,
                CondensatePhase::Solid,
                KilogramsPerSquareMetre::new(20.0 * 1_000.0),
                0.007,
                Persistence::Perennial,
            ),
            condensate(
                CO2,
                CondensatePhase::Solid,
                KilogramsPerSquareMetre::new(170.0),
                0.000_6,
                Persistence::Seasonal,
            ),
        ])
        .build()
        .expect("the reference Mars's inputs are valid")
}

/// A Moon-like world: an airless, synchronously locked satellite of an Earth, its seasons its
/// planet's orbit about the Sun.
///
/// | Figure | Value | Source |
/// | --- | --- | --- |
/// | radius, flattening | 1,737.4 km, 0.0012 | NASA's Moon fact sheet |
/// | gravity | GM ÷ R², GM = 4,902.800 km³ s⁻² | JPL's DE440 (NASA's fact sheet gives 0.004 90 × 10⁶ km³ s⁻²) |
/// | `σ_h` | 2.239 km | Design note 3's fit with V = 0: √(0.9² + 2.05²) km, its stagnant lid and its basin share at saturation, the share the Moon calibrates |
/// | mean temperature | the equilibrium temperature at the airless Bond albedo 0.11, about 270 K | plan 14's airless rule (P14.T13.c), a radiative mean: the arithmetic mean is far lower, Diviner's equator swinging between about 95 and 395 K (Williams et al. 2017, Icarus 283, 300) |
/// | contrasts | 120 K equator–pole, 200 K day–night | placeholders for P14.T24.a's, from Diviner's swings (low confidence); against the radiative mean above they are not one arithmetic field (R09's Risks) |
/// | spin | 1.535° from the ecliptic's pole, sidereal month 27.3217 d; no fixed equinox phase, since the pole precesses in 18.6 years | NASA's fact sheet: its 6.68° to the orbit less the orbit's 5.145° to the ecliptic, the Cassini state |
/// | orbit | its planet's: 1.000 au, e 0.0167, 365.256 d | the Earth's |
/// | regime | stagnant lid, no constructional volcanism, airless | Design note 3; P14.T13.c |
/// | heat flow | 0.018 W m⁻² | Apollo 15's 21 and Apollo 17's 14 mW m⁻² (Langseth, Keihm and Peters 1976, quoted by Saito et al. 2006), whose global estimate is about 1.8 µW cm⁻² |
/// | surface age | 4.4 Gyr | R09.T12.d's test ("N(1 km) at 4.4 Gyr") |
/// | crust | anorthositic, basaltic secondary crust and provinces | P14.T51.c's Moon |
/// | palette | anorthosite (primary crust), basalt (provinces: the maria, its secondary crust) | decision-composition §1.7; each figure its row's |
///
/// # Panics
///
/// Never: its figures are valid, as a test builds it.
#[must_use]
pub fn moon_like() -> CoarseInputs {
    let gravity = MetresPerSecondSquared::new(4.902_800e12 / (1_737.4e3 * 1_737.4e3));
    let year = Seconds::new(SIDEREAL_YEAR_DAYS * SECONDS_PER_DAY);
    let rotation = Seconds::new(27.3217 * SECONDS_PER_DAY);
    let surface_age = Gigayears::new(4.4);
    let flux = orbit_mean_flux(1.0, 0.0167);
    CoarseInputs::builder()
        .body(BodyRef::new(0, 3))
        .figure(figure(1_737.4e3, 0.0012))
        .gravity(gravity)
        .sigma_h(Metres::new(2_239.0))
        .mean_surface_temperature(airless_rock_temperature(flux))
        .equator_pole_contrast_k(120.0)
        .day_night_contrast_k(200.0)
        .spin(Spin {
            obliquity: degrees(6.68 - 5.145),
            rotation_period: rotation,
            solar_day: Some(solar_day(rotation, year)),
            equinox_true_anomaly: Radians::ZERO,
        })
        .host_flux(flux)
        .seasonal_orbit(SeasonalOrbit {
            eccentricity: 0.0167,
            period: year,
        })
        .surface_pressure(Pascals::ZERO)
        .surface(SurfaceState::Airless, SurfaceMaterial::Rock)
        .tectonics(TectonicRegime::StagnantLid, 0.0)
        .volcanism(0.0)
        .heat_flow(WattsPerSquareMetre::new(0.018))
        .surface_age(surface_age)
        .craters(crater_params(surface_age, gravity, Screening::None, 1.0))
        .climate(ClimateRegime {
            thermal: ThermalRegime::AirlessLike,
            forcing: Forcing::SlowRotator,
            condensable: None,
            model: ClimateModelKind::RadiativeEquilibrium,
        })
        .crust(CrustInputs {
            primary: Some(ANORTHOSITE_KEY),
            secondary: BASALT_KEY,
            tertiary: None,
            provinces: BASALT_KEY,
            melt_area_fraction: 0.0,
        })
        .palette(palette(vec![
            ANORTHOSITE.entry(PaletteRole::PrimaryCrust),
            BASALT.entry(PaletteRole::Province),
        ]))
        .build()
        .expect("the reference Moon's inputs are valid")
}

/// A Ceres-like world: an airless, ice-rich dwarf planet in the asteroid belt.
///
/// | Figure | Value | Source |
/// | --- | --- | --- |
/// | radius, flattening | 469.7 km, 0.075 | Ermakov et al. 2017 (R09.T2's); Park et al. 2016's 964.4 × 964.2 × 891.8 km shape (Nature 537, 515, via JPL's SBDB), f = 0.0752 |
/// | gravity | GM ÷ R², GM = 62.6284 km³ s⁻² | Park et al. 2016 |
/// | `σ_h` | 2.077 km | Design note 3's fit with V = 0: √(0.9² + (2.05 × (1.62 ÷ g) × 0.16)²) km at saturation, the ice factor `k_comp` the body calibrates |
/// | mean temperature | the equilibrium temperature at the airless Bond albedo 0.11, about 163 K | plan 14's airless rule, a radiative mean as on the Moon; Dawn's dark phyllosilicate surface (Ammannito et al. 2016, Science 353, aaf4279) is taken as rock, where `SurfaceMaterial::of` would call a body of a quarter water ice |
/// | contrasts | 55 K equator–pole, 100 K day–night | placeholders for P14.T24.a's (low confidence) |
/// | spin | obliquity 4°, sidereal day 9.074 17 h; equinox phase not taken | Park et al. 2016 (pole 291.42°, 66.76°, 4.03° from the orbit's; 952.1532° d⁻¹) |
/// | orbit | 2.7675 au, e 0.0758, a^1.5 sidereal years | plan 14's Solar System table; Kepler's third law |
/// | regime | stagnant lid, no constructional volcanism, airless | Design note 3 |
/// | heat flow | 0.001 W m⁻² | a placeholder: a chondritic radiogenic estimate's order (low confidence) |
/// | surface age | 4.0 Gyr | a placeholder (low confidence) |
/// | craters | the lunar chronology at that age, on an ice-rich target (`k_target` 0.12) | Design notes 3 and 12 |
/// | crust | phyllosilicate, its provinces too (V = 0 builds none) | R09.T2's synthetic Ceres |
/// | palette | phyllosilicate (secondary crust), water ice, sodium carbonate as its deposit (the faculae's) | decision-composition §1.7; each figure its row's. The water-ice entry names the ice of its cold traps and fresh craters (Platz et al. 2016, Nature Astronomy 1, 0007; Combe et al. 2016, Science 353, aaf3010), which no area share places until plan 14 states one |
///
/// # Panics
///
/// Never: its figures are valid, as a test builds it.
#[must_use]
pub fn ceres_like() -> CoarseInputs {
    let gravity = MetresPerSecondSquared::new(6.262_84e10 / (469.7e3 * 469.7e3));
    let a_au = 2.7675;
    let year = Seconds::new(math::powf(a_au, 1.5) * SIDEREAL_YEAR_DAYS * SECONDS_PER_DAY);
    let rotation = Seconds::new(9.074_17 * 3_600.0);
    let surface_age = Gigayears::new(4.0);
    let flux = orbit_mean_flux(a_au, 0.0758);
    CoarseInputs::builder()
        .body(BodyRef::new(0, 4))
        .figure(figure(469.7e3, 0.075))
        .gravity(gravity)
        .sigma_h(Metres::new(2_077.0))
        .mean_surface_temperature(airless_rock_temperature(flux))
        .equator_pole_contrast_k(55.0)
        .day_night_contrast_k(100.0)
        .spin(Spin {
            obliquity: degrees(4.0),
            rotation_period: rotation,
            solar_day: Some(solar_day(rotation, year)),
            equinox_true_anomaly: Radians::ZERO,
        })
        .host_flux(flux)
        .seasonal_orbit(SeasonalOrbit {
            eccentricity: 0.0758,
            period: year,
        })
        .surface_pressure(Pascals::ZERO)
        .surface(SurfaceState::Airless, SurfaceMaterial::Rock)
        .tectonics(TectonicRegime::StagnantLid, 0.0)
        .volcanism(0.0)
        .heat_flow(WattsPerSquareMetre::new(0.001))
        .surface_age(surface_age)
        .craters(crater_params(surface_age, gravity, Screening::None, 0.12))
        .climate(ClimateRegime {
            thermal: ThermalRegime::AirlessLike,
            forcing: Forcing::Seasonal,
            condensable: None,
            model: ClimateModelKind::RadiativeEquilibrium,
        })
        .crust(CrustInputs {
            primary: None,
            secondary: PHYLLOSILICATE_KEY,
            tertiary: None,
            provinces: PHYLLOSILICATE_KEY,
            melt_area_fraction: 0.0,
        })
        .palette(palette(vec![
            PHYLLOSILICATE.entry(PaletteRole::SecondaryCrust),
            WATER_ICE.entry(PaletteRole::Ice),
            SODIUM_CARBONATE.entry(PaletteRole::Deposit),
        ]))
        .build()
        .expect("the reference Ceres's inputs are valid")
}

/// The gas of key `key` at mole fraction `x`, called in a `const` item so that a malformed key
/// fails to compile ([`SubstanceKey::new_const`]).
const fn gas(key: &'static str, x: f64) -> GasShare {
    GasShare {
        substance: SubstanceKey::new_const(key),
        mole_fraction: x,
    }
}

/// A condensate of `substance` in `phase`, `reservoir` over the whole surface, covering `area` of
/// it all year.
#[must_use]
const fn condensate(
    substance: SubstanceKey,
    phase: CondensatePhase,
    reservoir: KilogramsPerSquareMetre,
    area: f64,
    persistence: Persistence,
) -> Condensate {
    Condensate {
        substance,
        phase,
        reservoir,
        area_fraction: area,
        persistence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_surface::field::coarse_level;

    #[test]
    fn the_reference_worlds_are_the_bodies_they_name() {
        let earth = earth_like();
        assert_eq!(coarse_level(earth.radius()).get(), 8);
        assert!((earth.gravity().value() - 9.82).abs() < 0.01);
        assert!((earth.ocean_fraction() - 0.7095).abs() < 1e-12);
        // The ocean's 1.38 × 10²¹ kg, about 2.7 × 10⁶ kg over each square metre.
        let ocean = earth.condensates()[0].reservoir.value();
        assert!((2.6e6..2.8e6).contains(&ocean), "{ocean} kg m⁻²");
        assert_eq!(earth.months(), 12);
        assert_eq!(earth.gases()[0].substance.as_str(), "N2");
        // Earth's solar day is 86,400 s to a few tenths of a second.
        let day = earth.spin().solar_day.unwrap().value();
        assert!((day - 86_400.0).abs() < 1.0, "{day} s");

        let mars = mars_like();
        assert_eq!(coarse_level(mars.radius()).get(), 7);
        assert!((mars.gravity().value() - 3.73).abs() < 0.01);
        // Design note 12's projectile scale on Mars from the air's column: 8.5 cm at 636 Pa (the
        // note's 8.2 cm takes about 610 Pa).
        let Screening::Atmosphere {
            column_mass,
            projectile_density,
        } = mars.craters().screening()
        else {
            panic!("Mars's craters are screened by its air");
        };
        let d_star = 1.5 * column_mass.value() / projectile_density.value();
        assert!((0.08..0.09).contains(&d_star), "{d_star} m");
        assert_eq!(mars.condensates()[0].substance, H2O);

        let moon = moon_like();
        assert_eq!(coarse_level(moon.radius()).get(), 6);
        let t = moon.mean_surface_temperature().value();
        assert!((269.0..272.0).contains(&t), "{t} K");
        // The synodic month, 29.53 d.
        let day = moon.spin().solar_day.unwrap().value() / SECONDS_PER_DAY;
        assert!((day - 29.53).abs() < 0.01, "{day} d");
        // The chronology at 4.4 Gyr is far past saturation's 0.018 km⁻² at 1 km.
        assert!(moon.craters().n_1km().value() > 0.5);

        let ceres = ceres_like();
        assert_eq!(coarse_level(ceres.radius()).get(), 5);
        assert!((ceres.gravity().value() - 0.284).abs() < 0.001);
        let t = ceres.mean_surface_temperature().value();
        assert!((160.0..166.0).contains(&t), "{t} K");
    }

    /// Each world's palette holds decision-composition §1.7's substances in their roles, its
    /// deposits among them.
    #[test]
    fn the_reference_palettes_are_decision_composition_s() {
        use PaletteRole::{Deposit, Ice, Liquid, PrimaryCrust, Province, SecondaryCrust};
        let entries = |inputs: &CoarseInputs| -> Vec<(PaletteRole, String)> {
            inputs
                .palette()
                .entries()
                .iter()
                .map(|e| (e.role, e.substance.as_str().to_owned()))
                .collect()
        };
        let expect = |list: &[(PaletteRole, &str)]| -> Vec<(PaletteRole, String)> {
            list.iter().map(|&(r, k)| (r, k.to_owned())).collect()
        };
        assert_eq!(
            entries(&earth_like()),
            expect(&[
                (SecondaryCrust, "basalt"),
                (PaletteRole::TertiaryCrust, "granite"),
                (Ice, "H2O"),
                (Liquid, "H2O"),
            ])
        );
        assert_eq!(
            entries(&mars_like()),
            expect(&[
                (SecondaryCrust, "basalt"),
                (Ice, "CO2"),
                (Ice, "H2O"),
                (Deposit, "mars_dust"),
            ])
        );
        assert_eq!(
            entries(&moon_like()),
            expect(&[(PrimaryCrust, "anorthosite"), (Province, "basalt")])
        );
        assert_eq!(
            entries(&ceres_like()),
            expect(&[
                (SecondaryCrust, "phyllosilicate"),
                (Ice, "H2O"),
                (Deposit, "Na2CO3"),
            ])
        );
        // Each world's sea, where it has one, is its main liquid.
        let earth = earth_like();
        let sea = earth.palette().get(earth.main_liquid().unwrap()).unwrap();
        assert_eq!((sea.role, sea.substance), (Liquid, H2O));
        assert!((sea.density_kg_m3 - 999.84).abs() < 1e-9);
    }

    #[test]
    fn the_fitted_sigma_h_follows_design_note_3() {
        // The Moon's and Ceres's σ_h: the stagnant lid's 0.9 km with the basin share at
        // saturation, 2.05 km × (1.62 ÷ g) × k_comp, and no constructional share.
        let sigma = |g: f64, k_comp: f64| {
            let basin = 2.05 * (1.62 / g) * k_comp;
            (0.9_f64 * 0.9 + basin * basin).sqrt() * 1e3
        };
        let moon = moon_like();
        assert!((sigma(1.62, 1.0) - moon.sigma_h().value()).abs() < 1.0);
        let ceres = ceres_like();
        let fitted = sigma(ceres.gravity().value(), 0.16);
        assert!((fitted - ceres.sigma_h().value()).abs() < 1.0, "{fitted} m");
    }
}
