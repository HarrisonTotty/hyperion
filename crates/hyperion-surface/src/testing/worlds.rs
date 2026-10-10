//! The six synthetic worlds: four bodies' fields built directly from closed forms, a flat field and
//! a single crater (plan R09, T2).
//!
//! Each body's radius, figure and gravity are near the real one's (volumetric mean radii and
//! flattenings from NASA's planetary fact sheets, but Earth's flattening WGS 84's, and Ceres's
//! radius and flattening from Dawn's shape, Ermakov et al. 2017); everything else is
//! illustrative: plates, basins and climates placed by hand where the real ones are, sized to
//! give each field the character of its body (an Earth's two crusts, belts and trenches; a Mars's
//! dichotomy, Tharsis and dry valleys; a Moon's basins and maria; a Ceres's ice-rich craters), not
//! plan 14's values or the coarse pass's. The four bodies carry palettes (decision-composition
//! §1.7): an Earth's basalt, granite and water as ice and as liquid; a Mars's basalt, dust and
//! water and CO₂ ices; a Moon's anorthosite and its maria's basalt; a Ceres's phyllosilicate,
//! sodium carbonate and water ice. Their keys are checked by the grammar alone until plan 14's
//! rows exist (P14.T49.b's test then checks them against the registry), and their values are
//! illustrative ones, each with its basis, standing in for nothing: the server resolves a
//! generated world's from the registry. The flat field and the single crater have an empty
//! palette. The four bodies' structural spectra are their crust classes' anchored laws
//! (`decision-r09-t5.md` item 3): the Earth-like world's a mobile lid's, the others a rocky
//! stagnant lid's (Ceres-like as the reference Ceres's rock); and the Earth-like world's sea floor
//! is closed forms in the place of R09.T11's and T12.a's (item 5).

use core::f64::consts::FRAC_PI_3;

use hyperion_base::math;
use hyperion_base::units::{
    Gigayears, Kelvin, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, MetresPerSecond,
    MetresPerSecondSquared, Pascals, PerSquareKilometre, Radians, consts::SECONDS_PER_JULIAN_YEAR,
};

use super::{
    CellSite, ClimateSample, FieldBuilder, PlateSpec, Routing, SeafloorSample, arc, cross, dot,
    norm, scale, sub, unit,
};
use crate::craters::{CraterParams, CraterParamsParts, Screening};
use crate::cube::unit_dir;
use crate::field::{
    BodyRef, BoundaryKind, ClimateModelKind, CoarseField, Crust, MaterialPalette, MechanicsFamily,
    PaletteEntry, PaletteRole,
};
use crate::num;
use crate::spheroid::Spheroid;
use crate::substance_key::SubstanceKey;
use crate::synth::{
    BandSpectrum, STRUCTURAL_RMS_MOBILE_LID, STRUCTURAL_RMS_STAGNANT_LID, SpectrumShape,
};

/// One of the synthetic worlds [`synthetic_field`] builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SyntheticWorld {
    /// An Earth-sized ocean world of ten plates (four continental), with belts, trenches, arcs
    /// and ridges at their boundaries, a seasonal climate, ice caps and rivers, and three coarse
    /// craters: level 8.
    EarthLike,
    /// A Mars-sized stagnant lid with a northern lowland, Tharsis and its shield, three great
    /// basins and twenty other coarse craters, a dry seasonal climate and drainage from a past
    /// wet epoch: level 7.
    MarsLike,
    /// A Moon-sized airless lid with highland and mare crust, five great basins and forty other
    /// coarse craters: level 6.
    MoonLike,
    /// A Ceres-sized ice-rich lid with three great craters and twenty-five others: level 5.
    CeresLike,
    /// A flat, dry, airless field at the Moon's radius with a one-month year, no relief finer than
    /// its cells and a crater density of zero, so that no crater, coarse or small, is on it: level
    /// 6.
    Flat,
    /// The flat field with one crater of 300 km on the edge between faces 0 and 2, its reach on
    /// both faces, and no other, small craters included: level 6.
    OneCrater,
}

impl SyntheticWorld {
    /// Every synthetic world.
    pub const ALL: &'static [Self] = &[
        Self::EarthLike,
        Self::MarsLike,
        Self::MoonLike,
        Self::CeresLike,
        Self::Flat,
        Self::OneCrater,
    ];
}

/// The field of `world`, the same to the bit every time.
#[must_use]
pub fn synthetic_field(world: SyntheticWorld) -> CoarseField {
    match world {
        SyntheticWorld::EarthLike => earth_like(),
        SyntheticWorld::MarsLike => mars_like(),
        SyntheticWorld::MoonLike => moon_like(),
        SyntheticWorld::CeresLike => ceres_like(),
        SyntheticWorld::Flat => flat(),
        SyntheticWorld::OneCrater => one_crater(),
    }
}

/// The raw system ID of the synthetic worlds' system, which no galaxy places.
const SYSTEM: u64 = 0x5e5e_0000_0000_0000;

/// Volumetric mean radii, metres (NASA's planetary fact sheets; Ceres from Dawn, about 470 km).
const EARTH_RADIUS_M: f64 = 6.371e6;
const MARS_RADIUS_M: f64 = 3.3895e6;
const MOON_RADIUS_M: f64 = 1.7374e6;
const CERES_RADIUS_M: f64 = 4.697e5;

