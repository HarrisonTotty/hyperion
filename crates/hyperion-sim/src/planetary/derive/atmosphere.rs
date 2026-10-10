//! Atmospheres: a body's volatile inventory, what escapes it, and the surface state and
//! temperature its greenhouse gives (plan 14, P14.T13).
//!
//! Every piece is a closed form of plain quantities, so that it can be checked on Solar System
//! values without a generator, and [`derive_body`](super::derive_body) assembles them:
//!
//! - **Inventory** (P14.T13.a). A hydrogen and helium envelope is not drawn here: it is the one the
//!   composition solve gave the body's radius rank (design note 8). What is drawn, on
//!   `planet.volatiles` ([`VolatileDraws`]), is the body's water, carbon and nitrogen, each a
//!   log-normal multiple of Earth's per unit mass ([`volatile_inventory`]), raised beyond the snow
//!   line where ices condensed, where a body's water is its bulk ice; a body that migrated keeps the
//!   inventory of the side it formed on. Radiogenic argon grows with age from the decay of ⁴⁰K.
//! - **Escape** (P14.T13.b). A gas is retained over the system's age when its Jeans parameter λ =
//!   G M m ÷ (k `T_exo` R) is at least [`JEANS_RETENTION_THRESHOLD`], with the exobase temperature
//!   a stated multiple of the equilibrium temperature at the host's largest past luminosity,
//!   raised by the X-ray and ultraviolet energy the body has received beyond Earth's
//!   ([`exobase_temperature`]). A hydrogen envelope, far too massive for Jeans escape to remove, is
//!   judged by energy-limited escape instead ([`energy_limited_loss`]; Owen and Wu 2017).
//! - **Greenhouse and surface state** (P14.T13.c). The retained gases, less what the climate
//!   stores (carbon in rocks where oceans weather it, water lost in a runaway greenhouse) and what
//!   condenses at the surface temperature, give the partial pressures from the body's gravity; a
//!   grey atmosphere warms the surface to `T_s` = `T_eq` (1 + ¾τ)^¼ ([`atmosphere`]). The state,
//!   and from it the Bond albedo and cloud fraction that the equilibrium temperature reads next,
//!   follow ([`SurfaceState`]).
//!
//! The figures the plan names are built as it names them. The ones it leaves to this task are
//! the lane's, each fixed by the Solar System and marked provisional on its constant: the
//! exobase multiple, the greenhouse constants and their exponents, the inventories, the albedos by
//! state, the carbon the oceans store and the pressure below which a body is airless.
//!
//! The irradiation this module reads is the equilibrium temperature from the hosts' light alone;
//! a giant's internal heat enters only its gas envelope's temperature, so that the two can be
//! told apart (ruling 112.7).

use crate::Seed;
use crate::id::BodyId;
use crate::math;
use crate::planetary::context::XuvHistory;
use crate::planetary::derive::composition::{MassFractions, SnowLineSide};
use crate::planetary::derive::irradiation::BondAlbedo;
use crate::planetary::params::{ICY_WATER_FRACTION, THIN_ENVELOPE_FRACTION};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::substance::saturation_pressure;
use crate::units::consts::{
    BOLTZMANN_CONSTANT, EARTH_MASS_KG, GRAVITATIONAL_CONSTANT, METRES_PER_AU,
};
use crate::units::{
    EarthMasses, JoulesPerSquareMetre, Kelvin, Kilograms, Metres, Pascals, SolarLuminosities,
    SolarMasses, Years,
};

/// The atomic mass constant, kg (CODATA 2022: 1.660 539 068 92 × 10⁻²⁷ kg), in which molecular
/// masses are counted.
pub const ATOMIC_MASS_CONSTANT_KG: f64 = 1.660_539_068_92e-27;

pub use crate::substance::MOLAR_GAS_CONSTANT;

/// One bar, Pa, the unit the greenhouse constants are fitted in.
const PASCALS_PER_BAR: f64 = 1e5;

/// The species whose escape an atmosphere is judged on (P14.T13.b): the typed view of the substance
/// registry's rows 0–8, which are these nine in [`Gas::ALL`]'s order ([`Gas::substance`],
/// [`SubstanceId::gas`](crate::substance::SubstanceId::gas)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Gas {
    /// Molecular hydrogen, H₂.
    Hydrogen,
    /// Helium, He.
    Helium,
    /// Water vapour, H₂O.
    Water,
    /// Methane, CH₄.
    Methane,
    /// Ammonia, NH₃.
    Ammonia,
    /// Molecular nitrogen, N₂.
    Nitrogen,
    /// Molecular oxygen, O₂.
    Oxygen,
    /// Carbon dioxide, CO₂.
    CarbonDioxide,
    /// Argon, Ar (radiogenic ⁴⁰Ar).
    Argon,
}

impl Gas {
    /// Every species, lightest first as listed.
    pub const ALL: [Self; 9] = [
        Self::Hydrogen,
        Self::Helium,
        Self::Water,
        Self::Methane,
        Self::Ammonia,
        Self::Nitrogen,
        Self::Oxygen,
        Self::CarbonDioxide,
        Self::Argon,
    ];

    /// The species' molar mass, g mol⁻¹: its registry row's (IUPAC 2021 standard atomic weights,
    /// abridged).
    ///
    /// # Panics
    ///
    /// Never: each of the nine rows holds a molar mass (a test of the registry checks).
    #[must_use]
    pub fn molar_mass_g_per_mol(self) -> f64 {
        self.substance()
            .substance()
            .molar_mass_g_per_mol()
            .expect("a gas row holds its molar mass")
    }

    /// The mass of one molecule.
    #[must_use]
    pub fn molecular_mass(self) -> Kilograms {
        Kilograms::new(self.molar_mass_g_per_mol() * ATOMIC_MASS_CONSTANT_KG)
    }

