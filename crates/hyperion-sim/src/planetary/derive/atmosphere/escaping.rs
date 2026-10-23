//! P14.T53.a's escape on the snapshot path: the atmosphere's steps 1 to 5 under the escape's batch
//! rules ([`EscapeRules::Batch`](crate::planetary::chem::escape::EscapeRules)), which read
//! [`chem::escape`](crate::planetary::chem::escape)'s laws. Under the hold, to generator version
//! 23, none of it is read, and the steps are version 23's to the bit (`held_state_and_greenhouse`);
//! P14.T54.b deletes that arm.
//!
//! The order, as P14.T53.a's wiring gives it:
//!
//! 1. **A gas envelope** (at least `THIN_ENVELOPE_FRACTION` of the mass) is as it was, with the
//!    helium its escape left ([`envelope_helium`]) on its outcome.
//! 2. **Retention**: Jeans's test and the ice-held volatiles, as built.
//! 3. **The cosmic shoreline** ([`Shoreline`]; Zahnle and Catling 2017, ApJ 843, 122): every
//!    airborne inventory gas is multiplied by the share it keeps, at the body's X-ray and
//!    ultraviolet fluence over Earth's, its escape speed √(2GM ÷ R), its mean impact speed and its
//!    age, but what its ice holds: an ice crust's water and nitrogen, and the methane beside the
//!    nitrogen it holds, which are not air.
//! 4. **A thin envelope** (0 < f < `THIN_ENVELOPE_FRACTION`): its hydrogen and helium at the
//!    fractionated helium mass fraction ([`thin_envelope_air`]), kept where Jeans's test retains
//!    them and eroded by the shoreline, enter the air as registry rows 0 and 1.
//! 5. **The climate's division**:
//!    - a runaway world loses its free water by photolysis and hydrogen escape (rule (a)), and
//!      keeps the oxygen of [`runaway_oxygen_timed`] over its exposure, its young melt taking it
//!      until `molten_for` up to the capacity of Earth's mantle per unit mass
//!      ([`melt_oxygen_capacity`], an upper bound), its surface at Earth's sink per unit area
//!      after. The water its ice crust holds stays, and frees no oxygen. A surface its greenhouse
//!      holds molten keeps the oxygen in its melt while it is molten, as before: that melt's own
//!      end needs the host's track and is P14.T54.b's to time;
//!    - a temperate world with water takes the weathering thermostat ([`weathering_thermostat`])
//!      and, where its upper atmosphere is moist, Wordsworth and Pierrehumbert's (2014) oxygen
//!      ([`cold_trap_failure_oxygen`]);
//!    - beyond the maximum greenhouse its carbon is airborne, as before, and a methane carrier's
//!      reservoir loses what escapes through its nitrogen and what its photolysis takes
//!      ([`methane_outcome`]).
//!
//!    The cold trap's oxygen and the methane's loss solve the greenhouse again on the air they
//!    leave, but where they change the airborne inventory by at most [`RESOLVE_SKIP_SHARE`] of its
//!    mass, or where a methane reservoir's air stays at its cap, its methane condensed before and
//!    after: there the state stands, and the air is the changed one. A saturated reservoir's
//!    partition holds each condensable at the least of its inventory and its caps, which read the
//!    climate and the other gases alone, so the state the greenhouse would solve again is the same
//!    but for the start of its iteration, the whole column (P14.T51.a); the census's check solves
//!    it again all the same and states the largest difference.
//! 6. **Gao et al. 2015**: a carbon dioxide air's photolysis under an M dwarf
//!    ([`desiccated_carbon_dioxide`]), on the outcome only: the carbon monoxide has no gas row, so
//!    the air keeps its carbon dioxide until P14.T54.b carries the CO and its O₂ (recorded).
//! 7. **The greenhouse**: P14.T53.a's broadened form, in force under the batch, from the body's
//!    base temperature, the airless mean (`surface_mix::airless_mean`: the larger of `T_eq` and
//!    the night floor under the unlit mean's rules, `T_eq` to the bit under their hold; P14.T24.i's
//!    follow-up, decision-t24i-airless-state D1.3), which the ice-held volatiles of step 2 read
//!    too. Under 100 Pa of its partial pressures the body is airless at that base.
//! 8. **The outcome** ([`EscapeOutcome`]): each element's, mass-conserving (tested).
//! 9. **The state published** (P14.T53.b part 1 (a) and (c), `present_state`): a world inside the
//!    runaway limit at its hosts' largest past luminosity publishes its present class, a runaway
//!    only while it is inside the limit now, while every step above (its water's loss, its oxygen,
//!    its carbon airborne, the thermostat's and the cold trap's gates and the steps' surface
//!    water) reads the division's runaway, which its water follows ([`Atmosphere::water_state`]).
//!    In the re-solve of a body whose passes judged its first solve a hothouse between the limits
//!    ((c): a dry `Temperate` at or above water's saturation temperature at its surface pressure,
//!    [`PublishedState::Hothouse`]), a `Temperate` publishes `RunawayGreenhouse` instead, every
//!    step above reading the same division.

use core::f64::consts::PI;

use crate::math;
use crate::planetary::chem::escape::{
    EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR, ESCAPE_ELEMENTS, ElementOutcome, EnvelopeHelium,
    EscapeOutcome, MeltWindow, MethaneOutcome, MoistAir, OxygenOutcome, Shoreline,
    TITAN_INSOLATION, UltravioletHost, XuvExposure, cold_trap_failure_oxygen,
    desiccated_carbon_dioxide, envelope_helium, methane_outcome, runaway_oxygen_timed,
    thin_envelope_air,
};
use crate::planetary::chem::partition::Ground;
use crate::planetary::params::THIN_ENVELOPE_FRACTION;
use crate::substance::{Element, MOLAR_GAS_CONSTANT, heat_capacity_over_r_at};
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{
    Kelvin, Kilograms, Metres, MetresPerSecond, MetresPerSecondSquared, Pascals,
    WattsPerSquareMetre, Years,
};

use super::{
    AIRLESS_PRESSURE, Airborne, Atmosphere, AtmosphereInputs, CarbonCarrier, Crust,
    EnvelopeHistory, GREENHOUSE_STEPS, Gas, GreenhouseCeiling, IceHeld, Insolation,
    PartialPressures, PassSurface, PublishedState, Retention, RunawayDeck, SILICATE_SOLIDUS,
    StepClimate, SurfaceState, TEMPERATE_AIRBORNE_CARBON, earth_xuv_fluence, exobase_temperature,
    mole_fraction_of, partitioned_greenhouse, present_state_in_force, retained, surface_mix,
};

/// The melting point of ice Ih at one standard atmosphere, K: 273.15, IAPWS R10-06's 273.152 519
/// K to 0.01 K (Feistel and Wagner 2006, J. Phys. Chem. Ref. Data 35, 1021, as the substance
/// registry's ice Ih takes it): the floor of liquid water at a body's warmest zone, which the
/// weathering thermostat reads (decision-bump-plan-after-22, "Rulings before T53.a lands", R3 Q1
/// (1)).
pub const WATER_MELTING_POINT: Kelvin = Kelvin::new(273.15);

/// The warmest zone at which the weathering thermostat sets a frozen temperate world, K: 273.16,
/// 0.01 K above [`WATER_MELTING_POINT`] (the lane's), so that the state it takes holds liquid water
/// there, its interpolation's error (under 10⁻³ K on smooth climates) well inside the margin, and
/// its surface's ice and sea do not flip from one pass to the next.
pub(crate) const THERMOSTAT_SET_POINT: Kelvin = Kelvin::new(273.16);

/// The greenhouse solves the weathering thermostat takes inside its bracket before it interpolates
/// the edge: 8.
///
/// The trials lie on the log of the airborne share of a temperate world's carbon, beyond the stored
/// state's and the all-airborne state's, and the edge is then interpolated and solved; the count
/// is fixed (R3 Q1 (2); the lane's). On smooth hand climates the interpolated edge's warmest zone
/// then lies within 10⁻³ K of the set point (stated by its test), where the 16 halvings the first
/// build took closed the bracket to 1.8 × 10⁻⁴ in ln s, some 4 × 10⁻³ K. Fewer do not hold a sharp
/// climate, its test's 260 K + 30 K tanh 5(ln s + 3): 7 leave it 0.02 K from the set point, and 5
/// trials against √s, which held P14.T53.a's census within 3.4 × 10⁻⁴ K below it at about half the
/// trials' cost, leave it 12 K below, frozen.
pub(crate) const THERMOSTAT_STEPS: u32 = 8;

/// The greenhouse steps each of the weathering thermostat's trials takes: 14 of
/// [`GREENHOUSE_STEPS`]' 32.
///
/// The count is the lane's, for the batch's cost against decision-p14-t54a-passes §C.6. A trial
/// reads its full solve's own first steps, which climb from the base temperature, so its
/// warmest zone lies at or under the full solve's and the edge it places errs toward the warm
/// side, liquid; the state taken is always a full solve. The census's trials took 19–24 steps in
/// full, their thick airs near the melting point settling slowly, against 5–8 for a pass's own
/// solve. Over P14.T53.a's census, its generation and its walk at every pass, each state taken at
/// the edge is liquid, at most 2.1 × 10⁻⁶ K below the set point and 0.070 K above it, against 2.1 ×
/// 10⁻⁶ K and 3.2 × 10⁻³ K in full; at 12 steps 0.114 K above, and at 10 one edge lay 36 K above
/// it, a trial near a runaway read as liquid that its full solve was not. A trial end whose full
/// solve reads hot is not taken ([`weathering_thermostat`]).
pub(crate) const THERMOSTAT_TRIAL_STEPS: u32 = 14;

/// How far from the set point the weathering thermostat's trials read a state's warmest zone, K:
/// 25.
///
/// The span is the lane's. A warmer or colder zone counts as 25 K, so that the jump of a climate
/// that runs away, or one that freezes over, steers its next trial no further than a halving
/// would; the bracket itself reads only which side of the set point a state lies, and the last
/// interpolation the zones' own temperatures.
pub(crate) const THERMOSTAT_INTERPOLATION_SPAN_K: f64 = 25.0;

/// The halvings of the weathering thermostat's bracket where a state reads no warmest zone to
/// interpolate, a boiling state's search for the share whose fuller air holds its water liquid: 16,
/// a fixed count (the lane's), which closes [ln 6 × 10⁻⁶, 0] to 12.02 ÷ 2¹⁶ = 1.8 × 10⁻⁴ in ln s.
pub(crate) const THERMOSTAT_HALVINGS: u32 = 16;

/// The share of the airborne inventory's mass below which the cold trap's oxygen and the methane's
/// loss leave the greenhouse as it was solved: 10⁻⁶ (the lane's, for the batch's cost against
/// decision-p14-t54a-passes §C.6).
///
/// Where the change is at most this share of the air's mass, the state stands and the air (and
/// its column) is the changed one: the state then reads an inventory up to this share off its
/// own, a jump of at most this share at the threshold, below the greenhouse's sensitivity to its
/// surface pressure by orders of magnitude.
pub(crate) const RESOLVE_SKIP_SHARE: f64 = 1e-6;

