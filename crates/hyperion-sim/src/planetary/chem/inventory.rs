//! A body's element inventory: its volatile and redox-active elements by mass and reservoir
//! (P14.T50.a; decision-composition §1.2).
//!
//! [`ElementInventory`] replaces [`VolatileInventory`]'s four masses with the masses of eleven
//! elements, H, He, C, N, O, Ne, S, Cl, Ar, Kr and Xe ([`INVENTORY_ELEMENTS`]), in two
//! [`Reservoir`]s, with the mantle's one redox figure, ΔIW ([`redox`](super::redox)).
//!
//! - **The stored carriers.** P14.T13.a's water, carbon dioxide, nitrogen and radiogenic ⁴⁰Ar are
//!   kept as they are drawn ([`volatile_inventory`]), so that [`ElementInventory::volatiles`], the
//!   view of them that [`VolatileInventory`] now is, returns today's values to the bit. Their
//!   elements are their shares of each carrier by the element registry's weights, in the
//!   exchangeable reservoir.
//! - **Sulphur and chlorine**, in the mantle, each a log-normal multiple of
//!   [`VOLATILE_SCATTER_DEX`] about the bulk silicate Earth's per unit mass of the body's rock, at
//!   the ranks of words 3 and 4 of `planet.volatiles`. A body formed beyond the snow line takes
//!   chlorine at CI chondrites' abundance per unit mass of its rock instead: the bulk silicate
//!   Earth is depleted in chlorine against the chondrites, and an icy body's rock is not.
//!   P14.T51.g's oceans leach it.
//! - **The primordial noble gases**, He, Ne, Ar (its primordial ³⁶Ar and ³⁸Ar), Kr and Xe, in the
//!   exchangeable reservoir at CI chondrites' abundance per unit mass of the body's rock, with no
//!   draw (Lodders 2010, Table 2).
//!
//! What is not modelled is held at zero and named so: the mantle's own water, carbon and nitrogen,
//! which its outgassing will read (P14.T55.a); radiogenic ⁴He; the exchangeable reservoir's
//! sulphur and chlorine, which outgassing (T24.g's present outgassing, P14.T55.a) and the oceans'
//! leaching (P14.T51.g) move there.
//!
//! Nothing on the record reads this module until P14.T54.a, which wires it in the 22 → 23 batch,
//! so no generated output moves with it.

use crate::math;
use crate::planetary::chem::redox::{self, RedoxClass, RedoxInputs};
use crate::planetary::derive::atmosphere::{
    VOLATILE_SCATTER_DEX, VolatileDraws, VolatileInventory, volatile_inventory,
};
use crate::planetary::derive::composition::{MassFractions, SnowLineSide};
use crate::stellar::draws::UnitUniform;
use crate::substance::Element;
use crate::units::consts::EARTH_MASS_KG;
use crate::units::{Dex, EarthMasses, Kilograms, Metres, Years};

/// The bulk silicate Earth's sulphur per unit of its mass: 235 ± 38 ppm (Wang, Lineweaver and
/// Ireland 2018, Icarus 299, 460, arXiv:1708.08718, Table 1, read: the concordance of
/// Lyubetskaya and Korenaga 2007, McDonough and Arevalo 2008, the update of McDonough and Sun
/// 1995, and Palme and O'Neill 2014).
pub const BSE_SULPHUR_PER_MASS: f64 = 235e-6;

/// The bulk silicate Earth's chlorine per unit of its mass: 16.2 (+13.9, −14.8) ppm (Wang et al.
/// 2018, Table 1, read; the same concordance).
pub const BSE_CHLORINE_PER_MASS: f64 = 16.2e-6;

/// CI chondrites' chlorine per unit of their mass: 698 ± 105 ppm (Lodders 2010, arXiv:1010.2746,
/// Table 2, Orgueil, read; plan 14's "about 700 ppm").
pub const CI_CHLORINE_PER_MASS: f64 = 698e-6;

/// CI chondrites' helium per unit of their mass: 0.009 17 ppm (Lodders 2010, Table 2, read), which
/// counts their radiogenic ⁴He.
pub const CI_HELIUM_PER_MASS: f64 = 9.17e-9;