    /// The species' chemical formula.
    #[must_use]
    pub const fn formula(self) -> &'static str {
        match self {
            Self::Hydrogen => "H₂",
            Self::Helium => "He",
            Self::Water => "H₂O",
            Self::Methane => "CH₄",
            Self::Ammonia => "NH₃",
            Self::Nitrogen => "N₂",
            Self::Oxygen => "O₂",
            Self::CarbonDioxide => "CO₂",
            Self::Argon => "Ar",
        }
    }

    /// The species' place in [`ALL`](Self::ALL), its registry row's index, by which the nine-wide
    /// [`PartialPressures`] and [`Retention`] are indexed.
    #[must_use]
    const fn index(self) -> usize {
        // A u16 widens to usize on every target; `usize::from` is not callable in a const fn.
        self.substance().index() as usize
    }
}

/// The Jeans parameter λ = G M m ÷ (k `T_exo` R) of `gas` at the exobase of a body of mass `mass`
/// and radius `radius` at exobase temperature `exobase` (P14.T13.b): the ratio of a molecule's
/// gravitational binding to its thermal energy. The exobase is taken at the surface radius.
///
/// Infinite for an exobase that is not positive; zero for a body of no mass.
///
/// # Examples
///
/// Earth's nitrogen is bound a hundred times over at its 1,000 K exobase, while the Moon cannot
/// hold argon at its sunlit surface's 390 K for long:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::{Gas, jeans_parameter};
/// use hyperion_sim::units::{Kelvin, Kilograms, Metres};
///
/// let earth = jeans_parameter(Kilograms::new(5.972e24), Metres::new(6.371e6), Gas::Nitrogen, Kelvin::new(1_000.0));
/// assert!(earth > 200.0);
/// let moon = jeans_parameter(Kilograms::new(7.342e22), Metres::new(1.737e6), Gas::Argon, Kelvin::new(390.0));
/// assert!(moon < 40.0);
/// ```
#[must_use]
pub fn jeans_parameter(mass: Kilograms, radius: Metres, gas: Gas, exobase: Kelvin) -> f64 {
    let thermal = BOLTZMANN_CONSTANT * exobase.value() * radius.value();
    if thermal.is_nan() || thermal <= 0.0 {
        return f64::INFINITY;
    }
    GRAVITATIONAL_CONSTANT * mass.value() * gas.molecular_mass().value() / thermal
}

/// The Jeans parameter above which a species is retained over a system's age: 25 (plan 14,
/// P14.T13.b, "a threshold near 25"), fixed by the Solar System table with
/// [`EXOBASE_MULTIPLE`]: Ganymede's nitrogen, at λ ≈ 23, is lost, and Titan's, at λ ≈ 31, kept.
pub const JEANS_RETENTION_THRESHOLD: f64 = 25.0;

/// The exobase temperature as a multiple of the equilibrium temperature: 5 (the lane's, fixed by
/// the Solar System table; provisional).
///
/// It is larger than a real exobase's ratio (Earth's 1,000 K is about 4 `T_eq`, Titan's 150–180 K
/// about 2), because Jeans escape alone must stand in for the non-thermal losses, sputtering and
/// pick-up ions, that empty the Moon and Mercury. Within the table it has a window of 4.6–5.5:
/// below it Ganymede keeps its nitrogen, above it Titan loses its own.
pub const EXOBASE_MULTIPLE: f64 = 5.0;

/// The index by which the X-ray and ultraviolet energy a body has received beyond Earth's raises
/// its exobase temperature: `T_exo` ∝ (Φ ÷ Φ⊕)^0.5 above Φ⊕ (the lane's, provisional; the plan's
/// "rises with the host's activity"). Tian et al. (2008, JGR 113, E05008) find Earth's
/// thermosphere heating steeply with the ultraviolet flux; the square root keeps a planet in an M
/// dwarf's habitable zone, with some 8 times Earth's fluence, below its hydrodynamic regime.
pub const EXOBASE_XUV_INDEX: f64 = 0.5;

/// The age at which Earth's X-ray and ultraviolet fluence is taken as the reference, yr: the
/// Sun's, 4.57 Gyr.
pub const EARTH_REFERENCE_AGE: Years = Years::new(4.57e9);

/// The zero-age Sun's luminosity at which Earth's reference fluence is taken, L☉: 0.70 (plan 06's
/// zero-age main sequence of 1 M☉ at solar composition, Tout et al. 1996, gives 0.698).
pub const REFERENCE_SUN_ZAMS_LUMINOSITY: SolarLuminosities = SolarLuminosities::new(0.70);

/// The X-ray and ultraviolet energy per unit area Earth has received from the Sun by its present
/// age ([`XuvHistory`] of 1 M☉ at [`REFERENCE_SUN_ZAMS_LUMINOSITY`], at 1 au on a circular orbit):
/// about 2.6 × 10¹⁵ J m⁻², the unit of the exposure the exobase reads.
#[must_use]
pub fn earth_xuv_fluence() -> JoulesPerSquareMetre {
    XuvHistory::new(SolarMasses::new(1.0), REFERENCE_SUN_ZAMS_LUMINOSITY).fluence(
        EARTH_REFERENCE_AGE,
        Metres::new(METRES_PER_AU),
        0.0,
    )
}

/// The exobase temperature of a body whose equilibrium temperature at its hosts' largest past
/// luminosity is `worst_equilibrium` and which has received the X-ray and ultraviolet fluence
/// `fluence`: [`EXOBASE_MULTIPLE`] × `T_eq` × max(1, Φ ÷ Φ⊕)^[`EXOBASE_XUV_INDEX`] (P14.T13.b).
///
/// Both inputs only grow with time (design note 11: the worst the body has seen), so the exobase
/// never cools and a gas once lost stays lost.
#[must_use]
pub fn exobase_temperature(worst_equilibrium: Kelvin, fluence: JoulesPerSquareMetre) -> Kelvin {
    let exposure = fluence.value() / earth_xuv_fluence().value();
    let boost = if exposure > 1.0 {
        math::powf(exposure, EXOBASE_XUV_INDEX)
    } else {
        1.0
    };
    Kelvin::new(EXOBASE_MULTIPLE * worst_equilibrium.value() * boost)
}

/// Which species a body retains over its system's age: those whose Jeans parameter at its
/// exobase is at least [`JEANS_RETENTION_THRESHOLD`] (P14.T13.b).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Retention(u16);