/// Earth's mantle's `FeO` by mass, 8.05 wt%: McDonough and Sun's (1995, Chem. Geol. 120, 223)
/// pyrolite, against Workman and Hart's (2005, EPSL 231, 53) 8.18 (science-p14-t24f §3, as
/// `runaway_oxygen`'s documentation takes them), the iron a young magma ocean oxidises
/// ([`MeltWindow`]'s capacity).
pub const EARTH_MANTLE_FEO_MASS_FRACTION: f64 = 0.0805;

/// Earth's mantle's share of its mass, 0.672: its 4.01 × 10²⁴ kg (science-p14-t24f §3, as
/// `runaway_oxygen`'s documentation takes it) over Earth's 5.97 × 10²⁴ kg.
pub const EARTH_MANTLE_MASS_FRACTION: f64 = 0.672;

/// The most oxygen, as O₂, a young magma ocean of a body of mass `mass` can take into its iron
/// ([`MeltWindow::capacity`]): [`EARTH_MANTLE_MASS_FRACTION`] of the mass at
/// [`EARTH_MANTLE_FEO_MASS_FRACTION`] of `FeO`, a quarter mole of O₂ per mole of `FeO` oxidised to
/// `FeO₁.₅` (Schaefer et al. 2016, ApJ 829, 63, §4.2), at the element registry's atomic weights:
/// 6.02 × 10⁻³ of the mass, some 3.6 × 10²² kg on an Earth, 29 oceans' worth (science-p14-t24f §3).
///
/// It is an upper bound, the lane's: all the mantle's `FeO` oxidised, where Kress and Carmichael's
/// (1991) equilibrium, which Schaefer et al. follow, oxidises only part of it at the melt's oxygen
/// fugacity (Schaefer et al. 2016, Fig. 9). Every body takes Earth's mantle per unit mass: the
/// passes do not reach a mantle `FeO` of the body's own, which its element inventory does not hold
/// (P14.T50.a's carries none). None for a mass that is not finite and positive.
#[must_use]
pub(crate) fn melt_oxygen_capacity(mass: Kilograms) -> Kilograms {
    let m = mass.value();
    if !(m.is_finite() && m > 0.0) {
        return Kilograms::ZERO;
    }
    let ferrous = Element::Fe.standard_atomic_weight_g_per_mol()
        + Element::O.standard_atomic_weight_g_per_mol();
    let moles = m * EARTH_MANTLE_MASS_FRACTION * EARTH_MANTLE_FEO_MASS_FRACTION / ferrous;
    Kilograms::new(0.25 * moles * Gas::Oxygen.molar_mass_g_per_mol())
}

/// The protosolar helium mass fraction of hydrogen and helium, Y = `m_He` n ÷ (`m_H` + `m_He` n):
/// n the element registry's protosolar He/H, 10^(10.986 − 12) = 0.0968 (Lodders 2010, Table 6, as
/// [`Element::protosolar_abundance_per_hydrogen`] holds it), at its standard atomic weights, 0.278.
/// It is the helium an envelope forms with, from which P14.T53.a's fractionation starts; P14.T54.b
/// passes the host star's own (decision-p14-t50b-envelope §5).
#[must_use]
pub(crate) fn protosolar_helium_mass_fraction() -> f64 {
    let per_hydrogen = Element::He.protosolar_abundance_per_hydrogen();
    let helium = per_hydrogen * Element::He.standard_atomic_weight_g_per_mol();
    helium / (Element::H.standard_atomic_weight_g_per_mol() + helium)
}

/// Whether liquid water stands at a climate's warmest zone `warmest` under the surface pressure
/// `surface_pressure`, on the registry's phase diagram of water (P14.T49.b): the zone at or above
/// [`WATER_MELTING_POINT`] and below water's critical temperature, with the surface pressure at
/// least water's saturation pressure there (decision-bump-plan-after-22, R3 Q1 (1), the water
/// check's rule). A temperature or pressure that is not a number holds none.
#[must_use]
pub(crate) fn holds_liquid_water(warmest: Kelvin, surface_pressure: Pascals) -> bool {
    let Some(water) = Gas::Water.substance().substance().phase_diagram() else {
        return false;
    };
    warmest >= WATER_MELTING_POINT
        && warmest < water.critical_temperature()
        && surface_pressure >= water.saturation_pressure(warmest)
}

/// What water a climate's warmest zone holds, as the weathering thermostat reads it
/// ([`water_phase`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum WaterPhase {
    /// The zone below [`WATER_MELTING_POINT`], or a temperature that is not a number: ice.
    Frozen,
    /// Liquid water ([`holds_liquid_water`]).
    Liquid,
    /// The zone at or above the melting point holding no liquid: at or past water's critical
    /// temperature, or under its saturation pressure there, so that its water boils.
    Hot,
}

/// What water a climate's warmest zone `warmest` holds under the surface pressure
/// `surface_pressure` ([`WaterPhase`]).
#[must_use]
pub(crate) fn water_phase(warmest: Kelvin, surface_pressure: Pascals) -> WaterPhase {
    if warmest.value().is_nan() || warmest < WATER_MELTING_POINT {
        WaterPhase::Frozen
    } else if holds_liquid_water(warmest, surface_pressure) {
        WaterPhase::Liquid
    } else {
        WaterPhase::Hot
    }
}

/// The weathering thermostat (P14.T53.a; decision-bump-plan-after-22, "Rulings before T53.a
/// lands", R3 Q1): the share of a temperate world's carbon left airborne, and the state `solve`
/// gives at it, from `stored_share`, the share its oceans' weathering stores at now
/// ([`TEMPERATE_AIRBORNE_CARBON`], Earth's), where `climate` gives a state's warmest zone and the
/// water it holds there ([`water_phase`]).
///
/// - The stored state stands where it holds liquid water, and then is `solve`'s at the stored
///   share, unchanged: Earth's is its own to the bit (the tripwire test).
/// - Where it is frozen, the oceans freeze over and weathering stops, so the airborne share rises
///   until the warmest zone thaws (R3 Q1 (3), as decision-bump-plan-after-22's "T53.a's thick-air
///   ceiling", K5, applies it). The all-airborne state is solved, and
///   - where its warmest zone is still below [`WATER_MELTING_POINT`], all of it is airborne. That
///     is where the cold trap caps the air: the partition then holds the rest as carbon dioxide
///     ice, which the shoreline has already eroded (step 3); Mars's 3–10 mbar is that end state
///     (Leighton and Murray 1966, from memory, as the ruling cites them);
///   - where it thaws, liquid or hot, a bracketed solve on ln s over [ln `stored_share`, 0] finds
///     the share at which the warmest zone reaches the set point, [`THERMOSTAT_SET_POINT`], at the
///     cold edge of the liquid window. Where the bracket's warm end holds liquid water, the edge is
///     interpolated between the ends' warmest zones and solved, a liquid state at the set point to
///     the interpolation's precision; else its cold end is taken, the largest share still below
///     it: a climate that jumps from ice past the window to steam stops below the jump. So warming
///     stops at the set point, and no world the thermostat moves lies above water's liquid range.
/// - Where it is hot already, its water boiling or past its critical point, the share rises only
///   where a fuller air's pressure brings its water back below its boiling point (the same solve,
///   on whether a state holds liquid, taking the liquid end); else the stored share stands, all
///   of it airborne being for a state too cold alone.
///
/// The edge's solve takes [`THERMOSTAT_STEPS`] trials beyond the two ends, a fixed count, by the
/// Illinois form of the false position (Dowell and Jarratt 1971, BIT 11, 168, from memory), on the
/// warmest zone's distance from the set point held to ± [`THERMOSTAT_INTERPOLATION_SPAN_K`], a
/// halving in its place where a trial would leave the bracket; then one more at the interpolated
/// edge. Each trial's greenhouse takes its first [`THERMOSTAT_TRIAL_STEPS`] steps alone, its full
/// solve's own: a trial only places the bracket, and the state taken is always a full solve, an
/// end that was a trial solved again in full. The boiling state's takes [`THERMOSTAT_HALVINGS`]
/// halvings and its liquid end. A trial end whose full solve reads hot, warmer than its trial, is
/// not taken: the warm end gives way to the cold end, the cold end to the stored state, and the
/// boiling state's liquid end to the all-airborne state, each a full solve already read frozen or
/// liquid, so that no world the thermostat moves lies above water's liquid range by construction
/// (counted on the test threads). Each trial is a fixed function of the ends before it, and the
/// edge's last interpolation is continuous where a trial's side changes (the edge then lies at that
/// trial, an end of every bracket after it), so the share is continuous in the climate wherever the
/// warmest zone is, as the passes' convergence to 10⁻⁹ needs (a share taken at a bracket's end moves
/// by the bracket's width between passes, and kept the census's thermostat worlds at the passes'
/// cap). Where the all-airborne state just thaws the share is all of it, continuous at that switch
/// too; where the stored state just freezes it is the share that warms it to the set point, a step
/// of the set point's 0.01 K margin from the stored share.
///
/// It is the minimal form of the carbonate–silicate cycle (Walker, Hays and Kasting 1981, JGR 86,
/// 9776). Recorded, not built (R3 Q1 (4)): the carbonate a wet epoch made, which a stagnant-lid
/// world keeps and this rule releases (Mars's escape calibration absorbs it); the outgassing–
/// weathering balance with `f_bio` (the life brainstorm's request 4); and low-outgassing limit
/// cycles (Menou 2015, from memory). Its solves beyond the stored state's are counted on the test
/// threads ([`with_thermostat`]).
///
/// It is a function of its own, `solve` (the state at a share in at most a given count of
/// greenhouse steps) and `climate` given, so that it can move.
#[must_use]
pub(crate) fn weathering_thermostat<S: Clone>(
    stored_share: f64,
    solve: impl Fn(f64, u32) -> S,
    climate: impl Fn(&S) -> (Kelvin, WaterPhase),
) -> (f64, S) {
    let stored = solve(stored_share, GREENHOUSE_STEPS);
    #[cfg(test)]
    if THERMOSTAT.with(core::cell::Cell::get) == Thermostat::Off {
        return (stored_share, stored);
    }
    let (stored_warmest, stored_phase) = climate(&stored);
    if stored_phase == WaterPhase::Liquid {
        return (stored_share, stored);
    }
    #[cfg(test)]
    let before = steps_so_far();
    let whole = solve(1.0, GREENHOUSE_STEPS);
    #[cfg(test)]
    count(|counts| {
        match stored_phase {
            WaterPhase::Frozen => counts.frozen += 1,
            WaterPhase::Hot => counts.hot += 1,
            WaterPhase::Liquid => {}
        }
        counts.solves += 1;
        counts.whole += 1;
        counts.whole_steps += steps_so_far() - before;
    });
    let (whole_warmest, whole_phase) = climate(&whole);
    let low = math::ln(stored_share);
    match (stored_phase, whole_phase) {
        (WaterPhase::Frozen | WaterPhase::Hot | WaterPhase::Liquid, WaterPhase::Frozen) => {
            #[cfg(test)]
            count(|counts| counts.too_cold += 1);
            (1.0, whole)
        }
        (WaterPhase::Hot | WaterPhase::Liquid, WaterPhase::Hot) => (stored_share, stored),
        (WaterPhase::Frozen, WaterPhase::Liquid | WaterPhase::Hot) => cold_edge(
            End::new(low, stored_share, stored_warmest.value(), stored),
            End::new(0.0, 1.0, whole_warmest.value(), whole),
            &solve,
            &climate,
        ),
        (WaterPhase::Hot | WaterPhase::Liquid, WaterPhase::Liquid) => {
            // A boiling state's edge, where a fuller air holds its water liquid: halvings alone.
            // Where the liquid end's full solve boils, the all-airborne state, liquid, is taken.
            let all = (1.0, whole.clone());
            let (_, liquid) = bracketed_edge(
                End::new(low, stored_share, f64::NAN, stored),
                End::new(0.0, 1.0, f64::NAN, whole),
                (THERMOSTAT_HALVINGS, &solve),
                |state| (climate(state).1 == WaterPhase::Liquid, f64::NAN),
            );
            taken(liquid, || all, &solve, &climate)
        }
    }
}

