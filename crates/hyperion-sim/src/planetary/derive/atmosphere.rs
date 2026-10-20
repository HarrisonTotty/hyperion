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
//!   inventory of the side it formed on. From version 23 nitrogen is raised only beyond the
//!   ammonia line, where its carrier, ammonia hydrate, condensed (P14.T50.c, held by
//!   `AMMONIA_LINE_VERSION`). Radiogenic argon grows with age from the decay of ⁴⁰K.
//!   The same stream's words 3–5 rank its sulphur, chlorine and mantle redox, which the element
//!   inventory reads ([`chem`](crate::planetary::chem), P14.T50.a); nothing here reads them.
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
//!   follow ([`SurfaceState`]). From version 23 (`SNAPSHOT_VERSION`, the `p14-snapshot` lane's
//!   hold) each condensable is held in the greenhouse by P14.T51.a's condensate partition
//!   ([`partition`](crate::planetary::chem::partition)) over the body's climate, on the
//!   registry's phase diagrams, and the Bond albedo and cloud fraction are P14.T51.b's area mix
//!   over the condensates the pass publishes, its regolith and its cloud
//!   (`area_mix`), in place of the state's table. From the same version the grey greenhouse's
//!   carbon dioxide and water constants are P14.T57.a's, fitted at Venus's and then Earth's
//!   measured states ([`GREENHOUSE_CARBON_DIOXIDE`], [`GREENHOUSE_WATER`]).
//! - **Carbon speciation and oxygen** (P14.T24.f). A body formed beyond the snow line that lies
//!   beyond the maximum-greenhouse limit carries its carbon as methane, held where a reservoir lies
//!   on its surface at [`METHANE_RELATIVE_HUMIDITY`] of its saturation pressure; a runaway world
//!   keeps the oxygen of the water it lost, less what escapes with the hydrogen, what its young
//!   magma ocean took up and what its surface takes up ([`runaway_oxygen`]; Luger and Barnes 2015;
//!   Schaefer et al. 2016). No rule makes
//!   biological oxygen: no plan produces life, so a generated Earth has none.
//!
//! The figures the plan names are built as it names them. The ones it leaves to this task are
//! the lane's, each fixed by the Solar System and marked provisional on its constant: the
//! exobase multiple, the greenhouse constants and their exponents, the inventories, the albedos by
//! state, the carbon the oceans store and the pressure below which a body is airless.
//!
//! The irradiation this module reads is the equilibrium temperature from the hosts' light alone;
//! a giant's internal heat enters only its gas envelope's temperature, so that the two can be
//! told apart (ruling 112.7).

mod surface_mix;

pub(crate) use surface_mix::{LARGEST_BOND_ALBEDO, PassLight, PassSurface};

use crate::id::BodyId;
use crate::math;
use crate::planetary::chem::partition::{
    AirborneWeights, BuildColumnInventoryError, ClimateSpan, ColumnInventory, Ground,
};
use crate::planetary::chem::redox::SNOW_LINE_TEMPERATURE;
use crate::planetary::context::XuvHistory;
use crate::planetary::derive::composition::{MassFractions, SnowLineSide};
use crate::planetary::derive::irradiation::BondAlbedo;
use crate::planetary::params::{ICY_WATER_FRACTION, THIN_ENVELOPE_FRACTION};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::substance::{Element, saturation_pressure};
use crate::units::consts::{
    BOLTZMANN_CONSTANT, EARTH_MASS_KG, EARTH_RADIUS_M, GRAVITATIONAL_CONSTANT, METRES_PER_AU,
    SECONDS_PER_JULIAN_YEAR,
};
use crate::units::{
    EarthMasses, JoulesPerSquareMetre, Kelvin, Kilograms, Metres, MetresPerSecondSquared, Pascals,
    SolarLuminosities, SolarMasses, WattsPerSquareMetre, Years,
};
use crate::{GENERATOR_VERSION, GeneratorVersion, Seed};

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

    /// This retention with methane's bit set to nitrogen's: on a cold body methane lies in the
    /// air of its surface reservoir and is kept where nitrogen, the bulk gas, is (P14.T24.f).
    /// Methane, the lighter, never passes Jeans's test where nitrogen fails it.
    #[must_use]
    pub(crate) const fn with_methane_as_nitrogen(self) -> Self {
        let methane = 1 << Gas::Methane.index();
        if self.retains(Gas::Nitrogen) {
            Self(self.0 | methane)
        } else {
            Self(self.0 & !methane)
        }
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
///
/// Carbon takes it beyond the snow line. Nitrogen takes it beyond the snow line below generator
/// version 23 (`AMMONIA_LINE_VERSION`), and from it only where its carrier, ammonia hydrate,
/// condensed: beyond the ammonia line ([`nitrogen_boosted`], P14.T50.c).
pub const ICY_VOLATILE_BOOST: f64 = 100.0;

/// The temperature at or below which nitrogen's icy carrier, ammonia monohydrate (NH₃·H₂O), holds
/// half of a solar-composition gas's nitrogen, K: 123, nitrogen's 50% condensation temperature at
/// 10⁻⁴ bar, the element registry's ([`Element::N`]'s
/// [`condensation_temperature`](Element::condensation_temperature)), so that the model holds one
/// figure (P14.T50.c; decision-ammonia-line, 2026-10-15).
///
/// Lodders (2003, ApJ 591, 1220, read), Table 8, column 4, at 10⁻⁴ bar total pressure ("N … 131
/// NH3·H2O 123 NH3·H2O"), and §3.4.3: "Under equilibrium conditions, ammonia hydrate (NH3·H2O)
/// condenses at 131 K by the reaction of ammonia gas and water ice, and 50% of all nitrogen is in
/// this hydrate by 123 K." The boost treats a body's nitrogen as condensed, which is the 50%
/// figure; 131 K, column 2, is where the hydrate appears and holds about none of it. Lodders,
/// Palme and Gail (2009, arXiv:0901.1149, Table 1, read) give the same 123 K, "taken from \[03L\]".
pub const AMMONIA_HYDRATE_CONDENSATION: Kelvin = Element::N.condensation_temperature();

/// The temperature of the disc a body formed in, at its formation distance `formation_distance`,
/// where the disc's snow line is `snow_line` (P14.T50.c): the disc's Hayashi profile, T ∝ r^−½
/// ([`disc`](crate::planetary::disc)), at the snow line's [`SNOW_LINE_TEMPERATURE`], so
/// `T_f` = 170 K × √(`d_snow` ÷ `d_form`).
///
/// The profile is Hayashi's (1981, Prog. Theor. Phys. Suppl. 70, 35, not read), as Ida and Lin
/// (2004, ApJ 604, 388, arXiv:astro-ph/0312144, §2.3, eqs. 14 and 15, read) quote it: "T = 280
/// (a/1AU)^−1/2 (L∗/L⊙)^1/4 K", with the ice line where "T ≃ 170K". Normalised to 170 K at the
/// snow line as built, the ammonia line keeps its ratio to the snow line exactly.
///
/// Both distances are positive and finite. A planet's, dwarf planet's or belt member's disc is
/// its star's, at the host's zero-age luminosity, so `T_f` does not change with time, and a body
/// that migrated keeps its formation distance's, as its [`SnowLineSide`] does. A regular moon,
/// derived in its planet's nursery, reads the nursery's at its own distance, 170 K × √(`d_ice` ÷
/// a); a captured or giant-impact moon reads its parent's zone of the star's disc
/// ([`FormationZone`](crate::planetary::derive::FormationZone), P14.T56.c). Outside that range
/// the temperature is infinite at a formation distance of +0 and not a number for a negative
/// distance, a negative snow line or a NaN, none of which [`nitrogen_boosted`] boosts; a snow line
/// of zero gives 0 K.
///
/// # Examples
///
/// The generated zero-age Sun's disc is 170 K at its 2.255 au snow line and 123 K at 4.31 au:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::formation_temperature;
/// use hyperion_sim::units::Metres;
/// use hyperion_sim::units::consts::METRES_PER_AU;
///
/// let au = |x: f64| Metres::new(x * METRES_PER_AU);
/// let at = |d: f64| formation_temperature(au(d), au(2.255)).value();
/// assert!((at(2.255) - 170.0).abs() < 1e-9);
/// assert!((at(4.31) - 123.0).abs() < 0.2);
/// ```
#[must_use]
pub fn formation_temperature(formation_distance: Metres, snow_line: Metres) -> Kelvin {
    Kelvin::new(SNOW_LINE_TEMPERATURE.value() * (snow_line / formation_distance).sqrt())
}

/// Whether a body formed at the temperature `formation_temperature` holds nitrogen's icy carrier
/// ([`AMMONIA_HYDRATE_CONDENSATION`]): true at or below 123 K (P14.T50.c). From generator version
/// 23 (`AMMONIA_LINE_VERSION`) a body formed beyond the snow line takes [`ICY_VOLATILE_BOOST`] in
/// its nitrogen only where this holds; below it, everywhere beyond the snow line. Water and carbon
/// keep the snow line, at [`SNOW_LINE_TEMPERATURE`].
#[must_use]
pub fn nitrogen_boosted(formation_temperature: Kelvin) -> bool {
    formation_temperature <= AMMONIA_HYDRATE_CONDENSATION
}

/// The ammonia line of a disc whose snow line is `snow_line`: where its
/// [`formation_temperature`] falls to [`AMMONIA_HYDRATE_CONDENSATION`], (170 ÷ 123)² = 1.910
/// times the snow line (P14.T50.c). That is 4.31 au about the generated zero-age Sun, of
/// 0.6977 L☉ (plan 06's zero-age main sequence, Tout et al. 1996), whose snow line lies at
/// 2.255 au (4.32 au at 0.7 L☉), and 5.16 √L au about a host of zero-age luminosity L.
#[must_use]
pub fn ammonia_line(snow_line: Metres) -> Metres {
    let ratio = SNOW_LINE_TEMPERATURE.value() / AMMONIA_HYDRATE_CONDENSATION.value();
    snow_line * (ratio * ratio)
}

/// The generator version from which a body's nitrogen takes the ammonia line: 23, the 22 → 23
/// batch's bump (P14.T50.c; plan 14, "Order and parallelism", Phase L).
///
/// Below it [`NitrogenLine::in_force`] is the snow line, so that every inventory is version 22's
/// bit for bit until P14.T54.a's wiring, which removes this hold and its tripwire
/// (`the_ammonia_line_is_held_only_until_the_batch_s_bump`).
pub(crate) const AMMONIA_LINE_VERSION: GeneratorVersion = GeneratorVersion::new(23);

/// The line beyond which a body's nitrogen takes [`ICY_VOLATILE_BOOST`] (P14.T50.c, held by
/// [`AMMONIA_LINE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum NitrogenLine {
    /// The snow line, as water's and carbon's: version 22's rule.
    Snow,
    /// The ammonia line ([`nitrogen_boosted`]), beyond the snow line.
    Ammonia,
}

impl NitrogenLine {
    /// The line this generator version gives: the ammonia line from [`AMMONIA_LINE_VERSION`], and
    /// the snow line below it.
    #[must_use]
    pub(crate) fn in_force() -> Self {
        if GENERATOR_VERSION >= AMMONIA_LINE_VERSION {
            Self::Ammonia
        } else {
            Self::Snow
        }
    }
}