impl Retention {
    /// The retention of a body of mass `mass` and radius `radius` at exobase temperature
    /// `exobase`.
    #[must_use]
    pub fn of(mass: Kilograms, radius: Metres, exobase: Kelvin) -> Self {
        let bits = Gas::ALL.iter().fold(0_u16, |bits, &gas| {
            if jeans_parameter(mass, radius, gas, exobase) >= JEANS_RETENTION_THRESHOLD {
                bits | 1 << gas.index()
            } else {
                bits
            }
        });
        Self(bits)
    }

    /// Whether `gas` is retained.
    #[must_use]
    pub const fn retains(self, gas: Gas) -> bool {
        self.0 & (1 << gas.index()) != 0
    }
}

/// The efficiency of energy-limited escape, the share of the absorbed X-ray and ultraviolet
/// energy that lifts gas out of the potential: 0.1 (plan 14, P14.T13.b; Owen and Wu 2017, ApJ 847,
/// 29, §3.1, whose constant η is 0.1).
pub const ESCAPE_EFFICIENCY: f64 = 0.1;

/// The age at which an envelope's radius is read for its escape, Gyr: 100 Myr, the youngest of
/// Lopez and Fortney's (2014) tables, when the host is saturated and nearly all the loss happens.
/// A fixed age keeps the loss a closed form, monotone in time (design note 9).
pub const ENVELOPE_LOSS_RADIUS_AGE: Years = Years::new(1.0e8);

/// Erkaev et al.'s (2007, A&A 472, 329) correction for the Roche lobe, the share of the potential
/// depth that escaping gas must climb: K(ξ) = 1 − 3 ÷ (2ξ) + 1 ÷ (2ξ³), with ξ the Roche-lobe (Hill)
/// radius over the planet's radius (eq. 11 of arXiv:astro-ph/0612729, (ξ − 1)² (2ξ + 1) ÷ (2ξ³),
/// expanded). ξ is held above 1, where a planet overflowing its lobe loses its envelope in any
/// case.
#[must_use]
pub fn roche_lobe_factor(hill_radius: Metres, radius: Metres) -> f64 {
    let xi = (hill_radius.value() / radius.value()).max(1.0 + 1e-3);
    if xi.is_nan() {
        return 1.0;
    }
    1.0 - 1.5 / xi + 0.5 / (xi * xi * xi)
}

/// The mass a hydrogen envelope loses by energy-limited escape (P14.T13.b; Owen and Wu 2017,
/// eq. 19, integrated over the fluence, with Erkaev et al.'s Roche-lobe factor, which Owen and Wu
/// leave out, joined by the lane): ε π R³ Φ ÷ (G M K), for a planet of mass `mass` and radius `radius` that has received
/// the X-ray and ultraviolet fluence `fluence`, with Hill radius `hill_radius` for the Roche-lobe
/// factor K ([`roche_lobe_factor`]) and ε [`ESCAPE_EFFICIENCY`].
///
/// # Examples
///
/// A 5 M⊕ core with a 2% envelope, 2.6 R⊕ across, at 0.05 au of a Sun loses more than the
/// envelope's 0.1 M⊕ within its first gigayear; at 0.5 au it loses about a hundredth of that:
///
/// ```
/// use hyperion_sim::planetary::context::XuvHistory;
/// use hyperion_sim::planetary::derive::atmosphere::energy_limited_loss;
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M, METRES_PER_AU};
/// use hyperion_sim::units::{Kilograms, Metres, SolarLuminosities, SolarMasses, Years};
///
/// let sun = XuvHistory::new(SolarMasses::new(1.0), SolarLuminosities::new(0.7));
/// let (mass, radius) = (Kilograms::new(5.0 * EARTH_MASS_KG), Metres::new(2.6 * EARTH_RADIUS_M));
/// let loss = |au: f64| {
///     let a = Metres::new(au * METRES_PER_AU);
///     let hill = Metres::new(a.value() * hyperion_sim::math::cbrt(5.0 * 3.0e-6 / 3.0));
///     energy_limited_loss(mass, radius, sun.fluence(Years::new(1e9), a, 0.0), hill).value()
/// };
/// assert!(loss(0.05) > 0.1 * EARTH_MASS_KG);
/// assert!(loss(0.5) < 0.002 * EARTH_MASS_KG);
/// ```
#[must_use]
pub fn energy_limited_loss(
    mass: Kilograms,
    radius: Metres,
    fluence: JoulesPerSquareMetre,
    hill_radius: Metres,
) -> Kilograms {
    let (m, r) = (mass.value(), radius.value());
    if !(m.is_finite() && m > 0.0 && r.is_finite() && r > 0.0) {
        return Kilograms::ZERO;
    }
    let k = roche_lobe_factor(hill_radius, radius);
    Kilograms::new(
        ESCAPE_EFFICIENCY * core::f64::consts::PI * r * r * r * fluence.value()
            / (GRAVITATIONAL_CONSTANT * m * k),
    )
}

/// Earth's surface water per unit of its mass: 2.3 × 10⁻⁴, its oceans' 1.37 × 10²¹ kg (Charette
/// and Smith 2010, Oceanography 23(2), 112) with its ice and ground water, over 5.97 × 10²⁴ kg.
pub const EARTH_WATER_PER_MASS: f64 = 2.3e-4;

/// Earth's near-surface carbon per unit of its mass, counted as carbon dioxide: 6.1 × 10⁻⁵, the
/// crust's and sediments' 1.0 × 10²⁰ kg of carbon (Sleep and Zahnle 2001, JGR 106, 1373; Hayes
/// and Waldbauer 2006, Phil. Trans. B 361, 931), the carbonates that, released, would give
/// Venus-like tens of bars (provisional).
pub const EARTH_CARBON_DIOXIDE_PER_MASS: f64 = 6.1e-5;

/// Earth's atmospheric nitrogen per unit of its mass: 6.5 × 10⁻⁷, its 3.87 × 10¹⁸ kg of N₂.
pub const EARTH_NITROGEN_PER_MASS: f64 = 6.5e-7;

/// Earth's atmospheric argon per unit of its rock at [`EARTH_REFERENCE_AGE`]: 1.64 × 10⁻⁸, its
/// 6.6 × 10¹⁶ kg, nearly all radiogenic ⁴⁰Ar, over the 67.5% of its mass that is rock (potassium,
/// its parent, is lithophile, so the iron core holds none).
pub const EARTH_ARGON_PER_ROCK_MASS: f64 = 1.64e-8;

