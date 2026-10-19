//! The solutions' tests (P14.T49.h's step 3): Titan's seas' composition, density and scattering
//! against their rulings and the lane's research, decision-p14-t49b-condensed §2.5's continuation
//! of a component below its triple point, the brines, ammonia–water and the aqueous acid against
//! their sources and the lane's research, and decision-p14-t49h-step3-composition's contract for a
//! given composition (its §5).

use hyperion_surface::substance_key::SubstanceKeyForm;
use hyperion_testkit::float::assert_same_bits;

use crate::tables::liquid_optics as bins;

use super::super::super::rows::liquids::LIQUIDS;
use super::super::super::{CondensedPhase, GrainDensity, TransitionKind, by_key, ids};
use super::super::{
    BuildLiquidCompositionError, Composition, LiquidConstitution, liquid_properties,
};
use super::*;

/// The row `key`, which a test knows to be a row.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap()
}

/// Titan's seas' row.
fn titan() -> SubstanceId {
    row("ch4_c2h6_n2")
}

/// A record at `t` K at `composition`, which a test knows to resolve.
fn record(id: SubstanceId, t: f64, composition: Composition<'_>) -> LiquidProperties {
    liquid_properties(id, Kelvin::new(t), composition).unwrap()
}

/// Titan's seas' reference state, its condensed columns' stated liquid: 518.9 kg m⁻³ at 93.65 K.
fn titan_reference() -> (Kelvin, KilogramsPerCubicMetre) {
    let GrainDensity::Fixed {
        liquid: Some(stated),
        ..
    } = titan().substance().condensed().unwrap().grain_density()
    else {
        panic!("Titan's seas state their liquid's density");
    };
    (stated.at(), stated.density())
}

/// `a` and `b` within `relative` of each other, labelled `what`.
fn assert_close(a: f64, b: f64, relative: f64, what: &str) {
    assert!(
        (a / b - 1.0).abs() <= relative,
        "{what}: {a} against {b} ({:e})",
        a / b - 1.0
    );
}

/// Every member of `a` and `b` the same, bit for bit.
fn assert_same_record(a: &LiquidProperties, b: &LiquidProperties) {
    assert_same_bits(a.temperature.value(), b.temperature.value());
    assert_same_bits(a.density_kg_m3, b.density_kg_m3);
    assert_eq!(a.basis, b.basis);
    for (x, y) in a.refractive_index.iter().zip(&b.refractive_index) {
        assert_same_bits(*x, *y);
    }
    for (p, q) in [
        (a.absorption_per_m, b.absorption_per_m),
        (a.scattering_per_m, b.scattering_per_m),
    ] {
        assert_eq!(p.is_some(), q.is_some());
        for (x, y) in p.iter().flatten().zip(q.iter().flatten()) {
            assert_same_bits(*x, *y);
        }
    }
    for (p, q) in [
        (a.surface_tension_n_per_m, b.surface_tension_n_per_m),
        (a.viscosity_pa_s, b.viscosity_pa_s),
    ] {
        assert_eq!(p.is_some(), q.is_some());
        if let (Some(x), Some(y)) = (p, q) {
            assert_same_bits(x, y);
        }
    }
}

/// Titan's reference mole fractions are Mastrogiuseppe et al. 2016's volume fractions (§V.B:
/// 71% CH4, 12% C2H6 and 17% N2) converted at the reference state's 93.65 K with the rows' own
/// saturated-liquid densities and molar masses, xᵢ ∝ φᵢ ρᵢ ÷ Mᵢ: the arithmetic's own values, to
/// round-off, which print as decision-p14-t49b-condensed §1.2's 0.739146, 0.164352 and 0.096503 to
/// six decimals and as decision-p14-t49h-step3-composition §5's 0.7391, 0.1644 and 0.0965 to four.
#[test]
fn titan_s_mole_fractions_are_ligeia_s_volume_fractions_converted() {
    let (t, _) = titan_reference();
    assert_same_bits(t.value(), 93.65);
    let volume = [("CH4", 0.71), ("N2", 0.17), ("C2H6", 0.12)];
    let moles = volume.map(|(key, phi)| {
        let s = row(key).substance();
        let rho = s.grain_density(CondensedPhase::Liquid, t).unwrap().value();
        phi * rho / s.molar_mass_g_per_mol().unwrap()
    });
    let total = moles[0] + moles[1] + moles[2];
    let reference = titan()
        .substance()
        .liquid()
        .unwrap()
        .reference_composition()
        .unwrap();
    let printed = [
        (0.739_146, 0.7391),
        (0.164_352, 0.1644),
        (0.096_503, 0.0965),
    ];
    assert_eq!(reference.mole_fractions().len(), 3);
    for (((&(id, held), (key, _)), n), (six, four)) in reference
        .mole_fractions()
        .iter()
        .zip(&volume)
        .zip(&moles)
        .zip(printed)
    {
        assert_eq!(id, row(key));
        let x = n / total;
        assert!(
            (held - x).abs() < 1e-15,
            "{key}: held {held:?}, computed {x:?}"
        );
        assert!(
            ((held * 1e6).round() / 1e6 - six).abs() < 1e-12,
            "{key}: {held}"
        );
        assert!(
            ((held * 1e4).round() / 1e4 - four).abs() < 1e-12,
            "{key}: {held}"
        );
    }
}

/// Titan's seas at their reference state agree with the condensed columns' 518.9 kg m⁻³ within
/// 0.05, its rounding (decision-p14-t49b-condensed §1.4, read as "agrees within" by
/// decision-p14-t49h-step3-composition §4; no anchor), and that is Σ φᵢ ρᵢ at Ligeia's volume
/// fractions: ideal volume additivity.
#[test]
fn titan_s_density_at_its_reference_state_is_518_9() {
    let (t, stated) = titan_reference();
    let sea = record(titan(), t.value(), Composition::Reference);
    assert!(
        (sea.density_kg_m3 - 518.9).abs() < 0.05,
        "{}",
        sea.density_kg_m3
    );
    assert!((sea.density_kg_m3 - stated.value()).abs() < 0.05);
    let rho = |key: &str| {
        row(key)
            .substance()
            .grain_density(CondensedPhase::Liquid, t)
            .unwrap()
            .value()
    };
    let volume_additive = 0.71 * rho("CH4") + 0.17 * rho("N2") + 0.12 * rho("C2H6");
    assert!(
        (sea.density_kg_m3 - volume_additive).abs() < 1e-9,
        "{} against Σ φρ = {volume_additive}",
        sea.density_kg_m3
    );
    assert_eq!(sea.basis, LiquidBasis::Derived);
}

/// decision-p14-t49b-condensed §2.5's rule for a component below its triple point is continuous
/// there with the row's own record, member by member, for every pure liquid with a phase diagram:
/// the density's line, the index carried to it, the surface tension's form and the continued
/// viscosity within round-off of the slope a microkelvin below; `κ_T` held at the triple point's
/// value, to the bit, a microkelvin and 5 K below; the absorption the file's, held. Every member
/// below is `Derived`, and a row with no continued viscosity has none below.
#[test]
fn the_continuation_below_the_triple_point_is_continuous() {
    let mut continued = 0;
    for (key, columns) in &LIQUIDS {
        if !matches!(columns.constitution, LiquidConstitution::Pure { .. }) {
            continue;
        }
        let id = row(key.as_str());
        let Some(diagram) = id.substance().phase_diagram() else {
            continue;
        };
        let triple = diagram.triple_temperature();
        let at = component_state(id, triple).unwrap();
        let own = liquid_properties(id, triple, Composition::Reference).unwrap();
        assert_same_bits(at.density.value(), own.density_kg_m3);
        let h = 1e-6;
        let below = component_state(id, Kelvin::new(triple.value() - h)).unwrap();
        assert!(below.basis >= LiquidBasis::Derived, "{key}");
        assert_close(
            below.density.value(),
            at.density.value(),
            1e-7,
            key.as_str(),
        );
        for (n, m) in below.refractive_index.iter().zip(&at.refractive_index) {
            assert_close(*n, *m, 1e-8, key.as_str());
        }
        assert_eq!(below.absorption_per_m, at.absorption_per_m, "{key}");
        let kappa = at.fluctuations.unwrap().compressibility_per_pa;
        assert_same_bits(below.fluctuations.unwrap().compressibility_per_pa, kappa);
        let colder = component_state(id, Kelvin::new(triple.value() - 5.0)).unwrap();
        assert_same_bits(colder.fluctuations.unwrap().compressibility_per_pa, kappa);
        assert_close(
            below.surface_tension_n_per_m.unwrap(),
            at.surface_tension_n_per_m.unwrap(),
            1e-7,
            key.as_str(),
        );
        match (columns.supercooled_viscosity, below.viscosity_pa_s) {
            (Some(_), Some(mu)) => {
                assert_close(mu, at.viscosity_pa_s.unwrap(), 1e-6, key.as_str());
                assert!(mu > at.viscosity_pa_s.unwrap(), "{key}");
                continued += 1;
            }
            (None, None) => {}
            (form, mu) => panic!("{key}: a continued form {form:?} gives {mu:?}"),
        }
    }
    assert_eq!(continued, 2, "methane's and ethane's viscosities continue");
}