/// The generator version from which T13.c's passes take the `p14-snapshot` lane's rules: 23, the
/// 22 → 23 batch's bump (plan 14, "Order and parallelism", Phase L; decision-bump-plan-after-22
/// §1.2, "The lane's hold").
///
/// It is the one hold of the lane's edits on the record path: P14.T51.a's condensate partition in
/// the greenhouse ([`SnapshotRules::Batch`], [`atmosphere_under`]), P14.T51.b's area-mix albedo
/// with its converging passes, its record bulk for icy members and its photometry's references, and
/// P14.T57.a's refitted greenhouse constants, which its `Batch` arm reads
/// ([`SnapshotRules::greenhouse`]). Below it [`SnapshotRules::in_force`] is [`SnapshotRules::Held`], so that every atmosphere is version
/// 22's bit for bit until P14.T54.a's wiring, which gives the passes the body's climate and removes
/// this hold and its tripwire (`the_snapshot_lane_is_held_only_until_the_batch_s_bump`).
pub(crate) const SNAPSHOT_VERSION: GeneratorVersion = GeneratorVersion::new(23);

/// Which rules T13.c's passes follow (the `p14-snapshot` lane's hold, [`SNAPSHOT_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SnapshotRules {
    /// Version 22's: P14.T13.c's greenhouse, each condensable held at its saturation pressure at
    /// the mean surface temperature on the held Clausius–Clapeyron curves, and methane at its
    /// humidity ([`greenhouse`]).
    Held,
    /// The 22 → 23 batch's: the condensate partition over the body's climate
    /// ([`partition`](crate::planetary::chem::partition), P14.T51.a) on the registry's phase
    /// diagrams.
    Batch,
}

impl SnapshotRules {
    /// The rules this generator version gives: the batch's from [`SNAPSHOT_VERSION`], and version
    /// 22's below it.
    #[must_use]
    pub(crate) fn in_force() -> Self {
        if GENERATOR_VERSION >= SNAPSHOT_VERSION {
            Self::Batch
        } else {
            Self::Held
        }
    }

    /// The grey greenhouse's constants these rules give: version 22's under
    /// [`SnapshotRules::Held`], and P14.T57.a's fits under [`SnapshotRules::Batch`].
    #[must_use]
    pub(crate) const fn greenhouse(self) -> GreenhouseConstants {
        match self {
            Self::Held => GreenhouseConstants::VERSION_22,
            Self::Batch => GreenhouseConstants::FITTED,
        }
    }
}

/// The log-normal scatter of each volatile's inventory about its median, dex: 0.5 (the lane's,
/// provisional). Earth, Venus and Mars differ in nitrogen and carbon per unit mass by factors of a
/// few, which half a decade of scatter spans at one sigma.
pub const VOLATILE_SCATTER_DEX: f64 = 0.5;

/// Words of `planet.volatiles` a planet reads or reserves: words 0–2, the ranks of its water,
/// carbon and nitrogen (P14.T13.a); words 3–5, those of its sulphur, chlorine and redox
/// (P14.T50.a); then two reserved, for later volatiles (phosphorus, isotopes).
pub const VOLATILE_WORDS: u64 = 8;

/// The word of `planet.volatiles` that holds a body's sulphur rank (P14.T50.a): 3, read by its
/// number, so that words 0–2 and every value they set are untouched.
pub const SULPHUR_WORD: u64 = 3;

/// The ranks of a body's volatile inventory, as drawn from `planet.volatiles` by
/// [`VolatileDraws::for_body`], or given explicitly by a test or a tool (P14.T13.a, P14.T50.a).
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
    /// The rank of its sulphur per unit mass of rock
    /// ([`element_inventory`](crate::planetary::chem::inventory::element_inventory)). Word 3.
    pub sulphur: UnitUniform,
    /// The rank of its chlorine. Word 4.
    pub chlorine: UnitUniform,
    /// The rank of its mantle's ΔIW about its mean
    /// ([`delta_iw`](crate::planetary::chem::redox::delta_iw)). Word 5.
    pub redox: UnitUniform,
}

impl VolatileDraws {
    /// Every rank at its median: Earth's inventory per unit mass, the bulk silicate Earth's
    /// sulphur and chlorine per unit mass of rock, and the mean ΔIW.
    pub const MEDIAN: Self = Self {
        water: UnitUniform::HALF,
        carbon: UnitUniform::HALF,
        nitrogen: UnitUniform::HALF,
        sulphur: UnitUniform::HALF,
        chlorine: UnitUniform::HALF,
        redox: UnitUniform::HALF,
    };

    /// The draws of the body `body` in the universe of `seed`: words 0–5 of its
    /// `planet.volatiles` stream.
    ///
    /// Words 0–2 are drawn in turn, as P14.T13.a drew them; words 3–5 are read by their numbers
    /// from [`SULPHUR_WORD`] ([`Stream::seek`], the same words as [`Stream::word_at`] addresses),
    /// so that adding them moved no earlier value.
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
        stream.seek(SULPHUR_WORD);
        let mut rank = || {
            UnitUniform::new(stream.uniform_open())
                .expect("an open uniform lies strictly between 0 and 1")
        };
        let sulphur = rank();
        let chlorine = rank();
        let redox = rank();
        Self {
            water,
            carbon,
            nitrogen,
            sulphur,
            chlorine,
            redox,
        }
    }
}

/// A body's volatile inventory: the masses of water, carbon dioxide, nitrogen and argon it holds
/// at its surface and in its air, before escape and the climate divide them (P14.T13.a).
///
/// Since P14.T50.a these are the stored carriers of the body's
/// [`ElementInventory`](crate::planetary::chem::inventory::ElementInventory), of which this is the
/// view its [`volatiles`](crate::planetary::chem::inventory::ElementInventory::volatiles) gives,
/// with the same values to the bit.
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

    /// The carbon, counted as carbon dioxide, its stored carrier (decision-composition §1.2). A
    /// body formed beyond the snow line and beyond the maximum-greenhouse limit carries it as
    /// methane, mole for mole ([`atmosphere`], P14.T24.f).
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
/// `formed`'s side of the snow line at the temperature `formation_temperature`, with ranks
/// `draws`, at age `age` (P14.T13.a; P14.T50.c).
///
/// - Water, carbon and nitrogen are each Earth's per unit mass ([`EARTH_WATER_PER_MASS`],
///   [`EARTH_CARBON_DIOXIDE_PER_MASS`], [`EARTH_NITROGEN_PER_MASS`]) times a log-normal multiple
///   of median 1 and scatter [`VOLATILE_SCATTER_DEX`], at the body's rank.
/// - Beyond the snow line carbon is [`ICY_VOLATILE_BOOST`] times richer, and the water is the
///   larger of that and the body's bulk water, its ice.
/// - Nitrogen is [`ICY_VOLATILE_BOOST`] times richer beyond the snow line below generator
///   version 23 (`AMMONIA_LINE_VERSION`). From it, only where the body also formed at or below
///   ammonia hydrate's [`AMMONIA_HYDRATE_CONDENSATION`] ([`nitrogen_boosted`]); between the two
///   lines it is Earth's per unit mass at its rank.
/// - Argon is Earth's per unit of rock, grown as the ⁴⁰K of a body of the Sun's age has
///   decayed: Earth's × (1 − e^(−λt)) ÷ (1 − e^(−λ × 4.57 Gyr)).
///
/// `formation_temperature` is the disc's at the body's formation distance
/// ([`formation_temperature`]); the caller keeps it consistent with `formed`.
#[must_use]
pub fn volatile_inventory(
    mass: EarthMasses,
    fractions: &MassFractions,
    formed: SnowLineSide,
    formation_temperature: Kelvin,
    draws: &VolatileDraws,
    age: Years,
) -> VolatileInventory {
    volatile_inventory_under(
        NitrogenLine::in_force(),
        mass,
        fractions,
        formed,
        formation_temperature,
        draws,
        age,
    )
}

