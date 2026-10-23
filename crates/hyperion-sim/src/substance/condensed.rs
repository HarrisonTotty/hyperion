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
//!
//! **The densities P14.T49.n read** are held until the 23 → 24 bump (`READ_DENSITIES_VERSION`):
//! a solid phase whose density it read ([`SolidPhase::density`](super::SolidPhase::density)) and
//! a row's liquid by temperature ([`GrainDensity::MeasuredLiquid`], which
//! [`CondensedColumns::grain_density`] gives as 23's [`GrainDensity::Fixed`] of the solid alone)
//! are not modelled below it, so that every reader gives 23's answer to the bit.

use hyperion_surface::field::MechanicsFamily;

use super::diagram::measured_at;
use super::{Source, Substance};
use crate::units::{Kelvin, KilogramsPerCubicMetre};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which the registry serves the densities P14.T49.n read: 24, the
/// 23 → 24 batch's bump, P14.T54.b (decision-p14-t54a-water-consistency §2 and §6;
/// decision-bump-plan-after-22, "Version-24 lanes after 23", §6's holds).
///
/// Below it [`ReadDensitiesRules::in_force`] is [`ReadDensitiesRules::Held`]: a solid phase whose
/// density P14.T49.n read has none ([`SolidPhase::density`](super::SolidPhase::density)), and a
/// [`GrainDensity::MeasuredLiquid`] row is 23's [`GrainDensity::Fixed`] of its solid alone
/// ([`CondensedColumns::grain_density`]), so that every reader, the registry's golden and the
/// client's fixture give 23's answers bit for bit. P14.T54.b deletes this hold,
/// [`ReadDensitiesRules`] with its `Held` arm, every phase's `held` flag and the tests' switch, and
/// its tripwires (`the_read_densities_are_held_only_until_the_24_bump` and
/// `a_panic_inside_the_read_densities_switch_sets_the_rules_back`).
pub(crate) const READ_DENSITIES_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether the registry serves the densities P14.T49.n read (the hold,
/// [`READ_DENSITIES_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ReadDensitiesRules {
    /// Versions 23's and before: those densities are not modelled, as 23 has none.
    Held,
    /// The 23 → 24 batch's: every density the registry's rows read.
    Read,
}

