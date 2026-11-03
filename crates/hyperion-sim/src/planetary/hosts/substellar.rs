//! Substellar hosts: a free-floating brown dwarf's planets (plan 14, design note 13, P14.T27.a),
//! from generator version 26.
//!
//! A free-floating brown dwarf is plan 13's object of layer F, 13 Jupiter masses to 0.08 M☉, with
//! plan 06's stellar state: a single [`StarModel`](crate::stellar::system::StarModel) on the
//! cooling fits ([`substellar::cooling`](crate::stellar::substellar::cooling)). From version 26
//! it runs the stellar path with three changes, each keyed on the context's
//! [`HostKind::BrownDwarf`]:
//!
//! - **The context.** [`SystemContext::for_system`] gives it one, where it was refused to 25, and
//!   [`SystemContext::from_stars`] calls its host [`HostKind::BrownDwarf`], so that a caller that
//!   holds its stars (the server's body cache) builds the same context
//!   ([`SystemContext::for_stars`]).
//! - **The disc** (P14.T3) reads the cooling fit's luminosity and radius at 10 Myr, design note
//!   6's rule for a substellar host, which has no main sequence, where a star of 0.08–0.1 M☉ reads
//!   its fits at 5 Gyr ([`SystemContext::zone_stars`]). Its mass, extent and solids scale with the
//!   host's mass as every disc's do ([`disc`](crate::planetary::disc)). The host's X-ray and
//!   ultraviolet history follows the same 10 Myr luminosity ([`SystemContext::xuv_history`]), as a
//!   brown-dwarf companion's does.
//! - **The class** (P14.T4) is drawn on [`brown_dwarf_class_weights`]: `Barren` or the
//!   `SubstellarCompact` chain, never a giant (design note 13: "no giants, a compact chain or
//!   nothing"). P14.T5–T8 place the chain's template as they place a star's.
//!
//! Nothing else changes, and nothing needs to: P14.T12's light, radius and temperature are the
//! host's [`StarModel::state_at`](crate::stellar::system::StarModel::state_at), the cooling fits at
//! its age plus the clock time, so its planets cool with it and its habitable zone sweeps inward
//! across them; a host above the fits' hydrogen-burning limit (about 0.07 M☉ at solar
//! metallicity, [`hydrogen_burning_limit`](crate::stellar::substellar::hydrogen_burning_limit))
//! stops cooling once it settles on its floor, and its zone with it. Its ultraviolet drives no
//! photochemistry, no ozone, haze or acid, as the stage already had it for this host kind and has
//! it for a brown-dwarf companion.
//!
//! **Only free-floating objects come here** (design note 13). A brown dwarf bound to a star is
//! plan 11's component, and design note 10 treats it as one: its zone keeps the stellar row's
//! `SubstellarCompact` weight, continuous with the stars' at 0.08 M☉.
//!
//! # Rogue planets (P14.T27.b)
//!
//! A rogue planet is plan 13's object of layer G, 0.3 M⊕ to 13 Jupiter masses, with its record and
//! its metallicity and no star. From generator version 26 it runs the satellite path only, as a
//! planet without a star (design note 13):
//!
//! - **The context** ([`SystemContext::for_system`], [`HostKind::RoguePlanet`]) holds no star: no
//!   stars, zones or hierarchy, and the object's mass, age and composition in their place.
//! - **The object** is body `0x0000`, derived by P14.T16 with no star on the orbit it was born on
//!   ([`BirthOrbit`]), drawn on its own `planet.origin`: its equilibrium temperature is the cosmic
//!   background's, 2.725 48 K, and it radiates its own heat, plan 13's giant cooling from 0.3
//!   Jupiter masses and P14.T24.b's radiogenic and residual heat below it. From [`EJECTED_BELOW`]
//!   it formed alone, and its metals are its cloud's, its composition's Z. What needs a star is
//!   `NotModelled` (`planetary::system`'s `rogue` module).
//! - **Its satellites** are P14.T17–T20's about it on its birth orbit, with design note 13's
//!   ejection cut: under [`EJECTED_BELOW`], one Jupiter mass, it was ejected and keeps only the
//!   moons inside [`EJECTED_MOON_HILL_SHARE`] of its birth Hill radius; heavier, it formed alone
//!   and keeps them all. Its moons are `0x0100` upward, its rings `0x0080` upward.
//!
//! [`HostKind::BrownDwarf`]: crate::planetary::context::HostKind::BrownDwarf
//! [`HostKind::RoguePlanet`]: crate::planetary::context::HostKind::RoguePlanet

use core::f64::consts::TAU;

use crate::galaxy::substellar::BROWN_DWARF_MIN_MSUN;
use crate::id::BodyId;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::planetary::architecture::{
    ArchitectureClass, CLASS_COUNT, ClassWeights, small_planet_metallicity_scaling,
};
#[cfg(doc)]
use crate::planetary::context::SystemContext;
use crate::planetary::derive::{FormationHost, hill_radius};
use crate::planetary::disc::snow_line;
use crate::planetary::placement::classes::orbits::SystemPlane;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::Composition;
use crate::stellar::sse::{ZCoeffs, zams};
use crate::units::{
    AstronomicalUnits, Dex, EarthMasses, GravitationalParameter, JupiterMasses, Kilograms, Metres,
    Radians, SolarMasses,
};
use crate::{Seed, math};

