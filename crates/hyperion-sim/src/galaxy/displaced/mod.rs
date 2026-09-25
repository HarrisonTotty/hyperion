//! Displaced objects: kicked remnants, runaways and walkaways, as further density components of
//! layers D and E (plan 08, P08.T8–T13).
//!
//! Each birth population's layer-E budget is split once per galaxy, by a fixed quadrature over
//! plan 06's kick law, into alive or retained, displaced classes by speed and time since death,
//! and gone ([`class_table`], P08.T9). The displaced classes are density components with fitted
//! dimensionless forms ([`forms`], P08.T10), bounded by the unimodal-factor rule ([`bound`],
//! P08.T11) and placed by the same thinning as everything else, with their marks drawn
//! conditionally on the class ([`marks`], P08.T12). Runaways and walkaways come from the same
//! machinery ([`runaway`]), and a displaced candidate can back out the site of its supernova for
//! plan 09's shared test ([`site`], P08.T13).
//!
//! What exists so far is P08.T1's layout: the class indices and kinds below, and the galaxy's
//! scales ([`scales`]). The class table, the forms, their bounds, the marks, the runaway model,
//! the binarity seam and the kick bins are built by the tasks named in each module, and nothing
//! reads a displaced class until P08.T12 switches them on with its version bump.
//!
//! Speeds are in units of the galaxy's circular speed `v_c` and times in units of `R_d ÷ v_c`
//! ([`GalaxyScales`]), as plan 15's form table has them.

pub mod binarity;
pub mod bound;
pub mod class_table;
pub mod forms;
pub mod kick_bins;
pub mod marks;
pub mod runaway;
pub mod scales;
pub mod site;

pub use scales::GalaxyScales;

/// The number of speed bins of the class table: edges at 0.25, 0.5, 0.85, 1.3, 1.75, 2.2 and 2.8
/// `v_c` (brainstorm, "Displaced objects: kicks and runaways"; plan 08, P08.T9.a).
pub const SPEED_BINS: usize = 8;

/// The number of age bins of the class table: edges at 0.1, 0.3, 1, 2, 4 and 8 `R_d ÷ v_c`
/// (brainstorm, "Displaced objects: kicks and runaways"; plan 08, P08.T9.a).
pub const AGE_BINS: usize = 7;

/// A speed bin of the class table, 0 to [`SPEED_BINS`] − 1: the kick's speed in units of the
/// circular speed, from below a quarter of it to above 2.8 times it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpeedBin(u8);

impl SpeedBin {
    /// The bin `index`, or `None` beyond the last.
    #[must_use]
    pub fn new(index: u8) -> Option<Self> {
        (usize::from(index) < SPEED_BINS).then_some(Self(index))
    }

    /// The bin's index, 0 to 7.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// An age bin of the class table, 0 to [`AGE_BINS`] − 1: the time since death in units of `R_d ÷
/// v_c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgeBin(u8);

impl AgeBin {
    /// The bin `index`, or `None` beyond the last.
    #[must_use]
    pub fn new(index: u8) -> Option<Self> {
        (usize::from(index) < AGE_BINS).then_some(Self(index))
    }

    /// The bin's index, 0 to 6.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// The birth population a displaced object's weight comes from (plan 08, Design note 14): the
/// thin disc is one source, young disc and sub-discs together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BirthSource {
    /// The young thin disc and the old thin disc's sub-discs.
    ThinDisc,
    /// The thick disc.
    ThickDisc,
    /// The stellar halo, summed over its components.
    Halo,
    /// The boxy bulge.
    Bulge,
    /// The long bar.
    LongBar,
    /// The nuclear disc.
    NuclearDisc,
}

/// A displaced class's place in the class list (`DisplacedFields::classes()`, P08.T12.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplacedClassId(u16);

impl DisplacedClassId {
    /// The class at `index` of the list.
    #[must_use]
    pub fn new(index: u16) -> Self {
        Self(index)
    }

    /// The index into the class list.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// What a displaced system is (plan 08, Provides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DisplacedKind {
    /// A remnant carried from its birth site by its natal kick.
    Remnant,
    /// A living star ejected from its birth cluster or binary at 30 km/s or more.
    Runaway,
    /// A living star ejected more slowly, under 30 km/s.
    Walkaway,
    /// A survivor of a Type Ia supernova's companion, moving at 1,900–2,500 km/s (registered with
    /// zero weight, Design note 25).
    HypervelocitySurvivor,
}

/// A system's placement class, the derived mark plans 09 and 11 read (plan 08, Provides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlacementClass {
    /// A living star, or any system outside layers D and E, in the field component that placed
    /// it.
    Alive,
    /// A remnant still in the field component that placed it, its kick in `speed`.
    Retained {
        /// The kick's speed bin.
        speed: SpeedBin,
    },
    /// A system of a displaced class.
    Displaced {
        /// The class.
        class: DisplacedClassId,
        /// What it is.
        kind: DisplacedKind,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bins_are_validated_indices() {
        assert_eq!(SpeedBin::new(7).map(SpeedBin::index), Some(7));
        assert_eq!(SpeedBin::new(8), None);
        assert_eq!(AgeBin::new(6).map(AgeBin::index), Some(6));
        assert_eq!(AgeBin::new(7), None);
        assert_eq!(DisplacedClassId::new(99).index(), 99);
    }
}
