//! Venus's deck only where the air reaches its base and is oxidised (plan 14, P14.T53.b part 1
//! (b)'s follow-up; decision-absurd-states-24 R3.2), from generator version 35.
//!
//! A published [`SurfaceState::RunawayGreenhouse`] is solved under Venus's sulphuric-acid deck,
//! cloud cover 1 at Venus's Bond albedo 0.76, and P14.T24.g's cycling sulphur. Arm (b) solves it
//! once more without the deck where the climate under the deck is too cold to cycle the acid, or
//! holds liquid water ([`solved_without_the_deck`](super::solved_without_the_deck)). From 35 the
//! deck also needs the air it hangs in ([`drops_the_deck`]):
//!
//! - **The base.** Venus's deck stands only where the surface pressure reaches its base's,
//!   [`DECK_BASE_PRESSURE`]. A thinner air ends inside or above the layer where Venus's deck
//!   lies, so the deck whose albedo the table gives cannot hang in it.
//! - **A reducing air.** An air that is not oxidising ([`reducing`]: hydrogen its largest gas by
//!   number, or no carbon dioxide, water or oxygen) takes no acid deck: its sulphur stays reduced,
//!   and no sulphuric acid forms.
//!
//! Such a body takes arm (b)'s deckless solve as it stands: its state unchanged, its cloud the
//! water cloud's availability, P14.T24.g's once-through case and its thin layer (in a reducing
//! air the gas would be H₂S and the layer S₈, not SO₂ and sulphate: recorded, not modelled). Most
//! such bodies are at or above the acid's cycling temperature without the deck, where a deck of
//! their own would form (a partial deck, not modelled, as arm (b) records). The test reads
//! the first solve alone, as arm (b)'s does: nothing returns to the decked branch, and the
//! re-solve runs at most once.

use super::{Atmosphere, PartialPressures, SurfaceState};
use crate::planetary::hooks::modifiers::oxidising;
use crate::units::Pascals;

/// The pressure at the base of Venus's sulphuric-acid deck, the least surface pressure under which
/// a runaway keeps it, Pa: 1.375 × 10⁵, the Venus International Reference Atmosphere's at 48 km,
/// where it gives 366.4 K (Seiff et al. 1985, Adv. Space Res. 5(11), 3, the low-latitude model,
/// read through a Venus-GRAM mean profile, the Aerocapture Mission Analysis Tool's
/// `venus-gram-avg.dat`, whose surface row is VIRA's 735.3 K and 92.09 bar; P14.T24.e's adiabat
/// reads its 1.066 bar at 350.5 K at 50 km through Venus-GRAM too). Pioneer Venus's particle-size
/// spectrometer put the lower cloud's base at 47.5 km (Knollenberg and Hunten 1980, JGR 85, 8039,
/// from memory), about 1.46 bar on VIRA; Venus's 1 bar lies near 50.5 km, the lower cloud's top,
/// and the deck's top, near 70 km, at 3,690 Pa and 229.8 K (the same profile).
///
/// The test is the template's, not the acid's: the area mix's deck is Venus's whole, cover 1 at
/// 0.76, which stands only in an air that reaches its base. A thinner air could hold a thinner
/// acid deck of its own, its base where the acid evaporates on its own profile and its τ set by
/// the column above it (Δp ÷ g, so that a column test would read gravity too): recorded, not
/// modelled.
pub(crate) const DECK_BASE_PRESSURE: Pascals = Pascals::new(1.375e5);

/// Whether the air of partial pressures `partials` is reducing for its sulphur: it is not
/// oxidising by P14.T51.j's test ([`oxidising`]: it holds no carbon dioxide, water or oxygen, or
/// hydrogen is its largest gas by number, each gas's moles its column weight over its molar mass),
/// one test of an oxidising air for the whole model.
///
/// Such an air makes no sulphuric acid. Its hydrogen takes the oxygen atoms that would make SO₃
/// (O + H₂ → OH + H), its sulphur stays H₂S, and the aerosol it makes is elemental sulphur, S₈
/// (Hu, Seager and Bains 2013, ApJ 769, 6, their H₂-dominated case, from memory); with no water,
/// no acid forms at all. However much water lies beside the hydrogen, its sulphur is sulphide: at
/// equal H₂ and H₂O the gas lies within about a dex of the iron–wüstite buffer at 700–1,000 K,
/// where H₂S outnumbers SO₂ by 10⁵ or more (the science check's estimate on NASA's thermodynamic
/// data, 2026-11-19). Methane's reducing power is not weighed, and sulphide is favoured before
/// hydrogen leads water: a coarse test, recorded.
#[must_use]
pub(crate) fn reducing(partials: &PartialPressures) -> bool {
    !oxidising(partials)
}

