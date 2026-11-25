//! What decides a site's rock abundance: its material, its surface's age, its atmosphere and the
//! ejecta of a coarse crater about it (plan R11, Design note 4).
//!
//! # The mapping
//!
//! > k = clamp(`k_base`(material) · A(age) + `k_ejecta`, 0, 0.40)
//!
//! - **`k_base`** ([`base_abundance`]) is the form's, read with its palette entry's mechanics
//!   family: young bedrock and volcanic rock 0.20 (Design note 4's 0.15–0.25), regolith 0.065
//!   (0.05–0.08), soil 0.08, sediment 0.05, dust 0.02 (where dust mantles the ground, 0.01–0.03),
//!   the ice forms 0.035 (0.02–0.05), an organic sediment 0.01 (about 0.01 on Titan-like fluvial
//!   plains), and nothing on a playa's salts; a rock form of an ice entry takes the ice forms'
//!   value, and a sediment of an ice or organic entry the organic one. A seabed (under any sea:
//!   R10 has no liquid class), sea ice, sand and melt bear no rocks at all, ejecta included
//!   ([`bears_rocks`]).
//! - **A(age)** ([`age_factor`]) is a + (1 − a) e^(−t ÷ τ) of the body's surface age t: rocks
//!   break down to the equilibrium share a that impacts keep making, faster and further on an
//!   airless or tenuous body (a = 0.6, τ = 0.3 Gyr: micrometeorite gardening and thermal fatigue
//!   unshielded, as lunar ejecta's blocks break down, their rockiness falling as the age^−0.46
//!   over nine craters, Ghent et al. 2014, Geology 42, 1059) than under an atmosphere (a = 0.85,
//!   τ = 2 Gyr). The airless floor puts mature mare regolith at 0.039,
//!   Design note 4's 0.03–0.05 for the authoritative sizes; the atmosphere's puts the landers'
//!   sites where they were measured, Viking 1's soil at 0.070, Viking 2's volcanic plain at 0.18
//!   and Pathfinder's flood-scoured rock at 0.18 (the tests' conditions).
//! - **`k_ejecta`** ([`ejecta_abundance`]) is 0.3 e^(−(r − R) ÷ R) outside the rim, at a distance r
//!   from the centre of a coarse crater of rim radius R, aged by A's decaying part e^(−t ÷ τ) of
//!   the crater's own age t, so that old ejecta keep no blocks (after Ghent et al. 2014), and only
//!   for a crater no older than the surface. It ends at the crater's reach, (1 + a) R at most 2.54 R,
//!   since the field indexes a coarse crater only in the cells its reach touches, not at Design
//!   note 4's 3R; where several reach the site, the largest term is taken.
//!
//! The mapping is a ruling of medium to low confidence (Design note 4): no published law covers
//! generated worlds, and on an airless world the one-parameter law cannot fit the flatter lunar
//! size distribution, so k is set there for the authoritative sizes and small rocks are knowingly
//! wrong. Every figure here but the law's is that ruling's choice; the sources named above were
//! re-checked on 2026-11-13 against Golombek and Rapp 1997's, Golombek et al. 2003's and Ghent et
//! al. 2014's abstracts, not their full texts (plan R11, the records of R11.T3).
//!
//! # Inputs both sides hold
//!
//! The material and the coarse craters come through R09's [`Synthesiser`] and R10's
//! [`material_at`], and the surface age and pressure from R09's
//! [`FieldHeader`](crate::field::FieldHeader) (plan 14's figures, which both sides' fields carry),
//! so the client places the rocks the server does. A site whose inputs the view does not hold is [`QueryHeightError::NotSurveyed`].

use hyperion_base::math;
use hyperion_base::units::{Gigayears, Metres, Pascals};

use super::law::RockAbundance;
use crate::craters::crater_shape;
use crate::cube::{PatchKey, xyz_to_face_uv};
use crate::field::{FieldView, MechanicsFamily};
use crate::ground::datum_foot;
use crate::material::{MaterialClass, material_at};
use crate::num;
use crate::synth::{QueryHeightError, SynthCache, Synthesiser};