/// CI chondrites' neon per unit of their mass: 1.80 × 10⁻⁴ ppm (Lodders 2010, Table 2, read).
pub const CI_NEON_PER_MASS: f64 = 1.80e-10;

/// CI chondrites' argon per unit of their mass: 0.001 33 ppm (Lodders 2010, Table 2, read), their
/// trapped, primordial argon.
pub const CI_ARGON_PER_MASS: f64 = 1.33e-9;

/// CI chondrites' krypton per unit of their mass: 5.22 × 10⁻⁵ ppm (Lodders 2010, Table 2, read).
pub const CI_KRYPTON_PER_MASS: f64 = 5.22e-11;

/// CI chondrites' xenon per unit of their mass: 1.74 × 10⁻⁴ ppm (Lodders 2010, Table 2, read).
pub const CI_XENON_PER_MASS: f64 = 1.74e-10;

/// The elements an [`ElementInventory`] holds, in the element registry's order.
pub const INVENTORY_ELEMENTS: [Element; 11] = [
    Element::H,
    Element::He,
    Element::C,
    Element::N,
    Element::O,
    Element::Ne,
    Element::S,
    Element::Cl,
    Element::Ar,
    Element::Kr,
    Element::Xe,
];

/// Where in a body an element lies (decision-composition §1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reservoir {
    /// The air, the surface's ices and liquids, and the crust's stored carbonate: what escape,
    /// condensation and the climate divide.
    Exchangeable,
    /// The mantle, which outgassing draws on.
    Mantle,
}

impl Reservoir {
    /// Both reservoirs, in the order in which an element's total sums them.
    pub const ALL: [Self; 2] = [Self::Exchangeable, Self::Mantle];
}

/// Everything [`element_inventory`] reads of a body, as plain values.
///
/// The fields are public, as the volatiles' draws are. A mass that is not positive gives an empty
/// inventory; the distances must be positive and finite, as [`redox::mean_delta_iw`] requires.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InventoryInputs {
    /// The body's mass.
    pub mass: EarthMasses,
    /// Its bulk composition.
    pub fractions: MassFractions,
    /// The side of its host's snow line it formed on: the atmosphere's
    /// ([`DerivedBody::formed`](crate::planetary::derive::DerivedBody::formed)), which sets its
    /// carriers and its chlorine. The caller keeps it consistent with the distances, which set its
    /// ΔIW.
    pub formed: SnowLineSide,
    /// Where it formed from its host.
    pub formation_distance: Metres,
    /// Its host's snow line.
    pub snow_line: Metres,
    /// Its ranks on `planet.volatiles`.
    pub draws: VolatileDraws,
    /// Its age at the time, over which its ⁴⁰Ar grows.
    pub age: Years,
}

/// A body's element inventory: masses per element and reservoir, the stored carriers they come
/// from, and the mantle's ΔIW (P14.T50.a).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ElementInventory {
    carriers: VolatileInventory,
    sulphur: Kilograms,
    chlorine: Kilograms,
    helium: Kilograms,
    neon: Kilograms,
    primordial_argon: Kilograms,
    krypton: Kilograms,
    xenon: Kilograms,
    mantle_delta_iw: Dex,
}

impl ElementInventory {
    /// The stored carriers, P14.T13.a's water, carbon dioxide, nitrogen and radiogenic argon: the
    /// [`VolatileInventory`] that [`volatile_inventory`] draws, to the bit, which the atmosphere
    /// reads.
    #[must_use]
    pub const fn volatiles(&self) -> &VolatileInventory {
        &self.carriers
    }

    /// The mass of `element` in `reservoir`; zero for an element of neither
    /// [`INVENTORY_ELEMENTS`] nor its reservoir's.
    #[must_use]
    pub fn mass(&self, element: Element, reservoir: Reservoir) -> Kilograms {
        match reservoir {
            Reservoir::Exchangeable => self.exchangeable(element),
            Reservoir::Mantle => match element {
                Element::S => self.sulphur,
                Element::Cl => self.chlorine,
                // The mantle's water, carbon and nitrogen are not modelled (P14.T55.a), nor are
                // its noble gases; the rock-formers are no inventory's.
                Element::H
                | Element::He
                | Element::C
                | Element::N
                | Element::O
                | Element::Ne
                | Element::Ar
                | Element::Kr
                | Element::Xe
                | Element::Na
                | Element::Mg
                | Element::Al
                | Element::Si
                | Element::P
                | Element::K
                | Element::Ca
                | Element::Ti
                | Element::V
                | Element::Cr
                | Element::Mn
                | Element::Fe
                | Element::Ni
                | Element::Zn => Kilograms::ZERO,
            },
        }
    }