/// The eccentricities of the synthetic worlds' seasonal orbits (Design note 8): the Earth–Moon
/// barycentre's 0.016 711 23 and Mars's 0.093 394 10 at J2000 (Standish and Williams 1992, as
/// JPL Solar System Dynamics' "Approximate Positions of the Planets", Table 1, gives them), the
/// first also the Moon-like world's, since a moon's seasons are its planet's orbit's; and Ceres's
/// 0.079 692 295, JPL's osculating orbit (Small-Body Database, solution 48, epoch JD 2461200.5
/// TDB, read 2026-10-09).
const EARTH_ORBIT_E: f64 = 0.016_711_23;
const MARS_ORBIT_E: f64 = 0.093_394_10;
const CERES_ORBIT_E: f64 = 0.079_692_295;

/// A substance of the synthetic palettes with its illustrative values: what a [`PaletteEntry`]
/// holds but its role.
///
/// The normal albedos are the same in B, V and R, since the synthetic worlds carry no colour
/// (P14.T49.c resolves each band from spectra): the midpoint of R10's Design note 8's visible
/// range for the class (researched there, mostly from memory, and settled in R10.T1.a), but the
/// anorthosite's, the top of its range, the phyllosilicate's, Ceres's measured albedo, and liquid
/// water's, the Fresnel reflectance of its surface at normal incidence in each band, which R11's
/// ocean replaces. Every phase row is 0 until R10 assigns its rows.
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

/// The dry solidus of a basalt at 1 atm, about 1,050 °C, kelvin (Yoder and Tilley 1962, J.
/// Petrol. 3, 342, from memory and not read; low confidence: a natural tholeiite's is 980 °C,
/// Wright and Okamura 1977, USGS Professional Paper 1004): basalt's, and the basaltic dust's and
/// the phyllosilicate's dehydrated residue's.
const BASALT_SOLIDUS_K: f64 = 1_323.15;