/// The weathering thermostat's cold edge ([`weathering_thermostat`]) between the frozen stored
/// state's end `stored` and the thawed all-airborne state's `whole`: the share at which the
/// warmest zone reaches the set point, interpolated and solved where the bracket's warm end holds
/// liquid water, else its cold end, and the state there. A warm end or an interpolated edge whose
/// full solve reads hot gives way to the cold end, and a cold end whose full solve reads hot to
/// the stored state, frozen.
#[must_use]
fn cold_edge<S: Clone>(
    stored: End<S>,
    whole: End<S>,
    solve: &impl Fn(f64, u32) -> S,
    climate: &impl Fn(&S) -> (Kelvin, WaterPhase),
) -> (f64, S) {
    let set = THERMOSTAT_SET_POINT.value();
    let coldest = (stored.share, stored.state.clone());
    let (frozen, thawed) = bracketed_edge(stored, whole, (THERMOSTAT_STEPS, solve), |state| {
        let warmest = climate(state).0.value();
        (warmest >= set, warmest)
    });
    let cold = |frozen: End<S>| taken(frozen, || coldest, solve, climate);
    let (share, state) = match climate(&thawed.state).1 {
        WaterPhase::Liquid => {
            // The edge, interpolated between the ends' warmest zones and solved.
            let edge = frozen.x
                + (set - frozen.warmest_k) * (thawed.x - frozen.x)
                    / (thawed.warmest_k - frozen.warmest_k);
            let x = if edge.is_nan() {
                thawed.x
            } else {
                edge.max(frozen.x).min(thawed.x)
            };
            if x.total_cmp(&thawed.x).is_eq() {
                taken(thawed, || cold(frozen), solve, climate)
            } else {
                let share = math::exp(x);
                #[cfg(test)]
                let before = steps_so_far();
                let state = solve(share, GREENHOUSE_STEPS);
                #[cfg(test)]
                count(|counts| {
                    counts.solves += 1;
                    counts.edges += 1;
                    counts.edge_steps += steps_so_far() - before;
                });
                match climate(&state).1 {
                    WaterPhase::Liquid | WaterPhase::Frozen => (share, state),
                    WaterPhase::Hot => cold(frozen),
                }
            }
        }
        WaterPhase::Hot | WaterPhase::Frozen => {
            #[cfg(test)]
            count(|counts| counts.cold_end += 1);
            cold(frozen)
        }
    };
    #[cfg(test)]
    count(|counts| {
        let (warmest, phase) = climate(&state);
        if phase == WaterPhase::Frozen {
            counts.taken_frozen += 1;
        }
        let error = warmest.value() - set;
        counts.taken_below = counts.taken_below.max(-error);
        counts.taken_above = counts.taken_above.max(error);
    });
    (share, state)
}

/// The share and state of the bracket's end `end` taken as the weathering thermostat's: its own
/// where its solve was a full one, else a trial's, solved again in full at its share by `solve`,
/// unless `climate` reads that full solve hot, warmer than its trial: then `fallback`'s, an end
/// whose full solve was read frozen or liquid.
#[must_use]
fn taken<S>(
    end: End<S>,
    fallback: impl FnOnce() -> (f64, S),
    solve: &impl Fn(f64, u32) -> S,
    climate: &impl Fn(&S) -> (Kelvin, WaterPhase),
) -> (f64, S) {
    if end.full {
        return (end.share, end.state);
    }
    #[cfg(test)]
    let before = steps_so_far();
    let state = solve(end.share, GREENHOUSE_STEPS);
    #[cfg(test)]
    count(|counts| {
        counts.solves += 1;
        counts.edges += 1;
        counts.retaken += 1;
        counts.edge_steps += steps_so_far() - before;
    });
    if climate(&state).1 == WaterPhase::Hot {
        #[cfg(test)]
        count(|counts| counts.retaken_hot += 1);
        return fallback();
    }
    (end.share, state)
}

/// An end of the weathering thermostat's bracket ([`bracketed_edge`]): its ln s, its share, its
/// warmest zone, K (not a number where none is read), its state, and whether that state is a full
/// solve's (else a trial's, [`THERMOSTAT_TRIAL_STEPS`]).
struct End<S> {
    x: f64,
    share: f64,
    warmest_k: f64,
    state: S,
    full: bool,
}

impl<S> End<S> {
    /// The end at ln s `x`, share `share`, warmest zone `warmest_k`, K, and state `state`, a full
    /// solve's.
    #[must_use]
    const fn new(x: f64, share: f64, warmest_k: f64, state: S) -> Self {
        Self {
            x,
            share,
            warmest_k,
            state,
            full: true,
        }
    }
}

/// The weathering thermostat's bracketed solve ([`weathering_thermostat`]) on ln s from `near`,
/// its end on the near side of the edge, to `far`, its end on the far side, in a fixed `steps`
/// trials, each `solve`'s in [`THERMOSTAT_TRIAL_STEPS`] greenhouse steps, where `side` tells
/// whether a state lies on the far side and gives its warmest zone, K. Each trial is the Illinois
/// false position on the zones' distances from the set point ([`THERMOSTAT_SET_POINT`]), held to ±
/// [`THERMOSTAT_INTERPOLATION_SPAN_K`], or a halving where it would leave the bracket or a zone is
/// not a number: the bracket's two ends after them, the near side's first.
#[must_use]
fn bracketed_edge<S>(
    mut near: End<S>,
    mut far: End<S>,
    (steps, solve): (u32, &impl Fn(f64, u32) -> S),
    side: impl Fn(&S) -> (bool, f64),
) -> (End<S>, End<S>) {
    #[cfg(test)]
    count(|counts| counts.searched += 1);
    let score = |warmest_k: f64| {
        (warmest_k - THERMOSTAT_SET_POINT.value()).clamp(
            -THERMOSTAT_INTERPOLATION_SPAN_K,
            THERMOSTAT_INTERPOLATION_SPAN_K,
        )
    };
    let (mut near_score, mut far_score) = (score(near.warmest_k), score(far.warmest_k));
    let mut last_far = None;
    for _ in 0..steps {
        let secant = far.x - far_score * (far.x - near.x) / (far_score - near_score);
        let x = if secant.is_nan() || secant <= near.x || secant >= far.x {
            f64::midpoint(near.x, far.x)
        } else {
            secant
        };
        let share = math::exp(x);
        #[cfg(test)]
        let before = steps_so_far();
        let state = solve(share, THERMOSTAT_TRIAL_STEPS);
        #[cfg(test)]
        count(|counts| {
            counts.solves += 1;
            counts.trials += 1;
            counts.trial_steps += steps_so_far() - before;
        });
        let (beyond, warmest_k) = side(&state);
        let trial = End {
            full: false,
            ..End::new(x, share, warmest_k, state)
        };
        if beyond {
            far = trial;
            far_score = score(warmest_k);
            if last_far == Some(true) {
                near_score *= 0.5;
            }
        } else {
            near = trial;
            near_score = score(warmest_k);
            if last_far == Some(false) {
                far_score *= 0.5;
            }
        }
        last_far = Some(beyond);
    }
    (near, far)
}

/// Whether the weathering thermostat runs, a test-only switch ([`with_thermostat`]): Earth's
/// tripwire derives it with and without.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Thermostat {
    /// As the generator runs it.
    On,
    /// The stored state stands, frozen or not.
    Off,
}

/// What the batch's escape counted on a test thread ([`with_thermostat`]), for its cost against
/// decision-p14-t54a-passes §C.6.
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ThermostatCounts {
    /// The thermostats whose stored state was frozen.
    pub(crate) frozen: u64,
    /// The thermostats whose stored state was hot: its water past its critical point or boiling.
    pub(crate) hot: u64,
    /// The thermostats that searched a bracket.
    pub(crate) searched: u64,
    /// The thermostats whose all-airborne state was still frozen, which took all of it airborne.
    pub(crate) too_cold: u64,
    /// The thermostats whose thaw jumped past the liquid window, which took the bracket's cold end.
    pub(crate) cold_end: u64,
    /// Their greenhouse solves beyond the stored state's.
    pub(crate) solves: u64,
    /// The greenhouse solved again for the oxygen of a failed cold trap.
    pub(crate) cold_trap: u64,
    /// The greenhouse solved again for a methane reservoir's loss.
    pub(crate) methane: u64,
    /// The cold traps' and methane reservoirs' changes at or below [`RESOLVE_SKIP_SHARE`], whose
    /// greenhouse was not solved again.
    pub(crate) skipped: u64,
    /// The methane reservoirs whose air stayed at its cap, whose greenhouse was not solved again.
    pub(crate) saturated: u64,
    /// Of those, under [`with_resolve_check`], the states a solve again moved.
    pub(crate) mismatched: u64,
    /// The envelopes whose helium the batch integrated ([`envelope_helium`]).
    pub(crate) envelopes: u64,
    /// The temperate airs whose cold trap the batch integrated ([`cold_trap_failure_oxygen`]).
    pub(crate) cold_traps: u64,
    /// Of the thermostat's solves, the all-airborne states'.
    pub(crate) whole: u64,
    /// Of them, the bracket's trials.
    pub(crate) trials: u64,
    /// Of them, the states' taken at the edge: the interpolated edge's, or an end that was a trial
    /// solved again in full.
    pub(crate) edges: u64,
    /// Of those, the ends that were a trial.
    pub(crate) retaken: u64,
    /// Of those, the ends whose full solve read hot, which took a colder end instead (the
    /// all-airborne state, for a boiling state's liquid end).
    pub(crate) retaken_hot: u64,
    /// The greenhouse steps (evaluations) of the all-airborne states' solves.
    pub(crate) whole_steps: u64,
    /// The greenhouse steps of the bracket's trials.
    pub(crate) trial_steps: u64,
    /// The greenhouse steps of the states' taken at the edge.
    pub(crate) edge_steps: u64,
    /// The greenhouse steps of the cold traps' and methane reservoirs' solves again.
    pub(crate) resolve_steps: u64,
    /// The states taken at the edge that are frozen at their warmest zone: below the melting
    /// point, or the cold end of a jump.
    pub(crate) taken_frozen: u64,
    /// The largest amount by which a state taken at the edge lies below the set point at its
    /// warmest zone, K.
    pub(crate) taken_below: f64,
    /// The largest amount by which one lies above it, K.
    pub(crate) taken_above: f64,
}

