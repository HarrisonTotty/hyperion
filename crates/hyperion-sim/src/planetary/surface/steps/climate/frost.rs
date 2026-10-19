//! Where a condensable condenses at the surface (R09.T13.d): the frost point each ice is ranked
//! against ([`frost_point_k`]; decision-r09-t13d-normalisation §1.2, the lane's research-d §2.2),
//! and the freezing point the energy-balance model's albedo and frozen sea turn about
//! ([`freezing_point_k`]; research-d §5, decision-composition (e)).
//!
//! Both read the substance registry by key ([`by_key`]), on the rows' phase diagrams (P14.T49.b's
//! reference equations, which the radiative model's frost floor reads too), so a condensable is
//! any row with a diagram, never a closed list. A row without one has no frost point here: an
//! ice of it is ranked against its palette entry's transition instead (the ice's own
//! documentation, `ice`), and a climate condensable without one takes the flat albedo.

use crate::planetary::chem::partition::{AIRLESS_ICE_LOSS_KG_PER_M2_GYR, sublimation_edge};
use crate::planetary::surface::inputs::{CoarseInputs, CondensatePhase};
use crate::substance::by_key;
use crate::units::Pascals;
use hyperion_surface::substance_key::SubstanceKey;

/// The frost point of `substance` on the body of `inputs`, K: the temperature below which its
/// solid lasts at the surface, a body constant at the mean surface pressure
/// (decision-r09-t13d-normalisation §1.2, research-d §2.2's three cases):
///
/// - under an air that holds the substance at mole fraction x, min(`T_sat`(x `p_s`), `T_t`):
///   below its triple pressure the frost point on the sublimation curve, above it the melting
///   line, which the registry draws at the triple temperature (Mars's carbon dioxide at 605 Pa,
///   147.7 K);
/// - under an air that does not hold it, `T_t`, the melting line (Mars's water ice, 273.16 K);
/// - on an airless body, where its Hertz–Knudsen loss into a vacuum is 144 kg m⁻² Gyr⁻¹
///   (Schorghofer and Williams 2020, PSJ 1, 54, eq. 8 and Table 1, as plan 14's
///   `sublimation_edge` reads it): about 110 K for water.
///
/// `None` for a key the registry lacks, a row with no phase diagram, or, airless, no molar mass.
#[must_use]
pub(super) fn frost_point_k(inputs: &CoarseInputs, substance: SubstanceKey) -> Option<f64> {
    let row = by_key(substance.as_str())?.substance();
    let diagram = row.phase_diagram()?;
    let triple = diagram.triple_temperature();
    let pressure = inputs.surface_pressure().value();
    // A pressure that is not a number reads as airless.
    if pressure.is_nan() || pressure <= 0.0 {
        let mass = row.molar_mass_g_per_mol()?;
        return Some(
            sublimation_edge(
                diagram,
                mass,
                triple,
                Pascals::ZERO,
                AIRLESS_ICE_LOSS_KG_PER_M2_GYR,
            )
            .value(),
        );
    }
    let held = inputs.gases().iter().find(|gas| gas.substance == substance);
    Some(match held {
        Some(gas) => {
            match diagram.saturation_temperature(Pascals::new(gas.mole_fraction * pressure)) {
                Some(t) if t < triple => t.value(),
                // Above the triple pressure the melting line, and above the critical one no
                // vapour curve at all.
                Some(_) | None => triple.value(),
            }
        }
        None => triple.value(),
    })
}

