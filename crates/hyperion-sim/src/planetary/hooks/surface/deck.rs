//! A world locked 1:1's substellar cloud deck (plan 14, P14.T24.j), behind its hold at generator
//! version 26 ([`LOCKED_CLIMATE_VERSION`]).
//!
//! On a world locked 1:1 to its seasonal host with open water under its substellar point, the
//! dayside's convergence and convection build a thick deck of cloud there, whose albedo rises with
//! the stellar flux, while the night's inversion makes the outgoing flux follow the surface, so
//! that the locked world keeps a small share of a rotator's greenhouse (Yang, Cowan and Abbot
//! 2013, ApJL 771, L45, §3 and Table 1). The GCMs at 1,360 W m⁻² about a 3,400 K host put the
//! mean at 246–272 K (Yang et al. 2019, ApJ 875, 46, Table 3; area means, where the column's is
//! the surface's emission mean), where P14.T24.a's column, a rotator's mean, gives the table's
//! Earth 290.4 K under the lock.
//!
//! The rule changes P14.T24.a's [`Column`] before its climate ([`climate_of`]), in
//! [`SurfaceConditions::derive`](super::SurfaceConditions::derive) and in the passes'
//! [`step_climate`](super::step_climate) alike, so that the record and T13.c's passes read one
//! climate ([`under_deck`]):
//!
//! - **The weight**, w = r · `f_S` · `f_p` · `f_open`, each 0 to 1: the rotation regime's
//!   ([`rotation_weight`]), the flux's ([`flux_weight`]), the pressure's ([`pressure_weight`]) and
//!   the open water's ([`open_water_weight`]), which is 0 where the surface holds no liquid water
//!   and otherwise a ramp of the warmest climate on the changed column, so that w is solved for
//!   itself.
//! - **The albedo**, `A_w` = A + w max(0, `A_deck`(S) − A) ([`deck_albedo`]), A the albedo the
//!   column's equilibrium temperature is at, and `T_eq,w` = `T_eq` ((1 − `A_w`) ÷ (1 − A))^¼.
//! - **The greenhouse**, G = `T_s` − `T_eq` the column's own and `G_w` = G (1 − w (1 − s(S))), s
//!   the locked share of a rotator's ([`locked_greenhouse_share`]), so that `T_s,w` = `T_eq,w` +
//!   `G_w`.
//!
//! The column takes `T_s,w` and `T_eq,w`; its air (pressure, specific heat, optical depth and
//! vapour gain) stays T13.c's. Cowan and Agol's day and night at Koll's ε, the zonal profile, the
//! range and the regime then follow on the changed column, unchanged. T13.c's own `T_s`, the
//! air's, is not changed, so the published mean differs from it on such a world.
//!
//! Every table is Yang et al. 2013's Table 1, the rotation's transition its §5 (CAM3, a 1 bar N₂
//! and H₂O aquaplanet with no CO₂ about a 3,400 K blackbody, a 50 m slab, 2 R⊕, 1.4 g⊕ and a 60 d
//! orbit), its M-star track only: a world locked in an M dwarf's habitable zone is the case. Its
//! dynamical-ocean runs (CCSM3, 18 K warmer at 1,600 W m⁻² with a ridge) are not read: plan 14
//! models no ocean heat transport. The rotation's ramp between its sourced ends, the flux's and
//! the pressure's tapers, and the deck held whole below their 1 bar are P14.T24.j's leans
//! (2026-10-30); so are the table held beyond its rows, the albedo above 2,200 W m⁻² and the
//! share above 1,600, extrapolations. Nothing here draws.

use super::{Column, ColumnClimate, SeasonalSpin, rotates};
use crate::math;
use crate::substance::phase::ice_ih_melting_temperature;
use crate::units::consts::{EARTH_RADIUS_M, SECONDS_PER_DAY, STEFAN_BOLTZMANN};
use crate::units::{Kelvin, Metres, Pascals, Seconds, WattsPerSquareMetre};
use crate::{GENERATOR_VERSION, GeneratorVersion};

#[cfg(test)]
mod tests;

/// The generator version from which a world locked 1:1 takes the substellar deck: 26, the
/// planetary bump after 25's (main, 2026-10-30).
///
/// It is P14.T24.j's one hold. Below it [`LockedClimateRules::in_force`] is
/// [`LockedClimateRules::Held`]: no column changes and every figure is version 24's and 25's bit
/// for bit. 26's bump deletes this hold, [`LockedClimateRules`] with its `Held` arm, the tests'
/// switch and its tripwire (`the_locked_climate_is_held_only_until_the_26_bump`).
pub(crate) const LOCKED_CLIMATE_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// Whether P14.T24.j's substellar deck is in force (its hold, [`LOCKED_CLIMATE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum LockedClimateRules {
    /// Version 25's and before: a locked world's column is a rotator's, T13.c's mean.
    Held,
    /// Version 26's: the deck on a locked world with open water ([`under_deck`]).
    Deck,
}