#[cfg(test)]
impl ThermostatCounts {
    /// None counted.
    const ZERO: Self = Self {
        frozen: 0,
        hot: 0,
        searched: 0,
        too_cold: 0,
        cold_end: 0,
        solves: 0,
        cold_trap: 0,
        methane: 0,
        skipped: 0,
        saturated: 0,
        mismatched: 0,
        envelopes: 0,
        cold_traps: 0,
        whole: 0,
        trials: 0,
        edges: 0,
        retaken: 0,
        retaken_hot: 0,
        whole_steps: 0,
        trial_steps: 0,
        edge_steps: 0,
        resolve_steps: 0,
        taken_frozen: 0,
        taken_below: 0.0,
        taken_above: 0.0,
    };

    /// The batch's greenhouse solves beyond each atmosphere's first.
    #[must_use]
    pub(crate) const fn extra_solves(&self) -> u64 {
        self.solves + self.cold_trap + self.methane
    }
}

/// The greenhouse steps this test thread has taken so far ([`exit_counts`](crate::planetary::chem::partition::exit_counts)).
#[cfg(test)]
#[must_use]
fn steps_so_far() -> u64 {
    crate::planetary::chem::partition::exit_counts().evaluations
}

#[cfg(test)]
thread_local! {
    /// The switch on this thread, a test's own.
    static THERMOSTAT: core::cell::Cell<Thermostat> = const { core::cell::Cell::new(Thermostat::On) };
    /// What the batch counted on this thread.
    static THERMOSTAT_COUNTS: core::cell::Cell<ThermostatCounts> = const {
        core::cell::Cell::new(ThermostatCounts::ZERO)
    };
    /// Whether a saturated methane reservoir's loss is solved again all the same, to check it
    /// ([`with_resolve_check`]).
    static RESOLVE_CHECK: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    /// The largest share by which a checked state moved.
    static RESOLVE_DIFFERENCE: core::cell::Cell<f64> = const { core::cell::Cell::new(0.0) };
}

/// `run`'s result with every saturated methane reservoir's loss solved again and compared with the
/// state that stands (P14.T53.a's cost lever), and the largest share by which a temperature or a
/// partial pressure moved; the switch before is set back after, a panic in `run` included, as
/// [`with_rules`](crate::planetary::chem::escape::with_rules) sets its rules back.
#[cfg(test)]
pub(crate) fn with_resolve_check<R>(run: impl FnOnce() -> R) -> (R, f64) {
    /// Sets the thread's check back as it drops.
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            RESOLVE_CHECK.with(|cell| cell.set(self.0));
        }
    }
    let restore = Restore(RESOLVE_CHECK.with(|cell| cell.replace(true)));
    RESOLVE_DIFFERENCE.with(|cell| cell.set(0.0));
    let result = run();
    drop(restore);
    (result, RESOLVE_DIFFERENCE.with(|cell| cell.replace(0.0)))
}

/// `run`'s result with the thermostat as `thermostat` sets it on this thread, and what the batch
/// counted meanwhile; the switch before is set back after, a panic in `run` included, as
/// [`with_rules`](crate::planetary::chem::escape::with_rules) sets its rules back.
#[cfg(test)]
pub(crate) fn with_thermostat<R>(
    thermostat: Thermostat,
    run: impl FnOnce() -> R,
) -> (R, ThermostatCounts) {
    /// Sets the thread's switch back as it drops.
    struct Restore(Thermostat);
    impl Drop for Restore {
        fn drop(&mut self) {
            THERMOSTAT.with(|cell| cell.set(self.0));
        }
    }
    let restore = Restore(THERMOSTAT.with(|cell| cell.replace(thermostat)));
    THERMOSTAT_COUNTS.with(|cell| cell.set(ThermostatCounts::default()));
    let result = run();
    drop(restore);
    (
        result,
        THERMOSTAT_COUNTS.with(|cell| cell.replace(ThermostatCounts::default())),
    )
}

/// Counts with `count` on this test thread.
#[cfg(test)]
fn count(count: impl FnOnce(&mut ThermostatCounts)) {
    THERMOSTAT_COUNTS.with(|cell| {
        let mut counts = cell.get();
        count(&mut counts);
        cell.set(counts);
    });
}

/// The mass share of `element` in `gas`, by the registry's stoichiometry and standard atomic
/// weights over the row's molar mass.
#[must_use]
fn share(gas: Gas, element: Element) -> f64 {
    let count = gas
        .substance()
        .substance()
        .elements()
        .iter()
        .find(|(e, _)| *e == element)
        .map_or(0, |&(_, n)| n);
    f64::from(count) * element.standard_atomic_weight_g_per_mol() / gas.molar_mass_g_per_mol()
}

/// Where a part of an element's inventory went ([`ElementOutcome`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// Escaped to space.
    Escaped,
    /// In the air or free to fill it: the column the condensate partition reads, its surface
    /// condensates included.
    Airborne,
    /// Held in the melt, rock or ice: a carbonate, a melt's ferric iron, a surface's oxygen sink,
    /// the interior beneath a gas envelope.
    Sequestered,
    /// Photolysed to hydrocarbons and hazes.
    Photolysed,
}

/// Each element's outcome as its parts are counted, kg, in [`ESCAPE_ELEMENTS`]'s order, each
/// part escaped, airborne, sequestered and photolysed.
#[derive(Debug, Clone, Copy, Default)]
struct Ledger([[f64; 4]; 6]);

impl Ledger {
    /// Counts `mass` of `element` to `fate`; nothing of an element escape does not record.
    fn element(&mut self, element: Element, mass: f64, fate: Fate) {
        let Some(row) = ESCAPE_ELEMENTS.iter().position(|&e| e == element) else {
            return;
        };
        let column = match fate {
            Fate::Escaped => 0,
            Fate::Airborne => 1,
            Fate::Sequestered => 2,
            Fate::Photolysed => 3,
        };
        self.0[row][column] += mass;
    }

    /// Counts `mass` of `gas`, each of its elements by its share, to `fate`.
    fn gas(&mut self, gas: Gas, mass: f64, fate: Fate) {
        for &(element, _) in gas.substance().substance().elements() {
            self.element(element, mass * share(gas, element), fate);
        }
    }

    /// Counts a mass `stored` of the carbon's stored carrier, carbon dioxide, carried as
    /// `carrier`, to `fate`: its carbon and its oxygen as carbon dioxide; as methane its carbon
    /// alone, its stored carrier's oxygen counted with the rock and ice
    /// ([`Ledger::methane_oxygen`]).
    fn carbon(&mut self, carrier: CarbonCarrier, stored: f64, fate: Fate) {
        match carrier {
            CarbonCarrier::CarbonDioxide => self.gas(Gas::CarbonDioxide, stored, fate),
            CarbonCarrier::Methane => self.element(
                Element::C,
                stored * share(Gas::CarbonDioxide, Element::C),
                fate,
            ),
        }
    }

    /// Counts a mass `methane` of methane's carbon to `fate`. Methane's hydrogen comes from the
    /// body's water, whose hydrogen its water's outcome counts; the model does not balance the
    /// water–rock chemistry that makes it, so methane's hydrogen is not counted again.
    fn methane(&mut self, methane: f64, fate: Fate) {
        self.element(Element::C, methane * share(Gas::Methane, Element::C), fate);
    }

    /// Counts the oxygen of a methane carrier's stored carbon dioxide, `stored`, with the rock and
    /// ice: the water–rock chemistry that makes its methane keeps it.
    fn methane_oxygen(&mut self, stored: f64) {
        self.element(
            Element::O,
            stored * share(Gas::CarbonDioxide, Element::O),
            Fate::Sequestered,
        );
    }

    /// The oxygen outcome `oxygen` of water lost: its water's hydrogen escaped, the oxygen it
    /// dragged escaped, its melt's and its surface's sequestered, and what the air keeps to `kept`:
    /// airborne, escaped where oxygen fails Jeans's test, and sequestered where a molten surface's
    /// melt holds it.
    fn water_oxygen(&mut self, oxygen: &OxygenOutcome, kept: Fate) {
        self.element(
            Element::H,
            oxygen.water_lost().value() * share(Gas::Water, Element::H),
            Fate::Escaped,
        );
        self.gas(Gas::Oxygen, oxygen.dragged().value(), Fate::Escaped);
        self.gas(
            Gas::Oxygen,
            oxygen.sequestered().value() + oxygen.sunk().value(),
            Fate::Sequestered,
        );
        self.gas(Gas::Oxygen, oxygen.kept().value(), kept);
    }

    /// The outcomes, in [`ESCAPE_ELEMENTS`]'s order.
    #[must_use]
    fn outcomes(&self) -> [ElementOutcome; 6] {
        core::array::from_fn(|i| {
            let [escaped, airborne, sequestered, photolysed] = self.0[i].map(Kilograms::new);
            ElementOutcome::new(
                ESCAPE_ELEMENTS[i],
                escaped,
                airborne,
                sequestered,
                photolysed,
            )
        })
    }
}

/// What else a body's escape records beside its elements' outcomes ([`EscapeOutcome`]).
#[derive(Debug, Clone, Copy)]
struct Recorded {
    envelope: Option<EnvelopeHelium>,
    shoreline: Shoreline,
    oxygen: OxygenOutcome,
    carbon_monoxide: Kilograms,
    airborne_carbon_share: f64,
    methane: MethaneOutcome,
}

impl Recorded {
    /// Nothing but the envelope's helium `envelope`.
    #[must_use]
    const fn of_envelope(envelope: Option<EnvelopeHelium>) -> Self {
        Self {
            envelope,
            shoreline: Shoreline::NONE,
            oxygen: OxygenOutcome::NONE,
            carbon_monoxide: Kilograms::ZERO,
            airborne_carbon_share: 1.0,
            methane: MethaneOutcome::NONE,
        }
    }