    /// The mass of `element` in both reservoirs, summed in [`Reservoir::ALL`]'s order.
    #[must_use]
    pub fn total(&self, element: Element) -> Kilograms {
        Reservoir::ALL
            .iter()
            .fold(Kilograms::ZERO, |sum, &r| sum + self.mass(element, r))
    }

    /// The primordial argon, ³⁶Ar and ³⁸Ar at CI chondrites' abundance, beside the radiogenic
    /// ⁴⁰Ar of [`volatiles`](Self::volatiles): their sum is the exchangeable argon.
    #[must_use]
    pub const fn primordial_argon(&self) -> Kilograms {
        self.primordial_argon
    }

    /// The mantle's oxygen fugacity relative to the iron–wüstite buffer, log₁₀ units
    /// ([`redox::delta_iw`]).
    #[must_use]
    pub const fn mantle_delta_iw(&self) -> Dex {
        self.mantle_delta_iw
    }

    /// The mantle's redox class, the banding of [`mantle_delta_iw`](Self::mantle_delta_iw).
    #[must_use]
    pub fn redox_class(&self) -> RedoxClass {
        RedoxClass::of(self.mantle_delta_iw)
    }

    /// The exchangeable mass of `element`: its share of each stored carrier, by the element
    /// registry's weights, then its primordial noble gas.
    #[must_use]
    fn exchangeable(&self, element: Element) -> Kilograms {
        let w = Element::standard_atomic_weight_g_per_mol;
        let (h, c, o) = (w(Element::H), w(Element::C), w(Element::O));
        let water = self.carriers.water().value();
        let carbon_dioxide = self.carriers.carbon_dioxide().value();
        Kilograms::new(match element {
            Element::H => water * (2.0 * h) / (2.0 * h + o),
            Element::C => carbon_dioxide * c / (c + 2.0 * o),
            Element::O => water * o / (2.0 * h + o) + carbon_dioxide * (2.0 * o) / (c + 2.0 * o),
            Element::N => self.carriers.nitrogen().value(),
            Element::Ar => self.carriers.argon().value() + self.primordial_argon.value(),
            Element::He => self.helium.value(),
            Element::Ne => self.neon.value(),
            Element::Kr => self.krypton.value(),
            Element::Xe => self.xenon.value(),
            // Sulphur and chlorine reach the surface by outgassing and leaching, not yet
            // modelled (T24.g, P14.T55.a, P14.T51.g); the rock-formers are no inventory's.
            Element::S
            | Element::Cl
            | Element::Na
            | Element::Mg
            | Element::Al
            | Element::Si
            | Element::P
            | Element::K
            | Element::Ca
            | Element::Ti
            | Element::V
            | Element::Cr
            | Element::Mn
            | Element::Fe
            | Element::Ni
            | Element::Zn => 0.0,
        })
    }
}