/// The pure limits (decision-p14-t49h-step3-composition §5, item 4): Titan's seas given one of
/// their components alone are that component's own record within 10⁻¹² relative, at 95 and 110 K
/// inside both ranges, in every member both give: density, index, absorption where the component
/// has one, scattering (the mixture's, whose concentration term vanishes, reducing to
/// [`pure_liquid_scattering_per_m`](super::super::pure_liquid_scattering_per_m)'s), surface
/// tension and viscosity; and it is `Derived`.
#[test]
fn a_solution_of_one_component_is_that_component_s_record() {
    for key in ["CH4", "N2", "C2H6"] {
        let component = row(key);
        let fractions = [(component, 1.0)];
        let alone = Composition::Given(LiquidComposition::new(&fractions).unwrap());
        for t in [95.0, 110.0] {
            let sea = record(titan(), t, alone);
            let own = record(component, t, Composition::Reference);
            let what = format!("{key} at {t} K");
            assert_close(sea.density_kg_m3, own.density_kg_m3, 1e-12, &what);
            for (n, m) in sea.refractive_index.iter().zip(&own.refractive_index) {
                assert_close(*n, *m, 1e-12, &what);
            }
            assert_eq!(
                sea.absorption_per_m.is_some(),
                own.absorption_per_m.is_some()
            );
            for (a, b) in sea
                .absorption_per_m
                .iter()
                .flatten()
                .zip(own.absorption_per_m.iter().flatten())
            {
                assert_close(*a, *b, 1e-12, &what);
            }
            assert_close(
                sea.surface_tension_n_per_m.unwrap(),
                own.surface_tension_n_per_m.unwrap(),
                1e-12,
                &what,
            );
            assert_close(
                sea.viscosity_pa_s.unwrap(),
                own.viscosity_pa_s.unwrap(),
                1e-12,
                &what,
            );
            for (a, b) in sea
                .scattering_per_m
                .unwrap()
                .iter()
                .zip(&own.scattering_per_m.unwrap())
            {
                assert_close(*a, *b, 1e-12, &what);
            }
            assert_eq!(sea.basis, LiquidBasis::Derived, "{what}");
        }
    }
}

/// `LiquidComposition::new` refuses a component that is not a neutral formula-key species, before
/// any other rule it breaks (decision-p14-t49h-step3-composition §2.1): a solution, a lithology, an
/// ion and the electron.
#[test]
fn a_composition_s_components_are_neutral_species() {
    for key in ["brine_nacl", "basalt", "H-", "e-", "ch4_c2h6_n2"] {
        let id = row(key);
        assert_eq!(
            LiquidComposition::new(&[(id, 1.0)]),
            Err(BuildLiquidCompositionError::NotASpecies { component: id }),
            "{key}"
        );
        assert_eq!(
            LiquidComposition::new(&[(id, 0.5), (id, 0.5)]),
            Err(BuildLiquidCompositionError::NotASpecies { component: id }),
            "{key}"
        );
    }
    let water = row("H2O");
    let fractions = [(water, 0.5), (row("NaCl"), 0.5)];
    assert!(LiquidComposition::new(&fractions).is_ok());
}

/// Every neutral formula-key row has a molar mass, so the resolver never meets a component
/// without one (decision-p14-t49h-step3-composition §2.1).
#[test]
fn every_neutral_formula_row_has_a_molar_mass() {
    let mut species = 0;
    for id in ids() {
        let row = id.substance();
        if row.key().form() == SubstanceKeyForm::Formula && row.charge() == 0 {
            assert!(row.molar_mass_g_per_mol().is_some(), "{}", row.key());
            species += 1;
        }
    }
    assert!(species > 50, "{species}");
}

/// A given component the row's model does not carry is refused, naming the first in the given
/// order: water or ammonia given to Titan's seas, and sodium chloride given to water.
#[test]
fn a_component_the_model_does_not_carry_is_refused() {
    let at = Kelvin::new(93.65);
    let refused = |id: SubstanceId, fractions: &[(SubstanceId, f64)], stranger: &str| {
        let given = Composition::Given(LiquidComposition::new(fractions).unwrap());
        assert_eq!(
            liquid_properties(id, at, given),
            Err(ResolveLiquidError::ComponentNotModelled {
                key: id.substance().key(),
                component: row(stranger).substance().key(),
            })
        );
    };
    refused(titan(), &[(row("H2O"), 1.0)], "H2O");
    refused(titan(), &[(row("CH4"), 0.9), (row("NH3"), 0.1)], "NH3");
    refused(
        titan(),
        &[(row("NH3"), 0.1), (row("H2O"), 0.1), (row("CH4"), 0.8)],
        "NH3",
    );
    let water = row("H2O");
    let warm = Kelvin::new(288.15);
    let fractions = [(water, 0.5), (row("NaCl"), 0.5)];
    let given = Composition::Given(LiquidComposition::new(&fractions).unwrap());
    assert_eq!(
        liquid_properties(water, warm, given),
        Err(ResolveLiquidError::ComponentNotModelled {
            key: water.substance().key(),
            component: row("NaCl").substance().key(),
        })
    );
}

/// One path (decision-p14-t49h-step3-composition §1.5 and §5, item 2): for every row whose
/// reference composition is stated, `Reference` is `Given(reference_composition())` to the bit,
/// across its range; a pure liquid given another row beside itself is refused; and a row with no
/// components (the perchlorate brines) has no reference composition and refuses any given one.
#[test]
fn the_reference_is_its_composition_given() {
    let mut stated = 0;
    for (key, columns) in &LIQUIDS {
        let id = row(key.as_str());
        let lowest = id
            .substance()
            .condensed()
            .unwrap()
            .transition()
            .temperature()
            .value();
        let Some(reference) = columns.reference_composition() else {
            assert!(columns.components().is_empty(), "{key}");
            let species = id.substance().key().form() == SubstanceKeyForm::Formula;
            let fractions = [(if species { id } else { row("H2O") }, 1.0)];
            let given = Composition::Given(LiquidComposition::new(&fractions).unwrap());
            assert_eq!(
                liquid_properties(id, Kelvin::new(lowest), given),
                Err(ResolveLiquidError::CompositionNotModelled { key: *key })
            );
            continue;
        };
        stated += 1;
        let top = columns
            .components()
            .iter()
            .filter_map(|c| c.substance().phase_diagram())
            .map(|d| d.critical_temperature().value())
            .fold(f64::INFINITY, f64::min);
        for fraction in [0.0, 0.3, 0.6, 0.999] {
            let t = lowest + fraction * (top - lowest);
            assert_same_record(
                &record(id, t, Composition::Given(reference)),
                &record(id, t, Composition::Reference),
            );
        }
        if let LiquidConstitution::Pure { .. } = columns.constitution {
            let other = if key.as_str() == "H2O" {
                row("CH4")
            } else {
                row("H2O")
            };
            let fractions = [(id, 0.5), (other, 0.5)];
            let given = Composition::Given(LiquidComposition::new(&fractions).unwrap());
            assert_eq!(
                liquid_properties(id, Kelvin::new(lowest), given),
                Err(ResolveLiquidError::ComponentNotModelled {
                    key: *key,
                    component: other.substance().key(),
                })
            );
        }
    }
    assert_eq!(
        stated, 10,
        "five pure liquids, the two modelled brines, ammonia–water, the acid and Titan's seas"
    );
}

/// Every stated reference composition is one `LiquidComposition::new` accepts unchanged, over the
/// row's components in their order, and each liquid's components are its rows, in registry order:
/// a pure liquid's itself; an ideal mixture's and a one-composition model's species rows with
/// liquid columns of their own; an aqueous electrolyte's water, which has them, and its salt, a
/// species row with no phase diagram; the aqueous acid's water and the acid's own row, which has
/// no phase diagram (decision-p14-t49h-step3-composition §1, item 4).
#[test]
fn each_liquid_s_components_are_its_rows() {
    for (key, columns) in &LIQUIDS {
        let id = row(key.as_str());
        let components = columns.components();
        assert!(components.windows(2).all(|w| w[0] < w[1]), "{key}");
        let Some(reference) = columns.reference_composition() else {
            continue;
        };
        let checked = LiquidComposition::new(reference.mole_fractions()).unwrap();
        assert_eq!(checked, reference, "{key}");
        let named: Vec<SubstanceId> = reference.mole_fractions().iter().map(|&(c, _)| c).collect();
        assert_eq!(named, components, "{key}");
        match &columns.constitution {
            LiquidConstitution::Pure { .. } => assert_eq!(components, [id], "{key}"),
            LiquidConstitution::Solution(solution) => {
                match solution.model {
                    MixingModel::AqueousElectrolyte(model) => {
                        assert_eq!(components, [model.solvent, model.solute], "{key}");
                        assert_eq!(model.solvent, row("H2O"), "{key}");
                        assert!(model.solute.substance().phase_diagram().is_none(), "{key}");
                        assert!(model.solute.substance().liquid().is_none(), "{key}");
                        continue;
                    }
                    MixingModel::AqueousAcid(model) => {
                        assert_eq!(components, [model.solvent, model.solute], "{key}");
                        assert_eq!(model.solvent, row("H2O"), "{key}");
                        assert_eq!(model.solute, id, "{key}");
                        assert!(model.solute.substance().phase_diagram().is_none(), "{key}");
                        continue;
                    }
                    MixingModel::IdealLiquids(_) | MixingModel::ReferenceOnly => {}
                }
                for component in components {
                    let own = component.substance().liquid().unwrap();
                    assert!(
                        matches!(own.constitution, LiquidConstitution::Pure { .. }),
                        "{key}: {}",
                        component.substance().key()
                    );
                }
                match solution.model {
                    MixingModel::AqueousElectrolyte(_)
                    | MixingModel::AqueousAcid(_)
                    | MixingModel::ReferenceOnly => {}
                    MixingModel::IdealLiquids(mixing) => {
                        let n = components.len();
                        assert_eq!(mixing.grunberg_nissan.len(), n * (n - 1) / 2);
                        for (i, a) in components.iter().enumerate() {
                            for b in &components[i + 1..] {
                                let listed = mixing
                                    .grunberg_nissan
                                    .iter()
                                    .filter(|&&(p, q, _)| {
                                        (p == *a && q == *b) || (p == *b && q == *a)
                                    })
                                    .count();
                                assert_eq!(listed, 1, "{key}: {a:?} and {b:?}");
                            }
                        }
                    }
                }
            }
            LiquidConstitution::AwaitingSolute(_) | LiquidConstitution::Melt(_) => {
                panic!("{key} states no reference composition")
            }
        }
    }
}