    /// The escape of the outcomes `ledger` counts, with these.
    ///
    /// # Panics
    ///
    /// Never: the ledger's outcomes are in [`ESCAPE_ELEMENTS`]'s order.
    #[must_use]
    fn outcome(&self, ledger: &Ledger) -> EscapeOutcome {
        EscapeOutcome::new(
            ledger.outcomes(),
            self.envelope,
            self.shoreline,
            self.oxygen,
            self.carbon_monoxide,
        )
        .expect("the ledger's outcomes are in ESCAPE_ELEMENTS's order")
        .with_airborne_carbon_share(self.airborne_carbon_share)
        .with_methane(self.methane)
    }
}

/// The helium that escape leaves the envelope of the body `inputs` describe under the exposure
/// `exposure`, its own unless its derivation gave it ([`AtmosphereInputs::envelope_helium`]).
#[must_use]
fn envelope_of(inputs: &AtmosphereInputs, exposure: &XuvExposure) -> Option<EnvelopeHelium> {
    inputs.envelope_helium.or_else(|| {
        helium_left(
            inputs.envelope_history,
            (inputs.mass, inputs.radius),
            exposure,
            inputs.age,
        )
    })
}

/// The helium that escape leaves an envelope of history `history` on a body of mass and radius
/// `bulk` at the time, whose gravity sets the diffusion that separates its helium from its escaping
/// hydrogen, under the exposure `exposure` by `age` ([`envelope_helium`], from the protosolar
/// helium, [`protosolar_helium_mass_fraction`]), where it formed with one: a function of what a
/// body's passes do not move, so that its derivation takes it once
/// ([`AtmosphereInputs::envelope_helium`]).
#[must_use]
pub(crate) fn helium_left(
    history: EnvelopeHistory,
    (mass, radius): (Kilograms, Metres),
    exposure: &XuvExposure,
    age: Years,
) -> Option<EnvelopeHelium> {
    (history.initial.value() > 0.0).then(|| {
        #[cfg(test)]
        count(|counts| counts.envelopes += 1);
        envelope_helium(
            history.initial,
            history.lost,
            mass,
            radius,
            exposure,
            age,
            protosolar_helium_mass_fraction(),
        )
    })
}

/// The envelope's hydrogen and helium at formation, kg, of the body `inputs` describe: its
/// history's mass at the protosolar helium mass fraction; none where it formed with none.
#[must_use]
fn envelope_at_formation(inputs: &AtmosphereInputs) -> (f64, f64) {
    let initial = inputs.envelope_history.initial.value().max(0.0);
    let helium = protosolar_helium_mass_fraction();
    ((1.0 - helium) * initial, helium * initial)
}

/// The inventory's carriers of the body `inputs` describe counted to `fate`, the carbon as carbon
/// dioxide.
fn carriers_to(ledger: &mut Ledger, inputs: &AtmosphereInputs, fate: Fate) {
    let inventory = &inputs.inventory;
    ledger.gas(Gas::Water, inventory.water().value(), fate);
    ledger.gas(Gas::CarbonDioxide, inventory.carbon_dioxide().value(), fate);
    ledger.gas(Gas::Nitrogen, inventory.nitrogen().value(), fate);
    ledger.gas(Gas::Argon, inventory.argon().value(), fate);
}

/// One greenhouse solve of the batch ([`Solver::solve`]): the airborne inventory it read, its
/// column weights, its surface temperature, partial pressures and optical depth, and the step
/// climate it read.
#[derive(Clone, Copy)]
struct Solve<'a> {
    air: Airborne,
    column: PartialPressures,
    t: f64,
    partials: PartialPressures,
    tau: f64,
    step: StepClimate<'a>,
}

impl Solve<'_> {
    /// The warmest zone of the climate its step climate gives at its final temperature and partial
    /// pressures, and the water it holds there ([`water_phase`]).
    ///
    /// Under the passes' stand-in, a climate with no contrast, the warmest zone is the mean: the
    /// test is then stricter than at a real warmest zone by Earth's tropical excess over its mean,
    /// some 10 K (recorded).
    #[must_use]
    fn climate(&self) -> (Kelvin, WaterPhase) {
        let warmest = self.step.at(Kelvin::new(self.t), &self.partials).warmest();
        (warmest, water_phase(warmest, self.partials.total()))
    }

    /// The same solve with its air `air`, whose change from the solved one is at most
    /// [`RESOLVE_SKIP_SHARE`] of its mass, and that air's column weights at `weight` Pa per kg: the
    /// state stands.
    #[must_use]
    fn with_air(self, air: Airborne, weight: f64) -> Self {
        Self {
            air,
            column: air.column_weights(weight),
            ..self
        }
    }
}

/// The airborne inventory's mass of `air`, kg: every gas its column weighs, the carbon as its
/// carrier.
#[must_use]
fn air_mass(air: &Airborne) -> f64 {
    let carbon = match air.carrier {
        CarbonCarrier::CarbonDioxide => air.carbon_dioxide,
        CarbonCarrier::Methane => air.methane,
    };
    air.hydrogen + air.helium + air.water + carbon + air.nitrogen + air.oxygen + air.argon
}

/// Whether a change of `changed` kg to the airborne inventory `air` solves the greenhouse again:
/// where it is more than [`RESOLVE_SKIP_SHARE`] of the air's mass.
#[must_use]
fn resolves(changed: f64, air: &Airborne) -> bool {
    let resolved = changed > RESOLVE_SKIP_SHARE * air_mass(air);
    #[cfg(test)]
    if !resolved {
        count(|counts| counts.skipped += 1);
    }
    resolved
}

/// The air of a solve with surface temperature `surface_temperature` and partial pressures
/// `partials`, as Wordsworth and Pierrehumbert's moisture number reads it ([`MoistAir`]): its
/// water's partial pressure, its mole fraction times the surface pressure; its other gases as the
/// background, their partial pressure, mean molar mass by mole and specific heat by mass, each
/// gas's ideal-gas `c_p` at the surface temperature on the registry's molecular constants
/// ([`heat_capacity_over_r_at`]); and water's latent heat there, its saturation curve's own
/// Clausius–Clapeyron enthalpy (P14.T49.b), which exceeds the true latent heat by a few per cent.
/// `None` where water has no phase diagram or no enthalpy at the temperature.
#[must_use]
fn moist_air(partials: &PartialPressures, surface_temperature: Kelvin) -> Option<MoistAir> {
    let water = Gas::Water.substance().substance().phase_diagram()?;
    let latent = water.clausius_clapeyron_enthalpy_j_per_mol(surface_temperature)?
        / (Gas::Water.molar_mass_g_per_mol() * 1e-3);
    let surface = partials.total().value();
    let vapour = mole_fraction_of(partials, Gas::Water) * surface;
    let (weight, moles, heat) = Gas::ALL.into_iter().filter(|&gas| gas != Gas::Water).fold(
        (0.0, 0.0, 0.0),
        |(weight, moles, heat), gas| {
            let p = partials.of(gas).value().max(0.0);
            let molar = gas.molar_mass_g_per_mol();
            let c_p = heat_capacity_over_r_at(gas.substance(), surface_temperature)
                .map_or(0.0, |c| c * MOLAR_GAS_CONSTANT / (molar * 1e-3));
            (weight + p, moles + p / molar, heat + p * c_p)
        },
    );
    Some(MoistAir {
        vapour_pressure: Pascals::new(vapour),
        background_pressure: Pascals::new((surface - vapour).max(0.0)),
        background_molar_mass_g_per_mol: weight / moles,
        background_heat_capacity_j_per_kg_k: heat / weight,
        latent_heat_j_per_kg: latent,
        surface_temperature,
    })
}

/// The share of a body's carbon dioxide air's atoms that are hydrogen, over the partial pressures
/// `partials` (column weights), each gas's moles times its atoms: what Gao et al.'s (2015)
/// photolysis reads ([`desiccated_carbon_dioxide`]). Zero for an empty air.
#[must_use]
fn hydrogen_atom_fraction(partials: &PartialPressures) -> f64 {
    let (hydrogen, atoms) = Gas::ALL.into_iter().fold((0.0, 0.0), |(h, all), gas| {
        let moles = partials.of(gas).value().max(0.0) / gas.molar_mass_g_per_mol();
        let elements = gas.substance().substance().elements();
        let atoms = elements.iter().fold(0.0, |sum, &(_, n)| sum + f64::from(n));
        let hydrogen = elements
            .iter()
            .find(|(e, _)| *e == Element::H)
            .map_or(0.0, |&(_, n)| f64::from(n));
        (h + moles * hydrogen, all + moles * atoms)
    });
    let fraction = hydrogen / atoms;
    if fraction.is_finite() { fraction } else { 0.0 }
}

/// What every step of the batch reads of one body ([`state_and_greenhouse`]).
struct Body<'a> {
    inputs: &'a AtmosphereInputs,
    exobase: Kelvin,
    /// Its base temperature, `T_base`: the airless mean over the pass's surface
    /// (`surface_mix::airless_mean`; P14.T24.i's follow-up, decision-t24i-airless-state D1.3),
    /// the larger of `T_eq` and the night floor under the unlit mean's rules and `T_eq` to the bit
    /// under their hold. The greenhouse starts from it, an airless body publishes at it, and the
    /// ice-held stand-in reads it.
    base: Kelvin,
    retention: Retention,
    held: IceHeld,
    exposure: XuvExposure,
    envelope: Option<EnvelopeHelium>,
    /// The envelope's hydrogen and helium at formation, kg ([`envelope_at_formation`]).
    formed: (f64, f64),
    /// The maximum-greenhouse ceiling its grey surface is held under ([`GreenhouseCeiling::of`]).
    ceiling: GreenhouseCeiling,
    /// The state its passes publish, the pass's surface's ([`PassSurface::published`]): the
    /// climate's, or P14.T53.b part 1 (c)'s hothouse in its re-solve (`finish`).
    published: PublishedState,
}

/// What the body's retention and ice leave it ([`Body::kept`], step 2), before the shoreline.
struct Kept {
    air: Airborne,
    /// The water free to leave, kg: not its ice's.
    free_water: f64,
    /// The water its ice crust holds, kg.
    held_water: f64,
    /// Whether it lies inside the runaway limit with a solid crust.
    runaway: bool,
    /// The carbon kept, as its stored carrier carbon dioxide, kg.
    carbon: f64,
}