/// [`volatile_inventory`] with nitrogen's boost beyond the line `nitrogen_line`, for the hold's tests
/// (P14.T50.c): [`volatile_inventory`] is this under [`NitrogenLine::in_force`]. The water, carbon
/// and argon do not read the line.
#[must_use]
pub(crate) fn volatile_inventory_under(
    nitrogen_line: NitrogenLine,
    mass: EarthMasses,
    fractions: &MassFractions,
    formed: SnowLineSide,
    formation_temperature: Kelvin,
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
    let nitrogen_boost = match (nitrogen_line, formed) {
        (NitrogenLine::Snow, _) => boost,
        (NitrogenLine::Ammonia, SnowLineSide::Inside) => 1.0,
        (NitrogenLine::Ammonia, SnowLineSide::Beyond) => {
            if nitrogen_boosted(formation_temperature) {
                ICY_VOLATILE_BOOST
            } else {
                1.0
            }
        }
    };
    let drawn_water = EARTH_WATER_PER_MASS * multiple(draws.water) * kg;
    let water = match formed {
        SnowLineSide::Inside => drawn_water,
        SnowLineSide::Beyond => drawn_water.max(fractions.water() * kg),
    };
    let carbon = EARTH_CARBON_DIOXIDE_PER_MASS * boost * multiple(draws.carbon) * kg;
    let nitrogen = EARTH_NITROGEN_PER_MASS * nitrogen_boost * multiple(draws.nitrogen) * kg;
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

/// The greenhouse constant of carbon dioxide, per √bar: 14.86, fitted on Venus's measured state
/// (P14.T57.a, decision-p14-moons §4 and decision-p14-t57a-fits §1–§3; amends P14.T13.c).
///
/// The grey greenhouse's constants are fitted in this order, each at its body's measured inventory
/// and measured Bond albedo under the generator's own irradiation: nitrogen on Titan
/// ([`GREENHOUSE_NITROGEN`]), carbon dioxide on Venus (this one), then water on Earth
/// ([`GREENHOUSE_WATER`]). A constant of the radiative model calibrates on a body as measured,
/// never as generated. Venus's measured Bond albedo is 0.76 (Haus et al. 2016, Icarus 272, 178,
/// which states no uncertainty; NASA's fact sheet gives 0.77), which its cloud gives it in
/// P14.T51.b's area mix; Earth's is 0.294 (NASA's Earth fact sheet), to which the reference
/// Earth's cloud is calibrated; Titan's reference temperature is taken at the snowball albedo
/// until P14.T24.h.
///
/// - _Venus's measured air:_ about 4.8 × 10²⁰ kg (NASA's Venus fact sheet, Williams, updated
///   2024), 96.5 ± 0.8% carbon dioxide and 3.5 ± 0.8% nitrogen by volume, recommended below 45 km
///   (von Zahn, Kumar, Niemann and Prinn 1983, in _Venus_, eds Hunten et al., University of
///   Arizona Press, 299–430, Table II, as Johnson and de Oliveira 2019, Earth Space Sci. 6, 1299,
///   compile it). At the registry's molar masses that is 4.69 × 10²⁰ kg of carbon dioxide and
///   1.08 × 10¹⁹ kg of nitrogen, 90.4 and 2.09 bar by weight over the generator's Venus, M g ÷ 4πR²
///   as its partial pressures are taken, not the volume shares of the surface pressure.
/// - _Its nitrogen:_ von Zahn et al.'s 3.5 ± 0.8% is compiled from Pioneer Venus's and Venera
///   11/12's samples in descent (Pioneer Venus's gas chromatograph: 4.60, 3.54 and 3.41% at 51.6,
///   41.7 and 21.6 km, Oyama et al. 1980, JGR 85, 7891, Table 2), as Johnson and de Oliveira 2019
///   compile them. Mogul, Limaye and Way 2025 (GRL 52, e2025GL118909) find 2.49 ± 0.10% below
///   15 km and 5.21 ± 0.18% in the clouds. A bulk anywhere in 2.5–4.5% moves this fit by about
///   0.3% at most (−0.30% at 2.5%, +0.31% at 4.5%).
/// - _Its surface:_ 737 K, the mean over a sphere of Venus's 6,051.8 km mean radius. The Venus
///   International Reference Atmosphere gives 735.3 K and 7.6 K km⁻¹ over its lowest kilometre at
///   its altitude zero (Seiff et al. 1985, Adv. Space Res. 5(11), 3, read in transcription:
///   Venus-GRAM's `VIRALow.txt`, Justh et al. 2021, NASA/TM-20210022168, which sets that level at
///   its own 6,051.8 km reference radius). Seiff et al. put that zero at 6,052 km, as a search
///   excerpt gives it (lower confidence); carried 0.2 km down, that is 736.8 K, NASA's fact sheet's
///   737 K (decision-p14-t57a-fits §3). Should Seiff et al. put it at 6,051.8 km, the target is
///   735 K and the fit 14.70.
/// - _The fit:_ at the generator's irradiation for Venus at 0.76, 229.06 K, which the batch's
///   passes give the measured Venus, with the nitrogen term as built and no water, which a runaway
///   world has lost, 737 K needs τ = 141.6, which gives 14.862. At 14.86 the measured Venus
///   reaches 737.0 K (the test `the_greenhouse_constants_are_their_fits`).
///
/// The generated Venus is a median draw of the inventory, not the measured one: the table's Venus
/// holds 57.8 bar and reaches 696.3 K, while the measured Venus lies at the 65th percentile of its
/// carbon and the 86th of its nitrogen under [`VOLATILE_SCATTER_DEX`] (decision-p14-moons §4.1).
/// P14.T13.c's 18.5 was fitted at the median's 57 bar, and put the measured Venus at 778 K.
///
/// It takes effect with the `p14-snapshot` lane's hold (`SNAPSHOT_VERSION`, 23); below it the
/// greenhouse reads version 22's 18.5.
pub const GREENHOUSE_CARBON_DIOXIDE: f64 = 14.86;

/// The greenhouse constant of water vapour, per √bar: 4.47, fitted on Earth's measured state after
/// carbon dioxide (P14.T57.a, decision-p14-t57a-fits §1–§2; amends P14.T13.c), the last of the
/// three fits ([`GREENHOUSE_CARBON_DIOXIDE`] gives their order and the rule).
///
/// - _Earth's air:_ the table's median Earth's, whose carbon dioxide and nitrogen are Earth's per
///   unit mass: 42 Pa of carbon dioxide by weight, the pre-industrial 2.2 × 10¹⁵ kg (278.3 ±
///   2.9 ppm in 1750, IPCC AR6 WG1 ch. 2, §2.2.3.2.1), and 0.747 bar of nitrogen.
/// - _Its water_ is P14.T51.a's partition's: a minor condensable whose partial pressure, its mole
///   fraction times the surface pressure, is held at Manabe and Wetherald's 0.77 of its saturation
///   pressure on the registry's curve ([`WATER_SURFACE_HUMIDITY`]). That is 837 Pa by weight at
///   288 K, 0.496 of the saturation pressure, since a water molecule weighs 0.64 of the air's mean.
///   P14.T13.c's 2.89 held the water's column weight at its saturation pressure, so at a given
///   temperature the new term, 4.47 × √0.496 = 3.15 times √`p_sat`, is 9% the stronger.
/// - _Its surface:_ 288.0 K, the pre-industrial Earth's surface emission temperature,
///   (S↑ ÷ σ)^¼, which is what the grey `T_s` is (P14.T24.a's zonal profile has F = σ `T_s`⁴).
///   Jones et al. 1999's 14.0 °C for 1961–1990 (Rev. Geophys. 37, 173), less 0.36 K of warming
///   since 1850–1900 (AR6 WG1, Cross-Chapter Box 2.3, Table 1), is the arithmetic mean, 286.8 K,
///   and the spatial structure of the emission adds 1.3 K (Trenberth, Fasullo and Kiehl 2009,
///   BAMS 90, 311: 396.4 against 389.2 W m⁻², p. 315), 288.1 K; their 396 W m⁻² less about 0.8 K
///   of warming gives 288.2–288.3 K. NASA's fact sheet's 288 K is the same figure, with no epoch.
///   The climatology's ±0.5 K (AR6 WG1 Fig. 1.11) is about ±2.5% in this constant.
/// - _The fit:_ at the generator's irradiation for the table's Earth at 0.294, 255.14 K, 288 K
///   needs τ = 0.8314; less carbon dioxide's 0.3048 and nitrogen's 0.1177 that leaves 0.4089 for
///   the water, which gives 4.469 (the test `the_greenhouse_constants_are_their_fits`).
///
/// The fit reads none of the passes' stand-ins, and the generated Earth is not fitted: through the
/// batch's passes at their stand-in surface (no basins, so no sea; a Bond albedo of 0.2798) the
/// table's Earth reaches about 290.6 K, which P14.T54.a's real inputs settle and its test holds
/// to 288 ± 2 K. It takes effect with the `p14-snapshot` lane's hold (`SNAPSHOT_VERSION`, 23);
/// below it the greenhouse reads version 22's 2.89.
///
/// [`WATER_SURFACE_HUMIDITY`]: crate::planetary::chem::partition::WATER_SURFACE_HUMIDITY
pub const GREENHOUSE_WATER: f64 = 4.47;

/// The greenhouse constant of nitrogen, per √bar at [`NITROGEN_REFERENCE_TEMPERATURE`]: 1.55,
/// fixed by Titan (P14.T13.c; the lane's fit, provisional), the first of the three fits
/// ([`GREENHOUSE_CARBON_DIOXIDE`] gives their order). Nitrogen absorbs only when molecules
/// collide, as the square of the density, so the constant falls as (`T_ref` ÷ `T_eq`)², which
/// leaves Earth's nitrogen a tenth of Titan's warming.
///
/// P14.T57.a refits nothing on Titan: its Bond albedo lacks its haze until P14.T24.h, which refits
/// this constant and its reference temperature on Titan's measured state (decision-p14-t51-batch
/// §7, E1). Under the batch the table's Titan reaches 92.8 K, inside 94 ± 3 K.
pub const GREENHOUSE_NITROGEN: f64 = 1.55;

/// The equilibrium temperature at which [`GREENHOUSE_NITROGEN`] holds, K: Titan's, 75.6, at the
/// snowball albedo.
pub const NITROGEN_REFERENCE_TEMPERATURE: Kelvin = Kelvin::new(75.6);

/// The grey greenhouse's constants, per √bar, under one of the `p14-snapshot` lane's rules
/// ([`SnapshotRules::greenhouse`]; P14.T13.c, P14.T57.a): carbon dioxide's, water's and nitrogen's
/// at [`NITROGEN_REFERENCE_TEMPERATURE`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GreenhouseConstants {
    carbon_dioxide: f64,
    water: f64,
    nitrogen: f64,
}

impl GreenhouseConstants {
    /// Version 22's, P14.T13.c's fits: carbon dioxide 18.5, at the median Venus's 57 bar, and
    /// water 2.89, on Earth with its water's column weight at its saturation pressure, with
    /// [`GREENHOUSE_NITROGEN`]. P14.T54.a deletes them with the lane's hold.
    pub(crate) const VERSION_22: Self = Self {
        carbon_dioxide: 18.5,
        water: 2.89,
        nitrogen: GREENHOUSE_NITROGEN,
    };

    /// The batch's: P14.T57.a's [`GREENHOUSE_CARBON_DIOXIDE`] and [`GREENHOUSE_WATER`], with
    /// [`GREENHOUSE_NITROGEN`].
    pub(crate) const FITTED: Self = Self {
        carbon_dioxide: GREENHOUSE_CARBON_DIOXIDE,
        water: GREENHOUSE_WATER,
        nitrogen: GREENHOUSE_NITROGEN,
    };

    /// Carbon dioxide's constant, per √bar, for the tests' values by hand.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn carbon_dioxide(&self) -> f64 {
        self.carbon_dioxide
    }

    /// Water's constant, per √bar, for the tests' values by hand.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn water(&self) -> f64 {
        self.water
    }

    /// Nitrogen's constant, per √bar at [`NITROGEN_REFERENCE_TEMPERATURE`], for the tests' values
    /// by hand.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn nitrogen(&self) -> f64 {
        self.nitrogen
    }

    /// The grey optical depth of partial pressures `partials` over a surface in equilibrium at
    /// `equilibrium` under these constants ([`optical_depth`]).
    #[must_use]
    pub(crate) fn optical_depth(&self, partials: &PartialPressures, equilibrium: Kelvin) -> f64 {
        let root = |gas: Gas| (partials.of(gas).value().max(0.0) / PASCALS_PER_BAR).sqrt();
        let collisions = collision_factor(equilibrium);
        self.carbon_dioxide * root(Gas::CarbonDioxide)
            + self.water * root(Gas::Water)
            + self.nitrogen * collisions * root(Gas::Nitrogen)
    }

    /// The terms of [`optical_depth`](Self::optical_depth) one by one, each with its gas, in its
    /// order (carbon dioxide, water, nitrogen), so that their sum in that order is its τ to the bit
    /// (tested). Each term goes as the square root of its gas's partial pressure, so that
    /// ∂τᵢ ÷ ∂ln pᵢ = ½τᵢ, which P14.T24.a's greenhouse gain reads, under its atmosphere's rules
    /// ([`Atmosphere::rules`]; science-p14-t24a-climate.md §1). [`optical_depth`](Self::optical_depth)'s own arithmetic is T13.c's output, and stays as
    /// it is.
    #[must_use]
    pub(crate) fn terms(
        &self,
        partials: &PartialPressures,
        equilibrium: Kelvin,
    ) -> [(Gas, f64); 3] {
        let root = |gas: Gas| (partials.of(gas).value().max(0.0) / PASCALS_PER_BAR).sqrt();
        let collisions = collision_factor(equilibrium);
        [
            (
                Gas::CarbonDioxide,
                self.carbon_dioxide * root(Gas::CarbonDioxide),
            ),
            (Gas::Water, self.water * root(Gas::Water)),
            (
                Gas::Nitrogen,
                self.nitrogen * collisions * root(Gas::Nitrogen),
            ),
        ]
    }
}

