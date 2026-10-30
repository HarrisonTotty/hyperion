//! The state from the present climate (plan 14, P14.T53.e; decision-p14-t54a-states §3.4): rules
//! 5–7 of the published state, judged on a pass's published figures, behind their hold at
//! generator version 25 ([`PRESENT_STATE_VERSION`]).
//!
//! Rules 1–4 stand as built, the first match winning: [`SurfaceState::GasEnvelope`], P14.T53.b
//! part 3's [`SurfaceState::SteamEnvelope`] (a label, [`Atmosphere::published_state`]),
//! [`SurfaceState::MagmaOcean`], and [`SurfaceState::Airless`] or P14.T53.c's
//! [`SurfaceState::Tenuous`] under [`AIRLESS_PRESSURE`](super::AIRLESS_PRESSURE). Where none
//! matches, the rest ([`present_climate_state`]):
//!
//! 5. **`Snowball`**: the published ices of every substance cover at least
//!    [`SNOWBALL_ICE_COVER`] of the surface (their area shares summed and held within the surface,
//!    as the record's figures sum them, [`published_ice_cover`]), an ocean under ice counted as its
//!    ice. Were that ice two polar caps, their edges would lie past about 30° latitude, the
//!    large-ice-cap instability of diffusive energy-balance models (North 1975, J. Atmos. Sci. 32,
//!    1301; North, Cahalan and Coakley 1981).
//! 6. **`RunawayGreenhouse`**: no surface liquid water, and the mean surface temperature at or above
//!    water's saturation temperature at the surface pressure ([`water_saturation_temperature`]:
//!    T49.b's curve, water's vapour-pressure equation, 647.096 K at and above 22.064 MPa, not
//!    judged below the triple point): no ocean could stand at the present climate.
//! 7. **`Temperate`**: the rest, hot seas below the boiling curve and cold or warm deserts such as
//!    Mars included.
//!
//! **Inside the passes.** Every branch of a pass keyed on the state (the cloud share and its
//! albedo, Venus's deck, the cover's threshold, the ocean under the ice, the regolith's and the
//! lid's states, and after the passes the phase template) reads the state its previous pass's
//! figures give by these rules, carried in the context the pass reads
//! ([`PassSurface::present`](super::PassSurface::present)) where it differs from the body's class,
//! the state its climate's division publishes (P14.T53.b part 1 (a) and (c)); the first pass reads
//! the class. A body whose figures agree with its class carries nothing, so its passes are version
//! 24's to the bit; a state flip is one of the discrete jumps the passes' bracket handles
//! (P14.T51.h), counted by decision-pre-23-review item 32's procedure. A runaway under Venus's deck
//! is not judged again ([`state_to_carry`]): rule 6 reads a runaway's climate without the deck, as
//! P14.T53.b part 1 (c) does, and part 1 (b)'s deckless re-solve, which follows below the acid's
//! cycling temperature, is judged.
//!
//! The insolation class keeps driving the inventory: the division (carbon stored in rock in the
//! habitable zone, water lost inside the runaway limit at the peak, carbon airborne beyond the
//! limit) and the state the water follows ([`Atmosphere::water_state`]) are the division's, so
//! that each rule is a comparison on figures a pass already publishes and a pass costs what it did.
//! P14.T53.b part 1 (b)'s deckless re-solve and (c)'s hothouse re-solve stand after the passes, as
//! built; (c)'s, whose state is fixed within it, reads and carries no state.

use super::{Atmosphere, Gas, RunawayDeck, SurfaceState, water_saturation_temperature};
use crate::planetary::chem::partition::{CondensatePhase, Condensates};
use crate::planetary::hooks::figures::published_ice_cover_of;
use crate::units::{Kelvin, Pascals};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which P14.T53.e's rules 5–7 publish the state: 25, the first
/// planetary bump after 24's (decision-bump-plan-after-22, "Rulings before T53.a lands", R1.5).
///
/// It is T53.e's one hold. Below it [`PresentStateRules::in_force`] is
/// [`PresentStateRules::Held`]: no pass carries a present state, and every figure on the record is
/// version 24's bit for bit. 25's bump deletes this hold, [`PresentStateRules`] with its `Held`
/// arm, the tests' switch and its tripwire (`the_present_state_is_held_only_until_the_25_bump`).
pub(crate) const PRESENT_STATE_VERSION: GeneratorVersion = GeneratorVersion::new(25);

/// Whether P14.T53.e's rules 5–7 are in force (its hold, [`PRESENT_STATE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PresentStateRules {
    /// Version 24's and before: the state is the insolation class's, its runaway arm the present
    /// insolation's (P14.T53.b part 1 (a)), and no pass reads another's state.
    Held,
    /// Version 25's: rules 5–7 on the previous pass's figures.
    Present,
}