/// Titan's seas below their components' triple points (at their 71.15 K first melt, at 80 K and at
/// 90 K, under methane's 90.694 K and ethane's 90.368 K) resolve by §2.5's continuation: `Derived`,
/// every present member finite and above 0, the absorption absent; colder is denser and more
/// viscous. Methane and ethane are continued there, nitrogen is its own record.
#[test]
fn titan_below_its_components_triple_points_resolves() {
    let (reference, _) = titan_reference();
    let mut previous = record(titan(), reference.value(), Composition::Reference);
    for t in [90.0, 80.0, 71.15] {
        let sea = record(titan(), t, Composition::Reference);
        assert_eq!(sea.basis, LiquidBasis::Derived, "{t} K");
        assert!(sea.density_kg_m3.is_finite() && sea.density_kg_m3 > previous.density_kg_m3);
        assert!(
            sea.refractive_index
                .iter()
                .all(|n| n.is_finite() && *n > 1.0),
            "{t} K: {:?}",
            sea.refractive_index
        );
        assert_eq!(sea.absorption_per_m, None, "{t} K");
        let b = sea.scattering_per_m.unwrap();
        assert!(b.iter().all(|v| v.is_finite() && *v > 0.0), "{t} K: {b:?}");
        let sigma = sea.surface_tension_n_per_m.unwrap();
        assert!(sigma.is_finite() && sigma > 0.0, "{t} K: {sigma}");
        let mu = sea.viscosity_pa_s.unwrap();
        assert!(
            mu.is_finite() && mu > previous.viscosity_pa_s.unwrap(),
            "{t} K: {mu}"
        );
        for key in ["CH4", "C2H6"] {
            let state = component_state(row(key), Kelvin::new(t)).unwrap();
            assert_eq!(state.basis, LiquidBasis::Derived, "{key} at {t} K");
            assert!(state.viscosity_pa_s.is_some(), "{key} at {t} K");
        }
        let nitrogen = component_state(row("N2"), Kelvin::new(t)).unwrap();
        let own = record(row("N2"), t, Composition::Reference);
        assert_same_bits(nitrogen.density.value(), own.density_kg_m3);
        previous = sea;
    }
}

/// An ideal mixture lies between its components: its density (Σ φᵢ ρᵢ), each bin's
/// Lorentz–Lorenz function (Σ φᵢ Lᵢ), its surface tension's fourth root (Σ φᵢ σᵢ^(1/4)) and, with
/// every Grunberg–Nissan G 0, its viscosity's logarithm (Σ xᵢ ln μᵢ) are each a weighted mean of
/// its components', from Titan's first melt to near nitrogen's critical point.
#[test]
fn the_ideal_mixture_lies_between_its_components() {
    let components = ["CH4", "N2", "C2H6"];
    for t in [71.15, 80.0, 93.65, 110.0, 125.0] {
        let sea = record(titan(), t, Composition::Reference);
        let states = components.map(|key| component_state(row(key), Kelvin::new(t)).unwrap());
        let between = |value: f64, of: &dyn Fn(&ComponentState) -> f64, what: &str| {
            let low = states.iter().map(of).fold(f64::INFINITY, f64::min);
            let high = states.iter().map(of).fold(f64::NEG_INFINITY, f64::max);
            assert!(
                (low..=high).contains(&value),
                "{what} at {t} K: {value} outside [{low}, {high}]"
            );
        };
        between(sea.density_kg_m3, &|s| s.density.value(), "density");
        for k in 0..LIQUID_BINS {
            between(sea.refractive_index[k], &|s| s.refractive_index[k], "index");
        }
        between(
            sea.surface_tension_n_per_m.unwrap(),
            &|s| s.surface_tension_n_per_m.unwrap(),
            "surface tension",
        );
        between(
            sea.viscosity_pa_s.unwrap(),
            &|s| s.viscosity_pa_s.unwrap(),
            "viscosity",
        );
    }
}

/// The range (decision-p14-t49h-step3-composition §3 and §5, item 5) runs from the solution's
/// first melt, 71.15 K, to below the lowest critical point among the components the composition
/// holds: nitrogen's 126.192 K for the reference, methane's 190.564 K for a nitrogen-free binary,
/// which resolves at 150 K; outside it the record is refused, reporting those bounds.
#[test]
fn titan_s_range_runs_to_its_lowest_component_s_critical_point() {
    let check = |composition: Composition<'_>, critical: f64| {
        assert!(liquid_properties(titan(), Kelvin::new(71.15), composition).is_ok());
        let top = Kelvin::new(critical - 1e-6);
        assert!(liquid_properties(titan(), top, composition).is_ok());
        for t in [71.149, critical, critical + 10.0, f64::NAN] {
            match liquid_properties(titan(), Kelvin::new(t), composition) {
                Err(ResolveLiquidError::OutOfRange {
                    key,
                    lowest,
                    below: Some(below),
                    ..
                }) => {
                    assert_eq!(key, titan().substance().key());
                    assert_same_bits(lowest.value(), 71.15);
                    assert_same_bits(below.value(), critical);
                }
                other => panic!("{t} K: {other:?}"),
            }
        }
    };
    check(Composition::Reference, 126.192);
    let fractions = [(row("C2H6"), 0.4), (row("CH4"), 0.6)];
    let binary = Composition::Given(LiquidComposition::new(&fractions).unwrap());
    check(binary, 190.564);
    assert!(liquid_properties(titan(), Kelvin::new(150.0), binary).is_ok());
}

/// A given composition's order moves nothing (decision-p14-t49h-step3-composition §5, item 3): the
/// rules sum over the model's components in registry order.
#[test]
fn a_given_composition_s_order_does_not_move_the_record() {
    let (ch4, n2, c2h6) = (row("CH4"), row("N2"), row("C2H6"));
    let orders = [
        [(ch4, 0.7), (n2, 0.2), (c2h6, 0.1)],
        [(c2h6, 0.1), (n2, 0.2), (ch4, 0.7)],
        [(n2, 0.2), (ch4, 0.7), (c2h6, 0.1)],
    ];
    for t in [75.0, 93.65, 120.0] {
        let first = record(
            titan(),
            t,
            Composition::Given(LiquidComposition::new(&orders[0]).unwrap()),
        );
        for order in &orders[1..] {
            let other = record(
                titan(),
                t,
                Composition::Given(LiquidComposition::new(order).unwrap()),
            );
            assert_same_record(&first, &other);
        }
    }
}

/// The binary composition of `solvent` and `solute` at the solute's mass fraction `w`, by mole
/// fraction with the rows' molar masses, in registry order (the solvent's first): `x_s` = (w ÷
/// `M_s`) ÷ (w ÷ `M_s` + (1 − w) ÷ `M_w`).
fn binary(solvent: &str, solute: &str, w: f64) -> [(SubstanceId, f64); 2] {
    let molar_mass = |key: &str| row(key).substance().molar_mass_g_per_mol().unwrap();
    let salt = w / molar_mass(solute);
    let water = (1.0 - w) / molar_mass(solvent);
    [
        (row(solvent), water / (salt + water)),
        (row(solute), salt / (salt + water)),
    ]
}

/// The row's first melt, its condensed columns' transition, K.
fn first_melt(key: &str) -> f64 {
    row(key)
        .substance()
        .condensed()
        .unwrap()
        .transition()
        .temperature()
        .value()
}