/// The half-life of ⁴⁰K, yr: 1.248 Gyr (Kossert and Günther 2004, Appl. Radiat. Isot. 60, 459;
/// NUBASE2020), whose decay makes the argon.
pub const POTASSIUM_40_HALF_LIFE: Years = Years::new(1.248e9);

/// How much richer in carbon and nitrogen a body formed beyond the snow line is than Earth, per
/// unit mass: 100, from Titan's 9 × 10¹⁸ kg of nitrogen over its 1.35 × 10²³ kg, about 105 times
/// Earth's share (the lane's, provisional). Its ices carried ammonia and carbon that the inner
/// disc never condensed (the plan's "raised beyond the snow line").
pub const ICY_VOLATILE_BOOST: f64 = 100.0;

/// The log-normal scatter of each volatile's inventory about its median, dex: 0.5 (the lane's,
/// provisional). Earth, Venus and Mars differ in nitrogen and carbon per unit mass by factors of a
/// few, which half a decade of scatter spans at one sigma.
pub const VOLATILE_SCATTER_DEX: f64 = 0.5;

/// Words of `planet.volatiles` a planet reads or reserves: words 0–2, the ranks of its water,
/// carbon and nitrogen, then five reserved.
pub const VOLATILE_WORDS: u64 = 8;

/// The ranks of a body's volatile inventory, as drawn from `planet.volatiles` by
/// [`VolatileDraws::for_body`], or given explicitly by a test or a tool (P14.T13.a).
///
/// The fields are plain ranks with no invariant between them, so they are public, as the disc's
/// draws are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolatileDraws {
    /// The rank of the body's water per unit mass. Word 0.
    pub water: UnitUniform,
    /// The rank of its carbon. Word 1.
    pub carbon: UnitUniform,
    /// The rank of its nitrogen. Word 2.
    pub nitrogen: UnitUniform,
}

impl VolatileDraws {
    /// Every rank at its median: Earth's inventory per unit mass.
    pub const MEDIAN: Self = Self {
        water: UnitUniform::HALF,
        carbon: UnitUniform::HALF,
        nitrogen: UnitUniform::HALF,
    };

    /// The draws of the body `body` in the universe of `seed`: words 0–2 of its
    /// `planet.volatiles` stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::id::{BodyId, SystemId};
    /// use hyperion_sim::planetary::derive::atmosphere::VolatileDraws;
    ///
    /// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
    /// let (b, c) = (BodyId::new(system, 0x0100), BodyId::new(system, 0x0200));
    /// let seed = Seed::new(7);
    /// assert_eq!(VolatileDraws::for_body(seed, b), VolatileDraws::for_body(seed, b));
    /// assert_ne!(VolatileDraws::for_body(seed, b), VolatileDraws::for_body(seed, c));
    /// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
    /// ```
    ///
    /// # Panics
    ///
    /// Never: an open uniform lies strictly between 0 and 1.
    #[must_use]
    pub fn for_body(seed: Seed, body: BodyId) -> Self {
        let mut stream = Stream::open(seed, tags::PLANET_VOLATILES, ObjectKey::from(body));
        let mut rank = || {
            UnitUniform::new(stream.uniform_open())
                .expect("an open uniform lies strictly between 0 and 1")
        };
        let water = rank();
        let carbon = rank();
        let nitrogen = rank();
        Self {
            water,
            carbon,
            nitrogen,
        }
    }
}

/// A body's volatile inventory: the masses of water, carbon dioxide, nitrogen and argon it holds
/// at its surface and in its air, before escape and the climate divide them (P14.T13.a).
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct VolatileInventory {
    water: Kilograms,
    carbon_dioxide: Kilograms,
    nitrogen: Kilograms,
    argon: Kilograms,
}

impl VolatileInventory {
    /// An inventory of the masses given, for a test or a tool; a value that is negative or not
    /// finite is taken as none.
    #[must_use]
    pub fn new(
        water: Kilograms,
        carbon_dioxide: Kilograms,
        nitrogen: Kilograms,
        argon: Kilograms,
    ) -> Self {
        let held = |x: Kilograms| {
            if x.value().is_finite() && x.value() > 0.0 {
                x
            } else {
                Kilograms::ZERO
            }
        };
        Self {
            water: held(water),
            carbon_dioxide: held(carbon_dioxide),
            nitrogen: held(nitrogen),
            argon: held(argon),
        }
    }

    /// The water.
    #[must_use]
    pub const fn water(&self) -> Kilograms {
        self.water
    }

    /// The carbon, as carbon dioxide.
    #[must_use]
    pub const fn carbon_dioxide(&self) -> Kilograms {
        self.carbon_dioxide
    }

    /// The nitrogen, as N₂.
    #[must_use]
    pub const fn nitrogen(&self) -> Kilograms {
        self.nitrogen
    }

    /// The radiogenic argon.
    #[must_use]
    pub const fn argon(&self) -> Kilograms {
        self.argon
    }
}