/// Nitrogen's collision-induced scaling at the equilibrium temperature `equilibrium`:
/// (`T_ref` ÷ `T_eq`)² with `T_ref` [`NITROGEN_REFERENCE_TEMPERATURE`], and 0 at or below 0 K.
#[must_use]
fn collision_factor(equilibrium: Kelvin) -> f64 {
    let t = equilibrium.value();
    if t > 0.0 {
        let ratio = NITROGEN_REFERENCE_TEMPERATURE.value() / t;
        ratio * ratio
    } else {
        0.0
    }
}

/// The grey optical depth of partial pressures `partials` over a surface in equilibrium at
/// `equilibrium`: τ = Σ k × (p ÷ 1 bar)^½ over carbon dioxide, water and nitrogen (P14.T13.c), the
/// plan's square root of each partial pressure, nitrogen's scaled for collision-induced
/// absorption. Methane and oxygen enter no term ([`atmosphere`] says why).
///
/// The constants are the generator version's: version 22's below the `p14-snapshot` lane's hold
/// (`SNAPSHOT_VERSION`, 23), and from it P14.T57.a's [`GREENHOUSE_CARBON_DIOXIDE`] and
/// [`GREENHOUSE_WATER`], with [`GREENHOUSE_NITROGEN`] at [`NITROGEN_REFERENCE_TEMPERATURE`].
#[must_use]
pub fn optical_depth(partials: &PartialPressures, equilibrium: Kelvin) -> f64 {
    SnapshotRules::in_force()
        .greenhouse()
        .optical_depth(partials, equilibrium)
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
    /// Every gas's partial pressure from `pressure`, taken in [`Gas::ALL`]'s order, unvalidated:
    /// for the crate's tests that state an air by hand, such as P14.T51.j's, and for the surface
    /// generator's energy balance, which takes plan 14's grey terms of its inputs' gases
    /// (R09.T13.i, decision-r09-t13i §2.2). A caller's values are its own to check, as
    /// `SurfaceModifiers::derive` checks its inputs'.
    #[must_use]
    pub(crate) fn from_fn(pressure: impl Fn(Gas) -> Pascals) -> Self {
        Self(Gas::ALL.map(pressure))
    }

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

/// The relative humidity of methane near a surface that holds a reservoir of it, over the
/// liquid's saturation pressure: 0.5 (Niemann et al. 2010, JGR 115, E12006, §4.1: Titan's methane
/// "just above the surface is subsaturated, at a relative humidity of approximately 50%", against
/// the saturation pressure of methane over liquid methane with nitrogen dissolved in it, after
/// Kouvaris and Flasar 1991; it reaches 100%, its lifting condensation level, near 7 km).
///
/// A cited constant, not a fit (main's ruling, 2026-10-10), for every cold world with a methane
/// reservoir. Against the substance registry's curve for pure methane at the Huygens landing site's
/// 93.65 K and 1,467 hPa (Fulchignoni et al. 2005, Nature 438, 785) it gives a mole fraction of
/// 5.75%, against the probe's 5.65 ± 0.18% from 6.7 km to the surface (Niemann et al. 2010,
/// abstract), so the nitrogen dissolved in the liquid, which lowers its vapour pressure, lies
/// within the paper's "approximately": by §4.1's own pair of figures, 5.5% at about 50%, the
/// paper's saturation, 11.0%, lies about 4% below the pure curve's 11.5%. Niemann et al. measured over the
/// liquid at 93.65 K only: below methane's triple point, 90.6941 K, the registry's curve is the
/// solid's, and the same 0.5 is extrapolated over it (the lane's, provisional), which keeps the
/// gases continuous there. Titan's figure was measured far from its seas; the humidity does not
/// vary with the surface's liquid coverage, which decides only whether a reservoir exists (the
/// lane's, provisional).
pub const METHANE_RELATIVE_HUMIDITY: f64 = 0.5;

/// The share of the absorbed X-ray and ultraviolet energy that drives hydrogen out of a runaway
/// world's steam atmosphere: 0.30 (Luger and Barnes 2015, Astrobiology 15, 119, §3: their default,
/// within the 0.15–0.30 they take as typical of hydrogen-rich atmospheres). It is not
/// [`ESCAPE_EFFICIENCY`], Owen and Wu's for a hydrogen and helium envelope.
pub const WATER_LOSS_ESCAPE_EFFICIENCY: f64 = 0.30;

/// The temperature of the flow that carries a runaway world's hydrogen away, K: 400, an average
/// thermospheric temperature (Luger and Barnes 2015, §2.4.2, after Hunten, Pepin and Walker 1987,
/// Icarus 69, 532, and Chassefière 1996b, Icarus 124, 537).
pub const ESCAPING_FLOW_TEMPERATURE: Kelvin = Kelvin::new(400.0);

/// The binary diffusion coefficient of atomic oxygen in atomic hydrogen at 1 K, m⁻¹ s⁻¹:
/// 4.8 × 10¹⁹, from b = 4.8 × 10¹⁷ (T ÷ K)^0.75 cm⁻¹ s⁻¹ (Zahnle and Kasting 1986, Icarus 68, 462,
/// as Luger and Barnes 2015 take it below their eq. 9), so that b = 4.8 × 10¹⁹ (T ÷ K)^0.75 m⁻¹ s⁻¹.
pub const OXYGEN_IN_HYDROGEN_DIFFUSION_PER_METRE_SECOND: f64 = 4.8e19;

/// Earth's removal of atmospheric oxygen by its surface, mol of O₂ per year: 2.21 × 10¹³, about
/// 150 bar Gyr⁻¹ (Catling 2014, "The Great Oxidation Event Transition", Treatise on Geochemistry,
/// 2nd ed., vol. 6, pp. 177–195, doi:10.1016/B978-0-08-095975-7.01307-3, as Luger and Barnes 2015,
/// §2.5.1, quote it; their §5.1 reads "terrestrial rates of O₂ removal of a few hundred bars per
/// Gyr" as able to remove Venus's oxygen). At 7.07 × 10¹¹ kg yr⁻¹ it is 137 bar Gyr⁻¹ in Luger
/// and Barnes's equivalent bar, the mass of Earth's whole atmosphere, 5.148 × 10¹⁸ kg (Trenberth
/// and Smith 2005, J. Climate 18, 864), and 136 at the 5.20 × 10¹⁸ kg that one bar over Earth's
/// surface weighs.
///
/// Over two-thirds of it is the weathering of surface rocks; the rates set by tectonics are
/// 3–12 bar Gyr⁻¹ of Fe³⁺ subducted (Catling, Zahnle and McKay 2001, Science 293, 839) and some
/// 15 bar Gyr⁻¹ of reduced gases outgassed (Catling 2014), both through Luger and Barnes's §2.5.1,
/// and Venus would need tectonics some 15 times Earth's to subduct its oxygen (Rosenqvist and
/// Chassefière 1995, Planet. Space Sci. 43, 3, through their §2.5.4). Taking Earth's
/// whole rate for a dry runaway world, per unit of surface area over Earth's 4π R⊕², over its
/// whole age, is the lane's ([`runaway_oxygen`]; provisional, until P14.T53.a).
pub const EARTH_OXYGEN_SINK_MOL_PER_YEAR: f64 = 2.21e13;

/// The oxygen escape parameter η of a body of mass `mass` and radius `radius` whose water is
/// broken by light and lost under the X-ray and ultraviolet flux `xuv`: the share of the oxygen
/// freed that the escaping hydrogen drags away with it (Luger and Barnes 2015, eqs. 5, 11 and 12,
/// after Hunten, Pepin and Walker 1987).
///
/// η = (x − 1) ÷ (x + 8) for x ≥ 1, and 0 below, where x = k T `F_ref` ÷ (10 b g `m_H`) and
/// `F_ref` = ε `F_XUV` R ÷ (4 G M `m_H`) is the energy-limited hydrogen flux with no tidal
/// enhancement, at ε [`WATER_LOSS_ESCAPE_EFFICIENCY`], T [`ESCAPING_FLOW_TEMPERATURE`] and
/// b = [`OXYGEN_IN_HYDROGEN_DIFFUSION_PER_METRE_SECOND`] T^0.75. x is the flux over Luger and Barnes's critical
/// flux, 180 (M ÷ M⊕)² (R ÷ R⊕)⁻³ erg cm⁻² s⁻¹ at ε 0.30 (their eq. 9), below which no oxygen
/// escapes; far above it the oxygen leaves with the hydrogen in the water's own proportion.
///
/// Zero for a mass or radius that is not positive and finite, or a flux that is not positive, and
/// one for an infinite flux.
///
/// # Examples
///
/// Earth's water would take its oxygen with it only under some 0.18 W m⁻² of X-rays and
/// ultraviolet, 39 times what Earth receives today (Luger and Barnes), and nearly all of it under
/// a hundred watts:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::oxygen_escape_parameter;
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M};
/// use hyperion_sim::units::{Kilograms, Metres, WattsPerSquareMetre};
///
/// let (mass, radius) = (Kilograms::new(EARTH_MASS_KG), Metres::new(EARTH_RADIUS_M));
/// let eta = |flux: f64| oxygen_escape_parameter(mass, radius, WattsPerSquareMetre::new(flux));
/// assert!(eta(0.17) <= 0.0);
/// assert!(eta(0.19) > 0.0);
/// assert!(eta(100.0) > 0.98);
/// ```
#[must_use]
pub fn oxygen_escape_parameter(mass: Kilograms, radius: Metres, xuv: WattsPerSquareMetre) -> f64 {
    let (m, r, flux) = (mass.value(), radius.value(), xuv.value());
    if !(m.is_finite() && m > 0.0 && r.is_finite() && r > 0.0 && flux > 0.0) {
        return 0.0;
    }
    let hydrogen = 0.5 * Gas::Hydrogen.molecular_mass().value();
    let reference_flux =
        WATER_LOSS_ESCAPE_EFFICIENCY * flux * r / (4.0 * GRAVITATIONAL_CONSTANT * m * hydrogen);
    let t = ESCAPING_FLOW_TEMPERATURE.value();
    let diffusion = OXYGEN_IN_HYDROGEN_DIFFUSION_PER_METRE_SECOND * math::powf(t, 0.75);
    let gravity = GRAVITATIONAL_CONSTANT * m / (r * r);
    let x = BOLTZMANN_CONSTANT * t * reference_flux / (10.0 * diffusion * gravity * hydrogen);
    if x.is_infinite() {
        1.0
    } else if x >= 1.0 {
        (x - 1.0) / (x + 8.0)
    } else {
        0.0
    }
}

