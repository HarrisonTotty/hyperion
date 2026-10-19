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
//! The substances are plan 14's registry keys ([`SubstanceKey`], decision-composition §1.1), and
//! a test holds every key these worlds and R09's synthetic ones use to a registry row, and each
//! reference row's figures to its row's condensed columns (P14.T49.b). Each world's palette
//! (decision-composition §1.7) carries the values the server will resolve from those rows
//! (P14.T49.c's resolution, not built): each substance's figures are the ones its row below
//! states with their sources. They began as R09.T2's synthetic palettes'
//! (`hyperion_surface::testing`) and are kept apart from them, so that a correction to either set
//! moves only its own output (the wire goldens pin the synthetic fields' palettes, and R09.T16's
//! coarse goldens will pin these). A science review of 2026-10-10 read their basalt solidus,
//! 980 °C, which the synthetic rows have taken since, and labelled what it could not read; the
//! ruling science-r09-drainage.md made every density a grain density (§3), carried the Moon's
//! cold-trapped water ice (§4) and Ceres's crust as primary (§5), which these rows and worlds
//! follow (R09.T10.c). The ruling science-composition-crusts.md (2026-10-11) verified or
//! corrected the figures that review could not read (granite's solidus is still unverified): the
//! transitions of the plagioclase rock, the phyllosilicate's residue, the dust and `Na2CO3` (§4)
//! and water ice's density, now a function of temperature (§3), which these rows take. It added
//! the highland crust's own row, `feldspathic` (§7), and an iron-rich basalt's, `basalt_fe` (§2),
//! which the Moon's highlands and its maria and Mars's crust take, and the crust's `lid`, which
//! crust each world's surface shows (§5; R09.T10.d); the anorthosite rock and its chondrite rows
//! are P14.T49.b's. Its §10 and §10.1 checked and ruled on these rows' citations, which the
//! synthetic rows share.
//!
//! The rows are the substances', not the worlds': a row serves every body whose composition names
//! its substance (decision-composition §1.7), so each figure is the substance's own (a grain
//! density, a solidus), or, where it is a body's measurement standing for the substance's (an
//! albedo), says so.

use hyperion_surface::craters::{
    self, CraterParams, CraterParamsParts, CraterTarget, Screening, TargetClass,
};
use hyperion_surface::field::{
    BodyRef, ClimateModelKind, LiquidBasis, LiquidProperties, MaterialPalette, MechanicsFamily,
    PaletteEntry, PaletteRole,
};
use hyperion_surface::spheroid::Spheroid;
use hyperion_surface::substance_key::SubstanceKey;

use super::inputs::{
    AreaShare, ClimateRegime, CoarseInputs, Condensate, CondensatePhase, CrustInputs, Forcing,
    GasShare, LidCrust, Persistence, SeasonalOrbit, Spin, TectonicRegime, ThermalRegime, WetEpoch,
};
use crate::math;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::derive::irradiation;
use crate::planetary::hooks::SurfaceSeed;
use crate::substance;
use crate::units::consts::{SECONDS_PER_DAY, SOLAR_CONSTANT_W_PER_M2};
use crate::units::{
    EarthFluxes, Gigayears, Kelvin, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres,
    MetresPerSecond, MetresPerSecondSquared, Pascals, Radians, Seconds, WattsPerSquareMetre, Years,
};

/// The surface seed the reference worlds' tests run under.
pub const SEED: SurfaceSeed = SurfaceSeed::new(0x5eed_0009_0010);

/// Water.
const H2O: SubstanceKey = SubstanceKey::new_const("H2O");
/// Carbon dioxide.
const CO2: SubstanceKey = SubstanceKey::new_const("CO2");
/// Basalt, the secondary crust's default (P14.T51.c).
const BASALT_KEY: SubstanceKey = SubstanceKey::new_const("basalt");
/// An iron-rich basalt, the secondary crust from an iron-rich source (P14.T51.c).
const BASALT_FE_KEY: SubstanceKey = SubstanceKey::new_const("basalt_fe");
/// Granite, a tertiary crust (P14.T51.c).
const GRANITE_KEY: SubstanceKey = SubstanceKey::new_const("granite");
/// A feldspathic crust, a flotation crust as impacts have mixed it (P14.T51.c).
const FELDSPATHIC_KEY: SubstanceKey = SubstanceKey::new_const("feldspathic");
/// A phyllosilicate-rich rock (P14.T49.b's lithologies).
const PHYLLOSILICATE_KEY: SubstanceKey = SubstanceKey::new_const("phyllosilicate");

/// A substance of the reference palettes with its figures: what a [`PaletteEntry`] holds but its
/// role, as the server would resolve it from the substance's registry row (P14.T49.b–c).
///
/// The normal albedos are the same in B, V and R: a band's value is P14.T49.c's to resolve from
/// spectra, so each row takes one visible figure, the midpoint of R10's Design note 8's range for
/// its class (researched there, mostly from memory, and to be settled in R10.T1.a), but the
/// feldspathic crust's, the top of its range, the phyllosilicate's, Ceres's measured albedo, and
/// liquid water's, the Fresnel reflectance of its surface at normal incidence in each band. Every
/// phase row is 0 until R10 assigns its rows. Every density is a grain density, the porosity being
/// how a material lies, which R10's forms carry (science-r09-drainage.md §3): a rock's or a
/// mineral's solid over its own volume, and an ice's crystal density or a liquid's density at the
/// temperature its row states.
struct Substance {
    key: SubstanceKey,
    normal_albedo_bvr: [f64; 3],
    grain_density_kg_m3: f64,
    transition_k: f64,
    mechanics: MechanicsFamily,
}

impl Substance {
    /// The entry of the substance in `role`, with no liquid record.
    #[must_use]
    const fn entry(&self, role: PaletteRole) -> PaletteEntry {
        PaletteEntry {
            substance: self.key,
            role,
            normal_albedo_bvr: self.normal_albedo_bvr,
            phase_row: 0,
            grain_density_kg_m3: self.grain_density_kg_m3,
            transition: Kelvin::new(self.transition_k),
            mechanics: self.mechanics,
            liquid: None,
        }
    }

    /// The substance's [`PaletteRole::Liquid`] entry with its record `liquid`, whose density is
    /// the substance's grain density.
    #[must_use]
    fn liquid_entry(&self, liquid: LiquidProperties) -> PaletteEntry {
        PaletteEntry {
            liquid: Some(liquid),
            ..self.entry(PaletteRole::Liquid)
        }
    }

    /// The substance's [`PaletteRole::Ice`] entry with ice Ih's density at `temperature`, where the
    /// ice lies on its body ([`substance::ice_ih_density`]): ice Ih's, so it serves [`WATER_ICE`]
    /// alone (science-composition-crusts.md §3).
    #[must_use]
    fn ice_entry_at(&self, temperature: Kelvin) -> PaletteEntry {
        PaletteEntry {
            grain_density_kg_m3: substance::ice_ih_density(temperature).value(),
            ..self.entry(PaletteRole::Ice)
        }
    }
}