/// The volatile inventory of a body of mass `mass` and bulk composition `fractions`, formed on
/// `formed`'s side of the snow line, with ranks `draws`, at age `age` (P14.T13.a).
///
/// - Water, carbon and nitrogen are each Earth's per unit mass ([`EARTH_WATER_PER_MASS`],
///   [`EARTH_CARBON_DIOXIDE_PER_MASS`], [`EARTH_NITROGEN_PER_MASS`]) times a log-normal multiple
///   of median 1 and scatter [`VOLATILE_SCATTER_DEX`], at the body's rank.
/// - Beyond the snow line carbon and nitrogen are [`ICY_VOLATILE_BOOST`] times richer, and the
///   water is the larger of that and the body's bulk water, its ice.
/// - Argon is Earth's per unit of rock, grown as the ⁴⁰K of a body of the Sun's age has
///   decayed: Earth's × (1 − e^(−λt)) ÷ (1 − e^(−λ × 4.57 Gyr)).
#[must_use]
pub fn volatile_inventory(
    mass: EarthMasses,
    fractions: &MassFractions,
    formed: SnowLineSide,
    draws: &VolatileDraws,
    age: Years,
) -> VolatileInventory {
    let kg = mass.value() * EARTH_MASS_KG;
    let multiple =
        |rank: UnitUniform| math::exp10(VOLATILE_SCATTER_DEX * math::normal_quantile(rank.value()));
    let boost = match formed {
        SnowLineSide::Inside => 1.0,
        SnowLineSide::Beyond => ICY_VOLATILE_BOOST,
    };
    let drawn_water = EARTH_WATER_PER_MASS * multiple(draws.water) * kg;
    let water = match formed {
        SnowLineSide::Inside => drawn_water,
        SnowLineSide::Beyond => drawn_water.max(fractions.water() * kg),
    };
    let carbon = EARTH_CARBON_DIOXIDE_PER_MASS * boost * multiple(draws.carbon) * kg;
    let nitrogen = EARTH_NITROGEN_PER_MASS * boost * multiple(draws.nitrogen) * kg;
    let decay = core::f64::consts::LN_2 / POTASSIUM_40_HALF_LIFE.value();
    let grown = |t: f64| 1.0 - math::exp(-decay * t.max(0.0));
    let argon = EARTH_ARGON_PER_ROCK_MASS * fractions.rock() * kg * grown(age.value())
        / grown(EARTH_REFERENCE_AGE.value());
    VolatileInventory::new(
        Kilograms::new(water),
        Kilograms::new(carbon),
        Kilograms::new(nitrogen),
        Kilograms::new(argon),
    )
}

/// The greenhouse constant of carbon dioxide, per √bar: 18.5, fixed by Venus (P14.T13.c; the lane's
/// fit, provisional). With 57 bar of carbon dioxide at its median inventory it warms Venus's
/// 229 K to 735 K.
pub const GREENHOUSE_CARBON_DIOXIDE: f64 = 18.5;

/// The greenhouse constant of water vapour, per √bar: 2.89, fixed by Earth with its carbon dioxide
/// and nitrogen (P14.T13.c; the lane's fit, provisional).
pub const GREENHOUSE_WATER: f64 = 2.89;

/// The greenhouse constant of nitrogen, per √bar at [`NITROGEN_REFERENCE_TEMPERATURE`]: 1.55,
/// fixed by Titan (P14.T13.c; the lane's fit, provisional). Nitrogen absorbs only when molecules
/// collide, as the square of the density, so the constant falls as (`T_ref` ÷ `T_eq`)², which
/// leaves Earth's nitrogen a tenth of Titan's warming.
pub const GREENHOUSE_NITROGEN: f64 = 1.55;

/// The equilibrium temperature at which [`GREENHOUSE_NITROGEN`] holds, K: Titan's, 75.6, at the
/// snowball albedo.
pub const NITROGEN_REFERENCE_TEMPERATURE: Kelvin = Kelvin::new(75.6);

/// The grey optical depth of partial pressures `partials` over a surface in equilibrium at
/// `equilibrium`: τ = Σ k × (p ÷ 1 bar)^½ over carbon dioxide, water and nitrogen (P14.T13.c), the
/// plan's square root of each partial pressure, nitrogen's scaled for collision-induced
/// absorption.
#[must_use]
pub fn optical_depth(partials: &PartialPressures, equilibrium: Kelvin) -> f64 {
    let root = |gas: Gas| (partials.of(gas).value().max(0.0) / PASCALS_PER_BAR).sqrt();
    let t = equilibrium.value();
    let collisions = if t > 0.0 {
        let ratio = NITROGEN_REFERENCE_TEMPERATURE.value() / t;
        ratio * ratio
    } else {
        0.0
    };
    GREENHOUSE_CARBON_DIOXIDE * root(Gas::CarbonDioxide)
        + GREENHOUSE_WATER * root(Gas::Water)
        + GREENHOUSE_NITROGEN * collisions * root(Gas::Nitrogen)
}

/// The surface temperature of a grey atmosphere of optical depth `tau` over a surface in
/// equilibrium at `equilibrium`: `T_s` = `T_eq` (1 + ¾τ)^¼ (P14.T13.c).
#[must_use]
pub fn grey_surface_temperature(equilibrium: Kelvin, tau: f64) -> Kelvin {
    Kelvin::new(equilibrium.value() * math::powf(1.0 + 0.75 * tau.max(0.0), 0.25))
}

/// The partial pressure of every [`Gas`] at a body's surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct PartialPressures([Pascals; 9]);

impl PartialPressures {
    /// The partial pressure of `gas`.
    #[must_use]
    pub const fn of(&self, gas: Gas) -> Pascals {
        self.0[gas.index()]
    }

    /// Their sum, the surface pressure, added in [`Gas::ALL`]'s order.
    #[must_use]
    pub fn total(&self) -> Pascals {
        Pascals::new(self.0.iter().fold(0.0, |sum, p| sum + p.value()))
    }
}

/// Carbon dioxide's share of a temperate world's carbon left in its air, the rest stored in
/// carbonates by weathering where there is liquid water: 6 × 10⁻⁶, Earth's pre-industrial
/// 2.2 × 10¹⁵ kg in the air over its [`EARTH_CARBON_DIOXIDE_PER_MASS`] (the carbonate–silicate
/// cycle, Walker, Hays and Kasting 1981, JGR 86, 9776, in its simplest form; provisional).
pub const TEMPERATE_AIRBORNE_CARBON: f64 = 6.0e-6;

/// The surface pressure below which a body is airless, Pa: 100 (1 mbar; the lane's). Mars's
/// 600 Pa is an atmosphere; Pluto's and Triton's 1 Pa, the exospheres of Mercury, the Moon and the
/// Galilean moons, and the radiogenic argon of some tens of pascals that an icy moon's rock would
/// give it by the per-mass rule of [`volatile_inventory`] are not.
pub const AIRLESS_PRESSURE: Pascals = Pascals::new(100.0);

/// The solidus of dry peridotite at the surface, K: 1,394 (Hirschmann 2000, G³ 1, 1042, whose fit
/// gives 1,120.7 °C at zero pressure). A surface hotter than this is a magma ocean.
pub const SILICATE_SOLIDUS: Kelvin = Kelvin::new(1_394.0);