/// The oxygen, as O₂, that a runaway-greenhouse world of mass `mass` and radius `radius` keeps at
/// age `age` of the water `water_lost` it lost, having received the X-ray and ultraviolet flux
/// `saturated_xuv` while its hosts were saturated, and having had a young magma ocean for
/// `molten_for` from its formation (P14.T24.f; Luger and Barnes 2015):
///
/// - the water's photolysis frees W M(O₂) ÷ 2 M(H₂O) of oxygen as its hydrogen escapes, with the
///   registry's molar masses;
/// - the share η of it ([`oxygen_escape_parameter`]) escapes with the hydrogen, so that the oxygen
///   kept is (8 − 8η) ÷ 9 of the mass of the water lost (Luger and Barnes's eqs. A8 and A9). The
///   flux is the saturated one, taking the ocean as lost while its hosts are saturated, when the
///   escape is fastest (the lane's, provisional; P14.T53.a integrates the escape over the
///   history). That holds while the ocean is lost within the saturation time: at Luger and
///   Barnes's eq. A9 rate, 9 ÷ (1 + 8η) times the energy-limited one, Venus's flux takes an Earth
///   ocean in some 40 Myr, inside a Sun-like host's 100 Myr, and some two and a half oceans in
///   all. The rest of a larger ocean is lost after the saturation ends, as the flux and η fall,
///   and keeps more oxygen than this gives; and a young M dwarf, brighter than its zero-age
///   luminosity, drives more escape;
/// - the water lost while its young magma ocean lasted, `molten_for`, at the same rate, since the
///   melt does "not directly affect water loss to space", several tens of bars of steam staying
///   over it (Luger and Barnes, §2.5.3, after Hamano, Abe and Genda 2013; the rate's form is the
///   lane's, provisional, until P14.T53.a), gave its oxygen to the melt, which oxidised its
///   ferrous iron and keeps it when it freezes (Schaefer et al. 2016, ApJ 829, 63, §4.2). A
///   convecting melt can take all the oxygen that does not escape (Luger and Barnes, §2.5.3). Its
///   capacity is no limit: an Earth mantle's iron, at 8.05–8.18 wt% `FeO` (McDonough and Sun
///   1995, Chem. Geol. 120, 223; Workman and Hart 2005, EPSL 231, 53) and 4.01 × 10²⁴ kg, all
///   oxidised to `FeO₁.₅`, takes some 29–30 oceans' worth (science-p14-t24f §3 and §6.2);
/// - the surface takes up oxygen at Earth's rate per unit area, [`EARTH_OXYGEN_SINK_MOL_PER_YEAR`]
///   over 4π R⊕², over the body's whole age, a rate Luger and Barnes (§5.1) find able to remove
///   Venus's. Earth's rate is mostly weathering, which a dry world lacks, and counting from
///   formation overstates the sink of a world that entered its runaway late, under a brightening
///   host (both the lane's, provisional).
///
/// None is left below zero, and a world that lost no water keeps none. A magma ocean the
/// greenhouse holds keeps the rest while it is molten ([`atmosphere`]). A rate that is not a
/// number counts as none, and an infinite one takes all the water into the melt.
///
/// # Examples
///
/// A young Earth that lost Earth's ocean keeps over a hundred bars of oxygen, which its surface
/// has taken up after a couple of gigayears:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::runaway_oxygen;
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M};
/// use hyperion_sim::units::{Kilograms, Metres, WattsPerSquareMetre, Years};
///
/// let (mass, radius) = (Kilograms::new(EARTH_MASS_KG), Metres::new(EARTH_RADIUS_M));
/// let ocean = Kilograms::new(1.4e21);
/// let flux = WattsPerSquareMetre::new(0.1);
/// let kept = |age: f64| runaway_oxygen(ocean, mass, radius, flux, Years::ZERO, Years::new(age));
/// // A bar over Earth's surface is 5.20 × 10¹⁸ kg.
/// assert!(kept(1e8).value() > 100.0 * 5.20e18);
/// assert!(kept(3e9).value() <= 0.0);
/// // Had its young magma ocean lasted all of its first 100 Myr, which outlasts the loss, the melt
/// // would have taken all of that oxygen.
/// let melt = runaway_oxygen(ocean, mass, radius, flux, Years::new(1e8), Years::new(1e8));
/// assert!(melt.value() <= 0.0);
/// ```
#[must_use]
pub fn runaway_oxygen(
    water_lost: Kilograms,
    mass: Kilograms,
    radius: Metres,
    saturated_xuv: WattsPerSquareMetre,
    molten_for: Years,
    age: Years,
) -> Kilograms {
    let (water, r) = (water_lost.value(), radius.value());
    if !(water.is_finite() && water > 0.0 && r.is_finite() && r > 0.0) {
        return Kilograms::ZERO;
    }
    let oxygen = Gas::Oxygen.molar_mass_g_per_mol();
    let eta = oxygen_escape_parameter(mass, radius, saturated_xuv);
    // Luger and Barnes's eq. A9: the ocean is lost at 9 ÷ (1 + 8η) times the energy-limited
    // rate ε π R³ F ÷ (G M) (their eq. 2, with no tidal enhancement), whose flux the escape
    // parameter's reference flux reads.
    let energy_limited =
        WATER_LOSS_ESCAPE_EFFICIENCY * core::f64::consts::PI * r * r * r * saturated_xuv.value()
            / (GRAVITATIONAL_CONSTANT * mass.value());
    let rate = 9.0 * energy_limited / (1.0 + 8.0 * eta);
    let seconds = molten_for.value() * SECONDS_PER_JULIAN_YEAR;
    // The water lost while the young melt lasted: its oxygen went into the melt as ferric iron
    // and stays there when the melt freezes (Schaefer et al. 2016, §4.2).
    let molten = if seconds > 0.0 && rate > 0.0 {
        (rate * seconds).min(water)
    } else {
        0.0
    };
    let freed = (water - molten) * oxygen / (2.0 * Gas::Water.molar_mass_g_per_mol());
    let kept = freed * (1.0 - eta);
    let area = (r / EARTH_RADIUS_M) * (r / EARTH_RADIUS_M);
    let years = if age.value() > 0.0 { age.value() } else { 0.0 };
    // Moles of O₂ times its molar mass in kilograms per mole.
    let taken = EARTH_OXYGEN_SINK_MOL_PER_YEAR * area * years * oxygen * 1e-3;
    Kilograms::new(if kept > taken { kept - taken } else { 0.0 })
}

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
    /// The side of the snow line its volatiles formed on, which its inventory and, beyond the
    /// maximum-greenhouse limit, its carbon's carrier follow (P14.T24.f): where the body formed,
    /// or for a captured or giant-impact moon its parent's zone of the star's disc (P14.T56.c,
    /// [`FormationZone`](crate::planetary::derive::FormationZone)).
    pub formed: SnowLineSide,
    /// Its equilibrium temperature from its hosts' light at the time, at the albedo in use.
    pub equilibrium: Kelvin,
    /// Its equilibrium temperature at its hosts' largest past luminosity, at the same albedo.
    pub worst_equilibrium: Kelvin,
    /// Its temperature with its internal heat added, which a gas envelope radiates at: the
    /// equilibrium temperature for a body with none.
    pub heated: Kelvin,
    /// The X-ray and ultraviolet energy per unit area it has received.
    pub xuv_fluence: JoulesPerSquareMetre,
    /// The X-ray and ultraviolet flux it received while its hosts' activity was saturated, the
    /// one under which a runaway world loses its water (P14.T24.f, [`runaway_oxygen`]).
    pub saturated_xuv_flux: WattsPerSquareMetre,
    /// Where it lies against its hosts' habitable zone, which also sets its carbon's carrier
    /// (P14.T24.f).
    pub insolation: Insolation,
    /// Whether its crust has formed.
    pub crust: Crust,
    /// How long its young magma ocean lasted from its formation: P14.T28.a's `molten_until`,
    /// zero for a body with none. A runaway world's melt took the oxygen of the water it lost
    /// meanwhile, and keeps it (P14.T24.f, [`runaway_oxygen`]).
    pub molten_for: Years,
    /// Its age at the time, over which its surface takes up oxygen (P14.T24.f).
    pub age: Years,
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
    carbon_carrier: Option<CarbonCarrier>,
    column: PartialPressures,
    /// The `p14-snapshot` lane's rules its greenhouse was taken under.
    rules: SnapshotRules,
}

impl Atmosphere {
    /// The `p14-snapshot` lane's rules its greenhouse was taken under ([`atmosphere_under`]): their
    /// constants ([`SnapshotRules::greenhouse`]) gave its optical depth, and their rule held its
    /// condensables, at their saturation pressure under [`SnapshotRules::Held`] and by P14.T51.a's
    /// partition under [`SnapshotRules::Batch`]. P14.T24.a's column reads both of it
    /// (`hooks::surface`'s greenhouse gain), so that a record's column and its greenhouse cannot
    /// read two sets.
    #[must_use]
    pub(crate) const fn rules(&self) -> SnapshotRules {
        self.rules
    }

    /// What escape and the climate left free to fill the air, before any of it condensed, as
    /// column weights M g ÷ 4πR² by gas ([`column_inventory`](Self::column_inventory)'s): all
    /// zero for a gas envelope, a body of no mass or radius, and one whose weights are not finite.
    #[must_use]
    pub(crate) const fn inventory_weights(&self) -> &PartialPressures {
        &self.column
    }

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

    /// Which species the body retains: those that pass Jeans's test at the exobase
    /// ([`Retention::of`], P14.T13.b), with methane kept where nitrogen is (P14.T24.f).
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

    /// The species the body's carbon is airborne as (P14.T24.f): [`CarbonCarrier::Methane`] on a
    /// body whose volatiles formed beyond the snow line that lies beyond the maximum-greenhouse
    /// limit with a solid crust, and [`CarbonCarrier::CarbonDioxide`] otherwise; `None` for a gas envelope, whose
    /// composition is not this module's, and for a body of no mass or radius. It names the species
    /// whether or not any carbon is airborne: a cold airless moon formed beyond the snow line,
    /// which kept none, reads methane.
    ///
    /// A change of carrier is a recorded state change, as a change of [`Self::state`] or of
    /// [`Self::retention`] is: the carbon's partial pressure moves from one species to the other
    /// there.
    #[must_use]
    pub const fn carbon_carrier(&self) -> Option<CarbonCarrier> {
        self.carbon_carrier
    }

    /// Whether the body keeps an atmosphere: every state but [`SurfaceState::Airless`]. "None" is
    /// data, not a missing value: an airless body's atmosphere is this, with its tenuous partial
    /// pressures.
    #[must_use]
    pub fn keeps_atmosphere(&self) -> bool {
        self.state != SurfaceState::Airless
    }

    /// What escape and the climate left free to fill the air, before any of it condensed, as column
    /// weights M g ÷ 4πR²: the condensate partition's inventory (P14.T51.a), every gas of positive
    /// weight in registry order. Empty for a gas envelope, and for a body whose weights the
    /// partition cannot read.
    ///
    /// # Panics
    ///
    /// Never: the weights are of the registry's gas rows, each named once, finite and not negative,
    /// as [`ColumnInventory::new`] requires.
    #[must_use]
    pub(crate) fn column_inventory(&self) -> ColumnInventory {
        ColumnInventory::new(Gas::ALL.into_iter().filter_map(|gas| {
            let w = self.column.of(gas);
            (w.value() > 0.0).then(|| (gas.substance(), w))
        }))
        .expect("an atmosphere's column holds finite weights of gas rows, each once")
    }