/// Titan's seas' scattering at their reference state, 93.65 K, is the lane's research's
/// (`.git/rm45-scratch/p14-liquids/step3/mixtures/mixtures.json`'s `5_scattering.numbers_93_65`,
/// from `scripts/mixtures.py`, which ports the code's pure-fluid forms and meets `liquids.golden`
/// to 3.4 × 10⁻⁷): in the bins across 443 and 545 nm, 6.958 × 10⁻³ and 2.913 × 10⁻³ m⁻¹ in all, of
/// which the density term is 3.269 × 10⁻³ and 1.383 × 10⁻³ and the concentration term 3.689 ×
/// 10⁻³ and 1.530 × 10⁻³, each within 2 × 10⁻⁵ relative (the research took the mole fractions to
/// six decimals and `S_0` from `k_B` with the rows' molar masses, where the code takes the
/// equations' gas constants, 3 × 10⁻⁵ apart in `S_0`). The record's scattering is the total to the
/// bit, and the concentration term is positive in every bin.
#[test]
fn titan_s_scattering_is_the_research_s() {
    let (t, _) = titan_reference();
    let sea = record(titan(), t.value(), Composition::Reference);
    let LiquidConstitution::Solution(solution) = titan().substance().liquid().unwrap().constitution
    else {
        panic!("Titan's seas are a solution");
    };
    let mut total = 0.0;
    for &(_, x) in solution.reference {
        total += x;
    }
    let parts = parts(solution.reference, total, t).unwrap();
    let mut molar_mass = 0.0;
    let mut molar_volume = 0.0;
    for part in &parts {
        molar_mass += part.fraction * part.molar_mass_kg_per_mol;
        molar_volume += part.fraction * part.molar_volume_m3_per_mol;
    }
    let lorentz = mixed_lorentz_lorenz(&parts, molar_volume);
    let index = lorentz.map(from_lorentz_lorenz);
    let scattering =
        mixture_scattering(&parts, molar_mass, molar_volume, &lorentz, &index, t).unwrap();
    for (b, total) in sea.scattering_per_m.unwrap().iter().zip(&scattering.total) {
        assert_same_bits(*b, *total);
    }
    assert!(scattering.concentration_term.iter().all(|c| *c > 0.0));
    for (bin, whole, density, concentration) in [
        (
            2,
            0.006_958_194_494_348_193_5,
            0.003_269_251_107_465_887_2,
            0.003_688_943_386_882_306_2,
        ),
        (
            6,
            0.002_913_358_543_796_551_6,
            0.001_382_895_088_524_733_2,
            0.001_530_463_455_271_818_2,
        ),
    ] {
        let what = |part: &str| format!("bin {bin}, {part}");
        assert_close(scattering.total[bin], whole, 2e-5, &what("total"));
        assert_close(
            scattering.density_term[bin],
            density,
            2e-5,
            &what("density"),
        );
        assert_close(
            scattering.concentration_term[bin],
            concentration,
            2e-5,
            &what("concentration"),
        );
    }
}

/// A brine given water alone is water's own record within 10⁻¹² relative
/// (decision-p14-t49h-step3-composition §5, item 4), at 288.15, 298.15 and 350 K inside both
/// ranges, in every member both give: density, index, absorption (the chloride brine's; the
/// sulphate brine's is absent) and viscosity. The brines give no surface tension or scattering, and
/// are `Derived`.
#[test]
fn a_brine_of_water_alone_is_water_s_record() {
    let water = row("H2O");
    let fractions = [(water, 1.0)];
    let alone = Composition::Given(LiquidComposition::new(&fractions).unwrap());
    for key in ["brine_nacl", "brine_mgso4"] {
        for t in [288.15, 298.15, 350.0] {
            let brine = record(row(key), t, alone);
            let own = record(water, t, Composition::Reference);
            let what = format!("{key} at {t} K");
            assert_close(brine.density_kg_m3, own.density_kg_m3, 1e-12, &what);
            for (n, m) in brine.refractive_index.iter().zip(&own.refractive_index) {
                assert_close(*n, *m, 1e-12, &what);
            }
            match brine.absorption_per_m {
                Some(a) => {
                    for (x, y) in a.iter().zip(&own.absorption_per_m.unwrap()) {
                        assert_close(*x, *y, 1e-12, &what);
                    }
                }
                None => assert_eq!(key, "brine_mgso4"),
            }
            assert_close(
                brine.viscosity_pa_s.unwrap(),
                own.viscosity_pa_s.unwrap(),
                1e-12,
                &what,
            );
            assert_eq!(brine.surface_tension_n_per_m, None, "{what}");
            assert_eq!(brine.scattering_per_m, None, "{what}");
            assert_eq!(brine.basis, LiquidBasis::Derived, "{what}");
        }
    }
}

/// The brines' and ammonia–water's reference compositions are their eutectic columns' solute mass
/// fractions (`NaCl` 0.233, `MgSO4` 0.173, NH3 0.321) converted to mole fractions with the rows'
/// molar masses (decision-p14-t49h-step3-composition §5, item 7), held as that arithmetic's own
/// values: the solute's 0.085 627, 0.030 360 and 0.333 364 to six decimals; and they give back the
/// eutectic's mass fraction.
#[test]
fn the_aqueous_references_are_their_eutectics() {
    for (key, solute, six) in [
        ("brine_nacl", "NaCl", 0.085_627),
        ("brine_mgso4", "MgSO4", 0.030_360),
        ("nh3_h2o", "NH3", 0.333_364),
    ] {
        let id = row(key);
        let TransitionKind::Eutectic {
            solute_mass_fraction,
        } = id.substance().condensed().unwrap().transition().kind()
        else {
            panic!("{key}'s first melt is a eutectic");
        };
        let expected = binary("H2O", solute, solute_mass_fraction);
        let reference = id
            .substance()
            .liquid()
            .unwrap()
            .reference_composition()
            .unwrap();
        assert_eq!(reference.mole_fractions().len(), 2, "{key}");
        for (&(component, held), &(named, x)) in reference.mole_fractions().iter().zip(&expected) {
            assert_eq!(component, named, "{key}");
            assert!(
                (held - x).abs() < 1e-15,
                "{key}: held {held:?}, computed {x:?}"
            );
        }
        let x = reference.mole_fractions()[1].1;
        assert!(((x * 1e6).round() / 1e6 - six).abs() < 1e-12, "{key}: {x}");
        let molar_mass = |k: &str| row(k).substance().molar_mass_g_per_mol().unwrap();
        let w = x * molar_mass(solute)
            / (x * molar_mass(solute) + reference.mole_fractions()[0].1 * molar_mass("H2O"));
        assert!((w - solute_mass_fraction).abs() < 1e-15, "{key}: {w}");
    }
}

/// decision-p14-t49h-step3-composition §4's table for the rows here: each solution's density at
/// its stated value's own temperature and its reference composition, against T49.b's stated
/// value, never anchored to it.
///
/// - **Ammonia–water and the perchlorate brines** take the stated value itself (946 ± 11, 1,445 ±
///   10 and 1,394 ± 15 kg m⁻³), so they agree to the bit; the perchlorates' record is refused for
///   its index, so their density is read where the record reads it.
/// - **The chloride and sulphate brines' models**, Laliberté and Cooper 2004's with the sim's
///   water, miss beyond the stated uncertainties (±0.3 and ±0.5): by +0.735 and −2.165 kg m⁻³
///   (+0.790 and −2.105 with their own water equation, brines.json's "anchors" finding), each
///   pinned here to ±0.01 and recorded for main, not anchored away (ruling §4). The stated values,
///   the International Critical Tables' measurements at 0 °C interpolated in w, are likely the
///   better at this state: the model sits 0.76–0.88 kg m⁻³ above ICT's `NaCl` at 22–24 wt% and
///   1.7–2.3 below its `MgSO4` at 16–18 wt% at 0 °C, while it meets ICT's `MgSO4` at 20 °C within
///   0.22, a bias at the fits' lowest temperature (brines.json's checks).
#[test]
fn the_solutions_densities_agree_with_their_stated_values_or_record_their_misses() {
    let stated = |key: &str| {
        let GrainDensity::Fixed {
            liquid: Some(given),
            ..
        } = row(key).substance().condensed().unwrap().grain_density()
        else {
            panic!("{key} states its liquid's density");
        };
        given
    };
    for (key, value, miss) in [
        ("brine_nacl", 1_184.1, 0.735),
        ("brine_mgso4", 1_193.3, -2.165),
        ("nh3_h2o", 946.0, 0.0),
    ] {
        let given = stated(key);
        assert_same_bits(given.density().value(), value);
        let solution = record(row(key), given.at().value(), Composition::Reference);
        let off = solution.density_kg_m3 - value;
        assert!((off - miss).abs() < 0.01, "{key}: {off:+} kg m⁻³");
    }
    let ammonia_water = record(row("nh3_h2o"), 176.15, Composition::Reference);
    assert_same_bits(ammonia_water.density_kg_m3, 946.0);
    for (key, value) in [("brine_naclo4", 1_445.0), ("brine_mgclo4", 1_394.0)] {
        let given = stated(key);
        let (density, basis) = record_density(row(key).substance(), given.at()).unwrap();
        assert_same_bits(density.value(), value);
        assert_eq!(basis, LiquidBasis::Measured, "{key}");
    }
}

/// R08's `seawater.json` is the brines' oracle for the salinity increment
/// (decision-p14-t49h-liquids §4): a 3.5 wt% `NaCl` brine at 25 °C raises water's index in the bin
/// across 550 nm (532–557 nm) by +0.006 06, against Quan and Fry 1995's +0.006 39 for seawater at S
/// = 35 (the file's reduction), within 6 × 10⁻⁴. brines.json expects −0.000 34 by its stand-in
/// dispersion at 550 nm: sea salt is about 78% `NaCl` by mass, its other salts differing in
/// specific refraction (that check's note), so `NaCl` falls short by a composition effect, not a
/// model miss. The increment rises towards the blue, as seawater's does.
#[test]
fn a_seawater_like_brine_meets_quan_and_fry_s_increment() {
    let fractions = binary("H2O", "NaCl", 0.035);
    let given = Composition::Given(LiquidComposition::new(&fractions).unwrap());
    let brine = record(row("brine_nacl"), 298.15, given);
    let water = record(row("H2O"), 298.15, Composition::Reference);
    let increment = |k: usize| brine.refractive_index[k] - water.refractive_index[k];
    assert!((increment(6) - 0.006_39).abs() < 6e-4, "{}", increment(6));
    assert!((increment(6) - 0.006_06).abs() < 2e-5, "{}", increment(6));
    assert!(increment(0) > increment(6) && increment(6) > increment(14));
}