/// The weight of `Barren` about a free-floating brown dwarf: 0.5, against
/// [`BROWN_DWARF_CHAIN_WEIGHT`]'s 0.5 at 0.05 M☉ (plan 14, P14.T27.a's figures).
///
/// Ruling 48 (a) left the `SubstellarCompact` weight of a brown-dwarf host to P14.T27, from an
/// occurrence estimate for ultracool dwarfs. None has been measured for brown-dwarf hosts: the
/// ultracool dwarfs' rates are of late M stars such as TRAPPIST-1, and transit searches of brown
/// dwarfs give upper limits on close-in planets only (He, Triaud and Gillon 2017, MNRAS 464, 2687,
/// from Spitzer's monitoring of 44 brown dwarfs), which no figure here exceeds. The discs
/// are there, about as often as about stars (Luhman et al. 2005, ApJ 631, L69), and formation
/// models grow Earth-mass bodies in them (Payne and Lodato 2007, MNRAS 381, 1597), but their solids
/// fall faster than linearly with the host's mass (Pascucci et al. 2016, ApJ 831, 125). The even
/// split is plan 14's judgement between those, as `Barren`'s 0.24 is about a star (the class
/// table's Risks). The sources are cited from memory and not re-checked: this lane has no web
/// access.
pub const BROWN_DWARF_BARREN_WEIGHT: f64 = 0.5;

/// The weight of the `SubstellarCompact` chain about a free-floating brown dwarf of
/// [`BROWN_DWARF_CHAIN_HELD_FROM`] (0.05 M☉) and more: 0.5 (plan 14, P14.T27.a; the sources are
/// [`BROWN_DWARF_BARREN_WEIGHT`]'s).
///
/// Above 0.05 M☉ the plan gives no weight, and it is held to 0.08 M☉ (lean, 2026-10-31): the
/// smallest choice. A free-floating host of just under 0.08 M☉ then has a chain half the time, and
/// a star of 0.08 M☉ nearly always (the stellar row's `TerrestrialOnly` and `CompactMulti`), a
/// step at the hydrogen-burning limit that no survey resolves.
pub const BROWN_DWARF_CHAIN_WEIGHT: f64 = 0.5;

/// The host mass from which a free-floating brown dwarf's chain takes its whole weight,
/// [`BROWN_DWARF_CHAIN_WEIGHT`]: 0.05 M☉ (plan 14, P14.T27.a).
pub const BROWN_DWARF_CHAIN_HELD_FROM: SolarMasses = SolarMasses::new(0.05);

/// The weight of the `SubstellarCompact` chain about a free-floating brown dwarf at the
/// deuterium-burning limit, 13 Jupiter masses, the lightest plan 13 places
/// ([`BROWN_DWARF_MIN`](crate::galaxy::substellar::BROWN_DWARF_MIN)): 0.2, to which the weight
/// falls linearly in mass from [`BROWN_DWARF_CHAIN_HELD_FROM`] (plan 14, P14.T27.a), as the
/// solids of a lighter host's disc fall faster than its mass (Pascucci et al. 2016, ApJ 831, 125;
/// cited from memory, not re-checked).
///
/// It is a weight against [`BROWN_DWARF_BARREN_WEIGHT`], so such a host has a chain
/// 0.2 ÷ 0.7 of the time, 29%.
pub const DEUTERIUM_LIMIT_CHAIN_WEIGHT: f64 = 0.2;

/// The weight of the `SubstellarCompact` chain about a free-floating brown dwarf of initial mass
/// `mass`, before metallicity: [`DEUTERIUM_LIMIT_CHAIN_WEIGHT`] at 13 Jupiter masses, rising
/// linearly in mass to [`BROWN_DWARF_CHAIN_WEIGHT`] at [`BROWN_DWARF_CHAIN_HELD_FROM`] and held
/// there; held at its end below 13 Jupiter masses, which no brown dwarf has.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::hosts::substellar::brown_dwarf_chain_weight;
/// use hyperion_sim::units::{JupiterMasses, SolarMasses};
///
/// let weight = |m: f64| brown_dwarf_chain_weight(SolarMasses::new(m));
/// assert!((weight(0.05) - 0.5).abs() < 1e-15);
/// assert!((weight(0.079) - 0.5).abs() < 1e-15);
/// let deuterium = brown_dwarf_chain_weight(SolarMasses::from(JupiterMasses::new(13.0)));
/// assert!((deuterium - 0.2).abs() < 1e-12);
/// assert!(weight(0.03) > deuterium && weight(0.03) < 0.5);
/// ```
#[must_use]
pub fn brown_dwarf_chain_weight(mass: SolarMasses) -> f64 {
    let m = mass.value();
    debug_assert!(
        m.is_finite() && m > 0.0,
        "a host mass is positive and finite, got {m}"
    );
    let (lo, hi) = (BROWN_DWARF_MIN_MSUN, BROWN_DWARF_CHAIN_HELD_FROM.value());
    if m >= hi {
        BROWN_DWARF_CHAIN_WEIGHT
    } else {
        let across = ((m - lo) / (hi - lo)).max(0.0);
        DEUTERIUM_LIMIT_CHAIN_WEIGHT
            + (BROWN_DWARF_CHAIN_WEIGHT - DEUTERIUM_LIMIT_CHAIN_WEIGHT) * across
    }
}