/// Whether the deck base drops Venus's deck from the published runaway `air` solved under it
/// ([the module](self)): a [`SurfaceState::RunawayGreenhouse`] whose surface pressure is below
/// [`DECK_BASE_PRESSURE`], or whose air is [`reducing`]. An air whose surface pressure is not a
/// number, or none, is not judged.
#[must_use]
pub(crate) fn drops_the_deck(air: &Atmosphere) -> bool {
    if air.state != SurfaceState::RunawayGreenhouse {
        return false;
    }
    air.surface_pressure
        .filter(|p| !p.value().is_nan())
        .is_some_and(|p| p.value() < DECK_BASE_PRESSURE.value() || reducing(&air.partials))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::derive::atmosphere::Gas;

    /// The air of `pressure` by gas, in Pa.
    fn air(pressure: &[(Gas, f64)]) -> PartialPressures {
        PartialPressures::from_fn(|gas| {
            Pascals::new(
                pressure
                    .iter()
                    .find(|(g, _)| *g == gas)
                    .map_or(0.0, |&(_, p)| p),
            )
        })
    }

    /// An air led by hydrogen by number is reducing, however much water lies beside it; one led by
    /// an oxidant is not, however much hydrogen it holds; and one with no oxidant is reducing.
    #[test]
    fn an_air_is_reducing_where_it_is_not_oxidising() {
        // 1 bar of H₂ beside 5 bar of H₂O: 0.496 against 0.278 mol per unit weight.
        assert!(reducing(&air(&[(Gas::Hydrogen, 1e5), (Gas::Water, 5e5)])));
        // H₂ at 1 bar beside 30 bar of CO₂: 0.496 against 0.682.
        assert!(!reducing(&air(&[
            (Gas::Hydrogen, 1e5),
            (Gas::CarbonDioxide, 30e5)
        ])));
        assert!(!reducing(&air(&[(Gas::CarbonDioxide, 92e5)])));
        assert!(reducing(&air(&[(Gas::Nitrogen, 2e5), (Gas::Argon, 1e3)])));
        assert!(reducing(&air(&[])));
    }

    /// The rule on hand-built atmospheres (a runaway under Venus's deck from T13.c's Earth at
    /// 255 K): kept at the base and above in an oxidising air, dropped just below it and in a
    /// reducing air, an air whose pressure is none or not a number not judged, nor any other
    /// state.
    #[test]
    fn the_deck_is_dropped_below_its_base_and_in_a_reducing_air_alone() {
        use crate::planetary::derive::atmosphere::tests::earth_inputs;
        use crate::planetary::derive::atmosphere::{Insolation, atmosphere};
        let runaway = atmosphere(&earth_inputs(255.0, Insolation::InsideRunaway));
        assert_eq!(runaway.state(), SurfaceState::RunawayGreenhouse);
        let own = runaway.partials;
        assert!(!reducing(&own), "{own:?}");
        let at = |p: Option<f64>, partials: PartialPressures| Atmosphere {
            surface_pressure: p.map(Pascals::new),
            partials,
            ..runaway
        };
        let base = DECK_BASE_PRESSURE.value();
        let hydrogen = air(&[(Gas::Hydrogen, 1e5), (Gas::Water, 5e5)]);
        let cases = [
            (at(Some(base), own), false),
            (at(Some(92e5), own), false),
            (at(Some(base.next_down()), own), true),
            (at(Some(1.2e2), own), true),
            (at(Some(92e5), hydrogen), true),
            (at(None, hydrogen), false),
            (at(Some(f64::NAN), hydrogen), false),
        ];
        for (atmosphere, drops) in cases {
            assert_eq!(
                drops_the_deck(&atmosphere),
                drops,
                "{:?}",
                atmosphere.surface_pressure
            );
        }
        for state in [
            SurfaceState::Temperate,
            SurfaceState::Snowball,
            SurfaceState::MagmaOcean,
            SurfaceState::GasEnvelope,
        ] {
            let thin = Atmosphere {
                state,
                ..at(Some(1.2e2), hydrogen)
            };
            assert!(!drops_the_deck(&thin), "{state:?}");
        }
    }

    /// Venus's own air, P14.T57.a's 90.4 bar of CO₂ and 2.09 bar of N₂ by weight, reaches its
    /// deck's base and is oxidising, so that the deck base never takes Venus's deck.
    #[test]
    fn venus_s_air_reaches_its_deck_s_base() {
        let venus = air(&[(Gas::CarbonDioxide, 90.4e5), (Gas::Nitrogen, 2.09e5)]);
        assert!(venus.total() >= DECK_BASE_PRESSURE, "{:?}", venus.total());
        assert!(!reducing(&venus));
    }
}