impl LockedClimateRules {
    /// The rules this generator version gives: the deck from [`LOCKED_CLIMATE_VERSION`], none
    /// below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_locked_climate`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= LOCKED_CLIMATE_VERSION {
            Self::Deck
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The locked climate's rules a test sets on its thread, `None` for the version's.
    static RULES: core::cell::Cell<Option<LockedClimateRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the locked climate's rules `rules` on this thread, for the tests to reach
/// the deck ahead of the bump; the version's rules are set back after, a panic in `run` included.
/// 26's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_locked_climate<R>(rules: LockedClimateRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<LockedClimateRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The deck's planetary albedo by stellar flux on a world locked 1:1, `(S, A_deck)` with S in
/// W m⁻²: Yang, Cowan and Abbot 2013 (ApJL 771, L45), Table 1, Group 3 (CAM3, spin:orbit 1:1,
/// about a 3,400 K M star), its "planetary albedo, primary contributed from clouds". The deck
/// brightens as the flux, and the substellar convection, grow ([`deck_albedo`]).
pub(crate) const DECK_ALBEDO: [(f64, f64); 7] = [
    (1_000.0, 0.42),
    (1_200.0, 0.45),
    (1_400.0, 0.47),
    (1_600.0, 0.49),
    (1_800.0, 0.50),
    (2_000.0, 0.51),
    (2_200.0, 0.53),
];

/// The global-mean greenhouse effect G of a world locked 1:1 by stellar flux, `(S, G)` with S in
/// W m⁻² and G in K: Yang, Cowan and Abbot 2013, Table 1, Group 3 (CAM3, spin:orbit 1:1), the
/// rows of [`DECK_ALBEDO`]. Read here at the fluxes of [`ROTATOR_GREENHOUSE_K`]
/// ([`locked_greenhouse_share`]). Their G is `T_srfem` − `T_ef` (their §3), the surface's and
/// the top's emission temperatures, which a grey column's `T_s` − `T_eq` is.
pub(crate) const LOCKED_GREENHOUSE_K: [(f64, f64); 4] = [
    (1_000.0, 7.3),
    (1_200.0, 6.3),
    (1_400.0, 5.6),
    (1_600.0, 6.1),
];

/// The global-mean greenhouse effect G of a rotator by stellar flux, `(S, G)` with S in W m⁻² and
/// G in K: Yang, Cowan and Abbot 2013, Table 1, Group 5 (CAM3, spin:orbit 6:1, the same world
/// turning six times an orbit), whose runs stop at 1,600 W m⁻². Over [`LOCKED_GREENHOUSE_K`] it is
/// the locked world's share of a rotator's greenhouse ([`locked_greenhouse_share`]).
pub(crate) const ROTATOR_GREENHOUSE_K: [(f64, f64); 4] = [
    (1_000.0, 16.9),
    (1_200.0, 21.9),
    (1_400.0, 27.7),
    (1_600.0, 32.1),
];

/// The orbital period below which a locked Earth-sized world's circulation is in the rapidly
/// rotating regime, days per Earth radius: 5 (Yang, Cowan and Abbot 2013, §5: "this regime
/// transition occurs at a 10-day orbit for a 2 R⊕ super-Earth, or a 5-day orbit for an
/// Earth-sized planet"), where the Rossby deformation radius falls below the planet's and the
/// convection, and its deck, concentrate. Their period runs at 2 R⊕ (Table 1, Group 6) read
/// albedos of 0.34 at 5 d, 0.36 at 10 d and 0.42 at 15 d against 0.45 at 20 d, so the deck is whole
/// from twice the transition ([`rotation_weight`]); the ramp's form between, log₂ in the period,
/// is P14.T24.j's lean (2026-10-30), not theirs.
pub(crate) const DECK_TRANSITION_DAYS_PER_EARTH_RADIUS: f64 = 5.0;

/// The stellar fluxes over which the deck comes in, W m⁻²: none at 800 and whole from 1,000,
/// Yang, Cowan and Abbot 2013's lowest run (Table 1, Group 3), below which no run reads it
/// ([`flux_weight`]). The taper below their table is P14.T24.j's lean (2026-10-30).
pub(crate) const DECK_FLUX_TAPER_W_PER_M2: (f64, f64) = (800.0, 1_000.0);

/// The surface pressures over which the deck goes out, Pa: whole to 10 bar, Yang, Cowan and
/// Abbot 2013's thickest run (Table 1, Group 6, 10 bar: albedo 0.43 and G 8.5 K at
/// 1,200 W m⁻²), and none from 30 bar, linearly in ln p between ([`pressure_weight`]). The taper
/// beyond their runs, and the deck held whole below their default and thinnest 1 bar, are
/// P14.T24.j's leans (2026-10-30).
pub(crate) const DECK_PRESSURE_TAPER: (Pascals, Pascals) =
    (Pascals::new(1.0e6), Pascals::new(3.0e6));

/// The width of the open water's ramp in the warmest climate above water's melting point, K: 2
/// (main's ruling of 2026-10-30): the deck needs open water under it, which GCMs with their sea
/// ice switched off never lose (Yang, Cowan and Abbot 2013's Group 1, Table 1 note d; Yang et al.
/// 2019, §2, "no snow or sea ice"), so a world it would freeze keeps the deck only to the open
/// water's edge ([`open_water_weight`]).
pub(crate) const OPEN_WATER_RAMP_K: f64 = 2.0;

/// The halvings of [`under_deck`]'s bisection for its weight: 48, a width of 3.6 × 10⁻¹⁵ of the
/// domain's weight, near an `f64`'s resolution at 1.
pub(crate) const DECK_HALVINGS: u32 = 48;

/// What the deck reads of a body beyond its column ([`under_deck`]): how it turns, the flux of its
/// hosts' light at it, and whether its surface holds liquid water.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DeckBody {
    /// How it turns under its seasonal host.
    pub(crate) spin: SeasonalSpin,
    /// The flux of its hosts' light at it, orbit-averaged, S.
    pub(crate) stellar_flux: WattsPerSquareMetre,
    /// Whether its surface holds open water, as published: its published pass's context on the
    /// record, the previous pass's at a pass (main's lean, 2026-10-30), which gives open water
    /// where its condensates list liquid water or, solved under the deck, its warmest zone melts
    /// its water.
    pub(crate) liquid_water: bool,
}