    /// The same atmosphere with the Bond albedo `albedo`, for the tests of what reads it.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn with_albedo(self, albedo: BondAlbedo) -> Self {
        Self { albedo, ..self }
    }
}

/// The species a body's carbon is airborne as (P14.T24.f, [`Atmosphere::carbon_carrier`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CarbonCarrier {
    /// Carbon dioxide, CO₂: a body whose volatiles formed inside the snow line, and one whose
    /// volatiles formed beyond it within the maximum-greenhouse limit or with a molten crust.
    CarbonDioxide,
    /// Methane, CH₄, mole for mole: a body whose volatiles formed beyond the snow line
    /// ([`AtmosphereInputs::formed`]: where it formed, or a captured or giant-impact moon's
    /// parent's zone, P14.T56.c) that lies beyond the maximum-greenhouse limit
    /// ([`Insolation::BeyondMaximumGreenhouse`]) with a solid crust (science-p14-t24f §5).
    ///
    /// Beyond that limit no pressure of carbon dioxide keeps a surface
    /// above water's freezing point, since its Rayleigh scattering outweighs its added greenhouse
    /// (Kopparapu et al. 2013, ApJ 765, 131) and it condenses (Kasting, Whitmire and Reynolds
    /// 1993, Icarus 101, 108): it is frost, as on Triton and Callisto, not the climate's gas. The
    /// water is frozen and cold-trapped, so no moist air oxidises methane, which is lost only to
    /// photolysis, as on Titan; and a body formed beyond the snow line supplies it from its ices
    /// and its water–rock chemistry. Every body beyond the snow line that keeps a nitrogen air
    /// carries methane (Titan, Triton, Pluto). Anywhere else carbon is carbon dioxide, standing in
    /// for magmatic outgassing's CO₂ and CO (the model carries no CO): in almost all of Wogan,
    /// Krissansen-Totton and Catling's outgassing calculations CO₂ and CO are much more abundant
    /// than methane (2020, PSJ 1, 58, Fig. 3 and §3.1, where 2–7% give more CH₄ than CO), which
    /// becomes a substantial share only below FMQ−3, at the extremes of Earth's and Mars's magmas,
    /// or from magma colder than about 1,200 K (§4.1.2–4.1.3). It stands in for the redox route
    /// (decision-composition §1.3; P14.T50.a's ΔIW, P14.T55.a), and reads a recorded state, the
    /// insolation, so that the carrier changes only where the climate does.
    Methane,
}

impl From<CarbonCarrier> for Gas {
    fn from(carrier: CarbonCarrier) -> Self {
        match carrier {
            CarbonCarrier::CarbonDioxide => Self::CarbonDioxide,
            CarbonCarrier::Methane => Self::Methane,
        }
    }
}

/// The masses of each gas a body's climate leaves free to fill its air, before condensation.
#[derive(Debug, Clone, Copy)]
struct Airborne {
    water: f64,
    carbon_dioxide: f64,
    /// The carbon as methane, airborne in the carbon dioxide's place where
    /// [`Self::carbon_carrier`] is methane (P14.T24.f).
    methane: f64,
    nitrogen: f64,
    argon: f64,
    oxygen: f64,
    /// The species the carbon is airborne as (P14.T24.f): set by where the body formed and where
    /// it lies, and carbon dioxide on a young magma ocean.
    carrier: CarbonCarrier,
}

impl Airborne {
    /// What escape leaves of the inventory of the body `inputs` describe, whose retention is
    /// `retention` ([`atmosphere`]'s step 2).
    #[must_use]
    fn kept(inputs: &AtmosphereInputs, retention: Retention) -> Self {
        let kept = |gas: Gas, mass: Kilograms| {
            if retention.retains(gas) {
                mass.value()
            } else {
                0.0
            }
        };
        let inventory = &inputs.inventory;
        let carrier = match (inputs.formed, inputs.insolation) {
            (SnowLineSide::Beyond, Insolation::BeyondMaximumGreenhouse) => CarbonCarrier::Methane,
            (SnowLineSide::Beyond, Insolation::InsideRunaway | Insolation::Habitable)
            | (SnowLineSide::Inside, _) => CarbonCarrier::CarbonDioxide,
        };
        // The same carbon, mole for mole, as methane: the hydrogen comes from the body's water.
        let methane = match carrier {
            CarbonCarrier::Methane if retention.retains(Gas::Methane) => {
                inventory.carbon_dioxide().value() * Gas::Methane.molar_mass_g_per_mol()
                    / Gas::CarbonDioxide.molar_mass_g_per_mol()
            }
            CarbonCarrier::Methane | CarbonCarrier::CarbonDioxide => 0.0,
        };
        Self {
            water: kept(Gas::Water, inventory.water()),
            carbon_dioxide: kept(Gas::CarbonDioxide, inventory.carbon_dioxide()),
            methane,
            nitrogen: kept(Gas::Nitrogen, inventory.nitrogen()),
            argon: kept(Gas::Argon, inventory.argon()),
            oxygen: 0.0,
            carrier,
        }
    }

    /// How the climate of the body `inputs` describe divides what it keeps, with retention
    /// `retention` ([`atmosphere`]'s step 3); the state it gives.
    fn divide(&mut self, inputs: &AtmosphereInputs, retention: Retention) -> SurfaceState {
        match (inputs.crust, inputs.insolation) {
            // A young magma ocean, whatever its greenhouse's temperature, holds its carbon as
            // carbon dioxide (the reported surface is at least the solidus).
            (Crust::Molten, _) => {
                self.carrier = CarbonCarrier::CarbonDioxide;
                self.methane = 0.0;
                SurfaceState::MagmaOcean
            }
            // Every drawn inventory holds water, so a body inside the runaway limit has lost an
            // ocean; a dry one given by a caller keeps its carbon airborne in the same state.
            (Crust::Solid, Insolation::InsideRunaway) => {
                let lost = self.water;
                self.water = 0.0;
                if retention.retains(Gas::Oxygen) {
                    self.oxygen = runaway_oxygen(
                        Kilograms::new(lost),
                        inputs.mass,
                        inputs.radius,
                        inputs.saturated_xuv_flux,
                        inputs.molten_for,
                        inputs.age,
                    )
                    .value();
                }
                SurfaceState::RunawayGreenhouse
            }
            (Crust::Solid, Insolation::Habitable) => {
                if self.water > 0.0 {
                    self.carbon_dioxide *= TEMPERATE_AIRBORNE_CARBON;
                    self.methane *= TEMPERATE_AIRBORNE_CARBON;
                }
                SurfaceState::Temperate
            }
            (Crust::Solid, Insolation::BeyondMaximumGreenhouse) => SurfaceState::Snowball,
        }
    }

    /// The species the carbon is airborne as.
    #[must_use]
    const fn carbon_carrier(&self) -> CarbonCarrier {
        self.carrier
    }

    /// What the climate leaves free to fill the air, as the condensate partition reads it: each
    /// gas's column weight, its mass times `weight` pascals per kilogram, in registry order, with
    /// the carbon as its carrier and the oxygen only where there is some, as [`greenhouse`] takes
    /// them (P14.T51.a).
    ///
    /// # Errors
    ///
    /// [`BuildColumnInventoryError::NotAWeight`] where a weight is not finite: a body so small
    /// that its weight per kilogram overflows, or a mass whose weight does.
    fn column(&self, weight: f64) -> Result<ColumnInventory, BuildColumnInventoryError> {
        let carbon = match self.carrier {
            CarbonCarrier::CarbonDioxide => (Gas::CarbonDioxide, self.carbon_dioxide),
            CarbonCarrier::Methane => (Gas::Methane, self.methane),
        };
        let oxygen = (self.oxygen > 0.0).then_some((Gas::Oxygen, self.oxygen));
        let gases = [
            Some((Gas::Water, self.water)),
            Some(carbon),
            Some((Gas::Nitrogen, self.nitrogen)),
            oxygen,
            Some((Gas::Argon, self.argon)),
        ];
        ColumnInventory::new(
            gases
                .into_iter()
                .flatten()
                .map(|(gas, mass)| (gas.substance(), Pascals::new(mass * weight))),
        )
    }

    /// [`column`](Self::column)'s weights as partial pressures, the record's view of the
    /// partition's inventory ([`Atmosphere::column_inventory`]): none where a weight is not finite,
    /// which the partition cannot read.
    #[must_use]
    fn column_weights(&self, weight: f64) -> PartialPressures {
        let mut p = [Pascals::ZERO; 9];
        let Ok(column) = self.column(weight) else {
            return PartialPressures(p);
        };
        for (substance, w) in column.species() {
            if let Some(gas) = substance.gas() {
                p[gas.index()] = w;
            }
        }
        PartialPressures(p)
    }
}

/// The weight of methane over a surface whose inventory would weigh `inventory` and whose
/// humidity caps its partial pressure in the air at `cap`, [`METHANE_RELATIVE_HUMIDITY`] ×
/// `p_sat`, among the other airborne gases' weights `others` (P14.T24.f; signoff-3.md, item 1.7).
///
/// The cap is on the partial pressure x `p_s`, with x methane's mole fraction, its weight over its
/// molar mass against every gas's, and `p_s` the summed weights: never on the column weight
/// M g ÷ 4πR², from which it differs by `μ_CH₄` ÷ μ̄ for a minor gas. With W and Σ the other
/// gases' summed weights and weights over molar masses, the weight w that meets the cap solves
/// w² + b w − c = 0, b = W − cap and c = cap `μ_CH₄` Σ; its positive root is taken as
/// 2c ÷ (b + √(b² + 4c)) for b ≥ 0 and (√(b² + 4c) − b) ÷ 2 below, so that neither cancels. The
/// weight is the smaller of that root and the inventory's: all of it above the critical point,
/// where the cap is infinite, and none where the cap is not positive. The other gases are summed
/// in [`Gas::ALL`]'s order.
#[must_use]
fn capped_methane_weight(inventory: Pascals, cap: Pascals, others: &[Pascals; 9]) -> Pascals {
    let cap = cap.value();
    if cap.is_nan() || cap <= 0.0 {
        return Pascals::ZERO;
    }
    if cap.is_infinite() {
        return inventory;
    }
    let (mut total, mut moles) = (0.0, 0.0);
    for gas in Gas::ALL {
        if gas != Gas::Methane {
            let w = others[gas.index()].value();
            total += w;
            moles += w / gas.molar_mass_g_per_mol();
        }
    }
    let b = total - cap;
    let c = cap * Gas::Methane.molar_mass_g_per_mol() * moles;
    let root = if b >= 0.0 {
        if c > 0.0 {
            2.0 * c / (b + (b * b + 4.0 * c).sqrt())
        } else {
            0.0
        }
    } else {
        ((b * b + 4.0 * c).sqrt() - b) / 2.0
    };
    Pascals::new(inventory.value().min(root))
}

