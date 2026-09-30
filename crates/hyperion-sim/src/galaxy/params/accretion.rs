//! The accretion history: the last major merger, the accreted progenitors and the globular
//! clusters.
//!
//! Plans 09 and 10 read these; plan 02 only draws them (Design note 12 and "Scope"). The orbits
//! are drawn from broad provisional ranges that plan 10 revalidates.

use crate::units::{LightYears, Radians, SolarMasses, Years};

/// A progenitor's orbit about the galaxy, as its five elements.
///
/// The orientation is relative to the galactic frame: `inclination` from the +z axis, `node` the
/// longitude of the ascending node from +x, `phase` the orbital phase at the epoch measured from
/// pericentre. Plan 10 turns these into a track in the potential.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    apocentre: LightYears,
    pericentre: LightYears,
    inclination: Radians,
    node: Radians,
    phase: Radians,
}

impl Orbit {
    /// An orbit from its elements. The builder checks their ranges.
    #[must_use]
    pub const fn new(
        apocentre: LightYears,
        pericentre: LightYears,
        inclination: Radians,
        node: Radians,
        phase: Radians,
    ) -> Self {
        Self {
            apocentre,
            pericentre,
            inclination,
            node,
            phase,
        }
    }

    /// The largest distance from the centre.
    #[must_use]
    pub fn apocentre(&self) -> LightYears {
        self.apocentre
    }

    /// The smallest distance from the centre, not above the apocentre.
    #[must_use]
    pub fn pericentre(&self) -> LightYears {
        self.pericentre
    }

    /// The angle between the orbit's pole and +z, `[0, π]`.
    #[must_use]
    pub fn inclination(&self) -> Radians {
        self.inclination
    }

    /// The longitude of the ascending node from +x, `[0, 2π]`.
    #[must_use]
    pub fn node(&self) -> Radians {
        self.node
    }

    /// The orbital phase at the epoch from pericentre, `[0, 2π]`.
    #[must_use]
    pub fn phase(&self) -> Radians {
        self.phase
    }
}

/// Which progenitor a [`Progenitor`] is, and its progenitor number.
///
/// Progenitor numbers key the `galaxy.params.accretion.progenitor.*` streams: the dominant merger
/// is 0, lesser progenitor `n` is `n` (1–5, whether or not all five exist), recent progenitor `j`
/// is `6 + j`. Fixed slots mean that a change in how many lesser progenitors a galaxy has moves
/// none of the recent ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProgenitorKind {
    /// The last major merger, whose debris is the halo's dominant component.
    DominantMerger,
    /// Lesser old progenitor `n`, 1–5, whose debris is a lesser halo component.
    Lesser(u8),
    /// Recent progenitor `j`, from 0, accreted in the last 6 Gyr.
    Recent(u32),
}

impl ProgenitorKind {
    /// The progenitor number: 0, `n`, or `6 + j`.
    #[must_use]
    pub fn number(self) -> u64 {
        match self {
            Self::DominantMerger => 0,
            Self::Lesser(n) => u64::from(n),
            Self::Recent(j) => FIRST_RECENT_PROGENITOR + u64::from(j),
        }
    }
}

/// The progenitor number of the first recent progenitor: after the dominant merger and five
/// lesser slots.
pub(super) const FIRST_RECENT_PROGENITOR: u64 = 6;

/// An accreted galaxy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progenitor {
    pub(super) kind: ProgenitorKind,
    pub(super) mass: SolarMasses,
    pub(super) accreted: Years,
    pub(super) orbit: Orbit,
}

impl Progenitor {
    /// Which progenitor this is.
    #[must_use]
    pub fn kind(&self) -> ProgenitorKind {
        self.kind
    }

    /// The progenitor's stellar mass. For the dominant merger and the lesser progenitors it is
    /// the stellar mass of their halo component, the share times the halo's mass; a recent
    /// progenitor's is drawn from M^−1.45 on 10⁵–10⁹·⁵ M☉ (brainstorm, "Streams and accreted
    /// structure").
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// How long before the epoch it was accreted: the last major merger for the dominant one,
    /// 6–12 Gyr for a lesser one, 0–6 Gyr for a recent one. The dominant and lesser progenitors'
    /// halo components hold no star younger than this: star formation stops on infall.
    #[must_use]
    pub fn accreted(&self) -> Years {
        self.accreted
    }

    /// Its orbit. The dominant merger's is radial, with an eccentricity of 0.85–0.95, like the
    /// Gaia Sausage's (Belokurov et al. 2018); the others' pericentres are 0.05–0.6 of their
    /// apocentres.
    #[must_use]
    pub fn orbit(&self) -> &Orbit {
        &self.orbit
    }
}

/// The galaxy's accretion history.
#[derive(Debug, Clone, PartialEq)]
pub struct AccretionHistory {
    pub(super) last_major_merger: Years,
    pub(super) progenitors: Vec<Progenitor>,
    pub(super) globular_count: u32,
}

impl AccretionHistory {
    /// How long before the epoch the last major merger happened, 6–11 Gyr.
    #[must_use]
    pub fn last_major_merger(&self) -> Years {
        self.last_major_merger
    }

    /// The progenitors: the dominant merger, the lesser old progenitors by number, then the
    /// recent ones.
    #[must_use]
    pub fn progenitors(&self) -> &[Progenitor] {
        &self.progenitors
    }

    /// The number of globular clusters: the dark halo's mass ÷ 6.5 × 10⁹ M☉ with 0.2 dex of
    /// scatter, clamped to 80–800.
    #[must_use]
    pub fn globular_count(&self) -> u32 {
        self.globular_count
    }
}