/// The solidus of a basalt at about 1 atm, 980 °C, kelvin: a natural Hawaiian tholeiite's, below
/// which its residual glass, 4 per cent, stops falling (Wright and Okamura 1977, USGS Professional
/// Paper 1004, abstract and Table 15, after Wright and Weiblen 1967; ±10 °C), as Alae lava lake's
/// (Peck 1978, USGS Professional Paper 935-B, p. B1; Wright and Peck 1978, Professional Paper
/// 935-C, p. C1; science-r09-drainage.md §2). Short dry quench runs at 1 atm, against the lava
/// lakes' slow natural cooling, leave two Kilauea tholeiites all crystalline at about 1,065–
/// 1,090 °C and a Mauna Kea alkali basalt at 1,040 °C (Yoder and Tilley 1957, Carnegie
/// Institution of Washington Year Book 56, pp. 157–158, which call them "approximate"; their 1962
/// monograph, J. Petrol. 3, 342, not read). A terrestrial tholeiite's.
const BASALT_SOLIDUS_K: f64 = 1_253.15;

/// The solidus of an anhydrous, iron-rich basalt at about 1 atm, 1,050 °C, kelvin: low-pressure
/// experiments on the lunar mare basalts imply that "if solid mare basalts were heated they would
/// begin to melt around 1050° C", about 100 °C above terrestrial basalts, "chiefly because of the
/// lack of H2O" (Taylor et al. 1991, Lunar Sourcebook, ch. 6, p. 206; science-composition-crusts.md
/// §2). Mars's degassed basalts and dust take it too, an assumption: no measurement of theirs was
/// read.
const BASALT_FE_SOLIDUS_K: f64 = 1_323.15;

/// The solidus of a plagioclase rock with a few per cent of low-calcium pyroxene at 1 atm,
/// 1,260 °C, kelvin: the alteration point anorthite + forsterite + clinoenstatite + liquid of the
/// system anorthite–forsterite–silica, "1260° ± 2°" (Andersen 1915, Am. J. Sci. s4-39, 407,
/// p. 439), where a mixture on the anorthite–enstatite join first melts; a quartz-normative one
/// begins at the ternary eutectic, 1,222 °C (p. 440). Its plagioclase alone, An₉₅, is on Bowen's
/// loop near 1,524 °C (Bowen 1913, Am. J. Sci. s4-35, 577, Table IV), where the rock becomes mostly
/// melt; iron and sodium lower both, by amounts not read (science-composition-crusts.md §4).
const PLAGIOCLASE_ROCK_SOLIDUS_K: f64 = 1_533.15;

/// The solidus of an anhydrous chondritic silicate assemblage at about 1 atm, 1,150 °C, kelvin, an
/// upper bound: the H and LL chondrites' "~1150-1160°C" at IW−1 (Jurewicz et al., LPSC 24, 1993,
/// p. 739), whose charges lost their volatiles, sodium among them (Jurewicz et al., LPSC 25, 1994,
/// p. 651), so a natural, sodium-bearing rock likely begins somewhat lower (an inference: neither
/// abstract measures a solidus). Anhydrous Murchison (CM) and Allende (CV) had more melt at
/// 1,170–1,180 °C, so theirs lie at or below it (Jurewicz, Mittlefehldt and Jones 1993, GCA 57,
/// 2123, as LPSC 24 reports it). The dehydrated residue of a phyllosilicate rock's
/// (science-composition-crusts.md §4 and §10.1; corrected 2026-10-11).
const CHONDRITIC_SOLIDUS_K: f64 = 1_423.15;