/// How many times the surface temperature is iterated against the greenhouse it holds, from the
/// equilibrium temperature upward: 32, which settles on the lowest temperature at which the
/// atmosphere holds itself, to a part in 10⁹ for every Solar System case (tested).
pub const GREENHOUSE_STEPS: u32 = 32;

/// What a body's surface is, which sets its albedo and cloud (P14.T13.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurfaceState {
    /// A hydrogen and helium envelope of at least 0.1% of the mass: no solid surface (a gas giant,
    /// an ice giant or a sub-Neptune).
    GasEnvelope,
    /// Molten rock at the surface: a young terrestrial planet (design note 12), or one hotter than
    /// the [`SILICATE_SOLIDUS`].
    MagmaOcean,
    /// A surface pressure below [`AIRLESS_PRESSURE`]: bare rock or ice.
    Airless,
    /// Inside the runaway-greenhouse limit when water was present: the oceans lost, a thick
    /// carbon-dioxide atmosphere, as on Venus.
    RunawayGreenhouse,
    /// Between the runaway and maximum-greenhouse limits, as on Earth and Mars.
    Temperate,
    /// Beyond the maximum-greenhouse limit: frozen, as Titan is.
    Snowball,
}

/// What an airless surface is made of, which sets its albedo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurfaceMaterial {
    /// Rock and dust.
    Rock,
    /// Ice: a body of at least [`ICY_WATER_FRACTION`] water.
    Ice,
}

impl SurfaceMaterial {
    /// The material of a body of bulk composition `fractions`.
    #[must_use]
    pub fn of(fractions: &MassFractions) -> Self {
        if fractions.water() >= ICY_WATER_FRACTION {
            Self::Ice
        } else {
            Self::Rock
        }
    }
}

impl SurfaceState {
    /// The Bond albedo of the state on a surface of `material` (the lane's table, provisional):
    ///
    /// - a gas envelope 0.34, the giants' 0.29–0.34 (Jupiter's 0.343, Hanel et al. 1981, JGR 86,
    ///   8705; Neptune's 0.29, Pearl and Conrath 1991, JGR 96, 18921; Li et al. 2018, Nat. Commun.
    ///   9, 3709, revise Jupiter's to 0.50);
    /// - a magma ocean 0.10, dark lava (Essack, Seager and Pajusalu 2020, ApJ 898, 160);
    /// - airless rock 0.11, the Moon's Bond albedo (NASA's fact sheet), and airless ice 0.35, the
    ///   lane's for a Galilean moon's dirty ice, below Ganymede's geometric albedo of 0.43;
    /// - a runaway greenhouse 0.76, Venus's (Haus et al. 2016, Icarus 272, 178);
    /// - temperate 0.294, Earth's (NASA's Earth fact sheet, since its update of 11 January 2024;
    ///   CERES's EBAF Ed4.0 gives 0.2915, Loeb et al. 2018, J. Climate 31, 895);
    /// - a snowball 0.50, between bare ice's 0.6 (Pierrehumbert et al. 2011) and Titan's hazy 0.27.
    #[must_use]
    pub const fn albedo(self, material: SurfaceMaterial) -> BondAlbedo {
        BondAlbedo::from_fraction(match self {
            Self::GasEnvelope => 0.34,
            Self::MagmaOcean => 0.10,
            Self::Airless => match material {
                SurfaceMaterial::Rock => 0.11,
                SurfaceMaterial::Ice => 0.35,
            },
            Self::RunawayGreenhouse => 0.76,
            Self::Temperate => 0.294,
            Self::Snowball => 0.50,
        })
    }

    /// The share of the disc covered by cloud (the lane's table, provisional): a gas envelope's
    /// and a runaway greenhouse's whole disc, Earth's 0.67 for a temperate world (King et al.
    /// 2013, IEEE TGRS 51, 3826), and none for the rest.
    #[must_use]
    pub const fn cloud_fraction(self) -> f64 {
        match self {
            Self::GasEnvelope | Self::RunawayGreenhouse => 1.0,
            Self::Temperate => 0.67,
            Self::MagmaOcean | Self::Airless | Self::Snowball => 0.0,
        }
    }
}

/// Where a body lies against its hosts' habitable zone (P14.T12.b), which sets its climate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Insolation {
    /// Inside the runaway-greenhouse limit at the hosts' largest past luminosity: any ocean it had
    /// is lost.
    InsideRunaway,
    /// Between the runaway limit (at the worst) and the maximum-greenhouse limit (now).
    Habitable,
    /// Beyond the maximum-greenhouse limit now.
    BeyondMaximumGreenhouse,
}

/// Whether a terrestrial body's crust has formed (design note 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Crust {
    /// The young magma ocean has not yet cooled: P14.T28.a's `molten_until` lies ahead.
    Molten,
    /// The crust is solid.
    Solid,
}

/// Everything [`atmosphere`] reads of a body at a time, as plain values.
///
/// The fields carry no invariant [`atmosphere`] relies on: a mass or radius that is not positive
/// gives an airless body, so they are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtmosphereInputs {
    /// The body's mass at the time.
    pub mass: Kilograms,
    /// Its radius at the time.
    pub radius: Metres,
    /// What its surface is made of, if it is airless.
    pub material: SurfaceMaterial,
    /// Its hydrogen and helium envelope's share of its mass at the time.
    pub envelope_fraction: f64,
    /// Its volatile inventory ([`volatile_inventory`]).
    pub inventory: VolatileInventory,
    /// Its equilibrium temperature from its hosts' light at the time, at the albedo in use.
    pub equilibrium: Kelvin,
    /// Its equilibrium temperature at its hosts' largest past luminosity, at the same albedo.
    pub worst_equilibrium: Kelvin,
    /// Its temperature with its internal heat added, which a gas envelope radiates at: the
    /// equilibrium temperature for a body with none.
    pub heated: Kelvin,
    /// The X-ray and ultraviolet energy per unit area it has received.
    pub xuv_fluence: JoulesPerSquareMetre,
    /// Where it lies against its hosts' habitable zone.
    pub insolation: Insolation,
    /// Whether its crust has formed.
    pub crust: Crust,
    /// The hottest effective temperature of its luminous hosts, which no surface exceeds.
    pub hottest_host: Kelvin,
}