impl<'a> Body<'a> {
    /// The body `inputs` describe over the pass's surface `surface`, whose heat flow sets its
    /// night floor and so its base temperature (its `base`; decision-t24i-airless-state D1.3).
    /// The exobase stays the irradiation temperature's.
    #[must_use]
    fn of(inputs: &'a AtmosphereInputs, surface: &PassSurface<'_>) -> Self {
        let exobase = exobase_temperature(inputs.irradiation_temperature, inputs.xuv_fluence);
        let base = surface_mix::airless_mean(inputs.equilibrium, surface);
        let (retention, held) = retained(inputs, exobase, base);
        let exposure = XuvExposure::new(inputs.saturated_xuv_flux, inputs.xuv_saturation_time);
        Self {
            inputs,
            exobase,
            base,
            retention,
            held,
            exposure,
            envelope: envelope_of(inputs, &exposure),
            formed: envelope_at_formation(inputs),
            ceiling: GreenhouseCeiling::of(inputs),
            published: surface.published,
        }
    }

    /// `t`, held at the hottest luminous host's effective temperature where one shines.
    #[must_use]
    fn saturate(&self, t: f64) -> Kelvin {
        let cap = self.inputs.hottest_host.value();
        Kelvin::new(if cap > 0.0 { t.min(cap) } else { t })
    }

    /// The body as `state`, with the partial pressures `partials` and no greenhouse, at `t`.
    #[must_use]
    fn airless(
        &self,
        state: SurfaceState,
        partials: PartialPressures,
        t: f64,
        carrier: Option<CarbonCarrier>,
    ) -> Atmosphere {
        Atmosphere {
            state,
            water_state: state,
            runaway_deck: RunawayDeck::Kept,
            dry_division: false,
            liquid_water: false,
            surface_temperature: self.saturate(t),
            surface_pressure: Some(partials.total()),
            partials,
            retention: self.retention,
            ice_held: self.held.retention(),
            exobase_temperature: self.exobase,
            optical_depth: 0.0,
            albedo: state.albedo(self.inputs.material),
            cloud_fraction: state.cloud_fraction(),
            carbon_carrier: carrier,
            column: PartialPressures::default(),
            escape: None,
            ceiling: self.ceiling,
        }
    }

    /// The body as one that holds nothing, airless at its base temperature, the airless mean (its
    /// `base`, as the held path's airless arms publish; decision-t24i-airless-state D1.3), every
    /// element escaped: its envelope's hydrogen and helium `formed` and its inventory.
    #[must_use]
    fn holding_nothing(&self, (hydrogen, helium): (f64, f64)) -> Atmosphere {
        let mut nothing = Ledger::default();
        nothing.element(Element::H, hydrogen, Fate::Escaped);
        nothing.element(Element::He, helium, Fate::Escaped);
        carriers_to(&mut nothing, self.inputs, Fate::Escaped);
        Atmosphere {
            escape: Some(Recorded::of_envelope(self.envelope).outcome(&nothing)),
            ..self.airless(
                SurfaceState::Airless,
                PartialPressures::default(),
                self.base.value(),
                None,
            )
        }
    }

    /// Step 1: the body as a gas envelope, with its helium. Its volatiles lie beneath it, and its
    /// metals are P14.T50.b's.
    #[must_use]
    fn gas_envelope(&self) -> Atmosphere {
        let (hydrogen_formed, helium_formed) = self.formed;
        let (hydrogen, helium) = self.envelope.map_or((1.0, 1.0), |envelope| {
            (envelope.hydrogen_kept_share(), envelope.helium_kept_share())
        });
        let mut ledger = Ledger::default();
        ledger.element(Element::H, hydrogen_formed * hydrogen, Fate::Airborne);
        ledger.element(
            Element::H,
            hydrogen_formed * (1.0 - hydrogen),
            Fate::Escaped,
        );
        ledger.element(Element::He, helium_formed * helium, Fate::Airborne);
        ledger.element(Element::He, helium_formed * (1.0 - helium), Fate::Escaped);
        carriers_to(&mut ledger, self.inputs, Fate::Sequestered);
        Atmosphere {
            surface_pressure: None,
            escape: Some(Recorded::of_envelope(self.envelope).outcome(&ledger)),
            ..self.airless(
                SurfaceState::GasEnvelope,
                PartialPressures::default(),
                self.inputs.heated.value(),
                None,
            )
        }
    }

    /// Step 2: Jeans's test and the ice, as built, counting what they lose to `ledger`; a young
    /// melt carries its carbon as carbon dioxide.
    #[must_use]
    fn kept(&self, ledger: &mut Ledger) -> Kept {
        let (inputs, held) = (self.inputs, self.held);
        let mut air = Airborne::kept(inputs, self.retention, held);
        if inputs.crust == Crust::Molten {
            air.carrier = CarbonCarrier::CarbonDioxide;
            air.methane = 0.0;
        }
        let carrier = air.carrier;
        let inventory = inputs.inventory;
        let runaway = matches!(
            (inputs.crust, inputs.insolation),
            (Crust::Solid, Insolation::InsideRunaway)
        );
        // A runaway world loses its free water by its own route, whatever water's Jeans test says
        // (rule (a)). The water its ice crust holds stays, and frees no oxygen: a past runaway
        // takes an ice crust's surface layer, not its bulk (P14.T53.a divides the oxygen
        // `runaway_oxygen` took of the whole inventory).
        let held_water = held.water.map_or(0.0, Kilograms::value);
        let free_water = match (runaway, held.water) {
            (true, None) => inventory.water().value(),
            (true, Some(_)) => 0.0,
            (false, _) => air.water - held_water,
        };
        ledger.gas(
            Gas::Water,
            inventory.water().value() - held_water - free_water,
            Fate::Escaped,
        );
        let carbon_kept = match carrier {
            CarbonCarrier::CarbonDioxide => air.carbon_dioxide,
            CarbonCarrier::Methane if self.retention.retains(Gas::Methane) => {
                inventory.carbon_dioxide().value()
            }
            CarbonCarrier::Methane => 0.0,
        };
        ledger.carbon(
            carrier,
            inventory.carbon_dioxide().value() - carbon_kept,
            Fate::Escaped,
        );
        if carrier == CarbonCarrier::Methane {
            ledger.methane_oxygen(inventory.carbon_dioxide().value());
        }
        ledger.gas(
            Gas::Nitrogen,
            inventory.nitrogen().value() - air.nitrogen,
            Fate::Escaped,
        );
        ledger.gas(
            Gas::Argon,
            inventory.argon().value() - air.argon,
            Fate::Escaped,
        );
        Kept {
            air,
            free_water,
            held_water,
            runaway,
            carbon: carbon_kept,
        }
    }

    /// Step 3: the cosmic shoreline, on every airborne gas of `kept` but what the ice holds,
    /// counting what it takes to `ledger`; its verdict.
    #[must_use]
    fn erode(&self, kept: &mut Kept, ledger: &mut Ledger) -> Shoreline {
        let inputs = self.inputs;
        let (m, r) = (inputs.mass.value(), inputs.radius.value());
        let escape_speed = MetresPerSecond::new((2.0 * GRAVITATIONAL_CONSTANT * m / r).sqrt());
        let shoreline = Shoreline::of(
            inputs.xuv_fluence.value() / earth_xuv_fluence().value(),
            escape_speed,
            inputs.impact_speed,
            inputs.age,
        );
        let share = shoreline.kept();
        let lost = 1.0 - share;
        let air = &mut kept.air;
        ledger.gas(Gas::Water, kept.free_water * lost, Fate::Escaped);
        kept.free_water *= share;
        air.water = kept.held_water + if kept.runaway { 0.0 } else { kept.free_water };
        // Methane lies in the air beside the nitrogen, and is held by the ice where the nitrogen
        // is.
        let held_nitrogen = self.held.nitrogen.is_some();
        if !(air.carrier == CarbonCarrier::Methane && held_nitrogen) {
            ledger.carbon(air.carrier, kept.carbon * lost, Fate::Escaped);
            air.carbon_dioxide *= share;
            air.methane *= share;
        }
        if !held_nitrogen {
            ledger.gas(Gas::Nitrogen, air.nitrogen * lost, Fate::Escaped);
            air.nitrogen *= share;
        }
        ledger.gas(Gas::Argon, air.argon * lost, Fate::Escaped);
        air.argon *= share;
        shoreline
    }

    /// Step 4: a thin envelope's hydrogen and helium into `air`, at its fractionated helium, kept
    /// where Jeans's test retains them, less the `shoreline`'s share; what energy-limited escape
    /// took of the envelope before is escaped, to `ledger`. The hydrogen and helium the body
    /// formed with, kg: its envelope's, or the thin envelope's where its history holds none.
    #[must_use]
    fn thin_envelope(
        &self,
        air: &mut Airborne,
        shoreline: &Shoreline,
        ledger: &mut Ledger,
    ) -> (f64, f64) {
        let inputs = self.inputs;
        let (kept, lost) = (shoreline.kept(), 1.0 - shoreline.kept());
        let helium_fraction = self
            .envelope
            .map_or_else(protosolar_helium_mass_fraction, |envelope| {
                envelope.helium_mass_fraction()
            });
        let (thin_hydrogen, thin_helium) =
            thin_envelope_air(inputs.envelope_fraction, inputs.mass, helium_fraction);
        let (thin_hydrogen, thin_helium) = (thin_hydrogen.value(), thin_helium.value());
        let formed = if inputs.envelope_history.initial.value() > 0.0 {
            self.formed
        } else {
            (thin_hydrogen, thin_helium)
        };
        let retention = self.retention;
        let mut thin_air = |element: Element, gas: Gas, formed: f64, thin: f64| {
            ledger.element(element, formed - thin, Fate::Escaped);
            if retention.retains(gas) {
                ledger.element(element, thin * lost, Fate::Escaped);
                thin * kept
            } else {
                ledger.element(element, thin, Fate::Escaped);
                0.0
            }
        };
        air.hydrogen = thin_air(Element::H, Gas::Hydrogen, formed.0, thin_hydrogen);
        air.helium = thin_air(Element::He, Gas::Helium, formed.1, thin_helium);
        formed
    }

    /// Step 5's division of `kept`'s free water by the climate: the state, and a runaway world's
    /// oxygen of its water lost over its exposure, which joins its air where oxygen is retained,
    /// the water it has not lost counted to `ledger` with the interior.
    #[must_use]
    fn divide(&self, kept: &mut Kept, ledger: &mut Ledger) -> (SurfaceState, OxygenOutcome) {
        let inputs = self.inputs;
        match (inputs.crust, inputs.insolation) {
            (Crust::Molten, _) => (SurfaceState::MagmaOcean, OxygenOutcome::NONE),
            (Crust::Solid, Insolation::InsideRunaway) => {
                let melt = MeltWindow {
                    until: inputs.molten_for,
                    capacity: melt_oxygen_capacity(inputs.mass),
                };
                let oxygen = runaway_oxygen_timed(
                    Kilograms::new(kept.free_water),
                    inputs.mass,
                    inputs.radius,
                    &self.exposure,
                    melt,
                    EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR,
                    inputs.age,
                );
                // The free water the timed route has not lost by the age leaves the air all the
                // same by rule (a), as before; P14.T53.b's steam is to keep it. Until then it is
                // counted with the interior.
                ledger.gas(
                    Gas::Water,
                    (kept.free_water - oxygen.water_lost().value()).max(0.0),
                    Fate::Sequestered,
                );
                if self.retention.retains(Gas::Oxygen) {
                    kept.air.oxygen = oxygen.kept().value();
                }
                (SurfaceState::RunawayGreenhouse, oxygen)
            }
            (Crust::Solid, Insolation::Habitable) => (SurfaceState::Temperate, OxygenOutcome::NONE),
            (Crust::Solid, Insolation::BeyondMaximumGreenhouse) => {
                (SurfaceState::Snowball, OxygenOutcome::NONE)
            }
        }
    }
}