/// Basalt: Design note 8's basaltic rock, `A_N` 0.05–0.15; a typical bulk density of 2,900 kg m⁻³
/// (2,700–3,100; Philpotts and Ague 2009, Principles of Igneous and Metamorphic Petrology, p. 22,
/// not read; low confidence).
const BASALT: Substance = Substance {
    key: SubstanceKey::new_const("basalt"),
    normal_albedo_bvr: [0.10; 3],
    density_kg_m3: 2_900.0,
    transition_k: BASALT_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// Granite: Design note 8's felsic rock, `A_N` 0.2–0.35; a bulk density of 2,650 kg m⁻³ (2,650–
/// 2,750) and a dry solidus at 1 atm of about 960 °C (no source read; low confidence).
const GRANITE: Substance = Substance {
    key: SubstanceKey::new_const("granite"),
    normal_albedo_bvr: [0.275; 3],
    density_kg_m3: 2_650.0,
    transition_k: 1_233.15,
    mechanics: MechanicsFamily::Silicate,
};

/// Anorthosite, as the lunar highlands' mature regolith: the top of Design note 8's regolith
/// range, 0.07–0.20, the highlands being its brightest; the highland crust's bulk density, 2,550
/// kg m⁻³ (Wieczorek et al. 2013, Science 339, 671, from GRAIL); and about the solidus of calcic
/// plagioclase (An₉₅) at 1 atm, 1,500 °C (an ideal-solution loop gives 1,515–1,527 °C; Bowen
/// 1913, Am. J. Sci. s4-35, 577, not read; low confidence).
const ANORTHOSITE: Substance = Substance {
    key: SubstanceKey::new_const("anorthosite"),
    normal_albedo_bvr: [0.20; 3],
    density_kg_m3: 2_550.0,
    transition_k: 1_773.15,
    mechanics: MechanicsFamily::Silicate,
};

/// A phyllosilicate-rich rock, as Ceres's crust: Ceres's geometric albedo, 0.094 ± 0.007 at
/// 0.55 µm (Ciarniello et al. 2017, A&A 598, A130), which is its normal albedo (Schröder et al.
/// 2017, Icarus 288, 201, who adopt 0.10 ± 0.01); a grain density of 2,500 kg m⁻³, between
/// saponite's 2,240–2,300 and lizardite's 2,550 (Anthony et al., Handbook of Mineralogy), not
/// Ceres's bulk crust, 1,200–1,400 kg m⁻³ with its ice and salts (Ermakov et al. 2017, JGR
/// Planets 122, 2267); and its dehydrated residue's solidus, as basalt's.
const PHYLLOSILICATE: Substance = Substance {
    key: SubstanceKey::new_const("phyllosilicate"),
    normal_albedo_bvr: [0.094; 3],
    density_kg_m3: 2_500.0,
    transition_k: BASALT_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// Sodium carbonate, the salt of Ceres's faculae: Design note 8's evaporite, `A_N` 0.5–0.8; its
/// density, 2,540 kg m⁻³ (CRC Handbook of Chemistry and Physics, 95th edition), and melting
/// point, 851 °C (ILO-WHO International Chemical Safety Cards; the literature spans 850–856 °C,
/// NIST's WebBook putting the change at 1,123 K and the CRC's 95th edition at 856 °C).
const SODIUM_CARBONATE: Substance = Substance {
    key: SubstanceKey::new_const("Na2CO3"),
    normal_albedo_bvr: [0.65; 3],
    density_kg_m3: 2_540.0,
    transition_k: 1_124.15,
    mechanics: MechanicsFamily::Salt,
};

/// Mars's bright dust: Design note 8's 0.18–0.25 in V; the bulk density of the Viking landers'
/// undisturbed drift material, 1,200 kg m⁻³ (Moore et al. 1987, USGS Professional Paper 1389,
/// p. 126); basaltic, so basalt's solidus.
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
/// 1 atm, 916.72 kg m⁻³ (IAPWS R10-06, Feistel and Wagner 2006, J. Phys. Chem. Ref. Data 35,
/// 1021, Table 6).
const WATER_ICE: Substance = Substance {
    key: SubstanceKey::new_const("H2O"),
    normal_albedo_bvr: [0.965; 3],
    density_kg_m3: 916.72,
    transition_k: WATER_MELTING_K,
    mechanics: MechanicsFamily::WaterIce,
};

/// Liquid water: the Fresnel reflectance at normal incidence, ((n − 1) ÷ (n + 1))², of n = 1.337,
/// 1.333 and 1.331 at 0.45, 0.55 and 0.65 µm, at 25 °C (Hale and Querry 1973, Appl. Opt. 12,
/// 555); its density at 0 °C, 999.84 kg m⁻³ (IAPWS-95, Wagner and Pruß 2002, J. Phys. Chem. Ref.
/// Data 31, 387).
const WATER: Substance = Substance {
    key: SubstanceKey::new_const("H2O"),
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
    key: SubstanceKey::new_const("CO2"),
    normal_albedo_bvr: [0.6; 3],
    density_kg_m3: 1_621.0,
    transition_k: 216.592,
    mechanics: MechanicsFamily::VolatileIce,
};

/// The palette of `entries`, which are in (role, key) order.
#[must_use]
fn palette(entries: Vec<PaletteEntry>) -> MaterialPalette {
    MaterialPalette::new(entries).expect("a synthetic palette is valid")
}

/// The index of `substance` in `role` in `palette`.
#[must_use]
fn index(palette: &MaterialPalette, substance: &Substance, role: PaletteRole) -> u8 {
    palette
        .find(role, substance.key)
        .expect("a synthetic world's palette has its substances")
}

/// Julian years a second, to turn rates in millimetres of water a year (kg m⁻² a⁻¹) into SI.
const PER_YEAR: f64 = 1.0 / SECONDS_PER_JULIAN_YEAR;

/// The unit vector at latitude `lat` and east longitude `lon`, degrees, in the body-fixed frame
/// with +z the north pole and +x the prime meridian.
fn lat_lon(lat: f64, lon: f64) -> [f64; 3] {
    let (sin_lat, cos_lat) = math::sin_cos(lat.to_radians());
    let (sin_lon, cos_lon) = math::sin_cos(lon.to_radians());
    [cos_lat * cos_lon, cos_lat * sin_lon, sin_lat]
}

/// The latitude of the unit vector `dir`, radians.
fn latitude(dir: [f64; 3]) -> f64 {
    math::asin(dir[2].clamp(-1.0, 1.0))
}

/// Point `index` of `count` on a golden-angle spiral, evenly spread over the sphere.
fn spiral(index: u32, count: u32) -> [f64; 3] {
    let height = 1.0 - (2.0 * f64::from(index) + 1.0) / f64::from(count);
    let ring = (1.0 - height * height).sqrt();
    let golden = core::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let (sine, cosine) = math::sin_cos(golden * f64::from(index));
    [ring * cosine, ring * sine, height]
}

/// `n` craters on a spiral, diameters spread geometrically from `smallest` to `largest` metres in
/// a shuffled order, ages spread from `youngest` to `oldest` Gyr, older ones more degraded.
fn spiral_craters(
    mut builder: FieldBuilder,
    n: u32,
    (smallest, largest): (f64, f64),
    (youngest, oldest): (f64, f64),
) -> FieldBuilder {
    let golden_fraction = (5.0_f64.sqrt() - 1.0) / 2.0;
    for k in 0..n {
        let rank = f64::from((k * 7) % n) / f64::from(n - 1);
        let diameter = smallest * math::powf(largest / smallest, rank);
        let spread = (f64::from(k) * golden_fraction).fract();
        let age = youngest + (oldest - youngest) * spread;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "200 × a fraction in [0, 1) rounds to 0 to 200"
        )]
        let degradation = (200.0 * spread).round() as u8;
        builder = builder.crater(
            spiral(k, n),
            Metres::new(diameter),
            Gigayears::new(age),
            degradation,
        );
    }
    builder
}

/// A sum of sinusoids of the projection of `dir` on fixed axes: smooth relief on the sphere.
fn undulation(dir: [f64; 3], terms: &[(f64, f64, f64, f64, f64)]) -> f64 {
    terms
        .iter()
        .map(|&(amplitude, lat, lon, frequency, phase)| {
            amplitude * math::sin(frequency * dot(dir, lat_lon(lat, lon)) + phase)
        })
        .sum()
}

/// exp(−(x ÷ width)²).
fn bell(x: f64, width: f64) -> f64 {
    math::exp(-(x / width) * (x / width))
}

/// The phase of the year at the middle of month `m` (from 0), 2π (m + ½) ÷ 12, radians: the
/// seasonal orbit's eccentric anomaly at the middle of the month's span (Design note 8), in which
/// the synthetic climates are closed forms, 0 at periapsis.
fn season(m: u8) -> f64 {
    core::f64::consts::TAU * (f64::from(m) + 0.5) / 12.0
}