/// `table`'s value at `x`, linear between its rows and held beyond its first and its last; each
/// row's own at a row, to the bit.
#[must_use]
fn interpolated(table: &[(f64, f64)], x: f64) -> f64 {
    let (Some(&(x0, y0)), Some(&(x1, y1))) = (table.first(), table.last()) else {
        return f64::NAN;
    };
    if x.is_nan() || x <= x0 {
        return y0;
    }
    if x >= x1 {
        return y1;
    }
    table
        .windows(2)
        .find(|pair| x < pair[1].0)
        .map_or(y1, |pair| {
            let ((a, ya), (b, yb)) = (pair[0], pair[1]);
            ya + (yb - ya) * ((x - a) / (b - a))
        })
}

/// The deck's planetary albedo at the stellar flux `flux`, 0 to 1: [`DECK_ALBEDO`], linear in S
/// between its rows and held at its ends, 0.42 below 1,000 W m⁻² and 0.53 above 2,200.
#[must_use]
pub(crate) fn deck_albedo(flux: WattsPerSquareMetre) -> f64 {
    interpolated(&DECK_ALBEDO, flux.value())
}

/// The share s of a rotator's greenhouse a world locked 1:1 keeps at the stellar flux `flux`,
/// 0 to 1: Yang, Cowan and Abbot 2013's locked G over their 6:1 rotator's at the same flux,
/// [`LOCKED_GREENHOUSE_K`] over [`ROTATOR_GREENHOUSE_K`] at each row (0.432, 0.288, 0.202 and 0.190
/// at 1,000–1,600 W m⁻²), linear in S between the rows and held at their ends.
///
/// The night's inversion makes the outgoing flux follow the surface on the locked world, so its
/// greenhouse is small and flat in the flux (5.6–7.3 K over 1,000–1,600), where the rotator's
/// grows with its water vapour.
#[must_use]
pub(crate) fn locked_greenhouse_share(flux: WattsPerSquareMetre) -> f64 {
    let shares: [(f64, f64); LOCKED_GREENHOUSE_K.len()] = core::array::from_fn(|i| {
        let ((s, locked), (_, rotator)) = (LOCKED_GREENHOUSE_K[i], ROTATOR_GREENHOUSE_K[i]);
        (s, locked / rotator)
    });
    interpolated(&shares, flux.value())
}