/// The element inventory of the body `inputs` describe (P14.T50.a).
///
/// - Its carriers are [`volatile_inventory`]'s of the same inputs, to the bit.
/// - Its sulphur is [`BSE_SULPHUR_PER_MASS`] of its rock times 10^(σ z), z the standard normal at
///   its sulphur rank and σ [`VOLATILE_SCATTER_DEX`]; its chlorine the same with
///   [`BSE_CHLORINE_PER_MASS`], or [`CI_CHLORINE_PER_MASS`] for a body formed beyond the snow line,
///   at its chlorine rank.
/// - Its primordial noble gases are CI chondrites' per unit mass of its rock.
/// - Its mantle's ΔIW is [`redox::delta_iw`] of its formation distance, its host's snow line, its
///   water fraction and its mass, at its redox rank.
///
/// # Panics
///
/// In debug builds, as [`redox::mean_delta_iw`]: if a distance is not positive and finite, or the
/// water fraction or the mass is not finite.
///
/// # Examples
///
/// An Earth at its median ranks holds the bulk silicate Earth's sulphur in its rock, and its
/// carriers are the atmosphere's inventory to the bit:
///
/// ```
/// use hyperion_sim::planetary::chem::inventory::{
///     BSE_SULPHUR_PER_MASS, ElementInventory, InventoryInputs, Reservoir, element_inventory,
/// };
/// use hyperion_sim::planetary::derive::atmosphere::{VolatileDraws, volatile_inventory};
/// use hyperion_sim::planetary::derive::{SnowLineSide, composition};
/// use hyperion_sim::substance::Element;
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, METRES_PER_AU};
/// use hyperion_sim::units::{EarthFluxes, EarthMasses, EarthRadii, Metres, Years};
///
/// let mass = EarthMasses::new(1.0);
/// let fractions = composition(mass, EarthRadii::new(1.0), SnowLineSide::Inside, EarthFluxes::new(1.0))?
///     .fractions();
/// let inputs = InventoryInputs {
///     mass,
///     fractions,
///     formed: SnowLineSide::Inside,
///     formation_distance: Metres::new(METRES_PER_AU),
///     snow_line: Metres::new(2.26 * METRES_PER_AU),
///     draws: VolatileDraws::MEDIAN,
///     age: Years::new(4.57e9),
/// };
/// let earth = element_inventory(&inputs);
/// let rock = fractions.rock() * EARTH_MASS_KG;
/// let sulphur = earth.mass(Element::S, Reservoir::Mantle).value();
/// assert!((sulphur / (BSE_SULPHUR_PER_MASS * rock) - 1.0).abs() < 1e-12);
/// let carriers = volatile_inventory(mass, &fractions, SnowLineSide::Inside, &VolatileDraws::MEDIAN, inputs.age);
/// assert_eq!(*earth.volatiles(), carriers);
/// # Ok::<(), hyperion_sim::planetary::derive::composition::SolveCompositionError>(())
/// ```
#[must_use]
pub fn element_inventory(inputs: &InventoryInputs) -> ElementInventory {
    let carriers = volatile_inventory(
        inputs.mass,
        &inputs.fractions,
        inputs.formed,
        &inputs.draws,
        inputs.age,
    );
    let kg = inputs.mass.value() * EARTH_MASS_KG;
    // Written as comparisons rather than `max`, so that a signed zero or a NaN gives +0 on every
    // target (the sim-determinism skill's "min and max are not exact at a signed zero").
    let rock_fraction = inputs.fractions.rock();
    let rock = if kg.is_finite() && kg > 0.0 && rock_fraction > 0.0 {
        rock_fraction * kg
    } else {
        0.0
    };
    let multiple =
        |rank: UnitUniform| math::exp10(VOLATILE_SCATTER_DEX * math::normal_quantile(rank.value()));
    let chlorine_per_mass = match inputs.formed {
        SnowLineSide::Inside => BSE_CHLORINE_PER_MASS,
        SnowLineSide::Beyond => CI_CHLORINE_PER_MASS,
    };
    let redox = RedoxInputs {
        formation_distance: inputs.formation_distance,
        snow_line: inputs.snow_line,
        water_fraction: inputs.fractions.water(),
        mass: inputs.mass,
    };
    ElementInventory {
        carriers,
        sulphur: Kilograms::new(BSE_SULPHUR_PER_MASS * rock * multiple(inputs.draws.sulphur)),
        chlorine: Kilograms::new(chlorine_per_mass * rock * multiple(inputs.draws.chlorine)),
        helium: Kilograms::new(CI_HELIUM_PER_MASS * rock),
        neon: Kilograms::new(CI_NEON_PER_MASS * rock),
        primordial_argon: Kilograms::new(CI_ARGON_PER_MASS * rock),
        krypton: Kilograms::new(CI_KRYPTON_PER_MASS * rock),
        xenon: Kilograms::new(CI_XENON_PER_MASS * rock),
        mantle_delta_iw: redox::delta_iw(&redox, inputs.draws.redox),
    }
}

#[cfg(test)]
mod tests;