/// The class weights of a free-floating brown dwarf of initial mass `mass` and metallicity `fe_h`
/// (P14.T27.a): `Barren` at [`BROWN_DWARF_BARREN_WEIGHT`] and `SubstellarCompact` at
/// [`brown_dwarf_chain_weight`], every other class at zero, so that it never has a giant.
///
/// The chain scales with \[Fe/H\] as the class table's `SubstellarCompact` row does, by the small
/// planets' s(\[Fe/H\]) ([`small_planet_metallicity_scaling`]), and `Barren` takes what the halo
/// takes from it, as it does about a star
/// ([`class_weights`](crate::planetary::architecture::class_weights)).
/// A brown dwarf bound to a star keeps the table's row (design note 10).
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite or `fe_h` is not finite.
///
/// # Examples
///
/// Half of the brown dwarfs of 0.05 M☉ and more host a chain, and none a giant:
///
/// ```
/// use hyperion_sim::planetary::architecture::ArchitectureClass;
/// use hyperion_sim::planetary::hosts::substellar::brown_dwarf_class_weights;
/// use hyperion_sim::units::{Dex, SolarMasses};
///
/// let p = brown_dwarf_class_weights(SolarMasses::new(0.06), Dex::ZERO).probabilities();
/// assert!((p.get(ArchitectureClass::SubstellarCompact) - 0.5).abs() < 1e-3);
/// assert!(p.giant_share() <= 0.0);
/// ```
#[must_use]
pub fn brown_dwarf_class_weights(mass: SolarMasses, fe_h: Dex) -> ClassWeights {
    let chain = brown_dwarf_chain_weight(mass);
    let small = small_planet_metallicity_scaling(fe_h);
    let mut weights = [0.0; CLASS_COUNT];
    weights[ArchitectureClass::Barren.index()] = BROWN_DWARF_BARREN_WEIGHT + chain * (1.0 - small);
    weights[ArchitectureClass::SubstellarCompact.index()] = chain * small;
    ClassWeights::from_array(weights)
}

/// The mass from which a rogue planet formed alone, rather than being ejected from the system it
/// formed in: one Jupiter mass (design note 13). Below it a rogue planet keeps only the moons
/// inside [`EJECTED_MOON_HILL_SHARE`] of its birth orbit's Hill radius, and its heavy elements are
/// a disc-formed planet's; from it, every moon its generators give it
/// ([`BirthOrbit::moon_bound`]), and a bulk metal fraction of its own composition's Z, a cloud
/// fragment's.
pub const EJECTED_BELOW: JupiterMasses = JupiterMasses::new(1.0);

/// The share of its birth orbit's Hill radius inside which an ejected planet keeps its moons: a
/// tenth (design note 13, after Hong et al. 2018, ApJ 852, 85, whose N-body runs of planet–planet
/// scattering find the moons of ejected planets kept close in; cited from memory and not
/// re-checked at source, which this lane cannot reach).
pub const EJECTED_MOON_HILL_SHARE: f64 = 0.1;

/// The mass of a rogue planet's birth host: 1 M☉ (lean, 2026-10-31). Plan 13 draws none; the
/// scattering that ejects a planet is a giant planets', and Hong et al.'s (2018) systems scatter
/// about a Sun-like star.
pub const BIRTH_HOST_MASS: SolarMasses = SolarMasses::new(1.0);

/// The bounds of a rogue planet's birth semi-major axis, au, between which it is drawn log-uniform:
/// 1 to 30 (lean, 2026-10-31), the Solar System's planets from Earth's orbit to Neptune's, across
/// the snow line of [`BIRTH_HOST_MASS`]'s disc at about 2.3 au, so that about a quarter of rogue
/// planets formed inside it.
pub const BIRTH_AXIS_AU: (f64, f64) = (1.0, 30.0);

/// The first word of `planet.origin` a rogue planet's birth orbit reads, on its object's key (body
/// `0x0000`, which no planet has): its semi-major axis's rank at word 2, and its plane's cos i and
/// node at words 3 and 4, words that a planet's formation reserves
/// ([`FormationDraws`](crate::planetary::hosts::young::FormationDraws)).
pub const BIRTH_ORBIT_WORD: u64 = 2;

/// Whether a rogue planet of mass `mass` was ejected from the system it formed in: under
/// [`EJECTED_BELOW`] (design note 13).
#[must_use]
pub fn ejected(mass: EarthMasses) -> bool {
    mass < EarthMasses::from(EJECTED_BELOW)
}