impl ReadDensitiesRules {
    /// The rules this generator version gives: the densities from [`READ_DENSITIES_VERSION`],
    /// held below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_read_densities`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = READ_DENSITIES_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= READ_DENSITIES_VERSION {
            Self::Read
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The read densities' rules a test sets on its thread, `None` for the version's.
    static READ_DENSITIES_RULES: core::cell::Cell<Option<ReadDensitiesRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the read densities' rules `rules` on this thread, for the tests to reach
/// 24's densities ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. A record derives lazily, so a test reads it inside the switch. P14.T54.b deletes it
/// with the hold.
#[cfg(test)]
pub(crate) fn with_read_densities<R>(rules: ReadDensitiesRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<ReadDensitiesRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            READ_DENSITIES_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(READ_DENSITIES_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

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
    /// The solid's one value, as [`Fixed`](Self::Fixed)'s, and the liquid's by temperature
    /// (P14.T49.n): a melt or a molecular liquid whose density falls as it warms, read where a
    /// cloud deck or a lava lies hundreds of kelvin above its melting point.
    ///
    /// The liquid's nodes rise in temperature, at one atmosphere unless its source states another
    /// pressure; its density is linear between them and held beyond the first and the last, the
    /// arithmetic of [`SolidDensity::Measured`](super::SolidDensity::Measured), to the bit. Two
    /// nodes are a source's linear law at the ends of its range. Each node is a measured law's or
    /// a melt model's, as the row's `liquid grain density` source states. Held until generator
    /// version 24 (`READ_DENSITIES_VERSION`), below which [`CondensedColumns::grain_density`]
    /// gives the solid alone as [`Fixed`](Self::Fixed).
    MeasuredLiquid {
        /// The solid's.
        solid: DensityAt,
        /// The liquid's (temperature, density) nodes, rising in temperature.
        liquid: &'static [(Kelvin, KilogramsPerCubicMetre)],
    },
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
    ///
    /// Below `READ_DENSITIES_VERSION` a [`GrainDensity::MeasuredLiquid`] row is 23's
    /// [`GrainDensity::Fixed`] of its solid alone, its liquid not modelled.
    #[must_use]
    pub fn grain_density(&self) -> GrainDensity {
        let given = self.grain_density;
        if let GrainDensity::MeasuredLiquid { solid, .. } = given
            && ReadDensitiesRules::in_force() == ReadDensitiesRules::Held
        {
            return GrainDensity::Fixed {
                solid: Some(solid),
                liquid: None,
            };
        }
        given
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

    /// Where its values come from: each source's `covers` names `grain density`, `liquid grain
    /// density` (a [`GrainDensity::MeasuredLiquid`] row's liquid), `transition` or `mechanics`.
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
    /// A [`GrainDensity::MeasuredLiquid`] row gives its solid's one value at any temperature and
    /// its liquid's nodes' law, held beyond them (held until `READ_DENSITIES_VERSION`, below
    /// which its liquid is `None`). `None` too for a row with no condensed columns.
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
        match self.condensed.as_ref()?.grain_density() {
            GrainDensity::Fixed { solid, liquid } => match phase {
                CondensedPhase::Solid => solid,
                CondensedPhase::Liquid => liquid,
            }
            .map(|given| given.density),
            GrainDensity::MeasuredLiquid { solid, liquid } => Some(match phase {
                CondensedPhase::Solid => solid.density,
                CondensedPhase::Liquid => measured_at(liquid, temperature),
            }),
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::substance::{PhaseDiagram, by_key, condensables, ids};

    /// A temperature range of a row's solid with no density: its key and from where, inclusive,
    /// to where, exclusive, kelvin, `f64::INFINITY` for through the triple point.
    type Gap = (&'static str, f64, f64);

    /// The solids the registry gives no density at 24: β-CO (61.585 K to the triple point) and
    /// hydrogen sulphide's phase II (103.52–126.22 K), whose densities were not found in a
    /// readable source (P14.T49.n's open gaps). Each leaves the list as its density is read.
    const GAPS_AT_24: [Gap; 2] = [("CO", 61.585, f64::INFINITY), ("H2S", 103.52, 126.22)];

    /// Those 23 has, decision-p14-t54a-grain-density's F3 list: 24's and solid argon's.
    const GAPS_AT_23: [Gap; 5] = [
        ("Ar", 0.0, f64::INFINITY),
        ("O2", 23.14, 43.78),
        ("CO", 61.585, f64::INFINITY),
        ("H2S", 0.0, f64::INFINITY),
        ("C2H6", 0.0, f64::INFINITY),
    ];

    /// The rows with a solid phase whose density P14.T49.n read, held until 24.
    const READ_SOLIDS: [&str; 4] = ["Ar", "O2", "H2S", "C2H6"];

    /// The rows whose liquid by temperature P14.T49.n read, held until 24.
    const READ_LIQUIDS: [&str; 6] = ["Fe", "MgSiO3", "Mg2SiO4", "Al2O3", "CaTiO3", "S8"];

    /// Whether `gaps` hold `key`'s solid at `t` kelvin.
    fn in_gap(gaps: &[Gap], key: &str, t: f64) -> bool {
        gaps.iter()
            .any(|&(row, from, to)| row == key && from <= t && t < to)
    }

    /// The keys `gaps` name.
    fn keys(gaps: &[Gap]) -> BTreeSet<String> {
        gaps.iter().map(|&(key, ..)| key.to_owned()).collect()
    }

    /// The temperatures, kelvin, at which a condensable's solid is read: 4,096 steps from 0 K to
    /// its triple temperature, each phase's start and 10⁻⁹ K either side of it, and the triple
    /// temperature itself, those inside [0, `T_t`].
    fn solid_samples(diagram: &PhaseDiagram) -> Vec<f64> {
        let t_t = diagram.triple_temperature().value();
        let mut samples: Vec<f64> = (0..=4_096_u32)
            .map(|j| t_t * f64::from(j) / 4_096.0)
            .collect();
        for phase in diagram.solid_phases() {
            let start = phase.start().value();
            samples.extend([start - 1e-9, start, start + 1e-9]);
        }
        samples.push(t_t);
        samples.retain(|t| (0.0..=t_t).contains(t));
        samples
    }

    /// `row`'s grain density as `phase` at `t` kelvin under `rules`, kg m⁻³.
    fn density(
        row: &Substance,
        phase: CondensedPhase,
        t: f64,
        rules: ReadDensitiesRules,
    ) -> Option<f64> {
        with_read_densities(rules, || {
            row.grain_density(phase, Kelvin::new(t))
                .map(KilogramsPerCubicMetre::value)
        })
    }

    /// Every condensable has a solid grain density over its whole solid range under 24's rules,
    /// finite and positive, but for the named gaps that remain ([`GAPS_AT_24`]): swept from 0 K
    /// to its triple temperature, at each phase's start and 10⁻⁹ K either side of it, and at the
    /// triple point (decision-p14-t51a-part2 §3.4's registry test). Under the hold it has none
    /// exactly where 23 has none ([`GAPS_AT_23`]), and everywhere else 24's density to the bit.
    #[test]
    fn every_condensable_has_a_solid_density_over_its_solid_range() {
        let (mut missing_at_23, mut missing_at_24) = (BTreeSet::new(), BTreeSet::new());
        let mut count = 0;
        for id in condensables() {
            count += 1;
            let row = id.substance();
            let key = row.key();
            let key = key.as_str();
            let diagram = row.phase_diagram().expect("a condensable has a diagram");
            for t in solid_samples(diagram) {
                let held = density(row, CondensedPhase::Solid, t, ReadDensitiesRules::Held);
                let read = density(row, CondensedPhase::Solid, t, ReadDensitiesRules::Read);
                assert_eq!(
                    held.is_none(),
                    in_gap(&GAPS_AT_23, key, t),
                    "{key} at {t} K, held"
                );
                assert_eq!(
                    read.is_none(),
                    in_gap(&GAPS_AT_24, key, t),
                    "{key} at {t} K, read"
                );
                if held.is_none() {
                    missing_at_23.insert(key.to_owned());
                }
                match (held, read) {
                    (Some(held), Some(read)) => assert_same_bits(held, read),
                    (None, Some(read)) => {
                        assert!(read.is_finite() && read > 0.0, "{key} at {t} K: {read}");
                    }
                    (_, None) => {
                        missing_at_24.insert(key.to_owned());
                    }
                }
            }
        }
        assert_eq!(count, 14, "the condensables");
        assert_eq!(missing_at_23, keys(&GAPS_AT_23), "23's gaps are F3's");
        assert_eq!(
            missing_at_24,
            keys(&GAPS_AT_24),
            "24's gaps are the named ones"
        );
    }

    /// The densities P14.T49.n read are their sources' (the rows' `ρ_s` and `liquid grain
    /// density` sources) under 24's rules: solid argon's Eq. (27) of Maltby, Hammer and
    /// Wilhelmsen 2024 within 0.013%, held at 1,770.74 kg m⁻³ below 5 K; liquid iron's and
    /// alumina's linear laws within 10⁻⁹; the melt models' M ÷ V within 1.2 × 10⁻⁴ at the cloud
    /// decks of `condense.golden`; and liquid sulphur within 3.5 kg m⁻³ of Kellas 1918's means.
    #[test]
    fn the_read_densities_follow_their_sources() {
        let read = |key: &str, phase, t| {
            let row = by_key(key).unwrap().substance();
            density(row, phase, t, ReadDensitiesRules::Read).unwrap()
        };
        let (solid, liquid) = (CondensedPhase::Solid, CondensedPhase::Liquid);
        let relative = |ours: f64, theirs: f64| (ours / theirs - 1.0).abs();
        let argon = |t: f64| {
            let x = t / 34.70;
            let p = 3.160e-4 + 0.038_21 * x - 0.015_40 * x * x + 0.003_152 * x * x * x;
            39.948e3 / (22.56 * (1.0 + p / (crate::math::exp(1.0 / x) - 1.0)))
        };
        assert_same_bits(read("Ar", solid, 0.0), 1_770.74);
        assert_same_bits(read("Ar", solid, 83.8058), 1_621.60);
        for t in [5.0, 12.5, 23.3, 40.0, 52.5, 61.0, 77.7, 83.8] {
            let ours = read("Ar", solid, t);
            assert!(relative(ours, argon(t)) < 1.3e-4, "Ar at {t} K: {ours}");
        }
        let iron = |t: f64| 7_034.96 - 0.926 * (t - 1_811.0);
        let alumina = |t: f64| 2_900.0 - 0.09 * (t - 2_327.0);
        for t in [1_811.15, 1_905.0, 2_400.0, 2_500.0] {
            assert!(
                relative(read("Fe", liquid, t), iron(t)) < 1e-9,
                "Fe at {t} K"
            );
        }
        assert_same_bits(read("Fe", liquid, 3_000.0), read("Fe", liquid, 2_500.0));
        for t in [2_327.0, 2_603.7, 3_240.0] {
            assert!(
                relative(read("Al2O3", liquid, t), alumina(t)) < 1e-9,
                "Al2O3 at {t} K"
            );
        }
        // Lange 1997's set for the magnesium silicates, MELTS's for perovskite, at the decks.
        for (key, molar_mass, volume, slope, reference, deck) in [
            ("MgSiO3", 100.387, 38.88, 0.003_27, 1_773.0, 2_028.9),
            ("Mg2SiO4", 140.691, 50.90, 0.006_54, 1_773.0, 2_276.9),
            ("CaTiO3", 135.942, 39.73, 0.010_166, 1_673.0, 2_514.7),
        ] {
            let model = molar_mass * 1e3 / (volume + slope * (deck - reference));
            let ours = read(key, liquid, deck);
            assert!(
                relative(ours, model) < 1.2e-4,
                "{key} at {deck} K: {ours} against {model}"
            );
        }
        // Kellas 1918's means, ICT vol. III p. 21, from the melt to 445 °C.
        for (celsius, kellas) in [
            (122.2, 1_802.75),
            (145.5, 1_782.60),
            (158.5, 1_772.40),
            (171.3, 1_770.45),
            (210.0, 1_750.75),
            (278.5, 1_709.50),
            (357.0, 1_659.10),
            (445.0, 1_606.66),
        ] {
            let ours = read("S8", liquid, celsius + 273.15);
            assert!((ours - kellas).abs() < 3.5, "S8 at {celsius} °C: {ours}");
        }
        // Each row's solid is its one fixed value, as at 23.
        for key in READ_LIQUIDS {
            let row = by_key(key).unwrap().substance();
            let held = density(row, solid, 1_000.0, ReadDensitiesRules::Held);
            assert_eq!(held, Some(read(key, solid, 1_000.0)), "{key}");
        }
    }

    /// The hold keeps 23's answers: under it every accessor gives none exactly where P14.T49.n's
    /// densities sit (a held solid phase, a liquid by temperature) and, everywhere else, 24's to
    /// the bit; a held phase has a density, and the rows that hold one are the named ones.
    #[test]
    fn the_hold_gives_none_exactly_where_the_read_densities_sit() {
        let mut held_solids = BTreeSet::new();
        let mut held_liquids = BTreeSet::new();
        for id in ids() {
            let row = id.substance();
            let key = row.key();
            let key = key.as_str();
            let Some(condensed) = row.condensed() else {
                continue;
            };
            let given = |rules| with_read_densities(rules, || condensed.grain_density());
            let (held_given, read_given) = (
                given(ReadDensitiesRules::Held),
                given(ReadDensitiesRules::Read),
            );
            let measured_liquid = match read_given {
                GrainDensity::MeasuredLiquid { solid, .. } => {
                    held_liquids.insert(key.to_owned());
                    let fixed = GrainDensity::Fixed {
                        solid: Some(solid),
                        liquid: None,
                    };
                    assert_eq!(held_given, fixed, "{key}: 23's solid alone");
                    true
                }
                GrainDensity::Fixed { .. } | GrainDensity::OfDiagram => {
                    assert_eq!(held_given, read_given, "{key}");
                    false
                }
            };
            let diagram = row.phase_diagram();
            let mut samples: Vec<f64> = (0..=3_000_u32).map(f64::from).collect();
            if let Some(d) = diagram {
                for phase in d.solid_phases() {
                    let read = with_read_densities(ReadDensitiesRules::Read, || phase.density());
                    let held = with_read_densities(ReadDensitiesRules::Held, || phase.density());
                    assert!(!phase.held || phase.density.is_some(), "{key}: held, read");
                    if phase.held {
                        held_solids.insert(key.to_owned());
                        assert_eq!(held, None, "{key} {}", phase.name());
                    } else {
                        assert_eq!(held, read, "{key} {}", phase.name());
                    }
                    let start = phase.start().value();
                    samples.extend([start - 1e-9, start, start + 1e-9]);
                }
                samples.extend([
                    d.triple_temperature().value(),
                    d.critical_temperature().value(),
                ]);
            }
            if let GrainDensity::MeasuredLiquid { liquid, .. } = read_given {
                for (at, _) in liquid {
                    let at = at.value();
                    samples.extend([at - 1e-9, at, at + 1e-9]);
                }
            }
            for t in samples {
                for phase in [CondensedPhase::Solid, CondensedPhase::Liquid] {
                    let at = |rules| {
                        with_read_densities(rules, || {
                            row.grain_density(phase, Kelvin::new(t))
                                .map(KilogramsPerCubicMetre::value)
                        })
                    };
                    let (held, read) = (at(ReadDensitiesRules::Held), at(ReadDensitiesRules::Read));
                    let new_here = match phase {
                        CondensedPhase::Solid => diagram
                            .and_then(|d| d.solid_phase_at(Kelvin::new(t)))
                            .is_some_and(|solid| solid.held),
                        CondensedPhase::Liquid => measured_liquid,
                    };
                    if new_here {
                        assert_eq!(held, None, "{key} {phase:?} at {t} K is held");
                        let read = read.unwrap_or_else(|| panic!("{key} {phase:?} at {t} K"));
                        assert!(read.is_finite() && read > 0.0, "{key} {phase:?} at {t} K");
                    } else {
                        match (held, read) {
                            (Some(held), Some(read)) => assert_same_bits(held, read),
                            (None, None) => {}
                            (held, read) => panic!("{key} {phase:?} at {t} K: {held:?}, {read:?}"),
                        }
                    }
                }
            }
        }
        let named = |keys: &[&str]| -> BTreeSet<String> {
            keys.iter().map(|&key| key.to_owned()).collect()
        };
        assert_eq!(held_solids, named(&READ_SOLIDS));
        assert_eq!(held_liquids, named(&READ_LIQUIDS));
    }

    /// The densities P14.T49.n read are held until P14.T54.b's bump: below it the registry gives
    /// 23's answers, and the tests' switch reaches 24's ahead of it.
    #[test]
    fn the_read_densities_are_held_only_until_the_24_bump() {
        assert!(
            GENERATOR_VERSION < READ_DENSITIES_VERSION,
            "the 23 → 24 bump (P14.T54.b) serves the densities P14.T49.n read: solid argon's \
             (fcc), β-O2's, H2S's phases III and I, C2H6's phases III and I, and the liquids by \
             temperature of Fe, MgSiO3, Mg2SiO4, Al2O3, CaTiO3 and S8. Delete \
             `READ_DENSITIES_VERSION`, `ReadDensitiesRules` with its `Held` arm, \
             `with_read_densities` and its thread's rules (and the switch in every test that sets \
             it), `SolidPhase`'s `held` flag on every row, the `Held` branches of \
             `SolidPhase::density` and `CondensedColumns::grain_density`, this test and \
             `a_panic_inside_the_read_densities_switch_sets_the_rules_back`. Re-bless \
             tests/golden/substance/registry.golden (`[diagram]`'s and `[condensed]`'s solid \
             densities of Ar, O2, H2S and C2H6 where they sample a read phase, and the six rows' \
             `[condensed]` lines, a fixed solid and a liquid's nodes), \
             packages/protocol/fixtures/substances.json (the solid pieces of Ar, O2's β, H2S's \
             III and I and C2H6's III and I, the six liquids' pieces, and \
             `FIXTURE_DESCRIPTION`'s rule for a liquid by temperature) and \
             tests/golden/planetary/condense.golden (Uranus's H2S at 145.57 K, the Fe, 600 K \
             sub-Neptune and hot Jupiter decks). Then settle what reads 23's gaps: \
             substance_golden.rs's `check_named_shapes` (O2 three contiguous pieces, H2S two \
             about phase II's gap, C2H6 two, Ar one); the client's `NO_GRAIN_DENSITY` \
             (parity.test.ts: drop solid Ar, solid H2S and liquid S8) and grainDensity.test.ts \
             (Ar at 60 K and O2 at 30 K have a density; CO at 65 K still none); \
             `diagram_solid_densities_follow_their_measurements` (its not-read list), \
             `substance_aerosol_refuses_what_it_cannot_serve`, \
             `every_condensable_has_a_solid_density_over_its_solid_range` and \
             `the_hold_gives_none_exactly_where_the_read_densities_sit` (their held halves); and \
             the record tests, each keeping its 24 case and losing its 23 one: \
             `a_solid_whose_density_was_not_read_has_a_stand_in_cap_and_no_layer` with \
             `under_24_s_densities_an_argon_ice_has_its_solid_s_cap_and_layer` (partition), \
             `every_generated_solid_body_reads_into_inputs_the_builder_takes` with \
             `under_24_s_densities_the_census_refuses_no_ice_for_its_density`, and \
             `a_surface_the_registry_or_the_record_does_not_model_answers_not_modelled` (its \
             argon arm) with `under_24_s_densities_the_argon_ice_body_reads` (surface/inputs.rs), \
             `the_radiative_model_reads_its_record_s_inertia_on_every_body` with \
             `under_24_s_densities_the_radiative_model_reads_its_record_s_inertia_on_every_body` \
             (airless.rs), and `the_golden_systems_unlit_records_read_their_night_floor`, \
             `the_sample_s_unlit_records_read_their_night_floor` and `unlit_moves` with \
             `under_24_s_densities_the_golden_systems_unlit_records_all_read`, \
             `under_24_s_densities_the_sample_s_unlit_records_all_read` and \
             `the_sample_s_argon_ices_under_24_s_rules` (system/tests.rs)"
        );
        assert_eq!(ReadDensitiesRules::in_force(), ReadDensitiesRules::Held);
        assert_eq!(
            with_read_densities(ReadDensitiesRules::Read, ReadDensitiesRules::in_force),
            ReadDensitiesRules::Read
        );
        assert_eq!(ReadDensitiesRules::in_force(), ReadDensitiesRules::Held);
    }

    /// A panic inside the tests' switch sets the rules back. Skipped, not failed, where panics
    /// abort (wasm32-wasip1), since `catch_unwind` would abort the whole test binary there; the
    /// other CI architectures run it.
    #[test]
    #[cfg(panic = "unwind")]
    fn a_panic_inside_the_read_densities_switch_sets_the_rules_back() {
        let failed = std::panic::catch_unwind(|| {
            with_read_densities(ReadDensitiesRules::Read, || panic!("a test fails inside"));
        });
        assert!(failed.is_err());
        assert_eq!(
            ReadDensitiesRules::in_force(),
            ReadDensitiesRules::Held,
            "a panic inside the switch sets the rules back"
        );
    }
}