/// The surface pressure below which a body is airless to the sim: 100 Pa, plan 14's
/// `AIRLESS_PRESSURE` (the sim's `planetary::derive::atmosphere`), below which its surface state is
/// `Airless` or `Tenuous`. This crate cannot see the sim, so the figure is repeated here; the two
/// change together.
pub const AIRLESS_PRESSURE: Pascals = Pascals::new(100.0);

/// A site's atmosphere, as its rocks' breakdown reads it: from the header's surface pressure,
/// which is exactly 0 on an airless body and a tenuous body's own below [`AIRLESS_PRESSURE`]
/// (decision-r11-palette-liquids §2, item 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AtmosphereClass {
    /// No atmosphere: a surface pressure of 0.
    Airless,
    /// A tenuous atmosphere, above 0 and below [`AIRLESS_PRESSURE`]: a Pluto's or a Triton's,
    /// which shields its ground from neither micrometeorites nor the day's heat, so that its rocks
    /// break down as an airless body's do.
    Tenuous,
    /// An atmosphere, at or above [`AIRLESS_PRESSURE`].
    Atmosphere,
}

impl AtmosphereClass {
    /// Whether its rocks break down as an airless body's: [`Airless`](Self::Airless) and
    /// [`Tenuous`](Self::Tenuous).
    #[must_use]
    pub const fn breaks_down_as_airless(self) -> bool {
        match self {
            Self::Airless | Self::Tenuous => true,
            Self::Atmosphere => false,
        }
    }
}

/// The atmosphere class of a surface pressure of `surface_pressure`.
///
/// # Panics
///
/// If the pressure is not finite or is negative, which no header holds.
#[must_use]
pub fn atmosphere_class(surface_pressure: Pascals) -> AtmosphereClass {
    let p = surface_pressure.value();
    assert!(
        p.is_finite() && p >= 0.0,
        "a surface pressure must be finite and non-negative, got {p}"
    );
    if p <= 0.0 {
        AtmosphereClass::Airless
    } else if p < AIRLESS_PRESSURE.value() {
        AtmosphereClass::Tenuous
    } else {
        AtmosphereClass::Atmosphere
    }
}

/// `k_base` of young bedrock and volcanic rock: 0.20, the middle of Design note 4's 0.15–0.25.
pub const YOUNG_ROCK_ABUNDANCE: f64 = 0.20;

/// `k_base` of a regolith: 0.065, the middle of Design note 4's 0.05–0.08.
pub const REGOLITH_ABUNDANCE: f64 = 0.065;

/// `k_base` of a soil: 0.08, the top of the regolith's range, Viking 1's rocky plain (lean,
/// 2026-11-02).
pub const SOIL_ABUNDANCE: f64 = 0.08;

/// `k_base` of a sediment: 0.05, a channel's bed or a basin's floor (lean, 2026-11-02).
pub const SEDIMENT_ABUNDANCE: f64 = 0.05;

/// `k_base` of dust: 0.02, the middle of Design note 4's 0.01–0.03 where dust mantles the ground.
pub const DUST_ABUNDANCE: f64 = 0.02;

/// `k_base` of the ice forms, and of a rock form of an ice entry: 0.035, the middle of Design
/// note 4's 0.02–0.05.
pub const ICE_ABUNDANCE: f64 = 0.035;

/// `k_base` of an organic sediment, and of a sediment of an ice or organic entry: 0.01, Design note
/// 4's "about 0.01 on Titan-like fluvial ice plains".
pub const ORGANIC_ABUNDANCE: f64 = 0.01;

/// The equilibrium share a of an airless or tenuous body's A(age) (lean, 2026-11-02).
const AIRLESS_FLOOR: f64 = 0.6;

/// The breakdown time τ of an airless or tenuous body's rocks, Gyr (lean, 2026-11-02).
const AIRLESS_BREAKDOWN_GYR: f64 = 0.3;

/// The equilibrium share a of A(age) under an atmosphere (lean, 2026-11-02).
const ATMOSPHERE_FLOOR: f64 = 0.85;

/// The breakdown time τ of rocks under an atmosphere, Gyr (lean, 2026-11-02).
const ATMOSPHERE_BREAKDOWN_GYR: f64 = 2.0;