/// Basalt: Design note 8's basaltic rock, `A_N` 0.05–0.15; a grain density of 3,010 kg m⁻³, the
/// mean of eight Alae lava lake tholeiites, 2,920–3,040 (Peck 1978, USGS Professional Paper
/// 935-B, p. B20; science-r09-drainage.md §3): a terrestrial tholeiite's; an iron-rich basalt is
/// [`BASALT_FE`].
const BASALT: Substance = Substance {
    key: BASALT_KEY,
    normal_albedo_bvr: [0.10; 3],
    grain_density_kg_m3: 3_010.0,
    transition_k: BASALT_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// An iron-rich basalt, `FeO` about 18–22 wt% against a terrestrial tholeiite's 10–12, as the lunar
/// maria's and Mars's (P14.T51.c's `basalt_fe`): the representative mare basalts' 16.0–22.4 wt%,
/// the high- and low-titanium ones' 18.7–22.1 (Taylor et al. 1991, Lunar Sourcebook, ch. 6,
/// Table A6.1, p. 261), and the shergottites', Shergotty's 18.7–19.8 and Zagami's 17.3–19 (Meyer
/// 2012, Mars Meteorite Compendium, NASA JSC, each one's Table 1a), against the Alae tholeiite's
/// 11.0 as total `FeO`, 9.80 `FeO` with 1.30 `Fe₂O₃` (Peck 1978, USGS Professional Paper 935-B,
/// p. B3; a mid-ocean ridge basalt's about 10, not read; science-composition-crusts.md §2.1, §10
/// and §10.1). Design note 8's basaltic rock, `A_N` 0.05–0.15; a grain density of 3,310 kg m⁻³,
/// the shergottites' mean (Macke, Britt and Consolmagno 2011, MAPS 46, 311, abstract), inside the
/// lunar basalts' 3,270 (high-alumina, NWA 4898) to 3,460 kg m⁻³ (high-titanium, 70215), which
/// rises with `TiO₂` (Kiefer et al. 2012, GRL 39, L07201, abstract and Table 1; a high-titanium
/// row waits for a second province lithology, science-composition-crusts.md §2); and
/// [`BASALT_FE_SOLIDUS_K`].
const BASALT_FE: Substance = Substance {
    key: BASALT_FE_KEY,
    normal_albedo_bvr: [0.10; 3],
    grain_density_kg_m3: 3_310.0,
    transition_k: BASALT_FE_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// Granite: Design note 8's felsic rock, `A_N` 0.2–0.35; a grain density of 2,650 kg m⁻³ (2,654
/// kg m⁻³ from the measured mode of the USGS standard granite G-1, Chayes in Fairbairn et al.
/// 1951, USGS Bulletin 980, Table 26, with the Handbook of Mineralogy's mineral densities,
/// 2,635–2,671 across their ranges; science-composition-crusts.md §4) and a dry solidus at 1 atm
/// of about 960 °C (from memory; the dry haplogranite solidus's primary source is Huang and
/// Wyllie 1975, J. Geol. 83, 737, not read; low confidence).
const GRANITE: Substance = Substance {
    key: GRANITE_KEY,
    normal_albedo_bvr: [0.275; 3],
    grain_density_kg_m3: 2_650.0,
    transition_k: 1_233.15,
    mechanics: MechanicsFamily::Silicate,
};

/// A feldspathic crust, as the lunar highlands': a primary flotation crust as basin-scale impacts
/// mixed it, about 75–80 per cent plagioclase by mass (about 80 by volume) with 4–6 wt% `FeO` in
/// bulk, held in its mafic minerals: the Apollo 16 soils' 76 per cent plagioclase from their
/// aluminium and the feldspathic lunar meteorites' 64–86, mean 77 (Peplowski, Beck and Lawrence
/// 2016, JGR Planets 121, 388, eqs. 3 and 4, Tables 1 and 3), the Apollo 16 soils' 4.7–5.4 wt%
/// `FeO` (McKay et al. 1991, Lunar Sourcebook, ch. 7, Table 7.15, p. 346) and the highlands' about
/// 5 from orbit (Lawrence et al. 2002, JGR 107, E12, abstract); more mafic than an anorthosite
/// (the name is Hu, Ehlmann and Seager 2012's surface type, theirs 97 per cent plagioclase, ApJ
/// 752, 7, Table 2; science-composition-crusts.md §7, §10 and §10.1). The top of Design note 8's
/// regolith range, 0.07–0.20, the highlands being its brightest (a body's measurement standing for
/// the substance's); its grain density, 2,900 kg m⁻³, implied by the highlands' GRAIL bulk
/// density, 2,550 kg m⁻³, at 12 per cent porosity (Wieczorek et al. 2013, Science 339, 671, who
/// note "2800 to 2900 kg m⁻³ … used typically for anorthositic crustal materials"), inside the
/// four feldspathic highland rocks' 2,840–2,910 by helium pycnometry (Kiefer et al. 2012, GRL 39,
/// L07201, Table 1), and what 75–78 per cent An₉₅ plagioclase gives with low-calcium pyroxene and
/// olivine of Mg# 66–72, 2,880–2,910 (science-composition-crusts.md §10.1), against a pure
/// anorthosite's about 2,790 (P14.T49.b's row); and [`PLAGIOCLASE_ROCK_SOLIDUS_K`].
const FELDSPATHIC: Substance = Substance {
    key: FELDSPATHIC_KEY,
    normal_albedo_bvr: [0.20; 3],
    grain_density_kg_m3: 2_900.0,
    transition_k: PLAGIOCLASE_ROCK_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// A phyllosilicate-rich rock, as Ceres's crust, a primary one, its frozen ocean's shell
/// (science-r09-drainage.md §5): Ceres's geometric albedo, 0.094 ± 0.007 at 0.55 µm (Ciarniello
/// et al. 2017, A&A 598, A130), which is its normal albedo (Schröder et al. 2017, Icarus 288, 201,
/// §4, who adopt Tedesco 1989's 0.10 ± 0.01; a body's measurement standing for the substance's); a
/// grain density of 2,500 kg m⁻³, between saponite's 2,240–2,300 and lizardite's 2,550 (Anthony
/// et al., Handbook of Mineralogy), not Ceres's crust's bulk, 1,287 (+70/−87) kg m⁻³ with its ice
/// and salts (Ermakov et al. 2017, JGR Planets 122, 2267, §5.4), which is the body's mixture, not
/// a substance's; and, for its dehydrated residue's solidus, [`CHONDRITIC_SOLIDUS_K`].
const PHYLLOSILICATE: Substance = Substance {
    key: PHYLLOSILICATE_KEY,
    normal_albedo_bvr: [0.094; 3],
    grain_density_kg_m3: 2_500.0,
    transition_k: CHONDRITIC_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// Sodium carbonate, the salt of Ceres's faculae (De Sanctis et al. 2016, Nature 536, 54):
/// Design note 8's evaporite, `A_N` 0.5–0.8, which Cerealia Facula's visual normal albedo,
/// 0.6 ± 0.1, bears out (Schröder et al. 2017, Icarus 288, 201); its grain density, 2,540 kg m⁻³:
/// the X-ray density of γ-Na₂CO₃ at 295 K, 2,537, from Dusek et al. 2003's cell (Acta Cryst. B59,
/// 337), and natrite's, 2,547 (Zubkova et al. 2002; the Handbook of Mineralogy's D(meas.) 2.54),
/// 2,575 at 110 K by Dusek et al.'s colder cell, a 1.5 per cent change; and its melting point,
/// 850 °C, where NIST-JANAF's solid ends and its liquid begins (Chase 1998, via the NIST WebBook;
/// safety cards give 851 °C; science-composition-crusts.md §4).
const SODIUM_CARBONATE: Substance = Substance {
    key: SubstanceKey::new_const("Na2CO3"),
    normal_albedo_bvr: [0.65; 3],
    grain_density_kg_m3: 2_540.0,
    transition_k: 1_123.15,
    mechanics: MechanicsFamily::Salt,
};

/// Mars's bright dust: Design note 8's 0.18–0.25 in V; a grain density of 2,600 kg m⁻³, which
/// Viking Lander 1's fine material's 58 per cent porosity assumes (Moore et al. 1987, USGS
/// Professional Paper 1389, p. 121 and Table 36; an assumption there, labelled so), the drift's
/// bulk, 1,200 kg m⁻³ (p. 126 and Table 37), being a form's (science-r09-drainage.md §3);
/// iron-rich and basaltic, so [`BASALT_FE_SOLIDUS_K`] (an assumption: no measurement of the dust
/// was found; its chlorides melt first and are minor; science-composition-crusts.md §4).
const MARS_DUST: Substance = Substance {
    key: SubstanceKey::new_const("mars_dust"),
    normal_albedo_bvr: [0.215; 3],
    grain_density_kg_m3: 2_600.0,
    transition_k: BASALT_FE_SOLIDUS_K,
    mechanics: MechanicsFamily::Silicate,
};

/// The melting point of water ice at 1 atm, kelvin: 273.152 519 K (IAPWS R10-06), to 0.01 K.
const WATER_MELTING_K: f64 = 273.15;

/// The temperature below which water ice is held in a cold trap, kelvin: Hertz–Knudsen sublimation
/// of 144 kg m⁻² Gyr⁻¹ (Schorghofer and Williams 2020, PSJ 1, 54, Table 1), P14.T51.f's threshold;
/// a cold-trapped ice's density is taken there, an upper bound on its temperature
/// (science-composition-crusts.md §3).
const COLD_TRAP_H2O_K: f64 = 110.0;

/// The coldest zone's temperature, T₀ − (2/3) × the equator–pole contrast: the pole of the zonal
/// fit T₀ + T₂ P₂(sin φ) whose contrast the tables define as −(3/2) T₂. It assumes a cold pole,
/// a positive contrast, as every reference world has; a warm pole's coldest zone would be its
/// equator, T₀ + (1/3) × the contrast.
#[must_use]
fn coldest_zone(mean: Kelvin, equator_pole_contrast_k: f64) -> Kelvin {
    Kelvin::new(mean.value() - 2.0 / 3.0 * equator_pole_contrast_k)
}

/// Water ice, as snow-covered ice: Design note 8's fresh snow, 0.95–0.98 in the visible
/// (Wiscombe and Warren 1980; Grenfell et al. 1994); ice Ih's density, a function of temperature
/// ([`substance::ice_ih_density`]; 916.72 kg m⁻³ at its melting point, the row's figure, IAPWS
/// R10-06(2009), Table 6: 916.721 463 kg m⁻³), each world's entry taking it where its ice lies
/// ([`Substance::ice_entry_at`]; science-composition-crusts.md §3).
const WATER_ICE: Substance = Substance {
    key: H2O,
    normal_albedo_bvr: [0.965; 3],
    grain_density_kg_m3: 916.72,
    transition_k: WATER_MELTING_K,
    mechanics: MechanicsFamily::WaterIce,
};

/// Liquid water at 25 °C, its record's temperature ([`WATER_RECORD`]): the Fresnel reflectance at
/// normal incidence, ((n − 1) ÷ (n + 1))², from its record's n at the bands' 0.44, 0.55 and
/// 0.64 µm, 0.021 092, 0.020 500 and 0.020 210, to four places, a specular surface's, never a
/// Lambertian albedo (decision-r11-palette-liquids §2, item 7); its record's density, 997.047 kg
/// m⁻³ at 25 °C and 100 kPa (IAPWS-95, Wagner and Pruß 2002, J. Phys. Chem. Ref. Data 31, 387).
const WATER: Substance = Substance {
    key: H2O,
    normal_albedo_bvr: [0.021_1, 0.020_5, 0.020_2],
    grain_density_kg_m3: WATER_RECORD.density_kg_m3,
    transition_k: WATER_MELTING_K,
    mechanics: MechanicsFamily::WaterIce,
};

/// Liquid water's record at 25 °C and 100 kPa, every member measured, so
/// `Measured`: the synthetic worlds' water (`hyperion_surface::testing::liquid_water`), whose
/// figures and sources these are, kept apart as the rows are (decision-r11-palette-liquids §1.8's
/// water row and §4).
///
/// - **n and a:** the uniform means over R08's fifteen bake bins of R08's `water.json` since
///   R08.T5.h (decision-r08-t5g.md §1 and §2), its 5 nm grid interpolated linearly in n and in
///   a = 4πk ÷ λ, n to six decimal places and a to six significant figures: n IAPWS R9-97's
///   (1997; the refit of Schiebener et al. 1990, J. Phys. Chem. Ref. Data 19, 677, that Harvey,
///   Gallagher and Levelt Sengers 1998, J. Phys. Chem. Ref. Data 27, 761, publish) at 25 °C, and
///   a Mason, Cone and Fry 2016's (Appl. Opt. 55, 7163) at 23 °C over 380–495 nm, Pope and Fry
///   1997's (Appl. Opt. 36, 8710) at 22 °C over 500–725 nm and Kou, Labrie and Chýlek 1993's
///   (Appl. Opt. 32, 3531) at 22 °C over 730–780 nm. The client's
///   `view/atmosphere/materials/paletteLiquids.test.ts` holds these bins to the file's means, so
///   that a change to the file fails it; taking the change here, with [`WATER`]'s normal albedo,
///   re-blesses at the version in force.
/// - **Density:** IAPWS-95's 997.047 kg m⁻³ at 298.15 K and 100 kPa (IAPWS R6-95(2018)).
/// - **Surface tension:** IAPWS R1-76(2014)'s 0.071 97 N m⁻¹ at 298.15 K.
/// - **Viscosity:** IAPWS R12-08's 8.900 × 10⁻⁴ Pa s at 298.15 K and 100 kPa (Huber et al. 2009,
///   J. Phys. Chem. Ref. Data 38, 101).
/// - **Scattering:** P14.T49.h's, `hyperion_sim::substance::liquid_properties`' b at 298.15 K, the
///   registry's Einstein–Smoluchowski–Cabannes (Zhang, Hu and He 2009's density term with
///   IAPWS-95's `κ_T` at the saturated liquid's density and δ = 0.039 there), copied as shortest
///   round-trip decimals (decision-r10-t10b-r09-t2e §7): 0.005 47 m⁻¹ in the bin about 393 nm and
///   0.000 370 in the bin about 747 nm. A sim test holds it to the registry's to the bit.
///
/// The reference Earth's mean surface temperature is 288 K, so the generated world's record would
/// be resolved colder (§1.7); this one keeps its file's temperature, its index's, at which every
/// member but the absorption (at 23 °C to 495 nm and 22 °C from 500 nm) was measured.
const WATER_RECORD: LiquidProperties = LiquidProperties {
    temperature: Kelvin::new(298.15),
    density_kg_m3: 997.047,
    basis: LiquidBasis::Measured,
    refractive_index: [
        1.343_789, 1.341_476, 1.339_564, 1.337_962, 1.336_599, 1.335_422, 1.334_396, 1.333_487,
        1.332_676, 1.331_946, 1.331_283, 1.330_673, 1.330_109, 1.329_581, 1.329_086,
    ],
    absorption_per_m: Some([
        0.001_875_00,
        0.003_054_05,
        0.006_244_14,
        0.010_301_2,
        0.017_715_8,
        0.039_037_0,
        0.051_761_7,
        0.073_031_0,
        0.180_294,
        0.277_590,
        0.334_405,
        0.442_138,
        0.601_402,
        1.442_47,
        2.800_88,
    ]),
    scattering_per_m: Some([
        0.005_469_387_272_826_296,
        0.004_181_643_650_890_754,
        0.003_254_641_324_180_969_4,
        0.002_572_758_840_890_256_4,
        0.002_061_546_784_888_825,
        0.001_671_864_057_626_776,
        0.001_370_471_055_788_929_3,
        0.001_134_243_878_283_069,
        0.000_946_934_327_145_038_1,
        0.000_796_821_586_256_235_6,
        0.000_675_356_514_061_437_1,
        0.000_576_186_174_358_237_9,
        0.000_494_570_413_029_009_8,
        0.000_426_895_418_578_785_24,
        0.000_370_397_943_504_135_35,
    ]),
    surface_tension_n_per_m: Some(0.071_97),
    viscosity_pa_s: Some(8.900e-4),
};

/// CO₂ ice: Design note 8's carbon dioxide frost, 0.4–0.8; its density at 150 K, a Mars-like
/// frost's temperature, 1,621 kg m⁻³ (Mangan et al. 2017, Icarus 294, 201, whose fit over 80–195
/// K is ρ = 1.723 91 − 2.53 × 10⁻⁴ T − 2.87 × 10⁻⁶ T² g cm⁻³); and its triple point, 216.592 K,
/// where it can first melt (Span and Wagner 1996, J. Phys. Chem. Ref. Data 25, 1509).
const CARBON_DIOXIDE_ICE: Substance = Substance {
    key: CO2,
    normal_albedo_bvr: [0.6; 3],
    grain_density_kg_m3: 1_621.0,
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

/// The density of the reference worlds' stony projectiles, kg m⁻³: 3,000, the value that turns
/// Design note 12's d\* = 1.5 (P ÷ g) ÷ `ρ_p` into its 5.2 m for Earth and 0.52 km for Venus (its
/// 8.2 cm for Mars takes about 610 Pa; NASA's 636 Pa gives 8.5 cm), and P14.T48.f's for a rocky
/// source belt (science-r09-craters.md item 2).
const PROJECTILE_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(3_000.0);

/// The crater target of a silicate crust: non-porous at the rock class's 2,550 kg m⁻³, the lunar
/// highlands crust's bulk density from GRAIL (Wieczorek et al. 2013, Science 339, 671), as
/// P14.T48.f gives it to every silicate body above 150 km (science-r09-craters.md item 2). Earth,
/// Mars and the Moon take it; Earth's continental crust's 2,700–2,900 would shrink a transient
/// cavity by up to 3.4% and a complex crater by up to 3.9% (the ruling's "3% at most" rounds the
/// cavity's).
const SILICATE_TARGET: CraterTarget = CraterTarget {
    class: TargetClass::NonPorous,
    density: KilogramsPerCubicMetre::new(2_550.0),
};

/// Ceres's crater target: non-porous at its crust's bulk density, 1,287 kg m⁻³ (Ermakov et al.
/// 2017, JGR Planets 122, 2267, §5.4, from Dawn's shape and gravity), as P14.T51.c's primary crust will
/// give it (science-r09-craters.md item 2's refinement).
const CERES_TARGET: CraterTarget = CraterTarget {
    class: TargetClass::NonPorous,
    density: KilogramsPerCubicMetre::new(1_287.0),
};

/// The Earth's mean impact velocity under P14.T48.f's model: 19.9 km/s, against Ivanov et al.
/// 2002's 18.6–20.2 km/s and about 21 from Le Feuvre and Wieczorek 2008 (via quotation;
/// science-r09-craters.md item 4).
const EARTH_IMPACT_VELOCITY: MetresPerSecond = MetresPerSecond::new(19_900.0);

/// Mars's mean impact velocity under P14.T48.f's model: 10.6 km/s, against 9.6 (Ivanov 2001, via
/// Ivanov et al. 2002, §5.2) to 11.5 km/s (JeongAhn and Malhotra 2015, arXiv 1503.03885)
/// (science-r09-craters.md item 4).
const MARS_IMPACT_VELOCITY: MetresPerSecond = MetresPerSecond::new(10_600.0);

/// Ceres's mean impact velocity under P14.T48.f's model: 5.6 km/s, as Vesta's, against about 5
/// (Bottke et al. 1994, Icarus 107, 255, as science-r09-craters.md item 4 recalls it, unverified).
const CERES_IMPACT_VELOCITY: MetresPerSecond = MetresPerSecond::new(5_600.0);

/// The reference Mars's impact rate per projectile size over the Moon's, `R_b` = 1.92, which plan
/// 14's belt scaling will supply: what Ivanov 2001's Mars chronology, 0.4928 of the Moon's
/// (Craterstats' "Mars, Ivanov (2001)": 2.68 × 10⁻¹⁴ and 4.13 × 10⁻⁴ against 5.44 × 10⁻¹⁴ and
/// 8.38 × 10⁻⁴), implies under the diameter map as built, 0.4928 ÷ 0.2560, the map's own N(>1 km)
/// over its N☾ at Ivanov's 9.6 and 16.1 km/s (science-r09-craters.md F1 and F4.1). Ivanov et al.
/// 2002, §5.2, quote "~2 for asteroids of the same size (Ivanov, 2001)". A change to the map
/// re-derives it.
const MARS_IMPACT_RATE: f64 = 1.92;

/// What a reference world's crater contract takes beyond its age, gravity and screening: its
/// target factor, P14.T48.f's impactors, and its impact rate per projectile size over the Moon's,
/// which multiplies the lunar chronology into the lunar-equivalent density.
struct Impactors {
    k_target: f64,
    velocity: MetresPerSecond,
    target: CraterTarget,
    rate: f64,
}

/// The sidereal year, days (NASA's Earth fact sheet: 365.256 d).
const SIDEREAL_YEAR_DAYS: f64 = 365.256;

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

/// The crater contract at the lunar-equivalent N☾(>1 km) for `surface_age` in a system of the
/// Solar System's age: the lunar chronology ([`craters::lunar_chronology`], Neukum, Ivanov and
/// Hartmann 2001's curve as plan 14's P14.T24.b cites it) times the world's impact rate, standing
/// for the system's belt scaling (science-r09-craters.md F1), under `gravity`, screened by
/// `screening`, with `impactors`.
#[must_use]
fn crater_params(
    surface_age: Gigayears,
    gravity: MetresPerSecondSquared,
    screening: Screening,
    impactors: &Impactors,
) -> CraterParams {
    CraterParams::new(CraterParamsParts {
        n_1km: craters::lunar_chronology(surface_age, craters::SOLAR_SYSTEM_AGE) * impactors.rate,
        screening,
        gravity,
        k_target: impactors.k_target,
        impact_velocity: Some(impactors.velocity),
        projectile_density: PROJECTILE_DENSITY,
        target: impactors.target,
        system_age: craters::SOLAR_SYSTEM_AGE,
    })
    .expect("a reference world's crater contract is valid")
}

/// An atmosphere's screening of projectiles: its column mass P ÷ g (Design note 12), over the
/// contract's stony projectiles.
#[must_use]
fn atmosphere_screening(pressure: Pascals, gravity: MetresPerSecondSquared) -> Screening {
    Screening::Atmosphere {
        column_mass: KilogramsPerSquareMetre::new(pressure.value() / gravity.value()),
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
/// | craters | lunar-equivalent: the lunar chronology at that age, screened by the air; struck at 19.9 km/s by stony projectiles of 3,000 kg m⁻³, on a non-porous target of 2,550 kg m⁻³ | Design note 12; P14.T48.f's model (science-r09-craters.md items 2 and 4) |
/// | wet epoch | 4.4 Gyr ago to now, the ocean's inventory | the oldest zircons' evidence of liquid water (Wilde, Valley, Peck and Graham 2001, Nature 409, 175) |
/// | crust | basaltic oceanic, granitic continents, basaltic provinces; the lid the secondary crust, a mobile lid's basalt | P14.T51.c's Earth; science-composition-crusts.md §5 |
/// | palette | basalt (secondary crust), granite (tertiary), water as ice and as liquid; the ice's density at its coldest zone, 260 K, 918.6 kg m⁻³ | decision-composition §1.7; each figure its substance's row in this module, with its sources; the coldest zone T₀ − (2/3) × the equator–pole contrast (science-composition-crusts.md §3) |
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
    let (mean, contrast) = (Kelvin::new(288.0), 42.0);
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
        .mean_surface_temperature(mean)
        .equator_pole_contrast_k(contrast)
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
            &Impactors {
                k_target: 1.0,
                velocity: EARTH_IMPACT_VELOCITY,
                target: SILICATE_TARGET,
                rate: 1.0,
            },
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
            secondary: Some(BASALT_KEY),
            tertiary: Some(GRANITE_KEY),
            bulk: None,
            lid: LidCrust::Secondary,
            provinces: BASALT_KEY,
            melt_area_fraction: 0.0,
        })
        .palette(palette(vec![
            BASALT.entry(PaletteRole::SecondaryCrust),
            GRANITE.entry(PaletteRole::TertiaryCrust),
            WATER_ICE.ice_entry_at(coldest_zone(mean, contrast)),
            WATER.liquid_entry(WATER_RECORD),
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
/// | craters | lunar-equivalent: the lunar chronology at that age times `R_b` = 1.92, screened by the air (d\* = 8.5 cm at 636 Pa); struck at 10.6 km/s by stony projectiles of 3,000 kg m⁻³, on a non-porous target of 2,550 kg m⁻³ | Design note 12, whose 8.2 cm takes about 610 Pa; `R_b` is Ivanov 2001's Mars-to-Moon impact rate under the map, standing for plan 14's belt scaling (science-r09-craters.md F1, F4.1); P14.T48.f's model (items 2 and 4) |
/// | wet epoch | 3.9 to 3.6 Gyr ago, 10⁶ Earth-equivalent years, 1.56 × 10⁵ kg m⁻² of water | the valley networks of about 3.6–3.8 Ga (Hoke and Hynek 2009) in 10⁵–10⁷ such years (Hoke, Hynek and Tucker 2011; not read); Carr and Head 2003's ocean, about 2.3 × 10⁷ km³, 156 m as a global layer (low confidence) |
/// | crust | iron-rich basalt (secondary crust, its provinces' too), the lid; no primary crust named | P14.T51.c's Mars; science-composition-crusts.md §2 and §5 |
/// | palette | iron-rich basalt (secondary crust, its provinces' too), CO₂ and water ices, the bright dust as its deposit; the water ice's density at its coldest zone, 177.3 K, 928.4 kg m⁻³, and the CO₂ ice's at 150 K, near its frost point | decision-composition §1.7; each figure its row's; the coldest zone as Earth's, the P₂ fit's pole (science-composition-crusts.md §3), warmer than the contrasts' placeholder pole of 160 K, at which the ice would be 929.9 kg m⁻³ |
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
    let (mean, contrast) = (Kelvin::new(214.0), 55.0);
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
        .mean_surface_temperature(mean)
        .equator_pole_contrast_k(contrast)
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
            &Impactors {
                k_target: 1.0,
                velocity: MARS_IMPACT_VELOCITY,
                target: SILICATE_TARGET,
                rate: MARS_IMPACT_RATE,
            },
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
            secondary: Some(BASALT_FE_KEY),
            tertiary: None,
            bulk: None,
            lid: LidCrust::Secondary,
            provinces: BASALT_FE_KEY,
            melt_area_fraction: 0.0,
        })
        .palette(palette(vec![
            BASALT_FE.entry(PaletteRole::SecondaryCrust),
            CARBON_DIOXIDE_ICE.entry(PaletteRole::Ice),
            WATER_ICE.ice_entry_at(coldest_zone(mean, contrast)),
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
/// | craters | lunar-equivalent: the lunar chronology at that age; the diameter map's lunar reference, 16.7 km/s on a non-porous target of 2,550 kg m⁻³ by stony projectiles of 3,000 kg m⁻³, so its own density is the chronology's but for its gravity's 1.624 m s⁻² against the reference's 1.62 (0.2% lower) | Design note 12; P14.T48.f's model (science-r09-craters.md items 2 and 4) |
/// | crust | feldspathic (primary crust: the highlands, its magma ocean's flotation crust as impacts mixed it), iron-rich basalt (provinces: the maria, its secondary crust); the lid the primary crust | P14.T51.c's Moon; science-composition-crusts.md §2, §5 and §7 |
/// | palette | feldspathic (primary crust: the highlands, its magma ocean's flotation crust as impacts mixed it), iron-rich basalt (provinces: the maria, its secondary crust), water ice (its cold traps', its density at their 110 K threshold, 932.7 kg m⁻³) | decision-composition §1.7; science-r09-drainage.md §4 and §5; science-composition-crusts.md §3; each figure its row's. The water-ice entry names the exposed ice of its polar cold traps (Li et al. 2018, PNAS 115, 8907), which no area share places until P14.T51.f states it and R09.T13.f places it; a generated Moon carries none until then (science-composition-crusts.md §6) |
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
        .craters(crater_params(
            surface_age,
            gravity,
            Screening::None,
            &Impactors {
                k_target: 1.0,
                velocity: craters::LUNAR_IMPACT_VELOCITY,
                target: SILICATE_TARGET,
                rate: 1.0,
            },
        ))
        .climate(ClimateRegime {
            thermal: ThermalRegime::AirlessLike,
            forcing: Forcing::SlowRotator,
            condensable: None,
            model: ClimateModelKind::RadiativeEquilibrium,
        })
        .crust(CrustInputs {
            primary: Some(FELDSPATHIC_KEY),
            secondary: Some(BASALT_FE_KEY),
            tertiary: None,
            bulk: None,
            lid: LidCrust::Primary,
            provinces: BASALT_FE_KEY,
            melt_area_fraction: 0.0,
        })
        .palette(palette(vec![
            FELDSPATHIC.entry(PaletteRole::PrimaryCrust),
            BASALT_FE.entry(PaletteRole::Province),
            WATER_ICE.ice_entry_at(Kelvin::new(COLD_TRAP_H2O_K)),
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
/// | craters | lunar-equivalent: the lunar chronology at that age, on an ice-rich target (`k_target` 0.12), non-porous at its crust's 1,287 kg m⁻³; struck at 5.6 km/s by stony projectiles of 3,000 kg m⁻³ | Design notes 3 and 12; Ermakov et al. 2017 (JGR Planets 122, 2267); P14.T48.f's model (science-r09-craters.md items 2 and 4) |
/// | crust | phyllosilicate (primary crust: Ceres's frozen ocean, Castillo-Rogez et al. 2018; Fu et al. 2017), the lid; no secondary crust, its rock having never melted; the provinces the phyllosilicate too (V = 0 builds none) | science-composition-crusts.md §5; science-r09-drainage.md §5: "Ceres's crust encompasses the bulk of Ceres's original ocean" (Castillo-Rogez et al. 2018, MAPS 53, 1820, p. 1838); its rocky interior "avoided igneous differentiation" (Fu et al. 2017, EPSL 476, 153, abstract); R09.T2's synthetic Ceres |
/// | palette | phyllosilicate (primary crust: Ceres's frozen ocean, Castillo-Rogez et al. 2018; Fu et al. 2017), water ice (its density at the cold traps' 110 K, 932.7 kg m⁻³), sodium carbonate as its deposit (the faculae's) | decision-composition §1.7; science-r09-drainage.md §5; science-composition-crusts.md §3; each figure its row's. The water-ice entry names the ice of its cold traps and fresh craters (Platz et al. 2016, Nature Astronomy 1, 0007; Combe et al. 2016, Science 353, aaf3010), which no area share places until plan 14 states one; a fresh crater's ice near the 163 K mean would be 929.6 kg m⁻³, 0.3% lighter |
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
        .craters(crater_params(
            surface_age,
            gravity,
            Screening::None,
            &Impactors {
                k_target: 0.12,
                velocity: CERES_IMPACT_VELOCITY,
                target: CERES_TARGET,
                rate: 1.0,
            },
        ))
        .climate(ClimateRegime {
            thermal: ThermalRegime::AirlessLike,
            forcing: Forcing::Seasonal,
            condensable: None,
            model: ClimateModelKind::RadiativeEquilibrium,
        })
        .crust(CrustInputs {
            primary: Some(PHYLLOSILICATE_KEY),
            secondary: None,
            tertiary: None,
            bulk: None,
            lid: LidCrust::Primary,
            provinces: PHYLLOSILICATE_KEY,
            melt_area_fraction: 0.0,
        })
        .palette(palette(vec![
            PHYLLOSILICATE.entry(PaletteRole::PrimaryCrust),
            WATER_ICE.ice_entry_at(Kelvin::new(COLD_TRAP_H2O_K)),
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

    /// The surface crate's liquid bins are the sky's bake bins to the bit, so that a liquid
    /// record's bin means and a client's interpolation of them share one grid with each star's
    /// bake spectrum (decision-r11-palette-liquids §1.2).
    #[test]
    fn the_liquid_bins_are_the_sky_s_bake_bins_to_the_bit() {
        let bits = hyperion_testkit::float::bits;
        assert_eq!(
            hyperion_surface::field::LIQUID_BIN_CENTRES_NM.map(bits),
            crate::sky::colour::BAKE_WAVELENGTHS_NM.map(bits)
        );
    }

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
        let Screening::Atmosphere { column_mass } = mars.craters().screening() else {
            panic!("Mars's craters are screened by its air");
        };
        let d_star = 1.5 * column_mass.value() / mars.craters().projectile_density().value();
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

    /// Each world's crater contract is lunar-equivalent and carries P14.T48.f's impactors
    /// (science-r09-craters.md items 2 and 4, F1 and F4.1): the Moon sits at the diameter map's
    /// lunar reference but for its gravity, and Mars takes Ivanov 2001's impact rate.
    #[test]
    fn the_reference_crater_contracts_carry_their_impactors() {
        use hyperion_testkit::float::assert_same_bits;
        let chronology = |inputs: &CoarseInputs| {
            craters::lunar_chronology(inputs.surface_age(), craters::SOLAR_SYSTEM_AGE).value()
        };
        for (name, world, velocity, target, rate) in [
            ("Earth", earth_like(), 19_900.0, SILICATE_TARGET, 1.0),
            ("Mars", mars_like(), 10_600.0, SILICATE_TARGET, 1.92),
            ("the Moon", moon_like(), 16_700.0, SILICATE_TARGET, 1.0),
            ("Ceres", ceres_like(), 5_600.0, CERES_TARGET, 1.0),
        ] {
            let c = world.craters();
            assert_eq!(
                c.impact_velocity(),
                Some(MetresPerSecond::new(velocity)),
                "{name}"
            );
            assert_eq!(c.target(), target, "{name}");
            assert_eq!(c.projectile_density(), PROJECTILE_DENSITY, "{name}");
            assert_eq!(c.system_age(), craters::SOLAR_SYSTEM_AGE, "{name}");
            assert_same_bits(c.n_1km().value(), chronology(&world) * rate);
        }
        // The rock target and the projectiles are the map's lunar reference, so the Moon's own
        // density is its N☾ but for its gravity, GM ÷ R² = 1.624 m s⁻² against the reference's
        // 1.62: 0.2% lower.
        assert_eq!(SILICATE_TARGET, craters::LUNAR_TARGET);
        assert_eq!(PROJECTILE_DENSITY, craters::LUNAR_PROJECTILE_DENSITY);
        let moon = moon_like();
        let own =
            craters::own_density_at_1km(moon.craters()).value() / moon.craters().n_1km().value();
        assert!((own - 1.0).abs() < 0.005, "{own}");
        // Mars's own N(>1 km) is about half its lunar chronology's, as Ivanov's 0.49 has it at his
        // velocities; at P14.T48.f's 10.6 km/s its craters are larger, so a little more.
        let mars = mars_like();
        let own = craters::own_density_at_1km(mars.craters()).value() / chronology(&mars);
        assert!((0.49..0.56).contains(&own), "{own}");
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
                (SecondaryCrust, "basalt_fe"),
                (Ice, "CO2"),
                (Ice, "H2O"),
                (Deposit, "mars_dust"),
            ])
        );
        assert_eq!(
            entries(&moon_like()),
            expect(&[
                (PrimaryCrust, "feldspathic"),
                (Province, "basalt_fe"),
                (Ice, "H2O"),
            ])
        );
        assert_eq!(
            entries(&ceres_like()),
            expect(&[
                (PrimaryCrust, "phyllosilicate"),
                (Ice, "H2O"),
                (Deposit, "Na2CO3"),
            ])
        );
        // The Moon's cold-trapped water ice and Ceres's are entries no share names yet
        // (science-r09-drainage.md §4).
        for world in [moon_like(), ceres_like()] {
            assert_eq!(world.ices(), []);
            assert_eq!(world.condensates(), []);
        }
        // Each world's sea, where it has one, is its main liquid.
        let earth = earth_like();
        let sea = earth.palette().get(earth.main_liquid().unwrap()).unwrap();
        assert_eq!((sea.role, sea.substance), (Liquid, H2O));
        assert!((sea.grain_density_kg_m3 - 997.047).abs() < 1e-9);
        // Its record is water's at 25 °C, measured, its normal albedo the record's Fresnel
        // reflectance to the row's four places (decision-r11-palette-liquids §2, item 7), which
        // follow the record's index when `water.json` moves it.
        let record = sea
            .liquid
            .as_ref()
            .expect("a sea carries its liquid record");
        assert_eq!(record.basis, hyperion_surface::field::LiquidBasis::Measured);
        assert!((record.temperature.value() - 298.15).abs() < 1e-9);
        for (held, fresnel) in sea
            .normal_albedo_bvr
            .iter()
            .zip(record.fresnel_normal_albedo_bvr())
        {
            assert!((held - fresnel).abs() < 5e-5, "{held} against {fresnel}");
        }
        // No other world's entry carries a record: none has a sea or a melting crust.
        for world in [mars_like(), moon_like(), ceres_like()] {
            assert!(world.palette().entries().iter().all(|e| e.liquid.is_none()));
        }
    }

    /// Each row's density is a grain density, within its sources' range of solids, or, at the
    /// porosity its sources state, giving back their bulk density (science-r09-drainage.md §3;
    /// science-composition-crusts.md §2, §4, §7 and §10.1).
    #[test]
    fn the_grain_densities_follow_their_sources() {
        let bulk = |row: &Substance, porosity: f64| row.grain_density_kg_m3 * (1.0 - porosity);
        // Kiefer et al. 2012's four feldspathic highland rocks, 2,840–2,910 kg m⁻³ by helium
        // pycnometry (Table 1).
        assert!((2_840.0..=2_910.0).contains(&FELDSPATHIC.grain_density_kg_m3));
        // At the 12 per cent porosity Wieczorek et al. 2013 derive from GRAIL's bulk density, the
        // highlands' 2,550 kg m⁻³, which the row's 2,900 rounds: 2,550 ÷ 0.88 is 2,898. Their own
        // average, so this checks the arithmetic, not an independent measurement.
        let highlands = bulk(&FELDSPATHIC, 0.12);
        assert!((highlands - 2_550.0).abs() < 5.0, "{highlands} kg m⁻³");
        // The lunar basalts' 3,270 (high-alumina, NWA 4898) to 3,460 kg m⁻³ (high-titanium,
        // 70215), which rises with TiO₂ (Kiefer et al. 2012, abstract and Table 1).
        assert!((3_270.0..=3_460.0).contains(&BASALT_FE.grain_density_kg_m3));
        // Viking Lander 1's fine material, 1,100 ± 150 kg m⁻³, at the 58 per cent porosity that
        // Moore et al. 1987 (p. 121) derive from an assumed grain density: this pins the row to
        // their arithmetic, not to a measured grain density.
        let fines = bulk(&MARS_DUST, 0.58);
        assert!((fines - 1_100.0).abs() < 150.0, "{fines} kg m⁻³");
        // Alae lava lake's eight tholeiites, 2,920–3,040 kg m⁻³ (Peck 1978).
        assert!((2_920.0..=3_040.0).contains(&BASALT.grain_density_kg_m3));
        // Granite G-1's measured mode across its minerals' ranges, 2,635–2,671 kg m⁻³.
        assert!((2_635.0..=2_671.0).contains(&GRANITE.grain_density_kg_m3));
        // Each palette entry carries its row's density: the Moon's lid its highlands', its maria
        // and Mars's lid the iron-rich basalt's.
        let density = |inputs: &CoarseInputs, entry: u8| {
            let e = inputs.palette().get(entry).unwrap();
            (e.substance, e.grain_density_kg_m3)
        };
        let moon = moon_like();
        let (lid, rho) = density(&moon, moon.lid_entry());
        assert_eq!(lid, FELDSPATHIC_KEY);
        assert!((rho - 2_900.0).abs() < 1e-9);
        let (maria, rho) = density(&moon, moon.crust_palette()[3].unwrap());
        assert_eq!(maria, BASALT_FE_KEY);
        assert!((rho - 3_310.0).abs() < 1e-9);
        let mars = mars_like();
        let (lid, rho) = density(&mars, mars.lid_entry());
        assert_eq!(lid, BASALT_FE_KEY);
        assert!((rho - 3_310.0).abs() < 1e-9);
    }

    /// Each world's surface shows the crust science-composition-crusts.md §5.2 gives it: Earth's
    /// and Mars's lids their secondary crusts, the Moon's and Ceres's their primary ones; none is
    /// an undifferentiated body, and the iron-rich basalt is the Moon's and Mars's, the tholeiite
    /// Earth's (§2).
    #[test]
    fn each_reference_world_shows_the_lid_the_ruling_gives_it() {
        for (name, world, lid, secondary) in [
            ("Earth", earth_like(), LidCrust::Secondary, Some(BASALT_KEY)),
            (
                "Mars",
                mars_like(),
                LidCrust::Secondary,
                Some(BASALT_FE_KEY),
            ),
            (
                "the Moon",
                moon_like(),
                LidCrust::Primary,
                Some(BASALT_FE_KEY),
            ),
            ("Ceres", ceres_like(), LidCrust::Primary, None),
        ] {
            let crust = world.crust();
            assert_eq!(crust.lid, lid, "{name}");
            assert_eq!(crust.bulk, None, "{name}");
            assert_eq!(crust.secondary, secondary, "{name}");
        }
        assert_eq!(moon_like().crust().primary, Some(FELDSPATHIC_KEY));
        assert_eq!(mars_like().crust().primary, None);
    }

    /// Each world's ice entry takes ice Ih's density ([`substance::ice_ih_density`], whose
    /// IAPWS checks moved with it) where its ice lies: the coldest zone on Earth and Mars, the
    /// cold traps' threshold on the Moon and Ceres (science-composition-crusts.md §3).
    #[test]
    fn the_ice_entries_take_ice_ih_s_density_where_the_ice_lies() {
        let ice = |inputs: &CoarseInputs| {
            let entry = inputs.palette().find(PaletteRole::Ice, H2O).unwrap();
            inputs.palette().get(entry).unwrap().grain_density_kg_m3
        };
        for (name, world, rho) in [
            ("Earth", earth_like(), 918.6),
            ("Mars", mars_like(), 928.4),
            ("the Moon", moon_like(), 932.7),
            ("Ceres", ceres_like(), 932.7),
        ] {
            let got = ice(&world);
            assert!((got - rho).abs() < 0.1, "{name}'s ice: {got} kg m⁻³");
        }
    }

    /// Every key the reference worlds use is a row of plan 14's registry (P14.T49.b): their gases,
    /// condensates, covers, crusts and palettes.
    #[test]
    fn every_key_the_reference_worlds_use_is_a_substance_row() {
        let mut keys = std::collections::BTreeSet::new();
        for world in [earth_like(), mars_like(), moon_like(), ceres_like()] {
            keys.extend(world.gases().iter().map(|gas| gas.substance));
            keys.extend(world.condensates().iter().map(|c| c.substance));
            keys.extend(
                world
                    .liquids()
                    .iter()
                    .chain(world.ices())
                    .map(|a| a.substance),
            );
            keys.extend(world.palette().entries().iter().map(|e| e.substance));
            let crust = world.crust();
            keys.extend(
                [crust.primary, crust.secondary, crust.tertiary, crust.bulk]
                    .into_iter()
                    .flatten(),
            );
            keys.insert(crust.provinces);
        }
        let keys: Vec<&str> = keys.iter().map(SubstanceKey::as_str).collect();
        for key in &keys {
            assert!(substance::by_key(key).is_some(), "{key} is a registry row");
        }
        assert_eq!(
            keys,
            [
                "Ar",
                "CH4",
                "CO",
                "CO2",
                "H2O",
                "He",
                "N2",
                "Na2CO3",
                "Ne",
                "O2",
                "basalt",
                "basalt_fe",
                "feldspathic",
                "granite",
                "mars_dust",
                "phyllosilicate",
            ]
        );
    }

    /// Every key R09's synthetic worlds' palettes use is a row of plan 14's registry
    /// (`hyperion_surface::testing`, under the crate's `testing` feature).
    #[cfg(feature = "testing")]
    #[test]
    fn every_key_the_synthetic_worlds_use_is_a_substance_row() {
        use hyperion_surface::field::FieldView;
        use hyperion_surface::testing::{SyntheticWorld, synthetic_field};
        for &world in SyntheticWorld::ALL {
            let field = synthetic_field(world);
            for entry in field.header().palette().entries() {
                let key = entry.substance;
                assert!(
                    substance::by_key(key.as_str()).is_some(),
                    "{world:?}'s {key} is a registry row"
                );
            }
        }
    }

    /// The reference rows are the registry's (P14.T49.b): each lithology's and salt's grain
    /// density, transition and mechanics family equal its row's condensed columns to the bit.
    #[test]
    fn the_reference_rows_are_the_substance_registry_s() {
        use crate::substance::CondensedPhase;
        use hyperion_testkit::float::assert_same_bits;
        for row in [
            &BASALT,
            &BASALT_FE,
            &GRANITE,
            &FELDSPATHIC,
            &PHYLLOSILICATE,
            &SODIUM_CARBONATE,
            &MARS_DUST,
        ] {
            let registry = substance::by_key(row.key.as_str())
                .unwrap_or_else(|| panic!("{} is a registry row", row.key))
                .substance();
            let condensed = registry.condensed().expect("a reference row is condensed");
            // A rock keeps its room-temperature grain density at every temperature.
            for t in [150.0, 293.0] {
                let rho = registry
                    .grain_density(CondensedPhase::Solid, Kelvin::new(t))
                    .expect("a rock has a solid's grain density");
                assert_same_bits(rho.value(), row.grain_density_kg_m3);
            }
            assert_same_bits(
                condensed.transition().temperature().value(),
                row.transition_k,
            );
            assert_eq!(condensed.mechanics(), row.mechanics, "{}", row.key);
        }
        // The ices take their rows' diagrams: water ice Ih's density where it lies, carbon
        // dioxide's at the 150 K of its row here, and their triple points, within the 0.01 K
        // between water's and its melting at one atmosphere.
        let water = substance::by_key("H2O").unwrap().substance();
        let at = Kelvin::new(COLD_TRAP_H2O_K);
        assert_same_bits(
            water
                .grain_density(CondensedPhase::Solid, at)
                .unwrap()
                .value(),
            WATER_ICE.ice_entry_at(at).grain_density_kg_m3,
        );
        let ice = water
            .condensed()
            .unwrap()
            .transition()
            .temperature()
            .value();
        assert!((ice - WATER_ICE.transition_k).abs() < 0.011, "{ice} K");
        assert_eq!(water.condensed().unwrap().mechanics(), WATER_ICE.mechanics);
        let co2 = substance::by_key("CO2").unwrap().substance();
        let frost = co2
            .grain_density(CondensedPhase::Solid, Kelvin::new(150.0))
            .unwrap()
            .value();
        assert!(
            (frost - CARBON_DIOXIDE_ICE.grain_density_kg_m3).abs() < 0.5,
            "{frost}"
        );
        assert_same_bits(
            co2.condensed().unwrap().transition().temperature().value(),
            CARBON_DIOXIDE_ICE.transition_k,
        );
        assert_eq!(
            co2.condensed().unwrap().mechanics(),
            CARBON_DIOXIDE_ICE.mechanics
        );
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

    /// The reference Earth's water carries the registry's scattering, to the bit: its copy, as
    /// shortest round-trip decimals, is `liquid_properties`' b at the record's temperature
    /// (P14.T49.h; decision-r10-t10b-r09-t2e §7, condition 1).
    #[test]
    fn the_reference_water_carries_the_registry_s_scattering() {
        use crate::substance::{Composition, by_key, liquid_properties};
        use hyperion_testkit::float::assert_same_bits;
        let water = by_key("H2O").unwrap();
        let resolved =
            liquid_properties(water, WATER_RECORD.temperature, Composition::Reference).unwrap();
        let held = WATER_RECORD.scattering_per_m.unwrap();
        for (h, r) in held.iter().zip(&resolved.scattering_per_m.unwrap()) {
            assert_same_bits(*h, *r);
        }
    }
}
