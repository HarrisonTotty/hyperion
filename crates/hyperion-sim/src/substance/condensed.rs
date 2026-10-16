//! The condensed columns: what a row is as a solid or a liquid on a surface or in a cloud, its
//! grain density, its transition (its first melt), and the family of its mechanical
//! rows (plan 14, P14.T49.b's second step; decision-composition §1.7;
//! science-composition-crusts.md §1.3, §2, §3, §4 and §7).
//!
//! Every row that can be a solid or a liquid and whose figures were read carries
//! [`CondensedColumns`] ([`Substance::condensed`]):
//!
//! - **the grain density** ([`GrainDensity`]) is intrinsic, the mass of the solid or the liquid
//!   over its own volume, with no pore space: porosity is how a material lies, which its reader
//!   states (science-r09-drainage.md §3). A row with a phase diagram takes its solids' `ρ_s`(T)
//!   and its saturated liquid's `ρ_l`(T) from it ([`GrainDensity::OfDiagram`]). Any other row
//!   keeps one value for each phase that was read, at a stated temperature
//!   ([`GrainDensity::Fixed`]): a rock or a salt at room temperature, with its error recorded in
//!   its sources (science-composition-crusts.md §3.2), a solution at its reference composition;
//! - **the transition** ([`Transition`]) is the row's first melt, for every kind: the lowest
//!   temperature at about one atmosphere at which its frozen assemblage holds any liquid at
//!   equilibrium, a pure substance's melting or triple point, a lithology's solidus, or a
//!   solution's eutectic of the solids its reference composition freezes to. Above it some of it
//!   is liquid; a solid that never melts at one atmosphere decomposes or sublimes there; an upper
//!   bound where only a bound is read, labelled one in its source ([`TransitionKind`] says which
//!   kind; decision-p14-t49b-condensed §2);
//! - **the mechanics family** ([`MechanicsFamily`]) is the family of R10's material rows the
//!   substance takes; a liquid's is its frozen form's.
//!
//! [`Substance::grain_density`] resolves the density of a phase at a temperature, which is what
//! P14.T49.c's palette resolution reads. P14.T49.c adds the condensed optics. P14.T49.j gives the
//! rows that can form a surface their solid's specific heat and conductivity, in
//! `substance::thermal`'s table by key, with the airless regolith's thermal-inertia law; the
//! geomorphic columns (`k_comp`, `k_target`, erodibility) are not part of the 22 → 23 batch
//! (decision-p14-t49c-split §1.3). P14.T49.h adds the liquids' optical and transport columns, and
//! the melts' densities, in their own group.
//!
//! Generator version 22 publishes none of it: nothing on the record reads these columns until
//! P14.T54.a wires the palette (decision-bump-plan-after-22 §1.2).

use hyperion_surface::field::MechanicsFamily;

use super::{Source, Substance};
use crate::units::{Kelvin, KilogramsPerCubicMetre};

/// A condensed phase: what a grain density is the density of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CondensedPhase {
    /// The solid: a crystal, a glass, a rock or an organic solid.
    Solid,
    /// The liquid: a pure liquid, a solution or a melt.
    Liquid,
}

/// One phase's grain density at the temperature it was measured or reduced at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DensityAt {
    pub(super) density: KilogramsPerCubicMetre,
    pub(super) at: Kelvin,
}

impl DensityAt {
    /// The density.
    #[must_use]
    pub const fn density(&self) -> KilogramsPerCubicMetre {
        self.density
    }

    /// The temperature it is given at.
    #[must_use]
    pub const fn at(&self) -> Kelvin {
        self.at
    }
}

/// How a row's grain density is given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GrainDensity {
    /// One value for each phase that was read, each at the temperature it was given at.
    ///
    /// A rock's or a salt's is at room temperature and kept at every temperature, with its error
    /// recorded (science-composition-crusts.md §3.2); a crystal's is its cell's, at the cell's
    /// temperature; a liquid's is at one atmosphere unless its source states another pressure,
    /// and a solution's at its reference composition. A phase whose density was not read is
    /// `None`, not modelled; at least one phase is given.
    Fixed {
        /// The solid's.
        solid: Option<DensityAt>,
        /// The liquid's.
        liquid: Option<DensityAt>,
    },
    /// The row's phase diagram's: each solid phase's measured `ρ_s`(T) from 0 K to the triple
    /// temperature, where one was read, and the saturated liquid's ancillary `ρ_l`(T), under its
    /// own vapour's pressure, from the triple to the critical temperature
    /// ([`PhaseDiagram`](super::PhaseDiagram)).
    OfDiagram,
}

/// What kind of first melt a row's transition is (the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionKind {
    /// A pure solid's melting point at one atmosphere.
    Melting,
    /// A lithology's solidus at about one atmosphere: where it first melts.
    Solidus,
    /// A solution's eutectic at one atmosphere, of the two solids its reference composition
    /// freezes to: the lowest temperature at which any of its liquid remains at equilibrium.
    ///
    /// For the brines, ammonia–water and Titan's liquid it is the solute–solvent eutectic; for
    /// the 75 wt% sulphuric acid, the dihydrate–monohydrate one (decision-p14-t49b-condensed
    /// §2.1). A metastable assemblage or a supercooled liquid can hold liquid below it.
    Eutectic {
        /// The solute's mass fraction at the eutectic, in (0, 1): a brine's salt, ammonia in the
        /// ammonia–water system, ethane in Titan's methane–ethane liquid, `H2SO4` in the aqueous
        /// acid.
        solute_mass_fraction: f64,
    },
    /// The temperature at which a solid decomposes at one atmosphere before it can melt.
    Decomposition,
    /// The temperature at which a solid's vapour reaches one atmosphere, where it sublimes
    /// before it can melt.
    Sublimation,
    /// The row's phase diagram's triple point, where its melting line meets its saturation curve,
    /// the lowest pressure at which its liquid exists.
    TriplePoint,
}