/// `k_ejecta` at a coarse crater's rim: 0.3 (Design note 4).
pub const RIM_EJECTA_ABUNDANCE: f64 = 0.3;

/// Whether a form bears rocks at all: every one but [`Seabed`](MaterialClass::Seabed),
/// [`SeaIce`](MaterialClass::SeaIce), [`Sand`](MaterialClass::Sand) and
/// [`Melt`](MaterialClass::Melt), on which no rock stands, a crater's ejecta included.
#[must_use]
pub const fn bears_rocks(class: MaterialClass) -> bool {
    match class {
        MaterialClass::Seabed
        | MaterialClass::SeaIce
        | MaterialClass::Sand
        | MaterialClass::Melt => false,
        MaterialClass::Bedrock
        | MaterialClass::Regolith
        | MaterialClass::Soil
        | MaterialClass::Sediment
        | MaterialClass::VolcanicRock
        | MaterialClass::Glacier
        | MaterialClass::Snow
        | MaterialClass::Frost
        | MaterialClass::OrganicSediment
        | MaterialClass::Evaporite
        | MaterialClass::Dust => true,
    }
}

/// `k_base`, a young surface's rock abundance, of the form `class` of an entry of the mechanics
/// family `mechanics` (`None` where the palette names no entry, read as rock): see the [module
/// documentation](self).
#[must_use]
pub const fn base_abundance(class: MaterialClass, mechanics: Option<MechanicsFamily>) -> f64 {
    let (icy, organic) = match mechanics {
        None | Some(MechanicsFamily::Silicate | MechanicsFamily::Metal | MechanicsFamily::Salt) => {
            (false, false)
        }
        Some(MechanicsFamily::WaterIce | MechanicsFamily::VolatileIce) => (true, false),
        Some(MechanicsFamily::Organic) => (false, true),
    };
    match class {
        MaterialClass::Seabed
        | MaterialClass::SeaIce
        | MaterialClass::Sand
        | MaterialClass::Melt
        | MaterialClass::Evaporite => 0.0,
        MaterialClass::Glacier | MaterialClass::Snow | MaterialClass::Frost => ICE_ABUNDANCE,
        MaterialClass::OrganicSediment => ORGANIC_ABUNDANCE,
        MaterialClass::Dust => DUST_ABUNDANCE,
        MaterialClass::Sediment => {
            if icy || organic {
                ORGANIC_ABUNDANCE
            } else {
                SEDIMENT_ABUNDANCE
            }
        }
        MaterialClass::Bedrock | MaterialClass::VolcanicRock => {
            if icy {
                ICE_ABUNDANCE
            } else if organic {
                ORGANIC_ABUNDANCE
            } else {
                YOUNG_ROCK_ABUNDANCE
            }
        }
        MaterialClass::Regolith | MaterialClass::Soil => {
            if icy {
                ICE_ABUNDANCE
            } else if organic {
                ORGANIC_ABUNDANCE
            } else if matches!(class, MaterialClass::Soil) {
                SOIL_ABUNDANCE
            } else {
                REGOLITH_ABUNDANCE
            }
        }
    }
}

/// The equilibrium share a and the breakdown time τ of `atmosphere`'s A(age).
#[must_use]
const fn breakdown(atmosphere: AtmosphereClass) -> (f64, Gigayears) {
    if atmosphere.breaks_down_as_airless() {
        (AIRLESS_FLOOR, Gigayears::new(AIRLESS_BREAKDOWN_GYR))
    } else {
        (ATMOSPHERE_FLOOR, Gigayears::new(ATMOSPHERE_BREAKDOWN_GYR))
    }
}

/// e^(−t ÷ τ) of the age `age` and the breakdown time `tau`.
///
/// # Panics
///
/// If `age` is not finite and non-negative.
#[must_use]
fn decay(age: Gigayears, tau: Gigayears) -> f64 {
    let t = age.value();
    assert!(
        t.is_finite() && t >= 0.0,
        "an age must be finite and non-negative, got {t}"
    );
    math::exp(-t / tau.value())
}