/// A body's atmosphere and surface at a time (P14.T13).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Atmosphere {
    state: SurfaceState,
    surface_temperature: Kelvin,
    surface_pressure: Option<Pascals>,
    partials: PartialPressures,
    retention: Retention,
    exobase_temperature: Kelvin,
    optical_depth: f64,
    albedo: BondAlbedo,
    cloud_fraction: f64,
}

impl Atmosphere {
    /// The surface state.
    #[must_use]
    pub const fn state(&self) -> SurfaceState {
        self.state
    }

    /// The mean surface temperature: for a gas envelope, the temperature it radiates at.
    #[must_use]
    pub const fn surface_temperature(&self) -> Kelvin {
        self.surface_temperature
    }

    /// The surface pressure, the sum of the partial pressures; `None` for a gas envelope, which
    /// has no surface.
    #[must_use]
    pub const fn surface_pressure(&self) -> Option<Pascals> {
        self.surface_pressure
    }

    /// The partial pressure of each gas at the surface; all zero for a gas envelope.
    #[must_use]
    pub const fn partial_pressures(&self) -> &PartialPressures {
        &self.partials
    }

    /// Which species the body retains against Jeans escape.
    #[must_use]
    pub const fn retention(&self) -> Retention {
        self.retention
    }

    /// The exobase temperature the retention was judged at.
    #[must_use]
    pub const fn exobase_temperature(&self) -> Kelvin {
        self.exobase_temperature
    }

    /// The grey optical depth of the greenhouse.
    #[must_use]
    pub const fn optical_depth(&self) -> f64 {
        self.optical_depth
    }

    /// The Bond albedo that follows from the state.
    #[must_use]
    pub const fn albedo(&self) -> BondAlbedo {
        self.albedo
    }

    /// The share of the disc under cloud.
    #[must_use]
    pub const fn cloud_fraction(&self) -> f64 {
        self.cloud_fraction
    }

    /// Whether the body keeps an atmosphere: every state but [`SurfaceState::Airless`]. "None" is
    /// data, not a missing value: an airless body's atmosphere is this, with its tenuous partial
    /// pressures.
    #[must_use]
    pub fn keeps_atmosphere(&self) -> bool {
        self.state != SurfaceState::Airless
    }
}

/// The masses of each gas a body's climate leaves free to fill its air, before condensation.
#[derive(Debug, Clone, Copy, Default)]
struct Airborne {
    water: f64,
    carbon_dioxide: f64,
    nitrogen: f64,
    argon: f64,
}

/// The surface temperature, partial pressures and optical depth of the airborne masses
/// `airborne`, each weighing `weight` pascals per kilogram at the surface, over a surface in
/// equilibrium at `equilibrium`: the temperature iterated [`GREENHOUSE_STEPS`] times upward from
/// the equilibrium temperature through the grey greenhouse its partial pressures give, each held at
/// its saturation vapour pressure there (the substance registry's
/// [`saturation_pressure`](crate::substance::saturation_pressure)), and held by `saturate` at the
/// hottest host's.
fn greenhouse(
    airborne: &Airborne,
    weight: f64,
    equilibrium: Kelvin,
    saturate: &impl Fn(f64) -> Kelvin,
) -> (f64, PartialPressures, f64) {
    let partials_at = |t: f64| {
        // A gas with no phase data never condenses: its saturation pressure is infinite.
        let held = |mass: f64, gas: Gas| {
            let saturation = saturation_pressure(gas.substance(), Kelvin::new(t))
                .map_or(f64::INFINITY, Pascals::value);
            (mass * weight).min(saturation)
        };
        let mut p = [Pascals::ZERO; 9];
        p[Gas::Water.index()] = Pascals::new(held(airborne.water, Gas::Water));
        p[Gas::CarbonDioxide.index()] =
            Pascals::new(held(airborne.carbon_dioxide, Gas::CarbonDioxide));
        p[Gas::Nitrogen.index()] = Pascals::new(held(airborne.nitrogen, Gas::Nitrogen));
        p[Gas::Argon.index()] = Pascals::new(held(airborne.argon, Gas::Argon));
        PartialPressures(p)
    };
    let mut t = equilibrium.value();
    for _ in 0..GREENHOUSE_STEPS {
        let tau = optical_depth(&partials_at(t), equilibrium);
        t = saturate(grey_surface_temperature(equilibrium, tau).value()).value();
    }
    let partials = partials_at(t);
    let tau = optical_depth(&partials, equilibrium);
    (t, partials, tau)
}