/// A rogue planet's birth orbit (P14.T27.b, design note 13): circular, about a host of
/// [`BIRTH_HOST_MASS`] at its zero-age main sequence, at a drawn semi-major axis, in an isotropic
/// plane.
///
/// It is the orbit the planet formed on, which its formation (P14.T11, as about any host) and its
/// moon generators (P14.T17–T20, its Hill sphere bounding its moons) read. Design note 13 draws it
/// for an ejected planet; one that formed alone is read on it too (lean, 2026-10-31), since the
/// generators read a parent on an orbit, but keeps every moon they give it.
///
/// # Examples
///
/// An ejected Neptune keeps its moons inside a tenth of its birth Hill radius; a Jupiter all:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::placement::CellKey;
/// use hyperion_sim::id::{BodyId, Layer};
/// use hyperion_sim::planetary::hosts::substellar::BirthOrbit;
/// use hyperion_sim::units::{EarthMasses, JupiterMasses};
///
/// let cell = CellKey::new(Layer::RoguePlanet, [0, 6_500, 0])?;
/// let system = cell.candidate_id(0).ok_or("inside the cell's index field")?;
/// let birth = BirthOrbit::draw(Seed::new(7), BodyId::new(system, 0x0000));
/// let neptune = EarthMasses::new(17.1);
/// let bound = birth.moon_bound(neptune).ok_or("a Neptune is ejected")?;
/// assert!((bound.value() / birth.hill_radius(neptune).value() - 0.1).abs() < 1e-15);
/// assert!(birth.moon_bound(EarthMasses::from(JupiterMasses::new(2.0))).is_none());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BirthOrbit {
    semi_major_axis: Metres,
    plane: SystemPlane,
}

impl BirthOrbit {
    /// The birth orbit of the rogue planet whose object is `object`, body `0x0000` of its system,
    /// in the universe of `seed`: words [`BIRTH_ORBIT_WORD`] to 4 of its `planet.origin` stream.
    ///
    /// # Panics
    ///
    /// Never: an open uniform gives a finite axis inside [`BIRTH_AXIS_AU`] and a valid plane.
    #[must_use]
    pub fn draw(seed: Seed, object: BodyId) -> Self {
        let mut stream = Stream::open(seed, tags::PLANET_ORIGIN, ObjectKey::from(object));
        stream.seek(BIRTH_ORBIT_WORD);
        let (lo, hi) = BIRTH_AXIS_AU;
        let au = lo * math::powf(hi / lo, stream.uniform_open());
        let cos_i = 1.0 - 2.0 * stream.uniform_open();
        let node = TAU * stream.uniform_open();
        Self {
            semi_major_axis: Metres::from(AstronomicalUnits::new(au)),
            plane: SystemPlane::new(Radians::new(math::acos(cos_i)), Radians::new(node))
                .expect("an inclination in [0, π] and a finite node make a plane"),
        }
    }

    /// The orbit's semi-major axis.
    #[must_use]
    pub const fn semi_major_axis(&self) -> Metres {
        self.semi_major_axis
    }

    /// The orbit's plane.
    #[must_use]
    pub const fn plane(&self) -> SystemPlane {
        self.plane
    }

    /// The orbit's elements for a planet of mass `mass`: circular, in its plane, with μ = G (M +
    /// m) about [`BIRTH_HOST_MASS`], at periapsis at the epoch.
    ///
    /// # Panics
    ///
    /// Never for a positive, finite mass.
    #[must_use]
    pub fn elements(&self, mass: EarthMasses) -> KeplerElements {
        let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(
            BIRTH_HOST_MASS.value() + SolarMasses::from(mass).value(),
        ));
        let orientation =
            Orientation::new(self.plane.inclination(), self.plane.node(), Radians::ZERO)
                .expect("a plane's inclination and node make an orientation");
        KeplerElements::from_semi_major_axis(
            self.semi_major_axis,
            mu,
            Eccentricity::new(0.0).expect("a circle is an ellipse"),
            orientation,
            Radians::ZERO,
        )
        .expect("a birth orbit's axis and parameter are positive")
    }

    /// The orbit's circular Hill radius for a planet of mass `mass`, a (m ÷ 3M)^⅓ about
    /// [`BIRTH_HOST_MASS`] (P14.T15's [`hill_radius`]).
    #[must_use]
    pub fn hill_radius(&self, mass: EarthMasses) -> Metres {
        hill_radius(
            self.semi_major_axis,
            0.0,
            Kilograms::from(mass),
            Kilograms::from(BIRTH_HOST_MASS),
        )
    }

    /// The apocentre inside which a rogue planet of mass `mass` keeps its moons: for an ejected
    /// one ([`ejected`]) [`EJECTED_MOON_HILL_SHARE`] of its [`hill_radius`](Self::hill_radius),
    /// and none for one that formed alone, which keeps them all.
    #[must_use]
    pub fn moon_bound(&self, mass: EarthMasses) -> Option<Metres> {
        ejected(mass).then(|| self.hill_radius(mass) * EJECTED_MOON_HILL_SHARE)
    }
}