/// Each month's three-cell wind pattern at latitude `lat` (radians), centred on that month's
/// energy-flux equator, which lies `flux_equator_deg(m)` degrees north in month `m`: trades within
/// 30° of it, westerlies to 60°, polar easterlies beyond, each blowing equatorward or poleward as
/// on Earth, at `speeds` metres a second (Design note 8's pattern, illustrative).
fn three_cell_wind(
    lat: f64,
    flux_equator_deg: impl Fn(u8) -> f64,
    speeds: [f64; 3],
) -> [(MetresPerSecond, Radians); 12] {
    let mut winds = [(MetresPerSecond::ZERO, Radians::ZERO); 12];
    for (m, wind) in (0_u8..).zip(&mut winds) {
        let from_equator_deg = lat.to_degrees() - flux_equator_deg(m);
        let north = from_equator_deg >= 0.0;
        let degrees = from_equator_deg.abs();
        let (speed, towards) = if degrees < 30.0 {
            (speeds[0], if north { 225.0 } else { 315.0 })
        } else if degrees < 60.0 {
            (speeds[1], if north { 45.0 } else { 135.0 })
        } else {
            (speeds[2], if north { 225.0 } else { 315.0 })
        };
        *wind = (
            MetresPerSecond::new(speed),
            Radians::new(f64::to_radians(towards)),
        );
    }
    winds
}

/// The Earth-like world's rain band, the energy-flux equator, in month `m`, degrees north: it
/// follows the sun north of the equator in the year's middle.
fn earth_flux_equator_deg(m: u8) -> f64 {
    6.0 - 8.0 * math::cos(season(m))
}

/// The ten plates of the Earth-like world: seed latitude and longitude, crust, Euler pole latitude
/// and longitude, and rotation rate in degrees per million years.
const EARTH_PLATES: [(f64, f64, Crust, f64, f64, f64); 10] = [
    (45.0, 20.0, Crust::Continental, 50.0, -100.0, 0.25),
    (45.0, -100.0, Crust::Continental, -20.0, -80.0, 0.20),
    (-15.0, -60.0, Crust::Continental, -25.0, -130.0, 0.20),
    (5.0, 20.0, Crust::Continental, 50.0, -20.0, 0.25),
    (-25.0, 135.0, Crust::Oceanic, 15.0, 40.0, 0.60),
    (-80.0, 0.0, Crust::Oceanic, 60.0, -120.0, 0.20),
    (-5.0, -150.0, Crust::Oceanic, -60.0, 100.0, 0.90),
    (10.0, 75.0, Crust::Oceanic, 20.0, 30.0, 0.50),
    (0.0, -30.0, Crust::Oceanic, 70.0, 0.0, 0.30),
    (35.0, 160.0, Crust::Oceanic, 40.0, -60.0, 0.60),
];

/// The Earth-like world's annual mean sea-level temperature at latitude `lat`, kelvin.
fn earth_temperature(lat: f64) -> f64 {
    let s = math::sin(lat);
    301.0 - 48.0 * s * s
}

fn earth_elevation(site: &CellSite) -> Metres {
    let base = match site.crust {
        Crust::Continental => 400.0,
        Crust::Oceanic => -4_300.0,
        Crust::Lid | Crust::Province => 0.0,
    };
    let d = site.boundary_distance.map_or(f64::INFINITY, Metres::value);
    let feature = match site.boundary {
        BoundaryKind::Collision => 4_000.0 * bell(d, 200e3),
        BoundaryKind::Subduction if d < 0.0 => -3_000.0 * bell(d, 60e3),
        BoundaryKind::Subduction => 2_000.0 * bell(d - 150e3, 60e3),
        BoundaryKind::Divergent if site.crust == Crust::Oceanic => {
            2_000.0 * math::exp(-d.abs() / 400e3)
        }
        BoundaryKind::Divergent => -1_000.0 * bell(d, 100e3),
        BoundaryKind::Transform | BoundaryKind::Absent => 0.0,
    };
    let relief = undulation(
        site.dir,
        &[
            (600.0, 30.0, 40.0, 3.0, 0.4),
            (350.0, -50.0, 170.0, 7.0, 1.3),
            (200.0, 10.0, -80.0, 13.0, 2.1),
        ],
    );
    Metres::new(base + feature + relief)
}

fn earth_climate(dir: [f64; 3]) -> ClimateSample {
    let lat = latitude(dir);
    let s = math::sin(lat);
    let degrees = lat.to_degrees();
    let mut sample = ClimateSample::still(Kelvin::new(earth_temperature(lat)));
    for m in 0..12_u8 {
        // Coldest in the year's first month in the north, the south opposite.
        sample.month_anomaly_k[usize::from(m)] = -16.0 * s * math::cos(season(m));
        // A rain band that follows the sun north of the equator in the year's middle, storm
        // tracks near 48°, and dry belts between.
        let itcz = earth_flux_equator_deg(m);
        let mm =
            2_200.0 * bell(degrees - itcz, 9.0) + 900.0 * bell(degrees.abs() - 48.0, 12.0) + 30.0;
        sample.month_precipitation_kg_m2_s[usize::from(m)] = mm * PER_YEAR;
    }
    sample.wind = three_cell_wind(lat, earth_flux_equator_deg, [6.0, 8.0, 4.0]);
    sample
}

/// Earth's mean ridge half-rate, mm a year: 23 (46.6 in full, Bird 2003, G³ 4, 1027, Table 3),
/// which oceanic floor with no divergent boundary of its plate takes, at the oldest age
/// (`decision-r09-t5.md` item 5).
const MEAN_HALF_RATE_MM_A: f64 = 23.0;

/// The oldest sea floor, Myr: ages are capped at 200 Myr, beyond which GDH1 is within 40 m of its
/// asymptote (Design note 7).
const OLDEST_FLOOR_MYR: f64 = 200.0;