/// A(age) = a + (1 − a) e^(−t ÷ τ), the share of a young surface's rocks a surface `age` old keeps
/// under `atmosphere`: 1 at no age, falling to a (see the [module documentation](self)).
///
/// # Panics
///
/// If `age` is not finite and non-negative.
#[must_use]
pub fn age_factor(age: Gigayears, atmosphere: AtmosphereClass) -> f64 {
    let (floor, tau) = breakdown(atmosphere);
    floor + (1.0 - floor) * decay(age, tau)
}

/// The share of its blocks a crater's ejecta keep at the crater's age `age` under `atmosphere`: A's
/// decaying part, e^(−t ÷ τ), 1 when fresh and nothing at length.
///
/// # Panics
///
/// If `age` is not finite and non-negative.
#[must_use]
pub fn ejecta_age_factor(age: Gigayears, atmosphere: AtmosphereClass) -> f64 {
    decay(age, breakdown(atmosphere).1)
}

/// A coarse crater's ejecta at a site: its rim radius, the site's distance from its centre (the
/// chord between their spheroid points) and its age.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct CraterEjecta {
    rim_radius: Metres,
    distance: Metres,
    age: Gigayears,
}

impl CraterEjecta {
    /// The ejecta of a crater of `rim_radius` and `age` at `distance` from its centre.
    ///
    /// # Panics
    ///
    /// If the rim radius is not positive and finite, the distance not finite and non-negative, or
    /// the age not finite and non-negative.
    #[must_use]
    pub fn new(rim_radius: Metres, distance: Metres, age: Gigayears) -> Self {
        let (r, d, t) = (rim_radius.value(), distance.value(), age.value());
        assert!(
            r.is_finite() && r > 0.0,
            "a rim radius must be positive, got {r}"
        );
        assert!(
            d.is_finite() && d >= 0.0,
            "a distance must be non-negative, got {d}"
        );
        assert!(
            t.is_finite() && t >= 0.0,
            "an age must be non-negative, got {t}"
        );
        Self {
            rim_radius,
            distance,
            age,
        }
    }

    /// The crater's rim radius.
    #[must_use]
    pub const fn rim_radius(&self) -> Metres {
        self.rim_radius
    }

    /// The site's distance from the crater's centre.
    #[must_use]
    pub const fn distance(&self) -> Metres {
        self.distance
    }

    /// The crater's age.
    #[must_use]
    pub const fn age(&self) -> Gigayears {
        self.age
    }
}

/// `k_ejecta`, the abundance a crater's ejecta add at a site under `atmosphere`: 0.3
/// e^(−(r − R) ÷ R) e^(−t ÷ τ) outside the rim, and nothing inside it (see the [module
/// documentation](self)).
#[must_use]
pub fn ejecta_abundance(ejecta: &CraterEjecta, atmosphere: AtmosphereClass) -> f64 {
    let (r_m, d_m) = (ejecta.rim_radius.value(), ejecta.distance.value());
    if d_m <= r_m {
        return 0.0;
    }
    RIM_EJECTA_ABUNDANCE * math::exp(-(d_m - r_m) / r_m) * ejecta_age_factor(ejecta.age, atmosphere)
}

/// What decides k at a point, from inputs both sides hold (see the [module documentation](self)):
/// the form and its entry's mechanics family, the body's surface age and atmosphere class, and the
/// coarse crater whose ejecta add the most there.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct RockSite {
    class: MaterialClass,
    mechanics: Option<MechanicsFamily>,
    surface_age: Gigayears,
    atmosphere: AtmosphereClass,
    ejecta: Option<CraterEjecta>,
}

impl RockSite {
    /// The site of the form `class` of an entry of the family `mechanics` (`None` where the palette
    /// names none), on a surface `surface_age` old under `atmosphere`, with `ejecta`'s crater.
    ///
    /// # Panics
    ///
    /// If the surface age is not finite and non-negative.
    #[must_use]
    pub fn new(
        class: MaterialClass,
        mechanics: Option<MechanicsFamily>,
        surface_age: Gigayears,
        atmosphere: AtmosphereClass,
        ejecta: Option<CraterEjecta>,
    ) -> Self {
        let t = surface_age.value();
        assert!(
            t.is_finite() && t >= 0.0,
            "a surface age must be finite and non-negative, got {t}"
        );
        Self {
            class,
            mechanics,
            surface_age,
            atmosphere,
            ejecta,
        }
    }