/// A rogue planet's birth host as its formation reads it (P14.T27.b): [`BIRTH_HOST_MASS`] at its
/// zero-age main sequence (plan 06's [`zams::luminosity`]) in the composition `composition`, the
/// planet's own, with its disc's snow line (P14.T3.b's [`snow_line`]), and its rock of Mg/Si
/// `magnesium_to_silicon` and \[α/Fe\] `alpha_fe`, the planet's own system's.
#[must_use]
pub(crate) fn birth_host(
    composition: &Composition,
    magnesium_to_silicon: f64,
    alpha_fe: Dex,
) -> FormationHost {
    let luminosity = zams::luminosity(BIRTH_HOST_MASS, &ZCoeffs::new(composition.z_fit()));
    FormationHost {
        mass: BIRTH_HOST_MASS,
        luminosity,
        snow_line: snow_line(luminosity),
        magnesium_to_silicon,
        alpha_fe,
    }
}

#[cfg(test)]
mod rogue_tests;

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::SystemKind;
    use crate::galaxy::placement::{CellKey, Existence, SystemRecord, generate_cell};
    use crate::id::Layer;
    use crate::planetary::HostKind;
    use crate::planetary::architecture::{SUBSTELLAR_LIMIT, class_weights};
    use crate::planetary::context::SystemContext;
    use crate::planetary::hosts::evolved::circularised;
    use crate::planetary::params::{HILL_STABLE_GAP, SPACING_GIANT_MASS};
    use crate::planetary::placement::{OrbitHost, mutual_hill_radius};
    use crate::planetary::system::{PlanetarySystem, generate, generate_planets};
    use crate::stellar::multiplicity::SlotKind;
    use crate::stellar::system::SystemStars;
    use crate::time::{CLOCK_WINDOW_H, ClockWindow, UniverseTime};
    use crate::units::{AstronomicalUnits, EarthMasses, JupiterMasses, Metres, Years};

    const SEED: u64 = 0x0e14_0027_a000_0001;

    fn galaxy() -> &'static Galaxy {
        static GALAXY: std::sync::OnceLock<Galaxy> = std::sync::OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral")
        })
    }

    fn seed() -> Seed {
        galaxy().seed()
    }

    /// The first `n` free-floating brown dwarfs of the cells of layer F about the solar circle,
    /// 26,000 ly out on +y, in a fixed walk of the cells: a slab two cells thick about the plane,
    /// widening outwards from the cell at the Sun-like point.
    fn brown_dwarfs(n: usize) -> Vec<SystemRecord> {
        let mut found = Vec::with_capacity(n);
        let mut cell = Vec::new();
        'walk: for ring in 0_i32.. {
            for x in -ring..=ring {
                for y in -ring..=ring {
                    if x.abs().max(y.abs()) != ring {
                        continue;
                    }
                    for z in [-1, 0] {
                        let key = CellKey::new(Layer::BrownDwarf, [x, 1_625 + y, z])
                            .expect("the solar circle lies inside the cube");
                        generate_cell(galaxy(), key, &mut cell);
                        found.extend(cell.iter().copied());
                        if found.len() >= n {
                            break 'walk;
                        }
                    }
                }
            }
        }
        found.truncate(n);
        assert!(
            found.iter().all(|r| r.kind() == SystemKind::BrownDwarf),
            "layer F places brown dwarfs alone"
        );
        found
    }

    /// The context and planets of `record`, as the server's body cache builds them: its stars, then
    /// [`SystemContext::for_stars`].
    fn hosted(record: &SystemRecord) -> (SystemContext, PlanetarySystem) {
        let stars = SystemStars::generate(galaxy(), record);
        let ctx = SystemContext::for_stars(galaxy(), &stars).expect("a brown dwarf is generated");
        let planets = generate_planets(seed(), &ctx);
        (ctx, planets)
    }

    /// The weights of P14.T27.a: `Barren` 0.5 and the chain 0.5 at 0.05 M☉, held to 0.08 M☉
    /// (lean), the chain falling linearly in mass to 0.2 at 13 Jupiter masses; nothing else,
    /// and the halo's loss to `Barren`.
    #[test]
    fn a_brown_dwarf_draws_nothing_or_a_chain_and_the_chain_fades_to_the_deuterium_limit() {
        let deuterium = SolarMasses::from(JupiterMasses::new(13.0));
        assert_same_bits(deuterium.value(), BROWN_DWARF_MIN_MSUN);
        let chain_share = |m: SolarMasses| {
            brown_dwarf_class_weights(m, Dex::ZERO)
                .probabilities()
                .get(ArchitectureClass::SubstellarCompact)
        };
        for m in [0.05, 0.06, 0.07, 0.079_999] {
            let w = brown_dwarf_class_weights(SolarMasses::new(m), Dex::ZERO);
            for class in ArchitectureClass::ALL {
                match class {
                    ArchitectureClass::Barren | ArchitectureClass::SubstellarCompact => {}
                    _ => assert_same_bits(w.get(class), 0.0),
                }
            }
            assert!((w.total() - 1.0).abs() < 1e-15, "{m}: {}", w.total());
            assert!((chain_share(SolarMasses::new(m)) - 0.5).abs() < 1e-3, "{m}");
        }
        assert!((chain_share(deuterium) - 0.2 / 0.7).abs() < 1e-3);
        // Linear in mass between the two ends.
        let mid = SolarMasses::new(f64::midpoint(deuterium.value(), 0.05));
        assert!((brown_dwarf_chain_weight(mid) - 0.35).abs() < 1e-12);
        let mut last = 0.0;
        for i in 0..=100 {
            let m = deuterium.value() + (0.08 - deuterium.value()) * f64::from(i) / 100.0;
            let w = brown_dwarf_chain_weight(SolarMasses::new(m));
            assert!(
                w >= last,
                "the chain's weight never falls with mass, at {m}"
            );
            last = w;
        }
        // The halo thins the chain into `Barren`, as about a star.
        let halo = brown_dwarf_class_weights(SolarMasses::new(0.06), Dex::new(-2.0));
        assert!((halo.total() - 1.0).abs() < 1e-15);
        let s = small_planet_metallicity_scaling(Dex::new(-2.0));
        assert!((halo.get(ArchitectureClass::SubstellarCompact) - 0.5 * s).abs() < 1e-15);
        // A bound brown dwarf keeps the stellar row's continuous weight: nearly always a chain.
        let bound = class_weights(SolarMasses::new(0.06), Dex::ZERO).probabilities();
        assert!(bound.get(ArchitectureClass::SubstellarCompact) > 0.99);
        // The step at the hydrogen-burning limit (lean, 2026-10-31): a star of 0.08 M☉ has planets
        // nearly always, a free-floating brown dwarf just under it half the time.
        let star = class_weights(SUBSTELLAR_LIMIT, Dex::ZERO).probabilities();
        assert!(star.get(ArchitectureClass::Barren) < 0.01);
    }

    /// `for_system` and the server's path, `for_stars` of the system's stars, give a brown dwarf the
    /// same `HostKind::BrownDwarf` context, as `from_stars` and `from_record` do.
    #[test]
    fn for_system_and_the_body_cache_s_context_agree_for_a_brown_dwarf() {
        for record in brown_dwarfs(40) {
            let id = record.id();
            let stars = SystemStars::generate(galaxy(), &record);
            let context = SystemContext::for_system(galaxy(), id).expect("generated");
            assert_eq!(context.host_kind(), HostKind::BrownDwarf);
            assert_eq!(
                SystemContext::for_stars(galaxy(), &stars),
                Ok(context.clone())
            );
            assert_eq!(SystemContext::from_stars(galaxy(), &stars), context);
            assert_eq!(SystemContext::from_record(galaxy(), &record), context);
            assert_eq!(context.stars().len(), 1);
            assert_eq!(
                context.hierarchy().unwrap().stars()[0].kind(),
                SlotKind::Star
            );
            assert!(context.hierarchy().unwrap().stars()[0].initial_mass() < SUBSTELLAR_LIMIT);
        }
    }

    /// Design note 6's substellar rule: a free-floating brown dwarf's disc reads its cooling fit
    /// at 10 Myr, where the same object read as a star would take its fits at 5 Gyr.
    #[test]
    fn a_brown_dwarf_s_disc_reads_its_cooling_fit_at_ten_myr() {
        for record in brown_dwarfs(20) {
            let (ctx, _) = hosted(&record);
            let mass = record.primary_initial_mass();
            let cooled = |age: f64| {
                crate::stellar::substellar::cooling(mass, Years::new(age), ctx.composition())
                    .expect("a brown dwarf's mass lies inside the cooling fits")
            };
            let young = cooled(1.0e7);
            let zone_star = ctx.zone_stars()[0];
            assert_same_bits(
                zone_star.zams_luminosity.value(),
                young.luminosity().value(),
            );
            assert_same_bits(zone_star.zams_radius.value(), young.radius().value());
            // Brighter young than old: the disc sees the young object.
            assert!(zone_star.zams_luminosity > cooled(5.0e9).luminosity());
        }
    }

    /// The census of [`no_brown_dwarf_has_a_giant_and_its_chains_never_overlap`].
    #[derive(Debug, Default)]
    struct Census {
        hosts: usize,
        empty: usize,
        chains: usize,
        planets: usize,
        pairs: u64,
        empty_chains: usize,
        deuterium_end: (usize, usize),
    }

    /// P14.T10.a on one brown dwarf's planets: each inside its zone, and every pair with the inner
    /// apocentre at least 2√3 mutual Hill radii below the outer pericentre at the epoch and at ±H,
    /// the orbits circularised to the host's age then (P14.T8.e). Returns the pairs checked.
    fn assert_no_overlap(ctx: &SystemContext, system: &PlanetarySystem) -> u64 {
        let window = CLOCK_WINDOW_H.as_julian_years_f64();
        let mut pairs = 0;
        for zone in system.zones() {
            let m = zone.host_mass();
            let mut planets: Vec<_> = system
                .planets()
                .filter(|p| p.host() == zone.host())
                .collect();
            planets.sort_by(|a, b| {
                let axis = |p: &crate::planetary::Body| p.orbit().unwrap().semi_major_axis();
                axis(a).total_cmp(&axis(b))
            });
            for p in &planets {
                let orbit = p.orbit().unwrap();
                if let Some(inner) = zone.inner() {
                    assert!(orbit.periapsis() >= inner * (1.0 - 1e-12), "{:?}", ctx.id());
                }
                if let Some(outer) = zone.outer() {
                    assert!(orbit.apoapsis() <= outer * (1.0 + 1e-12), "{:?}", ctx.id());
                }
            }
            let age = ctx.age_at_epoch().value();
            for t in [age - window, age, age + window] {
                let now: Vec<(EarthMasses, Metres, f64)> = planets
                    .iter()
                    .map(|p| {
                        let orbit = circularised(
                            p.orbit().unwrap(),
                            p.circularisation().unwrap(),
                            Years::new(t.max(0.0)),
                        );
                        (
                            p.mass(),
                            orbit.semi_major_axis(),
                            orbit.eccentricity().value(),
                        )
                    })
                    .collect();
                for pair in now.windows(2) {
                    let ((m1, a1, e1), (m2, a2, e2)) = (pair[0], pair[1]);
                    let hill = mutual_hill_radius(m1, m2, m, a1, a2).value();
                    let gap = a2.value() * (1.0 - e2) - a1.value() * (1.0 + e1);
                    assert!(
                        gap >= HILL_STABLE_GAP * hill * (1.0 - 1e-12),
                        "{:?}: {gap} m against {HILL_STABLE_GAP} Hill radii",
                        ctx.id()
                    );
                    pairs += 1;
                }
            }
        }
        pairs
    }

    /// The plan's test (a), under the 26 arm: 10⁴ free-floating brown dwarfs place chains that
    /// satisfy P14.T10.a, none with a giant, about half with nothing. Each host is the chain of
    /// its stages: the class it drew is `draw_class` on [`brown_dwarf_class_weights`].
    #[test]
    fn no_brown_dwarf_has_a_giant_and_its_chains_never_overlap() {
        use crate::planetary::architecture::draw_class;
        use crate::planetary::placement::ZoneDiscInputs;
        let giant = EarthMasses::from(SPACING_GIANT_MASS);
        let deuterium_end = SolarMasses::new(0.02);
        let mut census = Census::default();
        let (mut drawn_barren, mut expected_empty, mut variance) = (0_usize, 0.0, 0.0);
        for record in brown_dwarfs(10_000) {
            let (ctx, system) = hosted(&record);
            assert_eq!(system.zones().len(), 1, "a single object has one zone");
            let zone = &system.zones()[0];
            let host = &system.hosts()[0];
            assert_eq!(host.host(), OrbitHost::Star(0));
            let inputs =
                ZoneDiscInputs::for_zone(seed(), ctx.id(), zone, &ctx.zone_stars(), ctx.fe_h())
                    .unwrap();
            let weights = brown_dwarf_class_weights(zone.host_mass(), ctx.fe_h());
            let constraints = zone.class_constraints(&inputs.derive());
            assert_eq!(
                host.drawn_class(),
                draw_class(seed(), ctx.id(), zone.host_number(), &weights, &constraints)
            );
            assert!(
                matches!(
                    host.class(),
                    ArchitectureClass::Barren | ArchitectureClass::SubstellarCompact
                ),
                "{:?}: {:?}",
                ctx.id(),
                host.class()
            );
            assert!(!host.class().has_giants());
            let planets = system.planets().count();
            for planet in system.planets() {
                assert!(planet.mass() < giant, "{:?}: a giant", ctx.id());
            }
            census.pairs += assert_no_overlap(&ctx, &system);
            census.hosts += 1;
            census.planets += planets;
            if planets == 0 {
                census.empty += 1;
                census.empty_chains += usize::from(host.drawn_class() != ArchitectureClass::Barren);
            } else {
                census.chains += 1;
            }
            let p = weights
                .constrained(&constraints)
                .probabilities()
                .get(ArchitectureClass::Barren);
            expected_empty += p;
            variance += p * (1.0 - p);
            drawn_barren += usize::from(host.drawn_class() == ArchitectureClass::Barren);
            if zone.host_mass() < deuterium_end {
                census.deuterium_end.0 += 1;
                census.deuterium_end.1 += usize::from(planets == 0);
            }
        }
        #[expect(clippy::cast_precision_loss, reason = "counts of 10⁴ are exact in f64")]
        let (empty, expected, per_host, per_chain) = (
            census.empty as f64 / census.hosts as f64,
            expected_empty / census.hosts as f64,
            census.planets as f64 / census.hosts as f64,
            census.planets as f64 / census.chains.max(1) as f64,
        );
        println!(
            "brown-dwarf hosts: {} hosts, {} with nothing ({empty:.4}, weights {expected:.4}; {} \
             of them chains that placed nothing), {} chains, {} planets ({per_host:.3} per host, \
             {per_chain:.3} per chain), {} pairs; below 0.02 M_sun {} of {} empty",
            census.hosts,
            census.empty,
            census.empty_chains,
            census.chains,
            census.planets,
            census.pairs,
            census.deuterium_end.1,
            census.deuterium_end.0,
        );
        assert_eq!(census.hosts, 10_000);
        assert!(
            (0.45..0.65).contains(&empty),
            "about half with nothing: {empty}"
        );
        // The draws follow the weights: the drawn `Barren` count against its expectation, normal
        // to a good approximation at 10⁴ hosts.
        #[expect(clippy::cast_precision_loss, reason = "a count of 10⁴ is exact in f64")]
        let z = (drawn_barren as f64 - expected_empty) / variance.sqrt();
        println!("drawn Barren {drawn_barren}, expected {expected_empty:.1}, z = {z:.2}");
        assert_p_value("drawn Barren", 2.0 * normal_cdf(-z.abs()), ALPHA);
        assert!(census.pairs > 10_000, "{}", census.pairs);
    }

    /// The plan's test (a), its second half, under the 26 arm: the habitable zone of a 0.05 M☉
    /// host moves inward with age, over its life and across the clock window, and a planet's
    /// equilibrium temperature falls with it. The same object at five ages places the same planets:
    /// its disc is the 10 Myr object's. The object is the first of the sample's brown dwarfs that,
    /// made 0.05 M☉, hosts a chain.
    #[test]
    fn the_habitable_zone_of_a_brown_dwarf_moves_inward_with_age() {
        let of = |record: &SystemRecord, age: f64| {
            SystemRecord::from_parts(
                record.id(),
                *record.epoch_position(),
                record.origin(),
                record.population(),
                SolarMasses::new(0.05),
                Years::new(age),
            )
        };
        let first = brown_dwarfs(40)
            .into_iter()
            .find(|record| hosted(&of(record, 1.0e9)).1.planets().next().is_some())
            .expect("a 0.05 M_sun host has a chain half the time");
        let at = |age: f64| of(&first, age);
        let mut last: Option<(Metres, Metres)> = None;
        let mut planets_then: Option<Vec<_>> = None;
        let mut last_temperature: Option<f64> = None;
        for age in [1.0e8, 3.0e8, 1.0e9, 3.0e9, 1.0e10] {
            let (ctx, system) = hosted(&at(age));
            assert_eq!(ctx.existence_at(UniverseTime::EPOCH), Existence::Exists);
            let zone = |t| {
                system
                    .habitable_zone_at(&ctx, OrbitHost::Star(0), t)
                    .expect("a born host has a zone")
                    .conservative()
            };
            let now = zone(UniverseTime::EPOCH);
            assert!(now.0 < now.1);
            // Inward across the window: before, now, after.
            let (before, after) = (zone(ClockWindow::START), zone(ClockWindow::END));
            assert!(
                before.0 > now.0 && now.0 > after.0,
                "{age}: {before:?} {now:?} {after:?}"
            );
            assert!(before.1 > now.1 && now.1 > after.1, "{age}");
            if let Some(younger) = last {
                assert!(
                    now.0 < younger.0 && now.1 < younger.1,
                    "{age}: {now:?} after {younger:?}"
                );
            }
            last = Some(now);
            let planets: Vec<_> = system
                .planets()
                .map(|p| (p.index(), *p.orbit().unwrap()))
                .collect();
            if let Some(then) = &planets_then {
                assert_eq!(
                    &planets, then,
                    "the disc is the young object's at every age"
                );
            }
            if let Some(&(index, _)) = planets.first() {
                let record = system.body_at(&ctx, index, UniverseTime::EPOCH).unwrap();
                let t_eq = record
                    .bulk()
                    .ok()
                    .unwrap()
                    .equilibrium_temperature()
                    .value();
                if let Some(warmer) = last_temperature {
                    assert!(t_eq < warmer, "{age}: {t_eq} K after {warmer} K");
                }
                last_temperature = Some(t_eq);
            }
            planets_then = Some(planets);
        }
        assert!(
            last_temperature.is_some(),
            "the host's planets were derived"
        );
        // Its zone at 10 Gyr lies well inside a hundredth of an au: a cold, dim host.
        let (inner, _) = last.unwrap();
        assert!(
            inner < Metres::from(AstronomicalUnits::new(0.01)),
            "{inner:?}"
        );
    }

    /// D4 under the 26 arm: a brown dwarf's system is the same every time and in every order, and
    /// whole generation, with its belts, satellites and records at the epoch, runs on every one of
    /// a sample with finite figures.
    #[test]
    fn a_brown_dwarf_s_system_is_the_same_every_time_and_in_every_order() {
        let records = brown_dwarfs(60);
        {
            let whole = |record: &SystemRecord| {
                let ctx = SystemContext::for_system(galaxy(), record.id()).unwrap();
                let system = generate(seed(), &ctx);
                let snapshot = system.snapshot_at(&ctx, UniverseTime::EPOCH);
                (system, snapshot)
            };
            let first: Vec<_> = records.iter().map(whole).collect();
            assert_order_independent(&records, whole);
            for ((system, snapshot), record) in first.iter().zip(&records) {
                let again = whole(record);
                assert_eq!((system, snapshot), (&again.0, &again.1));
                for body in snapshot.bodies() {
                    if let Some(bulk) = body.bulk().ok() {
                        let t = bulk.equilibrium_temperature().value();
                        assert!(t.is_finite() && t > 0.0, "{:?}: {t} K", record.id());
                    }
                }
            }
            assert!(
                first
                    .iter()
                    .any(|(system, _)| system.planets().next().is_some()),
                "some of the sample host planets"
            );
        };
    }
}