/// A brine's salt runs from 0 to its density model's highest fraction, Laliberté and Cooper's
/// highest fitted mass fraction (`NaCl` 0.2603, `MgSO4` 0.26) converted with the rows' molar masses
/// (decision-p14-t49h-step3-composition §2.3): at it the brine resolves, and past it the
/// composition is refused, naming the first component out of range in the given order, water's
/// range being the salt's complement. A component the model does not carry is refused before any
/// fraction (§5, item 1: `{H2O, CH4}`).
#[test]
fn a_brine_past_its_highest_fraction_is_refused() {
    let t = Kelvin::new(298.15);
    for (key, salt, highest) in [
        ("brine_nacl", "NaCl", 0.260_310_803_875_142_66),
        ("brine_mgso4", "MgSO4", 0.26),
    ] {
        let id = row(key);
        let given = |fractions: &[(SubstanceId, f64)]| {
            liquid_properties(
                id,
                t,
                Composition::Given(LiquidComposition::new(fractions).unwrap()),
            )
        };
        let at = binary("H2O", salt, highest);
        assert!(given(&at).is_ok(), "{key}");
        let x_max = at[1].1;
        let past = binary("H2O", salt, highest + 0.01);
        match given(&[past[1], past[0]]) {
            Err(ResolveLiquidError::CompositionOutOfRange {
                key: refused,
                component,
                mole_fraction,
                lowest,
                highest,
            }) => {
                assert_eq!(refused, id.substance().key());
                assert_eq!(component, row(salt).substance().key());
                assert_same_bits(mole_fraction, past[1].1);
                assert_same_bits(lowest, 0.0);
                assert!((highest - x_max).abs() < 1e-15, "{key}: {highest}");
            }
            other => panic!("{key}: {other:?}"),
        }
        match given(&past) {
            Err(ResolveLiquidError::CompositionOutOfRange {
                component,
                lowest,
                highest,
                ..
            }) => {
                assert_eq!(component, row("H2O").substance().key());
                assert!((lowest - (1.0 - x_max)).abs() < 1e-15, "{key}: {lowest}");
                assert_same_bits(highest, 1.0);
            }
            other => panic!("{key}: {other:?}"),
        }
        for fractions in [
            [(row("H2O"), 0.9), (row("CH4"), 0.1)],
            [(row(salt), 0.5), (row("CH4"), 0.5)],
        ] {
            assert_eq!(
                given(&fractions),
                Err(ResolveLiquidError::ComponentNotModelled {
                    key: id.substance().key(),
                    component: row("CH4").substance().key(),
                }),
                "{key}"
            );
        }
    }
}

/// A brine's range runs from its eutectic (`NaCl` 251.9 K, `MgSO4` 269.29 K) to below water's
/// critical point, 647.096 K, its salt having no phase diagram (decision-p14-t49h-step3-composition
/// §3), at its reference and for water alone; outside it the record is refused with those bounds.
#[test]
fn a_brine_s_range_runs_from_its_eutectic_to_water_s_critical_point() {
    let fractions = [(row("H2O"), 1.0)];
    let alone = Composition::Given(LiquidComposition::new(&fractions).unwrap());
    for key in ["brine_nacl", "brine_mgso4"] {
        let id = row(key);
        let eutectic = first_melt(key);
        for composition in [Composition::Reference, alone] {
            for t in [eutectic, 647.096 - 1e-6] {
                let record = liquid_properties(id, Kelvin::new(t), composition).unwrap();
                assert!(
                    record.density_kg_m3.is_finite() && record.density_kg_m3 > 0.0,
                    "{key} at {t} K"
                );
            }
            for t in [eutectic - 1e-3, 647.096, 700.0, f64::NAN] {
                match liquid_properties(id, Kelvin::new(t), composition) {
                    Err(ResolveLiquidError::OutOfRange {
                        lowest,
                        below: Some(below),
                        ..
                    }) => {
                        assert_same_bits(lowest.value(), eutectic);
                        assert_same_bits(below.value(), 647.096);
                    }
                    other => panic!("{key} at {t} K: {other:?}"),
                }
            }
        }
    }
}

/// Below water's triple point, where IAPWS R12-08 is not evaluated, a brine's water viscosity is
/// Laliberté's water equation scaled to R12-08's value at 273.16 K: so the brine's viscosity is
/// continuous there, within 10⁻⁶ relative a microkelvin below, and rises as it cools to its
/// eutectic; the scale is 1.000 67; and at and above the triple point the form gives way to
/// R12-08's own. At the brines' 273.15 K reference states they are 3.09 and 6.46 mPa s, as their
/// sources state (brines.json's 3.084 and 6.457 with Laliberté's own water).
#[test]
fn a_brine_s_viscosity_is_continuous_at_water_s_triple_point() {
    let water = row("H2O");
    let triple = 273.16;
    for key in ["brine_nacl", "brine_mgso4"] {
        let mu = |t: f64| {
            record(row(key), t, Composition::Reference)
                .viscosity_pa_s
                .unwrap()
        };
        let (at, below) = (mu(triple), mu(triple - 1e-6));
        assert_close(below, at, 1e-6, key);
        assert!(below > at, "{key}");
        assert!(mu(first_melt(key)) > below, "{key}");
        let reference = if key == "brine_nacl" {
            3.09e-3
        } else {
            6.46e-3
        };
        assert!(
            (mu(273.15) - reference).abs() < 5e-6,
            "{key}: {}",
            mu(273.15)
        );
    }
    assert_eq!(
        supercooled_water_viscosity_pa_s(water, Kelvin::new(triple)),
        None
    );
    let continued = supercooled_water_viscosity_pa_s(water, Kelvin::new(triple - 1e-9)).unwrap();
    let own = record(water, triple, Composition::Reference)
        .viscosity_pa_s
        .unwrap();
    assert_close(continued, own, 1e-9, "water at its triple point");
    let laliberte = (0.01 + 246.0) / ((0.055_94 * 0.01 + 5.2842) * 0.01 + 137.37) * 1e-3;
    let scale = own / laliberte;
    assert!((scale - 1.000_67).abs() < 1e-5, "{scale}");
}

/// A brine's given composition's order moves nothing (decision-p14-t49h-step3-composition §5,
/// item 3): its terms sum water's first, in registry order.
#[test]
fn a_brine_s_composition_order_does_not_move_the_record() {
    for (key, salt) in [("brine_nacl", "NaCl"), ("brine_mgso4", "MgSO4")] {
        let [water, solute] = binary("H2O", salt, 0.1);
        let ordered = [water, solute];
        let reversed = [solute, water];
        for t in [first_melt(key) + 1.0, 298.15, 500.0] {
            assert_same_record(
                &record(
                    row(key),
                    t,
                    Composition::Given(LiquidComposition::new(&ordered).unwrap()),
                ),
                &record(
                    row(key),
                    t,
                    Composition::Given(LiquidComposition::new(&reversed).unwrap()),
                ),
            );
        }
    }
}

/// The perchlorate brines wait for their solutes' rows (decision-p14-t49h-step3-composition §1.3
/// and §5, item 1): no components and no reference composition, so any given composition is
/// `CompositionNotModelled`; their reference is `NotModelled` for the index, which no source gives,
/// from the eutectic to below water's 647.096 K, and `OutOfRange` outside, its upper end water's
/// critical point as the other brines' is (§3), refused for the range before the index (§2.3's
/// order); and each solute's held molar mass is its formula's IUPAC 2021 abridged weights summed,
/// the registry's arithmetic (`NaClO4` 122.436, Mg(ClO4)2 223.197 g mol⁻¹).
#[test]
fn the_perchlorate_brines_wait_for_their_solutes() {
    let sodium: &[(Element, u16)] = &[(Element::Na, 1), (Element::Cl, 1), (Element::O, 4)];
    let magnesium: &[(Element, u16)] = &[(Element::Mg, 1), (Element::Cl, 2), (Element::O, 8)];
    for (key, formula, held) in [
        ("brine_naclo4", sodium, 122.436),
        ("brine_mgclo4", magnesium, 223.197),
    ] {
        let id = row(key);
        let columns = id.substance().liquid().unwrap();
        assert!(columns.components().is_empty(), "{key}");
        assert_eq!(columns.reference_composition(), None, "{key}");
        let LiquidConstitution::AwaitingSolute(solute) = columns.constitution else {
            panic!("{key} awaits its solute");
        };
        assert_eq!(solute.elements, formula, "{key}");
        let summed = solute.elements.iter().fold(0.0, |sum, &(e, n)| {
            sum + f64::from(n) * e.standard_atomic_weight_g_per_mol()
        });
        assert_same_bits(solute.molar_mass_g_per_mol, held);
        assert!((summed - held).abs() < 1e-9, "{key}: {summed}");
        let key_of = id.substance().key();
        let eutectic = Kelvin::new(first_melt(key));
        assert_eq!(
            liquid_properties(id, eutectic, Composition::Reference),
            Err(ResolveLiquidError::NotModelled {
                key: key_of,
                member: LiquidMember::RefractiveIndex,
            })
        );
        let top = Kelvin::new(647.096 - 1e-6);
        assert_eq!(
            liquid_properties(id, top, Composition::Reference),
            Err(ResolveLiquidError::NotModelled {
                key: key_of,
                member: LiquidMember::RefractiveIndex,
            })
        );
        for t in [eutectic.value() - 1.0, 647.096, 700.0, f64::NAN] {
            match liquid_properties(id, Kelvin::new(t), Composition::Reference) {
                Err(ResolveLiquidError::OutOfRange {
                    key: refused,
                    lowest,
                    below: Some(below),
                    ..
                }) => {
                    assert_eq!(refused, key_of);
                    assert_same_bits(lowest.value(), eutectic.value());
                    assert_same_bits(below.value(), 647.096);
                }
                other => panic!("{key} at {t} K: {other:?}"),
            }
        }
        for fractions in [
            [(row("H2O"), 1.0)].as_slice(),
            &[(row("H2O"), 0.9), (row("NaCl"), 0.1)],
        ] {
            assert_eq!(
                liquid_properties(
                    id,
                    eutectic,
                    Composition::Given(LiquidComposition::new(fractions).unwrap())
                ),
                Err(ResolveLiquidError::CompositionNotModelled { key: key_of })
            );
        }
    }
}