    /// The ground's form, R10's [`material_at`].
    #[must_use]
    pub const fn class(&self) -> MaterialClass {
        self.class
    }

    /// The mechanics family of the ground's palette entry, or `None` where the palette names none.
    #[must_use]
    pub const fn mechanics(&self) -> Option<MechanicsFamily> {
        self.mechanics
    }

    /// The body's surface age.
    #[must_use]
    pub const fn surface_age(&self) -> Gigayears {
        self.surface_age
    }

    /// The body's atmosphere class.
    #[must_use]
    pub const fn atmosphere(&self) -> AtmosphereClass {
        self.atmosphere
    }

    /// The coarse crater whose ejecta add the most at the site, or `None` where none reaches it.
    #[must_use]
    pub const fn ejecta(&self) -> Option<CraterEjecta> {
        self.ejecta
    }
}

/// The rock abundance of `site`: clamp(`k_base` · A(age) + `k_ejecta`, 0, 0.40), and none where
/// its form bears no rocks (see the [module documentation](self)).
#[must_use]
pub fn rock_abundance(site: &RockSite) -> RockAbundance {
    if !bears_rocks(site.class) {
        return RockAbundance::NONE;
    }
    let base =
        base_abundance(site.class, site.mechanics) * age_factor(site.surface_age, site.atmosphere);
    let ejecta = site
        .ejecta
        .map_or(0.0, |ejecta| ejecta_abundance(&ejecta, site.atmosphere));
    RockAbundance::new(base + ejecta)
}

/// The site at the body-fixed point `at`, metres from the body's centre.
///
/// It is R10's [`material_at`] at the point's foot on the datum, the palette entry's family, the
/// header's surface age and atmosphere class, and the coarse crater of the foot's coarse cell whose
/// ejecta add the most there (see the [module documentation](self)). `cache` is the caller's, and
/// the site does not depend on what it holds.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the first cell the classifier reads that the view does
/// not hold.
///
/// # Panics
///
/// If `at` is not finite or is the body's centre, as [`ground_at`](crate::ground::ground_at); or
/// if a height is not finite (a bug).
pub fn rock_site<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    at: &[f64; 3],
) -> Result<RockSite, QueryHeightError> {
    let (dir, _) = datum_foot(&synth.field().header().figure(), at);
    site_at(synth, cache, dir)
}

/// [`rock_site`] at the unit direction `dir` of the point's foot.
///
/// # Errors
///
/// As [`rock_site`].
pub(crate) fn site_at<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    dir: [f64; 3],
) -> Result<RockSite, QueryHeightError> {
    let material = material_at(synth, cache, xyz_to_face_uv(dir))?;
    material_site(synth.field(), material.class(), material.entry(), dir)
}