/// The rotation regime's weight r of a world of radius `radius` turning once in
/// `rotation_period`, 0 to 1: clamp(log₂(P ÷ `P_t`), 0, 1) with `P_t` =
/// [`DECK_TRANSITION_DAYS_PER_EARTH_RADIUS`] days × R ÷ R⊕, none in the rapidly rotating regime
/// and whole from twice its transition (Yang, Cowan and Abbot 2013, §5 and Table 1's period runs).
///
/// A period that is not a number, or a radius that is not positive, takes none; a world that
/// does not turn, whose period is infinite, takes the whole.
#[must_use]
pub(crate) fn rotation_weight(rotation_period: Seconds, radius: Metres) -> f64 {
    let transition =
        DECK_TRANSITION_DAYS_PER_EARTH_RADIUS * SECONDS_PER_DAY * radius.value() / EARTH_RADIUS_M;
    let period = rotation_period.value();
    if !(transition > 0.0 && period > 0.0) {
        return 0.0;
    }
    math::log2(period / transition).clamp(0.0, 1.0)
}

/// The flux's weight `f_S` at the stellar flux `flux`, 0 to 1: (S − 800) ÷ 200 held to [0, 1]
/// ([`DECK_FLUX_TAPER_W_PER_M2`]), none for a flux that is not a number.
#[must_use]
pub(crate) fn flux_weight(flux: WattsPerSquareMetre) -> f64 {
    let (none, whole) = DECK_FLUX_TAPER_W_PER_M2;
    let s = flux.value();
    if s.is_nan() {
        return 0.0;
    }
    ((s - none) / (whole - none)).clamp(0.0, 1.0)
}

/// The pressure's weight `f_p` at the surface pressure `pressure`, 0 to 1:
/// 1 − ln(`p_s` ÷ 10 bar) ÷ ln 3 held to [0, 1] ([`DECK_PRESSURE_TAPER`]). None with no air, or a
/// pressure that is not a number: the deck is the air's.
#[must_use]
pub(crate) fn pressure_weight(pressure: Pascals) -> f64 {
    let (whole, none) = DECK_PRESSURE_TAPER;
    let p = pressure.value();
    if p.is_nan() || p <= 0.0 {
        return 0.0;
    }
    (1.0 - math::ln(p / whole.value()) / math::ln(none.value() / whole.value())).clamp(0.0, 1.0)
}

/// The open water's weight `f_open` of a climate whose warmest is `warmest` over water melting at
/// `melting`, 0 to 1: (`T_max` − `T_m`) ÷ 2 K held to [0, 1] ([`OPEN_WATER_RAMP_K`]), none for a
/// warmest that is not a number.
#[must_use]
pub(crate) fn open_water_weight(warmest: Kelvin, melting: Kelvin) -> f64 {
    let excess = warmest.value() - melting.value();
    if excess.is_nan() {
        return 0.0;
    }
    (excess / OPEN_WATER_RAMP_K).clamp(0.0, 1.0)
}

/// The weight of the deck's domain, r `f_S` `f_p`, 0 to 1, before its open water, of a world of
/// spin `spin` and radius `radius` under the flux `stellar_flux` at the surface pressure
/// `pressure`: none on a world that turns under its seasonal host, and under the deck's hold.
#[must_use]
fn domain_of(
    spin: &SeasonalSpin,
    radius: Metres,
    stellar_flux: WattsPerSquareMetre,
    pressure: Pascals,
) -> f64 {
    if LockedClimateRules::in_force() == LockedClimateRules::Held || rotates(spin) {
        return 0.0;
    }
    rotation_weight(spin.rotation_period, radius)
        * flux_weight(stellar_flux)
        * pressure_weight(pressure)
}

/// The weight of the deck's domain on the column `column` of the body `body`, r `f_S` `f_p`, 0 to
/// 1, before its open water ([`domain_of`]): at the column's radius and surface pressure.
#[must_use]
pub(crate) fn domain_weight(column: &Column, body: &DeckBody) -> f64 {
    domain_of(
        &body.spin,
        column.radius,
        body.stellar_flux,
        column.surface_pressure,
    )
}

/// Whether the deck reads the open water of a world of spin `spin` and radius `radius` under
/// the flux `stellar_flux` at the surface pressure `pressure`: where its domain's weight is
/// positive ([`domain_of`]). A pass's context carries its open water to the next only
/// where it does (`PassContext::with_open_water`), so that no other body's passes read it.
#[must_use]
pub(crate) fn reads_open_water(
    spin: &SeasonalSpin,
    radius: Metres,
    stellar_flux: WattsPerSquareMetre,
    pressure: Pascals,
) -> bool {
    domain_of(spin, radius, stellar_flux, pressure) > 0.0
}