impl TransitionKind {
    /// The kind's name, as the registry's golden writes it.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Melting => "melting",
            Self::Solidus => "solidus",
            Self::Eutectic { .. } => "eutectic",
            Self::Decomposition => "decomposition",
            Self::Sublimation => "sublimation",
            Self::TriplePoint => "triple point",
        }
    }
}

/// A substance's first melt, with what kind of transition it is: above it some of the substance
/// is liquid, or, for a solid that never melts at one atmosphere, decomposed or sublimed
/// ([`TransitionKind`]); an upper bound where only a bound was read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    pub(super) kind: TransitionKind,
    pub(super) temperature: Kelvin,
}

impl Transition {
    /// What kind of transition it is.
    #[must_use]
    pub const fn kind(&self) -> TransitionKind {
        self.kind
    }

    /// Its temperature: the value a palette entry's `transition` holds.
    #[must_use]
    pub const fn temperature(&self) -> Kelvin {
        self.temperature
    }
}

/// A row's condensed columns (the [module documentation](self)), each value named with its
/// sources in [`sources`](Self::sources).
#[derive(Debug, Clone, PartialEq)]
pub struct CondensedColumns {
    pub(super) grain_density: GrainDensity,
    pub(super) transition: Transition,
    pub(super) mechanics: MechanicsFamily,
    pub(super) sources: &'static [Source],
}

impl CondensedColumns {
    /// How its grain density is given; [`Substance::grain_density`] resolves it.
    #[must_use]
    pub const fn grain_density(&self) -> GrainDensity {
        self.grain_density
    }

    /// Its first melt, above which some of it is liquid, decomposed or sublimed.
    #[must_use]
    pub const fn transition(&self) -> Transition {
        self.transition
    }

    /// The family of its mechanical rows in R10's material table; a liquid's is its frozen form's.
    #[must_use]
    pub const fn mechanics(&self) -> MechanicsFamily {
        self.mechanics
    }

    /// Where its values come from: each source's `covers` names `grain density`, `transition` or
    /// `mechanics`.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }
}

impl Substance {
    /// Its condensed columns, for a row that can be a solid or a liquid and whose figures were
    /// read; `None` where they are not modelled.
    #[must_use]
    pub const fn condensed(&self) -> Option<&CondensedColumns> {
        self.condensed.as_ref()
    }

    /// Its grain density as `phase` at `temperature`, without pore space.
    ///
    /// A [`GrainDensity::Fixed`] row gives its phase's one value at any temperature, and `None`
    /// for a phase that was not read (a rock's melt is P14.T49.h's). A
    /// [`GrainDensity::OfDiagram`] row gives its diagram's: the solid's density from 0 K to the
    /// triple temperature, `None` above it and in a solid phase whose density was not read, and
    /// the saturated liquid's from the triple to the critical temperature, `None` outside them.
    /// `None` too for a row with no condensed columns.
    ///
    /// # Panics
    ///
    /// Never: an [`OfDiagram`](GrainDensity::OfDiagram) row has its phase diagram, which the
    /// registry's tests hold for every row.
    ///
    /// # Examples
    ///
    /// Carbon dioxide's frost is denser on a cold Mars's winter pole than near its triple point,
    /// while a basalt keeps its room-temperature grain density:
    ///
    /// ```
    /// use hyperion_sim::substance::{CondensedPhase, by_key};
    /// use hyperion_sim::units::Kelvin;
    ///
    /// # fn main() {
    /// #     assert!(example().is_some(), "every value the example reads is there");
    /// # }
    /// # fn example() -> Option<()> {
    /// let co2 = by_key("CO2")?.substance();
    /// let polar = co2.grain_density(CondensedPhase::Solid, Kelvin::new(148.0))?;
    /// let warm = co2.grain_density(CondensedPhase::Solid, Kelvin::new(190.0))?;
    /// assert!(polar > warm);
    /// let basalt = by_key("basalt")?.substance();
    /// let rock = basalt.grain_density(CondensedPhase::Solid, Kelvin::new(150.0))?;
    /// assert!((rock.value() - 3_010.0).abs() < 1e-9);
    /// // A rock's melt is not one of the registry's columns.
    /// assert_eq!(basalt.grain_density(CondensedPhase::Liquid, Kelvin::new(1_400.0)), None);
    /// # Some(())
    /// # }
    /// ```
    #[must_use]
    pub fn grain_density(
        &self,
        phase: CondensedPhase,
        temperature: Kelvin,
    ) -> Option<KilogramsPerCubicMetre> {
        match self.condensed.as_ref()?.grain_density {
            GrainDensity::Fixed { solid, liquid } => match phase {
                CondensedPhase::Solid => solid,
                CondensedPhase::Liquid => liquid,
            }
            .map(|given| given.density),
            GrainDensity::OfDiagram => {
                let diagram = self.diagram.as_ref().expect(
                    "an OfDiagram row has a phase diagram, as \
                     substance_condensed_columns_cover_the_condensed_rows holds for every row",
                );
                match phase {
                    CondensedPhase::Solid => diagram.solid_density(temperature),
                    CondensedPhase::Liquid => diagram.liquid_density(temperature),
                }
            }
        }
    }
}