/// The basement abyssal-hill relief at a ridge of half-rate `u` mm a year, metres: H₀(u) = 55 m +
/// 175 m ÷ (1 + e^((u − 25) ÷ 4)) + 70 m ÷ (1 + e^((u − 8) ÷ 2)), the ruling's logistic fit to the
/// Table 2 of Goff, Smith and Marks 2004 (Oceanography 17(1), 24–37, doi:10.5670/oceanog.2004.64,
/// p. 30; its rates full, halved here): 223 m at a half-rate of 14 mm a year (the table's 235.8 m
/// at a full 28; the ruling quotes 232 m, which this law reaches at 12), 191 m at 20, 62 m at 37.5
/// and 55 m above 55, with an ultraslow term for the 280–320 m of Sloan, Sauter, Goff and Cannat
/// 2012 (G³ 13, doi:10.1029/2011GC003850), 286 m at 5 (`decision-r09-t5.md` item 5). Medium
/// confidence. R09.T12.a's, written here as the Earth-like world's illustrative closed form.
#[must_use]
fn basement_hills_m(u: f64) -> f64 {
    55.0 + 175.0 / (1.0 + math::exp((u - 25.0) / 4.0)) + 70.0 / (1.0 + math::exp((u - 8.0) / 2.0))
}

/// The pelagic drape on floor `age_myr` old at latitude `lat` (radians), metres:
/// `S_p` = √(τ ÷ 1 Myr) × (52 − 2.46 |φ| + 0.045 φ²) m with φ in degrees and clamped to 72°, the
/// eq. 2a of Straume et al. 2019's global sediment grid with the ocean factor 1 (G³ 20, 1756,
/// doi:10.1029/2018GC008115, p. 1766). It is fitted to floor up to 82 Myr old within 72° of the
/// equator, and extended beyond both by the same √τ and the clamped latitude (labelled,
/// `decision-r09-t5.md` item 5), as most of this world's floor needs. R09.T12.a's, written here
/// as the Earth-like world's illustrative closed form.
#[must_use]
fn pelagic_drape_m(age_myr: f64, lat: f64) -> f64 {
    let phi = num::min(lat.to_degrees().abs(), 72.0);
    age_myr.sqrt() * (52.0 - 2.46 * phi + 0.045 * phi * phi)
}

/// The ponded turbidite wedge `margin_km` from a passive margin, metres:
/// `S_t` = 2.4 km e^(−x ÷ 350 km), zero beyond 1,050 km (`decision-r09-t5.md` item 5): the
/// continental rise's mean thickness (Harris et al. 2014, Mar. Geol. 352, 4–24,
/// doi:10.1016/j.margeo.2014.01.011, with Straume et al. 2019's sediment grid), and 350 km the
/// ruling's choice within ordinary margins' 200–500 km, which T12.a's test calibrates. Low
/// confidence. R09.T12.a's, written here as the Earth-like world's illustrative closed form.
#[must_use]
fn ponded_wedge_m(margin_km: f64) -> f64 {
    if margin_km < 1_050.0 {
        2_400.0 * math::exp(-margin_km / 350.0)
    } else {
        0.0
    }
}

/// The Earth-like world's sea floor at `site`, an oceanic cell of one of `plates`
/// (`decision-r09-t5.md` item 5), in illustrative closed forms of R09.T11's and T12.a's: the
/// world's plates are its hand-placed ones, each of one crust, so its ridges and margins are
/// their bisectors.
///
/// The ridge is the nearest bisector with another plate across which the two diverge (the
/// builder's own class, more than 120° from head-on), and its half-rate is their relative
/// motion's normal component at the cell, halved; the age is the distance to it over that rate,
/// capped at [`OLDEST_FLOOR_MYR`], and a plate with no divergent bisector takes the oldest age and
/// [`MEAN_HALF_RATE_MM_A`]. The hills' basement relief H₀ comes from the half-rate and their drape
/// from the age and latitude, H = max(H₀ − `S_p` ÷ 2, H₀ ÷ 2) (Goff 2010, JGR 115, B12104, eq. 5
/// and ¶36). The passive margin is the nearest bisector with a continental plate that does not
/// converge with this one (a trench traps the turbidites in its wedge), and the ponded wedge falls
/// from it. On this world the floor is old, a median near 130 Myr, since its ten plates put few
/// ridges in its oceans, and its plains fewer than Earth's third of the abyss, since its
/// continents are plates of their own with no passive margins inside them.
#[must_use]
fn earth_seafloor(site: &CellSite, plates: &[PlateSpec]) -> SeafloorSample {
    let dir = site.dir;
    let own = usize::from(site.plate);
    let radius_km = EARTH_RADIUS_M / 1e3;
    let mut ridge: Option<(f64, f64)> = None;
    let mut margin: Option<f64> = None;
    for (k, other) in plates.iter().enumerate() {
        if k == own {
            continue;
        }
        let normal = unit(sub(plates[own].seed, other.seed));
        let distance_km = math::asin(dot(dir, normal).clamp(-1.0, 1.0)) * radius_km;
        let towards = unit(scale(sub(normal, scale(dir, dot(normal, dir))), -1.0));
        let relative = cross(
            sub(plates[own].rotation_rad_per_myr, other.rotation_rad_per_myr),
            dir,
        );
        let closing = dot(relative, towards);
        let sliding = norm(sub(relative, scale(towards, closing)));
        let direction = math::atan2(sliding, closing);
        if direction > 2.0 * FRAC_PI_3 && ridge.is_none_or(|(d, _)| distance_km < d) {
            // Radians a million years times kilometres is kilometres a million years, which is
            // millimetres a year.
            ridge = Some((distance_km, -closing * radius_km / 2.0));
        }
        if other.crust == Crust::Continental
            && direction >= FRAC_PI_3
            && margin.is_none_or(|d| distance_km < d)
        {
            margin = Some(distance_km);
        }
    }
    let (age_myr, half_rate) = match ridge {
        Some((d, u)) if u > 0.0 => (num::min(d / u, OLDEST_FLOOR_MYR), u),
        Some(_) | None => (OLDEST_FLOOR_MYR, MEAN_HALF_RATE_MM_A),
    };
    let basement = basement_hills_m(half_rate);
    let drape = pelagic_drape_m(age_myr, latitude(dir));
    SeafloorSample {
        hill_relief: Metres::new(num::max(basement - drape / 2.0, basement / 2.0)),
        ponded_sediment: Metres::new(margin.map_or(0.0, ponded_wedge_m)),
    }
}