/// Ammonia–water at its eutectic, 176.15 K, is its components mixed at its stated density
/// ([`MixingModel::ReferenceOnly`]): the density 946 kg m⁻³, the index Lorentz–Lorenz from water's
/// and ammonia's molar refractions at that density (1.394 43, 1.388 57, 1.381 39 and 1.374 42 in
/// the bins across 393, 443, 545 and 747 nm) and the surface tension the parachor's at that
/// density (83.83 mN m⁻¹), each the lane's research's (mixtures.json's `ammonia_water_nh3_h2o`,
/// from `scripts/mixtures.py`) within 10⁻⁹ relative; no viscosity, absorption or scattering; and
/// `Derived`, as it is away from 176.15 K, where the density is held.
#[test]
fn ammonia_water_is_its_components_mixed_at_its_stated_density() {
    let id = row("nh3_h2o");
    let eutectic = record(id, 176.15, Composition::Reference);
    assert_same_bits(eutectic.density_kg_m3, 946.0);
    for (bin, n) in [
        (0, 1.394_432_778_013_636_5),
        (2, 1.388_573_448_129_866_6),
        (6, 1.381_394_930_736_625),
        (14, 1.374_423_323_379_643),
    ] {
        assert_close(
            eutectic.refractive_index[bin],
            n,
            1e-9,
            &format!("bin {bin}"),
        );
    }
    assert_close(
        eutectic.surface_tension_n_per_m.unwrap(),
        0.083_826_043_916_488_12,
        1e-9,
        "surface tension",
    );
    assert_eq!(eutectic.viscosity_pa_s, None);
    assert_eq!(eutectic.absorption_per_m, None);
    assert_eq!(eutectic.scattering_per_m, None);
    assert_eq!(eutectic.basis, LiquidBasis::Derived);
    let warm = record(id, 300.0, Composition::Reference);
    assert_same_bits(warm.density_kg_m3, 946.0);
    assert_eq!(warm.basis, LiquidBasis::Derived);
    assert!(warm.surface_tension_n_per_m.unwrap() < eutectic.surface_tension_n_per_m.unwrap());
}

/// Ammonia–water evaluates its eutectic composition alone (Croft et al. 1988 and Kargel et al. 1991
/// unread): any other composition is `CompositionOutOfRange`, its `lowest` and `highest` both the
/// reference's fraction of the first component given; one within 10⁻⁹ of the reference resolves
/// as the reference does, within round-off; a component the model does not carry is refused
/// first; and its range runs from its 176.15 K eutectic to below ammonia's 405.56 K
/// (decision-p14-t49h-step3-composition §3).
#[test]
fn ammonia_water_refuses_any_other_composition() {
    let id = row("nh3_h2o");
    let t = Kelvin::new(200.0);
    let [(water, x_w), (ammonia, x_a)] = binary("H2O", "NH3", 0.321);
    let given = |fractions: &[(SubstanceId, f64)]| {
        liquid_properties(
            id,
            t,
            Composition::Given(LiquidComposition::new(fractions).unwrap()),
        )
    };
    for (fractions, first, reference) in [
        ([(ammonia, 0.5), (water, 0.5)], ammonia, x_a),
        ([(water, 0.7), (ammonia, 0.3)], water, x_w),
    ] {
        match given(&fractions) {
            Err(ResolveLiquidError::CompositionOutOfRange {
                component,
                lowest,
                highest,
                ..
            }) => {
                assert_eq!(component, first.substance().key());
                assert!((lowest - reference).abs() < 1e-15, "{lowest}");
                assert_same_bits(lowest, highest);
            }
            other => panic!("{other:?}"),
        }
    }
    assert!(matches!(
        given(&[(water, 1.0)]),
        Err(ResolveLiquidError::CompositionOutOfRange { .. })
    ));
    let near = [(water, x_w - 5e-10), (ammonia, x_a + 5e-10)];
    let shifted = given(&near).unwrap();
    let own = liquid_properties(id, t, Composition::Reference).unwrap();
    assert_close(shifted.density_kg_m3, own.density_kg_m3, 1e-15, "density");
    for (n, m) in shifted.refractive_index.iter().zip(&own.refractive_index) {
        assert_close(*n, *m, 1e-8, "index");
    }
    assert_eq!(
        given(&[(water, 0.6), (row("NaCl"), 0.4)]),
        Err(ResolveLiquidError::ComponentNotModelled {
            key: id.substance().key(),
            component: row("NaCl").substance().key(),
        })
    );
    for t in [176.15 - 1e-3, 405.56, 450.0] {
        match liquid_properties(id, Kelvin::new(t), Composition::Reference) {
            Err(ResolveLiquidError::OutOfRange {
                lowest,
                below: Some(below),
                ..
            }) => {
                assert_same_bits(lowest.value(), 176.15);
                assert_same_bits(below.value(), 405.56);
            }
            other => panic!("{t} K: {other:?}"),
        }
    }
    assert!(liquid_properties(id, Kelvin::new(405.56 - 1e-6), Composition::Reference).is_ok());
}

/// The aqueous acid's model, `H2SO4`'s liquid columns'.
fn acid() -> &'static AqueousAcid {
    let LiquidConstitution::Solution(solution) =
        row("H2SO4").substance().liquid().unwrap().constitution
    else {
        panic!("the acid is a solution");
    };
    let MixingModel::AqueousAcid(model) = solution.model else {
        panic!("the acid's model is an aqueous acid's");
    };
    model
}

/// The acid at mass fraction `w` and `t` K, given by mole fraction in registry order.
fn acid_at(w: f64, t: f64) -> LiquidProperties {
    let fractions = binary("H2O", "H2SO4", w);
    record(
        row("H2SO4"),
        t,
        Composition::Given(LiquidComposition::new(&fractions).unwrap()),
    )
}

/// The acid's reference composition is the row's 75.0 wt% (decision-p14-t49h-step3-composition
/// §5, item 7) converted to mole fractions with the rows' molar masses, 18.015 and 98.072 g mol⁻¹,
/// held as that arithmetic's own values: `H2SO4` 0.355 29 to five decimals (the research's at the
/// row's 98.072; 0.355 27 at the older 98.079), and it gives back 0.75 by mass.
#[test]
fn the_acid_s_reference_is_75_wt_percent() {
    let reference = row("H2SO4")
        .substance()
        .liquid()
        .unwrap()
        .reference_composition()
        .unwrap();
    let expected = binary("H2O", "H2SO4", 0.75);
    assert_eq!(reference.mole_fractions().len(), 2);
    for (&(component, held), &(named, x)) in reference.mole_fractions().iter().zip(&expected) {
        assert_eq!(component, named);
        assert!((held - x).abs() < 1e-15, "held {held:?}, computed {x:?}");
    }
    let [(_, x_w), (_, x_a)] = expected;
    assert!(
        ((x_a * 1e5).round() / 1e5 - 0.355_29).abs() < 1e-12,
        "{x_a}"
    );
    let molar_mass = |k: &str| row(k).substance().molar_mass_g_per_mol().unwrap();
    assert_same_bits(molar_mass("H2SO4"), 98.072);
    let w = x_a * molar_mass("H2SO4") / (x_a * molar_mass("H2SO4") + x_w * molar_mass("H2O"));
    assert!((w - 0.75).abs() < 1e-15, "{w}");
}