/// The atmosphere of the body `inputs` describe (P14.T13.b–c).
///
/// 1. A body with a hydrogen and helium envelope of at least 0.1% of its mass is a
///    [`SurfaceState::GasEnvelope`] radiating at its heated temperature.
/// 2. Otherwise each inventoried gas is kept if it is retained against Jeans escape at the exobase
///    temperature ([`exobase_temperature`], [`Retention`]).
/// 3. The climate divides what is kept: inside the runaway limit the oceans are lost and all the
///    carbon is airborne; in the habitable zone a world with water stores all but
///    [`TEMPERATE_AIRBORNE_CARBON`] of its carbon in rock; beyond the maximum-greenhouse limit
///    the carbon is airborne, since no ocean weathers it. A molten crust holds nothing back.
/// 4. The partial pressures are each airborne mass's weight over the surface, M g ÷ 4πR², held at
///    each gas's saturation vapour pressure at the surface temperature (water, carbon dioxide,
///    nitrogen and argon condense, by their rows of the substance registry,
///    [`saturation_pressure`](crate::substance::saturation_pressure)), which is iterated
///    [`GREENHOUSE_STEPS`] times upward from the equilibrium temperature through the grey
///    greenhouse they give ([`optical_depth`], [`grey_surface_temperature`]).
/// 5. A surface pressure below [`AIRLESS_PRESSURE`] is [`SurfaceState::Airless`], at the
///    equilibrium temperature; a surface over the [`SILICATE_SOLIDUS`], or a crust not yet formed,
///    a [`SurfaceState::MagmaOcean`] (the young one at least at the solidus); the rest takes the
///    climate's state.
///
/// No surface is hotter than its hottest host: irradiation cannot lift it there, which is
/// thermodynamics, so the temperature saturates at [`AtmosphereInputs::hottest_host`].
///
/// # Examples
///
/// An Earth with its median inventory keeps its air, warms to about 288 K and is temperate. Its
/// equilibrium temperature of 254 K is illustrative: it is Earth's at the earlier temperate albedo
/// of 0.306, and 255 K at today's 0.294.
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::{
///     AtmosphereInputs, Crust, Insolation, SurfaceMaterial, SurfaceState, VolatileDraws,
///     atmosphere, earth_xuv_fluence, volatile_inventory,
/// };
/// use hyperion_sim::planetary::derive::{SnowLineSide, composition};
/// use hyperion_sim::units::{EarthMasses, EarthFluxes, EarthRadii, Kelvin, Kilograms, Metres, Years};
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M};
///
/// let solved = composition(EarthMasses::new(1.0), EarthRadii::new(1.0), SnowLineSide::Inside, EarthFluxes::new(1.0))?;
/// let fractions = solved.fractions();
/// let inventory = volatile_inventory(
///     EarthMasses::new(1.0), &fractions, SnowLineSide::Inside, &VolatileDraws::MEDIAN, Years::new(4.57e9),
/// );
/// let earth = atmosphere(&AtmosphereInputs {
///     mass: Kilograms::new(EARTH_MASS_KG),
///     radius: Metres::new(EARTH_RADIUS_M),
///     material: SurfaceMaterial::of(&fractions),
///     envelope_fraction: 0.0,
///     inventory,
///     equilibrium: Kelvin::new(254.0),
///     worst_equilibrium: Kelvin::new(254.0),
///     heated: Kelvin::new(254.0),
///     xuv_fluence: earth_xuv_fluence(),
///     insolation: Insolation::Habitable,
///     crust: Crust::Solid,
///     hottest_host: Kelvin::new(5_772.0),
/// });
/// assert_eq!(earth.state(), SurfaceState::Temperate);
/// assert!((earth.surface_temperature().value() - 288.0).abs() < 2.0);
/// # Ok::<(), hyperion_sim::planetary::derive::composition::SolveCompositionError>(())
/// ```
#[must_use]
pub fn atmosphere(inputs: &AtmosphereInputs) -> Atmosphere {
    let cap = inputs.hottest_host.value();
    let saturate = |t: f64| Kelvin::new(if cap > 0.0 { t.min(cap) } else { t });
    let exobase = exobase_temperature(inputs.worst_equilibrium, inputs.xuv_fluence);
    let (m, r) = (inputs.mass.value(), inputs.radius.value());
    let valid = m.is_finite() && m > 0.0 && r.is_finite() && r > 0.0;
    let retention = if valid {
        Retention::of(inputs.mass, inputs.radius, exobase)
    } else {
        Retention::default()
    };
    let airless = |state: SurfaceState, partials: PartialPressures, t: f64| Atmosphere {
        state,
        surface_temperature: saturate(t),
        surface_pressure: Some(partials.total()),
        partials,
        retention,
        exobase_temperature: exobase,
        optical_depth: 0.0,
        albedo: state.albedo(inputs.material),
        cloud_fraction: state.cloud_fraction(),
    };
    if inputs.envelope_fraction >= THIN_ENVELOPE_FRACTION {
        let state = SurfaceState::GasEnvelope;
        return Atmosphere {
            surface_pressure: None,
            ..airless(state, PartialPressures::default(), inputs.heated.value())
        };
    }
    if !valid {
        return airless(
            SurfaceState::Airless,
            PartialPressures::default(),
            inputs.equilibrium.value(),
        );
    }

    // Step 2: what escape leaves.
    let kept = |gas: Gas, mass: Kilograms| {
        if retention.retains(gas) {
            mass.value()
        } else {
            0.0
        }
    };
    let inventory = &inputs.inventory;
    let mut airborne = Airborne {
        water: kept(Gas::Water, inventory.water()),
        carbon_dioxide: kept(Gas::CarbonDioxide, inventory.carbon_dioxide()),
        nitrogen: kept(Gas::Nitrogen, inventory.nitrogen()),
        argon: kept(Gas::Argon, inventory.argon()),
    };

    // Step 3: the climate.
    let climate = match (inputs.crust, inputs.insolation) {
        (Crust::Molten, _) => SurfaceState::MagmaOcean,
        // Every drawn inventory holds water, so a body inside the runaway limit has lost an
        // ocean; a dry one given by a caller keeps its carbon airborne in the same state.
        (Crust::Solid, Insolation::InsideRunaway) => {
            airborne.water = 0.0;
            SurfaceState::RunawayGreenhouse
        }
        (Crust::Solid, Insolation::Habitable) => {
            if airborne.water > 0.0 {
                airborne.carbon_dioxide *= TEMPERATE_AIRBORNE_CARBON;
            }
            SurfaceState::Temperate
        }
        (Crust::Solid, Insolation::BeyondMaximumGreenhouse) => SurfaceState::Snowball,
    };

    // Step 4: the partial pressures and the greenhouse they hold.
    let equilibrium = inputs.equilibrium;
    let weight = GRAVITATIONAL_CONSTANT * m / (r * r) / (4.0 * core::f64::consts::PI * r * r);
    let (mut t, partials, tau) = greenhouse(&airborne, weight, equilibrium, &saturate);

    // Step 5: the state.
    let total = partials.total();
    let state = if climate == SurfaceState::MagmaOcean || t >= SILICATE_SOLIDUS.value() {
        t = t.max(SILICATE_SOLIDUS.value());
        SurfaceState::MagmaOcean
    } else if total < AIRLESS_PRESSURE {
        return airless(SurfaceState::Airless, partials, equilibrium.value());
    } else {
        climate
    };
    Atmosphere {
        state,
        surface_temperature: saturate(t),
        surface_pressure: Some(total),
        partials,
        retention,
        exobase_temperature: exobase,
        optical_depth: tau,
        albedo: state.albedo(inputs.material),
        cloud_fraction: state.cloud_fraction(),
    }
}

#[cfg(test)]
mod tests;