fn earth_like() -> CoarseField {
    let plates: Vec<PlateSpec> = EARTH_PLATES
        .iter()
        .map(
            |&(lat, lon, crust, pole_lat, pole_lon, degrees_per_myr)| PlateSpec {
                seed: lat_lon(lat, lon),
                crust,
                rotation_rad_per_myr: scale(
                    lat_lon(pole_lat, pole_lon),
                    degrees_per_myr.to_radians(),
                ),
            },
        )
        .collect();
    let seafloor_plates = plates.clone();
    let radius = Metres::new(EARTH_RADIUS_M);
    let earth = palette(vec![
        BASALT.entry(PaletteRole::SecondaryCrust),
        GRANITE.entry(PaletteRole::TertiaryCrust),
        WATER_ICE.entry(PaletteRole::Ice),
        WATER.entry(PaletteRole::Liquid),
    ]);
    let continents = index(&earth, &GRANITE, PaletteRole::TertiaryCrust);
    let ocean_floor = index(&earth, &BASALT, PaletteRole::SecondaryCrust);
    let sea = index(&earth, &WATER, PaletteRole::Liquid);
    FieldBuilder::new(radius)
        .body(BodyRef::new(SYSTEM, 1))
        .palette(earth)
        .crust_palette([Some(continents), Some(ocean_floor), None, None])
        .main_liquid(sea)
        .figure(
            Spheroid::from_volumetric(EARTH_RADIUS_M, 1.0 / 298.257_223_563)
                .expect("Earth's figure is valid"),
        )
        .sea(Metres::ZERO)
        .lapse_rate(0.0065)
        // A mobile lid at the Earth's gravity: its anchor needs no g⊕ ÷ g.
        .spectrum(BandSpectrum::anchored(
            SpectrumShape::SILICATE,
            radius,
            STRUCTURAL_RMS_MOBILE_LID,
        ))
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(1e-4),
                screening: Screening::Atmosphere {
                    column_mass: KilogramsPerSquareMetre::new(101_325.0 / 9.81),
                    projectile_density: KilogramsPerCubicMetre::new(3_000.0),
                },
                gravity: MetresPerSecondSquared::new(9.81),
                k_target: 1.0,
                impact_velocity: None,
            })
            .expect("the Earth-like crater contract is valid"),
        )
        .climate_model(ClimateModelKind::EnergyBalance)
        .months(12)
        .season_eccentricity(EARTH_ORBIT_E)
        .surface_age(Gigayears::new(2.5))
        .surface_pressure(Pascals::new(101_325.0))
        .plates(plates)
        .seafloor(move |site| earth_seafloor(site, &seafloor_plates))
        .elevation(earth_elevation)
        .ice(|site, elevation| {
            let t = earth_temperature(latitude(site.dir));
            // Sea ice poleward of about 66°, land ice where the ground's annual mean is below 263 K.
            let cold = if elevation.value() < 0.0 {
                t < 261.0
            } else {
                t - 0.0065 * elevation.value() < 263.0
            };
            if cold { 1.0 } else { 0.0 }
        })
        .climate(earth_climate)
        .routing(Routing::SteepestDescent)
        .crater(
            lat_lon(21.4, -89.5),
            Metres::new(180e3),
            Gigayears::new(0.066),
            10,
        )
        .crater(
            lat_lon(-27.0, 27.5),
            Metres::new(160e3),
            Gigayears::new(2.02),
            200,
        )
        .crater(
            lat_lon(46.6, -81.2),
            Metres::new(130e3),
            Gigayears::new(1.85),
            190,
        )
        .build()
}

/// A rocky stagnant lid's structural spectrum on a body of radius `radius_m` metres: the silicate
/// shape anchored at [`STRUCTURAL_RMS_STAGNANT_LID`] (`decision-r09-t5.md` item 3).
#[must_use]
fn stagnant_lid_spectrum(radius_m: f64) -> BandSpectrum {
    BandSpectrum::anchored(
        SpectrumShape::SILICATE,
        Metres::new(radius_m),
        STRUCTURAL_RMS_STAGNANT_LID,
    )
}

/// Mars's Tharsis rise, Olympus Mons and Elysium.
fn tharsis() -> [f64; 3] {
    lat_lon(2.0, -112.0)
}

fn olympus() -> [f64; 3] {
    lat_lon(18.65, -133.8)
}

fn elysium() -> [f64; 3] {
    lat_lon(25.0, 147.0)
}