/// The acid reproduces both of Palmer and Williams's files at their 300 K, the 75 wt% at its
/// reference and the 84.5 wt% given, in every bin: the index within 10⁻¹² and the absorption within
/// 10⁻¹² relative of `tables::liquid_optics`' bins; each file's state is its table label's; and the
/// record is `Derived`, with a surface tension and a viscosity and no scattering.
#[test]
fn the_acid_reproduces_its_two_files_at_300_k() {
    let model = acid();
    let [low, high] = model.files;
    for (file, label) in [
        (low, bins::SULPHURIC_ACID_75_TEMPERATURE_K),
        (high, bins::SULPHURIC_ACID_84_TEMPERATURE_K),
    ] {
        assert_same_bits(file.state.value(), 300.0);
        assert!((file.state.value() - label).abs() <= 0.5, "{label}");
    }
    assert_same_bits(low.mass_fraction, 0.75);
    assert_same_bits(high.mass_fraction, 0.845);
    let reference = record(row("H2SO4"), 300.0, Composition::Reference);
    for (acid, n, a) in [
        (
            reference,
            &bins::SULPHURIC_ACID_75_N,
            &bins::SULPHURIC_ACID_75_A,
        ),
        (
            acid_at(0.845, 300.0),
            &bins::SULPHURIC_ACID_84_N,
            &bins::SULPHURIC_ACID_84_A,
        ),
    ] {
        for (k, (ours, file)) in acid.refractive_index.iter().zip(n).enumerate() {
            assert!(
                (ours - file).abs() < 1e-12,
                "bin {k}: {ours} against {file}"
            );
        }
        for (k, (ours, file)) in acid.absorption_per_m.unwrap().iter().zip(a).enumerate() {
            assert_close(*ours, *file, 1e-12, &format!("absorption bin {k}"));
        }
        assert_eq!(acid.basis, LiquidBasis::Derived);
        assert_eq!(acid.scattering_per_m, None);
        assert!(acid.surface_tension_n_per_m.is_some() && acid.viscosity_pa_s.is_some());
    }
}

/// The acid's density is Myhre et al. 1998's Eq. 2 without its printed T⁴ column, as the lane's
/// research computes it (acid.json's `values_T4_dropped` and grid, to their 0.01 kg m⁻³): at the
/// condensed columns' 300 K and 75 wt% it agrees with their 1,662.6 within its stated ±0.5
/// (decision-p14-t49h-step3-composition §4; 1,662.59 here); it meets the International Critical
/// Tables' (vol. III, p. 57) within 1.7 kg m⁻³ at 70–85 wt% and 0–50 °C (the research's worst miss,
/// 1.61, at 85 wt% and 323.15 K); and it falls as the acid warms from its first melt to 373.15 K at
/// every mass fraction from 25 to 85 wt%, which the printed T⁴ column broke above about 295 K.
#[test]
fn the_acid_s_density_is_myhre_s_without_its_t4_column() {
    let GrainDensity::Fixed {
        liquid: Some(stated),
        ..
    } = row("H2SO4")
        .substance()
        .condensed()
        .unwrap()
        .grain_density()
    else {
        panic!("the acid states its liquid's density");
    };
    assert_same_bits(stated.at().value(), 300.0);
    let reference = record(row("H2SO4"), 300.0, Composition::Reference);
    let off = reference.density_kg_m3 - stated.density().value();
    assert!(off.abs() < 0.5, "{off:+} kg m⁻³");
    assert!((reference.density_kg_m3 - 1_662.59).abs() < 0.005);
    let density = |w: f64, t: f64| acid().density.density_kg_m3(w, Kelvin::new(t));
    for (w, t, research) in [
        (0.70, 220.0, 1_683.69),
        (0.75, 233.67, 1_730.41),
        (0.80, 273.15, 1_748.28),
        (0.845, 300.0, 1_766.66),
        (0.85, 323.15, 1_748.16),
    ] {
        assert!(
            (density(w, t) - research).abs() < 0.005,
            "{w} at {t} K: {} against {research}",
            density(w, t)
        );
    }
    for (w, t, ict) in [
        (0.70, 273.15, 1_629.25),
        (0.75, 298.15, 1_664.35),
        (0.80, 300.0, 1_720.16),
        (0.845, 300.0, 1_766.50),
        (0.75, 323.15, 1_641.15),
        (0.85, 323.15, 1_746.55),
    ] {
        assert!(
            (density(w, t) - ict).abs() < 1.7,
            "{w} at {t} K: {} against the tables' {ict}",
            density(w, t)
        );
    }
    for percent in 25..=85 {
        let w = f64::from(percent) / 100.0;
        let mut previous = density(w, 233.67);
        for step in 1..=28 {
            let t = 233.67 + (373.15 - 233.67) * f64::from(step) / 28.0;
            let rho = density(w, t);
            assert!(rho < previous, "{w} at {t} K: {rho} after {previous}");
            previous = rho;
        }
    }
}

/// The acid's index is specific refraction linear in mass fraction between the two files, each
/// at the density form's value at its 300 K state, carried by Lorentz–Lorenz to the record's
/// density:
///
/// - from the files' own 550 nm values (1.43105 and 1.43821) it gives the research's 1.43599 at 80
///   wt% and 300 K within 10⁻⁵ (acid.json's `n_550nm_80wt_300K`), and the record's bin across 545
///   nm lies within 2 × 10⁻⁴ of it, between the files' bins in every bin;
/// - colder, the acid is denser and more refractive, at constant specific refraction;
/// - extrapolated to Palmer and Williams's own 25, 38 and 50 wt% files at 300 K (ARIA's
///   `H2SO4_25%`, `38%` and `50%_300K_R_Palmer_1975.ri`, SHA-256 458d843c…, de53a908… and
///   a5492b35…, from its 75 and 84.5 wt% files' rows, ca84ce9a… and 6ae3eceb…), it meets them
///   within ±0.005 at 555.6 and 701.8 nm, where the composition range's lower end rests
///   (acid.json's checks).
#[test]
fn the_acid_s_index_is_specific_refraction_linear_in_mass_fraction() {
    let model = acid();
    let density = |w: f64, t: f64| model.density.density_kg_m3(w, Kelvin::new(t));
    let (low, high) = (density(0.75, 300.0), density(0.845, 300.0));
    let at = |n75: f64, n845: f64, w: f64| {
        let s = (w - 0.75) / (0.845 - 0.75);
        acid_index((n75, low), (n845, high), s, density(w, 300.0))
    };
    let n80 = at(1.431_05, 1.438_21, 0.80);
    assert!((n80 - 1.435_99).abs() < 1e-5, "{n80}");
    let eighty = acid_at(0.80, 300.0);
    assert!((eighty.refractive_index[6] - 1.435_99).abs() < 2e-4);
    for (k, n) in eighty.refractive_index.iter().enumerate() {
        let (a, b) = (bins::SULPHURIC_ACID_75_N[k], bins::SULPHURIC_ACID_84_N[k]);
        assert!(*n > a && *n < b, "bin {k}: {n} outside {a} to {b}");
    }
    let warm = record(row("H2SO4"), 300.0, Composition::Reference);
    let cold = record(row("H2SO4"), 250.0, Composition::Reference);
    assert!(cold.density_kg_m3 > warm.density_kg_m3);
    for (n, m) in cold.refractive_index.iter().zip(&warm.refractive_index) {
        assert!(n > m, "{n} against {m}");
        let (r_cold, r_warm) = (
            lorentz_lorenz(*n) / cold.density_kg_m3,
            lorentz_lorenz(*m) / warm.density_kg_m3,
        );
        assert_close(r_cold, r_warm, 1e-14, "specific refraction");
    }
    for (w, at_555, at_701) in [
        (0.25, 1.366, 1.363),
        (0.38, 1.384, 1.382),
        (0.50, 1.397, 1.394),
    ] {
        for (n75, n845, measured) in [(1.431, 1.438, at_555), (1.428, 1.436, at_701)] {
            let n = at(n75, n845, w);
            assert!(
                (n - measured).abs() < 0.005,
                "{w}: {n} against Palmer and Williams's {measured}"
            );
        }
    }
}

/// The acid's absorption is an upper bound (both files' k are): between the files, at 80 wt%, its
/// mass absorption coefficient α ÷ ρ in each bin is the files' mixed by mass, within 10⁻¹²
/// relative, so between the two; the 75 wt% file's is the larger in every bin, so outside the
/// files' fractions (at 50 and 85 wt%, and colder) the record holds it, and it is continuous at
/// 75 wt% and steps just past 84.5 wt%, beyond the composition's 10⁻⁹ slack.
#[test]
fn the_acid_s_absorption_is_an_upper_bound_between_its_files() {
    let model = acid();
    let density = |w: f64, t: f64| model.density.density_kg_m3(w, Kelvin::new(t));
    let (low, high) = (density(0.75, 300.0), density(0.845, 300.0));
    let per_kg = |k: usize| {
        (
            bins::SULPHURIC_ACID_75_A[k] / low,
            bins::SULPHURIC_ACID_84_A[k] / high,
        )
    };
    let eighty = acid_at(0.80, 300.0);
    let s = (0.80 - 0.75) / (0.845 - 0.75);
    for (k, a) in eighty.absorption_per_m.unwrap().iter().enumerate() {
        let (m75, m845) = per_kg(k);
        assert!(m75 > m845, "bin {k}");
        let mass = a / eighty.density_kg_m3;
        assert!(mass > m845 && mass < m75, "bin {k}: {mass}");
        assert_close(mass, (1.0 - s) * m75 + s * m845, 1e-12, &format!("bin {k}"));
    }
    for (w, t) in [(0.50, 300.0), (0.85, 300.0), (0.50, 240.0), (0.85, 280.0)] {
        let acid = acid_at(w, t);
        for (k, a) in acid.absorption_per_m.unwrap().iter().enumerate() {
            let (m75, _) = per_kg(k);
            assert_close(a / acid.density_kg_m3, m75, 1e-12, &format!("{w} at {t} K"));
        }
    }
    let below = acid_at(0.75 - 1e-9, 300.0).absorption_per_m.unwrap();
    let at = record(row("H2SO4"), 300.0, Composition::Reference)
        .absorption_per_m
        .unwrap();
    for (a, b) in below.iter().zip(&at) {
        assert_close(*a, *b, 1e-6, "at 75 wt%");
    }
    let (inside, outside) = (acid_at(0.845, 300.0), acid_at(0.845 + 1e-8, 300.0));
    let (m75, m845) = per_kg(6);
    let step = outside.absorption_per_m.unwrap()[6] / inside.absorption_per_m.unwrap()[6];
    assert_close(step, m75 / m845, 1e-6, "the step at 84.5 wt%");
}