/// The surface temperature, partial pressures and optical depth of the airborne masses
/// `airborne`, each weighing `weight` pascals per kilogram at the surface, over a surface in
/// equilibrium at `equilibrium`: the temperature iterated [`GREENHOUSE_STEPS`] times upward from
/// the equilibrium temperature through the grey greenhouse its partial pressures give, each held at
/// its saturation vapour pressure there (the substance registry's
/// [`saturation_pressure`](crate::substance::saturation_pressure)), methane's partial pressure in
/// the air at [`METHANE_RELATIVE_HUMIDITY`] of it ([`capped_methane_weight`]), and held by
/// `saturate` at the hottest host's. The carbon is airborne as its carrier
/// ([`Airborne::carbon_carrier`]). The greenhouse's constants are version 22's
/// ([`GreenhouseConstants::VERSION_22`]).
fn greenhouse(
    airborne: &Airborne,
    weight: f64,
    equilibrium: Kelvin,
    saturate: &impl Fn(f64) -> Kelvin,
) -> (f64, PartialPressures, f64) {
    let partials_at = |t: f64| {
        let saturation = |gas: Gas| {
            saturation_pressure(gas.substance(), Kelvin::new(t))
                .map_or(f64::INFINITY, Pascals::value)
        };
        // A gas with no phase data never condenses: its saturation pressure is infinite.
        let held = |mass: f64, gas: Gas| (mass * weight).min(saturation(gas));
        let mut p = [Pascals::ZERO; 9];
        p[Gas::Water.index()] = Pascals::new(held(airborne.water, Gas::Water));
        let carrier = airborne.carbon_carrier();
        match carrier {
            CarbonCarrier::CarbonDioxide => {
                p[Gas::CarbonDioxide.index()] =
                    Pascals::new(held(airborne.carbon_dioxide, Gas::CarbonDioxide));
            }
            // Written last, below.
            CarbonCarrier::Methane => {}
        }
        p[Gas::Nitrogen.index()] = Pascals::new(held(airborne.nitrogen, Gas::Nitrogen));
        // Oxygen never condenses on a world that keeps it, whatever phase data its row gains. It
        // is written only where there is some, so that a body without it is P14.T13.c's to the
        // bit for every input.
        if airborne.oxygen > 0.0 {
            p[Gas::Oxygen.index()] = Pascals::new(airborne.oxygen * weight);
        }
        p[Gas::Argon.index()] = Pascals::new(held(airborne.argon, Gas::Argon));
        // Methane last: its humidity caps its share of the air the others make.
        match carrier {
            CarbonCarrier::Methane => {
                p[Gas::Methane.index()] = capped_methane_weight(
                    Pascals::new(airborne.methane * weight),
                    Pascals::new(METHANE_RELATIVE_HUMIDITY * saturation(Gas::Methane)),
                    &p,
                );
            }
            CarbonCarrier::CarbonDioxide => {}
        }
        PartialPressures(p)
    };
    let constants = SnapshotRules::Held.greenhouse();
    let mut t = equilibrium.value();
    for _ in 0..GREENHOUSE_STEPS {
        let tau = constants.optical_depth(&partials_at(t), equilibrium);
        t = saturate(grey_surface_temperature(equilibrium, tau).value()).value();
    }
    let partials = partials_at(t);
    let tau = constants.optical_depth(&partials, equilibrium);
    (t, partials, tau)
}

/// The climate the condensate partition reads at a step of the greenhouse: a function of the
/// step's mean surface temperature and of the partial pressures the previous step gave (P14.T51.a;
/// [`atmosphere_under`]).
pub(crate) type ClimateOf<'a> = dyn Fn(Kelvin, &PartialPressures) -> ClimateSpan + 'a;

/// The partial pressures, as column weights, of the airborne inventory `inventory` over the climate
/// `climate` on the ground `ground` (the condensate partition's, [`AirborneWeights::solve`]), with
/// `weights` the scratch it solves in.
#[must_use]
fn partitioned_partials(
    inventory: &ColumnInventory,
    climate: &ClimateSpan,
    ground: &Ground,
    weights: &mut AirborneWeights,
) -> PartialPressures {
    weights.solve(climate, ground, inventory);
    let mut p = [Pascals::ZERO; 9];
    for ((substance, _), &w) in inventory.species().zip(weights.weights()) {
        if let Some(gas) = substance.gas() {
            p[gas.index()] = w;
        }
    }
    PartialPressures(p)
}

/// The partial pressures, as column weights, of the airborne inventory `inventory` with none of it
/// condensed: the column the partitioned greenhouse's first step reads its climate from.
#[must_use]
fn whole_column(inventory: &ColumnInventory) -> PartialPressures {
    let mut p = [Pascals::ZERO; 9];
    for (substance, w) in inventory.species() {
        if let Some(gas) = substance.gas() {
            p[gas.index()] = w;
        }
    }
    PartialPressures(p)
}

/// [`greenhouse`] under the condensate partition (P14.T51.a, [`SnapshotRules::Batch`]): the
/// temperature iterated [`GREENHOUSE_STEPS`] times upward from the equilibrium temperature through
/// the grey greenhouse its partial pressures give, each step's partial pressures the partition's
/// of the airborne inventory `inventory`, as column weights ([`Airborne::column`]), over the climate
/// `climate` gives for the step's temperature and the previous step's partial pressures, on the
/// ground `ground`, and held by `saturate` at the hottest host's. The greenhouse's constants are
/// P14.T57.a's fits ([`GreenhouseConstants::FITTED`]).
///
/// The climate reads the column of the step before, so that a column's contrasts can follow its
/// pressure: a thin air's large contrast holds its dominant condensable at a cold trap, which keeps
/// it thin (Wordsworth 2015's collapse). The first step's column is the whole airborne inventory,
/// the largest it can be and so the smallest contrasts, as a body's outgassed air starts: an empty
/// column's contrasts, an airless body's, would freeze out even a bar of nitrogen that a full
/// column's transport keeps airborne (the collapse is bistable, Wordsworth 2015), and the
/// iteration keeps the state the full column leads to.
#[must_use]
fn partitioned_greenhouse(
    inventory: &ColumnInventory,
    equilibrium: Kelvin,
    saturate: &impl Fn(f64) -> Kelvin,
    climate: &ClimateOf<'_>,
    ground: &Ground,
) -> (f64, PartialPressures, f64) {
    let constants = SnapshotRules::Batch.greenhouse();
    let mut weights = AirborneWeights::new();
    let mut partials = whole_column(inventory);
    let mut t = equilibrium.value();
    for _ in 0..GREENHOUSE_STEPS {
        let span = climate(Kelvin::new(t), &partials);
        partials = partitioned_partials(inventory, &span, ground, &mut weights);
        let tau = constants.optical_depth(&partials, equilibrium);
        t = saturate(grey_surface_temperature(equilibrium, tau).value()).value();
    }
    let span = climate(Kelvin::new(t), &partials);
    partials = partitioned_partials(inventory, &span, ground, &mut weights);
    let tau = constants.optical_depth(&partials, equilibrium);
    (t, partials, tau)
}

/// The atmosphere of the body `inputs` describe (P14.T13.b–c, with P14.T24.f's carbon and oxygen).
///
/// 1. A body with a hydrogen and helium envelope of at least 0.1% of its mass is a
///    [`SurfaceState::GasEnvelope`] radiating at its heated temperature.
/// 2. Otherwise each inventoried gas is kept if it is retained against Jeans escape at the exobase
///    temperature ([`exobase_temperature`], [`Retention`]). The carbon of a body formed beyond the
///    snow line may become methane (step 4), which lies in the air of its surface reservoir and is
///    kept where nitrogen, the cold worlds' bulk gas, is (P14.T24.f), and the reported
///    [`Atmosphere::retention`] says so. Kept where nitrogen is, methane passes its own Jeans test
///    at 5 × 16.043 ÷ 28.014 = 2.86 `T_eq`, on a body of any size, above the real ratio of
///    exobase to equilibrium temperature of every nitrogen and methane air known, 1.8–2.7
///    (Titan's exobase near 4,000 km and 150 K, Johnson 2010, ApJ 716, 1573, §5; Pluto's upper
///    atmosphere near 70 K, Gladstone and Young 2019, Annu. Rev. Earth Planet. Sci. 47, 119;
///    Triton's isothermal 102 K to its exobase at 870 km, Krasnopolsky et al. 1993, JGR 98, 3065,
///    over its nitrogen ice's 38 K, Tryka et al. 1993, Science 261, 751), whose methane
///    and its products cool their thermospheres. Titan's own methane, at λ ≈ 18 at the exobase
///    multiple that stands in for the bulk air's non-thermal losses ([`EXOBASE_MULTIPLE`]), has
///    λ ≈ 24–28 at its real exobase, near 4,075 km and 150–180 K (G M m ÷ k T r; Johnson 2010
///    gives about 20). That is marginal against [`JEANS_RETENTION_THRESHOLD`]: methane stays
///    because its thermal escape is negligible at λ ≳ 20, and because through nitrogen a minor
///    gas escapes at most at its diffusion limit, which over 4.57 Gyr takes under a quarter of the
///    median reservoir on a body of any size (science-p14-t24f §4, with methane's diffusion in
///    nitrogen b = 7.34 × 10¹⁶ T^0.75 cm⁻¹ s⁻¹, Marrero and Mason 1972, J. Phys. Chem. Ref. Data
///    1, 3, Table 13, fitted from 298 K and extrapolated below it; Pluto's measured methane
///    escape, some 5 × 10²⁵ s⁻¹, Gladstone and Young 2019, is under 1% of its reservoir's median
///    over that time). Methane's real loss is to photolysis, and the rule takes the reservoir as
///    resupplied.
/// 3. The climate divides what is kept: inside the runaway limit the oceans are lost and all the
///    carbon is airborne, and the oxygen of the water lost stays, less its sinks
///    ([`runaway_oxygen`], P14.T24.f, if oxygen is retained); in the habitable zone a world with
///    water stores all but [`TEMPERATE_AIRBORNE_CARBON`] of its carbon in rock; beyond the
///    maximum-greenhouse limit the carbon is airborne, since no ocean weathers it. A molten crust
///    holds nothing back.
/// 4. The partial pressures are each airborne mass's weight over the surface, M g ÷ 4πR², held at
///    each gas's saturation vapour pressure at the surface temperature (water, carbon dioxide,
///    nitrogen and argon condense, by their rows of the substance registry,
///    [`saturation_pressure`]), which is iterated
///    [`GREENHOUSE_STEPS`] times upward from the equilibrium temperature through the grey
///    greenhouse they give ([`optical_depth`], [`grey_surface_temperature`]). On a body formed
///    beyond the snow line that lies beyond the maximum-greenhouse limit the carbon is methane,
///    mole for mole ([`CarbonCarrier::Methane`]); where its surface holds a reservoir of it, its
///    partial pressure in the air, its mole fraction times the surface pressure, is held at
///    [`METHANE_RELATIVE_HUMIDITY`] of its saturation pressure (signoff-3.md, item 1.7); oxygen
///    never condenses (P14.T24.f).
/// 5. A surface pressure below [`AIRLESS_PRESSURE`] is [`SurfaceState::Airless`], at the
///    equilibrium temperature; a surface over the [`SILICATE_SOLIDUS`], or a crust not yet formed,
///    a [`SurfaceState::MagmaOcean`] (the young one at least at the solidus), whose melt holds any
///    oxygen while it is molten (Luger and Barnes 2015, §2.5.3). The oxygen of the water lost while a young melt lasted
///    ([`AtmosphereInputs::molten_for`]) stays in it ([`runaway_oxygen`]; Schaefer et al.
///    2016, §4.2: the melt's exchange with the air halts when it freezes). A surface its
///    greenhouse holds molten is not timed: should it freeze again, the oxygen it held comes
///    back, an overstatement, for a planet whose mantle froze while it still lost water (Luger
///    and Barnes's dimming M dwarfs), of the oxygen freed before it froze (provisional, until
///    P14.T53.a). The rest takes the climate's state.
///
/// No surface is hotter than its hottest host: irradiation cannot lift it there, which is
/// thermodynamics, so the temperature saturates at [`AtmosphereInputs::hottest_host`].
///
/// Methane and oxygen enter no term of the grey optical depth. P14.T13.c's nitrogen term was
/// fitted on Titan, whose greenhouse is "caused primarily by pressure-induced opacity of N2, CH4,
/// and H2" (McKay, Pollack and Courtin 1991, Science 253, 1118), so it already carries methane's
/// share at Titan's proportion, and a methane term would count it twice. Where methane's share of
/// the air differs from Titan's, its warming is not scaled, and the feedback of its vapour, set by
/// the surface temperature, is not modelled. No term is fitted for oxygen (the lane's,
/// provisional).
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
///     atmosphere, earth_xuv_fluence, formation_temperature, volatile_inventory,
/// };
/// use hyperion_sim::planetary::derive::{SnowLineSide, composition};
/// use hyperion_sim::units::{
///     EarthMasses, EarthFluxes, EarthRadii, Kelvin, Kilograms, Metres, WattsPerSquareMetre, Years,
/// };
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M, METRES_PER_AU};
///
/// let solved = composition(EarthMasses::new(1.0), EarthRadii::new(1.0), SnowLineSide::Inside, EarthFluxes::new(1.0))?;
/// let fractions = solved.fractions();
/// // Formed at 1 au, inside the zero-age Sun's 2.26 au snow line.
/// let formed_at = formation_temperature(Metres::new(METRES_PER_AU), Metres::new(2.26 * METRES_PER_AU));
/// let inventory = volatile_inventory(
///     EarthMasses::new(1.0), &fractions, SnowLineSide::Inside, formed_at, &VolatileDraws::MEDIAN, Years::new(4.57e9),
/// );
/// let earth = atmosphere(&AtmosphereInputs {
///     mass: Kilograms::new(EARTH_MASS_KG),
///     radius: Metres::new(EARTH_RADIUS_M),
///     material: SurfaceMaterial::of(&fractions),
///     envelope_fraction: 0.0,
///     inventory,
///     formed: SnowLineSide::Inside,
///     equilibrium: Kelvin::new(254.0),
///     worst_equilibrium: Kelvin::new(254.0),
///     heated: Kelvin::new(254.0),
///     xuv_fluence: earth_xuv_fluence(),
///     saturated_xuv_flux: WattsPerSquareMetre::new(0.3),
///     insolation: Insolation::Habitable,
///     crust: Crust::Solid,
///     molten_for: Years::ZERO,
///     age: Years::new(4.57e9),
///     hottest_host: Kelvin::new(5_772.0),
/// });
/// assert_eq!(earth.state(), SurfaceState::Temperate);
/// assert!((earth.surface_temperature().value() - 288.0).abs() < 2.0);
/// # Ok::<(), hyperion_sim::planetary::derive::composition::SolveCompositionError>(())
/// ```
#[must_use]
pub fn atmosphere(inputs: &AtmosphereInputs) -> Atmosphere {
    // Until P14.T54.a gives the passes the body's climate, which `derive_body` has not (its range
    // is T24.a's, of the body's spin), the batch's partition would read a climate with no contrast,
    // and its area mix a cover, basins and a regolith that stand in for the body's. The hold's
    // tripwire stops the bump until it does.
    atmosphere_under(
        SnapshotRules::in_force(),
        inputs,
        &PassSurface::stand_in(PassLight::HottestHost),
    )
}