fn mars_elevation(site: &CellSite) -> Metres {
    let p = site.dir;
    // The dichotomy: a lowland about a pole near 60° N, 180° E.
    let north = dot(p, lat_lon(60.0, 180.0));
    let dichotomy = 1_500.0 - 4_500.0 * 0.5 * (1.0 + math::tanh((north - 0.35) / 0.12));
    let rise = 6_000.0 * bell(arc(p, tharsis()), 0.45) + 18_000.0 * bell(arc(p, olympus()), 0.08);
    let relief = undulation(
        p,
        &[
            (400.0, -20.0, 60.0, 5.0, 0.7),
            (250.0, 40.0, -30.0, 11.0, 1.9),
        ],
    );
    Metres::new(dichotomy + rise + relief)
}

fn mars_climate(dir: [f64; 3]) -> ClimateSample {
    let lat = latitude(dir);
    let s = math::sin(lat);
    let mut sample = ClimateSample::still(Kelvin::new(215.0 - 35.0 * s * s));
    for m in 0..12_u8 {
        sample.month_anomaly_k[usize::from(m)] = -30.0 * s * math::cos(season(m));
    }
    // An illustrative flux equator at the subsolar latitude of the solstices, Mars's obliquity of
    // 25.19° (NASA's Mars fact sheet), in the summer hemisphere: the south at periapsis.
    sample.wind = three_cell_wind(lat, |m| -25.19 * math::cos(season(m)), [4.0, 6.0, 3.0]);
    sample
}

fn mars_like() -> CoarseField {
    let mars = palette(vec![
        BASALT.entry(PaletteRole::SecondaryCrust),
        CARBON_DIOXIDE_ICE.entry(PaletteRole::Ice),
        WATER_ICE.entry(PaletteRole::Ice),
        MARS_DUST.entry(PaletteRole::Deposit),
    ]);
    let crust = index(&mars, &BASALT, PaletteRole::SecondaryCrust);
    let water_ice = index(&mars, &WATER_ICE, PaletteRole::Ice);
    let dry_ice = index(&mars, &CARBON_DIOXIDE_ICE, PaletteRole::Ice);
    let builder = FieldBuilder::new(Metres::new(MARS_RADIUS_M))
        .body(BodyRef::new(SYSTEM, 2))
        .palette(mars)
        .crust_palette([None, None, Some(crust), Some(crust)])
        // The residual caps, illustrative: water ice in the north (Kieffer et al. 1976, Science
        // 194, 1341), and CO₂ ice on the south's surface (Byrne and Ingersoll 2003, Science 299,
        // 1051).
        .ice_entry(move |site| {
            if latitude(site.dir) > 0.0 {
                water_ice
            } else {
                dry_ice
            }
        })
        .figure(Spheroid::from_volumetric(MARS_RADIUS_M, 0.005_89).expect("Mars's figure is valid"))
        .lapse_rate(0.0025)
        .spectrum(stagnant_lid_spectrum(MARS_RADIUS_M))
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(0.005),
                screening: Screening::Atmosphere {
                    column_mass: KilogramsPerSquareMetre::new(610.0 / 3.71),
                    projectile_density: KilogramsPerCubicMetre::new(3_000.0),
                },
                gravity: MetresPerSecondSquared::new(3.71),
                k_target: 1.0,
                impact_velocity: None,
            })
            .expect("the Mars-like crater contract is valid"),
        )
        .climate_model(ClimateModelKind::EnergyBalance)
        .months(12)
        .season_eccentricity(MARS_ORBIT_E)
        .surface_age(Gigayears::new(3.7))
        .surface_pressure(Pascals::new(610.0))
        .crust(|p| {
            if arc(p, tharsis()) < 0.5 || arc(p, elysium()) < 0.2 {
                Crust::Province
            } else {
                Crust::Lid
            }
        })
        .elevation(mars_elevation)
        .ice(|site, _| {
            if latitude(site.dir).abs() > 80_f64.to_radians() {
                1.0
            } else {
                0.0
            }
        })
        .climate(mars_climate)
        .routing(Routing::SteepestDescent)
        .crater(
            lat_lon(-42.4, 70.5),
            Metres::new(2_300e3),
            Gigayears::new(4.0),
            120,
        )
        .crater(
            lat_lon(-49.7, -43.0),
            Metres::new(1_800e3),
            Gigayears::new(3.9),
            110,
        )
        .crater(
            lat_lon(12.9, 87.0),
            Metres::new(1_500e3),
            Gigayears::new(3.9),
            100,
        );
    spiral_craters(builder, 20, (100e3, 500e3), (3.5, 4.0)).build()
}

/// The Moon's great basins and its broadest mare.
fn imbrium() -> [f64; 3] {
    lat_lon(32.8, -15.6)
}

fn serenitatis() -> [f64; 3] {
    lat_lon(27.0, 18.4)
}

fn crisium() -> [f64; 3] {
    lat_lon(17.0, 59.1)
}

fn procellarum() -> [f64; 3] {
    lat_lon(18.4, -57.4)
}