/// The column `column` under the deck at the weight `weight`, 0 to 1, at the stellar flux `flux`:
/// its equilibrium temperature at the albedo `A_w` = A + w max(0, `A_deck` − A), A = 1 −
/// 4σ`T_eq`⁴ ÷ S the albedo it is at, and its surface temperature `T_eq,w` + G (1 − w (1 − s)),
/// G = `T_s` − `T_eq`; the rest of the column as it is. The column itself at no weight, to the
/// bit.
#[must_use]
pub(crate) fn decked_column(column: &Column, weight: f64, flux: WattsPerSquareMetre) -> Column {
    let s = flux.value();
    let t_eq = column.equilibrium_temperature.value();
    let absorbed = 4.0 * STEFAN_BOLTZMANN * math::powi(t_eq, 4) / s;
    if !(weight > 0.0 && s > 0.0 && absorbed > 0.0 && absorbed.is_finite()) {
        return *column;
    }
    // The share of the light the column absorbs, 1 − A, and what the deck takes of it.
    let brightening = (deck_albedo(flux) - (1.0 - absorbed)).max(0.0);
    let ratio = (absorbed - weight * brightening) / absorbed;
    let equilibrium = t_eq * math::powf(ratio, 0.25);
    let greenhouse = column.surface_temperature.value() - t_eq;
    let kept = 1.0 - weight * (1.0 - locked_greenhouse_share(flux));
    Column {
        surface_temperature: Kelvin::new(equilibrium + greenhouse * kept),
        equilibrium_temperature: Kelvin::new(equilibrium),
        ..*column
    }
}

/// A column under the deck, its climate and its weight ([`under_deck`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Decked {
    /// The column under the deck: the column given where the weight is none.
    pub(super) column: Column,
    /// Its climate.
    pub(super) climate: ColumnClimate,
    /// The deck's weight w, 0 to 1.
    pub(super) weight: f64,
}

/// The column `column` of the body `body` under its substellar deck, with its climate by
/// `climate_of` and the deck's weight w = r `f_S` `f_p` `f_open` (the [module](self)'s rule).
///
/// `f_open` is none where the surface holds no liquid water, and otherwise the ramp of the
/// warmest climate on the changed column above water's melting point at the column's pressure
/// ([`open_water_weight`]; ice Ih's melting curve, IAPWS R14-08's eq. 1, 0.22 K under the triple
/// point at 30 bar, [`ice_ih_melting_temperature`]), so that w = `w_d` `f_open`(w), `w_d` the
/// domain's weight ([`domain_weight`]). The ramp falls as w rises, since the deck cools the column,
/// so w − `w_d` `f_open`(w) rises from below 0 at no weight to above it at the domain's, and its
/// root is unique: w is `w_d` where the deck there leaves the ramp whole, none where the column
/// without it lies wholly below the ramp, and otherwise the lower end of [`DECK_HALVINGS`]
/// halvings of [0, `w_d`], whose warmest climate lies above water's melting point.
///
/// Outside the domain, or over no liquid water, the column is the column given, to the bit, and
/// `climate_of` runs once. It allocates nothing.
#[must_use]
pub(super) fn under_deck(
    column: &Column,
    body: &DeckBody,
    climate_of: impl Fn(&Column) -> ColumnClimate,
) -> Decked {
    let as_built = |column: &Column| Decked {
        column: *column,
        climate: climate_of(column),
        weight: 0.0,
    };
    let domain = domain_weight(column, body);
    if !(body.liquid_water && domain > 0.0) {
        return as_built(column);
    }
    let melting = ice_ih_melting_temperature(column.surface_pressure);
    let at = |weight: f64| {
        let decked = decked_column(column, weight, body.stellar_flux);
        let climate = climate_of(&decked);
        let open = open_water_weight(climate.range.1, melting);
        (
            Decked {
                column: decked,
                climate,
                weight,
            },
            open,
        )
    };
    let (whole, open) = at(domain);
    if open >= 1.0 {
        return whole;
    }
    let (none, open) = at(0.0);
    if open <= 0.0 {
        return none;
    }
    // w − w_d f_open(w) is below 0 at `low` and not below it at `high`.
    let (mut low, mut high) = (none, domain);
    for _ in 0..DECK_HALVINGS {
        let mid = f64::midpoint(low.weight, high);
        let (decked, open) = at(mid);
        if mid < domain * open {
            low = decked;
        } else {
            high = mid;
        }
    }
    low
}