impl PresentStateRules {
    /// The rules this generator version gives: rules 5–7 from [`PRESENT_STATE_VERSION`], none
    /// below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_present_state`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= PRESENT_STATE_VERSION {
            Self::Present
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The present state's rules a test sets on its thread, `None` for the version's.
    static RULES: core::cell::Cell<Option<PresentStateRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the present state's rules `rules` on this thread, for the tests to reach
/// rules 5–7 ahead of the bump; the version's rules are set back after, a panic in `run` included.
/// 25's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_present_state<R>(rules: PresentStateRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<PresentStateRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// Whether P14.T53.e's rules 5–7 are in force ([`PresentStateRules::Present`]); never under
/// their hold.
#[must_use]
pub(crate) fn present_state_in_force() -> bool {
    PresentStateRules::in_force() == PresentStateRules::Present
}

/// The share of the surface its published ices must cover for rule 5 to publish
/// [`SurfaceState::Snowball`]: 0.5, the share poleward of 30° latitude in both hemispheres
/// (1 − sin 30°), were the ice two polar caps.
///
/// In diffusive energy-balance models the ice-albedo feedback makes an ice edge that has advanced
/// equatorward of about 30° latitude unstable, so that the climate falls to global glaciation: the
/// large-ice-cap instability (North 1975, J. Atmos. Sci. 32, 1301; North, Cahalan and Coakley
/// 1981, Rev. Geophys. Space Phys. 19, 91). The critical latitude depends on the model's heat
/// transport and its outgoing-radiation slope (it moves poleward as transport strengthens or the
/// slope falls), so half the surface is the rule's round figure, not a fit.
pub(crate) const SNOWBALL_ICE_COVER: f64 = 0.5;

/// A state rules 5–7 publish (P14.T53.e): the three their tests give, so that a carried state is
/// never one of rules 1–4's.
///
/// Its declaration order is output from 25: the passes' contexts rank by it
/// ([`Context::rank`](crate::planetary::derive::passes::Context::rank)), which breaks a tie between
/// two passes at the frozen context and at a jump, so reordering or inserting a variant is a
/// generator-version change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PresentClimate {
    /// Rule 5: ice on at least [`SNOWBALL_ICE_COVER`] of the surface.
    Snowball,
    /// Rule 6: no surface liquid water, at or above water's saturation temperature.
    RunawayGreenhouse,
    /// Rule 7: the rest.
    Temperate,
}

impl PresentClimate {
    /// The surface state it publishes.
    #[must_use]
    pub(crate) const fn state(self) -> SurfaceState {
        match self {
            Self::Snowball => SurfaceState::Snowball,
            Self::RunawayGreenhouse => SurfaceState::RunawayGreenhouse,
            Self::Temperate => SurfaceState::Temperate,
        }
    }
}

/// What rules 5–7 read of a pass ([`present_climate_state`]): its published figures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PresentFigures {
    /// The share of the surface its published ices of every substance cover, 0 to 1
    /// ([`published_ice_cover`]).
    pub(crate) ice_cover: f64,
    /// Whether its published condensates list liquid water at the surface.
    pub(crate) liquid_water: bool,
    /// Its mean surface temperature.
    pub(crate) surface_temperature: Kelvin,
    /// Its surface pressure, `None` for a gas envelope.
    pub(crate) surface_pressure: Option<Pascals>,
}

/// The state a pass's published figures `figures` give by rules 5–7 (the [module](self)'s): a
/// `Snowball` under ice on at least [`SNOWBALL_ICE_COVER`] of its surface, else a
/// `RunawayGreenhouse` with no liquid water at or above water's saturation temperature at its
/// surface pressure, else `Temperate`.
///
/// An ice cover or a temperature that is not a number fails its rule's test, so that the state
/// falls through to the next; a surface pressure below water's triple point, or none, is not
/// judged by rule 6 (such a world is a desert, as Mars is).
#[must_use]
pub(crate) fn present_climate_state(figures: &PresentFigures) -> PresentClimate {
    if figures.ice_cover >= SNOWBALL_ICE_COVER {
        PresentClimate::Snowball
    } else if !figures.liquid_water
        && figures
            .surface_pressure
            .and_then(water_saturation_temperature)
            .is_some_and(|saturation| figures.surface_temperature >= saturation)
    {
        PresentClimate::RunawayGreenhouse
    } else {
        PresentClimate::Temperate
    }
}