fn moon_like() -> CoarseField {
    let moon = palette(vec![
        ANORTHOSITE.entry(PaletteRole::PrimaryCrust),
        BASALT.entry(PaletteRole::Province),
    ]);
    let highlands = index(&moon, &ANORTHOSITE, PaletteRole::PrimaryCrust);
    let maria = index(&moon, &BASALT, PaletteRole::Province);
    let builder = FieldBuilder::new(Metres::new(MOON_RADIUS_M))
        .body(BodyRef::new(SYSTEM, 3))
        .palette(moon)
        .crust_palette([None, None, Some(highlands), Some(maria)])
        .figure(
            Spheroid::from_volumetric(MOON_RADIUS_M, 0.0012).expect("the Moon's figure is valid"),
        )
        .spectrum(stagnant_lid_spectrum(MOON_RADIUS_M))
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(0.05),
                screening: Screening::None,
                gravity: MetresPerSecondSquared::new(1.62),
                k_target: 1.0,
                impact_velocity: None,
            })
            .expect("the Moon-like crater contract is valid"),
        )
        .climate_model(ClimateModelKind::RadiativeEquilibrium)
        .months(12)
        .season_eccentricity(EARTH_ORBIT_E)
        .surface_age(Gigayears::new(4.4))
        .crust(|p| {
            let mare = arc(p, imbrium()) < 0.25
                || arc(p, serenitatis()) < 0.15
                || arc(p, crisium()) < 0.12
                || arc(p, procellarum()) < 0.35;
            if mare { Crust::Province } else { Crust::Lid }
        })
        .elevation(|site| {
            let highlands = 1_800.0 * dot(site.dir, lat_lon(0.0, 180.0));
            let relief = undulation(
                site.dir,
                &[
                    (300.0, 15.0, -40.0, 6.0, 0.2),
                    (200.0, -35.0, 100.0, 13.0, 1.1),
                ],
            );
            Metres::new(highlands + relief)
        })
        .climate(|dir| {
            let lat = latitude(dir);
            let mut sample = ClimateSample::still(Kelvin::new(100.0 + 150.0 * math::cos(lat)));
            for m in 0..12_u8 {
                sample.month_anomaly_k[usize::from(m)] =
                    -0.5 * math::sin(lat) * math::cos(season(m));
            }
            sample
        })
        .crater(
            lat_lon(-53.0, -169.0),
            Metres::new(2_500e3),
            Gigayears::new(4.3),
            180,
        )
        .crater(imbrium(), Metres::new(1_145e3), Gigayears::new(3.9), 60)
        .crater(serenitatis(), Metres::new(740e3), Gigayears::new(3.9), 90)
        .crater(crisium(), Metres::new(1_060e3), Gigayears::new(3.9), 80)
        .crater(
            lat_lon(-19.4, -92.8),
            Metres::new(930e3),
            Gigayears::new(3.8),
            40,
        );
    spiral_craters(builder, 40, (100e3, 400e3), (3.0, 4.3)).build()
}

fn ceres_like() -> CoarseField {
    let ceres = palette(vec![
        PHYLLOSILICATE.entry(PaletteRole::SecondaryCrust),
        WATER_ICE.entry(PaletteRole::Ice),
        SODIUM_CARBONATE.entry(PaletteRole::Deposit),
    ]);
    let crust = index(&ceres, &PHYLLOSILICATE, PaletteRole::SecondaryCrust);
    let builder = FieldBuilder::new(Metres::new(CERES_RADIUS_M))
        .body(BodyRef::new(SYSTEM, 4))
        .palette(ceres)
        .crust_palette([None, None, Some(crust), None])
        .figure(Spheroid::from_volumetric(CERES_RADIUS_M, 0.075).expect("Ceres's figure is valid"))
        .spectrum(stagnant_lid_spectrum(CERES_RADIUS_M))
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(0.003),
                screening: Screening::None,
                gravity: MetresPerSecondSquared::new(0.28),
                k_target: 0.12,
                impact_velocity: None,
            })
            .expect("the Ceres-like crater contract is valid"),
        )
        .climate_model(ClimateModelKind::RadiativeEquilibrium)
        .months(12)
        .season_eccentricity(CERES_ORBIT_E)
        .surface_age(Gigayears::new(3.0))
        .elevation(|site| {
            Metres::new(undulation(
                site.dir,
                &[
                    (2_000.0, 20.0, 10.0, 4.0, 0.3),
                    (1_200.0, -60.0, -120.0, 9.0, 1.7),
                ],
            ))
        })
        .climate(|dir| {
            let lat = latitude(dir);
            let mut sample = ClimateSample::still(Kelvin::new(110.0 + 55.0 * math::cos(lat)));
            for m in 0..12_u8 {
                sample.month_anomaly_k[usize::from(m)] =
                    -3.0 * math::sin(lat) * math::cos(season(m));
            }
            sample
        })
        .crater(
            lat_lon(-10.8, 123.9),
            Metres::new(280e3),
            Gigayears::new(1.0),
            40,
        )
        .crater(
            lat_lon(-42.6, -67.5),
            Metres::new(260e3),
            Gigayears::new(1.5),
            60,
        )
        .crater(
            lat_lon(-46.3, -110.8),
            Metres::new(170e3),
            Gigayears::new(0.5),
            20,
        );
    spiral_craters(builder, 25, (55e3, 150e3), (1.0, 4.0)).build()
}

fn flat_builder(body_index: u16) -> FieldBuilder {
    FieldBuilder::new(Metres::new(MOON_RADIUS_M)).body(BodyRef::new(SYSTEM, body_index))
}

fn flat() -> CoarseField {
    flat_builder(5).build()
}

fn one_crater() -> CoarseField {
    // On the edge between faces 0 and 2 (x = z), which face 0 owns by the lowest-index rule.
    flat_builder(6)
        .crater(
            unit_dir([1.0, 0.3, 1.0]),
            Metres::new(300e3),
            Gigayears::new(3.9),
            0,
        )
        .build()
}