/// The batch's greenhouse of one body: the partition's partial pressures over the step climate of
/// each airborne inventory's own column, as before ([`partitioned_greenhouse`]).
struct Solver<'a, 'b> {
    body: &'a Body<'a>,
    surface: &'a PassSurface<'b>,
    /// The column's weight per kilogram, Pa kg⁻¹: g ÷ 4πR².
    weight: f64,
    ground: Ground,
    state: SurfaceState,
    /// Whether the body keeps its water against escape.
    retained_water: bool,
}

impl<'b> Solver<'_, 'b> {
    /// The greenhouse of the airborne inventory `air`; `None` where its column holds a weight the
    /// partition cannot read.
    #[must_use]
    fn solve(&self, air: &Airborne) -> Option<Solve<'b>> {
        self.solve_in(air, GREENHOUSE_STEPS)
    }

    /// [`solve`](Self::solve) in at most `steps` greenhouse steps, from the body's base
    /// temperature, the airless mean (P14.T24.i's follow-up, decision-t24i-airless-state D1.3): the
    /// steps start there, the grey surface is `T_base` (1 + ¾τ)^¼, and N₂'s collision factor, the
    /// batch's H₂ term and the ceiling's `T_rest` read it. The step climate keeps the body's own
    /// `T_eq` and heat flow ([`StepClimate::of`]).
    #[must_use]
    fn solve_in(&self, air: &Airborne, steps: u32) -> Option<Solve<'b>> {
        let inputs = self.body.inputs;
        let Ok(inventory) = air.column(self.weight) else {
            // A weight that is negative, infinite or not a number (`NotAWeight`): every mass here
            // is held at zero or above, so only a weight per kilogram that is not finite, of a body
            // far smaller than any the generator makes, reaches it; the body then holds nothing
            // the partition can weigh (`holding_nothing`).
            return None;
        };
        let column = air.column_weights(self.weight);
        let step = StepClimate::of(
            self.surface,
            inputs,
            column,
            (self.state, self.retained_water),
        );
        let climate_of = |t: Kelvin, partials: &PartialPressures| step.at(t, partials);
        let (t, partials, tau) = partitioned_greenhouse(
            &inventory,
            self.body.base,
            &|t| self.body.saturate(t),
            &climate_of,
            (&self.ground, self.body.ceiling),
            steps,
        );
        Some(Solve {
            air: *air,
            column,
            t,
            partials,
            tau,
            step,
        })
    }

    /// A temperate world with water: the weathering thermostat on the air `air`, its carbonate the
    /// carbon it leaves stored. Anywhere else the carbon is airborne: the share left airborne, and
    /// the solve.
    ///
    /// A temperate air that holds no carbon is solved once: every share gives the same state, so
    /// the share is the thermostat's at it, the stored one where it holds liquid water and all of
    /// it elsewhere.
    #[must_use]
    fn airborne_carbon(&self, air: Airborne) -> (f64, Option<Solve<'b>>) {
        if self.state != SurfaceState::Temperate || air.water <= 0.0 {
            return (1.0, self.solve(&air));
        }
        let climate = |solved: &Option<Solve<'_>>| {
            solved
                .as_ref()
                .map_or((Kelvin::new(f64::NAN), WaterPhase::Frozen), Solve::climate)
        };
        if air.carbon_dioxide <= 0.0 && air.methane <= 0.0 {
            let solved = self.solve(&air);
            let share = match climate(&solved).1 {
                WaterPhase::Liquid => TEMPERATE_AIRBORNE_CARBON,
                WaterPhase::Frozen | WaterPhase::Hot => 1.0,
            };
            return (share, solved);
        }
        weathering_thermostat(
            TEMPERATE_AIRBORNE_CARBON,
            |s: f64, steps: u32| {
                let mut trial = air;
                trial.carbon_dioxide = air.carbon_dioxide * s;
                trial.methane = air.methane * s;
                self.solve_in(&trial, steps)
            },
            climate,
        )
    }

    /// A temperate world whose cold trap fails for want of a non-condensing gas builds oxygen
    /// (Wordsworth and Pierrehumbert 2014), at the state that stands, `solved`, the thermostat's:
    /// its surface temperature and its partial pressures. The oxygen it keeps joins the air where
    /// oxygen is retained, the water it cost leaves it, and the greenhouse is solved again once
    /// with them ([`resolves`]). The solve that stands, and the oxygen built, if any.
    #[must_use]
    fn cold_trap(&self, solved: Option<&Solve<'b>>) -> (Option<Solve<'b>>, Option<OxygenOutcome>) {
        let inputs = self.body.inputs;
        let Some(&chosen) = solved.filter(|chosen| {
            self.state == SurfaceState::Temperate && self.retained_water && chosen.air.water > 0.0
        }) else {
            return (solved.copied(), None);
        };
        let Some(moist) = moist_air(&chosen.partials, Kelvin::new(chosen.t)) else {
            return (solved.copied(), None);
        };
        #[cfg(test)]
        count(|counts| counts.cold_traps += 1);
        let failed = cold_trap_failure_oxygen(
            Kilograms::new(chosen.air.water),
            inputs.mass,
            inputs.radius,
            &moist,
            &self.body.exposure,
            EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR,
            inputs.age,
        );
        let lost = failed.water_lost().value();
        if lost <= 0.0 {
            return (solved.copied(), None);
        }
        let mut wet = chosen.air;
        wet.water = (wet.water - lost).max(0.0);
        let added = if self.body.retention.retains(Gas::Oxygen) {
            wet.oxygen += failed.kept().value();
            failed.kept().value()
        } else {
            0.0
        };
        let solved = if resolves(lost + added, &chosen.air) {
            #[cfg(test)]
            let before = steps_so_far();
            let again = self.solve(&wet);
            #[cfg(test)]
            count(|counts| {
                counts.cold_trap += 1;
                counts.resolve_steps += steps_so_far() - before;
            });
            again
        } else {
            Some(chosen.with_air(wet, self.weight))
        };
        (solved, Some(failed))
    }

    /// A methane reservoir loses what escapes through its nitrogen and what its photolysis takes
    /// (science-p14-t24f §4), and the greenhouse is solved again once without them, but where the
    /// air stays at its cap or the change is a tiny share of it ([`resolves`]): the solve that
    /// stands after `solved`, and the methane's outcome.
    ///
    /// - The escape reads the methane's mole fraction at the surface, above its homopause's:
    ///   Titan's (5.65 ± 0.18) × 10⁻² near the surface against its (1.48 ± 0.09) × 10⁻² in the
    ///   lower stratosphere, which the tropopause's cold trap sets (Niemann et al. 2010, JGR Planets
    ///   115, E12006), 3.82 times. The body's irradiation temperature stands in for its
    ///   homopause's: the diffusion limit goes as T^−¼, so Titan's 89.92 K against its 150 K gives
    ///   (150 ÷ 89.92)^¼ = 1.14 times more. Together the escape is overstated 3.82 × 1.14 = 4.3
    ///   times at Titan (recorded).
    /// - The photolysis is scaled by the body's present bolometric flux, averaged over its orbit,
    ///   over Titan's ([`TITAN_INSOLATION`]): the bolometric flux stands in for the Lyman α and far
    ///   ultraviolet that photolyse methane (Yung, Allen and Pinto 1984, ApJS 55, 465), at the
    ///   present light over the whole age. An evolved host's history, whose light was not its
    ///   present one, is out of its range (recorded).
    #[must_use]
    fn methane(
        &self,
        solved: Option<&Solve<'b>>,
        ledger: &mut Ledger,
    ) -> (Option<Solve<'b>>, MethaneOutcome) {
        let inputs = self.body.inputs;
        let Some(&chosen) = solved.filter(|chosen| {
            chosen.air.carrier == CarbonCarrier::Methane && chosen.air.methane > 0.0
        }) else {
            return (solved.copied(), MethaneOutcome::NONE);
        };
        let flux = WattsPerSquareMetre::from(inputs.flux).value();
        let methane = methane_outcome(
            Kilograms::new(chosen.air.methane),
            mole_fraction_of(&chosen.partials, Gas::Methane),
            inputs.irradiation_temperature,
            inputs.mass,
            inputs.radius,
            flux / TITAN_INSOLATION.value(),
            inputs.age,
        );
        ledger.methane(methane.escaped().value(), Fate::Escaped);
        ledger.methane(methane.photolysed().value(), Fate::Photolysed);
        let lost = methane.escaped().value() + methane.photolysed().value();
        if lost <= 0.0 {
            return (solved.copied(), methane);
        }
        let mut drier = chosen.air;
        drier.methane = methane.kept().value();
        // Where the air held its methane at its cap, some of it condensed, and what is kept still
        // fills that cap, the partition holds the same air: the state stands, with the drier
        // column (checked against a solve again by `with_resolve_check`).
        let airborne = chosen.partials.of(Gas::Methane).value();
        let saturated = airborne < chosen.column.of(Gas::Methane).value()
            && drier.methane * self.weight >= airborne;
        let solved = if saturated {
            #[cfg(test)]
            self.check_saturated(&chosen, &drier);
            Some(chosen.with_air(drier, self.weight))
        } else if resolves(lost, &chosen.air) {
            #[cfg(test)]
            let before = steps_so_far();
            let again = self.solve(&drier);
            #[cfg(test)]
            count(|counts| {
                counts.methane += 1;
                counts.resolve_steps += steps_so_far() - before;
            });
            again
        } else {
            Some(chosen.with_air(drier, self.weight))
        };
        (solved, methane)
    }

    /// Counts a saturated reservoir's loss whose greenhouse is not solved again, and under
    /// [`with_resolve_check`] solves it all the same, counting a state that differs from `chosen`
    /// in its temperature or a partial pressure, and the largest share by which one does.
    #[cfg(test)]
    fn check_saturated(&self, chosen: &Solve<'_>, drier: &Airborne) {
        count(|counts| counts.saturated += 1);
        if !RESOLVE_CHECK.with(core::cell::Cell::get) {
            return;
        }
        let Some(again) = self.solve(drier) else {
            count(|counts| counts.mismatched += 1);
            return;
        };
        let relative = |a: f64, b: f64| {
            if a.total_cmp(&b).is_eq() {
                0.0
            } else {
                (a - b).abs() / a.abs().max(b.abs())
            }
        };
        let largest = Gas::ALL
            .into_iter()
            .fold(relative(chosen.t, again.t), |most, gas| {
                most.max(relative(
                    chosen.partials.of(gas).value(),
                    again.partials.of(gas).value(),
                ))
            });
        if largest > 0.0 {
            count(|counts| counts.mismatched += 1);
        }
        RESOLVE_DIFFERENCE.with(|cell| cell.set(cell.get().max(largest)));
    }
}