/// What step 4 of [`atmosphere_under`] follows ([`greenhouse_under`]): the lane's rules, the
/// climate the condensate partition reads at each step, and the ground it lies on.
#[derive(Clone, Copy)]
struct Greenhouse<'a> {
    rules: SnapshotRules,
    climate_of: &'a ClimateOf<'a>,
    ground: Ground,
}

/// Step 4 of [`atmosphere_under`] as `how` says: the surface temperature, partial pressures and
/// optical depth of the airborne masses `airborne`, each weighing `weight` pascals per kilogram,
/// over a surface in equilibrium at `equilibrium`, by [`greenhouse`] or, under the batch's rules,
/// [`partitioned_greenhouse`] over the climate `how.climate_of` gives on the ground `how.ground`.
///
/// `None` only under the batch's rules, where a weight is not finite and the partition cannot
/// weigh the inventory ([`Airborne::column`]'s error, which says no more than that).
#[must_use]
fn greenhouse_under(
    how: &Greenhouse<'_>,
    airborne: &Airborne,
    weight: f64,
    equilibrium: Kelvin,
    saturate: &impl Fn(f64) -> Kelvin,
) -> Option<(f64, PartialPressures, f64)> {
    match how.rules {
        SnapshotRules::Held => Some(greenhouse(airborne, weight, equilibrium, saturate)),
        SnapshotRules::Batch => airborne.column(weight).ok().map(|inventory| {
            partitioned_greenhouse(
                &inventory,
                equilibrium,
                saturate,
                how.climate_of,
                &how.ground,
            )
        }),
    }
}

/// [`atmosphere`] under the lane's rules `rules`, over the surface `surface` (P14.T51.a–b, held by
/// [`SNAPSHOT_VERSION`]): [`atmosphere`] is this under [`SnapshotRules::in_force`] on the passes'
/// stand-in surface ([`PassSurface::stand_in`]).
///
/// Under [`SnapshotRules::Held`] step 4 is P14.T13.c's ([`greenhouse`]), `surface` is not read, and
/// the Bond albedo and cloud fraction are T13.c's table's ([`SurfaceState::albedo`],
/// [`SurfaceState::cloud_fraction`]). Under [`SnapshotRules::Batch`]:
///
/// - step 4 is the partition's ([`partitioned_greenhouse`]): after escape (step 2) and the
///   climate's division (step 3), each greenhouse step holds every condensable as
///   [`partition`](crate::planetary::chem::partition) does over the climate `surface.climate_of`
///   gives for the step's mean surface temperature and the previous step's partial pressures, on
///   the basins and with the water's surface share `surface` gives its own seas' weight
///   (decision-p14-t51a-partition §4; decision-p14-t51a-part2 §4.2): the previous pass's, from
///   P14.T54.a;
/// - the grey greenhouse reads P14.T57.a's constants, refitted at Venus's and then Earth's
///   measured states ([`GreenhouseConstants::FITTED`]);
/// - after step 5 the Bond albedo and cloud fraction are the area mix's (P14.T51.b) over the
///   condensates the pass's partition publishes at its final climate, its regolith and its cloud
///   (`surface_mix::area_mixed`).
///
/// Every other step is the same.
#[must_use]
pub(crate) fn atmosphere_under(
    rules: SnapshotRules,
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
) -> Atmosphere {
    let air = state_and_greenhouse(rules, inputs, surface);
    match rules {
        SnapshotRules::Held => air,
        SnapshotRules::Batch => surface_mix::area_mixed(air, inputs, surface),
    }
}

/// Steps 1 to 5 of [`atmosphere_under`] under the lane's rules `rules`, over the surface `surface`:
/// the state, the greenhouse and the partial pressures, with T13.c's table's Bond albedo and cloud
/// fraction of the state.
#[must_use]
fn state_and_greenhouse(
    rules: SnapshotRules,
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
) -> Atmosphere {
    let cap = inputs.hottest_host.value();
    let saturate = |t: f64| Kelvin::new(if cap > 0.0 { t.min(cap) } else { t });
    let exobase = exobase_temperature(inputs.worst_equilibrium, inputs.xuv_fluence);
    let (m, r) = (inputs.mass.value(), inputs.radius.value());
    let valid = m.is_finite() && m > 0.0 && r.is_finite() && r > 0.0;
    let retention = if valid {
        Retention::of(inputs.mass, inputs.radius, exobase)
    } else {
        Retention::default()
    }
    .with_methane_as_nitrogen();
    let airless = |state: SurfaceState, partials: PartialPressures, t: f64, carrier| Atmosphere {
        state,
        surface_temperature: saturate(t),
        surface_pressure: Some(partials.total()),
        partials,
        retention,
        exobase_temperature: exobase,
        optical_depth: 0.0,
        albedo: state.albedo(inputs.material),
        cloud_fraction: state.cloud_fraction(),
        carbon_carrier: carrier,
        column: PartialPressures::default(),
        rules,
    };
    if inputs.envelope_fraction >= THIN_ENVELOPE_FRACTION {
        let state = SurfaceState::GasEnvelope;
        return Atmosphere {
            surface_pressure: None,
            ..airless(
                state,
                PartialPressures::default(),
                inputs.heated.value(),
                None,
            )
        };
    }
    if !valid {
        return airless(
            SurfaceState::Airless,
            PartialPressures::default(),
            inputs.equilibrium.value(),
            None,
        );
    }

    // Steps 2 and 3: what escape leaves, and how the climate divides it.
    let mut airborne = Airborne::kept(inputs, retention);
    let climate = airborne.divide(inputs, retention);

    // Step 4: the partial pressures and the greenhouse they hold.
    let equilibrium = inputs.equilibrium;
    let gravity = MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r));
    let weight = gravity.value() / (4.0 * core::f64::consts::PI * r * r);
    let how = Greenhouse {
        rules,
        climate_of: surface.climate_of,
        ground: Ground::new(gravity, surface.basins, surface.water_at_surface),
    };
    let step = greenhouse_under(&how, &airborne, weight, equilibrium, &saturate);
    let Some((mut t, mut partials, tau)) = step else {
        // A weight that is not finite, of a body far smaller than any the generator makes, holds
        // nothing it can weigh: the body is airless, as one of no mass or radius is.
        return airless(
            SurfaceState::Airless,
            PartialPressures::default(),
            equilibrium.value(),
            None,
        );
    };
    let carrier = Some(airborne.carbon_carrier());
    let column = airborne.column_weights(weight);

    // Step 5: the state.
    let molten = climate == SurfaceState::MagmaOcean || t >= SILICATE_SOLIDUS.value();
    if molten && airborne.oxygen > 0.0 {
        partials.0[Gas::Oxygen.index()] = Pascals::ZERO;
    }
    let total = partials.total();
    let state = if molten {
        t = t.max(SILICATE_SOLIDUS.value());
        SurfaceState::MagmaOcean
    } else if total < AIRLESS_PRESSURE {
        let air = airless(
            SurfaceState::Airless,
            partials,
            equilibrium.value(),
            carrier,
        );
        return Atmosphere { column, ..air };
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
        carbon_carrier: carrier,
        column,
        rules,
    }
}

#[cfg(test)]
mod tests;