/// The share of the surface the published condensates `condensates` cover with ice of every
/// substance, 0 to 1: their solids' area shares with water's ice `water_ice` where they list none
/// of water's, held within the surface and summed, as the record's figures sum their ices
/// ([`GlobalFigures::ice_fraction`](crate::planetary::hooks::figures::GlobalFigures::ice_fraction),
/// the same bits on the same inputs).
#[must_use]
pub(crate) fn published_ice_cover(condensates: &Condensates, water_ice: f64) -> f64 {
    let water = Gas::Water.substance();
    let water_listed = condensates.entry(water, CondensatePhase::Solid).is_some();
    published_ice_cover_of(
        condensates.surface_ices(),
        (!water_listed).then_some(water_ice),
    )
}

/// The state the pass whose atmosphere is `air`, of the climate class `class`, under the runaway's
/// cloud `deck`, carries to the next pass (P14.T53.e).
///
/// It is rules 5–7's on its published figures, which `figures` gives, where they differ from its
/// class; `None` where they agree, where rules 1–4 decided its state and under the hold (where
/// `figures` is not called), the next pass then reading its class.
///
/// A runaway under Venus's deck is judged by rule 6's first test alone, liquid water at its
/// surface. Rule 6's temperature test reads a runaway's climate as it would be without the deck,
/// P14.T53.b part 1 (c)'s convention (decision-t53b-arm-c §2, decision-p14-t54b-k-hothouses §5),
/// which its passes do not solve, and under the deck a thick carbon-dioxide air can collapse to its
/// frost, a state no present-climate rule may publish (plan 14's record of P14.T54.b.k-2's middle
/// solves). So a dry decked runaway keeps its state, and part 1 (b)'s deckless re-solve, whose
/// passes are judged in full, follows below the acid's cycling temperature; where a sea stands
/// under its deck no runaway does, and rules 5–7 judge it.
///
/// Carrying only a difference keeps every body whose figures agree with its class on version 24's
/// passes to the bit, their count included: its contexts never change.
#[must_use]
pub(crate) fn state_to_carry(
    air: &Atmosphere,
    (class, deck): (SurfaceState, RunawayDeck),
    figures: impl FnOnce() -> PresentFigures,
) -> Option<PresentClimate> {
    if !present_state_in_force() {
        return None;
    }
    let state = match (air.state(), deck) {
        (SurfaceState::RunawayGreenhouse, RunawayDeck::Kept) => {
            let figures = figures();
            if figures.liquid_water {
                present_climate_state(&figures)
            } else {
                PresentClimate::RunawayGreenhouse
            }
        }
        (
            SurfaceState::RunawayGreenhouse | SurfaceState::Temperate | SurfaceState::Snowball,
            RunawayDeck::Kept | RunawayDeck::Deckless,
        ) => present_climate_state(&figures()),
        (
            SurfaceState::GasEnvelope
            | SurfaceState::MagmaOcean
            | SurfaceState::Airless
            | SurfaceState::Tenuous
            | SurfaceState::SteamEnvelope,
            RunawayDeck::Kept | RunawayDeck::Deckless,
        ) => return None,
    };
    (state.state() != class).then_some(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::substance::SubstanceId;

    /// The state rules 5–7 give the figures `figures`.
    fn state_of(figures: &PresentFigures) -> SurfaceState {
        present_climate_state(figures).state()
    }

    /// Figures of a body with `ice_cover` under ice, liquid water where `liquid_water`, at `t_s`
    /// K under `p_s` Pa.
    fn figures(ice_cover: f64, liquid_water: bool, t_s: f64, p_s: f64) -> PresentFigures {
        PresentFigures {
            ice_cover,
            liquid_water,
            surface_temperature: Kelvin::new(t_s),
            surface_pressure: Some(Pascals::new(p_s)),
        }
    }

    /// P14.T53.e is held until the 24 → 25 bump: the rules in force are version 24's, the
    /// insolation class's state, and no pass carries a present state.
    #[test]
    fn the_present_state_is_held_only_until_the_25_bump() {
        assert!(
            GENERATOR_VERSION < PRESENT_STATE_VERSION,
            "the 24 → 25 bump puts P14.T53.e's rules 5–7 on the record \
             (decision-bump-plan-after-22 R1.5): delete `PRESENT_STATE_VERSION`, \
             `PresentStateRules` with its `Held` arm, `present_state_in_force` and the arms that \
             read it (`state_to_carry`, the stand-in's `passes::solve` arm in \
             `PassBody::stand_in_passes` and `escaping::present_state`'s hothouse guard), the \
             tests' `with_present_state`, its switch in `with_every_25_hold` and this test"
        );
        assert_eq!(PresentStateRules::in_force(), PresentStateRules::Held);
        assert!(!present_state_in_force());
        assert!(with_present_state(
            PresentStateRules::Present,
            present_state_in_force
        ));
        assert!(!present_state_in_force());
    }

    /// Rule 5: ice on at least half the surface is a `Snowball`, whatever else holds; just under
    /// half is not.
    #[test]
    fn rule_five_takes_half_the_surface_under_ice() {
        assert_eq!(
            state_of(&figures(SNOWBALL_ICE_COVER, true, 270.0, 1e5)),
            SurfaceState::Snowball
        );
        assert_eq!(
            state_of(&figures(1.0, false, 94.0, 1.467e5)),
            SurfaceState::Snowball,
            "Titan's"
        );
        let under = SNOWBALL_ICE_COVER - 1e-12;
        assert_eq!(
            state_of(&figures(under, true, 260.0, 1e5)),
            SurfaceState::Temperate
        );
        // A dry world under 1 bar above its boiling point with ice on half its surface is still
        // a `Snowball`: rule 5 comes first.
        assert_eq!(
            state_of(&figures(0.5, false, 400.0, 1e5)),
            SurfaceState::Snowball
        );
    }

    /// Rule 6 reads water's saturation temperature at the surface pressure: at one standard
    /// atmosphere the normal boiling point, 373.124 K (IAPWS-95, Wagner and Pruß 2002, J. Phys.
    /// Chem. Ref. Data 31, 387; water's vapour-pressure equation, IAPWS SR1-86(1992), agrees within
    /// 1 µK), and above 22.064 MPa the critical temperature, 647.096 K; liquid water at the surface
    /// keeps a world off it.
    #[test]
    fn rule_six_reads_water_s_saturation_curve() {
        let at = |t_s: f64, p_s: f64, liquid: bool| state_of(&figures(0.0, liquid, t_s, p_s));
        let boiling = water_saturation_temperature(Pascals::new(101_325.0))
            .expect("1 atm is above water's triple point")
            .value();
        assert!((boiling - 373.12).abs() < 0.05, "{boiling}");
        assert_eq!(
            at(boiling, 101_325.0, false),
            SurfaceState::RunawayGreenhouse
        );
        assert_eq!(
            at(boiling - 0.01, 101_325.0, false),
            SurfaceState::Temperate
        );
        assert_eq!(at(boiling + 50.0, 101_325.0, true), SurfaceState::Temperate);
        // Venus: 737 K under 92 bar, no water.
        assert_eq!(at(737.0, 9.2e6, false), SurfaceState::RunawayGreenhouse);
        // Above the critical pressure the curve ends at the critical temperature.
        assert_eq!(at(647.096, 3e7, false), SurfaceState::RunawayGreenhouse);
        assert_eq!(at(647.0, 3e7, false), SurfaceState::Temperate);
    }

    /// Rule 7 takes the rest: a hot sea below the boiling curve (the hot `Snowball`s P14.T54.b.l's
    /// bless left, liquid water at 375–520 K under 300–750 bar), a desert below water's triple
    /// point at any temperature (Mars), and a temperature that is not a number.
    #[test]
    fn rule_seven_takes_the_hot_seas_and_the_deserts() {
        assert_eq!(
            state_of(&figures(0.0, true, 519.6, 4.769e7)),
            SurfaceState::Temperate
        );
        assert_eq!(
            state_of(&figures(0.007, false, 210.0, 600.0)),
            SurfaceState::Temperate,
            "Mars's 6 mbar lies under water's triple point"
        );
        assert_eq!(
            state_of(&figures(0.0, false, 900.0, 600.0)),
            SurfaceState::Temperate,
            "a hot thin desert is not judged by rule 6"
        );
        assert_eq!(
            state_of(&figures(0.0, false, f64::NAN, 1e7)),
            SurfaceState::Temperate
        );
        assert_eq!(
            state_of(&figures(f64::NAN, true, 288.0, 1e5)),
            SurfaceState::Temperate
        );
        assert_eq!(
            state_of(&PresentFigures {
                surface_pressure: None,
                ..figures(0.0, false, 900.0, 1e5)
            }),
            SurfaceState::Temperate
        );
    }

    /// The ice cover is the record's sum: the solids' shares in order, water's ice added where
    /// none of water's is listed, scaled within the surface and held at 1.
    #[test]
    fn the_ice_cover_is_summed_within_the_surface() {
        let co2 = Gas::CarbonDioxide.substance();
        let water = Gas::Water.substance();
        let ices = |list: &[(SubstanceId, f64)]| list.to_vec();
        assert!(
            (published_ice_cover_of(ices(&[(co2, 0.2), (water, 0.25)]), None) - 0.45).abs() < 1e-15
        );
        assert!(
            (published_ice_cover_of(ices(&[(co2, 0.2)]), Some(0.3)) - 0.5).abs() < 1e-15,
            "water's ice joins where none is listed"
        );
        assert_eq!(
            published_ice_cover_of(ices(&[(co2, 0.8), (water, 0.7)]), None),
            1.0,
            "overlapping ices cover the surface once"
        );
        assert_eq!(published_ice_cover_of(Vec::new(), None), 0.0);
    }
}