/// The site of the ground of the form `class` of the palette entry `entry` (`None` where the palette
/// names none) at the unit direction `dir` of a point's foot on `field`'s body: [`rock_site`]'s,
/// for a material already classified there. The palette entry's family, the header's surface age
/// and atmosphere class, and the coarse crater of the foot's coarse cell whose ejecta add the most
/// there (see the [module documentation](self)).
///
/// R11.T6.a's decorative scatter reads it at a patch's centre for each material of the patch's
/// palette (`WorkerFields::rock_abundances`), so that its pebbles take the law the rocks do.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the foot's coarse cell where `field` does not hold it.
///
/// # Panics
///
/// If `dir` is not a finite direction, or if a crater's term is not finite (a bug).
pub fn material_site<F: FieldView>(
    field: &F,
    class: MaterialClass,
    entry: Option<u8>,
    dir: [f64; 3],
) -> Result<RockSite, QueryHeightError> {
    let header = field.header();
    let figure = header.figure();
    let mechanics = entry
        .and_then(|entry| header.palette().get(entry))
        .map(|entry| entry.mechanics);
    let surface_age = header.surface_age();
    let atmosphere = atmosphere_class(header.surface_pressure());
    let coarse = PatchKey::containing(header.level().get(), dir)
        .expect("a coarse level is below the cube's deepest");
    // The crater index is the cell's: a view without the cell would list none.
    if field.cell(coarse).is_none() {
        return Err(QueryHeightError::NotSurveyed(coarse));
    }
    let p = figure.point(dir);
    let mut best: Option<(f64, CraterEjecta)> = None;
    for crater in field.craters_reaching(coarse) {
        if crater.age.value() > surface_age.value() {
            continue;
        }
        let centre = figure.point(crater.centre);
        let delta = [0, 1, 2].map(|k| p[k] - centre[k]);
        let d2 = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
        let reach = crater_shape(header.craters(), crater.diameter)
            .reach()
            .value();
        if d2 >= reach * reach {
            continue;
        }
        let ejecta = CraterEjecta::new(crater.diameter * 0.5, Metres::new(d2.sqrt()), crater.age);
        let k = ejecta_abundance(&ejecta, atmosphere);
        // The first of equal terms, in the field's list order, is kept.
        if best.is_none_or(|(kept, _)| k > kept) {
            best = Some((num::assert_finite(k), ejecta));
        }
    }
    Ok(RockSite::new(
        class,
        mechanics,
        surface_age,
        atmosphere,
        best.and_then(|(k, ejecta)| (k > 0.0).then_some(ejecta)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use crate::synth::test_support::{SEED, world};
    use crate::testing::SyntheticWorld;

    const MARS_PRESSURE: Pascals = Pascals::new(610.0);

    fn site(class: MaterialClass, age: f64, atmosphere: AtmosphereClass) -> RockSite {
        RockSite::new(
            class,
            Some(MechanicsFamily::Silicate),
            Gigayears::new(age),
            atmosphere,
            None,
        )
    }

    /// The landers' sites, as the tests build them: Viking 1's rocky plain in Chryse Planitia as
    /// soil, Viking 2's in Utopia Planitia as volcanic rock and Pathfinder's flood-scoured plain in
    /// Ares Vallis as bedrock, each on a Hesperian surface under Mars's air. Their k: Viking 1
    /// 0.06–0.09, Viking 2 0.15–0.20 and Pathfinder 0.16–0.20: Viking 1 8% without its outcrops and
    /// Viking 2 about 20% (Golombek and Rapp 1997, JGR 102(E2), 4117, the abstract), Viking 2's
    /// rocks over 3.5 cm about 16% (Moore and Keller's count, as Herkenhoff et al. 2008 quote it),
    /// and Pathfinder 19% in all (Golombek et al. 2003, JGR 108(E12), 8086, the abstract).
    #[test]
    fn the_landers_sites_have_their_measured_abundance() {
        let mars = atmosphere_class(MARS_PRESSURE);
        assert_eq!(mars, AtmosphereClass::Atmosphere);
        for (name, class, age, low, high) in [
            ("Viking 1", MaterialClass::Soil, 3.5, 0.06, 0.09),
            ("Viking 2", MaterialClass::VolcanicRock, 3.2, 0.15, 0.20),
            ("Pathfinder", MaterialClass::Bedrock, 3.5, 0.16, 0.20),
        ] {
            let k = rock_abundance(&site(class, age, mars)).k();
            assert!(
                (low..=high).contains(&k),
                "{name}: k = {k}, not in {low}–{high}"
            );
        }
    }

    /// Mature mare regolith sits at Design note 4's 0.03–0.05, set for the authoritative sizes.
    #[test]
    fn mature_mare_regolith_sits_in_its_range() {
        let k = rock_abundance(&site(
            MaterialClass::Regolith,
            3.5,
            AtmosphereClass::Airless,
        ))
        .k();
        assert!((0.03..=0.05).contains(&k), "{k}");
    }

    /// k is at most 0.40 everywhere, at every form, family, age and atmosphere, with the freshest
    /// crater's ejecta at its rim.
    #[test]
    fn the_abundance_is_bounded() {
        let families = [
            None,
            Some(MechanicsFamily::Silicate),
            Some(MechanicsFamily::Metal),
            Some(MechanicsFamily::WaterIce),
            Some(MechanicsFamily::VolatileIce),
            Some(MechanicsFamily::Salt),
            Some(MechanicsFamily::Organic),
        ];
        let rim = CraterEjecta::new(Metres::new(500.0), Metres::new(500.0001), Gigayears::ZERO);
        for code in 0..=u8::MAX {
            let Ok(class) = MaterialClass::try_from(code) else {
                continue;
            };
            for mechanics in families {
                for atmosphere in [
                    AtmosphereClass::Airless,
                    AtmosphereClass::Tenuous,
                    AtmosphereClass::Atmosphere,
                ] {
                    for age in [0.0, 0.1, 1.0, 4.5] {
                        for ejecta in [None, Some(rim)] {
                            let s = RockSite::new(
                                class,
                                mechanics,
                                Gigayears::new(age),
                                atmosphere,
                                ejecta,
                            );
                            let k = rock_abundance(&s).k();
                            assert!((0.0..=0.40).contains(&k), "{s:?}: {k}");
                        }
                    }
                }
            }
        }
    }

    /// k never rises with the surface's age at a fixed material, and falls faster on an airless
    /// body than under an atmosphere: below it at every age past zero.
    #[test]
    fn the_abundance_falls_with_age_and_faster_without_air() {
        for class in [
            MaterialClass::Bedrock,
            MaterialClass::Regolith,
            MaterialClass::Soil,
            MaterialClass::Dust,
            MaterialClass::Glacier,
        ] {
            let mut last = [f64::INFINITY; 2];
            for step in 0..=90 {
                let age = 0.05 * f64::from(step);
                let airless = rock_abundance(&site(class, age, AtmosphereClass::Airless)).k();
                let aired = rock_abundance(&site(class, age, AtmosphereClass::Atmosphere)).k();
                assert!(
                    airless <= last[0] && aired <= last[1],
                    "{class:?} at {age} Gyr"
                );
                if step > 0 {
                    assert!(
                        airless < aired,
                        "{class:?} at {age} Gyr: {airless} ≥ {aired}"
                    );
                }
                last = [airless, aired];
            }
        }
        assert!(AtmosphereClass::Tenuous.breaks_down_as_airless());
    }

    /// The header's pressure gives three classes: 0 airless, below 100 Pa tenuous, and from it up an
    /// atmosphere.
    #[test]
    fn the_atmosphere_is_classed_from_the_pressure() {
        assert_eq!(atmosphere_class(Pascals::ZERO), AtmosphereClass::Airless);
        assert_eq!(
            atmosphere_class(Pascals::new(1.0)),
            AtmosphereClass::Tenuous
        );
        assert_eq!(
            atmosphere_class(Pascals::new(99.9)),
            AtmosphereClass::Tenuous
        );
        assert_eq!(
            atmosphere_class(AIRLESS_PRESSURE),
            AtmosphereClass::Atmosphere
        );
        assert_eq!(
            atmosphere_class(Pascals::new(101_325.0)),
            AtmosphereClass::Atmosphere
        );
    }

    /// No rock stands on a seabed, sea ice or sand, whatever crater is near.
    #[test]
    fn seabed_sea_ice_and_sand_bear_no_rocks() {
        let rim = CraterEjecta::new(Metres::new(500.0), Metres::new(600.0), Gigayears::ZERO);
        for class in [
            MaterialClass::Seabed,
            MaterialClass::SeaIce,
            MaterialClass::Sand,
        ] {
            for ejecta in [None, Some(rim)] {
                let s = RockSite::new(
                    class,
                    Some(MechanicsFamily::Silicate),
                    Gigayears::ZERO,
                    AtmosphereClass::Atmosphere,
                    ejecta,
                );
                assert!(rock_abundance(&s).is_none(), "{class:?}");
            }
        }
    }

    /// The ejecta add 0.3 just outside a fresh rim, fall by e over each rim radius, nothing inside
    /// the rim, and fade with the crater's age.
    #[test]
    fn a_craters_ejecta_add_blocks_outside_its_rim() {
        let air = AtmosphereClass::Airless;
        let at = |d: f64, t: f64| {
            ejecta_abundance(
                &CraterEjecta::new(Metres::new(1_000.0), Metres::new(d), Gigayears::new(t)),
                air,
            )
        };
        assert!((at(1_000.0 + 1e-9, 0.0) - 0.3).abs() < 1e-12);
        assert!((at(2_000.0, 0.0) - 0.3 * math::exp(-1.0)).abs() < 1e-15);
        assert_eq!(at(999.0, 0.0), 0.0);
        assert!(at(1_500.0, 1.0) < 0.05 * at(1_500.0, 0.0));
    }

    /// A site within a coarse crater's reach, outside its rim, takes the largest ejecta of the
    /// craters reaching it, and old ejecta keep next to none.
    #[test]
    fn a_site_takes_the_largest_craters_ejecta_about_it() {
        let field = world(SyntheticWorld::MoonLike);
        let header = field.header();
        let figure = header.figure();
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let mut seen = 0;
        for crater in field.craters().iter().take(8) {
            // A point 1.5 rim radii from the centre, along a tangent of its own components.
            let centre = crater.centre;
            let tangent = crate::cube::unit_dir([-centre[1], centre[0], 0.3]);
            let along = [0, 1, 2].map(|k| {
                tangent[k] - centre[k] * tangent.iter().zip(centre).map(|(t, c)| t * c).sum::<f64>()
            });
            let along = crate::cube::unit_dir(along);
            let radius = header.figure().equatorial_radius_m;
            let angle = 0.75 * crater.diameter.value() / radius;
            let dir = crate::cube::unit_dir([0, 1, 2].map(|k| centre[k] + angle * along[k]));
            let site = rock_site(&synth, &mut cache, &figure.point(dir)).expect("a whole field");
            let Some(ejecta) = site.ejecta() else {
                continue;
            };
            seen += 1;
            let added = ejecta_abundance(&ejecta, site.atmosphere());
            assert!(added > 0.0);
            // No crater reaching the site adds more.
            let coarse = PatchKey::containing(header.level().get(), dir).unwrap();
            for other in field.craters_reaching(coarse) {
                if other.age > header.surface_age() {
                    continue;
                }
                let centre_m = figure.point(other.centre);
                let at_m = figure.point(dir);
                let d = (0..3)
                    .map(|i| (at_m[i] - centre_m[i]) * (at_m[i] - centre_m[i]))
                    .sum::<f64>()
                    .sqrt();
                let reach = crater_shape(header.craters(), other.diameter)
                    .reach()
                    .value();
                if d < reach {
                    let e = CraterEjecta::new(other.diameter * 0.5, Metres::new(d), other.age);
                    let other_k = ejecta_abundance(&e, site.atmosphere());
                    // The site's distance is from its foot, a rounding away.
                    assert!(
                        other_k <= added * (1.0 + 1e-12),
                        "{other_k} > {added}: {e:?}"
                    );
                }
            }
        }
        assert!(seen > 0, "no site lay in a crater's ejecta");
        let old = CraterEjecta::new(Metres::new(1e3), Metres::new(1.5e3), Gigayears::new(9.0));
        assert!(ejecta_abundance(&old, AtmosphereClass::Airless) < 1e-12);
    }

    /// A site read from a world: its class is the classifier's, its age and atmosphere the
    /// header's, and the Moon-like world's sites are airless; a craterless flat world has no
    /// ejecta.
    #[test]
    fn a_site_reads_the_classifier_and_the_header() {
        for w in [SyntheticWorld::MoonLike, SyntheticWorld::Flat] {
            let field = world(w);
            let synth = Synthesiser::new(field, SEED);
            let mut cache = SynthCache::new();
            let figure = field.header().figure();
            for dir in [[1.0, 0.0, 0.0], [0.0, 0.6, 0.8], [-0.36, 0.48, -0.8]] {
                let p = figure.point(dir);
                let s = rock_site(&synth, &mut cache, &p).expect("a whole field");
                let material = material_at(&synth, &mut cache, xyz_to_face_uv(dir)).unwrap();
                assert_eq!(s.class(), material.class());
                assert_eq!(s.surface_age(), field.header().surface_age());
                assert_eq!(s.atmosphere(), AtmosphereClass::Airless);
                if w == SyntheticWorld::Flat {
                    assert_eq!(s.ejecta(), None);
                }
            }
        }
    }
}