/// The acid's viscosity is Williams and Long 1995's as the research evaluates it (acid.json's
/// `grid_mPa_s`, to its 10⁻³ mPa s): 11.437 mPa s at 75 wt% and 300 K, 380.25 at its 233.67 K first
/// melt and 1,143.45 at 80 wt% there. Outside the stated 30–80 wt% the mass fraction is held at the
/// nearer end, so 84.5 and 85 wt% are 80 wt%'s and 25 wt% is 30 wt%'s, to the bit; it is
/// continuous at 80 wt%, rises with the acid's fraction below it, and rises as the acid cools.
#[test]
fn the_acid_s_viscosity_holds_its_mass_fraction_in_williams_and_long_s_range() {
    let model = acid();
    let mu = |w: f64, t: f64| model.viscosity.viscosity_pa_s(w, Kelvin::new(t));
    for (w, t, research) in [
        (0.75, 300.0, 11.437),
        (0.75, 233.67, 380.247),
        (0.80, 233.67, 1_143.451),
        (0.70, 273.15, 19.402),
    ] {
        assert_close(mu(w, t) * 1e3, research, 1e-4, &format!("{w} at {t} K"));
    }
    for t in [233.67, 273.15, 300.0, 500.0] {
        for w in [0.845, 0.85] {
            assert_same_bits(acid_at(w, t).viscosity_pa_s.unwrap(), mu(0.9, t));
        }
        assert_close(mu(0.9, t), mu(0.80, t), 1e-15, "held at 80 wt%");
        assert_same_bits(acid_at(0.25, t).viscosity_pa_s.unwrap(), mu(0.2, t));
        assert_close(mu(0.2, t), mu(0.30, t), 1e-15, "held at 30 wt%");
        assert_close(
            mu(0.80 - 1e-9, t),
            mu(0.80, t),
            1e-6,
            "continuous at 80 wt%",
        );
        assert!(mu(0.70, t) < mu(0.75, t) && mu(0.75, t) < mu(0.80, t));
    }
    let mut previous = mu(0.75, 300.0);
    for t in [280.0, 260.0, 240.0, 233.67] {
        assert!(mu(0.75, t) > previous, "{t} K");
        previous = mu(0.75, t);
    }
}

/// The acid's surface tension is Vehkamäki et al. 2002's Eq. 9 as the research evaluates it
/// (acid.json's grid, to its 0.01 mN m⁻¹): 71.30 mN m⁻¹ at 80 wt% and 273 K, within 0.2 of Myhre
/// et al. 1998's own 71.11 there, the ruled source it stands in for; 72.48 at 75 wt% and 300 K and
/// 70.23 at 84.5 wt% and 233.67 K; and it is present across the whole range.
#[test]
fn the_acid_s_surface_tension_is_vehkamaki_s() {
    let sigma = |w: f64, t: f64| {
        acid()
            .surface_tension
            .surface_tension_n_per_m(w, Kelvin::new(t))
            * 1e3
    };
    assert!((sigma(0.80, 273.0) - 71.30).abs() < 0.005);
    assert!((sigma(0.80, 273.0) - 71.11).abs() < 0.2);
    for (w, t, research) in [
        (0.75, 300.0, 72.48),
        (0.845, 233.67, 70.23),
        (0.70, 250.0, 75.87),
    ] {
        assert!(
            (sigma(w, t) - research).abs() < 0.005,
            "{w} at {t} K: {}",
            sigma(w, t)
        );
    }
    for w in [0.25, 0.5, 0.75, 0.85] {
        for t in [233.67, 300.0, 450.0, 647.0] {
            let acid = acid_at(w, t);
            assert_same_bits(acid.surface_tension_n_per_m.unwrap(), sigma(w, t) / 1e3);
            assert!(acid.surface_tension_n_per_m.unwrap() > 0.0, "{w} at {t} K");
        }
    }
}

/// decision-p14-t49h-step3-composition §5's refusals for the acid, in §2.3's order: a component
/// the model does not carry first (`{H2O, CH4}`); then a fraction outside 25–85 wt%, naming the
/// first component given out of range with the range converted to its mole fractions (the acid's
/// alone and water's alone among them), before a temperature outside the range; then a
/// temperature outside the first melt, 233.67 K, to below water's 647.096 K, H2SO4 having no
/// phase diagram (§3). At 25 and 85 wt% the acid resolves.
#[test]
fn the_acid_refuses_what_its_model_does_not_evaluate() {
    let id = row("H2SO4");
    let key = id.substance().key();
    let given = |fractions: &[(SubstanceId, f64)], t: f64| {
        liquid_properties(
            id,
            Kelvin::new(t),
            Composition::Given(LiquidComposition::new(fractions).unwrap()),
        )
    };
    for fractions in [
        [(row("H2O"), 0.6), (row("CH4"), 0.4)],
        [(row("CH4"), 0.4), (id, 0.6)],
    ] {
        assert_eq!(
            given(&fractions, 300.0),
            Err(ResolveLiquidError::ComponentNotModelled {
                key,
                component: row("CH4").substance().key(),
            })
        );
    }
    let [_, (_, x_low)] = binary("H2O", "H2SO4", 0.25);
    let [_, (_, x_high)] = binary("H2O", "H2SO4", 0.85);
    for w in [0.25, 0.85] {
        assert!(given(&binary("H2O", "H2SO4", w), 300.0).is_ok(), "{w}");
    }
    for (w, t) in [(0.86, 300.0), (0.24, 300.0), (0.90, 700.0), (0.20, 200.0)] {
        let [water, acid] = binary("H2O", "H2SO4", w);
        for (fractions, first) in [([acid, water], "H2SO4"), ([water, acid], "H2O")] {
            match given(&fractions, t) {
                Err(ResolveLiquidError::CompositionOutOfRange {
                    key: refused,
                    component,
                    mole_fraction,
                    lowest,
                    highest,
                }) => {
                    assert_eq!(refused, key);
                    assert_eq!(component, row(first).substance().key(), "{w}");
                    let (expected, low, high) = if first == "H2SO4" {
                        (acid.1, x_low, x_high)
                    } else {
                        (water.1, 1.0 - x_high, 1.0 - x_low)
                    };
                    assert_same_bits(mole_fraction, expected);
                    assert!((lowest - low).abs() < 1e-15, "{lowest}");
                    assert!((highest - high).abs() < 1e-15, "{highest}");
                }
                other => panic!("{w} at {t} K: {other:?}"),
            }
        }
    }
    for alone in ["H2O", "H2SO4"] {
        assert!(matches!(
            given(&[(row(alone), 1.0)], 300.0),
            Err(ResolveLiquidError::CompositionOutOfRange { .. })
        ));
    }
    let eighty = binary("H2O", "H2SO4", 0.80);
    let eighty = Composition::Given(LiquidComposition::new(&eighty).unwrap());
    for composition in [Composition::Reference, eighty] {
        assert!(liquid_properties(id, Kelvin::new(233.67), composition).is_ok());
        assert!(liquid_properties(id, Kelvin::new(647.096 - 1e-6), composition).is_ok());
        for t in [233.66, 647.096, 700.0, f64::NAN] {
            match liquid_properties(id, Kelvin::new(t), composition) {
                Err(ResolveLiquidError::OutOfRange {
                    key: refused,
                    lowest,
                    below: Some(below),
                    ..
                }) => {
                    assert_eq!(refused, key);
                    assert_same_bits(lowest.value(), 233.67);
                    assert_same_bits(below.value(), 647.096);
                }
                other => panic!("{t} K: {other:?}"),
            }
        }
    }
}

/// The acid's given composition's order moves nothing (decision-p14-t49h-step3-composition §5,
/// item 3): its mass fraction sums water's term first, in registry order.
#[test]
fn the_acid_s_composition_order_does_not_move_the_record() {
    let [water, acid] = binary("H2O", "H2SO4", 0.8);
    for t in [240.0, 300.0, 500.0] {
        assert_same_record(
            &record(
                row("H2SO4"),
                t,
                Composition::Given(LiquidComposition::new(&[water, acid]).unwrap()),
            ),
            &record(
                row("H2SO4"),
                t,
                Composition::Given(LiquidComposition::new(&[acid, water]).unwrap()),
            ),
        );
    }
}