/// The freezing point of the climate's condensable on the body of `inputs`, K, about which the
/// energy-balance model's ice albedo and frozen sea turn (research-d §5): `None` where no ice
/// forms, so that the albedo is flat and the sea keeps its open-water heat capacity.
///
/// - Where the surface pressure is at least the condensable's triple pressure, a liquid can lie
///   under the air and freezes at its melting line, the triple temperature (water's 273.16 K under
///   Earth's air, methane's 90.69 K under a Titan's).
/// - Below it no liquid can lie (Mars's carbon dioxide under 636 Pa): the solid forms from the
///   vapour at its frost point, [`frost_point_k`]'s.
/// - `None` with no condensable, one plan 14's partition lists as supercritical
///   ([`CondensatePhase::Supercritical`]), or one the registry has no phase diagram for.
///
/// The test is on the total surface pressure, not the condensable's partial pressure: a liquid's
/// melting point is set by the pressure on it, and the vapour's share only by its evaporation. On
/// the partial pressure, a generated water world colder than 273 K, whose vapour plan 14 holds at
/// saturation at its mean (`derive::atmosphere`'s greenhouse), would take its own mean as its
/// freezing point, and a snowball would read as open ground (R09's Risks, "Deviations in T13.d,
/// as built").
#[must_use]
pub(super) fn freezing_point_k(inputs: &CoarseInputs) -> Option<f64> {
    let key = inputs.climate().condensable?;
    let supercritical = inputs
        .condensates()
        .iter()
        .any(|c| c.substance == key && c.phase == CondensatePhase::Supercritical);
    if supercritical {
        return None;
    }
    let diagram = by_key(key.as_str())?.substance().phase_diagram()?;
    if inputs.surface_pressure().value() >= diagram.triple_pressure().value() {
        Some(diagram.triple_temperature().value())
    } else {
        frost_point_k(inputs, key)
    }
}

/// Water's freezing point on the registry's diagram, K: its triple temperature, 273.16, from which
/// [`freezing_point_k`]'s shift of Spiegel et al.'s water-calibrated albedo is measured.
///
/// # Panics
///
/// Never: the registry holds water's phase diagram (its rows' tests hold every condensable's).
#[must_use]
pub(super) fn water_freezing_point_k() -> f64 {
    by_key("H2O")
        .and_then(|water| water.substance().phase_diagram())
        .expect("the registry holds water's phase diagram")
        .triple_temperature()
        .value()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::surface::reference;
    use crate::planetary::surface::steps::climate::tests::builder_of;

    fn key(k: &str) -> SubstanceKey {
        SubstanceKey::new(k).unwrap()
    }

    /// The three cases of the frost point: Mars's carbon dioxide on its sublimation curve at its
    /// 605 Pa, about 147.7 K, and its water, not in its air, at the melting line; airless water at
    /// its Hertz–Knudsen edge and carbon dioxide's, 110.0 K and about 58 K on the registry's phase
    /// diagrams.
    #[test]
    fn the_frost_points_follow_their_three_cases() {
        let mars = reference::mars_like();
        let co2 = frost_point_k(&mars, key("CO2")).unwrap();
        let water = frost_point_k(&mars, key("H2O")).unwrap();
        assert!((co2 - 147.7).abs() < 0.2, "{co2}");
        assert!((water - 273.16).abs() < 1e-9, "{water}");
        let moon = reference::moon_like();
        let airless_water = frost_point_k(&moon, key("H2O")).unwrap();
        let airless_co2 = frost_point_k(&moon, key("CO2")).unwrap();
        assert!((airless_water - 110.0).abs() < 1.0, "{airless_water}");
        assert!((airless_co2 - 57.0).abs() < 2.0, "{airless_co2}");
        assert_eq!(frost_point_k(&moon, key("He")), None);
    }

    /// Water's freezing point under Earth's air is its triple temperature, so its shift is zero;
    /// carbon dioxide under 636 Pa freezes at its frost point; a world with no condensable, or a
    /// supercritical one, has none.
    #[test]
    fn the_freezing_point_reads_the_total_pressure() {
        let earth = reference::earth_like();
        hyperion_testkit::float::assert_same_bits(
            freezing_point_k(&earth).unwrap(),
            water_freezing_point_k(),
        );
        let mars = reference::mars_like();
        let co2 = freezing_point_k(&mars).unwrap();
        assert!((co2 - 147.7).abs() < 0.2, "{co2}");
        assert_eq!(freezing_point_k(&reference::moon_like()), None);
        let supercritical = builder_of(&earth)
            .condensates(earth.condensates().iter().map(|c| {
                let mut c = *c;
                if c.phase == CondensatePhase::Liquid {
                    c.phase = CondensatePhase::Supercritical;
                }
                c
            }))
            .build()
            .unwrap();
        assert_eq!(freezing_point_k(&supercritical), None);
    }
}