/// Steps 1 to 5 of the atmosphere ([`atmosphere_under`](super::atmosphere_under)) under the
/// escape's batch rules, over the surface `surface`, with the escape's outcome
/// ([`Atmosphere::escape`]): the [module documentation](self)'s order.
#[must_use]
pub(super) fn state_and_greenhouse(
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
) -> Atmosphere {
    let body = Body::of(inputs, surface);
    if inputs.envelope_fraction >= THIN_ENVELOPE_FRACTION {
        return body.gas_envelope();
    }
    let (m, r) = (inputs.mass.value(), inputs.radius.value());
    if !(m.is_finite() && m > 0.0 && r.is_finite() && r > 0.0) {
        // A body of no mass or radius holds nothing.
        return body.holding_nothing(body.formed);
    }
    let mut ledger = Ledger::default();
    let mut kept = body.kept(&mut ledger);
    let shoreline = body.erode(&mut kept, &mut ledger);
    let formed = body.thin_envelope(&mut kept.air, &shoreline, &mut ledger);
    let (state, runaway_oxygen) = body.divide(&mut kept, &mut ledger);
    let (air, carrier) = (kept.air, kept.air.carrier);
    // Whether the division stores no carbon for want of water, which P14.T53.b part 1 (c) reads
    // (`solved_as_a_hothouse`): a runaway's, its water lost, or one between the limits that holds
    // no water, which the weathering thermostat then skips (`Solver::airborne_carbon`).
    let dry = match state {
        SurfaceState::RunawayGreenhouse => true,
        SurfaceState::Temperate => air.water <= 0.0,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::Snowball => false,
    };

    // Step 5's greenhouse, the thermostat's carbonate, the cold trap's oxygen and the methane's
    // loss.
    let gravity = MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r));
    let greenhouse = Solver {
        body: &body,
        surface,
        weight: gravity.value() / (4.0 * PI * r * r),
        ground: Ground::new(gravity, surface.basins, surface.water_at_surface),
        state,
        retained_water: body
            .retention
            .retains_against_escape(body.held.retention(), Gas::Water),
    };
    let (share, solved) = greenhouse.airborne_carbon(air);
    if let Some(chosen) = solved.as_ref() {
        let stored = match carrier {
            CarbonCarrier::CarbonDioxide => air.carbon_dioxide - chosen.air.carbon_dioxide,
            CarbonCarrier::Methane => {
                (air.methane - chosen.air.methane) * Gas::CarbonDioxide.molar_mass_g_per_mol()
                    / Gas::Methane.molar_mass_g_per_mol()
            }
        };
        ledger.carbon(carrier, stored, Fate::Sequestered);
    }
    let (solved, failed_trap) = greenhouse.cold_trap(solved.as_ref());
    let oxygen = failed_trap.unwrap_or(runaway_oxygen);
    let (solved, methane) = greenhouse.methane(solved.as_ref(), &mut ledger);
    let Some(solved) = solved else {
        return body.holding_nothing(formed);
    };
    finish(
        &body,
        &solved,
        (state, dry, gravity),
        &mut ledger,
        Recorded {
            envelope: body.envelope,
            shoreline,
            oxygen,
            carbon_monoxide: Kilograms::ZERO,
            airborne_carbon_share: share,
            methane,
        },
    )
}

/// The state a body the atmosphere's inputs `inputs` describe publishes in place of the state
/// `state` of its climate's division, its passes publishing `published` (P14.T53.b part 1 (a) and
/// (c); decision-absurd-states-24 R4.1, decision-t53b-arm-c):
///
/// - (a): a [`SurfaceState::RunawayGreenhouse`], a world inside the runaway limit at its hosts'
///   largest past luminosity, publishes its present class
///   ([`AtmosphereInputs::present_insolation`]): a runaway still inside the limit now,
///   `Temperate` between the limits now, `Snowball` beyond the maximum-greenhouse limit now, its
///   water lost and its carbon airborne all the same (the division's). The present class comes
///   from the hosts' luminosities alone, not from the passes' figures.
/// - (c): in the re-solve of a body whose first solve its passes judged a hothouse between the
///   limits ([`PublishedState::Hothouse`], [`solved_as_a_hothouse`]), a `Temperate` publishes
///   [`SurfaceState::RunawayGreenhouse`], its division and its water's state unchanged. The state
///   is fixed within that solve: no feedback.
///
/// Every other state is its own, `Airless` and `MagmaOcean` by their own tests, and so is every
/// state where part 1 is not in force ([`present_state_in_force`]), whose passes judge no hothouse.
///
/// [`solved_as_a_hothouse`]: super::solved_as_a_hothouse
#[must_use]
fn present_state(
    state: SurfaceState,
    inputs: &AtmosphereInputs,
    published: PublishedState,
) -> SurfaceState {
    let present = match state {
        SurfaceState::RunawayGreenhouse if present_state_in_force() => {
            match inputs.present_insolation {
                Insolation::InsideRunaway => SurfaceState::RunawayGreenhouse,
                Insolation::Habitable => SurfaceState::Temperate,
                Insolation::BeyondMaximumGreenhouse => SurfaceState::Snowball,
            }
        }
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Snowball => state,
    };
    if present == SurfaceState::Temperate && published == PublishedState::Hothouse {
        SurfaceState::RunawayGreenhouse
    } else {
        present
    }
}

/// The atmosphere of the body `body` from the solve that stands, `solved`, in the state of its
/// climate's division, whether that division stores no carbon for want of water (`dry`), at
/// surface gravity `gravity`: its airborne gases counted to `ledger`, Gao et al.'s carbon monoxide
/// (step 6) and the outcome `recorded` (step 8) with it. Under [`AIRLESS_PRESSURE`] of the solve's
/// partial pressures it is airless at the body's base temperature, the airless mean
/// (decision-t24i-airless-state D1.3). Otherwise it publishes the present class of a world inside
/// the runaway limit at its hosts' peak alone, or in a hothouse's re-solve a runaway
/// (`present_state`, P14.T53.b part 1 (a) and (c)), its water following the division's state
/// ([`Atmosphere::water_state`]).
#[must_use]
fn finish(
    body: &Body<'_>,
    solved: &Solve<'_>,
    (state, dry, gravity): (SurfaceState, bool, MetresPerSecondSquared),
    ledger: &mut Ledger,
    recorded: Recorded,
) -> Atmosphere {
    let inputs = body.inputs;
    let Solve {
        air,
        column,
        mut t,
        mut partials,
        tau,
        ..
    } = *solved;
    let carrier = air.carrier;
    // The state, as before: a molten surface's melt holds the air's oxygen while it is molten.
    let molten = state == SurfaceState::MagmaOcean || t >= SILICATE_SOLIDUS.value();
    if molten && air.oxygen > 0.0 {
        partials.0[Gas::Oxygen.index()] = Pascals::ZERO;
    }
    let kept_oxygen = match (body.retention.retains(Gas::Oxygen), molten) {
        (false, _) => Fate::Escaped,
        (true, true) => Fate::Sequestered,
        (true, false) => Fate::Airborne,
    };
    ledger.water_oxygen(&recorded.oxygen, kept_oxygen);
    ledger.gas(Gas::Water, air.water, Fate::Airborne);
    match carrier {
        CarbonCarrier::CarbonDioxide => {
            ledger.carbon(carrier, air.carbon_dioxide, Fate::Airborne);
        }
        CarbonCarrier::Methane => ledger.methane(air.methane, Fate::Airborne),
    }
    ledger.gas(Gas::Nitrogen, air.nitrogen, Fate::Airborne);
    ledger.gas(Gas::Argon, air.argon, Fate::Airborne);
    // A thin envelope's hydrogen and helium are counted as the elements' own masses, as its
    // formation's and its losses are.
    ledger.element(Element::H, air.hydrogen, Fate::Airborne);
    ledger.element(Element::He, air.helium, Fate::Airborne);

    // Step 6: Gao et al. 2015, under a host that shines, on the outcome only.
    let r = inputs.radius.value();
    let carbon_monoxide =
        if carrier == CarbonCarrier::CarbonDioxide && inputs.hottest_host.value() > 0.0 {
            let co2 = partials.of(Gas::CarbonDioxide).value() * 4.0 * PI * r * r / gravity.value();
            let moles = co2 / (Gas::CarbonDioxide.molar_mass_g_per_mol() * 1e-3);
            let photolysed = desiccated_carbon_dioxide(
                moles,
                hydrogen_atom_fraction(&partials),
                UltravioletHost::of(inputs.hottest_host),
            );
            let monoxide = Element::C.standard_atomic_weight_g_per_mol()
                + Element::O.standard_atomic_weight_g_per_mol();
            Kilograms::new(photolysed.carbon_monoxide_mol() * monoxide * 1e-3)
        } else {
            Kilograms::ZERO
        };

    // Step 8: the outcome.
    let escape = Some(
        Recorded {
            carbon_monoxide,
            ..recorded
        }
        .outcome(ledger),
    );
    let carrier = Some(carrier);
    let total = partials.total();
    let state = if molten {
        t = t.max(SILICATE_SOLIDUS.value());
        SurfaceState::MagmaOcean
    } else if total < AIRLESS_PRESSURE {
        // Under 100 Pa of the greenhouse's own partial pressures, from the base: published at the
        // base, the airless mean, as the held path's airless arms are (decision-t24i-airless-state
        // D1.3).
        let air = body.airless(SurfaceState::Airless, partials, body.base.value(), carrier);
        return Atmosphere {
            column,
            escape,
            ..air
        };
    } else {
        state
    };
    // P14.T53.b part 1 (a) and (c): the state the water follows is the division's, and the
    // published one the present insolation's, or in a hothouse's re-solve a runaway's.
    let water_state = state;
    let state = present_state(state, inputs, body.published);
    Atmosphere {
        state,
        water_state,
        runaway_deck: RunawayDeck::Kept,
        dry_division: dry,
        liquid_water: false,
        surface_temperature: body.saturate(t),
        surface_pressure: Some(total),
        partials,
        retention: body.retention,
        ice_held: body.held.retention(),
        exobase_temperature: body.exobase,
        optical_depth: tau,
        albedo: state.albedo(inputs.material),
        cloud_fraction: state.cloud_fraction(),
        carbon_carrier: carrier,
        column,
        escape,
        ceiling: body.ceiling,
    }
}
